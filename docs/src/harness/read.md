# test files

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
