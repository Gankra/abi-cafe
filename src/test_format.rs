use std::{path::Path, collections::HashMap, sync::Arc};

use kdl_script::types::{ArrayTy, RefTy, AliasTy};

use crate::abis::Test;
pub fn do_kdl(test_file: &Path, input: String) -> Result<Test, miette::Report> {
    use std::fmt::Write;
    let mut compiler = kdl_script::Compiler::new();
    let typed = compiler.compile_string(&test_file.to_string_lossy(), input)?;

    let env = Arc::new(kdl_script::PunEnv {
        lang: "rust".to_string(),
    });
    let graph = Arc::new(typed.definition_graph(&env)?);

    let abi = crate::abis::rust2::RustcAbiImpl::new(None);
    let mut output = Vec::new();
    let test = crate::abis::rust2::Test {
        typed,
        env,
        graph,
        convention: crate::abis::CallingConvention::C,
    };

    abi.generate_caller(&mut output, &test, test.typed.all_funcs()).unwrap();
    let output = String::from_utf8_lossy(&output);
    println!("{output}");
    todo!()
}