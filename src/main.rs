mod abis;
mod cli;
mod error;
mod fivemat;
mod harness;

mod procgen;
mod report;

use abis::*;
use camino::{Utf8Path, Utf8PathBuf};
use error::*;
use harness::*;
use report::*;
use std::error::Error;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::sync::OnceCell;
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
}

#[derive(Debug, thiserror::Error)]
#[error("some tests failed")]
pub struct TestsFailed {}

#[derive(Default)]
pub struct TestHarness {
    tests: SortedMap<TestId, Arc<Test>>,
    abi_impls: SortedMap<AbiImplId, Arc<dyn AbiImpl + Send + Sync>>,
    test_with_abi_impls: Mutex<SortedMap<(TestId, AbiImplId), Arc<OnceCell<Arc<TestForAbi>>>>>,
    sources: Mutex<SortedMap<Utf8PathBuf, Arc<OnceCell<()>>>>,
    static_libs: Mutex<SortedMap<String, Arc<OnceCell<String>>>>,
}

impl TestHarness {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_abi_impl<A: AbiImpl + Send + Sync + 'static>(&mut self, id: AbiImplId, abi_impl: A) {
        let old = self.abi_impls.insert(id.clone(), Arc::new(abi_impl));
        assert!(old.is_none(), "duplicate abi impl id: {}", id);
    }
    pub fn abi_by_test_key(
        &self,
        key: &TestKey,
        call_side: CallSide,
    ) -> Arc<dyn AbiImpl + Send + Sync> {
        let abi_id = key.abi_id(call_side);
        self.abi_impls[abi_id].clone()
    }

    pub fn set_tests(&mut self, tests: Vec<Test>) {
        for test in tests {
            let id = test.name.clone();
            let old = self.tests.insert(id.clone(), Arc::new(test));
            assert!(old.is_none(), "duplicate test id: {}", id);
        }
    }
    pub async fn test_with_abi_impl(
        &self,
        test: &Test,
        abi_id: AbiImplId,
    ) -> Result<Arc<TestForAbi>, GenerateError> {
        let test_id = test.name.clone();
        let abi_impl = self.abi_impls[&abi_id].clone();
        // Briefly lock this map to insert/acquire a OnceCell and then release the lock
        let once = self
            .test_with_abi_impls
            .lock()
            .unwrap()
            .entry((test_id, abi_id.clone()))
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        // Either acquire the cached result, or make it
        let output = once
            .get_or_try_init(|| test.for_abi(&*abi_impl))
            .await?
            .clone();
        Ok(output)
    }
    pub fn get_test_rules(&self, test_key: &TestKey) -> TestRules {
        let caller = self.abi_impls[&test_key.caller].clone();
        let callee = self.abi_impls[&test_key.callee].clone();

        get_test_rules(test_key, &*caller, &*callee)
    }

    pub fn spawn_test(
        self: Arc<Self>,
        rt: &tokio::runtime::Runtime,
        rules: TestRules,
        test_key: TestKey,
        out_dir: Utf8PathBuf,
    ) -> tokio::task::JoinHandle<TestRunResults> {
        let harness = self.clone();
        rt.spawn(async move { harness.do_test(test_key, rules, out_dir).await })
    }

    /// Generate, Compile, Link, Load, and Run this test.
    pub async fn do_test(
        &self,
        test_key: TestKey,
        test_rules: TestRules,
        out_dir: Utf8PathBuf,
    ) -> TestRunResults {
        use TestRunMode::*;

        let mut run_results = TestRunResults::default();
        if test_rules.run <= Skip {
            return run_results;
        }

        run_results.ran_to = Generate;
        run_results.source = Some(self.generate_test(&test_key).await);
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
        run_results.build = Some(self.build_test(&test_key, source, &out_dir).await);
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
        run_results.link = Some(self.link_dynamic_lib(&test_key, build, &out_dir).await);
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
        run_results.run = Some(self.run_dynamic_test(&test_key, link).await);
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
        run_results.check = Some(self.check_test(&test_key, run).await);

        run_results
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    eprintln!("starting!");
    let cfg = cli::make_app();
    eprintln!("parsed cli!");
    // Before doing anything, regenerate the procgen tests, if needed.
    // procgen::procgen_tests(cfg.procgen_tests);
    eprintln!("generated tests!");
    let out_dir = init_dirs()?;

    let mut harness = TestHarness::new();

    harness.add_abi_impl(
        ABI_IMPL_RUSTC.to_owned(),
        abis::RustcAbiImpl::new(&cfg, None),
    );
    /*
    runner.add_abi_impl(
        ABI_IMPL_CC.to_owned(),
        abis::CcAbiImpl::new(&cfg, ABI_IMPL_CC),
    );
    runner.add_abi_impl(
        ABI_IMPL_GCC.to_owned(),
        abis::CcAbiImpl::new(&cfg, ABI_IMPL_GCC),
    );
    runner.add_abi_impl(
        ABI_IMPL_CLANG.to_owned(),
        abis::CcAbiImpl::new(&cfg, ABI_IMPL_CLANG),
    );
    runner.add_abi_impl(
        ABI_IMPL_MSVC.to_owned(),
        abis::CcAbiImpl::new(&cfg, ABI_IMPL_MSVC),
    );
    */
    for (name, path) in &cfg.rustc_codegen_backends {
        harness.add_abi_impl(
            name.to_owned(),
            abis::RustcAbiImpl::new(&cfg, Some(path.to_owned())),
        );
    }
    eprintln!("configured ABIs!");

    // Grab all the tests
    let tests = read_tests(cfg.val_generator)?;
    harness.set_tests(tests);
    eprintln!("got tests!");
    let harness = Arc::new(harness);
    // Run the tests
    use TestConclusion::*;
    let rt = tokio::runtime::Runtime::new().expect("failed to init tokio runtime");
    let _handle = rt.enter();
    // This is written as nested iterator adaptors so that it can maybe be changed to use
    // rayon's par_iter, but currently the code isn't properly threadsafe due to races on
    // the filesystem when setting up the various output dirs :(
    let tasks = harness
        .tests
        .values()
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
                                    val_writer: WriteImpl::HarnessCallback,
                                    val_generator: cfg.val_generator,
                                },
                            };
                            let rules = harness.get_test_rules(&test_key);
                            let task = harness.clone().spawn_test(
                                &rt,
                                rules.clone(),
                                test_key.clone(),
                                out_dir.clone(),
                            );

                            // FIXME: we can make everything parallel by immediately returning
                            // and making the following code happen in subsequent pass. For now
                            // let's stay single-threaded to do things one step at a time.
                            // Some((test_key, rules, task))

                            let results = rt.block_on(task).expect("failed to join task");
                            Some(report_test(test_key, rules, results))
                        })
                        .collect()
                })
                .collect()
        })
        .collect::<Vec<_>>();

    // Join on all the tasks, and compute their results
    let reports = tasks.into_iter().map(|report| report).collect::<Vec<_>>();

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
        do_thing(&harness, &rt, &out_dir, &full_report);
        Err(TestsFailed {})?;
    }
    Ok(())
}

fn do_thing(
    harness: &Arc<TestHarness>,
    rt: &tokio::runtime::Runtime,
    out_dir: &Utf8Path,
    reports: &FullReport,
) {
    eprintln!("rerunning failures");
    for report in &reports.tests {
        let Some(check) = report.results.check.as_ref() else {
            continue;
        };
        for func_result in &check.subtest_checks {
            let Err(failure) = func_result else {
                continue;
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
            test_key.options.val_writer = WriteImpl::Print;
            let rules = report.rules.clone();
            eprintln!("rerunning {}", harness.base_id(&test_key, None, "::"));
            let task = harness
                .clone()
                .spawn_test(rt, rules, test_key, out_dir.to_owned());
            let results = rt.block_on(task).expect("failed to join task");
            let source = results.source.unwrap().unwrap();
            eprintln!("  caller: {}", source.caller_src);
            eprintln!("  callee: {}", source.callee_src);
        }
    }
}
