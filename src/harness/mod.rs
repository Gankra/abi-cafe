//! The test harness, which provides:
//!
//! 1. reading test .kdl files (kdl-script)
//! 2. generating impls of the tests
//! 3. building + linking the test impls together
//! 4. running the test impls
//! 5. checking the test results

use std::error::Error;

use crate::{TestKey, TestOptions};

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

/// The name of a test for pretty-printing.
pub fn full_test_name(
    TestKey {
        test,
        options: TestOptions { convention },
        caller,
        callee,
    }: &TestKey,
) -> String {
    format!("{test}::{convention}::{caller}_calls_{callee}")
}

/// The name of a subtest for pretty-printing.
pub fn full_subtest_name(
    TestKey {
        test,
        options: TestOptions { convention },
        caller,
        callee,
    }: &TestKey,
    func_name: &str,
) -> String {
    format!("{test}::{convention}::{caller}_calls_{callee}::{func_name}")
}
