pub mod c;
pub mod rust;
pub mod vals;

pub use c::CcAbiImpl;
pub use rust::RustcAbiImpl;

use std::{collections::HashMap, fmt::Write, sync::Arc};

use camino::Utf8Path;
use kdl_script::{
    types::{FuncIdx, TyIdx},
    DefinitionGraph, PunEnv, TypedProgram,
};
use serde::Serialize;
use vals::{ValueGeneratorKind, ValueTree};

use crate::{
    error::{BuildError, GenerateError},
    CliParseError,
};

pub type AbiImplId = String;
pub type TestId = String;

pub static ABI_IMPL_RUSTC: &str = "rustc";
pub static ABI_IMPL_CC: &str = "cc";
pub static ABI_IMPL_GCC: &str = "gcc";
pub static ABI_IMPL_CLANG: &str = "clang";
pub static ABI_IMPL_MSVC: &str = "msvc";

pub static ALL_CONVENTIONS: &[CallingConvention] = &[
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

/// A test case, fully abstract.
///
/// An abi-cafe Test is essentially a series of function signatures
/// that we're interested in testing. That is, we want to generate a
/// caller and a callee that implement the signature, and check that
/// both sides agree on the values that were passed between them
/// (implying the two implementations agree on the ABI for that signature).
///
/// To describe these signatures, we use a toy programming language called
/// [kdl-script][], which was designed explicitly for the purpose of declaring
/// type definitions and function signatures, without mandating a specific impl.
///
/// At this point we have parsed and typechecked the kdl-script program,
/// giving us the signatures but no specific compiler/language to lower them to.
///
/// Notably, at this level of abtraction kdl-script [Pun Types][pun-types] are
/// still unresolved. You can think of these as types wrapped in
/// an `ifdef`/`#[cfg]`, allowing a test program to declare that
/// two different compilers/languages have fundamentally different
/// understandings of the *shape* of a type, but are still expected
/// to interopate if a function signature puns them.
///
/// [kdl-script]: https://github.com/Gankra/kdl-script
/// [pun-types]: https://github.com/Gankra/kdl-script/blob/main/README.md#pun-types
#[derive(Debug, Clone)]
pub struct Test {
    /// Name of the test (file stem)
    pub name: String,
    /// Parsed and Typechecked kdl-script program
    pub types: Arc<TypedProgram>,
}

#[derive(Debug, Clone)]
pub struct TestWithVals {
    pub inner: Arc<Test>,
    /// Values that the test should have
    pub vals: Arc<ValueTree>,
}
impl std::ops::Deref for TestWithVals {
    type Target = Test;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Options for a test
#[derive(Clone, Debug, Serialize)]
pub struct TestOptions {
    /// The calling convention
    pub convention: CallingConvention,
    pub functions: FunctionSelector,
    pub val_writer: WriteImpl,
    pub val_generator: ValueGeneratorKind,
}
impl FunctionSelector {
    pub fn should_write_arg(&self, func_idx: usize, arg_idx: usize) -> bool {
        match &self {
            FunctionSelector::All => true,
            FunctionSelector::One { idx, args } => {
                if func_idx != *idx {
                    false
                } else {
                    match args {
                        ArgSelector::All => true,
                        ArgSelector::One { idx, vals: _ } => arg_idx == *idx,
                    }
                }
            }
        }
    }
    pub fn should_write_val(&self, func_idx: usize, arg_idx: usize, val_idx: usize) -> bool {
        match &self {
            FunctionSelector::All => true,
            FunctionSelector::One { idx, args } => {
                if func_idx != *idx {
                    false
                } else {
                    match args {
                        ArgSelector::All => true,
                        ArgSelector::One { idx, vals } => {
                            if arg_idx != *idx {
                                false
                            } else {
                                match vals {
                                    ValSelector::All => true,
                                    ValSelector::One { idx } => val_idx == *idx,
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn active_funcs(&self, types: &TypedProgram) -> Vec<FuncIdx> {
        match self {
            FunctionSelector::All => types.all_funcs().collect(),
            FunctionSelector::One { idx, args: _ } => vec![*idx],
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum FunctionSelector {
    All,
    One { idx: FuncIdx, args: ArgSelector },
}

#[derive(Clone, Debug, Serialize)]
pub enum ArgSelector {
    All,
    One { idx: usize, vals: ValSelector },
}

#[derive(Clone, Debug, Serialize)]
pub enum ValSelector {
    All,
    One { idx: usize },
}

#[derive(Copy, Clone, Debug)]
pub enum CallSide {
    Caller,
    Callee,
}
impl CallSide {
    pub fn name(&self) -> &'static str {
        match self {
            CallSide::Caller => "caller",
            CallSide::Callee => "callee",
        }
    }
}
impl std::fmt::Display for CallSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name().fmt(f)
    }
}

/// A test case, specialized to a specific ABI (PunEnv)
///
/// This refines a [`Test`][] with a specific [`AbiImpl`][] like "Rust (rustc)" or "C (gcc)".
/// The [`PunEnv`][] describes how the AbiImpl wishes to resolve any "Pun Types".
///
/// The [`DefinitionGraph`][] provides a DAG of the type/function
/// definitions that result from applying the PunEnv to the Program.
/// This can only be computed once we know how to resolve Puns because
/// an ifdef can completely change which types are referenced.
///
/// This DAG is queried with a list of functions we're interested
/// in generating code for, producing a topological sort of the type
/// and function declarations so each [`AbiImpl`][] doesn't need to work that out.
///
/// Typically the query is "all functions", because we want to test everything.
/// However if a test fails we can requery with "just this one failing function"
/// to generate a minimized test-case for debugging/reporting.
#[derive(Debug, Clone)]
pub struct TestWithAbi {
    pub inner: Arc<TestWithVals>,
    pub env: Arc<PunEnv>,
    pub defs: Arc<DefinitionGraph>,
}
impl std::ops::Deref for TestWithAbi {
    type Target = TestWithVals;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// A test case, fully specialized to specify:
///
/// * What [`AbiImpl`][] (compiler/language) we're using
/// * What [`CallingConvention`] we're using
/// * Which functions we're generating (usually "all of them")
/// * How to [display/report][`WriteImpl`] values (callbacks vs print vs noop)
/// * Whether we're generating the callee or caller (currently implicit)
///
/// This also contains some utilities for interning compute type names/expressions.
#[derive(Debug, Clone)]
pub struct TestImpl {
    pub inner: Arc<TestWithAbi>,
    pub options: TestOptions,
}
impl std::ops::Deref for TestImpl {
    type Target = TestWithAbi;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq)]
pub enum WriteImpl {
    HarnessCallback,
    Assert,
    Print,
    Noop,
}
impl std::str::FromStr for WriteImpl {
    type Err = CliParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "harness" => Ok(Self::HarnessCallback),
            "assert" => Ok(Self::Assert),
            "print" => Ok(Self::Print),
            "noop" => Ok(Self::Noop),
            _ => Err(CliParseError::Other(format!("{s} is not a write impl"))),
        }
    }
}
impl std::fmt::Display for WriteImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::HarnessCallback => "harness",
            Self::Assert => "assert",
            Self::Print => "print",
            Self::Noop => "noop",
        };
        s.fmt(f)
    }
}

/// ABI is probably a bad name for this... it's like, a language/compiler impl. idk.
pub trait AbiImpl {
    fn lang(&self) -> &'static str;
    fn src_ext(&self) -> &'static str;
    fn pun_env(&self) -> Arc<PunEnv>;
    fn generate_callee(&self, f: &mut dyn Write, test: TestImpl) -> Result<(), GenerateError>;
    fn generate_caller(&self, f: &mut dyn Write, test: TestImpl) -> Result<(), GenerateError>;

    fn compile_callee(
        &self,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<String, BuildError>;
    fn compile_caller(
        &self,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<String, BuildError>;
}

impl Test {
    pub fn has_convention(&self, _convention: CallingConvention) -> bool {
        true
    }

    pub async fn with_vals(
        self: &Arc<Self>,
        vals: ValueGeneratorKind,
    ) -> Result<Arc<TestWithVals>, GenerateError> {
        let vals = Arc::new(ValueTree::new(&self.types, vals)?);
        Ok(Arc::new(TestWithVals {
            inner: self.clone(),
            vals,
        }))
    }
}

impl TestWithVals {
    pub async fn with_abi(
        self: &Arc<Self>,
        abi: &(dyn AbiImpl + Send + Sync),
    ) -> Result<Arc<TestWithAbi>, GenerateError> {
        let env = abi.pun_env();
        let defs = Arc::new(self.types.definition_graph(&env)?);
        Ok(Arc::new(TestWithAbi {
            inner: self.clone(),
            env,
            defs,
        }))
    }
}

impl TestWithAbi {
    pub fn with_options(self: &Arc<Self>, options: TestOptions) -> Result<TestImpl, GenerateError> {
        Ok(TestImpl {
            inner: self.clone(),
            options,
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename = "lowercase")]
pub enum CallingConvention {
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
}

impl std::fmt::Display for CallingConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name().fmt(f)
    }
}

impl std::str::FromStr for CallingConvention {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = match s {
            "c" => CallingConvention::C,
            "cdecl" => CallingConvention::Cdecl,
            "system" => CallingConvention::System,
            "win64" => CallingConvention::Win64,
            "sysv64" => CallingConvention::Sysv64,
            "aapcs" => CallingConvention::Aapcs,
            "stdcall" => CallingConvention::Stdcall,
            "fastcall" => CallingConvention::Fastcall,
            "vectorcall" => CallingConvention::Vectorcall,
            _ => return Err(format!("unknown CallingConvention: {s}")),
        };
        Ok(val)
    }
}
