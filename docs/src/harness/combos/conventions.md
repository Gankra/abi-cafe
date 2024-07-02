# calling conventions

Each language may claim to support a particular set of calling conventions
(and may use knowledge of the target platform to adjust their decisions).
We try to generate and test all supported conventions.

Universal Conventions:

* c: the platform's default C convention (`extern "C"`)
* rust: the platform's default Rust convention (`extern "Rust"`)

Windows Conventions:

* cdecl
* fastcall
* stdcall
* vectorcall