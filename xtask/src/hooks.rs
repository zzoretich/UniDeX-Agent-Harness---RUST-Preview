//! `xtask hooks install`: writes the git hooks that run the privacy gate;
//! `xtask hooks pre-push`: what the pre-push hook runs.
//!
//! The installer writes `pre-commit`, `commit-msg` and `pre-push` into this
//! repository's own hooks folder (`<git common dir>/hooks`) and nowhere else.
//! It refuses, before it writes anything, when git's hooks folder for the
//! repository is another one (`core.hooksPath` set in any configuration, or
//! a hooks folder that is a symbolic link elsewhere), and when a hook it would
//! replace was not written by it, cannot be read, is not UTF-8, or is not a
//! regular file (a symbolic link included). With `--require-local` the hooks
//! pass `--require-local` to the gate, so a missing or empty local term list
//! stops the commit or push; without it the gate warns on every run that the
//! local terms are not being checked.
//!
//! The pre-push hook hands git's standard input and the push target's URL to
//! `xtask hooks pre-push`, which reads one line per ref (`<local ref> <local
//! id> <remote ref> <remote id>`) and scans, for each ref that is not being
//! deleted, the commits the push would publish (`push_ranges`). What counts
//! as already published is asked of the push target itself (`git ls-remote`
//! on the URL, after `--end-of-options`); local remote-tracking refs are never
//! trusted. Ref names are never put in a command: the gate is given object
//! ids only, on standard input.

use crate::privacy::{PushRange, git, git_input, is_object_id};
use std::fs;
use std::path::{Path, PathBuf};

/// The first two lines of every hook the installer writes. A hook counts as
/// the installer's own only when it starts with exactly these lines.
const HEADER: &str = "#!/bin/sh\n# xtask-privacy-hook (written by `cargo xtask hooks install`)\n";

const PRELUDE: &str = r#"if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi
top=$(git rev-parse --show-toplevel) || exit 1
cd "$top" || exit 1
if [ ! -f xtask/Cargo.toml ]; then
    echo "privacy gate: no xtask/Cargo.toml in $top, so the gate cannot run; refusing to continue" >&2
    exit 1
fi
"#;

fn script(last_line: &str) -> String {
    format!("{HEADER}{PRELUDE}{last_line}\n")
}

/// The three hooks, each with the line that runs the gate.
fn hooks(require_local: bool) -> [(&'static str, String); 3] {
    let flag = if require_local {
        " --require-local"
    } else {
        ""
    };
    [
        (
            "pre-commit",
            script(&format!("exec cargo xtask privacy --staged{flag}")),
        ),
        (
            "commit-msg",
            script(&format!(
                "exec cargo xtask privacy --stdin commit-message --git-message{flag} < \"$1\""
            )),
        ),
        (
            "pre-push",
            script(&format!(
                "exec cargo xtask hooks pre-push{flag} -- \"$1\" \"$2\""
            )),
        ),
    ]
}

fn git_path(cwd: &Path, args: &[&str]) -> Result<PathBuf, String> {
    let out = git::git(cwd, args).map_err(|e| format!("not inside a git repository: {e}"))?;
    // `--path-format=absolute` makes it absolute.
    Ok(PathBuf::from(
        String::from_utf8_lossy(&out).trim_end_matches(['\n', '\r']),
    ))
}

/// `p` with symbolic links resolved: the whole path when it exists, else its
/// parent's resolution and its last part (a hooks folder that does not exist
/// yet).
fn resolved(p: &Path) -> PathBuf {
    if let Ok(real) = fs::canonicalize(p) {
        return real;
    }
    match (p.parent().map(fs::canonicalize), p.file_name()) {
        (Some(Ok(parent)), Some(name)) => parent.join(name),
        _ => p.to_path_buf(),
    }
}

/// This repository's own hooks folder, after checking that git uses it.
fn hooks_dir(cwd: &Path) -> Result<PathBuf, String> {
    let used = git_path(
        cwd,
        &["rev-parse", "--path-format=absolute", "--git-path", "hooks"],
    )?;
    let common = git_path(
        cwd,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let own = resolved(&common).join("hooks");
    if resolved(&used) != own {
        return Err(format!(
            "git runs this repository's hooks from {}, not from its own hooks folder {}: \
             core.hooksPath is set (in this repository's or your global git configuration) \
             or the hooks folder is a symbolic link. Hooks written there could run for other \
             repositories or land in the working tree, so nothing was written. Unset \
             core.hooksPath for this repository, or set it to {}, and run again",
            used.display(),
            own.display(),
            own.display()
        ));
    }
    Ok(own)
}

/// Refuses to replace `path` unless it is missing or a regular UTF-8 file
/// that this installer wrote (it starts with `HEADER`).
fn check_replaceable(path: &Path) -> Result<(), String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("cannot inspect {}: {e}", path.display())),
    };
    if !meta.is_file() {
        return Err(format!(
            "{} exists and is not a regular file (a symbolic link or a folder); \
             move it aside and run again",
            path.display()
        ));
    }
    let bytes = fs::read(path).map_err(|e| {
        format!(
            "{} exists and cannot be read ({e}); it was left as it is: move it aside and run again",
            path.display()
        )
    })?;
    match String::from_utf8(bytes) {
        Ok(text) if text.starts_with(HEADER) => Ok(()),
        Ok(_) => Err(format!(
            "{} already exists and was not written by xtask; move it aside and run again",
            path.display()
        )),
        Err(_) => Err(format!(
            "{} already exists and is not UTF-8, so it was not written by xtask; \
             move it aside and run again",
            path.display()
        )),
    }
}

/// Writes one hook through a temporary file in the same folder, renamed over
/// the old one, so a half-written hook never exists. Whatever sits at the
/// temporary name (a leftover, a symbolic link) is removed first, not
/// followed, and the file is created new (`create_new` never opens through a
/// link, and fails if anything could not be removed); its mode is set on the
/// open file.
fn write_hook(dir: &Path, name: &str, body: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    let path = dir.join(name);
    let tmp = dir.join(format!(".{name}.xtask-new"));
    let cannot = |e: std::io::Error| format!("cannot write {}: {e}", tmp.display());
    // Any error is left to `create_new`, which fails while anything remains.
    let _ = fs::remove_file(&tmp);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(cannot)?;
    file.write_all(body.as_bytes()).map_err(cannot)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o755))
            .map_err(cannot)?;
    }
    drop(file);
    fs::rename(&tmp, &path).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

/// Installs `pre-commit`, `commit-msg` and `pre-push` (see the module notes).
/// Every check runs before the first write.
pub fn install(require_local: bool) -> Result<Vec<PathBuf>, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("no working directory: {e}"))?;
    let dir = hooks_dir(&cwd)?;
    let hooks = hooks(require_local);
    for (name, _) in &hooks {
        check_replaceable(&dir.join(name))?;
    }
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    hooks
        .iter()
        .map(|(name, body)| write_hook(&dir, name, body))
        .collect()
}

/// One line of the pre-push hook's input.
struct PushLine {
    remote_ref: String,
    local_id: String,
    remote_id: String,
}

fn all_zero(id: &str) -> bool {
    id.bytes().all(|b| b == b'0')
}

/// Parses git's pre-push input. The fields are taken from the right: the
/// remote id, the remote ref and the local id never hold a space, while the
/// local ref is written as the user typed it (`HEAD@{1 minute ago}`). A line
/// without four fields, or whose ids are not object ids, is an error (so the
/// push is refused).
fn parse_push_input(input: &[u8]) -> Result<Vec<PushLine>, String> {
    let mut out = Vec::new();
    for (i, line) in input.split(|b| *b == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let mut fields: Vec<&[u8]> = line.rsplitn(4, |b| *b == b' ').collect();
        fields.reverse();
        let id = |f: &[u8]| {
            std::str::from_utf8(f)
                .ok()
                .filter(|s| is_object_id(s))
                .map(str::to_string)
        };
        let parsed = match fields[..] {
            [_, local, remote_ref, remote] => id(local).zip(id(remote)).map(|(l, r)| PushLine {
                remote_ref: String::from_utf8_lossy(remote_ref).into_owned(),
                local_id: l,
                remote_id: r,
            }),
            _ => None,
        };
        out.push(parsed.ok_or_else(|| {
            format!(
                "line {} of git's input is not \
                 `<local ref> <local id> <remote ref> <remote id>`",
                i + 1
            )
        })?);
    }
    Ok(out)
}

/// The commits the push target lists (`git ls-remote` on `url`, which reaches
/// git after `--end-of-options`, so a URL shaped like an option is still a
/// URL), peeled to commits and kept only when present here. `url` is where
/// git pushes (`pushInsteadOf` already applied); `ls-remote` would apply the
/// fetch rewrites (`insteadOf`) and could list a mirror instead, so when they
/// would rewrite `url` nothing is listed. Empty when there is no URL, it would
/// be rewritten, or the listing fails: then nothing is taken as published.
fn listed_commits(root: &Path, url: Option<&str>) -> Vec<String> {
    let Some(url) = url else {
        return Vec::new();
    };
    let resolved =
        git::git(root, &["ls-remote", "--get-url", "--end-of-options", url]).unwrap_or_default();
    if String::from_utf8_lossy(&resolved).trim_end_matches(['\n', '\r']) != url {
        return Vec::new();
    }
    let Ok(listing) = git::git(root, &["ls-remote", "--end-of-options", url]) else {
        return Vec::new();
    };
    let ids: String = String::from_utf8_lossy(&listing)
        .lines()
        .filter_map(|l| l.split('\t').next())
        .filter(|id| is_object_id(id))
        .map(|id| format!("{id}^{{commit}}\n"))
        .collect();
    // One process for every id: the commit each names, or `missing`.
    let checked = git_input(
        root,
        &["cat-file", "--batch-check=%(objectname) %(objecttype)"],
        ids.as_bytes(),
    )
    .unwrap_or_default();
    let mut out: Vec<String> = String::from_utf8_lossy(&checked)
        .lines()
        .filter_map(|l| l.strip_suffix(" commit"))
        .filter(|id| is_object_id(id))
        .map(str::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Whether `id` names a commit (or a tag of one) present in this repository.
fn has_commit(root: &Path, id: &str) -> bool {
    git::git(root, &["cat-file", "-e", &format!("{id}^{{commit}}")]).is_ok()
}

/// What a push would publish, one range per ref that is not being deleted:
///
/// - a ref the remote already has, at a commit present here: the commits
///   since that commit (`<remote id>..<local id>`);
/// - a new ref, or one whose remote commit is not present here (a forced
///   push over history this repository never fetched): the commits not
///   reachable from any commit the push target lists (`listed_commits`);
///   when it lists none, or cannot be listed, every commit of the ref;
/// - a deleted ref (local id all zeros): nothing.
///
/// Local remote-tracking refs are never used: they can be stale, set by
/// hand, or describe another repository than the push target.
pub fn push_ranges(root: &Path, url: Option<&str>, input: &[u8]) -> Result<Vec<PushRange>, String> {
    let mut listed: Option<Vec<String>> = None;
    let mut out = Vec::new();
    for line in parse_push_input(input)? {
        if all_zero(&line.local_id) {
            continue;
        }
        let exclude = if !all_zero(&line.remote_id) && has_commit(root, &line.remote_id) {
            vec![line.remote_id]
        } else {
            listed
                .get_or_insert_with(|| listed_commits(root, url))
                .clone()
        };
        out.push(PushRange {
            name: line.remote_ref,
            tip: line.local_id,
            exclude,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_input_lines_need_four_fields_and_object_ids() {
        let a = "a".repeat(40);
        let z = "0".repeat(40);
        let ok = format!("refs/heads/x {a} refs/heads/x {z}\n");
        let lines = parse_push_input(ok.as_bytes()).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].remote_ref, "refs/heads/x");
        assert_eq!(
            (lines[0].local_id.as_str(), lines[0].remote_id.as_str()),
            (a.as_str(), z.as_str())
        );
        let long = "b".repeat(64);
        assert!(parse_push_input(format!("r {long} r {long}").as_bytes()).is_ok());
        for bad in [
            format!("r {a} r"),
            format!("r {a} r {z} extra"),
            format!("r {} r {z}", "a".repeat(39)),
            format!("r {} r {z}", "g".repeat(40)),
            format!("r {a} r {}", "a".repeat(41)),
            format!("r {a}  r {z}"),
        ] {
            let e = parse_push_input(bad.as_bytes()).err().unwrap_or_default();
            assert!(e.contains("line 1"), "{bad}: {e}");
        }
        let e = parse_push_input(format!("{ok}r x r y\n").as_bytes())
            .err()
            .unwrap_or_default();
        assert!(e.contains("line 2"), "{e}");
        assert!(parse_push_input(b"").unwrap().is_empty());
        // The local ref may hold spaces; the other three fields never do.
        let spaced = format!("HEAD@{{1 minute ago}} {a} refs/heads/x {z}\n");
        let lines = parse_push_input(spaced.as_bytes()).unwrap();
        assert_eq!(lines[0].remote_ref, "refs/heads/x");
        assert_eq!(lines[0].local_id, a);
    }

    #[test]
    fn a_missing_hooks_folder_resolves_through_its_parent() {
        let root = std::env::var_os("UNIDEX_TEST_ROOT").expect("cargo sets UNIDEX_TEST_ROOT");
        let root = PathBuf::from(root).join("xtask-unit");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        let real = fs::canonicalize(&root).unwrap();
        // A parent spelled with `..` is resolved; the missing last part kept.
        let spelled = root.join("sub").join("..").join("not-yet");
        assert_eq!(resolved(&spelled), real.join("not-yet"));
        assert_eq!(resolved(&root.join("sub").join("..")), real);
        let nowhere = root.join("missing-parent").join("hooks");
        assert_eq!(resolved(&nowhere), nowhere);
    }
}
