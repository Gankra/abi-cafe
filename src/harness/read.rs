mod procgen;

use std::{
    fs::File,
    io::{BufReader, Read},
    sync::Arc,
};

use camino::{Utf8Path, Utf8PathBuf};
use tracing::warn;

use crate::error::*;
use crate::harness::test::*;
use crate::*;

#[derive(Debug, Clone)]
pub enum TestFile {
    Kdl(Pathish),
    KdlProcgen(Pathish),
}

#[derive(Debug, Clone)]
pub enum Pathish {
    Runtime(Utf8PathBuf),
    Static(Utf8PathBuf),
}
impl Pathish {
    fn as_str(&self) -> &str {
        match self {
            Pathish::Runtime(path) | Pathish::Static(path) => path.as_str(),
        }
    }
}

pub fn find_tests(cfg: &Config) -> Result<SortedMap<TestId, TestFile>, GenerateError> {
    let mut tests = find_tests_runtime(cfg.paths.runtime_test_input_dir.as_deref())?;
    let mut more_tests = find_tests_static(cfg.disable_builtin_tests)?;
    tests.append(&mut more_tests);
    Ok(tests)
}

pub fn find_tests_runtime(
    start_dir: Option<&Utf8Path>,
) -> Result<SortedMap<TestId, TestFile>, GenerateError> {
    let mut tests = SortedMap::new();
    let Some(start_dir) = start_dir else {
        return Ok(tests);
    };
    let mut dirs = vec![start_dir.as_std_path().to_owned()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;

            // If it's a dir, add it to the working set
            if entry.file_type()?.is_dir() {
                dirs.push(entry.path());
                continue;
            }

            let path = entry.path();
            let test_file = Utf8PathBuf::from_path_buf(path).expect("non-utf8 test path");
            let Some((name, test)) = classify_test(&test_file, true) else {
                warn!("test isn't a known test format: {}", test_file);
                continue;
            };
            tests.insert(name, test);
        }
    }
    Ok(tests)
}

pub fn find_tests_static(
    disable_builtin_tests: bool,
) -> Result<SortedMap<TestId, TestFile>, GenerateError> {
    let mut tests = SortedMap::new();
    if disable_builtin_tests {
        return Ok(tests);
    }

    let mut dirs = vec![crate::files::tests()];
    while let Some(dir) = dirs.pop() {
        for entry in dir.entries() {
            // If it's a dir, add it to the working set
            if let Some(dir) = entry.as_dir() {
                dirs.push(dir);
                continue;
            }

            if let Some(file) = entry.as_file() {
                let path = file.path();
                let test_file =
                    Utf8PathBuf::from_path_buf(path.to_owned()).expect("non-utf8 test path");
                let Some((name, test)) = classify_test(&test_file, false) else {
                    warn!("test isn't a known test format: {}", test_file);
                    continue;
                };
                tests.insert(name, test);
            }
        }
    }
    Ok(tests)
}

pub fn spawn_read_test(
    rt: &tokio::runtime::Runtime,
    test: TestId,
    test_file: TestFile,
) -> tokio::task::JoinHandle<Result<Arc<Test>, GenerateError>> {
    rt.spawn(async move { read_test(test, test_file).await })
}

/// Read a test .kdl file
async fn read_test(test: TestId, test_file: TestFile) -> Result<Arc<Test>, GenerateError> {
    read_test_inner(&test, test_file)
        .await
        .map_err(|e| GenerateError::ReadTest {
            test,
            details: Box::new(e),
        })
}

async fn read_test_inner(test: &TestId, test_file: TestFile) -> Result<Arc<Test>, GenerateError> {
    let (test_file, input) = match test_file {
        TestFile::KdlProcgen(test_file) => {
            let ty_def = read_file_to_string(&test_file)?;
            let input = procgen::procgen_test_for_ty_string(test, Some(&ty_def));
            (test_file, input)
        }
        TestFile::Kdl(test_file) => {
            let input = read_file_to_string(&test_file)?;
            (test_file, input)
        }
    };
    let mut compiler = kdl_script::Compiler::new();
    let types = compiler.compile_string(test_file.as_str(), input)?;
    Ok(Arc::new(Test {
        name: test.to_owned(),
        types,
    }))
}

fn read_file_to_string(pathish: &Pathish) -> std::io::Result<String> {
    match pathish {
        Pathish::Runtime(path) => read_runtime_file_to_string(path),
        Pathish::Static(path) => Ok(crate::files::get_file(path)),
    }
}

#[allow(clippy::manual_map)]
fn classify_test(test_file: &Utf8Path, is_runtime: bool) -> Option<(String, TestFile)> {
    let file_name = test_file.file_name().expect("test file had no name!?");
    let pathish = if is_runtime {
        Pathish::Runtime(test_file.to_owned())
    } else {
        Pathish::Static(test_file.to_owned())
    };
    if let Some(test_name) = file_name.strip_suffix(".procgen.kdl") {
        Some((test_name.to_owned(), TestFile::KdlProcgen(pathish)))
    } else if let Some(test_name) = file_name.strip_suffix(".kdl") {
        Some((test_name.to_owned(), TestFile::Kdl(pathish)))
    } else {
        None
    }
}

fn read_runtime_file_to_string(file: &Utf8Path) -> std::io::Result<String> {
    let file = File::open(file)?;
    let mut reader = BufReader::new(file);
    let mut input = String::new();
    reader.read_to_string(&mut input)?;
    Ok(input)
}
