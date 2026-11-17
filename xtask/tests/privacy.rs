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

fn empty_gitconfig() -> &'static Path {
    use std::sync::OnceLock;
    static EMPTY: OnceLock<std::path::PathBuf> = OnceLock::new();
    EMPTY.get_or_init(|| {
        let p = std::env::temp_dir().join(format!("xtask-test-gitconfig-{}", std::process::id()));
        fs::write(&p, "").unwrap();
        p
    })
}

/// A test must never read or write a real user's git configuration or hooks.
fn isolated(mut cmd: std::process::Command) -> std::process::Command {
    cmd.env("GIT_CONFIG_GLOBAL", empty_gitconfig())
        .env("GIT_CONFIG_NOSYSTEM", "1");
    cmd
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    let out = isolated(std::process::Command::new("git"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn git(dir: &Path, args: &[&str]) {
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
    cmd.current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", empty_gitconfig())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("CI");
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
        "[[allow]]\npath = \"a.md\"\nrule = \"HomePath\"\nvalues = [\"/Users/jroe\"]\nreason = \"test\"\n",
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
    let v: Vec<serde_json::Value> =
        serde_json::from_str(&stdout[start..=end]).expect("valid json array");
    assert_eq!(v.len(), 3, "{stdout}");
    for f in &v {
        for field in ["path", "line", "rule", "excerpt"] {
            assert!(f.get(field).is_some(), "field {field} in {f}");
        }
    }
}

#[test]
fn allow_file_glob_matches_nested_paths() {
    let repo = new_repo();
    fs::create_dir_all(repo.path().join("fixtures/deep")).unwrap();
    fs::create_dir_all(repo.path().join("other")).unwrap();
    let body = "path /Users/jroe/x\n";
    write(repo.path(), "fixtures/deep/x.md", body);
    write(repo.path(), "other/x.md", body);
    write(
        repo.path(),
        "privacy-allow.toml",
        "[[allow]]\npath = \"fixtures/**\"\nrule = \"HomePath\"\nvalues = [\"/Users/jroe\"]\nreason = \"test\"\n",
    );
    commit_all(repo.path(), "add files");
    let out = xtask(repo.path())
        .args(["privacy", "--all"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let text = output_text(&out);
    assert!(text.contains("other/x.md"), "{text}");
    assert!(!text.contains("fixtures/deep/x.md"), "{text}");
}

#[test]
fn hooks_install_writes_both_hooks() {
    let repo = new_repo();
    xtask(repo.path())
        .args(["hooks", "install"])
        .assert()
        .code(0);
    for name in ["pre-commit", "commit-msg"] {
        let p = repo.path().join(".git/hooks").join(name);
        let body = fs::read_to_string(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(body.contains("xtask privacy"), "{name}: {body}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&p).unwrap().permissions().mode();
            assert!(mode & 0o111 != 0, "{name} not executable");
        }
    }
}

#[test]
fn staged_mode_scans_the_staged_version() {
    let repo = new_repo();
    write(repo.path(), "a.md", "clean\n");
    commit_all(repo.path(), "add a");
    write(repo.path(), "a.md", "see /Users/jroe/x\n");
    git(repo.path(), &["add", "a.md"]);
    write(repo.path(), "a.md", "clean again\n"); // working tree is clean, index is not
    xtask(repo.path())
        .args(["privacy", "--staged"])
        .assert()
        .code(1);
}

#[test]
fn allowing_a_local_term_is_a_usage_error() {
    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    write(
        repo.path(),
        "privacy-allow.toml",
        "[[allow]]\npath = \"a.md\"\nrule = \"LocalTerm\"\nvalues = [\"x\"]\nreason = \"test\"\n",
    );
    commit_all(repo.path(), "add files");
    xtask(repo.path())
        .args(["privacy", "--all"])
        .assert()
        .code(2);
}

#[test]
fn an_allow_entry_without_a_reason_is_a_usage_error() {
    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    write(
        repo.path(),
        "privacy-allow.toml",
        "[[allow]]\npath = \"a.md\"\nrule = \"Email\"\nvalues = [\"x@y.io\"]\nreason = \"\"\n",
    );
    commit_all(repo.path(), "add files");
    xtask(repo.path())
        .args(["privacy", "--all"])
        .assert()
        .code(2);
}

#[test]
fn ci_does_not_echo_a_local_term() {
    let repo = new_repo();
    write(repo.path(), "privacy-terms.local.txt", "Zebracorp\n");
    write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
    write(repo.path(), "a.md", "about zebracorp here\n");
    commit_all(repo.path(), "add a");
    let out = xtask(repo.path())
        .env("CI", "true")
        .args(["privacy", "--all"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let text = output_text(&out).to_lowercase();
    assert!(text.contains("a.md:1"), "{text}");
    assert!(!text.contains("zebracorp"), "{text}");
}

#[test]
fn hooks_install_refuses_to_replace_a_foreign_hook() {
    let repo = new_repo();
    let hooks = repo.path().join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::write(hooks.join("pre-commit"), "#!/bin/sh\necho mine\n").unwrap();
    xtask(repo.path())
        .args(["hooks", "install"])
        .assert()
        .code(1);
    assert_eq!(
        fs::read_to_string(hooks.join("pre-commit")).unwrap(),
        "#!/bin/sh\necho mine\n"
    );
    assert!(!hooks.join("commit-msg").exists());
}

// ---- review round 1 ----

fn run(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = xtask(dir).args(args).output().unwrap();
    (out.status.code().unwrap(), output_text(&out))
}

fn flagged(text: &str) -> bool {
    rules(text).contains(&RuleId::HomePath)
}

// C1
#[test]
fn range_scans_the_files_of_a_root_commit() {
    let repo = new_repo();
    write(repo.path(), "a.md", "see /Users/jroe/x\n");
    commit_all(repo.path(), "first");
    assert_eq!(run(repo.path(), &["privacy", "--range", "HEAD"]).0, 1);
    let sha = git_out(repo.path(), &["rev-parse", "HEAD"]);
    assert_eq!(run(repo.path(), &["privacy", "--range", &sha]).0, 1);
}

// I1
#[cfg(unix)]
#[test]
fn type_change_symlink_to_file_is_scanned() {
    let repo = new_repo();
    std::os::unix::fs::symlink("harmless-target", repo.path().join("cfg")).unwrap();
    commit_all(repo.path(), "link");
    fs::remove_file(repo.path().join("cfg")).unwrap();
    write(repo.path(), "cfg", "home /Users/jroe/x\n");
    git(repo.path(), &["add", "-A"]);
    assert_eq!(run(repo.path(), &["privacy", "--staged"]).0, 1);
    git(repo.path(), &["commit", "-q", "--no-verify", "-m", "file"]);
    assert_eq!(
        run(repo.path(), &["privacy", "--range", "HEAD~1..HEAD"]).0,
        1
    );
}

#[cfg(unix)]
#[test]
fn type_change_file_to_symlink_is_scanned() {
    let repo = new_repo();
    write(repo.path(), "cfg", "harmless\n");
    commit_all(repo.path(), "file");
    fs::remove_file(repo.path().join("cfg")).unwrap();
    std::os::unix::fs::symlink("/Users/jroe/x", repo.path().join("cfg")).unwrap();
    git(repo.path(), &["add", "-A"]);
    assert_eq!(run(repo.path(), &["privacy", "--staged"]).0, 1);
    git(repo.path(), &["commit", "-q", "--no-verify", "-m", "link"]);
    assert_eq!(
        run(repo.path(), &["privacy", "--range", "HEAD~1..HEAD"]).0,
        1
    );
}

// I2 / I3
#[test]
fn home_paths_without_a_trailing_separator_are_flagged() {
    assert!(flagged("HOME=/Users/jroe"));
    assert!(flagged("cd /home/jroe"));
    assert!(flagged(r#""cwd": "/Users/jroe""#));
}

#[test]
fn home_paths_with_any_prefix_are_flagged() {
    for bad in [
        "/mnt/c/Users/jroe/x",
        "/System/Volumes/Data/Users/jroe/x",
        "/var/home/jroe/x",
        "/usr/home/jroe/x",
        "C:/Users/jroe/x",
        "/Users/runner//Users/jroe/x",
    ] {
        assert!(flagged(bad), "{bad}");
    }
}

#[test]
fn home_paths_match_case_insensitively() {
    assert!(flagged("/users/jroe/x"));
    assert!(flagged("/HOME/jroe/x"));
    assert!(flagged(r"c:\users\jroe\x"));
}

#[test]
fn windows_names_may_hold_spaces_and_non_ascii() {
    assert!(flagged(r"C:\Users\Jane Roe\AppData"));
    assert!(flagged(r"C:\Users\Jane Roe"));
    assert!(flagged("/Users/jos\u{e9}/x"));
    assert!(flagged(r"C:\\Users\\jroe\\x"));
}

#[test]
fn more_placeholder_homes_are_allowed() {
    for ok in [
        "/Users/Shared/x",
        r"C:\Users\Public\x",
        r"C:\Users\runneradmin\x",
        r"C:\Users\Default\x",
        "/Users/$USER/x",
        "/home/<name>/x",
        r"C:\Users\%USERNAME%\x",
    ] {
        assert!(!flagged(ok), "{ok}");
    }
}

#[test]
fn urls_are_not_home_paths_but_file_urls_are() {
    assert!(!flagged("https://example.com/users/octocat"));
    assert!(!flagged("see https://example.com/home/page for more"));
    assert!(flagged("file:///Users/jroe/x"));
    assert!(flagged("/users/octocat"));
}

// I4
const SCISSORS: &str = "# ------------------------ >8 ------------------------";

fn git_message_run(dir: &Path, msg: &str, flag: bool) -> i32 {
    let mut args = vec!["privacy", "--stdin", "msg"];
    if flag {
        args.push("--git-message");
    }
    xtask(dir)
        .args(args)
        .write_stdin(msg.to_string())
        .output()
        .unwrap()
        .status
        .code()
        .unwrap()
}

fn verbose_message(above: &str) -> String {
    format!(
        "{above}\n# Please enter the commit message for your changes.\n# Lines starting with '#' will be ignored.\n{SCISSORS}\n# Do not modify or remove the line above.\n# Everything below it will be ignored.\ndiff --git a/x b/x\n+ path /Users/jroe/x\n"
    )
}

#[test]
fn git_message_flag_cuts_a_verbose_diff_after_the_scissors_line() {
    let repo = new_repo();
    let msg = verbose_message("fix a thing");
    assert_eq!(git_message_run(repo.path(), &msg, true), 0);
    assert_eq!(git_message_run(repo.path(), &msg, false), 1);
}

#[test]
fn git_message_flag_still_scans_above_a_verbose_scissors_line() {
    let repo = new_repo();
    let msg = verbose_message("see /Users/jroe/x");
    assert_eq!(git_message_run(repo.path(), &msg, true), 1);
}

#[test]
fn git_message_flag_scans_comment_like_lines() {
    let repo = new_repo();
    assert_eq!(
        git_message_run(repo.path(), "#12 moved to /Users/jroe/x\n", true),
        1
    );
    assert_eq!(
        git_message_run(repo.path(), "subject\n# note /Users/jroe/x\n", true),
        1
    );
}

#[test]
fn git_message_flag_keeps_text_after_a_scissors_line_without_a_diff() {
    let repo = new_repo();
    let msg = format!("subject\n{SCISSORS}\nordinary text /Users/jroe/x\n");
    assert_eq!(git_message_run(repo.path(), &msg, true), 1);
}

#[test]
fn the_commit_msg_hook_passes_git_message() {
    let repo = new_repo();
    xtask(repo.path())
        .args(["hooks", "install"])
        .assert()
        .code(0);
    let body = fs::read_to_string(repo.path().join(".git/hooks/commit-msg")).unwrap();
    assert!(body.contains("--git-message"), "{body}");
}

// I5
#[test]
fn local_terms_match_across_line_breaks_and_extra_whitespace() {
    let t = Terms::generic().with_local("Jane Roe\n");
    for text in [
        "met Jane\nRoe today",
        "met Jane   Roe",
        "met Jane\tRoe",
        "Jane \n  Roe",
    ] {
        let f = scan_text("f.md", text, &t);
        assert_eq!(f.len(), 1, "{text:?}");
        assert_eq!(f[0].rule, RuleId::LocalTerm);
    }
    let f = scan_text("f.md", "x\ny\nJane\nRoe\n", &t);
    assert_eq!(f[0].line, 3);
}

// I6
#[test]
fn a_linked_worktree_uses_the_main_checkouts_local_list() {
    let main = new_repo();
    write(main.path(), ".gitignore", "privacy-terms.local.txt\n");
    write(main.path(), "privacy-terms.local.txt", "Zebracorp\n");
    write(main.path(), "a.md", "about zebracorp\n");
    commit_all(main.path(), "add a");
    let wt = TempDir::new().unwrap();
    let wt_path = wt.path().join("wt");
    git(
        main.path(),
        &["worktree", "add", "-q", wt_path.to_str().unwrap()],
    );
    let (code, text) = run(&wt_path, &["privacy", "--all"]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("LocalTerm"), "{text}");
    assert_eq!(run(&wt_path, &["privacy", "--all", "--require-local"]).0, 1);
}

#[test]
fn a_missing_local_list_is_announced_on_stderr() {
    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    commit_all(repo.path(), "add a");
    let out = xtask(repo.path())
        .args(["privacy", "--all"])
        .assert()
        .code(0)
        .get_output()
        .clone();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("not being checked"), "{err}");
}

#[cfg(unix)]
#[test]
fn an_unreadable_local_list_is_a_usage_error() {
    let repo = new_repo();
    fs::create_dir(repo.path().join("privacy-terms.local.txt")).unwrap();
    write(repo.path(), "a.md", "harmless\n");
    commit_all(repo.path(), "add a");
    assert_eq!(run(repo.path(), &["privacy", "--all"]).0, 2);
}

// I7
fn allow(path: &str, rule: &str, values: &str) -> String {
    format!(
        "[[allow]]\npath = \"{path}\"\nrule = \"{rule}\"\nvalues = [{values}]\nreason = \"test\"\n"
    )
}

#[test]
fn an_allow_entry_without_values_is_a_usage_error() {
    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    write(
        repo.path(),
        "privacy-allow.toml",
        "[[allow]]\npath = \"a.md\"\nrule = \"Email\"\nreason = \"test\"\n",
    );
    commit_all(repo.path(), "add files");
    assert_eq!(run(repo.path(), &["privacy", "--all"]).0, 2);
    write(
        repo.path(),
        "privacy-allow.toml",
        &allow("a.md", "Email", ""),
    );
    assert_eq!(run(repo.path(), &["privacy", "--all"]).0, 2);
}

#[test]
fn only_the_listed_value_is_exempt() {
    let repo = new_repo();
    write(repo.path(), "a.md", "mail jane.roe@corp.io\n");
    write(repo.path(), "b.md", "mail jane.roe@corp.io\n");
    write(
        repo.path(),
        "privacy-allow.toml",
        &allow("a.md", "Email", "\"jane.roe@corp.io\""),
    );
    commit_all(repo.path(), "add files");
    let (code, text) = run(repo.path(), &["privacy", "--all"]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("b.md") && !text.contains("a.md"), "{text}");
    // a different value of the same rule on the same path is flagged
    write(repo.path(), "a.md", "mail other.person@corp.io\n");
    let (code, text) = run(repo.path(), &["privacy", "--all"]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("a.md"), "{text}");
}

#[test]
fn the_allow_files_own_values_do_not_flag_it() {
    let repo = new_repo();
    write(repo.path(), "a.md", "mail jane.roe@corp.io\n");
    write(
        repo.path(),
        "privacy-allow.toml",
        &allow("a.md", "Email", "\"jane.roe@corp.io\""),
    );
    commit_all(repo.path(), "add files");
    let (code, text) = run(repo.path(), &["privacy", "--all"]);
    assert_eq!(code, 0, "{text}");
}

// R-A
#[test]
fn allow_globs_do_not_cross_directories_with_a_single_star() {
    let repo = new_repo();
    fs::create_dir_all(repo.path().join("docs")).unwrap();
    write(repo.path(), "docs/x.md", "mail jane.roe@corp.io\n");
    write(
        repo.path(),
        "privacy-allow.toml",
        &allow("*.md", "Email", "\"jane.roe@corp.io\""),
    );
    commit_all(repo.path(), "add files");
    assert_eq!(run(repo.path(), &["privacy", "--all"]).0, 1);
}

#[test]
fn allow_entries_never_exempt_stdin_or_commit_messages() {
    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    write(
        repo.path(),
        "privacy-allow.toml",
        &allow("*", "Email", "\"jane.roe@corp.io\""),
    );
    commit_all(repo.path(), "contact jane.roe@corp.io");
    xtask(repo.path())
        .args(["privacy", "--stdin", "msg"])
        .write_stdin("contact jane.roe@corp.io")
        .assert()
        .code(1);
    assert_eq!(run(repo.path(), &["privacy", "--range", "HEAD"]).0, 1);
}

// R-C
#[test]
fn retina_asset_names_are_not_emails() {
    let t = Terms::generic();
    assert!(scan_path_name("assets/icon@2x.png", &t).is_empty());
    assert!(scan_text("f.md", "![i](assets/icon@3x.webp)", &t).is_empty());
    assert_eq!(rules("icon@2x.io"), vec![RuleId::Email]);
}

// R-D
#[test]
fn range_scans_author_and_committer_emails() {
    let repo = new_repo();
    write(repo.path(), "a.md", "one\n");
    commit_all(repo.path(), "first");
    assert_eq!(run(repo.path(), &["privacy", "--range", "HEAD"]).0, 0);
    git(repo.path(), &["config", "user.email", "jane.roe@corp.io"]);
    write(repo.path(), "a.md", "two\n");
    commit_all(repo.path(), "second");
    let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD~1..HEAD"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("author") && text.contains("committer"),
        "{text}"
    );
}

// N2
#[test]
fn urls_only_guard_http_matches_inside_the_url() {
    for ok in [
        "https://example.com/users/octocat",
        "https://example.com/home/page",
        "[docs](https://example.com/users/octocat)",
    ] {
        assert!(!flagged(ok), "{ok}");
    }
    for bad in [
        r#"{"url":"https://example.com/a","cwd":"/Users/jroe"}"#,
        "https://example.com/a,/Users/jroe/x",
        r#"<a href="https://example.com/a">x</a><code>/Users/jroe/x</code>"#,
        "file:///Users/jroe/x",
        "vscode://file/Users/jroe/x",
        "ssh://git@example.com/home/jroe/repo.git",
    ] {
        assert!(flagged(bad), "{bad}");
    }
}

// N1
#[test]
fn a_local_term_in_the_allow_file_is_never_exempt() {
    let repo = new_repo();
    write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
    write(repo.path(), "privacy-terms.local.txt", "jane.roe@corp.io\n");
    write(repo.path(), "a.md", "harmless\n");
    write(
        repo.path(),
        "privacy-allow.toml",
        &allow("other.md", "Email", "\"jane.roe@corp.io\""),
    );
    git(repo.path(), &["add", "-A"]);
    for args in [vec!["privacy", "--all"], vec!["privacy", "--staged"]] {
        let (code, text) = run(repo.path(), &args);
        assert_eq!(code, 1, "{args:?}: {text}");
        assert!(
            text.contains("LocalTerm") && text.contains("privacy-allow.toml"),
            "{text}"
        );
    }
    git(repo.path(), &["commit", "-q", "--no-verify", "-m", "add"]);
    assert_eq!(run(repo.path(), &["privacy", "--range", "HEAD"]).0, 1);
}
