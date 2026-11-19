//! Helpers shared by the task 1d test files (`secrets.rs`, `hooks.rs`).
//!
//! The older helpers in `privacy.rs` are private to that file; these follow the
//! same pattern (an isolated git configuration, the test root, the binary under
//! test) so a test never touches a real user's git configuration or hooks.
//!
//! Every key-shaped vector is assembled at run time from parts that are each
//! too short to match a secret rule, so no key-shaped literal is committed.
#![allow(dead_code)]

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub fn test_root() -> PathBuf {
    let root = std::env::var_os("UNIDEX_TEST_ROOT").expect("cargo sets UNIDEX_TEST_ROOT");
    let dir = PathBuf::from(root).join("xtask-tests");
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn empty_gitconfig() -> PathBuf {
    let p = test_root().join("xtask-test-gitconfig");
    fs::write(&p, "").unwrap();
    p
}

pub fn isolated(mut cmd: std::process::Command) -> std::process::Command {
    cmd.env("GIT_CONFIG_GLOBAL", empty_gitconfig())
        .env("GIT_CONFIG_NOSYSTEM", "1");
    cmd
}

pub fn git(dir: &Path, args: &[&str]) {
    let out = isolated(std::process::Command::new("git"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

pub fn git_out(dir: &Path, args: &[&str]) -> String {
    let out = isolated(std::process::Command::new("git"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub fn temp_dir() -> TempDir {
    TempDir::new_in(test_root()).unwrap()
}

/// A repository on a branch named `main`, with a commit identity and no signing.
pub fn new_repo() -> TempDir {
    let dir = temp_dir();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.name", "Test User"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    dir
}

/// A bare repository standing in for the hosting service. Nothing is contacted.
pub fn new_bare() -> TempDir {
    let dir = temp_dir();
    git(dir.path(), &["init", "-q", "--bare", "-b", "main"]);
    dir
}

pub fn write(dir: &Path, name: &str, body: &str) {
    if let Some(parent) = Path::new(name).parent() {
        fs::create_dir_all(dir.join(parent)).unwrap();
    }
    fs::write(dir.join(name), body).unwrap();
}

/// Commits everything, bypassing this temporary repository's own hooks.
pub fn commit_all(dir: &Path, msg: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--no-verify", "-m", msg]);
}

pub fn xtask(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("xtask").unwrap();
    cmd.current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", empty_gitconfig())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("CI");
    cmd
}

pub fn output_text(out: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Runs the gate with `args`; returns its exit code and stdout and stderr together.
pub fn run(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = xtask(dir).args(args).output().unwrap();
    (out.status.code().unwrap(), output_text(&out))
}

/// A directory holding a stand-in `cargo` that runs the `xtask` binary under
/// test, and the PATH value that puts it first. The installed hooks run
/// `cargo xtask ...`; this keeps them inside the test.
#[cfg(unix)]
pub fn stand_in_cargo() -> (TempDir, std::ffi::OsString) {
    use std::os::unix::fs::PermissionsExt;
    let bin = temp_dir();
    let cargo = bin.path().join("cargo");
    fs::write(
        &cargo,
        format!(
            "#!/bin/sh\n[ \"$1\" = xtask ] || exit 97\nshift\nexec '{}' \"$@\"\n",
            env!("CARGO_BIN_EXE_xtask")
        ),
    )
    .unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    let mut path = vec![bin.path().to_path_buf()];
    path.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let path = std::env::join_paths(path).unwrap();
    (bin, path)
}

/// Pseudo-random text from `alphabet`, fixed by `seed`, generated at run time.
pub fn body(seed: u64, len: usize, alphabet: &str) -> String {
    let chars: Vec<char> = alphabet.chars().collect();
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            let r = x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33;
            chars[(r as usize) % chars.len()]
        })
        .collect()
}

pub const ALNUM: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
pub const UPPER_DIGITS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
pub const URLSAFE: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
pub const BASE64: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+/";

/// One secret-shaped vector: the text to commit and the part that must never
/// be printed in full.
pub struct Vector {
    pub name: &'static str,
    /// The line (or lines, for a key block) written at line 3 of a file.
    pub text: String,
    /// The secret part: the tool's output must not contain any 12-character
    /// stretch of it.
    pub hidden: String,
}

/// One vector per pattern family, every one built at run time from short parts.
pub fn vectors() -> Vec<Vector> {
    let mut v = Vec::new();
    let b = body(1, 86, URLSAFE);
    v.push(Vector {
        name: "anthropic key",
        text: format!("key = \"{}{}{}\"", "sk-", "ant-api03-", b),
        hidden: b,
    });
    let b = body(2, 56, URLSAFE);
    v.push(Vector {
        name: "openai project key",
        text: format!("OPENAI_KEY={}{}{}", "sk-", "proj-", b),
        hidden: b,
    });
    let b = body(3, 48, ALNUM);
    v.push(Vector {
        name: "openai legacy key",
        text: format!("OPENAI_KEY={}{}", "sk-", b),
        hidden: b,
    });
    let b = body(4, 16, UPPER_DIGITS);
    v.push(Vector {
        name: "cloud access key",
        text: format!("aws_access_key_id = {}{}", "AKIA", b),
        hidden: b,
    });
    let b = body(5, 16, UPPER_DIGITS);
    v.push(Vector {
        name: "cloud temporary access key",
        text: format!("aws_access_key_id = {}{}", "ASIA", b),
        hidden: b,
    });
    for (i, (name, prefix)) in [
        ("hosting personal token", "ghp_"),
        ("hosting oauth token", "gho_"),
        ("hosting server token", "ghs_"),
    ]
    .into_iter()
    .enumerate()
    {
        let b = body(10 + i as u64, 36, ALNUM);
        v.push(Vector {
            name,
            text: format!("token: {prefix}{b}"),
            hidden: b,
        });
    }
    let (b1, b2) = (body(20, 22, ALNUM), body(21, 59, ALNUM));
    v.push(Vector {
        name: "hosting fine-grained token",
        text: format!("token: {}{}{}_{}", "github_", "pat_", b1, b2),
        hidden: format!("{b1}{b2}"),
    });
    let b = body(30, 64, BASE64);
    v.push(Vector {
        name: "private key block",
        text: format!(
            "{}{}\n{}\n{}{}",
            "-----BEGIN ", "OPENSSH PRIVATE KEY-----", b, "-----END ", "OPENSSH PRIVATE KEY-----"
        ),
        hidden: b,
    });
    let b = body(31, 64, BASE64);
    v.push(Vector {
        name: "rsa private key block",
        text: format!(
            "{}{}\n{}\n{}{}",
            "-----BEGIN ", "RSA PRIVATE KEY-----", b, "-----END ", "RSA PRIVATE KEY-----"
        ),
        hidden: b,
    });
    let b = body(40, 40, ALNUM);
    v.push(Vector {
        name: "bearer authorization value",
        text: format!("{}{}{}", "Authorization: ", "Bearer ", b),
        hidden: b,
    });
    v
}

/// A file whose line 3 is `text` (which may span lines).
pub fn file_with(text: &str) -> String {
    format!("first line\nsecond line\n{text}\nlast line\n")
}

/// A minimal `xtask/Cargo.toml` so the installed hooks' prelude lets them run.
pub fn add_stand_in_manifest(dir: &Path) {
    write(dir, "xtask/Cargo.toml", "[package]\nname = \"stand-in\"\n");
}
