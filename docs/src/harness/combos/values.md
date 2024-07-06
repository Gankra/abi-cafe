# value generators

When generating the source for a program to test, we need to pick values which will be passed to all the functions. These values are baked into the program, with both sides statically knowing which values they *should* have, and the test harness also having that information available for checking and diagnostics.

As later sections will explain, this system also allows us to do magic like "generate only the necessary branches of match statements", "generate random-depthed values of self-referential types", "derive debug on c-like untagged unions", and "compare the values of type puns with different fields".


## `--gen-vals`

By default we only set `--gen-vals=graffiti`, as this mode produces the most useful diagnostic information.

The possible values are

* graffiti: prefers patterning the bytes of values in a way that helps you identify which byte of which field each recorded value was.
* randomN (random1, random37, ...): seeds an RNG with N to make random (repeatable) values with



## graffiti values

The grafitti pattern stores two indices in each byte: the high nibble contains the index of the field that the byte belongs to (mod 16), and the low nibble contains the index of the byte (mod 16).

For instance graffiti values should look something like:

```rust ,ignore
let array_of_points = [
    Point { x: 0x0102_0304, y: 0x1112_1314 },
    Point { x: 0x2122_2324, y: 0x3132_3334 },
]
let bytes = [0x40, 0x50, 0x60, 0x70];

some_func(array_of_points, bytes);
```

The benefit of this system is that when there's an ABI mismatch if you see something like:

```text
mismatch in some_func val 2 (array_of_points[1].x: u32)
expect: [21, 22, 23, 24]
caller: [21, 22, 23, 24]
callee: [23, 24, 31, 32]
```

You can pretty clearly see that the callee got half its bytes from val 2, and half of its bytes from val 3, indicating some kind of alignment/padding disagreement.


## The Value Tree

The value tree is the solution that was created to address the various problems sketched out [in this article on ABI Cafe](https://faultlore.com/blah/abi-puns/#what-does-it-mean-for-compilers-to-agree).


### Value Tree Problem Statement

There are several problems we need ABI Cafe to solve regarding values:

* We need to be able to pick values for fields in a deterministic/repeatable way so we can reliably reproduce any issues found
* We would like the test harness to know what values the programs *should* have when checking them (at very least to produce better diagnostics)
* Given a type with "cases" like a tagged union, untagged union, or c-like enum, we need to be able to pick the case it should have, and deal with the fact that this changes the number/names of the fields
* If we want to report the values of an untagged union, we find ourselves needing to `derive(Debug)` on an untagged union, which is impossible (nothing in the value itself specifies its case)
* Self-referential types may have an arbitrary number of values (like a linked list)
* Type puns may introduce completely different shapes/paths for the same logical values (`(u32, u32)` vs `[u32; 2]`)

The value tree is able to handle all these cases!


### Value Tree Solution

Essentially the key insight here is that no matter how complicated a type is, an *instance* of that type has a tree structure (which syntax like `x.y[2].3` is a path on). A *traversal* of that tree then gives a linear list of (paths to) fields and types, for which we need to generate values.

(Note: intrusive values which contain pointers to themselves *aren't* treelike, but we simply don't support those because they aren't interesting to us. When we refer to self-referential types, we're talking about types which can contain *other instances* of the same type, and not literally the same instance.)

When traversing a struct, there isn't anything special to do: we can just recursively traverse each of its fields. When traversing a tagged or untagged union we apply a simple trick: we introduce an additional artificial leaf field representing the case (tag) of the union.

Since we're already assuming we have a way to deterministically generate pseudorandom values, this gives us a way to uniformly select one of the cases of the union for each instance of the type. This also naturally handles all valid self-referential types, because they inevitably need to contain something that looks like `Option<Self>`. Each time we encounter that Option, we are essentially flipping a coin as to whether we should add another layer of Self, or finish.

These artificial tag fields also give us a way to "derive(Debug) on an untagged union" -- because we're emitting the print statements totally inline for each instance, the code generator can consult the value tree for which case each union has, and only emit the code for accessing that case.

This ends up being nice even for tagged unions, because it lets us emit only a minimal `if let` instead of a full `match`. We even get an internal entry for "which case, semantically, did the program think the tagged union was in", which the program can use to report back that information back to to the harness (as a u32 value, reporting u32::MAX in the `else` branch of the if-let to indicate "not the one we expected").

Also, we don't actually care about any of the internal fields, we only care about the leaf (primitive) fields, which have actual logical bytes we can inspect and compare (or the cases of our unions). So, as long as two value tree traversals produce lists with the same lengths, we can compare them, allowing us to handle puns like `(u32, u32)` vs `[u32; 2]` or even more complex cases readily.


### Value Tree Compromises

The only kind of pun we really can't handle is one like `u64` vs `(u32, u32)`, because the counts desync. In the current implementation when traversing [a pun](../../kdl-script/types/pun.md), we assert that all blocks of the pun produce the same length list. This is a bit sad, but honestly, that was always going to be a semantic nightmare.

For various reasons it's tempting to want to "statically" number the leaf fields, such that `x.y[2].3` can be referred to by a stable index, regardless of the value generation strategy. This is theoretically possible even with unions, because you can traverse even the cases that aren't selected and still number them.

However the notion of a static numbering completely falls apart once you introduce self-referential types, as there is no static bound on the size of a self-referential subtree (and trying to artifically bound it is more work than it's worth). I also vaguely recall this completely breaking my brain to think about in the context of type puns, so, no big loss. We can just make value numbering value-generation-scoped, and get much the same benefit.

The same logic that allows for type puns to be handled would also allow [function puns](https://github.com/Gankra/abi-cafe/issues/47) to be handled (`fn(x: u32, y: u32)` vs `fn(point: (u32, u32))`). We currently don't allow for this, in an attempt to make diagnostics better. [In the future we might lift this restriction.](https://github.com/Gankra/abi-cafe/issues/53)
