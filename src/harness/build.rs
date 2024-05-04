use std::path::PathBuf;
use std::process::Command;

use crate::error::*;
use crate::harness::full_test_name;
use crate::report::*;
use crate::{built_info, AbiImpl, Test, TestKey};

pub fn build_test(
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
pub fn link_test(
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
