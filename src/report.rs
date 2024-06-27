use camino::Utf8PathBuf;
use serde::Serialize;
use serde_json::json;

use crate::abis::*;
use crate::error::*;
use crate::AbiImplId;
use crate::TestHarness;
use crate::TestId;
use crate::WriteBuffer;

/// These are the builtin test-expectations, edit these if there are new rules!
pub fn get_test_rules(test: &TestKey, caller: &dyn AbiImpl, callee: &dyn AbiImpl) -> TestRules {
    use TestCheckMode::*;
    use TestRunMode::*;

    // By default, require tests to run completely and pass
    let mut result = TestRules {
        run: Check,
        check: Pass(Check),
    };

    // Now apply specific custom expectations for platforms/suites
    let is_c = caller.lang() == "c" || callee.lang() == "c";
    let is_rust = caller.lang() == "rust" || callee.lang() == "rust";
    let is_rust_and_c = is_c && is_rust;

    // llvm and gcc disagree on the u128 ABI everywhere but aarch64 (arm64) and s390x.
    // This is Bad! Ideally we should check for all clang<->gcc pairs but to start
    // let's mark rust <-> C as disagreeing (because rust also disagrees with clang).
    if !cfg!(any(target_arch = "aarch64", target_arch = "s390x"))
        && test.test == "ui128"
        && is_rust_and_c
    {
        result.check = Busted(Check);
    }

    // i128 types are fake on windows so this is all random garbage that might
    // not even compile, but that datapoint is a little interesting/useful
    // so let's keep running them and just ignore the result for now.
    //
    // Anyone who cares about this situation more can make the expectations more precise.
    if cfg!(windows) && test.test == "ui128" {
        result.check = Random;
    }

    // This test is just for investigation right now, nothing normative
    if test.test == "sysv_i128_emulation" {
        result.check = Random;
    }

    //
    //
    // THIS AREA RESERVED FOR VENDORS TO APPLY PATCHES

    // END OF VENDOR RESERVED AREA
    //
    //

    result
}

impl Serialize for BuildError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let string = self.to_string();
        serializer.serialize_str(&string)
    }
}
impl Serialize for RunError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let string = self.to_string();
        serializer.serialize_str(&string)
    }
}
impl Serialize for LinkError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let string = self.to_string();
        serializer.serialize_str(&string)
    }
}
impl Serialize for CheckFailure {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let string = self.to_string();
        serializer.serialize_str(&string)
    }
}
impl Serialize for GenerateError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let string = self.to_string();
        serializer.serialize_str(&string)
    }
}

#[derive(Debug, Serialize)]
pub struct RunOutput {
    #[serde(skip)]
    pub caller_inputs: WriteBuffer,
    #[serde(skip)]
    pub caller_outputs: WriteBuffer,
    #[serde(skip)]
    pub callee_inputs: WriteBuffer,
    #[serde(skip)]
    pub callee_outputs: WriteBuffer,
}

pub fn report_test(results: TestRunResults) -> TestReport {
    use TestConclusion::*;
    use TestRunMode::*;
    // Ok now check if it matched our expectation
    let conclusion = if results.rules.run == Skip {
        // If we were told to skip, we skipped
        Skipped
    } else if let Some(Err(GenerateError::Skipped)) = results.source {
        // The generate step is allowed to unilaterally skip things
        // to avoid different configs having to explicitly disable
        // a million unsupported combinations
        Skipped
    } else {
        let passed = match &results.rules.check {
            TestCheckMode::Pass(must_pass) => match must_pass {
                Skip => true,
                Generate => results.source.as_ref().map(|r| r.is_ok()).unwrap_or(false),
                Build => results.build.as_ref().map(|r| r.is_ok()).unwrap_or(false),
                Link => results.link.as_ref().map(|r| r.is_ok()).unwrap_or(false),
                Run => results.run.as_ref().map(|r| r.is_ok()).unwrap_or(false),
                Check => results
                    .check
                    .as_ref()
                    .map(|r| r.all_passed)
                    .unwrap_or(false),
            },
            TestCheckMode::Fail(must_fail) | TestCheckMode::Busted(must_fail) => match must_fail {
                Skip => true,
                Generate => results.source.as_ref().map(|r| !r.is_ok()).unwrap_or(false),
                Build => results.build.as_ref().map(|r| !r.is_ok()).unwrap_or(false),
                Link => results.link.as_ref().map(|r| !r.is_ok()).unwrap_or(false),
                Run => results.run.as_ref().map(|r| !r.is_ok()).unwrap_or(false),
                Check => results
                    .check
                    .as_ref()
                    .map(|r| !r.all_passed)
                    .unwrap_or(false),
            },
            TestCheckMode::Random => true,
        };
        if passed {
            if matches!(results.rules.check, TestCheckMode::Busted(_)) {
                TestConclusion::Busted
            } else {
                TestConclusion::Passed
            }
        } else {
            TestConclusion::Failed
        }
    };
    TestReport {
        key: results.key.clone(),
        rules: results.rules.clone(),
        conclusion,
        results,
    }
}

#[derive(Debug, Serialize)]
pub struct FullReport {
    pub summary: TestSummary,
    pub config: TestConfig,
    pub tests: Vec<TestReport>,
}

#[derive(Debug, Serialize)]
pub struct TestReport {
    pub key: TestKey,
    pub rules: TestRules,
    pub results: TestRunResults,
    pub conclusion: TestConclusion,
}

#[derive(Debug, Serialize)]
pub struct TestConfig {}
#[derive(Debug, Serialize)]
pub struct TestSummary {
    pub num_tests: u64,
    pub num_passed: u64,
    pub num_busted: u64,
    pub num_failed: u64,
    pub num_skipped: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestKey {
    pub test: TestId,
    pub caller: AbiImplId,
    pub callee: AbiImplId,
    pub options: TestOptions,
}
impl TestKey {
    pub(crate) fn abi_id(&self, call_side: CallSide) -> &str {
        match call_side {
            CallSide::Caller => &self.caller,
            CallSide::Callee => &self.callee,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TestRules {
    pub run: TestRunMode,
    pub check: TestCheckMode,
}

/// How far the test should be executed
///
/// Each case implies all the previous cases.
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Serialize)]
#[allow(dead_code)]

pub enum TestRunMode {
    /// Don't run the test at all (marked as skipped)
    Skip,
    /// Just generate the source
    Generate,
    /// Just build the source
    Build,
    /// Just link the source
    Link,
    /// Run the tests, but don't check the results
    Run,
    /// Run the tests, and check the results
    Check,
}

/// To what level of correctness should the test be graded?
///
/// Tests that are Skipped ignore this.
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Serialize)]
#[allow(dead_code)]
pub enum TestCheckMode {
    /// The test must successfully complete this phase,
    /// whatever happens after that is gravy.
    Pass(TestRunMode),
    /// The test must fail at this exact phase.
    Fail(TestRunMode),
    /// Same as Fail, but indicates this is a bug/flaw that should eventually
    /// be fixed, and not the desired result.
    Busted(TestRunMode),
    /// The test is flakey and random but we want to run it anyway,
    /// so accept whatever result we get as ok.
    Random,
}

#[derive(Debug, Serialize)]
pub struct TestRunResults {
    pub key: TestKey,
    pub rules: TestRules,
    pub ran_to: TestRunMode,
    pub source: Option<Result<GenerateOutput, GenerateError>>,
    pub build: Option<Result<BuildOutput, BuildError>>,
    pub link: Option<Result<LinkOutput, LinkError>>,
    pub run: Option<Result<RunOutput, RunError>>,
    pub check: Option<CheckOutput>,
}

impl TestRunResults {
    pub fn new(key: TestKey, rules: TestRules) -> Self {
        Self {
            key,
            rules,
            ran_to: TestRunMode::Skip,
            source: None,
            build: None,
            link: None,
            run: None,
            check: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GenerateOutput {
    pub caller_src: Utf8PathBuf,
    pub callee_src: Utf8PathBuf,
}

#[derive(Debug, Serialize)]
pub struct BuildOutput {
    pub caller_lib: String,
    pub callee_lib: String,
}

#[derive(Debug, Serialize)]
pub struct LinkOutput {
    pub test_bin: Utf8PathBuf,
}

#[derive(Debug, Serialize)]
pub struct CheckOutput {
    pub all_passed: bool,
    pub subtest_names: Vec<String>,
    pub subtest_checks: Vec<Result<(), CheckFailure>>,
}

#[derive(Debug, Clone, Serialize)]
pub enum TestConclusion {
    Skipped,
    Passed,
    Failed,
    Busted,
}

impl FullReport {
    pub fn print_human(
        &self,
        harness: &TestHarness,
        mut f: impl std::io::Write,
    ) -> Result<(), std::io::Error> {
        use TestCheckMode::*;
        use TestConclusion::*;
        writeln!(f, "Final Results:")?;

        for test in &self.tests {
            let pretty_test_name = harness.full_test_name(&test.key);
            write!(f, "{pretty_test_name:<40} ")?;
            match (&test.conclusion, &test.rules.check) {
                (Skipped, _) => write!(f, "skipped")?,

                (Passed, Pass(_)) => write!(f, "passed")?,
                (Passed, Random) => write!(f, "passed (random, result ignored)")?,
                (Passed, Fail(_)) => write!(f, "passed (failed as expected)")?,

                (Failed, Pass(_)) => write!(f, "failed")?,
                (Failed, Random) => write!(f, "failed!? (failed but random!?)")?,
                (Failed, Fail(_)) => write!(f, "failed (passed unexpectedly!)")?,
                (Failed, TestCheckMode::Busted(_)) => {
                    write!(f, "fixed (test was busted, congrats!)")?
                }

                (TestConclusion::Busted, _) | (Passed, TestCheckMode::Busted(_)) => {
                    write!(f, "busted (known failure, ignored)")?
                }
            }

            let be_detailed = test.results.ran_to >= TestRunMode::Check;
            if !be_detailed {
                writeln!(f)?;
                continue;
            }
            let check_result = test.results.check.as_ref().unwrap();
            let sub_results = &check_result.subtest_checks;
            let num_passed = sub_results.iter().filter(|r| r.is_ok()).count();

            writeln!(f, " ({num_passed:>3}/{:<3} passed)", sub_results.len())?;
            // If all the subtests pass, don't bother with a breakdown.
            if check_result.all_passed {
                continue;
            }

            let max_name_len = check_result
                .subtest_names
                .iter()
                .fold(0, |max, name| max.max(name.len()));
            for (subtest_name, result) in check_result.subtest_names.iter().zip(sub_results.iter())
            {
                write!(f, "  {:width$} ", subtest_name, width = max_name_len)?;
                if let Err(_e) = result {
                    writeln!(f, "failed!")?;
                } else {
                    writeln!(f)?;
                }
            }
            writeln!(f)?;
        }
        writeln!(f)?;
        writeln!(
            f,
            "{} tests run - {} passed, {} busted, {} failed, {} skipped",
            self.summary.num_tests,
            self.summary.num_passed,
            self.summary.num_busted,
            self.summary.num_failed,
            self.summary.num_skipped,
        )?;
        Ok(())
    }

    pub fn print_json(
        &self,
        _harness: &TestHarness,
        f: impl std::io::Write,
    ) -> Result<(), std::io::Error> {
        serde_json::to_writer_pretty(f, self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
    pub fn print_rustc_json(
        &self,
        harness: &TestHarness,
        mut f: impl std::io::Write,
    ) -> Result<(), std::io::Error> {
        serde_json::to_writer(
            &mut f,
            &json!({
                "type": "suite",
                "event": "started",
                "test_count": self.summary.num_tests - self.summary.num_skipped,
            }),
        )
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        writeln!(&mut f)?;

        for test in &self.tests {
            let (status, status_message) = match test.conclusion {
                TestConclusion::Skipped => continue,
                TestConclusion::Passed => ("ok", None),
                TestConclusion::Failed => ("failed", Some("todo fill this message in")),
                TestConclusion::Busted => ("ok", None),
            };
            let test_name = harness.full_test_name(&test.key);
            serde_json::to_writer(
                &mut f,
                &json!({
                    "type": "test",
                    "event": "started",
                    "name": &test_name,
                }),
            )
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            writeln!(&mut f)?;
            serde_json::to_writer(
                &mut f,
                &json!({
                    "type": "test",
                    "name": &test_name,
                    "event": status,
                    "stdout": status_message,
                }),
            )
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            writeln!(&mut f)?;
        }

        let status = if self.failed() { "failed" } else { "ok" };
        serde_json::to_writer(
            &mut f,
            &json!({
                "type": "suite",
                "event": status,
                "passed": self.summary.num_passed + self.summary.num_busted,
                "failed": self.summary.num_failed,
                "ignored": 0,
                "measured": 0,
                "filtered_out": self.summary.num_skipped,
                "exec_time": 0.0,
            }),
        )
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        writeln!(&mut f)?;

        Ok(())
    }

    pub fn failed(&self) -> bool {
        self.summary.num_failed > 0
    }
}
