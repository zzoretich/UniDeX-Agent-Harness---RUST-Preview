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

/// How many symlinks one lookup may follow before it is refused.
const MAX_LINK_HOPS: u32 = 40;

/// A source of environment variables, so tests need not touch the real one.
pub trait Env {
    /// The value of `key`, if it is set.
    fn var(&self, key: &str) -> Option<String>;
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
}

impl AppPaths {
    /// Decide the folders from `env`. Reads no folder contents and creates nothing.
    pub fn resolve(env: &dyn Env) -> Result<AppPaths, PathError> {
        let get = |key: &str| env.var(key).filter(|v| !v.is_empty());
        // Cargo sets CARGO_MANIFEST_DIR for everything it runs; such a process
        // must have a test root and never falls through to the OS folders.
        if get(TEST_ROOT).is_none() && get("CARGO_MANIFEST_DIR").is_some() {
            return Err(PathError::CargoWithoutTestRoot);
        }
        let over = [get(CONFIG_DIR), get(DATA_DIR), get(CACHE_DIR)];

        let [config_dir, data_dir, cache_dir] = match get(TEST_ROOT) {
            Some(root) => {
                let root = std::path::absolute(&root).map_err(|_| PathError::NoHome)?;
                let real_root = resolve_path(&root).ok_or_else(|| outside(&root, &root))?;
                let pick = |over: Option<String>, default: &str| {
                    let dir = match over {
                        Some(o) => std::path::absolute(o).map_err(|_| PathError::NoHome)?,
                        None => real_root.join(default),
                    };
                    let real = resolve_path(&dir).ok_or_else(|| outside(&dir, &real_root))?;
                    if real.starts_with(&real_root) {
                        Ok(real)
                    } else {
                        Err(outside(&real, &real_root))
                    }
                };
                let [c, d, k] = over;
                [pick(c, "config")?, pick(d, "data")?, pick(k, "cache")?]
            }
            None => {
                let [c, d, k] = over;
                let os = || directories::ProjectDirs::from("", "", "unidex-preview");
                let pick =
                    |over: Option<String>, os_dir: fn(&directories::ProjectDirs) -> &Path| {
                        match over {
                            Some(o) => std::path::absolute(o).map_err(|_| PathError::NoHome),
                            None => os()
                                .map(|p| os_dir(&p).to_path_buf())
                                .ok_or(PathError::NoHome),
                        }
                    };
                [
                    pick(c, directories::ProjectDirs::config_dir)?,
                    pick(d, directories::ProjectDirs::data_dir)?,
                    pick(k, directories::ProjectDirs::cache_dir)?,
                ]
            }
        };
        Ok(AppPaths {
            config_dir,
            data_dir,
            cache_dir,
        })
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
