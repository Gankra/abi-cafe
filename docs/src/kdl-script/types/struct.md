# KDLScript struct types

A KDLScript `struct` type is just what you expect! This definition:

```kdl
struct "Point" {
    x "f32"
    y "f32"
}
```

(or `struct "Point" { x "f32"; y "f32"; }`)

is equivalent to this Rust:

```rust
struct Point {
    x: f32,
    y: f32,
}
```

and this C:

```C
typedef struct Point {
    float x;
    float y;
} Point;
```



## Attributes And Layouts

[The various KDLScript attributes can be applied to structs to specify how they should be laid out](../attributes.md), like so:

```kdl
@repr "transparent"
struct "MetersU32" {
    _ "u32"
}
```

If no explicit `@repr` attribute is applied (the default, which is recommended), the struct will be [eligible for repr combinatorics](../../harness/combos/reprs.md). Basically, we'll generate a version of the test where it's set to `#[repr(C)]` and version where it's set to `#[repr(Rust)]`, improving your test coverage.

It's up to each [compiler / language](../../harness/combos/impls.md) to implement these attributes [however they see fit](../../harness/generate.md). But for instance we would expect Rust backends to support both layouts, and C backends to bail on the Rust repr, producing twice as many rust-calls-rust test cases.

Note that repr(transparent) *is not* currently eligible for repr combinatorics. If you want to test that, set it explicitly.




## Tuple Structs

As a convenience, you can omit the names of the fields by calling them `_`, and we'll make up names like `field0` and `field1` for you:

```kdl
struct "Point" {
    _ "f32"
    _ "f32"
}
```

[If all fields have the names omitted, then languages like Rust can emit a "tuple struct"](https://github.com/Gankra/abi-cafe/issues/25). So the above example can/should be emitted like this:

```rust ,ignore
struct Point(f64, f64);
```




## Generic Structs

[Generic structs are not supported.](https://github.com/Gankra/abi-cafe/issues/50)

