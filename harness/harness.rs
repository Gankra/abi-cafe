#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct WriteBuffer(*mut ());
unsafe impl Send for WriteBuffer {}
unsafe impl Sync for WriteBuffer {}

type WriteValCallback = unsafe extern fn(WriteBuffer, *const u8, u32) -> ();

#[no_mangle]
pub static mut CALLER_INPUTS: WriteBuffer = WriteBuffer(core::ptr::null_mut());
#[no_mangle]
pub static mut CALLER_OUTPUTS: WriteBuffer = WriteBuffer(core::ptr::null_mut());
#[no_mangle]
pub static mut CALLEE_INPUTS: WriteBuffer = WriteBuffer(core::ptr::null_mut());
#[no_mangle]
pub static mut CALLEE_OUTPUTS: WriteBuffer = WriteBuffer(core::ptr::null_mut());
#[no_mangle]
pub static mut WRITE: Option<WriteValCallback> = None;

extern { 
    fn do_test();
}

#[no_mangle]
pub extern fn test_start(
    write_val_callback: WriteValCallback, 
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
        WRITE = Some(write_val_callback);

        do_test();
    }
}