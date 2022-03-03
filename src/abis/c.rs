use super::super::*;
use super::*;

impl Val {
    pub fn c_arg_type(&self) -> Result<String, GenerateError> {
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
                c_int128_t(_) => format!("int128_t"),
                c_int64_t(_) => format!("int64_t"),
                c_int32_t(_) => format!("int33_t"),
                c_int16_t(_) => format!("int16_t"),
                c_int8_t(_) => format!("int8_t"),
                c_uint128_t(_) => format!("uint128_t"),
                c_uint64_t(_) => format!("uint64_t"),
                c_uint32_t(_) => format!("uint32_t"),
                c_uint16_t(_) => format!("uint16_t"),
                c_uint8_t(_) => format!("uint8_t"),
            },
        };
        Ok(val)
    }
    pub fn c_nested_type(&self) -> Result<String, GenerateError> {
        self.c_arg_type()
    }

    pub fn c_forward_decl(&self) -> Result<Option<(String, String)>, GenerateError> {
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

    pub fn c_val(&self) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let val = match self {
            Ref(x) => x.c_val()?,
            Ptr(addr) => format!("(void*){addr}"),
            Bool(val) => format!("{val}"),
            Array(vals) => {
                let mut output = String::new();
                output.push_str(&format!("{{",));
                for (idx, val) in vals.iter().enumerate() {
                    if idx != 0 {
                        output.push_str(", ");
                    }
                    let part = format!("{}", val.c_val()?);
                    output.push_str(&part);
                }
                output.push_str("}");
                output
            }
            Struct(name, fields) => {
                let mut output = String::new();
                output.push_str(&format!("struct {name} {{"));
                for (idx, field) in fields.iter().enumerate() {
                    if idx != 0 {
                        output.push_str(", ");
                    }
                    let part = format!("{}", field.c_val()?);
                    output.push_str(&part);
                }
                output.push_str("}");
                output
            }
            Float(FloatVal::c_double(val)) => format!("{val}"),
            Float(FloatVal::c_float(val)) => format!("{val}"),
            Int(int_val) => match int_val {
                c_int128_t(val) => format!("{val}"),
                c_int64_t(val) => format!("{val}"),
                c_int32_t(val) => format!("{val}"),
                c_int16_t(val) => format!("{val}"),
                c_int8_t(val) => format!("{val}"),
                c_uint128_t(val) => format!("{val}"),
                c_uint64_t(val) => format!("{val}"),
                c_uint32_t(val) => format!("{val}"),
                c_uint16_t(val) => format!("{val}"),
                c_uint8_t(val) => format!("{val}"),
            },
        };
        Ok(val)
    }
    pub fn c_pass(&self, arg: String) -> String {
        match self {
            Val::Ref(..) => format!("&{arg}"),
            _ => arg,
        }
    }

    pub fn c_returned_as_out(&self) -> bool {
        match self {
            Val::Ref(..) | Val::Array(..) => true,
            _ => false,
        }
    }
    pub fn c_write_val(&self, to: &str, from: &str) -> Result<String, GenerateError> {
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
    pub fn c_var_paths(&self, from: &str) -> Result<Vec<String>, GenerateError> {
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
            Val::Ref(_) => return Err(GenerateError::RustUnsupported),
            // TODO: not yet implemented
            Val::Array(_) => return Err(GenerateError::RustUnsupported),
        };

        Ok(paths)
    }
    /*
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

pub fn generate_c_callee<W: Write>(f: &mut W, test: &Test) -> Result<(), BuildError> {
    write!(f, "{}", C_TEST_PREFIX)?;

    // Forward-decl struct types
    let mut forward_decls = std::collections::HashSet::new();
    for function in &test.funcs {
        for val in function.inputs.iter().chain(function.output.as_ref()) {
            if let Some((name, decl)) = val.c_forward_decl()? {
                if forward_decls.insert(name) {
                    writeln!(f, "{decl}")?;
                }
            }
        }
    }

    // Generate the impls
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
        writeln!(f, ") {{")?;

        // writeln!(f, r#"    printf("\n{}::{} C callee inputs: \n");"#, test.name, function.name)?;
        writeln!(f)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            // let formatter = input.cfmt();
            // writeln!(f, r#"    printf("%" {formatter} "\n", arg{idx});"#)?;
            let val = format!("arg{idx}");
            writeln!(f, "{}", input.c_write_val("CALLEE_INPUTS", &val)?)?;
        }
        writeln!(f)?;
        if let Some(output) = &function.output {
            // let formatter = output.cfmt();
            writeln!(
                f,
                "    {} output = {};",
                output.c_arg_type()?,
                output.c_val()?
            )?;
            // writeln!(f, r#"    printf("\n{}::{} C callee outputs: \n");"#, test.name, function.name)?;
            // writeln!(f, r#"    printf("%" {formatter} "\n", output);"#)?;
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

pub fn build_cc_callee(base_path: &Path, test: &str) -> Result<String, BuildError> {
    let filename = format!("{test}_c_callee.c");
    let output_lib = format!("{test}_c_callee");
    let mut src = PathBuf::from(base_path);
    src.push("c");
    src.push(filename);

    cc::Build::new()
        .file(src)
        .cargo_metadata(false)
        // .warnings_into_errors(true)
        .try_compile(&output_lib)?;
    Ok(output_lib)
}
pub fn build_cc_caller(base_path: &Path, test: &str) -> Result<String, BuildError> {
    todo!()
}
