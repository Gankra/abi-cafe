use crate::{abis::*, Config, OutputFormat};
use clap::{AppSettings, Arg};
use log::LevelFilter;
use simplelog::{ColorChoice, TermLogger, TerminalMode};

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
        (ABI_IMPL_RUSTC, ABI_IMPL_CC), // Rust calls C
        (ABI_IMPL_CC, ABI_IMPL_RUSTC), // C calls Rust
        (ABI_IMPL_CC, ABI_IMPL_CC),    // C calls C
    ];

    let app = clap::Command::new("abi-cafe")
        .version(clap::crate_version!())
        .about("Compares the FFI ABIs of different langs/compilers by generating and running them.")
        .next_line_help(true)
        .setting(AppSettings::DeriveDisplayOrder)
        .arg(
            Arg::new("procgen-tests")
                .long("procgen-tests")
                .long_help("Regenerate the procgen test manifests"),
        )
        .arg(
            Arg::new("conventions")
                .long("conventions")
                .long_help("Only run the given calling conventions")
                .possible_values(&[
                    "c",
                    "cdecl",
                    "fastcall",
                    "stdcall",
                    "vectorcall",
                    "handwritten",
                ])
                .multiple_values(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("impls")
                .long("impls")
                .long_help("Only run the given impls (compilers/languages)")
                .possible_values(ABI_IMPLS)
                .multiple_values(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("tests")
                .long("tests")
                .long_help("Only run the given tests")
                .multiple_values(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("pairs")
                .long("pairs")
                .long_help("Only run the given impl pairs, in the form of impl_calls_impl")
                .multiple_values(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("add-rustc-codegen-backend")
                .long("add-rustc-codegen-backend")
                .long_help("Add a rustc codegen backend, in the form of impl_name:path/to/backend")
                .multiple_values(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("output-format")
                .long("output-format")
                .long_help("Set the output format")
                .possible_values(&["human", "json", "rustc-json"])
                .default_value("human")
                .takes_value(true),
        )
        .after_help("");

    let matches = app.get_matches();
    let procgen_tests = matches.is_present("procgen-tests");

    let mut run_conventions: Vec<_> = matches
        .values_of("conventions")
        .into_iter()
        .flatten()
        .map(|conv| CallingConvention::from_str(conv).unwrap())
        .collect();

    if run_conventions.is_empty() {
        run_conventions = ALL_CONVENTIONS.to_vec();
    }

    let run_impls = matches
        .values_of("impls")
        .into_iter()
        .flatten()
        .map(String::from)
        .collect();

    let mut run_pairs: Vec<_> = matches
        .values_of("pairs")
        .into_iter()
        .flatten()
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

    let run_tests = matches
        .values_of("tests")
        .into_iter()
        .flatten()
        .map(String::from)
        .collect();

    let rustc_codegen_backends: Vec<(String, String)> = matches
        .values_of("add-rustc-codegen-backend")
        .into_iter()
        .flatten()
        .map(|pair| {
            pair.split_once(':')
                .expect("invalid syntax, must be 'impl_name:path/to/backend'")
        })
        .map(|(a, b)| (String::from(a), String::from(b)))
        .collect();

    for &(ref name, ref _path) in &rustc_codegen_backends {
        if !run_pairs.iter().any(|(a, b)| a == name || b == name) {
            eprintln!("Warning: Rustc codegen backend `{name}` is not tested.");
            eprintln!(
                "Hint: Try using `--pairs {name}_calls_rustc` or `--pairs rustc_calls_{name}`."
            );
            eprintln!();
        }
    }

    let output_format = match matches.value_of("output-format").unwrap() {
        "human" => OutputFormat::Human,
        "json" => OutputFormat::Json,
        "rustc-json" => OutputFormat::RustcJson,
        _ => unreachable!(),
    };

    let _ = TermLogger::init(
        LevelFilter::Info,
        simplelog::Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .unwrap();

    Config {
        output_format,
        procgen_tests,
        run_conventions,
        run_impls,
        run_tests,
        run_pairs,
        rustc_codegen_backends,
    }
}
