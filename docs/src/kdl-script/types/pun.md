# KDLScript pun types

A pun is the equivalent of an ifdef/cfg'd type, allowing us to declare that two wildly different declarations in different languages should in fact have the same layout and/or ABI. A pun type contains "selector blocks" which are sequentially matched on much like CSS. The first one to match wins. When lowering to a specific backend/config if no selector matches, then compilation fails.

Here is an example that claims that a Rust `repr(transparent)` newtype of a `u32` should match the ABI of a `uint32_t` in C/C++:

```kdl
pun "MetersU32" {
    lang "rust" {
        @repr "transparent"
        struct "MetersU32" {
            _ "u32"
        }
    }

    lang "c" "cpp" {
        alias "MetersU32" "u32"
    }
}
```

Because of this design, the typechecker does not "desugar" `pun` types to their underlying type when computing type ids. This means `[MetersU32; 4]` will not be considered the same type as `[u32; 4]`... because it's not! This is fine because type equality is just an optimization for our transpiler usecase. Typeids mostly exist to deal with type name resolution.

Pun resolving is done as a second step when lowering the abstract `TypedProgram` to a more backend-concrete `DefinitionGraph`.

(`alias` also isn't desugarred and has the same "problem" but this is less "fundamental" and more "I want the backend to actually emit
a type alias and use the alias", just like the source KDLScript program says!)


The currently supported selector blocks are:

* `lang "lang1" "lang2" ...` - matches *any* of the languages
* `default` - always matches

Potentially Supported In The Future:

* `compiler "compiler1" "compiler2" ...`
* `cpu` ...
* `os` ...
* `triple` ...
* `any { selector1; selector2; }`
* `all { selector1; selector2; }`
* `not { selector; }`
