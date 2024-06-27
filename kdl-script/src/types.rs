//! The type checker and types!
//!
//! The entry point is [`typeck`][] which is implicitly
//! handled by [`Compiler::compile_path`][] or [`Compiler::compile_string`][]
//! and will produce a [`TypedProgram`][].
//!
//! You should then call [`TypedProgram::definition_graph`][] with your
//! target backend's [`PunEnv`][] to resolve all the [`PunTy`]s and get a
//! final [`DefinitionGraph`][].
//!
//! You should then call [`DefinitionGraph::definitions`][] with the set
//! of functions you want to emit (usually [`TypedProgram::all_funcs`][])
//! to get the final forward-decls and definitions your target should emit
//! to generate its program.
//!
//! If a test (function) fails, you can pass just that function to
//! [`DefinitionGraph::definitions`][] to get a minimized program for just
//! that one function.
//!
//! The type system is phased like this to allow work to be reused and shared
//! where possible. Each of the above "lowerings" represents increased levels
//! of specificity:
//!
//! * [`TypedProgram`][] is abstract over all possible backends and can be computed once.
//! * [`DefinitionGraph`][] is for a concrete backend but still abstract over what parts
//!   of the program you might care about emitting. Computed once per backend config ([`PunEnv`]).
//! * [`DefinitionGraph::definitions`][] is the final concrete program we want to emit.
//!
//! In principle a backend emitting various configs for a single [`TypedProgram`][] can
//! share everything for a specific [`TyIdx`][] or [`FuncIdx`][], except they need to be
//! careful about [`PunTy`][]s which can have [`DefinitionGraph`][]-specific lowerings...
//! so really you should only recycle state created for a specific [`DefinitionGraph`]!
//!
//! FIXME: unlike [`AliasTy`][]s, [`PunTy`][]s really *should* completely evaporate in the
//! backend's lowering. Perhaps we should do something in [`TypedProgram`][] to actually
//! make them transparent?
//!
//! While performance isn't a huge concern for this project, combinatorics do get
//! kind of out of control so work sharing is kinda important, especially as the backends
//! get more complex! Also it's just nice to handle backend-agnostic issues once to keep
//! things simple and correct.

use std::collections::HashMap;
use std::sync::Arc;

use miette::{Diagnostic, NamedSource, SourceSpan};
use petgraph::graph::DiGraph;
use petgraph::graph::NodeIndex;
use thiserror::Error;

use crate::parse::*;
use crate::spanned::*;
use crate::Compiler;
use crate::Result;

/// An error that occured while processing the types of a program.
#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
pub struct KdlScriptTypeError {
    pub message: String,
    #[source_code]
    pub src: Arc<NamedSource>,
    #[label]
    pub span: SourceSpan,
    #[help]
    pub help: Option<String>,
}

/// A program that has had its symbolic types resolved to actual type ids.
///
/// Aliases and Puns are not fully resolved at this point.
///
/// Aliases still exist so that you can emit the target language's form of
/// an alias if you want to most accurately express the input program.
///
/// Puns still exist because a TypedProgram is abstract over every possible
/// output language to share the workload between each concrete backend.
/// The next step in lowering the program is to ask it to resolve
/// the puns for a specific [`crate::PunEnv`][] with [`TypedProgram::definition_graph`].
/// Which will also handle computing the order of declarations for languages like C.
#[derive(Debug)]
pub struct TypedProgram {
    tcx: TyCtx,
    funcs: Vec<Func>,
    builtin_funcs_start: usize,
}

/// A type id
pub type TyIdx = usize;
/// A function id
pub type FuncIdx = usize;

/// The actual structure of a type
///
/// This may be either a nominal, structural, or primitive type.
///
/// Any types that this type references will already have been normalized to a [`TyIdx`][]
/// so you don't have to worry about name resolution or interning/memoizing. Notably
/// all uses of `[u32; 5]` will have the same [`TyIdx`][], although `[MyU32Alias; 5]` will
/// be get a separate type id to allow a backend to more accurately reproduce the input program.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    /// A primitive (int, float, bool, ptr)
    Primitive(PrimitiveTy),
    /// A nominal struct
    Struct(StructTy),
    /// A nominal untagged union
    Union(UnionTy),
    /// A nominal C-style enum (see `Tagged` for a full rust-style enum)
    Enum(EnumTy),
    /// A nominal tagged union (rust-style enum, see `Enum` for a c-style enum)
    Tagged(TaggedTy),
    /// A transparent type alias (like typed)
    Alias(AliasTy),
    /// A type pun that can have different underlying types for different targets
    Pun(PunTy),
    /// A fixed-length array
    Array(ArrayTy),
    /// A reference to a type (behaves as if is the Pointee, but just passed by-ref)
    Ref(RefTy),
    /// Empty tuple -- `()`
    Empty,
}

/// A function
#[derive(Debug, Clone)]
pub struct Func {
    /// The function's name
    pub name: Ident,
    /// The function's inputs
    pub inputs: Vec<Arg>,
    /// The function's outputs (note that outparams will appear as Ty::Ref outputs!)
    pub outputs: Vec<Arg>,
    /// Any attributes hanging off the function
    pub attrs: Vec<Attr>,
    #[cfg(feature = "eval")]
    /// The body of the function (TBD, not needed for abi-cafe)
    pub body: (),
}

/// A function argument (input or output).
#[derive(Debug, Clone)]
pub struct Arg {
    /// The name of the argument
    pub name: Ident,
    /// The type of the arg
    pub ty: TyIdx,
}

/// A primitive
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrimitiveTy {
    /// `i8` / `int8_t`
    I8,
    /// `i16` / `int16_t`
    I16,
    /// `i32` / `int32_t`
    I32,
    /// `i64` / `int64_t`
    I64,
    /// `i128` / `int128_t`
    I128,
    /// `i256` / `int256_t`
    I256,
    /// `u8` / `uint8_t`
    U8,
    /// `u16` / `uint16_t`
    U16,
    /// `u32` / `uint32_t`
    U32,
    /// `u64` / `uint64_t`
    U64,
    /// `u128` / `uint128_t`
    U128,
    /// `u256` / `uint256_t`
    U256,
    /// `f16` / `half`
    F16,
    /// `f32` / `float`
    F32,
    /// `f64` / `double`
    F64,
    /// `f128` / `quad`
    F128,
    /// `bool`
    Bool,
    /// An opaque pointer (like `void*`)
    Ptr,
}

/// The Ty of a nominal struct.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructTy {
    pub name: Ident,
    pub fields: Vec<FieldTy>,
    pub attrs: Vec<Attr>,
    /// True if all fields had was_blank set, indicating this could be emitted as a tuple-struct
    pub all_fields_were_blank: bool,
}

/// The Ty of an untagged union.
///
/// See [`TaggedTy`][] for a tagged union.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnionTy {
    pub name: Ident,
    pub fields: Vec<FieldTy>,
    pub attrs: Vec<Attr>,
}

/// The Ty of an Enum.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumTy {
    pub name: Ident,
    pub variants: Vec<EnumVariantTy>,
    pub attrs: Vec<Attr>,
}

/// An enum variant
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumVariantTy {
    pub name: Ident,
    // pub val: LiteralExpr,
}

/// The Ty of a tagged union (rust-style enum).
///
/// See [`UnionTy`][] for an untagged union.
///
/// See [`EnumTy`][] for a c-style enum.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaggedTy {
    pub name: Ident,
    pub variants: Vec<TaggedVariantTy>,
    pub attrs: Vec<Attr>,
}

/// A variant for a tagged union.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaggedVariantTy {
    pub name: Ident,
    pub fields: Option<Vec<FieldTy>>,
    /// True if all fields have was_blank set, indicating this could be emitted as a tuple-variant
    pub all_fields_were_blank: bool,
}

/// The Ty of a transparent type alias.
///
/// i.e. `type name = real` in rust
///
/// i.e. `typedef real name` in C++
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AliasTy {
    pub name: Ident,
    pub real: TyIdx,
    pub attrs: Vec<Attr>,
}

/// A field of a [`StructTy`][] or [`UnionTy`][].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldTy {
    pub idx: usize,
    pub ident: Ident,
    pub ty: TyIdx,
}

/// The Ty of a fixed length array.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayTy {
    pub elem_ty: TyIdx,
    pub len: u64,
}

/// The Ty of a reference (transparent pointer).
///
/// This is used to represent passing a value by-reference, and so backends
/// should consider the "value" to be the pointee. If you want to test that
/// a pointer doesn't have its value corrupted but don't care about the pointee,
/// use `PrimitiveTy::Ptr`.
///
/// When used in the `outputs` of a [`Func`], this expresses an out-param
/// that the caller is responsible for "allocating" (and initializing?) and
/// the callee is responsible for "writing" the value to it. The caller then
/// checks the value just like other outputs.
///
/// Out-params should appear after "normal" inputs but before vararg inputs,
/// with the name specified.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefTy {
    pub pointee_ty: TyIdx,
}

/// The Ty of a Pun.
///
/// Puns express the fact that different languages might express a type
/// in completely different ways but we expect the layout and/or ABI to
/// match.
///
/// e.g. `Option<&T>` in Rust is equivalent to `T*` in C!
///
/// Resolve this with [`TypedProgram::resolve_pun`][].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PunTy {
    pub name: Ident,
    pub blocks: Vec<PunBlockTy>,
    pub attrs: Vec<Attr>,
}

/// A block for a [`PunTy`][]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PunBlockTy {
    pub selector: PunSelector,
    pub real: TyIdx,
}

/// Information on all the types.
///
/// The key function of TyCtx is to `memoize` all parsed types (TyName) into
/// type ids (TyIdx), to enable correct type comparison. Two types are equal
/// *if and only if* they have the same TyIdx.
///
/// This is necessary because *nominal* types (TyName::Named, i.e. structs) can
/// be messy due to shenanigans like captures/scoping/shadowing/inference. Types
/// may refer to names that are out of scope, and two names that are equal
/// (as strings) may not actually refer to the same type declaration.
///
/// To handle this, whenever a new named type is declared ([TyCtx::push_nominal_decl_incomplete][]),
/// we generate a unique type id ([`TyIdx`][]) for it. Then whenever we encounter
/// a reference to a Named type, we lookup the currently in scope TyIdx for that
/// name, and use that instead. Named type scoping is managed by `envs`.
///
/// Replacing type names with type ids requires a change of representation,
/// which is why we have [`Ty`][]. A Ty is the *structure* of a type with all types
/// it refers to resolved to TyIdx's (e.g. a field of a tuple, the return type of a function).
/// For convenience, non-typing metadata may also be stored in a Ty.
///
/// So a necessary intermediate step of converting an Ident to a TyIdx is to first
/// convert it to a Ty. This intermediate value is stored in `tys`.
/// If you have a TyIdx, you can get its Ty with [`realize_ty`][]. This lets you
/// e.g. check if a value being called is actually a Func, and if it is,
/// what the type ids of its arguments/return types are.
///
/// `ty_map` stores all the *structural* Tys we've seen before (everything that
/// *isn't* TyName::Named), ensuring two structural types have the same TyIdx.
/// i.e. `[u32; 4]` will have the same TyIdx everywhere it occurs.
#[derive(Debug)]
pub(crate) struct TyCtx {
    /// The source code this is from, for resolving spans/errors.
    src: Arc<NamedSource>,

    /// The list of every known type.
    ///
    /// These are the "canonical" copies of each type. Types are
    /// registered here via `memoize`, which returns a TyIdx into
    /// this array.
    ///
    /// Types should be compared by checking if they have the same
    /// TyIdx. This allows you to properly compare nominal types
    /// in the face of shadowing and similar situations.
    tys: Vec<Ty>,

    /// Mappings from structural types we've seen to type indices.
    ///
    /// This is used to get the canonical TyIdx of a structural type
    /// (including builtin primitives).
    ///
    /// Nominal types (structs) are stored in `envs`, because they
    /// go in and out of scope.
    ty_map: HashMap<Ty, TyIdx>,

    /// Scoped type info, reflecting the fact that struct definitions
    /// and variables come in and out of scope.
    ///
    /// These values are "cumulative", so type names and variables
    /// should be looked up by searching backwards in this array.
    ///
    /// If nothing is found, that type name / variable name is undefined
    /// at this point in the program.
    envs: Vec<CheckEnv>,
}

/// Information about types for a specific scope.
#[derive(Debug)]
struct CheckEnv {
    /// The struct definitions and TyIdx's
    tys: HashMap<Ident, TyIdx>,
}

/// Take a ParsedProgram and produce a TypedProgram for it!
pub fn typeck(comp: &mut Compiler, parsed: &ParsedProgram) -> Result<TypedProgram> {
    let mut tcx = TyCtx {
        src: comp.source.clone().unwrap(),
        tys: vec![],
        ty_map: HashMap::new(),
        envs: vec![],
    };

    // Add global builtins
    tcx.envs.push(CheckEnv {
        tys: HashMap::new(),
    });
    tcx.add_builtins();

    // Put user-defined types in a separate scope just to be safe
    tcx.envs.push(CheckEnv {
        tys: HashMap::new(),
    });

    // Add all the user defined types
    for (ty_name, _ty_decl) in &parsed.tys {
        let _ty_idx = tcx.push_nominal_decl_incomplete(ty_name.clone());
    }
    for (ty_name, ty_decl) in &parsed.tys {
        tcx.complete_nominal_decl(ty_name, ty_decl)?;
    }

    let funcs = parsed
        .funcs
        .iter()
        .map(|(_func_name, func_decl)| -> Result<Func> {
            let inputs = func_decl
                .inputs
                .iter()
                .enumerate()
                .map(|(idx, var)| -> Result<Arg> {
                    let name = ident_var(var.name.clone(), "arg", idx, &var.ty);
                    let ty = tcx.memoize_ty(&var.ty)?;
                    Ok(Arg { name, ty })
                })
                .collect::<Result<Vec<_>>>()?;
            let outputs = func_decl
                .outputs
                .iter()
                .enumerate()
                .map(|(idx, var)| {
                    let name = ident_var(var.name.clone(), "out", idx, &var.ty);
                    let ty = tcx.memoize_ty(&var.ty)?;
                    Ok(Arg { name, ty })
                })
                .collect::<Result<Vec<_>>>()?;

            let name = func_decl.name.clone();
            let attrs = func_decl.attrs.clone();
            Ok(Func {
                name,
                inputs,
                outputs,
                attrs,
                body: (),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let builtin_funcs_start = parsed.builtin_funcs_start;
    Ok(TypedProgram {
        tcx,
        funcs,
        builtin_funcs_start,
    })
}

impl TyCtx {
    /// Add the builtin types to the TyCtx
    fn add_builtins(&mut self) {
        let builtins = [
            ("i8", Ty::Primitive(PrimitiveTy::I8)),
            ("i16", Ty::Primitive(PrimitiveTy::I16)),
            ("i32", Ty::Primitive(PrimitiveTy::I32)),
            ("i64", Ty::Primitive(PrimitiveTy::I64)),
            ("i128", Ty::Primitive(PrimitiveTy::I128)),
            ("i256", Ty::Primitive(PrimitiveTy::I256)),
            ("u8", Ty::Primitive(PrimitiveTy::U8)),
            ("u16", Ty::Primitive(PrimitiveTy::U16)),
            ("u32", Ty::Primitive(PrimitiveTy::U32)),
            ("u64", Ty::Primitive(PrimitiveTy::U64)),
            ("u128", Ty::Primitive(PrimitiveTy::U128)),
            ("u256", Ty::Primitive(PrimitiveTy::U256)),
            ("f16", Ty::Primitive(PrimitiveTy::F16)),
            ("f32", Ty::Primitive(PrimitiveTy::F32)),
            ("f64", Ty::Primitive(PrimitiveTy::F64)),
            ("f128", Ty::Primitive(PrimitiveTy::F128)),
            ("bool", Ty::Primitive(PrimitiveTy::Bool)),
            ("ptr", Ty::Primitive(PrimitiveTy::Ptr)),
            ("()", Ty::Empty),
        ];

        for (ty_name, ty) in builtins {
            let ty_idx = self.tys.len();
            self.tys.push(ty);
            self.envs
                .last_mut()
                .unwrap()
                .tys
                .insert(Ident::from(ty_name.to_owned()), ty_idx);
        }
    }

    /// Register a new nominal struct in this scope.
    ///
    /// This creates a valid TyIdx for the type, but the actual Ty
    /// while be garbage (Ty::Empty arbitrarily) and needs to be
    /// filled in properly with [`TyCtx::complete_nominal_decl`][].
    ///
    /// This two-phase system is necessary to allow nominal types to
    /// be unordered or self-referential.
    fn push_nominal_decl_incomplete(&mut self, ty_name: Ident) -> TyIdx {
        let ty_idx = self.tys.len();
        let dummy_ty = Ty::Empty;
        self.tys.push(dummy_ty);
        self.envs.last_mut().unwrap().tys.insert(ty_name, ty_idx);
        ty_idx
    }

    /// Complete a nominal decl created with [`TyCtx::push_nominal_decl_incomplete`][].
    fn complete_nominal_decl(&mut self, ty_name: &Ident, ty_decl: &TyDecl) -> Result<()> {
        // This failing is an ICE and not a user issue!
        let ty_idx = self
            .resolve_nominal_ty(ty_name)
            .expect("completing a nominal ty that hasn't been decl'd");
        let ty = self.memoize_nominal_parts(ty_decl)?;
        self.tys[ty_idx] = ty;
        Ok(())
    }

    /// Memoize the parts of a nominal ty.
    fn memoize_nominal_parts(&mut self, ty_decl: &TyDecl) -> Result<Ty> {
        let ty = match ty_decl {
            TyDecl::Struct(decl) => {
                let fields = decl
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(idx, f)| {
                        Ok(FieldTy {
                            idx,
                            ident: ident_var(f.name.clone(), "field", idx, &f.ty),
                            ty: self.memoize_ty(&f.ty)?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let all_fields_were_blank = fields.iter().all(|f| f.ident.was_blank);
                Ty::Struct(StructTy {
                    name: decl.name.clone(),
                    fields,
                    attrs: decl.attrs.clone(),
                    all_fields_were_blank,
                })
            }
            TyDecl::Union(decl) => {
                let fields = decl
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(idx, f)| {
                        Ok(FieldTy {
                            idx,
                            ident: ident_var(f.name.clone(), "field", idx, &f.ty),
                            ty: self.memoize_ty(&f.ty)?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ty::Union(UnionTy {
                    name: decl.name.clone(),
                    fields,
                    attrs: decl.attrs.clone(),
                })
            }
            TyDecl::Enum(decl) => {
                let variants = decl
                    .variants
                    .iter()
                    .map(|v| EnumVariantTy {
                        name: v.name.clone(),
                    })
                    .collect::<Vec<_>>();
                Ty::Enum(EnumTy {
                    name: decl.name.clone(),
                    variants,
                    attrs: decl.attrs.clone(),
                })
            }
            TyDecl::Tagged(decl) => {
                let variants = decl
                    .variants
                    .iter()
                    .map(|v| {
                        let fields = if let Some(fields) = &v.fields {
                            Some(
                                fields
                                    .iter()
                                    .enumerate()
                                    .map(|(idx, f)| {
                                        Ok(FieldTy {
                                            idx,
                                            ident: ident_var(f.name.clone(), "field", idx, &f.ty),
                                            ty: self.memoize_ty(&f.ty)?,
                                        })
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            )
                        } else {
                            None
                        };
                        let all_fields_were_blank = fields
                            .as_deref()
                            .unwrap_or_default()
                            .iter()
                            .all(|f| f.ident.was_blank);
                        Ok(TaggedVariantTy {
                            name: v.name.clone(),
                            fields,
                            all_fields_were_blank,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ty::Tagged(TaggedTy {
                    name: decl.name.clone(),
                    variants,
                    attrs: decl.attrs.clone(),
                })
            }
            TyDecl::Alias(decl) => {
                let real_ty = self.memoize_ty(&decl.alias)?;
                Ty::Alias(AliasTy {
                    name: decl.name.clone(),
                    real: real_ty,
                    attrs: decl.attrs.clone(),
                })
            }
            TyDecl::Pun(decl) => {
                let blocks = decl
                    .blocks
                    .iter()
                    .map(|block| {
                        // !!! If this ever becomes fallible we'll want a proper stack guard to pop!
                        self.envs.push(CheckEnv {
                            tys: HashMap::new(),
                        });
                        let real_decl = &block.decl;
                        let real = self.push_nominal_decl_incomplete(decl.name.clone());
                        self.complete_nominal_decl(&decl.name, real_decl)?;
                        self.envs.pop();

                        Ok(PunBlockTy {
                            selector: block.selector.clone(),
                            real,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;

                Ty::Pun(PunTy {
                    name: decl.name.clone(),
                    blocks,
                    attrs: decl.attrs.clone(),
                })
            }
        };
        Ok(ty)
    }

    /// Resolve the type id (TyIdx) associated with a nominal type (struct name),
    /// at this point in the program.
    fn resolve_nominal_ty(&mut self, ty_name: &str) -> Option<TyIdx> {
        for env in self.envs.iter_mut().rev() {
            if let Some(ty) = env.tys.get(ty_name) {
                return Some(*ty);
            }
        }
        None
    }

    /// Converts a TyName (parsed type) into a TyIdx (type id).
    ///
    /// All TyNames in the program must be memoized, as this is the only reliable
    /// way to do type comparisons. See the top level docs of TyIdx for details.
    fn memoize_ty(&mut self, ty_ref: &Spanned<Tydent>) -> Result<TyIdx> {
        let ty_idx = match &**ty_ref {
            Tydent::Empty => self.memoize_inner(Ty::Empty),
            Tydent::Ref(pointee_ty_ref) => {
                let pointee_ty = self.memoize_ty(pointee_ty_ref)?;
                self.memoize_inner(Ty::Ref(RefTy { pointee_ty }))
            }
            Tydent::Array(elem_ty_ref, len) => {
                let elem_ty = self.memoize_ty(elem_ty_ref)?;
                self.memoize_inner(Ty::Array(ArrayTy { elem_ty, len: *len }))
            }
            Tydent::Name(name) => {
                // Nominal types take a separate path because they're scoped
                if let Some(ty_idx) = self.resolve_nominal_ty(name) {
                    ty_idx
                } else {
                    return Err(KdlScriptTypeError {
                        message: "use of undefined type name".to_string(),
                        src: self.src.clone(),
                        span: Spanned::span(name),
                        help: None,
                    })?;
                }
            }
        };

        Ok(ty_idx)
    }

    /// Converts a Ty (structural type with all subtypes resolved) into a TyIdx (type id).
    fn memoize_inner(&mut self, ty: Ty) -> TyIdx {
        if let Some(idx) = self.ty_map.get(&ty) {
            *idx
        } else {
            let ty1 = ty.clone();
            let ty2 = ty;
            let idx = self.tys.len();

            self.ty_map.insert(ty1, idx);
            self.tys.push(ty2);
            idx
        }
    }

    /// Get the type-structure (Ty) associated with this type id (TyIdx).
    pub fn realize_ty(&self, ty: TyIdx) -> &Ty {
        self.tys
            .get(ty)
            .expect("Internal Compiler Error: invalid TyIdx")
    }

    /// Resolve a [`PunTy`][] based on the current [`PunEnv`][].
    pub fn resolve_pun(&self, pun: &PunTy, env: &PunEnv) -> Result<TyIdx> {
        for block in &pun.blocks {
            if block.selector.matches(env) {
                return Ok(block.real);
            }
        }

        Err(KdlScriptTypeError {
            message: "Failed to find applicable pun for this target environment".to_string(),
            src: self.src.clone(),
            span: Spanned::span(&pun.name),
            help: Some(format!("Add another block that matches {:#?}", env)),
        })?
    }

    /*
    pub fn pointee_ty(&self, ty: TyIdx) -> TyIdx {
        if let Ty::TypedPtr(pointee) = self.realize_ty(ty) {
            *pointee
        } else {
            unreachable!("expected typed to be pointer");
        }
    }
     */

    /// Stringify a type.
    pub fn format_ty(&self, ty: TyIdx) -> String {
        match self.realize_ty(ty) {
            Ty::Primitive(prim) => format!("{:?}", prim).to_lowercase(),
            Ty::Empty => "()".to_string(),
            Ty::Struct(decl) => format!("{}", decl.name),
            Ty::Enum(decl) => format!("{}", decl.name),
            Ty::Tagged(decl) => format!("{}", decl.name),
            Ty::Union(decl) => format!("{}", decl.name),
            Ty::Alias(decl) => format!("{}", decl.name),
            Ty::Pun(decl) => format!("{}", decl.name),
            Ty::Array(array_ty) => {
                let inner = self.format_ty(array_ty.elem_ty);
                format!("[{}; {}]", inner, array_ty.len)
            }
            Ty::Ref(ref_ty) => {
                let inner = self.format_ty(ref_ty.pointee_ty);
                format!("&{}", inner)
            }
        }
    }
}

/// A node in the DefinitionGraph
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum DefinitionGraphNode {
    Func(FuncIdx),
    Ty(TyIdx),
}

/// A Dependency Graph of all the type/function definitions.
#[derive(Debug, Clone)]
pub struct DefinitionGraph {
    /// The actual Graph
    graph: DiGraph<DefinitionGraphNode, ()>,
    /// FuncIdx = NodeIdx
    func_nodes: Vec<NodeIndex>,
    /// The Strongly Connected Components in topological order
    def_order: Vec<Vec<NodeIndex>>,
}

impl TypedProgram {
    /// Get the Ty for this TyIdx
    pub fn realize_ty(&self, ty: TyIdx) -> &Ty {
        self.tcx.realize_ty(ty)
    }

    /// Get the Func for this FuncIdx
    pub fn realize_func(&self, func: FuncIdx) -> &Func {
        &self.funcs[func]
    }

    pub fn all_funcs(&self) -> impl Iterator<Item = FuncIdx> {
        0..self.builtin_funcs_start
    }

    /// Resolve a [`PunTy`][] based on the current [`PunEnv`][].
    pub fn resolve_pun(&self, pun: &PunTy, env: &PunEnv) -> Result<TyIdx> {
        self.tcx.resolve_pun(pun, env)
    }

    /// Stringify a type (for debugging).
    pub fn format_ty(&self, ty: TyIdx) -> String {
        self.tcx.format_ty(ty)
    }

    /// Compute the dependency graph between types ([`DefinitionGraph`][]).
    ///
    /// This serves two purposes:
    ///
    /// * Figuring out the correct order of type/function declarations (and forward declarations)
    ///   for languages like C that need that kind of thing (and its prettier for other langs).
    ///
    /// * Producing minimized examples for subsets of the program (by only emitting the types
    ///   needed for a single function).
    ///
    /// The next step in lowering the program is to query [`DefinitionGraph::definitions`][] with the
    /// functions you want to emit!
    ///
    /// This can fail if the given [`PunEnv`][] fails to resolve a [`PunTy`][].
    pub fn definition_graph(&self, env: &PunEnv) -> Result<DefinitionGraph> {
        let mut graph = petgraph::graph::DiGraph::new();
        let mut nodes = vec![];

        // First create all the nodes for all the types
        for (ty_idx, _ty) in self.tcx.tys.iter().enumerate() {
            let ty_node = graph.add_node(DefinitionGraphNode::Ty(ty_idx));
            nodes.push(ty_node);
        }

        // Now create edges between them for deps
        //
        // NOTE: it's fine to make an edge from a node to itself, that doesn't
        // change anything we do further down with SCCs!
        for (ty_idx, ty) in self.tcx.tys.iter().enumerate() {
            let ty_node = nodes[ty_idx];
            match ty {
                Ty::Struct(ty) => {
                    for field in &ty.fields {
                        let field_ty_node = nodes[field.ty];
                        graph.update_edge(ty_node, field_ty_node, ());
                    }
                }
                Ty::Union(ty) => {
                    for field in &ty.fields {
                        let field_ty_node = nodes[field.ty];
                        graph.update_edge(ty_node, field_ty_node, ());
                    }
                }
                Ty::Tagged(ty) => {
                    for variant in &ty.variants {
                        if let Some(fields) = &variant.fields {
                            for field in fields {
                                let field_ty_node = nodes[field.ty];
                                graph.update_edge(ty_node, field_ty_node, ());
                            }
                        }
                    }
                }
                Ty::Alias(ty) => {
                    let real_ty_node = nodes[ty.real];
                    graph.update_edge(ty_node, real_ty_node, ());
                }
                Ty::Pun(ty) => {
                    let real_ty_node = nodes[self.tcx.resolve_pun(ty, env)?];
                    graph.update_edge(ty_node, real_ty_node, ());
                }
                Ty::Array(ty) => {
                    let elem_ty_node = nodes[ty.elem_ty];
                    graph.update_edge(ty_node, elem_ty_node, ());
                }
                Ty::Ref(ty) => {
                    let pointee_ty_node = nodes[ty.pointee_ty];
                    graph.update_edge(ty_node, pointee_ty_node, ());
                }
                Ty::Enum(_) => {
                    // Arguably this can't depend on any types...
                    // BUT we should consider whether `@tag i32` is a dependency on i32!
                    // These kinds of annotations aren't configured yet though!
                }
                Ty::Primitive(_) | Ty::Empty => {
                    // These types have no deps, no edges to add!
                }
            }
        }

        // Add edges from functions to the things they reference
        let mut func_nodes = vec![];
        for (func_idx, func) in self.funcs.iter().enumerate() {
            let func_node = graph.add_node(DefinitionGraphNode::Func(func_idx));
            for arg in func.inputs.iter().chain(func.outputs.iter()) {
                let arg_ty_node = nodes[arg.ty];
                graph.update_edge(func_node, arg_ty_node, ());
            }
            func_nodes.push(func_node);
        }

        // Now compute the Strongly Connected Components!
        // See the comment in `DefinitionGraph::definitions` for details on what this is!
        let def_order = petgraph::algo::kosaraju_scc(&graph);

        Ok(DefinitionGraph {
            graph,
            func_nodes,
            def_order,
        })
    }
}

/// Kinds of definitions/declarations a backend should emit
pub enum Definition {
    /// Forward-declare this type
    DeclareTy(TyIdx),
    /// Define this type fully
    DefineTy(TyIdx),
    /// Forward-declare the function (only ever necessary with `feature="eval"`)
    /// otherwise functions are always roots and never need forward-declares.
    DeclareFunc(FuncIdx),
    /// Define this function fully
    DefineFunc(FuncIdx),
}

impl DefinitionGraph {
    /// Get the exact list of forward-declares and definitions to emit the program!
    ///
    /// Note that the recommendations are *extremely* agnostic to the target language
    /// and will generally recommend you forward-declare or define a lot of types
    /// that need no such thing in basically every language.
    ///
    /// For instance with a definition like:
    ///
    /// ```kdl
    /// struct "SelfReferential" {
    ///     me "Option<&SelfReferential>"
    ///     val "u32"
    /// }
    /// ```
    ///
    /// (Generics aren't currently supported, this is just easier to express.)
    ///
    /// You will get recommended something like:
    ///
    /// 1. Define `u32`
    /// 2. Forward-declare `SelfReferential`
    /// 3. Forward-declare `&SelfReferential`
    /// 4. Define `Option<&SelfReferential>`
    /// 5. Define `SelfReferential`
    /// 6. Define `&SelfReferential`
    ///
    /// Which contains a lot of things that are nonsensical in basically every language!
    /// That's ok! Just ignore the nonsensical recommendations like "declare a primitive"
    /// or "forward-declare a reference" if they aren't necessary in your language!
    ///
    /// A Rust backend would only need to emit 5 (and maybe 4 if it's not Real Option).
    ///
    /// A C backend would only need to emit 2 and 5 (and maybe 4 if it's not Real Option).
    pub fn definitions(&self, funcs: impl IntoIterator<Item = FuncIdx>) -> Vec<Definition> {
        // Take the requested functions and compute all their dependencies!
        let mut reachable = std::collections::HashSet::new();
        petgraph::visit::depth_first_search(
            &self.graph,
            funcs.into_iter().map(|f| self.func_nodes[f]),
            |event| {
                if let petgraph::visit::DfsEvent::Discover(node, _) = event {
                    reachable.insert(node);
                }
            },
        );

        // Languages like C and C++ require types to be defined before they're used,
        // so we need to build a dependency graph between types and functions and
        // compute the topological sort. Unfortunately, types don't necessarily
        // form a DAG, so how do we do this!?
        //
        // An "SCC" algorithm gives us a topo-sort of our graph as a DAG,
        // but if there are any cycles then they get grouped together into one Mega Node
        // called a "Strongly Connected Component" (defined as "every node in an SCC can
        // reach every other node in the SCC"). This is why our toposort has elements that
        // are Vec<Node> instead of just Node. The order of elements within an SCC
        // is arbitrary because they're basically a dependency soup.
        //
        // If the graph is a proper DAG then every inner Vec will have one element. Otherwise
        // cycles will be "hidden" by cramming it into a Vec. In the limit everything will
        // get crammed into one inner Vec and that basically tells you everything is super
        // fucked.
        //
        // To unfuck an SCC, languages like C and C++ have "forward declarations" that let
        // you reserve a type name before actually specifying its definition. This breaks
        // the dependency cycle. Determining the optimal forward declarations is NP-Hard,
        // so we opt for the conservative solution of "emit a forward decl for everything
        // in the SCC except for 1 node", which is necessary and sufficient if the SCC
        // is the complete graph on N nodes.
        let mut output = vec![];
        for component in &self.def_order {
            // Get all the nodes in this SCC, and filter out the ones not reachable from
            // the functions we want to emit. (Filtering lets us emit minimal examples for
            // test failures so it's easy to reproduce/report!)
            let nodes = component.iter().filter(|n| reachable.contains(*n));

            // Emit forward decls for everything but the first node
            // Note that this cutely does The Right thing (no forward decl)
            // for the "happy" case of an SCC of one node (proper DAG).
            for &node_idx in nodes.clone().skip(1) {
                let node = &self.graph[node_idx];
                match *node {
                    DefinitionGraphNode::Func(func_idx) => {
                        output.push(Definition::DeclareFunc(func_idx));
                    }
                    DefinitionGraphNode::Ty(ty_idx) => {
                        output.push(Definition::DeclareTy(ty_idx));
                    }
                }
            }

            // Now that cycles have been broken with forward declares, we can
            // just emit everything in the SCC. Note that we emit the type we
            // *didn't* forward-declare first. All of its dependencies have
            // been resolved by the forward-declares, but it still needs to be
            // defined before anyone else in case they refer to it!
            for &node_idx in nodes {
                let node = &self.graph[node_idx];
                match *node {
                    DefinitionGraphNode::Func(func_idx) => {
                        output.push(Definition::DefineFunc(func_idx));
                    }
                    DefinitionGraphNode::Ty(ty_idx) => {
                        output.push(Definition::DefineTy(ty_idx));
                    }
                }
            }
        }

        output
    }
}

fn ident_var<T>(val: Option<Ident>, basename: &str, idx: usize, backup_span: &Spanned<T>) -> Ident {
    if let Some(val) = val {
        val
    } else {
        let val = format!("{basename}{idx}");
        let val = Spanned::new(val, Spanned::span(backup_span));
        Ident {
            was_blank: false,
            val,
        }
    }
}
