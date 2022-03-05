use super::super::*;
use super::*;

pub static C_TEST_PREFIX: &str = include_str!("../../harness/c_test_prefix.h");

pub struct CAbi;

impl Abi for CAbi {
    fn name(&self) -> &'static str {
        "c"
    }
    fn src_ext(&self) -> &'static str {
        "c"
    }

    fn generate_callee(&self, f: &mut dyn Write, test: &Test) -> Result<(), BuildError> {
        write_c_prefix(f, test)?;

        // Generate the impls
        for function in &test.funcs {
            write_c_signature(f, function)?;
            writeln!(f, " {{")?;

            writeln!(f)?;
            for (idx, input) in function.inputs.iter().enumerate() {
                writeln!(
                    f,
                    "{}",
                    input.c_write_val("CALLEE_INPUTS", ARG_NAMES[idx], false)?
                )?;
            }
            writeln!(f)?;
            if let Some(output) = &function.output {
                writeln!(
                    f,
                    "    {} = {};",
                    output.c_var_decl(OUTPUT_NAME)?,
                    output.c_val()?
                )?;
                writeln!(
                    f,
                    "{}",
                    output.c_write_val("CALLEE_OUTPUTS", OUTPUT_NAME, true)?
                )?;
                writeln!(f, "    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);")?;
                writeln!(
                    f,
                    "    {}",
                    output.c_var_return(OUTPUT_NAME, OUT_PARAM_NAME)?
                )?;
            } else {
                writeln!(f, "    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);")?;
            }
            writeln!(f, "}}")?;
            writeln!(f)?;
        }

        Ok(())
    }

    fn generate_caller(&self, f: &mut dyn Write, test: &Test) -> Result<(), BuildError> {
        write_c_prefix(f, test)?;

        // Generate the extern block
        for function in &test.funcs {
            write_c_signature(f, function)?;
            writeln!(f, ";")?;
        }

        writeln!(f)?;
        writeln!(f, "void do_test(void) {{")?;

        // Generate the impls
        for function in &test.funcs {
            // Add an extra scope to avoid clashes between subtests
            writeln!(f, "{{")?;
            // Inputs
            for (idx, input) in function.inputs.iter().enumerate() {
                writeln!(
                    f,
                    "    {} = {};",
                    input.c_var_decl(ARG_NAMES[idx])?,
                    input.c_val()?
                )?;
                writeln!(
                    f,
                    "{}",
                    input.c_write_val("CALLER_INPUTS", ARG_NAMES[idx], true)?
                )?;
            }
            writeln!(f)?;

            // Output
            let pass_out = if let Some(output) = &function.output {
                if let Some(out_param_var) = output.c_out_param_var(OUTPUT_NAME)? {
                    writeln!(f, "    {};", out_param_var)?;
                    write!(f, "    ")?;
                    true
                } else {
                    write!(f, "    {} = ", output.c_var_decl(OUTPUT_NAME)?)?;
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
                write!(f, "{}", input.c_arg_pass(ARG_NAMES[idx])?)?;
            }
            if pass_out {
                let pass = function.output.as_ref().unwrap().c_arg_pass(OUTPUT_NAME)?;
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
                    output.c_write_val("CALLER_OUTPUTS", OUTPUT_NAME, true)?
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
        cc::Build::new()
            .file(src_path)
            .cargo_metadata(false)
            // .warnings_into_errors(true)
            .try_compile(lib_name)?;
        Ok(String::from(lib_name))
    }

    fn compile_caller(&self, src_path: &Path, lib_name: &str) -> Result<String, BuildError> {
        // Currently no need to be different
        self.compile_callee(src_path, lib_name)
    }
}

/// Every test should start by loading in the harness' "header"
/// and forward-declaring any structs that will be used.
fn write_c_prefix(f: &mut dyn Write, test: &Test) -> Result<(), BuildError> {
    // Load test harness "headers"
    write!(f, "{}", C_TEST_PREFIX)?;

    // Forward-decl struct types
    let mut forward_decls = std::collections::HashMap::<String, String>::new();
    for function in &test.funcs {
        for val in function.inputs.iter().chain(function.output.as_ref()) {
            for (name, decl) in val.c_forward_decl()? {
                match forward_decls.entry(name) {
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        if entry.get() != &decl {
                            return Err(BuildError::InconsistentStructDefinition {
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

// Emit a function signature
fn write_c_signature(f: &mut dyn Write, function: &Func) -> Result<(), BuildError> {
    // First figure out the return (by-ref requires an out-param)
    let out_param = if let Some(output) = &function.output {
        let out_param = output.c_out_param(OUT_PARAM_NAME)?;
        if out_param.is_none() {
            write!(f, "{} ", output.c_arg_type()?)?;
        } else {
            write!(f, "void ")?;
        }
        out_param
    } else {
        write!(f, "void ")?;
        None
    };

    // Now write out the args
    write!(f, "{}(", function.name)?;
    for (idx, input) in function.inputs.iter().enumerate() {
        if idx != 0 {
            write!(f, ", ")?;
        }
        write!(f, "{}", input.c_arg_decl(ARG_NAMES[idx])?)?;
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

impl Val {
    /// If this value defines a nominal type, this will spit out:
    ///
    /// * The type name
    /// * The forward-declaration of that type
    ///
    /// To catch buggy test definitions, you should validate that all
    /// structs that claim a particular name have the same declaration.
    /// This is done in write_rust_prefix.
    fn c_forward_decl(&self) -> Result<Vec<(String, String)>, GenerateError> {
        use Val::*;
        match self {
            Struct(name, fields) => {
                let mut results = vec![];
                for field in fields.iter() {
                    results.extend(field.c_forward_decl()?);
                }
                let mut output = String::new();
                let ref_name = format!("struct {name}");
                output.push_str(&format!("struct {name} {{\n"));
                for (idx, field) in fields.iter().enumerate() {
                    let line = format!("    {};\n", field.c_field_decl(FIELD_NAMES[idx])?);
                    output.push_str(&line);
                }
                output.push_str("};\n");
                results.push((ref_name, output));
                Ok(results)
            }
            Array(vals) => vals[0].c_forward_decl(),
            Ref(x) => x.c_forward_decl(),
            _ => Ok(vec![]),
        }
    }

    /// The decl to use for a local var (reference-ness stripped)
    fn c_var_decl(&self, var_name: &str) -> Result<String, GenerateError> {
        use Val::*;
        let val = match self {
            Ref(x) => x.c_var_decl(var_name)?,
            Array(_) => {
                let mut cur_val = self;
                let mut array_levels = String::new();
                while let Val::Array(vals) = cur_val {
                    array_levels.push_str(&format!("[{}]", vals.len()));
                    cur_val = &vals[0];
                }
                format!("{} {var_name}{array_levels}", cur_val.c_arg_type()?)
            }
            normal_val => format!("{} {var_name}", normal_val.c_arg_type()?),
        };
        Ok(val)
    }

    /// The decl to use for a function arg (apply referenceness)
    fn c_arg_decl(&self, arg_name: &str) -> Result<String, GenerateError> {
        let val = if let Val::Ref(x) = self {
            let mut cur_val = &**x;
            let mut array_levels = String::new();
            while let Val::Array(vals) = cur_val {
                array_levels.push_str(&format!("[{}]", vals.len()));
                cur_val = &vals[0];
            }
            if array_levels.is_empty() {
                format!("{}* {arg_name}", cur_val.c_arg_type()?)
            } else {
                format!("{} {arg_name}{array_levels}", cur_val.c_arg_type()?)
            }
        } else {
            format!("{} {arg_name}", self.c_arg_type()?)
        };
        Ok(val)
    }

    /// If the return type needs to be an out_param, this returns it
    fn c_out_param(&self, out_param_name: &str) -> Result<Option<String>, GenerateError> {
        let val = if let Val::Ref(x) = self {
            let mut cur_val = &**x;
            let mut array_levels = String::new();
            while let Val::Array(vals) = cur_val {
                array_levels.push_str(&format!("[{}]", vals.len()));
                cur_val = &vals[0];
            }
            if array_levels.is_empty() {
                Some(format!("{}* {out_param_name}", cur_val.c_arg_type()?))
            } else {
                Some(format!(
                    "{} {out_param_name}{array_levels}",
                    cur_val.c_arg_type()?
                ))
            }
        } else {
            None
        };
        Ok(val)
    }

    /// If the return type needs to be an out_param, this returns it
    fn c_out_param_var(&self, output_name: &str) -> Result<Option<String>, GenerateError> {
        if let Val::Ref(x) = self {
            Ok(Some(x.c_var_decl(output_name)?))
        } else {
            Ok(None)
        }
    }

    /// How to pass an argument
    fn c_arg_pass(&self, arg_name: &str) -> Result<String, GenerateError> {
        if let Val::Ref(x) = self {
            if let Val::Array(_) = &**x {
                Ok(format!("{arg_name}"))
            } else {
                Ok(format!("&{arg_name}"))
            }
        } else {
            Ok(format!("{arg_name}"))
        }
    }

    /// How to return a value
    fn c_var_return(&self, var_name: &str, out_param_name: &str) -> Result<String, GenerateError> {
        if let Val::Ref(_) = self {
            Ok(format!(
                "memcpy({out_param_name}, &{var_name}, sizeof({var_name}));"
            ))
        } else {
            Ok(format!("return {var_name};"))
        }
    }

    /// The type name to use for this value when it is stored in args/vars.
    fn c_arg_type(&self) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let val = match self {
            Ref(x) => {
                let mut cur_val = &**x;
                while let Val::Array(vals) = cur_val {
                    cur_val = &vals[0];
                }
                format!("{}*", cur_val.c_arg_type()?)
            }
            Ptr(_) => format!("void*"),
            Bool(_) => format!("bool"),
            // This API doesn't work for expressing C type syntax with arrays
            Array(_vals) => {
                return Err(GenerateError::CUnsupported(format!(
                    "C Arrays can't be passed directly, wrap this in ByRef"
                )))
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
    fn c_field_decl(&self, field_name: &str) -> Result<String, GenerateError> {
        let mut cur_val = self;
        let mut array_levels = String::new();
        while let Val::Array(vals) = cur_val {
            array_levels.push_str(&format!("[{}]", vals.len()));
            cur_val = &vals[0];
        }
        Ok(format!(
            "{} {field_name}{array_levels}",
            cur_val.c_arg_type()?
        ))
    }

    /// An expression that generates this value.
    pub fn c_val(&self) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let val = match self {
            Ref(x) => x.c_val()?,
            Ptr(addr) => format!("(void*){addr}"),
            Bool(val) => format!("{val}"),
            Array(vals) => {
                let mut output = String::new();
                output.push_str("{ ");
                for (idx, val) in vals.iter().enumerate() {
                    if idx != 0 {
                        output.push_str(", ");
                    }
                    let part = format!("{}", val.c_val()?);
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
                    let part = format!(".{} = {}", FIELD_NAMES[idx], field.c_val()?);
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
            Int(int_val) => match int_val {
                c__int128(val) => {
                    let lower = val & 0x00000000_00000000_FFFFFFFF_FFFFFFFF;
                    let higher = (val & 0xFFFFFFF_FFFFFFFF_00000000_00000000) >> 64;
                    format!("((__int128_t){lower}) | (((__int128_t){higher}) << 64)")
                }
                c__uint128(val) => {
                    let lower = val & 0x00000000_00000000_FFFFFFFF_FFFFFFFF;
                    let higher = (val & 0xFFFFFFF_FFFFFFFF_00000000_00000000) >> 64;
                    format!("((__uint128_t){lower}) | (((__uint128_t){higher}) << 64)")
                }
                c_int64_t(val) => format!("{val}"),
                c_int32_t(val) => format!("{val}"),
                c_int16_t(val) => format!("{val}"),
                c_int8_t(val) => format!("{val}"),
                c_uint64_t(val) => format!("{val}"),
                c_uint32_t(val) => format!("{val}"),
                c_uint16_t(val) => format!("{val}"),
                c_uint8_t(val) => format!("{val}"),
            },
        };
        Ok(val)
    }

    /// Emit the WRITE calls and FINISHED_VAL for this value.
    /// This will WRITE every leaf subfield of the type.
    /// `to` is the BUFFER to use, `from` is the variable name of the value.
    fn c_write_val(
        &self,
        to: &str,
        from: &str,
        is_var_root: bool,
    ) -> Result<String, GenerateError> {
        use std::fmt::Write;
        let mut output = String::new();
        for path in self.c_var_paths(from, is_var_root)? {
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
    fn c_var_paths(&self, from: &str, is_var_root: bool) -> Result<Vec<String>, GenerateError> {
        let paths = match self {
            Val::Int(_) | Val::Float(_) | Val::Bool(_) | Val::Ptr(_) => {
                vec![format!("{from}")]
            }
            Val::Struct(_name, fields) => {
                let mut paths = vec![];
                for (idx, field) in fields.iter().enumerate() {
                    let base = format!("{from}.{}", FIELD_NAMES[idx]);
                    paths.extend(field.c_var_paths(&base, false)?);
                }
                paths
            }
            Val::Ref(val) => {
                if is_var_root {
                    val.c_var_paths(from, false)?
                } else if let Val::Array(_) = &**val {
                    val.c_var_paths(from, false)?
                } else {
                    let base = format!("(*{from})");
                    val.c_var_paths(&base, false)?
                }
            }
            Val::Array(vals) => {
                let mut paths = vec![];
                for (i, val) in vals.iter().enumerate() {
                    let base = format!("{from}[{i}]");
                    paths.extend(val.c_var_paths(&base, false)?);
                }
                paths
            }
        };

        Ok(paths)
    }

    /*
    /// Format specifiers for C types, for print debugging.
    /// This is no longer used but it's a shame to throw out.
    pub fn cfmt(&self) -> &'static str {
        use Val::*;
        use IntVal::*;
        match self {
            Ref(x) => x.cfmt(),
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
