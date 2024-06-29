use std::process::Command;
use std::sync::Arc;

use camino::Utf8Path;
use tracing::info;

use crate::error::*;
use crate::report::*;
use crate::*;

impl TestHarness {
    pub async fn build_test(
        &self,
        key: &TestKey,
        src: &GenerateOutput,
        out_dir: &Utf8Path,
    ) -> Result<BuildOutput, BuildError> {
        // FIXME: these two could be done concurrently
        let caller_lib = self
            .build_static_lib(key, CallSide::Caller, &src.caller_src, out_dir)
            .await?;
        let callee_lib = self
            .build_static_lib(key, CallSide::Callee, &src.callee_src, out_dir)
            .await?;
        Ok(BuildOutput {
            caller_lib,
            callee_lib,
        })
    }

    async fn build_static_lib(
        &self,
        key: &TestKey,
        call_side: CallSide,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
    ) -> Result<String, BuildError> {
        let abi_impl = self.abi_by_test_key(key, call_side);
        let lib_name = self.static_lib_name(key, call_side);
        // Briefly lock this map to insert/acquire a OnceCell and then release the lock
        let once = self
            .built_static_libs
            .lock()
            .unwrap()
            .entry(lib_name.clone())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        // Either acquire the cached result, or make it
        let real_lib_name = once
            .get_or_try_init(|| {
                info!("compiling   {lib_name}");
                build_static_lib(src_path, abi_impl, call_side, out_dir, &lib_name)
            })
            .await?
            .clone();
        Ok(real_lib_name)
    }

    pub async fn link_dynamic_lib(
        &self,
        key: &TestKey,
        build: &BuildOutput,
        out_dir: &Utf8Path,
    ) -> Result<LinkOutput, LinkError> {
        let dynamic_lib_name = self.dynamic_lib_name(key);
        info!("linking     {dynamic_lib_name}");
        link_dynamic_lib(build, out_dir, &dynamic_lib_name)
    }

    fn static_lib_name(&self, key: &TestKey, call_side: CallSide) -> String {
        self.base_id(key, Some(call_side), "_")
    }

    fn dynamic_lib_name(&self, key: &TestKey) -> String {
        let mut output = self.base_id(key, None, "_");
        output.push_str(".dll");
        output
    }
}

async fn build_static_lib(
    src_path: &Utf8Path,
    abi: Arc<dyn AbiImpl + Send + Sync>,
    call_side: CallSide,
    out_dir: &Utf8Path,
    static_lib_name: &str,
) -> Result<String, BuildError> {
    let lib_name = match call_side {
        CallSide::Callee => abi.compile_callee(src_path, out_dir, static_lib_name)?,
        CallSide::Caller => abi.compile_caller(src_path, out_dir, static_lib_name)?,
    };

    Ok(lib_name)
}

/// Compile and link the test harness with the two sides of the FFI boundary.
fn link_dynamic_lib(
    build: &BuildOutput,
    out_dir: &Utf8Path,
    dynamic_lib_name: &str,
) -> Result<LinkOutput, LinkError> {
    let src = out_dir.join("harness.rs");
    let output = out_dir.join(dynamic_lib_name);
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

    debug!("running: {:?}", cmd);
    let out = cmd.output()?;

    if !out.status.success() {
        Err(LinkError::RustLink(out))
    } else {
        Ok(LinkOutput { test_bin: output })
    }
}
