//! Helpers shared by the tests of the UniDeX crates.

use std::path::Path;

use udx_core::paths::Env;

/// A fresh folder inside the process's test root, removed on drop.
///
/// As an [`Env`] it sets `UNIDEX_TEST_ROOT` to that folder and nothing else.
pub struct TestEnv {
    dir: tempfile::TempDir,
}

impl TestEnv {
    /// Panics if `UNIDEX_TEST_ROOT` is missing: run the tests through cargo.
    pub fn new() -> TestEnv {
        let root = std::env::var("UNIDEX_TEST_ROOT")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                panic!("UNIDEX_TEST_ROOT is not set: run the tests through cargo (cargo test)")
            });
        std::fs::create_dir_all(&root).expect("create the test root");
        let dir = tempfile::Builder::new()
            .prefix("udx-")
            .tempdir_in(&root)
            .expect("create a folder inside the test root");
        TestEnv { dir }
    }

    /// The fresh folder. It exists on disk.
    pub fn root(&self) -> &Path {
        self.dir.path()
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        TestEnv::new()
    }
}

impl Env for TestEnv {
    fn var(&self, key: &str) -> Option<String> {
        (key == "UNIDEX_TEST_ROOT").then(|| self.dir.path().to_string_lossy().into_owned())
    }
}
