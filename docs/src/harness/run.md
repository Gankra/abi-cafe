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
    // Declare we're running the first function
    set_func(CALLER_VALS, 0);

    // Initialize and report the inputs
    MyStruct arg0 = { 241, 1234.23 };
    write_val(CALLER_VALS, 0, arg0.field0);
    write_val(CALLER_VALS, 1, arg0.field1);

    int32_t arg1 = 5;
    write_val(CALLER_VALS, 2, arg1);

    // Do the call
    uint64_t output = basic_val(arg0, arg1);

    // Report the output
    write_val(CALLER_VALS, 3, output);
}
```

```C
// Callee Side
uint64_t basic_val(MyStruct arg0, int32_t arg1) {
    // Declare we're running the first function
    set_func(CALLEE_VALS, 0);

    // Report the inputs
    write_val(CALLEE_VALS, 0, arg0.field0);
    write_val(CALLEE_VALS, 1, arg0.field1);

    write_val(CALLEE_VALS, 2, arg1);

    // Initialize and report the output
    uint64_t output = 17;
    write_val(CALLEE_VALS, 3, output);

    // Actually return
    return output;
}
```

The high level idea is that each side:

* Uses set_func to specify which function we're running
* Uses write_val to report individual fields of args (to avoid padding)

There are 2 buffers: CALLER_VALS and CALLEE_VALS

The signatures of the callbacks are as follows, but each language wraps these
in functions/macros to keep the codegen readable:

```c
SET_FUNC(Buffer buffer, uint32_t func_idx)`
WRITE_VAL(Buffer buffer, uint32_t val_idx, char* input, uint32_t size_of_input)
```

Doing things in this very explicit way gives the test harness a better semantic understanding of what the implementations think is happening. This helps us emit better diagnostics and avoid cascading failures between subtests.
