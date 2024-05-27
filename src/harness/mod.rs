//! The test harness, which provides:
//!
//! 1. reading test .kdl files (kdl-script)
//! 2. generating impls of the tests
//! 3. building + linking the test impls together
//! 4. running the test impls
//! 5. checking the test results

use crate::{TestKey, TestOptions};

mod build;
mod check;
mod generate;
mod read;
mod run;

pub use build::{compile_lib, init_build_dir, lib_name, link_test};
pub use check::check_test;
pub use generate::{generate_src, init_generate_dir, src_path};
pub use read::read_tests;
pub use run::{run_dynamic_test, WriteBuffer};

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
