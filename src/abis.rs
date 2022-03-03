#![allow(non_camel_case_types)]

#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    #[error("rust didn't support features of this test")]
    RustUnsupported,
    #[error("c didn't support features of this test")]
    CUnsupported,
    #[error("the function didn't have a valid convention")]
    NoCallingConvention,
}

/// A test, containing several subtests, each its own function
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Test {
    pub name: String,
    pub generated: bool,
    pub funcs: Vec<Func>,
}

/// A function's calling convention + signature which will
/// be used to generate the caller+callee automatically.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Func {
    /// The type has an opaque interface, must use a custom impl.
    /// the string is the name of the function.
    Custom(String),
    /// Emit every possible calling convention this platform supports.
    /// Each calling convention will get its own function name.
    All(Sig),
    /// Emit the platform's default C calling convention.
    C(Sig),
    // TODO: cdecl, stdcall, thiscall, fastcall, ...
}

/// The signature of a function
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Sig {
    pub name: String,
    pub inputs: Vec<Val>,
    pub output: Option<Val>,
}

/// A typed value.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum Val {
    /// A Ref is passed-by-reference (is a pointer) but the
    /// pointee will be regarded as the real value that we check.
    Ref(Box<Val>),
    /// Some integer
    Int(IntVal),
    /// Some float
    Float(FloatVal),
    /// A bool
    Bool(bool),
    /// An array (homogeneous types, checked on construction)
    Array(Vec<Val>),
    /// A named struct (heterogeneous type)
    Struct(String, Vec<Val>),
    /// An opaque pointer
    Ptr(u64),
    // TODO: vectors
    // TODO: enums (enum classes?)
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum IntVal {
    c_int128_t(i128),
    c_int64_t(i64),
    c_int32_t(i32),
    c_int16_t(i16),
    c_int8_t(i8),

    c_uint128_t(u128),
    c_uint64_t(u64),
    c_uint32_t(u32),
    c_uint16_t(u16),
    c_uint8_t(u8),
    // TODO: nastier c-types?
    // c_int(i64),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum FloatVal {
    c_double(f64),
    c_float(f32),
}

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

impl Func {
    pub fn sig(&self) -> Result<&Sig, GenerateError> {
        match self {
            Func::Custom(_) => Err(GenerateError::NoCallingConvention),
            Func::All(sig) | Func::C(sig) => Ok(sig),
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Func::Custom(name) => name,
            Func::All(sig) | Func::C(sig) => &sig.name,
        }
    }
}
