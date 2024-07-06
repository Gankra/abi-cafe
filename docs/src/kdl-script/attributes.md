# attributes

KDLScript Attributes start with `@` and apply to the next item (function or type) that follows them. There are currently 3 major classes of attributes:

* repr attrs
    * lang reprs
        * `@repr "rust"` - use rust's native struct layout
        * `@repr "c"` - use C-compatible struct layout
    * primitive reprs - for any enums, use the given primitive as its type
        * `@repr "u8"`
        * `@repr "f32"`
        * ...
    * transparent repr - equivalent of rust's `repr(transparent)`
        * `@repr "transparent"`
* modifier attrs
    * `@align 16` - align to N
    * `@packed` - pack fields to eliminate padding
* passthrough attrs
    * `@ "literally anything here"`

The significance of repr attributes is that providing *any* explicit `repr` attribute is considered an opt-out from the default automatic repr all user-defined types receive.

When we generate tests we will typically generate both a `repr(rust)` version and a `repr(C)` version. In these versions any user-defined type gets (an equivalent of) those attributes applied to it.

This means that applying `@align 16` still leaves a struct eligible to have the rust layout and c layout tested, while applying `@repr "u8"` to a tagged union does not (if you want to test `repr(C, u8)`, you need to set `@repr "C" "u8"`).
