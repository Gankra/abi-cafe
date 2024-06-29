mod abis;
mod cli;
mod error;
mod files;
mod fivemat;
mod harness;
mod log;

mod procgen;
mod report;

use abis::*;
use error::*;
use files::Paths;
use harness::*;
use report::*;
use std::error::Error;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::{debug, info};
use vals::ValueGeneratorKind;

pub type SortedMap<K, V> = std::collections::BTreeMap<K, V>;

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
impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            OutputFormat::Human => "human",
            OutputFormat::Json => "json",
            OutputFormat::RustcJson => "rustc-json",
        };
        string.fmt(f)
    }
}
impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = match s {
            "human" => OutputFormat::Human,
            "json" => OutputFormat::Json,
            "rustc-json" => OutputFormat::RustcJson,
            _ => return Err(format!("unknown output format: {s}")),
        };
        Ok(val)
    }
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
    pub val_generator: ValueGeneratorKind,
    pub write_impl: WriteImpl,
    pub minimizing_write_impl: WriteImpl,
    pub paths: Paths,
}

#[derive(Debug, thiserror::Error)]
#[error("some tests failed")]
pub struct TestsFailed {}

fn main() -> Result<(), Box<dyn Error>> {
    let cfg = cli::make_app();
    debug!("parsed cli!");
    cfg.paths.init_dirs()?;

    let rt = tokio::runtime::Runtime::new().expect("failed to init tokio runtime");
    let _handle = rt.enter();

    // Grab all the tests
    let test_sources = harness::find_tests(&cfg.paths)?;
    let read_tasks = test_sources
        .into_iter()
        .map(|(test, test_file)| harness::spawn_read_test(&rt, test, test_file));

    // We could async pipeline this harder but it's nice to know all the tests upfront
    let tests = read_tasks
        .filter_map(|task| rt.block_on(task).expect("failed to join on task").ok())
        .map(|test| (test.name.clone(), test))
        .collect();
    let mut harness = TestHarness::new(tests, cfg.paths.clone());

    harness.add_abi_impl(
        ABI_IMPL_RUSTC.to_owned(),
        abis::RustcAbiImpl::new(&cfg, None),
    );
    harness.add_abi_impl(
        ABI_IMPL_CC.to_owned(),
        abis::CcAbiImpl::new(&cfg, ABI_IMPL_CC),
    );
    harness.add_abi_impl(
        ABI_IMPL_GCC.to_owned(),
        abis::CcAbiImpl::new(&cfg, ABI_IMPL_GCC),
    );
    harness.add_abi_impl(
        ABI_IMPL_CLANG.to_owned(),
        abis::CcAbiImpl::new(&cfg, ABI_IMPL_CLANG),
    );
    harness.add_abi_impl(
        ABI_IMPL_MSVC.to_owned(),
        abis::CcAbiImpl::new(&cfg, ABI_IMPL_MSVC),
    );

    for (name, path) in &cfg.rustc_codegen_backends {
        harness.add_abi_impl(
            name.to_owned(),
            abis::RustcAbiImpl::new(&cfg, Some(path.to_owned())),
        );
    }

    debug!("configured ABIs!");
    let harness = Arc::new(harness);

    debug!("loaded tests!");
    // Run the tests
    use TestConclusion::*;

    let tasks = harness
        .all_tests()
        .into_iter()
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

                            // Run the test!
                            let test_key = TestKey {
                                test: test.name.to_owned(),
                                caller: caller_id.to_owned(),
                                callee: callee_id.to_owned(),
                                options: TestOptions {
                                    convention: *convention,
                                    functions: FunctionSelector::All,
                                    val_writer: cfg.write_impl,
                                    val_generator: cfg.val_generator,
                                },
                            };
                            let rules = harness.get_test_rules(&test_key);
                            let task =
                                harness
                                    .clone()
                                    .spawn_test(&rt, rules.clone(), test_key.clone());

                            Some(task)
                        })
                        .collect()
                })
                .collect()
        })
        .collect::<Vec<_>>();

    // Join on all the tasks, and compute their results
    let reports = tasks
        .into_iter()
        .map(|task| {
            let results = rt.block_on(task).expect("failed to join task");
            report_test(results)
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
        // FIXME: put in a bunch of metadata here?
        config: TestConfig {},
        tests: reports,
    };

    let mut output = std::io::stdout();
    match cfg.output_format {
        OutputFormat::Human => full_report.print_human(&harness, &mut output).unwrap(),
        OutputFormat::Json => full_report.print_json(&harness, &mut output).unwrap(),
        OutputFormat::RustcJson => full_report.print_rustc_json(&harness, &mut output).unwrap(),
    }

    if full_report.failed() {
        generate_minimized_failures(&cfg, &harness, &rt, &full_report);
        Err(TestsFailed {})?;
    }
    Ok(())
}

fn generate_minimized_failures(
    cfg: &Config,
    harness: &Arc<TestHarness>,
    rt: &tokio::runtime::Runtime,
    reports: &FullReport,
) {
    info!("rerunning failures");
    let tasks = reports.tests.iter().flat_map(|report| {
        let Some(check) = report.results.check.as_ref() else {
            return vec![];
        };
        check
            .subtest_checks
            .iter()
            .filter_map(|func_result| {
                let Err(failure) = func_result else {
                    return None;
                };
                let functions = match *failure {
                    CheckFailure::ArgCountMismatch { func_idx, .. } => FunctionSelector::One {
                        idx: func_idx,
                        args: ArgSelector::All,
                    },
                    CheckFailure::ValCountMismatch {
                        func_idx, arg_idx, ..
                    } => FunctionSelector::One {
                        idx: func_idx,
                        args: ArgSelector::One {
                            idx: arg_idx,
                            vals: ValSelector::All,
                        },
                    },
                    CheckFailure::ValMismatch {
                        func_idx,
                        arg_idx,
                        val_idx,
                        ..
                    }
                    | CheckFailure::TagMismatch {
                        func_idx,
                        arg_idx,
                        val_idx,
                        ..
                    } => FunctionSelector::One {
                        idx: func_idx,
                        args: ArgSelector::One {
                            idx: arg_idx,
                            vals: ValSelector::One { idx: val_idx },
                        },
                    },
                };

                let mut test_key = report.key.clone();
                test_key.options.functions = functions;
                test_key.options.val_writer = cfg.minimizing_write_impl;
                let mut rules = report.rules.clone();
                rules.run = TestRunMode::Generate;

                let task = harness.clone().spawn_test(rt, rules, test_key);
                Some(task)
            })
            .collect()
    });

    let _results = tasks
        .into_iter()
        .map(|task| rt.block_on(task).expect("failed to join task"))
        .collect::<Vec<_>>();
}
