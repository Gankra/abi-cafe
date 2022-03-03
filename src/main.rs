use std::env;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

mod abis;

use abis::*;

static TESTS: &[&str] = &["opaque_example", "u64", "u128", "structs"];

static RUST_TEST_PREFIX: &str = include_str!("../harness/rust_test_prefix.rs");
static C_TEST_PREFIX: &str = include_str!("../harness/c_test_prefix.h");

#[derive(Debug, thiserror::Error)]
enum BuildError {
    #[error("io error\n{0}")]
    Io(#[from] std::io::Error),
    #[error("parse error {0}\n{2}\n{}\n{:width$}^",
        .1.lines().nth(.2.position.line - 1).unwrap(),
        "",
        width=.2.position.col - 1,
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
}

#[derive(Debug, thiserror::Error)]
enum TestFailure {
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
struct TestReport {
    test: Test,
    results: Vec<Result<(), TestFailure>>,
}

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from("target/temp/");

    env::set_var("OUT_DIR", &out_dir);
    env::set_var("HOST", built_info::HOST);
    env::set_var("TARGET", built_info::TARGET);
    env::set_var("OPT_LEVEL", "3");

    let mut reports = Vec::new();
    for test_name in TESTS {
        let result = do_test(&out_dir, test_name);

        if let Err(e) = &result {
            eprintln!("test failed: {}", e);
        }
        reports.push((test_name, result));
    }

    println!();
    println!("Final Results:");
    // Do a cleaned up printout now
    let mut passes = 0;
    let mut fails = 0;
    let mut total_fails = 0;
    for (test_name, report) in reports {
        print!("{test_name}: ");
        match report {
            Err(e) => {
                println!("failed completely (bad input?)");
                total_fails += 1;
            }
            Ok(report) => {
                let passed = report.results.iter().filter(|r| r.is_ok()).count();
                println!("{passed}/{} passed!", report.results.len());
                for (test_func, result) in report.test.funcs.iter().zip(report.results.iter()) {
                    print!("  {test_name}::{}... ", test_func.name());
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

fn do_test(out_dir: &Path, test_name: &str) -> Result<TestReport, BuildError> {
    eprintln!("preparing test {test_name}");
    let test = read_test_manifest(test_name)?;

    let base_dir = if test.generated {
        PathBuf::from("target/temp/generated/")
    } else {
        PathBuf::from("impls/")
    };

    if test.generated {
        let rust_src = base_dir.join(format!("rust/{test_name}_rust_caller.rs"));
        let c_src = base_dir.join(format!("c/{test_name}_c_callee.c"));

        {
            std::fs::create_dir_all(rust_src.parent().unwrap())?;
            std::fs::create_dir_all(c_src.parent().unwrap())?;
            let mut rust_output = File::create(rust_src)?;
            generate_rust_caller(&mut rust_output, &test)?;

            let mut c_output = File::create(c_src)?;
            generate_c_callee(&mut c_output, &test)?;
        }
    }

    let caller = build_rust_caller(&base_dir, test_name)?;
    let callee = build_cc_callee(&base_dir, test_name)?;
    let dylib = build_harness(&base_dir, &caller, &callee, test_name)?;

    run_dynamic_test(&out_dir, test_name, &dylib, test)
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

fn build_cc_callee(base_path: &Path, test: &str) -> Result<String, BuildError> {
    let filename = format!("{test}_c_callee.c");
    let output_lib = format!("{test}_c_callee");
    let mut src = PathBuf::from(base_path);
    src.push("c");
    src.push(filename);

    cc::Build::new()
        .file(src)
        .cargo_metadata(false)
        // .warnings_into_errors(true)
        .try_compile(&output_lib)?;
    Ok(output_lib)
}
fn build_cc_caller(base_path: &Path, test: &str) -> Result<String, BuildError> {
    todo!()
}

fn build_rust_callee(base_path: &Path, test: &str) -> Result<String, BuildError> {
    todo!()
}
fn build_rust_caller(base_path: &Path, test: &str) -> Result<String, BuildError> {
    let filename = format!("{test}_rust_caller.rs");
    let output_lib = format!("{test}_rust_caller");

    let mut src = PathBuf::from(base_path);
    src.push("rust");
    src.push(filename);

    let out = Command::new("rustc")
        .arg("--crate-type")
        .arg("staticlib")
        .arg("--out-dir")
        .arg("target/temp/")
        .arg(src)
        .output()?;

    if !out.status.success() {
        Err(BuildError::RustCompile(out))
    } else {
        Ok(output_lib)
    }
}

fn build_harness(
    _base_path: &Path,
    caller_name: &str,
    callee_name: &str,
    test: &str,
) -> Result<String, BuildError> {
    let src = PathBuf::from("harness/harness.rs");
    let caller = format!("target/temp/{caller_name}");
    let callee = format!("target/temp/{callee_name}");
    let output = format!(
        "target/temp/{test}{}{}_harness.dll",
        caller_name.strip_prefix(test).unwrap(),
        callee_name.strip_prefix(test).unwrap()
    );

    let out = Command::new("rustc")
        .arg("-v")
        .arg("-l")
        .arg(&callee)
        .arg("-l")
        .arg(&caller)
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

fn generate_rust_caller<W: Write>(f: &mut W, test: &Test) -> Result<(), BuildError> {
    // Load test harness "headers"
    write!(f, "{}", RUST_TEST_PREFIX)?;

    // Forward-decl struct types
    let mut forward_decls = std::collections::HashMap::<String, String>::new();
    for function in &test.funcs {
        let function = function.sig()?;
        for val in function.inputs.iter().chain(function.output.as_ref()) {
            if let Some((name, decl)) = val.rust_forward_decl()? {
                match forward_decls.entry(name) {
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        if entry.get() != &decl {
                            return Err(BuildError::InconsistentStructDefinition {
                                name: entry.key().clone(),
                                old_decl: entry.remove(),
                                new_decl: decl,
                            });
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        writeln!(f, "{decl}")?;
                        entry.insert(decl);
                    }
                }
            }
        }
    }

    // Generate the extern block
    writeln!(f, "extern {{")?;
    for function in &test.funcs {
        let function = function.sig()?;
        write!(f, "  fn {}(", function.name)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            let ty = input.rust_arg_type()?;
            write!(f, "arg{idx}: {ty}, ",)?;
        }
        write!(f, ")")?;
        if let Some(output) = &function.output {
            let ty = output.rust_arg_type()?;
            write!(f, " -> {ty}")?;
        }
        writeln!(f, ";")?;
    }
    writeln!(f, "}}")?;
    writeln!(f)?;

    // Now generate the body
    writeln!(f, "#[no_mangle] pub extern fn do_test() {{")?;

    for function in &test.funcs {
        let function = function.sig()?;
        writeln!(f, "   unsafe {{")?;
        // writeln!(f, r#"        println!("test {}::{}\n");"#, test.name, function.name)?;
        // writeln!(f, r#"        println!("\n{}::{} rust caller inputs: ");"#, test.name, function.name)?;
        // writeln!(f)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            let ty = input.rust_arg_type()?;
            writeln!(f, "        let arg{idx}: {ty} = {};", input.rust_val()?)?;
        }
        writeln!(f)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            //    writeln!(f, r#"        println!("{{}}", arg{idx});"#)?;
            let val = format!("arg{idx}");
            writeln!(f, "{}", input.rust_write_val("CALLER_INPUTS", &val)?)?;
        }
        writeln!(f)?;
        write!(f, "        ")?;
        if let Some(output) = &function.output {
            let ty = output.rust_arg_type()?;
            write!(f, "        let output: {ty} = ")?;
        }
        write!(f, "{}(", function.name)?;
        for (idx, _input) in function.inputs.iter().enumerate() {
            write!(f, "arg{idx}, ")?;
        }
        writeln!(f, ");")?;
        writeln!(f)?;
        if let Some(output) = &function.output {
            //    writeln!(f, r#"        println!("\n{}::{} rust caller outputs: ");"#, test.name, function.name)?;
            //    writeln!(f, r#"        println!("{{}}", output);"#)?;
            writeln!(f, "{}", output.rust_write_val("CALLER_OUTPUTS", "output")?)?;
        }
        writeln!(
            f,
            "        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);"
        )?;
        writeln!(f, "   }}")?;
    }

    writeln!(f, "}}")?;

    Ok(())
}

fn generate_c_callee<W: Write>(f: &mut W, test: &Test) -> Result<(), BuildError> {
    write!(f, "{}", C_TEST_PREFIX)?;

    // Forward-decl struct types
    let mut forward_decls = std::collections::HashSet::new();
    for function in &test.funcs {
        let function = function.sig()?;
        for val in function.inputs.iter().chain(function.output.as_ref()) {
            if let Some((name, decl)) = val.c_forward_decl()? {
                if forward_decls.insert(name) {
                    writeln!(f, "{decl}")?;
                }
            }
        }
    }

    // Generate the impls
    for function in &test.funcs {
        let function = function.sig()?;
        if let Some(output) = &function.output {
            write!(f, "{} ", output.c_arg_type()?)?;
        } else {
            write!(f, "void ")?;
        }
        write!(f, "{}(", function.name)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            if idx != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{} arg{idx}", input.c_arg_type()?)?;
        }
        writeln!(f, ") {{")?;

        // writeln!(f, r#"    printf("\n{}::{} C callee inputs: \n");"#, test.name, function.name)?;
        writeln!(f)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            // let formatter = input.cfmt();
            // writeln!(f, r#"    printf("%" {formatter} "\n", arg{idx});"#)?;
            let val = format!("arg{idx}");
            writeln!(f, "{}", input.c_write_val("CALLEE_INPUTS", &val)?)?;
        }
        writeln!(f)?;
        if let Some(output) = &function.output {
            // let formatter = output.cfmt();
            writeln!(
                f,
                "    {} output = {};",
                output.c_arg_type()?,
                output.c_val()?
            )?;
            // writeln!(f, r#"    printf("\n{}::{} C callee outputs: \n");"#, test.name, function.name)?;
            // writeln!(f, r#"    printf("%" {formatter} "\n", output);"#)?;
            writeln!(f, "{}", output.c_write_val("CALLEE_OUTPUTS", "output")?)?;
            writeln!(f, "    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);")?;
            writeln!(f, "    return output;")?;
        } else {
            writeln!(f, "    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);")?;
        }
        writeln!(f, "}}")?;
        writeln!(f)?;
    }

    Ok(())
}

fn run_dynamic_test(
    base_path: &Path,
    test_name: &str,
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
        eprintln!("running test {test_name}");
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
            // TODO: fix this abstraction boundary?
            match result {
                Ok(()) => {
                    eprintln!("Test {}::{}... passed!", test.name, func.name());
                }
                Err(e) => {
                    eprintln!("Test {}::{}... failed!", test.name, func.name());
                    eprintln!("{}", e);
                }
            }
        }

        Ok(TestReport { test, results })
    }
}
