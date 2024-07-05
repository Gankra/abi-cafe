# calling conventions

A calling convention is as close as ABI Cafe ever gets to referring to "An ABI" directly, but they're still pretty abstract, since a single calling convention can mean different things on different platforms.

By default, for each test we will generate a copy of it for every known calling convention (changing the convention of all functions declared by that test).

Each [Toolchain](./toolchains.md) may claim to support a particular set of calling conventions
(and may use knowledge of the target platform to adjust their decisions). Refusing to support a convention will result in those tests getting marked as "skipped" and omitted from the final report.

If two [Toolchains](./toolchains.md) claim to support a calling convention on a platform, it is assumed that they want to have compatible ABIs, and it's our goal to identify what does and doesn't work.


## `--conventions`

All of the following conventions are enabled by default, and only these conventions are supported.

Universal Conventions:

* c: the platform's default C convention (`extern "C"`)
* rust: the platform's default Rust convention (`extern "Rust"`)

Windows Conventions:

* cdecl
* fastcall
* stdcall
* vectorcall

(There exists some code for other weird conventions rustc supports, but they aren't really wired up properly and it's not clear if they serve any purpose.)
