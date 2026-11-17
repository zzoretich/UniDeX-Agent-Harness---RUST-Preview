use std::collections::HashMap;
use std::path::{Path, PathBuf};

use udx_core::paths::{AppPaths, Env, PathError, ProcessEnv};

struct MapEnv(HashMap<String, String>);

impl MapEnv {
    fn of<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a Path)>) -> MapEnv {
        MapEnv(
            pairs
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string_lossy().into_owned()))
                .collect(),
        )
    }
}

impl Env for MapEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn real(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap()
}

#[test]
fn defaults_live_inside_the_test_root() {
    let root = tmp();
    let p = AppPaths::resolve(&MapEnv::of([("UNIDEX_TEST_ROOT", root.path())])).unwrap();
    assert_eq!(p.config_dir, real(root.path()).join("config"));
    assert_eq!(p.data_dir, real(root.path()).join("data"));
    assert_eq!(p.cache_dir, real(root.path()).join("cache"));
}

#[test]
fn an_override_inside_the_root_is_accepted() {
    let root = tmp();
    let inside = root.path().join("elsewhere");
    let p = AppPaths::resolve(&MapEnv::of([
        ("UNIDEX_TEST_ROOT", root.path()),
        ("UNIDEX_DATA_DIR", inside.as_path()),
    ]))
    .unwrap();
    assert_eq!(p.data_dir, real(root.path()).join("elsewhere"));
}

#[test]
fn an_override_outside_the_root_is_refused() {
    let (root, other) = (tmp(), tmp());
    let e = AppPaths::resolve(&MapEnv::of([
        ("UNIDEX_TEST_ROOT", root.path()),
        ("UNIDEX_DATA_DIR", other.path()),
    ]));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })));
}

#[cfg(unix)]
#[test]
fn a_symlink_that_escapes_the_root_is_refused() {
    let (root, other) = (tmp(), tmp());
    std::os::unix::fs::symlink(other.path(), root.path().join("link")).unwrap();
    let via = root.path().join("link").join("data");
    let e = AppPaths::resolve(&MapEnv::of([
        ("UNIDEX_TEST_ROOT", root.path()),
        ("UNIDEX_DATA_DIR", via.as_path()),
    ]));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })));
}

#[cfg(unix)]
#[test]
fn a_root_reached_through_a_symlink_is_accepted() {
    let (holder, target) = (tmp(), tmp());
    let alias = holder.path().join("alias");
    std::os::unix::fs::symlink(target.path(), &alias).unwrap();
    assert!(AppPaths::resolve(&MapEnv::of([("UNIDEX_TEST_ROOT", alias.as_path())])).is_ok());
}

#[test]
fn without_a_test_root_the_os_folders_are_used() {
    let p = AppPaths::resolve(&MapEnv::of([] as [(&str, &Path); 0])).unwrap();
    assert!(p.config_dir.to_string_lossy().contains("unidex-preview"));
}

#[test]
fn cargo_gives_every_test_process_a_test_root() {
    assert!(std::env::var("UNIDEX_TEST_ROOT").is_ok());
    assert!(AppPaths::resolve(&ProcessEnv).is_ok());
}

#[test]
fn test_env_folders_sit_inside_the_process_root() {
    let env = udx_test_support::TestEnv::new();
    let p = AppPaths::resolve(&env).unwrap();
    assert!(p.data_dir.starts_with(real(env.root())));
}

#[test]
fn all_three_overrides_are_honoured() {
    let root = tmp();
    let (c, d, k) = (
        root.path().join("c-over"),
        root.path().join("d-over"),
        root.path().join("k-over"),
    );
    let p = AppPaths::resolve(&MapEnv::of([
        ("UNIDEX_TEST_ROOT", root.path()),
        ("UNIDEX_CONFIG_DIR", c.as_path()),
        ("UNIDEX_DATA_DIR", d.as_path()),
        ("UNIDEX_CACHE_DIR", k.as_path()),
    ]))
    .unwrap();
    assert_eq!(p.config_dir, real(root.path()).join("c-over"));
    assert_eq!(p.data_dir, real(root.path()).join("d-over"));
    assert_eq!(p.cache_dir, real(root.path()).join("k-over"));
}

#[test]
fn resolve_creates_nothing() {
    let root = tmp();
    AppPaths::resolve(&MapEnv::of([("UNIDEX_TEST_ROOT", root.path())])).unwrap();
    assert!(!root.path().join("config").exists());
    assert!(!root.path().join("data").exists());
    assert!(!root.path().join("cache").exists());
}

#[test]
fn a_parent_dir_escape_is_refused() {
    let (root, other) = (tmp(), tmp());
    let name = other.path().file_name().unwrap();
    let via = root.path().join("..").join(name).join("data");
    let e = AppPaths::resolve(&MapEnv::of([
        ("UNIDEX_TEST_ROOT", root.path()),
        ("UNIDEX_DATA_DIR", via.as_path()),
    ]));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })));
}
