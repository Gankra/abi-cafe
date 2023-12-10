# abi-cafe Codegen

abi-cafe exists to test that two languages/compilers/backends agree on ABIs for the purposes of FFI The principle of the tool is as follows:

1. Take something morally equivalent to a C header (a bunch of types and function signatures)
2. Define codegen backends ("ABIs") that know how to take such a header and:
  * generate code for the "caller" (the user of the function)
  * generate code for the "callee" (the impl of the function)
  * compile the result
3. For each ABI pairing we're interested in, link and run the two halves together

At a lower level we:

* define a test harness with callbacks for "hi i'm the callee, i think arg1.0.3 = 7"
* have the codegen backends generate code that invokes those callbacks
* statically link the two halves together into a dylib, with the caller defining a "run the test" entrypoint
* have the harness dlopen and run the dylib
* have the harness check that both sides reported the exact same values

However when we discover an issue, we want to be able to explain it to humans, so there are some additional features:

* Codegen backends are expected to generate "graffiti" values, where each byte of a value signals roughly where it was supposed to come from. e.g. the 2nd byte of the 3rd value should be 0x32 (both indices are modulo 16).
* If a particular function fails (or is just requested in isolation), the codegen backend should be able to generate a cleaned up standalone version of the code for just that function for filing issues or investigating on godbolt -- only the function and types we care about are included, and all the callback gunk is replaced by prints or nothing at all.

Within abi-cafe we regard each header as a "test suite" and each function as a "test". Or if you prefer, headers are tests, functions are subtests (FIXME: double-check which set of terminology the code actually uses). Batching multiple functions into one "header" serves the following functions:

* type definitions can be shared, making things easier to write/maintain
* performance is significantly improved by replacing 100,000 linker calls with 1000 (there's a lot of procedural generation and combinatorics here)
* results are more organized (you can see that all your failures are "in the i128 tests")



# kdl-script: the header language for abi weirdos

See [kdl-script's docs for details](https://github.com/Gankra/kdl-script#kdl-script), but we'll give you a quick TLDR here too. Especially pay attention to [Pun Types](https://github.com/Gankra/kdl-script#pun-types) which are a totally novel concept that exists purely for the kind of thing abi-cafe is interested in doing. See [kdl_script::types for how we use kdl-script's compiler](https://docs.rs/kdl-script/latest/kdl_script/types/index.html)


## kdl-script tldr

Rather than relying on a specific language's format for defining our "headers", we want a language-agnostic(ish) solution that needs to hold two contradictory concepts in its head:

* The definitions should be vague enough that multiple languages can implement it
* The definitions should be specific enough that we can explore the breadth of the languages' ABIs

And so we made [kdl-script](https://github.com/Gankra/kdl-script), which is a silly toy language whose syntax happens to be a [kdl document](https://kdl.dev/) for literally no other reason than "it looks kind of like rust code and it's extremely funny".

The kdl-script language includes:

* a set of types, each with a unique type id:
  * primitives (i32, f64, bool, opaque pointer, etc.)
  * nominal types (structs, unions, tagged-unions, c-like enums)
  * structural types (fixed-length arrays)
  * alias types (aliases, [puns](https://github.com/Gankra/kdl-script#pun-types))
  * references (tells to pass by-ref)
* a set of function signatures using those types with
  * inputs
  * outputs (including outparams, which are just outputs that contain references)
  * calling conventions (c, fastcall, rust, etc.)

All of these can also be decorated with attributes for e.g. overaligning a struct or whatever. Pun types also let different languages define completely different layouts, to check that non-trivial cross-language FFI puns Work.


## using kdl-script

The kdl-script compiler will parse and type our program, and gives us an API that should make it relatively simple for a codegen backend to do its job. Per [kdl_script::types](https://docs.rs/kdl-script/latest/kdl_script/types/index.html), we ask it to parse the "header" into a `TypedProgram`, then each codegen backend lowers that to a `DefinitionGraph` (resolving [puns](https://github.com/Gankra/kdl-script#pun-types)).

We then pass `DefinitionGraph::definitions` a list of the functions we want to generate code for, and it produces an iterator of `Definitions` the codegen backend needs to generate in exactly that order (forward-declare a type, define a type, define a function).

Languages that don't need forward-declarations are technically free to ignore those messages, but in general a type will always be defined before anything that refers to it, and the forward-declarations exist to break circular dependencies. As such even the Rust backend benefits from those messages, as it can use it as a signal to intern a type's name before anything else uses that name.

Note also that you will receive messages to "define" types which otherwise wouldn't normally need to be defined like primitives or structural-types (arrays). This is because kdl-script is trying to not make any assumptions about what's built into the target language. Most backends will treat these messages as equivalent to forward-declares: just a signal for type name interning.

To allow for interning and circular definitions, kdl-script will always refer to types by `TyIdx` (type id). `TypedProgram::realize_ty` turns those type ids into a proper description of the type like "this is a struct named XYZ with fields A, B, C", which can then be used to generate the type definition. Because kdl-script handles sorting the type definitions you will never need to recursively descend into the fields to work out their types -- if you've been interning type names as you go you should be able to just resolve them by TyIdx.

Here are some quirks to keep in mind:


### kdl-script can ask for gibberish and that's ok

Different languages contain different concepts, and so kdl-script necessarily needs to be able to specify things that some codegen backends will have no reasonable implementation for. It's ok for the backend to return a `GenerateError::*Unsupported` in such a case. When comparing two languages this will not be treated as an error, and instead will be used to just disable that particular test.

This allows us to define a bunch of generic tests with little concern for which languages support what. When pairing up two ABIs we will just test the functionality that both languages agree they can implement.

FIXME(?): right now the granularity of this is per-header (suite) instead of per-function (test). It would be cool if granularity was per-function, but this would require two things:

* handling "i can't generate this type" errors by populating type interners with poison values that bubble up until they hit a function, so that we can mark the function as unimplementable (this sounds good and desirable for diagnostics anyway)
* rerunning the whole process again whenever two ABIs we want to pair up disagree on the functions they can implement, generating the intersection of the two (kinda a pain in the ass for our abstractions, which want to be able to generate each callee/caller independently and then blindly link the pairings up).


### type aliases break type equality for beauty

Many compilers attempt to "evaporate" a type alias as soon as possible for the sake of type ids defining strict type equality. Because we don't actually *care* about type equality except for the purposes of interning type names, kdl-script and abi-cafe treat type aliases as separate types. So `[u32; 5]` and `[MyU32Alias; 5]` will have different type ids, because we want to be able to generate code that actually contains either `[u32; 5]` or `[MyU32Alias; 5]`, depending on what the particular usage site actually asked for.

I 100% get why most compiler toolchains don't try to do this, but for our purposes it's easy for us to do and produces better output.

FIXME: we actually don't go *quite* as far as we could. This is valid Rust:

```rust
struct RealStruct { x: u32 }
type MyStruct = RealStruct;

let x = MyStruct { x: 0 };
```

This actually wouldn't be terribly hard to do, we could tweak "generate a value" to take an optional type alias, so that when a type alias recursively asks the real type to "generate a value" it can tweak its own name on the fly (since "generate a value" is not too interned).


### references are a mess

I'm so sorry. The "an output that contains a reference is sugar for an explicit outparam" shit was an absolute feverdream that really doesn't need to exist BUT here we are.


### annotations are half-baked

Stubbed out the concept, but it's all very "pass a string through" right now so nothing uses them yet.


### variants are half-baked

You can declare unions, tagged-unions, and c-like enums, but it's not obvious how abi-cafe should select which variant to use when generating values to pass across the ABI boundary.

Currently abi-cafe always uses "the first one". Presumably it should be allowed to select a "random" (but deterministic?) one. It's annoying to think about missing a bug because ABI-cafe always pick MyEnum::ThirdVariant for the third argument of a function.

It might also be reasonable to introduce a concept to kdl-script that `Option::Some` is a valid type name to use in a function signature, signaling that this is an Option, and that the Some variant should be used when generating values to pass (not super clear on how that would look codewise, but probably similar to how Pun Types both exist and don't exist, requiring an extra level of resolving to get the "real" type?).




### Coming Soon™

* varargs
    * sketch: have a "..." input arg signal all the subsequent args should be passed as varargs
    * did i hallucinate that swift supports multiple varargs lists? i think it makes sense with named args?
* simd types
    * sketch: as primitives? or treated like structural types like arrays? (`[u32 x 5]`?)
    * this is apparently an ABI minefield that would benefit from more attention
* tuples?
    * not exactly complex to do, just not clear what would use these
* slices?
    * very rust-specific...
* `_BitInt(N)`
    * I can't believe C is actually standardizing these what a time to be alive
* "the whole fucking Swift ABI"
    * lmfao sure thing buddy