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
fn a_test_build_without_a_test_root_never_returns_os_folders() {
    // This crate's tests run with the `testing` feature, so the operating
    // system's folders are never an answer. The non-testing path (the OS
    // folders) is covered by the unit tests in `src/paths.rs`.
    let e = AppPaths::resolve(&MapEnv::of([] as [(&str, &Path); 0]));
    assert!(
        matches!(e, Err(PathError::TestBuildWithoutTestRoot)),
        "{e:?}"
    );
}

#[test]
fn cargo_gives_every_test_process_a_test_root() {
    let root = std::env::var("UNIDEX_TEST_ROOT").unwrap();
    assert!(!root.is_empty());
    std::fs::create_dir_all(&root).unwrap();
    let real_root = real(Path::new(&root));
    let p = AppPaths::resolve(&ProcessEnv).unwrap();
    assert!(p.config_dir.starts_with(&real_root));
    assert!(p.data_dir.starts_with(&real_root));
    assert!(p.cache_dir.starts_with(&real_root));
}

#[test]
fn test_env_folders_sit_inside_the_process_root() {
    let env = udx_core::paths::testing::TestEnv::new();
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

#[test]
fn a_parent_dir_escape_through_a_missing_folder_is_refused() {
    let (root, other) = (tmp(), tmp());
    let name = other.path().file_name().unwrap();
    let via = root
        .path()
        .join("missing")
        .join("..")
        .join("..")
        .join(name)
        .join("data");
    let e = AppPaths::resolve(&MapEnv::of([
        ("UNIDEX_TEST_ROOT", root.path()),
        ("UNIDEX_DATA_DIR", via.as_path()),
    ]));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })));
}

#[test]
fn an_empty_variable_counts_as_unset() {
    let root = tmp();
    let mut env = MapEnv::of([("UNIDEX_TEST_ROOT", root.path())]);
    env.0.insert("UNIDEX_DATA_DIR".to_string(), String::new());
    let p = AppPaths::resolve(&env).unwrap();
    assert_eq!(p.data_dir, real(root.path()).join("data"));
}

#[test]
fn a_dot_dot_that_stays_inside_the_root_is_accepted() {
    let root = tmp();
    let via = root.path().join("a").join("..").join("b");
    let p = AppPaths::resolve(&MapEnv::of([
        ("UNIDEX_TEST_ROOT", root.path()),
        ("UNIDEX_DATA_DIR", via.as_path()),
    ]))
    .unwrap();
    assert_eq!(p.data_dir, real(root.path()).join("b"));
}

#[cfg(unix)]
#[test]
fn a_dangling_symlink_that_points_outside_is_refused() {
    let (root, other) = (tmp(), tmp());
    let target = other.path().join("not-yet");
    std::os::unix::fs::symlink(&target, root.path().join("link")).unwrap();
    let via = root.path().join("link").join("data");
    let e = AppPaths::resolve(&MapEnv::of([
        ("UNIDEX_TEST_ROOT", root.path()),
        ("UNIDEX_DATA_DIR", via.as_path()),
    ]));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })));
}

#[cfg(unix)]
fn data_via(root: &Path, via: &Path) -> Result<AppPaths, PathError> {
    AppPaths::resolve(&MapEnv::of([
        ("UNIDEX_TEST_ROOT", root),
        ("UNIDEX_DATA_DIR", via),
    ]))
}

#[cfg(unix)]
#[test]
fn a_dot_dot_then_a_symlink_that_escapes_is_refused() {
    let (root, other) = (tmp(), tmp());
    std::os::unix::fs::symlink(other.path(), root.path().join("link")).unwrap();
    let via = root.path().join("missing/../link/data");
    assert!(matches!(
        data_via(root.path(), &via),
        Err(PathError::OutsideTestRoot { .. })
    ));
}

#[cfg(unix)]
#[test]
fn a_dot_dot_then_a_symlink_itself_is_refused() {
    let (root, other) = (tmp(), tmp());
    std::os::unix::fs::symlink(other.path(), root.path().join("link")).unwrap();
    let via = root.path().join("missing/../link");
    assert!(matches!(
        data_via(root.path(), &via),
        Err(PathError::OutsideTestRoot { .. })
    ));
}

#[cfg(unix)]
#[test]
fn a_dot_dot_then_a_dangling_symlink_is_refused() {
    let (root, other) = (tmp(), tmp());
    std::os::unix::fs::symlink(other.path().join("nope"), root.path().join("dang")).unwrap();
    let via = root.path().join("missing/../dang/x");
    assert!(matches!(
        data_via(root.path(), &via),
        Err(PathError::OutsideTestRoot { .. })
    ));
}

#[cfg(unix)]
#[test]
fn a_symlink_loop_is_refused() {
    let root = tmp();
    std::os::unix::fs::symlink(root.path().join("b"), root.path().join("a")).unwrap();
    std::os::unix::fs::symlink(root.path().join("a"), root.path().join("b")).unwrap();
    let via = root.path().join("a").join("data");
    assert!(matches!(
        data_via(root.path(), &via),
        Err(PathError::OutsideTestRoot { .. })
    ));
}

#[cfg(unix)]
#[test]
fn a_root_reached_through_a_symlink_returns_resolved_folders() {
    let (holder, target) = (tmp(), tmp());
    let alias = holder.path().join("alias");
    std::os::unix::fs::symlink(target.path(), &alias).unwrap();
    let p = AppPaths::resolve(&MapEnv::of([("UNIDEX_TEST_ROOT", alias.as_path())])).unwrap();
    assert_eq!(p.data_dir, real(target.path()).join("data"));
}

#[test]
fn under_cargo_a_missing_test_root_is_an_error() {
    let env = MapEnv::of([("CARGO_MANIFEST_DIR", Path::new("/some/crate"))]);
    assert!(matches!(
        AppPaths::resolve(&env),
        Err(PathError::CargoWithoutTestRoot)
    ));
}

#[test]
fn under_cargo_an_empty_test_root_is_an_error() {
    let env = MapEnv::of([
        ("CARGO_MANIFEST_DIR", Path::new("/some/crate")),
        ("UNIDEX_TEST_ROOT", Path::new("")),
    ]);
    assert!(matches!(
        AppPaths::resolve(&env),
        Err(PathError::CargoWithoutTestRoot)
    ));
}

#[test]
fn under_cargo_overrides_do_not_lift_the_requirement() {
    let root = tmp();
    let env = MapEnv::of([
        ("CARGO_MANIFEST_DIR", Path::new("/some/crate")),
        ("UNIDEX_CONFIG_DIR", root.path()),
        ("UNIDEX_DATA_DIR", root.path()),
        ("UNIDEX_CACHE_DIR", root.path()),
    ]);
    assert!(matches!(
        AppPaths::resolve(&env),
        Err(PathError::CargoWithoutTestRoot)
    ));
}

#[cfg(unix)]
#[test]
fn an_unreadable_folder_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let root = tmp();
    let locked = root.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    // Permissions are not enforced for root, so there is nothing to test.
    let enforced = std::fs::read_dir(&locked).is_err();
    let via = locked.join("x").join("data");
    let result = data_via(root.path(), &via);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    if enforced {
        assert!(matches!(result, Err(PathError::OutsideTestRoot { .. })));
    } else {
        skipped_as_root_notice(&mut std::io::stderr(), "an_unreadable_folder_is_refused");
    }
}

/// The notice printed when a test is skipped because the process runs as root.
#[cfg(unix)]
fn skipped_as_root_notice(out: &mut dyn std::io::Write, test: &str) {
    writeln!(
        out,
        "SKIPPED {test}: running as root, so file permissions are not enforced"
    )
    .unwrap();
}

#[cfg(unix)]
#[test]
fn skipped_as_root_prints_notice() {
    let mut captured: Vec<u8> = Vec::new();
    skipped_as_root_notice(&mut captured, "an_unreadable_folder_is_refused");
    let text = String::from_utf8(captured).unwrap();
    assert!(text.contains("SKIPPED"), "{text}");
    assert!(text.contains("an_unreadable_folder_is_refused"), "{text}");
    assert!(text.contains("root"), "{text}");
    assert!(text.ends_with('\n'), "{text}");
}

#[test]
fn relative_override_is_made_absolute_inside_root() {
    let root = tmp();
    let real_root = real(root.path());
    let mut env = MapEnv::of([("UNIDEX_TEST_ROOT", root.path())]);
    env.0
        .insert("UNIDEX_CONFIG_DIR".to_string(), "rel/c".to_string());
    env.0
        .insert("UNIDEX_DATA_DIR".to_string(), "rel/d".to_string());
    env.0
        .insert("UNIDEX_CACHE_DIR".to_string(), "k".to_string());
    let p = AppPaths::resolve(&env).unwrap();
    assert_eq!(p.config_dir, real_root.join("rel").join("c"));
    assert_eq!(p.data_dir, real_root.join("rel").join("d"));
    assert_eq!(p.cache_dir, real_root.join("k"));
    assert!(p.data_dir.is_absolute());
    // Nothing is created, and nothing is resolved against the working folder.
    assert!(!real_root.join("rel").exists());
}

#[test]
fn relative_override_escaping_root_refused() {
    let root = tmp();
    for var in ["UNIDEX_CONFIG_DIR", "UNIDEX_DATA_DIR", "UNIDEX_CACHE_DIR"] {
        let mut env = MapEnv::of([("UNIDEX_TEST_ROOT", root.path())]);
        env.0.insert(var.to_string(), "../outside".to_string());
        let e = AppPaths::resolve(&env);
        assert!(
            matches!(e, Err(PathError::OutsideTestRoot { .. })),
            "{var}: {e:?}"
        );
    }
}
