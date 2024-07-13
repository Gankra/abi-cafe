use console::Style;
use harness::run::{FuncBuffer, ValBuffer};
use kdl_script::types::PrimitiveTy;
use kdl_script::types::Ty;
use tracing::{error, info};

use crate::error::*;
use crate::report::*;
use crate::*;

impl TestHarness {
    pub async fn check_test(
        &self,
        key: &TestKey,
        RunOutput {
            caller_funcs,
            callee_funcs,
        }: &RunOutput,
    ) -> CheckOutput {
        let test = self
            .test_with_vals(&key.test, key.options.val_generator)
            .await
            .expect("check-test called before test_with_vals!?");
        let options = &key.options;
        let empty_func = FuncBuffer::default();
        let empty_val = ValBuffer::default();
        // Now check the results

        // Start peeling back the layers of the buffers.
        // funcs (subtests) -> vals (args/returns) -> fields -> bytes

        let mut results: Vec<Result<(), CheckFailure>> = Vec::new();

        // `Run` already checks that this length is congruent with all the inputs/outputs Vecs
        let expected_funcs = key.options.functions.active_funcs(&test.types);

        // Layer 1 is the funcs/subtests. Because we have already checked
        // that they agree on their lengths, we can zip them together
        // to walk through their views of each subtest's execution.
        'funcs: for func_idx in expected_funcs {
            let caller_func = caller_funcs.funcs.get(func_idx).unwrap_or(&empty_func);
            let callee_func = callee_funcs.funcs.get(func_idx).unwrap_or(&empty_func);
            let mut expected_vals = vec![];
            for arg in test.vals.at_func(func_idx) {
                for val in arg {
                    if val.should_write_val(options) {
                        expected_vals.push(val);
                    }
                }
            }

            for expected_val in expected_vals {
                let val_idx = expected_val.absolute_val_idx;
                let caller_val = caller_func.vals.get(val_idx).unwrap_or(&empty_val);
                let callee_val = callee_func.vals.get(val_idx).unwrap_or(&empty_val);
                if let Err(e) = self.check_val(&test, expected_val, caller_val, callee_val) {
                    results.push(Err(e));
                    // FIXME: now that each value is absolutely indexed,
                    // we should be able to check all the values independently
                    // and return all errors. However the first one is the most
                    // important one, so the UX needs to be worked on...
                    continue 'funcs;
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
            .types
            .all_funcs()
            .map(|func_id| self.full_subtest_name(key, &test.types.realize_func(func_id).name))
            .collect::<Vec<_>>();
        let max_name_len = names.iter().fold(0, |max, name| max.max(name.len()));
        let num_passed = results.iter().filter(|r| r.is_ok()).count();
        let all_passed = num_passed == results.len();

        if !all_passed {
            for (subtest_name, result) in names.iter().zip(&results) {
                match result {
                    Ok(()) => {
                        info!("Test {subtest_name:width$} passed", width = max_name_len);
                    }
                    Err(e) => {
                        let red = console::Style::new().red();
                        let message = format!(
                            "Test {subtest_name:width$} failed!\n{e}",
                            width = max_name_len
                        );
                        info!("{}", red.apply_to(message));
                    }
                }
            }
        }

        if all_passed {
            info!("{}", Style::new().green().apply_to("all tests passed"));
        } else {
            error!("only {}/{} tests passed!", num_passed, results.len());
        }

        CheckOutput {
            all_passed,
            subtest_names: names,
            subtest_checks: results,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_val(
        &self,
        test: &TestWithVals,
        expected_val: ValueRef,
        caller_val: &ValBuffer,
        callee_val: &ValBuffer,
    ) -> Result<(), CheckFailure> {
        let types = &test.types;
        // Enums and Taggeds are "fake" fields representing the semantic value (tag).
        // In this case showing the bytes doesn't make sense, so show the Variant name
        // (although we get bytes here they're the array index into the variant,
        // a purely magical value that only makes sense to the harness itself!).
        //
        // Also we use u32::MAX to represent a poison "i dunno what it is, but it's
        // definitely not the One variant we statically expected!", so most of the
        // time we're here to print <other variant> and shrug.
        if let Ty::Tagged(tagged_ty) = types.realize_ty(expected_val.ty) {
            let expected_tag = expected_val.generate_idx(tagged_ty.variants.len());
            let caller_tag = load_tag(caller_val);
            let callee_tag = load_tag(callee_val);

            if caller_tag != Some(expected_tag) || callee_tag != Some(expected_tag) {
                let expected = tagged_variant_name(tagged_ty, Some(expected_tag));
                let caller = tagged_variant_name(tagged_ty, caller_tag);
                let callee = tagged_variant_name(tagged_ty, callee_tag);
                return Err(tag_error(types, &expected_val, expected, caller, callee));
            }
        } else if let Ty::Enum(enum_ty) = types.realize_ty(expected_val.ty) {
            let expected_tag = expected_val.generate_idx(enum_ty.variants.len());
            let caller_tag = load_tag(caller_val);
            let callee_tag = load_tag(callee_val);

            if caller_tag != Some(expected_tag) || callee_tag != Some(expected_tag) {
                let expected = enum_variant_name(enum_ty, Some(expected_tag));
                let caller = enum_variant_name(enum_ty, caller_tag);
                let callee = enum_variant_name(enum_ty, callee_tag);
                return Err(tag_error(types, &expected_val, expected, caller, callee));
            }
        } else if let Ty::Primitive(PrimitiveTy::Bool) = types.realize_ty(expected_val.ty) {
            let expected_tag = expected_val.generate_idx(2);
            let caller_tag = load_tag(caller_val);
            let callee_tag = load_tag(callee_val);

            if caller_tag != Some(expected_tag) || callee_tag != Some(expected_tag) {
                let expected = bool_variant_name(expected_tag, Some(expected_tag));
                let caller = bool_variant_name(expected_tag, caller_tag);
                let callee = bool_variant_name(expected_tag, callee_tag);
                return Err(tag_error(types, &expected_val, expected, caller, callee));
            }
        } else if caller_val.bytes != callee_val.bytes {
            // General case, just get a pile of bytes to span both values
            let func = expected_val.func();
            let arg = expected_val.arg();
            let mut expected = vec![0; caller_val.bytes.len().max(callee_val.bytes.len())];
            expected_val.fill_bytes(&mut expected);
            // FIXME: this doesn't do the right thing for enums
            // <https://github.com/Gankra/abi-cafe/issues/34>
            return Err(CheckFailure::ValMismatch {
                func_idx: expected_val.func_idx,
                arg_idx: expected_val.arg_idx,
                val_idx: expected_val.val_idx,
                func_name: func.func_name.to_string(),
                arg_name: arg.arg_name.to_string(),
                arg_ty_name: types.format_ty(arg.ty),
                val_path: expected_val.path.to_string(),
                val_ty_name: types.format_ty(expected_val.ty),
                expected,
                caller: caller_val.bytes.clone(),
                callee: callee_val.bytes.clone(),
            });
        }

        Ok(())
    }
}

fn load_tag(val: &ValBuffer) -> Option<usize> {
    let buf = val.bytes.get(..4)?;
    let bytes = <[u8; 4]>::try_from(buf).ok()?;
    Some(u32::from_ne_bytes(bytes) as usize)
}

fn tagged_variant_name(tagged_ty: &kdl_script::types::TaggedTy, tag: Option<usize>) -> String {
    let Some(tag) = tag else {
        return "<tag never recorded?>".to_owned();
    };
    let tagged_name = &tagged_ty.name;
    let variant_name = tagged_ty
        .variants
        .get(tag)
        .map(|v| v.name.as_str())
        .unwrap_or("<other variant>");
    format!("{tagged_name}::{variant_name}")
}

fn enum_variant_name(enum_ty: &kdl_script::types::EnumTy, tag: Option<usize>) -> String {
    let Some(tag) = tag else {
        return "<tag never recorded?>".to_owned();
    };
    let enum_name = &enum_ty.name;
    let variant_name = enum_ty
        .variants
        .get(tag)
        .map(|v| v.name.as_str())
        .unwrap_or("<other variant>");
    format!("{enum_name}::{variant_name}")
}

fn bool_variant_name(expected_tag: usize, tag: Option<usize>) -> String {
    let Some(tag) = tag else {
        return "<tag never recorded?>".to_owned();
    };
    // Because we're using the tag variant machinery, this code is a bit weird,
    // because we essentially get passed Option<bool> for `tag`, where we get
    // None when the wrong path is taken.
    //
    // So to figure out what variant a bool is supposed to have, we work out
    // what variant the expected_tag has, and then either say "the same or opposite"
    let bools = ["false", "true"];
    let expected_bool = bools[expected_tag];
    let unexpected_bool = bools[1 - expected_tag];

    let res = if tag == expected_tag {
        expected_bool
    } else {
        unexpected_bool
    };
    res.to_owned()
}

fn tag_error(
    types: &kdl_script::TypedProgram,
    expected_val: &ValueRef,
    expected: String,
    caller: String,
    callee: String,
) -> CheckFailure {
    let func = expected_val.func();
    let arg = expected_val.arg();
    CheckFailure::TagMismatch {
        func_idx: expected_val.func_idx,
        arg_idx: expected_val.arg_idx,
        val_idx: expected_val.val_idx,
        func_name: func.func_name.to_string(),
        arg_name: arg.arg_name.to_string(),
        arg_ty_name: types.format_ty(arg.ty),
        val_path: expected_val.path.to_string(),
        val_ty_name: types.format_ty(expected_val.ty),
        expected,
        caller,
        callee,
    }
}
