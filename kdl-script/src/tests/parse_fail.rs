#[test]
#[should_panic = "need a type name"]
fn struct_no_name() {
    let program = r##"
        struct {

        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "need a type name"]
fn union_no_name() {
    let program = r##"
        union {

        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "need a type name"]
fn enum_no_name() {
    let program = r##"
        enum {

        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "need a type name"]
fn tagged_no_name() {
    let program = r##"
        tagged {
            
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "need a type name"]
fn pun_no_name() {
    let program = r##"
        pun {
            default { }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "need a type name"]
fn alias_no_name() {
    let program = r##"
        alias
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "should be a type"]
fn struct_int_as_type() {
    let program = r##"
        struct "bad" {
            x 0
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "need a type"]
fn struct_no_type() {
    let program = r##"
        struct "bad" {
            x
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "should be a type"]
fn alias_int_as_alias() {
    let program = r##"
        alias "bad" 1
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "need a type"]
fn alias_no_alias() {
    let program = r##"
        alias "bad"
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "should be a type"]
fn union_int_as_type() {
    let program = r##"
        union "bad" {
            x 0
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "need a type"]
fn union_no_type() {
    let program = r##"
        union "bad" {
            x
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "children should never have been born"]
fn tagged_field_sub_block() {
    let program = r##"
        tagged "bad" {
            x { 
                val "i32" { }
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "should be a type"]
fn tagged_int_as_type() {
    let program = r##"
        tagged "bad" {
            x { 
                val 1
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "need a type"]
fn tagged_no_type() {
    let program = r##"
        tagged "bad" {
            x { 
                val 
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn tagged_prop_field_after() {
    let program = r##"
        tagged "bad" {
            x { 
                val "i32" x=1
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "Named values don't belong here"]
fn tagged_prop_field_before() {
    let program = r##"
        tagged "bad" {
            x { 
                val  x=1 "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "shouldn't have arguments"]
fn tagged_prop_case() {
    let program = r##"
        tagged "bad" {
            x z=1 { 
                val "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "Named values don't belong here"]
fn tagged_prop_name_before() {
    let program = r##"
        tagged z=1 "bad" {
            x { 
                val "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn tagged_prop_name_after() {
    let program = r##"
        tagged "bad" z=1 {
            x { 
                val "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "children should never have been born"]
fn struct_field_block() {
    let program = r##"
        struct "bad" {
            val "i32" { }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn struct_prop_field_after() {
    let program = r##"
        struct "bad" {
            val "i32" x=1
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "Named values don't belong here"]
fn struct_prop_field_before() {
    let program = r##"
        struct "bad" {
            val x=1 "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "Named values don't belong here"]
fn struct_prop_name_before() {
    let program = r##"
        struct z=1 "bad" {
            val "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn struct_prop_name_after() {
    let program = r##"
        struct "bad" z=1 {
            val "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn struct_int_name_after() {
    let program = r##"
        struct "bad" 4 {
            val "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "should be a type name"]
fn struct_int_name_before() {
    let program = r##"
        struct 4 "bad" {
            val "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "children should never have been born"]
fn union_field_block() {
    let program = r##"
        union "bad" {
            val "i32" { }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn union_prop_field_after() {
    let program = r##"
        union "bad" {
            val "i32" x=1
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "Named values don't belong here"]
fn union_prop_field_before() {
    let program = r##"
        union "bad" {
            val x=1 "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "Named values don't belong here"]
fn union_prop_name_before() {
    let program = r##"
        union z=1 "bad" {
            val "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn union_prop_name_after() {
    let program = r##"
        union "bad" z=1 {
            val "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn union_int_name_after() {
    let program = r##"
        union "bad" 4 {
            val "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "should be a type name"]
fn union_int_name_before() {
    let program = r##"
        union 4 "bad" {
            val "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "children should never have been born"]
fn enum_field_block() {
    let program = r##"
        enum "bad" {
            val { }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "children should never have been born"]
fn enum_field_val_block() {
    let program = r##"
        enum "bad" {
            val 1 { }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn enum_prop_field_after() {
    let program = r##"
        enum "bad" {
            val 1 x=1
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "Named values don't belong here"]
fn enum_prop_field_before() {
    let program = r##"
        enum "bad" {
            val x=1 2
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "Named values don't belong here"]
fn enum_prop_name_before() {
    let program = r##"
        enum z=1 "bad" {
            val
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn enum_prop_name_after() {
    let program = r##"
        enum "bad" z=1 {
            val
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn enum_int_name_after() {
    let program = r##"
        enum "bad" 4 {
            val
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "should be a type name"]
fn enum_int_name_before() {
    let program = r##"
        union 4 "bad" {
            val
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "blocks need bodies"]
fn pun_lang_no_body() {
    let program = r##"
        pun "bad" {
            lang "rust"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "need a lang name"]
fn pun_lang_no_lang() {
    let program = r##"
        pun "bad" {
            lang {
                alias "bad" "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "blocks need bodies"]
fn pun_default_no_body() {
    let program = r##"
        pun "bad" {
            default
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "don't know what a 'x' is"]
fn pun_unknown_selector() {
    let program = r##"
        pun "bad" {
            x
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "shouldn't have arguments"]
fn pun_default_lang() {
    let program = r##"
        pun "bad" {
            default "rust" {
                alias "bad" "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "Named values don't belong here"]
fn pun_prop_lang_after() {
    let program = r##"
        pun "bad" {
            lang "rust" x=1 { 
                alias "bad" "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "Named values don't belong here"]
fn pub_prop_lang_before() {
    let program = r##"
        pun "bad" {
            lang x=1 "rust" {
                alias "bad" "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "Named values don't belong here"]
fn pun_prop_name_before() {
    let program = r##"
        pun x=1 "bad" {
            lang "rust" {
                alias "bad" "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn pun_prop_name_after() {
    let program = r##"
        pun "bad" x=1  {
            lang "rust" {
                alias "bad" "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn pun_int_name_after() {
    let program = r##"
        pun "bad" 1  {
            lang "rust" {
                alias "bad" "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "should be a type name"]
fn pun_int_name_before() {
    let program = r##"
        pun 2 "bad" {
            lang "rust" {
                alias "bad" "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "something extra"]
fn pun_two_names() {
    let program = r##"
        pun "bad" "verybad" {
            lang "rust" {
                alias "bad" "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn hanging_ref() {
    let program = r##"
        struct "bad" {
            x "&"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn unclosed_array() {
    let program = r##"
        struct "bad" {
            x "[i32; 0"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn undelimited_array() {
    let program = r##"
        struct "bad" {
            x "[i32 0]"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn unopened_array() {
    let program = r##"
        struct "bad" {
            x "i32; 0]"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn empty_tuple_space() {
    let program = r##"
        struct "bad" {
            x "( )"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn tyname_space() {
    let program = r##"
        struct "bad" {
            x "i 32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn generic() {
    let program = r##"
        struct "bad" {
            x "Option<u32>"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn invalid_tyname_int() {
    let program = r##"
        struct "bad" {
            x "12"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn invalid_tyname_at() {
    let program = r##"
        struct "bad" {
            x "@i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn invalid_tyname_pound() {
    let program = r##"
        struct "bad" {
            x "#i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn invalid_tyname_dash() {
    let program = r##"
        struct "bad" {
            x "-i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "couldn't parse type"]
fn invalid_tyname_leading_int() {
    let program = r##"
        struct "bad" {
            x "1i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "invalid identifier"]
fn struct_invalid_name_ident() {
    let program = r##"
        struct "123" {
            x "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "invalid identifier"]
fn struct_invalid_field_ident() {
    let program = r##"
        struct "bad" {
            #123 "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "invalid identifier"]
fn union_invalid_name_ident() {
    let program = r##"
        union "123" {
            x "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "invalid identifier"]
fn union_invalid_field_ident() {
    let program = r##"
        union "bad" {
            #123 "i32"
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "invalid identifier"]
fn enum_invalid_field_ident() {
    let program = r##"
        enum "bad" {
            #123
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "invalid identifier"]
fn enum_invalid_name_ident() {
    let program = r##"
        struct "123" {
            x
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "invalid identifier"]
fn alias_invalid_field_ident() {
    let program = r##"
        alias "#123" "i32"
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "invalid identifier"]
fn pun_invalid_name_ident() {
    let program = r##"
        pun "#123" {
            default {
                alias "#123" "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "invalid identifier"]
fn tagged_invalid_name_ident() {
    let program = r##"
        tagged "#123" {
            Some {
                _0 "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "invalid identifier"]
fn tagged_invalid_variant_ident() {
    let program = r##"
        tagged "bad" {
            #123 {
                _0 "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "invalid identifier"]
fn tagged_invalid_subfield_ident() {
    let program = r##"
        tagged "bad" {
            Some {
                #123 "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

#[test]
#[should_panic = "declared a type other than what it should have"]
fn pun_unfulfilled() {
    let program = r##"
        pun "bad" {
            default { 
                alias "reallybad" "i32"
            }
        }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.map_err(miette::Report::new).unwrap();
}

/*
#[test]
#[should_panic]
fn bodyless_struct() {
    let program = r##"
        struct "Empty"
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.unwrap();
}

#[test]
#[should_panic]
fn bodyless_enum() {
    let program = r##"
        enum "Empty"
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.unwrap();
}

#[test]
#[should_panic]
fn bodyless_tagged() {
    let program = r##"
        tagged "Empty"
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.unwrap();
}

#[test]
#[should_panic]
fn bodyless_union() {
    let program = r##"
        union "Empty"
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.unwrap();
}

#[test]
#[should_panic]
fn bodyless_pun() {
    let program = r##"
        pun "Empty"
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.unwrap();
}

#[test]
#[should_panic]
fn caseless_pun() {
    let program = r##"
        pun "Empty" { }
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.unwrap();
}

#[test]
#[should_panic]
fn aliasless_alias() {
    let program = r##"
        alias "Empty"
    "##;
    let mut compiler = crate::Compiler::new();
    let res = compiler.compile_string("test.kdl", program.to_owned());
    res.unwrap();
}

 */
