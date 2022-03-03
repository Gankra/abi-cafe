use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;

static TESTS: &[&str] = &[
    "u128",
];

static GEN_TESTS: &[&str] = &[
    "u64",
];

static RUST_TEST_PREFIX: &str = include_str!("../harness/rust_test_prefix.rs");
static C_TEST_PREFIX: &str = include_str!("../harness/c_test_prefix.h");

type WriteValCallback = unsafe extern fn(&mut WriteBuffer, *const u8, u32) -> ();
type TestInit = unsafe extern fn(WriteValCallback, &mut WriteBuffer,  &mut WriteBuffer,  &mut WriteBuffer,  &mut WriteBuffer) -> ();

#[derive(Default)]
struct WriteBuffer {
    offsets: Vec<usize>,
    data: Vec<u8>,
}

unsafe extern fn write_val(output: &mut WriteBuffer, input: *const u8, size: u32) {
    let data = std::slice::from_raw_parts(input, size as usize);
    output.offsets.push(output.data.len());
    output.data.extend(data);
}

struct Obj();
pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn main() -> Result<(), Box<dyn Error>> {
    let manual_impls = PathBuf::from("impls/");
    let generated_impls = PathBuf::from("target/temp/generated/");
    let out_dir = PathBuf::from("target/temp/");

    env::set_var("OUT_DIR", &out_dir);
    env::set_var("HOST", built_info::HOST);
    env::set_var("TARGET", built_info::TARGET);
    env::set_var("OPT_LEVEL", "3");
    for test_name in TESTS {
        println!("parsing test {test_name}\n");
        let test_file = format!("tests/{test_name}.json");
        let file = File::open(test_file)?;
        let reader = BufReader::new(file);
        let test: TestDecl = serde_json::from_reader(reader)?;

        println!("building test {test_name}\n");
        
        build_cc_callee(&manual_impls, test_name);
        build_rust_caller(&manual_impls, test_name);
        build_harness(&manual_impls, test_name);

        println!("\nrunning test {test_name}\n");
        run_dynamic_test(&out_dir, test_name, &test)?;
    }

    for test_name in GEN_TESTS {
        let test_file = format!("tests/{test_name}.json");
        let file = File::open(test_file)?;
        let reader = BufReader::new(file);
        let test: TestDecl = serde_json::from_reader(reader)?;

        let rust_src = PathBuf::from(format!("target/temp/generated/rust/{test_name}_caller.rs")); 
        let c_src = PathBuf::from(format!("target/temp/generated/c/{test_name}_callee.c"));

        {
            println!("generating test {test_name}\n");
            std::fs::create_dir_all(rust_src.parent().unwrap())?;
            std::fs::create_dir_all(c_src.parent().unwrap())?;
            let mut rust_output = File::create(rust_src)?;
            generate_rust_caller(&mut rust_output, &test)?;

            let mut c_output = File::create(c_src)?;
            generate_c_callee(&mut c_output, &test)?;
        }

        println!("building test {test_name}\n");
        
        build_cc_callee(&generated_impls, test_name);
        build_rust_caller(&generated_impls, test_name);
        build_harness(&generated_impls, test_name);

        println!("\nrunning test {test_name}\n");
        run_dynamic_test(&out_dir, test_name, &test)?;
    }

    Ok(())
}

fn run_test(test: &str) {
    let filename = format!("{test}_caller.exe");
    let mut src = PathBuf::new();
    src.push("target");
    src.push("temp");
    src.push(filename);

    Command::new(src)
        .status()
        .unwrap();
}

fn run_dynamic_test(base_path: &Path, test_name: &str, test: &TestDecl) -> Result<(), Box<dyn Error>> {
    let filename = format!("{test_name}_harness.dll");
    let mut dylib = PathBuf::from(base_path);
    dylib.push(filename);

    unsafe {
        let mut caller_inputs = WriteBuffer::default();
        let mut caller_outputs = WriteBuffer::default();
        let mut callee_inputs = WriteBuffer::default();
        let mut callee_outputs = WriteBuffer::default();

        let lib = libloading::Library::new(dylib)?;
        let func: libloading::Symbol<TestInit> = lib.get(b"test_start")?;
        func(write_val, &mut caller_inputs, &mut caller_outputs, &mut callee_inputs, &mut callee_outputs);

        let expected_inputs: usize = test.functions.iter().map(|f| f.inputs.len()).sum();
        let expected_outputs: usize = test.functions.iter().map(|f| if f.output.is_some() { 1 } else { 0 }).sum();
        assert_eq!(callee_inputs.offsets.len(), expected_inputs);
        assert_eq!(caller_inputs.offsets.len(), expected_inputs);
        assert_eq!(callee_outputs.offsets.len(), expected_outputs);
        assert_eq!(caller_outputs.offsets.len(), expected_outputs);

        let mut input_count = 0;
        let mut output_count = 0;
        for func in &test.functions {
            print!("test {}::{}... ", test.name, func.name);
            for (input_idx, input) in func.inputs.iter().enumerate() {
                let caller_start = caller_inputs.offsets[input_count];
                let caller_end = caller_inputs.offsets.get(input_count + 1).copied().unwrap_or(caller_inputs.data.len());
                let callee_start = callee_inputs.offsets[input_count];
                let callee_end = callee_inputs.offsets.get(input_count + 1).copied().unwrap_or(callee_inputs.data.len());

                assert_eq!(&caller_inputs.data[caller_start..caller_end],
                    &callee_inputs.data[callee_start..callee_end],
                    "{}::{} input {} ({}) didn't match", 
                    test.name, func.name, input_idx, input.ctype);
                input_count += 1;
            }
            if let Some(_output) = &func.output {
                let caller_start = caller_outputs.offsets[output_count];
                let caller_end = caller_outputs.offsets.get(output_count + 1).copied().unwrap_or(caller_outputs.data.len());
                let callee_start = callee_outputs.offsets[output_count];
                let callee_end = callee_outputs.offsets.get(output_count + 1).copied().unwrap_or(callee_outputs.data.len());

                assert_eq!(&caller_outputs.data[caller_start..caller_end],
                    &callee_outputs.data[callee_start..callee_end],
                    "{}::{} outputs didn't match", 
                    test.name, func.name);
                output_count += 1;
            }
            println!("passed!");
        }
    }

    Ok(())
}

fn build_cc_callee(base_path: &Path, test: &str) -> Obj {
    let filename = format!("{test}_callee.c");
    let output_lib = format!("{test}_callee");
    let mut src = PathBuf::from(base_path);
    src.push("c");
    src.push(filename);

    cc::Build::new()
        .file(src)
        .compile(&output_lib);
    Obj()
}
fn build_cc_caller(base_path: &Path, test: &str) -> Obj {
    todo!()
}

fn build_rust_callee(base_path: &Path, test: &str) -> Obj {
    todo!()
}
fn build_rust_caller(base_path: &Path, test: &str) -> Obj {
    let filename = format!("{test}_caller.rs");
    let mut src = PathBuf::from(base_path);
    src.push("rust");
    src.push(filename);

    Command::new("rustc")
        .arg("--crate-type")
        .arg("staticlib")
        .arg("--out-dir")
        .arg("target/temp/")
        .arg(src)
        .status().unwrap();

    Obj()
}

fn build_harness(base_path: &Path, test: &str) -> Obj {
    let src = PathBuf::from("harness/harness.rs");
    let callee = format!("target/temp/{test}_callee");
    let caller = format!("target/temp/{test}_caller");
    let output = format!("target/temp/{test}_harness.dll");

    Command::new("rustc")
        .arg("-l")
        .arg(callee)
        .arg("-l")
        .arg(caller)
        .arg("--crate-type")
        .arg("dylib")
        .arg("--out-dir")
        .arg("target/temp/")
        .arg("-o")
        .arg(output)
        .arg(src)
        .status().unwrap();

    Obj()    
}

fn build_dyn_rust_caller(base_path: &Path, test: &str) -> Obj {
    let filename = format!("{test}_caller.rs");
    let mut src = PathBuf::from(base_path);
    src.push("rust");
    src.push(filename);

    Command::new("rustc")
        .arg("-l")
        .arg("target/temp/callee")
        .arg("--crate-type")
        .arg("dylib")
        .arg("--out-dir")
        .arg("target/temp/")
        .arg(src)
        .status().unwrap();

    Obj()
}


#[derive(serde::Deserialize)]
struct TestDecl {
    name: String,
    functions: Vec<FunctionDecl>,
}

#[derive(serde::Deserialize)]
struct FunctionDecl {
    name: String,
    convention: String,
    inputs: Vec<ValDecl>,
    output: Option<ValDecl>,
}

#[derive(serde::Deserialize)]
struct ValDecl {
    ctype: String,
    val: String,
}


fn generate_rust_caller<W: Write>(f: &mut W, test: &TestDecl) -> std::io::Result<()> {
    write!(f, "{}", RUST_TEST_PREFIX)?;
    // Generate the extern block
    writeln!(f, "extern {{")?;
    for function in &test.functions {
        writeln!(f, "  #[no_mangle]")?;
        write!(f, "  fn {}(", function.name)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            let ty = rust_type(&input.ctype);
            write!(f, "arg{idx}: {ty}, ",)?;
        }
        write!(f, ")")?;
        if let Some(output) = &function.output {
            let ty = rust_type(&output.ctype);
            write!(f, " -> {ty}")?;
        }
        writeln!(f, ";")?;
    }
    writeln!(f, "}}")?;

    // Now generate the body
    writeln!(f, "#[no_mangle] pub extern fn do_test() {{")?;

    for function in &test.functions {
        writeln!(f, "   unsafe {{")?;
        writeln!(f, r#"        println!("test {}::{}\n");"#, test.name, function.name)?;
        writeln!(f, r#"        println!("\n{}::{} rust caller inputs: ");"#, test.name, function.name)?;
        writeln!(f)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            let ty = rust_type(&input.ctype);
            writeln!(f, "        let arg{idx}: {ty} = {};", input.val)?;
        }
        writeln!(f)?;
        for (idx, _input) in function.inputs.iter().enumerate() {
            writeln!(f, r#"        println!("{{}}", arg{idx});"#)?;
            writeln!(f, "WRITE.unwrap()(CALLER_INPUTS, &arg{idx} as *const _ as *const _, core::mem::size_of_val(&arg{idx}) as u32);")?;
        }
        writeln!(f)?;
        write!(f, "        ")?;
        if let Some(output) = &function.output {
            let ty = rust_type(&output.ctype);
            write!(f, "let output: {ty} = ")?;
        }
        write!(f, "{}(", function.name)?;
        for (idx, _input) in function.inputs.iter().enumerate() {
            write!(f, "arg{idx}, ")?;
        }
        writeln!(f, ");")?;
        writeln!(f)?;
        if let Some(_output) = &function.output {
            writeln!(f, r#"        println!("\n{}::{} rust caller outputs: ");"#, test.name, function.name)?;
            writeln!(f, r#"        println!("{{}}", output);"#)?;
            writeln!(f, "WRITE.unwrap()(CALLER_OUTPUTS, &output as *const _ as *const _, core::mem::size_of_val(&output) as u32);")?;
        }
        writeln!(f, "   }}")?;
    }

    writeln!(f, "}}")?;

    Ok(())
}

fn generate_c_callee<W: Write>(f: &mut W, test: &TestDecl) -> std::io::Result<()> {
    write!(f, "{}", C_TEST_PREFIX)?;

    // Generate the impls
    for function in &test.functions {
        if let Some(output) = &function.output {
            write!(f, "{} ", output.ctype)?;
        } else {
            write!(f, "void ")?;
        }
        write!(f,"{}(", function.name)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            if idx != 0 {
                write!(f, ", ")?;   
            }
            write!(f, "{} arg{idx}", input.ctype)?;
        }
        writeln!(f, ") {{")?;

        writeln!(f, r#"    printf("\n{}::{} C callee inputs: \n");"#, test.name, function.name)?;
        writeln!(f)?;
        for (idx, input) in function.inputs.iter().enumerate() {
            let formatter = cfmt(&input.ctype);
            writeln!(f, r#"    printf("%" {formatter} "\n", arg{idx});"#)?;
            writeln!(f, r#"    WRITE(CALLEE_INPUTS, (char*)&arg{idx}, sizeof(arg{idx}));"#)?;
        }
        writeln!(f)?;
        if let Some(output) = &function.output {
            let formatter = cfmt(&output.ctype);
            writeln!(f, "    {} output = {};", output.ctype, output.val)?;
            writeln!(f, r#"    printf("\n{}::{} C callee outputs: \n");"#, test.name, function.name)?;
            writeln!(f, r#"    printf("%" {formatter} "\n", output);"#)?;
            writeln!(f, r#"    WRITE(CALLEE_OUTPUTS, (char*)&output, sizeof(output));"#)?;
            writeln!(f, "    return output;")?;
        }
        writeln!(f, "}}")?;
        writeln!(f)?;
    }

    Ok(())
}

fn rust_type(ctype: &str) -> &'static str {
    match ctype {
        "bool" => "bool",
        "void*" => "*mut ()",

        "uint8_t" => "u8",
        "uint16_t" => "u16",
        "uint32_t" => "u32",
        "uint64_t" => "u64",
        "uint128_t" => "u128",

        "int8_t" => "i8",
        "int16_t" => "i16",
        "int32_t" => "i32",
        "int64_t" => "i64",
        "int128_t" => "i128",

        _ => unimplemented!(),
    }
}

fn cfmt(ctype: &str) -> &'static str {
    match ctype {
        "bool" => r#""d""#,
        "void*" => r#""p""#,

        "uint8_t" => "PRIu8",
        "uint16_t" => "PRIu16",
        "uint32_t" => "PRIu32",
        "uint64_t" => "PRIu64",
        "uint128_t" => "PRIu128",

        "int8_t" => "PRId8",
        "int16_t" => "PRId16",
        "int32_t" => "PRId32",
        "int64_t" => "PRId64",
        "int128_t" => "PRId128",

        _ => unimplemented!(), 
    }
}