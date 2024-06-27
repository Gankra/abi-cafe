//! The test harness, which provides:
//!
//! 1. reading test .kdl files (kdl-script)
//! 2. generating impls of the tests
//! 3. building + linking the test impls together
//! 4. running the test impls
//! 5. checking the test results

use crate::{
    get_test_rules, vals::ValueGeneratorKind, AbiImpl, AbiImplId, ArgSelector, CallSide,
    FunctionSelector, GenerateError, SortedMap, Test, TestId, TestKey, TestOptions, TestRules,
    TestRunMode, TestRunResults, TestWithAbi, TestWithVals, ValSelector, WriteImpl,
};
use camino::Utf8PathBuf;
use std::{
    error::Error,
    sync::{Arc, Mutex},
};
use tokio::sync::OnceCell;
use tracing::warn;

mod build;
mod check;
mod generate;
mod read;
mod run;

use build::init_build_dir;
use generate::init_generate_dir;
pub use read::{find_tests, spawn_read_test};
pub use run::WriteBuffer;

pub type Memoized<K, V> = Mutex<SortedMap<K, Arc<OnceCell<V>>>>;

#[derive(Default)]
pub struct TestHarness {
    abi_impls: SortedMap<AbiImplId, Arc<dyn AbiImpl + Send + Sync>>,
    tests: SortedMap<TestId, Arc<Test>>,
    tests_with_vals: Memoized<(TestId, ValueGeneratorKind), Arc<TestWithVals>>,
    tests_with_abi_impl: Memoized<(TestId, ValueGeneratorKind, AbiImplId), Arc<TestWithAbi>>,
    generated_sources: Memoized<Utf8PathBuf, ()>,
    built_static_libs: Memoized<String, String>,
}

impl TestHarness {
    pub fn new(tests: SortedMap<TestId, Arc<Test>>) -> Self {
        Self {
            tests,
            ..Self::default()
        }
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
    pub fn all_tests(&self) -> Vec<Arc<Test>> {
        self.tests.values().cloned().collect()
    }
    pub fn test(&self, test: &TestId) -> Arc<Test> {
        self.tests[test].clone()
    }
    pub async fn test_with_vals(
        &self,
        test_id: &TestId,
        vals: ValueGeneratorKind,
    ) -> Result<Arc<TestWithVals>, GenerateError> {
        let test_id = test_id.clone();
        let test = self.test(&test_id);
        let once = self
            .tests_with_vals
            .lock()
            .unwrap()
            .entry((test_id, vals))
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        // Either acquire the cached result, or make it
        let output = once.get_or_try_init(|| test.with_vals(vals)).await?.clone();
        Ok(output)
    }
    pub async fn test_with_abi_impl(
        &self,
        test: Arc<TestWithVals>,
        abi_id: AbiImplId,
    ) -> Result<Arc<TestWithAbi>, GenerateError> {
        let test_id = test.name.clone();
        let vals = test.vals.generator_kind;
        let abi_impl = self.abi_impls[&abi_id].clone();
        // Briefly lock this map to insert/acquire a OnceCell and then release the lock
        let once = self
            .tests_with_abi_impl
            .lock()
            .unwrap()
            .entry((test_id, vals, abi_id.clone()))
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        // Either acquire the cached result, or make it
        let output = once
            .get_or_try_init(|| test.with_abi(&*abi_impl))
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
    #[tracing::instrument(name = "test", skip_all, fields(id = self.base_id(&test_key, None, "::")))]
    pub async fn do_test(
        &self,
        test_key: TestKey,
        test_rules: TestRules,
        out_dir: Utf8PathBuf,
    ) -> TestRunResults {
        use TestRunMode::*;

        let mut res = TestRunResults::new(test_key, test_rules);
        if res.rules.run <= Skip {
            return res;
        }

        res.ran_to = Generate;
        res.source = Some(self.generate_test(&res.key).await);
        let source = match res.source.as_ref().unwrap() {
            Ok(v) => v,
            Err(e) => {
                // If the codegen says "hey i don't support this", respect
                // that as an opt-out. (Doing it in this late-bound way
                // reduces the maintenance burden on backend authors.)
                if let GenerateError::Unsupported(e) = e {
                    res.rules.run = Skip;
                    warn!("skipping {}", e);
                } else {
                    warn!("failed to generate source: {}", e);
                }
                return res;
            }
        };
        if res.rules.run <= Generate {
            return res;
        }

        res.ran_to = Build;
        res.build = Some(self.build_test(&res.key, source, &out_dir).await);
        let build = match res.build.as_ref().unwrap() {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to build test: {}", e);
                return res;
            }
        };
        if res.rules.run <= Build {
            return res;
        }

        res.ran_to = Link;
        res.link = Some(self.link_dynamic_lib(&res.key, build, &out_dir).await);
        let link = match res.link.as_ref().unwrap() {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to link test: {}", e);
                return res;
            }
        };
        if res.rules.run <= Link {
            return res;
        }

        res.ran_to = Run;
        res.run = Some(self.run_dynamic_test(&res.key, link).await);
        let run = match res.run.as_ref().unwrap() {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to run test: {}", e);
                return res;
            }
        };
        if res.rules.run <= Run {
            return res;
        }

        res.ran_to = Check;
        res.check = Some(self.check_test(&res.key, run).await);

        res
    }
}

pub fn init_dirs() -> Result<Utf8PathBuf, Box<dyn Error>> {
    init_generate_dir()?;
    let build_dir = init_build_dir()?;
    Ok(build_dir)
}

impl TestHarness {
    pub fn base_id(
        &self,
        TestKey {
            test,
            options:
                TestOptions {
                    convention,
                    functions,
                    val_writer,
                    val_generator,
                },
            caller,
            callee,
        }: &TestKey,
        call_side: Option<CallSide>,
        separator: &str,
    ) -> String {
        let mut output = format!("{test}{separator}{convention}");
        if let FunctionSelector::One { idx, args } = functions {
            let test = self.tests[test].clone();
            let func = test.types.realize_func(*idx);
            output.push_str(separator);
            output.push_str(&func.name);
            if let ArgSelector::One { idx, vals } = args {
                output.push_str(separator);
                output.push_str(&format!("arg{idx}"));
                if let ValSelector::One { idx } = vals {
                    output.push_str(separator);
                    output.push_str(&format!("val{idx}"));
                }
            }
        }
        output.push_str(separator);
        match call_side {
            None => {
                output.push_str(caller);
                output.push_str("_calls_");
                output.push_str(callee);
            }
            Some(CallSide::Caller) => {
                output.push_str(caller);
                output.push_str("_caller");
            }
            Some(CallSide::Callee) => {
                output.push_str(callee);
                output.push_str("_callee");
            }
        }
        match val_writer {
            WriteImpl::HarnessCallback => {
                // Do nothing, implicit default
            }
            WriteImpl::Print => {
                output.push_str(separator);
                output.push_str("print");
            }
            WriteImpl::Assert => {
                output.push_str(separator);
                output.push_str("assert");
            }
            WriteImpl::Noop => {
                output.push_str(separator);
                output.push_str("noop");
            }
        }
        match val_generator {
            ValueGeneratorKind::Graffiti => {
                // Do nothing, implicit default
            }
            ValueGeneratorKind::Random { seed } => {
                output.push_str(separator);
                output.push_str(&format!("random{seed}"));
            }
        }
        output
    }

    /// The name of a test for pretty-printing.
    pub fn full_test_name(&self, key: &TestKey) -> String {
        self.base_id(key, None, "::")
    }

    /// The name of a subtest for pretty-printing.
    pub fn full_subtest_name(&self, key: &TestKey, func_name: &str) -> String {
        let base = self.full_test_name(key);
        format!("{base}::{func_name}")
    }
}
