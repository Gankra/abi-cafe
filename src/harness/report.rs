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
        use TestCheckMode::*;
        use TestRunMode::*;

        // By default, require tests to run completely and pass
        let mut result = TestRules {
            run: Check,
            check: Pass(Check),
        };

        for expect_file in &self.test_rules {
            let rulesets = [
                expect_file.targets.get("*"),
                expect_file.targets.get(built_info::TARGET),
            ];
            for rules in rulesets {
                let Some(rules) = rules else {
                    continue;
                };
                for (pattern, rules) in rules {
                    if pattern.matches(key) {
                        if let Some(run) = rules.run {
                            result.run = run;
                        }
                        if let Some(check) = rules.check {
                            result.check = check;
                        }
                    }
                }
            }
        }

        //
        //
        // THIS AREA RESERVED FOR VENDORS TO APPLY PATCHES

        // END OF VENDOR RESERVED AREA
        //
        //

        result
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ExpectFile {
    #[serde(default)]
    pub targets: IndexMap<String, IndexMap<TestKeyPattern, TestRulesPattern>>,
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
            TestCheckMode::Pass(must_pass) => success_at_step(&results, must_pass, true),
            TestCheckMode::Fail(must_fail) => success_at_step(&results, must_fail, false),
            TestCheckMode::Busted(must_fail) => success_at_step(&results, must_fail, false),
            TestCheckMode::Random(_) => Some(true),
        };
        if passed.unwrap_or(false) {
            if matches!(results.rules.check, TestCheckMode::Busted(_)) {
                TestConclusion::Busted
            } else {
                TestConclusion::Passed
            }
        } else {
            TestConclusion::Failed
        }
    };

    // Compute what the annotation *could* be to make CI green
    let did_pass = success_at_step(&results, &results.ran_to, true).unwrap_or(false);
    let could_be = TestRulesPattern {
        run: if results.rules.run != TestRunMode::Check {
            Some(results.rules.run)
        } else {
            None
        },
        check: if did_pass {
            Some(TestCheckMode::Pass(results.rules.run))
        } else {
            Some(TestCheckMode::Busted(results.rules.run))
        },
    };
    TestReport {
        key: results.key.clone(),
        rules: results.rules,
        conclusion,
        could_be,
        results,
    }
}

fn success_at_step(results: &TestRunResults, step: &TestRunMode, wants_pass: bool) -> Option<bool> {
    use TestRunMode::*;
    let res = match step {
        Skip => return Some(true),
        Generate => results.source.as_ref().map(|r| r.is_ok()),
        Build => results.build.as_ref().map(|r| r.is_ok()),
        Link => results.link.as_ref().map(|r| r.is_ok()),
        Run => results.run.as_ref().map(|r| r.is_ok()),
        Check => results.check.as_ref().map(|r| r.all_passed),
    };
    res.map(|res| res == wants_pass)
}

#[derive(Debug, Serialize)]
pub struct FullReport {
    pub summary: TestSummary,
    pub possible_rules: Option<ExpectFile>,
    pub tests: Vec<TestReport>,
}

#[derive(Debug, Serialize)]
pub struct TestReport {
    pub key: TestKey,
    pub rules: TestRules,
    pub results: TestRunResults,
    pub conclusion: TestConclusion,
    pub could_be: TestRulesPattern,
}

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestKeyPattern {
    pub test: Option<TestId>,
    pub caller: Option<ToolchainId>,
    pub callee: Option<ToolchainId>,
    pub toolchain: Option<ToolchainId>,
    pub options: TestOptionsPattern,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestOptionsPattern {
    pub convention: Option<CallingConvention>,
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

impl std::str::FromStr for TestKeyPattern {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let separator = "::";
        let parts = input.split(separator).collect::<Vec<_>>();

        let mut key = TestKeyPattern {
            test: None,
            caller: None,
            callee: None,
            toolchain: None,
            options: TestOptionsPattern {
                convention: None,
                repr: None,
                val_generator: None,
            },
        };

        let [test, rest @ ..] = &parts[..] else {
            return Ok(key);
        };
        key.test = (!test.is_empty()).then(|| test.to_string());

        for part in rest {
            // pairs
            if let Some((caller, callee)) = part.split_once("_calls_") {
                key.caller = Some(caller.to_owned());
                key.callee = Some(callee.to_owned());
                continue;
            }
            if let Some(caller) = part.strip_suffix("_caller") {
                key.caller = Some(caller.to_owned());
                continue;
            }
            if let Some(callee) = part.strip_suffix("_callee") {
                key.callee = Some(callee.to_owned());
                continue;
            }
            if let Some(toolchain) = part.strip_suffix("_toolchain") {
                key.toolchain = Some(toolchain.to_owned());
                continue;
            }

            // repr
            if let Some(repr) = part.strip_prefix("repr_") {
                key.options.repr = Some(repr.parse()?);
                continue;
            }

            // conv
            if let Some(conv) = part.strip_prefix("conv_") {
                key.options.convention = Some(conv.parse()?);
                continue;
            }
            // generator
            if let Ok(val_generator) = part.parse() {
                key.options.val_generator = Some(val_generator);
                continue;
            }

            return Err(format!("unknown testkey part: {part}"));
        }
        Ok(key)
    }
}
impl std::fmt::Display for TestKeyPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let TestKeyPattern {
            test,
            caller,
            callee,
            toolchain,
            options:
                TestOptionsPattern {
                    convention,
                    val_generator,
                    repr,
                },
        } = self;
        let separator = "::";
        let mut output = String::new();
        if let Some(test) = test {
            output.push_str(test);
        }
        if let Some(convention) = convention {
            output.push_str(separator);
            output.push_str(&format!("conv_{convention}"));
        }
        if let Some(repr) = repr {
            output.push_str(separator);
            output.push_str(&format!("repr_{repr}"));
        }
        if let Some(toolchain) = toolchain {
            output.push_str(separator);
            output.push_str(&format!("{toolchain}_toolchain"));
        }
        match (caller, callee) {
            (Some(caller), Some(callee)) => {
                output.push_str(separator);
                output.push_str(caller);
                output.push_str("_calls_");
                output.push_str(callee);
            }
            (Some(caller), None) => {
                output.push_str(separator);
                output.push_str(caller);
                output.push_str("_caller");
            }
            (None, Some(callee)) => {
                output.push_str(separator);
                output.push_str(callee);
                output.push_str("_callee");
            }
            (None, None) => {
                // Noting
            }
        }
        if let Some(val_generator) = val_generator {
            output.push_str(separator);
            output.push_str(&val_generator.to_string());
        }
        output.fmt(f)
    }
}
impl<'de> Deserialize<'de> for TestKeyPattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let input = String::deserialize(deserializer)?;
        input.parse().map_err(D::Error::custom)
    }
}
impl Serialize for TestKeyPattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
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
    pub subtest_checks: Vec<SubtestDetails>,
}

#[derive(Debug, Serialize)]
pub struct SubtestDetails {
    pub result: Result<(), CheckFailure>,
    pub minimized: Option<GenerateOutput>,
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

            let be_detailed = test.results.ran_to >= TestRunMode::Check
                && test.conclusion != TestConclusion::Busted;
            if !be_detailed {
                writeln!(f)?;
                continue;
            }
            let Some(check_result) = &test.results.check else {
                continue;
            };
            let sub_results = &check_result.subtest_checks;
            let num_passed = sub_results.iter().filter(|t| t.result.is_ok()).count();

            writeln!(f, " ({num_passed:>3}/{:<3} passed)", sub_results.len())?;
            // If all the subtests pass, don't bother with a breakdown.
            if check_result.all_passed {
                continue;
            }

            let max_name_len = check_result
                .subtest_names
                .iter()
                .fold(0, |max, name| max.max(name.len()));
            for (subtest_name, subtest) in check_result.subtest_names.iter().zip(sub_results.iter())
            {
                write!(f, "  {:width$} ", subtest_name, width = max_name_len)?;
                if let Err(e) = &subtest.result {
                    writeln!(f, "{}", red.apply_to("failed!"))?;
                    if let Some(minimized) = &subtest.minimized {
                        writeln!(f, "    {}", blue.apply_to("minimized to:"))?;
                        writeln!(f, "      caller: {}", blue.apply_to(&minimized.caller_src))?;
                        writeln!(f, "      callee: {}", blue.apply_to(&minimized.callee_src))?;
                    }
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
            blue.clone()
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
        if let Some(rules) = &self.possible_rules {
            writeln!(f)?;
            writeln!(
                f,
                "{}",
                blue.apply_to("(experimental) adding this to your abi-cafe-rules.toml might help:")
            )?;
            let toml = toml::to_string_pretty(rules).expect("failed to serialize possible rules!?");
            writeln!(f, "{}", toml)?;
        }
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
