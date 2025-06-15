//! Rust(c) codegen backend backend

mod declare;
mod init;
mod write;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use kdl_script::types::*;
use kdl_script::PunEnv;
use std::collections::HashMap;
use std::fmt::Write;
use std::str::FromStr;
use std::sync::Arc;

use super::super::*;
use super::*;
use crate::fivemat::Fivemat;
use crate::vals::ArgValuesIter;

const CALLER_VALS: &str = "CALLER_VALS";
const CALLEE_VALS: &str = "CALLEE_VALS";
const INDENT: &str = "    ";

pub struct TestState {
    pub inner: TestImpl,
    // interning state
    pub desired_funcs: Vec<FuncIdx>,
    pub tynames: HashMap<TyIdx, String>,
    pub borrowed_tynames: HashMap<TyIdx, String>,
}
impl std::ops::Deref for TestState {
    type Target = TestImpl;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl TestState {
    fn new(inner: TestImpl) -> Self {
        let desired_funcs = inner.options.functions.active_funcs(&inner.types);
        Self {
            inner,
            desired_funcs,
            tynames: Default::default(),
            borrowed_tynames: Default::default(),
        }
    }
}

#[allow(dead_code)]
pub struct RustcToolchain {
    /// What command should we invoke rustc from?
    command: Utf8PathBuf,
    /// The rustc version
    version: String,
    /// Is this a nightly rustc?
    is_nightly: bool,
    /// Info about the host platform
    pub platform_info: PlatformInfo,
    /// Windowsy or Unixy?
    platform: Platform,
    /// What codegen backend are we using?
    codegen_backend: Option<String>,
}

#[derive(PartialEq)]
enum Platform {
    Windows,
    Unixy,
}

impl Toolchain for RustcToolchain {
    fn lang(&self) -> &'static str {
        "rust"
    }
    fn src_ext(&self) -> &'static str {
        "rs"
    }
    fn pun_env(&self) -> Arc<PunEnv> {
        Arc::new(kdl_script::PunEnv {
            lang: "rust".to_string(),
        })
    }
    fn compile_callee(
        &self,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<String, BuildError> {
        let mut cmd = Command::new(&self.command);
        cmd.arg("--crate-type")
            .arg("staticlib")
            .arg("--out-dir")
            .arg(out_dir)
            .arg("--target")
            .arg(&self.platform_info.target)
            .arg(format!("-Cmetadata={lib_name}"))
            .arg(src_path);
        if let Some(codegen_backend) = &self.codegen_backend {
            cmd.arg(format!("-Zcodegen-backend={codegen_backend}"));
        }
        debug!("running: {:?}", cmd);
        let out = cmd.output()?;

        if !out.status.success() {
            Err(BuildError::RustCompile(out))
        } else {
            Ok(String::from(lib_name))
        }
    }

    fn compile_caller(
        &self,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<String, BuildError> {
        // Currently no need to be different
        self.compile_callee(src_path, out_dir, lib_name)
    }

    fn generate_callee(&self, f: &mut dyn Write, test: TestImpl) -> Result<(), GenerateError> {
        let mut f = Fivemat::new(f, INDENT);
        let mut state = TestState::new(test);
        self.generate_callee_impl(&mut f, &mut state)
    }

    fn generate_caller(&self, f: &mut dyn Write, test: TestImpl) -> Result<(), GenerateError> {
        let mut f = Fivemat::new(f, INDENT);
        let mut state = TestState::new(test);
        self.generate_caller_impl(&mut f, &mut state)
    }
}

impl RustcToolchain {
    pub fn generate_caller_impl(
        &self,
        f: &mut Fivemat,
        state: &mut TestState,
    ) -> Result<(), GenerateError> {
        // Generate type decls and gather up functions
        self.generate_definitions(f, state)?;
        // Generate decls of the functions we want to call
        self.generate_caller_externs(f, state)?;

        // Generate the test function the harness will call
        writeln!(f, "#[no_mangle]\npub extern \"C\" fn do_test() {{")?;
        for &func in &state.desired_funcs {
            // Generate the individual function calls
            self.generate_caller_body(f, state, func)?;
        }
        writeln!(f, "}}")?;

        Ok(())
    }

    fn generate_caller_body(
        &self,
        f: &mut Fivemat,
        state: &TestState,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        writeln!(f, "unsafe {{")?;
        f.add_indent(1);
        let function = state.types.realize_func(func);

        // Report we're starting a function
        self.write_set_function(f, state, CALLER_VALS, func)?;

        // Create vars for all the inputs
        let mut func_vals = state.vals.at_func(func);
        for arg in &function.inputs {
            let arg_vals: ArgValuesIter = func_vals.next_arg();
            // Create and report the input
            self.init_var(f, state, &arg.name, arg.ty, arg_vals.clone())?;
            self.write_var(f, state, &arg.name, arg.ty, arg_vals, CALLER_VALS)?;
        }

        // Call the function
        self.call_function(f, state, function)?;

        // Report all the outputs
        for arg in &function.outputs {
            let arg_vals: ArgValuesIter = func_vals.next_arg();

            self.write_var(f, state, &arg.name, arg.ty, arg_vals, CALLER_VALS)?;
        }

        f.sub_indent(1);
        writeln!(f, "}}")?;
        Ok(())
    }

    fn call_function(
        &self,
        f: &mut Fivemat,
        state: &TestState,
        function: &Func,
    ) -> Result<(), GenerateError> {
        let func_name = &function.name;

        // make sure the outputs aren't weird
        self.check_returns(state, function)?;
        if let Some(output) = function.outputs.first() {
            write!(f, "let {} = ", output.name)?;
        }

        // Call the function
        write!(f, "{func_name}(")?;
        let inputs = function.inputs.iter();

        for (arg_idx, arg) in inputs.enumerate() {
            if arg_idx > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", arg.name)?;
        }
        writeln!(f, ");")?;
        writeln!(f)?;
        Ok(())
    }
}

impl RustcToolchain {
    pub fn generate_callee_impl(
        &self,
        f: &mut Fivemat,
        state: &mut TestState,
    ) -> Result<(), GenerateError> {
        // Generate type decls and gather up functions
        self.generate_definitions(f, state)?;

        for &func in &state.desired_funcs {
            // Generate the individual function definitions
            self.generate_callee_body(f, state, func)?;
        }
        Ok(())
    }

    fn generate_callee_body(
        &self,
        f: &mut Fivemat,
        state: &TestState,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        let function = state.types.realize_func(func);
        let convention_decl = self.convention_decl(state.options.convention)?;
        writeln!(f, "#[no_mangle]")?;
        write!(f, "pub unsafe extern \"{convention_decl}\" ")?;
        self.generate_signature(f, state, func)?;
        writeln!(f, " {{")?;
        f.add_indent(1);
        writeln!(f, "unsafe {{")?;
        f.add_indent(1);

        // Report we're starting a function
        self.write_set_function(f, state, CALLEE_VALS, func)?;

        // Report the inputs
        let mut func_vals = state.vals.at_func(func);
        for arg in &function.inputs {
            let arg_vals = func_vals.next_arg();
            let arg_name = &arg.name;
            self.write_var(f, state, arg_name, arg.ty, arg_vals, CALLEE_VALS)?;
        }

        // Create outputs and report them
        for arg in &function.outputs {
            let arg_vals = func_vals.next_arg();
            self.init_var(f, state, &arg.name, arg.ty, arg_vals.clone())?;
            self.write_var(f, state, &arg.name, arg.ty, arg_vals, CALLEE_VALS)?;
        }

        // Return the outputs
        self.check_returns(state, function)?;
        if let Some(arg) = function.outputs.first() {
            writeln!(f, "{}", arg.name)?;
        }
        f.sub_indent(1);
        writeln!(f, "}}")?;
        f.sub_indent(1);
        writeln!(f, "}}")?;
        Ok(())
    }
}

impl RustcToolchain {
    pub fn new(_system_info: &Config, command: &Utf8Path, codegen_backend: Option<String>) -> Self {
        // Get rustc's version and host
        let rustc_info = Command::new(command)
            .arg("-Vv")
            .output()
            .expect("rustc -vV failed to run");
        let rustc_info_stdout = String::from_utf8(rustc_info.stdout).unwrap();
        let mut version = None;
        let mut host = None;
        for line in rustc_info_stdout.lines() {
            if let Some(val) = line.strip_prefix("host: ") {
                host = Some(val.to_owned());
            }
            if let Some(line) = line.strip_prefix("rustc ") {
                if let Some((val, _rest)) = line.split_once(' ') {
                    version = Some(val.to_owned())
                }
            }
        }
        let version = version.expect("failed to get rustc version");
        let host = host.expect("failed to get rustc host triple");
        let is_nightly = version.contains("nightly");

        // Get rustc's cfgs for the platform we're interested in
        // (Yes we don't have to pass `--target` because host but showing how we can get *any*)
        let rustc_cfgs = Command::new(command)
            .arg("--print=cfg")
            .arg(format!("--target={host}"))
            .output()
            .expect("rustc failed to run");
        let rustc_cfgs_stdout = String::from_utf8(rustc_cfgs.stdout).unwrap();
        let cfgs = rustc_cfgs_stdout
            .lines()
            .map(|line| cargo_platform::Cfg::from_str(line).expect("failed to parse rustc cfg"))
            .collect::<Vec<_>>();
        let is_windowsy = cfgs.contains(
            &cargo_platform::Cfg::from_str("windows").expect("failed to parse windows cfg"),
        );

        let platform = if is_windowsy {
            Platform::Windows
        } else {
            Platform::Unixy
        };

        Self {
            command: command.to_owned(),
            version,
            is_nightly,
            platform_info: PlatformInfo { target: host, cfgs },
            platform,
            codegen_backend,
        }
    }

    fn check_returns(&self, state: &TestState, function: &Func) -> Result<(), GenerateError> {
        let has_outparams = function
            .outputs
            .iter()
            .any(|arg| state.types.ty_contains_ref(arg.ty));
        if has_outparams {
            return Err(UnsupportedError::Other(
                "outparams (outputs containing references) aren't supported".to_owned(),
            ))?;
        }
        if function.outputs.len() > 1 {
            return Err(UnsupportedError::Other(
                "multiple returns (should this be a struct?)".to_owned(),
            ))?;
        }
        Ok(())
    }
}
