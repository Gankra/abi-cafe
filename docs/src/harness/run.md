# the runtime

Implementation details of dylib test harness are split up between [src/harness/run.rs](https://github.com/Gankra/abi-cafe/blob/main/src/harness/run.rs) and the contents of the top-level [/include/harness/](https://github.com/Gankra/abi-cafe/blob/main/include/harness/). The contents of /include/harness/ are:

* "headers" for the testing framework for each language
* harness.rs, which defines the entry-point for the test and sets up all the global callbacks/pointers. This is linked with the callee and caller to create the final dylib.

Ideally you shouldn't have to worry about *how* the callbacks work, so I'll just focus on the idea/usage. To begin with, here is an example of using this interface:

```C
// Caller Side
uint64_t basic_val(MyStruct arg0, int32_t arg1);

// The test harness will invoke your test through this symbol!
void do_test(void) {
    // Initialize and report the inputs
    MyStruct arg0 = { 241, 1234.23 };
    write_field(CALLER_INPUTS, arg0.field0);
    write_filed(CALLER_INPUTS, arg0.field1);
    finished_val(CALLER_INPUTS);

    int32_t arg1 = 5;
    write_field(CALLER_INPUTS, arg1);
    finished_val(CALLER_INPUTS);

    // Do the call
    uint64_t output = basic_val(arg0, arg1);

    // Report the output
    write_field(CALLER_OUTPUTS, output);
    finished_val(CALLER_OUTPUTS);

    // Declare that the test is complete on our side
    finished_func(CALLER_INPUTS, CALLER_OUTPUTS);
}
```

```C
// Callee Side
uint64_t basic_val(MyStruct arg0, int32_t arg1) {
    // Report the inputs
    write_field(CALLEE_INPUTS, arg0.field0);
    write_field(CALLEE_INPUTS, arg0.field1);
    finished_val(CALLEE_INPUTS);

    write_field(CALLEE_INPUTS, arg1);
    finished_val(CALLEE_INPUTS);

    // Initialize and report the output
    uint64_t output = 17;
    write_field(CALLEE_OUTPUTS, output);
    finished_val(CALLEE_OUTPUTS);

    // Declare that the test is complete on our side
    finished_func(CALLEE_INPUTS, CALLEE_OUTPUTS);

    // Actually return
    return output;
}
```

The high level idea is that each side:

* Uses write_field to report individual fields of values (to avoid padding)
* Uses finished_val to specify that all fields for a value have been written
* Uses finished_func to specify that the current function is done (the caller will usually contain many subtests, FINISHED_FUNC delimits those)

There are 4 buffers: CALLER_INPUTS, CALLER_OUTPUTS, CALLEE_INPUTS, CALLEE_OUTPUTS. Each side should only use its two buffers.

The signatures of the callbacks are as follows, but each language wraps these
in functions/macros to keep the codegen readable:

* `WRITE(Buffer buffer, char* input, uint32_t size_of_input)`
* `FINISH_VAL(Buffer buffer)`
* `FINISH_TEST(Buffer input_buffer, Buffer output_buffer)`

Doing things in this very explicit way gives the test harness a better semantic understanding of what the implementations think is happening. This helps us emit better diagnostics and avoid cascading failures between subtests.
