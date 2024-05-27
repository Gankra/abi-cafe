use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::*;
use crate::report::*;
use crate::*;

const OUT_DIR: &str = "target/temp";

impl TestRunner {
    pub async fn build_test(
        &self,
        key: &TestKey,
        src: &GenerateOutput,
        out_dir: &Utf8Path,
    ) -> Result<BuildOutput, BuildError> {
        let full_test_name = full_test_name(key);
        eprintln!("compiling  {full_test_name}");

        // FIXME: these two could be done concurrently
        let caller_lib = self
            .build_lib(
                key.test.clone(),
                key.caller.clone(),
                CallSide::Caller,
                key.options.clone(),
                &src.caller_src,
                out_dir,
            )
            .await?;
        let callee_lib = self
            .build_lib(
                key.test.clone(),
                key.callee.clone(),
                CallSide::Callee,
                key.options.clone(),
                &src.callee_src,
                out_dir,
            )
            .await?;
        Ok(BuildOutput {
            caller_lib,
            callee_lib,
        })
    }

    async fn build_lib(
        &self,
        test_id: TestId,
        abi_id: AbiImplId,
        call_side: CallSide,
        options: TestOptions,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
    ) -> Result<String, BuildError> {
        let abi_impl = self.abi_impls[&abi_id].clone();
        let lib_name = lib_name(&test_id, &abi_id, call_side, &options);
        // Briefly lock this map to insert/acquire a OnceCell and then release the lock
        let once = self
            .static_libs
            .lock()
            .unwrap()
            .entry(lib_name.clone())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        // Either acquire the cached result, or make it
        let real_lib_name = once
            .get_or_try_init(|| compile_lib(&src_path, abi_impl, call_side, out_dir, &lib_name))
            .await?
            .clone();
        Ok(real_lib_name)
    }

    pub async fn link_test(
        &self,
        key: &TestKey,
        build: &BuildOutput,
        out_dir: &Utf8Path,
    ) -> Result<LinkOutput, LinkError> {
        link_test(key, build, out_dir)
    }
}

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

fn lib_name(
    test_id: &TestId,
    abi_impl: &AbiImplId,
    call_side: CallSide,
    options: &TestOptions,
) -> String {
    let TestOptions { convention } = options;
    format!("{test_id}_{convention}_{abi_impl}_{call_side}")
}

pub async fn compile_lib(
    src_path: &Utf8Path,
    abi: Arc<dyn AbiImpl + Send + Sync>,
    call_side: CallSide,
    out_dir: &Utf8Path,
    lib_name: &str,
) -> Result<String, BuildError> {
    let lib_name = match call_side {
        CallSide::Callee => abi.compile_callee(src_path, out_dir, lib_name)?,
        CallSide::Caller => abi.compile_caller(src_path, out_dir, lib_name)?,
    };

    Ok(lib_name)
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
