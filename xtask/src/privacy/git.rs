//! Thin wrappers over the `git` command line.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub type Result<T> = std::result::Result<T, String>;

/// Runs git in `dir` and returns stdout bytes, or an error holding stderr.
pub fn git(dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn success(dir: &Path, args: &[&str]) -> Result<bool> {
    let st = Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("cannot run git: {e}"))?;
    Ok(st.success())
}

pub fn repo_root(dir: &Path) -> Result<PathBuf> {
    let out = git(dir, &["rev-parse", "--show-toplevel"])?;
    let s = String::from_utf8_lossy(&out);
    Ok(PathBuf::from(s.trim_end_matches(['\n', '\r'])))
}

fn split_nul(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
}

/// A path as git lists it: the name to scan and report (invalid UTF-8
/// decoded lossily) and the real path to open, which keeps every byte.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub file: PathBuf,
}

fn os_path(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
    }
    // Windows file names are UTF-16, so git hands over valid UTF-8 there.
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// Tracked files plus untracked files that are not ignored.
pub fn list_all(root: &Path) -> Result<Vec<Entry>> {
    let out = git(
        root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )?;
    let mut v: Vec<&[u8]> = out.split(|b| *b == 0).filter(|s| !s.is_empty()).collect();
    v.sort();
    v.dedup();
    Ok(v.into_iter()
        .map(|b| Entry {
            name: String::from_utf8_lossy(b).into_owned(),
            file: os_path(b),
        })
        .collect())
}

/// A file as recorded in a commit or the index.
#[derive(Debug, Clone)]
pub struct Blob {
    pub path: String,
    pub oid: String,
    /// A submodule entry: its path name is scanned, there is no content.
    pub submodule: bool,
}

/// Parses `git diff --raw -z --no-renames` output, keeping blobs and submodules.
/// A token that is not a raw entry is an error, never silently skipped.
fn parse_raw(bytes: &[u8]) -> Result<Vec<Blob>> {
    let mut toks = split_nul(bytes);
    let mut out = Vec::new();
    while let Some(meta) = toks.next() {
        if !meta.starts_with(':') {
            return Err(format!("unexpected git diff output: {meta:?}"));
        }
        let path = toks
            .next()
            .ok_or_else(|| "git diff output ended after an entry header".to_string())?;
        // ":<old mode> <new mode> <old oid> <new oid> <status>"
        let f: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
        if f.len() < 5 {
            return Err(format!("unexpected git diff entry: {meta:?}"));
        }
        if f[3].chars().all(|c| c == '0') {
            continue;
        }
        out.push(Blob {
            path,
            oid: f[3].to_string(),
            submodule: f[1] == "160000",
        });
    }
    Ok(out)
}

/// Added, copied, modified or type-changed files in the index.
pub fn staged(root: &Path) -> Result<Vec<Blob>> {
    let out = git(
        root,
        &[
            "diff",
            "--cached",
            "--raw",
            "-z",
            "--no-renames",
            "--diff-filter=ACMT",
        ],
    )?;
    parse_raw(&out)
}

pub struct CommitInfo {
    pub sha: String,
    pub parents: Vec<String>,
}

/// Commits in `range`, oldest first. The range comes after
/// `--end-of-options`, so git never reads it as an option (the caller also
/// refuses one that begins with `-`).
pub fn commits(root: &Path, range: &str) -> Result<Vec<CommitInfo>> {
    let out = git(
        root,
        &[
            "rev-list",
            "--parents",
            "--reverse",
            "--end-of-options",
            range,
            "--",
        ],
    )?;
    Ok(String::from_utf8_lossy(&out)
        .lines()
        .filter_map(|l| {
            let mut it = l.split(' ');
            let sha = it.next()?.to_string();
            Some(CommitInfo {
                sha,
                parents: it.map(str::to_string).collect(),
            })
        })
        .collect())
}

pub fn commit_message(root: &Path, sha: &str) -> Result<Vec<u8>> {
    git(root, &["log", "-1", "--format=%B", sha, "--"])
}

/// Files a commit adds or changes, against its first parent (or the empty tree).
pub fn commit_blobs(root: &Path, c: &CommitInfo) -> Result<Vec<Blob>> {
    let mut args = vec![
        "diff-tree",
        "--no-commit-id",
        "-r",
        "--raw",
        "-z",
        "--no-renames",
        "--diff-filter=ACMT",
    ];
    match c.parents.first() {
        Some(p) => {
            args.push(p);
            args.push(&c.sha);
        }
        None => {
            args.push("--root");
            args.push(&c.sha);
        }
    }
    parse_raw(&git(root, &args)?)
}

pub fn read_blob(root: &Path, oid: &str) -> Result<Vec<u8>> {
    git(root, &["cat-file", "blob", oid])
}

/// Why the local term list must not exist in the repository's view, if it is wrong.
pub fn local_list_problem(root: &Path, name: &str) -> Result<Option<&'static str>> {
    if success(root, &["ls-files", "--error-unmatch", "--", name])? {
        return Ok(Some("the local term list is tracked by git"));
    }
    if root.join(name).exists() && !success(root, &["check-ignore", "-q", "--", name])? {
        return Ok(Some("the local term list is not ignored by git"));
    }
    Ok(None)
}

/// One author or committer line of a commit, as raw bytes.
pub struct Identity {
    /// "author" or "committer".
    pub who: &'static str,
    /// The whole line after `author ` or `committer `: name, address, time.
    pub line: Vec<u8>,
}

/// A commit's author and committer identities, read from the raw commit
/// object so that no byte in them (a NUL, a control character, invalid
/// UTF-8) can make a field come out empty. Each identity is taken whole
/// (`name <email> time zone`, or whatever the line holds), so an address in
/// the name and a second bracketed part are checked too.
pub fn commit_identities(root: &Path, sha: &str) -> Result<Vec<Identity>> {
    let raw = git(root, &["cat-file", "commit", sha])?;
    let mut out = Vec::new();
    // The header ends at the first empty line; continuation lines (a
    // signature, an embedded tag) begin with a space and are skipped.
    for line in raw.split(|b| *b == b'\n') {
        if line.is_empty() {
            break;
        }
        let (who, rest) = if let Some(rest) = line.strip_prefix(b"author ") {
            ("author", rest)
        } else if let Some(rest) = line.strip_prefix(b"committer ") {
            ("committer", rest)
        } else {
            continue;
        };
        out.push(Identity {
            who,
            line: rest.to_vec(),
        });
    }
    Ok(out)
}

/// The commit name configured for the repository at `root`, as
/// `git config --get user.name` resolves it there (local configuration
/// first, then the user's global one). `None` when no name is set or git
/// fails. The caller never prints it.
pub fn configured_user_name(root: &Path) -> Option<String> {
    let out = git(root, &["config", "--get", "user.name"]).ok()?;
    let name = String::from_utf8_lossy(&out)
        .trim_end_matches(['\n', '\r'])
        .to_string();
    (!name.is_empty()).then_some(name)
}

/// The top level of the main worktree, when `root` is a linked worktree.
pub fn main_worktree_root(root: &Path) -> Option<PathBuf> {
    let out = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .ok()?;
    let common = PathBuf::from(String::from_utf8_lossy(&out).trim_end_matches(['\n', '\r']));
    if common.file_name()? == ".git" {
        common.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::os_path;
    use std::os::unix::ffi::OsStrExt;

    #[test]
    fn a_raw_path_keeps_every_byte() {
        // The lossy name would turn the 0xFF byte into U+FFFD and name a file
        // that does not exist; the real path must keep it.
        let p = os_path(b"dir/a\xffb.md");
        assert_eq!(p.as_os_str().as_bytes(), b"dir/a\xffb.md");
        assert_eq!(os_path(b"plain.md").as_os_str().as_bytes(), b"plain.md");
    }
}
