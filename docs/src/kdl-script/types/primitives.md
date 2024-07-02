# KDLScript primitive types

There are various builtin primitives, such as:

* integers - fixed width integers
    * `i8`, `i16`, `i32`, `i64`, `i128`, `i256`
    * `u8`, `u16`, `u32`, `u64`, `u128`, `u256`
* floats - fixed with floating point numbers
    * `f16`, `f32`, `f64`, `f128`
* `bool`- your old pal the boolean
* `ptr` - an opaque pointer (`void*`), used when you're interested in the address as a value ([unlike `&T`](./refs.md))

The lowering of these to Rust is pretty direct, since we're reusing Rust's naming scheme.

The lowering of these to C uses `uint8_t` and friends for the integers, and then the usual types for the rest.

In the future there will probably be language-specific primitives like `c_long`...?


