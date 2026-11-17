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

/// Tracked files plus untracked files that are not ignored.
pub fn list_all(root: &Path) -> Result<Vec<String>> {
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
    let mut v: Vec<String> = split_nul(&out).collect();
    v.sort();
    v.dedup();
    Ok(v)
}

/// A file as recorded in a commit or the index.
#[derive(Debug, Clone)]
pub struct Blob {
    pub path: String,
    pub oid: String,
}

/// Parses `git diff --raw -z --no-renames` output, keeping blobs (not submodules).
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
        if f[1] == "160000" || f[3].chars().all(|c| c == '0') {
            continue;
        }
        out.push(Blob {
            path,
            oid: f[3].to_string(),
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

/// Commits in `range`, oldest first.
pub fn commits(root: &Path, range: &str) -> Result<Vec<CommitInfo>> {
    let out = git(root, &["rev-list", "--parents", "--reverse", range, "--"])?;
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

/// A commit's author and committer email addresses.
pub fn commit_identities(root: &Path, sha: &str) -> Result<(String, String)> {
    let out = git(root, &["log", "-1", "--format=%ae%n%ce", sha, "--"])?;
    let s = String::from_utf8_lossy(&out);
    let mut it = s.lines();
    Ok((
        it.next().unwrap_or("").to_string(),
        it.next().unwrap_or("").to_string(),
    ))
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
