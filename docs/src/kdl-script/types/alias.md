# alias

A KDLScript `alias` type is just what you expect! [for superpowered ifdefy aliases, see pun types](./pun.md)

```kdl
alias "MetersU32" "u32"
```

is equivalent to this Rust:

```rust
type MetersU32 = u32;
```

and this C:

```C
typedef uint32_t MetersU32;
```

Note that the ordering matches Rust's `type Alias = RealType;` syntax and not C/C++'s backwards-ass typedef syntax (yes I know why the C syntax is like that, it's very cute).



## Attributes And Layouts

[The various KDLScript attributes can be applied to aliases](../attributes.md), but nothing currently respects them, because, what the fuck?



## Generic Aliases

[Generic aliases are not supported.](https://github.com/Gankra/abi-cafe/issues/50)



## I'm Normal And Can Be Trusted With Codegen

[The abi-cafe codegen backends](../../harness/generate.md) will go out of their way to "remember" that a type alias exists and use it when the alias was specified there. So for instance given this definition:

```kdl
enum "ComplexLongName" {
    A,
    B,
}

alias "Clean" "ComplexLongName"

struct "Enums" {
    x "Clean"
    y "ComplexLongName"
}
```

The Rust backend should initialize an instance of `Enums` as follows:

```rust
let temp = Enums { x: Clean::A, y: ComplexLongName::B }
```

Is this important?

No.

Am I happy the section is longer than the actual description of `alias`?

Yes.

I will fight every compiler that doesn't work like this. Preserve my fucking aliases in diagnostics and code refactors, cowards. Yes I *will* accept longer compile times to get this. Who wouldn't? People who are also cowards, that's who.
