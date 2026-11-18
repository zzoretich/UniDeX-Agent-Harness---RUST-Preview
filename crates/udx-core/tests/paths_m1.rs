//! T1b's path-guard tests that need the interfaces of A 3.1.1 (`PathError` additions, `test_root`, `home_dir`,
//! `user_var`, `check_test_path`, `os_side_effects_allowed`, `paths::testing::TestEnv`). They do not compile until
//! the builder adds those items and enables the `testing` feature for this crate's own tests (for example through a
//! dev-dependency of the crate on itself). The older tests stay in `paths.rs`.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use udx_core::paths::testing::TestEnv;
use udx_core::paths::{
    AppPaths, Env, OsSideEffect, PathError, UserVar, check_test_path, home_dir,
    os_side_effects_allowed, test_root, user_var,
};

const TEST_ROOT: &str = "UNIDEX_TEST_ROOT";
const OPT_IN: &str = "UNIDEX_TEST_OS_SIDE_EFFECTS";

struct MapEnv(HashMap<String, String>);

impl MapEnv {
    fn of<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> MapEnv {
        MapEnv(
            pairs
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }
    fn rooted(root: &Path) -> MapEnv {
        MapEnv::of([(TEST_ROOT, root.to_str().unwrap())])
    }
    fn with(mut self, k: &str, v: &str) -> MapEnv {
        self.0.insert(k.to_string(), v.to_string());
        self
    }
}

impl Env for MapEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}

/// A fresh folder inside the process's test root (never the OS temp folder).
fn tmp() -> udx_test_support::TestRoot {
    udx_test_support::TestRoot::new()
}

fn real(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap()
}

const OVERRIDES: [&str; 3] = ["UNIDEX_CONFIG_DIR", "UNIDEX_DATA_DIR", "UNIDEX_CACHE_DIR"];

#[test]
fn missing_root_is_created_by_test_env_and_refused_by_resolve_when_unresolvable() {
    // TestEnv creates its root on disk.
    let env = TestEnv::new();
    assert!(env.root().is_dir());
    assert!(AppPaths::resolve(&env).is_ok());

    // A root that does not exist yet is not an error: resolve creates nothing and keeps the walk going.
    let holder = tmp();
    let missing = holder.path().join("not-yet");
    let p = AppPaths::resolve(&MapEnv::rooted(&missing)).unwrap();
    assert!(p.data_dir.ends_with("not-yet/data") || p.data_dir.ends_with("not-yet\\data"));
    assert!(!missing.exists(), "resolve must create nothing");

    // A root that cannot be resolved (a symlink loop) is refused.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(holder.path().join("b"), holder.path().join("a")).unwrap();
        std::os::unix::fs::symlink(holder.path().join("a"), holder.path().join("b")).unwrap();
        let e = AppPaths::resolve(&MapEnv::rooted(&holder.path().join("a")));
        assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })), "{e:?}");
    }
}

#[test]
fn non_absolute_value_reports_not_absolute() {
    // The test root itself must be absolute.
    let e = AppPaths::resolve(&MapEnv::of([(TEST_ROOT, "relative/root")]));
    match e {
        Err(PathError::NotAbsolute { var, .. }) => assert_eq!(var, TEST_ROOT),
        other => panic!("expected NotAbsolute for {TEST_ROOT}, got {other:?}"),
    }
    // Without a test root, a relative override of each variable is not absolute. With the `testing` feature on, the
    // guard comes first: no test root is then TestBuildWithoutTestRoot (A 2.2), never the OS folders.
    for var in OVERRIDES {
        let e = AppPaths::resolve(&MapEnv::of([(var, "relative/dir")]));
        if cfg!(feature = "testing") {
            assert!(
                matches!(e, Err(PathError::TestBuildWithoutTestRoot)),
                "{var}: {e:?}"
            );
        } else {
            match e {
                Err(PathError::NotAbsolute { var: got, value }) => {
                    assert_eq!(got, var);
                    assert_eq!(value, PathBuf::from("relative/dir"));
                }
                other => panic!("expected NotAbsolute for {var}, got {other:?}"),
            }
        }
    }
}

#[test]
fn whitespace_only_value_reports_blank() {
    for blank in ["   ", "\t", " \t \n"] {
        let e = AppPaths::resolve(&MapEnv::of([(TEST_ROOT, blank)]));
        match e {
            Err(PathError::Blank { var }) => assert_eq!(var, TEST_ROOT),
            other => panic!("expected Blank for {TEST_ROOT} = {blank:?}, got {other:?}"),
        }
        let root = tmp();
        for var in OVERRIDES {
            let e = AppPaths::resolve(&MapEnv::rooted(root.path()).with(var, blank));
            match e {
                Err(PathError::Blank { var: got }) => assert_eq!(got, var),
                other => panic!("expected Blank for {var} = {blank:?}, got {other:?}"),
            }
        }
    }
    // The test_root accessor agrees.
    assert!(matches!(
        test_root(&MapEnv::of([(TEST_ROOT, "  ")])),
        Err(PathError::Blank { .. })
    ));
}

#[test]
fn testing_feature_never_returns_os_folders() {
    // Neither the test root nor the cargo marker is set: the answer is the typed error, not the OS's folders.
    let none = MapEnv::of([]);
    assert!(matches!(
        AppPaths::resolve(&none),
        Err(PathError::TestBuildWithoutTestRoot)
    ));
    // Overrides alone do not lift the requirement.
    let over = tmp();
    let env = MapEnv::of([("UNIDEX_DATA_DIR", over.path().to_str().unwrap())]);
    assert!(matches!(
        AppPaths::resolve(&env),
        Err(PathError::TestBuildWithoutTestRoot)
    ));
}

#[test]
fn check_test_path_refuses_outside_and_accepts_inside() {
    let (root, other) = (tmp(), tmp());
    let env = MapEnv::rooted(root.path());
    check_test_path(&env, &root.path().join("a").join("b")).unwrap();
    check_test_path(&env, root.path()).unwrap();
    let e = check_test_path(&env, &other.path().join("x"));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })), "{e:?}");
    // `..` out of the root.
    let name = other.path().file_name().unwrap();
    let e = check_test_path(&env, &root.path().join("..").join(name));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })), "{e:?}");
    // No test root in a `testing` build: refused like `resolve`, never "nothing to enforce". (The non-testing build's
    // answer is covered by the unit tests in `src/paths.rs`.)
    let e = check_test_path(&MapEnv::of([]), &other.path().join("x"));
    assert!(
        matches!(e, Err(PathError::TestBuildWithoutTestRoot)),
        "{e:?}"
    );
    let marker = MapEnv::of([("CARGO_MANIFEST_DIR", "/some/crate")]);
    let e = check_test_path(&marker, &other.path().join("x"));
    assert!(matches!(e, Err(PathError::CargoWithoutTestRoot)), "{e:?}");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(other.path(), root.path().join("link")).unwrap();
        let e = check_test_path(&env, &root.path().join("link").join("file"));
        assert!(
            matches!(e, Err(PathError::OutsideTestRoot { .. })),
            "a symlink that escapes: {e:?}"
        );
    }
}

#[test]
fn test_root_not_cached_across_envs() {
    let (a, b) = (TestEnv::new(), TestEnv::new());
    assert_ne!(a.root(), b.root());
    assert_eq!(test_root(&a).unwrap(), Some(real(a.root())));
    assert_eq!(test_root(&b).unwrap(), Some(real(b.root())));
    // Asked again in the other order, each still sees its own.
    assert_eq!(test_root(&a).unwrap(), Some(real(a.root())));
    // Their resolved folders lie in their own roots.
    assert!(
        AppPaths::resolve(&b)
            .unwrap()
            .data_dir
            .starts_with(real(b.root()))
    );
    // No root set: None.
    assert_eq!(test_root(&MapEnv::of([])).unwrap(), None);
}

#[test]
fn home_dir_inside_test_root() {
    let env = TestEnv::new();
    let home = home_dir(&env).unwrap();
    assert_eq!(home, real(env.root()).join("home"));
    // A HOME variable in the environment does not move it.
    let root = tmp();
    let with_home = MapEnv::rooted(root.path()).with("HOME", "/home/user");
    assert_eq!(
        home_dir(&with_home).unwrap(),
        real(root.path()).join("home")
    );
    // The guard applies: a root that cannot be resolved gives no home.
    #[cfg(unix)]
    {
        let holder = tmp();
        std::os::unix::fs::symlink(holder.path().join("b"), holder.path().join("a")).unwrap();
        std::os::unix::fs::symlink(holder.path().join("a"), holder.path().join("b")).unwrap();
        assert!(home_dir(&MapEnv::rooted(&holder.path().join("a"))).is_err());
    }
}

/// Records every key asked for through `var` or `var_os`, and answers each.
struct Recording {
    seen: RefCell<BTreeSet<String>>,
    root: PathBuf,
}

impl Recording {
    fn answer(&self, key: &str) -> String {
        self.seen.borrow_mut().insert(key.to_ascii_uppercase());
        if key == TEST_ROOT {
            self.root.to_string_lossy().into_owned()
        } else {
            format!("v-{key}")
        }
    }
}

impl Env for Recording {
    fn var(&self, key: &str) -> Option<String> {
        Some(self.answer(key))
    }
    fn var_os(&self, key: &str) -> Option<OsString> {
        Some(OsString::from(self.answer(key)))
    }
}

#[test]
fn user_var_reads_only_listed_names() {
    let all = || {
        [
            UserVar::Home,
            UserVar::User,
            UserVar::Logname,
            UserVar::Shell,
            UserVar::Lang,
            UserVar::Tz,
            UserVar::Term,
            UserVar::XdgRuntimeDir,
            UserVar::UserProfile,
            UserVar::SystemRoot,
            UserVar::Windir,
            UserVar::ComSpec,
            UserVar::PathExt,
            UserVar::Path,
        ]
    };
    // A user variable that is a path into the user's folders is answered from the test root, so `user_var` reads the
    // test root too; the root is a valid one here, as it is under cargo.
    let holder = tmp();
    let rec = Recording {
        seen: RefCell::new(BTreeSet::new()),
        root: holder.path().to_path_buf(),
    };
    for v in all() {
        assert!(user_var(&rec, v).is_some());
    }
    // Windows spells some of these in mixed case; compare without case. Under a test root the three path variables
    // that point into the user's folders (HOME, USERPROFILE, XDG_RUNTIME_DIR) are answered from the root and their
    // real values are never read (nor are USER and LOGNAME, which the root answers with a placeholder); a build without the `testing` feature reads them (unit test in `src/paths.rs`).
    let expected: BTreeSet<String> = [
        "SHELL",
        "LANG",
        "TZ",
        "TERM",
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "PATHEXT",
        "PATH",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain([TEST_ROOT.to_string()])
    .collect();
    assert_eq!(*rec.seen.borrow(), expected);

    // Unset in the environment is None, for every variant.
    for v in all() {
        assert_eq!(user_var(&MapEnv::of([]), v), None);
    }
    // A set value comes back as set.
    let env = MapEnv::of([("LANG", "en_US.UTF-8")]);
    assert_eq!(
        user_var(&env, UserVar::Lang),
        Some(OsString::from("en_US.UTF-8"))
    );
}

fn every_kind() -> Vec<(OsSideEffect, &'static str)> {
    vec![
        (OsSideEffect::SecretStore, "secret_store"),
        (OsSideEffect::WakeUp, "wake_up"),
        (OsSideEffect::WindowsAcl, "windows_acl"),
        (OsSideEffect::AppContainerProfile, "app_container_profile"),
        (OsSideEffect::TrustStoreCert, "trust_store_cert"),
        (OsSideEffect::SandboxEscapeMatrix, "sandbox_escape_matrix"),
        (OsSideEffect::DiskImage, "disk_image"),
        (OsSideEffect::PackageInstall, "package_install"),
    ]
}

#[test]
fn os_side_effects_refused_with_test_root() {
    let root = tmp();
    let env = MapEnv::rooted(root.path());
    for (kind, name) in every_kind() {
        assert!(
            os_side_effects_allowed(&env, kind).is_err(),
            "{name} must be refused under a test root without the opt-in"
        );
    }
    // The wake-up in particular (M6's scheduler calls this).
    assert!(os_side_effects_allowed(&env, OsSideEffect::WakeUp).is_err());
    // An empty opt-in names nothing.
    let env = MapEnv::rooted(root.path()).with(OPT_IN, "");
    for (kind, name) in every_kind() {
        assert!(os_side_effects_allowed(&env, kind).is_err(), "{name}");
    }
    // No test root in a `testing` build or under cargo: refused like `resolve`, so a unit test with a hand-built
    // Env can never be told an OS side effect is allowed. (An installed app, which has no `testing` build, is allowed;
    // the unit tests in `src/paths.rs` cover that.) The opt-in alone does not lift it.
    let marker = MapEnv::of([("CARGO_MANIFEST_DIR", "/some/crate"), (OPT_IN, "all")]);
    let unrooted_all = MapEnv::of([(OPT_IN, "all")]);
    for (kind, name) in every_kind() {
        let e = os_side_effects_allowed(&MapEnv::of([]), kind);
        assert!(
            matches!(e, Err(PathError::TestBuildWithoutTestRoot)),
            "{name}: {e:?}"
        );
        let e = os_side_effects_allowed(&marker, kind);
        assert!(
            matches!(e, Err(PathError::CargoWithoutTestRoot)),
            "{name}: {e:?}"
        );
        let e = os_side_effects_allowed(&unrooted_all, kind);
        assert!(
            matches!(e, Err(PathError::TestBuildWithoutTestRoot)),
            "{name}: {e:?}"
        );
    }
}

#[test]
fn os_side_effects_opt_in_by_kind() {
    let root = tmp();
    let rooted = |optin: &str| MapEnv::rooted(root.path()).with(OPT_IN, optin);

    // One kind, and a list of two: only the named kinds pass.
    for (named, allowed) in [
        ("secret_store", vec!["secret_store"]),
        ("secret_store,wake_up", vec!["secret_store", "wake_up"]),
        (
            "disk_image,package_install",
            vec!["disk_image", "package_install"],
        ),
    ] {
        let env = rooted(named);
        for (kind, name) in every_kind() {
            let r = os_side_effects_allowed(&env, kind);
            if allowed.contains(&name) {
                assert!(r.is_ok(), "{name} is named in {named:?}: {r:?}");
            } else {
                assert!(r.is_err(), "{name} is not named in {named:?}");
            }
        }
    }
    // `all` names every kind.
    let env = rooted("all");
    for (kind, name) in every_kind() {
        assert!(
            os_side_effects_allowed(&env, kind).is_ok(),
            "{name} under all"
        );
    }
    // A prefix or an unknown word names nothing.
    for junk in ["secret", "windows", "secret_stor", "everything", "wake-up"] {
        let env = rooted(junk);
        for (kind, name) in every_kind() {
            assert!(
                os_side_effects_allowed(&env, kind).is_err(),
                "{name} must stay refused under {junk:?}"
            );
        }
    }
}

#[test]
fn user_var_under_a_test_root_never_returns_the_real_folders() {
    let holder = tmp();
    let root = real(holder.path());
    let env = MapEnv::rooted(holder.path())
        .with("HOME", "/home/user")
        .with("USERPROFILE", "C:\\Users\\user")
        .with("XDG_RUNTIME_DIR", "/run/user/1000")
        .with("SHELL", "/bin/sh")
        .with("PATH", "/usr/bin")
        .with("USER", "someone")
        .with("LANG", "en_US.UTF-8")
        .with("SYSTEMROOT", "C:\\Windows")
        .with("WINDIR", "C:\\Windows")
        .with("COMSPEC", "C:\\Windows\\cmd.exe")
        .with("LOGNAME", "someone")
        .with("TZ", "UTC")
        .with("TERM", "xterm")
        .with("PATHEXT", ".EXE");
    // Paths into the user's folders: the test root's home, and a runtime folder inside the root.
    let home = home_dir(&env).unwrap();
    assert_eq!(home, root.join("home"));
    assert_eq!(
        user_var(&env, UserVar::Home),
        Some(home.clone().into_os_string())
    );
    assert_eq!(
        user_var(&env, UserVar::UserProfile),
        Some(home.into_os_string())
    );
    assert_eq!(
        user_var(&env, UserVar::XdgRuntimeDir),
        Some(root.join("runtime").into_os_string())
    );
    // The account name is the last part of the real home folder and a personal name: a placeholder, never the real one.
    assert_eq!(user_var(&env, UserVar::User), Some(OsString::from("user")));
    assert_eq!(
        user_var(&env, UserVar::Logname),
        Some(OsString::from("user"))
    );
    // Everything else passes through: paths to system programs and folders, names, locale.
    for (var, want) in [
        (UserVar::Shell, "/bin/sh"),
        (UserVar::Path, "/usr/bin"),
        (UserVar::Lang, "en_US.UTF-8"),
        (UserVar::Tz, "UTC"),
        (UserVar::Term, "xterm"),
        (UserVar::SystemRoot, "C:\\Windows"),
        (UserVar::Windir, "C:\\Windows"),
        (UserVar::ComSpec, "C:\\Windows\\cmd.exe"),
        (UserVar::PathExt, ".EXE"),
    ] {
        assert_eq!(user_var(&env, var), Some(OsString::from(want)), "{var:?}");
    }
    // A test root that cannot be used gives no real value for a path variable (fail closed), and the others still pass.
    let bad = MapEnv::of([
        (TEST_ROOT, "relative/root"),
        ("HOME", "/home/user"),
        ("LANG", "C"),
    ]);
    assert_eq!(user_var(&bad, UserVar::Home), None);
    assert_eq!(user_var(&bad, UserVar::Lang), Some(OsString::from("C")));
    // No test root in a `testing` build: also no real value.
    let none = MapEnv::of([
        ("HOME", "/home/user"),
        ("USERPROFILE", "x"),
        ("XDG_RUNTIME_DIR", "y"),
    ]);
    let none = none.with("USER", "someone").with("LOGNAME", "someone");
    for var in [
        UserVar::Home,
        UserVar::UserProfile,
        UserVar::XdgRuntimeDir,
        UserVar::User,
        UserVar::Logname,
    ] {
        assert_eq!(user_var(&none, var), None, "{var:?}");
    }
}

#[test]
fn a_test_root_with_no_normal_component_is_refused() {
    let top = if cfg!(windows) { "C:\\" } else { "/" };
    let e = test_root(&MapEnv::of([(TEST_ROOT, top)]));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })), "{e:?}");
    let e = AppPaths::resolve(&MapEnv::of([(TEST_ROOT, top)]));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })), "{e:?}");
}

#[test]
fn test_env_creates_its_home() {
    let env = TestEnv::new();
    assert!(env.root().join("home").is_dir());
    assert!(home_dir(&env).unwrap().is_dir());
}

/// An Env that answers only through `var_os`, as a non-UTF-8 value can only be read.
struct OsOnly(PathBuf);

impl Env for OsOnly {
    fn var(&self, _key: &str) -> Option<String> {
        None
    }
    fn var_os(&self, key: &str) -> Option<OsString> {
        (key == TEST_ROOT).then(|| self.0.clone().into_os_string())
    }
}

#[test]
fn path_values_are_read_with_var_os() {
    let holder = tmp();
    let root = real(holder.path()).join("sub-0000");
    // The exact value, not the lossy one, is what is resolved.
    assert_eq!(test_root(&OsOnly(root.clone())).unwrap(), Some(root));
    // A root that only `var_os` can see is still a root.
    assert!(AppPaths::resolve(&OsOnly(real(holder.path()))).is_ok());
}

#[cfg(target_os = "linux")]
#[test]
fn a_non_utf8_test_root_is_kept_exactly() {
    use std::os::unix::ffi::OsStringExt;
    let holder = tmp();
    let mut name = real(holder.path()).into_os_string().into_vec();
    name.extend_from_slice(b"/odd-\xFF-name");
    let root = PathBuf::from(OsString::from_vec(name));
    assert_eq!(test_root(&OsOnly(root.clone())).unwrap(), Some(root));
}

#[test]
fn test_env_creates_its_runtime_folder() {
    let env = TestEnv::new();
    let runtime = user_var(&env, UserVar::XdgRuntimeDir).unwrap();
    assert_eq!(PathBuf::from(&runtime), real(env.root()).join("runtime"));
    assert!(Path::new(&runtime).is_dir());
}

/// An Env that shows the cargo marker through `var` and the test root only through `var_os`, as a non-UTF-8 root is.
struct MarkerAndOsRoot(PathBuf);

impl Env for MarkerAndOsRoot {
    fn var(&self, key: &str) -> Option<String> {
        (key == "CARGO_MANIFEST_DIR").then(|| "/some/crate".to_string())
    }
    fn var_os(&self, key: &str) -> Option<OsString> {
        match key {
            TEST_ROOT => Some(self.0.clone().into_os_string()),
            _ => self.var(key).map(OsString::from),
        }
    }
}

#[test]
fn the_cargo_check_and_test_root_read_the_root_the_same_way() {
    let holder = tmp();
    let env = MarkerAndOsRoot(real(holder.path()));
    // The root is there (for `var_os`), so the marker does not make it "missing".
    assert_eq!(test_root(&env).unwrap(), Some(real(holder.path())));
    assert!(AppPaths::resolve(&env).is_ok());
    assert_eq!(home_dir(&env).unwrap(), real(holder.path()).join("home"));
    assert!(check_test_path(&env, &holder.path().join("x")).is_ok());
}

#[cfg(target_os = "linux")]
#[test]
fn a_non_utf8_test_root_gives_the_same_answer_everywhere() {
    use std::os::unix::ffi::OsStringExt;
    let holder = tmp();
    let mut name = real(holder.path()).into_os_string().into_vec();
    name.extend_from_slice(b"/odd-\xFF-name");
    let root = PathBuf::from(OsString::from_vec(name));
    let env = MarkerAndOsRoot(root.clone());
    assert_eq!(test_root(&env).unwrap(), Some(root.clone()));
    assert_eq!(AppPaths::resolve(&env).unwrap().data_dir, root.join("data"));
    assert_eq!(home_dir(&env).unwrap(), root.join("home"));
    // A sibling folder outside the root is still refused.
    let e = check_test_path(&env, &real(holder.path()).join("elsewhere"));
    assert!(matches!(e, Err(PathError::OutsideTestRoot { .. })), "{e:?}");
}
