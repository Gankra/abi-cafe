//! Rust(c) codegen backend backend

mod declare;
mod init;
mod write;

use camino::Utf8Path;
use kdl_script::types::{Func, FuncIdx, TyIdx};
use kdl_script::PunEnv;
use std::fmt::Write;
use std::sync::Arc;

use super::super::*;
use super::*;
use crate::fivemat::Fivemat;
use crate::vals::ArgValuesIter;

pub static RUST_TEST_PREFIX: &str = include_str!("../../harness/rust_test_prefix.rs");

const VAR_CALLER_INPUTS: &str = "CALLER_INPUTS";
const VAR_CALLER_OUTPUTS: &str = "CALLER_OUTPUTS";
const VAR_CALLEE_INPUTS: &str = "CALLEE_INPUTS";
const VAR_CALLEE_OUTPUTS: &str = "CALLEE_OUTPUTS";
const INDENT: &str = "    ";

#[allow(dead_code)]
pub struct RustcAbiImpl {
    is_nightly: bool,
    codegen_backend: Option<String>,
}

impl AbiImpl for RustcAbiImpl {
    fn name(&self) -> &'static str {
        "rustc"
    }
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
        let mut cmd = Command::new("rustc");
        cmd.arg("--crate-type")
            .arg("staticlib")
            .arg("--out-dir")
            .arg(out_dir)
            .arg("--target")
            .arg(built_info::TARGET)
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

    fn generate_callee(&self, f: &mut dyn Write, mut test: TestImpl) -> Result<(), GenerateError> {
        let mut f = Fivemat::new(f, INDENT);
        self.generate_callee_impl(&mut f, &mut test)
    }

    fn generate_caller(&self, f: &mut dyn Write, mut test: TestImpl) -> Result<(), GenerateError> {
        let mut f = Fivemat::new(f, INDENT);
        self.generate_caller_impl(&mut f, &mut test)
    }
}

impl RustcAbiImpl {
    pub fn generate_caller_impl(
        &self,
        f: &mut Fivemat,
        state: &mut TestImpl,
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

    fn generate_caller_externs(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
    ) -> Result<(), GenerateError> {
        let convention_decl = self.convention_decl(state.options.convention)?;
        writeln!(f, "extern \"{convention_decl}\" {{",)?;
        f.add_indent(1);
        for &func in &state.desired_funcs {
            self.generate_signature(f, state, func)?;
            writeln!(f, ";")?;
        }
        f.sub_indent(1);
        writeln!(f, "}}")?;
        writeln!(f)?;
        Ok(())
    }

    fn generate_caller_body(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        writeln!(f, "unsafe {{")?;
        f.add_indent(1);
        let function = state.types.realize_func(func);

        // Create vars for all the inputs
        let mut func_vals = state.vals.at_func(func);
        for arg in &function.inputs {
            let arg_vals: ArgValuesIter = func_vals.next_arg();
            // Create and report the input
            self.create_var(f, state, &arg.name, arg.ty, arg_vals.clone())?;
            self.write_var(f, state, &arg.name, arg.ty, arg_vals, VAR_CALLER_INPUTS)?;
        }

        // Call the function
        self.call_function(f, state, function)?;

        // Report all the outputs
        for arg in &function.outputs {
            let arg_vals: ArgValuesIter = func_vals.next_arg();

            self.write_var(f, state, &arg.name, arg.ty, arg_vals, VAR_CALLER_OUTPUTS)?;
        }

        // Report the function is complete
        self.write_end_function(f, state, VAR_CALLER_INPUTS, VAR_CALLER_OUTPUTS)?;
        f.sub_indent(1);
        writeln!(f, "}}")?;
        Ok(())
    }

    fn call_function(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
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
            self.pass_var(f, state, &arg.name, arg.ty)?;
        }
        writeln!(f, ");")?;
        writeln!(f)?;
        Ok(())
    }
}

impl RustcAbiImpl {
    pub fn generate_callee_impl(
        &self,
        f: &mut Fivemat,
        state: &mut TestImpl,
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
        state: &TestImpl,
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

        // Report the inputs
        let mut func_vals = state.vals.at_func(func);
        for arg in &function.inputs {
            let arg_vals = func_vals.next_arg();
            let arg_name = &arg.name;
            self.write_var(f, state, arg_name, arg.ty, arg_vals, VAR_CALLEE_INPUTS)?;
        }

        // Create outputs and report them
        for arg in &function.outputs {
            let arg_vals = func_vals.next_arg();
            self.create_var(f, state, &arg.name, arg.ty, arg_vals.clone())?;
            self.write_var(f, state, &arg.name, arg.ty, arg_vals, VAR_CALLEE_OUTPUTS)?;
        }

        // Report the function is complete
        self.write_end_function(f, state, VAR_CALLEE_INPUTS, VAR_CALLEE_OUTPUTS)?;

        // Return the outputs
        self.check_returns(state, function)?;
        for arg in function.outputs.iter() {
            self.return_var(f, state, &arg.name, arg.ty)?;
        }
        f.sub_indent(1);
        writeln!(f, "}}")?;
        f.sub_indent(1);
        writeln!(f, "}}")?;
        Ok(())
    }
}

impl RustcAbiImpl {
    pub fn new(_system_info: &Config, codegen_backend: Option<String>) -> Self {
        Self {
            is_nightly: built_info::RUSTC_VERSION.contains("nightly"),
            codegen_backend,
        }
    }

    fn pass_var(
        &self,
        f: &mut dyn Write,
        _state: &TestImpl,
        var_name: &str,
        _var_ty: TyIdx,
    ) -> Result<(), GenerateError> {
        write!(f, "{var_name}")?;
        Ok(())
    }

    fn return_var(
        &self,
        f: &mut dyn Write,
        _state: &TestImpl,
        var_name: &str,
        _var_ty: TyIdx,
    ) -> Result<(), GenerateError> {
        writeln!(f, "{var_name}")?;
        Ok(())
    }

    fn check_returns(&self, state: &TestImpl, function: &Func) -> Result<(), GenerateError> {
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
