use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use xtask::privacy::{RuleId, Terms, scan_bytes, scan_path_name, scan_text};

fn rules(text: &str) -> Vec<RuleId> {
    scan_text("f.md", text, &Terms::generic())
        .into_iter()
        .map(|f| f.rule)
        .collect()
}

#[test]
fn flags_a_personal_email() {
    let f = scan_text(
        "f.md",
        "first line\nwrite to jane.roe@corp.io",
        &Terms::generic(),
    );
    assert_eq!((f.len(), f[0].rule, f[0].line), (1, RuleId::Email, 2));
}

#[test]
fn allows_look_alike_addresses() {
    for ok in [
        "Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>",
        "12345+someone@users.noreply.github.com",
        "dev@example.com",
        "git@github.com:org/repo.git",
    ] {
        assert!(rules(ok).is_empty(), "{ok}");
    }
}

#[test]
fn flags_home_paths_on_three_platforms() {
    for bad in [
        "/Users/jroe/code/x",
        "/home/jroe/.config/x",
        r"C:\Users\jroe\AppData\x",
    ] {
        assert_eq!(rules(bad), vec![RuleId::HomePath], "{bad}");
    }
}

#[test]
fn allows_runner_and_placeholder_homes() {
    for ok in [
        "/home/runner/work/x",
        "/Users/runner/x",
        "/Users/you/vault/x",
    ] {
        assert!(rules(ok).is_empty(), "{ok}");
    }
}

#[test]
fn flags_private_addresses_but_not_loopback_or_public() {
    for bad in [
        "10.1.2.3",
        "172.20.0.5",
        "192.168.1.10",
        "100.64.0.7",
        "169.254.0.9",
    ] {
        assert_eq!(rules(bad), vec![RuleId::PrivateAddress], "{bad}");
    }
    for ok in ["127.0.0.1", "8.8.8.8", "172.32.0.1", "100.128.0.1"] {
        assert!(rules(ok).is_empty(), "{ok}");
    }
}

#[test]
fn flags_private_hosts_and_session_links() {
    assert_eq!(
        rules("see box.tail0a1b2c.ts.net"),
        vec![RuleId::PrivateHost]
    );
    assert_eq!(
        rules("https://claude.ai/code/session_AbC123"),
        vec![RuleId::SessionLink]
    );
}

#[test]
fn local_terms_match_any_case_and_skip_comments() {
    let t = Terms::generic().with_local("# a comment\n\nAcme Corp\n");
    assert!(t.has_local());
    assert_eq!(
        scan_text("f.md", "we met at ACME corp", &t)[0].rule,
        RuleId::LocalTerm
    );
    assert!(scan_text("f.md", "a comment", &t).is_empty());
}

#[test]
fn a_term_in_a_file_name_is_flagged() {
    let t = Terms::generic().with_local("Acme Corp\nacme-corp\n");
    assert_eq!(
        scan_path_name("docs/acme-corp-notes.md", &t)[0].rule,
        RuleId::LocalTerm
    );
}

#[test]
fn invalid_utf8_is_scanned_without_a_panic() {
    let mut bytes = vec![0xff, 0xfe, 0x00];
    bytes.extend_from_slice(b"/Users/jroe/secret");
    assert_eq!(
        scan_bytes("blob.bin", &bytes, &Terms::generic())[0].rule,
        RuleId::HomePath
    );
}

// ---- command-level tests ----

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
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

fn new_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.name", "Test User"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    dir
}

fn write(dir: &Path, name: &str, body: &str) {
    fs::write(dir.join(name), body).unwrap();
}

fn commit_all(dir: &Path, msg: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--no-verify", "-m", msg]);
}

fn xtask(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("xtask").unwrap();
    cmd.current_dir(dir);
    cmd
}

fn output_text(out: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn clean_repo_exits_zero() {
    let repo = new_repo();
    write(repo.path(), "a.md", "nothing personal here\n");
    commit_all(repo.path(), "add a");
    xtask(repo.path())
        .args(["privacy", "--all"])
        .assert()
        .code(0);
}

#[test]
fn finding_exits_one() {
    let repo = new_repo();
    write(repo.path(), "notes.md", "see /Users/jroe/x\n");
    commit_all(repo.path(), "add notes");
    let out = xtask(repo.path())
        .args(["privacy", "--all"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let text = output_text(&out);
    assert!(text.contains("notes.md"), "{text}");
    assert!(text.contains("HomePath"), "{text}");
}

#[test]
fn no_local_list_is_fine_unless_required() {
    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    commit_all(repo.path(), "add a");
    xtask(repo.path())
        .args(["privacy", "--all"])
        .assert()
        .code(0);
    xtask(repo.path())
        .args(["privacy", "--all", "--require-local"])
        .assert()
        .code(2);
}

#[test]
fn a_tracked_local_list_is_a_finding() {
    let repo = new_repo();
    write(repo.path(), "privacy-terms.local.txt", "Acme Corp\n");
    commit_all(repo.path(), "add list");
    let out = xtask(repo.path())
        .args(["privacy", "--all"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let text = output_text(&out);
    assert!(text.contains("LocalFileTracked"), "{text}");
}

#[test]
fn stdin_mode_scans_a_commit_message() {
    let repo = new_repo();
    xtask(repo.path())
        .args(["privacy", "--stdin", "msg"])
        .write_stdin("fix for jane.roe@corp.io")
        .assert()
        .code(1);
}

#[test]
fn range_mode_scans_commit_messages() {
    let repo = new_repo();
    write(repo.path(), "a.md", "one\n");
    commit_all(repo.path(), "first");
    write(repo.path(), "a.md", "two\n");
    commit_all(repo.path(), "move to host 10.1.2.3");
    xtask(repo.path())
        .args(["privacy", "--range", "HEAD~1..HEAD"])
        .assert()
        .code(1);
}

#[test]
fn allow_file_exempts_one_rule_on_one_path_only() {
    let repo = new_repo();
    let body = "path /Users/jroe/x\nmail jane.roe@corp.io\n";
    write(repo.path(), "a.md", body);
    write(repo.path(), "b.md", body);
    write(
        repo.path(),
        "privacy-allow.toml",
        "[[allow]]\npath = \"a.md\"\nrule = \"HomePath\"\nreason = \"test\"\n",
    );
    commit_all(repo.path(), "add files");
    let out = xtask(repo.path())
        .args(["privacy", "--all", "--json"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let start = stdout.find('[').expect("json array in stdout");
    let end = stdout.rfind(']').expect("json array end");
    let v: serde_json_lite::Value = serde_json_lite::parse(&stdout[start..=end]);
    assert_eq!(v.len, 3, "{stdout}");
}

/// Tiny JSON array counter so the tests need no extra dependency: counts
/// top-level objects in an array and checks each has the four fields.
mod serde_json_lite {
    pub struct Value {
        pub len: usize,
    }
    pub fn parse(s: &str) -> Value {
        let mut depth = 0;
        let mut len = 0;
        let mut in_str = false;
        let mut esc = false;
        for c in s.chars() {
            if in_str {
                if esc {
                    esc = false;
                } else if c == '\\' {
                    esc = true;
                } else if c == '"' {
                    in_str = false;
                }
                continue;
            }
            match c {
                '"' => in_str = true,
                '{' => {
                    depth += 1;
                    if depth == 1 {
                        len += 1;
                    }
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        for field in ["\"path\"", "\"line\"", "\"rule\"", "\"excerpt\""] {
            assert_eq!(s.matches(field).count(), len, "field {field} in {s}");
        }
        Value { len }
    }
}
