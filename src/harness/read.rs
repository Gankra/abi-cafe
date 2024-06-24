use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    error::*,
    vals::{ValueGeneratorKind, ValueTree},
    Test,
};

pub fn read_tests(value_generator: ValueGeneratorKind) -> Result<Vec<Test>, GenerateError> {
    let mut tests = vec![];
    let mut dirs = vec![PathBuf::from("tests")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;

            // If it's a dir, add it to the working set
            if entry.file_type()?.is_dir() {
                dirs.push(entry.path());
                continue;
            }

            // Otherwise, assume it's a test and parse it
            let test = match read_test_manifest(entry.path().to_owned(), value_generator) {
                Ok(test) => test,
                Err(e) => {
                    eprintln!("test {:?}'s file couldn't be parsed {}", entry, e);
                    continue;
                }
            };
            tests.push(test);
        }
    }
    tests.sort_by(|t1, t2| t1.name.cmp(&t2.name));
    // FIXME: assert test names don't collide!

    Ok(tests)
}

/// Read a test .kdl file
fn read_test_manifest(
    test_file: PathBuf,
    value_generator: ValueGeneratorKind,
) -> Result<Test, GenerateError> {
    let (input, test_name) = if let Some(test_name) = filename(&test_file).strip_suffix(".procgen.kdl") {
        let ty_def = read_file_to_string(&test_file)?;
        let input = crate::procgen::procgen_test_for_ty_string(test_name, Some(&ty_def));
        (input, test_name)
    } else if let Some(test_name) = filename(&test_file).strip_suffix(".kdl") {
        let input = read_file_to_string(&test_file)?;
        (input, test_name)
    } else {
        return Err(GenerateError::Skipped);
    };
    let mut compiler = kdl_script::Compiler::new();
    let types = compiler.compile_string(&test_file.to_string_lossy(), input)?;
    let vals = Arc::new(ValueTree::new(&types, value_generator));
    Ok(Test {
        name: test_name.to_owned(),
        types,
        vals,
    })
}

fn filename(file: &Path) -> &str {
    file.file_name().and_then(|s| s.to_str()).unwrap_or("")
}

fn read_file_to_string(file: &Path) -> std::io::Result<String> {
    let file = File::open(file)?;
    let mut reader = BufReader::new(file);
    let mut input = String::new();
    reader.read_to_string(&mut input)?;
    Ok(input)
}