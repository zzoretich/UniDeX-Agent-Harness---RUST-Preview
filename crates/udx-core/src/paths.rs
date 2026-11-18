//! Where the application keeps its config, data and cache folders.
//!
//! Anything started through cargo is given `UNIDEX_TEST_ROOT` by
//! `.cargo/config.toml`. When that variable is set, every folder must lie
//! inside it after symlinks and `..` components are resolved, so a test run
//! can never write into a real user's folders. Only an installed binary,
//! which has no test root, uses the operating system's folders.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

const TEST_ROOT: &str = "UNIDEX_TEST_ROOT";
const CONFIG_DIR: &str = "UNIDEX_CONFIG_DIR";
const DATA_DIR: &str = "UNIDEX_DATA_DIR";
const CACHE_DIR: &str = "UNIDEX_CACHE_DIR";
const OS_SIDE_EFFECTS: &str = "UNIDEX_TEST_OS_SIDE_EFFECTS";

/// The override variables, in the order of [`AppPaths`]' fields.
const OVERRIDE_VARS: [&str; 3] = [CONFIG_DIR, DATA_DIR, CACHE_DIR];

/// How many symlinks one lookup may follow before it is refused.
const MAX_LINK_HOPS: u32 = 40;

/// A source of environment variables, so tests need not touch the real one.
pub trait Env {
    /// The value of `key`, if it is set.
    fn var(&self, key: &str) -> Option<String>;

    /// The value of `key` as the operating system holds it, if it is set, so a
    /// value that is not UTF-8 (a `PATH`, say) survives.
    fn var_os(&self, key: &str) -> Option<OsString> {
        self.var(key).map(OsString::from)
    }
}

/// The real process environment.
pub struct ProcessEnv;

impl Env for ProcessEnv {
    fn var(&self, key: &str) -> Option<String> {
        use std::env::VarError;
        match std::env::var(key) {
            Ok(v) => Some(v),
            // A set variable is never read as unset.
            Err(VarError::NotUnicode(v)) => Some(v.to_string_lossy().into_owned()),
            Err(VarError::NotPresent) => None,
        }
    }

    fn var_os(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }
}

/// The three folders the application uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

/// Why the folders could not be decided.
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("no home folder found")]
    NoHome,
    #[error("{dir} is outside the test root {root}")]
    OutsideTestRoot { dir: PathBuf, root: PathBuf },
    #[error("started by cargo without UNIDEX_TEST_ROOT")]
    CargoWithoutTestRoot,
    /// A value that has to be an absolute path is not.
    #[error("{var} is not an absolute path: {value}")]
    NotAbsolute { var: &'static str, value: PathBuf },
    /// A value that is only whitespace.
    #[error("{var} is blank")]
    Blank { var: &'static str },
    /// The `testing` feature is on and no test root is set: never the OS folders.
    #[error("a test build has no UNIDEX_TEST_ROOT and never uses the operating system's folders")]
    TestBuildWithoutTestRoot,
    /// A change to OS-level state that a test run may not make (amendment to A 3.1.1).
    #[error(
        "{} changes OS state and is refused in a test run; name it in UNIDEX_TEST_OS_SIDE_EFFECTS",
        .what.name()
    )]
    SideEffectRefused { what: OsSideEffect },
}

/// A kind of change to OS-level state outside the app's folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsSideEffect {
    /// A write to the OS secret store, or planting an item a test then looks for.
    SecretStore,
    /// M6 scheduler.
    WakeUp,
    /// An access entry on a file or folder for a sandbox SID.
    WindowsAcl,
    /// Creating or deleting an AppContainer profile.
    AppContainerProfile,
    /// Adding a test certificate authority to an OS trust store.
    TrustStoreCert,
    /// Running escape payloads that, if the sandbox failed, would open windows or register launch items.
    SandboxEscapeMatrix,
    /// Creating and attaching a disk image.
    DiskImage,
    /// Installing a built package.
    PackageInstall,
}

impl OsSideEffect {
    /// The kind's name in `UNIDEX_TEST_OS_SIDE_EFFECTS`.
    fn name(self) -> &'static str {
        match self {
            OsSideEffect::SecretStore => "secret_store",
            OsSideEffect::WakeUp => "wake_up",
            OsSideEffect::WindowsAcl => "windows_acl",
            OsSideEffect::AppContainerProfile => "app_container_profile",
            OsSideEffect::TrustStoreCert => "trust_store_cert",
            OsSideEffect::SandboxEscapeMatrix => "sandbox_escape_matrix",
            OsSideEffect::DiskImage => "disk_image",
            OsSideEffect::PackageInstall => "package_install",
        }
    }
}

/// The few user variables other modules need, read only through [`user_var`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserVar {
    Home,
    User,
    Logname,
    Shell,
    Lang,
    Tz,
    Term,
    XdgRuntimeDir,
    UserProfile,
    SystemRoot,
    Windir,
    ComSpec,
    PathExt,
    Path,
}

impl UserVar {
    fn name(self) -> &'static str {
        match self {
            UserVar::Home => "HOME",
            UserVar::User => "USER",
            UserVar::Logname => "LOGNAME",
            UserVar::Shell => "SHELL",
            UserVar::Lang => "LANG",
            UserVar::Tz => "TZ",
            UserVar::Term => "TERM",
            UserVar::XdgRuntimeDir => "XDG_RUNTIME_DIR",
            UserVar::UserProfile => "USERPROFILE",
            UserVar::SystemRoot => "SYSTEMROOT",
            UserVar::Windir => "WINDIR",
            UserVar::ComSpec => "COMSPEC",
            UserVar::PathExt => "PATHEXT",
            UserVar::Path => "PATH",
        }
    }
}

/// A variable's value, or `None` when it is unset or empty.
fn non_empty(env: &dyn Env, key: &str) -> Option<String> {
    env.var(key).filter(|v| !v.is_empty())
}

/// A path-valued variable as the OS holds it (not lossy), or `None` when it is unset or empty.
fn non_empty_os(env: &dyn Env, key: &str) -> Option<OsString> {
    env.var_os(key).filter(|v| !v.is_empty())
}

/// A path-valued value that is only whitespace.
fn is_blank(v: &OsString) -> bool {
    v.to_str().is_some_and(|s| s.trim().is_empty())
}

/// An override variable: unset or empty is `None`, whitespace only is `Blank`.
fn override_var(env: &dyn Env, var: &'static str) -> Result<Option<PathBuf>, PathError> {
    match non_empty_os(env, var) {
        None => Ok(None),
        Some(v) if is_blank(&v) => Err(PathError::Blank { var }),
        Some(v) => Ok(Some(PathBuf::from(v))),
    }
}

/// The test root, or `None` where the operating system's folders are allowed.
/// Fails closed first: a process cargo started has a test root, and a build
/// with the `testing` feature (`testing`) never falls through to the OS.
fn guard(env: &dyn Env, testing: bool) -> Result<Option<PathBuf>, PathError> {
    // Cargo sets CARGO_MANIFEST_DIR for everything it runs; such a process
    // must have a test root and never falls through to the OS folders.
    // The root is read the way `test_root` reads it (exactly, not lossily).
    if non_empty_os(env, TEST_ROOT).is_none() && non_empty(env, "CARGO_MANIFEST_DIR").is_some() {
        return Err(PathError::CargoWithoutTestRoot);
    }
    let root = test_root(env)?;
    if root.is_none() && testing {
        return Err(PathError::TestBuildWithoutTestRoot);
    }
    Ok(root)
}

/// The test root of this process, if any (`UNIDEX_TEST_ROOT`, resolved). Not
/// cached: every call reads `env`, so each test environment's own root is honoured.
pub fn test_root(env: &dyn Env) -> Result<Option<PathBuf>, PathError> {
    let Some(raw) = non_empty_os(env, TEST_ROOT) else {
        return Ok(None);
    };
    if is_blank(&raw) {
        return Err(PathError::Blank { var: TEST_ROOT });
    }
    let root = PathBuf::from(raw);
    if !root.is_absolute() {
        return Err(PathError::NotAbsolute {
            var: TEST_ROOT,
            value: root,
        });
    }
    // A root such as `/` or `C:\` would make every path "inside" it.
    let real =
        resolve_path(&root).filter(|r| r.components().any(|c| matches!(c, Component::Normal(_))));
    real.map(Some).ok_or_else(|| outside(&root, &root))
}

/// `dir` resolved, which must lie inside the resolved `root`.
fn inside(root: &Path, dir: &Path) -> Result<PathBuf, PathError> {
    let real = resolve_path(dir).ok_or_else(|| outside(dir, root))?;
    if real.starts_with(root) {
        Ok(real)
    } else {
        Err(outside(&real, root))
    }
}

impl AppPaths {
    /// Decide the folders from `env`. Reads no folder contents and creates nothing.
    ///
    /// With the `testing` feature on, a missing test root is an error, never
    /// the operating system's folders. A relative override is joined onto the
    /// test root; without a test root it is `NotAbsolute`.
    pub fn resolve(env: &dyn Env) -> Result<AppPaths, PathError> {
        resolve_with(env, cfg!(feature = "testing"))
    }
}

fn resolve_with(env: &dyn Env, testing: bool) -> Result<AppPaths, PathError> {
    let root = guard(env, testing)?;
    let over = [
        override_var(env, CONFIG_DIR)?,
        override_var(env, DATA_DIR)?,
        override_var(env, CACHE_DIR)?,
    ];
    let [config_dir, data_dir, cache_dir] = match root {
        Some(root) => {
            let pick = |i: usize, default: &str| {
                let dir = match &over[i] {
                    Some(o) => root.join(o),
                    None => root.join(default),
                };
                inside(&root, &dir)
            };
            [pick(0, "config")?, pick(1, "data")?, pick(2, "cache")?]
        }
        None => {
            let os = directories::ProjectDirs::from("", "", "unidex-preview");
            let pick = |i: usize, os_dir: fn(&directories::ProjectDirs) -> &Path| match &over[i] {
                Some(o) if o.is_absolute() => Ok(o.clone()),
                Some(o) => Err(PathError::NotAbsolute {
                    var: OVERRIDE_VARS[i],
                    value: o.clone(),
                }),
                None => os
                    .as_ref()
                    .map(|p| os_dir(p).to_path_buf())
                    .ok_or(PathError::NoHome),
            };
            [
                pick(0, directories::ProjectDirs::config_dir)?,
                pick(1, directories::ProjectDirs::data_dir)?,
                pick(2, directories::ProjectDirs::cache_dir)?,
            ]
        }
    };
    Ok(AppPaths {
        config_dir,
        data_dir,
        cache_dir,
    })
}

/// The user's home folder: the OS's answer normally; `<test root>/home` when a
/// test root is set (refused if that lies outside the root).
pub fn home_dir(env: &dyn Env) -> Result<PathBuf, PathError> {
    home_dir_with(env, cfg!(feature = "testing"))
}

fn home_dir_with(env: &dyn Env, testing: bool) -> Result<PathBuf, PathError> {
    match guard(env, testing)? {
        Some(root) => inside(&root, &root.join("home")),
        None => directories::BaseDirs::new()
            .map(|b| b.home_dir().to_path_buf())
            .ok_or(PathError::NoHome),
    }
}

/// The few user variables other modules need, read only here.
///
/// Under a test root, a variable whose value is a path into the user's folders, or the account name (the last part of
/// the real home folder, a personal name), is answered from the test root, never with the real value: `Home` and
/// `UserProfile` give [`home_dir`], `XdgRuntimeDir` gives `<test root>/runtime`, and `User` and `Logname` give the
/// placeholder `user`. A test root that cannot be used, or none in a `testing` build, gives `None` for those five.
/// Every other variable passes through: `Lang`, `Tz`, `Term` and `PathExt` are not paths, and `Shell`, `SystemRoot`,
/// `Windir` and `ComSpec` name system programs and folders. Known limit: `Path` and `Shell` are not redirected, so a
/// test that reads them can see a home folder inside a `PATH` entry; tests that run commands need them.
pub fn user_var(env: &dyn Env, var: UserVar) -> Option<OsString> {
    user_var_with(env, var, cfg!(feature = "testing"))
}

fn user_var_with(env: &dyn Env, var: UserVar, testing: bool) -> Option<OsString> {
    let redirected = match var {
        UserVar::Home
        | UserVar::UserProfile
        | UserVar::XdgRuntimeDir
        | UserVar::User
        | UserVar::Logname => true,
        UserVar::Shell
        | UserVar::Lang
        | UserVar::Tz
        | UserVar::Term
        | UserVar::SystemRoot
        | UserVar::Windir
        | UserVar::ComSpec
        | UserVar::PathExt
        | UserVar::Path => false,
    };
    if !redirected {
        return env.var_os(var.name());
    }
    match guard(env, testing) {
        Ok(Some(root)) => match var {
            UserVar::User | UserVar::Logname => Some(OsString::from("user")),
            UserVar::XdgRuntimeDir => inside(&root, &root.join("runtime"))
                .ok()
                .map(PathBuf::into_os_string),
            _ => inside(&root, &root.join("home"))
                .ok()
                .map(PathBuf::into_os_string),
        },
        Ok(None) => env.var_os(var.name()),
        Err(_) => None,
    }
}

/// Fails when a test root is set and `path` (resolved as the folders are)
/// lies outside it. A relative path cannot be shown to lie inside, so it is
/// refused too. In a `testing` build, or under cargo, with no test root it
/// fails closed like [`AppPaths::resolve`].
pub fn check_test_path(env: &dyn Env, path: &Path) -> Result<(), PathError> {
    check_test_path_with(env, path, cfg!(feature = "testing"))
}

fn check_test_path_with(env: &dyn Env, path: &Path, testing: bool) -> Result<(), PathError> {
    let Some(root) = guard(env, testing)? else {
        return Ok(());
    };
    if !path.is_absolute() {
        return Err(outside(path, &root));
    }
    inside(&root, path).map(|_| ())
}

/// Fails under a test root, and in a `testing` build or under cargo without one, unless `UNIDEX_TEST_OS_SIDE_EFFECTS` names
/// this kind (snake case, comma-separated) or `all`. Called before any change
/// to OS-level state outside the app's folders; only CI runners set the variable.
pub fn os_side_effects_allowed(env: &dyn Env, what: OsSideEffect) -> Result<(), PathError> {
    os_side_effects_allowed_with(env, what, cfg!(feature = "testing"))
}

fn os_side_effects_allowed_with(
    env: &dyn Env,
    what: OsSideEffect,
    testing: bool,
) -> Result<(), PathError> {
    // The `?` is the refusal: with no test root in a `testing` build or under cargo, `guard` returns an error, so the
    // `Ok(())` below is reached only with no test root in a build that is neither (an installed app).
    if guard(env, testing)?.is_none() {
        return Ok(());
    }
    let named = non_empty(env, OS_SIDE_EFFECTS).unwrap_or_default();
    if named
        .split(',')
        .map(str::trim)
        .any(|k| k == "all" || k == what.name())
    {
        Ok(())
    } else {
        Err(PathError::SideEffectRefused { what })
    }
}

/// Helpers for tests, behind the `testing` feature (never in a release build).
#[cfg(feature = "testing")]
pub mod testing {
    use std::collections::HashMap;
    use std::path::Path;

    use super::{Env, TEST_ROOT};

    /// Marks a binary that carries the feature; the release check looks for it.
    #[used]
    static UNIDEX_TESTING_MARKER_UDX_CORE: &str = "UNIDEX_TESTING_MARKER_udx_core";

    /// A fresh folder inside the process's test root, removed on drop, with its `home` and `runtime` folders made.
    ///
    /// As an [`Env`] it sets `UNIDEX_TEST_ROOT` to that folder, plus any
    /// variable added with [`TestEnv::with`], and nothing else.
    pub struct TestEnv {
        dir: tempfile::TempDir,
        vars: HashMap<String, String>,
    }

    impl TestEnv {
        /// Panics if `UNIDEX_TEST_ROOT` is missing: run the tests through cargo.
        pub fn new() -> TestEnv {
            // Read as the OS holds it, so a root that is not valid UTF-8 is not mistaken for a missing one.
            let root = std::env::var_os(TEST_ROOT)
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| {
                    panic!("{TEST_ROOT} is not set: run the tests through cargo (cargo test)")
                });
            // Checked before anything is created, so a relative value never makes a folder in the working folder.
            assert!(
                Path::new(&root).is_absolute(),
                "{TEST_ROOT} must be an absolute path"
            );
            std::fs::create_dir_all(&root).expect("create the test root");
            let dir = tempfile::Builder::new()
                .prefix("udx-")
                .tempdir_in(&root)
                .expect("create a folder inside the test root");
            for sub in ["home", "runtime"] {
                std::fs::create_dir(dir.path().join(sub))
                    .expect("create a folder of the test home");
            }
            TestEnv {
                dir,
                vars: HashMap::new(),
            }
        }

        /// The fresh folder. It exists on disk.
        pub fn root(&self) -> &Path {
            self.dir.path()
        }

        /// Also answer `key` with `value`, before the root is consulted.
        pub fn with(mut self, key: &str, value: &str) -> TestEnv {
            self.vars.insert(key.to_string(), value.to_string());
            self
        }
    }

    impl Default for TestEnv {
        fn default() -> Self {
            TestEnv::new()
        }
    }

    impl Env for TestEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.var_os(key).map(|v| v.to_string_lossy().into_owned())
        }
        fn var_os(&self, key: &str) -> Option<std::ffi::OsString> {
            if let Some(v) = self.vars.get(key) {
                return Some(v.into());
            }
            (key == TEST_ROOT).then(|| self.dir.path().as_os_str().to_owned())
        }
    }
}

fn outside(dir: &Path, root: &Path) -> PathError {
    PathError::OutsideTestRoot {
        dir: dir.to_path_buf(),
        root: root.to_path_buf(),
    }
}

/// Resolve an absolute path without creating anything, walking its components
/// forward from the root and keeping the base symlink-free at every step.
/// `.` is dropped; `..` pops one component of the resolved base; a normal
/// component is pushed and, if it is a symlink (dangling or not), replaced by
/// its target, whose own components are then walked. A component that does not
/// exist is kept and the walk continues, so a later `..` or symlink is still
/// handled. `None` means a loop, an exhausted hop budget, or a component or
/// link that cannot be read for any reason other than not existing: the caller
/// refuses. The result's spelling is then settled by `normalise`.
fn resolve_path(path: &Path) -> Option<PathBuf> {
    let mut hops = MAX_LINK_HOPS;
    let mut base = PathBuf::new();
    // Components still to walk, next one last.
    let mut todo: Vec<OsString> = Vec::new();
    push_components(&mut todo, path);
    while let Some(part) = todo.pop() {
        match Path::new(&part).components().next() {
            Some(Component::CurDir) | None => {}
            Some(Component::ParentDir) => {
                base.pop();
            }
            Some(Component::RootDir | Component::Prefix(_)) => base.push(&part),
            Some(Component::Normal(_)) => {
                base.push(&part);
                let is_link = match std::fs::symlink_metadata(&base) {
                    Ok(m) => m.file_type().is_symlink(),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                    // Anything else (for example no permission) cannot be trusted.
                    Err(_) => return None,
                };
                if is_link {
                    hops = hops.checked_sub(1)?;
                    let target = std::fs::read_link(&base).ok()?;
                    base.pop();
                    if target.has_root() {
                        base = PathBuf::new();
                    }
                    push_components(&mut todo, &target);
                }
            }
        }
    }
    Some(normalise(base))
}

/// Settle the spelling of a resolved path: canonicalize its longest existing
/// prefix and append the rest. The path has no `..` and its existing
/// components are already symlink-free, so only the spelling can change (for
/// example `\\?\` prefixes or short names on Windows), never the location.
fn normalise(path: PathBuf) -> PathBuf {
    let mut prefix = path.clone();
    let mut rest: Vec<OsString> = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(&prefix) {
            return rest.iter().rev().fold(real, |p, part| p.join(part));
        }
        match prefix.file_name() {
            Some(name) => rest.push(name.to_os_string()),
            None => return path,
        }
        prefix.pop();
    }
}

/// Queue the components of `path` so the first is walked next.
fn push_components(todo: &mut Vec<OsString>, path: &Path) {
    for c in path.components().rev() {
        todo.push(c.as_os_str().to_os_string());
    }
}

#[cfg(test)]
mod tests {
    //! The paths a build without the `testing` feature takes. The crate's own
    //! tests run with the feature on, so these call the functions that
    //! `resolve` and `home_dir` call, with the feature flag passed in.

    use std::collections::HashMap;

    use super::*;

    struct Map(HashMap<&'static str, String>);

    impl Env for Map {
        fn var(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
    }

    fn map(pairs: &[(&'static str, &str)]) -> Map {
        Map(pairs.iter().map(|(k, v)| (*k, v.to_string())).collect())
    }

    fn abs(base: &udx_test_support::TestRoot, name: &str) -> String {
        base.child(name).to_string_lossy().into_owned()
    }

    #[test]
    fn without_a_test_root_the_os_folders_are_used() {
        let p = resolve_with(&map(&[]), false).unwrap();
        for dir in [&p.config_dir, &p.data_dir, &p.cache_dir] {
            assert!(dir.is_absolute(), "{dir:?}");
            assert!(dir.to_string_lossy().contains("unidex-preview"), "{dir:?}");
        }
        assert!(home_dir_with(&map(&[]), false).unwrap().is_absolute());
    }

    #[test]
    fn os_folders_honour_absolute_overrides_and_refuse_relative_ones() {
        let base = udx_test_support::TestRoot::new();
        let (c, d, k) = (abs(&base, "c"), abs(&base, "d"), abs(&base, "k"));
        let env = map(&[(CONFIG_DIR, &c), (DATA_DIR, &d), (CACHE_DIR, &k)]);
        let p = resolve_with(&env, false).unwrap();
        assert_eq!(p.config_dir, PathBuf::from(&c));
        assert_eq!(p.data_dir, PathBuf::from(&d));
        assert_eq!(p.cache_dir, PathBuf::from(&k));
        for var in OVERRIDE_VARS {
            let e = resolve_with(&map(&[(var, "rel/dir")]), false);
            match e {
                Err(PathError::NotAbsolute { var: got, value }) => {
                    assert_eq!(got, var);
                    assert_eq!(value, PathBuf::from("rel/dir"));
                }
                other => panic!("{var}: {other:?}"),
            }
            let e = resolve_with(&map(&[(var, "  ")]), false);
            assert!(matches!(e, Err(PathError::Blank { var: got }) if got == var));
        }
    }

    #[test]
    fn only_the_feature_flag_decides_the_missing_root_case() {
        assert!(matches!(
            resolve_with(&map(&[]), true),
            Err(PathError::TestBuildWithoutTestRoot)
        ));
        assert!(matches!(
            home_dir_with(&map(&[]), true),
            Err(PathError::TestBuildWithoutTestRoot)
        ));
        // Cargo's marker without a root fails closed in either build.
        let env = map(&[("CARGO_MANIFEST_DIR", "/some/crate")]);
        for testing in [false, true] {
            assert!(matches!(
                resolve_with(&env, testing),
                Err(PathError::CargoWithoutTestRoot)
            ));
            assert!(matches!(
                home_dir_with(&env, testing),
                Err(PathError::CargoWithoutTestRoot)
            ));
        }
    }

    #[test]
    fn process_env_var_reads_the_real_environment() {
        let real = std::env::var("PATH").expect("cargo sets a UTF-8 PATH");
        assert!(!real.is_empty());
        assert_eq!(ProcessEnv.var("PATH"), Some(real));
        assert_eq!(ProcessEnv.var("UNIDEX_NO_SUCH_VARIABLE_0000"), None);
    }

    #[test]
    fn process_env_var_os_reads_the_real_environment() {
        let real = std::env::var_os("PATH").expect("cargo sets PATH");
        assert!(!real.is_empty());
        assert_eq!(ProcessEnv.var_os("PATH"), Some(real));
        assert_eq!(ProcessEnv.var_os("UNIDEX_NO_SUCH_VARIABLE_0000"), None);
    }

    #[test]
    fn test_env_with_adds_a_variable_and_keeps_the_root() {
        let env = testing::TestEnv::new().with("UNIDEX_SOME_KEY", "some-value");
        assert_eq!(env.var("UNIDEX_SOME_KEY").as_deref(), Some("some-value"));
        assert_eq!(env.var_os("UNIDEX_SOME_KEY"), Some("some-value".into()));
        assert_eq!(
            env.var(TEST_ROOT).map(PathBuf::from).as_deref(),
            Some(env.root())
        );
        assert_eq!(env.var("UNIDEX_OTHER_KEY"), None);
    }

    #[test]
    fn without_the_testing_feature_a_missing_root_allows_what_a_test_build_refuses() {
        let none = map(&[]);
        let elsewhere = udx_test_support::TestRoot::new();
        let path = elsewhere.child("x");
        // An installed app (no test root, no `testing` build): nothing to enforce, nothing refused.
        assert!(check_test_path_with(&none, &path, false).is_ok());
        for kind in ALL_KINDS {
            assert!(os_side_effects_allowed_with(&none, kind, false).is_ok());
        }
        assert_eq!(
            user_var_with(&map(&[("HOME", "/home/user")]), UserVar::Home, false),
            Some(OsString::from("/home/user"))
        );
        // The same Env in a `testing` build, or under cargo, fails closed.
        assert!(matches!(
            check_test_path_with(&none, &path, true),
            Err(PathError::TestBuildWithoutTestRoot)
        ));
        let marker = map(&[
            ("CARGO_MANIFEST_DIR", "/some/crate"),
            (OS_SIDE_EFFECTS, "all"),
        ]);
        for testing in [false, true] {
            assert!(matches!(
                check_test_path_with(&marker, &path, testing),
                Err(PathError::CargoWithoutTestRoot)
            ));
            for kind in ALL_KINDS {
                assert!(matches!(
                    os_side_effects_allowed_with(&marker, kind, testing),
                    Err(PathError::CargoWithoutTestRoot)
                ));
            }
        }
        for kind in ALL_KINDS {
            assert!(matches!(
                os_side_effects_allowed_with(&none, kind, true),
                Err(PathError::TestBuildWithoutTestRoot)
            ));
        }
        assert_eq!(
            user_var_with(&map(&[("HOME", "/home/user")]), UserVar::Home, true),
            None
        );
    }

    const ALL_KINDS: [OsSideEffect; 8] = [
        OsSideEffect::SecretStore,
        OsSideEffect::WakeUp,
        OsSideEffect::WindowsAcl,
        OsSideEffect::AppContainerProfile,
        OsSideEffect::TrustStoreCert,
        OsSideEffect::SandboxEscapeMatrix,
        OsSideEffect::DiskImage,
        OsSideEffect::PackageInstall,
    ];

    #[test]
    fn a_refusal_names_the_kind_the_way_the_variable_spells_it() {
        let root = udx_test_support::TestRoot::new();
        let env = map(&[(TEST_ROOT, root.path().to_str().unwrap())]);
        let e = os_side_effects_allowed_with(&env, OsSideEffect::SecretStore, true).unwrap_err();
        assert!(e.to_string().contains("secret_store"), "{e}");
        assert!(e.to_string().contains(OS_SIDE_EFFECTS), "{e}");
    }
}
