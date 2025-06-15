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
* [MetersU32.procgen.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/pun/MetersU32.procgen.kdl) - an example of a ["pun type"](../../kdl-script/types/pun.md), where different languages use different definitions
* [IntrusiveList.procgen.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/fancy/IntrusiveList.procgen.kdl) - an example of how we can procgen tests for self-referential types and tagged unions
* [i8.procgen.kdl](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/primitive/i8.procgen.kdl) - ok this one isn't instructive it's just funny that it can be a blank file because i8 is builtin so all the info needed is in the filename


## Test Rules (Expectations)

ABI Cafe's default expectations can be [found in `/include/harness/abi-cafe-rules.toml`](https://github.com/Gankra/abi-cafe/tree/main/include/harness/abi-cafe-rules.toml), which is statically embedded in abi-cafe's binary (that file also contains some example annotations).

Every entry in the rules is a *selector* of the form `target.<platform>.<test-pattern>`, how far to `run` tests that match the selector, and an *expectation* (`pass`, `fail`, `busted`, or `random`). Both run and expectation directives specify a testing stage to help ensure that tests pass or fail for the reason we expect (more on that below).

This annotated test expectation file from rustc_codegen_cranelift is a good example that flexes the syntax and features:

```toml
# on aarch64 linux...
# the F32Array test, with C convention...
# will run to completion, but have ABI errors (should be fixed).
[target.'cfg(all(target_arch = "aarch64", target_os = "linux"))']
'F32Array::conv_c'.busted = "check"

# On aarch64 macos...
[target.'cfg(all(target_arch = "aarch64", target_os = "macos"))']
# The SingleVariantUnion tests, with C convention, using repr(Rust)
# will run to completion, but have ABI errors (should be fixed)
'SingleVariantUnion::conv_c::repr_c'.busted = "check"
# The OptionU128 tests, with C convention, using repr(Rust), when Rust is the caller
# will crash when run (should be fixed)
'OptionU128::conv_rust::repr_c::rustc_caller'.busted = "run"
# The OptionU128 tests, with C convention, using repr(Rust), when Cranelift is the caller
# will run to completion, but have ABI errors (should be fixed)
'OptionU128::conv_rust::repr_c::cgclif_caller'.busted = "check"

# On x64 msvc windows...
[target.x86_64-pc-windows-msvc]
# The simple test, using Rust convention
# will run to completion, but have ABI errors (should be fixed)
'simple::conv_rust'.busted = "check"
# The simple test, using Rust convention, when the caller is rustc
# will crash when run (should be fixed)
'simple::conv_rust::rustc_caller'.busted = "run"

# skip the f16 tests
[target.'*'.'f16']
run = "skip"

# skip the f128 tests
[target.'*'.'f128']
run = "skip"
```


### `<platform>`

A test rule's `<platform>` has the same syntax as [cargo target tables](https://doc.rust-lang.org/cargo/reference/config.html#target): it can either be a target triple, or a rustc cfg. We also support `*` as a platform that matches all.


### `<test-key>`

A test rule's `<test-key>` is the same `::`-separated value that abi-cafe will use to refer to [a combinatoric variant of a test](../combos.md), like `CLikeTagged::conv_c::repr_c::cc_calls_rustc` (this test-key is of the form `<test>::<convention>::<repr>::<pair>`). If abi-cafe reports a test failure you can just copy that key and use it here. However parts of the test key can be omitted to wildcard match groups of tests. 

A test-key can include the following parts:

* `<test>`: the name of the test (`CLikeTagged`, this is the only purely positional part of a key, it must come first. You can omit it by starting a key with `::`) 
* `<convention>`: the [calling convention](./conventions.md) (`conv_c`)
* `<repr>`: the [repr of aggregates](./reprs.md) (`repr_c`)
* `<generator>`: the [value generator](./values.md) (`graffiti`)
*  a [toolchain pairing selector](./toolchains.md):
    * `<pairing>`: this exact toolchain pairing in this order (`cc_calls_rustc`)
    * `<caller>`: the caller must be this toolchain (`rustc_caller`)
    * `<callee>`: the callee must be this toolchain (`cc_callee`)
    * `<toolchain>`: any pairing that includes this toolchain (`rustc_toolchain`)


### `run`

A test rule's `run` field specifies what `<phase>` to run the test to. The default is `run = "check"`.
The phases are, in increasing order:

* `"skip"`: don't run the test at all (marked as skipped)
* `"generate"`: generate the source code
* `"build"`: compile the source code into staticlibs
* `"link"`: link the staticlibs into a binary
* `"run"`: execute the binary
* `"check"`: check the values reported by the binary (this is the default)


### `<expectation>`

A test rule's `<expectation>` field specifies what result the test should produce. The default is `pass = "check"`.
The expectations are:

* `pass = <phase>`: The test must run at least up to `<phase>` and report a success in that phase
* `fail = <phase>`: The test must fail at this exact phase (use when failing is "correct")
* `busted = <phase>`: Same as Fail, but indicates this is a bug/flaw that should eventually be fixed, and *not* the desired result longterm.
* `random = true`: The test is flakey and random but we want to run it anyway, so accept whatever result we get as ok. 


## Configuring Tests

The following CLI flags are notable for changing what tests/rules to use:

### `--tests`

By default we will run all known tests. Passing the names of tests (the filename without the extension(s)) to `--tests` will instead make us run only those tests (unlike the cargo test harness this isn't a fuzzy/substring match (but it could be if someone wants to implement that)).

See [the top-level combo docs for other flags that change the set of tests we combinatorically generate](../combos.md).

### `--add-tests`

While it's ideal for tests to be [upstreamed into ABI Cafe's codebase](https://github.com/Gankra/abi-cafe/tree/main/include/tests) where everyone can benefit from them, you can also add your own custom tests that are read at runtime (instead of baked into the binary) by passing a path to a directory containing them via `--add-tests path/to/dir/`.


### `--rules`

While it's ideal for rules to be [upstreamed into ABI Cafe's codebase](https://github.com/Gankra/abi-cafe/blob/main/include/harness/abi-cafe-rules.toml) where everyone can benefit from them, you can also add your own custom test rules that are read at runtime (instead of baked into the binary) by passing a path to a file containing them via `--add-tests path/to/abi-cafe-rules.toml`.


### `--disable-builtin-tests`

If, for whatever reason, you want all the [builtin tests](https://github.com/Gankra/abi-cafe/tree/main/include/tests) to go away, you can pass `--disable-builtin-tests` to do so. Presumably you'll want to use `--add-tests` as well if you do.


### `--disable-builtin-rules`

If, for whatever reason, you want all the [builtin rules](https://github.com/Gankra/abi-cafe/blob/main/include/harness/abi-cafe-rules.toml) to go away, you can pass `--disable-builtin-rules` to do so. Presumably you'll want to use `--rules` as well if you do.
