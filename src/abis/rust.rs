use super::super::*;
use super::*;

impl Val {
    pub fn rust_forward_decl(&self) -> Result<Option<(String, String)>, GenerateError> {
        use Val::*;
        if let Struct(name, fields) = self {
            let mut output = String::new();
            let ref_name = format!("{name}");
            output.push_str("\n#[repr(C)]\n");
            output.push_str(&format!("struct {name} {{\n"));
            for (idx, field) in fields.iter().enumerate() {
                let line = format!("    field{idx}: {},\n", field.rust_nested_type()?);
                output.push_str(&line);
            }
            output.push_str("}\n");
            Ok(Some((ref_name, output)))
        } else {
            // Don't need to forward decl any other types
            Ok(None)
        }
    }
    pub fn rust_arg_type(&self) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let val = match self {
            Ref(x) => format!("*mut {}", x.c_arg_type()?),
            Ptr(_) => format!("*mut ()"),
            Bool(_) => format!("bool"),
            Array(vals) => format!(
                "[{}; {}]",
                vals.get(0).unwrap_or(&Val::Ptr(0)).c_arg_type()?,
                vals.len()
            ),
            Struct(name, _) => format!("{name}"),
            Float(FloatVal::c_double(_)) => format!("f64"),
            Float(FloatVal::c_float(_)) => format!("f32"),
            Int(int_val) => match int_val {
                c_int128_t(_) => format!("i128"),
                c_int64_t(_) => format!("i64"),
                c_int32_t(_) => format!("i32"),
                c_int16_t(_) => format!("i16"),
                c_int8_t(_) => format!("i8"),
                c_uint128_t(_) => format!("u128"),
                c_uint64_t(_) => format!("u64"),
                c_uint32_t(_) => format!("u32"),
                c_uint16_t(_) => format!("u16"),
                c_uint8_t(_) => format!("u8"),
            },
        };
        Ok(val)
    }
    pub fn rust_val(&self) -> Result<String, GenerateError> {
        use IntVal::*;
        use Val::*;
        let val = match self {
            Ref(x) => x.rust_val()?,
            Ptr(addr) => format!("{addr} as *const ()"),
            Bool(val) => format!("{val}"),
            Array(vals) => {
                let mut output = String::new();
                output.push_str(&format!("[",));
                for (idx, val) in vals.iter().enumerate() {
                    let part = format!("{},", val.rust_val()?);
                    output.push_str(&part);
                }
                output.push_str("]");
                output
            }
            Struct(name, fields) => {
                let mut output = String::new();
                output.push_str(&format!("{name} {{"));
                for (idx, field) in fields.iter().enumerate() {
                    let part = format!("field{idx}: {},", field.rust_val()?);
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
    pub fn rust_write_val(&self, to: &str, from: &str) -> Result<String, GenerateError> {
        use std::fmt::Write;
        let mut output = String::new();
        for path in self.rust_var_paths(from)? {
            write!(output, "        WRITE.unwrap()({to}, &{path} as *const _ as *const _, core::mem::size_of_val(&{path}) as u32);\n").unwrap();
        }
        write!(output, "        FINISHED_VAL.unwrap()({to});").unwrap();

        Ok(output)
    }
    pub fn rust_var_paths(&self, from: &str) -> Result<Vec<String>, GenerateError> {
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
    pub fn rust_nested_type(&self) -> Result<String, GenerateError> {
        self.rust_arg_type()
    }
    pub fn rust_pass(&self, arg: String) -> String {
        match self {
            Val::Ref(..) | Val::Array(..) => format!("&{arg}"),
            _ => arg,
        }
    }
    pub fn rust_returned_as_out(&self) -> bool {
        match self {
            Val::Ref(..) | Val::Array(..) => true,
            _ => false,
        }
    }
}

pub fn generate_rust_caller<W: Write>(f: &mut W, test: &Test) -> Result<(), BuildError> {
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
        // writeln!(f, r#"        println!("test {}::{}\n");"#, test.name, function.name)?;
        // writeln!(f, r#"        println!("\n{}::{} rust caller inputs: ");"#, test.name, function.name)?;
        // writeln!(f)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            let ty = input.rust_arg_type()?;
            writeln!(f, "        let arg{idx}: {ty} = {};", input.rust_val()?)?;
        }
        writeln!(f)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            //    writeln!(f, r#"        println!("{{}}", arg{idx});"#)?;
            let val = format!("arg{idx}");
            writeln!(f, "{}", input.rust_write_val("CALLER_INPUTS", &val)?)?;
        }
        writeln!(f)?;
        write!(f, "        ")?;
        if let Some(output) = &function.output {
            let ty = output.rust_arg_type()?;
            write!(f, "        let output: {ty} = ")?;
        }
        write!(f, "{}(", function.name)?;
        for (idx, _input) in function.inputs.iter().enumerate() {
            write!(f, "arg{idx}, ")?;
        }
        writeln!(f, ");")?;
        writeln!(f)?;
        if let Some(output) = &function.output {
            //    writeln!(f, r#"        println!("\n{}::{} rust caller outputs: ");"#, test.name, function.name)?;
            //    writeln!(f, r#"        println!("{{}}", output);"#)?;
            writeln!(f, "{}", output.rust_write_val("CALLER_OUTPUTS", "output")?)?;
        }
        writeln!(
            f,
            "        FINISHED_FUNC.unwrap()(CALLER_INPUTS, CALLER_OUTPUTS);"
        )?;
        writeln!(f, "   }}")?;
    }

    writeln!(f, "}}")?;

    Ok(())
}

pub fn build_rust_callee(base_path: &Path, test: &str) -> Result<String, BuildError> {
    todo!()
}
pub fn build_rust_caller(base_path: &Path, test: &str) -> Result<String, BuildError> {
    let filename = format!("{test}_rust_caller.rs");
    let output_lib = format!("{test}_rust_caller");

    let mut src = PathBuf::from(base_path);
    src.push("rust");
    src.push(filename);

    let out = Command::new("rustc")
        .arg("--crate-type")
        .arg("staticlib")
        .arg("--out-dir")
        .arg("target/temp/")
        .arg(src)
        .output()?;

    if !out.status.success() {
        Err(BuildError::RustCompile(out))
    } else {
        Ok(output_lib)
    }
}
