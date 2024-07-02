# KDLScript tagged types

A KDLScript `tagged` type is the equivalent of Rust's `enum`: a tagged union where variants have fields, which has no "obvious" C/C++ analog. Variant bodies may either be missing (indicating no payload) or have the syntax of a `struct` body. [For c-like enums, see enum types](./enum.md). [For c-like untagged unions, see union types](./union.md).

This definition:

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

Is equivalent to this Rust:

```rust
enum MyOptionU32 {
    None,
    Some(u32),
    FileNotFound {
        path: [u8; 100],
        error_code: i64,
    }
}
```

[We may one day implement the C(++) equivalents to this definition which are real and my friend](https://github.com/Gankra/abi-cafe/issues/28). They could theoretically detect when a tagged is equivalent to a c-like enum, but that kinda defeats the purpose of making them separate concepts for backend simplicity.



## Attributes And Layouts

[The various KDLScript attributes can be applied to tagged unions to specify how they should be laid out](../attributes.md), like so:

```kdl
@repr "u8"
tagged "MyOptionU32" {
    Some { _ "u32"; }
    None
}
```

If no explicit `@repr` attribute is applied (the default, which is recommended), the struct will be [eligible for repr combinatorics](../../harness/combos/reprs.md). Basically, we'll generate a version of the test where it's set to `#[repr(C)]` and version where it's set to `#[repr(Rust)]`, improving your test coverage.

It's up to each [compiler / language](../../harness/combos/impls.md) to implement these attributes [however they see fit](../../harness/generate.md). But for instance we would expect Rust backends to support both layouts, and C backends to bail on the Rust repr, producing twice as many rust-calls-rust test cases.

Note that `repr(u32)` and friends are *not* currently eligible for repr combinatorics. If you want to test that, set it explicitly.



## Tuple Variants

As a convenience (and as shown liberally above), you can omit the names of the fields by calling them `_`, and we'll make up names like `field0` and `field1` for you.

[If all fields of a variant have the names omitted, then languages like Rust can emit a "tuple variant"](https://github.com/Gankra/abi-cafe/issues/25).



## Explicit Tag Values

Tagged unions currently do not support explicit tag values, [unlike enums](./enum.md#explicit-tag-values).



## Generic Tagged Unions

[Generic tagged unions are not supported.](https://github.com/Gankra/abi-cafe/issues/50)


