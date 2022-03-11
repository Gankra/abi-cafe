mod abis;

use abis::*;
use clap::{AppSettings, Arg};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
/*
use log::error;
use simplelog::{
    ColorChoice, ConfigBuilder, Level, LevelFilter, TermLogger, TerminalMode, WriteLogger,
};
 */

/// Slurps up details of how this crate was compiled, which we can use
/// to better compile the actual tests since we're currently compiling them on
/// the same platform with the same toolchains!
pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

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
    #[error("No handwritten source for this pairing (skipping)")]
    NoHandwrittenSource,
}

#[derive(Debug, thiserror::Error)]
pub enum TestFailure {
    #[error("test {0} {} field {2} mismatch \ncaller: {3:02X?} \ncallee: {4:02X?}", ARG_NAMES[*.1])]
    InputFieldMismatch(usize, usize, usize, Vec<u8>, Vec<u8>),
    #[error(
        "test {0} {} field {2} mismatch \ncaller: {3:02X?} \ncallee: {4:02X?}",
        OUTPUT_NAME
    )]
    OutputFieldMismatch(usize, usize, usize, Vec<u8>, Vec<u8>),
    #[error("test {0} {} field count mismatch \ncaller: {2:#02X?} \ncallee: {3:#02X?}", ARG_NAMES[*.1])]
    InputFieldCountMismatch(usize, usize, Vec<Vec<u8>>, Vec<Vec<u8>>),
    #[error(
        "test {0} {} field count mismatch \ncaller: {2:#02X?} \ncallee: {3:#02X?}",
        OUTPUT_NAME
    )]
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

#[derive(Debug, Clone)]
pub struct Config {
    procgen_tests: bool,
    run_conventions: Vec<CallingConvention>,
    run_impls: Vec<String>,
    run_pairs: Vec<(String, String)>,
    run_tests: Vec<String>,
}

fn make_app() -> Config {
    static ABI_IMPLS: &[&str] = &[
        ABI_IMPL_RUSTC,
        ABI_IMPL_CC,
        ABI_IMPL_GCC,
        ABI_IMPL_CLANG,
        ABI_IMPL_MSVC,
    ];
    /// The pairings of impls to run. LHS calls RHS.
    static DEFAULT_TEST_PAIRS: &[(&str, &str)] = &[
        (ABI_IMPL_RUSTC, ABI_IMPL_CC), // Rust calls C
        (ABI_IMPL_CC, ABI_IMPL_RUSTC), // C calls Rust
        (ABI_IMPL_CC, ABI_IMPL_CC),    // C calls C
    ];

    let app = clap::Command::new("abi-checker")
        .version(clap::crate_version!())
        .about("Compares the FFI ABIs of different langs/compilers by generating and running them.")
        .next_line_help(true)
        .setting(AppSettings::DeriveDisplayOrder)
        .arg(
            Arg::new("procgen-tests")
                .long("procgen-tests")
                .long_help("Regenerate the procgen test manifests"),
        )
        .arg(
            Arg::new("conventions")
                .long("conventions")
                .long_help("Only run the given calling conventions")
                .possible_values(&[
                    "c",
                    "cdecl",
                    "fastcall",
                    "stdcall",
                    "vectorcall",
                    "handwritten",
                ])
                .multiple_values(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("impls")
                .long("impls")
                .long_help("Only run the given impls (compilers/languages)")
                .possible_values(ABI_IMPLS)
                .multiple_values(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("tests")
                .long("tests")
                .long_help("Only run the given tests")
                .multiple_values(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("pairs")
                .long("pairs")
                .long_help("Only run the given impl pairs, in the form of impl_calls_impl")
                .multiple_values(true)
                .takes_value(true),
        )
        .after_help("");

    let matches = app.get_matches();
    let procgen_tests = matches.is_present("procgen-tests");

    let mut run_conventions: Vec<_> = matches
        .values_of("conventions")
        .into_iter()
        .flatten()
        .map(|conv| CallingConvention::from_str(conv).unwrap())
        .collect();

    if run_conventions.is_empty() {
        run_conventions = ALL_CONVENTIONS.into_iter().copied().collect();
    }

    let run_impls = matches
        .values_of("impls")
        .into_iter()
        .flatten()
        .map(String::from)
        .collect();

    let mut run_pairs: Vec<_> = matches
        .values_of("pairs")
        .into_iter()
        .flatten()
        .map(|pair| {
            pair.split_once("_calls_")
                .expect("invalid 'pair' syntax, must be 'impl_calls_impl'")
        })
        .map(|(a, b)| (String::from(a), String::from(b)))
        .collect();

    if run_pairs.is_empty() {
        run_pairs = DEFAULT_TEST_PAIRS
            .into_iter()
            .map(|&(a, b)| (String::from(a), String::from(b)))
            .collect()
    }

    let run_tests = matches
        .values_of("tests")
        .into_iter()
        .flatten()
        .map(String::from)
        .collect();

    Config {
        procgen_tests,
        run_conventions,
        run_impls,
        run_tests,
        run_pairs,
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cfg = make_app();
    // Before doing anything, regenerate the procgen tests, if needed.
    procgen_tests(cfg.procgen_tests);

    let out_dir = PathBuf::from("target/temp/");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::remove_dir_all(&out_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    // Set up env vars for CC
    env::set_var("OUT_DIR", &out_dir);
    env::set_var("HOST", built_info::HOST);
    env::set_var("TARGET", built_info::TARGET);
    env::set_var("OPT_LEVEL", "0");

    let mut abi_impls: HashMap<&str, Box<dyn AbiImpl>> = HashMap::new();
    abi_impls.insert(ABI_IMPL_RUSTC, Box::new(abis::RustcAbiImpl::new(&cfg)));
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

    let mut reports = Vec::new();
    let mut skips = 0;

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
    // FIXME: assert test names don't collide!

    // Run the tests
    for test in tests {
        if !cfg.run_tests.is_empty() && !cfg.run_tests.contains(&test.name) {
            continue;
        }
        for &convention in &cfg.run_conventions {
            if !test.has_convention(convention) {
                // Don't bother with a convention if the test doesn't use it.
                continue;
            }
            // Create versions of the test for each "X calls Y" pair we care about.
            for (caller_id, callee_id) in &cfg.run_pairs {
                if !cfg.run_impls.is_empty()
                    && !cfg.run_impls.iter().any(|x| x == caller_id)
                    && !cfg.run_impls.iter().any(|x| &**x == callee_id)
                {
                    continue;
                }
                let caller = &**abi_impls.get(&**caller_id).expect("invalid id for caller!");
                let callee = &**abi_impls.get(&**callee_id).expect("invalid id for callee!");

                let caller_name = caller.name();
                let callee_name = callee.name();
                let convention_name = convention.name();
                let full_test_name =
                    full_test_name(&test.name, convention_name, caller_name, callee_name);
                if !caller.supports_convention(convention) {
                    eprintln!("skipping {full_test_name}: {caller_name} doesn't support convention {convention_name}");
                    skips += 1;
                    continue;
                }
                if !callee.supports_convention(convention) {
                    eprintln!("skipping {full_test_name}: {callee_name} doesn't support convention {convention_name}");
                    skips += 1;
                    continue;
                }

                let result = do_test(&test, convention, caller, callee, &out_dir);

                if let Err(BuildError::NoHandwrittenSource) = &result {
                    eprintln!(
                        "skipping {full_test_name}: source for callee and caller doesn't exist"
                    );
                    skips += 1;
                    continue;
                } else if let Err(e) = &result {
                    eprintln!("test failed: {}", e);
                }
                reports.push((
                    test.name.clone(),
                    convention,
                    caller.name(),
                    callee.name(),
                    result,
                ));
            }
        }
    }

    println!();
    println!("Final Results:");
    // Do a cleaned up printout now
    let mut passes = 0;
    let mut fails = 0;
    let mut total_fails = 0;
    for (test_name, convention, caller_name, callee_name, report) in reports {
        let convention_name = convention.name();
        let pretty_test_name =
            full_test_name(&test_name, convention_name, caller_name, callee_name);
        print!("{pretty_test_name:<32} ");
        match report {
            Err(_) => {
                println!("failed completely (bad input?)");
                total_fails += 1;
            }
            Ok(report) => {
                let num_passed = report.results.iter().filter(|r| r.is_ok()).count();
                let all_passed = num_passed == report.results.len();

                if all_passed {
                    print!("all ");
                } else {
                    print!("    ");
                }
                print!("{num_passed:>3}/{:<3} ", report.results.len());
                println!("passed");
                // If all the subtests pass, don't bother with a breakdown.
                if all_passed {
                    passes += num_passed;
                    continue;
                }

                let names = report
                    .test
                    .funcs
                    .iter()
                    .map(|test_func| {
                        full_subtest_name(
                            &test_name,
                            convention_name,
                            caller_name,
                            callee_name,
                            &test_func.name,
                        )
                    })
                    .collect::<Vec<_>>();
                let max_name_len = names.iter().fold(0, |max, name| max.max(name.len()));
                for (subtest_name, result) in names.iter().zip(report.results.iter()) {
                    print!("  {:width$} ", subtest_name, width = max_name_len);
                    if let Err(_e) = result {
                        println!("failed!");
                        // A bit too noisy?
                        // println!("{}", e);
                        fails += 1;
                    } else {
                        println!("passed");
                        passes += 1;
                    }
                }
                println!();
            }
        }
    }
    println!();
    println!("{passes} passed, {fails} failed, {total_fails} completely failed, {skips} skipped");

    Ok(())
}

/// Generate, Compile, Link, Load, and Run this test.
fn do_test(
    test: &Test,
    convention: CallingConvention,
    caller: &dyn AbiImpl,
    callee: &dyn AbiImpl,
    _out_dir: &Path,
) -> Result<TestReport, BuildError> {
    let test_name = &test.name;
    let convention_name = convention.name();
    let caller_name = caller.name();
    let caller_src_ext = caller.src_ext();
    let callee_name = callee.name();
    let callee_src_ext = callee.src_ext();
    let full_test_name = full_test_name(test_name, convention_name, caller_name, callee_name);

    let src_dir = if convention == CallingConvention::Handwritten {
        PathBuf::from("handwritten_impls/")
    } else {
        PathBuf::from("generated_impls/")
    };

    let caller_src = src_dir.join(format!(
        "{caller_name}/{test_name}_{convention_name}_{caller_name}_caller.{caller_src_ext}"
    ));
    let callee_src = src_dir.join(format!(
        "{callee_name}/{test_name}_{convention_name}_{callee_name}_callee.{callee_src_ext}"
    ));
    let caller_lib = format!("{test_name}_{convention_name}_{caller_name}_caller");
    let callee_lib = format!("{test_name}_{convention_name}_{callee_name}_callee");

    if convention == CallingConvention::Handwritten {
        if !caller_src.exists() || !callee_src.exists() {
            return Err(BuildError::NoHandwrittenSource);
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
    eprintln!("compiling  {full_test_name}");
    // Compile the tests (and let them change the lib name).
    let caller_lib = caller.compile_caller(&caller_src, &caller_lib)?;
    let callee_lib = callee.compile_callee(&callee_src, &callee_lib)?;

    // Compile the harness dylib and link in the tests.
    let dylib = build_harness(test, caller_name, &caller_lib, callee_name, &callee_lib)?;

    // Load and run the test
    run_dynamic_test(test, convention_name, caller_name, callee_name, &dylib)
}

/// Read a test .ron file
fn read_test_manifest(test_file: &Path) -> Result<Test, BuildError> {
    let file = File::open(&test_file)?;
    let mut reader = BufReader::new(file);
    let mut input = String::new();
    reader.read_to_string(&mut input)?;
    let test: Test = ron::from_str(&input)
        .map_err(|e| BuildError::ParseError(test_file.to_string_lossy().into_owned(), input, e))?;
    Ok(test)
}

/// Compile and link the test harness with the two sides of the FFI boundary.
fn build_harness(
    test: &Test,
    caller_name: &str,
    caller_lib: &str,
    callee_name: &str,
    callee_lib: &str,
) -> Result<String, BuildError> {
    let test_name = &test.name;
    let src = PathBuf::from("harness/harness.rs");
    let output = format!("target/temp/{test_name}_{caller_name}_calls_{callee_name}_harness.dll");

    let out = Command::new("rustc")
        .arg("-v")
        .arg("-L")
        .arg("target/temp/")
        .arg("-l")
        .arg(&caller_lib)
        .arg("-l")
        .arg(&callee_lib)
        .arg("--crate-type")
        .arg("cdylib")
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

/// Run the test!
fn run_dynamic_test(
    test: &Test,
    convention_name: &str,
    caller_name: &str,
    callee_name: &str,
    dylib: &str,
) -> Result<TestReport, BuildError> {
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

    ////////////////////////////////////////////////////////////////////
    //////////////////// THE ACTUAL TEST EXECUTION /////////////////////
    ////////////////////////////////////////////////////////////////////

    unsafe {
        let full_test_name = full_test_name(&test.name, convention_name, caller_name, callee_name);
        // Initialize all the buffers the tests will write to
        let mut caller_inputs = WriteBuffer::new();
        let mut caller_outputs = WriteBuffer::new();
        let mut callee_inputs = WriteBuffer::new();
        let mut callee_outputs = WriteBuffer::new();

        // Load the dylib of the test, and get its test_start symbol
        let lib = libloading::Library::new(dylib)?;
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

        // Now check the results

        // As a basic sanity-check, make sure everything agrees on how
        // many tests actually executed. If this fails, then something
        // is very fundamentally broken and needs to be fixed.
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

        // Start peeling back the layers of the buffers.
        // funcs (subtests) -> vals (args/returns) -> fields -> bytes

        let mut results: Vec<Result<(), TestFailure>> = Vec::new();

        // Layer 1 is the funcs/subtests. Because we have already checked
        // that they agree on their lengths, we can zip them together
        // to walk through their views of each subtest's execution.
        'funcs: for (
            func_idx,
            (((caller_inputs, caller_outputs), callee_inputs), callee_outputs),
        ) in caller_inputs
            .funcs
            .into_iter()
            .zip(caller_outputs.funcs)
            .zip(callee_inputs.funcs)
            .zip(callee_outputs.funcs)
            .enumerate()
        {
            // Now we must enforce that the caller and callee agree on how
            // many inputs and outputs there were. If this fails that's a
            // very fundamental issue, and indicative of a bad test generator.
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
                    results.push(Err(TestFailure::InputFieldCountMismatch(
                        func_idx, input_idx, caller_val, callee_val,
                    )));
                    continue 'funcs;
                }

                // Layer 3 is the leaf subfields of the values.
                // At this point we just need to assert that they agree on the bytes.
                for (field_idx, (caller_field, callee_field)) in
                    caller_val.into_iter().zip(callee_val).enumerate()
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
            for (output_idx, (caller_val, callee_val)) in
                caller_outputs.into_iter().zip(callee_outputs).enumerate()
            {
                // Now we must enforce that the caller and callee agree on how
                // many fields each value had.
                if caller_val.len() != callee_val.len() {
                    results.push(Err(TestFailure::OutputFieldCountMismatch(
                        func_idx, output_idx, caller_val, callee_val,
                    )));
                    continue 'funcs;
                }

                // Layer 3 is the leaf subfields of the values.
                // At this point we just need to assert that they agree on the bytes.
                for (field_idx, (caller_field, callee_field)) in
                    caller_val.into_iter().zip(callee_val).enumerate()
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

        // Report the results of each subtest
        //
        // This will be done again after all tests have been run, but it's
        // useful to keep a version of this near the actual compilation/execution
        // in case the compilers spit anything interesting to stdout/stderr.
        let test_name = &test.name;
        let names = test
            .funcs
            .iter()
            .map(|test_func| {
                full_subtest_name(
                    test_name,
                    convention_name,
                    caller_name,
                    callee_name,
                    &test_func.name,
                )
            })
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

        Ok(TestReport {
            test: test.clone(),
            results,
        })
    }
}

/// The name of a test for pretty-printing.
fn full_test_name(
    test_name: &str,
    convention_name: &str,
    caller_name: &str,
    callee_name: &str,
) -> String {
    format!("{test_name}::{convention_name}::{caller_name}_calls_{callee_name}")
}

/// The name of a subtest for pretty-printing.
fn full_subtest_name(
    test_name: &str,
    convention_name: &str,
    caller_name: &str,
    callee_name: &str,
    func_name: &str,
) -> String {
    format!("{test_name}::{convention_name}::{caller_name}_calls_{callee_name}::{func_name}")
}

/// For tests that are too tedious to even hand-write the .ron file,
/// this code generates it programmatically.
///
/// **NOTE: this is disabled by default, the results are checked in.
/// If you want to regenerate these tests, just remove the early return.**
fn procgen_tests(regenerate: bool) {
    // Regeneration disabled by default.
    if !regenerate {
        return;
    }

    let proc_gen_root = PathBuf::from("tests/procgen");

    // Make sure the path exists, then delete its contents, then recreate the empty dir.
    std::fs::create_dir_all(&proc_gen_root).unwrap();
    std::fs::remove_dir_all(&proc_gen_root).unwrap();
    std::fs::create_dir_all(&proc_gen_root).unwrap();

    let tests: &[(&str, &[Val])] = &[
        // Just run basic primitives that everyone should support through their paces.
        // This is chunked out a bit to avoid stressing the compilers/linkers too much,
        // in case some work scales non-linearly. It also keeps the test suite
        // a bit more "responsive" instead of just stalling one enormous supertest.
        ("i64", &[Val::Int(IntVal::c_int64_t(0x1a2b3c4d_23eaf142))]),
        ("i32", &[Val::Int(IntVal::c_int32_t(0x1a2b3c4d))]),
        ("i16", &[Val::Int(IntVal::c_int16_t(0x1a2b))]),
        ("i8", &[Val::Int(IntVal::c_int8_t(0x1a))]),
        ("u64", &[Val::Int(IntVal::c_uint64_t(0x1a2b3c4d_23eaf142))]),
        ("u32", &[Val::Int(IntVal::c_uint32_t(0x1a2b3c4d))]),
        ("u16", &[Val::Int(IntVal::c_uint16_t(0x1a2b))]),
        ("u8", &[Val::Int(IntVal::c_uint8_t(0x1a))]),
        ("ptr", &[Val::Ptr(0x1a2b3c4d_23eaf142)]),
        ("bool", &[Val::Bool(true)]),
        ("f64", &[Val::Float(FloatVal::c_double(809239021.392))]),
        ("f32", &[Val::Float(FloatVal::c_float(-4921.3527))]),
        // These are split out because they are the buggy mess that inspired this whole enterprise!
        // These types are a GCC exenstion. Windows is a huge dumpster fire where no one agrees on
        // it (MSVC doesn't even define __(u)int128_t afaict, but has some equivalent extension).
        //
        // On linux-based platforms where this is a more established thing, current versions of
        // rustc underalign the value (as if it's emulated, like u64 on x86). This isn't a problem
        // in-and-of-itself because rustc accurately says "this isn't usable for FFI".
        // Unfortunately platforms like aarch64 (arm64) use this type in their definitions for
        // saving/restoring float registers, so it's very much so part of the platform ABI,
        // and Rust should just *fix this*.
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

        let mut perturb_float = 0.0f32;
        let mut perturb_byte = 0u8;

        for val in vals.iter() {
            let new_val = |i| -> Val {
                // TODO: actually perturb the values?
                let mut new_val = val.clone();
                let mut cur_val = Some(&mut new_val);
                while let Some(temp) = cur_val.take() {
                    match temp {
                        Val::Ref(pointee) => {
                            cur_val = Some(&mut **pointee);
                            continue;
                        }
                        Val::Struct(_, _) => unimplemented!(),
                        Val::Array(_) => unimplemented!(),
                        Val::Ptr(out) => graffiti_primitive(out, i),
                        Val::Int(int_val) => match int_val {
                            IntVal::c__int128(out) => graffiti_primitive(out, i),
                            IntVal::c_int64_t(out) => graffiti_primitive(out, i),
                            IntVal::c_int32_t(out) => graffiti_primitive(out, i),
                            IntVal::c_int16_t(out) => graffiti_primitive(out, i),
                            IntVal::c_int8_t(out) => graffiti_primitive(out, i),
                            IntVal::c__uint128(out) => graffiti_primitive(out, i),
                            IntVal::c_uint64_t(out) => graffiti_primitive(out, i),
                            IntVal::c_uint32_t(out) => graffiti_primitive(out, i),
                            IntVal::c_uint16_t(out) => graffiti_primitive(out, i),
                            IntVal::c_uint8_t(out) => graffiti_primitive(out, i),
                        },
                        Val::Float(float_val) => match float_val {
                            FloatVal::c_double(out) => graffiti_primitive(out, i),
                            FloatVal::c_float(out) => graffiti_primitive(out, i),
                        },
                        Val::Bool(out) => *out = true,
                    }
                }

                new_val
            };

            let val_name = arg_ty(val);

            // Start gentle with basic one value in/out tests
            test.funcs.push(Func {
                name: format!("{val_name}_val_in"),
                conventions: vec![CallingConvention::All],
                inputs: vec![new_val(0)],
                output: None,
            });

            test.funcs.push(Func {
                name: format!("{val_name}_val_out"),
                conventions: vec![CallingConvention::All],
                inputs: vec![],
                output: Some(new_val(0)),
            });

            test.funcs.push(Func {
                name: format!("{val_name}_val_in_out"),
                conventions: vec![CallingConvention::All],
                inputs: vec![new_val(0)],
                output: Some(new_val(1)),
            });

            // Start gentle with basic one value in/out tests
            test.funcs.push(Func {
                name: format!("{val_name}_ref_in"),
                conventions: vec![CallingConvention::All],
                inputs: vec![Val::Ref(Box::new(new_val(0)))],
                output: None,
            });

            test.funcs.push(Func {
                name: format!("{val_name}_ref_out"),
                conventions: vec![CallingConvention::All],
                inputs: vec![],
                output: Some(Val::Ref(Box::new(new_val(0)))),
            });

            test.funcs.push(Func {
                name: format!("{val_name}_ref_in_out"),
                conventions: vec![CallingConvention::All],
                inputs: vec![Val::Ref(Box::new(new_val(0)))],
                output: Some(Val::Ref(Box::new(new_val(1)))),
            });

            // Stress out the calling convention and try lots of different
            // input counts. For many types this will result in register
            // exhaustion and get some things passed on the stack.
            for len in 2..=16 {
                test.funcs.push(Func {
                    name: format!("{val_name}_val_in_{len}"),
                    conventions: vec![CallingConvention::All],
                    inputs: (0..len).map(|i| new_val(i)).collect(),
                    output: None,
                });
            }

            // Stress out the calling convention with a struct full of values.
            // Some conventions will just shove this in a pointer/stack,
            // others will try to scalarize this into registers anyway.
            for len in 1..=16 {
                test.funcs.push(Func {
                    name: format!("{val_name}_struct_in_{len}"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Struct(
                        format!("{val_name}_{len}"),
                        (0..len).map(|i| new_val(i)).collect(),
                    )],
                    output: None,
                });
            }
            // Check that by-ref works, for good measure
            for len in 1..=16 {
                test.funcs.push(Func {
                    name: format!("{val_name}_ref_struct_in_{len}"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Ref(Box::new(Val::Struct(
                        format!("{val_name}_{len}"),
                        (0..len).map(|i| new_val(i)).collect(),
                    )))],
                    output: None,
                });
            }

            // Now perturb the arguments by including a byte and a float in
            // the argument list. This will mess with alignment and also mix
            // up the "type classes" (float vs int) and trigger more corner
            // cases in the ABIs as things get distributed to different classes
            // of register.

            // We do small and big versions to check the cases where everything
            // should fit in registers vs not.
            let small_count = 4;
            let big_count = 16;

            for idx in 0..small_count {
                let mut inputs = (0..small_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = small_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_val_in_{idx}_perturbed_small"),
                    conventions: vec![CallingConvention::All],
                    inputs: inputs,
                    output: None,
                });
            }
            for idx in 0..big_count {
                let mut inputs = (0..big_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = big_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_val_in_{idx}_perturbed_big"),
                    conventions: vec![CallingConvention::All],
                    inputs: inputs,
                    output: None,
                });
            }

            for idx in 0..small_count {
                let mut inputs = (0..small_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = small_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_struct_in_{idx}_perturbed_small"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Struct(
                        format!("{val_name}_{idx}_perturbed_small"),
                        inputs,
                    )],
                    output: None,
                });
            }
            for idx in 0..big_count {
                let mut inputs = (0..big_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = big_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_struct_in_{idx}_perturbed_big"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Struct(
                        format!("{val_name}_{idx}_perturbed_big"),
                        inputs,
                    )],
                    output: None,
                });
            }

            // Should be an exact copy-paste of the above but with Ref's added
            for idx in 0..small_count {
                let mut inputs = (0..small_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = small_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_ref_struct_in_{idx}_perturbed_small"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Ref(Box::new(Val::Struct(
                        format!("{val_name}_{idx}_perturbed_small"),
                        inputs,
                    )))],
                    output: None,
                });
            }
            for idx in 0..big_count {
                let mut inputs = (0..big_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = big_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_ref_struct_in_{idx}_perturbed_big"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Ref(Box::new(Val::Struct(
                        format!("{val_name}_{idx}_perturbed_big"),
                        inputs,
                    )))],
                    output: None,
                });
            }
        }
        let mut file =
            std::fs::File::create(proc_gen_root.join(format!("{test_name}.ron"))).unwrap();
        let output = ron::to_string(&test).unwrap();
        file.write_all(output.as_bytes()).unwrap();
    }
}

/// The type name to use for this value when it is stored in args/vars.
pub fn arg_ty(val: &Val) -> String {
    use IntVal::*;
    use Val::*;
    match val {
        Ref(x) => format!("ref_{}", arg_ty(x)),
        Ptr(_) => format!("ptr"),
        Bool(_) => format!("bool"),
        Array(vals) => format!(
            "arr_{}_{}",
            vals.len(),
            arg_ty(vals.get(0).expect("arrays must have length > 0")),
        ),
        Struct(name, _) => format!("struct_{name}"),
        Float(FloatVal::c_double(_)) => format!("f64"),
        Float(FloatVal::c_float(_)) => format!("f32"),
        Int(int_val) => match int_val {
            c__int128(_) => format!("i128"),
            c_int64_t(_) => format!("i64"),
            c_int32_t(_) => format!("i32"),
            c_int16_t(_) => format!("i16"),
            c_int8_t(_) => format!("i8"),
            c__uint128(_) => format!("u128"),
            c_uint64_t(_) => format!("u64"),
            c_uint32_t(_) => format!("u32"),
            c_uint16_t(_) => format!("u16"),
            c_uint8_t(_) => format!("u8"),
        },
    }
}

fn graffiti_primitive<T>(output: &mut T, idx: usize) {
    let mut input = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        0x0F,
    ];
    for byte in &mut input {
        *byte |= 0x10 * idx as u8;
    }
    unsafe {
        let out_size = std::mem::size_of::<T>();
        assert!(out_size <= input.len());
        let raw_out = output as *mut T as *mut u8;
        raw_out.copy_from(input.as_ptr(), out_size)
    }
}
