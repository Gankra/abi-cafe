//! The runtime actual types and functions that are injected into
//! compiled tests.

use std::sync::Arc;

use kdl_script::types::{Ty, TyIdx};
use linked_hash_map::LinkedHashMap;
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
        let test = self.tests[&key.test].clone();
        let full_test_name = self.full_test_name(key);
        let caller_impl = self
            .test_with_abi_impl(&test, key.caller.clone())
            .await
            .unwrap();
        let callee_impl = self
            .test_with_abi_impl(&test, key.callee.clone())
            .await
            .unwrap();
        run_dynamic_test(caller_impl, callee_impl, test_dylib, &full_test_name)
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
fn run_dynamic_test(
    caller_impl: Arc<TestForAbi>,
    callee_impl: Arc<TestForAbi>,
    test_dylib: &LinkOutput,
    full_test_name: &str,
) -> Result<RunOutput, RunError> {
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

    digest_test_run(
        caller_impl,
        callee_impl,
        caller_inputs,
        caller_outputs,
        callee_inputs,
        callee_outputs,
    )
}

fn digest_test_run(
    caller_impl: Arc<TestForAbi>,
    callee_impl: Arc<TestForAbi>,
    caller_inputs: WriteBuffer,
    caller_outputs: WriteBuffer,
    callee_inputs: WriteBuffer,
    callee_outputs: WriteBuffer,
) -> Result<RunOutput, RunError> {
    let mut callee = Functions::new();
    let mut caller = Functions::new();

    // As a basic sanity-check, make sure everything agrees on how
    // many tests actually executed. If this fails, then something
    // is very fundamentally broken and needs to be fixed.
    let all_func_ids = caller_impl.types.all_funcs().collect::<Vec<_>>();
    let expected_test_count = all_func_ids.len();
    if caller_inputs.funcs.len() != expected_test_count
        || caller_outputs.funcs.len() != expected_test_count
        || callee_inputs.funcs.len() != expected_test_count
        || callee_outputs.funcs.len() != expected_test_count
    {
        return Err(RunError::TestCountMismatch(
            expected_test_count,
            caller_inputs.funcs.len(),
            caller_outputs.funcs.len(),
            callee_inputs.funcs.len(),
            callee_outputs.funcs.len(),
        ));
    }

    let empty_func = Vec::new();
    let empty_arg = Vec::new();
    for (func_idx, func_id) in all_func_ids.into_iter().enumerate() {
        let func = caller_impl.types.realize_func(func_id);
        let caller_func = caller.entry(func.name.clone()).or_default();
        let callee_func = callee.entry(func.name.clone()).or_default();
        for (arg_idx, arg) in func.inputs.iter().enumerate() {
            let caller_arg = caller_func.entry(arg.name.clone()).or_default();
            let callee_arg = callee_func.entry(arg.name.clone()).or_default();

            let caller_arg_bytes = caller_inputs
                .funcs
                .get(func_idx)
                .unwrap_or(&empty_func)
                .get(arg_idx)
                .unwrap_or(&empty_arg);
            let callee_arg_bytes = callee_inputs
                .funcs
                .get(func_idx)
                .unwrap_or(&empty_func)
                .get(arg_idx)
                .unwrap_or(&empty_arg);

            add_field(
                &callee_impl,
                caller_arg_bytes,
                caller_arg,
                &mut 0,
                String::new(),
                arg.ty,
            );
            add_field(
                &callee_impl,
                callee_arg_bytes,
                callee_arg,
                &mut 0,
                String::new(),
                arg.ty,
            );
        }

        for (arg_idx, arg) in func.outputs.iter().enumerate() {
            let caller_arg = caller_func.entry(arg.name.clone()).or_default();
            let callee_arg = callee_func.entry(arg.name.clone()).or_default();

            let caller_output_bytes = caller_outputs
                .funcs
                .get(func_idx)
                .unwrap_or(&empty_func)
                .get(arg_idx)
                .unwrap_or(&empty_arg);
            let callee_output_bytes = callee_outputs
                .funcs
                .get(func_idx)
                .unwrap_or(&empty_func)
                .get(arg_idx)
                .unwrap_or(&empty_arg);

            add_field(
                &caller_impl,
                caller_output_bytes,
                caller_arg,
                &mut 0,
                String::new(),
                arg.ty,
            );
            add_field(
                &callee_impl,
                callee_output_bytes,
                callee_arg,
                &mut 0,
                String::new(),
                arg.ty,
            );
        }
    }

    Ok(RunOutput {
        callee,
        caller,
        caller_inputs,
        caller_outputs,
        callee_inputs,
        callee_outputs,
    })
}

fn format_bytes(input: &[Vec<u8>], cur_idx: &mut usize) -> String {
    use std::fmt::Write;

    let bytes = input.get(*cur_idx).map(|v| &v[..]).unwrap_or(&[]);
    let mut output = String::new();
    let mut looped = false;
    for byte in bytes {
        if looped {
            write!(&mut output, " ").unwrap();
        }
        write!(&mut output, "{:02x}", byte).unwrap();
        looped = true;
    }
    *cur_idx += 1;
    output
}

/// Recursive subroutine of write_var, which builds up rvalue paths and generates
/// appropriate match statements. Actual WRITE calls are done by write_leaf_field.
fn add_field(
    test_impl @ TestForAbi {
        inner: Test { types: program, .. },
        env,
        ..
    }: &TestForAbi,
    input: &[Vec<u8>],
    output: &mut LinkedHashMap<String, String>,
    cur_idx: &mut usize,
    cur_path: String,
    var_ty: TyIdx,
) {
    match program.realize_ty(var_ty) {
        Ty::Primitive(_) | Ty::Enum(_) => {
            // Hey an actual leaf, report it
            output.insert(cur_path, format_bytes(input, cur_idx));
            *cur_idx += 1;
        }
        Ty::Empty => {
            // nothing worth producing
        }
        Ty::Alias(alias_ty) => {
            // keep going but with the type changed
            add_field(test_impl, input, output, cur_idx, cur_path, alias_ty.real);
        }
        Ty::Pun(pun) => {
            // keep going but with the type changed
            let real_ty = program.resolve_pun(pun, env).unwrap();
            add_field(test_impl, input, output, cur_idx, cur_path, real_ty);
        }
        Ty::Array(array_ty) => {
            // recurse into each array index
            for i in 0..array_ty.len {
                let base = format!("{cur_path}[{i}]");
                add_field(test_impl, input, output, cur_idx, base, array_ty.elem_ty);
            }
        }
        Ty::Struct(struct_ty) => {
            // recurse into each field
            for field in &struct_ty.fields {
                let field_name = &field.ident;
                let base = format!("{cur_path}.{field_name}");
                add_field(test_impl, input, output, cur_idx, base, field.ty);
            }
        }
        Ty::Tagged(tagged_ty) => {
            // FIXME(variant_select): hardcoded to access variant 0 for now
            if let Some(variant) = tagged_ty.variants.get(0) {
                if let Some(fields) = &variant.fields {
                    for field in fields {
                        add_field(
                            test_impl,
                            input,
                            output,
                            cur_idx,
                            field.ident.to_string(),
                            field.ty,
                        );
                    }
                }
            }
        }
        Ty::Ref(ref_ty) => {
            // Add a deref, and recurse into the pointee
            let base = format!("(*{cur_path})");
            add_field(test_impl, input, output, cur_idx, base, ref_ty.pointee_ty);
        }
        Ty::Union(union_ty) => {
            // FIXME(variant_select): hardcoded to access field 0 for now
            if let Some(field) = union_ty.fields.get(0) {
                let field_name = &field.ident;
                let base = format!("{cur_path}.{field_name}");
                add_field(test_impl, input, output, cur_idx, base, field.ty);
            }
        }
    }
}
