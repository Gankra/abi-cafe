#![doc = include_str!("../README.md")]

use std::{fs::File, io::Read, path::Path, sync::Arc};

use kdl::KdlDocument;
use miette::{Diagnostic, NamedSource};
use thiserror::Error;

pub use parse::{KdlScriptParseError, ParsedProgram, PunEnv};
pub use types::{Definition, DefinitionGraph, KdlScriptTypeError, TypedProgram};

#[cfg(feature = "eval")]
pub mod eval;
pub mod parse;
pub mod spanned;
#[cfg(test)]
mod tests;
pub mod types;

#[derive(Debug, Error, Diagnostic)]
pub enum KdlScriptError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Kdl(#[from] kdl::KdlError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] KdlScriptParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Type(#[from] KdlScriptTypeError),
}

pub struct ErrorHandler {
    pub error_style: ErrorStyle,
    pub error_mode: ErrorMode,
}

pub enum ErrorMode {
    Gather(Vec<KdlScriptError>),
    Scream,
}

pub enum ErrorStyle {
    Human,
    Json,
}

pub struct Compiler {
    // error_handler: ErrorHandler,
    pub source: Option<Arc<NamedSource>>,
    pub parsed: Option<Arc<ParsedProgram>>,
    pub typed: Option<Arc<TypedProgram>>,
}

pub type Result<T> = std::result::Result<T, KdlScriptError>;

impl Compiler {
    pub fn new() -> Self {
        Self {
            /*
            error_handler: ErrorHandler {
                error_mode: ErrorMode::Scream,
                error_style: ErrorStyle::Human,
            },
             */
            source: None,
            parsed: None,
            typed: None,
        }
    }

    pub fn compile_path(
        &mut self,
        src_path: impl AsRef<Path>,
    ) -> std::result::Result<Arc<TypedProgram>, KdlScriptError> {
        let src_path = src_path.as_ref();
        let input_name = src_path.display().to_string();
        let mut input_file = File::open(src_path)?;
        let mut input_string = String::new();
        input_file.read_to_string(&mut input_string)?;

        self.compile_string(&input_name, input_string)
    }

    pub fn compile_string(
        &mut self,
        input_name: &str,
        input_string: String,
    ) -> std::result::Result<Arc<TypedProgram>, KdlScriptError> {
        let input_string = Arc::new(input_string);

        let src = Arc::new(miette::NamedSource::new(input_name, input_string.clone()));
        self.source = Some(src.clone());

        let kdl_doc: KdlDocument = input_string.parse::<kdl::KdlDocument>()?;
        let parsed = Arc::new(parse::parse_kdl_script(self, src, &kdl_doc)?);
        self.parsed = Some(parsed.clone());
        let typed = Arc::new(types::typeck(self, &parsed)?);
        self.typed = Some(typed.clone());

        Ok(typed)
    }

    pub fn eval(&mut self) -> std::result::Result<Option<i64>, KdlScriptError> {
        if let (Some(src), Some(parsed)) = (&self.source, &self.parsed) {
            if parsed.funcs.contains_key("main") {
                let val = eval::eval_kdl_script(src, parsed)?;
                return Ok(Some(val));
            }
        }
        Ok(None)
    }

    /*
    fn diagnose(&mut self, err: KdlScriptError) {
        use ErrorMode::*;
        use ErrorStyle::*;
        match &mut self.error_handler {
            ErrorHandler {
                error_mode: Scream,
                error_style: Human,
            } => {
                eprintln!("{:?}", miette::Report::new(err));
            }
            _ => todo!(),
        }
    }
     */
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}
