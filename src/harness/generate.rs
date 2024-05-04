use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use crate::abis::*;
use crate::error::*;
use crate::report::*;

use super::full_test_name;

pub fn generate_test_src(
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
        let mut caller_output_string = String::new();
        caller.generate_caller(
            &mut caller_output_string,
            test_key.caller_variant.for_impl(
                convention,
                test_key.caller_variant.types.all_funcs(),
                WriteImpl::HarnessCallback,
            )?,
        )?;
        caller_output.write_all(caller_output_string.as_bytes())?;

        let mut callee_output = File::create(&callee_src)?;
        let mut callee_output_string = String::new();
        callee.generate_callee(
            &mut callee_output_string,
            test_key.callee_variant.for_impl(
                convention,
                test_key.callee_variant.types.all_funcs(),
                WriteImpl::HarnessCallback,
            )?,
        )?;
        callee_output.write_all(callee_output_string.as_bytes())?;
    }

    Ok(GenerateOutput {
        caller_src,
        callee_src,
    })
}
