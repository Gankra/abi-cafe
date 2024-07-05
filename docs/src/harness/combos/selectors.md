# value selection

When generating the source for a program to test, we want a way to identify which of the [values of the function arguments](./values.md) we actually care about [writing somewhere](./writers.md). This is specifically relevant when generating a minimized test case.

Let's say we generate a program with a dozen functions, and specifically we find a mismatch in `function3` on `arg2.field2[7].y`, and want to now generate a minimized program that focuses on only that one value. What is safe to throw out?

It's hopefully safe to throw out all the other functions, because that kind of spooky action at a distance isn't something we're concerned with. (There *is* a concern that removing random garbage on the stack from previous function calls could change the values captured if something is reading ~padding bytes. We try to avoid repetitive values to minimize the chance of this mattering.)

Once we throw away functions, we can also throw away any type definitions that aren't used by the remaining function. (God help your poor compiler if this substantially changes the result.)

Now we have function arguments we don't actually care about *but* removing those is expected to change the ABI of the function, and will likely make the mismatch disappear. Changing the [values of the arguments](./values.md) is also something we want to avoid here, as this may change which variants unions take on and has a chance of introducing unfortunate new coincidental value matches.

What we *can* hopefully do is remove all [the code that *writes* the other arguments/values to output](./writers.md). So in an ideal world the final result is something like this:

```rust ,ignore
struct SomeComplicatedType {
    /* omitted for brevity of example */
}

fn function3(arg0: f32, arg1: u32, arg2: SomeComplicatedType, arg3: bool) {
    println!("{:?}", arg2.field2[7].y);
}
```

(This is the callee, the caller ends up being uglier because it needs to still initialize all those values and pass them in.)



## `--select-vals`

This CLI flag is reserved but not yet implemented. Its semantics are however used internally when regenerating a failed test, as described above.

When filtering a test you currently get 3 levels of granularity:

* functions: all or one
* arguments: all or one
* values (fields): all or one

The default is for all levels to be set to "all", because we want to check everything.

When abi-cafe detects an error, it will regenerate the test with all levels set to "one", so that it can highlight only the one field that matters.
