# array types

KDLScript array types like `[u32; 4]` have the layout/repr you would expect from languages like C and Rust.

But there's a problem with passing them by-value: C is supremely fucking weird about passing arrays by value if they're not wrapped in a struct.

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

