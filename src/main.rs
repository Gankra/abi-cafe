use std::env;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

mod abis;

use abis::*;

pub static TESTS: &[&str] = &["opaque_example", "structs", "core_primitives", "ui128"];

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("io error\n{0}")]
    Io(#[from] std::io::Error),
    #[error("parse error {0}\n{2}\n{}\n{:width$}^",
        .1.lines().nth(.2.position.line.saturating_sub(1)).unwrap(),
        "",
        width=.2.position.col.saturating_sub(1),
)]
    ParseError(String, String, ron::error::Error),
    #[error("rust compile error \n{} \n{}", 
        std::str::from_utf8(&.0.stdout).unwrap(),
        std::str::from_utf8(&.0.stderr).unwrap())]
    RustCompile(std::process::Output),
    #[error("c compile errror\n{0}")]
    CCompile(#[from] cc::Error),
    #[error("test loading error (dynamic linking failed)\n{0}")]
    LoadError(#[from] libloading::Error),
    #[error("test uses features unsupported by this backend\n{0}")]
    Unsupported(#[from] abis::GenerateError),
    #[error("wrong number of tests reported! \nExpected {0} \nGot (caller_in: {1}, caller_out: {2}, callee_in: {3}, callee_out: {4})")]
    TestCountMismatch(usize, usize, usize, usize, usize),
    #[error("Two structs had the name {name}, but different layout! \nExpected {old_decl} \nGot {new_decl}")]
    InconsistentStructDefinition {
        name: String,
        old_decl: String,
        new_decl: String,
    },
    #[error("If you use the Handwritten calling convention, all functions in the test must use only that.")]
    HandwrittenMixing,
}

#[derive(Debug, thiserror::Error)]
pub enum TestFailure {
    #[error("test {0} input {1} field {2} mismatch \ncaller: {3:02X?} \ncallee: {4:02X?}")]
    InputFieldMismatch(usize, usize, usize, Vec<u8>, Vec<u8>),
    #[error("test {0} output {1} field {2} mismatch \ncaller: {3:02X?} \ncallee: {4:02X?}")]
    OutputFieldMismatch(usize, usize, usize, Vec<u8>, Vec<u8>),
    #[error("test {0} input {1} field count mismatch \ncaller: {2:#02X?} \ncallee: {3:#02X?}")]
    InputFieldCountMismatch(usize, usize, Vec<Vec<u8>>, Vec<Vec<u8>>),
    #[error("test {0} output {1} field count mismatch \ncaller: {2:#02X?} \ncallee: {3:#02X?}")]
    OutputFieldCountMismatch(usize, usize, Vec<Vec<u8>>, Vec<Vec<u8>>),
    #[error("test {0} input count mismatch \ncaller: {1:#02X?} \ncallee: {2:#02X?}")]
    InputCountMismatch(usize, Vec<Vec<Vec<u8>>>, Vec<Vec<Vec<u8>>>),
    #[error("test {0} output count mismatch \ncaller: {1:#02X?} \ncallee: {2:#02X?}")]
    OutputCountMismatch(usize, Vec<Vec<Vec<u8>>>, Vec<Vec<Vec<u8>>>),
}

#[derive(Debug)]
pub struct TestReport {
    test: Test,
    results: Vec<Result<(), TestFailure>>,
}

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn main() -> Result<(), Box<dyn Error>> {
    // generate_primitive_tests();

    let out_dir = PathBuf::from("target/temp/");

    env::set_var("OUT_DIR", &out_dir);
    env::set_var("HOST", built_info::HOST);
    env::set_var("TARGET", built_info::TARGET);
    env::set_var("OPT_LEVEL", "3");

    let mut reports = Vec::new();
    for test_name in TESTS {
        for (caller, callee) in TEST_PAIRS {
            let result = do_test(&out_dir, *caller, *callee, test_name);

            if let Err(e) = &result {
                eprintln!("test failed: {}", e);
            }
            reports.push((test_name, caller.name(), callee.name(), result));
        }
    }

    println!();
    println!("Final Results:");
    // Do a cleaned up printout now
    let mut passes = 0;
    let mut fails = 0;
    let mut total_fails = 0;
    for (test_name, caller_name, callee_name, report) in reports {
        let pretty_test_name = full_test_name(test_name, caller_name, callee_name);
        print!("{pretty_test_name}: ");
        match report {
            Err(_) => {
                println!("failed completely (bad input?)");
                total_fails += 1;
            }
            Ok(report) => {
                let passed = report.results.iter().filter(|r| r.is_ok()).count();
                println!("{passed}/{} passed!", report.results.len());
                for (test_func, result) in report.test.funcs.iter().zip(report.results.iter()) {
                    let subtest_name =
                        full_subtest_name(test_name, caller_name, callee_name, &test_func.name);
                    print!("  {}... ", subtest_name);
                    if result.is_ok() {
                        println!("passed!");
                        passes += 1;
                    } else {
                        println!("failed!");
                        fails += 1;
                    }
                }
            }
        }
        println!();
    }
    println!("total: {passes} passed, {fails} failed, {total_fails} completely failed");

    Ok(())
}

fn do_test(
    _out_dir: &Path,
    caller: AbiRef,
    callee: AbiRef,
    test_name: &str,
) -> Result<TestReport, BuildError> {
    eprintln!("preparing test {test_name}");
    let caller_name = caller.name();
    let caller_src_ext = caller.src_ext();
    let callee_name = callee.name();
    let callee_src_ext = callee.src_ext();

    let test = read_test_manifest(test_name)?;
    let is_handwritten = test.funcs.iter().any(|f| {
        f.conventions
            .iter()
            .any(|c| matches!(c, CallingConvention::Handwritten))
    });
    let is_all_handwritten = test.funcs.iter().all(|f| {
        f.conventions
            .iter()
            .all(|c| matches!(c, CallingConvention::Handwritten))
    });

    if is_handwritten && !is_all_handwritten {
        return Err(BuildError::HandwrittenMixing);
    }

    let src_dir = if is_handwritten {
        PathBuf::from("handwritten_impls/")
    } else {
        PathBuf::from("generated_impls/")
    };

    let caller_src = src_dir.join(format!(
        "{caller_name}/{test_name}_{caller_name}_caller.{caller_src_ext}"
    ));
    let callee_src = src_dir.join(format!(
        "{callee_name}/{test_name}_{callee_name}_callee.{callee_src_ext}"
    ));
    let caller_lib = format!("{test_name}_{caller_name}_caller");
    let callee_lib = format!("{test_name}_{callee_name}_callee");

    if !is_handwritten {
        // If the impl isn't handwritten, then we need to generate it.
        std::fs::create_dir_all(caller_src.parent().unwrap())?;
        std::fs::create_dir_all(callee_src.parent().unwrap())?;
        let mut caller_output = File::create(&caller_src)?;
        caller.generate_caller(&mut caller_output, &test)?;

        let mut callee_output = File::create(&callee_src)?;
        callee.generate_callee(&mut callee_output, &test)?;
    }

    let caller_lib = caller.compile_caller(&caller_src, &caller_lib)?;
    let callee_lib = callee.compile_callee(&callee_src, &callee_lib)?;
    let dylib = build_harness(
        caller_name,
        &caller_lib,
        callee_name,
        &callee_lib,
        test_name,
    )?;

    run_dynamic_test(test_name, caller_name, callee_name, &dylib, test)
}

fn read_test_manifest(test_name: &str) -> Result<Test, BuildError> {
    let test_file = format!("tests/{test_name}.ron");
    let file = File::open(&test_file)?;
    let mut reader = BufReader::new(file);
    let mut input = String::new();
    reader.read_to_string(&mut input)?;
    let test: Test =
        ron::from_str(&input).map_err(|e| BuildError::ParseError(test_file, input, e))?;
    Ok(test)
}

fn build_harness(
    caller_name: &str,
    caller_lib: &str,
    callee_name: &str,
    callee_lib: &str,
    test: &str,
) -> Result<String, BuildError> {
    let src = PathBuf::from("harness/harness.rs");
    let output = format!("target/temp/{test}_{caller_name}_calls_{callee_name}_harness.dll");

    let out = Command::new("rustc")
        .arg("-v")
        .arg("-L")
        .arg("target/temp/")
        .arg("-l")
        .arg(&callee_lib)
        .arg("-l")
        .arg(&caller_lib)
        .arg("--crate-type")
        .arg("dylib")
        // .arg("--out-dir")
        // .arg("target/temp/")
        .arg("-o")
        .arg(&output)
        .arg(&src)
        .output()?;

    if !out.status.success() {
        Err(BuildError::RustCompile(out))
    } else {
        Ok(output)
    }
}

fn run_dynamic_test(
    test_name: &str,
    caller_name: &str,
    callee_name: &str,
    dylib: &str,
    test: Test,
) -> Result<TestReport, BuildError> {
    type WriteCallback = unsafe extern "C" fn(&mut WriteBuffer, *const u8, u32) -> ();
    type FinishedValCallback = unsafe extern "C" fn(&mut WriteBuffer) -> ();
    type FinishedFuncCallback = unsafe extern "C" fn(&mut WriteBuffer, &mut WriteBuffer) -> ();
    type TestInit = unsafe extern "C" fn(
        WriteCallback,
        FinishedValCallback,
        FinishedFuncCallback,
        &mut WriteBuffer,
        &mut WriteBuffer,
        &mut WriteBuffer,
        &mut WriteBuffer,
    ) -> ();

    /// Tests write back the raw bytes of their values to a WriteBuffer in a
    /// hierarchical way: tests (functions) => values => fields => bytes.
    struct WriteBuffer {
        funcs: Vec<Vec<Vec<Vec<u8>>>>,
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

    unsafe extern "C" fn write_field(output: &mut WriteBuffer, input: *const u8, size: u32) {
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
    unsafe extern "C" fn finished_val(output: &mut WriteBuffer) {
        // This value is finished, push a new entry
        output
            .funcs
            .last_mut() // values
            .unwrap()
            .push(vec![]);
    }
    unsafe extern "C" fn finished_func(output1: &mut WriteBuffer, output2: &mut WriteBuffer) {
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

    unsafe {
        let mut caller_inputs = WriteBuffer::new();
        let mut caller_outputs = WriteBuffer::new();
        let mut callee_inputs = WriteBuffer::new();
        let mut callee_outputs = WriteBuffer::new();

        let lib = libloading::Library::new(dylib)?;
        let do_test: libloading::Symbol<TestInit> = lib.get(b"test_start")?;
        eprintln!("running test");
        do_test(
            write_field,
            finished_val,
            finished_func,
            &mut caller_inputs,
            &mut caller_outputs,
            &mut callee_inputs,
            &mut callee_outputs,
        );

        caller_inputs.finish_tests();
        caller_outputs.finish_tests();
        callee_inputs.finish_tests();
        callee_outputs.finish_tests();

        let expected_test_count = test.funcs.len();
        if caller_inputs.funcs.len() != expected_test_count
            || caller_outputs.funcs.len() != expected_test_count
            || callee_inputs.funcs.len() != expected_test_count
            || callee_outputs.funcs.len() != expected_test_count
        {
            return Err(BuildError::TestCountMismatch(
                expected_test_count,
                caller_inputs.funcs.len(),
                caller_outputs.funcs.len(),
                callee_inputs.funcs.len(),
                callee_outputs.funcs.len(),
            ));
        }

        let mut results: Vec<Result<(), TestFailure>> = Vec::new();
        'funcs: for (
            func_idx,
            (((caller_inputs, caller_outputs), callee_inputs), callee_outputs),
        ) in caller_inputs
            .funcs
            .into_iter()
            .zip(caller_outputs.funcs.into_iter())
            .zip(callee_inputs.funcs.into_iter())
            .zip(callee_outputs.funcs.into_iter())
            .enumerate()
        {
            if caller_inputs.len() != callee_inputs.len() {
                results.push(Err(TestFailure::InputCountMismatch(
                    func_idx,
                    caller_inputs,
                    callee_inputs,
                )));
                continue 'funcs;
            }
            if caller_outputs.len() != callee_outputs.len() {
                results.push(Err(TestFailure::OutputCountMismatch(
                    func_idx,
                    caller_outputs,
                    callee_outputs,
                )));
                continue 'funcs;
            }

            // Process Inputs
            for (input_idx, (caller_val, callee_val)) in caller_inputs
                .into_iter()
                .zip(callee_inputs.into_iter())
                .enumerate()
            {
                if caller_val.len() != callee_val.len() {
                    results.push(Err(TestFailure::InputFieldCountMismatch(
                        func_idx, input_idx, caller_val, callee_val,
                    )));
                    continue 'funcs;
                }
                for (field_idx, (caller_field, callee_field)) in caller_val
                    .into_iter()
                    .zip(callee_val.into_iter())
                    .enumerate()
                {
                    if caller_field != callee_field {
                        results.push(Err(TestFailure::InputFieldMismatch(
                            func_idx,
                            input_idx,
                            field_idx,
                            caller_field,
                            callee_field,
                        )));
                        continue 'funcs;
                    }
                }
            }

            // Process Outputs
            for (output_idx, (caller_val, callee_val)) in caller_outputs
                .into_iter()
                .zip(callee_outputs.into_iter())
                .enumerate()
            {
                if caller_val.len() != callee_val.len() {
                    results.push(Err(TestFailure::OutputFieldCountMismatch(
                        func_idx, output_idx, caller_val, callee_val,
                    )));
                    continue 'funcs;
                }
                for (field_idx, (caller_field, callee_field)) in caller_val
                    .into_iter()
                    .zip(callee_val.into_iter())
                    .enumerate()
                {
                    if caller_field != callee_field {
                        results.push(Err(TestFailure::OutputFieldMismatch(
                            func_idx,
                            output_idx,
                            field_idx,
                            caller_field,
                            callee_field,
                        )));
                        continue 'funcs;
                    }
                }
            }

            // If we got this far then the test passes
            results.push(Ok(()));
        }

        for (result, func) in results.iter().zip(test.funcs.iter()) {
            let subtest_name = full_subtest_name(test_name, caller_name, callee_name, &func.name);
            // TODO: fix this abstraction boundary?
            match result {
                Ok(()) => {
                    eprintln!("Test {subtest_name}... passed!");
                }
                Err(e) => {
                    eprintln!("Test {subtest_name}... failed!");
                    eprintln!("{}", e);
                }
            }
        }

        Ok(TestReport { test, results })
    }
}

fn full_test_name(test_name: &str, caller_name: &str, callee_name: &str) -> String {
    format!("{test_name}::{caller_name}_calls_{callee_name}")
}

fn full_subtest_name(
    test_name: &str,
    caller_name: &str,
    callee_name: &str,
    func_name: &str,
) -> String {
    format!("{test_name}::{caller_name}_calls_{callee_name}::{func_name}")
}

/*
fn generate_primitive_tests() {
    let tests: &[(&str, &[Val])] = &[
        (
            "core_primitives",
            &[
                Val::Int(IntVal::c_int64_t(0x1a2b3c4d_23eaf142)),
                Val::Int(IntVal::c_int32_t(0x1a2b3c4d)),
                Val::Int(IntVal::c_int16_t(0x1a2b)),
                Val::Int(IntVal::c_int8_t(0x1a)),
                Val::Int(IntVal::c_uint64_t(0x1a2b3c4d_23eaf142)),
                Val::Int(IntVal::c_uint32_t(0x1a2b3c4d)),
                Val::Int(IntVal::c_uint16_t(0x1a2b)),
                Val::Int(IntVal::c_uint8_t(0x1a)),
                Val::Bool(true),
                Val::Float(FloatVal::c_float(-4921.3527)),
                Val::Float(FloatVal::c_double(809239021.392)),
            ],
        ),
        (
            "ui128",
            &[
                Val::Int(IntVal::c__int128(0x1a2b3c4d_23eaf142_7a320c01_e0120a82)),
                Val::Int(IntVal::c__uint128(0x1a2b3c4d_23eaf142_7a320c01_e0120a82)),
            ],
        ),
    ];

    for (test_name, vals) in tests {
        let mut test = Test {
            name: test_name.to_string(),
            funcs: Vec::new(),
        };

        for val in vals.iter() {
            let new_val = || -> Val {
                // TODO: actually perturb the values?
                val.clone()
            };

            let val_name = val.rust_arg_type().unwrap();

            test.funcs.push(Func {
                name: format!("{val_name}_val_in"),
                conventions: vec![CallingConvention::All],
                inputs: vec![new_val()],
                output: None,
            });

            test.funcs.push(Func {
                name: format!("{val_name}_val_out"),
                conventions: vec![CallingConvention::All],
                inputs: vec![],
                output: Some(new_val()),
            });

            test.funcs.push(Func {
                name: format!("{val_name}_val_in_out"),
                conventions: vec![CallingConvention::All],
                inputs: vec![new_val()],
                output: Some(new_val()),
            });

            for len in 2..=16 {
                test.funcs.push(Func {
                    name: format!("{val_name}_val_in_{len}"),
                    conventions: vec![CallingConvention::All],
                    inputs: (0..len).map(|_| new_val()).collect(),
                    output: None,
                });
            }

            for len in 1..=16 {
                test.funcs.push(Func {
                    name: format!("struct_{val_name}_val_in_{len}"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Struct(
                        format!("{val_name}_val_in_{len}"),
                        (0..len).map(|_| new_val()).collect(),
                    )],
                    output: None,
                });
            }
            for idx in 0..=16 {
                let mut inputs = (0..16).map(|_| new_val()).collect::<Vec<_>>();
                inputs.insert(idx, Val::Int(IntVal::c_uint8_t(0xeb)));
                inputs.insert(17 - idx, Val::Float(FloatVal::c_float(1234.456)));
                test.funcs.push(Func {
                    name: format!("{val_name}_val_in_{idx}_perturbed"),
                    conventions: vec![CallingConvention::All],
                    inputs: inputs,
                    output: None,
                });
            }
            for idx in 0..=16 {
                let mut inputs = (0..16).map(|_| new_val()).collect::<Vec<_>>();
                inputs.insert(idx, Val::Int(IntVal::c_uint8_t(0xeb)));
                inputs.insert(16 - idx, Val::Float(FloatVal::c_float(1234.456)));
                test.funcs.push(Func {
                    name: format!("struct_{val_name}_val_in_{idx}_perturbed"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Struct(
                        format!("{val_name}_val_in_{idx}_perturbed"),
                        inputs,
                    )],
                    output: None,
                });
            }
        }
        let mut file = std::fs::File::create(format!("tests/{test_name}.ron")).unwrap();
        let output = ron::to_string(&test).unwrap();
        file.write_all(output.as_bytes()).unwrap();
    }
}
*/