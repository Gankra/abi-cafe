use crate::abis::*;
use std::io::Write;
use std::path::PathBuf;

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

    let proc_gen_root = PathBuf::from("tests/procgen");

    // Make sure the path exists, then delete its contents, then recreate the empty dir.
    std::fs::create_dir_all(&proc_gen_root).unwrap();
    std::fs::remove_dir_all(&proc_gen_root).unwrap();
    std::fs::create_dir_all(&proc_gen_root).unwrap();

    let tests: &[(&str, &[Val])] = &[
        // Just run basic primitives that everyone should support through their paces.
        // This is chunked out a bit to avoid stressing the compilers/linkers too much,
        // in case some work scales non-linearly. It also keeps the test suite
        // a bit more "responsive" instead of just stalling one enormous supertest.
        ("i64", &[Val::Int(IntVal::c_int64_t(0x1a2b3c4d_23eaf142))]),
        ("i32", &[Val::Int(IntVal::c_int32_t(0x1a2b3c4d))]),
        ("i16", &[Val::Int(IntVal::c_int16_t(0x1a2b))]),
        ("i8", &[Val::Int(IntVal::c_int8_t(0x1a))]),
        ("u64", &[Val::Int(IntVal::c_uint64_t(0x1a2b3c4d_23eaf142))]),
        ("u32", &[Val::Int(IntVal::c_uint32_t(0x1a2b3c4d))]),
        ("u16", &[Val::Int(IntVal::c_uint16_t(0x1a2b))]),
        ("u8", &[Val::Int(IntVal::c_uint8_t(0x1a))]),
        ("ptr", &[Val::Ptr(0x1a2b3c4d_23eaf142)]),
        ("bool", &[Val::Bool(true)]),
        ("f64", &[Val::Float(FloatVal::c_double(809239021.392))]),
        ("f32", &[Val::Float(FloatVal::c_float(-4921.3527))]),
        // These are split out because they are the buggy mess that inspired this whole enterprise!
        // These types are a GCC exenstion. Windows is a huge dumpster fire where no one agrees on
        // it (MSVC doesn't even define __(u)int128_t afaict, but has some equivalent extension).
        //
        // On linux-based platforms where this is a more established thing, current versions of
        // rustc underalign the value (as if it's emulated, like u64 on x86). This isn't a problem
        // in-and-of-itself because rustc accurately says "this isn't usable for FFI".
        // Unfortunately platforms like aarch64 (arm64) use this type in their definitions for
        // saving/restoring float registers, so it's very much so part of the platform ABI,
        // and Rust should just *fix this*.
        (
            "ui128",
            &[
                Val::Int(IntVal::c__int128(0x1a2b3c4d_23eaf142_7a320c01_e0120a82)),
                Val::Int(IntVal::c__uint128(0x1a2b3c4d_23eaf142_7a320c01_e0120a82)),
            ],
        ),
    ];

    for (test_name, vals) in tests {
        let mut test = Test {
            name: test_name.to_string(),
            funcs: Vec::new(),
        };

        let mut perturb_float = 0.0f32;
        let mut perturb_byte = 0u8;

        for val in vals.iter() {
            let new_val = |i| -> Val {
                // TODO: actually perturb the values?
                let mut new_val = val.clone();
                let mut cur_val = Some(&mut new_val);
                while let Some(temp) = cur_val.take() {
                    match temp {
                        Val::Ref(pointee) => {
                            cur_val = Some(&mut **pointee);
                            continue;
                        }
                        Val::Struct(_, _) => unimplemented!(),
                        Val::Array(_) => unimplemented!(),
                        Val::Ptr(out) => graffiti_primitive(out, i),
                        Val::Int(int_val) => match int_val {
                            IntVal::c__int128(out) => graffiti_primitive(out, i),
                            IntVal::c_int64_t(out) => graffiti_primitive(out, i),
                            IntVal::c_int32_t(out) => graffiti_primitive(out, i),
                            IntVal::c_int16_t(out) => graffiti_primitive(out, i),
                            IntVal::c_int8_t(out) => graffiti_primitive(out, i),
                            IntVal::c__uint128(out) => graffiti_primitive(out, i),
                            IntVal::c_uint64_t(out) => graffiti_primitive(out, i),
                            IntVal::c_uint32_t(out) => graffiti_primitive(out, i),
                            IntVal::c_uint16_t(out) => graffiti_primitive(out, i),
                            IntVal::c_uint8_t(out) => graffiti_primitive(out, i),
                        },
                        Val::Float(float_val) => match float_val {
                            FloatVal::c_double(out) => graffiti_primitive(out, i),
                            FloatVal::c_float(out) => graffiti_primitive(out, i),
                        },
                        Val::Bool(out) => *out = true,
                    }
                }

                new_val
            };

            let val_name = arg_ty(val);

            // Start gentle with basic one value in/out tests
            test.funcs.push(Func {
                name: format!("{val_name}_val_in"),
                conventions: vec![CallingConvention::All],
                inputs: vec![new_val(0)],
                output: None,
            });

            test.funcs.push(Func {
                name: format!("{val_name}_val_out"),
                conventions: vec![CallingConvention::All],
                inputs: vec![],
                output: Some(new_val(0)),
            });

            test.funcs.push(Func {
                name: format!("{val_name}_val_in_out"),
                conventions: vec![CallingConvention::All],
                inputs: vec![new_val(0)],
                output: Some(new_val(1)),
            });

            // Start gentle with basic one value in/out tests
            test.funcs.push(Func {
                name: format!("{val_name}_ref_in"),
                conventions: vec![CallingConvention::All],
                inputs: vec![Val::Ref(Box::new(new_val(0)))],
                output: None,
            });

            test.funcs.push(Func {
                name: format!("{val_name}_ref_out"),
                conventions: vec![CallingConvention::All],
                inputs: vec![],
                output: Some(Val::Ref(Box::new(new_val(0)))),
            });

            test.funcs.push(Func {
                name: format!("{val_name}_ref_in_out"),
                conventions: vec![CallingConvention::All],
                inputs: vec![Val::Ref(Box::new(new_val(0)))],
                output: Some(Val::Ref(Box::new(new_val(1)))),
            });

            // Stress out the calling convention and try lots of different
            // input counts. For many types this will result in register
            // exhaustion and get some things passed on the stack.
            for len in 2..=16 {
                test.funcs.push(Func {
                    name: format!("{val_name}_val_in_{len}"),
                    conventions: vec![CallingConvention::All],
                    inputs: (0..len).map(|i| new_val(i)).collect(),
                    output: None,
                });
            }

            // Stress out the calling convention with a struct full of values.
            // Some conventions will just shove this in a pointer/stack,
            // others will try to scalarize this into registers anyway.
            for len in 1..=16 {
                test.funcs.push(Func {
                    name: format!("{val_name}_struct_in_{len}"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Struct(
                        format!("{val_name}_{len}"),
                        (0..len).map(|i| new_val(i)).collect(),
                    )],
                    output: None,
                });
            }
            // Check that by-ref works, for good measure
            for len in 1..=16 {
                test.funcs.push(Func {
                    name: format!("{val_name}_ref_struct_in_{len}"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Ref(Box::new(Val::Struct(
                        format!("{val_name}_{len}"),
                        (0..len).map(|i| new_val(i)).collect(),
                    )))],
                    output: None,
                });
            }

            // Now perturb the arguments by including a byte and a float in
            // the argument list. This will mess with alignment and also mix
            // up the "type classes" (float vs int) and trigger more corner
            // cases in the ABIs as things get distributed to different classes
            // of register.

            // We do small and big versions to check the cases where everything
            // should fit in registers vs not.
            let small_count = 4;
            let big_count = 16;

            for idx in 0..small_count {
                let mut inputs = (0..small_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = small_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_val_in_{idx}_perturbed_small"),
                    conventions: vec![CallingConvention::All],
                    inputs: inputs,
                    output: None,
                });
            }
            for idx in 0..big_count {
                let mut inputs = (0..big_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = big_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_val_in_{idx}_perturbed_big"),
                    conventions: vec![CallingConvention::All],
                    inputs: inputs,
                    output: None,
                });
            }

            for idx in 0..small_count {
                let mut inputs = (0..small_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = small_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_struct_in_{idx}_perturbed_small"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Struct(
                        format!("{val_name}_{idx}_perturbed_small"),
                        inputs,
                    )],
                    output: None,
                });
            }
            for idx in 0..big_count {
                let mut inputs = (0..big_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = big_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_struct_in_{idx}_perturbed_big"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Struct(
                        format!("{val_name}_{idx}_perturbed_big"),
                        inputs,
                    )],
                    output: None,
                });
            }

            // Should be an exact copy-paste of the above but with Ref's added
            for idx in 0..small_count {
                let mut inputs = (0..small_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = small_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_ref_struct_in_{idx}_perturbed_small"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Ref(Box::new(Val::Struct(
                        format!("{val_name}_{idx}_perturbed_small"),
                        inputs,
                    )))],
                    output: None,
                });
            }
            for idx in 0..big_count {
                let mut inputs = (0..big_count).map(|i| new_val(i)).collect::<Vec<_>>();

                let byte_idx = idx;
                let float_idx = big_count - 1 - idx;
                graffiti_primitive(&mut perturb_byte, byte_idx);
                graffiti_primitive(&mut perturb_float, float_idx);
                inputs[byte_idx] = Val::Int(IntVal::c_uint8_t(perturb_byte));
                inputs[float_idx] = Val::Float(FloatVal::c_float(perturb_float));

                test.funcs.push(Func {
                    name: format!("{val_name}_ref_struct_in_{idx}_perturbed_big"),
                    conventions: vec![CallingConvention::All],
                    inputs: vec![Val::Ref(Box::new(Val::Struct(
                        format!("{val_name}_{idx}_perturbed_big"),
                        inputs,
                    )))],
                    output: None,
                });
            }
        }
        let mut file =
            std::fs::File::create(proc_gen_root.join(format!("{test_name}.ron"))).unwrap();
        let output = ron::to_string(&test).unwrap();
        file.write_all(output.as_bytes()).unwrap();
    }
}

/// The type name to use for this value when it is stored in args/vars.
pub fn arg_ty(val: &Val) -> String {
    use IntVal::*;
    use Val::*;
    match val {
        Ref(x) => format!("ref_{}", arg_ty(x)),
        Ptr(_) => format!("ptr"),
        Bool(_) => format!("bool"),
        Array(vals) => format!(
            "arr_{}_{}",
            vals.len(),
            arg_ty(vals.get(0).expect("arrays must have length > 0")),
        ),
        Struct(name, _) => format!("struct_{name}"),
        Float(FloatVal::c_double(_)) => format!("f64"),
        Float(FloatVal::c_float(_)) => format!("f32"),
        Int(int_val) => match int_val {
            c__int128(_) => format!("i128"),
            c_int64_t(_) => format!("i64"),
            c_int32_t(_) => format!("i32"),
            c_int16_t(_) => format!("i16"),
            c_int8_t(_) => format!("i8"),
            c__uint128(_) => format!("u128"),
            c_uint64_t(_) => format!("u64"),
            c_uint32_t(_) => format!("u32"),
            c_uint16_t(_) => format!("u16"),
            c_uint8_t(_) => format!("u8"),
        },
    }
}

fn graffiti_primitive<T>(output: &mut T, idx: usize) {
    let mut input = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        0x0F,
    ];
    for byte in &mut input {
        *byte |= 0x10 * idx as u8;
    }
    unsafe {
        let out_size = std::mem::size_of::<T>();
        assert!(out_size <= input.len());
        let raw_out = output as *mut T as *mut u8;
        raw_out.copy_from(input.as_ptr(), out_size)
    }
}
