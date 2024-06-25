use super::super::*;
use super::*;

pub static RUST_TEST_PREFIX: &str = include_str!("../../harness/rust_test_prefix.rs");

static STRUCT_128: bool = false; // cfg!(target_arch="x86_64");

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
    fn supports_convention(&self, convention: CallingConvention) -> bool {
        // NOTE: Rustc spits out:
        //
        // Rust, C, C-unwind, cdecl, stdcall, stdcall-unwind, fastcall,
        // vectorcall, thiscall, thiscall-unwind, aapcs, win64, sysv64,
        // ptx-kernel, msp430-interrupt, x86-interrupt, amdgpu-kernel,
        // efiapi, avr-interrupt, avr-non-blocking-interrupt, C-cmse-nonsecure-call,
        // wasm, system, system-unwind, rust-intrinsic, rust-call,
        // platform-intrinsic, unadjusted
        match convention {
            CallingConvention::All => unreachable!(),
            CallingConvention::Handwritten => true,
            CallingConvention::C => true,
            CallingConvention::Cdecl => true,
            CallingConvention::System => true,
            CallingConvention::Win64 => true,
            CallingConvention::Sysv64 => true,
            CallingConvention::Aapcs => true,
            CallingConvention::Stdcall => true,
            CallingConvention::Fastcall => true,
            CallingConvention::Vectorcall => false, // too experimental even for nightly use?
        }
    }

    fn generate_caller(
        &self,
        f: &mut dyn Write,
        test: &Test,
        convention: CallingConvention,
    ) -> Result<(), GenerateError> {
        self.write_rust_prefix(f, test, convention)?;
        let convention_decl = self.rust_convention_decl(convention);

        // Generate the extern block
        writeln!(f, "extern \"{convention_decl}\" {{",)?;
        for function in &test.funcs {
            write!(f, "  ")?;
            self.write_rust_signature(f, function)?;
            writeln!(f, ";")?;
        }
        writeln!(f, "}}")?;
        writeln!(f)?;

        // Now generate the body
        writeln!(f, "#[no_mangle] pub extern \"C\" fn do_test() {{")?;

        for function in &test.funcs {
            if !function.has_convention(convention) {
                continue;
            }
            writeln!(f, "   unsafe {{")?;

            // Inputs
            for (idx, input) in function.inputs.iter().enumerate() {
                writeln!(
                    f,
                    "        {} = {};",
                    self.rust_var_decl(input, ARG_NAMES[idx])?,
                    self.rust_val(input)?
                )?;
            }
            writeln!(f)?;
            for (idx, input) in function.inputs.iter().enumerate() {
                writeln!(
                    f,
                    "{}",
                    self.rust_write_val(input, "CALLER_INPUTS", ARG_NAMES[idx], true)?
                )?;
            }
            writeln!(f)?;

            // Outputs
            write!(f, "        ")?;
            let pass_out = if let Some(output) = &function.output {
                if let Some(decl) = self.rust_out_param_var(output, OUTPUT_NAME)? {
                    writeln!(f, "        {}", decl)?;
                    true
                } else {
                    write!(f, "        {} = ", self.rust_var_decl(output, OUTPUT_NAME)?)?;
                    false
                }
            } else {
                false
            };

            // Do the call
            write!(f, "{}(", function.name)?;
            for (idx, input) in function.inputs.iter().enumerate() {
                write!(f, "{}, ", self.rust_arg_pass(input, ARG_NAMES[idx])?)?;
            }
            if pass_out {
                writeln!(f, "&mut {OUTPUT_NAME}")?;
            }
            writeln!(f, ");")?;
            writeln!(f)?;

            // Report the output
            if let Some(output) = &function.output {
                writeln!(
                    f,
                    "{}",
                    self.rust_write_val(output, "CALLER_OUTPUTS", OUTPUT_NAME, true)?
                )?;
            }

            // Finished
            writeln!(
                f,
                "        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);"
            )?;
            writeln!(f, "   }}")?;
        }

        writeln!(f, "}}")?;

        Ok(())
    }
    fn generate_callee(
        &self,
        f: &mut dyn Write,
        test: &Test,
        convention: CallingConvention,
    ) -> Result<(), GenerateError> {
        self.write_rust_prefix(f, test, convention)?;
        let convention_decl = self.rust_convention_decl(convention);
        for function in &test.funcs {
            if !function.has_convention(convention) {
                continue;
            }
            // Write the signature
            writeln!(f, "#[no_mangle]")?;
            write!(f, "pub unsafe extern \"{convention_decl}\" ")?;
            self.write_rust_signature(f, function)?;
            writeln!(f, " {{")?;

            // Now the body

            // Report Inputs
            for (idx, input) in function.inputs.iter().enumerate() {
                writeln!(
                    f,
                    "{}",
                    self.rust_write_val(input, "CALLEE_INPUTS", ARG_NAMES[idx], false)?
                )?;
            }
            writeln!(f)?;

            // Report outputs and return
            if let Some(output) = &function.output {
                let decl = self.rust_var_decl(output, OUTPUT_NAME)?;
                let val = self.rust_val(output)?;
                writeln!(f, "        {decl} = {val};")?;
                writeln!(
                    f,
                    "{}",
                    self.rust_write_val(output, "CALLEE_OUTPUTS", OUTPUT_NAME, true)?
                )?;
                writeln!(
                    f,
                    "        FINISHED_FUNC.unwrap()(CALLEE_INPUTS, CALLEE_OUTPUTS);"
                )?;
                writeln!(
                    f,
                    "        {}",
                    self.rust_var_return(output, OUTPUT_NAME, OUT_PARAM_NAME)?
                )?;
            } else {
                writeln!(
                    f,
                    "        FINISHED_FUNC.unwrap()(CALLEE_INPUTS, CALLEE_OUTPUTS);"
                )?;
            }
            writeln!(f, "}}")?;
        }

        Ok(())
    }

    fn compile_callee(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError> {
        let mut cmd = Command::new("rustc");
        cmd.arg("--crate-type")
            .arg("staticlib")
            .arg("--out-dir")
            .arg("target/temp/")
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
    fn compile_caller(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError> {
        // Currently no need to be different
        self.compile_callee(src_path, lib_name)
    }
}

impl RustcAbiImpl {
    pub fn new(_system_info: &Config, codegen_backend: Option<String>) -> Self {
        Self {
            is_nightly: built_info::RUSTC_VERSION.contains("nightly"),
            codegen_backend,
        }
    }

    fn rust_convention_decl(&self, convention: CallingConvention) -> &'static str {
        match convention {
            CallingConvention::All => {
                unreachable!("CallingConvention::All is sugar that shouldn't reach here")
            }
            CallingConvention::Handwritten => {
                unreachable!("CallingConvention::Handwritten shouldn't reach codegen backends!")
            }
            CallingConvention::C => "C",
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

    /// Every test should start by loading in the harness' "header"
    /// and forward-declaring any structs that will be used.
    fn write_rust_prefix(
        &self,
        f: &mut dyn Write,
        test: &Test,
        convention: CallingConvention,
    ) -> Result<(), GenerateError> {
        if convention == CallingConvention::Vectorcall {
            writeln!(f, "#![feature(abi_vectorcall)]")?;
        }
        // Load test harness "headers"
        write!(f, "{}", RUST_TEST_PREFIX)?;

        // Forward-decl struct types
        let mut forward_decls = std::collections::HashMap::<String, String>::new();
        for function in &test.funcs {
            for val in function.inputs.iter().chain(function.output.as_ref()) {
                for (name, decl) in self.rust_forward_decl(val)? {
                    match forward_decls.entry(name) {
                        std::collections::hash_map::Entry::Occupied(entry) => {
                            if entry.get() != &decl {
                                return Err(GenerateError::InconsistentStructDefinition {
                                    name: entry.key().clone(),
                                    old_decl: entry.remove(),
                                    new_decl: decl,
                                });
                            }
                        }
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            writeln!(f, "{decl}")?;
                            entry.insert(decl);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn write_rust_signature(
        &self,
        f: &mut dyn Write,
        function: &Func,
    ) -> Result<(), GenerateError> {
        write!(f, "fn {}(", function.name)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            write!(f, "{}, ", self.rust_arg_decl(input, ARG_NAMES[idx])?)?;
        }
        if let Some(output) = &function.output {
            if let Some(out_param) = self.rust_out_param(output, OUT_PARAM_NAME)? {
                write!(f, "{}", out_param)?;
                write!(f, ")")?;
            } else {
                write!(f, ")")?;
                let ty = self.rust_arg_type(output)?;
                write!(f, " -> {ty}")?;
            }
        } else {
            write!(f, ")")?;
        }
        Ok(())
    }

    /// If this value defines a nominal type, this will spit out:
    ///
    /// * The type name
    /// * The forward-declaration of that type
    ///
    /// To catch buggy test definitions, you should validate that all
    /// structs that claim a particular name have the same declaration.
    /// This is done in write_rust_prefix.
    fn rust_forward_decl(&self, val: &Val) -> Result<Vec<(String, String)>, GenerateError> {
        use Val::*;
        match val {
            Struct(name, fields) => {
                let mut results = vec![];
                for field in fields.iter() {
                    results.extend(self.rust_forward_decl(field)?);
                }
                let mut output = String::new();
                let ref_name = name.to_string();
                output.push_str("\n#[repr(C)]\n");
                output.push_str(&format!("pub struct {name} {{\n"));
                for (idx, field) in fields.iter().enumerate() {
                    let line = format!(
                        "    {}: {},\n",
                        FIELD_NAMES[idx],
                        self.rust_nested_type(field)?
                    );
                    output.push_str(&line);
                }
                output.push('}');
                results.push((ref_name, output));
                Ok(results)
            }
            Array(vals) => self.rust_forward_decl(&vals[0]),
            Ref(pointee) => self.rust_forward_decl(pointee),
            _ => Ok(vec![]),
        }
    }

    /// The decl to use for a local var (reference-ness stripped)
    fn rust_var_decl(&self, val: &Val, var_name: &str) -> Result<String, GenerateError> {
        if let Val::Ref(pointee) = val {
            Ok(self.rust_var_decl(pointee, var_name)?)
        } else {
            Ok(format!("let {var_name}: {}", self.rust_arg_type(val)?))
        }
    }

    /// The decl to use for a function arg (apply referenceness)
    fn rust_arg_decl(&self, val: &Val, arg_name: &str) -> Result<String, GenerateError> {
        if let Val::Ref(pointee) = val {
            Ok(format!("{arg_name}: &{}", self.rust_arg_type(pointee)?))
        } else {
            Ok(format!("{arg_name}: {}", self.rust_arg_type(val)?))
        }
    }

    /// If the return type needs to be an out_param, this returns it
    fn rust_out_param(
        &self,
        val: &Val,
        out_param_name: &str,
    ) -> Result<Option<String>, GenerateError> {
        if let Val::Ref(pointee) = val {
            Ok(Some(format!(
                "{out_param_name}: &mut {}",
                self.rust_arg_type(pointee)?
            )))
        } else {
            Ok(None)
        }
    }

    /// If the return type needs to be an out_param, this returns it
    fn rust_out_param_var(
        &self,
        val: &Val,
        output_name: &str,
    ) -> Result<Option<String>, GenerateError> {
        if let Val::Ref(pointee) = val {
            Ok(Some(format!(
                "let mut {output_name}: {} = {};",
                self.rust_arg_type(pointee)?,
                self.rust_default_val(pointee)?
            )))
        } else {
            Ok(None)
        }
    }

    /// How to pass an argument
    fn rust_arg_pass(&self, val: &Val, arg_name: &str) -> Result<String, GenerateError> {
        if let Val::Ref(_) = val {
            Ok(format!("&{arg_name}"))
        } else {
            Ok(arg_name.to_string())
        }
    }

    /// How to return a value
    fn rust_var_return(
        &self,
        val: &Val,
        var_name: &str,
        out_param_name: &str,
    ) -> Result<String, GenerateError> {
        if let Val::Ref(_) = val {
            Ok(format!("*{out_param_name} = {var_name};"))
        } else {
            Ok(format!("return {var_name};"))
        }
    }

    /// The type name to use for this value when it is stored in args/vars.
    fn rust_arg_type(&self, val: &Val) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let out = match val {
            Ref(pointee) => format!("*mut {}", self.rust_arg_type(pointee)?),
            Ptr(_) => "*mut ()".to_string(),
            Bool(_) => "bool".to_string(),
            Array(vals) => format!("[{}; {}]", self.rust_arg_type(&vals[0])?, vals.len()),
            Struct(name, _) => name.to_string(),
            Float(FloatVal::c_double(_)) => "f64".to_string(),
            Float(FloatVal::c_float(_)) => "f32".to_string(),
            Int(int_val) => match int_val {
                c__int128(_) => {
                    if STRUCT_128 {
                        "FfiI128".to_string()
                    } else {
                        "i128".to_string()
                    }
                }
                c_int64_t(_) => "i64".to_string(),
                c_int32_t(_) => "i32".to_string(),
                c_int16_t(_) => "i16".to_string(),
                c_int8_t(_) => "i8".to_string(),
                c__uint128(_) => {
                    if STRUCT_128 {
                        "FfiU128".to_string()
                    } else {
                        "u128".to_string()
                    }
                }
                c_uint64_t(_) => "u64".to_string(),
                c_uint32_t(_) => "u32".to_string(),
                c_uint16_t(_) => "u16".to_string(),
                c_uint8_t(_) => "u8".to_string(),
            },
        };
        Ok(out)
    }

    /// The type name to use for this value when it is stored in composite.
    ///
    /// This is separated out in case there's a type that needs different
    /// handling in this context to conform to a layout (i.e. how C arrays
    /// decay into pointers when used in function args).
    fn rust_nested_type(&self, val: &Val) -> Result<String, GenerateError> {
        self.rust_arg_type(val)
    }

    /// An expression that generates this value.
    fn rust_val(&self, val: &Val) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let out = match val {
            Ref(pointee) => self.rust_val(pointee)?,
            Ptr(addr) => format!("{addr:#X} as *mut ()"),
            Bool(val) => format!("{val}"),
            Array(vals) => {
                let mut output = String::new();
                output.push('[');
                for elem in vals {
                    let part = format!("{}, ", self.rust_val(elem)?);
                    output.push_str(&part);
                }
                output.push(']');
                output
            }
            Struct(name, fields) => {
                let mut output = String::new();
                output.push_str(&format!("{name} {{ "));
                for (idx, field) in fields.iter().enumerate() {
                    let part = format!("{}: {},", FIELD_NAMES[idx], self.rust_val(field)?);
                    output.push_str(&part);
                }
                output.push_str(" }");
                output
            }
            Float(FloatVal::c_double(val)) => {
                if val.fract() == 0.0 {
                    format!("{val}.0")
                } else {
                    format!("{val}")
                }
            }
            Float(FloatVal::c_float(val)) => {
                if val.fract() == 0.0 {
                    format!("{val}.0")
                } else {
                    format!("{val}")
                }
            }
            Int(int_val) => match int_val {
                c__int128(val) => {
                    if STRUCT_128 {
                        format!("FfiI128::new({val})")
                    } else {
                        format!("{val}")
                    }
                }
                c_int64_t(val) => format!("{val}"),
                c_int32_t(val) => format!("{val}"),
                c_int16_t(val) => format!("{val}"),
                c_int8_t(val) => format!("{val}"),
                c__uint128(val) => {
                    if STRUCT_128 {
                        format!("FfiU128::new({val:#X})")
                    } else {
                        format!("{val:#X}")
                    }
                }
                c_uint64_t(val) => format!("{val:#X}"),
                c_uint32_t(val) => format!("{val:#X}"),
                c_uint16_t(val) => format!("{val:#X}"),
                c_uint8_t(val) => format!("{val:#X}"),
            },
        };
        Ok(out)
    }

    /// A suitable default value for this type
    fn rust_default_val(&self, val: &Val) -> Result<String, GenerateError> {
        use Val::*;
        let out = match val {
            Ref(pointee) => self.rust_default_val(pointee)?,
            Ptr(_) => "0 as *mut ()".to_string(),
            Bool(_) => "false".to_string(),
            Array(vals) => {
                let mut output = String::new();
                output.push('[');
                for elem in vals {
                    let part = format!("{}, ", self.rust_default_val(elem)?);
                    output.push_str(&part);
                }
                output.push(']');
                output
            }
            Struct(name, fields) => {
                let mut output = String::new();
                output.push_str(&format!("{name} {{ "));
                for (idx, field) in fields.iter().enumerate() {
                    let part = format!("{}: {},", FIELD_NAMES[idx], self.rust_default_val(field)?);
                    output.push_str(&part);
                }
                output.push_str(" }");
                output
            }
            Float(..) => "0.0".to_string(),
            Int(IntVal::c__int128(..)) => {
                if STRUCT_128 {
                    "FfiI128::new(0)".to_string()
                } else {
                    "0".to_string()
                }
            }
            Int(IntVal::c__uint128(..)) => {
                if STRUCT_128 {
                    "FfiU128::new(0)".to_string()
                } else {
                    "0".to_string()
                }
            }
            Int(..) => "0".to_string(),
        };
        Ok(out)
    }

    /// Emit the WRITE calls and FINISHED_VAL for this value.
    /// This will WRITE every leaf subfield of the type.
    /// `to` is the BUFFER to use, `from` is the variable name of the value.
    fn rust_write_val(
        &self,
        val: &Val,
        to: &str,
        from: &str,
        is_var_root: bool,
    ) -> Result<String, GenerateError> {
        use std::fmt::Write;
        let mut output = String::new();
        for path in self.rust_var_paths(val, from, is_var_root)? {
            writeln!(output, "        WRITE_FIELD.unwrap()({to}, &{path} as *const _ as *const _, core::mem::size_of_val(&{path}) as u32);").unwrap();
        }
        write!(output, "        FINISHED_VAL.unwrap()({to});").unwrap();

        Ok(output)
    }

    /// Compute the paths to every subfield of this value, with `from`
    /// as the base path to that value, for rust_write_val's use.
    fn rust_var_paths(
        &self,
        val: &Val,
        from: &str,
        is_var_root: bool,
    ) -> Result<Vec<String>, GenerateError> {
        let paths = match val {
            Val::Int(_) | Val::Float(_) | Val::Bool(_) | Val::Ptr(_) => {
                vec![format!("{from}")]
            }
            Val::Struct(_name, fields) => {
                let mut paths = vec![];
                for (idx, field) in fields.iter().enumerate() {
                    let base = format!("{from}.{}", FIELD_NAMES[idx]);
                    paths.extend(self.rust_var_paths(field, &base, false)?);
                }
                paths
            }
            Val::Ref(pointee) => {
                if is_var_root {
                    self.rust_var_paths(pointee, from, false)?
                } else {
                    let base = format!("(*{from})");
                    self.rust_var_paths(pointee, &base, false)?
                }
            }
            Val::Array(vals) => {
                let mut paths = vec![];
                for (i, elem) in vals.iter().enumerate() {
                    let base = format!("{from}[{i}]");
                    paths.extend(self.rust_var_paths(elem, &base, false)?);
                }
                paths
            }
        };

        Ok(paths)
    }
}
