# tests

ABI Cafe tests are defined are [KDLScript header files](../../kdl-script/index.md) describing an interface, for which we'll [generate and build many different](../combos.md) implementations (callees) and users (callers). KDLScript was purpose-made for ABI Cafe, with a custom syntax that avoids us tying our hands to any particular language/semantics. The syntax will feel fairly familiar to Rust programmers.

Each function in the test is regarded as a "subtest" that can be individually passed/failed.


## Adding Tests

The default suite of tests can be [found in `/include/tests/`](https://github.com/Gankra/abi-cafe/tree/main/include/tests), which is statically embedded in abi-cafe's binary. You don't need to register the test anywhere, we will just try to parse every file in the tests directory.

There are two kinds of tests: `.kdl` ("[normal](https://github.com/Gankra/abi-cafe/tree/main/include/tests/normal)") and `.procgen.kdl` ("[procgen](https://github.com/Gankra/abi-cafe/tree/main/include/tests/procgen)").

Procgen tests are sugar for normal tests, where you just define a type with the same name of the file (so `MetersU32.procgen.kdl` is expected to define a type named `MetersU32`), and we generate a battery of types/functions that stress test that the ABI handles that type properly.

**We recommend preferring procgen tests, because they're simpler to write and will probably have better coverage than if you tried to manually define all the functions.**

Suggested Examples:

* [simple.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/normal/simple.kdl) - a little example of a "normal" test with explicitly defined functions to test
* [SimpleStruct.procgen.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/struct/SimpleStruct.procgen.kdl) - similar to simple.kdl, but procgen
* [MetersU32.procgen.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/pun/MetersU32.procgen.kdl) - an example of a ["pun type"](../kdl-script/types/pun.md), where different languages use different definitions
* [IntrusiveList.procgen.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/fancy/IntrusiveList.procgen.kdl) - an example of how we can procgen tests for self-referential types and tagged unions
* [i8.procgen.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/primitive/i8.procgen.kdl) - ok this one isn't instructive it's just funny that it can be a blank file because i8 is builtin so all the info needed is in the filename


## Test Rules (Expectations)

ABI Cafe test expectations are [statically defined in ABI Cafe's code](https://github.com/Gankra/abi-cafe/blob/42d906a0f6c422a9ae345abc3bb257483a369b69/src/report.rs#L35-L50). There is a region specifically reserved where patchfiles can be applied to the codebase to allow updates to happen.

The benefit of this approach is that you get the full expressivity of actual code to specify when a test should fail and why, [but having proper runtime test expectation files is a good idea](https://github.com/Gankra/abi-cafe/issues/9).

We call test expectations TestRules, [which have two settings](https://github.com/Gankra/abi-cafe/blob/42d906a0f6c422a9ae345abc3bb257483a369b69/src/report.rs#L19-L23):

* [TestRunMode](https://github.com/Gankra/abi-cafe/blob/42d906a0f6c422a9ae345abc3bb257483a369b69/src/report.rs#L219-L238): up to what "phase" should we run the test. These are, in increasing order:
    * Skip: don't run the test at all (marked as skipped)
    * Generate: generate the source code
    * Build: compile the source code into staticlibs
    * Link: link the staticlibs into a dylib
    * Run: load and run the dylib's main function
    * Check: check the output of the dylib for agreement
* [TestCheckMode](https://github.com/Gankra/abi-cafe/blob/42d906a0f6c422a9ae345abc3bb257483a369b69/src/report.rs#L240-L257): to what level of correctness should the test be graded?
    * Pass(TestRunMode): The test must succesfully complete this phase, after that whatever
    * Fail(TestRunMode): The test must fail at this exact phase (indicates failing is "correct")
    * Busted(TestRunMode): Same as Fail, but indicates this is a bug/flaw that should eventually be fixed, and *not* the desired result longterm.
    * Random: The test is flakey and random but we want to run it anyway, so accept whatever result we get as ok.

The default TestRule is of course to run everything and expect everything to work (`run: Check, check: Pass(Check)`).


## `--tests`

By default we will run all known tests. Passing the names of tests (the filename without the extension(s)) to `--tests` will instead make us run only those tests (unlike the cargo test harness this isn't a fuzzy/substring match (but it could be if someone wants to implement that)).


## `--add-tests`

While it's ideal for tests to be upstreamed into ABI Cafe's codebase where everyone can benefit from them, you can also add your own custom tests that are read at runtime (instead of baked into the binary) by passing a path to a directory containing them to `--add-tests`.


## `--disable-builtin-tests`

If, for whatever reason, you want all the builtin tests to go away, you can pass `--disable-builtin-tests` to do so. Presumably you'll want to use `--add-tests` as well if you do.
