mod abis;
mod cli;
mod error;
mod fivemat;
mod harness;

// mod procgen;
mod report;

use abis::*;
use error::*;
use harness::*;
use report::*;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Slurps up details of how this crate was compiled, which we can use
/// to better compile the actual tests since we're currently compiling them on
/// the same platform with the same toolchains!
pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Human,
    Json,
    RustcJson,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub output_format: OutputFormat,
    pub procgen_tests: bool,
    pub run_conventions: Vec<CallingConvention>,
    pub run_impls: Vec<String>,
    pub run_pairs: Vec<(String, String)>,
    pub run_tests: Vec<String>,
    pub rustc_codegen_backends: Vec<(String, String)>,
}

#[derive(Debug, thiserror::Error)]
#[error("some tests failed")]
pub struct TestsFailed {}

fn main() -> Result<(), Box<dyn Error>> {
    eprintln!("starting!");
    let cfg = cli::make_app();
    eprintln!("parsed cli!");
    // Before doing anything, regenerate the procgen tests, if needed.
    // TODO: procgen::procgen_tests(cfg.procgen_tests);
    eprintln!("generated tests!");

    let out_dir = PathBuf::from("target/temp/");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::remove_dir_all(&out_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    // Set up env vars for CC
    env::set_var("OUT_DIR", &out_dir);
    env::set_var("HOST", built_info::HOST);
    env::set_var("TARGET", built_info::TARGET);
    env::set_var("OPT_LEVEL", "0");

    let mut abi_impls: HashMap<&str, Box<dyn AbiImpl + Send + Sync>> = HashMap::new();
    abi_impls.insert(
        ABI_IMPL_RUSTC,
        Box::new(abis::RustcAbiImpl::new(&cfg, None)),
    );
    /*
    abi_impls.insert(
        ABI_IMPL_CC,
        Box::new(abis::CcAbiImpl::new(&cfg, ABI_IMPL_CC)),
    );
    abi_impls.insert(
        ABI_IMPL_GCC,
        Box::new(abis::CcAbiImpl::new(&cfg, ABI_IMPL_GCC)),
    );
    abi_impls.insert(
        ABI_IMPL_CLANG,
        Box::new(abis::CcAbiImpl::new(&cfg, ABI_IMPL_CLANG)),
    );
    abi_impls.insert(
        ABI_IMPL_MSVC,
        Box::new(abis::CcAbiImpl::new(&cfg, ABI_IMPL_MSVC)),
    );
    */
    for &(ref name, ref path) in &cfg.rustc_codegen_backends {
        abi_impls.insert(
            name,
            Box::new(abis::RustcAbiImpl::new(&cfg, Some(path.to_owned()))),
        );
    }
    eprintln!("configured ABIs!");

    // Grab all the tests
    let tests = read_tests()?;
    eprintln!("got tests!");

    // Run the tests
    use TestConclusion::*;

    // This is written as nested iterator adaptors so that it can maybe be changed to use
    // rayon's par_iter, but currently the code isn't properly threadsafe due to races on
    // the filesystem when setting up the various output dirs :(
    let reports = tests
        .iter()
        .flat_map(|test| {
            // If the cli has test filters, apply those
            if !cfg.run_tests.is_empty() && !cfg.run_tests.contains(&test.name) {
                return Vec::new();
            }
            cfg.run_conventions
                .iter()
                .flat_map(|convention| {
                    if !test.has_convention(*convention) {
                        // Don't bother with a convention if the test doesn't use it.
                        return Vec::new();
                    }
                    // Create versions of the test for each "X calls Y" pair we care about.
                    cfg.run_pairs
                        .iter()
                        .filter_map(|(caller_id, callee_id)| {
                            if !cfg.run_impls.is_empty()
                                && !cfg.run_impls.iter().any(|x| x == caller_id)
                                && !cfg.run_impls.iter().any(|x| &**x == callee_id)
                            {
                                return None;
                            }
                            let caller =
                                &**abi_impls.get(&**caller_id).expect("invalid id for caller!");
                            let callee =
                                &**abi_impls.get(&**callee_id).expect("invalid id for callee!");

                            let convention_name = convention.name();

                            // Run the test!
                            let test_key = TestKey {
                                test_name: test.name.to_owned(),
                                convention: convention_name.to_owned(),
                                caller_id: caller_id.to_owned(),
                                callee_id: callee_id.to_owned(),
                                caller_variant: test.for_abi(caller).unwrap(),
                                callee_variant: test.for_abi(callee).unwrap(),
                            };
                            let rules = get_test_rules(&test_key, caller, callee);
                            let results = do_test(
                                test,
                                &test_key,
                                &rules,
                                *convention,
                                caller,
                                callee,
                                &out_dir,
                            );
                            let report = report_test(test_key, rules, results);
                            Some(report)
                        })
                        .collect()
                })
                .collect()
        })
        .collect::<Vec<_>>();

    // Compute the final report
    let mut num_tests = 0;
    let mut num_passed = 0;
    let mut num_busted = 0;
    let mut num_failed = 0;
    let mut num_skipped = 0;
    for report in &reports {
        num_tests += 1;
        match report.conclusion {
            Busted => num_busted += 1,
            Skipped => num_skipped += 1,
            Passed => num_passed += 1,
            Failed => num_failed += 1,
        }
    }

    let full_report = FullReport {
        summary: TestSummary {
            num_tests,
            num_passed,
            num_busted,
            num_failed,
            num_skipped,
        },
        // TODO: put in a bunch of metadata here?
        config: TestConfig {},
        tests: reports,
    };

    let mut output = std::io::stdout();
    match cfg.output_format {
        OutputFormat::Human => full_report.print_human(&mut output).unwrap(),
        OutputFormat::Json => full_report.print_json(&mut output).unwrap(),
        OutputFormat::RustcJson => full_report.print_rustc_json(&mut output).unwrap(),
    }

    if full_report.failed() {
        Err(TestsFailed {})?;
    }
    Ok(())
}

/// Generate, Compile, Link, Load, and Run this test.
fn do_test(
    test: &Test,
    test_key: &TestKey,
    test_rules: &TestRules,
    convention: CallingConvention,
    caller: &dyn AbiImpl,
    callee: &dyn AbiImpl,
    _out_dir: &Path,
) -> TestRunResults {
    use TestRunMode::*;

    let mut run_results = TestRunResults::default();
    if test_rules.run <= Skip {
        return run_results;
    }

    run_results.ran_to = Generate;
    run_results.source = Some(generate_test_src(
        test, test_key, convention, caller, callee,
    ));
    let source = match run_results.source.as_ref().unwrap() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to generate source: {}", e);
            return run_results;
        }
    };
    if test_rules.run <= Generate {
        return run_results;
    }

    run_results.ran_to = Build;
    run_results.build = Some(build_test(test, test_key, caller, callee, source));
    let build = match run_results.build.as_ref().unwrap() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to build test: {}", e);
            return run_results;
        }
    };
    if test_rules.run <= Build {
        return run_results;
    }

    run_results.ran_to = Link;
    run_results.link = Some(link_test(test, test_key, build));
    let link = match run_results.link.as_ref().unwrap() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to link test: {}", e);
            return run_results;
        }
    };
    if test_rules.run <= Link {
        return run_results;
    }

    run_results.ran_to = Run;
    run_results.run = Some(run_dynamic_test(test, test_key, link));
    let run = match run_results.run.as_ref().unwrap() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to run test: {}", e);
            return run_results;
        }
    };
    if test_rules.run <= Run {
        return run_results;
    }

    run_results.ran_to = Check;
    run_results.check = Some(check_test(test, test_key, run));

    run_results
}
