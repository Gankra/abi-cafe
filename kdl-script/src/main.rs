use std::path::PathBuf;

use clap::Parser;
use kdl_script::Compiler;

#[derive(Parser, Debug)]
pub struct Cli {
    pub src: PathBuf,
}

fn main() -> std::result::Result<(), miette::Report> {
    real_main()?;
    Ok(())
}

fn real_main() -> std::result::Result<(), miette::Report> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_max_level(tracing::level_filters::LevelFilter::WARN)
        .init();

    let mut compiler = Compiler::new();
    let typed = compiler.compile_path(&cli.src)?;

    // Try to eval, otherwise dump the type info / decls
    let result = compiler.eval()?;
    if let Some(result) = result {
        println!("{}", result);
    } else {
        println!("typed program:");
        println!("{:?}", typed);

        println!();
        println!("decls:");
        let env = kdl_script::PunEnv {
            lang: "rust".to_string(),
        };
        let graph = typed.definition_graph(&env)?;
        for def in graph.definitions(typed.all_funcs()) {
            match def {
                kdl_script::Definition::DeclareTy(ty_idx) => {
                    println!("forward-decl type: {}", typed.format_ty(ty_idx));
                }
                kdl_script::Definition::DefineTy(ty_idx) => {
                    println!("define type: {}", typed.format_ty(ty_idx));
                }
                kdl_script::Definition::DeclareFunc(func_idx) => {
                    println!("forward-decl func: {}", typed.realize_func(func_idx).name);
                }
                kdl_script::Definition::DefineFunc(func_idx) => {
                    println!("define func: {}", typed.realize_func(func_idx).name);
                }
            }
        }
    }
    Ok(())
}

/*
fn backend_to_the_future(program: &Arc<TypedProgram>) {

}

fn emit_types_for_funcs
*/
