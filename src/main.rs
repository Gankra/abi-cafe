mod abis;
mod cli;
mod error;
mod fivemat;
mod harness;

// mod procgen;
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

pub type SortedMap<K, V> = std::collections::BTreeMap<K, V>;
pub type AbiImplId = String;
pub type TestId = String;

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
}

#[derive(Debug, thiserror::Error)]
#[error("some tests failed")]
pub struct TestsFailed {}

#[derive(Default)]
pub struct TestRunner {
    tests: SortedMap<TestId, Arc<Test>>,
    abi_impls: SortedMap<AbiImplId, Arc<dyn AbiImpl + Send + Sync>>,
    test_with_abi_impls: Mutex<SortedMap<(TestId, AbiImplId), Arc<OnceCell<Arc<TestForAbi>>>>>,
    sources: Mutex<SortedMap<Utf8PathBuf, Arc<OnceCell<()>>>>,
    static_libs: Mutex<SortedMap<String, Arc<OnceCell<String>>>>,
}

impl TestRunner {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_abi_impl<A: AbiImpl + Send + Sync + 'static>(&mut self, id: AbiImplId, abi_impl: A) {
        let old = self.abi_impls.insert(id.clone(), Arc::new(abi_impl));
        assert!(old.is_none(), "duplicate abi impl id: {}", id);
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
    pub async fn generate_src(
        &self,
        test_id: TestId,
        abi_id: AbiImplId,
        call_side: CallSide,
        options: TestOptions,
    ) -> Result<Utf8PathBuf, GenerateError> {
        let abi_impl = self.abi_impls[&abi_id].clone();
        let src_path = harness::src_path(&test_id, &abi_id, &*abi_impl, call_side, &options);
        let test = self.tests[&test_id].clone();
        let test_with_abi = self.test_with_abi_impl(&test, abi_id).await?;
        // Briefly lock this map to insert/acquire a OnceCell and then release the lock
        let once = self
            .sources
            .lock()
            .unwrap()
            .entry(src_path.clone())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        // Either acquire the cached result, or make it
        let _ = once
            .get_or_try_init(|| {
                generate_src(&src_path, abi_impl, test_with_abi, call_side, options)
            })
            .await?;
        Ok(src_path)
    }
    pub async fn build_lib(
        &self,
        test_id: TestId,
        abi_id: AbiImplId,
        call_side: CallSide,
        options: TestOptions,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
    ) -> Result<String, BuildError> {
        let abi_impl = self.abi_impls[&abi_id].clone();
        let lib_name = harness::lib_name(&test_id, &abi_id, call_side, &options);
        // Briefly lock this map to insert/acquire a OnceCell and then release the lock
        let once = self
            .static_libs
            .lock()
            .unwrap()
            .entry(lib_name.clone())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        // Either acquire the cached result, or make it
        let real_lib_name = once
            .get_or_try_init(|| compile_lib(&src_path, abi_impl, call_side, out_dir, &lib_name))
            .await?
            .clone();
        Ok(real_lib_name)
    }
    pub fn get_test_rules(&self, test_key: &TestKey) -> TestRules {
        let caller = self.abi_impls[&test_key.caller].clone();
        let callee = self.abi_impls[&test_key.callee].clone();

        get_test_rules(test_key, &*caller, &*callee)
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
        run_results.link = Some(self.link_test(&test_key, build, &out_dir).await);
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
    pub async fn generate_test(&self, key: &TestKey) -> Result<GenerateOutput, GenerateError> {
        let full_test_name = full_test_name(key);
        eprintln!("generating  {full_test_name}");

        // FIXME: these two could be done concurrently
        let caller_src = self
            .generate_src(
                key.test.clone(),
                key.caller.clone(),
                CallSide::Caller,
                key.options.clone(),
            )
            .await?;
        let callee_src = self
            .generate_src(
                key.test.clone(),
                key.callee.clone(),
                CallSide::Callee,
                key.options.clone(),
            )
            .await?;

        Ok(GenerateOutput {
            caller_src,
            callee_src,
        })
    }
    pub async fn build_test(
        &self,
        key: &TestKey,
        src: &GenerateOutput,
        out_dir: &Utf8Path,
    ) -> Result<BuildOutput, BuildError> {
        let full_test_name = full_test_name(key);
        eprintln!("compiling  {full_test_name}");

        // FIXME: these two could be done concurrently
        let caller_lib = self
            .build_lib(
                key.test.clone(),
                key.caller.clone(),
                CallSide::Caller,
                key.options.clone(),
                &src.caller_src,
                out_dir,
            )
            .await?;
        let callee_lib = self
            .build_lib(
                key.test.clone(),
                key.callee.clone(),
                CallSide::Callee,
                key.options.clone(),
                &src.callee_src,
                out_dir,
            )
            .await?;
        Ok(BuildOutput {
            caller_lib,
            callee_lib,
        })
    }
    pub async fn link_test(
        &self,
        key: &TestKey,
        build: &BuildOutput,
        out_dir: &Utf8Path,
    ) -> Result<LinkOutput, LinkError> {
        link_test(key, build, out_dir)
    }
    pub async fn run_dynamic_test(
        &self,
        key: &TestKey,
        test_dylib: &LinkOutput,
    ) -> Result<RunOutput, RunError> {
        let test = self.tests[&key.test].clone();
        let caller_impl = self
            .test_with_abi_impl(&test, key.caller.clone())
            .await
            .unwrap();
        let callee_impl = self
            .test_with_abi_impl(&test, key.callee.clone())
            .await
            .unwrap();
        run_dynamic_test(key, caller_impl, callee_impl, test_dylib)
    }
    pub async fn check_test(&self, key: &TestKey, results: &RunOutput) -> CheckOutput {
        let test = self.tests[&key.test].clone();
        let caller_impl = self
            .test_with_abi_impl(&test, key.caller.clone())
            .await
            .unwrap();
        let callee_impl = self
            .test_with_abi_impl(&test, key.callee.clone())
            .await
            .unwrap();
        check_test(key, caller_impl, callee_impl, results)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    eprintln!("starting!");
    let cfg = cli::make_app();
    eprintln!("parsed cli!");
    // Before doing anything, regenerate the procgen tests, if needed.
    // TODO: procgen::procgen_tests(cfg.procgen_tests);
    eprintln!("generated tests!");
    init_generate_dir()?;
    let out_dir = init_build_dir()?;

    let mut runner = TestRunner::new();

    runner.add_abi_impl(
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
    for &(ref name, ref path) in &cfg.rustc_codegen_backends {
        runner.add_abi_impl(
            name.to_owned(),
            abis::RustcAbiImpl::new(&cfg, Some(path.to_owned())),
        );
    }
    eprintln!("configured ABIs!");

    // Grab all the tests
    let tests = read_tests()?;
    runner.set_tests(tests);
    eprintln!("got tests!");
    let runner = Arc::new(runner);
    // Run the tests
    use TestConclusion::*;
    let rt = tokio::runtime::Runtime::new().expect("failed to init tokio runtime");
    let _handle = rt.enter();
    // This is written as nested iterator adaptors so that it can maybe be changed to use
    // rayon's par_iter, but currently the code isn't properly threadsafe due to races on
    // the filesystem when setting up the various output dirs :(
    let tasks = runner
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
                                    convention: convention.clone(),
                                },
                            };
                            let rules = runner.get_test_rules(&test_key);

                            let task = {
                                let runner = runner.clone();
                                let rules = rules.clone();
                                let test_key = test_key.clone();
                                let out_dir = out_dir.clone();
                                rt.spawn(
                                    async move { runner.do_test(test_key, rules, out_dir).await },
                                )
                            };
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
        OutputFormat::Human => full_report.print_human(&mut output).unwrap(),
        OutputFormat::Json => full_report.print_json(&mut output).unwrap(),
        OutputFormat::RustcJson => full_report.print_rustc_json(&mut output).unwrap(),
    }

    if full_report.failed() {
        Err(TestsFailed {})?;
    }
    Ok(())
}
