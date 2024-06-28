//! C codegen backend backend

mod declare;
mod init;
mod write;

use camino::Utf8Path;
use kdl_script::types::{Func, FuncIdx};
use kdl_script::PunEnv;
use std::fmt::Write;
use std::sync::Arc;

use super::super::*;
use super::*;
use crate::fivemat::Fivemat;
use crate::vals::ArgValuesIter;

pub static TEST_PREFIX: &str = include_str!("../../harness/c_test_prefix.h");
pub static HARNESS_PREFIX: &str = include_str!("../../harness/c_harness_prefix.h");

const VAR_CALLER_INPUTS: &str = "CALLER_INPUTS";
const VAR_CALLER_OUTPUTS: &str = "CALLER_OUTPUTS";
const VAR_CALLEE_INPUTS: &str = "CALLEE_INPUTS";
const VAR_CALLEE_OUTPUTS: &str = "CALLEE_OUTPUTS";
const INDENT: &str = "    ";

pub struct TestState {
    pub inner: TestImpl,
    // interning state
    pub desired_funcs: Vec<FuncIdx>,
    pub tynames: HashMap<TyIdx, (String, String)>,
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
        }
    }
}

pub struct CcAbiImpl {
    cc_flavor: CCFlavor,
    platform: Platform,
    mode: &'static str,
}

#[derive(PartialEq)]
enum CCFlavor {
    Clang,
    Gcc,
    Msvc,
}

#[derive(PartialEq)]
enum Platform {
    Windows,
    Unixy,
}

impl AbiImpl for CcAbiImpl {
    fn name(&self) -> &'static str {
        self.mode
    }
    fn lang(&self) -> &'static str {
        "c"
    }
    fn src_ext(&self) -> &'static str {
        "c"
    }

    fn pun_env(&self) -> Arc<PunEnv> {
        Arc::new(kdl_script::PunEnv {
            lang: "c".to_string(),
        })
    }

    fn compile_callee(
        &self,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<String, BuildError> {
        match self.mode {
            "cc" => self.compile_cc(src_path, out_dir, lib_name),
            "gcc" => self.compile_gcc(src_path, out_dir, lib_name),
            "clang" => self.compile_clang(src_path, out_dir, lib_name),
            "msvc" => self.compile_msvc(src_path, out_dir, lib_name),
            _ => unimplemented!("unknown c compiler"),
        }
    }

    fn compile_caller(
        &self,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<String, BuildError> {
        match self.mode {
            "cc" => self.compile_cc(src_path, out_dir, lib_name),
            "gcc" => self.compile_gcc(src_path, out_dir, lib_name),
            "clang" => self.compile_clang(src_path, out_dir, lib_name),
            "msvc" => self.compile_msvc(src_path, out_dir, lib_name),
            _ => unimplemented!("unknown c compiler"),
        }
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

impl CcAbiImpl {
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
        writeln!(f, "void do_test(void) {{")?;
        f.add_indent(1);
        for &func in &state.desired_funcs {
            // Generate the individual function calls
            self.generate_caller_body(f, state, func)?;
        }
        f.sub_indent(1);
        writeln!(f, "}}")?;

        Ok(())
    }

    fn generate_caller_body(
        &self,
        f: &mut Fivemat,
        state: &TestState,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        let function = state.types.realize_func(func);

        // Create vars for all the inputs
        let mut func_vals = state.vals.at_func(func);
        for arg in &function.inputs {
            let arg_vals: ArgValuesIter = func_vals.next_arg();
            // Create and report the input
            self.init_var(f, state, &arg.name, arg.ty, arg_vals.clone())?;
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
        if let Some(arg) = function.outputs.first() {
            let (pre, post) = &state.tynames[&arg.ty];
            write!(f, "{pre}{}{post} = ", arg.name)?;
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

impl CcAbiImpl {
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
        self.generate_signature(f, state, func)?;
        writeln!(f, " {{")?;
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
            self.init_var(f, state, &arg.name, arg.ty, arg_vals.clone())?;
            self.write_var(f, state, &arg.name, arg.ty, arg_vals, VAR_CALLEE_OUTPUTS)?;
        }

        // Report the function is complete
        self.write_end_function(f, state, VAR_CALLEE_INPUTS, VAR_CALLEE_OUTPUTS)?;

        // Return the outputs
        self.check_returns(state, function)?;
        if let Some(arg) = function.outputs.first() {
            writeln!(f, "return {};", arg.name)?;
        }
        f.sub_indent(1);
        writeln!(f, "}}")?;
        Ok(())
    }
}

impl CcAbiImpl {
    pub fn new(_system_info: &Config, mode: &'static str) -> Self {
        let compiler = cc::Build::new().get_compiler();
        let cc_flavor = if compiler.is_like_msvc() {
            CCFlavor::Msvc
        } else if compiler.is_like_gnu() {
            CCFlavor::Gcc
        } else if compiler.is_like_clang() {
            CCFlavor::Clang
        } else {
            panic!("Unknown compiler flavour for CC");
        };

        let platform = if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Unixy
        };

        Self {
            cc_flavor,
            platform,
            mode,
        }
    }

    fn compile_cc(
        &self,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<String, BuildError> {
        cc::Build::new()
            .file(src_path)
            .opt_level(0)
            .cargo_metadata(false)
            .target(built_info::TARGET)
            .out_dir(out_dir)
            // .warnings_into_errors(true)
            .try_compile(lib_name)?;
        Ok(String::from(lib_name))
    }

    fn compile_clang(
        &self,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<String, BuildError> {
        let obj_path = out_dir.join(format!("{lib_name}.o"));
        let lib_path = out_dir.join(format!("lib{lib_name}.a"));
        Command::new("clang")
            .arg("-ffunction-sections")
            .arg("-fdata-sections")
            .arg("-fPIC")
            .arg("-o")
            .arg(&obj_path)
            .arg("-c")
            .arg(&src_path)
            .status()
            .unwrap();
        Command::new("ar")
            .arg("cq")
            .arg(&lib_path)
            .arg(&obj_path)
            .status()
            .unwrap();
        Command::new("ar").arg("s").arg(&lib_path).status().unwrap();
        Ok(String::from(lib_name))
    }

    fn compile_gcc(
        &self,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<String, BuildError> {
        let obj_path = out_dir.join(format!("{lib_name}.o"));
        let lib_path = out_dir.join(format!("lib{lib_name}.a"));
        Command::new("gcc")
            .arg("-ffunction-sections")
            .arg("-fdata-sections")
            .arg("-fPIC")
            .arg("-o")
            .arg(&obj_path)
            .arg("-c")
            .arg(&src_path)
            .status()
            .unwrap();
        Command::new("ar")
            .arg("cq")
            .arg(&lib_path)
            .arg(&obj_path)
            .status()
            .unwrap();
        Command::new("ar").arg("s").arg(&lib_path).status().unwrap();
        Ok(String::from(lib_name))
    }

    fn compile_msvc(
        &self,
        _src_path: &Utf8Path,
        _out_dir: &Utf8Path,
        _lib_name: &str,
    ) -> Result<String, BuildError> {
        unimplemented!()
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
