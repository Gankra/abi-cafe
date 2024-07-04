# Configuration and Combinatorics

While ABI Cafe isn't a For Reals Fuzzer (yet?), it accomplishes a similar goal through the magic of procedural generation and combinatorics. These docs serve to describe the N layers of combinatorics we use to turn a grain of sand into a mountain of broken compilers.

- [tests](./harness/combos/tests.md)
- [calling conventions](./harness/combos/conventions.md)
- [layouts](./harness/combos/reprs.md)
- [impls (pairing compilers!)](./harness/combos/impls.md)
- [value generation (randomizing!)](./harness/combos/values.md)
- [value selection (minimizing!)](./harness/combos/filters.md)
- [value writes (exporting!)](./harness/combos/writes.md)


