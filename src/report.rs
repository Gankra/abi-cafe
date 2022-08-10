use std::path::PathBuf;

use serde::Serialize;

use crate::{abis::*, full_test_name, WriteBuffer};

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

    // llvm and gcc disagree on the u128 ABI everywhere but aarch64 (arm64).
    // This is Bad! Ideally we should check for all clang<->gcc pairs but to start
    // let's mark rust <-> C as disagreeing (because rust also disagrees with clang).
    if !cfg!(target_arch = "aarch64") && test.test_name == "ui128" && is_rust_and_c {
        result.check = Busted(Check);
    }

    // i128 types are fake on windows so this is all random garbage that might
    // not even compile, but that datapoint is a little interesting/useful
    // so let's keep running them and just ignore the result for now.
    //
    // Anyone who cares about this situation more can make the expectations more precise.
    if cfg!(windows) && test.test_name == "ui128" {
        result.check = Random;
    }

    // This test is just for investigation right now, nothing normative
    if test.test_name == "sysv_i128_emulation" {
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

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("io error\n{0}")]
    Io(#[from] std::io::Error),
    #[error("rust compile error \n{} \n{}",
        std::str::from_utf8(&.0.stdout).unwrap(),
        std::str::from_utf8(&.0.stderr).unwrap())]
    RustCompile(std::process::Output),
    #[error("c compile errror\n{0}")]
    CCompile(#[from] cc::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CheckFailure {
    #[error("test {0} {} field {2} mismatch \ncaller: {3:02X?} \ncallee: {4:02X?}", ARG_NAMES[*.1])]
    InputFieldMismatch(usize, usize, usize, Vec<u8>, Vec<u8>),
    #[error(
        "test {0} {} field {2} mismatch \ncaller: {3:02X?} \ncallee: {4:02X?}",
        OUTPUT_NAME
    )]
    OutputFieldMismatch(usize, usize, usize, Vec<u8>, Vec<u8>),
    #[error("test {0} {} field count mismatch \ncaller: {2:#02X?} \ncallee: {3:#02X?}", ARG_NAMES[*.1])]
    InputFieldCountMismatch(usize, usize, Vec<Vec<u8>>, Vec<Vec<u8>>),
    #[error(
        "test {0} {} field count mismatch \ncaller: {2:#02X?} \ncallee: {3:#02X?}",
        OUTPUT_NAME
    )]
    OutputFieldCountMismatch(usize, usize, Vec<Vec<u8>>, Vec<Vec<u8>>),
    #[error("test {0} input count mismatch \ncaller: {1:#02X?} \ncallee: {2:#02X?}")]
    InputCountMismatch(usize, Vec<Vec<Vec<u8>>>, Vec<Vec<Vec<u8>>>),
    #[error("test {0} output count mismatch \ncaller: {1:#02X?} \ncallee: {2:#02X?}")]
    OutputCountMismatch(usize, Vec<Vec<Vec<u8>>>, Vec<Vec<Vec<u8>>>),
}

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("io error\n{0}")]
    Io(#[from] std::io::Error),
    #[error("rust link error \n{} \n{}",
        std::str::from_utf8(&.0.stdout).unwrap(),
        std::str::from_utf8(&.0.stderr).unwrap())]
    RustLink(std::process::Output),
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("test loading error (dynamic linking failed)\n{0}")]
    LoadError(#[from] libloading::Error),
    #[error("wrong number of tests reported! \nExpected {0} \nGot (caller_in: {1}, caller_out: {2}, callee_in: {3}, callee_out: {4})")]
    TestCountMismatch(usize, usize, usize, usize, usize),
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
    pub test_name: String,
    pub convention: String,
    pub caller_id: String,
    pub callee_id: String,
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
    pub ran_to: TestRunMode,
    pub source: Option<Result<GenerateOutput, GenerateError>>,
    pub build: Option<Result<BuildOutput, BuildError>>,
    pub link: Option<Result<LinkOutput, LinkError>>,
    pub run: Option<Result<RunOutput, RunError>>,
    pub check: Option<CheckOutput>,
}

impl Default for TestRunResults {
    fn default() -> Self {
        Self {
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
    pub caller_src: PathBuf,
    pub callee_src: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct BuildOutput {
    pub caller_lib: PathBuf,
    pub callee_lib: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct LinkOutput {
    pub test_bin: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct RunOutput {
    pub caller_inputs: WriteBuffer,
    pub caller_outputs: WriteBuffer,
    pub callee_inputs: WriteBuffer,
    pub callee_outputs: WriteBuffer,
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
    pub fn print_human(&self, mut f: impl std::io::Write) -> Result<(), std::io::Error> {
        use TestCheckMode::*;
        use TestConclusion::*;
        writeln!(f, "Final Results:")?;

        for test in &self.tests {
            let pretty_test_name = full_test_name(&test.key);
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

    pub fn print_json(&self, f: impl std::io::Write) -> Result<(), std::io::Error> {
        serde_json::to_writer_pretty(f, self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
    pub fn failed(&self) -> bool {
        self.summary.num_failed > 0
    }
}
