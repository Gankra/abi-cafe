//! The runtime actual types and functions that are injected into
//! compiled tests.

use serde::Serialize;
use tracing::info;

use crate::error::*;
use crate::report::*;
use crate::*;

impl TestHarness {
    pub async fn run_dynamic_test(
        &self,
        key: &TestKey,
        test_dylib: &LinkOutput,
    ) -> Result<RunOutput, RunError> {
        let full_test_name = self.full_test_name(key);
        let output = run_dynamic_test(test_dylib, &full_test_name)?;
        Ok(output)
    }
}

/// Tests write back the raw bytes of their values to a WriteBuffer.
#[derive(Debug, Serialize)]
pub struct TestBuffer {
    pub funcs: Vec<FuncBuffer>,
    pub cur_func: Option<usize>,
    pub had_missing_set_func: bool,
    pub had_double_writes: Vec<(usize, usize)>,
}

#[derive(Debug, Serialize, Default)]
pub struct FuncBuffer {
    pub vals: Vec<ValBuffer>,
}

#[derive(Debug, Serialize, Default)]
pub struct ValBuffer {
    pub bytes: Vec<u8>,
}

impl TestBuffer {
    fn new() -> Self {
        // Preload the hierarchy for the first test.
        TestBuffer {
            funcs: vec![],
            cur_func: None,
            had_missing_set_func: false,
            had_double_writes: vec![],
        }
    }
    fn finish_tests(&mut self) -> Result<(), RunError> {
        if self.had_missing_set_func {
            return Err(RunError::MissingSetFunc);
        }
        if let Some(&(func, val)) = self.had_double_writes.first() {
            return Err(RunError::DoubleWrite { func, val });
        }

        Ok(())
    }
}

// The signatures of the interface from our perspective.
// From the test's perspective the WriteBuffers are totally opaque.
pub type SetFuncCallback = unsafe extern "C" fn(&mut TestBuffer, u32) -> ();
pub type WriteValCallback = unsafe extern "C" fn(&mut TestBuffer, u32, *const u8, u32) -> ();
pub type TestInit =
    unsafe extern "C" fn(SetFuncCallback, WriteValCallback, &mut TestBuffer, &mut TestBuffer) -> ();

pub unsafe extern "C" fn set_func(test: &mut TestBuffer, func: u32) {
    let idx = func as usize;
    // If things aren't in-order, add empty entries to make the index exist
    let new_len = test.funcs.len().max(idx + 1);
    test.funcs
        .resize_with(new_len, || FuncBuffer { vals: vec![] });
    test.cur_func = Some(idx);
}

pub unsafe extern "C" fn write_val(
    test: &mut TestBuffer,
    val_idx: u32,
    input: *const u8,
    size: u32,
) {
    // Get the current function
    let Some(func_idx) = test.cur_func else {
        test.had_missing_set_func = true;
        return;
    };
    let func = test
        .funcs
        .get_mut(func_idx)
        .expect("harness corrupted its own func idx!?");

    // Get the value of the function (making room for it if need be)
    let val_idx = val_idx as usize;
    let new_len = func.vals.len().max(val_idx + 1);
    func.vals
        .resize_with(new_len, || ValBuffer { bytes: vec![] });
    let val = &mut func.vals[val_idx];

    // Push all the bytes of the value
    if !val.bytes.is_empty() {
        test.had_double_writes.push((func_idx, val_idx));
        return;
    }
    let data = std::slice::from_raw_parts(input, size as usize);
    val.bytes = data.to_vec();
}

/// Run the test!
///
/// See the README for a high-level description of this design.
fn run_dynamic_test(test_dylib: &LinkOutput, _full_test_name: &str) -> Result<RunOutput, RunError> {
    // Initialize all the buffers the tests will write to
    let mut caller_vals = TestBuffer::new();
    let mut callee_vals = TestBuffer::new();

    unsafe {
        info!("running     {}", test_dylib.test_bin.file_name().unwrap());
        // Load the dylib of the test, and get its test_start symbol
        debug!("loading     {}", &test_dylib.test_bin);
        let lib = libloading::Library::new(&test_dylib.test_bin)?;
        let do_test: libloading::Symbol<TestInit> = lib.get(b"test_start")?;
        debug!("calling harness dynamic function");
        // Actually run the test!
        do_test(set_func, write_val, &mut caller_vals, &mut callee_vals);

        // Finalize the buffers (clear all the pending values).
        caller_vals.finish_tests()?;
        callee_vals.finish_tests()?;
    }

    Ok(RunOutput {
        caller_funcs: caller_vals,
        callee_funcs: callee_vals,
    })
}
