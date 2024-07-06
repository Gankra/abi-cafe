# toolchains

Toolchains refer to a specific compiler (or configuration of a compiler). The entire purpose of ABI Cafe is to take two compilers and pair them up, checking that code built with one can properly call into code built by the other.

Within ABI Cafe, each Toolchain also comes with a [code generation backend](../combos/toolchains.md), which can take [a header file describing some types and functions](../../kdl-script/index.md) and generate either an implementation of those functions, or a caller of those functions.


## `--toolchains`

The following toolchains are available, but only "rustc" and "cc" are enabled by default.

* rustc - uses the rustc on your PATH
* cc - gets the "system" C compiler via the CC crate (supports msvc on windows)
* gcc - explicitly run the gcc on your PATH (probably less reliable than cc)
* clang  - explicitly run the clang on your PATH (probably less reliable than cc)
* ~~msvc~~ (incomplete)

You can also add custom rustc codegen backends as new toolchain (inheriting all the behaviour of the rustc toolchain) with `--rust-codegen-backend=mytoolchain:path/to/codegen_backend`. Where `mytoolchain` is a custom id for referring to it in `--pairs` and test output.


## `--pairs`

By default, we will look at the enabled toolchains and pair them with themselves and all the default "pairer" toolchains (if they're enabled). The default pairer toolchains are "rustc" and "cc".

With the default toolchains enabled, that means we will test:

* rustc_calls_rustc
* cc_calls_cc
* rustc_calls_cc
* cc_calls_rustc


## Adding A Toolchain

Adding a toolchain has two levels of difficulty:

* Easier: Adding a new compiler or mode for an existing language
* Harder: Adding a brand new language (and its compiler)

The easier case is probably "adding some settings to an existing Toolchain" while the harder case is probably "writing a code generator for a language (with the help of abi-cafe's libraries)".

(Although [if you want to add a C++ backend](https://github.com/Gankra/abi-cafe/issues/31), *probably* you want it to be a variant of the C toolchain, and not a whole new one, since, a lot of overlap there?)


### Adding A New Compiler Or Mode For An Existing Language

All the work you'll probably want to do is in `src/toolchains/mod.rs`. Looking at how gcc is implemented as a variant of `CcToolchain` (`src/toolchains/c.rs`) is probably informative.

At a minimum you will need to change `toolchains::create_toolchains` to create and register your `Toolchain`.

`Toolchain::compile_caller` and `Toolchain::compile_callee` will likely need to be changed to select your compiler, or use the compiler flags for your mode.

`Toolchain::generate_caller` and `Toolchain::generate_callee` may also need to be modified to generate source code that is compatible with your new compiler/mode. For instance when adding a

To test your new toolchain out you can first make sure it works with itself by running:

```
cargo run -- --toolchains=mytoolchain
```

(where "mytoolchain" is the id you registered in `create_toolchains`)

Once more confident you can pair it up with other compilers by running:

```
cargo run -- --toolchains=mytoolchain,rustc,cc
```


### Adding A New Language (And Its Compiler)

In addition to the things you need to do in the previous section, you now need to specify how to generate source code for your language given [a header file describing some types and functions](../../kdl-script/index.md).

I have good news, bad news, and okay news.

The good news is we have several libraries and utilities for helping with this.

The bad news is that no matter what you're going to need to create like a thousand lines of business logic for specifying the syntax of the language.

The okay news is that a lot of this can be accomplished without *too much* pain by copying one of the existing Toolchains and just editing it incrementally, aggressively returning `UnimplementedError` to indicate unsupported features, or things you just haven't gotten to yet. This is totally fine, all the backends have places where they give up!

As a codegen backend you will need to answer 3 major questions:

* declare: How do you declare types in this language?
* init: How do you initialize values in this language?
* write: How do you access and print fields of values in this language?

This largely amounts to writing recursive functions which match on a type definition and loop over all the fields of that type, or handle primitive types as the base case.

For reference, when the author of ABI Cafe rewrote the codebase, the C backend was recreated mostly from scratch in a day by copying the Rust implementation and changing Rust syntax to C syntax.

In doing this, you will have 3 major allies (assuming you match the idioms of the other codgen backends):

* `state.types` (`TypedProgram`) is the type system of the KDLScript Compiler, which gives you type ids for interning state, and handles computing facts about the type definitions
* `state.vals`(`ValueTree`) has the values and enum variants your program should use
* `f` (`Fivemat`) is an indent-aware Write implementation for creating pretty formatted code
