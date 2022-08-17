mod abis;
mod cli;
mod procgen;
mod report;

use abis::*;
use linked_hash_map::LinkedHashMap;
use report::*;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Slurps up details of how this crate was compiled, which we can use
/// to better compile the actual tests since we're currently compiling them on
/// the same platform with the same toolchains!
pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub output_format: OutputFormat,
    pub procgen_tests: bool,
    pub run_conventions: Vec<CallingConvention>,
    pub run_impls: Vec<String>,
    pub run_pairs: Vec<(String, String)>,
    pub run_tests: Vec<String>,
    pub rustc_codegen_backends: Vec<(String, String)>,
}

#[derive(Debug, thiserror::Error)]
#[error("some tests failed")]
pub struct TestsFailed {}

fn main() -> Result<(), Box<dyn Error>> {
    eprintln!("starting!");
    let cfg = cli::make_app();
    eprintln!("parsed cli!");
    // Before doing anything, regenerate the procgen tests, if needed.
    procgen::procgen_tests(cfg.procgen_tests);
    eprintln!("generated tests!");

    let out_dir = PathBuf::from("target/temp/");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::remove_dir_all(&out_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    // Set up env vars for CC
    env::set_var("OUT_DIR", &out_dir);
    env::set_var("HOST", built_info::HOST);
    env::set_var("TARGET", built_info::TARGET);
    env::set_var("OPT_LEVEL", "0");

    let mut abi_impls: HashMap<&str, Box<dyn AbiImpl + Send + Sync>> = HashMap::new();
    abi_impls.insert(
        ABI_IMPL_RUSTC,
        Box::new(abis::RustcAbiImpl::new(&cfg, None)),
    );
    abi_impls.insert(
        ABI_IMPL_CC,
        Box::new(abis::CcAbiImpl::new(&cfg, ABI_IMPL_CC)),
    );
    abi_impls.insert(
        ABI_IMPL_GCC,
        Box::new(abis::CcAbiImpl::new(&cfg, ABI_IMPL_GCC)),
    );
    abi_impls.insert(
        ABI_IMPL_CLANG,
        Box::new(abis::CcAbiImpl::new(&cfg, ABI_IMPL_CLANG)),
    );
    abi_impls.insert(
        ABI_IMPL_MSVC,
        Box::new(abis::CcAbiImpl::new(&cfg, ABI_IMPL_MSVC)),
    );

    for &(ref name, ref path) in &cfg.rustc_codegen_backends {
        abi_impls.insert(
            name,
            Box::new(abis::RustcAbiImpl::new(&cfg, Some(path.to_owned()))),
        );
    }
    eprintln!("configured ABIs!");

    // Grab all the tests
    let mut tests = vec![];
    let mut dirs = vec![PathBuf::from("tests")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;

            // If it's a dir, add it to the working set
            if entry.file_type()?.is_dir() {
                dirs.push(entry.path());
                continue;
            }

            // Otherwise, assume it's a test and parse it
            let test = match read_test_manifest(&entry.path()) {
                Ok(test) => test,
                Err(e) => {
                    eprintln!("test {:?}'s .ron file couldn't be parsed {}", entry, e);
                    continue;
                }
            };
            tests.push(test);
        }
    }
    tests.sort_by(|t1, t2| t1.name.cmp(&t2.name));
    eprintln!("got tests!");
    // FIXME: assert test names don't collide!

    // Run the tests
    use TestConclusion::*;

    // This is written as nested iterator adaptors so that it can maybe be changed to use
    // rayon's par_iter, but currently the code isn't properly threadsafe due to races on
    // the filesystem when setting up the various output dirs :(
    let reports = tests
        .iter()
        .flat_map(|test| {
            // If the cli has test filters, apply those
            if !cfg.run_tests.is_empty() && !cfg.run_tests.contains(&test.name) {
                return Vec::new();
            }
            cfg.run_conventions
                .iter()
                .flat_map(|convention| {
                    if !test.has_convention(*convention) {
                        // Don't bother with a convention if the test doesn't use it.
                        return Vec::new();
                    }
                    // Create versions of the test for each "X calls Y" pair we care about.
                    cfg.run_pairs
                        .iter()
                        .filter_map(|(caller_id, callee_id)| {
                            if !cfg.run_impls.is_empty()
                                && !cfg.run_impls.iter().any(|x| x == caller_id)
                                && !cfg.run_impls.iter().any(|x| &**x == callee_id)
                            {
                                return None;
                            }
                            let caller =
                                &**abi_impls.get(&**caller_id).expect("invalid id for caller!");
                            let callee =
                                &**abi_impls.get(&**callee_id).expect("invalid id for callee!");

                            let convention_name = convention.name();

                            // Run the test!
                            let test_key = TestKey {
                                test_name: test.name.to_owned(),
                                convention: convention_name.to_owned(),
                                caller_id: caller_id.to_owned(),
                                callee_id: callee_id.to_owned(),
                            };
                            let rules = get_test_rules(&test_key, caller, callee);
                            let results = do_test(
                                &test,
                                &test_key,
                                &rules,
                                *convention,
                                caller,
                                callee,
                                &out_dir,
                            );
                            let report = report_test(test_key, rules, results);
                            Some(report)
                        })
                        .collect()
                })
                .collect()
        })
        .collect::<Vec<_>>();

    // Compute the final report
    let mut num_tests = 0;
    let mut num_passed = 0;
    let mut num_busted = 0;
    let mut num_failed = 0;
    let mut num_skipped = 0;
    for report in &reports {
        num_tests += 1;
        match report.conclusion {
            Busted => num_busted += 1,
            Skipped => num_skipped += 1,
            Passed => num_passed += 1,
            Failed => num_failed += 1,
        }
    }

    let full_report = FullReport {
        summary: TestSummary {
            num_tests,
            num_passed,
            num_busted,
            num_failed,
            num_skipped,
        },
        // TODO: put in a bunch of metadata here?
        config: TestConfig {},
        tests: reports,
    };

    let mut output = std::io::stdout();
    match cfg.output_format {
        OutputFormat::Human => full_report.print_human(&mut output).unwrap(),
        OutputFormat::Json => full_report.print_json(&mut output).unwrap(),
    }

    if full_report.failed() {
        Err(TestsFailed {})?;
    }
    Ok(())
}

/// Generate, Compile, Link, Load, and Run this test.
fn do_test(
    test: &Test,
    test_key: &TestKey,
    test_rules: &TestRules,
    convention: CallingConvention,
    caller: &dyn AbiImpl,
    callee: &dyn AbiImpl,
    _out_dir: &Path,
) -> TestRunResults {
    use TestRunMode::*;

    let mut run_results = TestRunResults::default();
    if test_rules.run <= Skip {
        return run_results;
    }

    run_results.ran_to = Generate;
    run_results.source = Some(generate_test_src(
        test, test_key, convention, caller, callee,
    ));
    let source = match run_results.source.as_ref().unwrap() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to generate source: {}", e);
            return run_results;
        }
    };
    if test_rules.run <= Generate {
        return run_results;
    }

    run_results.ran_to = Build;
    run_results.build = Some(build_test(test, test_key, caller, callee, source));
    let build = match run_results.build.as_ref().unwrap() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to build test: {}", e);
            return run_results;
        }
    };
    if test_rules.run <= Build {
        return run_results;
    }

    run_results.ran_to = Link;
    run_results.link = Some(link_test(test, test_key, build));
    let link = match run_results.link.as_ref().unwrap() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to link test: {}", e);
            return run_results;
        }
    };
    if test_rules.run <= Link {
        return run_results;
    }

    run_results.ran_to = Run;
    run_results.run = Some(run_dynamic_test(test, test_key, link));
    let run = match run_results.run.as_ref().unwrap() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to run test: {}", e);
            return run_results;
        }
    };
    if test_rules.run <= Run {
        return run_results;
    }

    run_results.ran_to = Check;
    run_results.check = Some(check_test(test, test_key, run));

    run_results
}

/// Read a test .ron file
fn read_test_manifest(test_file: &Path) -> Result<Test, GenerateError> {
    let file = File::open(&test_file)?;
    let mut reader = BufReader::new(file);
    let mut input = String::new();
    reader.read_to_string(&mut input)?;
    let test: Test = ron::from_str(&input).map_err(|e| {
        GenerateError::ParseError(test_file.to_string_lossy().into_owned(), input, e)
    })?;
    Ok(test)
}

fn generate_test_src(
    test: &Test,
    test_key: &TestKey,
    convention: CallingConvention,
    caller: &dyn AbiImpl,
    callee: &dyn AbiImpl,
) -> Result<GenerateOutput, GenerateError> {
    let test_name = &test_key.test_name;
    let convention_name = &test_key.convention;
    let caller_src_ext = caller.src_ext();
    let callee_src_ext = callee.src_ext();
    let full_test_name = full_test_name(test_key);
    let caller_id = &test_key.caller_id;
    let callee_id = &test_key.callee_id;

    if !caller.supports_convention(convention) {
        eprintln!(
            "skipping {full_test_name}: {caller_id} doesn't support convention {convention_name}"
        );
        return Err(GenerateError::Skipped);
    }
    if !callee.supports_convention(convention) {
        eprintln!(
            "skipping {full_test_name}: {callee_id} doesn't support convention {convention_name}"
        );
        return Err(GenerateError::Skipped);
    }

    let src_dir = if convention == CallingConvention::Handwritten {
        PathBuf::from("handwritten_impls/")
    } else {
        PathBuf::from("generated_impls/")
    };

    let caller_src = src_dir.join(format!(
        "{caller_id}/{test_name}_{convention_name}_{caller_id}_caller.{caller_src_ext}"
    ));
    let callee_src = src_dir.join(format!(
        "{callee_id}/{test_name}_{convention_name}_{callee_id}_callee.{callee_src_ext}"
    ));

    if convention == CallingConvention::Handwritten {
        if !caller_src.exists() || !callee_src.exists() {
            eprintln!("skipping {full_test_name}: source for callee and caller doesn't exist");
            return Err(GenerateError::Skipped);
        }
    } else {
        eprintln!("generating {full_test_name}");
        // If the impl isn't handwritten, then we need to generate it.
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::remove_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(caller_src.parent().unwrap())?;
        std::fs::create_dir_all(callee_src.parent().unwrap())?;
        let mut caller_output = File::create(&caller_src)?;
        caller.generate_caller(&mut caller_output, &test, convention)?;

        let mut callee_output = File::create(&callee_src)?;
        callee.generate_callee(&mut callee_output, &test, convention)?;
    }

    Ok(GenerateOutput {
        caller_src,
        callee_src,
    })
}

fn build_test(
    _test: &Test,
    test_key: &TestKey,
    caller: &dyn AbiImpl,
    callee: &dyn AbiImpl,
    src: &GenerateOutput,
) -> Result<BuildOutput, BuildError> {
    let test_name = &test_key.test_name;
    let convention_name = &test_key.convention;
    let full_test_name = full_test_name(test_key);
    let caller_id = &test_key.caller_id;
    let callee_id = &test_key.callee_id;
    eprintln!("compiling  {full_test_name}");

    let caller_lib = format!("{test_name}_{convention_name}_{caller_id}_caller");
    let callee_lib = format!("{test_name}_{convention_name}_{callee_id}_callee");

    // Compile the tests (and let them change the lib name).
    let caller_lib = caller.compile_caller(&src.caller_src, &caller_lib)?;
    let callee_lib = callee.compile_callee(&src.callee_src, &callee_lib)?;

    Ok(BuildOutput {
        caller_lib,
        callee_lib,
    })
}

/// Compile and link the test harness with the two sides of the FFI boundary.
fn link_test(
    _test: &Test,
    test_key: &TestKey,
    build: &BuildOutput,
) -> Result<LinkOutput, LinkError> {
    let test_name = &test_key.test_name;
    let caller_id = &test_key.caller_id;
    let callee_id = &test_key.callee_id;
    let full_test_name = full_test_name(test_key);
    let src = PathBuf::from("harness/harness.rs");
    let output = format!("target/temp/{test_name}_{caller_id}_calls_{callee_id}_harness.dll");
    eprintln!("linking  {full_test_name}");

    let mut cmd = Command::new("rustc");
    cmd.arg("-v")
        .arg("-L")
        .arg("target/temp/")
        .arg("-l")
        .arg(&build.caller_lib)
        .arg("-l")
        .arg(&build.callee_lib)
        .arg("--crate-type")
        .arg("cdylib")
        .arg("--target")
        .arg(built_info::TARGET)
        // .arg("-Csave-temps=y")
        // .arg("--out-dir")
        // .arg("target/temp/")
        .arg("-o")
        .arg(&output)
        .arg(&src);

    eprintln!("running: {:?}", cmd);
    let out = cmd.output()?;

    if !out.status.success() {
        Err(LinkError::RustLink(out))
    } else {
        Ok(LinkOutput {
            test_bin: PathBuf::from(output),
        })
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

/// Run the test!
fn run_dynamic_test(
    test: &Test,
    test_key: &TestKey,
    test_dylib: &LinkOutput,
) -> Result<RunOutput, RunError> {
    // See the README for a high-level description of this design.

    ////////////////////////////////////////////////////////////////////
    //////////////////// DEFINING THE TEST HARNESS /////////////////////
    ////////////////////////////////////////////////////////////////////

    // The signatures of the interface from our perspective.
    // From the test's perspective the WriteBuffers are totally opaque.
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

    ////////////////////////////////////////////////////////////////////
    //////////////////// THE ACTUAL TEST EXECUTION /////////////////////
    ////////////////////////////////////////////////////////////////////

    unsafe {
        let full_test_name = full_test_name(test_key);
        // Initialize all the buffers the tests will write to
        let mut caller_inputs = WriteBuffer::new();
        let mut caller_outputs = WriteBuffer::new();
        let mut callee_inputs = WriteBuffer::new();
        let mut callee_outputs = WriteBuffer::new();

        // Load the dylib of the test, and get its test_start symbol
        eprintln!("loading: {}", &test_dylib.test_bin.display());
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

        // As a basic sanity-check, make sure everything agrees on how
        // many tests actually executed. If this fails, then something
        // is very fundamentally broken and needs to be fixed.
        let expected_test_count = test.funcs.len();
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

        fn add_field(
            input: &[Vec<u8>],
            output: &mut LinkedHashMap<String, String>,
            cur_idx: &mut usize,
            cur_path: String,
            val: &Val,
        ) {
            match val {
                Val::Int(_) | Val::Float(_) | Val::Bool(_) | Val::Ptr(_) => {
                    output.insert(cur_path, format_bytes(input, cur_idx));
                }
                Val::Ref(sub_val) => add_field(input, output, cur_idx, cur_path, sub_val),
                Val::Array(arr) => {
                    for (arr_idx, sub_val) in arr.iter().enumerate() {
                        let sub_path = format!("{}[{}]", cur_path, arr_idx);
                        add_field(input, output, cur_idx, sub_path, sub_val);
                    }
                }
                Val::Struct(_struct_name, fields) => {
                    for (field_idx, field) in fields.iter().enumerate() {
                        let sub_path = format!("{}.{}", cur_path, abis::FIELD_NAMES[field_idx]);
                        add_field(input, output, cur_idx, sub_path, field);
                    }
                }
            }
        }

        let mut callee = report::Functions::new();
        let mut caller = report::Functions::new();
        let empty_func = Vec::new();
        let empty_arg = Vec::new();
        for (func_idx, func) in test.funcs.iter().enumerate() {
            let caller_func = caller.entry(func.name.clone()).or_default();
            let callee_func = callee.entry(func.name.clone()).or_default();
            for (arg_idx, arg) in func.inputs.iter().enumerate() {
                let caller_arg = caller_func
                    .entry(ARG_NAMES[arg_idx].to_owned())
                    .or_default();
                let callee_arg = callee_func
                    .entry(ARG_NAMES[arg_idx].to_owned())
                    .or_default();

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

                add_field(caller_arg_bytes, caller_arg, &mut 0, String::new(), arg);
                add_field(callee_arg_bytes, callee_arg, &mut 0, String::new(), arg);
            }

            for (arg_idx, arg) in func.output.iter().enumerate() {
                let caller_arg = caller_func.entry(format!("return{}", arg_idx)).or_default();
                let callee_arg = callee_func.entry(format!("return{}", arg_idx)).or_default();

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

                add_field(caller_output_bytes, caller_arg, &mut 0, String::new(), arg);
                add_field(callee_output_bytes, callee_arg, &mut 0, String::new(), arg);
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
}

fn check_test(
    test: &Test,
    test_key: &TestKey,
    RunOutput {
        caller_inputs,
        caller_outputs,
        callee_inputs,
        callee_outputs,
        ..
    }: &RunOutput,
) -> CheckOutput {
    // Now check the results

    // Start peeling back the layers of the buffers.
    // funcs (subtests) -> vals (args/returns) -> fields -> bytes

    let mut results: Vec<Result<(), CheckFailure>> = Vec::new();

    // Layer 1 is the funcs/subtests. Because we have already checked
    // that they agree on their lengths, we can zip them together
    // to walk through their views of each subtest's execution.
    'funcs: for (func_idx, (((caller_inputs, caller_outputs), callee_inputs), callee_outputs)) in
        caller_inputs
            .funcs
            .iter()
            .zip(&caller_outputs.funcs)
            .zip(&callee_inputs.funcs)
            .zip(&callee_outputs.funcs)
            .enumerate()
    {
        // Now we must enforce that the caller and callee agree on how
        // many inputs and outputs there were. If this fails that's a
        // very fundamental issue, and indicative of a bad test generator.
        if caller_inputs.len() != callee_inputs.len() {
            results.push(Err(CheckFailure::InputCountMismatch(
                func_idx,
                caller_inputs.clone(),
                callee_inputs.clone(),
            )));
            continue 'funcs;
        }
        if caller_outputs.len() != callee_outputs.len() {
            results.push(Err(CheckFailure::OutputCountMismatch(
                func_idx,
                caller_outputs.clone(),
                callee_outputs.clone(),
            )));
            continue 'funcs;
        }

        // Layer 2 is the values (arguments/returns).
        // The inputs and outputs loop do basically the same work,
        // but are separate for the sake of error-reporting quality.

        // Process Inputs
        for (input_idx, (caller_val, callee_val)) in
            caller_inputs.into_iter().zip(callee_inputs).enumerate()
        {
            // Now we must enforce that the caller and callee agree on how
            // many fields each value had.
            if caller_val.len() != callee_val.len() {
                results.push(Err(CheckFailure::InputFieldCountMismatch(
                    func_idx,
                    input_idx,
                    caller_val.clone(),
                    callee_val.clone(),
                )));
                continue 'funcs;
            }

            // Layer 3 is the leaf subfields of the values.
            // At this point we just need to assert that they agree on the bytes.
            for (field_idx, (caller_field, callee_field)) in
                caller_val.into_iter().zip(callee_val).enumerate()
            {
                if caller_field != callee_field {
                    results.push(Err(CheckFailure::InputFieldMismatch(
                        func_idx,
                        input_idx,
                        field_idx,
                        caller_field.clone(),
                        callee_field.clone(),
                    )));
                    continue 'funcs;
                }
            }
        }

        // Process Outputs
        for (output_idx, (caller_val, callee_val)) in
            caller_outputs.into_iter().zip(callee_outputs).enumerate()
        {
            // Now we must enforce that the caller and callee agree on how
            // many fields each value had.
            if caller_val.len() != callee_val.len() {
                results.push(Err(CheckFailure::OutputFieldCountMismatch(
                    func_idx,
                    output_idx,
                    caller_val.clone(),
                    callee_val.clone(),
                )));
                continue 'funcs;
            }

            // Layer 3 is the leaf subfields of the values.
            // At this point we just need to assert that they agree on the bytes.
            for (field_idx, (caller_field, callee_field)) in
                caller_val.into_iter().zip(callee_val).enumerate()
            {
                if caller_field != callee_field {
                    results.push(Err(CheckFailure::OutputFieldMismatch(
                        func_idx,
                        output_idx,
                        field_idx,
                        caller_field.clone(),
                        callee_field.clone(),
                    )));
                    continue 'funcs;
                }
            }
        }

        // If we got this far then the test passes
        results.push(Ok(()));
    }

    // Report the results of each subtest
    //
    // This will be done again after all tests have been run, but it's
    // useful to keep a version of this near the actual compilation/execution
    // in case the compilers spit anything interesting to stdout/stderr.
    let names = test
        .funcs
        .iter()
        .map(|test_func| full_subtest_name(test_key, &test_func.name))
        .collect::<Vec<_>>();
    let max_name_len = names.iter().fold(0, |max, name| max.max(name.len()));
    let num_passed = results.iter().filter(|r| r.is_ok()).count();
    let all_passed = num_passed == results.len();

    for (subtest_name, result) in names.iter().zip(&results) {
        match result {
            Ok(()) => {
                eprintln!("Test {subtest_name:width$} passed", width = max_name_len);
            }
            Err(e) => {
                eprintln!("Test {subtest_name:width$} failed!", width = max_name_len);
                eprintln!("{}", e);
            }
        }
    }

    if all_passed {
        eprintln!("all tests passed");
    } else {
        eprintln!("only {}/{} tests passed!", num_passed, results.len());
    }
    eprintln!();

    CheckOutput {
        all_passed,
        subtest_names: names,
        subtest_checks: results,
    }
}

fn report_test(key: TestKey, rules: TestRules, results: TestRunResults) -> TestReport {
    use TestConclusion::*;
    use TestRunMode::*;
    // Ok now check if it matched our expectation
    let conclusion = if rules.run == Skip {
        // If we were told to skip, we skipped
        Skipped
    } else if let Some(Err(GenerateError::Skipped)) = results.source {
        // The generate step is allowed to unilaterally skip things
        // to avoid different configs having to explicitly disable
        // a million unsupported combinations
        Skipped
    } else {
        let passed = match &rules.check {
            TestCheckMode::Pass(must_pass) => match must_pass {
                Skip => true,
                Generate => results.source.as_ref().map(|r| r.is_ok()).unwrap_or(false),
                Build => results.build.as_ref().map(|r| r.is_ok()).unwrap_or(false),
                Link => results.link.as_ref().map(|r| r.is_ok()).unwrap_or(false),
                Run => results.run.as_ref().map(|r| r.is_ok()).unwrap_or(false),
                Check => results
                    .check
                    .as_ref()
                    .map(|r| r.all_passed)
                    .unwrap_or(false),
            },
            TestCheckMode::Fail(must_fail) | TestCheckMode::Busted(must_fail) => match must_fail {
                Skip => true,
                Generate => results.source.as_ref().map(|r| !r.is_ok()).unwrap_or(false),
                Build => results.build.as_ref().map(|r| !r.is_ok()).unwrap_or(false),
                Link => results.link.as_ref().map(|r| !r.is_ok()).unwrap_or(false),
                Run => results.run.as_ref().map(|r| !r.is_ok()).unwrap_or(false),
                Check => results
                    .check
                    .as_ref()
                    .map(|r| !r.all_passed)
                    .unwrap_or(false),
            },
            TestCheckMode::Random => true,
        };
        if passed {
            if matches!(rules.check, TestCheckMode::Busted(_)) {
                TestConclusion::Busted
            } else {
                TestConclusion::Passed
            }
        } else {
            TestConclusion::Failed
        }
    };
    TestReport {
        key,
        rules,
        conclusion,
        results,
    }
}

/// The name of a test for pretty-printing.
fn full_test_name(
    TestKey {
        test_name,
        convention,
        caller_id,
        callee_id,
    }: &TestKey,
) -> String {
    format!("{test_name}::{convention}::{caller_id}_calls_{callee_id}")
}

/// The name of a subtest for pretty-printing.
fn full_subtest_name(
    TestKey {
        test_name,
        convention,
        caller_id,
        callee_id,
    }: &TestKey,
    func_name: &str,
) -> String {
    format!("{test_name}::{convention}::{caller_id}_calls_{callee_id}::{func_name}")
}
