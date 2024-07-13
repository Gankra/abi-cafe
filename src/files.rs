use std::io::Write;

use camino::{Utf8Path, Utf8PathBuf};
use include_dir::{include_dir, Dir, File};

use crate::{built_info, GenerateError};

const INCLUDES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/include");

#[derive(Debug, Clone)]
pub struct Paths {
    pub target_dir: Utf8PathBuf,
    pub out_dir: Utf8PathBuf,
    pub generated_src_dir: Utf8PathBuf,
    pub runtime_test_input_dir: Option<Utf8PathBuf>,
}
impl Paths {
    pub fn harness_dylib_main_file(&self) -> Utf8PathBuf {
        self.out_dir.join("harness_lib.rs")
    }
    pub fn harness_bin_main_file(&self) -> Utf8PathBuf {
        self.out_dir.join("harness_main.rs")
    }
    pub fn freestanding_bin_main_file(&self) -> Utf8PathBuf {
        self.out_dir.join("main.rs")
    }

    /// Delete and recreate the build dir
    pub fn init_dirs(&self) -> Result<(), GenerateError> {
        // Make sure these dirs exist and are empty
        clear_and_create_dir(&self.out_dir);
        clear_and_create_dir(&self.generated_src_dir);

        // Initialize harness.rs
        {
            let harness_file_contents = get_file("harness/harness_lib.rs");
            let harness_file_path = self.harness_dylib_main_file();
            let mut file = std::fs::File::create_new(harness_file_path)
                .expect("failed to create harness_lib.rs");
            file.write_all(harness_file_contents.as_bytes())
                .expect("failed to initialize harness_lib.rs");
        }
        {
            let harness_file_contents = get_file("harness/harness_main.rs");
            let harness_file_path = self.harness_bin_main_file();
            let mut file = std::fs::File::create_new(harness_file_path)
                .expect("failed to create harness_main.rs");
            file.write_all(harness_file_contents.as_bytes())
                .expect("failed to initialize harness_main.rs");
        }
        {
            let harness_file_contents = get_file("harness/main.rs");
            let harness_file_path = self.freestanding_bin_main_file();
            let mut file =
                std::fs::File::create_new(harness_file_path).expect("failed to create main.rs");
            file.write_all(harness_file_contents.as_bytes())
                .expect("failed to initialize main.rs");
        }

        // Set up env vars for CC
        std::env::set_var("OUT_DIR", &self.out_dir);
        std::env::set_var("HOST", built_info::HOST);
        std::env::set_var("TARGET", built_info::TARGET);
        std::env::set_var("OPT_LEVEL", "0");

        Ok(())
    }
}

pub fn clear_and_create_dir(path: impl AsRef<Utf8Path>) {
    let path = path.as_ref();
    std::fs::create_dir_all(path).expect("failed to clear and create build dir");
    std::fs::remove_dir_all(path).expect("failed to clear and create build dir");
    std::fs::create_dir_all(path).expect("failed to clear and create build dir");
}

pub fn get_file(path: impl AsRef<Utf8Path>) -> String {
    let path = path.as_ref();
    let Some(file) = INCLUDES.get_file(path) else {
        unreachable!("embedded file didn't exist: {path}");
    };
    load_file(file)
}

pub fn load_file(file: &File) -> String {
    let Some(string) = file.contents_utf8() else {
        unreachable!("embedded file wasn't utf8: {}", file.path().display());
    };
    clean_newlines(string)
}

pub fn tests() -> &'static Dir<'static> {
    INCLUDES
        .get_dir("tests")
        .expect("includes didn't contain ./test")
}

fn clean_newlines(input: &str) -> String {
    input.replace('\r', "")
}
