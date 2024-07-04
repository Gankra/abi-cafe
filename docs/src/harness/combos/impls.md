# impls (pairing compilers!)

"ABI Implementations" refer to a specific compiler or language which claims to implement some ABIs.
The currently supported Toolchains are:

* rustc - uses the rustc on your PATH
* cc - gets the "system" C compiler via the CC crate (supports msvc on windows)
* gcc - explicitly run the gcc on your PATH (probably less reliable than cc)
* clang  - explicitly run the clang on your PATH (probably less reliable than cc)
* ~~msvc~~ (unimplemented)

By default, we test the following pairings:

* rustc_calls_rustc
* cc_calls_cc
* rustc_calls_cc
* cc_calls_rustc

In theory other implementations aren't *too bad* to add. You just need to:

* Add an implementation of abis::Toolchain
    * Specify the language and source-file extension
    * Specify how to generate source for a caller from a signature
    * Specify how to generate source for a callee from a signature
    * Specify how to compile a source file to a static lib
* Register it in the `toolchains` map in `fn main`
* (Optional) Register what you want it paired with by default in `DEFAULT_TEST_PAIRS`
    * i.e. (TOOLCHAIN_YOU, TOOLCHAIN_CC) will have the harness test you calling into C

The bulk of the work is specifying how to generate source code, which can be done
incrementally by return UnimplementedError to indicate unsupported features. This
is totally fine, all the backends have places where they give up!

See the Test Harness section below for details on how to use it.

