pub mod c;
pub mod rust;

use std::{fmt::Write, path::Path, sync::Arc};

pub use c::CcAbiImpl;
use kdl_script::{DefinitionGraph, KdlScriptError, PunEnv, TypedProgram};
pub use rust::RustcAbiImpl;

use crate::BuildError;

pub static ABI_IMPL_RUSTC: &str = "rustc";
pub static ABI_IMPL_CC: &str = "cc";
pub static ABI_IMPL_GCC: &str = "gcc";
pub static ABI_IMPL_CLANG: &str = "clang";
pub static ABI_IMPL_MSVC: &str = "msvc";

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

pub static OUTPUT_NAME: &str = "output";

pub struct Test {
    pub name: String,
    pub program: Arc<TypedProgram>,
}

#[derive(Debug, Clone)]
pub struct TestVariant {
    pub env: Arc<PunEnv>,
    pub graph: Arc<DefinitionGraph>,
}

/// ABI is probably a bad name for this... it's like, a language/compiler impl. idk.
pub trait AbiImpl {
    fn name(&self) -> &'static str;
    fn lang(&self) -> &'static str;
    fn src_ext(&self) -> &'static str;
    fn supports_convention(&self, _convention: CallingConvention) -> bool;
    fn pun_env(&self) -> Arc<PunEnv>;
    fn generate_callee(
        &self,
        f: &mut dyn Write,
        test: &Test,
        variant: &TestVariant,
        convention: CallingConvention,
    ) -> Result<(), GenerateError>;
    fn generate_caller(
        &self,
        f: &mut dyn Write,
        test: &Test,
        variant: &TestVariant,
        convention: CallingConvention,
    ) -> Result<(), GenerateError>;

    fn compile_callee(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError>;
    fn compile_caller(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError>;
}

impl Test {
    pub fn has_convention(&self, convention: CallingConvention) -> bool {
        // TODO
        true
    }
    pub fn abi_variant(
        &self,
        abi: &(dyn AbiImpl + Send + Sync),
    ) -> Result<TestVariant, GenerateError> {
        let env = abi.pun_env();
        let graph = Arc::new(self.program.definition_graph(&env)?);
        Ok(TestVariant { env, graph })
    }
}

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum GenerateError {
    #[error("io error\n{0}")]
    Fmt(#[from] std::fmt::Error),
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
    #[error("kdl-script error {0}")]
    KdlScriptError(#[from] kdl_script::KdlScriptError),
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
