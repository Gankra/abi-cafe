use super::*;
use crate::harness::vals::{ArgValuesIter, Value};
use kdl_script::types::{AliasTy, ArrayTy, PrimitiveTy, RefTy, Ty, TyIdx};
use std::fmt::Write;

impl CcToolchain {
    pub fn init_leaf_value(
        &self,
        f: &mut Fivemat,
        state: &TestState,
        ty: TyIdx,
        val: &Value,
        alias: Option<&str>,
    ) -> Result<(), GenerateError> {
        match state.types.realize_ty(ty) {
            // Primitives are the only "real" values with actual bytes that advance val_idx
            Ty::Primitive(prim) => match prim {
                PrimitiveTy::I8 => write!(f, "{}", val.generate_i8())?,
                PrimitiveTy::I16 => write!(f, "{}", val.generate_i16())?,
                PrimitiveTy::I32 => write!(f, "{}", val.generate_i32())?,
                PrimitiveTy::I64 => write!(f, "{}", val.generate_i64())?,
                PrimitiveTy::U8 => write!(f, "{}", val.generate_u8())?,
                PrimitiveTy::U16 => write!(f, "{}", val.generate_u16())?,
                PrimitiveTy::U32 => write!(f, "{}", val.generate_u32())?,
                PrimitiveTy::U64 => write!(f, "{}ull", val.generate_u64())?,
                PrimitiveTy::I128 => {
                    let val = val.generate_i128();
                    let lower = (val as u128) & 0x0000_0000_0000_0000_FFFF_FFFF_FFFF_FFFF;
                    let higher = ((val as u128) & 0xFFFF_FFFF_FFFF_FFFF_0000_0000_0000_0000) >> 64;
                    write!(
                        f,
                        "((__int128_t){lower:#X}ull) | (((__int128_t){higher:#X}ull) << 64)"
                    )?
                }
                PrimitiveTy::U128 => {
                    let val = val.generate_u128();
                    let lower = val & 0x0000_0000_0000_0000_FFFF_FFFF_FFFF_FFFF;
                    let higher = (val & 0xFFFF_FFFF_FFFF_FFFF_0000_0000_0000_0000) >> 64;
                    write!(
                        f,
                        "((__uint128_t){lower:#X}ull) | (((__uint128_t){higher:#X}ull) << 64)"
                    )?
                }

                PrimitiveTy::F32 => {
                    let val = f32::from_bits(val.generate_u32());
                    if val.fract() == 0.0 {
                        write!(f, "{val}.0f")?
                    } else {
                        write!(f, "{val}f")?
                    }
                }
                PrimitiveTy::F64 => {
                    let val = f64::from_bits(val.generate_u64());
                    if val.fract() == 0.0 {
                        write!(f, "{val}.0")?
                    } else {
                        write!(f, "{val}")?
                    }
                }
                PrimitiveTy::Bool => write!(f, "{}", val.generate_bool())?,
                PrimitiveTy::Ptr => {
                    if true {
                        write!(f, "(void*){:#X}ull", val.generate_u64())?
                    } else {
                        write!(f, "(void*){:#X}ul", val.generate_u32())?
                    }
                }
                PrimitiveTy::I256 => {
                    Err(UnsupportedError::Other("c doesn't have i256?".to_owned()))?
                }
                PrimitiveTy::U256 => {
                    Err(UnsupportedError::Other("c doesn't have u256?".to_owned()))?
                }
                PrimitiveTy::F16 => write!(
                    f,
                    "(((union {{ uint16_t bits; _Float16 value; }}){{ .bits = {} }}).value)",
                    val.generate_u16()
                )?,
                PrimitiveTy::F128 => {
                    let val = val.generate_u128();
                    let lower = val & 0x0000_0000_0000_0000_FFFF_FFFF_FFFF_FFFF;
                    let higher = (val & 0xFFFF_FFFF_FFFF_FFFF_0000_0000_0000_0000) >> 64;
                    write!(
                        f,
                        "(((union {{ __uint128_t bits; __float128 value; }}){{ .bits = ((__uint128_t){lower:#X}ull) | (((__uint128_t){higher:#X}ull) << 64) }}).value)"
                    )?
                }
            },
            Ty::Enum(enum_ty) => {
                let name = alias.unwrap_or(&enum_ty.name);
                if let Some(variant) = val.select_val(&enum_ty.variants) {
                    let variant_name = &variant.name;
                    write!(f, "{name}_{variant_name}")?;
                }
            }
            _ => unreachable!("only primitives and enums should be passed to generate_leaf_value"),
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn init_value(
        &self,
        f: &mut Fivemat,
        state: &TestState,
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
                self.init_leaf_value(f, state, ty, &val, alias)?;
            }
            Ty::Ref(RefTy { pointee_ty }) => {
                // The value is a mutable reference to a temporary
                write!(f, "&{ref_temp_name}")?;

                // Now do the rest of the recursion on constructing the temporary
                let mut ref_temp = String::new();
                let mut ref_temp_f = Fivemat::new(&mut ref_temp, INDENT);
                let (pre, post) = &state.tynames[pointee_ty];
                write!(&mut ref_temp_f, "{pre}{ref_temp_name}{post} = ")?;
                let ref_temp_name = format!("{ref_temp_name}_");
                self.init_value(
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
                write!(f, "{{")?;
                for arr_idx in 0..*len {
                    if arr_idx > 0 {
                        write!(f, ", ")?;
                    }
                    let ref_temp_name = format!("{ref_temp_name}{arr_idx}_");
                    self.init_value(f, state, *elem_ty, vals, alias, &ref_temp_name, extra_decls)?;
                }
                write!(f, "}}")?;
            }
            // Nominal types we need to emit a decl for
            Ty::Struct(struct_ty) => {
                write!(f, "{{ ")?;
                for (field_idx, field) in struct_ty.fields.iter().enumerate() {
                    if field_idx > 0 {
                        write!(f, ", ")?;
                    }
                    let field_name = &field.ident;
                    write!(f, ".{field_name} = ")?;
                    let ref_temp_name = format!("{ref_temp_name}{field_name}_");
                    self.init_value(f, state, field.ty, vals, alias, &ref_temp_name, extra_decls)?;
                }
                write!(f, " }}")?;
            }
            Ty::Union(union_ty) => {
                write!(f, "{{ ")?;
                let tag_val = vals.next_val();
                if let Some(field) = tag_val.select_val(&union_ty.fields) {
                    let field_name = &field.ident;
                    write!(f, ".{field_name} = ")?;
                    let ref_temp_name = format!("{ref_temp_name}{field_name}_");
                    self.init_value(f, state, field.ty, vals, alias, &ref_temp_name, extra_decls)?;
                }
                write!(f, " }}")?;
            }

            Ty::Tagged(_tagged_ty) => {
                return Err(UnsupportedError::Other(
                    "c doesn't have tagged unions impled yet".to_owned(),
                ))?;
                /*
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
                            self.init_value(
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
                 */
            }
            Ty::Alias(AliasTy { real, name, .. }) => {
                let alias = alias.or_else(|| Some(name));
                self.init_value(f, state, *real, vals, alias, ref_temp_name, extra_decls)?;
            }

            // Puns should be evaporated
            Ty::Pun(pun) => {
                let real_ty = state.types.resolve_pun(pun, &state.env).unwrap();
                self.init_value(f, state, real_ty, vals, alias, ref_temp_name, extra_decls)?;
            }

            Ty::Empty => {
                return Err(UnsupportedError::Other(
                    "c doesn't have empty tuples".to_owned(),
                ))?
            }
        };

        Ok(())
    }

    pub fn init_var(
        &self,
        f: &mut Fivemat,
        state: &TestState,
        var_name: &str,
        var_ty: TyIdx,
        mut vals: ArgValuesIter,
    ) -> Result<(), GenerateError> {
        // Generate the input
        let mut real_var_decl = String::new();
        let mut real_var_decl_f = Fivemat::new(&mut real_var_decl, INDENT);
        let mut extra_decls = Vec::new();
        let (pre, post) = &state.tynames[&var_ty];
        write!(&mut real_var_decl_f, "{pre}{var_name}{post} = ")?;
        let ref_temp_name = format!("{var_name}_");
        self.init_value(
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
}
