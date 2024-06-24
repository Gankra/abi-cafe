use std::io::Write;
use std::path::PathBuf;
/*
const PROCGEN_ROOT: &str = "tests/procgen";

/// For tests that are too tedious to even hand-write the .ron file,
/// this code generates it programmatically.
///
/// **NOTE: this is disabled by default, the results are checked in.
/// If you want to regenerate these tests, just remove the early return.**
pub fn procgen_tests(regenerate: bool) {
    // Regeneration disabled by default.
    if !regenerate {
        return;
    }

    let proc_gen_root = PathBuf::from(PROCGEN_ROOT);

    // Make sure the path exists, then delete its contents, then recreate the empty dir.
    std::fs::create_dir_all(&proc_gen_root).unwrap();
    std::fs::remove_dir_all(&proc_gen_root).unwrap();
    std::fs::create_dir_all(&proc_gen_root).unwrap();

    let tests = &[
        "i128", "i64", "i32", "i16", "i8", "u128", "u64", "u32", "u16", "u8", "ptr", "bool", "f64",
        "f32",
    ];

    for test_name in tests {
        procgen_test_for_ty_file(test_name, test_name, None);
    }
}

pub fn procgen_test_for_ty_file(
    test_name: &str,
    ty_name: &str,
    ty_def: Option<&str>,
) -> PathBuf {
    let test_body = procgen_test_for_ty_string(ty_name, ty_def);
    let proc_gen_root = PathBuf::from(PROCGEN_ROOT);
    let filepath = proc_gen_root.join(format!("{test_name}.kdl"));
    let mut file = std::fs::File::create(&filepath).unwrap();
    file.write_all(test_body.as_bytes()).unwrap();
    filepath
}
*/

pub fn procgen_test_for_ty_string(
    ty_name: &str,
    ty_def: Option<&str>,
) -> String {
    let mut test_body = String::new();
    procgen_test_for_ty_impl(&mut test_body, ty_name, ty_def).unwrap();
    test_body
}

fn procgen_test_for_ty_impl(
    out: &mut dyn std::fmt::Write,
    ty_name: &str,
    ty_def: Option<&str>,
) -> std::fmt::Result {
    let ty = ty_name;
    let ty_ref = format!("&{ty_name}");
    if let Some(ty_def) = ty_def {
        writeln!(out, "{}", ty_def)?;
    }

    // Start gentle with basic one value in/out tests
    add_func(out, "val_in", &[ty], &[])?;
    add_func(out, "val_out", &[], &[ty])?;
    add_func(out, "val_in_out", &[ty], &[ty])?;
    add_func(out, "ref_in", &[&ty_ref], &[])?;
    // add_func(out, "ref_out", &[], &[&ty_ref])?;
    // add_func(out, "ref_in_out", &[&ty_ref], &[&ty_ref])?;

    // Stress out the calling convention and try lots of different
    // input counts. For many types this will result in register
    // exhaustion and get some things passed on the stack.
    for len in 2..=16 {
        add_func(out, &format!("val_in_{len}"), &vec![ty; len], &[])?;
    }

    // Stress out the calling convention with a struct full of values.
    // Some conventions will just shove this in a pointer/stack,
    // others will try to scalarize this into registers anyway.
    add_structs(out, ty)?;

    // Now perturb the arguments by including a byte and a float in
    // the argument list. This will mess with alignment and also mix
    // up the "type classes" (float vs int) and trigger more corner
    // cases in the ABIs as things get distributed to different classes
    // of register.

    // We do small and big versions to check the cases where everything
    // should fit in registers vs not.
    let small_count = 4;
    let big_count = 16;

    add_perturbs(out, ty, small_count, "small")?;
    add_perturbs(out, ty, big_count, "big")?;
    add_perturbs_struct(out, ty, small_count, "small")?;
    add_perturbs_struct(out, ty, big_count, "big")?;
    Ok(())
}

fn add_structs(out: &mut dyn std::fmt::Write, ty: &str) -> std::fmt::Result {
    for len in 1..=16 {
        // Establish type names
        let struct_ty = format!("Many{len}");
        let struct_ty_ref = format!("&{struct_ty}");

        // Emit struct defs
        writeln!(out, r#"struct "{struct_ty}" {{"#)?;
        for field_idx in 0..len {
            writeln!(out, r#"    f{field_idx} "{ty}""#)?;
        }
        writeln!(out, r#"}}"#)?;

        // Check that by-val works
        add_func(out, &format!("struct_in_{len}"), &[&struct_ty], &[])?;
        // Check that by-ref works, for good measure
        add_func(
            out,
            &format!("ref_struct_in_{len}"),
            &[&struct_ty_ref],
            &[],
        )?;
    }
    Ok(())
}

fn add_perturbs(
    out: &mut dyn std::fmt::Write,
    ty: &str,
    count: usize,
    label: &str,
) -> std::fmt::Result {
    for idx in 0..count {
        let inputs = perturb_list(ty, count, idx);
        add_func(
            out,
            &format!("val_in_{idx}_perturbed_{label}"),
            &inputs,
            &[],
        )?;
    }
    Ok(())
}

fn add_perturbs_struct(
    out: &mut dyn std::fmt::Write,
    ty: &str,
    count: usize,
    label: &str,
) -> std::fmt::Result {
    for idx in 0..count {
        let inputs = perturb_list(ty, count, idx);

        // Establish type names
        let struct_ty = format!("Perturbed{label}{idx}");

        // Emit struct defs
        writeln!(out, r#"struct "{struct_ty}" {{"#)?;
        for (field_idx, field_ty) in inputs.iter().enumerate() {
            writeln!(out, r#"    f{field_idx} "{field_ty}""#)?;
        }
        writeln!(out, r#"}}"#)?;

        // Add the function
        add_func(
            out,
            &format!("val_in_{idx}_perturbed_{label}"),
            &[&struct_ty],
            &[],
        )?;
    }
    Ok(())
}

fn perturb_list(ty: &str, count: usize, idx: usize) -> Vec<&str> {
    let mut inputs = vec![ty; count];

    let byte_idx = idx;
    let float_idx = count - 1 - idx;
    inputs[byte_idx] = "u8";
    inputs[float_idx] = "f32";
    inputs
}

fn add_func(
    out: &mut dyn std::fmt::Write,
    func_name: &str,
    inputs: &[&str],
    outputs: &[&str],
) -> std::fmt::Result {
    writeln!(out, r#"fn "{func_name}" {{"#)?;
    writeln!(out, r#"    inputs {{"#)?;
    for (idx, arg_ty) in inputs.iter().enumerate() {
        writeln!(out, r#"        arg{idx} "{arg_ty}""#)?;
    }
    writeln!(out, r#"    }}"#)?;
    writeln!(out, r#"    outputs {{"#)?;
    for (idx, arg_ty) in outputs.iter().enumerate() {
        writeln!(out, r#"        arg{idx} "{arg_ty}""#)?;
    }
    writeln!(out, r#"    }}"#)?;
    writeln!(out, r#"}}"#)?;
    Ok(())
}
