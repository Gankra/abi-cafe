# value writers

When generating the source for a program to test, we want the program to write the [values of the function arguments](./values.md) somewhere for validation: callbacks, prints, asserts, etc.


## `--write-vals`

This isn't a setting you typically want to mess with in normal usage, since the default ("harness") is the only one that is machine-checkable. All the others are intended for minimizing/exporting the test for human inspection (see `--minimize-vals` below).

The supported writers are:

* harness: send values to the abi-cafe harness with callbacks
* print: print the values to stdout
* assert: assert the values have their expected value
* noop: disable all writes (see also the less blunt [value selectors](./selectors.md))



## `--minimize-vals`

This takes the same values as write-vals, but is specifically the writer used when a test has failed and we want to regenerate the test with a minimized human readable output.

The default is "print".
