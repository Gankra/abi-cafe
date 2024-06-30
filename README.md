# abi-cafe 🧩☕️❤️

Not sure if your compilers have matching ABIs? Then put them through the ultimate compatibility crucible and pair them up on a shift at The ABI Café! Find out if your one true pairing fastcalls for each other or are just another slowburn disaster. (Maid outfits optional but recommended.)

# About

Run --help to get options for configuring execution.

This tool helps automate testing that two languages/compilers agree on ABIs for the purposes of FFI. This is still in early development so lots of stuff is stubbed out.

The principle of the tool is as follows:

* Define a function signature that one impl should call the other with
* Generate both impls' versions (of both sides) of the interface
* Have each side report what it thinks the values are with global callbacks
* Compile both sides as static libs, and link into a dynamic lib harness
* Load that dynamic lib in, pass in the callbacks, and run it
* Check that both sides reported the same values
* Generate minimized/simplified versions of any found failures

By running this natively on whatever platform you care about, this will tell you what FFI interfaces do and don't currently work. Ideally all you need to do is `cargo run`, but we're dealing with native toolchains so, expect toolchain bugs!

By default we will:

* run all tests
* under every possible calling convention
* for a selection of reasonable "impl calls impl" pairings (e.g. rustc_calls_cc)

But you can the CLI interface lets you override these defaults. This is especially useful for --pairs because it lets you access *more* specific pairings, like if you really want to specifically test gcc_calls_clang.



# Supported Features

Here are the current things that work.


## Implementations

"ABI Implementations" refer to a specific compiler or language which claims to implement some ABIs.
The currently supported AbiImpls are:

* rustc - uses the rustc on your PATH
* cc - gets the "system" C compiler via the CC crate (supports msvc on windows)
* gcc - explicitly run the gcc on your PATH (probably less reliable than cc)
* clang  - explicitly run the clang on your PATH (probably less reliable than cc)
* ~~msvc~~ (unimplemented)

By default, we test the following pairings:

* rustc_calls_rustc
* cc_calls_cc
* rustc_calls_cc
* cc_calls_rustc

In theory other implementations aren't *too bad* to add. You just need to:

* Add an implementation of abis::AbiImpl
    * Specify the language and source-file extension
    * Specify how to generate source for a caller from a signature
    * Specify how to generate source for a callee from a signature
    * Specify how to compile a source file to a static lib
* Register it in the `abi_impls` map in `fn main`
* (Optional) Register what you want it paired with by default in `DEFAULT_TEST_PAIRS`
    * i.e. (ABI_IMPL_YOU, ABI_IMPL_CC) will have the harness test you calling into C

The bulk of the work is specifying how to generate source code, which can be done
incrementally by return UnimplementedError to indicate unsupported features. This
is totally fine, all the backends have places where they give up!

See the Test Harness section below for details on how to use it.


## Calling Conventions

Each language may claim to support a particular set of calling conventions
(and may use knowledge of the target platform to adjust their decisions).
We try to generate and test all supported conventions.

Universal Conventions:

* c: the platform's default C convention (`extern "C"`)

Windows Conventions:

* cdecl
* fastcall
* stdcall
* vectorcall


## Types

The abi-cafe typesystem is [defined by kdl-script](https://github.com/Gankra/abi-cafe/blob/main/kdl-script/README.md#types), see those docs for details, but, we basically support most of the types you could define/use in core Rust.


# Adding Tests

Tests are specified as [kdl-script](https://github.com/Gankra/abi-cafe/blob/main/kdl-script/README.md) files, which are basically C header files, but with a custom syntax that avoids us tying our hands to any particular language/semantics. The syntax will feel fairly familiar to Rust programmers.

The default suite of tests can be [found in `/include/tests/`](https://github.com/Gankra/abi-cafe/tree/main/include/tests), which is statically embedded in abi-cafe's binary. You don't need to register the test anywhere, we will just try to parse every file in the tests directory.

There are two kinds of tests: `.kdl` ("[normal](https://github.com/Gankra/abi-cafe/tree/main/include/tests/normal)") and `.procgen.kdl` ("[procgen](https://github.com/Gankra/abi-cafe/tree/main/include/tests/procgen)").

The latter is sugar for the former, where you just define a type with the same name of the file (so `MetersU32.procgen.kdl` is expected to define a type named `MetersU32`), and we generate a battery of types/functions that stress it out.

Suggested Examples:

* [simple.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/normal/simple.kdl) - a little example of a "normal" test with explicitly defined functions to test
* [SimpleStruct.procgen.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/struct/SimpleStruct.procgen.kdl) - similar to simple.kdl, but procgen
* [MetersU32.procgen.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/pun/MetersU32.procgen.kdl) - an example of a ["pun type"](https://github.com/Gankra/abi-cafe/tree/main/kdl-script#pun-types), where different languages use different definitions
* [IntrusiveList.procgen.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/fancy/IntrusiveList.procgen.kdl) - an example of how we can procgen tests for self-referential types and tagged unions
* [i8.procgen.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/primitive/i8.procgen.kdl) - ok this one isn't instructive it's just funny that it can be a blank file because i8 is builtin so all the info needed is in the filename



# The Test Harness

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

# Trophy Case

* [x64 linux clang and gcc disagree on __int128 pass-on-stack ABI](https://github.com/rust-lang/rust/issues/54341#issuecomment-1064729606)
  * We already knew clang and rustc disagreed [because clang does a manual alignment adjustment](https://reviews.llvm.org/D28990), but we didn't seem to fully understand that the clang adjustment is actually buggy and doesn't apply to the implicit push-to-stack when passing __int128 by-val. gcc always aligns the value, even when pushing to stack, so the two desync in this case.
  * This tool was written to investigate the clang-rustc issue, and helped establish that everyone agreed on the ABI on ARM64, where __int128 is essentially part of the *hardware's* ABI due to it showing up in the layout for saving/restoring SIMD register state. As a result, [rust's libc crate now exposes typedefs for __int128 on those platforms](https://github.com/rust-lang/libc/pull/2719)
* [rustc_codegen_cranelift ICE on passing 11 bools by-val](https://github.com/bjorn3/rustc_codegen_cranelift/issues/1234)
