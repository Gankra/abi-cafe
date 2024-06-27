use camino::Utf8Path;
use camino::Utf8PathBuf;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::info;

use crate::abis::*;
use crate::error::*;
use crate::*;

const GENERATED_SRC_DIR: &str = "generated_impls";

impl TestHarness {
    pub async fn generate_test(&self, key: &TestKey) -> Result<GenerateOutput, GenerateError> {
        // FIXME: these two could be done concurrently
        let caller_src = self.generate_src(key, CallSide::Caller).await?;
        let callee_src = self.generate_src(key, CallSide::Callee).await?;

        Ok(GenerateOutput {
            caller_src,
            callee_src,
        })
    }

    async fn generate_src(
        &self,
        key: &TestKey,
        call_side: CallSide,
    ) -> Result<Utf8PathBuf, GenerateError> {
        let test = self
            .test_with_vals(&key.test, key.options.val_generator)
            .await?;
        let abi_id = key.abi_id(call_side).to_owned();
        let test_with_abi = self.test_with_abi_impl(test, abi_id).await?;
        let src_path = self.src_path(key, call_side);

        // Briefly lock this map to insert/acquire a OnceCell and then release the lock
        let once = self
            .generated_sources
            .lock()
            .unwrap()
            .entry(src_path.clone())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        // Either acquire the cached result, or make it
        let _ = once
            .get_or_try_init(|| {
                let abi_impl = self.abi_by_test_key(key, call_side);
                let options = key.options.clone();
                info!("generating  {}", &src_path);
                generate_src(&src_path, abi_impl, test_with_abi, call_side, options)
            })
            .await?;
        Ok(src_path)
    }

    fn src_path(&self, key: &TestKey, call_side: CallSide) -> Utf8PathBuf {
        let src_dir = Utf8PathBuf::from(GENERATED_SRC_DIR);
        let abi_id = key.abi_id(call_side);
        let abi = self.abi_by_test_key(key, call_side);
        let mut output = self.base_id(key, Some(call_side), "_");
        output.push('.');
        output.push_str(abi.src_ext());
        src_dir.join(abi_id).join(output)
    }
}

/// Delete and recreate the generated src dir
pub fn init_generate_dir() -> Result<(), GenerateError> {
    std::fs::create_dir_all(GENERATED_SRC_DIR)?;
    std::fs::remove_dir_all(GENERATED_SRC_DIR)?;
    std::fs::create_dir_all(GENERATED_SRC_DIR)?;
    Ok(())
}

async fn generate_src(
    src_path: &Utf8Path,
    abi: Arc<dyn AbiImpl + Send + Sync>,
    test_with_abi: Arc<TestWithAbi>,
    call_side: CallSide,
    options: TestOptions,
) -> Result<(), GenerateError> {
    let mut output_string = String::new();
    let test = test_with_abi.with_options(options)?;
    match call_side {
        CallSide::Callee => abi.generate_callee(&mut output_string, test)?,
        CallSide::Caller => abi.generate_caller(&mut output_string, test)?,
    }

    // Write the result to disk
    std::fs::create_dir_all(src_path.parent().expect("source file had no parent!?"))?;
    let mut output = File::create(src_path)?;
    output.write_all(output_string.as_bytes())?;

    Ok(())
}
