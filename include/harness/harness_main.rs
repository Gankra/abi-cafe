//! This is the primary file for the abi-cafe harness main that all tests are compiled into.
//!
//! This will be statically linked into a cdylib with two other static libraries:
//! the caller and callee. The caller is expected to define the function `do_test`,
//! and call a bunch of functions defined by the callee. The cdylib
//! is run by the harness `dlopen`ing it and running `test_start`, passing in various
//! buffers and callbacks for instrumenting the result of the execution.
//!
//! This instrumentation is only used in the default mode of `WriteImpl::HarnessCallback`.
//! Otherwise the caller/callee may use things like asserts/prints.

/// Tests write back the raw bytes of their values to a WriteBuffer.
pub struct WriteBuffer {
    pub identity: &'static str,
}

impl WriteBuffer {
    fn new(identity: &'static str) -> Self {
        // Preload the hierarchy for the first test.
        WriteBuffer {
            identity,
        }
    }
}

// The signatures of the interface from our perspective.
// From the test's perspective the WriteBuffers are totally opaque.
pub type SetFuncCallback = unsafe extern "C" fn(&mut WriteBuffer, u32) -> ();
pub type WriteValCallback = unsafe extern "C" fn(&mut WriteBuffer, u32, *const u8, u32) -> ();
pub type TestInit =
    unsafe extern "C" fn(SetFuncCallback, WriteValCallback, &mut WriteBuffer, &mut WriteBuffer) -> ();

pub unsafe extern "C" fn set_func(test: &mut WriteBuffer, func: u32) {
    let ident = &test.identity;
    println!(r#"{{ "info": "func", "id": "{ident}", "func": {func} }}"#);
}

pub unsafe extern "C" fn write_val(
    test: &mut WriteBuffer,
    val_idx: u32,
    input: *const u8,
    size: u32,
) {
    let data = std::slice::from_raw_parts(input, size as usize);
    let ident = &test.identity;
    println!(r#"{{ "info": "val", "id": "{ident}", "val": {val_idx}, "bytes": {data:?} }}"#);
}


#[no_mangle]
pub static mut CALLER_VALS: *mut () = core::ptr::null_mut();
#[no_mangle]
pub static mut CALLEE_VALS: *mut () = core::ptr::null_mut();
#[no_mangle]
pub static mut SET_FUNC: Option<SetFuncCallback> = None;
#[no_mangle]
pub static mut WRITE_VAL: Option<WriteValCallback> = None;

extern {
    fn do_test();
}

pub fn main() {
    unsafe {
        let mut caller_vals = WriteBuffer::new("caller");
        let mut callee_vals = WriteBuffer::new("callee");
        CALLER_VALS = &mut caller_vals as *mut _ as *mut _;
        CALLEE_VALS = &mut callee_vals as *mut _ as *mut _;
        SET_FUNC = Some(set_func);
        WRITE_VAL = Some(write_val);

        do_test();
        println!(r#"{{ "info": "done" }}"#);
    }
}