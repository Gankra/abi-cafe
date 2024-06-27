#[test]
#[should_panic = "undefined type name"]
fn no_primitive() {
    let program = r##"
        struct "wrong" {
            x "i1"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "undefined type name"]
fn no_arg_type() {
    let program = r##"
        fn "bad" {
            inputs { x "bad"; }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}
