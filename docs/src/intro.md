# introduction

> Not sure if your compilers have matching ABIs? Then put them through the ultimate compatibility crucible and pair them up on a shift at The ABI Café! Find out if your one true pairing fastcalls for each other or are just another slowburn disaster. (Maid outfits optional but recommended.)

abi-cafe automates testing that two languages/compilers agree on ABIs for the purposes of FFI.

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

