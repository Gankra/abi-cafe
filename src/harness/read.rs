use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use crate::{error::*, Test};

pub fn read_tests() -> Result<Vec<Test>, GenerateError> {
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
            let test = match read_test_manifest(&entry.path()) {
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
pub fn read_test_manifest(test_file: &Path) -> Result<Test, GenerateError> {
    let file = File::open(&test_file)?;
    let mut reader = BufReader::new(file);
    let mut input = String::new();
    reader.read_to_string(&mut input)?;

    let ext = test_file.extension().and_then(|s| s.to_str()).unwrap_or("");

    if ext == "kdl" {
        let mut compiler = kdl_script::Compiler::new();
        let types = compiler.compile_string(&test_file.to_string_lossy(), input)?;
        Ok(Test {
            name: test_file
                .file_stem()
                .expect("test had no filename")
                .to_str()
                .expect("test filename wasn't utf8")
                .to_owned(),
            types,
        })
    } else {
        Err(GenerateError::Skipped)
    }
}
