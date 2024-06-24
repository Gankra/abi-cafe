use kdl_script::types::Ty;
use vals::ArgValuesIter;

use crate::error::*;
use crate::report::*;
use crate::*;

impl TestHarness {
    pub async fn check_test(
        &self,
        key: &TestKey,
        RunOutput {
            caller_inputs,
            caller_outputs,
            callee_inputs,
            callee_outputs,
        }: &RunOutput,
    ) -> CheckOutput {
        let test = self.tests[&key.test].clone();
        let options = &key.options;
        // Now check the results

        // Start peeling back the layers of the buffers.
        // funcs (subtests) -> vals (args/returns) -> fields -> bytes

        let mut results: Vec<Result<(), CheckFailure>> = Vec::new();

        // `Run` already checks that this length is congruent with all the inputs/outputs Vecs
        let expected_funcs = key.options.functions.active_funcs(&test.types);

        // Layer 1 is the funcs/subtests. Because we have already checked
        // that they agree on their lengths, we can zip them together
        // to walk through their views of each subtest's execution.
        'funcs: for (
            (((&func_idx, caller_inputs), caller_outputs), callee_inputs),
            callee_outputs,
        ) in expected_funcs
            .iter()
            .zip(&caller_inputs.funcs)
            .zip(&caller_outputs.funcs)
            .zip(&callee_inputs.funcs)
            .zip(&callee_outputs.funcs)
        {
            let func = test.types.realize_func(func_idx);
            let func_name = &func.name;
            let mut expected_args = test.vals.at_func(func_idx);
            let mut expected_inputs = vec![];
            let mut expected_outputs = vec![];
            for _ in &func.inputs {
                let arg = expected_args.next_arg();
                if arg.should_write_arg(options) {
                    expected_inputs.push((arg.arg_idx, arg));
                }
            }
            for _ in &func.outputs {
                let arg = expected_args.next_arg();
                if arg.should_write_arg(options) {
                    expected_outputs.push((arg.arg_idx, arg));
                }
            }

            // Now we must enforce that the caller and callee agree on how
            // many inputs and outputs there were. If this fails that's a
            // very fundamental issue, and indicative of a bad test generator.
            if caller_inputs.len() != expected_inputs.len()
                || callee_inputs.len() != expected_inputs.len()
            {
                results.push(Err(CheckFailure::ArgCountMismatch {
                    func_idx,
                    func_name: func_name.to_string(),
                    arg_kind: "input".to_string(),
                    expected_len: expected_inputs.len(),
                    caller: caller_inputs.clone(),
                    callee: callee_inputs.clone(),
                }));
                continue 'funcs;
            }
            if caller_outputs.len() != expected_outputs.len()
                || callee_outputs.len() != expected_outputs.len()
            {
                results.push(Err(CheckFailure::ArgCountMismatch {
                    func_idx,
                    func_name: func_name.to_string(),
                    arg_kind: "output".to_string(),
                    expected_len: expected_inputs.len(),
                    caller: caller_outputs.clone(),
                    callee: callee_outputs.clone(),
                }));
                continue 'funcs;
            }

            // Layer 2 is the values (arguments/returns).
            // The inputs and outputs loop do basically the same work,
            // but are separate for the sake of error-reporting quality.

            // Process Inputs
            for (((arg_idx, expected_arg), caller_vals), callee_vals) in expected_inputs
                .into_iter()
                .zip(caller_inputs)
                .zip(callee_inputs)
            {
                if let Err(e) = self.check_vals(
                    key,
                    "input",
                    func_idx,
                    arg_idx,
                    expected_arg,
                    caller_vals,
                    callee_vals,
                ) {
                    results.push(Err(e));
                    continue 'funcs;
                }
            }

            // Process Outputs
            for (((arg_idx, expected_arg), caller_vals), callee_vals) in expected_outputs
                .into_iter()
                .zip(caller_outputs)
                .zip(callee_outputs)
            {
                if let Err(e) = self.check_vals(
                    key,
                    "output",
                    func_idx,
                    arg_idx,
                    expected_arg,
                    caller_vals,
                    callee_vals,
                ) {
                    results.push(Err(e));
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

        CheckOutput {
            all_passed,
            subtest_names: names,
            subtest_checks: results,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_vals(
        &self,
        key: &TestKey,
        arg_kind: &str,
        func_idx: usize,
        arg_idx: usize,
        mut expected_arg: ArgValuesIter,
        caller_vals: &Vec<Vec<u8>>,
        callee_vals: &Vec<Vec<u8>>,
    ) -> Result<(), CheckFailure> {
        let test = &self.tests[&key.test];
        let types = &test.types;
        let options = &key.options;
        let func = types.realize_func(func_idx);

        let mut expected_vals = vec![];
        let arg_name = &expected_arg.arg().arg_name;
        let arg_ty = expected_arg.arg().ty;
        for val_idx in 0..expected_arg.arg().vals.len() {
            let val = expected_arg.next_val();
            if val.should_write_val(options) {
                expected_vals.push((val_idx, val))
            }
        }
        // Now we must enforce that the caller and callee agree on how
        // many fields each value had.
        if caller_vals.len() != expected_vals.len() || callee_vals.len() != expected_vals.len() {
            return Err(CheckFailure::ValCountMismatch {
                func_idx,
                arg_idx,
                arg_kind: arg_kind.to_string(),
                func_name: func.name.to_string(),
                arg_name: arg_name.to_string(),
                expected_len: expected_vals.len(),
                caller: caller_vals.clone(),
                callee: callee_vals.clone(),
            });
        }

        // Layer 3 is the leaf subfields of the values.
        // At this point we just need to assert that they agree on the bytes.
        for (((val_idx, expected_val), caller_val), callee_val) in
            expected_vals.into_iter().zip(caller_vals).zip(callee_vals)
        {
            if let Ty::Tagged(tagged_ty) = types.realize_ty(expected_val.ty) {
                // This value is "fake" and is actually the semantic tag of tagged union.
                // In this case showing the bytes doesn't make sense, so show the Variant name
                // (although we get bytes here they're the array index into the variant,
                // a purely magical value that only makes sense to the harness itself!).
                //
                // Also we use u32::MAX to represent a poison "i dunno what it is, but it's
                // definitely not the One variant we statically expected!", so most of the
                // time we're here to print <other variant> and shrug.
                let expected_tag = expected_val.generate_idx(tagged_ty.variants.len());
                let caller_tag =
                    u32::from_ne_bytes(<[u8; 4]>::try_from(&caller_val[..4]).unwrap()) as usize;
                let callee_tag =
                    u32::from_ne_bytes(<[u8; 4]>::try_from(&callee_val[..4]).unwrap()) as usize;

                if caller_tag != expected_tag || callee_tag != expected_tag {
                    let expected = tagged_ty.variants[expected_tag].name.to_string();
                    let caller = tagged_ty
                        .variants
                        .get(caller_tag)
                        .map(|v| v.name.as_str())
                        .unwrap_or("<other variant>")
                        .to_owned();
                    let callee = tagged_ty
                        .variants
                        .get(callee_tag)
                        .map(|v| v.name.as_str())
                        .unwrap_or("<other variant>")
                        .to_owned();
                    return Err(CheckFailure::TagMismatch {
                        func_idx,
                        arg_idx,
                        val_idx,
                        arg_kind: arg_kind.to_string(),
                        func_name: func.name.to_string(),
                        arg_name: arg_name.to_string(),
                        arg_ty_name: types.format_ty(arg_ty),
                        val_path: expected_val.path.to_string(),
                        val_ty_name: types.format_ty(expected_val.ty),
                        expected,
                        caller,
                        callee,
                    });
                }
            } else if caller_val != callee_val {
                // Make a buffer with the expected value
                let mut expected = vec![0; caller_val.len().max(callee_val.len())];
                expected_val.fill_bytes(&mut expected);
                return Err(CheckFailure::ValMismatch {
                    func_idx,
                    arg_idx,
                    val_idx,
                    arg_kind: arg_kind.to_string(),
                    func_name: func.name.to_string(),
                    arg_name: arg_name.to_string(),
                    arg_ty_name: types.format_ty(arg_ty),
                    val_path: expected_val.path.to_string(),
                    val_ty_name: types.format_ty(expected_val.ty),
                    expected,
                    caller: caller_val.clone(),
                    callee: callee_val.clone(),
                });
            }
        }

        Ok(())
    }
}
