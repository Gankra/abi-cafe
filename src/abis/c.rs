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
            // Function signature
            if let Some(output) = &function.output {
                write!(f, "{} ", output.c_arg_type()?)?;
            } else {
                write!(f, "void ")?;
            }
            write!(f, "{}(", function.name)?;
            for (idx, input) in function.inputs.iter().enumerate() {
                if idx != 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{} arg{idx}", input.c_arg_type()?)?;
            }
            if function.inputs.is_empty() {
                write!(f, "void")?;
            }
            writeln!(f, ") {{")?;

            writeln!(f)?;
            for (idx, input) in function.inputs.iter().enumerate() {
                let val = format!("arg{idx}");
                writeln!(f, "{}", input.c_write_val("CALLEE_INPUTS", &val)?)?;
            }
            writeln!(f)?;
            if let Some(output) = &function.output {
                writeln!(
                    f,
                    "    {} output = {};",
                    output.c_arg_type()?,
                    output.c_val()?
                )?;
                writeln!(f, "{}", output.c_write_val("CALLEE_OUTPUTS", "output")?)?;
                writeln!(f, "    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);")?;
                writeln!(f, "    return output;")?;
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
            if let Some(output) = &function.output {
                write!(f, "{} ", output.c_arg_type()?)?;
            } else {
                write!(f, "void ")?;
            }
            write!(f, "{}(", function.name)?;
            for (idx, input) in function.inputs.iter().enumerate() {
                if idx != 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{} arg{idx}", input.c_arg_type()?)?;
            }
            if function.inputs.is_empty() {
                write!(f, "void")?;
            }
            writeln!(f, ");")?;
        }

        writeln!(f)?;
        writeln!(f, "void do_test(void) {{")?;

        // Generate the impls
        for function in &test.funcs {
            // Add an extra scope to avoid clashes between subtests
            writeln!(f, "{{")?;
            // Inputs
            for (idx, input) in function.inputs.iter().enumerate() {
                let var = format!("arg{idx}");
                writeln!(f, "    {} {var} = {};", input.c_arg_type()?, input.c_val()?)?;
                writeln!(f, "{}", input.c_write_val("CALLER_INPUTS", &var)?)?;
            }
            writeln!(f)?;

            // Output
            write!(f, "    ")?;
            if let Some(output) = &function.output {
                write!(f, "{} output = ", output.c_arg_type()?,)?;
            }

            // Do the actual call
            write!(f, "{}(", function.name)?;
            for (idx, _input) in function.inputs.iter().enumerate() {
                if idx != 0 {
                    write!(f, ", ")?;
                }
                write!(f, "arg{idx}")?;
            }
            writeln!(f, ");")?;

            if let Some(output) = &function.output {
                writeln!(f, "{}", output.c_write_val("CALLER_OUTPUTS", "output")?)?;
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
            if let Some((name, decl)) = val.c_forward_decl()? {
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

impl Val {
    /// If this value defines a nominal type, this will spit out:
    ///
    /// * The type name
    /// * The forward-declaration of that type
    ///
    /// To catch buggy test definitions, you should validate that all
    /// structs that claim a particular name have the same declaration.
    /// This is done in write_rust_prefix.
    fn c_forward_decl(&self) -> Result<Option<(String, String)>, GenerateError> {
        use Val::*;
        if let Struct(name, fields) = self {
            let mut output = String::new();
            let ref_name = format!("struct {name}");
            output.push_str(&format!("struct {name} {{\n"));
            for (idx, field) in fields.iter().enumerate() {
                let line = format!("    {} field{idx};\n", field.c_nested_type()?);
                output.push_str(&line);
            }
            output.push_str("};\n");
            Ok(Some((ref_name, output)))
        } else {
            // Don't need to forward decl any other types
            Ok(None)
        }
    }

    /// The type name to use for this value when it is stored in args/vars.
    fn c_arg_type(&self) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let val = match self {
            Ref(x) => format!("{}*", x.c_arg_type()?),
            Ptr(_) => format!("void*"),
            Bool(_) => format!("bool"),
            // This API doesn't work for expressing C type syntax with arrays
            Array(_vals) => return Err(GenerateError::CUnsupported),
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
    fn c_nested_type(&self) -> Result<String, GenerateError> {
        self.c_arg_type()
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
                    let part = format!("{}", field.c_val()?);
                    output.push_str(&part);
                }
                output.push_str(" }");
                output
            }
            Float(FloatVal::c_double(val)) => format!("{val}"),
            Float(FloatVal::c_float(val)) => format!("{val}f"),
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
    fn c_write_val(&self, to: &str, from: &str) -> Result<String, GenerateError> {
        use std::fmt::Write;
        let mut output = String::new();
        for path in self.c_var_paths(from)? {
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
    fn c_var_paths(&self, from: &str) -> Result<Vec<String>, GenerateError> {
        let paths = match self {
            Val::Int(_) | Val::Float(_) | Val::Bool(_) | Val::Ptr(_) => {
                vec![format!("{from}")]
            }
            Val::Struct(_name, fields) => {
                let mut paths = vec![];
                for (idx, field) in fields.iter().enumerate() {
                    let base = format!("{from}.field{idx}");
                    paths.extend(field.c_var_paths(&base)?);
                }
                paths
            }
            // TODO: need to think about this
            Val::Ref(_) => return Err(GenerateError::CUnsupported),
            // TODO: not yet implemented
            Val::Array(_) => return Err(GenerateError::CUnsupported),
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
