use crate::{abis::*, files::Paths, Config, OutputFormat};
use camino::Utf8PathBuf;
use clap::Parser;
use tracing::warn;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
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
    #[clap(long)]
    gen_vals: Option<ValueGeneratorKind>,
    #[clap(long)]
    write_vals: Option<WriteImpl>,
    #[clap(long)]
    minimize_vals: Option<WriteImpl>,
}

pub fn make_app() -> Config {
    /// The pairings of impls to run. LHS calls RHS.
    static DEFAULT_TEST_PAIRS: &[(&str, &str)] = &[
        (ABI_IMPL_RUSTC, ABI_IMPL_RUSTC), // Rust calls Rust
        (ABI_IMPL_RUSTC, ABI_IMPL_CC),    // Rust calls C
        (ABI_IMPL_CC, ABI_IMPL_RUSTC),    // C calls Rust
        (ABI_IMPL_CC, ABI_IMPL_CC),       // C calls C
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
            warn!(
                "Rustc codegen backend `{name}` is not tested.
Hint: Try using `--pairs {name}_calls_rustc` or `--pairs rustc_calls_{name}`.
"
            );
        }
    }

    let val_generator = config.gen_vals.unwrap_or(ValueGeneratorKind::Graffiti);
    let minimizing_write_impl = config.minimize_vals.unwrap_or(WriteImpl::Print);
    let write_impl = config.write_vals.unwrap_or(WriteImpl::HarnessCallback);

    let output_format = config.output_format;

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let logger = crate::log::MapLogger::new();
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(logger.clone())
        .init();

    let target_dir: Utf8PathBuf = "target".into();
    let out_dir = target_dir.join("temp");
    let generated_src_dir = target_dir.join("generated_impls");
    let runtime_test_input_dir = "abi_cafe_tests".into();
    let paths = Paths {
        target_dir,
        out_dir,
        generated_src_dir,
        runtime_test_input_dir,
    };
    Config {
        output_format,
        procgen_tests,
        run_conventions,
        run_impls,
        run_tests,
        run_pairs,
        rustc_codegen_backends,
        val_generator,
        write_impl,
        minimizing_write_impl,
        paths,
    }
}
