use camino::Utf8PathBuf;
use console::Style;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::*;
use crate::harness::test::*;
use crate::*;

/// These are the builtin test-expectations, edit these if there are new rules!
impl TestHarness {
    #[allow(unused_variables)]
    pub fn get_test_rules(&self, key: &TestKey) -> TestRules {
        let caller = self.toolchain_by_test_key(key, CallSide::Caller);
        let callee = self.toolchain_by_test_key(key, CallSide::Callee);

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

        // i128 types are fake on windows so this is all random garbage that might
        // not even compile, but that datapoint is a little interesting/useful
        // so let's keep running them and just ignore the result for now.
        //
        // Anyone who cares about this situation more can make the expectations more precise.
        if cfg!(windows) && (key.test == "i128" || key.test == "u128") {
            result.check = Random(true);
        }

        // CI GCC is too old to support `_Float16`.
        if cfg!(all(target_arch = "x86_64", target_os = "linux")) && is_c && key.test == "f16" {
            result.check = Random(true);
        }

        // FIXME: investigate why this is failing to build
        if cfg!(windows) && is_c && (key.test == "EmptyStruct" || key.test == "EmptyStructInside") {
            result.check = Busted(Build);
        }

        //
        //
        // THIS AREA RESERVED FOR VENDORS TO APPLY PATCHES

        // END OF VENDOR RESERVED AREA
        //
        //

        if let Some(rules) = self.test_rules.targets.get(built_info::TARGET) {
            for (pattern, rules) in rules {
                let pat = self
                    .parse_test_pattern(pattern)
                    .expect("failed to parse test pattern");
                if pat.matches(key) {
                    if let Some(run) = rules.run.clone() {
                        result.run = run;
                    }
                    if let Some(check) = rules.check.clone() {
                        result.check = check;
                    }
                }
            }
        }
        result
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ExpectFile {
    pub targets: SortedMap<String, SortedMap<String, TestRulesPattern>>,
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
    pub caller_funcs: TestBuffer,
    #[serde(skip)]
    pub callee_funcs: TestBuffer,
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
            TestCheckMode::Random(_) => true,
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
    pub caller: ToolchainId,
    pub callee: ToolchainId,
    pub options: TestOptions,
}

#[derive(Debug, Clone)]
pub struct TestKeyPattern {
    pub test: Option<TestId>,
    pub caller: Option<ToolchainId>,
    pub callee: Option<ToolchainId>,
    pub toolchain: Option<ToolchainId>,
    pub options: TestOptionsPattern,
}
#[derive(Debug, Clone)]
pub struct TestOptionsPattern {
    pub convention: Option<CallingConvention>,
    pub functions: Option<FunctionSelector>,
    pub val_writer: Option<WriteImpl>,
    pub val_generator: Option<ValueGeneratorKind>,
    pub repr: Option<LangRepr>,
}
impl TestKey {
    pub(crate) fn toolchain_id(&self, call_side: CallSide) -> &str {
        match call_side {
            CallSide::Caller => &self.caller,
            CallSide::Callee => &self.callee,
        }
    }
}

impl TestKeyPattern {
    fn matches(&self, key: &TestKey) -> bool {
        let TestKeyPattern {
            test,
            caller,
            callee,
            toolchain,
            options:
                TestOptionsPattern {
                    convention,
                    functions,
                    val_writer,
                    val_generator,
                    repr,
                },
        } = self;

        if let Some(test) = test {
            if test != &key.test {
                return false;
            }
        }

        if let Some(caller) = caller {
            if caller != &key.caller {
                return false;
            }
        }
        if let Some(callee) = callee {
            if callee != &key.callee {
                return false;
            }
        }
        if let Some(toolchain) = toolchain {
            if toolchain != &key.caller && toolchain != &key.callee {
                return false;
            }
        }

        if let Some(convention) = convention {
            if convention != &key.options.convention {
                return false;
            }
        }
        if let Some(functions) = functions {
            if functions != &key.options.functions {
                return false;
            }
        }
        if let Some(val_writer) = val_writer {
            if val_writer != &key.options.val_writer {
                return false;
            }
        }
        if let Some(val_generator) = val_generator {
            if val_generator != &key.options.val_generator {
                return false;
            }
        }
        if let Some(repr) = repr {
            if repr != &key.options.repr {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TestRules {
    pub run: TestRunMode,
    #[serde(flatten)]
    pub check: TestCheckMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRulesPattern {
    pub run: Option<TestRunMode>,
    #[serde(flatten)]
    pub check: Option<TestCheckMode>,
}
/// How far the test should be executed
///
/// Each case implies all the previous cases.
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
    Random(bool),
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

#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum TestConclusion {
    Skipped,
    Passed,
    Busted,
    Failed,
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

        let red = Style::new().red();
        let green = Style::new().green();
        let blue = Style::new().blue();
        let mut sorted_tests = self.tests.iter().collect::<Vec<_>>();
        sorted_tests.sort_by_key(|t| t.conclusion);
        for test in sorted_tests {
            if let Skipped = test.conclusion {
                continue;
            }
            let pretty_test_name = harness.full_test_name(&test.key);
            write!(f, "{pretty_test_name:<64} ")?;
            match (&test.conclusion, &test.rules.check) {
                (Skipped, _) => {
                    // Don't mention these, too many
                    // write!(f, "skipped")?
                }

                (Passed, Pass(_)) => write!(f, "passed")?,
                (Passed, Random(_)) => write!(f, "passed (random, result ignored)")?,
                (Passed, Fail(_)) => write!(f, "passed (failed as expected)")?,

                (Failed, Pass(_)) => {
                    write!(f, "{}", red.apply_to("failed"))?;
                    if test.results.ran_to < TestRunMode::Check {
                        let (msg, err) = match &test.results.ran_to {
                            TestRunMode::Generate => {
                                ("generate source code", format_err(&test.results.source))
                            }
                            TestRunMode::Build => {
                                ("compile source code", format_err(&test.results.build))
                            }
                            TestRunMode::Link => {
                                ("link both sides together", format_err(&test.results.link))
                            }
                            TestRunMode::Run => ("run the program", format_err(&test.results.run)),
                            TestRunMode::Skip | TestRunMode::Check => ("", String::new()),
                        };
                        write!(f, "{}", red.apply_to(" to "))?;
                        writeln!(f, "{}", red.apply_to(msg))?;
                        writeln!(f, "  {}", red.apply_to(err))?;
                    }
                }
                (Failed, Random(_)) => {
                    write!(f, "{}", red.apply_to("failed!? (failed but random!?)"))?
                }
                (Failed, Fail(_)) => {
                    write!(f, "{}", red.apply_to("failed (passed unexpectedly!)"))?
                }
                (Failed, TestCheckMode::Busted(_)) => write!(
                    f,
                    "{}",
                    green.apply_to("fixed (test was busted, congrats!)")
                )?,

                (TestConclusion::Busted, _) | (Passed, TestCheckMode::Busted(_)) => {
                    write!(f, "{}", blue.apply_to("busted (known failure, ignored)"))?
                }
            }

            let be_detailed = test.results.ran_to >= TestRunMode::Check;
            if !be_detailed {
                writeln!(f)?;
                continue;
            }
            let Some(check_result) = &test.results.check else {
                continue;
            };
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
                if let Err(e) = result {
                    writeln!(f, "{}", red.apply_to("failed!"))?;
                    writeln!(f, "{}", red.apply_to(e))?;
                } else {
                    writeln!(f)?;
                }
            }
            writeln!(f)?;
        }
        writeln!(f)?;
        let summary_style = if self.summary.num_failed > 0 {
            red
        } else if self.summary.num_busted > 0 {
            blue
        } else {
            green
        };
        let summary = format!(
            "{} test sets run - {} passed, {} busted, {} failed, {} skipped",
            self.summary.num_tests,
            self.summary.num_passed,
            self.summary.num_busted,
            self.summary.num_failed,
            self.summary.num_skipped
        );
        writeln!(f, "{}", summary_style.apply_to(summary),)?;
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
                TestConclusion::Failed => ("failed", Some("FIXME fill this message in")),
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

fn format_err<T, E: std::fmt::Display>(maybe_res: &Option<Result<T, E>>) -> String {
    let Some(res) = maybe_res else {
        return String::new();
    };
    let Some(res) = res.as_ref().err() else {
        return String::new();
    };
    format!("{res}")
}
