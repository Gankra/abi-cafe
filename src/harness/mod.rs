//! The test harness, which provides:
//!
//! 1. reading test .kdl files (kdl-script)
//! 2. generating impls of the tests
//! 3. building + linking the test impls together
//! 4. running the test impls
//! 5. checking the test results

use crate::TestKey;

mod build;
mod check;
mod generate;
mod read;
mod run;

pub use build::{build_test, link_test};
pub use check::check_test;
pub use generate::generate_test_src;
pub use read::read_tests;
pub use run::{run_dynamic_test, WriteBuffer};

/// The name of a test for pretty-printing.
pub fn full_test_name(
    TestKey {
        test_name,
        convention,
        caller_id,
        callee_id,
        ..
    }: &TestKey,
) -> String {
    format!("{test_name}::{convention}::{caller_id}_calls_{callee_id}")
}

/// The name of a subtest for pretty-printing.
pub fn full_subtest_name(
    TestKey {
        test_name,
        convention,
        caller_id,
        callee_id,
        ..
    }: &TestKey,
    func_name: &str,
) -> String {
    format!("{test_name}::{convention}::{caller_id}_calls_{callee_id}::{func_name}")
}
