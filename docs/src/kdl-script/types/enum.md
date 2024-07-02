# KDLScript enum types

A KDLScript `enum` type is a C-like enum with no nest fields. [For a Rust-like enum (tagged union), see tagged types](./tagged.md).

This definition:

```kdl
enum "IoError" {
    FileNotFound
    FileClosed
    FightMe
}
```

is equivalent to this Rust:

```rust
enum IoError {
    FileNotFound,
    FileClosed,
    FightMe,
}
```

and this C:

```C
typedef enum IoError {
    FileNotFound,
    FileClosed,
    FightMe,
} IoError;
```

(There are like 3 ways we could lower this concept to C, it's an eternal struggle/argument, I know.)



## Attributes And Layouts

[The various KDLScript attributes can be applied to enums to specify how they should be laid out](../attributes.md), like so:

```kdl
@repr "u32"
enum "MyEnum" {
    Case1
    Case2
}
```

If no explicit `@repr` attribute is applied (the default, which is recommended), the enum will be [eligible for repr combinatorics](../../harness/combos/reprs.md). Basically, we'll generate a version of the test where it's set to `#[repr(C)]` and version where it's set to `#[repr(Rust)]`, improving your test coverage.

It's up to each [compiler / language](../../harness/combos/impls.md) to implement these attributes [however they see fit](../../harness/generate.md). But for instance we would expect Rust backends to support both layouts, and C backends to bail on the Rust repr, producing twice as many rust-calls-rust test cases.

Note that `repr(u32)` and friends are *not* currently eligible for repr combinatorics. If you want to test that, set it explicitly.



## Explicit Tag Values

⚠️ [This feature exists in the KDLScript parser but isn't fully implemented yet.](https://github.com/Gankra/abi-cafe/issues/29)

You can give enum variants an integer value (currently limited to i64 range):

```kdl
enum "IoError" {
    FileNotFound -1
    FileClosed
    FightMe 4
}
```

It's up to each to each [compiler / language](../../harness/combos/impls.md) to implement these [however they see fit](../../harness/generate.md).


## Value Initialization And Analysis

When [initializing an instance of an enum](../../harness/combos/values.md), we will uniformly select a random variant to use (deterministically).

When [checking the value of an enum](../../harness/check.md), we will just check its bytes. In the future [we may instead check it semantically with a match/switch](https://github.com/Gankra/abi-cafe/issues/34).

