use crate::Config;

use super::super::*;
use super::*;

pub static C_TEST_PREFIX: &str = include_str!("../../harness/c_test_prefix.h");

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

    fn supports_convention(&self, convention: CallingConvention) -> bool {
        self.c_convention_decl(convention).is_ok()
    }

    fn generate_callee(
        &self,
        f: &mut dyn Write,
        test: &Test,
        convention: CallingConvention,
    ) -> Result<(), GenerateError> {
        self.write_c_prefix(f, test)?;

        // Generate the impls
        for function in &test.funcs {
            if !function.has_convention(convention) {
                continue;
            }
            self.write_c_signature(f, function, convention)?;
            writeln!(f, " {{")?;

            writeln!(f)?;
            for (idx, input) in function.inputs.iter().enumerate() {
                writeln!(
                    f,
                    "{}",
                    self.c_write_val(input, "CALLEE_INPUTS", ARG_NAMES[idx], false)?
                )?;
            }
            writeln!(f)?;
            if let Some(output) = &function.output {
                writeln!(
                    f,
                    "    {} = {};",
                    self.c_var_decl(output, OUTPUT_NAME)?,
                    self.c_val(output)?
                )?;
                writeln!(
                    f,
                    "{}",
                    self.c_write_val(output, "CALLEE_OUTPUTS", OUTPUT_NAME, true)?
                )?;
                writeln!(f, "    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);")?;
                writeln!(
                    f,
                    "    {}",
                    self.c_var_return(output, OUTPUT_NAME, OUT_PARAM_NAME)?
                )?;
            } else {
                writeln!(f, "    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);")?;
            }
            writeln!(f, "}}")?;
            writeln!(f)?;
        }

        Ok(())
    }

    fn generate_caller(
        &self,
        f: &mut dyn Write,
        test: &Test,
        convention: CallingConvention,
    ) -> Result<(), GenerateError> {
        self.write_c_prefix(f, test)?;

        // Generate the extern block
        for function in &test.funcs {
            self.write_c_signature(f, function, convention)?;
            writeln!(f, ";")?;
        }

        writeln!(f)?;
        writeln!(f, "void do_test(void) {{")?;

        // Generate the impls
        for function in &test.funcs {
            if !function.has_convention(convention) {
                continue;
            }
            // Add an extra scope to avoid clashes between subtests
            writeln!(f, "{{")?;
            // Inputs
            for (idx, input) in function.inputs.iter().enumerate() {
                writeln!(
                    f,
                    "    {} = {};",
                    self.c_var_decl(input, ARG_NAMES[idx])?,
                    self.c_val(input)?
                )?;
                writeln!(
                    f,
                    "{}",
                    self.c_write_val(input, "CALLER_INPUTS", ARG_NAMES[idx], true)?
                )?;
            }
            writeln!(f)?;

            // Output
            let pass_out = if let Some(output) = &function.output {
                if let Some(out_param_var) = self.c_out_param_var(output, OUTPUT_NAME)? {
                    writeln!(f, "    {};", out_param_var)?;
                    write!(f, "    ")?;
                    true
                } else {
                    write!(f, "    {} = ", self.c_var_decl(output, OUTPUT_NAME)?)?;
                    false
                }
            } else {
                write!(f, "    ")?;
                false
            };

            // Do the actual call
            write!(f, "{}(", function.name)?;
            for (idx, input) in function.inputs.iter().enumerate() {
                if idx != 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", self.c_arg_pass(input, ARG_NAMES[idx])?)?;
            }
            if pass_out {
                let pass = self.c_arg_pass(function.output.as_ref().unwrap(), OUTPUT_NAME)?;
                if function.inputs.is_empty() {
                    write!(f, "{}", pass)?;
                } else {
                    write!(f, ", {}", pass)?;
                }
            }
            writeln!(f, ");")?;

            if let Some(output) = &function.output {
                writeln!(
                    f,
                    "{}",
                    self.c_write_val(output, "CALLER_OUTPUTS", OUTPUT_NAME, true)?
                )?;
            }
            writeln!(f, "    FINISHED_FUNC(CALLER_INPUTS, CALLER_OUTPUTS);")?;
            writeln!(f, "}}")?;
            writeln!(f)?;
        }
        writeln!(f, "}}")?;
        Ok(())
    }

    fn compile_callee(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError> {
        match self.mode {
            "cc" => self.compile_cc(src_path, lib_name),
            "gcc" => self.compile_gcc(src_path, lib_name),
            "clang" => self.compile_clang(src_path, lib_name),
            "msvc" => self.compile_msvc(src_path, lib_name),
            _ => unimplemented!("unknown c compiler"),
        }
    }

    fn compile_caller(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError> {
        match self.mode {
            "cc" => self.compile_cc(src_path, lib_name),
            "gcc" => self.compile_gcc(src_path, lib_name),
            "clang" => self.compile_clang(src_path, lib_name),
            "msvc" => self.compile_msvc(src_path, lib_name),
            _ => unimplemented!("unknown c compiler"),
        }
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

    fn compile_cc(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError> {
        cc::Build::new()
            .file(src_path)
            .opt_level(0)
            .cargo_metadata(false)
            // .warnings_into_errors(true)
            .try_compile(lib_name)?;
        Ok(String::from(lib_name))
    }

    fn compile_clang(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError> {
        let base_path = PathBuf::from("target/temp/");
        let obj_path = base_path.join(format!("{lib_name}.o"));
        let lib_path = base_path.join(format!("lib{lib_name}.a"));
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

    fn compile_gcc(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError> {
        let base_path = PathBuf::from("target/temp/");
        let obj_path = base_path.join(format!("{lib_name}.o"));
        let lib_path = base_path.join(format!("lib{lib_name}.a"));
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

    fn compile_msvc(&self, _src_path: &Path, _lib_name: &str) -> Result<String, BuildError> {
        unimplemented!()
    }

    fn c_convention_decl(
        &self,
        convention: CallingConvention,
    ) -> Result<&'static str, GenerateError> {
        use CCFlavor::*;
        use CallingConvention::*;
        use Platform::*;
        // GCC (as __attribute__'s)
        //
        //  * x86: cdecl, fastcall, thiscall, stdcall,
        //         sysv_abi, ms_abi (64-bit: -maccumulate-outgoing-args?),
        //         naked, interrupt, sseregparm
        //  * ARM: pcs="aapcs", pcs="aapcs-vfp",
        //         long_call, short_call, naked,
        //         interrupt("IRQ", "FIQ", "SWI", "ABORT", "UNDEF"),
        //
        // MSVC (as ~keywords)
        //
        //  * __cdecl, __clrcall, __stdcall, __fastcall, __thiscall, __vectorcall

        let val = match convention {
            Handwritten => "handwritten",
            All => {
                // All is sugar, we shouldn't get here!
                return Err(GenerateError::UnsupportedConvention);
            }
            System | Win64 | Sysv64 | Aapcs => {
                // Don't want to think about these yet, I think they're
                // all properly convered by other ABIs
                return Err(GenerateError::UnsupportedConvention);
            }
            C => "",
            Cdecl => {
                if self.platform == Windows {
                    match self.cc_flavor {
                        Msvc => "__cdecl ",
                        Gcc | Clang => "__attribute__((cdecl)) ",
                    }
                } else {
                    return Err(GenerateError::UnsupportedConvention);
                }
            }
            Stdcall => {
                if self.platform == Windows {
                    match self.cc_flavor {
                        Msvc => "__stdcall ",
                        Gcc | Clang => "__attribute__((stdcall)) ",
                    }
                } else {
                    return Err(GenerateError::UnsupportedConvention);
                }
            }
            Fastcall => {
                if self.platform == Windows {
                    match self.cc_flavor {
                        Msvc => "__fastcall ",
                        Gcc | Clang => "__attribute__((fastcall)) ",
                    }
                } else {
                    return Err(GenerateError::UnsupportedConvention);
                }
            }
            Vectorcall => {
                if self.platform == Windows {
                    match self.cc_flavor {
                        Msvc => "__vectorcall ",
                        Gcc | Clang => "__attribute__((vectorcall)) ",
                    }
                } else {
                    return Err(GenerateError::UnsupportedConvention);
                }
            }
        };

        Ok(val)
    }

    // Emit a function signature
    fn write_c_signature(
        &self,
        f: &mut dyn Write,
        function: &Func,
        convention: CallingConvention,
    ) -> Result<(), GenerateError> {
        let convention_decl = self.c_convention_decl(convention)?;

        // First figure out the return (by-ref requires an out-param)
        let out_param = if let Some(output) = &function.output {
            let out_param = self.c_out_param(output, OUT_PARAM_NAME)?;
            if out_param.is_none() {
                write!(f, "{} ", self.c_arg_type(output)?)?;
            } else {
                write!(f, "void ")?;
            }
            out_param
        } else {
            write!(f, "void ")?;
            None
        };

        write!(f, "{}", convention_decl)?;

        // Now write out the args
        write!(f, "{}(", function.name)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            if idx != 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", self.c_arg_decl(input, ARG_NAMES[idx])?)?;
        }

        // Add extra implicit args
        if let Some(out_param) = out_param {
            if !function.inputs.is_empty() {
                write!(f, ", ")?;
            }
            write!(f, "{out_param}")?;
        } else if function.inputs.is_empty() {
            write!(f, "void")?;
        }
        write!(f, ")")?;

        Ok(())
    }

    /// Every test should start by loading in the harness' "header"
    /// and forward-declaring any structs that will be used.
    fn write_c_prefix(&self, f: &mut dyn Write, test: &Test) -> Result<(), GenerateError> {
        // Load test harness "headers"
        write!(f, "{}", C_TEST_PREFIX)?;

        // Forward-decl struct types
        let mut forward_decls = std::collections::HashMap::<String, String>::new();
        for function in &test.funcs {
            for val in function.inputs.iter().chain(function.output.as_ref()) {
                for (name, decl) in self.c_forward_decl(val)? {
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

    fn c_forward_decl(&self, val: &Val) -> Result<Vec<(String, String)>, GenerateError> {
        use Val::*;
        match val {
            Struct(name, fields) => {
                let mut results = vec![];
                for field in fields.iter() {
                    results.extend(self.c_forward_decl(field)?);
                }
                let mut output = String::new();
                let ref_name = format!("struct {name}");
                output.push_str(&format!("struct {name} {{\n"));
                for (idx, field) in fields.iter().enumerate() {
                    let line = format!("    {};\n", self.c_field_decl(field, FIELD_NAMES[idx])?);
                    output.push_str(&line);
                }
                output.push_str("};\n");
                results.push((ref_name, output));
                Ok(results)
            }
            Array(vals) => self.c_forward_decl(&vals[0]),
            Ref(pointee) => self.c_forward_decl(pointee),
            _ => Ok(vec![]),
        }
    }

    /// The decl to use for a local var (reference-ness stripped)
    fn c_var_decl(&self, val: &Val, var_name: &str) -> Result<String, GenerateError> {
        use Val::*;
        let val = match val {
            Ref(pointee) => self.c_var_decl(pointee, var_name)?,
            Array(_) => {
                let mut cur_val = val;
                let mut array_levels = String::new();
                while let Val::Array(vals) = cur_val {
                    array_levels.push_str(&format!("[{}]", vals.len()));
                    cur_val = &vals[0];
                }
                format!("{} {var_name}{array_levels}", self.c_arg_type(cur_val)?)
            }
            normal_val => format!("{} {var_name}", self.c_arg_type(normal_val)?),
        };
        Ok(val)
    }

    /// The decl to use for a function arg (apply referenceness)
    fn c_arg_decl(&self, val: &Val, arg_name: &str) -> Result<String, GenerateError> {
        let out = if let Val::Ref(pointee) = val {
            let mut cur_val = &**pointee;
            let mut array_levels = String::new();
            while let Val::Array(vals) = cur_val {
                array_levels.push_str(&format!("[{}]", vals.len()));
                cur_val = &vals[0];
            }
            if array_levels.is_empty() {
                format!("{}* {arg_name}", self.c_arg_type(cur_val)?)
            } else {
                format!("{} {arg_name}{array_levels}", self.c_arg_type(cur_val)?)
            }
        } else {
            format!("{} {arg_name}", self.c_arg_type(val)?)
        };
        Ok(out)
    }

    /// If the return type needs to be an out_param, this returns it
    fn c_out_param(
        &self,
        val: &Val,
        out_param_name: &str,
    ) -> Result<Option<String>, GenerateError> {
        let out = if let Val::Ref(pointee) = val {
            let mut cur_val = &**pointee;
            let mut array_levels = String::new();
            while let Val::Array(vals) = cur_val {
                array_levels.push_str(&format!("[{}]", vals.len()));
                cur_val = &vals[0];
            }
            if array_levels.is_empty() {
                Some(format!("{}* {out_param_name}", self.c_arg_type(cur_val)?))
            } else {
                Some(format!(
                    "{} {out_param_name}{array_levels}",
                    self.c_arg_type(cur_val)?
                ))
            }
        } else {
            None
        };
        Ok(out)
    }

    /// If the return type needs to be an out_param, this returns it
    fn c_out_param_var(
        &self,
        val: &Val,
        output_name: &str,
    ) -> Result<Option<String>, GenerateError> {
        if let Val::Ref(pointee) = val {
            Ok(Some(self.c_var_decl(pointee, output_name)?))
        } else {
            Ok(None)
        }
    }

    /// How to pass an argument
    fn c_arg_pass(&self, val: &Val, arg_name: &str) -> Result<String, GenerateError> {
        if let Val::Ref(pointee) = val {
            if let Val::Array(_) = &**pointee {
                Ok(format!("{arg_name}"))
            } else {
                Ok(format!("&{arg_name}"))
            }
        } else {
            Ok(format!("{arg_name}"))
        }
    }

    /// How to return a value
    fn c_var_return(
        &self,
        val: &Val,
        var_name: &str,
        out_param_name: &str,
    ) -> Result<String, GenerateError> {
        if let Val::Ref(_) = val {
            Ok(format!(
                "memcpy({out_param_name}, &{var_name}, sizeof({var_name}));"
            ))
        } else {
            Ok(format!("return {var_name};"))
        }
    }

    /// The type name to use for this value when it is stored in args/vars.
    fn c_arg_type(&self, val: &Val) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let val = match val {
            Ref(pointee) => {
                let mut cur_val = &**pointee;
                while let Val::Array(vals) = cur_val {
                    cur_val = &vals[0];
                }
                format!("{}*", self.c_arg_type(cur_val)?)
            }
            Ptr(_) => format!("void*"),
            Bool(_) => format!("bool"),
            Array(_vals) => {
                // C arrays are kinda fake due to how they decay in function arg
                // position, so a ton of code needs to very delicately detect arrays
                // and desugar them properly. Since most things eventually sink into
                // c_arg_type, this is a good guard against something forgetting to
                // specially handle arrays!
                //
                // But also it just isn't legal to pass an array by-value in C
                // (it decays to a pointer, so you need to wrap it in Ref for
                // other ABIs to understand that's what we're doing.
                return Err(GenerateError::CUnsupported(format!(
                    "C Arrays can't be passed directly, wrap this in Ref"
                )));
            }
            Struct(name, _) => format!("struct {name}"),
            Float(FloatVal::c_double(_)) => format!("double"),
            Float(FloatVal::c_float(_)) => format!("float"),
            Int(int_val) => match int_val {
                c__int128(_) => format!("__int128_t"),
                c_int64_t(_) => format!("int64_t"),
                c_int32_t(_) => format!("int32_t"),
                c_int16_t(_) => format!("int16_t"),
                c_int8_t(_) => format!("int8_t"),
                c__uint128(_) => format!("__uint128_t"),
                c_uint64_t(_) => format!("uint64_t"),
                c_uint32_t(_) => format!("uint32_t"),
                c_uint16_t(_) => format!("uint16_t"),
                c_uint8_t(_) => format!("uint8_t"),
            },
        };
        Ok(val)
    }

    /// The type name to use for this value when it is stored in composite.
    ///
    /// This is separated out in case there's a type that needs different
    /// handling in this context to conform to a layout (i.e. how C arrays
    /// decay into pointers when used in function args).
    fn c_field_decl(&self, val: &Val, field_name: &str) -> Result<String, GenerateError> {
        let mut cur_val = val;
        let mut array_levels = String::new();
        while let Val::Array(vals) = cur_val {
            array_levels.push_str(&format!("[{}]", vals.len()));
            cur_val = &vals[0];
        }
        Ok(format!(
            "{} {field_name}{array_levels}",
            self.c_arg_type(cur_val)?
        ))
    }

    /// An expression that generates this value.
    pub fn c_val(&self, val: &Val) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let val = match val {
            Ref(pointee) => self.c_val(pointee)?,
            Ptr(addr) => format!("(void*){addr:#X}ull"),
            Bool(val) => format!("{val}"),
            Array(vals) => {
                let mut output = String::new();
                output.push_str("{ ");
                for (idx, elem) in vals.iter().enumerate() {
                    if idx != 0 {
                        output.push_str(", ");
                    }
                    let part = format!("{}", self.c_val(elem)?);
                    output.push_str(&part);
                }
                output.push_str(" }");
                output
            }
            Struct(_name, fields) => {
                let mut output = String::new();
                output.push_str("{ ");
                for (idx, field) in fields.iter().enumerate() {
                    if idx != 0 {
                        output.push_str(", ");
                    }
                    let part = format!(".{} = {}", FIELD_NAMES[idx], self.c_val(field)?);
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
                    format!("{val}.0f")
                } else {
                    format!("{val}f")
                }
            }
            Int(int_val) => match *int_val {
                c__int128(val) => {
                    let lower = (val as u128) & 0x00000000_00000000_FFFFFFFF_FFFFFFFF;
                    let higher = ((val as u128) & 0xFFFFFFFF_FFFFFFFF_00000000_00000000) >> 64;
                    format!("((__int128_t){lower:#X}ull) | (((__int128_t){higher:#X}ull) << 64)")
                }
                c__uint128(val) => {
                    let lower = val & 0x00000000_00000000_FFFFFFFF_FFFFFFFF;
                    let higher = (val & 0xFFFFFFFF_FFFFFFFF_00000000_00000000) >> 64;
                    format!("((__uint128_t){lower:#X}ull) | (((__uint128_t){higher:#X}ull) << 64)")
                }
                c_int64_t(val) => format!("{val}"),
                c_int32_t(val) => format!("{val}"),
                c_int16_t(val) => format!("{val}"),
                c_int8_t(val) => format!("{val}"),
                c_uint64_t(val) => format!("{val}ull"),
                c_uint32_t(val) => format!("{val:#X}"),
                c_uint16_t(val) => format!("{val:#X}"),
                c_uint8_t(val) => format!("{val:#X}"),
            },
        };
        Ok(val)
    }

    /// Emit the WRITE calls and FINISHED_VAL for this value.
    /// This will WRITE every leaf subfield of the type.
    /// `to` is the BUFFER to use, `from` is the variable name of the value.
    fn c_write_val(
        &self,
        val: &Val,
        to: &str,
        from: &str,
        is_var_root: bool,
    ) -> Result<String, GenerateError> {
        use std::fmt::Write;
        let mut output = String::new();
        for path in self.c_var_paths(val, from, is_var_root)? {
            write!(
                output,
                "    WRITE({to}, (char*)&{path}, (uint32_t)sizeof({path}));\n"
            )
            .unwrap();
        }
        write!(output, "    FINISHED_VAL({to});").unwrap();

        Ok(output)
    }

    /// Compute the paths to every subfield of this value, with `from`
    /// as the base path to that value, for c_write_val's use.
    fn c_var_paths(
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
                    paths.extend(self.c_var_paths(field, &base, false)?);
                }
                paths
            }
            Val::Ref(pointee) => {
                if is_var_root {
                    self.c_var_paths(pointee, from, false)?
                } else if let Val::Array(_) = &**pointee {
                    self.c_var_paths(pointee, from, false)?
                } else {
                    let base = format!("(*{from})");
                    self.c_var_paths(pointee, &base, false)?
                }
            }
            Val::Array(vals) => {
                let mut paths = vec![];
                for (i, elem) in vals.iter().enumerate() {
                    let base = format!("{from}[{i}]");
                    paths.extend(self.c_var_paths(elem, &base, false)?);
                }
                paths
            }
        };

        Ok(paths)
    }

    /*
    /// Format specifiers for C types, for print debugging.
    /// This is no longer used but it's a shame to throw out.
    pub fn cfmt(&self, val: &Val) -> &'static str {
        use Val::*;
        use IntVal::*;
        match val {
            Ref(x) => self.cfmt(x),
            Ptr(_) => "\"p\"",
            Bool(_) => "\"d\"",
            Array(_) => {
                todo!()
            }
            Struct(_name, _fields) => {
                todo!()
            }
            Float(FloatVal::c_double(_val)) => "\"f\"",
            Float(FloatVal::c_float(_val)) => "\"f\"",
            Int(int_val) => match int_val {
                c_uint8_t(..) => "PRIu8",
                c_uint16_t(..) => "PRIu16",
                c_uint32_t(..) => "PRIu32",
                c_uint64_t(..) => "PRIu64",
                c_uint128_t(..) => "PRIu128",

                c_int8_t(..) => "PRId8",
                c_int16_t(..) => "PRId16",
                c_int32_t(..) => "PRId32",
                c_int64_t(..) => "PRId64",
                c_int128_t(..) => "PRId128",
            }
        }
    }
    */
}
