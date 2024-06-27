#[test]
fn prim_struct() -> Result<(), miette::Report> {
    let program = r##"
        struct "Primitives" {
            _0 "u8"
            _1 "u16"
            _2 "u32"
            _3 "u64"
            _4 "u128"
            _5 "u256"
            _6 "i8"
            _7 "i16"
            _8 "i32"
            _9 "i64"
            _10 "i128"
            _11 "i256"
            _12 "bool"
            _13 "f16"
            _14 "f32"
            _15 "f64"
            _16 "f128"
            _17 "ptr"
            _18 "()"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn c_enum_simple() -> Result<(), miette::Report> {
    let program = r##"
        enum "Cases" {
            A
            B
            C
        }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn c_enum_literals1() -> Result<(), miette::Report> {
    let program = r##"
        enum "Cases" {
            A 8
            B
            C
        }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn c_enum_literals2() -> Result<(), miette::Report> {
    let program = r##"
        enum "Cases" {
            A 8
            B 10
            C 52
        }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn c_enum_literals3() -> Result<(), miette::Report> {
    // TODO: this one might be dubious? Check what C/C++ allow with puns here
    let program = r##"
        enum "Cases" {
            A 8
            B 10
            C 10
        }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn tagged_simple() -> Result<(), miette::Report> {
    let program = r##"
        tagged "MyOption" {
            None
            Some {
                _0 "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn union_simple() -> Result<(), miette::Report> {
    let program = r##"
        union "IntFloat" {
            A "i32"
            B "f32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn alias_simple() -> Result<(), miette::Report> {
    let program = r##"
        alias "BigMeters" "u128"
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn pun_simple() -> Result<(), miette::Report> {
    let program = r##"
        pun "Blah" {
            lang "rust" {
                alias "Blah" "i32"
            }
            default {
                alias "Blah" "u32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn empty_struct() -> Result<(), miette::Report> {
    let program = r##"
        struct "Empty" { }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn empty_enum() -> Result<(), miette::Report> {
    let program = r##"
        enum "Empty" { }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn empty_tagged() -> Result<(), miette::Report> {
    let program = r##"
        tagged "Empty" { }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn empty_union() -> Result<(), miette::Report> {
    let program = r##"
        union "Empty" { }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn array_basics() -> Result<(), miette::Report> {
    let program = r##"
        fn "arrays" {
            inputs {
                arr0 "[i32; 0]"
                arr1 "[bool;1]"
                arr2 "[ [ u128 ; 10 ] ; 100 ]"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn ref_basics() -> Result<(), miette::Report> {
    let program = r##"
        fn "arrays" {
            inputs {
                ref0 "&bool"
                ref1 "& i32"
                ref2 "&()"
                ref4 "&&&f64"
                ref5 "[&u8; 100]"
                ref6 "&[&[&&[&u32; 10]; 11]; 12]"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn anon_vars() -> Result<(), miette::Report> {
    let program = r##"
        struct "MyTupleStruct" {
            _ "bool"
            _ "i32"
            _ "u64"
        }
        tagged "MyTagged" {
            TupleVariant {
                _ "bool"
                _ "i32"
                _ "u64"
            }
            HybridVariant {
                x "bool"
                _ "u32"
            }
            StructVariant {
                x "u8"
                z "i64"
            }
            EmptyVariant {
            }
            NoneLike
        }
        fn "anons" {
            inputs {
                _ "u32"
                _ "MyTupleStruct"
                z "i8"
            }
            outputs {
                _ "MyTagged"
                _ "&i32"
                w "&i64"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    compiler.compile_string("test.kdl", program.to_owned())?;
    Ok(())
}

#[test]
fn example_types() -> Result<(), miette::Report> {
    let mut compiler = crate::Compiler::new();
    compiler.compile_path("examples/types.kdl")?;
    Ok(())
}

#[test]
fn example_simple() -> Result<(), miette::Report> {
    let mut compiler = crate::Compiler::new();
    compiler.compile_path("examples/simple.kdl")?;
    Ok(())
}

#[test]
fn example_puns() -> Result<(), miette::Report> {
    let mut compiler = crate::Compiler::new();
    compiler.compile_path("examples/puns.kdl")?;
    Ok(())
}
