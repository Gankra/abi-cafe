# kdl-script

[![crates.io](https://img.shields.io/crates/v/kdl-script.svg)](https://crates.io/crates/kdl-script) [![docs](https://docs.rs/kdl-script/badge.svg)](https://docs.rs/kdl-script) ![Rust CI](https://github.com/Gankra/abi-cafe/workflows/Rust%20CI/badge.svg?branch=main)


A Compiler for KDLScript, the [KDL][]-based programming language!

KDLScript is a "fake" scripting language that actually just exists to declare
type/function definitions in a language-agnostic way to avoid getting muddled
in the details of each language when trying to talk about All Languages In Reality.
It exists to be used by [abi-cafe][].

Basically, KDLScript is a header format we can make as weird as we want for our own usecase.

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




# CLI Usage

kdl-script is both a library and a CLI application. The CLI is just for funsies.

The main entry point to the library is [`Compiler::compile_path`][] or [`Compiler::compile_string`][],
which will produce a [`TypedProgram`][]. See the [`types`][] module docs for how to use that.

The CLI application can be invoked as `kdl-script path/to/program.kdl` to run a KDLScript program.

FIXME: Write some examples! (See the `examples` dir for some.)



# Concepts

A KdlScript program is a single file that has types, functions, and attributes (`@`) on those functions.


## Types

The following kinds of types exist in KDLScript.

* nominal types
    * `struct` - a plain ol' struct
    * `enum` - a c-style enum
    * `union` - an untagged union
    * `tagged` - a tagged union (rust-style enum)
    * `alias` - a transparent type alias
    * `pun` - a pun across the FFI boundary, "CSS for ifdefs"
* structural types
    * `[T;N]` - an array of T, length N
    * `&T` - a (mutable) reference to T (the pointee is regarded as the value)
* builtin primitives
    * integers (`i8`, `u128`, ...)
    * floats (`f16`, `f32`, `f64`, `f128`, ...)
    * `bool`- your old pal the boolean (TriBool support TBD)
    * `ptr` - an opaque pointer (`void*`), used when you're interested in the value of the pointer and not its pointee (unlike `&T`)
    * `()` - empty tuple

All of these types can be combined together as you expect, and self-referential
types

Note that we do not currently support generics

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

FIXME: is it ok to have multiple variants with the same value? Whose responsibility is it to bounds check them to the underlying type, especially in the default case where it's "probably" a fuzzy `c_int`?



#### Union Types

Just what you expect!

```kdl
union "FloatOrInt" {
    Float "f32"
    Int "u32"
}
```

Just like struct fields, union variants can have "positional" naming with `_` which will get autonaming like `Case0`, `Case1`, etc.




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

FIXME: should payload-less variants allow for integer values like `enum` variants do?



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



### Structural Types

A KDLScript user is allowed to use the following kinds of structural types

* `&T` - a transparent reference to T
* `[T; N]` - a fixed-length array of T (length N)
* `()` - the empty tuple (I just like adding this cuz it's cute!!!)
    * [non-empty tuples could be added](https://github.com/Gankra/abi-cafe/issues/48)


#### Reference Types

The "value" of a reference type `&T` is its pointee for the purposes of [abi-cafe][]. In this regard it's similar to C++ references or Rust references, where most operations automagically talk about the pointee and not the pointer. Using a reference type lets you test that something can properly be passed-by-reference, as opposed to passed-by-value.

Reference types may appear in other composite types, indicating that the caller is responsible for allocating variables for each one and then storing pointers to them in the composite type.

> Currently theoretical and probably will never be implemented: When used in the outputs of a function, a reference type is sugar for an out-param that the caller is responsible for allocating and the callee is responsible for initializing. Out-params should appear after all normal inputs but before varargs.





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





### Primitives

There are various builtin primitives, such as:

* integers (i8, u128, ...)
* floats (f16, f32, f64, f128, ...)
* `bool`- your old pal the boolean (TriBool support TBD)
* `ptr` - an opaque pointer (`void*`), used when you're interested in the value of the pointer and not its pointee (unlike `&T`)
* `()` - empty tuple

In the future there will probably be language-specific primitives like `c_long`.




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

<details>
<summary> not implemented distracting ramblings about outparams </summary>
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
>
> Update: actually even automating this was miserable, and also outparams aren't really substantial ABI-wise right now, so I'm not sure I'll ever implement outparams. It's more complexity than it's worth!

</details>

Currently there is no meaning ascribed to multiple outputs, every backend refuses to implement them. Note that "returning a tuple" or any other composite is still one output. We would need to like, support Go or something to make this a meaningful expression.

Named args [*could* be the equivalent of Swift named args](https://github.com/Gankra/abi-cafe/issues/32), where the inner and outer name can vary, but the outer name is like, part of the function name itself (and/or ABI)?

[Varargs support is also TBD but has a sketch](https://github.com/Gankra/abi-cafe/issues/1#issuecomment-2200345710).


## Attributes

Attributes start with `@` and apply to the next item (function or type) that follows them. There are currently 3 major classes of attributes:

* repr attrs
    * lang reprs
        * `@repr "rust"` - use rust's native struct layout
        * `@repr "c"` - use C-compatible struct layout
    * primitive reprs - for any enums, use the given primitive as its type
        * `@repr "u8"`
        * `@repr "f32"`
        * ...
    * transparent repr - equivalent of rust's `repr(transparent)`
        * `@repr "transparent"`
* modifier attrs
    * `@align 16` - align to N
    * `@packed` - pack fields to eliminate padding
* passthrough attrs -
    * `@ "literally anything here"

The significance of repr attributes is that providing *any* explicit `repr` attribute is considered an opt-out from the default automatic repr all user-defined types recieve.

When we generate tests we will typically generate both a `repr(rust)` version and a `repr(C)` version. In these versions any user-defined type gets (an equivalent of) those attributes applied to it.

This means that applying `@align 16` still leaves a struct eligible to have the rust layout and c layout tested, while applying `@repr "u8"` to a tagged union does not (if you want to test `repr(C, u8)`, you need to set `@repr "C" "u8"`).




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