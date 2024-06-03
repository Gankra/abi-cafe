use crate::error::*;
use crate::report::*;
use crate::*;

impl TestHarness {
    pub async fn check_test(
        &self,
        key: &TestKey,
        RunOutput {
            caller: _,
            callee: _,
            caller_inputs,
            caller_outputs,
            callee_inputs,
            callee_outputs,
        }: &RunOutput,
    ) -> CheckOutput {
        let test = self.tests[&key.test].clone();
        let caller_impl = self
            .test_with_abi_impl(&test, key.caller.clone())
            .await
            .unwrap();

        // Now check the results

        // Start peeling back the layers of the buffers.
        // funcs (subtests) -> vals (args/returns) -> fields -> bytes

        let mut results: Vec<Result<(), CheckFailure>> = Vec::new();

        // Layer 1 is the funcs/subtests. Because we have already checked
        // that they agree on their lengths, we can zip them together
        // to walk through their views of each subtest's execution.
        'funcs: for (
            func_idx,
            (((caller_inputs, caller_outputs), callee_inputs), callee_outputs),
        ) in caller_inputs
            .funcs
            .iter()
            .zip(&caller_outputs.funcs)
            .zip(&callee_inputs.funcs)
            .zip(&callee_outputs.funcs)
            .enumerate()
        {
            // Now we must enforce that the caller and callee agree on how
            // many inputs and outputs there were. If this fails that's a
            // very fundamental issue, and indicative of a bad test generator.
            if caller_inputs.len() != callee_inputs.len() {
                results.push(Err(CheckFailure::InputCountMismatch(
                    func_idx,
                    caller_inputs.clone(),
                    callee_inputs.clone(),
                )));
                continue 'funcs;
            }
            if caller_outputs.len() != callee_outputs.len() {
                results.push(Err(CheckFailure::OutputCountMismatch(
                    func_idx,
                    caller_outputs.clone(),
                    callee_outputs.clone(),
                )));
                continue 'funcs;
            }

            // Layer 2 is the values (arguments/returns).
            // The inputs and outputs loop do basically the same work,
            // but are separate for the sake of error-reporting quality.

            // Process Inputs
            for (input_idx, (caller_val, callee_val)) in
                caller_inputs.iter().zip(callee_inputs).enumerate()
            {
                // Now we must enforce that the caller and callee agree on how
                // many fields each value had.
                if caller_val.len() != callee_val.len() {
                    results.push(Err(CheckFailure::InputFieldCountMismatch(
                        func_idx,
                        input_idx,
                        caller_val.clone(),
                        callee_val.clone(),
                        String::from("todo"),
                    )));
                    continue 'funcs;
                }

                // Layer 3 is the leaf subfields of the values.
                // At this point we just need to assert that they agree on the bytes.
                for (field_idx, (caller_field, callee_field)) in
                    caller_val.iter().zip(callee_val).enumerate()
                {
                    if caller_field != callee_field {
                        results.push(Err(CheckFailure::InputFieldMismatch(
                            func_idx,
                            input_idx,
                            field_idx,
                            caller_field.clone(),
                            callee_field.clone(),
                            String::from("todo"),
                        )));
                        continue 'funcs;
                    }
                }
            }

            // Process Outputs
            for (output_idx, (caller_val, callee_val)) in
                caller_outputs.iter().zip(callee_outputs).enumerate()
            {
                // Now we must enforce that the caller and callee agree on how
                // many fields each value had.
                if caller_val.len() != callee_val.len() {
                    results.push(Err(CheckFailure::OutputFieldCountMismatch(
                        func_idx,
                        output_idx,
                        caller_val.clone(),
                        callee_val.clone(),
                    )));
                    continue 'funcs;
                }

                // Layer 3 is the leaf subfields of the values.
                // At this point we just need to assert that they agree on the bytes.
                for (field_idx, (caller_field, callee_field)) in
                    caller_val.iter().zip(callee_val).enumerate()
                {
                    if caller_field != callee_field {
                        results.push(Err(CheckFailure::OutputFieldMismatch(
                            func_idx,
                            output_idx,
                            field_idx,
                            caller_field.clone(),
                            callee_field.clone(),
                        )));
                        continue 'funcs;
                    }
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
        let names = caller_impl
            .types
            .all_funcs()
            .map(|func_id| {
                self.full_subtest_name(key, &caller_impl.types.realize_func(func_id).name)
            })
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
}
