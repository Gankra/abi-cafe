//! Rust(c) codegen backend backend

use std::sync::Arc;

use kdl_script::parse::FuncDecl;
use kdl_script::types::{FuncIdx, Func, ArrayTy, RefTy, AliasTy, Ty, TyIdx, PrimitiveTy};
use kdl_script::{DefinitionGraph, TypedProgram, PunEnv};

use super::super::*;
use super::*;

pub static RUST_TEST_PREFIX: &str = include_str!("../../harness/rust_test_prefix.rs");

const VAR_CALLER_INPUTS: &str = "CALLER_INPUTS";
const VAR_CALLER_OUTPUTS: &str = "CALLER_OUTPUTS";
const VAR_CALLEE_INPUTS: &str = "CALLEE_INPUTS";
const VAR_CALLEE_OUTPUTS: &str = "CALLEE_OUTPUTS";

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
    val_writer: WriteImpl,
}

enum WriteImpl {
    HarnessCallback,
    Print,
    Noop,
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

    /*
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
    pub fn generate_caller(
        &self,
        f: &mut dyn Write,
        test: &Test,
        query: impl Iterator<Item = FuncIdx>,
    ) -> Result<(), GenerateError> {
        let mut state = gen_state(test);

        // Generate type decls and gather up functions
        self.generate_definitions(f, &mut state, query)?;
        // Generate decls of the functions we want to call
        self.generate_caller_externs(f, &state)?;

        // Generate the test function the harness will call
        writeln!(f, "#[no_mangle] pub extern \"C\" fn do_test() {{")?;
        for &func in &state.funcs {
            // Generate the individual function calls
            self.generate_caller_body(f, &state, func)?;
        }
        writeln!(f, "}}")?;

        Ok(())
    }

    fn generate_caller_externs(
        &self,
        f: &mut dyn Write,
        state: &GenState,
    ) -> Result<(), GenerateError>  {
        let convention_decl = self.convention_decl(state.test.convention)?;
        writeln!(f, "extern \"{convention_decl}\" {{",)?;
        for &func in &state.funcs {
            write!(f, "  ")?;
            self.generate_signature(f, &state, func)?;
            writeln!(f, ";")?;
        }
        writeln!(f, "}}")?;
        writeln!(f)?;
        Ok(())
    }

    fn generate_caller_body(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        writeln!(f, "    unsafe {{")?;
        let function = state.test.typed.realize_func(func);
        let mut val_idx = 0;

        // Create vars for all the inputs
        for arg in function.inputs.iter() {
            // Create and report the input
            self.create_var(f, state, &arg.name, arg.ty, &mut val_idx)?;
            self.write_var(f, state, &arg.name, arg.ty, VAR_CALLER_INPUTS)?;
        }

        // Call the function
        self.call_function(f, state, function)?;

        // Report all the outputs
        for arg in function.outputs.iter() {
            let arg_name = &arg.name;
            self.write_var(f, state, arg_name, arg.ty, VAR_CALLER_OUTPUTS)?;
        }

        // Report the function is complete
        self.end_function(f, state, VAR_CALLER_INPUTS, VAR_CALLER_OUTPUTS)?;
        writeln!(f, "    }}")?;
        Ok(())
    }


    fn call_function(&self, f: &mut dyn Write, state: &GenState, function: &Func) -> Result<(), GenerateError> {
        let func_name = &function.name;
        write!(f, "        ")?;

        // if there's an output, bind it
        let mut proper_outputs = function.outputs.iter().filter(|arg| !state.borrowed_tynames.contains_key(&arg.ty));
        let output = proper_outputs.next();
        let too_many_outputs = proper_outputs.next();
        if too_many_outputs.is_some() {
            return Err(GenerateError::RustUnsupported(format!("multiple normal returns (should this be a tuple?)")));
        }
        if let Some(output) = output {
            write!(f, "let {} = ", output.name)?;
        }

        // Call the function
        write!(f, "{func_name}(")?;
        let inputs = function.inputs.iter();
        let out_params = function.outputs.iter().filter(|arg| state.borrowed_tynames.contains_key(&arg.ty));

        for (arg_idx, arg) in inputs.chain(out_params).enumerate() {
            if arg_idx > 0 {
                write!(f, ", ")?;
            }
            self.pass_var(f, state, &arg.name, arg.ty)?;
        }
        writeln!(f, ");")?;
        writeln!(f)?;
        Ok(())
    }

}

impl RustcAbiImpl {
    pub fn generate_callee(
        &self,
        f: &mut dyn Write,
        test: &Test,
        query: impl Iterator<Item = FuncIdx>,
    ) -> Result<(), GenerateError> {
        let mut state = gen_state(test);

        // Generate type decls and gather up functions
        self.generate_definitions(f, &mut state, query)?;

        for &func in &state.funcs {
            // Generate the individual function definitions
            self.generate_callee_body(f, &state, func)?;
        }
        Ok(())
    }

    fn generate_callee_body(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        let function = state.test.typed.realize_func(func);
        let convention_decl = self.convention_decl(state.test.convention)?;
        writeln!(f, "#[no_mangle]")?;
        write!(f, "pub unsafe extern \"{convention_decl}\" ")?;
        self.generate_signature(f, &state, func)?;
        writeln!(f, " {{")?;
        writeln!(f, "    unsafe {{")?;
        // Report the inputs
        for arg in function.inputs.iter() {
            let arg_name = &arg.name;
            self.write_var(f, state, arg_name, arg.ty, VAR_CALLEE_INPUTS)?;
        }

        // Create outputs and report them
        let mut val_idx = 0;
        for arg in function.outputs.iter() {
            // Create and report the input
            self.create_var(f, state, &arg.name, arg.ty, &mut val_idx)?;
            self.write_var(f, state, &arg.name, arg.ty, VAR_CALLEE_OUTPUTS)?;
        }

        // Report the function is complete
        self.end_function(f, state, VAR_CALLEE_INPUTS, VAR_CALLEE_OUTPUTS)?;

        // Return the outputs
        for arg in function.outputs.iter() {
            self.return_var(f, state, &arg.name, arg.ty)?;
        }
        writeln!(f, "    }}")?;
        writeln!(f, "}}")?;
        Ok(())
    }
}

impl RustcAbiImpl {
    pub fn new(codegen_backend: Option<String>) -> Self {
        Self {
            is_nightly: built_info::RUSTC_VERSION.contains("nightly"),
            codegen_backend,
        }
    }

    pub fn generate_definitions(&self,
        f: &mut dyn Write,
        state: &mut GenState,
        query: impl Iterator<Item = FuncIdx>
    ) -> Result<(), GenerateError> {
        self.write_harness_prefix(f, &state)?;

        for def in state.test.graph.definitions(query) {
            match def {
                kdl_script::Definition::DeclareTy(ty) => {
                    self.intern_tyname(f, state, ty)?;
                }
                kdl_script::Definition::DefineTy(ty) => {
                    self.generate_tydef(f, state, ty)?;
                },
                kdl_script::Definition::DefineFunc(func) => {
                    // Buffer up the funcs
                    state.funcs.push(func);
                },
                kdl_script::Definition::DeclareFunc(_) => {
                    // nothing to do, executable kdl-script isn't real and can't hurt us
                },
            }
        }

        Ok(())
    }


    pub fn intern_tyname(
        &self,
        f: &mut dyn Write,
        state: &mut GenState,
        ty: TyIdx,
    ) -> Result<(), GenerateError> {
        // Don't double-intern
        if state.tynames.contains_key(&ty) {
            return Ok(());
        }

        let (tyname, borrowed_tyname) = match state.test.typed.realize_ty(ty) {
            // Structural types that don't need definitions but we should
            // intern the name of
            Ty::Primitive(prim) => {
                let name = match prim {
                    PrimitiveTy::I8 => "i8",
                    PrimitiveTy::I16 => "i16",
                    PrimitiveTy::I32 => "i32",
                    PrimitiveTy::I64 => "i64",
                    PrimitiveTy::I128 => "i128",
                    PrimitiveTy::U8 => "u8",
                    PrimitiveTy::U16 => "u16",
                    PrimitiveTy::U32 => "u32",
                    PrimitiveTy::U64 => "u64",
                    PrimitiveTy::U128 => "u128",
                    PrimitiveTy::F32 => "f32",
                    PrimitiveTy::F64 => "f64",
                    PrimitiveTy::Bool => "bool",
                    PrimitiveTy::Ptr => "*mut ()",
                    PrimitiveTy::I256 => Err(GenerateError::RustUnsupported(format!("rust doesn't have i256")))?,
                    PrimitiveTy::U256 => Err(GenerateError::RustUnsupported(format!("rust doesn't have u256")))?,
                    PrimitiveTy::F16 => Err(GenerateError::RustUnsupported(format!("rust doesn't have f16")))?,
                    PrimitiveTy::F128 => Err(GenerateError::RustUnsupported(format!("rust doesn't have f128")))?,
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
                let has_borrows = struct_ty.fields.iter().any(|field| state.borrowed_tynames.contains_key(&field.ty));
                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", struct_ty.name));
                ((*struct_ty.name).clone(), borrowed_tyname)
            },
            Ty::Union(union_ty) => {
                let has_borrows = union_ty.fields.iter().any(|field| state.borrowed_tynames.contains_key(&field.ty));
                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", union_ty.name));
                ((*union_ty.name).clone(), borrowed_tyname)
            },
            Ty::Enum(enum_ty) => {
                ((*enum_ty.name).clone(), None)
            },
            Ty::Tagged(tagged_ty) => {
                let has_borrows = tagged_ty.variants.iter().any(|v| v.fields.as_ref().map(|fields| fields.iter().any(|field|state.borrowed_tynames.contains_key(&field.ty))).unwrap_or(false));
                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", tagged_ty.name));
                ((*tagged_ty.name).clone(), borrowed_tyname)
            },
            Ty::Alias(AliasTy { name, real, attrs }) => {
                let borrowed_tyname = state.borrowed_tynames.get(&real).map(|name| format!("{name}<'a>"));
                ((**name).clone(), borrowed_tyname)
            },

            // Puns should be evaporated
            Ty::Pun(pun) => {
                let real_ty = state.test.typed.resolve_pun(pun, &state.test.env).unwrap();
                (state.tynames[&real_ty].clone(), state.borrowed_tynames.get(&real_ty).cloned())
            },
        };

        state.tynames.insert(ty, tyname);
        if let Some(borrowed) = borrowed_tyname {
            state.borrowed_tynames.insert(ty, borrowed);
        }

        Ok(())
    }

    pub fn generate_tydef(
        &self,
        f: &mut dyn Write,
        state: &mut GenState,
        ty: TyIdx,
    ) -> Result<(), GenerateError> {
        // Make sure our own name is interned
        self.intern_tyname(f, state, ty)?;

        match state.test.typed.realize_ty(ty) {
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
            },
            Ty::Alias(AliasTy { name, real, attrs }) => {
                assert!(attrs.is_empty(), "don't yet know how to apply attrs to type aliases");

                // Emit an actual type alias decl
                if let Some(real_tyname) = state.borrowed_tynames.get(&real) {
                    writeln!(f, "type {name}<'a> = {real_tyname};\n")?;
                } else {
                    let real_tyname = &state.tynames[&real];
                    writeln!(f, "type {name} = {real_tyname};\n")?;
                }
            },
            Ty::Pun(..) => {
                // Puns should be evaporated by the type name interner
            },
            Ty::Primitive(prim) => {
                match prim {
                    PrimitiveTy::I8 | PrimitiveTy::I16 | PrimitiveTy::I32 |
                    PrimitiveTy::I64 |
                    PrimitiveTy::I128 |
                    PrimitiveTy::I256 |
                    PrimitiveTy::U8 |
                    PrimitiveTy::U16 |
                    PrimitiveTy::U32 |
                    PrimitiveTy::U64 |
                    PrimitiveTy::U128 |
                    PrimitiveTy::U256 |
                    PrimitiveTy::F16 |
                    PrimitiveTy::F32 |
                    PrimitiveTy::F64 |
                    PrimitiveTy::F128 |
                    PrimitiveTy::Bool |
                    PrimitiveTy::Ptr => {
                        // Builtin
                    },
                };
            },
            Ty::Array(ArrayTy { .. }) => {
                // Builtin
            },
            Ty::Ref(RefTy { .. }) => {
                // Builtin
            }
            Ty::Empty => {
                // Builtin
            }
        }
        Ok(())
    }

    pub fn generate_value(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        ty: TyIdx,
        val_idx: &mut usize,
        alias: Option<&str>,
        ref_temp_name: &str,
        extra_decls: &mut Vec<Vec<u8>>,
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
                    PrimitiveTy::Bool => write!(f, "{}", true)?,
                    PrimitiveTy::Ptr => {
                        if true {
                            write!(f, "{:#X} as *mut ()", graffiti_primitive::<u64>(*val_idx))?
                        } else {
                            write!(f, "{:#X} as *mut ()", graffiti_primitive::<u32>(*val_idx))?
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
                // The value is a mutable reference to a temporary
                write!(f, "&mut {ref_temp_name}")?;

                // TODO: should this be a recursive call to create_var (need create_var_inner?)
                // Now do the rest of the recursion on constructing the temporary
                let mut ref_temp = Vec::new();
                let ty_name = &state.tynames[pointee_ty];
                write!(&mut ref_temp, "        let mut {ref_temp_name}: {ty_name} = ")?;
                let ref_temp_name = format!("{ref_temp_name}_");
                self.generate_value(&mut ref_temp, state, *pointee_ty, val_idx, alias, &ref_temp_name, extra_decls)?;
                write!(&mut ref_temp, ";")?;
                extra_decls.push(ref_temp);
            }
            Ty::Array(ArrayTy { elem_ty, len }) => {
                write!(f, "[")?;
                for arr_idx in 0..*len {
                    if arr_idx > 0 {
                        write!(f, ", ")?;
                    }
                    let ref_temp_name = format!("{ref_temp_name}{arr_idx}_");
                    self.generate_value(f, state, *elem_ty, val_idx, alias, &ref_temp_name, extra_decls)?;
                }
                write!(f, "]")?;
            },
            // Nominal types we need to emit a decl for
            Ty::Struct(struct_ty) => {
                let name = alias.unwrap_or(&struct_ty.name);
                write!(f, "{name} {{ ")?;
                for (field_idx, field) in struct_ty.fields.iter().enumerate() {
                    if field_idx > 0 {
                        write!(f, ", ")?;
                    }
                    let field_name = &field.ident;
                    write!(f, "{field_name}: ")?;
                    let ref_temp_name = format!("{ref_temp_name}{field_name}_");
                    self.generate_value(f, state, field.ty, val_idx, alias, &ref_temp_name, extra_decls)?;
                }
                write!(f, " }}")?;
            },
            Ty::Union(union_ty) => {
                let name = alias.unwrap_or(&union_ty.name);
                write!(f, "{name} {{ ")?;
                // FIXME(variant_select):have a way to pick the variant!
                if let Some(field) = union_ty.fields.get(0) {
                    let field_name = &field.ident;
                    write!(f, "{field_name}: ")?;
                    let ref_temp_name = format!("{ref_temp_name}{field_name}_");
                    self.generate_value(f, state, field.ty, val_idx, alias, &ref_temp_name, extra_decls)?;
                }
                write!(f, " }}")?;
            },
            Ty::Enum(enum_ty) => {
                let name = alias.unwrap_or(&enum_ty.name);
                // FIXME(variant_select):have a way to pick the variant!
                if let Some(variant) = enum_ty.variants.get(0) {
                    let variant_name = &variant.name;
                    write!(f, "{name}::{variant_name}")?;
                }
            },
            Ty::Tagged(tagged_ty) => {
                let name = alias.unwrap_or(&tagged_ty.name);
                // FIXME(variant_select): have a way to pick the variant!
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
                            write!(f, "{field_name}: ")?;
                            let ref_temp_name = format!("{ref_temp_name}{field_name}_");
                            self.generate_value(f, state, field.ty, val_idx, alias, &ref_temp_name, extra_decls)?;
                        }
                        write!(f, " }}")?;
                    }
                }
            },
            Ty::Alias(AliasTy { real, name, .. }) => {
                let alias = alias.or_else(|| Some(name));
                self.generate_value(f, state, *real, val_idx, alias, ref_temp_name, extra_decls)?;
            },

            // Puns should be evaporated
            Ty::Pun(pun) => {
                let real_ty = state.test.typed.resolve_pun(pun, &state.test.env).unwrap();
                self.generate_value(f, state, real_ty, val_idx, alias, ref_temp_name, extra_decls)?;
            },
        };

        Ok(names)
    }

    fn convention_decl(&self, convention: CallingConvention) -> Result<&'static str, GenerateError> {
        let conv = match convention {
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
        };
        Ok(conv)
    }

    /// Every test should start by loading in the harness' "header"
    /// and forward-declaring any structs that will be used.
    fn write_harness_prefix(
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

    fn create_var(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        var_name: &str,
        var_ty: TyIdx,
        val_idx: &mut usize,
    ) -> Result<(), GenerateError> {
        // Generate the input
        let ty_name = &state.tynames[&var_ty];
        let needs_mut = false;
        let let_mut = if needs_mut { "let mut" } else { "let" };
        let mut real_var_decl = Vec::new();
        let mut extra_decls = Vec::new();
        write!(&mut real_var_decl, "        {let_mut} {var_name}: {ty_name} = ")?;
        let ref_temp_name = format!("{var_name}_");
        self.generate_value(&mut real_var_decl, state, var_ty, val_idx, None, &ref_temp_name, &mut extra_decls)?;
        writeln!(&mut real_var_decl, ";")?;

        for decl in extra_decls {
            writeln!(f, "{}", std::str::from_utf8(&decl).unwrap())?;
        }
        writeln!(f, "{}", std::str::from_utf8(&real_var_decl).unwrap())?;
        Ok(())
    }

    /// Emit the WRITE calls and FINISHED_VAL for this value.
    /// This will WRITE every leaf subfield of the type.
    /// `to` is the BUFFER to use, `from` is the variable name of the value.
    fn write_var(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        var_name: &str,
        var_ty: TyIdx,
        to: &str,
    ) -> Result<(), GenerateError> {
        let mut indent = 8;
        use VarPathPart::*;
        match state.val_writer {
            WriteImpl::HarnessCallback => {
                for part in self.var_paths(state, var_name, var_ty, true)? {
                    match part {
                        Gunk { text, preindent_change, postindent_change } => {
                            indent += preindent_change;
                            writeln!(f, "{:indent$}{}", "", text, indent=indent as usize)?;
                            indent += postindent_change;
                        }
                        Field(path) => {
                            writeln!(f, "{:indent$}write_field({to}, &{path});", "", indent=indent as usize)?;
                        }
                    }
                }
                writeln!(f, "{:indent$}finished_val({to});", "", indent=indent as usize)?;
                writeln!(f)?;
            },
            WriteImpl::Print => {
                for part in self.var_paths(state, var_name, var_ty, true)? {
                    match part {
                        Gunk { text, preindent_change, postindent_change } => {
                            indent += preindent_change;
                            writeln!(f, "{:indent$}{}", text, "", indent=indent as usize)?;
                            indent += postindent_change;
                        }
                        Field(path) => {
                            writeln!(f, "{:indent$}println!(\"{{}}\", {path});", "", indent=indent as usize)?;
                        }
                    }
                }
            },
            WriteImpl::Noop => {
                // Noop, do nothing
            },
        }
        Ok(())
    }

    fn end_function(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        inputs: &str,
        outputs: &str,
    ) -> Result<(), GenerateError> {
        match state.val_writer {
            WriteImpl::HarnessCallback => {
                writeln!(f, "        finished_func({inputs}, {outputs});")?;
            },
            WriteImpl::Print | WriteImpl::Noop => {
                // Noop
            },
        }
        Ok(())
    }

    fn pass_var(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        var_name: &str,
        var_ty: TyIdx,
    ) -> Result<(), GenerateError> {
        write!(f, "{var_name}")?;
        Ok(())
    }

    fn return_var(
        &self,
        f: &mut dyn Write,
        state: &GenState,
        var_name: &str,
        var_ty: TyIdx,
    ) -> Result<(), GenerateError> {
        // TODO: implement outparam returns
        write!(f, "{var_name}")?;
        Ok(())
    }

    /// Compute the paths to every subfield of this value, with `from`
    /// as the base path to that value, for rust_write_val's use.
    fn var_paths(
        &self,
        state: &GenState,
        from: &str,
        var_ty: TyIdx,
        is_root: bool,
    ) -> Result<Vec<VarPathPart>, GenerateError> {
        use VarPathPart::*;
        let paths = match state.test.typed.realize_ty(var_ty) {
            Ty::Primitive(_) | Ty::Enum(_) => vec![Field(from.to_owned())],
            Ty::Empty => vec![],
            Ty::Alias(alias_ty) => self.var_paths(state, from, alias_ty.real, is_root)?,
            Ty::Pun(pun) => {
                let real_ty = state.test.typed.resolve_pun(pun, &state.test.env).unwrap();
                self.var_paths(state, from, real_ty, is_root)?
            }
            Ty::Array(array_ty) => {
                let mut paths = vec![];
                for i in 0..array_ty.len {
                    let base = format!("{from}[{i}]");
                    paths.extend(self.var_paths(state, &base, array_ty.elem_ty, false)?)
                }
                paths
            }
            Ty::Struct(struct_ty) => {
                let mut paths = vec![];
                for field in &struct_ty.fields {
                    let field_name = &field.ident;
                    let base = format!("{from}.{field_name}");
                    paths.extend(self.var_paths(state, &base, field.ty, false)?)
                }
                paths
            },
            Ty::Tagged(tagged_ty) => {
                let mut paths = vec![];

                paths.push(Gunk {
                    text: format!("match &{from} {{"),
                    preindent_change: 0,
                    postindent_change: 4,
                });
                for variant in &tagged_ty.variants {
                    let variant_name = &variant.name;
                    match &variant.fields {
                        Some(fields) => {
                            let field_list = fields.iter().map(|f| f.ident.to_string()).collect::<Vec<_>>().join(", ");
                            paths.push(Gunk {
                                text: format!("{variant_name} {{ {field_list} }} => {{"),
                                preindent_change: 0,
                                postindent_change: 4,
                            });
                            for field in fields {
                                paths.extend(self.var_paths(state, &field.ident, field.ty, false)?)
                            }
                            paths.push(Gunk {
                                text: format!("}}"),
                                preindent_change: -4,
                                postindent_change: 0,
                            });
                        }
                        None => {
                            paths.push(Gunk {
                                text: format!("{variant_name} => {{ }}"),
                                preindent_change: 0,
                                postindent_change: 0,
                            });
                        }
                    }
                }
                paths.push(Gunk {
                    text: format!("}}"),
                    preindent_change: -4,
                    postindent_change: 0,
                });
                paths
            },
            Ty::Ref(ref_ty) => {
                let base = format!("(*{from})");
                self.var_paths(state, &base, ref_ty.pointee_ty, is_root)?
            },
            Ty::Union(union_ty) => {
                let mut paths = vec![];
                // FIXME(variant_select): hardcoded to access field 0 for now
                if let Some(field) = union_ty.fields.get(0) {
                    let field_name = &field.ident;
                    let base = format!("{from}.{field_name}");
                    paths.extend(self.var_paths(state, &base, field.ty, false)?)
                }
                paths
            },
        };
        Ok(paths)
    }
}

/// We want to access all the subfields of a variable in order.
/// Ideally we would just return a list of rvalue expressions like
/// ["val.field", "val.arr[0]", "val.arr[1]", "val.field3.x"]
///
/// but if a subfield is an enum, then we can't actually write out its rvalue expression
/// as a single expression. Instead we need to break things up with random "match" statements
/// that introduce new temporary names. These are the "gunks" which should just be emitted
/// verbatim.
enum VarPathPart {
    /// Some random gunk like a fragment of a "match" that must be emitted for subsequent
    /// fields (or gunks) to make sense.
    Gunk {
        /// Text to emit
        text: String,
        /// How much the indent is changed by this gunk before it's printed
        preindent_change: i32,
        /// How much the indent is changed by this gunk after it's printed
        postindent_change: i32,
    },
    /// An actual rvalue expression to a leaf of the variable
    Field(String),
}

fn gen_state(test: &Test) -> GenState {
    GenState {
        test,
        tynames: HashMap::new(),
        borrowed_tynames: HashMap::new(),
        funcs: vec![],
        val_writer: WriteImpl::HarnessCallback,
    }
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