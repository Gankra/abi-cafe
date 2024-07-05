# Usage

To run ABI Cafe, just [checkout the repository](https://github.com/Gankra/abi-cafe) and `cargo run`!

(Working on a prebuilt solution, a few last blockers to resolve before shipping that.)

While ABI Cafe isn't a For Reals Fuzzer (yet?), it accomplishes a similar goal through the magic of procedural generation and combinatorics. These docs serve to describe the N layers of combinatorics we use to turn a grain of sand into a mountain of broken compilers.

- [test files: `--tests`](./harness/combos/tests.md)
- [calling conventions: `--conventions`](./harness/combos/conventions.md)
- [type reprs: `--reprs`](./harness/combos/reprs.md)
- [toolchain pairings: `--pairs`](./harness/combos/toolchains.md)
- [value generators: `--gen-vals`](./harness/combos/values.md)
- [value selectors: `--select-vals`](./harness/combos/selectors.md)
- [value writers: `--write-vals`](./harness/combos/writers.md)

When you run `abi-cafe` we will end up running the cross-product of all of these settings, typically resulting in thousands of function calls. See the subsections for details!

You can also run `--help` to get information on all the supported features.



