use crate::OUTPUT_NAME;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum GenerateError {
    #[error("io error\n{0}")]
    Fmt(#[from] std::fmt::Error),
    #[error("io error\n{0}")]
    Io(#[from] std::io::Error),
    #[error("parse error {0}\n{2}\n{}\n{:width$}^",
        .1.lines().nth(.2.position.line.saturating_sub(1)).unwrap(),
        "",
        width=.2.position.col.saturating_sub(1),
    )]
    ParseError(String, String, ron::error::Error),
    #[error("kdl parse error {}", .2)]
    KdlParseError(String, String, kdl::KdlError),
    #[error("kdl-script error {0}")]
    KdlScriptError(#[from] kdl_script::KdlScriptError),
    #[error("Two structs had the name {name}, but different layout! \nExpected {old_decl} \nGot {new_decl}")]
    InconsistentStructDefinition {
        name: String,
        old_decl: String,
        new_decl: String,
    },
    #[error("If you use the Handwritten calling convention, all functions in the test must use only that.")]
    HandwrittenMixing,
    #[error("No handwritten source for this pairing (skipping)")]
    NoHandwrittenSource,
    #[error("Unsupported Signature For Rust: {0}")]
    RustUnsupported(String),
    #[error("Unsupported Signature For C: {0}")]
    CUnsupported(String),
    #[error("ABI impl doesn't support this calling convention.")]
    UnsupportedConvention,
    /// Used to signal we just skipped it
    #[error("<skipped>")]
    Skipped,
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("io error\n{0}")]
    Io(#[from] std::io::Error),
    #[error("rust compile error \n{} \n{}",
        std::str::from_utf8(&.0.stdout).unwrap(),
        std::str::from_utf8(&.0.stderr).unwrap())]
    RustCompile(std::process::Output),
    #[error("c compile errror\n{0}")]
    CCompile(#[from] cc::Error),
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error)]
pub enum CheckFailure {
    #[error("test {0} {5} field {2} mismatch \ncaller: {3:02X?} \ncallee: {4:02X?}")]
    InputFieldMismatch(usize, usize, usize, Vec<u8>, Vec<u8>, String),
    #[error(
        "test {0} {} field {2} mismatch \ncaller: {3:02X?} \ncallee: {4:02X?}",
        OUTPUT_NAME
    )]
    OutputFieldMismatch(usize, usize, usize, Vec<u8>, Vec<u8>),
    #[error("test {0} {4} field count mismatch \ncaller: {2:#02X?} \ncallee: {3:#02X?}")]
    InputFieldCountMismatch(usize, usize, Vec<Vec<u8>>, Vec<Vec<u8>>, String),
    #[error(
        "test {0} {} field count mismatch \ncaller: {2:#02X?} \ncallee: {3:#02X?}",
        OUTPUT_NAME
    )]
    OutputFieldCountMismatch(usize, usize, Vec<Vec<u8>>, Vec<Vec<u8>>),
    #[error("test {0} input count mismatch \ncaller: {1:#02X?} \ncallee: {2:#02X?}")]
    InputCountMismatch(usize, Vec<Vec<Vec<u8>>>, Vec<Vec<Vec<u8>>>),
    #[error("test {0} output count mismatch \ncaller: {1:#02X?} \ncallee: {2:#02X?}")]
    OutputCountMismatch(usize, Vec<Vec<Vec<u8>>>, Vec<Vec<Vec<u8>>>),
}

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("io error\n{0}")]
    Io(#[from] std::io::Error),
    #[error("rust link error \n{} \n{}",
        std::str::from_utf8(&.0.stdout).unwrap(),
        std::str::from_utf8(&.0.stderr).unwrap())]
    RustLink(std::process::Output),
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("test loading error (dynamic linking failed)\n{0}")]
    LoadError(#[from] libloading::Error),
    #[error("wrong number of tests reported! \nExpected {0} \nGot (caller_in: {1}, caller_out: {2}, callee_in: {3}, callee_out: {4})")]
    TestCountMismatch(usize, usize, usize, usize, usize),
}
