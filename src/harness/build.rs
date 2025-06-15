use std::process::Command;
use std::sync::Arc;

use camino::Utf8Path;
use tracing::info;

use crate::error::*;
use crate::harness::report::*;
use crate::harness::test::*;
use crate::*;

impl TestHarness {
    pub async fn build_test(
        &self,
        key: &TestKey,
        src: &GenerateOutput,
    ) -> Result<BuildOutput, BuildError> {
        // FIXME: these two could be done concurrently
        let caller_lib = self
            .build_static_lib(key, CallSide::Caller, &src.caller_src)
            .await?;
        let callee_lib = self
            .build_static_lib(key, CallSide::Callee, &src.callee_src)
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
    ) -> Result<String, BuildError> {
        let toolchain = self.toolchain_by_test_key(key, call_side);
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
            .get_or_try_init(|| async {
                let _token = self
                    .concurrency_limiter
                    .acquire()
                    .await
                    .expect("failed to acquire concurrency limit semaphore");
                info!("compiling   {lib_name}");
                build_static_lib(&self.paths, src_path, toolchain, call_side, &lib_name).await
            })
            .await?
            .clone();
        Ok(real_lib_name)
    }

    #[allow(dead_code)]
    pub async fn link_dylib(
        &self,
        key: &TestKey,
        build: &BuildOutput,
    ) -> Result<LinkOutput, LinkError> {
        let _token = self
            .concurrency_limiter
            .acquire()
            .await
            .expect("failed to acquire concurrency limit semaphore");
        let dynamic_lib_name = self.dynamic_lib_name(key);
        info!("linking     {dynamic_lib_name}");
        build_harness_dylib(&self.toolchains, &self.paths, build, &dynamic_lib_name)
    }

    pub async fn link_bin(
        &self,
        key: &TestKey,
        build: &BuildOutput,
    ) -> Result<LinkOutput, LinkError> {
        let _token = self
            .concurrency_limiter
            .acquire()
            .await
            .expect("failed to acquire concurrency limit semaphore");
        let bin_name = self.bin_name(key);
        info!("linking     {bin_name}");
        let bin_main = if let WriteImpl::HarnessCallback = key.options.val_writer {
            self.paths.harness_bin_main_file()
        } else {
            self.paths.freestanding_bin_main_file()
        };
        build_harness_main(&self.toolchains, &self.paths, build, &bin_name, &bin_main)
    }

    fn static_lib_name(&self, key: &TestKey, call_side: CallSide) -> String {
        self.base_id(key, Some(call_side), "_")
    }

    fn dynamic_lib_name(&self, key: &TestKey) -> String {
        let mut output = self.base_id(key, None, "_");
        output.push_str(".dll");
        output
    }

    fn bin_name(&self, key: &TestKey) -> String {
        let mut output = self.base_id(key, None, "_");
        if cfg!(target_os = "windows") {
            output.push_str(".exe");
        }
        output
    }
}

async fn build_static_lib(
    paths: &Paths,
    src_path: &Utf8Path,
    toolchain: Arc<dyn Toolchain + Send + Sync>,
    call_side: CallSide,
    static_lib_name: &str,
) -> Result<String, BuildError> {
    let lib_name = match call_side {
        CallSide::Callee => toolchain.compile_callee(src_path, &paths.out_dir, static_lib_name)?,
        CallSide::Caller => toolchain.compile_caller(src_path, &paths.out_dir, static_lib_name)?,
    };

    Ok(lib_name)
}

/// Compile and link the test harness with the two sides of the FFI boundary.
fn build_harness_dylib(
    toolchains: &Toolchains,
    paths: &Paths,
    build: &BuildOutput,
    dynamic_lib_name: &str,
) -> Result<LinkOutput, LinkError> {
    let target = &toolchains.platform_info.target;
    let rustc = &toolchains.rustc_command;

    let src = paths.harness_dylib_main_file();
    let output = paths.out_dir.join(dynamic_lib_name);
    let mut cmd = Command::new(rustc);
    cmd.arg("-v")
        .arg("-L")
        .arg(&paths.out_dir)
        .arg("-l")
        .arg(&build.caller_lib)
        .arg("-l")
        .arg(&build.callee_lib)
        .arg("--crate-type")
        .arg("cdylib")
        .arg("--target")
        .arg(target)
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

/// Compile and link the test harness with the two sides of the FFI boundary.
fn build_harness_main(
    toolchains: &Toolchains,
    paths: &Paths,
    build: &BuildOutput,
    bin_name: &str,
    bin_main: &Utf8Path,
) -> Result<LinkOutput, LinkError> {
    let target = &toolchains.platform_info.target;
    let rustc = &toolchains.rustc_command;

    let output = paths.out_dir.join(bin_name);
    let mut cmd = Command::new(rustc);
    cmd.arg("-v")
        .arg("-L")
        .arg(&paths.out_dir)
        .arg("-l")
        .arg(&build.caller_lib)
        .arg("-l")
        .arg(&build.callee_lib)
        .arg("--crate-type")
        .arg("bin")
        .arg("--target")
        .arg(target)
        // .arg("-Csave-temps=y")
        // .arg("--out-dir")
        // .arg("target/temp/")
        .arg("-o")
        .arg(&output)
        .arg(bin_main);

    debug!("running: {:?}", cmd);
    let out = cmd.output()?;

    if !out.status.success() {
        Err(LinkError::RustLink(out))
    } else {
        Ok(LinkOutput { test_bin: output })
    }
}
