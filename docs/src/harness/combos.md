# usage

To run ABI Cafe, just `cargo install abi-cafe` and run `abi-cafe`!

While ABI Cafe isn't a For Reals Fuzzer (yet?), it accomplishes a similar goal through the magic of procedural generation and combinatorics. These docs serve to describe the N layers of combinatorics we use to turn a grain of sand into a mountain of broken compilers.

- [test files: `--tests`](./combos/tests.md)
- [calling conventions: `--conventions`](./combos/conventions.md)
- [type reprs: `--reprs`](./combos/reprs.md)
- [toolchain pairings: `--pairs`](./combos/toolchains.md)
- [value generators: `--gen-vals`](./combos/values.md)
- [value selectors: `--select-vals`](./combos/selectors.md)
- [value writers: `--write-vals`](./combos/writers.md)

When you run `abi-cafe` we will end up running the cross-product of all of these settings, typically resulting in thousands of function calls. See the subsections for details!

You can also run `--help` to get information on all the supported features.


## As Part Of Your Testsuite

We're still cleaning up the details of this usecase to make it nicer. If you would like to use abi-cafe in your testsuite, [please let us know what you'd need/want](https://github.com/Gankra/abi-cafe/issues/60)!

At a minimum, all you need to "do" to run abi-cafe in your test suite is run `abi-cafe` and check its status.
Of course that's not so helpful when you don't know what it does! The default behaviours you are most likely to want to configure are:

* [Selecting toolchains](./combos/toolchains.md)
* [Running tests](./combos/tests.md)

(FIXME: write some docs that go over how to read test failures. [grafitti](./combos/values.md#graffiti-values) is really useful to understand, as is [the minimizer](https://github.com/Gankra/abi-cafe/issues/38), although that needs work).


## Examples

The following uses of abi-cafe all vendor it, as previously this was required.

* [abi-cafe's own CI (runs on various platforms with rustc stable and nightly)](https://github.com/Gankra/abi-cafe/blob/main/.github/workflows/cafe.yml)
* [rustc_codegen_cranelift's CI (adds custom rustc codegen backend, configures toolchains)](https://github.com/rust-lang/rustc_codegen_cranelift/blob/master/.github/workflows/abi-cafe.yml)
* [rustc_codgegen_gcc's WIP support for abi-cafe](https://github.com/rust-lang/rustc_codegen_gcc/pull/710)
