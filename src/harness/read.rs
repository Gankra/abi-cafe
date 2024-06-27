use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
    sync::Arc,
};

use camino::{Utf8Path, Utf8PathBuf};
use tracing::warn;

use crate::{error::*, SortedMap, Test, TestId};

#[derive(Debug, Clone)]
pub enum TestFile {
    Kdl(Utf8PathBuf),
    KdlProcgen(Utf8PathBuf),
}

pub fn find_tests(start_dir: &Path) -> Result<SortedMap<TestId, TestFile>, GenerateError> {
    let mut tests = SortedMap::new();
    let mut dirs = vec![start_dir.to_owned()];
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
            let Some((name, test)) = classify_test(&test_file) else {
                warn!("test isn't a known test format: {}", test_file);
                continue;
            };
            tests.insert(name, test);
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
    read_test_inner(&test, test_file).await.map_err(|e| {
        warn!(
            "failed to read and parse test {test}, skipping\n{:?}",
            miette::Report::new(e)
        );
        GenerateError::Skipped
    })
}

async fn read_test_inner(test: &TestId, test_file: TestFile) -> Result<Arc<Test>, GenerateError> {
    let (test_file, input) = match test_file {
        TestFile::KdlProcgen(test_file) => {
            let ty_def = read_file_to_string(&test_file)?;
            let input = crate::procgen::procgen_test_for_ty_string(&test, Some(&ty_def));
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

fn classify_test(test_file: &Utf8Path) -> Option<(String, TestFile)> {
    let file_name = test_file.file_name().expect("test file had no name!?");
    if let Some(test_name) = file_name.strip_suffix(".procgen.kdl") {
        Some((
            test_name.to_owned(),
            TestFile::KdlProcgen(test_file.to_owned()),
        ))
    } else if let Some(test_name) = file_name.strip_suffix(".kdl") {
        Some((test_name.to_owned(), TestFile::Kdl(test_file.to_owned())))
    } else {
        None
    }
}

fn read_file_to_string(file: &Utf8Path) -> std::io::Result<String> {
    let file = File::open(file)?;
    let mut reader = BufReader::new(file);
    let mut input = String::new();
    reader.read_to_string(&mut input)?;
    Ok(input)
}
