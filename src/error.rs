use miette::Diagnostic;

use crate::harness::test::TestId;

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum CliParseError {
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum UnsupportedError {
    #[error("unsupported: {0}")]
    Other(String),
}

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum GenerateError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Unsupported(#[from] UnsupportedError),
    #[error("io error\n{0}")]
    Fmt(#[from] std::fmt::Error),
    #[error("io error\n{0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    KdlScriptError(#[from] kdl_script::KdlScriptError),
    /// Used to signal we just skipped it
    #[error("<skipped>")]
    Skipped,
    #[error(
        "pun {pun} had blocks with different numbers of values
  block1 had {block1_val_count}: {block1}
  block2 had {block2_val_count}: {block2}"
    )]
    MismatchedPunVals {
        pun: String,
        block1: String,
        block1_val_count: usize,
        block2: String,
        block2_val_count: usize,
    },
    #[error("failed to read and parse test {test}")]
    ReadTest {
        test: TestId,
        #[source]
        #[diagnostic_source]
        details: Box<GenerateError>,
    },
}

impl std::borrow::Borrow<dyn Diagnostic> for Box<GenerateError> {
    fn borrow(&self) -> &(dyn Diagnostic + 'static) {
        &**self
    }
}

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum BuildError {
    #[error("io error\n{0}")]
    Io(#[from] std::io::Error),
    #[error("rust compile error \n{} \n{}",
        std::str::from_utf8(&.0.stdout).unwrap(),
        std::str::from_utf8(&.0.stderr).unwrap())]
    RustCompile(std::process::Output),
    #[error("c compile error\n{0}")]
    CCompile(#[from] cc::Error),
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum CheckFailure {
    #[error(
        "  {func_name} {arg_kind} differed:
    {arg_kind:<6}: {arg_name}: {arg_ty_name}
    field : {val_path}: {val_ty_name}
    expect: {expected:02X?}
    caller: {caller:02X?}
    callee: {callee:02X?}"
    )]
    ValMismatch {
        func_idx: usize,
        arg_idx: usize,
        val_idx: usize,
        func_name: String,
        arg_name: String,
        arg_kind: String,
        arg_ty_name: String,
        val_path: String,
        val_ty_name: String,
        expected: Vec<u8>,
        caller: Vec<u8>,
        callee: Vec<u8>,
    },
    #[error(
        "  {func_name} {arg_kind} had unexpected tagged variant:
    {arg_kind:<6}: {arg_name}: {arg_ty_name}
    field : {val_path}: {val_ty_name}
    expect: {expected}
    caller: {caller}
    callee: {callee}"
    )]
    TagMismatch {
        func_idx: usize,
        arg_idx: usize,
        val_idx: usize,
        func_name: String,
        arg_name: String,
        arg_kind: String,
        arg_ty_name: String,
        val_path: String,
        val_ty_name: String,
        expected: String,
        caller: String,
        callee: String,
    },
}

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum LinkError {
    #[error("io error\n{0}")]
    Io(#[from] std::io::Error),
    #[error("rust link error \n{} \n{}",
        std::str::from_utf8(&.0.stdout).unwrap(),
        std::str::from_utf8(&.0.stderr).unwrap())]
    RustLink(std::process::Output),
}

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum RunError {
    #[error("test loading error (dynamic linking failed)\n{0}")]
    LoadError(#[from] libloading::Error),
    #[error("test impl didn't call set_func before calling write_val")]
    MissingSetFunc,
    #[error("test impl called write_val on func {func} val {val} twice")]
    DoubleWrite { func: usize, val: usize },
}
