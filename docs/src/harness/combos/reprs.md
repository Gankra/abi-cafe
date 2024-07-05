# lang reprs

Lang reprs abstractly describe an interop target for the layout of [structs and enums and the like](../../kdl-script/types/index.md). These currently exactly match the ["lang reprs" in KDLScript](../../kdl-script/attributes.md).

For each test we will generate a copy of it for every enabled lang repr (changing the definitions of all types which [don't specify an explicit repr](../../kdl-script/attributes.md)).


## `--reprs`

All of the following reprs are enabled by default, and only these reprs are supported.

* c: layout structs in a C-compatible way (`repr(C)`)
* rust: layout structs in a Rust-compatible way (`repr(Rust)`)
