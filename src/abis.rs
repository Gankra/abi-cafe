#![allow(non_camel_case_types)]

// Backends that can generate + compile an implementation's code into a staticlib.
pub mod c;
pub mod rust;

use super::report::BuildError;
use std::io::Write;
use std::path::Path;

pub use c::CcAbiImpl;
pub use rust::RustcAbiImpl;

pub static ABI_IMPL_RUSTC: &str = "rustc";
pub static ABI_IMPL_CC: &str = "cc";
pub static ABI_IMPL_GCC: &str = "gcc";
pub static ABI_IMPL_CLANG: &str = "clang";
pub static ABI_IMPL_MSVC: &str = "msvc";

// pub static ALL_ABIS: &[AbiRef] = &[RUST_ABI, C_ABI];
pub static ALL_CONVENTIONS: &[CallingConvention] = &[
    CallingConvention::Handwritten,
    CallingConvention::C,
    CallingConvention::Cdecl,
    CallingConvention::Stdcall,
    CallingConvention::Fastcall,
    CallingConvention::Vectorcall,
    // Note sure if these have a purpose, so omitting them for now
    // CallingConvention::System,
    // CallingConvention::Win64,
    // CallingConvention::Sysv64,
    // CallingConvention::Aapcs,
];

// pre-computed arg/field names to avoid a bunch of tedious formatting, and to make
// it easy to refactor this detail if we decide we don't like this naming scheme.
pub static ARG_NAMES: &[&str] = &[
    "arg0", "arg1", "arg2", "arg3", "arg4", "arg5", "arg6", "arg7", "arg8", "arg9", "arg10",
    "arg11", "arg12", "arg13", "arg14", "arg15", "arg16", "arg17", "arg18", "arg19", "arg20",
    "arg21", "arg22", "arg23", "arg24", "arg25", "arg26", "arg27", "arg28", "arg29", "arg30",
    "arg31", "arg32",
];
pub static FIELD_NAMES: &[&str] = &[
    "field0", "field1", "field2", "field3", "field4", "field5", "field6", "field7", "field8",
    "field9", "field10", "field11", "field12", "field13", "field14", "field15", "field16",
    "field17", "field18", "field19", "field20", "field21", "field22", "field23", "field24",
    "field25", "field26", "field27", "field28", "field29", "field30", "field31", "field32",
];
pub static OUTPUT_NAME: &str = "output";
pub static OUT_PARAM_NAME: &str = "out";

/// ABI is probably a bad name for this... it's like, a language/compiler impl. idk.
pub trait AbiImpl {
    fn name(&self) -> &'static str;
    fn lang(&self) -> &'static str;
    fn src_ext(&self) -> &'static str;
    fn supports_convention(&self, _convention: CallingConvention) -> bool;

    fn generate_callee(
        &self,
        f: &mut dyn Write,
        test: &Test,
        convention: CallingConvention,
    ) -> Result<(), GenerateError>;
    fn generate_caller(
        &self,
        f: &mut dyn Write,
        test: &Test,
        convention: CallingConvention,
    ) -> Result<(), GenerateError>;

    fn compile_callee(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError>;
    fn compile_caller(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError>;
}

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum GenerateError {
    #[error("io error\n{0}")]
    Io(#[from] std::io::Error),
    #[error("parse error {0}\n{2}\n{}\n{:width$}^",
        .1.lines().nth(.2.position.line.saturating_sub(1)).unwrap(),
        "",
        width=.2.position.col.saturating_sub(1),
    )]
    ParseError(String, String, ron::error::Error),
    #[error("kdl parse error {}", .2)]
    KdlParseError(String, String, kdl::KdlError),
    #[error("Two structs had the name {name}, but different layout! \nExpected {old_decl} \nGot {new_decl}")]
    InconsistentStructDefinition {
        name: String,
        old_decl: String,
        new_decl: String,
    },
    #[error("If you use the Handwritten calling convention, all functions in the test must use only that.")]
    HandwrittenMixing,
    #[error("No handwritten source for this pairing (skipping)")]
    NoHandwrittenSource,
    #[error("Unsupported Signature For Rust: {0}")]
    RustUnsupported(String),
    #[error("Unsupported Signature For C: {0}")]
    CUnsupported(String),
    #[error("ABI impl doesn't support this calling convention.")]
    UnsupportedConvention,
    /// Used to signal we just skipped it
    #[error("<skipped>")]
    Skipped,
}

/// A test, containing several subtests, each its own function
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Test {
    pub name: String,
    pub funcs: Vec<Func>,
}

/// A function's calling convention + signature which will
/// be used to generate the caller+callee automatically.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Func {
    pub name: String,
    pub conventions: Vec<CallingConvention>,
    pub inputs: Vec<Val>,
    pub output: Option<Val>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum CallingConvention {
    // These conventions are special ones that "desugar" to others
    /// Sugar for "every possible convention"
    All,
    /// A complete opaque convention, the implementation must be manually
    /// written in the handwritten_impls directory.
    Handwritten,
    /// The platform's default C convention (cdecl?)
    C,
    /// ???
    Cdecl,
    /// The platorm's default OS convention (usually C, but Windows is Weird).
    System,

    // These conventions are specific ones
    /// x64 windows C convention
    Win64,
    /// x64 non-windows C convention
    Sysv64,
    /// ARM C convention
    Aapcs,
    /// Win32 x86 system APIs
    Stdcall,
    /// Microsoft fastcall
    /// MSVC` __fastcall`
    /// GCC/Clang `__attribute__((fastcall))`
    Fastcall,
    /// Microsoft vectorcall
    /// MSCV `__vectorcall`
    /// GCC/Clang `__attribute__((vectorcall))`
    Vectorcall,
}

/// A typed value.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Val {
    /// A Ref is passed-by-reference (is a pointer) but the
    /// pointee will be regarded as the real value that we check.
    ///
    /// If a Ref val is used as the return value for a function, it will
    /// implicitly introduce an outparam that the callee memcpy's to.
    Ref(Box<Val>),
    /// Some integer
    Int(IntVal),
    /// Some float
    Float(FloatVal),
    /// A bool
    Bool(bool),
    /// An array (homogeneous types, checked on construction)
    ///
    /// Arrays must be wrapped in a Ref to directly use them as args/returns
    /// when compiling to C. Rust is fine with passing them by-value, but of
    /// course this is pointless when the other half of the equation pukes.
    ///
    /// FIXME: it isn't currently enforced that this is homogeneous, anything that needs
    /// the type of the elements just grabs the type of element 0
    ///
    /// FIXME: it's illegal to have an array of length 0, because it's impossible to
    /// attach a type to it
    Array(Vec<Val>),
    /// A named struct (heterogeneous type)
    ///
    /// Struct decls are implicitly derived from their usage as a value.
    /// If any two structs claim the same name but have different layouts,
    /// the ABI backends should notice this and return an error.
    Struct(String, Vec<Val>),
    /// An opaque pointer
    ///
    /// FIXME?: it's gross to just pick "u64" as the type here when ostensibly it would
    /// be nice for this to be able to taget 32-bit platforms. But using usize doesn't really
    /// make sense either because we're slurping these values out of a static config file!
    /// I guess just truncating the pointer is "fine".
    Ptr(u64),
    // TODO: unions. This is hard to do with the current design where
    // types are implicit in their values. You could maybe hack it in
    // by having dummy vals for all the different cases and then a
    // "real" value for the variant that's actually used, but, kinda gross.
    //
    // TODO: simd vectors (they have special passing rules!)
    //
    // TODO: enums (enum classes?)
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum IntVal {
    c__int128(i128),
    c_int64_t(i64),
    c_int32_t(i32),
    c_int16_t(i16),
    c_int8_t(i8),

    c__uint128(u128),
    c_uint64_t(u64),
    c_uint32_t(u32),
    c_uint16_t(u16),
    c_uint8_t(u8),
    // TODO: nastier platform-specific-layout c-types?
    // i.e. c_int(i64), c_long(i32), char(i8), ...
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum FloatVal {
    c_double(f64),
    c_float(f32),
    // Is there a reason to mess with `long double`? Surely not.
}

impl CallingConvention {
    pub fn name(&self) -> &'static str {
        match self {
            CallingConvention::All => {
                unreachable!("CallingConvention::All is sugar and shouldn't reach here!")
            }
            CallingConvention::Handwritten => "handwritten",
            CallingConvention::C => "c",
            CallingConvention::Cdecl => "cdecl",
            CallingConvention::System => "system",
            CallingConvention::Win64 => "win64",
            CallingConvention::Sysv64 => "sysv64",
            CallingConvention::Aapcs => "aapcs",
            CallingConvention::Stdcall => "stdcall",
            CallingConvention::Fastcall => "fastcall",
            CallingConvention::Vectorcall => "vectorcall",
        }
    }
    pub fn from_str(input: &str) -> Option<Self> {
        Some(match input {
            "all" => CallingConvention::All,
            "handwritten" => CallingConvention::Handwritten,
            "c" => CallingConvention::C,
            "cdecl" => CallingConvention::Cdecl,
            "system" => CallingConvention::System,
            "win64" => CallingConvention::Win64,
            "sysv64" => CallingConvention::Sysv64,
            "aapcs" => CallingConvention::Aapcs,
            "stdcall" => CallingConvention::Stdcall,
            "fastcall" => CallingConvention::Fastcall,
            "vectorcall" => CallingConvention::Vectorcall,
            _ => return None,
        })
    }
}

impl Func {
    pub fn has_convention(&self, convention: CallingConvention) -> bool {
        self.conventions.iter().any(|&func_cc| {
            (func_cc == CallingConvention::All && convention != CallingConvention::Handwritten)
                || func_cc == convention
        })
    }
}

impl Test {
    pub fn has_convention(&self, convention: CallingConvention) -> bool {
        self.funcs
            .iter()
            .any(|func| func.has_convention(convention))
    }
}
