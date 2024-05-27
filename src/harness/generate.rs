use camino::Utf8Path;
use camino::Utf8PathBuf;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;

use crate::abis::*;
use crate::error::*;
use crate::{AbiImplId, CallSide, TestId, TestOptions};

const GENERATED_SRC_DIR: &str = "generated_impls";
const HANDWRITTEN_SRC_DIR: &str = "handwritten_impls";

pub fn src_path(
    test_id: &TestId,
    abi_id: &AbiImplId,
    abi: &dyn AbiImpl,
    call_side: CallSide,
    options: &TestOptions,
) -> Utf8PathBuf {
    let src_ext = abi.src_ext();
    let convention_name = options.convention.name();
    let call_side = call_side.name();
    let src_dir = if options.convention == CallingConvention::Handwritten {
        Utf8PathBuf::from(HANDWRITTEN_SRC_DIR)
    } else {
        Utf8PathBuf::from(GENERATED_SRC_DIR)
    };

    src_dir.join(abi_id).join(format!(
        "{test_id}_{convention_name}_{abi_id}_{call_side}.{src_ext}"
    ))
}

/// Delete and recreate the generated src dir
pub fn init_generate_dir() -> Result<(), GenerateError> {
    std::fs::create_dir_all(GENERATED_SRC_DIR)?;
    std::fs::remove_dir_all(GENERATED_SRC_DIR)?;
    std::fs::create_dir_all(GENERATED_SRC_DIR)?;
    Ok(())
}

pub async fn generate_src(
    src_path: &Utf8Path,
    abi: Arc<dyn AbiImpl + Send + Sync>,
    test_with_abi: Arc<TestForAbi>,
    call_side: CallSide,
    options: TestOptions,
) -> Result<(), GenerateError> {
    if let CallingConvention::Handwritten = options.convention {
        if src_path.exists() {
            return Ok(());
        } else {
            return Err(GenerateError::Skipped);
        }
    }
    let mut output_string = String::new();
    let query = test_with_abi.types.all_funcs();
    let write_impl = WriteImpl::HarnessCallback;
    let test = test_with_abi.with_options(options, query, write_impl)?;
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
