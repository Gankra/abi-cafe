//! Rust(c) codegen backend backend

use std::sync::Arc;

use kdl_script::types::{FuncIdx, ArrayTy, RefTy, AliasTy, Ty, TyIdx, PrimitiveTy};
use kdl_script::{DefinitionGraph, TypedProgram, PunEnv};

use super::super::*;
use super::*;

pub static RUST_TEST_PREFIX: &str = include_str!("../../harness/rust_test_prefix.rs");

static STRUCT_128: bool = false; // cfg!(target_arch="x86_64");

#[allow(dead_code)]
pub struct RustcAbiImpl {
    is_nightly: bool,
    codegen_backend: Option<String>,
}


pub struct Test {
    pub typed: Arc<TypedProgram>,
    pub env: Arc<PunEnv>,
    pub graph: Arc<DefinitionGraph>,
    pub convention: CallingConvention,
}

pub struct GenState<'a> {
    test: &'a Test,
    tynames: HashMap<TyIdx, String>,
    borrowed_tynames: HashMap<TyIdx, String>,
    funcs: Vec<FuncIdx>,
}

impl RustcAbiImpl {
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

    pub fn generate_caller(
        &self,
        f: &mut dyn Write,
        test: &Test,
        query: impl Iterator<Item = FuncIdx>,
    ) -> Result<(), GenerateError> {
        let mut state = GenState {
            test,
            tynames: HashMap::new(),
            borrowed_tynames: HashMap::new(),
            funcs: vec![],
        };

        self.write_rust_prefix(f, &state)?;

        for def in test.graph.definitions(query) {
            match def {
                kdl_script::Definition::DeclareTy(_) => {
                    todo!("we should intern this typename here!");
                }
                kdl_script::Definition::DeclareFunc(_) => {
                    // nothing to do, rust doesn't need forward-declares
                },
                kdl_script::Definition::DefineTy(ty) => {
                    let (tyname, borrowed_tyname) = self.generate_tydef(f, &state, ty)?;
                    state.tynames.insert(ty, tyname);
                    if let Some(borrowed) = borrowed_tyname {
                        state.borrowed_tynames.insert(ty, borrowed);
                    }
                },
                kdl_script::Definition::DefineFunc(func) => {
                    // Buffer up the funcs
                    state.funcs.push(func);
                },
            }
        }

        // Generate the extern block for all the funcs we'll call
        let convention_decl = self.convention_decl(test.convention);
        writeln!(f, "extern \"{convention_decl}\" {{",)?;
        for &func in &state.funcs {
            write!(f, "  ")?;
            self.generate_signature(f, &state, func)?;
            writeln!(f, ";")?;
        }
        writeln!(f, "}}")?;
        writeln!(f)?;

        // Generate the test function the harness will call
        writeln!(f, "#[no_mangle] pub extern \"C\" fn do_test() {{")?;

        for &func in &state.funcs {
            writeln!(f, "    unsafe {{")?;
            self.generate_caller_body(f, &state, func)?;
            writeln!(f, "    }}")?;
        }

        writeln!(f, "}}")?;

        Ok(())
    }

/*
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
        eprintln!("running: {:?}", cmd);
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
    */
}

impl RustcAbiImpl {
    pub fn new(codegen_backend: Option<String>) -> Self {
        Self {
            is_nightly: built_info::RUSTC_VERSION.contains("nightly"),
            codegen_backend,
        }
    }

    pub fn generate_caller_body(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        let function = state.test.typed.realize_func(func);
        let mut val_idx = 0;
        for (arg_idx, arg) in function.inputs.iter().enumerate() {
            let arg_name = &arg.name;
            let ty_name = &state.tynames[&arg.ty];
            write!(f, "        let {arg_name} = ")?;
            self.generate_value(f, state, arg.ty, &mut val_idx)?;
            writeln!(f, ";")?;
        }

        Ok(())
    }

    pub fn generate_tydef(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        ty: TyIdx,
    ) -> Result<(String, Option<String>), GenerateError> {
        let names = match state.test.typed.realize_ty(ty) {
            // Structural types that don't need definitions but we should
            // intern the name of
            Ty::Primitive(prim) => {
                let name = match prim {
                    PrimitiveTy::I8 => "i8",
                    PrimitiveTy::I16 => "i16",
                    PrimitiveTy::I32 => "i32",
                    PrimitiveTy::I64 => "i64",
                    PrimitiveTy::I128 => "i128",
                    PrimitiveTy::I256 => "i256",
                    PrimitiveTy::U8 => "u8",
                    PrimitiveTy::U16 => "u16",
                    PrimitiveTy::U32 => "u32",
                    PrimitiveTy::U64 => "u64",
                    PrimitiveTy::U128 => "u128",
                    PrimitiveTy::U256 => "u256",
                    PrimitiveTy::F16 => "f16",
                    PrimitiveTy::F32 => "f32",
                    PrimitiveTy::F64 => "f64",
                    PrimitiveTy::F128 => "f128",
                    PrimitiveTy::Bool => "bool",
                    PrimitiveTy::Ptr => "*mut ()",
                };
                (name.to_owned(), None)
            },
            Ty::Array(ArrayTy { elem_ty, len }) => {

                let elem_tyname = &state.tynames[elem_ty];
                let borrowed_tyname = state.borrowed_tynames.get(elem_ty).map(|elem_tyname| format!("[{elem_tyname}; {len}]"));
                (format!("[{elem_tyname}; {len}]"), borrowed_tyname)
            },
            Ty::Ref(RefTy { pointee_ty }) => {
                let pointee_tyname = &state.tynames[pointee_ty];
                let borrowed_pointee_tyname = state.borrowed_tynames.get(pointee_ty).unwrap_or(pointee_tyname);
                (format!("&mut {pointee_tyname}"), Some(format!("&'a mut {borrowed_pointee_tyname}")))
            }
            Ty::Empty => {
                ("()".to_owned(), None)
            }

            // Nominal types we need to emit a decl for
            Ty::Struct(struct_ty) => {
                assert!(struct_ty.attrs.is_empty(), "don't yet know how to apply attrs to structs");

                let has_borrows = struct_ty.fields.iter().any(|field| state.borrowed_tynames.contains_key(&field.ty));

                // Emit an actual struct decl
                writeln!(f, "#[repr(C)]")?;
                if has_borrows {
                    writeln!(f, "struct {}<'a> {{", struct_ty.name)?;
                } else {
                    writeln!(f, "struct {} {{", struct_ty.name)?;
                }
                for field in &struct_ty.fields {
                    let field_name = &field.ident;
                    let field_tyname = state.borrowed_tynames.get(&field.ty).unwrap_or(&state.tynames[&field.ty]);
                    writeln!(f, "    {field_name}: {field_tyname},")?;
                }
                writeln!(f, "}}\n")?;

                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", struct_ty.name));
                ((*struct_ty.name).clone(), borrowed_tyname)
            },
            Ty::Union(union_ty) => {
                assert!(union_ty.attrs.is_empty(), "don't yet know how to apply attrs to unions");

                let has_borrows = union_ty.fields.iter().any(|field| state.borrowed_tynames.contains_key(&field.ty));

                // Emit an actual union decl
                writeln!(f, "#[repr(C)]")?;
                if has_borrows {
                    writeln!(f, "union {}<'a> {{", union_ty.name)?;
                } else {
                    writeln!(f, "union {} {{", union_ty.name)?;
                }
                for field in &union_ty.fields {
                    let field_name = &field.ident;
                    let field_tyname = state.borrowed_tynames.get(&field.ty).unwrap_or(&state.tynames[&field.ty]);
                    writeln!(f, "    {field_name}: {field_tyname},")?;
                }
                writeln!(f, "}}\n")?;

                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", union_ty.name));
                ((*union_ty.name).clone(), borrowed_tyname)
            },
            Ty::Enum(enum_ty) => {
                assert!(enum_ty.attrs.is_empty(), "don't yet know how to apply attrs to enums");

                // Emit an actual enum decl
                writeln!(f, "#[repr(C)]")?;
                writeln!(f, "enum {} {{", enum_ty.name)?;
                for variant in &enum_ty.variants {
                    let variant_name = &variant.name;
                    writeln!(f, "    {variant_name},")?;
                }
                writeln!(f, "}}\n")?;

                ((*enum_ty.name).clone(), None)
            },
            Ty::Tagged(tagged_ty) => {
                assert!(tagged_ty.attrs.is_empty(), "don't yet know how to apply attrs to tagged unions");

                let has_borrows = tagged_ty.variants.iter().any(|v| v.fields.as_ref().map(|fields| fields.iter().any(|field|state.borrowed_tynames.contains_key(&field.ty))).unwrap_or(false));

                // Emit an actual enum decl
                writeln!(f, "#[repr(C)]")?;
                if has_borrows {
                    writeln!(f, "enum {}<'a> {{", tagged_ty.name)?;
                } else {
                    writeln!(f, "enum {} {{", tagged_ty.name)?;
                }
                for variant in &tagged_ty.variants {
                    let variant_name = &variant.name;
                    if let Some(fields) = &variant.fields {
                        writeln!(f, "    {variant_name} {{")?;
                        for field in fields {
                            let field_name = &field.ident;
                            let field_tyname = state.borrowed_tynames.get(&field.ty).unwrap_or(&state.tynames[&field.ty]);
                            writeln!(f, "        {field_name}: {field_tyname},")?;
                        }
                    } else {
                        writeln!(f, "    {variant_name},")?;
                    }
                    writeln!(f, "    }}")?;
                }
                writeln!(f, "}}\n")?;

                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", tagged_ty.name));
                ((*tagged_ty.name).clone(), borrowed_tyname)
            },
            Ty::Alias(AliasTy { name, real, attrs }) => {
                assert!(attrs.is_empty(), "don't yet know how to apply attrs to type aliases");

                // Emit an actual type alias decl
                if let Some(real_tyname) = state.borrowed_tynames.get(&real) {
                    writeln!(f, "type {name}<'a> = {real_tyname};\n")?;
                    ((**name).clone(), Some(format!("{name}<'a>")))
                } else {
                    let real_tyname = &state.tynames[&real];
                    writeln!(f, "type {name} = {real_tyname};\n")?;
                    ((**name).clone(), None)
                }
            },

            // Puns should be evaporated
            Ty::Pun(pun) => {
                let real_ty = state.test.typed.resolve_pun(pun, &state.test.env).unwrap();
                (state.tynames[&real_ty].clone(), state.borrowed_tynames.get(&real_ty).cloned())
            },
        };

        Ok(names)
    }

    pub fn generate_value(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        ty: TyIdx,
        val_idx: &mut usize,
    ) -> Result<(), GenerateError> {
        let names = match state.test.typed.realize_ty(ty) {
            // Primitives are the only "real" values with actual bytes that advance val_idx
            Ty::Primitive(prim) => {
                match prim {
                    PrimitiveTy::I8 => write!(f, "{:#X}", graffiti_primitive::<i8>(*val_idx))?,
                    PrimitiveTy::I16 => write!(f, "{:#X}", graffiti_primitive::<i16>(*val_idx))?,
                    PrimitiveTy::I32 => write!(f, "{:#X}", graffiti_primitive::<i32>(*val_idx))?,
                    PrimitiveTy::I64 => write!(f, "{:#X}", graffiti_primitive::<i64>(*val_idx))?,
                    PrimitiveTy::I128 => write!(f, "{:#X}", graffiti_primitive::<i128>(*val_idx))?,
                    PrimitiveTy::U8 => write!(f, "{:#X}", graffiti_primitive::<u8>(*val_idx))?,
                    PrimitiveTy::U16 => write!(f, "{:#X}", graffiti_primitive::<u16>(*val_idx))?,
                    PrimitiveTy::U32 => write!(f, "{:#X}", graffiti_primitive::<u32>(*val_idx))?,
                    PrimitiveTy::U64 => write!(f, "{:#X}", graffiti_primitive::<u64>(*val_idx))?,
                    PrimitiveTy::U128 => write!(f, "{:#X}", graffiti_primitive::<u128>(*val_idx))?,

                    PrimitiveTy::F32 => write!(f, "{}", graffiti_primitive::<f32>(*val_idx))?,
                    PrimitiveTy::F64 => write!(f, "{}", graffiti_primitive::<f64>(*val_idx))?,
                    PrimitiveTy::Bool => write!(f, "true")?,
                    PrimitiveTy::Ptr => {
                        if true {
                            write!(f, "{:X} as *mut ()", graffiti_primitive::<u64>(*val_idx))?
                        } else {
                            write!(f, "{:X} as *mut ()", graffiti_primitive::<u32>(*val_idx))?
                        }
                    },
                    PrimitiveTy::I256 => Err(GenerateError::RustUnsupported(format!("rust doesn't have i256")))?,
                    PrimitiveTy::U256 => Err(GenerateError::RustUnsupported(format!("rust doesn't have u256")))?,
                    PrimitiveTy::F16 => Err(GenerateError::RustUnsupported(format!("rust doesn't have f16")))?,
                    PrimitiveTy::F128 => Err(GenerateError::RustUnsupported(format!("rust doesn't have f128")))?,
                };
                *val_idx += 1;
            },
            Ty::Empty => {
                write!(f, "()")?;
            }
            Ty::Ref(RefTy { pointee_ty }) => {
                todo!("we need to forward declare this variable so we can pass it in")
            }
            Ty::Array(ArrayTy { elem_ty, len }) => {
                write!(f, "[")?;
                for arr_idx in 0..*len {
                    if arr_idx > 0 {
                        write!(f, ", ")?;
                    }
                    self.generate_value(f, state, *elem_ty, val_idx)?;
                }
                write!(f, "]")?;
            },
            // Nominal types we need to emit a decl for
            Ty::Struct(struct_ty) => {
                let name = &struct_ty.name;
                write!(f, "{name} {{ ")?;
                for (field_idx, field) in struct_ty.fields.iter().enumerate() {
                    if field_idx > 0 {
                        write!(f, ", ")?;
                    }
                    let field_name = &field.ident;
                    write!(f, "{field_name}: ")?;
                    self.generate_value(f, state, field.ty, val_idx)?;
                }
                write!(f, " }}")?;
            },
            Ty::Union(union_ty) => {
                let name = &union_ty.name;
                write!(f, "{name} {{ ")?;
                // FIXME: have a way to pick the variant!
                if let Some(field) = union_ty.fields.get(0) {
                    let field_name = &field.ident;
                    write!(f, "{field_name}: ")?;
                    self.generate_value(f, state, field.ty, val_idx)?;
                }
                write!(f, " }}")?;
            },
            Ty::Enum(enum_ty) => {
                let name = &enum_ty.name;
                // FIXME: have a way to pick the variant!
                if let Some(variant) = enum_ty.variants.get(0) {
                    let variant_name = &variant.name;
                    write!(f, "{name}::{variant_name}")?;
                }
            },
            Ty::Tagged(tagged_ty) => {
                let name = &tagged_ty.name;
                // FIXME: have a way to pick the variant!
                if let Some(variant) = tagged_ty.variants.get(0) {
                    let variant_name = &variant.name;
                    write!(f, "{name}::{variant_name}")?;
                    if let Some(fields) = &variant.fields {
                        write!(f, " {{ ")?;
                        for (field_idx, field) in fields.iter().enumerate() {
                            if field_idx > 0 {
                                write!(f, ", ")?;
                            }
                            let field_name = &field.ident;
                            writeln!(f, "{field_name}: ")?;
                            self.generate_value(f, state, ty, val_idx)?;
                        }
                        write!(f, " }}")?;
                    }
                }
            },
            Ty::Alias(AliasTy { real, .. }) => {
                self.generate_value(f, state, *real, val_idx)?;
            },

            // Puns should be evaporated
            Ty::Pun(pun) => {
                let real_ty = state.test.typed.resolve_pun(pun, &state.test.env).unwrap();
                self.generate_value(f, state, real_ty, val_idx)?;
            },
        };

        Ok(names)
    }

    fn convention_decl(&self, convention: CallingConvention) -> &'static str {
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
        state: &GenState,
    ) -> Result<(), GenerateError> {
        if state.test.convention == CallingConvention::Vectorcall {
            writeln!(f, "#![feature(abi_vectorcall)]")?;
        }
        // Load test harness "headers"
        writeln!(f, "{}", RUST_TEST_PREFIX)?;
        writeln!(f)?;

        Ok(())
    }

    fn generate_signature(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        let function = state.test.typed.realize_func(func);

        write!(f, "fn {}(", function.name)?;

        // Add inputs
        for (_idx, arg) in function.inputs.iter().enumerate() {
            let arg_name = &arg.name;
            let arg_ty = &state.tynames[&arg.ty];
            write!(f, "{}: {}, ", arg_name, arg_ty)?;
        }
        // Add outparams
        for (_idx, arg) in function.outputs.iter().enumerate() {
            let is_outparam = state.borrowed_tynames.contains_key(&arg.ty);
            if !is_outparam {
                // Handled in next loop
                continue;
            }
            // NOTE: we intentionally don't use the "borrowed" tyname
            // as we still don't need lifetimes here!
            let arg_name = &arg.name;
            let arg_ty = &state.tynames[&arg.ty];
            write!(f, "{}: {}, ", arg_name, arg_ty)?;
        }
        // Add normal returns
        let mut has_normal_return = false;
        for (_idx, arg) in function.outputs.iter().enumerate() {
            let is_outparam = state.borrowed_tynames.contains_key(&arg.ty);
            if is_outparam {
                // Already handled
                continue;
            }
            if has_normal_return {
                return Err(GenerateError::RustUnsupported(format!("multiple normal returns (should this be a tuple?)")));
            }
            has_normal_return = true;
            let arg_ty = &state.tynames[&arg.ty];
            write!(f, ") -> {}", arg_ty)?;
        }
        if !has_normal_return {
            write!(f, ")")?;
        }
        Ok(())
    }

/*
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
     */
}

/// For a given primitive type, generate an instance
/// where all the high nybbles of each byte is val_idx
/// and all the low nybbles are the number byte.
///
/// This lets us look at a random byte a function read
/// and go "hey this was SUPPOSED to be the 3rd byte of the 7th arg",
/// which is useful for figuring out how an argument got fucked up
/// (how much it was misaligned, or passed in the wrong slot).
fn graffiti_primitive<T: Copy>(val_idx: usize) -> T {
    const MAX_SIZE: usize = 32;
    const MAX_HEX: usize = 16;
    assert!(std::mem::size_of::<T>() <= MAX_SIZE, "only primitives as big as u256 are supported!");

    let bytes: [u8; MAX_SIZE] = std::array::from_fn(|i| {
        (0x10 * (val_idx % MAX_HEX) as u8) | ((i % MAX_HEX) as u8)
    });
    unsafe {
        std::mem::transmute_copy(&bytes)
    }
}