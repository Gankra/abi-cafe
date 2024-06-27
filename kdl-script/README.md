# kdl-script

[![crates.io](https://img.shields.io/crates/v/kdl-script.svg)](https://crates.io/crates/kdl-script) [![docs](https://docs.rs/kdl-script/badge.svg)](https://docs.rs/kdl-script) ![Rust CI](https://github.com/Gankra/kdl-script/workflows/Rust%20CI/badge.svg?branch=main)


A Compiler for KDLScript, the [KDL][]-based programming language!

KDLScript is a "fake" scripting language that actually just exists to declare
type/function definitions in a language-agnostic way to avoid getting muddled
in the details of each language when trying to talk about All Languages In Reality.
It exists to be used by [abi-cafe][].

Ultimately the syntax and concepts are heavily borrowed from Rust, for a few reasons:

* The author is very comfortable with Rust
* This (and [abi-cafe][]) were originally created to find bugs in rustc 
* Rust is genuinely just a solid language for expressing ABIs! (Better than C/C++)

The ultimate goal of this is to test that languages can properly communicate over
FFI by declaring the types/interface once and generating the Rust/C/C++/... versions
of the program (both caller and callee) and then linking them into various combinations
like "Rust Calls C++" to check that the values are passed correctly. 

Since C is the lingua-franca of FFI, it's assumed that when lowering definitions
to the target language that they should be the "C Compatible" equivalent. For instance,
"u32" is intended to be your language's equivalent of "uint32_t".

In Rust this means all type definitions are implicitly `#[repr(C)]` (ideally should be
opt-outable). Most details about the function are left abstract so that [abi-cafe][]
can choose how to fill those in.

"C Compatible" gets fuzzier for some things like tagged unions. 
Just uh... don't worry about it (see "Concepts" below for details).




# Usage

kdl-script is both a library and a CLI application.

The main entry point to the library is [`Compiler::compile_path`][] or [`Compiler::compile_string`][],
which will produce a [`TypedProgram`][]. See the [`types`][] module docs for how to use that.

The CLI application can be invoked as `kdl-script path/to/program.kdl` to run a KDLScript program.

TODO: Write some examples! (See the `examples` dir for some.)



# Concepts

A KdlScript program is a single file that has types, functions, and attributes (`@`) on those functions.


## Types

The following kinds of types exist in KDLScript.

### Nominal Types

A KDLScript user is allowed to declare the following kinds of nominal types

* `struct` - a plain ol' struct
* `enum` - a c-style enum
* `union` - an untagged union
* `tagged` - a tagged union (rust-style enum)
* `alias` - a transparent type alias
* `pun` - a pun across the FFI boundary, "CSS for ifdefs"

`struct`, `enum`, `union`, and `alias` are hopefully pretty self-explanatory and uncontroversial as they have immediate C/C++ equivalents, so we'll speed through them quickly. `tagged` and `pun` merit deeper discussion.





#### Struct Types

Just what you expect!

```kdl
struct "Point" {
    x "f32"
    y "f32"
}
```

Field names may be marked "positional" by giving them the name `_`. If all fields are positional then languages like Rust should emit a "tuple struct". So this:

```kdl
struct "Point" {
    _ "f64"
    _ "f64"
}
```

would be emitted as this Rust:

```rust
#[repr(C)]
struct Point(f64, f64);
```

Otherwise positional names will get autonamed something like `field0`, `field1`, etc.





#### Enum Types

Just what you expect!

```kdl
enum "IOError" {
    FileNotFound 2
    FileClosed
    FightMe 4
}
```

The optional integer values specify what value that variant should "have" in its underlying integer repr. Otherwise variants start at 0(?) and auto-increment.

Just like struct fields, enum variants can have "positional" naming with `_` which will get autonaming like `Case0`, `Case1`, etc.

TODO: is it ok to have multiple variants with the same value? Whose responsibility is it to bounds check them to the underlying type, especially in the default case where it's "probably" a fuzzy `c_int`?

TODO: add an attribute for specifying the underlying integer type. (`@tag`?)



#### Union Types

Just what you expect!

```kdl
union "FloatOrInt" {
    Float "f32"
    Int "u32"
}
```

Just like struct fields, union variants can have "positional" naming with `_` which will get autonaming like `Case0`, `Case1`, etc.

TODO: should we allow inline struct decls like C/C++ does, or require the user to define the structs separately?




#### Alias Types

Just what you expect!

```kdl
alias "MetersU32" "u32"
```

Note that the ordering matches Rust `type Alias = RealType;` syntax and not C/C++'s backwards-ass typedef syntax.

See Pun Types for additional notes on how aliases interact with typeids!



#### Tagged Types

Tagged is the equivalent of Rust's `enum`, a tagged union where variants have fields, which has no "obvious" C/C++ analog.
Variant bodies may either be missing (indicating no payload) or have the syntax of a `struct` body. 

```kdl
tagged "MyOptionU32" {
    None
    Some { _ "u32"; }
    FileNotFound { 
        path "[u8; 100]"
        error_code "i64"
    }
}
```

However, [Rust RFC 2195 - "Really Tagged Unions"](https://github.com/rust-lang/rfcs/blob/master/text/2195-really-tagged-unions.md) specifies the C/C++ equivalent layout/ABI for these types when `repr(C)` and/or `repr(u8)` is applied to one. [cbindgen](https://github.com/eqrion/cbindgen) implements this conversion. By default the `repr(C)` layout is to be used (how to select others is TBD).

A quick TL;DR of the RFC:

The `repr(C)` layout is the *most* obvious lowering to a `struct` containing a c-style `enum` and a `union`. The `enum` says which case of the union is currently valid.

The `repr(u8)` layout (also `repr(u32)`, etc.) is similar but the `enum` specifically has that integer type, and instead of being stored in a wrapper struct it's stored as the first field of every variant of the `union`. This is a more compact layout because space doesn't need to be wasted for padding between the `enum` and the `union`. Also it's more reliable because C refuses to specify what the backing integer of a normal enum *is* so rustc just guesses based on the platform.

TODO: should payload-less variants allow for integer values like `enum` variants do?

TODO: figure out how exactly to specify you really want `repr(rust)` or `repr(u8)` or `repr(C, u8)` layouts.

TODO: if we ever add support for Swift or whatever there will need to be A Reckoning because it has its own ABI/layouts for enums that are way more complex and guaranteed for more complicated cases! For now, we punt!



#### Pun Types

A pun is the equivalent of an ifdef'd type, allowing us to declare that two wildly different declarations in different languages should in fact have the same layout and/or ABI. A pun type contains "selector blocks" which are sequentially matched on much like CSS. The first one to match wins. When lowering to a specific backend/config if no selector matches, then compilation fails.

Here is an example that claims that a Rust `repr(transparent)` newtype of a `u32` should match the ABI of a `uint32_t` in C/C++:

```kdl
pun "MetersU32" {
    lang "rust" {        
        @ "#[repr(transparent)]"
        struct "MetersU32" {
            a "u32"
        }
    }

    lang "c" "cpp" {
        alias "MetersU32" "u32"
    }
}
```

Because of this design, the typechecker does not "desugar" `pun` types to their underlying type when computing type ids. This means `[MetersU32; 4]` will not be considered the same type as `[u32; 4]`... because it's not! This is fine because type equality is just an optimization for our transpiler usecase. Typeids mostly exist to deal with type name resolution.

Pun resolving is done as a second step when lowering the abstract `TypedProgram` to a more backend-concrete `DefinitionGraph`.

(`alias` also isn't desugarred and has the same "problem" but this is less "fundamental" and more "I want the backend to actually emit
a type alias and use the alias", just like the source KDLScript program says!)


The currently supported selector blocks are:

* `lang "lang1" "lang2" ...` - matches *any* of the languages
* `default` - always matches

Potentially Supported In The Future:

* `compiler "compiler1" "compiler2" ...`
* `cpu` ...
* `os` ...
* `triple` ...
* `any { selector1; selector2; }`
* `all { selector1; selector2; }`
* `not { selector; }`

TODO: should we allow pun blocks to have other types defined in the block that are "private" from the rest of the program but used in the final type?

TODO: figure out how to talk about "language-native types" in much the same way the above example uses "language-native annotations".



### Structural Types

A KDLScript user is allowed to use the following kinds of structural types

* `&T` - a transparent reference to T 
* `[T; N]` - a fixed-length array of T (length N) 
* `()` - the empty tuple (I just like adding this cuz it's cute!!!)

TODO: figure out if we want to bake in Rust's `Option` type here, if only for `Option<&T>`.

TODO: figure out if there's any point in non-empty tuples



#### Reference Types

The "value" of a reference type `&T` is its pointee for the purposes of [abi-cafe][]. In this regard it's similar to C++ references or Rust references, where most operations automagically talk about the pointee and not the pointer. Using a reference type lets you test that something can properly be passed-by-reference, as opposed to passed-by-value.

Reference types may appear in other composite types, indicating that the caller is responsible for allocating variables for each one and then storing pointers to them in the composite type.

When used in the outputs of a function, a reference type is sugar for an out-param that the caller is responsible for allocating
and the callee is responsible for initializing. Out-params should appear after all normal inputs but before varargs.

TODO: think through if the caller has any need to initialize an out-param (Currently irrelevant as we only suppport POD)

TODO: think through what it means to have an out-param of `[&u32; 4]`! (We should conservatively reject it for now.)




#### Array Types

Array types like `[u32; 4]` have the layout/repr you would expect from languages like C and Rust, but there's a problem with passing them by-value: C is supremely fucking weird about passing arrays by value if they're not wrapped in a struct.

This is actually sugar for pass-by-reference (and largely decays into `u32*`):

```C
void blah(u32[4] array);
```

And this doesn't even compile:

```C
u32[4] blah();
```

To avoid trying to squish weird square pegs in round holes, passing an array by-value like this in KDLScript should indeed mean passing it by-value! C/C++ backends should *simply refuse to lower such a KDLScript program and produce an error*. Rust backends are free to lower it in the obvious way. If you want to test the C way, use this:

```kdl
fn "blah" {
    inputs { _ "&[u32; 4]"; }
}
```

**NOT THIS**:

```kdl
fn "blah" {
    inputs { _ "[u32; 4]"; }
}
```

TODO: does anything special need to be said about empty arrays? Or do backends just YOLO the obvious lowering? (yes?)




### Primitives

There are various builtin primitives, such as:

* integers (i8, u128, ...)
* floats (f16, f32, f64, f128, ...)
* `bool`- your old pal the boolean (TriBool support TBD)
* `ptr` - an opaque pointer (`void*`), used when you're interested in the value of the pointer and not its pointee (unlike `&T`)

In the future there will probably be language-specific primitives like `c_long`.

TODO: figure out if `ptr` should be nullable or not by default. Relevant to whether we want to have Option and if Rust code should lower it to `*mut ()`.



## Functions

Functions are where the Actually Useful *library* version of KDLScript and the Just A Meme *application* version of KDLScript diverge. This difference is configured by the `eval` feature.

In library form KDLScript only has function *signature declarations*, and it's the responsibility of the [abi-cafe][] backend using KDLScript to figure out what the body should be. In binary form you can actually fill in the body with some hot garbage I hacked up.

For now we'll only document declaration.

Here is a fairly complicated/contrived example function:

```kdl
fn "my_func" {
    inputs {
        x "u32"
        y "[&MyType; 3]"
        _ "&bool"
    }
    outputs {
        _ "bool"
        _ "&ErrorCode"
    }
}
```

Functions can have arbitrarily many inputs and outputs with either named or "positional" names (which will get autonaming like `arg0`, `arg1` and `out0`, `out1`, etc.).

As discussed in the section on "Reference Types", references in outputs are sugar for out-params, which should appear after the inputs and before outputs. So the above would lower to something like the following in Rust (values chosen arbitrarily here, and we wouldn't use asserts in practice, but instead record the values for comparison):

```rust ,ignore
fn my_func(
    x: u32,
    y: [&MyType; 3],
    arg2: &bool,
    out1: &mut ErrorCode,
) -> bool {
    // Check the inputs are what we expect...
    assert_eq!(x, 5);
    assert_eq!(y[0].val, 8);
    assert_eq!(y[1].val, 9);
    assert_eq!(y[2].val, 10);
    assert_eq!(*arg2, true);

    // Return outputs
    *out1 = ErrorCode::Bad;
    return true;
}


fn my_func_caller() {
    // Setup the inputs
    let x = 5;
    let y_0 = MyType { val: 8 };
    let y_1 = MyType { val: 9 };
    let y_2 = MyType { val: 10 };
    let y = [&y_0, &y_1, &y_1];
    let arg2 = false;

    // Setup outparams
    let mut out1 = ErrorCode::default();
    
    // Do the call
    let out0 = my_func(x, y, &arg2, &mut out1);

    // Checkout outputs
    assert_eq!(out0, true);
    assert_eq!(*out1, ErrorCode::Bad);
}
```

> God writing that sucked ass, and it wasn't even the "proper" value checking! This is why I built all this complicated crap to automate it!

TODO: what does it mean if you have multiple non-out-param outputs? Return a tuple? Error out on all known backends?

TODO: contemplate if named args should be the equivalent of Swift named args, where the inner and outer name can vary, but the outer name is like, part of the function name itself (and/or ABI)?

TODO: contemplate varargs support

Strawman varargs syntax for saying there's varargs but that you want to test this particular set of values passed in them:

```kdl
func "blah" {
    inputs {
        x "u32"
        "..." {
            _ "f32"
            _ "f32"
        }
    }
}
```

SUB TODO: figure out if varargs should allow for specifying what the callee and caller think is happening as different

SUB TODO: figure out if we should try to support that fucked up thing where Swift supports multiple named varargs lists



## Attributes

Attributes start with `@` and apply to the next item (function or type) that follows them. This is only kinda stubbed out and not really impl'd.

TODO: add attributes:

* `@ "whatever you want here"` - passthrough attrs to underlying language
* `@tag "u32"` - setting the backing type on an `enum`'s tag
* `@packed N?` - making a type packed
* `@align N` - making a type aligned

TODO: should fields/args be able to have attributes?

TODO: at what level should attributes be validated/processed? The parser? Type checker? Backends?



# Demo

The evaluator has not at all kept up with the type system, so it can only handle some really simply stuff.
You can run the `examples/simple.kdl`. All the other examples will just dump type information and decl order
as they don't define `main`.

```text
> cargo run examples/simple.kdl

{
  y: 22
  x: 11
}
33
```

Is executing the following kdl document:


```kdl
@derive "Display"
struct "Point" {
    x "f64"
    y "f64"
}

fn "main" {
    outputs { _ "f64"; }

    let "pt1" "Point" {
        x 1.0
        y 2.0
    }
    let "pt2" "Point" {
        x 10.0
        y 20.0
    }
    
    let "sum" "add:" "pt1" "pt2"
    print "sum"

    return "+:" "sum.x" "sum.y"
}

fn "add" {
    inputs { a "Point"; b "Point"; }
    outputs { _ "Point"; }

    return "Point" {
        x "+:" "a.x" "b.x"
        y "+:" "a.y" "b.y"
    }
}
```


# Why Did You Make KDL Documents Executable???

To spite parsers.

Ok more seriously because I needed something like this for [abi-cafe][] but it's a ton of work so I'm self-motivating by wrapping it in the guise of a scripting language because it's funny and I can make more incremental progress, as I have a lot to implement before it's usable in the thing it's built for.



[abi-cafe]: https://github.com/Gankra/abi-cafe
[KDL]: https://kdl.dev/