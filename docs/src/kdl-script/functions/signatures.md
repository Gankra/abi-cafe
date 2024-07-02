# KDLScript function signatures


Functions are where the Actually Useful *library* version of KDLScript and the Just A Meme *application* version of KDLScript diverge. This difference is configured by the `eval` feature.

In library form KDLScript only has function *signature declarations*, and it's the responsibility of the [abi-cafe][] backend using KDLScript to figure out what the body should be. In binary form you can actually fill in the body with some hot garbage I hacked up.

For now we'll only document declaration.

Here is a fairly complicated/contrived example function:

```kdl
fn "my_func" {
    inputs {
        x "u32"
        y "[&MyType; 3]"
        _ "&bool"
    }
    outputs {
        _ "bool"
        _ "&ErrorCode"
    }
}
```

Functions can have arbitrarily many inputs and outputs with either named or "positional" names (which will get autonaming like `arg0`, `arg1` and `out0`, `out1`, etc.).

<details>
<summary> not implemented distracting ramblings about outparams </summary>
As discussed in the section on "Reference Types", references in outputs are sugar for out-params, which should appear after the inputs and before outputs. So the above would lower to something like the following in Rust (values chosen arbitrarily here, and we wouldn't use asserts in practice, but instead record the values for comparison):

```rust ,ignore
fn my_func(
    x: u32,
    y: [&MyType; 3],
    arg2: &bool,
    out1: &mut ErrorCode,
) -> bool {
    // Check the inputs are what we expect...
    assert_eq!(x, 5);
    assert_eq!(y[0].val, 8);
    assert_eq!(y[1].val, 9);
    assert_eq!(y[2].val, 10);
    assert_eq!(*arg2, true);

    // Return outputs
    *out1 = ErrorCode::Bad;
    return true;
}


fn my_func_caller() {
    // Setup the inputs
    let x = 5;
    let y_0 = MyType { val: 8 };
    let y_1 = MyType { val: 9 };
    let y_2 = MyType { val: 10 };
    let y = [&y_0, &y_1, &y_1];
    let arg2 = false;

    // Setup outparams
    let mut out1 = ErrorCode::default();

    // Do the call
    let out0 = my_func(x, y, &arg2, &mut out1);

    // Checkout outputs
    assert_eq!(out0, true);
    assert_eq!(*out1, ErrorCode::Bad);
}
```

> God writing that sucked ass, and it wasn't even the "proper" value checking! This is why I built all this complicated crap to automate it!
>
> Update: actually even automating this was miserable, and also outparams aren't really substantial ABI-wise right now, so I'm not sure I'll ever implement outparams. It's more complexity than it's worth!

</details>

Currently there is no meaning ascribed to multiple outputs, every backend refuses to implement them. Note that "returning a tuple" or any other composite is still one output. We would need to like, support Go or something to make this a meaningful expression.

Named args [*could* be the equivalent of Swift named args](https://github.com/Gankra/abi-cafe/issues/32), where the inner and outer name can vary, but the outer name is like, part of the function name itself (and/or ABI)?

[Varargs support is also TBD but has a sketch](https://github.com/Gankra/abi-cafe/issues/1#issuecomment-2200345710).
