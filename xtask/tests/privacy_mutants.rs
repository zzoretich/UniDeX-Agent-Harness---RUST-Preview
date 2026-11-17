//! Tests added after mutation testing: each pins required behaviour that the
//! first suite left unchecked.

use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use xtask::privacy::{RuleId, Terms, scan_path_name, scan_text};

fn empty_gitconfig() -> &'static Path {
    use std::sync::OnceLock;
    static EMPTY: OnceLock<std::path::PathBuf> = OnceLock::new();
    EMPTY.get_or_init(|| {
        let p = std::env::temp_dir().join(format!("xtask-mut-gitconfig-{}", std::process::id()));
        fs::write(&p, "").unwrap();
        p
    })
}

fn isolated(mut cmd: std::process::Command) -> std::process::Command {
    cmd.env("GIT_CONFIG_GLOBAL", empty_gitconfig())
        .env("GIT_CONFIG_NOSYSTEM", "1");
    cmd
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

fn run(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = xtask(dir).args(args).output().unwrap();
    (out.status.code().unwrap(), output_text(&out))
}

fn excerpts(text: &str, rule: RuleId) -> Vec<String> {
    let mut v: Vec<String> = scan_text("f.md", text, &Terms::generic())
        .into_iter()
        .filter(|f| f.rule == rule)
        .map(|f| f.excerpt)
        .collect();
    v.sort();
    v
}

fn allow(path: &str, rule: &str, values: &str) -> String {
    format!(
        "[[allow]]\npath = \"{path}\"\nrule = \"{rule}\"\nvalues = [{values}]\nreason = \"test\"\n"
    )
}

// ---- the allow file ----

#[cfg(unix)]
#[test]
fn an_unreadable_allow_file_is_a_usage_error() {
    let repo = new_repo();
    fs::create_dir(repo.path().join("privacy-allow.toml")).unwrap();
    write(repo.path(), "a.md", "harmless\n");
    commit_all(repo.path(), "add a");
    assert_eq!(run(repo.path(), &["privacy", "--all"]).0, 2);
}

#[test]
fn a_windows_value_declared_in_the_allow_file_does_not_flag_it() {
    // The value reads `C:\Users\jroe`; inside the TOML text its backslashes
    // are doubled, and that text must not be flagged as a finding.
    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    write(
        repo.path(),
        "privacy-allow.toml",
        &allow("other.md", "HomePath", r#""C:\\Users\\jroe""#),
    );
    commit_all(repo.path(), "add files");
    let (code, text) = run(repo.path(), &["privacy", "--all"]);
    assert_eq!(code, 0, "{text}");
}

#[test]
fn a_value_longer_than_an_excerpt_can_still_be_exempted() {
    // Excerpts are cut at 100 characters, so a longer value is matched on
    // its cut form: the exemption applies to the long address.
    let long = format!("{}@corp.io", "a".repeat(120));
    let repo = new_repo();
    write(repo.path(), "a.md", &format!("mail {long}\n"));
    write(
        repo.path(),
        "privacy-allow.toml",
        &allow("a.md", "Email", &format!("\"{long}\"")),
    );
    commit_all(repo.path(), "add files");
    let (code, text) = run(repo.path(), &["privacy", "--all"]);
    assert_eq!(code, 0, "{text}");
    // an address that is not the listed value (it differs early) is flagged
    write(
        repo.path(),
        "a.md",
        &format!("mail {}@corp.io\n", "b".repeat(120)),
    );
    assert_eq!(run(repo.path(), &["privacy", "--all"]).0, 1);
}

// ---- git messages ----

const SCISSORS: &str = "# ------------------------ >8 ------------------------";

fn stdin_code(dir: &Path, msg: &str, git_message: bool) -> i32 {
    let mut args = vec!["privacy", "--stdin", "msg"];
    if git_message {
        args.push("--git-message");
    }
    run_stdin(dir, &args, msg)
}

fn run_stdin(dir: &Path, args: &[&str], input: &str) -> i32 {
    xtask(dir)
        .args(args)
        .write_stdin(input.to_string())
        .output()
        .unwrap()
        .status
        .code()
        .unwrap()
}

#[test]
fn a_scissors_line_right_after_the_subject_still_cuts_the_diff() {
    let repo = new_repo();
    let msg = format!(
        "fix a thing\n{SCISSORS}\n# Do not modify or remove the line above.\ndiff --git a/x b/x\n+ path /Users/jroe/x\n"
    );
    assert_eq!(stdin_code(repo.path(), &msg, true), 0);
    assert_eq!(stdin_code(repo.path(), &msg, false), 1);
}

// ---- running outside a repository ----

// Requirement (controller ruling): the file-scanning modes need a work tree
// and exit 2 without one; `--stdin` needs no repository.
#[test]
fn stdin_needs_no_repository() {
    let dir = TempDir::new().unwrap();
    let ceiling = dir.path().parent().unwrap().to_path_buf();
    let mut clean = xtask(dir.path());
    clean
        .env("GIT_CEILING_DIRECTORIES", &ceiling)
        .args(["privacy", "--stdin", "msg"])
        .write_stdin("nothing personal\n")
        .assert()
        .code(0);
    let mut dirty = xtask(dir.path());
    dirty
        .env("GIT_CEILING_DIRECTORIES", &ceiling)
        .args(["privacy", "--stdin", "msg"])
        .write_stdin("see /Users/jroe/x\n")
        .assert()
        .code(1);
}

#[test]
fn scanning_files_outside_a_repository_is_an_environment_error() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.md", "harmless\n");
    let ceiling = dir.path().parent().unwrap().to_path_buf();
    for mode in [vec!["--all"], vec!["--staged"], vec!["--range", "HEAD"]] {
        let mut args = vec!["privacy"];
        args.extend(mode.iter().copied());
        let out = xtask(dir.path())
            .env("GIT_CEILING_DIRECTORIES", &ceiling)
            .args(&args)
            .output()
            .unwrap();
        let text = output_text(&out);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {text}");
    }
}

#[test]
fn a_bare_repository_is_an_environment_error() {
    let clone = new_repo();
    write(clone.path(), "a.md", "harmless\n");
    commit_all(clone.path(), "add a");
    let bare = TempDir::new().unwrap();
    git(
        bare.path(),
        &["init", "--bare", "-q", "--initial-branch=main"],
    );
    git(
        clone.path(),
        &[
            "push",
            "-q",
            bare.path().to_str().unwrap(),
            "HEAD:refs/heads/main",
        ],
    );
    let (code, text) = run(bare.path(), &["privacy", "--range", "HEAD"]);
    assert_eq!(code, 2, "{text}");
}

// ---- what a report shows ----

#[test]
fn a_finding_shows_its_excerpt_in_text_and_json() {
    let repo = new_repo();
    write(repo.path(), "notes.md", "see /Users/jroe/x\n");
    commit_all(repo.path(), "add notes");
    let (code, text) = run(repo.path(), &["privacy", "--all"]);
    assert_eq!(code, 1);
    for part in ["notes.md", "HomePath", "/Users/jroe"] {
        assert!(text.contains(part), "{part}: {text}");
    }
    let out = xtask(repo.path())
        .args(["privacy", "--all", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Vec<serde_json::Value> = serde_json::from_str(&stdout).expect("json array");
    assert_eq!(v.len(), 1, "{stdout}");
    assert_eq!(v[0]["excerpt"], "/Users/jroe");
}

#[test]
fn in_ci_a_generic_finding_still_shows_its_excerpt() {
    let repo = new_repo();
    write(repo.path(), "notes.md", "see /Users/jroe/x\n");
    commit_all(repo.path(), "add notes");
    let out = xtask(repo.path())
        .env("CI", "true")
        .args(["privacy", "--all"])
        .output()
        .unwrap();
    let text = output_text(&out);
    for part in ["notes.md", "HomePath", "/Users/jroe"] {
        assert!(text.contains(part), "{part}: {text}");
    }
}

// ---- the local term list in the repository's view ----

fn local_list_repo(ignored: bool) -> TempDir {
    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    if ignored {
        write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
    }
    commit_all(repo.path(), "add a");
    write(repo.path(), "privacy-terms.local.txt", "Zebracorp\n");
    repo
}

#[test]
fn an_ignored_untracked_local_list_is_fine() {
    let repo = local_list_repo(true);
    for args in [["privacy", "--all"], ["privacy", "--staged"]] {
        let (code, text) = run(repo.path(), &args);
        assert_eq!(code, 0, "{args:?}: {text}");
    }
}

#[test]
fn an_untracked_local_list_that_is_not_ignored_is_a_finding() {
    let repo = local_list_repo(false);
    for args in [["privacy", "--all"], ["privacy", "--staged"]] {
        let (code, text) = run(repo.path(), &args);
        assert_eq!(code, 1, "{args:?}: {text}");
        assert!(text.contains("LocalFileTracked"), "{text}");
    }
}

// ---- submodule entries ----

#[test]
fn a_submodule_entry_is_not_read_as_a_file() {
    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    commit_all(repo.path(), "add a");
    git(
        repo.path(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000,1111111111111111111111111111111111111111,sub",
        ],
    );
    let (code, text) = run(repo.path(), &["privacy", "--staged"]);
    assert_eq!(code, 0, "{text}");
    git(repo.path(), &["commit", "-q", "--no-verify", "-m", "sub"]);
    let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD"]);
    assert_eq!(code, 0, "{text}");
}

// ---- git output that is not a raw entry ----

#[cfg(unix)]
#[test]
fn a_malformed_diff_entry_from_git_is_an_error_not_a_skip() {
    use std::os::unix::fs::PermissionsExt;
    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    commit_all(repo.path(), "add a");
    let real = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|d| d.join("git"))
        .find(|p| p.is_file())
        .expect("a git on PATH");
    let shim = TempDir::new().unwrap();
    // `git diff` prints an entry header with only four fields.
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = diff ]; then printf ':100644 100644 0000 0000\\0a.md\\0'; exit 0; fi\nexec '{}' \"$@\"\n",
        real.display()
    );
    let shim_git = shim.path().join("git");
    fs::write(&shim_git, script).unwrap();
    fs::set_permissions(&shim_git, fs::Permissions::from_mode(0o755)).unwrap();
    let mut path = std::ffi::OsString::from(shim.path());
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap());
    let out = xtask(repo.path())
        .env("PATH", path)
        .args(["privacy", "--staged"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", output_text(&out));
}

// ---- home path details ----

#[test]
fn a_path_right_after_a_url_ending_delimiter_is_not_part_of_the_url() {
    for bad in [
        ",/Users/jroe/x",
        " /Users/jroe/x",
        "\u{3000}/Users/jroe/x",
        "https://example.com/a\u{3000}/Users/jroe/x",
        "https://example.com/a\u{a0}/Users/jroe/x",
    ] {
        assert_eq!(excerpts(bad, RuleId::HomePath), ["/Users/jroe"], "{bad:?}");
    }
}

#[test]
fn names_made_only_of_dots_are_not_home_paths() {
    for ok in ["/Users/../x", "/home/.../x", "cd /Users/.."] {
        assert!(excerpts(ok, RuleId::HomePath).is_empty(), "{ok}");
    }
    assert_eq!(
        excerpts("/Users/.. then /Users/jroe/x", RuleId::HomePath),
        ["/Users/jroe"]
    );
}

#[test]
fn a_windows_name_with_a_space_is_reported_once() {
    assert_eq!(
        excerpts(r"C:\Users\Jane Roe\AppData", RuleId::HomePath),
        [r"C:\Users\Jane Roe"]
    );
}

#[test]
fn a_plain_windows_path_before_a_spaced_one_is_still_reported() {
    assert_eq!(
        excerpts(r"C:\Users\jroe and C:\Users\Jane Roe\x", RuleId::HomePath),
        [r"C:\Users\Jane Roe", r"C:\Users\jroe"]
    );
}

// ---- private addresses ----

#[test]
fn a_private_address_with_padded_parts_is_flagged() {
    // Vectors are built at run time from their parts.
    let join = |p: [&str; 4]| p.join(".");
    for private in [
        join(["010", "001", "002", "003"]),
        join(["10", "1", "2", "0003"]),
        join(["0192", "168", "1", "10"]),
        join(["172", "020", "0", "5"]),
        join(["100", "064", "000", "007"]),
        join(["169", "254", "00", "009"]),
    ] {
        let found = excerpts(&format!("host {private} here"), RuleId::PrivateAddress);
        assert_eq!(found.as_slice(), [private.as_str()], "{private}");
    }
    for clean in [
        join(["008", "008", "008", "008"]),
        join(["0008", "8", "8", "8"]),
        join(["10", "1", "2", "300"]),
        join(["10", "1", "2", "0300"]),
        join(["010", "001", "002", "256"]),
    ] {
        assert!(
            excerpts(&format!("host {clean} here"), RuleId::PrivateAddress).is_empty(),
            "{clean}"
        );
    }
}

// ---- local terms ----

#[test]
fn blank_lines_and_comments_are_not_terms() {
    assert!(!Terms::generic().with_local("\n  \n\t\n").has_local());
    let t = Terms::generic().with_local("# Zebracorp\n");
    assert!(!t.has_local());
    assert!(scan_text("f.md", "# Zebracorp", &t).is_empty());
    assert!(scan_text("f.md", "Zebracorp", &t).is_empty());
}

#[test]
fn separator_variants_of_a_local_term_are_flagged_everywhere() {
    let t = Terms::generic().with_local("Acme Corp\n");
    for form in [
        "acme-corp",
        "acme_corp",
        "acmecorp",
        "ACME-CORP",
        "Acme_Corp",
        "AcmeCorp",
        "ACMECORP",
    ] {
        let f = scan_text("f.md", &format!("see {form} here"), &t);
        assert_eq!(f.len(), 1, "text {form}");
        assert_eq!(f[0].rule, RuleId::LocalTerm, "text {form}");
        let f = scan_path_name(&format!("docs/{form}.md"), &t);
        assert_eq!(f.len(), 1, "name {form}");
        assert_eq!(f[0].rule, RuleId::LocalTerm, "name {form}");
    }
    assert_eq!(scan_text("f.md", "met Acme  Corp", &t).len(), 1);
    // words that merely contain one of the two words are not the term
    for text in ["acmeville", "corporate", "acme and corporate"] {
        assert!(scan_text("f.md", text, &t).is_empty(), "text {text}");
        assert!(
            scan_path_name(&format!("{text}.md"), &t).is_empty(),
            "{text}"
        );
    }
    // a single-word term has no variants
    let single = Terms::generic().with_local("Zebracorp\n");
    assert_eq!(scan_text("f.md", "zebracorp", &single).len(), 1);
    for text in ["zebra-corp", "zebra_corp", "zebra corp"] {
        assert!(scan_text("f.md", text, &single).is_empty(), "text {text}");
        assert!(
            scan_path_name(&format!("{text}.md"), &single).is_empty(),
            "{text}"
        );
    }
}
