//! The runtime actual types and functions that are injected into
//! compiled tests.

use serde::Deserialize;
use serde::Serialize;
use tracing::info;

use crate::error::*;
use crate::report::*;
use crate::*;

impl TestHarness {
    #[allow(dead_code)]
    pub async fn run_dylib_test(
        &self,
        _key: &TestKey,
        linked_test: &LinkOutput,
    ) -> Result<RunOutput, RunError> {
        let output = run_dylib_test(linked_test)?;
        Ok(output)
    }

    pub async fn run_bin_test(
        &self,
        key: &TestKey,
        linked_test: &LinkOutput,
    ) -> Result<RunOutput, RunError> {
        let test = self.test(&key.test);
        let output = run_bin_test(test, linked_test)?;
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
    write_val_inner(
        test,
        val_idx,
        std::slice::from_raw_parts(input, size as usize),
    )
}

fn write_val_inner(test: &mut TestBuffer, val_idx: u32, data: &[u8]) {
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
    val.bytes = data.to_vec();
}

/// Run the test!
///
/// See the README for a high-level description of this design.
fn run_dylib_test(test_dylib: &LinkOutput) -> Result<RunOutput, RunError> {
    // Initialize all the buffers the tests will write to
    let mut caller_vals = TestBuffer::new();
    let mut callee_vals = TestBuffer::new();

    unsafe {
        info!(
            "running     {}",
            test_dylib.test_bin.file_name().unwrap_or_default()
        );
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

/// Run the test!
///
/// See the README for a high-level description of this design.
fn run_bin_test(test: Arc<Test>, test_bin: &LinkOutput) -> Result<RunOutput, RunError> {
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    #[serde(tag = "info")]
    enum HarnessJsonMessage {
        Func {
            id: HarnessSide,
            func: u32,
        },
        Val {
            id: HarnessSide,
            val: u32,
            bytes: Vec<u8>,
        },
        Done,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    enum HarnessSide {
        Caller,
        Callee,
    }

    // Initialize all the buffers the tests will write to
    let mut caller_vals = TestBuffer::new();
    let mut callee_vals = TestBuffer::new();
    let mut finished_clean = false;

    unsafe {
        info!(
            "running     {}",
            test_bin.test_bin.file_name().unwrap_or_default()
        );
        // Load the dylib of the test, and get its test_start symbol

        debug!("loading     {}", &test_bin.test_bin);
        let mut cmd = Command::new(&test_bin.test_bin);
        let output = cmd.output().map_err(|e| RunError::ExecError {
            bin: test_bin.test_bin.clone(),
            e,
        })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let Ok(message): Result<HarnessJsonMessage, _> = serde_json::from_str(line) else {
                finished_clean = false;
                continue;
            };
            match message {
                HarnessJsonMessage::Func { id, func } => {
                    let buf = match id {
                        HarnessSide::Caller => &mut caller_vals,
                        HarnessSide::Callee => &mut callee_vals,
                    };
                    set_func(buf, func)
                }
                HarnessJsonMessage::Val { id, val, bytes } => {
                    let buf = match id {
                        HarnessSide::Caller => &mut caller_vals,
                        HarnessSide::Callee => &mut callee_vals,
                    };
                    write_val_inner(buf, val, &bytes)
                }
                HarnessJsonMessage::Done => {
                    finished_clean = true;
                }
            }
        }
        if !output.status.success() {
            let (caller_func_idx, caller_val_idx, caller_func) = best_vals(&test, &caller_vals);
            let (callee_func_idx, callee_val_idx, callee_func) = best_vals(&test, &callee_vals);
            return Err(RunError::BadExit {
                status: output.status,
                caller_func_idx,
                caller_val_idx,
                caller_func,
                callee_func_idx,
                callee_val_idx,
                callee_func,
            });
        }
    }

    if !finished_clean {
        return Err(RunError::InvalidMessages {
            caller_funcs: caller_vals,
            callee_funcs: callee_vals,
        });
    }

    caller_vals.finish_tests()?;
    callee_vals.finish_tests()?;

    Ok(RunOutput {
        caller_funcs: caller_vals,
        callee_funcs: callee_vals,
    })
}

fn best_vals(test: &Test, vals: &TestBuffer) -> (usize, usize, String) {
    let default_funcs = FuncBuffer::default();
    let func_idx = vals.cur_func.unwrap_or(0);
    let funcs = vals.funcs.get(func_idx).unwrap_or(&default_funcs);
    let val_idx = funcs.vals.len().saturating_sub(1);
    let func_name = test.types.realize_func(func_idx).name.to_string();

    (func_idx, val_idx, func_name)
}
