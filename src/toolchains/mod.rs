use std::fmt::Write;
use std::sync::Arc;

use crate::error::*;
use crate::harness::test::*;

use camino::Utf8Path;
use kdl_script::PunEnv;

pub mod c;
pub mod rust;

pub use c::CcToolchain;
pub use rust::RustcToolchain;

pub static TOOLCHAIN_RUSTC: &str = "rustc";
pub static TOOLCHAIN_CC: &str = "cc";
pub static TOOLCHAIN_GCC: &str = "gcc";
pub static TOOLCHAIN_CLANG: &str = "clang";
pub static TOOLCHAIN_MSVC: &str = "msvc";

/// A compiler/language toolchain!
pub trait Toolchain {
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
