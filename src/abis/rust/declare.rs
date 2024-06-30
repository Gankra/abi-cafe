use super::*;
use kdl_script::parse::{Attr, AttrAligned, AttrPacked, AttrPassthrough, AttrRepr, LangRepr, Repr};
use kdl_script::types::{AliasTy, ArrayTy, FuncIdx, PrimitiveTy, RefTy, Ty, TyIdx};
use std::fmt::Write;

impl RustcAbiImpl {
    pub fn generate_caller_externs(
        &self,
        f: &mut Fivemat,
        state: &TestState,
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

    pub fn generate_definitions(
        &self,
        f: &mut Fivemat,
        state: &mut TestState,
    ) -> Result<(), GenerateError> {
        self.write_harness_prefix(f, state)?;

        for def in state.defs.definitions(state.desired_funcs.iter().copied()) {
            match def {
                kdl_script::Definition::DeclareTy(ty) => {
                    debug!("declare ty {}", state.types.format_ty(ty));
                    self.intern_tyname(state, ty)?;
                }
                kdl_script::Definition::DefineTy(ty) => {
                    debug!("define ty {}", state.types.format_ty(ty));
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

    pub fn intern_tyname(&self, state: &mut TestState, ty: TyIdx) -> Result<(), GenerateError> {
        // Don't double-intern
        if state.tynames.contains_key(&ty) {
            return Ok(());
        }

        let has_borrows = state.types.ty_contains_ref(ty);
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
                    PrimitiveTy::I256 => {
                        Err(UnsupportedError::Other("rust doesn't have i256".to_owned()))?
                    }
                    PrimitiveTy::U256 => {
                        Err(UnsupportedError::Other("rust doesn't have u256".to_owned()))?
                    }
                    PrimitiveTy::F16 => {
                        Err(UnsupportedError::Other("rust doesn't have f16".to_owned()))?
                    }
                    PrimitiveTy::F128 => {
                        Err(UnsupportedError::Other("rust doesn't have f128".to_owned()))?
                    }
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
                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", struct_ty.name));
                (struct_ty.name.to_string(), borrowed_tyname)
            }
            Ty::Union(union_ty) => {
                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", union_ty.name));
                (union_ty.name.to_string(), borrowed_tyname)
            }
            Ty::Enum(enum_ty) => ((**enum_ty.name).clone(), None),
            Ty::Tagged(tagged_ty) => {
                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", tagged_ty.name));
                (tagged_ty.name.to_string(), borrowed_tyname)
            }
            Ty::Alias(alias_ty) => {
                let borrowed_tyname = has_borrows.then(|| format!("{}<'a>", alias_ty.name));
                (alias_ty.name.to_string(), borrowed_tyname)
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
        state: &mut TestState,
        ty: TyIdx,
    ) -> Result<(), GenerateError> {
        // Make sure our own name is interned
        self.intern_tyname(state, ty)?;

        let has_borrows = state.types.ty_contains_ref(ty);
        match state.types.realize_ty(ty) {
            // Nominal types we need to emit a decl for
            Ty::Struct(struct_ty) => {
                // Emit an actual struct decl
                self.generate_repr_attr(f, &struct_ty.attrs, "struct")?;
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
                // Emit an actual union decl
                self.generate_repr_attr(f, &union_ty.attrs, "union")?;
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
                // Emit an actual enum decl
                self.generate_repr_attr(f, &enum_ty.attrs, "enum")?;
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
                // Emit an actual enum decl
                self.generate_repr_attr(f, &tagged_ty.attrs, "tagged")?;
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
                if !attrs.is_empty() {
                    return Err(UnsupportedError::Other(
                        "don't yet know how to apply attrs to aliases".to_string(),
                    ))?;
                }

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

    pub fn generate_repr_attr(
        &self,
        f: &mut Fivemat,
        attrs: &[Attr],
        _ty_style: &str,
    ) -> Result<(), GenerateError> {
        let mut default_c_repr = true;
        let mut repr_attrs = vec![];
        let mut other_attrs = vec![];
        for attr in attrs {
            match attr {
                Attr::Align(AttrAligned { align }) => {
                    repr_attrs.push(format!("align({})", align.val));
                }
                Attr::Packed(AttrPacked {}) => {
                    repr_attrs.push("packed".to_owned());
                }
                Attr::Passthrough(AttrPassthrough(attr)) => {
                    other_attrs.push(attr.to_string());
                }
                Attr::Repr(AttrRepr { reprs }) => {
                    // Any explicit repr attributes disables default C
                    default_c_repr = false;
                    for repr in reprs {
                        let val = match repr {
                            Repr::Primitive(prim) => match prim {
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
                                PrimitiveTy::I256
                                | PrimitiveTy::U256
                                | PrimitiveTy::F16
                                | PrimitiveTy::F32
                                | PrimitiveTy::F64
                                | PrimitiveTy::F128
                                | PrimitiveTy::Bool
                                | PrimitiveTy::Ptr => {
                                    return Err(UnsupportedError::Other(format!(
                                        "unsupport repr({prim:?})"
                                    )))?;
                                }
                            },
                            Repr::Lang(LangRepr::C) => "C",
                            Repr::Lang(LangRepr::Rust) => {
                                continue;
                            }
                            Repr::Transparent => "transparent",
                        };
                        repr_attrs.push(val.to_owned());
                    }
                }
            }
        }
        if default_c_repr {
            repr_attrs.push("C".to_owned());
        }
        write!(f, "#[repr(")?;
        let mut multi = false;
        for repr in repr_attrs {
            if multi {
                write!(f, ", ")?;
            }
            multi = true;
            write!(f, "{repr}")?;
        }
        writeln!(f, ")]")?;
        for attr in other_attrs {
            writeln!(f, "{}", attr)?;
        }
        Ok(())
    }

    pub fn generate_signature(
        &self,
        f: &mut Fivemat,
        state: &TestState,
        func: FuncIdx,
    ) -> Result<(), GenerateError> {
        let function = state.types.realize_func(func);
        self.check_returns(state, function)?;

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
        // Add normal returns
        if let Some(arg) = function.outputs.first() {
            let arg_ty = &state.tynames[&arg.ty];
            write!(f, ") -> {arg_ty}")?;
        } else {
            write!(f, ")")?;
        }
        Ok(())
    }

    pub fn convention_decl(
        &self,
        convention: CallingConvention,
    ) -> Result<&'static str, GenerateError> {
        use super::Platform::*;

        let conv = match convention {
            CallingConvention::C => "C",
            CallingConvention::System => "system",
            CallingConvention::Win64 => "win64",
            CallingConvention::Sysv64 => "sysv64",
            CallingConvention::Aapcs => "aapcs",
            CallingConvention::Cdecl => {
                if self.platform == Windows {
                    "cdecl"
                } else {
                    return Err(self.unsupported_convention(&convention))?;
                }
            }
            CallingConvention::Stdcall => {
                if self.platform == Windows {
                    "stdcall"
                } else {
                    return Err(self.unsupported_convention(&convention))?;
                }
            }
            CallingConvention::Fastcall => {
                if self.platform == Windows {
                    "fastcall"
                } else {
                    return Err(self.unsupported_convention(&convention))?;
                }
            }
            CallingConvention::Vectorcall => {
                if self.platform == Windows {
                    if self.is_nightly {
                        "vectorcall"
                    } else {
                        return Err(UnsupportedError::Other(
                            "vectorcall is an unstable rust feature, requires nightly".to_owned(),
                        ))?;
                    }
                } else {
                    return Err(self.unsupported_convention(&convention))?;
                }
            }
        };
        Ok(conv)
    }

    fn unsupported_convention(&self, convention: &CallingConvention) -> UnsupportedError {
        UnsupportedError::Other(format!("unsupported convention {convention}"))
    }
}
