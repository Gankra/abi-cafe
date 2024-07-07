use super::*;
use kdl_script::types::{Ty, TyIdx};
use std::fmt::Write;

impl RustcToolchain {
    /// Every test should start by loading in the harness' "header"
    /// and forward-declaring any structs that will be used.
    pub fn write_harness_prefix(
        &self,
        f: &mut Fivemat,
        state: &TestState,
    ) -> Result<(), GenerateError> {
        // No extra harness gunk if not needed
        if state.options.val_writer != WriteImpl::HarnessCallback {
            return Ok(());
        }
        if state.options.convention == CallingConvention::Vectorcall {
            writeln!(f, "#![feature(abi_vectorcall)]")?;
        }
        let mut has_f16 = false;
        let mut has_f128 = false;
        for def in state.defs.definitions(state.desired_funcs.iter().copied()) {
            match def {
                kdl_script::Definition::DeclareTy(ty) | kdl_script::Definition::DefineTy(ty) => {
                    match state.types.realize_ty(ty) {
                        Ty::Primitive(PrimitiveTy::F16) => has_f16 = true,
                        Ty::Primitive(PrimitiveTy::F128) => has_f128 = true,
                        _ => {}
                    }
                }
                kdl_script::Definition::DefineFunc(_) | kdl_script::Definition::DeclareFunc(_) => {}
            }
        }
        if has_f16 {
            writeln!(f, "#![feature(f16)]")?;
        }
        if has_f128 {
            writeln!(f, "#![feature(f128)]")?;
        }
        // Load test harness "headers"
        writeln!(
            f,
            "{}",
            crate::files::get_file("harness/rust/harness_prefix.rs")
        )?;
        writeln!(f)?;

        Ok(())
    }

    /// Emit the WRITE calls and FINISHED_VAL for this value.
    /// This will WRITE every leaf subfield of the type.
    /// `to` is the BUFFER to use, `from` is the variable name of the value.
    pub fn write_var(
        &self,
        f: &mut Fivemat,
        state: &TestState,
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

        Ok(())
    }

    /// Recursive subroutine of write_var, which builds up rvalue paths and generates
    /// appropriate match statements. Actual WRITE calls are done by write_leaf_field.
    pub fn write_fields(
        &self,
        f: &mut Fivemat,
        state: &TestState,
        to: &str,
        from: &str,
        var_ty: TyIdx,
        vals: &mut ArgValuesIter,
    ) -> Result<(), GenerateError> {
        match state.types.realize_ty(var_ty) {
            Ty::Primitive(_) => {
                // Hey an actual leaf, report it (and burn a value)
                let val = vals.next_val();
                if val.should_write_val(&state.options) {
                    self.write_leaf_field(f, state, to, from, &val)?;
                }
            }
            Ty::Enum(enum_ty) => {
                // enums are leaves but we care about their semantic value (variant case)
                // so don't just pass the raw bytes, treat this like a tag we match on
                let tag_generator = vals.next_val();
                let tag_idx = tag_generator.generate_idx(enum_ty.variants.len());
                if let Some(variant) = enum_ty.variants.get(tag_idx) {
                    let enum_name = &enum_ty.name;
                    let variant_name = &variant.name;
                    if tag_generator.should_write_val(&state.options) {
                        writeln!(f, "if let {enum_name}::{variant_name} = &{from} {{")?;
                        f.add_indent(1);
                        self.write_tag_field(f, state, to, from, tag_idx, &tag_generator)?;
                        f.sub_indent(1);
                        writeln!(f, "}} else {{")?;
                        f.add_indent(1);
                        self.write_error_tag_field(f, state, to, &tag_generator)?;
                        f.sub_indent(1);
                        writeln!(f, "}}")?;
                    }
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
                            self.write_tag_field(f, state, to, from, tag_idx, &tag_generator)?;
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
                            self.write_error_tag_field(f, state, to, &tag_generator)?;
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
                    self.write_tag_field(f, state, to, from, tag_idx, &tag_generator)?;
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
    pub fn write_leaf_field(
        &self,
        f: &mut Fivemat,
        state: &TestState,
        to: &str,
        path: &str,
        val: &ValueRef,
    ) -> Result<(), GenerateError> {
        match state.options.val_writer {
            WriteImpl::HarnessCallback => {
                let val_idx = val.absolute_val_idx;
                // Convenience for triggering test failures
                let rvalue = if path.contains("abicafepoison") && to.contains(CALLEE_VALS) {
                    "0x12345678u32"
                } else {
                    path
                };
                writeln!(f, "write_val({to}, {val_idx}, &{rvalue});")?;
            }
            WriteImpl::Assert => {
                write!(f, "assert_eq!({path}, ")?;
                self.init_leaf_value(f, state, val.ty, val, None)?;
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

    pub fn write_tag_field(
        &self,
        f: &mut Fivemat,
        state: &TestState,
        to: &str,
        path: &str,
        variant_idx: usize,
        val: &ValueRef,
    ) -> Result<(), GenerateError> {
        match state.options.val_writer {
            WriteImpl::HarnessCallback => {
                // Convenience for triggering test failures
                if path.contains("abicafepoison") && to.contains(CALLEE_VALS) {
                    return self.write_error_tag_field(f, state, to, val);
                }
                let val_idx = val.absolute_val_idx;
                writeln!(f, "write_val({to}, {val_idx}, &{}u32);", variant_idx)?;
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

    pub fn write_error_tag_field(
        &self,
        f: &mut Fivemat,
        state: &TestState,
        to: &str,
        val: &ValueRef,
    ) -> Result<(), GenerateError> {
        match state.options.val_writer {
            WriteImpl::HarnessCallback => {
                let val_idx = val.absolute_val_idx;
                writeln!(f, "write_val({to}, {val_idx}, &{}u32);", u32::MAX)?;
            }
            WriteImpl::Assert | WriteImpl::Print => {
                writeln!(f, r#"unreachable!("enum had unexpected variant!?")"#)?;
            }
            WriteImpl::Noop => {
                // Noop, do nothing
            }
        }
        Ok(())
    }

    pub fn write_set_function(
        &self,
        f: &mut dyn Write,
        state: &TestState,
        vals: &str,
        idx: usize,
    ) -> Result<(), GenerateError> {
        match state.options.val_writer {
            WriteImpl::HarnessCallback => {
                writeln!(f, "set_func({vals}, {idx});")?;
            }
            WriteImpl::Print | WriteImpl::Noop | WriteImpl::Assert => {
                // Noop
            }
        }
        Ok(())
    }
}
