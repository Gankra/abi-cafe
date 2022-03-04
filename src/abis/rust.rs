use super::super::*;
use super::*;

pub static RUST_TEST_PREFIX: &str = include_str!("../../harness/rust_test_prefix.rs");

pub struct RustAbi;

impl Abi for RustAbi {
    fn name(&self) -> &'static str {
        "rust"
    }
    fn src_ext(&self) -> &'static str {
        "rs"
    }

    fn generate_caller(&self, f: &mut dyn Write, test: &Test) -> Result<(), BuildError> {
        write_rust_prefix(f, test)?;

        // Generate the extern block
        writeln!(f, "extern {{")?;
        for function in &test.funcs {
            write!(f, "  fn {}(", function.name)?;
            for (idx, input) in function.inputs.iter().enumerate() {
                let ty = input.rust_arg_type()?;
                write!(f, "arg{idx}: {ty}, ",)?;
            }
            write!(f, ")")?;
            if let Some(output) = &function.output {
                let ty = output.rust_arg_type()?;
                write!(f, " -> {ty}")?;
            }
            writeln!(f, ";")?;
        }
        writeln!(f, "}}")?;
        writeln!(f)?;

        // Now generate the body
        writeln!(f, "#[no_mangle] pub extern fn do_test() {{")?;

        for function in &test.funcs {
            writeln!(f, "   unsafe {{")?;

            // Inputs
            for (idx, input) in function.inputs.iter().enumerate() {
                let ty = input.rust_arg_type()?;
                writeln!(f, "        let arg{idx}: {ty} = {};", input.rust_val()?)?;
            }
            writeln!(f)?;
            for (idx, input) in function.inputs.iter().enumerate() {
                let val = format!("arg{idx}");
                writeln!(f, "{}", input.rust_write_val("CALLER_INPUTS", &val)?)?;
            }
            writeln!(f)?;

            // Outputs
            write!(f, "        ")?;
            if let Some(output) = &function.output {
                let ty = output.rust_arg_type()?;
                write!(f, "        let output: {ty} = ")?;
            }

            // Do the call
            write!(f, "{}(", function.name)?;
            for (idx, _input) in function.inputs.iter().enumerate() {
                write!(f, "arg{idx}, ")?;
            }
            writeln!(f, ");")?;
            writeln!(f)?;

            // Report the output
            if let Some(output) = &function.output {
                writeln!(f, "{}", output.rust_write_val("CALLER_OUTPUTS", "output")?)?;
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
    fn generate_callee(&self, f: &mut dyn Write, test: &Test) -> Result<(), BuildError> {
        write_rust_prefix(f, test)?;

        for function in &test.funcs {
            // Write the signature
            writeln!(f, "#[no_mangle]")?;
            write!(f, "pub unsafe extern fn {}(", function.name)?;
            for (idx, input) in function.inputs.iter().enumerate() {
                let ty = input.rust_arg_type()?;
                write!(f, "arg{idx}: {ty}, ",)?;
            }
            write!(f, ")")?;
            if let Some(output) = &function.output {
                let ty = output.rust_arg_type()?;
                write!(f, " -> {ty}")?;
            }
            writeln!(f, " {{")?;

            // Now the body

            // Report Inputs
            for (idx, input) in function.inputs.iter().enumerate() {
                let val = format!("arg{idx}");
                writeln!(f, "{}", input.rust_write_val("CALLEE_INPUTS", &val)?)?;
            }
            writeln!(f)?;

            // Report outputs and return
            if let Some(output) = &function.output {
                let ty = output.rust_arg_type()?;
                let val = output.rust_val()?;
                writeln!(f, "        let output: {ty} = {val};")?;
                writeln!(f, "{}", output.rust_write_val("CALLEE_OUTPUTS", "output")?)?;
                writeln!(
                    f,
                    "        FINISHED_FUNC.unwrap()(CALLEE_INPUTS, CALLEE_OUTPUTS);"
                )?;
                writeln!(f, "        return output;")?;
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
        let out = Command::new("rustc")
            .arg("--crate-type")
            .arg("staticlib")
            .arg("--out-dir")
            .arg("target/temp/")
            .arg(src_path)
            .output()?;

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

/// Every test should start by loading in the harness' "header"
/// and forward-declaring any structs that will be used.
fn write_rust_prefix(f: &mut dyn Write, test: &Test) -> Result<(), BuildError> {
    // Load test harness "headers"
    write!(f, "{}", RUST_TEST_PREFIX)?;

    // Forward-decl struct types
    let mut forward_decls = std::collections::HashMap::<String, String>::new();
    for function in &test.funcs {
        for val in function.inputs.iter().chain(function.output.as_ref()) {
            if let Some((name, decl)) = val.rust_forward_decl()? {
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
    fn rust_forward_decl(&self) -> Result<Option<(String, String)>, GenerateError> {
        use Val::*;
        if let Struct(name, fields) = self {
            let mut output = String::new();
            let ref_name = format!("{name}");
            output.push_str("\n#[repr(C)]\n");
            output.push_str(&format!("pub struct {name} {{\n"));
            for (idx, field) in fields.iter().enumerate() {
                let line = format!("    field{idx}: {},\n", field.rust_nested_type()?);
                output.push_str(&line);
            }
            output.push_str("}");
            Ok(Some((ref_name, output)))
        } else {
            // Don't need to forward decl any other types
            Ok(None)
        }
    }

    /// The type name to use for this value when it is stored in args/vars.
    pub fn rust_arg_type(&self) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let val = match self {
            Ref(x) => format!("*mut {}", x.rust_arg_type()?),
            Ptr(_) => format!("*mut ()"),
            Bool(_) => format!("bool"),
            Array(vals) => format!(
                "[{}; {}]",
                vals.get(0).unwrap_or(&Val::Ptr(0)).rust_arg_type()?,
                vals.len()
            ),
            Struct(name, _) => format!("{name}"),
            Float(FloatVal::c_double(_)) => format!("f64"),
            Float(FloatVal::c_float(_)) => format!("f32"),
            Int(int_val) => match int_val {
                c__int128(_) => format!("i128"),
                c_int64_t(_) => format!("i64"),
                c_int32_t(_) => format!("i32"),
                c_int16_t(_) => format!("i16"),
                c_int8_t(_) => format!("i8"),
                c__uint128(_) => format!("u128"),
                c_uint64_t(_) => format!("u64"),
                c_uint32_t(_) => format!("u32"),
                c_uint16_t(_) => format!("u16"),
                c_uint8_t(_) => format!("u8"),
            },
        };
        Ok(val)
    }

    /// The type name to use for this value when it is stored in composite.
    ///
    /// This is separated out in case there's a type that needs different
    /// handling in this context to conform to a layout (i.e. how C arrays
    /// decay into pointers when used in function args).
    fn rust_nested_type(&self) -> Result<String, GenerateError> {
        self.rust_arg_type()
    }

    /// An expression that generates this value.
    fn rust_val(&self) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let val = match self {
            Ref(x) => x.rust_val()?,
            Ptr(addr) => format!("{addr} as *const ()"),
            Bool(val) => format!("{val}"),
            Array(vals) => {
                let mut output = String::new();
                output.push_str(&format!("[",));
                for val in vals {
                    let part = format!("{}, ", val.rust_val()?);
                    output.push_str(&part);
                }
                output.push_str("]");
                output
            }
            Struct(name, fields) => {
                let mut output = String::new();
                output.push_str(&format!("{name} {{ "));
                for (idx, field) in fields.iter().enumerate() {
                    let part = format!("field{idx}: {},", field.rust_val()?);
                    output.push_str(&part);
                }
                output.push_str(" }");
                output
            }
            Float(FloatVal::c_double(val)) => format!("{val}"),
            Float(FloatVal::c_float(val)) => format!("{val}"),
            Int(int_val) => match int_val {
                c__int128(val) => format!("{val}"),
                c_int64_t(val) => format!("{val}"),
                c_int32_t(val) => format!("{val}"),
                c_int16_t(val) => format!("{val}"),
                c_int8_t(val) => format!("{val}"),
                c__uint128(val) => format!("{val}"),
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
    fn rust_write_val(&self, to: &str, from: &str) -> Result<String, GenerateError> {
        use std::fmt::Write;
        let mut output = String::new();
        for path in self.rust_var_paths(from)? {
            write!(output, "        WRITE.unwrap()({to}, &{path} as *const _ as *const _, core::mem::size_of_val(&{path}) as u32);\n").unwrap();
        }
        write!(output, "        FINISHED_VAL.unwrap()({to});").unwrap();

        Ok(output)
    }

    /// Compute the paths to every subfield of this value, with `from`
    /// as the base path to that value, for rust_write_val's use.
    fn rust_var_paths(&self, from: &str) -> Result<Vec<String>, GenerateError> {
        let paths = match self {
            Val::Int(_) | Val::Float(_) | Val::Bool(_) | Val::Ptr(_) => {
                vec![format!("{from}")]
            }
            Val::Struct(_name, fields) => {
                let mut paths = vec![];
                for (idx, field) in fields.iter().enumerate() {
                    let base = format!("{from}.field{idx}");
                    paths.extend(field.rust_var_paths(&base)?);
                }
                paths
            }
            // TODO: need to think about this
            Val::Ref(_) => return Err(GenerateError::RustUnsupported),
            // TODO: not yet implemented
            Val::Array(_) => return Err(GenerateError::RustUnsupported),
        };

        Ok(paths)
    }
}
