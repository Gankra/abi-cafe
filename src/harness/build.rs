use std::env;
use std::path::PathBuf;
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};

use crate::harness::full_test_name;
use crate::report::*;
use crate::{built_info, AbiImpl, TestKey};
use crate::{error::*, TestOptions};

const OUT_DIR: &str = "target/temp";

/// Delete and recreate the build dir
pub fn init_build_dir() -> Result<Utf8PathBuf, BuildError> {
    let out_dir = Utf8PathBuf::from(OUT_DIR);
    std::fs::create_dir_all(&out_dir)?;
    std::fs::remove_dir_all(&out_dir)?;
    std::fs::create_dir_all(&out_dir)?;

    // Set up env vars for CC
    env::set_var("OUT_DIR", &out_dir);
    env::set_var("HOST", built_info::HOST);
    env::set_var("TARGET", built_info::TARGET);
    env::set_var("OPT_LEVEL", "0");

    Ok(out_dir)
}

pub fn build_test(
    test_key @ TestKey {
        test: test_id,
        caller: caller_id,
        callee: callee_id,
        options: TestOptions { convention },
    }: &TestKey,
    caller: &dyn AbiImpl,
    callee: &dyn AbiImpl,
    src: &GenerateOutput,
    out_dir: &Utf8Path,
) -> Result<BuildOutput, BuildError> {
    let full_test_name = full_test_name(test_key);
    eprintln!("compiling  {full_test_name}");

    let caller_lib = format!("{test_id}_{convention}_{caller_id}_caller");
    let callee_lib = format!("{test_id}_{convention}_{callee_id}_callee");

    // Compile the tests (and let them change the lib name).
    let caller_lib = caller.compile_caller(&src.caller_src, out_dir, &caller_lib)?;
    let callee_lib = callee.compile_callee(&src.callee_src, out_dir, &callee_lib)?;

    Ok(BuildOutput {
        caller_lib,
        callee_lib,
    })
}

/// Compile and link the test harness with the two sides of the FFI boundary.
pub fn link_test(
    test_key @ TestKey {
        test: test_id,
        caller: caller_id,
        callee: callee_id,
        options: TestOptions { convention },
    }: &TestKey,
    build: &BuildOutput,
    out_dir: &Utf8Path,
) -> Result<LinkOutput, LinkError> {
    let full_test_name = full_test_name(test_key);
    let src = PathBuf::from("harness/harness.rs");
    let output = out_dir.join(format!(
        "{test_id}_{convention}_{caller_id}_calls_{callee_id}_harness.dll"
    ));
    eprintln!("linking  {full_test_name}");

    let mut cmd = Command::new("rustc");
    cmd.arg("-v")
        .arg("-L")
        .arg(out_dir)
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
            test_bin: Utf8PathBuf::from(output),
        })
    }
}
