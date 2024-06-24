//! Rust(c) codegen backend backend

use std::sync::Arc;

use camino::Utf8Path;
use kdl_script::types::{AliasTy, ArrayTy, Func, FuncIdx, PrimitiveTy, RefTy, Ty, TyIdx};
use kdl_script::PunEnv;
use vals::{ArgValuesIter, Value};

use self::error::GenerateError;

use super::super::*;
use super::*;
use crate::fivemat::Fivemat;
use std::fmt::Write;

pub static RUST_TEST_PREFIX: &str = include_str!("../../harness/rust_test_prefix.rs");

const VAR_CALLER_INPUTS: &str = "CALLER_INPUTS";
const VAR_CALLER_OUTPUTS: &str = "CALLER_OUTPUTS";
const VAR_CALLEE_INPUTS: &str = "CALLEE_INPUTS";
const VAR_CALLEE_OUTPUTS: &str = "CALLEE_OUTPUTS";
const INDENT: &str = "    ";

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
    fn pun_env(&self) -> Arc<PunEnv> {
        Arc::new(kdl_script::PunEnv {
            lang: "rust".to_string(),
        })
    }
    fn supports_options(
        &self,
        TestOptions {
            convention,
            functions,
            val_writer,
            val_generator: _,
        }: &TestOptions,
    ) -> bool {
        // NOTE: Rustc spits out:
        //
        // Rust, C, C-unwind, cdecl, stdcall, stdcall-unwind, fastcall,
        // vectorcall, thiscall, thiscall-unwind, aapcs, win64, sysv64,
        // ptx-kernel, msp430-interrupt, x86-interrupt, amdgpu-kernel,
        // efiapi, avr-interrupt, avr-non-blocking-interrupt, C-cmse-nonsecure-call,
        // wasm, system, system-unwind, rust-intrinsic, rust-call,
        // platform-intrinsic, unadjusted
        let supports_convention = match convention {
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
        };
        let supports_writer = match val_writer {
            WriteImpl::HarnessCallback => true,
            WriteImpl::Print => true,
            WriteImpl::Assert => true,
            WriteImpl::Noop => true,
        };
        let supports_query = match functions {
            FunctionSelector::All => true,
            FunctionSelector::One { idx: _, args } => match args {
                ArgSelector::All => true,
                ArgSelector::One { idx: _, vals } => match vals {
                    ValSelector::All => true,
                    ValSelector::One { idx: _ } => true,
                },
            },
        };

        supports_convention && supports_writer && supports_query
    }

    fn compile_callee(
        &self,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<String, BuildError> {
        let mut cmd = Command::new("rustc");
        cmd.arg("--crate-type")
            .arg("staticlib")
            .arg("--out-dir")
            .arg(out_dir)
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

    fn compile_caller(
        &self,
        src_path: &Utf8Path,
        out_dir: &Utf8Path,
        lib_name: &str,
    ) -> Result<String, BuildError> {
        // Currently no need to be different
        self.compile_callee(src_path, out_dir, lib_name)
    }

    fn generate_callee(&self, f: &mut dyn Write, mut test: TestImpl) -> Result<(), GenerateError> {
        let mut f = Fivemat::new(f, INDENT);
        self.generate_callee_impl(&mut f, &mut test)
    }

    fn generate_caller(&self, f: &mut dyn Write, mut test: TestImpl) -> Result<(), GenerateError> {
        let mut f = Fivemat::new(f, INDENT);
        self.generate_caller_impl(&mut f, &mut test)
    }
}

impl RustcAbiImpl {
    pub fn generate_caller_impl(
        &self,
        f: &mut Fivemat,
        state: &mut TestImpl,
    ) -> Result<(), GenerateError> {
        // Generate type decls and gather up functions
        self.generate_definitions(f, state)?;
        // Generate decls of the functions we want to call
        self.generate_caller_externs(f, state)?;

        // Generate the test function the harness will call
        writeln!(f, "#[no_mangle]\npub extern \"C\" fn do_test() {{")?;
        for &func in &state.desired_funcs {
            // Generate the individual function calls
            self.generate_caller_body(f, state, func)?;
        }
        writeln!(f, "}}")?;

        Ok(())
    }

    fn generate_caller_externs(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
    ) -> Result<(), GenerateError> {
        let convention_decl = self.convention_decl(state.options.convention)?;
        writeln!(f, "extern \"{convention_decl}\" {{",)?;
        f.add_indent(1);
        for &func in &state.desired_funcs {
            self.generate_signature(f, state, func)?;
            writeln!(f, ";")?;
        }
        f.sub_indent(1);
        writeln!(f, "}}")?;
        writeln!(f)?;
        Ok(())
    }

    fn generate_caller_body(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        writeln!(f, "unsafe {{")?;
        f.add_indent(1);
        let function = state.types.realize_func(func);

        // Create vars for all the inputs
        let mut func_vals = state.vals.at_func(func);
        for arg in &function.inputs {
            let arg_vals: ArgValuesIter = func_vals.next_arg();
            // Create and report the input
            self.create_var(f, state, &arg.name, arg.ty, arg_vals.clone())?;
            self.write_var(f, state, &arg.name, arg.ty, arg_vals, VAR_CALLER_INPUTS)?;
        }

        // Call the function
        self.call_function(f, state, function)?;

        // Report all the outputs
        for arg in &function.outputs {
            let arg_vals: ArgValuesIter = func_vals.next_arg();

            self.write_var(f, state, &arg.name, arg.ty, arg_vals, VAR_CALLER_OUTPUTS)?;
        }

        // Report the function is complete
        self.end_function(f, state, VAR_CALLER_INPUTS, VAR_CALLER_OUTPUTS)?;
        f.sub_indent(1);
        writeln!(f, "}}")?;
        Ok(())
    }

    fn call_function(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        function: &Func,
    ) -> Result<(), GenerateError> {
        let func_name = &function.name;

        // if there's an output, bind it
        let mut proper_outputs = function
            .outputs
            .iter()
            .filter(|arg| !state.borrowed_tynames.contains_key(&arg.ty));
        let output = proper_outputs.next();
        let too_many_outputs = proper_outputs.next();
        if too_many_outputs.is_some() {
            return Err(GenerateError::RustUnsupported(
                "multiple normal returns (should this be a tuple?)".to_owned(),
            ));
        }
        if let Some(output) = output {
            write!(f, "let {} = ", output.name)?;
        }

        // Call the function
        write!(f, "{func_name}(")?;
        let inputs = function.inputs.iter();
        let out_params = function
            .outputs
            .iter()
            .filter(|arg| state.borrowed_tynames.contains_key(&arg.ty));

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
    pub fn generate_callee_impl(
        &self,
        f: &mut Fivemat,
        state: &mut TestImpl,
    ) -> Result<(), GenerateError> {
        // Generate type decls and gather up functions
        self.generate_definitions(f, state)?;

        for &func in &state.desired_funcs {
            // Generate the individual function definitions
            self.generate_callee_body(f, state, func)?;
        }
        Ok(())
    }

    fn generate_callee_body(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        let function = state.types.realize_func(func);
        let convention_decl = self.convention_decl(state.options.convention)?;
        writeln!(f, "#[no_mangle]")?;
        write!(f, "pub unsafe extern \"{convention_decl}\" ")?;
        self.generate_signature(f, state, func)?;
        writeln!(f, " {{")?;
        f.add_indent(1);
        writeln!(f, "unsafe {{")?;
        f.add_indent(1);
        // Report the inputs
        let mut func_vals = state.vals.at_func(func);
        for arg in &function.inputs {
            let arg_vals = func_vals.next_arg();
            let arg_name = &arg.name;
            self.write_var(f, state, arg_name, arg.ty, arg_vals, VAR_CALLEE_INPUTS)?;
        }

        // Create outputs and report them
        for arg in &function.outputs {
            let arg_vals = func_vals.next_arg();
            self.create_var(f, state, &arg.name, arg.ty, arg_vals.clone())?;
            self.write_var(f, state, &arg.name, arg.ty, arg_vals, VAR_CALLEE_OUTPUTS)?;
        }

        // Report the function is complete
        self.end_function(f, state, VAR_CALLEE_INPUTS, VAR_CALLEE_OUTPUTS)?;

        // Return the outputs
        for arg in function.outputs.iter() {
            self.return_var(f, state, &arg.name, arg.ty)?;
        }
        f.sub_indent(1);
        writeln!(f, "}}")?;
        f.sub_indent(1);
        writeln!(f, "}}")?;
        Ok(())
    }
}

impl RustcAbiImpl {
    pub fn new(_system_info: &Config, codegen_backend: Option<String>) -> Self {
        Self {
            is_nightly: built_info::RUSTC_VERSION.contains("nightly"),
            codegen_backend,
        }
    }

    pub fn generate_definitions(
        &self,
        f: &mut Fivemat,
        state: &mut TestImpl,
    ) -> Result<(), GenerateError> {
        self.write_harness_prefix(f, state)?;

        for def in state.defs.definitions(state.desired_funcs.iter().copied()) {
            match def {
                kdl_script::Definition::DeclareTy(ty) => {
                    self.intern_tyname(state, ty)?;
                }
                kdl_script::Definition::DefineTy(ty) => {
                    self.generate_tydef(f, state, ty)?;
                }
                kdl_script::Definition::DefineFunc(_) => {
                    // we'd buffer these up to generate them all at the end,
                    // but we've already got them buffered, so... do nothing.
                }
                kdl_script::Definition::DeclareFunc(_) => {
                    // nothing to do, executable kdl-script isn't real and can't hurt us
                }
            }
        }

        Ok(())
    }

    pub fn intern_tyname(&self, state: &mut TestImpl, ty: TyIdx) -> Result<(), GenerateError> {
        // Don't double-intern
        if state.tynames.contains_key(&ty) {
            return Ok(());
        }

        let (tyname, borrowed_tyname) = match state.types.realize_ty(ty) {
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
                    PrimitiveTy::I256 => Err(GenerateError::RustUnsupported(
                        "rust doesn't have i256".to_owned(),
                    ))?,
                    PrimitiveTy::U256 => Err(GenerateError::RustUnsupported(
                        "rust doesn't have u256".to_owned(),
                    ))?,
                    PrimitiveTy::F16 => Err(GenerateError::RustUnsupported(
                        "rust doesn't have f16".to_owned(),
                    ))?,
                    PrimitiveTy::F128 => Err(GenerateError::RustUnsupported(
                        "rust doesn't have f128".to_owned(),
                    ))?,
                };
                (name.to_owned(), None)
            }
            Ty::Array(ArrayTy { elem_ty, len }) => {
                let elem_tyname = &state.tynames[elem_ty];
                let borrowed_tyname = state
                    .borrowed_tynames
                    .get(elem_ty)
                    .map(|elem_tyname| format!("[{elem_tyname}; {len}]"));
                (format!("[{elem_tyname}; {len}]"), borrowed_tyname)
            }
            Ty::Ref(RefTy { pointee_ty }) => {
                let pointee_tyname = &state.tynames[pointee_ty];
                let borrowed_pointee_tyname = state
                    .borrowed_tynames
                    .get(pointee_ty)
                    .unwrap_or(pointee_tyname);
                (
                    format!("&mut {pointee_tyname}"),
                    Some(format!("&'a mut {borrowed_pointee_tyname}")),
                )
            }
            Ty::Empty => ("()".to_owned(), None),
            // Nominal types we need to emit a decl for
            Ty::Struct(struct_ty) => {
                let has_borrows = struct_ty
                    .fields
                    .iter()
                    .any(|field| state.borrowed_tynames.contains_key(&field.ty));
                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", struct_ty.name));
                ((*struct_ty.name).clone(), borrowed_tyname)
            }
            Ty::Union(union_ty) => {
                let has_borrows = union_ty
                    .fields
                    .iter()
                    .any(|field| state.borrowed_tynames.contains_key(&field.ty));
                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", union_ty.name));
                ((*union_ty.name).clone(), borrowed_tyname)
            }
            Ty::Enum(enum_ty) => ((*enum_ty.name).clone(), None),
            Ty::Tagged(tagged_ty) => {
                let has_borrows = tagged_ty.variants.iter().any(|v| {
                    v.fields
                        .as_ref()
                        .map(|fields| {
                            fields
                                .iter()
                                .any(|field| state.borrowed_tynames.contains_key(&field.ty))
                        })
                        .unwrap_or(false)
                });
                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", tagged_ty.name));
                ((*tagged_ty.name).clone(), borrowed_tyname)
            }
            Ty::Alias(AliasTy { name, real, attrs }) => {
                assert!(
                    attrs.is_empty(),
                    "don't yet know how to apply attrs to structs"
                );
                let borrowed_tyname = state
                    .borrowed_tynames
                    .get(real)
                    .map(|name| format!("{name}<'a>"));
                ((**name).clone(), borrowed_tyname)
            }

            // Puns should be evaporated
            Ty::Pun(pun) => {
                let real_ty = state.types.resolve_pun(pun, &state.env).unwrap();
                (
                    state.tynames[&real_ty].clone(),
                    state.borrowed_tynames.get(&real_ty).cloned(),
                )
            }
        };

        state.tynames.insert(ty, tyname);
        if let Some(borrowed) = borrowed_tyname {
            state.borrowed_tynames.insert(ty, borrowed);
        }

        Ok(())
    }

    pub fn generate_tydef(
        &self,
        f: &mut Fivemat,
        state: &mut TestImpl,
        ty: TyIdx,
    ) -> Result<(), GenerateError> {
        // Make sure our own name is interned
        self.intern_tyname(state, ty)?;

        match state.types.realize_ty(ty) {
            // Nominal types we need to emit a decl for
            Ty::Struct(struct_ty) => {
                assert!(
                    struct_ty.attrs.is_empty(),
                    "don't yet know how to apply attrs to structs"
                );

                let has_borrows = struct_ty
                    .fields
                    .iter()
                    .any(|field| state.borrowed_tynames.contains_key(&field.ty));

                // Emit an actual struct decl
                writeln!(f, "#[repr(C)]")?;
                if has_borrows {
                    writeln!(f, "struct {}<'a> {{", struct_ty.name)?;
                } else {
                    writeln!(f, "#[derive(Copy, Clone)]")?;
                    writeln!(f, "struct {} {{", struct_ty.name)?;
                }
                f.add_indent(1);
                for field in &struct_ty.fields {
                    let field_name = &field.ident;
                    let field_tyname = state
                        .borrowed_tynames
                        .get(&field.ty)
                        .unwrap_or(&state.tynames[&field.ty]);
                    writeln!(f, "{field_name}: {field_tyname},")?;
                }
                f.sub_indent(1);
                writeln!(f, "}}\n")?;
            }
            Ty::Union(union_ty) => {
                assert!(
                    union_ty.attrs.is_empty(),
                    "don't yet know how to apply attrs to unions"
                );

                let has_borrows = union_ty
                    .fields
                    .iter()
                    .any(|field| state.borrowed_tynames.contains_key(&field.ty));

                // Emit an actual union decl
                writeln!(f, "#[repr(C)]")?;
                if has_borrows {
                    writeln!(f, "union {}<'a> {{", union_ty.name)?;
                } else {
                    writeln!(f, "#[derive(Copy, Clone)]")?;
                    writeln!(f, "union {} {{", union_ty.name)?;
                }
                f.add_indent(1);
                for field in &union_ty.fields {
                    let field_name = &field.ident;
                    let field_tyname = state
                        .borrowed_tynames
                        .get(&field.ty)
                        .unwrap_or(&state.tynames[&field.ty]);
                    writeln!(f, "{field_name}: {field_tyname},")?;
                }
                f.sub_indent(1);
                writeln!(f, "}}\n")?;
            }
            Ty::Enum(enum_ty) => {
                assert!(
                    enum_ty.attrs.is_empty(),
                    "don't yet know how to apply attrs to enums"
                );

                // Emit an actual enum decl
                writeln!(f, "#[repr(C)]")?;
                writeln!(f, "#[derive(Debug, Copy, Clone, PartialEq)]")?;
                writeln!(f, "enum {} {{", enum_ty.name)?;
                f.add_indent(1);
                for variant in &enum_ty.variants {
                    let variant_name = &variant.name;
                    writeln!(f, "{variant_name},")?;
                }
                f.sub_indent(1);
                writeln!(f, "}}\n")?;
            }
            Ty::Tagged(tagged_ty) => {
                assert!(
                    tagged_ty.attrs.is_empty(),
                    "don't yet know how to apply attrs to tagged unions"
                );

                let has_borrows = tagged_ty.variants.iter().any(|v| {
                    v.fields
                        .as_ref()
                        .map(|fields| {
                            fields
                                .iter()
                                .any(|field| state.borrowed_tynames.contains_key(&field.ty))
                        })
                        .unwrap_or(false)
                });

                // Emit an actual enum decl
                writeln!(f, "#[repr(C)]")?;
                if has_borrows {
                    writeln!(f, "enum {}<'a> {{", tagged_ty.name)?;
                } else {
                    writeln!(f, "#[derive(Copy, Clone)]")?;
                    writeln!(f, "enum {} {{", tagged_ty.name)?;
                }
                f.add_indent(1);
                for variant in &tagged_ty.variants {
                    let variant_name = &variant.name;
                    if let Some(fields) = &variant.fields {
                        writeln!(f, "{variant_name} {{")?;
                        f.add_indent(1);
                        for field in fields {
                            let field_name = &field.ident;
                            let field_tyname = state
                                .borrowed_tynames
                                .get(&field.ty)
                                .unwrap_or(&state.tynames[&field.ty]);
                            writeln!(f, "{field_name}: {field_tyname},")?;
                        }
                        f.sub_indent(1);
                        writeln!(f, "}},")?;
                    } else {
                        writeln!(f, "{variant_name},")?;
                    }
                }
                f.sub_indent(1);
                writeln!(f, "}}\n")?;
            }
            Ty::Alias(AliasTy { name, real, attrs }) => {
                assert!(
                    attrs.is_empty(),
                    "don't yet know how to apply attrs to type aliases"
                );

                // Emit an actual type alias decl
                if let Some(real_tyname) = state.borrowed_tynames.get(real) {
                    writeln!(f, "type {name}<'a> = {real_tyname};\n")?;
                } else {
                    let real_tyname = &state.tynames[&real];
                    writeln!(f, "type {name} = {real_tyname};\n")?;
                }
            }
            Ty::Pun(..) => {
                // Puns should be evaporated by the type name interner
            }
            Ty::Primitive(prim) => {
                match prim {
                    PrimitiveTy::I8
                    | PrimitiveTy::I16
                    | PrimitiveTy::I32
                    | PrimitiveTy::I64
                    | PrimitiveTy::I128
                    | PrimitiveTy::I256
                    | PrimitiveTy::U8
                    | PrimitiveTy::U16
                    | PrimitiveTy::U32
                    | PrimitiveTy::U64
                    | PrimitiveTy::U128
                    | PrimitiveTy::U256
                    | PrimitiveTy::F16
                    | PrimitiveTy::F32
                    | PrimitiveTy::F64
                    | PrimitiveTy::F128
                    | PrimitiveTy::Bool
                    | PrimitiveTy::Ptr => {
                        // Builtin
                    }
                };
            }
            Ty::Array(ArrayTy { .. }) => {
                // Builtin
            }
            Ty::Ref(RefTy { .. }) => {
                // Builtin
            }
            Ty::Empty => {
                // Builtin
            }
        }
        Ok(())
    }

    pub fn generate_leaf_value(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        ty: TyIdx,
        val: &Value,
        alias: Option<&str>,
    ) -> Result<(), GenerateError> {
        match state.types.realize_ty(ty) {
            // Primitives are the only "real" values with actual bytes that advance val_idx
            Ty::Primitive(prim) => match prim {
                PrimitiveTy::I8 => write!(f, "{}i8", val.generate_i8())?,
                PrimitiveTy::I16 => write!(f, "{}i16", val.generate_i16())?,
                PrimitiveTy::I32 => write!(f, "{}i32", val.generate_i32())?,
                PrimitiveTy::I64 => write!(f, "{}i64", val.generate_i64())?,
                PrimitiveTy::I128 => write!(f, "{}i128", val.generate_i128())?,
                PrimitiveTy::U8 => write!(f, "{}u8", val.generate_u8())?,
                PrimitiveTy::U16 => write!(f, "{}u16", val.generate_u16())?,
                PrimitiveTy::U32 => write!(f, "{}u32", val.generate_u32())?,
                PrimitiveTy::U64 => write!(f, "{}u64", val.generate_u64())?,
                PrimitiveTy::U128 => write!(f, "{}u128", val.generate_u128())?,

                PrimitiveTy::F32 => write!(f, "f32::from_bits({})", val.generate_u32())?,
                PrimitiveTy::F64 => write!(f, "f64::from_bits({})", val.generate_u64())?,
                PrimitiveTy::Bool => write!(f, "true")?,
                PrimitiveTy::Ptr => {
                    if true {
                        write!(f, "{:#X}u64 as *mut ()", val.generate_u64())?
                    } else {
                        write!(f, "{:#X}u32 as *mut ()", val.generate_u32())?
                    }
                }
                PrimitiveTy::I256 => Err(GenerateError::RustUnsupported(
                    "rust doesn't have i256".to_owned(),
                ))?,
                PrimitiveTy::U256 => Err(GenerateError::RustUnsupported(
                    "rust doesn't have u256".to_owned(),
                ))?,
                PrimitiveTy::F16 => Err(GenerateError::RustUnsupported(
                    "rust doesn't have f16".to_owned(),
                ))?,
                PrimitiveTy::F128 => Err(GenerateError::RustUnsupported(
                    "rust doesn't have f128".to_owned(),
                ))?,
            },
            Ty::Enum(enum_ty) => {
                let name = alias.unwrap_or(&enum_ty.name);
                if let Some(variant) = val.select_val(&enum_ty.variants) {
                    let variant_name = &variant.name;
                    write!(f, "{name}::{variant_name}")?;
                }
            }
            _ => unreachable!("only primitives and enums should be passed to generate_leaf_value"),
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate_value(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        ty: TyIdx,
        vals: &mut ArgValuesIter,
        alias: Option<&str>,
        ref_temp_name: &str,
        extra_decls: &mut Vec<String>,
    ) -> Result<(), GenerateError> {
        match state.types.realize_ty(ty) {
            // Primitives and Enums are the only "real" values with actual bytes
            Ty::Primitive(_) | Ty::Enum(_) => {
                let val = vals.next_val();
                self.generate_leaf_value(f, state, ty, &val, alias)?;
            }
            Ty::Empty => {
                write!(f, "()")?;
            }
            Ty::Ref(RefTy { pointee_ty }) => {
                // The value is a mutable reference to a temporary
                write!(f, "&mut {ref_temp_name}")?;

                // TODO: should this be a recursive call to create_var (need create_var_inner?)
                // Now do the rest of the recursion on constructing the temporary
                let mut ref_temp = String::new();
                let mut ref_temp_f = Fivemat::new(&mut ref_temp, INDENT);
                write!(&mut ref_temp_f, "let mut {ref_temp_name} = ")?;
                let ref_temp_name = format!("{ref_temp_name}_");
                self.generate_value(
                    &mut ref_temp_f,
                    state,
                    *pointee_ty,
                    vals,
                    alias,
                    &ref_temp_name,
                    extra_decls,
                )?;
                write!(&mut ref_temp_f, ";")?;
                extra_decls.push(ref_temp);
            }
            Ty::Array(ArrayTy { elem_ty, len }) => {
                write!(f, "[")?;
                for arr_idx in 0..*len {
                    if arr_idx > 0 {
                        write!(f, ", ")?;
                    }
                    let ref_temp_name = format!("{ref_temp_name}{arr_idx}_");
                    self.generate_value(
                        f,
                        state,
                        *elem_ty,
                        vals,
                        alias,
                        &ref_temp_name,
                        extra_decls,
                    )?;
                }
                write!(f, "]")?;
            }
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
                    self.generate_value(
                        f,
                        state,
                        field.ty,
                        vals,
                        alias,
                        &ref_temp_name,
                        extra_decls,
                    )?;
                }
                write!(f, " }}")?;
            }
            Ty::Union(union_ty) => {
                let name = alias.unwrap_or(&union_ty.name);
                write!(f, "{name} {{ ")?;
                let tag_val = vals.next_val();
                if let Some(field) = tag_val.select_val(&union_ty.fields) {
                    let field_name = &field.ident;
                    write!(f, "{field_name}: ")?;
                    let ref_temp_name = format!("{ref_temp_name}{field_name}_");
                    self.generate_value(
                        f,
                        state,
                        field.ty,
                        vals,
                        alias,
                        &ref_temp_name,
                        extra_decls,
                    )?;
                }
                write!(f, " }}")?;
            }

            Ty::Tagged(tagged_ty) => {
                let name = alias.unwrap_or(&tagged_ty.name);
                let tag_val = vals.next_val();
                if let Some(variant) = tag_val.select_val(&tagged_ty.variants) {
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
                            self.generate_value(
                                f,
                                state,
                                field.ty,
                                vals,
                                alias,
                                &ref_temp_name,
                                extra_decls,
                            )?;
                        }
                        write!(f, " }}")?;
                    }
                }
            }
            Ty::Alias(AliasTy { real, name, .. }) => {
                let alias = alias.or_else(|| Some(name));
                self.generate_value(f, state, *real, vals, alias, ref_temp_name, extra_decls)?;
            }

            // Puns should be evaporated
            Ty::Pun(pun) => {
                let real_ty = state.types.resolve_pun(pun, &state.env).unwrap();
                self.generate_value(f, state, real_ty, vals, alias, ref_temp_name, extra_decls)?;
            }
        };

        Ok(())
    }

    fn convention_decl(
        &self,
        convention: CallingConvention,
    ) -> Result<&'static str, GenerateError> {
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
    fn write_harness_prefix(&self, f: &mut Fivemat, state: &TestImpl) -> Result<(), GenerateError> {
        // No extra harness gunk if not needed
        if state.options.val_writer != WriteImpl::HarnessCallback {
            return Ok(());
        }
        if state.options.convention == CallingConvention::Vectorcall {
            writeln!(f, "#![feature(abi_vectorcall)]")?;
        }
        // Load test harness "headers"
        writeln!(f, "{}", RUST_TEST_PREFIX)?;
        writeln!(f)?;

        Ok(())
    }

    fn generate_signature(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        let function = state.types.realize_func(func);

        write!(f, "fn {}(", function.name)?;
        let mut multiarg = false;
        // Add inputs
        for arg in &function.inputs {
            if multiarg {
                write!(f, ", ")?;
            }
            multiarg = true;
            let arg_name = &arg.name;
            let arg_ty = &state.tynames[&arg.ty];
            write!(f, "{}: {}", arg_name, arg_ty)?;
        }
        // Add outparams
        for arg in &function.outputs {
            let is_outparam = state.borrowed_tynames.contains_key(&arg.ty);
            if !is_outparam {
                // Handled in next loop
                continue;
            }
            if multiarg {
                write!(f, ", ")?;
            }
            multiarg = true;
            // NOTE: we intentionally don't use the "borrowed" tyname
            // as we still don't need lifetimes here!
            let arg_name = &arg.name;
            let arg_ty = &state.tynames[&arg.ty];
            write!(f, "{}: {}", arg_name, arg_ty)?;
        }
        // Add normal returns
        let mut has_normal_return = false;
        for arg in &function.outputs {
            let is_outparam = state.borrowed_tynames.contains_key(&arg.ty);
            if is_outparam {
                // Already handled
                continue;
            }
            if has_normal_return {
                return Err(GenerateError::RustUnsupported(
                    "multiple normal returns (should this be a tuple?)".to_owned(),
                ));
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
        f: &mut Fivemat,
        state: &TestImpl,
        var_name: &str,
        var_ty: TyIdx,
        mut vals: ArgValuesIter,
    ) -> Result<(), GenerateError> {
        // Generate the input
        let needs_mut = false;
        let let_mut = if needs_mut { "let mut" } else { "let" };
        let mut real_var_decl = String::new();
        let mut real_var_decl_f = Fivemat::new(&mut real_var_decl, INDENT);
        let mut extra_decls = Vec::new();
        write!(&mut real_var_decl_f, "{let_mut} {var_name} = ")?;
        let ref_temp_name = format!("{var_name}_");
        self.generate_value(
            &mut real_var_decl_f,
            state,
            var_ty,
            &mut vals,
            None,
            &ref_temp_name,
            &mut extra_decls,
        )?;
        writeln!(&mut real_var_decl, ";")?;

        for decl in extra_decls {
            writeln!(f, "{}", decl)?;
        }
        writeln!(f, "{}", real_var_decl)?;
        Ok(())
    }

    /// Emit the WRITE calls and FINISHED_VAL for this value.
    /// This will WRITE every leaf subfield of the type.
    /// `to` is the BUFFER to use, `from` is the variable name of the value.
    fn write_var(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        var_name: &str,
        var_ty: TyIdx,
        mut vals: ArgValuesIter,
        to: &str,
    ) -> Result<(), GenerateError> {
        // If we're generating a minimized test, skip this
        if !vals.should_write_arg(&state.options) {
            return Ok(());
        }
        // If noop, don't bother doing anything (avoids tagged union matches being generated)
        if let WriteImpl::Noop = state.options.val_writer {
            return Ok(());
        };
        self.write_fields(f, state, to, var_name, var_ty, &mut vals)?;

        // If doing full harness callbacks, signal we wrote all the fields of a variable
        if let WriteImpl::HarnessCallback = state.options.val_writer {
            writeln!(f, "finished_val({to});")?;
            writeln!(f)?;
        }
        Ok(())
    }

    /// Recursive subroutine of write_var, which builds up rvalue paths and generates
    /// appropriate match statements. Actual WRITE calls are done by write_leaf_field.
    fn write_fields(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        to: &str,
        from: &str,
        var_ty: TyIdx,
        vals: &mut ArgValuesIter,
    ) -> Result<(), GenerateError> {
        match state.types.realize_ty(var_ty) {
            Ty::Primitive(_) | Ty::Enum(_) => {
                // Hey an actual leaf, report it (and burn a value)
                let val = vals.next_val();
                if val.should_write_val(&state.options) {
                    self.write_leaf_field(f, state, to, from, &val)?;
                }
            }
            Ty::Empty => {
                // nothing worth producing
            }
            Ty::Alias(alias_ty) => {
                // keep going but with the type changed
                self.write_fields(f, state, to, from, alias_ty.real, vals)?;
            }
            Ty::Pun(pun) => {
                // keep going but with the type changed
                let real_ty = state.types.resolve_pun(pun, &state.env).unwrap();
                self.write_fields(f, state, to, from, real_ty, vals)?
            }
            Ty::Array(array_ty) => {
                // recurse into each array index
                for i in 0..array_ty.len {
                    let base = format!("{from}[{i}]");
                    self.write_fields(f, state, to, &base, array_ty.elem_ty, vals)?;
                }
            }
            Ty::Struct(struct_ty) => {
                // recurse into each field
                for field in &struct_ty.fields {
                    let field_name = &field.ident;
                    let base = format!("{from}.{field_name}");
                    self.write_fields(f, state, to, &base, field.ty, vals)?;
                }
            }
            Ty::Tagged(tagged_ty) => {
                // Process the implicit "tag" value
                let tag_generator = vals.next_val();
                let tag_idx = tag_generator.generate_idx(tagged_ty.variants.len());
                if let Some(variant) = tagged_ty.variants.get(tag_idx) {
                    let tagged_name = &tagged_ty.name;
                    let variant_name = &variant.name;
                    let pat = match &variant.fields {
                        Some(fields) => {
                            // Variant with fields, recurse into them
                            let field_list = fields
                                .iter()
                                .map(|f| f.ident.to_string())
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!("{tagged_name}::{variant_name} {{ {field_list} }}")
                        }
                        None => {
                            // Variant without fields, still need the pattern to check the tag
                            format!("{tagged_name}::{variant_name}")
                        }
                    };

                    // We're going to make an if-let for the case we expect, but there might not
                    // be anything we care about in here (especially with should_write_val) so we
                    // buffer up if and else branches and then only emit the if-let if one of them
                    // is non-empty
                    let if_branch = {
                        let mut temp_out = String::new();
                        let f = &mut Fivemat::new(&mut temp_out, INDENT);
                        f.add_indent(1);
                        if tag_generator.should_write_val(&state.options) {
                            self.write_tag_field(f, state, to, tag_idx)?;
                        }
                        if let Some(fields) = &variant.fields {
                            for field in fields {
                                // Do the ugly deref thing to deal with pattern autoref
                                let base = format!("(*{})", field.ident);
                                self.write_fields(f, state, to, &base, field.ty, vals)?;
                            }
                        }
                        f.sub_indent(1);
                        temp_out
                    };
                    // Add an else case to complain that the variant is wrong
                    let else_branch = {
                        let mut temp_out = String::new();
                        let f = &mut Fivemat::new(&mut temp_out, INDENT);
                        f.add_indent(1);
                        if tag_generator.should_write_val(&state.options) {
                            self.error_tag_field(f, state, to)?;
                        }
                        f.sub_indent(1);
                        temp_out
                    };

                    let if_has_content = !if_branch.trim().is_empty();
                    let else_has_content = !else_branch.trim().is_empty();
                    if if_has_content || else_has_content {
                        writeln!(f, "if let {pat} = &{from} {{")?;
                        write!(f, "{}", if_branch)?;
                        write!(f, "}}")?;
                    }
                    if else_has_content {
                        writeln!(f, " else {{")?;
                        write!(f, "{}", else_branch)?;
                        writeln!(f, "}}")?;
                    }
                }
            }
            Ty::Ref(ref_ty) => {
                // Add a deref, and recurse into the pointee
                let base = format!("(*{from})");
                self.write_fields(f, state, to, &base, ref_ty.pointee_ty, vals)?
            }
            Ty::Union(union_ty) => {
                // Process the implicit "tag" value
                let tag_generator = vals.next_val();
                let tag_idx = tag_generator.generate_idx(union_ty.fields.len());
                if tag_generator.should_write_val(&state.options) {
                    self.write_tag_field(f, state, to, tag_idx)?;
                }
                if let Some(field) = union_ty.fields.get(tag_idx) {
                    let field_name = &field.ident;
                    let base = format!("{from}.{field_name}");
                    self.write_fields(f, state, to, &base, field.ty, vals)?;
                }
            }
        };
        Ok(())
    }

    /// WRITE an actual indivisible value (primitive or c-like enum)
    fn write_leaf_field(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        to: &str,
        path: &str,
        val: &Value,
    ) -> Result<(), GenerateError> {
        match state.options.val_writer {
            WriteImpl::HarnessCallback => {
                // Convenience for triggering test failures
                if path.contains("abicafepoison") && to.contains(VAR_CALLEE_INPUTS) {
                    writeln!(f, "write_field({to}, &0x12345678u32);")?;
                } else {
                    writeln!(f, "write_field({to}, &{path});")?;
                }
            }
            WriteImpl::Assert => {
                write!(f, "assert_eq!({path}, ")?;
                self.generate_leaf_value(f, state, val.ty, val, None)?;
                writeln!(f, ");")?;
            }
            WriteImpl::Print => {
                writeln!(f, "println!(\"{{:?}}\", {path});")?;
            }
            WriteImpl::Noop => {
                // Noop, do nothing
            }
        }
        Ok(())
    }

    fn write_tag_field(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        to: &str,
        variant_idx: usize,
    ) -> Result<(), GenerateError> {
        match state.options.val_writer {
            WriteImpl::HarnessCallback => {
                writeln!(f, "write_field({to}, &{}u32);", variant_idx)?;
            }
            WriteImpl::Assert => {
                // Noop, do nothing
            }
            WriteImpl::Print => {
                // Noop, do nothing
            }
            WriteImpl::Noop => {
                // Noop, do nothing
            }
        }
        Ok(())
    }

    fn error_tag_field(
        &self,
        f: &mut Fivemat,
        state: &TestImpl,
        to: &str,
    ) -> Result<(), GenerateError> {
        match state.options.val_writer {
            WriteImpl::HarnessCallback => {
                writeln!(f, "write_field({to}, &{}u32);", u32::MAX)?;
            }
            WriteImpl::Assert => {
                unreachable!("enum had unexpected variant!?");
            }
            WriteImpl::Print => {
                unreachable!("enum had unexpected variant!?");
            }
            WriteImpl::Noop => {
                // Noop, do nothing
            }
        }
        Ok(())
    }

    fn end_function(
        &self,
        f: &mut dyn Write,
        state: &TestImpl,
        inputs: &str,
        outputs: &str,
    ) -> Result<(), GenerateError> {
        match state.options.val_writer {
            WriteImpl::HarnessCallback => {
                writeln!(f, "finished_func({inputs}, {outputs});")?;
            }
            WriteImpl::Print | WriteImpl::Noop | WriteImpl::Assert => {
                // Noop
            }
        }
        Ok(())
    }

    fn pass_var(
        &self,
        f: &mut dyn Write,
        _state: &TestImpl,
        var_name: &str,
        _var_ty: TyIdx,
    ) -> Result<(), GenerateError> {
        write!(f, "{var_name}")?;
        Ok(())
    }

    fn return_var(
        &self,
        f: &mut dyn Write,
        _state: &TestImpl,
        var_name: &str,
        _var_ty: TyIdx,
    ) -> Result<(), GenerateError> {
        // TODO: implement outparam returns
        writeln!(f, "{var_name}")?;
        Ok(())
    }
}
