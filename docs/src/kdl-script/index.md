# kdl-script

kdl-script is the Compiler for KDLScript, the [KDL][]-based programming language!

KDLScript is a "fake" scripting language that actually just exists to declare
type/function definitions in a language-agnostic way to avoid getting muddled
in the details of each language when trying to talk about All Languages In Reality.
It exists to be used by [abi-cafe](./harness/index.md).

Basically, KDLScript is a header format we can make as weird as we want for our own usecase.

Ultimately the syntax and concepts are heavily borrowed from Rust, for a few reasons:

* The author is very comfortable with Rust
* This (and [abi-cafe][]) were originally created to find bugs in rustc
* Rust is genuinely just a solid language for expressing ABIs! (Better than C/C++)

The ultimate goal of this is to test that languages can properly communicate over
FFI by declaring the types/interface once and generating the Rust/C/C++/... versions
of the program (both caller and callee) and then linking them into various combinations like "Rust calls C++" to check that the values are passed correctly.

-------


kdl-script is both a library and a CLI application. The CLI is just for funsies.

The main entry point to the library is [`Compiler::compile_path`][] or [`Compiler::compile_string`][],
which will produce a [`TypedProgram`][]. See the [`types`][] module docs for how to use that.

The CLI application can be invoked as `kdl-script path/to/program.kdl` to run a KDLScript program.

FIXME: Write some examples! (See the `examples` dir for some.)

