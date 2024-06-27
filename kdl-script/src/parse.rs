//! Parsing and AST types.
//!
//! The entrypoint is to invoke [`parse_kdl_script`][] which is implicitly
//! handled by [`Compiler::compile_path`][] or [`Compiler::compile_string`][]
//! and will produce a [`ParsedProgram`][].
//!
//! Things like name resolution are handled by the [type checker](`crate::types`).

use std::sync::Arc;

use kdl::{KdlDocument, KdlEntry, KdlNode};
use miette::{Diagnostic, NamedSource, SourceSpan};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{alpha1, alphanumeric1};
use nom::combinator::{all_consuming, cut, opt, recognize};
use nom::error::{context, VerboseError};
use nom::multi::{many0, many0_count, separated_list1};
use nom::sequence::{delimited, pair, preceded, separated_pair};
use nom::{Finish, IResult};
use thiserror::Error;
use tracing::trace;

use crate::spanned::Spanned;
use crate::{Compiler, Result};

pub type StableMap<K, V> = linked_hash_map::LinkedHashMap<K, V>;

/// A string that may refer to another item like a type of function
#[derive(Debug, Clone)]
pub struct Ident {
    /// true if the ident was actually "_" and we made up a name.
    ///
    /// Languages which have anonymous/positional fields may choose
    /// to emit this field as such, instead of using `val`.
    pub was_blank: bool,
    /// the string to use for the ident
    pub val: Spanned<String>,
}

impl std::cmp::PartialEq for Ident {
    fn eq(&self, other: &Self) -> bool {
        self.val == other.val
    }
}
impl std::cmp::PartialEq<String> for Ident {
    fn eq(&self, other: &String) -> bool {
        self.val.as_str() == other
    }
}
impl std::cmp::PartialEq<str> for Ident {
    fn eq(&self, other: &str) -> bool {
        self.val.as_str() == other
    }
}
impl std::cmp::Eq for Ident {}
impl std::cmp::PartialOrd for Ident {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl std::cmp::Ord for Ident {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.val.cmp(&other.val)
    }
}
impl std::hash::Hash for Ident {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.val.hash(state);
    }
}
impl std::borrow::Borrow<str> for Ident {
    fn borrow(&self) -> &str {
        &self.val
    }
}

impl std::ops::Deref for Ident {
    type Target = Spanned<String>;
    fn deref(&self) -> &Self::Target {
        &self.val
    }
}
impl From<String> for Ident {
    fn from(val: String) -> Self {
        Self::new(Spanned::from(val))
    }
}

impl Ident {
    fn new(val: Spanned<String>) -> Self {
        Self {
            val,
            was_blank: false,
        }
    }
    fn with_span(val: String, span: SourceSpan) -> Self {
        Self {
            val: Spanned::new(val, span),
            was_blank: false,
        }
    }
}

impl std::fmt::Display for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.val.fmt(f)
    }
}

/// An error that occured during parsing
#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
pub struct KdlScriptParseError {
    pub message: String,
    #[source_code]
    pub src: Arc<NamedSource>,
    #[label]
    pub span: SourceSpan,
    #[help]
    pub help: Option<String>,
}

/// A Parsed KDLScript program
#[derive(Debug, Clone)]
pub struct ParsedProgram {
    /// The type definitions
    pub tys: StableMap<Ident, TyDecl>,
    /// The function definitions
    pub funcs: StableMap<Ident, FuncDecl>,
    /// Where in funcs builtins like `+` start (if at all).
    pub builtin_funcs_start: usize,
}

/// A Type declaration
#[derive(Debug, Clone)]
pub enum TyDecl {
    /// A Struct
    Struct(StructDecl),
    /// An untagged union
    Union(UnionDecl),
    /// A c-style enum
    Enum(EnumDecl),
    /// A tagged union (rust-style enum)
    Tagged(TaggedDecl),
    /// A transparent type alias
    Alias(AliasDecl),
    /// A type pun
    Pun(PunDecl),
}

/// A type "name" (which may be structural like `[u32; 4]`).
///
/// It's like an ident but, for types -- a tydent!
#[derive(Debug, Clone)]
pub enum Tydent {
    /// A named type (the type checker will resolve this)
    Name(Ident),
    /// A fixed length array
    Array(Box<Spanned<Tydent>>, u64),
    /// A by-reference type
    Ref(Box<Spanned<Tydent>>),
    /// The empty tuple -- `()`
    Empty,
}

/// An attribute that can be hung off a function or type.
///
/// Currently stubbed out, not really used. Potential uses:
///
/// * setting the backing type on an Enum's tag
/// * packed(N)
/// * align(N)
/// * passthrough attrs to underlying language
///
/// Probably this should be broken up so that you can't "pack"
/// a function or other such nonsense...
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Attr {
    /// rust-style derive (unused, just for testing)
    Derive(AttrDerive),
    /// The type should be packed
    Packed(AttrPacked),
    /// Pass this attribute through to the target language
    Passthrough(AttrPassthrough),
}

/// An attribute declaring this type should be packed (remove padding/align).
///
/// TODO: add support for an integer argument for the max align of a
/// field? Without one the default is 1. I never see packed(2) or whatever
/// so I've never seriously thought through the implications...
///
/// @packed (N?)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttrPacked {}

/// An attribute declaring a rust-style derive (unused, just for testing).
///
/// @derive "Trait1" "Trait2" ...
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttrDerive(Vec<Spanned<String>>);

/// An attribute to passthrough to the target language.
///
/// @ "whatever you want buddy"
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttrPassthrough(Spanned<String>);

/// A struct decl.
///
/// Field names may be positional by naming them underscore (`_`).
/// For languages like Rust, if all fields are declared like this
/// it should be lowered to a tuple-struct. Otherwise positional
/// fields will be autonamed something like field0, field1, ...
#[derive(Debug, Clone)]
pub struct StructDecl {
    /// Name of the struct
    pub name: Ident,
    /// Fields
    pub fields: Vec<TypedVar>,
    /// Attributes
    pub attrs: Vec<Attr>,
}

/// An untagged union decl.
///
/// Variant names may be positional by naming them underscore (`_`).
/// Positional variants will be autonamed something like Case0, Case1, ...
#[derive(Debug, Clone)]
pub struct UnionDecl {
    /// Name of the union
    pub name: Ident,
    /// Fields (variants)
    pub fields: Vec<TypedVar>,
    pub attrs: Vec<Attr>,
}

/// A c-like enum decl.
///
/// Variant names may be positional by naming them underscore (`_`).
/// Positional variants will be autonamed something like Case0, Case1, ...
#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: Ident,
    pub variants: Vec<EnumVariant>,
    pub attrs: Vec<Attr>,
}

/// A variant of an [`EnumDecl`].
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: Ident,
    /// Optional value this case is required to have
    /// in its underlying integer representation.
    pub val: Option<IntExpr>,
}

/// A tagged union (rust-like enum).
///
/// Variant names may be positional by naming them underscore (`_`).
/// Positional variants will be autonamed something like Case0, Case1, ...
#[derive(Debug, Clone)]
pub struct TaggedDecl {
    pub name: Ident,
    pub variants: Vec<TaggedVariant>,
    pub attrs: Vec<Attr>,
}

/// A variant of a [`TaggedDecl`][].
#[derive(Debug, Clone)]
pub struct TaggedVariant {
    pub name: Ident,
    pub fields: Option<Vec<TypedVar>>,
}

/// A type pun between different languages.
#[derive(Debug, Clone)]
pub struct PunDecl {
    pub name: Ident,
    pub blocks: Vec<PunBlock>,
    pub attrs: Vec<Attr>,
}

/// A block for a [`PunDecl`][].
#[derive(Debug, Clone)]
pub struct PunBlock {
    pub selector: PunSelector,
    pub decl: TyDecl,
}

/// A selector for a [`PunBlock`][]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PunSelector {
    /// Selector applies if any of the following apply.
    Any(Vec<PunSelector>),
    /// Selector applies if all of the following apply.
    All(Vec<PunSelector>),
    /// Selector applies if the [`PunEnv::lang`] is the following.
    Lang(Spanned<String>),
    /// Selector always applies (default fallback).
    Default,
}

/// The environment required to resolve a [`PunSelector`][].
#[derive(Debug, Clone)]
pub struct PunEnv {
    /// The target language
    ///
    /// TODO: make this an enum?
    pub lang: String,
    // compiler: String,
    // os: String,
    // cpu: String,
}

impl PunSelector {
    /// Check if this [`PunSelector`][] matches the given [`PunEnv`][].
    pub fn matches(&self, env: &PunEnv) -> bool {
        use PunSelector::*;
        match self {
            Any(args) => args.iter().any(|s| s.matches(env)),
            All(args) => args.iter().all(|s| s.matches(env)),
            Lang(lang) => env.lang == **lang,
            Default => true,
        }
    }
}

/// A transparent type alias.
#[derive(Debug, Clone)]
pub struct AliasDecl {
    pub name: Ident,
    pub alias: Spanned<Tydent>,
    pub attrs: Vec<Attr>,
}

/// A (name, type) pair that occurs in many places like field/arg decls.
#[derive(Debug, Clone)]
pub struct TypedVar {
    pub name: Option<Ident>,
    pub ty: Spanned<Tydent>,
}

/// A function declaration
#[derive(Debug, Clone)]
pub struct FuncDecl {
    pub name: Ident,
    pub inputs: Vec<TypedVar>,
    pub outputs: Vec<TypedVar>,
    pub attrs: Vec<Attr>,
    #[cfg(feature = "eval")]
    pub body: Vec<Stmt>,
}

/// The parser, used to hold onto some global state for things like diagnostic.
struct Parser<'a> {
    // comp: &'a mut Compiler,
    src: Arc<NamedSource>,
    ast: &'a KdlDocument,
}

/// Parse a KdlScript program!
pub fn parse_kdl_script(
    _comp: &mut Compiler,
    src: Arc<NamedSource>,
    ast: &KdlDocument,
) -> Result<ParsedProgram> {
    let mut parser = Parser { src, ast };
    parser.parse()
}

impl Parser<'_> {
    /// Parse a KdlScript program!
    fn parse(&mut self) -> Result<ParsedProgram> {
        trace!("parsing");

        let mut program = self.parse_module(self.ast)?;
        #[cfg(feature = "eval")]
        program.add_builtin_funcs()?;

        Ok(program)
    }

    /// Parse a "module" which is either the entire program or the contents of a [`PunBlock`][].
    fn parse_module(&mut self, doc: &KdlDocument) -> Result<ParsedProgram> {
        let mut funcs = StableMap::new();
        let mut tys = StableMap::new();

        let mut cur_attrs = vec![];
        for node in doc.nodes() {
            let name = node.name().value();

            // If it's an attribute, gather it up to be given to a "real" item
            if name.starts_with('@') {
                cur_attrs.push(self.attr(node)?);
                continue;
            }

            // Ok it's a real item, grab all the attributes, they belong to it
            let attrs = std::mem::take(&mut cur_attrs);

            // Now parse the various kinds of top-level items
            match name {
                "fn" => {
                    let func = self.func_decl(node, attrs)?;
                    funcs.insert(func.name.clone(), func);
                }
                "struct" => {
                    let ty = self.struct_decl(node, attrs)?;
                    let old = tys.insert(ty.name.clone(), TyDecl::Struct(ty));
                    assert!(old.is_none(), "duplicate type def");
                }
                "union" => {
                    let ty = self.union_decl(node, attrs)?;
                    let old = tys.insert(ty.name.clone(), TyDecl::Union(ty));
                    assert!(old.is_none(), "duplicate type def");
                }
                "enum" => {
                    let ty = self.enum_decl(node, attrs)?;
                    let old = tys.insert(ty.name.clone(), TyDecl::Enum(ty));
                    assert!(old.is_none(), "duplicate type def");
                }
                "tagged" => {
                    let ty = self.tagged_decl(node, attrs)?;
                    let old = tys.insert(ty.name.clone(), TyDecl::Tagged(ty));
                    assert!(old.is_none(), "duplicate type def");
                }
                "alias" => {
                    let ty = self.alias_decl(node, attrs)?;
                    let old = tys.insert(ty.name.clone(), TyDecl::Alias(ty));
                    assert!(old.is_none(), "duplicate type def");
                }
                "pun" => {
                    let ty = self.pun_decl(node, attrs)?;
                    let old = tys.insert(ty.name.clone(), TyDecl::Pun(ty));
                    assert!(old.is_none(), "duplicate type def");
                }
                x => {
                    return Err(KdlScriptParseError {
                        message: format!("I don't know what a '{x}' is"),
                        src: self.src.clone(),
                        span: *node.name().span(),
                        help: None,
                    })?;
                }
            }
        }

        // Anything added after this point is a builtin!
        let builtin_funcs_start = funcs.len();
        Ok(ParsedProgram {
            tys,
            funcs,
            builtin_funcs_start,
        })
    }

    /// Parse a `struct` node.
    fn struct_decl(&mut self, node: &KdlNode, attrs: Vec<Attr>) -> Result<StructDecl> {
        trace!("struct decl");
        let name = self.one_string(node, "type name")?;
        let name = self.ident(name)?;
        let fields = self.typed_var_children(node)?;

        Ok(StructDecl {
            name,
            fields,
            attrs,
        })
    }

    /// Parse a `union` node.
    fn union_decl(&mut self, node: &KdlNode, attrs: Vec<Attr>) -> Result<UnionDecl> {
        trace!("union decl");
        let name = self.one_string(node, "type name")?;
        let name = self.ident(name)?;
        let fields = self.typed_var_children(node)?;

        Ok(UnionDecl {
            name,
            fields,
            attrs,
        })
    }

    /// Parse an `enum` node.
    fn enum_decl(&mut self, node: &KdlNode, attrs: Vec<Attr>) -> Result<EnumDecl> {
        trace!("enum decl");
        let name = self.one_string(node, "type name")?;
        let name = self.ident(name)?;
        let variants = self.enum_variant_children(node)?;

        Ok(EnumDecl {
            name,
            variants,
            attrs,
        })
    }

    /// Parse a `tagged` node.
    fn tagged_decl(&mut self, node: &KdlNode, attrs: Vec<Attr>) -> Result<TaggedDecl> {
        trace!("enum decl");
        let name = self.one_string(node, "type name")?;
        let name = self.ident(name)?;
        let variants = self.tagged_variant_children(node)?;

        Ok(TaggedDecl {
            name,
            variants,
            attrs,
        })
    }

    /// Parse a `pun` node.
    fn pun_decl(&mut self, node: &KdlNode, attrs: Vec<Attr>) -> Result<PunDecl> {
        let name = self.one_string(node, "type name")?;
        let name = self.ident(name)?;

        // Parse the pun blocks
        let mut blocks = vec![];
        for item in node.children().into_iter().flat_map(|d| d.nodes()) {
            let item_name = item.name().value();
            match item_name {
                "lang" => {
                    let langs = self.string_list(item.entries())?;
                    if langs.is_empty() {
                        let node_ident = item.name().span();
                        let after_ident = node_ident.offset() + node_ident.len();
                        return Err(KdlScriptParseError {
                            message: "Hey I need a lang name (string) here!".to_string(),
                            src: self.src.clone(),
                            span: (after_ident..after_ident).into(),
                            help: None,
                        })?;
                    }
                    let final_ty = self.pun_block(item, &name)?;
                    blocks.push(PunBlock {
                        selector: PunSelector::Any(
                            langs.into_iter().map(PunSelector::Lang).collect(),
                        ),
                        decl: final_ty,
                    });
                }
                "default" => {
                    self.no_args(item)?;
                    let final_ty = self.pun_block(item, &name)?;
                    blocks.push(PunBlock {
                        selector: PunSelector::Default,
                        decl: final_ty,
                    });
                }
                x => {
                    return Err(KdlScriptParseError {
                        message: format!("I don't know what a '{x}' is here"),
                        src: self.src.clone(),
                        span: *item.name().span(),
                        help: None,
                    })?;
                }
            }
        }

        Ok(PunDecl {
            name,
            blocks,
            attrs,
        })
    }

    /// Parse a pun block, expecting only a single type with the pun's name.
    ///
    /// In the future we may allow [`PunBlock`][]s to declare other "private" types
    /// that are only in scope for the block but required to define the final type.
    /// For now we punt on this to avoid thinking through the name resolution implications
    /// for things like name shadowing.
    fn pun_block(&mut self, block: &KdlNode, final_ty_name: &Ident) -> Result<TyDecl> {
        if let Some(doc) = block.children() {
            // Recursively parse this block as an entire KdlScript program
            let defs = self.parse_module(doc)?;

            // Don't want any functions
            if let Some((_name, func)) = defs.funcs.iter().next() {
                return Err(KdlScriptParseError {
                    message: "puns can't contain function decls".to_string(),
                    src: self.src.clone(),
                    span: Spanned::span(&func.name),
                    help: None,
                })?;
            }

            let mut final_ty = None;

            // Only want one type declared (might loosen this later)
            for (ty_name, ty) in defs.tys {
                if &ty_name == final_ty_name {
                    // this is the type
                    final_ty = Some(ty);
                } else {
                    return Err(KdlScriptParseError {
                        message: "pun declared a type other than what it should have".to_string(),
                        src: self.src.clone(),
                        span: Spanned::span(&ty_name),
                        help: None,
                    })?;
                }
            }

            // Check that we defined the type
            if let Some(ty) = final_ty {
                Ok(ty)
            } else {
                Err(KdlScriptParseError {
                    message: "pun block failed to define the type it puns!".to_string(),
                    src: self.src.clone(),
                    span: *block.span(),
                    help: None,
                })?
            }
        } else {
            Err(KdlScriptParseError {
                message: "pun blocks need bodies".to_string(),
                src: self.src.clone(),
                span: *block.span(),
                help: None,
            })?
        }
    }

    /// Parse an `alias` node.
    fn alias_decl(&mut self, node: &KdlNode, attrs: Vec<Attr>) -> Result<AliasDecl> {
        let name = self.string_at(node, "type name", 0)?;
        let name = self.ident(name)?;
        let alias_str = self.string_at(node, "type name", 1)?;
        let alias = self.tydent(&alias_str)?;

        Ok(AliasDecl { name, alias, attrs })
    }

    /// Parse a `fn` node.
    fn func_decl(&mut self, node: &KdlNode, attrs: Vec<Attr>) -> Result<FuncDecl> {
        trace!("fn");
        let name = self.one_string(node, "function name")?;
        let name = self.ident(name)?;
        let mut inputs = vec![];
        let mut outputs = vec![];
        #[cfg(feature = "eval")]
        let mut body = vec![];

        let mut reached_body = false;
        let mut input_span = None;
        let mut output_span = None;
        for stmt in node.children().into_iter().flat_map(|d| d.nodes()) {
            match stmt.name().value() {
                "inputs" => {
                    trace!("fn input");
                    if reached_body {
                        return Err(KdlScriptParseError {
                            message: "input declaration must come before the body".to_string(),
                            src: self.src.clone(),
                            span: *stmt.name().span(),
                            help: None,
                        })?;
                    }
                    if let Some(_old_input) = input_span {
                        return Err(KdlScriptParseError {
                            message: "duplicate input block".to_string(),
                            src: self.src.clone(),
                            span: *stmt.name().span(),
                            help: None,
                        })?;
                    }
                    if let Some(_old_output) = output_span {
                        return Err(KdlScriptParseError {
                            message: "It's confusing to declare inputs after outputs".to_string(),
                            src: self.src.clone(),
                            span: *stmt.name().span(),
                            help: Some("Move this before the output block".to_string()),
                        })?;
                    }
                    self.no_args(stmt)?;
                    inputs = self.typed_var_children(stmt)?;
                    input_span = Some(*stmt.name().span());
                    continue;
                }
                "outputs" => {
                    trace!("fn output");
                    if reached_body {
                        return Err(KdlScriptParseError {
                            message: "output declaration must come before the body".to_string(),
                            src: self.src.clone(),
                            span: *stmt.name().span(),
                            help: None,
                        })?;
                    }
                    if let Some(_old_output) = output_span {
                        return Err(KdlScriptParseError {
                            message: "duplicate output block".to_string(),
                            src: self.src.clone(),
                            span: *stmt.name().span(),
                            help: None,
                        })?;
                    }
                    self.no_args(stmt)?;
                    outputs = self.typed_var_children(stmt)?;
                    output_span = Some(*stmt.name().span());
                    continue;
                }
                x => {
                    #[cfg(feature = "eval")]
                    match x {
                        "let" => {
                            trace!("let stmt");
                            let name = self.string_at(stmt, "variable name", 0)?;
                            let name = self.ident(name)?;
                            let name = if &*name == "_" { None } else { Some(name) };
                            let expr = self.expr_rhs(stmt, 1)?;
                            body.push(Stmt::Let(LetStmt { var: name, expr }));
                        }
                        "return" => {
                            trace!("return stmt");
                            let expr = self.expr_rhs(stmt, 0)?;
                            body.push(Stmt::Return(ReturnStmt { expr }));
                        }
                        "print" => {
                            trace!("print stmt");
                            let expr = self.expr_rhs(stmt, 0)?;
                            body.push(Stmt::Print(PrintStmt { expr }));
                        }
                        x => {
                            return Err(KdlScriptParseError {
                                message: format!("I don't know what a '{x}' statement is"),
                                src: self.src.clone(),
                                span: *stmt.name().span(),
                                help: None,
                            })?;
                        }
                    }
                    #[cfg(not(feature = "eval"))]
                    return Err(KdlScriptParseError {
                        message: format!("I don't know what a '{x}' statement is"),
                        src: self.src.clone(),
                        span: *stmt.name().span(),
                        help: None,
                    })?;
                }
            }

            reached_body = true;
        }

        Ok(FuncDecl {
            name,
            inputs,
            outputs,
            #[cfg(feature = "eval")]
            body,
            attrs,
        })
    }

    /// Parse an `@attribute` node.
    fn attr(&mut self, attr: &KdlNode) -> Result<Attr> {
        let entries = attr.entries();
        let attr = match attr.name().value() {
            "@packed" => {
                trace!("packed attr");
                self.no_children(attr)?;
                Attr::Packed(AttrPacked {})
            }
            "@derive" => {
                trace!("derive attr");
                let traits = self.string_list(entries)?;
                self.no_children(attr)?;
                Attr::Derive(AttrDerive(traits))
            }
            "@" => {
                trace!("passthrough attr");
                let val = self.one_string(attr, "attribute to pass through to target language")?;
                Attr::Passthrough(AttrPassthrough(val))
            }
            x => {
                return Err(KdlScriptParseError {
                    message: format!("I don't know what a '{x}' attribute is"),
                    src: self.src.clone(),
                    span: *attr.name().span(),
                    help: None,
                })?;
            }
        };
        Ok(attr)
    }

    /// This node's entries should only be a positional list of strings.
    fn string_list(&mut self, entries: &[KdlEntry]) -> Result<Vec<Spanned<String>>> {
        entries
            .iter()
            .map(|e| -> Result<Spanned<String>> {
                if e.name().is_some() {
                    return Err(KdlScriptParseError {
                        message: "Named values don't belong here, only strings".to_string(),
                        src: self.src.clone(),
                        span: *e.span(),
                        help: Some("try removing the name".to_owned()),
                    })?;
                }
                match e.value() {
                    kdl::KdlValue::RawString(s) | kdl::KdlValue::String(s) => {
                        Ok(Spanned::new(s.clone(), *e.span()))
                    }
                    _ => Err(KdlScriptParseError {
                        message: "This should be a string".to_string(),
                        src: self.src.clone(),
                        span: *e.span(),
                        help: Some("try adding quotes?".to_owned()),
                    })?,
                }
            })
            .collect()
    }

    /// This node's entries should be only one string.
    fn one_string(&mut self, node: &KdlNode, desc: &str) -> Result<Spanned<String>> {
        let res = self.string_at(node, desc, 0)?;
        let entries = node.entries();
        if let Some(e) = entries.get(1) {
            return Err(KdlScriptParseError {
                message: format!("You have something extra after your {desc}"),
                src: self.src.clone(),
                span: *e.span(),
                help: Some("remove this?".to_owned()),
            })?;
        }
        Ok(res)
    }

    /// This node should have a string at this entry offset.
    fn string_at(&mut self, node: &KdlNode, desc: &str, offset: usize) -> Result<Spanned<String>> {
        let entries = node.entries();
        if let Some(e) = entries.get(offset) {
            if e.name().is_some() {
                return Err(KdlScriptParseError {
                    message: "Named values don't belong here, only strings".to_string(),
                    src: self.src.clone(),
                    span: *e.span(),
                    help: Some("try removing the name".to_owned()),
                })?;
            }

            match e.value() {
                kdl::KdlValue::RawString(s) | kdl::KdlValue::String(s) => {
                    Ok(Spanned::new(s.clone(), *e.span()))
                }
                _ => Err(KdlScriptParseError {
                    message: format!("This should be a {desc} (string)"),
                    src: self.src.clone(),
                    span: *e.span(),
                    help: Some("try adding quotes".to_owned()),
                })?,
            }
        } else {
            let node_ident = node.name().span();
            let after_ident = node_ident.offset() + node_ident.len();
            Err(KdlScriptParseError {
                message: format!("Hey I need a {desc} (string) here!"),
                src: self.src.clone(),
                span: (after_ident..after_ident).into(),
                help: None,
            })?
        }
    }

    /// This node should have no entries.
    fn no_args(&mut self, node: &KdlNode) -> Result<()> {
        if let Some(entry) = node.entries().first() {
            return Err(KdlScriptParseError {
                message: "This shouldn't have arguments".to_string(),
                src: self.src.clone(),
                span: *entry.span(),
                help: Some("delete them?".to_string()),
            })?;
        }
        Ok(())
    }

    /// This node should have no children.
    fn no_children(&mut self, node: &KdlNode) -> Result<()> {
        if let Some(children) = node.children() {
            return Err(KdlScriptParseError {
                message: "These children should never have been born".to_string(),
                src: self.src.clone(),
                span: *children.span(),
                help: Some("delete this block?".to_string()),
            })?;
        }
        Ok(())
    }

    /// This node's children should be TypedVars
    fn typed_var_children(&mut self, node: &KdlNode) -> Result<Vec<TypedVar>> {
        node.children()
            .into_iter()
            .flat_map(|d| d.nodes())
            .map(|var| {
                let name = self.var_name_decl(var)?;
                let ty_str = self.one_string(var, "type")?;
                let ty = self.tydent(&ty_str)?;
                self.no_children(var)?;
                Ok(TypedVar { name, ty })
            })
            .collect()
    }

    /// This node's children should be enum variants
    fn enum_variant_children(&mut self, node: &KdlNode) -> Result<Vec<EnumVariant>> {
        node.children()
            .into_iter()
            .flat_map(|d| d.nodes())
            .map(|var| {
                let name = var.name();
                let name = Spanned::new(name.value().to_owned(), *name.span());
                let name = self.ident(name)?;
                let entries = var.entries();
                let val = if let Some(e) = entries.first() {
                    Some(self.int_expr(e)?)
                } else {
                    None
                };
                if let Some(e) = entries.get(1) {
                    return Err(KdlScriptParseError {
                        message: "You have something extra after your enum case".to_string(),
                        src: self.src.clone(),
                        span: *e.span(),
                        help: Some("remove this?".to_owned()),
                    })?;
                }
                // TODO: deny any other members of `entries`
                self.no_children(var)?;
                Ok(EnumVariant { name, val })
            })
            .collect()
    }

    /// This node's children should be tagged variants
    fn tagged_variant_children(&mut self, node: &KdlNode) -> Result<Vec<TaggedVariant>> {
        node.children()
            .into_iter()
            .flat_map(|d| d.nodes())
            .map(|var| {
                self.no_args(var)?;
                let name = var.name();
                let name = Spanned::new(name.value().to_owned(), *name.span());
                let name = self.ident(name)?;
                let fields = if var.children().is_some() {
                    Some(self.typed_var_children(var)?)
                } else {
                    None
                };
                Ok(TaggedVariant { name, fields })
            })
            .collect()
    }

    /// Parse this node's name as a possibly-positional variable name
    ///
    /// TODO: probably want this `Option<Ident>` to be its own special enum?
    fn var_name_decl(&mut self, var: &KdlNode) -> Result<Option<Ident>> {
        let name = var.name();
        let name = if name.value() == "_" {
            None
        } else {
            let name = self.ident(Spanned::new(name.value().to_owned(), *name.span()))?;
            Some(name)
        };
        Ok(name)
    }

    /// Parse a [`Tydent`][] from this String.
    fn tydent(&mut self, input: &Spanned<String>) -> Result<Spanned<Tydent>> {
        let (_, ty_ref) = all_consuming(context("a type", tydent))(input)
            .finish()
            .map_err(|_e| KdlScriptParseError {
                message: String::from("couldn't parse type"),
                src: self.src.clone(),
                span: Spanned::span(input),
                help: None,
            })?;
        Ok(ty_ref)
    }

    /// Parse an [`Ident`][] from this String.
    fn ident(&mut self, input: Spanned<String>) -> Result<Ident> {
        let (_, _) =
            all_consuming(context("a type", tydent))(&input).map_err(|_e| KdlScriptParseError {
                message: String::from("invalid identifier"),
                src: self.src.clone(),
                span: Spanned::span(&input),
                help: None,
            })?;
        Ok(Ident {
            val: input,
            was_blank: false,
        })
    }

    /// Parse an [`IntExpr`][] (literal) from this entry.
    fn int_expr(&mut self, entry: &KdlEntry) -> Result<IntExpr> {
        if entry.name().is_some() {
            return Err(KdlScriptParseError {
                message: "Named values don't belong here, only literals".to_string(),
                src: self.src.clone(),
                span: *entry.span(),
                help: Some("try removing the name".to_owned()),
            })?;
        }
        let val = match entry.value() {
            kdl::KdlValue::Base2(int)
            | kdl::KdlValue::Base8(int)
            | kdl::KdlValue::Base10(int)
            | kdl::KdlValue::Base16(int) => *int,
            _ => {
                return Err(KdlScriptParseError {
                    message: String::from("must be an integer"),
                    src: self.src.clone(),
                    span: *entry.span(),
                    help: None,
                })?;
            }
        };
        Ok(IntExpr {
            span: *entry.span(),
            val,
        })
    }
}

// A fuckton of nom parsing for sub-syntax like Tydents.

type NomResult<I, O> = IResult<I, O, VerboseError<I>>;

/// Matches the syntax for tydent ("identifier, but for types") incl structural types like arrays/references.
fn tydent(input: &str) -> NomResult<&str, Spanned<Tydent>> {
    alt((tydent_ref, tydent_array, tydent_empty_tuple, tydent_named))(input)
}

/// Matches a reference type (&T)
fn tydent_ref(input: &str) -> NomResult<&str, Spanned<Tydent>> {
    let (input, pointee_ty) = preceded(
        tag("&"),
        context("pointee type", cut(preceded(many0(unicode_space), tydent))),
    )(input)?;
    // TODO: properly setup this span!
    Ok((input, Spanned::from(Tydent::Ref(Box::new(pointee_ty)))))
}

/// Matches an array type ([T; N])
fn tydent_array(input: &str) -> NomResult<&str, Spanned<Tydent>> {
    let (input, (elem_ty, array_len)) = delimited(
        tag("["),
        cut(separated_pair(
            context(
                "an element type",
                delimited(many0(unicode_space), tydent, many0(unicode_space)),
            ),
            tag(";"),
            context(
                "an array length (integer)",
                delimited(many0(unicode_space), array_len, many0(unicode_space)),
            ),
        )),
        tag("]"),
    )(input)?;

    // TODO: properly setup these spans!
    Ok((
        input,
        Spanned::from(Tydent::Array(Box::new(elem_ty), array_len)),
    ))
}

/// Matches an array length (u64)
fn array_len(input: &str) -> NomResult<&str, u64> {
    nom::character::complete::u64(input)
}

/// Matches the empty tuple
fn tydent_empty_tuple(input: &str) -> NomResult<&str, Spanned<Tydent>> {
    let (input, _tup) = tag("()")(input)?;
    // TODO: properly setup this span!
    Ok((input, Spanned::from(Tydent::Empty)))
}

/// Matches a named type
fn tydent_named(input: &str) -> NomResult<&str, Spanned<Tydent>> {
    let (input, (ty_name, generics)) = pair(
        ident,
        opt(delimited(
            pair(unicode_space, tag("<")),
            cut(separated_list1(
                tag(","),
                delimited(unicode_space, tydent_ref, unicode_space),
            )),
            tag(">"),
        )),
    )(input)?;

    if let Some(_generics) = generics {
        panic!("generics aren't yet implemented!");
    }
    // TODO: properly setup this span!
    Ok((
        input,
        Spanned::from(Tydent::Name(Ident {
            val: Spanned::from(ty_name.to_owned()),
            was_blank: false,
        })),
    ))
}

/// Matches an identifier
fn ident(input: &str) -> NomResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_")))),
    ))(input)
}

/// Matches various kinds of whitespace we allow
fn unicode_space(input: &str) -> NomResult<&str, &str> {
    alt((
        tag(" "),
        tag("\t"),
        tag("\u{FEFF}"), // BOM
        tag("\u{00A0}"),
        tag("\u{1680}"),
        tag("\u{2000}"),
        tag("\u{2001}"),
        tag("\u{2002}"),
        tag("\u{2003}"),
        tag("\u{2004}"),
        tag("\u{2005}"),
        tag("\u{2006}"),
        tag("\u{2007}"),
        tag("\u{2008}"),
        tag("\u{2009}"),
        tag("\u{200A}"),
        tag("\u{202F}"),
        tag("\u{205F}"),
        tag("\u{3000}"),
    ))(input)
}

/// An integer expression (literal)
///
/// TODO: this should actually defer deserializing into an integer
/// so that it can be some huge type like `u256`. Not sure who/where
/// would be responsible for validating that the value fits in the
/// expected range for where it's placed!
///
/// Possibly the type checker, but it also kinda needs to be deferred
/// to the target language backends as a language may not be able to
/// handle a `u256` or whatever.
#[derive(Debug, Clone)]
pub struct IntExpr {
    pub span: SourceSpan,
    pub val: i64,
}

/// All of this gunk is only used for function bodies, which only
/// exist in the meme `feature=eval` mode.
#[cfg(feature = "eval")]
pub use runnable::*;
#[cfg(feature = "eval")]
mod runnable {
    use super::*;

    #[derive(Debug, Clone)]
    pub enum Stmt {
        Let(LetStmt),
        Return(ReturnStmt),
        Print(PrintStmt),
    }

    #[derive(Debug, Clone)]
    pub struct LetStmt {
        pub var: Option<Ident>,
        pub expr: Spanned<Expr>,
    }

    #[derive(Debug, Clone)]
    pub struct ReturnStmt {
        pub expr: Spanned<Expr>,
    }

    #[derive(Debug, Clone)]
    pub struct PrintStmt {
        pub expr: Spanned<Expr>,
    }

    #[derive(Debug, Clone)]
    pub enum Expr {
        Call(CallExpr),
        Path(PathExpr),
        Ctor(CtorExpr),
        Literal(LiteralExpr),
    }

    #[derive(Debug, Clone)]
    pub struct CallExpr {
        pub func: Ident,
        pub args: Vec<Spanned<Expr>>,
    }

    #[derive(Debug, Clone)]
    pub struct PathExpr {
        pub var: Ident,
        pub path: Vec<Ident>,
    }

    #[derive(Debug, Clone)]
    pub struct CtorExpr {
        pub ty: Ident,
        pub vals: Vec<Spanned<LetStmt>>,
    }

    #[derive(Debug, Clone)]
    pub struct LiteralExpr {
        pub span: SourceSpan,
        pub val: Literal,
    }

    #[derive(Debug, Clone)]
    pub enum Literal {
        Float(f64),
        Int(i64),
        Bool(bool),
    }

    impl Parser<'_> {
        pub(crate) fn func_args(
            &mut self,
            node: &KdlNode,
            expr_start: usize,
        ) -> Result<Vec<Spanned<Expr>>> {
            node.entries()[expr_start..]
                .iter()
                .enumerate()
                .map(|(idx, _e)| self.smol_expr(node, expr_start + idx))
                .collect()
        }

        pub(crate) fn literal_expr(&mut self, entry: &KdlEntry) -> Result<LiteralExpr> {
            if entry.name().is_some() {
                return Err(KdlScriptParseError {
                    message: "Named values don't belong here, only literals".to_string(),
                    src: self.src.clone(),
                    span: *entry.span(),
                    help: Some("try removing the name".to_owned()),
                })?;
            }

            let val = match entry.value() {
                kdl::KdlValue::RawString(_) | kdl::KdlValue::String(_) => {
                    return Err(KdlScriptParseError {
                        message: "strings aren't supported literals".to_string(),
                        src: self.src.clone(),
                        span: *entry.span(),
                        help: None,
                    })?;
                }
                kdl::KdlValue::Null => {
                    return Err(KdlScriptParseError {
                        message: "nulls aren't supported literals".to_string(),
                        src: self.src.clone(),
                        span: *entry.span(),
                        help: None,
                    })?;
                }
                kdl::KdlValue::Base2(int)
                | kdl::KdlValue::Base8(int)
                | kdl::KdlValue::Base10(int)
                | kdl::KdlValue::Base16(int) => Literal::Int(*int),
                kdl::KdlValue::Base10Float(val) => Literal::Float(*val),
                kdl::KdlValue::Bool(val) => Literal::Bool(*val),
            };

            Ok(LiteralExpr {
                span: *entry.span(),
                val,
            })
        }

        pub(crate) fn expr_rhs(
            &mut self,
            node: &KdlNode,
            expr_start: usize,
        ) -> Result<Spanned<Expr>> {
            trace!("expr rhs");
            let expr = if let Ok(string) = self.string_at(node, "", expr_start) {
                if let Some((func, "")) = string.rsplit_once(':') {
                    trace!("  call expr");
                    let func = Ident::with_span(func.to_owned(), Spanned::span(&string));
                    let args = self.func_args(node, expr_start + 1)?;
                    Expr::Call(CallExpr { func, args })
                } else if node.children().is_some() {
                    trace!("  ctor expr");
                    let ty = Ident::new(string);
                    let vals = self.let_stmt_children(node)?;
                    Expr::Ctor(CtorExpr { ty, vals })
                } else {
                    trace!("  path expr");
                    let mut parts = string.split('.');
                    let var =
                        Ident::with_span(parts.next().unwrap().to_owned(), Spanned::span(&string));
                    let path = parts
                        .map(|s| Ident::with_span(s.to_owned(), Spanned::span(&string)))
                        .collect();
                    Expr::Path(PathExpr { var, path })
                }
            } else if let Some(val) = node.entries().get(expr_start) {
                trace!("  literal expr");
                Expr::Literal(self.literal_expr(val)?)
            } else {
                return Err(KdlScriptParseError {
                    message: "I thought there was supposed to be an expression after here?"
                        .to_string(),
                    src: self.src.clone(),
                    span: *node.span(),
                    help: None,
                })?;
            };

            Ok(Spanned::new(expr, *node.span()))
        }

        pub(crate) fn smol_expr(
            &mut self,
            node: &KdlNode,
            expr_at: usize,
        ) -> Result<Spanned<Expr>> {
            trace!("smol expr");
            let expr = if let Ok(string) = self.string_at(node, "", expr_at) {
                if let Some((_func, "")) = string.rsplit_once(':') {
                    return Err(KdlScriptParseError {
                        message:
                            "Nested function calls aren't supported because this is a shitpost"
                                .to_string(),
                        src: self.src.clone(),
                        span: *node.span(),
                        help: None,
                    })?;
                } else if node.children().is_some() {
                    return Err(KdlScriptParseError {
                        message: "Ctors exprs can't be nested in function calls because this is a shitpost".to_string(),
                        src: self.src.clone(),
                        span: *node.span(),
                        help: None,
                    })?;
                } else {
                    trace!("  path expr");
                    let mut parts = string.split('.');
                    let var =
                        Ident::with_span(parts.next().unwrap().to_owned(), Spanned::span(&string));
                    let path = parts
                        .map(|s| Ident::with_span(s.to_owned(), Spanned::span(&string)))
                        .collect();
                    Expr::Path(PathExpr { var, path })
                }
            } else if let Some(val) = node.entries().get(expr_at) {
                trace!("  literal expr");
                Expr::Literal(self.literal_expr(val)?)
            } else {
                return Err(KdlScriptParseError {
                    message: "I thought there was supposed to be an expression after here?"
                        .to_string(),
                    src: self.src.clone(),
                    span: *node.span(),
                    help: None,
                })?;
            };

            Ok(Spanned::new(expr, *node.span()))
        }

        pub(crate) fn let_stmt_children(
            &mut self,
            node: &KdlNode,
        ) -> Result<Vec<Spanned<LetStmt>>> {
            node.children()
                .into_iter()
                .flat_map(|d| d.nodes())
                .map(|var| {
                    let name = self.var_name_decl(var)?;
                    let expr = self.expr_rhs(var, 0)?;
                    Ok(Spanned::new(LetStmt { var: name, expr }, *var.span()))
                })
                .collect()
        }
    }

    impl ParsedProgram {
        pub(crate) fn add_builtin_funcs(&mut self) -> Result<()> {
            // Add builtins jankily
            self.funcs.insert(
                Ident::from(String::from("+")),
                FuncDecl {
                    name: Ident::from(String::from("+")),
                    inputs: vec![
                        TypedVar {
                            name: Some(Ident::from(String::from("lhs"))),
                            ty: Spanned::from(Tydent::Name(Ident::from(String::from("i64")))),
                        },
                        TypedVar {
                            name: Some(Ident::from(String::from("rhs"))),
                            ty: Spanned::from(Tydent::Name(Ident::from(String::from("i64")))),
                        },
                    ],
                    outputs: vec![TypedVar {
                        name: Some(Ident::from(String::from("out"))),
                        ty: Spanned::from(Tydent::Name(Ident::from(String::from("i64")))),
                    }],
                    attrs: vec![],

                    body: vec![],
                },
            );

            Ok(())
        }
    }
}
