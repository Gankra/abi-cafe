# ABI-Checker

This tool helps automate testing that two languages/compilers agree on ABIs for the purposes of FFI. This is still in early development so lots of stuff is stubbed out.

The principle of the tool is as follows:

* Define a function signature that one impl should call the other with
* Generate (or handwrite) both impls' versions (of both sides) of the interface
* Have each side report what it thinks the values are with global callbacks
* Compile both sides as static libs, and link into a dynamic lib harness
* Load that dynamic lib in, pass in the callbacks, and run it
* Check that both sides reported the same values

By running this natively on whatever platform you care about, this will tell you what FFI interfaces do and don't currently work. Ideally all you need to do is `cargo run`, but we're dealing with native toolchains...




# Supported Features

Here are the current things that work.


## Implementations

* System rustc (via your PATH)
* System C (via the CC crate, which can also grab a local msvc)

We automatically test both "Rust calls C" and "C calls Rust".

In theory other implementations aren't *too bad* to add. You just need to:

* Add an implementation of abis::Abi
    * Specify the name/extension of the impl
    * Specify how to generate a caller from a signature
    * Specify how to generate a callee from a signature
    * Specify how to compile a source file to a static lib
* Register what you want it paired with in abis::TEST_PAIRS
    * i.e. (YOUR_ABI, C_ABI) will have the harness test you calling into C

See the Test Harness section below for details on how to use it.


## Calling Conventions

* Platform Default C Convention (extern "C")
* Handwritten Conventions (opaque to the framework, lets you do whatever)

In the future we'd like to also be able to generate more specific ABIs like stdcall/fastcall/etc. Any test which specifies the "All" convention would implicitly combinatorically generate every known convention (constrained to the platform, presumably).


## Types

The test format support for the following types/concepts, although the implementations need to fill in support for some of them:

* fixed-width integer types (uint8_t and friends)
* float/double
* bool
* structs
* opaque pointers (void*)
* (TODO) arrays 
* (TODO) all of the above but explicitly wrapped in "by-reference"


# Adding Tests

Tests are specified as [ron](https://github.com/ron-rs/ron) files in the test directory, because it's more compact than JSON, has comments, and is more reliable with large integers. Because C is in some sense the "lingua franca" of FFI that everyone has to deal with, we prefer using C types in these definitions.

**Tests must also be registered in the TESTS global in main.rs**

The "default" workflow is to handwrite a ron file, and the testing framework will handle generating the actual code implementating that interface (example: structs.ron). Generated impls will be output to the generated_impls dir for debugging. Build artifacts will get dumped in target/temp/ if you want to debug those too.

Example:

```rust
Test(
    // name of this set of tests
    name: "examples",  
    // generate tests for the following function signatures
    funcs: [
        (
            // base function/subtest name (but also subtest name)
            name: "some_prims",
            // what calling conventions to generate this test for
            // (All = do them all, a good default)
            conventions: [All],
            // args
            inputs: [
               Int(c_int32_t(5)),
               Int(c_uint64_t(0x123_abc)),
            ]
            // return value
            output: Some(Bool(true)),
        ),
        (
            name: "some_structs",
            conventions: [All],
            inputs: [
               Int(c_int32_t(5)),
               // Struct decls are implicit in usage.
               // All structs with the same name must match!
               Struct("MyStruct", [
                  Int(c_uint8_t(0xf1)), 
                  Float(c_double(1234.23)),
               ]), 
               Struct("MyStruct", [
                  Int(c_uint8_t(0x1)), 
                  Float(c_double(0.23)),
               ]),
            ],
            // no return (void)
            output: None,
        ),
    ]
)
```

However, you have two "power user" options available:

* Generate the ron itself with generate_procedural_tests in main.rs (example: ui128.ron). This is good for bruteforcing a bunch of different combinations if you just want to make sure a type/feature generally works in many different situations.

* Use the "Handwritten" convention and manually provide the implementations (example: opaque_example.ron). This lets you basically do *anything* without the testing framework having to understand your calling convention or type/feature. Manual impls go in handwritten_impls and use the same naming/structure as generated_impls.



# The Test Harness

Implementation details of dylib test harness are split up between main.rs and the contents of the top-level harness/ directory. The contents of harness/ include:

* "headers" for the testing framework for each language
* harness.rs, which defines the entry-point for the test and sets up all the global callbacks/pointers. This is linked with the callee and caller to create the final dylib.

Ideally you shouldn't have to worry about *how* the callbacks work, so I'll just focus on the idea/usage. To begin with, here is an example of using this interface:

```C
// Caller Side
uint64_t basic_val(struct MyStruct arg0, int32_t arg1);

void do_test(void) {
    // Initialize and report the inputs
    struct MyStruct arg0 = { 241, 1234.23 };
    WRITE(CALLER_INPUTS, (char*)&arg0.field0, (uint32_t)sizeof(arg0.field0));
    WRITE(CALLER_INPUTS, (char*)&arg0.field1, (uint32_t)sizeof(arg0.field1));
    FINISHED_VAL(CALLER_INPUTS);
    
    int32_t arg1 = 5;
    WRITE(CALLER_INPUTS, (char*)&arg1, (uint32_t)sizeof(arg1));
    FINISHED_VAL(CALLER_INPUTS);

    // Do the call
    uint64_t output = basic_val(arg0, arg1);

    // Report the output
    WRITE(CALLER_OUTPUTS, (char*)&output, (uint32_t)sizeof(output));
    FINISHED_VAL(CALLER_INPUTS);

    // Declare that the test is complete on our side
    FINISHED_FUNC(CALLER_INPUTS, CALLER_OUTPUTS);
}
```

```C
// Callee Side
uint64_t basic_val(struct MyStruct arg0, int32_t arg1) {
    // Report the inputs
    WRITE(CALLEE_INPUTS, (char*)&arg0.field0, (uint32_t)sizeof(arg0.field0));
    WRITE(CALLEE_INPUTS, (char*)&arg0.field1, (uint32_t)sizeof(arg0.field1));
    FINISHED_VAL(CALLEE_INPUTS);
    
    WRITE(CALLEE_INPUTS, (char*)&arg1, (uint32_t)sizeof(arg1));
    FINISHED_VAL(CALLEE_INPUTS);

    // Initialize and report the output
    uint64_t output = 17;
    WRITE(CALLEE_OUTPUTS, (char*)&output, (uint32_t)sizeof(output));
    FINISHED_VAL(CALLEE_INPUTS);

    // Declare that the test is complete on our side
    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);

    // Actually return
    return output;
}
```

The high level idea is that each side:

* Uses WRITE to report individual fields of values (to avoid padding)
* Uses FINISHED_VAL to specify that all fields for a value have been written
* Uses FINISHED_FUNC to specify that the current function is done (the caller will usually contain many subtests, FINISHED_FUNC delimits those)

There are 4 buffers: CALLER_INPUTS, CALLER_OUTPUTS, CALLEE_INPUTS, CALLEE_OUTPUTS. Each side should only use its two buffers.

The signatures of the callbacks are:

* `WRITE(Buffer buffer, char* input, uint32_t size_of_input)`
* `FINISH_VAL(Buffer buffer)`
* `FINISH_TEST(Buffer input_buffer, Buffer output_buffer)`

Doing things in this very explicit way gives the test harness a better semantic understanding of what the implementations think is happening. This helps us emit better diagnostics and avoid cascading failures between subtests.
