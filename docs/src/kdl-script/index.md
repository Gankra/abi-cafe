# KDLScript

KDLScript, the [KDL](https://kdl.dev/)-based programming language!

KDLScript ("Cuddle Script") is a "fake" scripting language that actually just exists to declare type/function signatures without tying ourselves to any particular language's semantics. It exists to be used by [ABI Cafe](../intro.md).

Basically, KDLScript is a header format we can make as weird as we want for our own usecase:


```kdl
struct "Point" {
    x "f32"
    y "f32"
}

enum "ScaleMode" {
    Width
    Height
}

fn "print" {
    inputs { _ "Point"; }
}

fn "scale" {
    inputs { _ "Point"; factor "f32"; scalemode "ScaleMode"; }
    outputs { _ "Point"; }
}

fn "sum" {
    inputs { _ "&[Point; 4]"; }
    outputs { _ "Point"; }
}
```

Ultimately the syntax and concepts are heavily borrowed from Rust, for a few reasons:

* The author is very comfortable with Rust
* This (and [ABI Cafe](../intro.md)) were originally created to find bugs in rustc
* Rust is genuinely just a solid language for interfaces! (Better than C/C++)

The ultimate goal of this is to test that languages can properly communicate over
FFI by declaring the types/interface once and generating the Rust/C/C++/... versions
of the program (both caller and callee) and then linking them into various combinations like "Rust calls C++" to check that the values are passed correctly.



## Quickstart


[kdl-script](https://github.com/Gankra/abi-cafe/tree/main/kdl-script) is both a library and a CLI application. The CLI is just for funsies.

The main entry point to the library is `Compiler::compile_path` or `Compiler::compile_string`, which will produce a `TypedProgram`. [See the `types` module docs for how to use that](https://github.com/Gankra/abi-cafe/blob/main/kdl-script/src/types.rs).

The CLI application can be invoked as `kdl-script path/to/program.kdl` to run a KDLScript program.
