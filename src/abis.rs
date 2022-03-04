#![allow(non_camel_case_types)]

use super::BuildError;
use std::io::Write;
use std::path::Path;

pub mod c;
pub mod rust;

pub type AbiRef = &'static (dyn Abi + Sync);

pub static RUST_ABI: AbiRef = &rust::RustAbi;
pub static C_ABI: AbiRef = &c::CAbi;

pub static TEST_PAIRS: &[(AbiRef, AbiRef)] = &[(RUST_ABI, C_ABI), (C_ABI, RUST_ABI)];

pub trait Abi {
    fn name(&self) -> &'static str;
    fn src_ext(&self) -> &'static str;
    fn generate_callee(&self, f: &mut dyn Write, test: &Test) -> Result<(), BuildError>;
    fn generate_caller(&self, f: &mut dyn Write, test: &Test) -> Result<(), BuildError>;
    fn compile_callee(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError>;
    fn compile_caller(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError>;
}

#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    #[error("rust didn't support features of this test")]
    RustUnsupported,
    #[error("c didn't support features of this test")]
    CUnsupported,
    #[error("the function didn't have a valid convention")]
    NoCallingConvention,
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

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum CallingConvention {
    /// Sugar for "every possible convention"
    All,
    /// A complete opaque convention, the implementation must be manually
    /// written in the handwritten_impls directory.
    Handwritten,
    /// The platform's default C convention
    C,
    // TODO: more specific CC's like stdcall, fastcall, thiscall, ...
}

/// A typed value.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Val {
    /// A Ref is passed-by-reference (is a pointer) but the
    /// pointee will be regarded as the real value that we check.
    Ref(Box<Val>),
    /// Some integer
    Int(IntVal),
    /// Some float
    Float(FloatVal),
    /// A bool
    Bool(bool),
    /// An array (homogeneous types, checked on construction)
    Array(Vec<Val>),
    /// A named struct (heterogeneous type)
    Struct(String, Vec<Val>),
    /// An opaque pointer
    Ptr(u64),
    // TODO: vectors
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
    // TODO: nastier c-types?
    // c_int(i64),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum FloatVal {
    c_double(f64),
    c_float(f32),
}
