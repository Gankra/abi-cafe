# KDLScript reference types

The "value" of a reference type `&T` is its pointee for the purposes of [abi-cafe](../../intro.md). In this regard it's similar to C++ references or Rust references, where most operations automagically talk about the pointee and not the pointer. Using a reference type lets you test that something can properly be passed-by-reference, as opposed to passed-by-value.

Reference types may appear in other composite types, indicating that the caller is responsible for allocating variables for each one and then storing pointers to them in the composite type.

> Currently theoretical and probably will never be implemented: When used in the outputs of a function, a reference type is sugar for an out-param that the caller is responsible for allocating and the callee is responsible for initializing. Out-params should appear after all normal inputs but before varargs.


