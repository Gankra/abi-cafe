//! This is the primary file for the abi-cafe cdylib that all tests are compiled into.
//!
//! This will be statically linked into a cdylib with two other static libraries:
//! the caller and callee. The caller is expected to define the function `do_test`,
//! and call a bunch of functions defined by the callee. The cdylib
//! is run by the harness `dlopen`ing it and running `test_start`, passing in various
//! buffers and callbacks for instrumenting the result of the execution.
//!
//! This instrumentation is only used in the default mode of `WriteImpl::HarnessCallback`.
//! Otherwise the caller/callee may use things like asserts/prints.

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct WriteBuffer(*mut ());
unsafe impl Send for WriteBuffer {}
unsafe impl Sync for WriteBuffer {}

type SetFuncCallback = unsafe extern fn(WriteBuffer, u32) -> ();
type WriteValCallback = unsafe extern fn(WriteBuffer, u32, *const u8, u32) -> ();

#[no_mangle]
pub static mut CALLER_VALS: WriteBuffer = WriteBuffer(core::ptr::null_mut());
#[no_mangle]
pub static mut CALLEE_VALS: WriteBuffer = WriteBuffer(core::ptr::null_mut());
#[no_mangle]
pub static mut SET_FUNC: Option<SetFuncCallback> = None;
#[no_mangle]
pub static mut WRITE_VAL: Option<WriteValCallback> = None;

extern {
    fn do_test();
}

#[no_mangle]
pub extern fn test_start(
    set_func_callback: SetFuncCallback,
    write_val_callback: WriteValCallback,
    caller_vals: WriteBuffer,
    callee_vals: WriteBuffer,
) {
    unsafe {
        CALLER_VALS = caller_vals;
        CALLEE_VALS = callee_vals;
        SET_FUNC = Some(set_func_callback);
        WRITE_VAL = Some(write_val_callback);

        do_test();
    }
}