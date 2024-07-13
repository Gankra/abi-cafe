//! This is the primary file for the abi-cafe standalone binary/project mode.
//!
//! This mode is primarily intended for reporting/debugging abi-cafe test failures,
//! where you want the particulars of abi-cafe to go away, and want a minimized
//! reproduction of the issue.
//!
//! As such this is incompatible with `WriteImpl::HarnessCallback`.
//!
//! In theory this could be replaced with just making `caller::do_test` into `main`
//! but this might be a bit easier..?

extern {
    fn do_test();
}

fn main() {
    do_test();
}