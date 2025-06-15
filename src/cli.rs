use crate::harness::test::*;
use crate::harness::vals::*;
use crate::toolchains::*;
use crate::{files::Paths, Config, OutputFormat};

use camino::Utf8PathBuf;
use clap::Parser;
use kdl_script::parse::LangRepr;
use tracing::warn;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static DEFAULT_TOOLCHAINS: &[&str] = &[TOOLCHAIN_CC, TOOLCHAIN_RUSTC];
static DEFAULT_REPRS: &[LangRepr] = &[LangRepr::Rust, LangRepr::C];
static DEFAULT_PAIRERS: &[&str] = &[TOOLCHAIN_RUSTC, TOOLCHAIN_CC];
static DEFAULT_CONVENTIONS: &[CallingConvention] = &[
    // C!
    CallingConvention::C,
    CallingConvention::Cdecl,
    CallingConvention::Stdcall,
    CallingConvention::Fastcall,
    CallingConvention::Vectorcall,
    // Rust!
    CallingConvention::Rust,
];

/// Pair your toolchains at the ABI Cafe!
///
/// When run, we will generate, build, run, and check the crossproduct of:
///
/// --tests --conventions --reprs --pairs --gen-vals --write-vals --select-vals
///
/// Most of these combinations will end up marked as "skipped", because e.g.
/// the cc codegen backend will refuse to try to generate repr(Rust) structs,
/// or becuase fastcall doesn't exist on linux, etc.
///
/// Some of the combinations will end up marked as "busted" or "random" because
/// they're known to be gibberish or broken, and that's ok! We're here to find those things!
#[derive(Parser)]
struct Cli {
    /// which test files to run (SimpleStruct, MetersU32, ...)
    ///
    /// default: (all of them)
    #[clap(long, short, value_delimiter(','))]
    tests: Vec<String>,

    /// calling conventions to try for each test (c, rust, fastcall, ...)
    #[clap(long, short, value_delimiter(','))]
    #[clap(default_values_t = DEFAULT_CONVENTIONS.to_owned())]
    conventions: Vec<CallingConvention>,

    /// type reprs to try for each test (c, rust, ...)
    #[clap(long, short, value_delimiter(','))]
    #[clap(default_values_t = DEFAULT_REPRS.to_owned())]
    reprs: Vec<LangRepr>,

    /// which toolchains should be available for pairing (cc, rustc, gcc, ...)
    #[clap(long, short = 'l', alias = "impls", value_delimiter(','))]
    #[clap(default_values_t = DEFAULT_TOOLCHAINS.iter().map(|s| s.to_string()).collect::<Vec<_>>())]
    toolchains: Vec<String>,

    /// which toolchain pairings to run for each test (cc_calls_rustc, rustc_calls_rustc, ..)
    ///
    /// default: all enabled toolchains will call themselves,
    /// and call/be-called-by rustc and cc (if those are enabled)
    #[clap(long, short, value_delimiter(','))]
    pairs: Vec<String>,

    /// which values to try for each test (graffiti, random1, random17, ...)
    ///
    /// "graffiti" prefers patterning the bytes of values in a way that helps you
    /// identify which byte of which field each recorded value was.
    ///
    /// "randomN" seeds an RNG with N to make random (repeatable) values with.
    #[clap(long, short, value_delimiter(','))]
    #[clap(default_values_t = vec![ValueGeneratorKind::Graffiti])]
    gen_vals: Vec<ValueGeneratorKind>,

    /// which value wrting/reporting styles to generate for each test (harness, print, assert, noop)
    ///
    /// "harness" uses callbacks to report the values back to the abi cafe test harness
    /// "print" uses println/printf
    /// "assert" uses asserts against the expected value
    /// "noop" emits no printing
    ///
    /// Note that only "harness" mode can actually be *checked*. The other modes
    /// exist for exporting the programs into a form that can be inspected/reported.
    #[clap(long, short, value_delimiter(','))]
    #[clap(default_values_t = vec![WriteImpl::HarnessCallback])]
    write_vals: Vec<WriteImpl>,

    /// UNIMPLEMENTED: which of the values in a test to write (see --write-vals)
    ///
    /// This is an internal feature of abi-cafe, and used in minimization (see --minimize-vals),
    /// but is not currently exposed as a thing you can actually ask for, pending a syntax.
    #[clap(long, short, value_delimiter(','))]
    select_vals: Vec<String>,

    /// when a test fails, and we regenerate a minimized value,
    /// replace the --write-vals selection with this one (presumably cleaner/prettier)
    #[clap(long, short)]
    #[clap(default_value_t = WriteImpl::Print)]
    minimize_vals: WriteImpl,

    /// UNIMPLEMENTED: sugar for selecting all the test combo settings at once using
    /// the test key syntax. i.e. "mytest::conv_rust::repr_rust::rustc_calls_cc::random3"
    ///
    /// See <https://github.com/Gankra/abi-cafe/issues/37>
    #[clap(long, short, value_delimiter(','))]
    key: Vec<String>,

    /// final report output format (human, json)
    #[clap(long, default_value_t = OutputFormat::Human)]
    output_format: OutputFormat,

    /// add a rustc_codegen_backend, with the syntax "toolchain_name:path/to/backend"
    ///
    /// toolchain_name here is an arbitrary id that will be used to uniquely identify
    /// the backend as a toolchain, for the purposes of --toolchains and --pairs
    #[clap(long, value_delimiter(','))]
    add_rustc_codegen_backend: Vec<String>,

    /// spider the given directory for .kdl and .procgen.kdl test files at runtime,
    /// and add them to the test suite.
    ///
    /// The structure of the subdirectories doesn't matter, you can organize them
    /// however you want, just know we'll spider into all of them to look for test files!
    ///
    /// Note that there are already builtin tests (disabled with `--disable-builtin-tests`),
    /// and it would be nice for tests to be upstreamed so everyone can benefit!
    #[clap(long)]
    add_tests: Option<Utf8PathBuf>,

    /// Add the test expectations at the given path
    ///
    /// (If not specified we'll look for a file called abi-cafe-rules.toml in the working dir)
    ///
    /// Note that there are already builtin rules (disabled with `--disable-builtin-rules`),
    /// and it would be nice for rules to be upstreamed so everyone can benefit!
    #[clap(long)]
    rules: Option<Utf8PathBuf>,

    /// disable the builtin tests
    ///
    /// See also `--add-tests`
    #[clap(long)]
    disable_builtin_tests: bool,

    /// disable the builtin rules
    ///
    /// See also `--add-rules`
    #[clap(long)]
    disable_builtin_rules: bool,

    /// deprecated, does nothing (we always procgen now)
    #[clap(long, hide = true)]
    procgen_tests: bool,
}

pub fn make_app() -> Config {
    let Cli {
        tests,
        conventions,
        reprs,
        toolchains,
        pairs,
        gen_vals,
        write_vals,
        minimize_vals,
        output_format,
        add_rustc_codegen_backend,
        add_tests,
        rules,
        disable_builtin_tests,
        disable_builtin_rules,
        // unimplemented
        select_vals: _,
        key: _,
        // deprecated
        procgen_tests: _,
    } = Cli::parse();

    let run_tests = tests;
    let run_toolchains = toolchains;
    let run_conventions = conventions;
    let run_reprs = reprs;
    let run_values = gen_vals;
    let run_writers = write_vals;
    let run_selections = vec![FunctionSelector::All];
    let minimizing_write_impl = minimize_vals;

    let mut run_pairs: Vec<_> = pairs
        .iter()
        .map(|pair| {
            pair.split_once("_calls_")
                .expect("invalid 'pair' syntax, must be 'impl_calls_impl'")
        })
        .map(|(a, b)| (String::from(a), String::from(b)))
        .collect();

    // If no pairs specified, add default ones
    if run_pairs.is_empty() {
        let mut pairs = std::collections::BTreeSet::new();
        for toolchain in &run_toolchains {
            // have it call itself
            pairs.insert((toolchain.clone(), toolchain.clone()));
            // have it call/be-called-by all default pairers
            for pairer in DEFAULT_PAIRERS {
                pairs.insert((toolchain.clone(), pairer.to_string()));
                pairs.insert((pairer.to_string(), toolchain.clone()));
            }
        }
        run_pairs = pairs.into_iter().collect();
    }

    let rustc_codegen_backends: Vec<(String, String)> = add_rustc_codegen_backend
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

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("failed to initialize logger");

    let logger = crate::log::MapLogger::new();
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(logger.clone())
        .init();

    let target_dir: Utf8PathBuf = "target".into();
    let out_dir = target_dir.join("temp");
    let generated_src_dir = target_dir.join("generated_impls");
    let runtime_test_input_dir = add_tests;
    let runtime_rules_file = if let Some(rules) = rules {
        // If they specify rules, require them to exist
        if !rules.exists() {
            panic!("could not find --rules {rules}");
        }
        Some(rules)
    } else {
        // Otherwise try to find the default rules
        let default_rules: Utf8PathBuf = "abi-cafe-rules.toml".into();
        if default_rules.exists() {
            Some(default_rules)
        } else {
            None
        }
    };

    let paths = Paths {
        target_dir,
        out_dir,
        generated_src_dir,
        runtime_test_input_dir,
        runtime_rules_file,
    };
    Config {
        output_format,
        run_conventions,
        run_reprs,
        run_toolchains,
        run_tests,
        run_pairs,
        rustc_codegen_backends,
        run_values,
        run_writers,
        run_selections,
        minimizing_write_impl,
        disable_builtin_tests,
        disable_builtin_rules,
        paths,
    }
}
