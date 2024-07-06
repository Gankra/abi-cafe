# types

The following kinds of types exist in KDLScript.

* nominal types
    * [`struct` - a plain ol' struct](./struct.md)
    * [`union` - an untagged union](./union.md)
    * [`enum` - a c-style enum](./enum.md)
    * [`tagged` - a tagged union (rust-style enum)](./tagged.md)
    * [`alias` - a transparent type alias](./alias.md)
    * [`pun` - a pun across the FFI boundary, "CSS for ifdefs"](./pun.md)
* structural types
    * [`[T; N]` - an array of T, length N](./arrays.md)
    * [`&T` - a reference to T (the pointee is regarded as the value)](./refs.md)
    * [`(T, U, V)` - a tuple](./tuples.md)
* [builtin primitives](./primitives.md)
    * integers (`i8`, `u128`, ...)
    * floats (`f16`, `f32`, `f64`, `f128`, ...)
    * `bool`- your old pal the boolean (TriBool support TBD)
    * `ptr` - an opaque pointer (`void*`), used when you're interested in the value of the pointer and not its pointee (unlike `&T`)

All of these types can be combined together as you expect, and [self-referential types do in fact work](https://github.com/Gankra/abi-cafe/blob/main/include/tests/procgen/fancy/IntrusiveList.procgen.kdl)!

[We do not currently support generics.](https://github.com/Gankra/abi-cafe/issues/50)
