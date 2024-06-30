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

type WriteCallback = unsafe extern fn(WriteBuffer, *const u8, u32) -> ();
type FinishedValCallback = unsafe extern fn(WriteBuffer) -> ();
type FinishedFuncCallback = unsafe extern fn(WriteBuffer, WriteBuffer) -> ();

#[no_mangle]
pub static mut CALLER_INPUTS: WriteBuffer = WriteBuffer(core::ptr::null_mut());
#[no_mangle]
pub static mut CALLER_OUTPUTS: WriteBuffer = WriteBuffer(core::ptr::null_mut());
#[no_mangle]
pub static mut CALLEE_INPUTS: WriteBuffer = WriteBuffer(core::ptr::null_mut());
#[no_mangle]
pub static mut CALLEE_OUTPUTS: WriteBuffer = WriteBuffer(core::ptr::null_mut());
#[no_mangle]
pub static mut WRITE_FIELD: Option<WriteCallback> = None;
#[no_mangle]
pub static mut FINISHED_VAL: Option<FinishedValCallback> = None;
#[no_mangle]
pub static mut FINISHED_FUNC: Option<FinishedFuncCallback> = None;

extern {
    fn do_test();
}

#[no_mangle]
pub extern fn test_start(
    write_callback: WriteCallback,
    finished_val_callback: FinishedValCallback,
    finished_func_callback: FinishedFuncCallback,
    caller_inputs: WriteBuffer,
    caller_outputs: WriteBuffer,
    callee_inputs: WriteBuffer,
    callee_outputs: WriteBuffer,
) {
    unsafe {
        CALLER_INPUTS = caller_inputs;
        CALLER_OUTPUTS = caller_outputs;
        CALLEE_INPUTS = callee_inputs;
        CALLEE_OUTPUTS = callee_outputs;
        WRITE_FIELD = Some(write_callback);
        FINISHED_VAL = Some(finished_val_callback);
        FINISHED_FUNC = Some(finished_func_callback);

        do_test();
    }
}