//! The test harness, which provides:
//!
//! 1. reading test .kdl files (kdl-script)
//! 2. generating impls of the tests
//! 3. building + linking the test impls together
//! 4. running the test impls
//! 5. checking the test results

use std::error::Error;

use crate::{ArgSelector, CallSide, FunctionSelector, TestHarness, TestKey, TestOptions};

mod build;
mod check;
mod generate;
mod read;
mod run;

use build::init_build_dir;
use camino::Utf8PathBuf;
use generate::init_generate_dir;
pub use read::read_tests;
pub use run::WriteBuffer;

pub fn init_dirs() -> Result<Utf8PathBuf, Box<dyn Error>> {
    init_generate_dir()?;
    let build_dir = init_build_dir()?;
    Ok(build_dir)
}

impl TestHarness {
    fn base_id(
        &self,
        TestKey {
            test,
            options:
                TestOptions {
                    convention,
                    functions,
                    val_writer,
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
            if let ArgSelector::One { idx } = args {
                let arg = func
                    .inputs
                    .iter()
                    .chain(&func.outputs)
                    .nth(*idx)
                    .expect("argument index out of bounds");
                output.push_str(separator);
                output.push_str(&arg.name);
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
            crate::WriteImpl::HarnessCallback => {
                // Do nothing, implicit default
            }
            crate::WriteImpl::Print => {
                output.push_str(separator);
                output.push_str("print");
            }
            crate::WriteImpl::Noop => {
                output.push_str(separator);
                output.push_str("noop");
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
