# ABI Cafe 🧩☕️❤️

> *Not sure if your compilers have matching ABIs? Then put them through the ultimate compatibility crucible and pair them up on a shift at The ABI Cafe! Find out if your one true pairing fastcalls for each other or are just another slowburn disaster. (Maid outfits optional but recommended.)*


## Quickstart

To run ABI Cafe, just [checkout the repository](https://github.com/Gankra/abi-cafe) and `cargo run`!


## What Is This

ABI Cafe automates testing that two languages/compilers agree on their ABIs.

**ABI Cafe is essentially an ABI fuzzer**, which:

* [Creates a header file describing an interface](https://faultlore.com/abi-cafe/book/kdl-script/index.html)
* [Generates source code for a *user* and *implementation* of that interface](https://faultlore.com/abi-cafe/book/harness/combos/toolchains.html)
* [Builds and runs the resulting program](https://faultlore.com/abi-cafe/book/harness/combos.html)
* [Checks that both sides saw the same values](https://faultlore.com/abi-cafe/book/harness/combos/values.html)

If they agree, great!

If they don't agree, even better, we just learned something! **We then try to diagnose why they disagreed, and generate a minimized version that a human can inspect and report!**

Now do this [a bajillion times](https://faultlore.com/abi-cafe/book/harness/combos.html) and suddenly we're learning a whole lot! Alternatively, you can [hand-craft any type or function signature you're interested in](https://faultlore.com/abi-cafe/book/kdl-script/index.html), and explore its interoperability between different toolchains.

ABI Cafe is purely *descriptive*. It has no preconceived notion of what *should* work, and it doesn't trust any damn thing anyone says about it. We don't analyze assembly or metadata, and we'll gleefully create programs riddled with Undefined Behaviour. We're here to *learn* not *lecture*.

This design is based on a fundamental belief that **ABIs exist only through sheer force of will**. The spec if often "read GCC's source code", and damn if that ain't an error-prone process. Also GCC doesn't even know you exist, and you're only going to keep interoperating with them if you check and maintain your work. So here's a tool for checking and maintaining your work!



## Choose Your Own Adventure

* [I want to use ABI Cafe in my compiler's testsuite](https://faultlore.com/abi-cafe/book/harness/combos.html)
* [I want to add support for my compiler/language to ABI Cafe](https://faultlore.com/abi-cafe/book/harness/combos/toolchains.html)
* [I want to add a test to ABI Cafe](https://faultlore.com/abi-cafe/book/harness/combos/tests.html)
* [I want to add a new kind of type to ABI Cafe](https://faultlore.com/abi-cafe/book/kdl-script/types/index.html)
