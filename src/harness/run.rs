//! The runtime actual types and functions that are injected into
//! compiled tests.

use serde::Serialize;

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
        run_dynamic_test(test_dylib, &full_test_name)
    }
}

/// Tests write back the raw bytes of their values to a WriteBuffer.
///
/// This hierarchical design is confusing as hell, but represents the
/// nested levels of abstraction we are concerned with:
///
/// subtests (functions) => values (args/returns) => subfields => bytes.
///
/// Having this much hierarchy means that we can specifically say
/// "ah yeah, on test 3 the two sides disagreed on arg2.field1.field2"
/// and also reduces the chance of failures in one test "cascading"
/// into the subsequent ones.
#[derive(Debug, Serialize)]
pub struct WriteBuffer {
    pub funcs: Vec<Vec<Vec<Vec<u8>>>>,
}

impl WriteBuffer {
    fn new() -> Self {
        // Preload the hierarchy for the first test.
        WriteBuffer {
            funcs: vec![vec![vec![]]],
        }
    }
    fn finish_tests(&mut self) {
        // Remove the pending test
        self.funcs.pop();
    }
}

// The signatures of the interface from our perspective.
// From the test's perspective the WriteBuffers are totally opaque.
pub type WriteCallback = unsafe extern "C" fn(&mut WriteBuffer, *const u8, u32) -> ();
pub type FinishedValCallback = unsafe extern "C" fn(&mut WriteBuffer) -> ();
pub type FinishedFuncCallback = unsafe extern "C" fn(&mut WriteBuffer, &mut WriteBuffer) -> ();
pub type TestInit = unsafe extern "C" fn(
    WriteCallback,
    FinishedValCallback,
    FinishedFuncCallback,
    &mut WriteBuffer,
    &mut WriteBuffer,
    &mut WriteBuffer,
    &mut WriteBuffer,
) -> ();

pub unsafe extern "C" fn write_field(output: &mut WriteBuffer, input: *const u8, size: u32) {
    // Push the bytes of an individual field
    let data = std::slice::from_raw_parts(input, size as usize);
    output
        .funcs
        .last_mut() // values
        .unwrap()
        .last_mut() // fields
        .unwrap()
        .push(data.to_vec());
}
pub unsafe extern "C" fn finished_val(output: &mut WriteBuffer) {
    // This value is finished, push a new entry
    output
        .funcs
        .last_mut() // values
        .unwrap()
        .push(vec![]);
}
pub unsafe extern "C" fn finished_func(output1: &mut WriteBuffer, output2: &mut WriteBuffer) {
    // Remove the pending value
    output1
        .funcs
        .last_mut() // values
        .unwrap()
        .pop()
        .unwrap();
    output2
        .funcs
        .last_mut() // values
        .unwrap()
        .pop()
        .unwrap();

    // Push a new pending function
    output1.funcs.push(vec![vec![]]);
    output2.funcs.push(vec![vec![]]);
}

/// Run the test!
///
/// See the README for a high-level description of this design.
fn run_dynamic_test(test_dylib: &LinkOutput, full_test_name: &str) -> Result<RunOutput, RunError> {
    // Initialize all the buffers the tests will write to
    let mut caller_inputs = WriteBuffer::new();
    let mut caller_outputs = WriteBuffer::new();
    let mut callee_inputs = WriteBuffer::new();
    let mut callee_outputs = WriteBuffer::new();

    unsafe {
        // Load the dylib of the test, and get its test_start symbol
        eprintln!("loading: {}", &test_dylib.test_bin);
        let lib = libloading::Library::new(&test_dylib.test_bin)?;
        let do_test: libloading::Symbol<TestInit> = lib.get(b"test_start")?;
        eprintln!("running    {full_test_name}");

        // Actually run the test!
        do_test(
            write_field,
            finished_val,
            finished_func,
            &mut caller_inputs,
            &mut caller_outputs,
            &mut callee_inputs,
            &mut callee_outputs,
        );

        // Finalize the buffers (clear all the pending values).
        caller_inputs.finish_tests();
        caller_outputs.finish_tests();
        callee_inputs.finish_tests();
        callee_outputs.finish_tests();
    }

    Ok(RunOutput {
        caller_inputs,
        caller_outputs,
        callee_inputs,
        callee_outputs,
    })
}
