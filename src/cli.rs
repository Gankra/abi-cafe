use crate::{abis::*, Config, OutputFormat};
use clap::Parser;
use log::LevelFilter;
use simplelog::{ColorChoice, TermLogger, TerminalMode};
use vals::ValueGeneratorKind;

#[derive(Parser)]
struct Cli {
    #[clap(long)]
    procgen_tests: bool,
    #[clap(long)]
    conventions: Vec<CallingConvention>,
    #[clap(long)]
    impls: Vec<String>,
    #[clap(long)]
    pairs: Vec<String>,
    #[clap(long)]
    tests: Vec<String>,
    #[clap(long)]
    add_rustc_codegen_backend: Vec<String>,
    #[clap(long, default_value_t = OutputFormat::Human)]
    output_format: OutputFormat,
}

pub fn make_app() -> Config {
    static ABI_IMPLS: &[&str] = &[
        ABI_IMPL_RUSTC,
        ABI_IMPL_CC,
        ABI_IMPL_GCC,
        ABI_IMPL_CLANG,
        ABI_IMPL_MSVC,
    ];
    /// The pairings of impls to run. LHS calls RHS.
    static DEFAULT_TEST_PAIRS: &[(&str, &str)] = &[
        (ABI_IMPL_RUSTC, ABI_IMPL_RUSTC), // (ABI_IMPL_RUSTC, ABI_IMPL_CC), // Rust calls C
                                          // (ABI_IMPL_CC, ABI_IMPL_RUSTC), // C calls Rust
                                          // (ABI_IMPL_CC, ABI_IMPL_CC),    // C calls C
    ];

    let config = Cli::parse();
    let procgen_tests = config.procgen_tests;
    let run_conventions = if config.conventions.is_empty() {
        ALL_CONVENTIONS.to_vec()
    } else {
        config.conventions
    };

    let run_impls = config.impls;

    let mut run_pairs: Vec<_> = config
        .pairs
        .iter()
        .map(|pair| {
            pair.split_once("_calls_")
                .expect("invalid 'pair' syntax, must be 'impl_calls_impl'")
        })
        .map(|(a, b)| (String::from(a), String::from(b)))
        .collect();
    if run_pairs.is_empty() {
        run_pairs = DEFAULT_TEST_PAIRS
            .iter()
            .map(|&(a, b)| (String::from(a), String::from(b)))
            .collect()
    }
    let run_tests = config.tests;

    let rustc_codegen_backends: Vec<(String, String)> = config
        .add_rustc_codegen_backend
        .iter()
        .map(|pair| {
            pair.split_once(':')
                .expect("invalid syntax, must be 'impl_name:path/to/backend'")
        })
        .map(|(a, b)| (String::from(a), String::from(b)))
        .collect();

    for (name, _path) in &rustc_codegen_backends {
        if !run_pairs.iter().any(|(a, b)| a == name || b == name) {
            eprintln!("Warning: Rustc codegen backend `{name}` is not tested.");
            eprintln!(
                "Hint: Try using `--pairs {name}_calls_rustc` or `--pairs rustc_calls_{name}`."
            );
            eprintln!();
        }
    }

    let value_generator = ValueGeneratorKind::Graffiti;

    let output_format = config.output_format;
    TermLogger::init(
        LevelFilter::Info,
        simplelog::Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .expect("failed to initialize logger");

    Config {
        output_format,
        procgen_tests,
        run_conventions,
        run_impls,
        run_tests,
        run_pairs,
        rustc_codegen_backends,
        val_generator: value_generator,
    }
}
