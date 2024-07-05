use std::fmt::Write;
use std::sync::Arc;

use crate::harness::test::*;
use crate::{error::*, SortedMap};

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

/// All the toolchains
pub type Toolchains = SortedMap<String, Arc<dyn Toolchain + Send + Sync>>;

/// Create all the toolchains
pub(crate) fn create_toolchains(cfg: &crate::Config) -> Toolchains {
    let mut toolchains = Toolchains::default();

    add_toolchain(
        &mut toolchains,
        TOOLCHAIN_RUSTC,
        RustcToolchain::new(cfg, None),
    );
    add_toolchain(
        &mut toolchains,
        TOOLCHAIN_CC,
        CcToolchain::new(cfg, TOOLCHAIN_CC),
    );
    add_toolchain(
        &mut toolchains,
        TOOLCHAIN_GCC,
        CcToolchain::new(cfg, TOOLCHAIN_GCC),
    );
    add_toolchain(
        &mut toolchains,
        TOOLCHAIN_CLANG,
        CcToolchain::new(cfg, TOOLCHAIN_CLANG),
    );
    add_toolchain(
        &mut toolchains,
        TOOLCHAIN_MSVC,
        CcToolchain::new(cfg, TOOLCHAIN_MSVC),
    );

    for (name, path) in &cfg.rustc_codegen_backends {
        add_toolchain(
            &mut toolchains,
            name,
            RustcToolchain::new(cfg, Some(path.to_owned())),
        );
    }

    toolchains
}

/// Register a toolchain
fn add_toolchain<A: Toolchain + Send + Sync + 'static>(
    toolchains: &mut Toolchains,
    id: impl Into<ToolchainId>,
    toolchain: A,
) {
    let id = id.into();
    let old = toolchains.insert(id.clone(), Arc::new(toolchain));
    assert!(old.is_none(), "duplicate toolchain id: {}", id);
}