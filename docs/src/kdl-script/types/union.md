# KDLScript union types

A KDLScript union type is a C-like untagged union. [For rust-like tagged unions, see tagged types](./tagged.md).

This definition:

```kdl
union "FloatOrInt" {
    a "f32"
    b "u32"
}
```

is equivalent to this Rust:

```rust
union FloatOrInt {
    a: f32,
    b: u32,
}
```

and this C:

```C
typedef union FloatOrInt {
    float a;
    int32_t b;
} FloatOrInt;
```

