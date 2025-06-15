use std::fmt::Write;
use std::sync::Arc;

use crate::harness::test::*;
use crate::{error::*, SortedMap};

use camino::{Utf8Path, Utf8PathBuf};
use kdl_script::PunEnv;

pub mod c;
pub mod rust;

pub use c::CcToolchain;
pub use rust::RustcToolchain;

pub const TOOLCHAIN_RUSTC: &str = "rustc";
pub const TOOLCHAIN_CC: &str = "cc";
pub const TOOLCHAIN_GCC: &str = "gcc";
pub const TOOLCHAIN_CLANG: &str = "clang";
pub const TOOLCHAIN_MSVC: &str = "msvc";

const C_TOOLCHAINS: &[&str] = &[TOOLCHAIN_CC, TOOLCHAIN_GCC, TOOLCHAIN_CLANG, TOOLCHAIN_MSVC];

/// A compiler/language toolchain!
pub trait Toolchain {
    #[allow(dead_code)]
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
pub struct Toolchains {
    pub platform_info: PlatformInfo,
    pub rustc_command: Utf8PathBuf,
    pub toolchains: ToolchainMap,
}
pub type ToolchainMap = SortedMap<String, Arc<dyn Toolchain + Send + Sync>>;

#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// Platform we're targetting
    pub target: String,
    /// Enabled rustc cfgs, used for our own test harness cfgs
    pub cfgs: Vec<cargo_platform::Cfg>,
}

/// Create all the toolchains
pub(crate) fn create_toolchains(cfg: &crate::Config) -> Toolchains {
    let mut toolchains = ToolchainMap::default();

    let rustc_command: Utf8PathBuf = "rustc".into();
    let base_rustc = RustcToolchain::new(cfg, &rustc_command, None);
    let platform_info = base_rustc.platform_info.clone();

    // Set up env vars for CC
    std::env::set_var("OUT_DIR", &cfg.paths.out_dir);
    std::env::set_var("HOST", platform_info.target.clone());
    std::env::set_var("TARGET", platform_info.target.clone());
    std::env::set_var("OPT_LEVEL", "0");

    // Add rust toolchains
    add_toolchain(&mut toolchains, TOOLCHAIN_RUSTC, base_rustc);
    for (name, path) in &cfg.rustc_codegen_backends {
        add_toolchain(
            &mut toolchains,
            name,
            RustcToolchain::new(cfg, &rustc_command, Some(path.to_owned())),
        );
    }

    // Add c toolchains
    for &name in C_TOOLCHAINS {
        add_toolchain(
            &mut toolchains,
            name,
            CcToolchain::new(cfg, &platform_info.target, name),
        );
    }

    Toolchains {
        platform_info,
        rustc_command,
        toolchains,
    }
}

/// Register a toolchain
fn add_toolchain<A: Toolchain + Send + Sync + 'static>(
    toolchains: &mut ToolchainMap,
    id: impl Into<ToolchainId>,
    toolchain: A,
) {
    let id = id.into();
    let old = toolchains.insert(id.clone(), Arc::new(toolchain));
    assert!(old.is_none(), "duplicate toolchain id: {}", id);
}
