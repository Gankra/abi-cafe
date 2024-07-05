# Configuration and Combinatorics

While ABI Cafe isn't a For Reals Fuzzer (yet?), it accomplishes a similar goal through the magic of procedural generation and combinatorics. These docs serve to describe the N layers of combinatorics we use to turn a grain of sand into a mountain of broken compilers.

- [test files](./harness/combos/tests.md)
- [calling conventions](./harness/combos/conventions.md)
- [type reprs](./harness/combos/reprs.md)
- [toolchains and pairings](./harness/combos/toolchains.md)
- [value generation (randomizing!)](./harness/combos/values.md)
- [value selection (minimizing!)](./harness/combos/filters.md)
- [value writes (exporting!)](./harness/combos/writes.md)

In a default execution, we will end up running a the cross-product of all of these settings

TODO