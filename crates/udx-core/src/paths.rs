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
        std::env::var(key).ok()
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
}

impl AppPaths {
    /// Decide the folders from `env`. Reads no folder contents and creates nothing.
    pub fn resolve(env: &dyn Env) -> Result<AppPaths, PathError> {
        let get = |key: &str| env.var(key).filter(|v| !v.is_empty());
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

/// Resolve an absolute path without creating anything: symlinks of the longest
/// existing ancestor are followed, a dangling symlink is followed to its
/// target, and the remaining components are applied lexically (`..` pops one
/// component, `.` is dropped). `None` means it could not be resolved safely.
fn resolve_path(path: &Path) -> Option<PathBuf> {
    resolve_hops(path, MAX_LINK_HOPS)
}

fn resolve_hops(path: &Path, hops: u32) -> Option<PathBuf> {
    let mut cur = path.to_path_buf();
    let mut tail: Vec<OsString> = Vec::new(); // innermost component first
    let mut base = loop {
        if let Ok(real) = std::fs::canonicalize(&cur) {
            break real;
        }
        if std::fs::symlink_metadata(&cur).is_ok_and(|m| m.file_type().is_symlink()) {
            // A dangling link: continue from where it points.
            let target = std::fs::read_link(&cur).ok()?;
            let mut next = cur.parent()?.join(target);
            for part in tail.iter().rev() {
                next.push(part);
            }
            return resolve_hops(&next, hops.checked_sub(1)?);
        }
        match cur.components().next_back() {
            Some(Component::Normal(n)) => tail.push(n.to_os_string()),
            Some(Component::ParentDir) => tail.push(OsString::from("..")),
            Some(Component::CurDir) => {}
            // Nothing left to pop: no ancestor exists, use what is left.
            _ => break cur,
        }
        cur.pop();
    };
    for part in tail.iter().rev() {
        if part == ".." {
            base.pop();
        } else {
            base.push(part);
        }
    }
    Some(base)
}
