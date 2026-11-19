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
        // Under the test root cargo sets, never the OS temp folder.
        let root = std::env::var_os("UNIDEX_TEST_ROOT").expect("cargo sets UNIDEX_TEST_ROOT");
        let dir = std::path::PathBuf::from(root).join("xtask-tests");
        fs::create_dir_all(&dir).unwrap();
        // One fixed name, so a file is not left behind for every run.
        let p = dir.join("xtask-mut-gitconfig");
        fs::write(&p, "").unwrap();
        p
    })
}

fn isolated(mut cmd: std::process::Command) -> std::process::Command {
    cmd.env("GIT_CONFIG_GLOBAL", empty_gitconfig())
        .env("GIT_CONFIG_SYSTEM", empty_gitconfig())
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
        .env("GIT_CONFIG_SYSTEM", empty_gitconfig())
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
    // Excerpts are cut at 100 characters, but an allow value is compared with
    // the whole matched text: the exemption applies to the long address.
    let long = format!("{}@corp.example", "a".repeat(120));
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
        &format!("mail {}@corp.example\n", "b".repeat(120)),
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

// ---- task 1a ----

#[test]
fn xtask_tests_write_only_under_test_root() {
    let helper_file = empty_gitconfig().to_path_buf();
    let root = std::env::var("UNIDEX_TEST_ROOT").expect("cargo sets UNIDEX_TEST_ROOT");
    let root = fs::canonicalize(&root).unwrap_or_else(|_| std::path::PathBuf::from(&root));
    let file = fs::canonicalize(&helper_file).unwrap();
    assert!(
        file.starts_with(&root),
        "{} is not under {}",
        file.display(),
        root.display()
    );
}

// ---- task 1a: pins for the wider coverage ----
//
// Vectors that a rule flags are built at run time from parts.

fn user() -> String {
    format!("{}{}", "wren", "field")
}

fn all_rules(text: &str) -> Vec<RuleId> {
    scan_text("f.md", text, &Terms::generic())
        .into_iter()
        .map(|f| f.rule)
        .collect()
}

fn utf16le(text: &str, bom: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if bom {
        out.extend_from_slice(&[0xFF, 0xFE]);
    }
    for u in text.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

#[test]
fn a_home_folder_is_flagged_after_any_absolute_start() {
    let u = user();
    // Where the home folder rule no longer fires: a relative path with a
    // lowercase `home` or `users` module folder.
    for ok in [
        format!("src/{}/mod.rs", "home"),
        format!("./{}/{}/x", "home", u),
        // a relative path after a word, a quote, a bracket, `=` or `:`
        format!("see src/{}/mod.rs", "home"),
        format!("\"src/{}/mod.rs\"", "users"),
        format!("(src/{}/mod.rs)", "home"),
        format!("x=src/{}/mod.rs", "home"),
        format!("a:src/{}/mod.rs", "users"),
        format!("see caf\u{e9}/{}/mod.rs", "home"),
    ] {
        assert!(all_rules(&ok).is_empty(), "flagged: {ok}");
    }
    // Where it still does, in the Unix and the Windows form. The first two
    // were pinned clean by an earlier round; a capitalised `Users` folder is
    // never a module folder (controller's ruling, rule 2).
    for bad in [
        format!("a/b/{}/{}/x", "Users", u),
        format!("../{}/{}/x", "Users", u),
        format!("/{}/{}/x", "home", u),
        format!("/usr/{}/{}/x", "home", u),
        format!("/mnt/c/{}/{}/x", "Users", u),
        format!("PATH=/bin:/{}/{}/bin", "home", u),
        format!("PATH=$X/bin:/{}/{}/bin", "home", u),
        format!("ssh://host.example/{}/{}/r.git", "home", u),
        format!("user@host.example.com:/{}/{}/r", "home", u),
        format!("C:/{}/{}/x", "Users", u),
        format!("C:\\{}\\{}\\x", "Users", u),
        format!("\\\\?\\C:\\{}\\{}\\x", "Users", u),
        // garbled bytes in front of a path do not make it relative
        format!("\u{FFFD}\u{FFFD}/{}/{}/x", "home", u),
        format!("$HOME/{}/{}/x", "home", u),
    ] {
        assert!(all_rules(&bad).contains(&RuleId::HomePath), "{bad}");
    }
}

#[test]
fn a_home_folder_in_a_url_is_exempt_only_in_the_path() {
    let u = user();
    for ok in [
        format!("https://example.com/{}/{}", "users", u),
        format!("http://example.com/a/{}/{}/x", "home", u),
        format!("https://example.com//{}/{}/x", "home", u),
        // the scheme is not at the start of its word
        format!("redirect:https://example.com/{}/{}/x", "users", u),
        format!("next:http://example.com/{}/{}/x", "home", u),
    ] {
        assert!(all_rules(&ok).is_empty(), "flagged: {ok}");
    }
    for bad in [
        format!("https://example.com/?p=/{}/{}/x", "home", u),
        format!("https://example.com/#/{}/{}/x", "Users", u),
        format!("https://example.com/?p=%2F{}%2F{}%2Fx", "home", u),
        format!("https://example.com/?p=C%3A%2F{}%2F{}%2Fx", "Users", u),
        format!("https://example.com/?p=c%3a%5c{}%5c{}", "users", u),
    ] {
        assert_eq!(all_rules(&bad), vec![RuleId::HomePath], "{bad}");
    }
    // A URL after a non-breaking space (two bytes) is still found.
    let nbsp = format!("abcd\u{a0}https://example.com/{}/{}", "users", u);
    assert!(all_rules(&nbsp).is_empty(), "flagged: {nbsp}");
    // A percent-encoded path is never read as relative (an earlier round
    // pinned this one clean; the ruling's rule 1 flags it).
    assert_eq!(
        all_rules(&format!("a%2F{}%2F{}%2Fx", "home", u)),
        vec![RuleId::HomePath]
    );
}

#[test]
fn local_network_names_that_are_code_or_files_are_clean() {
    for ok in [
        "self.local",
        "this.lan",
        "crate.local",
        "super.local",
        "terms.local.contains(x)",
        "x.local::y",
        "f.local(1)",
        "config.local.json",
        "cp .env.local .env",
        "my_var.local",
        "x.local._y",
        "a.lan_x",
        "x.localhost",
        "a.landing",
    ] {
        assert!(all_rules(ok).is_empty(), "flagged: {ok}");
    }
    for bad in [
        format!("{}.{}", "printer", "local"),
        format!("see http://{}.{}:8080/x", "printer", "local"),
        format!("{}.{}.{}", "a", "b", "lan"),
        format!("a.b.{}.{}", "home", "arpa"),
        format!("({}.{})", "nas", "lan"),
        format!("x {}.{}.", "nas", "lan"),
        format!("{}.{}", "NAS", "LAN"),
        format!("dana@{}.{}", "box", "lan"),
    ] {
        assert!(
            all_rules(&bad).contains(&RuleId::PrivateHost),
            "not flagged: {bad}"
        );
    }
}

#[test]
fn prompts_naming_an_account_and_a_machine_are_flagged() {
    let who = format!("{}@{}", "dana", "devbox");
    for bad in [
        format!("{who}:~$ ls"),
        format!("{who}:/srv/app# whoami"),
        format!("{who}:~/code $"),
        format!("[{who} code]$ ls"),
        format!("[{who} ~]# ls"),
        format!("$ {who}:~$ x"),
        format!("{}@{}.corp.example:~$ x", "dana", "devbox"),
    ] {
        assert!(all_rules(&bad).contains(&RuleId::PrivateHost), "{bad}");
    }
    for ok in [
        "user@host:~$ ls",
        "me@hostname:~$ ls",
        "me@localhost:~$ ls",
        "me@example:~$ ls",
        "me@box.example.com:~$ ls",
        "me@box.example.org:~$ ls",
        "me@box.example.net:~$ ls",
        "[me@host ~]$ ls",
        "ssh dana@devbox",
        // The hosting service's SSH user address is real on purpose: it is
        // the address the email rule allows, and the prompt rule must leave
        // it alone too.
        "git@github.com:org/repo.git",
        "git@github.com:/org/repo.git",
        "url git@example.org:/x#frag here",
        "dana@devbox:~ no prompt char",
    ] {
        assert!(all_rules(ok).is_empty(), "flagged: {ok}");
    }
}

#[test]
fn ipv6_private_ranges_are_flagged_at_their_edges() {
    // `~` splits each prefix so this file holds no address literally.
    for bad in [
        "fe~80::1",
        "fe~bf::1",
        "fc~00::1",
        "fd~ff:ffff::1",
        "fd~12:3456:789a:1::1",
        "(fd~12::1)",
        "[fd~12::1]:8080",
        "addr=fd~12::1.",
        "fd~12::1:",
        "fc~00::",
        "see fd~12:: now",
        "fe~80::1%en0",
        "FD~12::1",
        "x fe~80:0:0:0:0:0:0:1 y",
    ] {
        let bad = bad.replace('~', "");
        let bad = bad.as_str();
        let hits = all_rules(bad);
        assert_eq!(hits, vec![RuleId::PrivateAddress], "{bad}");
    }
    for ok in [
        "fec0::1",
        "fbff::1",
        "fe7f::1",
        "ff02::1",
        "2001:db8::1",
        "::1",
        "::",
        "fd12:3456",
        "12:34:56",
        "std::fmt::Display",
        "f:e",
    ] {
        assert!(all_rules(ok).is_empty(), "flagged: {ok}");
    }
}

#[test]
fn hardware_addresses_are_flagged_with_either_separator() {
    // `~` splits each address so this file holds none literally.
    for bad in [
        "02:4~2:ac:11:00:02",
        "02-4~2-AC-11-00-02",
        "mac 00:1~1:22:33:44:55 end",
    ] {
        let bad = bad.replace('~', "");
        assert_eq!(all_rules(&bad), vec![RuleId::PrivateAddress], "{bad}");
    }
    for ok in ["02:42:ac:11:00", "02:42:ac:11:00:0", "2026-11-18T10:20:30"] {
        assert!(
            !all_rules(ok).contains(&RuleId::PrivateAddress),
            "flagged: {ok}"
        );
    }
}

#[test]
fn utf16_in_every_reading_is_scanned_and_text_is_not_double_reported() {
    let u = user();
    let line = format!("see /{}/{}/x\n", "home", u);
    // Without a mark, a short text, a trailing odd byte, and a text with a
    // NUL but no UTF-16 in it.
    let mut odd = utf16le(&line, true);
    odd.push(0x41);
    for bytes in [utf16le(&line, false), odd] {
        let f = xtask::privacy::scan_bytes("u.txt", &bytes, &Terms::generic());
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!((f[0].rule, f[0].line), (RuleId::HomePath, 1));
    }
    let mut plain = line.clone().into_bytes();
    plain.insert(0, 0);
    let f = xtask::privacy::scan_bytes("u.txt", &plain, &Terms::generic());
    assert_eq!(f.len(), 1, "{f:?}");
    // The second line of a UTF-16 text reports line 2.
    let two = utf16le(&format!("fine\n{line}"), true);
    let f = xtask::privacy::scan_bytes("u.txt", &two, &Terms::generic());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].line, 2);
    // An unpaired surrogate is replaced, never a panic.
    let bad = [0xFF, 0xFE, 0x00, 0xD8, 0x41, 0x00];
    assert!(xtask::privacy::scan_bytes("u.txt", &bad, &Terms::generic()).is_empty());
    // A text that is neither stays one reading.
    let f = xtask::privacy::scan_bytes("u.txt", line.as_bytes(), &Terms::generic());
    assert_eq!(f.len(), 1);
}

#[test]
fn a_git_message_in_utf16_is_cleaned_and_scanned_from_stdin() {
    let u = user();
    let repo = new_repo();
    let msg = format!(
        "subject\n\n{}\n{}\n/{}/{}/x\n",
        SCISSORS, "diff --git a/f b/f", "home", u
    );
    // The part after the scissors line and the diff header is cut in every reading.
    let out = xtask(repo.path())
        .args(["privacy", "--stdin", "msg", "--git-message"])
        .write_stdin(utf16le(&msg, true))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", output_text(&out));
    let body = format!("subject\n\nsee /{}/{}/x\n", "home", u);
    for bom in [true, false] {
        let out = xtask(repo.path())
            .args(["privacy", "--stdin", "msg", "--git-message"])
            .write_stdin(utf16le(&body, bom))
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{}", output_text(&out));
    }
}

#[test]
fn a_utf8_term_list_with_a_mark_is_read_and_a_nul_makes_it_an_error() {
    let mut with_bom = vec![0xEF, 0xBB, 0xBF];
    with_bom.extend_from_slice(b"Zebracorp\n");
    let t = Terms::generic().with_local(&String::from_utf8(with_bom).unwrap());
    assert_eq!(scan_text("f.md", "zebracorp", &t).len(), 1);

    // A stray NUL is named as one (fix round 1, review 1 M5); a mark or a
    // text with NULs on one side of its pairs looks like UTF-16.
    for (name, list, says) in [
        (
            "nul inside",
            b"Zebra\0corp\n".to_vec(),
            "a NUL byte at offset 5",
        ),
        ("nul only", b"\0".to_vec(), "a NUL byte at offset 0"),
        ("le mark only", vec![0xFF, 0xFE], "looks like UTF-16"),
        ("be mark only", vec![0xFE, 0xFF], "looks like UTF-16"),
        (
            "le text",
            utf16le("Zebracorp\n", false),
            "looks like UTF-16",
        ),
    ] {
        let repo = new_repo();
        write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
        fs::write(repo.path().join("privacy-terms.local.txt"), &list).unwrap();
        write(repo.path(), "a.md", "about zebracorp\n");
        commit_all(repo.path(), "add");
        let (code, text) = run(repo.path(), &["privacy", "--all"]);
        assert_eq!(code, 2, "{name}: {text}");
        assert!(text.contains("privacy: error"), "{name}: {text}");
        assert!(text.contains(says), "{name}: {text}");
        assert!(!text.to_lowercase().contains("zebra"), "{name}: {text}");
    }
}

#[test]
fn a_term_list_that_is_not_valid_utf8_fails_loudly_in_every_mode() {
    // One invalid byte in the middle of an otherwise valid list: reading it
    // lossily would silently disable the term that holds it.
    let list: &[u8] = b"Zebracorp\nQuo\xffrum\nOther Term\n";
    let modes: [&[&str]; 5] = [
        &["privacy", "--all"],
        &["privacy", "--staged"],
        &["privacy", "--range", "HEAD"],
        &["privacy", "--all", "--require-local"],
        &["privacy", "--stdin", "x"],
    ];
    for args in modes {
        let repo = new_repo();
        write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
        fs::write(repo.path().join("privacy-terms.local.txt"), list).unwrap();
        write(repo.path(), "a.md", "harmless\n");
        commit_all(repo.path(), "add");
        let out = xtask(repo.path())
            .args(args)
            .write_stdin("harmless")
            .output()
            .unwrap();
        let text = output_text(&out);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {text}");
        assert!(text.contains("privacy: error"), "{args:?}: {text}");
        assert!(text.contains("not valid UTF-8"), "{args:?}: {text}");
        let lower = text.to_lowercase();
        assert!(!lower.contains("zebra") && !lower.contains("quo"), "{text}");
    }
    // A UTF-8 list with a mark and non-ASCII text is still read.
    let repo = new_repo();
    write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
    fs::write(
        repo.path().join("privacy-terms.local.txt"),
        "\u{FEFF}Caf\u{e9}corp\n".as_bytes(),
    )
    .unwrap();
    write(repo.path(), "a.md", "about caf\u{e9}corp\n");
    commit_all(repo.path(), "add");
    assert_eq!(run(repo.path(), &["privacy", "--all"]).0, 1);
}

#[test]
fn a_range_says_how_many_commits_it_scanned() {
    let repo = new_repo();
    write(repo.path(), "a.md", "one\n");
    commit_all(repo.path(), "one");
    write(repo.path(), "a.md", "two\n");
    commit_all(repo.path(), "two");
    write(repo.path(), "a.md", "three\n");
    commit_all(repo.path(), "three");
    let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD~1..HEAD"]);
    assert_eq!(
        (code, text.contains("clean (1 commit scanned)")),
        (0, true),
        "{text}"
    );
    let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD~2..HEAD"]);
    assert_eq!(
        (code, text.contains("clean (2 commits scanned)")),
        (0, true),
        "{text}"
    );
    let (_, text) = run(repo.path(), &["privacy", "--range", "HEAD..HEAD"]);
    assert!(text.contains("clean (0 commits scanned)"), "{text}");
    // The other modes keep the bare form.
    let (_, text) = run(repo.path(), &["privacy", "--all"]);
    assert!(text.trim_end().ends_with("privacy: clean"), "{text}");
}

#[test]
fn a_submodule_path_name_is_checked_in_range_and_whole_tree_modes() {
    // `git add -A` drops a gitlink whose folder is missing, so these commit
    // the index as it is.
    let path = "vendor/zebracorp-lib";
    let repo = new_repo();
    write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
    write(repo.path(), "privacy-terms.local.txt", "Zebracorp\n");
    write(repo.path(), "a.md", "harmless\n");
    git(repo.path(), &["add", "a.md", ".gitignore"]);
    git(
        repo.path(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},{path}", "1".repeat(40)),
        ],
    );
    let (code, text) = run(repo.path(), &["privacy", "--staged"]);
    assert_eq!(code, 1, "--staged: {text}");
    assert!(text.contains(path), "{text}");
    git(repo.path(), &["commit", "-q", "--no-verify", "-m", "sub"]);
    let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD"]);
    assert_eq!(code, 1, "--range: {text}");
    assert!(text.contains("LocalTerm") && text.contains(path), "{text}");
    let (code, text) = run(repo.path(), &["privacy", "--all"]);
    assert_eq!(code, 1, "--all: {text}");
}

#[test]
fn an_allow_value_must_be_the_whole_matched_text() {
    let long = format!("{}@corp.example", "x".repeat(140));
    for (value, want) in [
        (long.clone(), 0),
        (format!("{}...", &long[..100]), 1),
        (long[..100].to_string(), 1),
        (format!("{long}x"), 1),
    ] {
        let repo = new_repo();
        write(repo.path(), "a.md", &format!("mail {long}\n"));
        write(
            repo.path(),
            "privacy-allow.toml",
            &allow("a.md", "Email", &format!("\"{value}\"")),
        );
        commit_all(repo.path(), "add");
        assert_eq!(run(repo.path(), &["privacy", "--all"]).0, want, "{value}");
    }
}

#[test]
fn a_utf16_text_with_no_nul_byte_is_found_by_its_mark_and_plain_text_is_not_misread() {
    // Non-Latin text in UTF-16 has no zero byte, so only the byte-order mark
    // says what it is. The term is three Greek capitals.
    let term = "\u{3A9}\u{3A8}\u{39E}";
    let t = Terms::generic().with_local(term);
    let mut le = vec![0xFF, 0xFE];
    let mut be = vec![0xFE, 0xFF];
    for u in format!("a {term} b").encode_utf16() {
        le.extend_from_slice(&u.to_le_bytes());
        be.extend_from_slice(&u.to_be_bytes());
    }
    // Make the text free of zero bytes: only the three capitals and no ASCII.
    let only: Vec<u8> = {
        let mut v = vec![0xFF, 0xFE];
        for u in term.encode_utf16() {
            v.extend_from_slice(&u.to_le_bytes());
        }
        v
    };
    assert!(!only.contains(&0));
    let only_be: Vec<u8> = {
        let mut v = vec![0xFE, 0xFF];
        for u in term.encode_utf16() {
            v.extend_from_slice(&u.to_be_bytes());
        }
        v
    };
    assert!(!only_be.contains(&0));
    for bytes in [&le, &be, &only, &only_be] {
        let f = xtask::privacy::scan_bytes("u.txt", bytes, &t);
        assert_eq!(f.len(), 1, "{bytes:?}");
        assert_eq!(f[0].rule, RuleId::LocalTerm);
    }
    // Plain text with no NUL is read as UTF-8 only: its byte pairs, read as
    // UTF-16, would spell this one character.
    let odd = Terms::generic().with_local("\u{6261}");
    assert!(xtask::privacy::scan_bytes("u.txt", b"ab", &odd).is_empty());
    assert!(xtask::privacy::scan_bytes("u.txt", b"ba", &odd).is_empty());
}

// ---- fix round 1, group A: the home-folder exemption (controller's ruling) ----
//
// The exemption for a relative path with a `home` or `users` module folder
// holds only when all five rules of `relative_module_folder` hold (plain Unix
// form; lowercase folder; a run that starts with a letter, digit or `_`,
// optionally after one `./`; no `..` segment; not the continuation of an
// absolute path across a single space). Every vector is built at run time.

fn home_flagged(text: &str) -> bool {
    all_rules(text).contains(&RuleId::HomePath)
}

#[test]
fn the_inputs_the_reviews_found_passing_are_flagged_again() {
    let u = user();
    let mut missed: Vec<String> = Vec::new();
    for bad in [
        // an option glued to its value, in a command line and in a JSON
        // array of arguments
        format!("clang -I/{}/{}/include -c a.c", "home", u),
        format!("rustc -L/{}/{}/lib a.rs", "Users", u),
        format!("RUSTFLAGS=-L/{}/{}/lib", "Users", u),
        format!(
            "gcc -c a.c -I/{h}/{u}/.local/include -L/{h}/{u}/.local/lib -lfoo",
            h = "home"
        ),
        format!("-F/{}/{}/Library/Frameworks", "Users", u),
        format!("-isystem/{}/{}/include", "home", u),
        format!("docker run -v/{}/{}:/data img", "home", u),
        format!("[\"-I/{}/{}/proj/include\", \"-c\", \"a.c\"]", "home", u),
        format!("[\"-I/{}/{}/proj/include\", \"-c\", \"a.c\"]", "Users", u),
        // a parent folder whose name holds a space (a volume)
        format!("open /Volumes/Macintosh HD/{}/{}/x.txt", "Users", u),
        format!("/Volumes/Backup Disk/{}/{}/x", "home", u),
        // climbing into a home folder
        format!("path = \"../../../{}/{}/lib\"", "home", u),
        format!("../../{}/{}/p/a.rs", "home", u),
        format!("../../../{}/{}/proj/src/a.ts", "Users", u),
        format!(".../{}/{}/project/src/main.rs", "home", u),
        format!("~/../../{}/{}/x", "Users", u),
        format!("~/{}/{}/x", "Users", u),
        format!("_/{}/{}/x_", "Users", u),
        // coverage (lcov) and source-map shapes
        format!("SF:../../{}/{}/p/a.ts", "Users", u),
        format!("SF:../../{}/{}/p/a.rs", "home", u),
        format!("\"sources\":[\"../../{}/{}/p/a.ts\"]", "Users", u),
        // a capitalised folder after a relative-looking prefix
        format!("cp -r a/b/{}/{}/x .", "Users", u),
        format!("mnt/c/{}/{}/x", "Users", u),
        // the drive-letter forms glued to an option or a word
        format!("-IC:\\{}\\{}\\include", "Users", u),
        format!("build-C:\\{}\\{}\\x", "Users", u),
        format!("docsC:\\{}\\{}\\x", "Users", u),
        // the encoded forms after a relative-looking prefix
        format!("-IC%3A%5C{}%5C{}", "Users", u),
        format!("p=x-C%3A%5C{}%5C{}", "Users", u),
        format!("-I%2F{}%2F{}", "home", u),
    ] {
        if !home_flagged(&bad) {
            missed.push(bad);
        }
    }
    assert!(missed.is_empty(), "not flagged: {missed:#?}");
}

#[test]
fn each_rule_of_the_exemption_is_needed() {
    let u = user();
    let mut wrong: Vec<String> = Vec::new();
    // (input, flagged?) for each rule, in the Unix form; the Windows forms
    // are never exempt.
    for (text, want) in [
        // rule 1: only the plain Unix form
        (format!("src/{}/{}/x", "home", u), false),
        (format!("src%2F{}%2F{}%2Fx", "home", u), true),
        (format!("src/C:\\{}\\{}\\x", "Users", u), true),
        // rule 2: only a lowercase `home` or `users`
        (format!("src/{}/{}/x", "users", u), false),
        (format!("src/{}/{}/x", "Users", u), true),
        (format!("src/{}/{}/x", "Home", u), true),
        (format!("src/{}/{}/x", "HOME", u), true),
        (format!("src/{}/{}/x", "uSers", u), true),
        // rule 3: the run starts with a letter, digit or `_`, after at most
        // one `./`
        (format!("9lives/{}/{}/x", "home", u), false),
        (format!("_build/{}/{}/x", "users", u), false),
        (format!("./{}/{}/x", "home", u), false),
        (format!("./src/{}/{}/x", "users", u), false),
        (format!("-src/{}/{}/x", "home", u), true),
        (format!("~src/{}/{}/x", "home", u), true),
        (format!(".src/{}/{}/x", "home", u), true),
        (format!("././{}/{}/x", "home", u), true),
        (format!(".//{}/{}/x", "home", u), true),
        (format!("/src/{}/{}/x", "home", u), true),
        (format!("/{}/{}/x", "home", u), true),
        (format!("cd /{}/{}/x", "home", u), true),
        // ... and holds only path characters, back to a delimiter
        (format!("a$b/{}/{}/x", "home", u), true),
        (format!("a~b/{}/{}/x", "home", u), true),
        (format!("a\\b/{}/{}/x", "home", u), true),
        (format!("\u{FFFD}b/{}/{}/x", "home", u), true),
        (format!("see caf\u{e9}/{}/{}/x", "home", u), false),
        // rule 4: no `..` segment
        (format!("src/../{}/{}/x", "home", u), true),
        (format!("./../{}/{}/x", "home", u), true),
        (format!("a/b/../../{}/{}/x", "users", u), true),
        (format!("a..b/{}/{}/x", "home", u), false),
        // rule 5: not an absolute path continued across a single space
        (format!("/Volumes/Backup Disk/{}/{}/x", "home", u), true),
        (format!("~/My Files/{}/{}/x", "home", u), true),
        (format!("D:/Backup Disk/{}/{}/x", "home", u), true),
        (format!("see src/{}/{}/x", "home", u), false),
        (format!("/a x src/{}/{}/x", "home", u), false),
        (format!("x /a src/{}/{}/x", "home", u), true),
        (format!("\tsrc/{}/{}/x", "home", u), false),
        (format!("/srv\tsrc/{}/{}/x", "home", u), false),
        (format!("/srv  src/{}/{}/x", "home", u), false),
        (format!("1:2 src/{}/{}/x", "home", u), false),
        // a digit and a colon before a backslash is not a drive path (the
        // forward-slash spelling is flagged since round 3: `:/` begins an
        // absolute path inside the token)
        (format!("9:\\b\\10:20/{}/{}/x", "home", u), false),
        (format!("9:/b/10:20/{}/{}/x", "home", u), true),
        (format!("C:\\b\\10:20/{}/{}/x", "home", u), true),
        // only the cut after the last opener sees these (a `)` inside a
        // parent folder's name ends the other cut, and a digit after it is
        // not dropped like an option)
        (format!("\"/mnt/a)b/Backup Disk/{}/{}/x\"", "home", u), true),
        (format!("\"/mnt/a)1/Backup Disk/{}/{}/x\"", "home", u), true),
        (format!("x=\"/srv/a)1/old disk/{}/{}\"", "home", u), true),
        // a known false positive, kept by the ruling: a closing bracket
        // before the space (the cut after the last opener sees `/usr/bin)`)
        (format!("(/usr/bin) src/{}/mod.rs", "home"), true),
        // a climb inside the token that holds the run
        (format!("../b:src/{}/{}/x", "home", u), true),
        // a quoted drive path in backslash spelling: the leading quote is
        // dropped before the drive test
        (format!("\"C:\\b\\10:20/{}/{}/x\"", "home", u), true),
        (format!("(C:\\b\\10:20/{}/{}/x)", "home", u), true),
        (format!("C: src/{}/{}/x", "home", u), true),
        (format!(" src/{}/{}/x", "home", u), false),
    ] {
        if home_flagged(&text) != want {
            wrong.push(format!("{text:?} should be flagged={want}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn the_exemption_holds_in_unix_and_never_in_windows_form() {
    let u = user();
    for ok in [
        format!("src/{}/mod.rs", "home"),
        format!("src/{}/handlers.rs", "users"),
        format!("./{}/index.md", "home"),
        format!("./src/{}/mod.rs", "users"),
        // a relative Windows path is never a match at all
        format!("src\\{}\\handlers.rs", "users"),
        format!(".\\{}\\{}\\x", "home", u),
    ] {
        assert!(all_rules(&ok).is_empty(), "flagged: {ok}");
    }
    for bad in [
        format!("/{}/{}/x", "home", u),
        format!("C:\\{}\\{}\\x", "Users", u),
        format!("x C:\\{}\\{}\\x", "Users", u),
        format!("src/C:\\{}\\{}\\x", "Users", u),
        format!("c%3a%5c{}%5c{}", "users", u),
    ] {
        assert!(home_flagged(&bad), "not flagged: {bad}");
    }
}

#[test]
fn a_look_back_that_cannot_decide_flags() {
    let u = user();
    // A run longer than the look-back's bound (1024 bytes) is flagged, one
    // well under it is not.
    let long = format!("{}/{}/{}/x", "a".repeat(2000), "home", u);
    assert!(home_flagged(&long));
    let short = format!("{}/{}/{}/x", "a".repeat(200), "home", u);
    assert!(!home_flagged(&short));
    // The same for the token before a single space (rule 5).
    let token = format!("{} src/{}/{}/x", "b".repeat(2000), "home", u);
    assert!(home_flagged(&token));
    let token = format!("{} src/{}/{}/x", "b".repeat(200), "home", u);
    assert!(!home_flagged(&token));
    // A delimiter inside the bound of a long line still ends the run there
    // (a tab before the token: since fix round 2 rule 5 also reads the token
    // that holds the run, and one that starts beyond the bound flags).
    let far = format!("{}\tx=src/{}/{}/x", "c".repeat(3000), "home", u);
    assert!(!home_flagged(&far));
    let far = format!("{}=src/{}/{}/x", "c".repeat(3000), "home", u);
    assert!(home_flagged(&far));
    let far = format!("{}=/{}/{}/x", "c".repeat(3000), "home", u);
    assert!(home_flagged(&far));
    let far = format!(
        "{} https://example.com/{}/{}/x",
        "c".repeat(3000),
        "users",
        u
    );
    assert!(!home_flagged(&far));
    // And for a URL: a path segment further back than the bound is no
    // longer read as part of the URL.
    let url = format!(
        "https://example.com/{}{}/{}/x",
        "p/".repeat(1000),
        "users",
        u
    );
    assert!(home_flagged(&url));
    let url = format!(
        "https://example.com/{}{}/{}/x",
        "p/".repeat(100),
        "users",
        u
    );
    assert!(!home_flagged(&url));
}

#[test]
fn a_staged_path_name_is_judged_by_the_same_rules() {
    let u = user();
    let flagged_name = format!("cache/{}/{}/Desktop/notes.txt", "Users", u);
    // A residual named in the task report: a lowercase folder under a
    // relative-looking prefix passes rules 2 and 3 even when it is a copy of
    // someone's home folder inside the repository.
    let exempt_name = format!("mirror/{}/{}/bashrc", "home", u);
    assert!(home_flagged(&flagged_name));
    let t = Terms::generic();
    assert!(
        scan_path_name(&flagged_name, &t)
            .iter()
            .any(|f| f.rule == RuleId::HomePath)
    );
    assert!(scan_path_name(&exempt_name, &t).is_empty());

    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    for name in [&flagged_name, &exempt_name] {
        let p = repo.path().join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "harmless\n").unwrap();
    }
    git(repo.path(), &["add", "-A"]);
    let (code, text) = run(repo.path(), &["privacy", "--staged"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains(&format!("{flagged_name}: HomePath")),
        "{text}"
    );
    assert!(!text.contains(&exempt_name), "{text}");
}

// ---- fix round 1, group B: the private term list ----
//
// Every list here is invented and lives in a temporary repository.

/// A repository with an ignored local term list holding `list` and a file
/// that mentions the invented term `Brackenfold`.
fn term_repo(list: &[u8]) -> TempDir {
    let repo = new_repo();
    write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
    fs::write(repo.path().join("privacy-terms.local.txt"), list).unwrap();
    write(repo.path(), "a.md", "about brackenfold here\n");
    commit_all(repo.path(), "add");
    repo
}

const EVERY_MODE: [&[&str]; 6] = [
    &["privacy", "--all"],
    &["privacy", "--staged"],
    &["privacy", "--range", "HEAD"],
    &["privacy", "--stdin", "x"],
    &["privacy", "--all", "--require-local"],
    &["privacy", "--stdin", "x", "--require-local"],
];

fn run_mode(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = xtask(dir)
        .args(args)
        .write_stdin("about brackenfold here")
        .output()
        .unwrap();
    (out.status.code().unwrap(), output_text(&out))
}

#[test]
fn a_term_list_with_an_invisible_or_control_character_fails_loudly() {
    let mut cases: Vec<(String, Vec<u8>)> = Vec::new();
    // A second byte-order mark, as when two files are joined.
    let mut joined = b"Alpha\n".to_vec();
    joined.extend_from_slice("\u{FEFF}".as_bytes());
    joined.extend_from_slice(b"Brackenfold\n");
    cases.push(("mark after the start".into(), joined));
    for c in [
        '\u{FEFF}', '\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{00AD}', '\u{0001}',
        '\u{001B}', '\u{007F}', '\u{0085}',
    ] {
        cases.push((
            format!("U+{:04X} inside a term", c as u32),
            format!("Bracken{c}fold\n").into_bytes(),
        ));
    }
    for (name, list) in cases {
        let repo = term_repo(&list);
        for args in EVERY_MODE {
            let (code, text) = run_mode(repo.path(), args);
            assert_eq!(code, 2, "{name} {args:?}: {text}");
            assert!(
                text.contains("privacy: error")
                    && text.contains("privacy-terms.local.txt")
                    && text.contains("offset"),
                "{name} {args:?}: {text}"
            );
            let lower = text.to_lowercase();
            assert!(
                !lower.contains("bracken") && !lower.contains("alpha"),
                "{name} {args:?}: {text}"
            );
        }
    }
    // The offset is the byte where the character starts, after a leading
    // mark too.
    for (list, offset) in [
        ("Bracken\u{200B}fold\n", "offset 7;"),
        ("\u{FEFF}Bracken\u{200B}fold\n", "offset 10;"),
        ("Ab\nBracken\u{00AD}fold\n", "offset 10;"),
    ] {
        let repo = term_repo(list.as_bytes());
        let (code, text) = run(repo.path(), &["privacy", "--all"]);
        assert_eq!(code, 2, "{text}");
        assert!(text.contains(offset), "{list:?}: {text}");
    }
    // A single leading mark, a tab, CR and LF are fine.
    let repo = term_repo("\u{FEFF}Brackenfold\t\r\nOther\n".as_bytes());
    let (code, text) = run(repo.path(), &["privacy", "--all", "--require-local"]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("LocalTerm"), "{text}");
}

#[test]
fn crlf_and_lone_cr_line_endings_separate_terms() {
    for list in [
        "Alpha\rBrackenfold\r",
        "Alpha\r\nBrackenfold\r\n",
        "Alpha\rBrackenfold",
    ] {
        let t = Terms::generic().with_local(list);
        for text in ["about brackenfold", "about alpha"] {
            let f = scan_text("f.md", text, &t);
            assert_eq!(f.len(), 1, "{list:?} on {text:?}: {f:?}");
            assert_eq!(f[0].rule, RuleId::LocalTerm);
        }
        // Never one giant term, never a term with a carriage return.
        assert!(
            scan_path_name("docs/brackenfold.md", &t)
                .iter()
                .any(|f| f.rule == RuleId::LocalTerm),
            "{list:?}"
        );
    }
}

#[test]
fn a_local_list_with_no_terms_is_never_silent() {
    for list in [
        "",
        "\n\n",
        "# only a comment\n",
        "  \n# a\n\t\n",
        "\u{FEFF}",
    ] {
        let repo = term_repo(list.as_bytes());
        let (code, text) = run(repo.path(), &["privacy", "--all", "--require-local"]);
        assert_eq!(code, 2, "{list:?}: {text}");
        assert!(text.contains("has no terms"), "{list:?}: {text}");
        let (code, text) = run(repo.path(), &["privacy", "--all"]);
        assert_eq!(code, 0, "{list:?}: {text}");
        assert!(
            text.contains("warning") && text.contains("has no terms"),
            "{list:?}: {text}"
        );
        assert!(!text.contains("no privacy-terms.local.txt found"), "{text}");
    }
}

/// A main checkout with list `main_list` and a linked worktree of it with
/// list `wt_list` (none when `None`), both ignored. The tracked file names
/// two invented terms, one per list.
fn worktree_pair(main_list: &str, wt_list: Option<&str>) -> (TempDir, TempDir, std::path::PathBuf) {
    let main = new_repo();
    write(main.path(), ".gitignore", "privacy-terms.local.txt\n");
    write(main.path(), "privacy-terms.local.txt", main_list);
    write(main.path(), "a.md", "about brackenfold\nand quorvale\n");
    commit_all(main.path(), "add a");
    let holder = TempDir::new().unwrap();
    let wt = holder.path().join("wt");
    git(
        main.path(),
        &["worktree", "add", "-q", wt.to_str().unwrap()],
    );
    if let Some(l) = wt_list {
        write(&wt, "privacy-terms.local.txt", l);
    }
    (main, holder, wt)
}

#[test]
fn a_worktree_combines_its_list_with_the_main_checkouts() {
    let (_main, _holder, wt) = worktree_pair("Brackenfold\n", Some("Quorvale\n"));
    let out = xtask(&wt)
        .args(["privacy", "--all", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    let v: Vec<serde_json::Value> = serde_json::from_str(&stdout).expect("json array");
    let lines: Vec<i64> = v
        .iter()
        .filter(|f| f["rule"] == "LocalTerm")
        .map(|f| f["line"].as_i64().unwrap())
        .collect();
    assert_eq!(lines, vec![1, 2], "{stdout}");

    // An empty list in the worktree hides nothing: the main checkout's term
    // is still checked, and the empty list is announced.
    let (_main, _holder, wt) = worktree_pair("Brackenfold\n", Some("# nothing yet\n"));
    let (code, text) = run(&wt, &["privacy", "--all"]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("a.md:1: LocalTerm"), "{text}");
    assert!(text.contains("has no terms"), "{text}");
    // Under --require-local an empty list is an error even beside a full one.
    let (code, text) = run(&wt, &["privacy", "--all", "--require-local"]);
    assert_eq!(code, 2, "{text}");
    // The same the other way round.
    let (_main, _holder, wt) = worktree_pair("\n", Some("Quorvale\n"));
    let (code, text) = run(&wt, &["privacy", "--all"]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("a.md:2: LocalTerm"), "{text}");
    // A broken list in either place is an error.
    let (_main, _holder, wt) = worktree_pair("Bracken\u{200B}fold\n", Some("Quorvale\n"));
    assert_eq!(run(&wt, &["privacy", "--all"]).0, 2);
    let (_main, _holder, wt) = worktree_pair("Brackenfold\n", Some("Quor\u{00AD}vale\n"));
    assert_eq!(run(&wt, &["privacy", "--all"]).0, 2);
}

#[cfg(unix)]
#[test]
fn a_dangling_symlink_for_the_term_list_is_an_error() {
    let repo = new_repo();
    write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
    write(repo.path(), "a.md", "harmless\n");
    commit_all(repo.path(), "add");
    std::os::unix::fs::symlink(
        repo.path().join("missing-target"),
        repo.path().join("privacy-terms.local.txt"),
    )
    .unwrap();
    let (code, text) = run(repo.path(), &["privacy", "--all"]);
    assert_eq!(code, 2, "{text}");
}

// ---- fix round 1, group C: work that stays linear ----
//
// Each test asserts its findings and a time bound of 30 seconds. On the
// reference Mac (debug build) each finishes in about a second; the quadratic
// code they replace took minutes on the same input, so the bound leaves a
// wide margin for a slow CI runner and still fails the quadratic forms.

const LINEAR_BOUND: std::time::Duration = std::time::Duration::from_secs(30);

#[test]
fn a_long_line_without_delimiters_is_scanned_in_linear_time() {
    // One 160 KB line: a relative-looking run with a home folder every 8
    // bytes. The look-backs stop at 1024 bytes, so the first matches (inside
    // the bound, a relative run) are exempt and every later one is flagged.
    let name = format!("{}{}", "k", "v");
    let line = format!("a{}\n", format!("/{}/{}", "home", name).repeat(20_000));
    let started = std::time::Instant::now();
    let f = scan_text("f.md", &line, &Terms::generic());
    let took = started.elapsed();
    let want = format!("/{}/{}", "home", name);
    assert!(
        f.iter()
            .all(|x| x.rule == RuleId::HomePath && x.value == want && x.line == 1)
    );
    // Match k starts at byte 1 + 8k; it is exempt while 1 + 8k <= 1024.
    assert_eq!(f.len(), 20_000 - 128);
    assert!(took < LINEAR_BOUND, "took {took:?}");
}

#[test]
fn a_file_with_many_findings_is_scanned_in_linear_time() {
    // 100,000 lines, each with an address and a local term: 200,000
    // findings, merged across readings without a quadratic search.
    let t = Terms::generic().with_local("Brackenfold\n");
    let mail = format!("{}@{}", "ann", "corp.example");
    let text: String = (0..100_000)
        .map(|i| format!("{i} write {mail} about brackenfold\n"))
        .collect();
    let started = std::time::Instant::now();
    let f = xtask::privacy::scan_bytes("f.md", text.as_bytes(), &t);
    let took = started.elapsed();
    assert_eq!(f.len(), 200_000);
    for (i, pair) in f.chunks(2).enumerate() {
        let mut rules: Vec<RuleId> = pair.iter().map(|x| x.rule).collect();
        rules.sort_by_key(|r| r.name());
        assert_eq!(rules, [RuleId::Email, RuleId::LocalTerm], "line {}", i + 1);
        assert!(pair.iter().all(|x| x.line == i + 1), "line {}", i + 1);
    }
    assert!(took < LINEAR_BOUND, "took {took:?}");
}

#[test]
fn a_utf16_part_inside_a_larger_binary_is_still_found() {
    // Pseudo-random bytes with scattered NULs around a UTF-16 line. (Which
    // readings a binary gets is pinned by the unit tests in `scan.rs`.)
    let mut bytes: Vec<u8> = (0u32..4096)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    for i in (0..bytes.len()).step_by(97) {
        bytes[i] = 0;
    }
    let mut mixed = bytes.clone();
    let user = format!("{}{}", "wren", "field");
    let line = format!("see /{}/{}/x\n", "home", user);
    mixed.extend(line.encode_utf16().flat_map(u16::to_le_bytes));
    mixed.extend_from_slice(&bytes);
    let f = xtask::privacy::scan_bytes("b.bin", &mixed, &Terms::generic());
    assert!(f.iter().any(|x| x.rule == RuleId::HomePath), "{f:?}");
}

// ---- fix round 1, group D ----

#[test]
fn the_default_macos_prompts_are_flagged() {
    // zsh's `user@host dir %` and the older bash `host:dir user$`, built at
    // run time from invented names.
    let host = format!("{}-{}", "wrens", "laptop");
    let who = format!("{}@{}", "wren", host);
    let mut missed: Vec<String> = Vec::new();
    for bad in [
        format!("{who} ~ % ls"),
        format!("{who} unidex-rust % git status"),
        format!("{who} ~ %"),
        format!("{who} ~ $ ls"),
        format!("{who} /srv # whoami"),
        format!("{host}:~ wren$ ls"),
        format!("{host}:~/code wren$ make"),
        format!("{host}:/tmp root# id"),
        format!("{host}:/ root#"),
    ] {
        if !all_rules(&bad).contains(&RuleId::PrivateHost) {
            missed.push(bad);
        }
    }
    assert!(missed.is_empty(), "not flagged: {missed:?}");
    let mut wrongly: Vec<String> = Vec::new();
    for ok in [
        format!("reach {who} for help"),
        format!("ssh {who}"),
        format!("ask {who} about it % later"),
        format!("{host}:~ no prompt here"),
        "50 % of runs".to_string(),
        "user@host ~ % ls".to_string(),
        "me@localhost ~ % ls".to_string(),
        "localhost:~ user$ ls".to_string(),
        "host:~ me$ ls".to_string(),
        // a public address with an option after it is not a prompt
        "ssh -T git@github.com -v # check the key".to_string(),
        // a URL followed by a word that ends in `$` is not one either
        "see https://example.com/a x$ b".to_string(),
    ] {
        if !all_rules(&ok).is_empty() {
            wrongly.push(ok);
        }
    }
    assert!(wrongly.is_empty(), "flagged: {wrongly:?}");
}

#[test]
fn a_byte_order_mark_that_contradicts_the_text_hides_nothing() {
    let u = user();
    let t = Terms::generic().with_local("Brackenfold\n");
    let text = format!("about brackenfold\nsee /{}/{}/x\n", "home", u);
    let mut le_mark_be_text = vec![0xFF, 0xFE];
    le_mark_be_text.extend(text.encode_utf16().flat_map(u16::to_be_bytes));
    let mut be_mark_le_text = vec![0xFE, 0xFF];
    be_mark_le_text.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    for (name, bytes) in [("FF FE", le_mark_be_text), ("FE FF", be_mark_le_text)] {
        let f = xtask::privacy::scan_bytes("u.txt", &bytes, &t);
        let rules: Vec<RuleId> = f.iter().map(|x| x.rule).collect();
        assert!(
            rules.contains(&RuleId::LocalTerm) && rules.contains(&RuleId::HomePath),
            "{name}: {f:?}"
        );
        let repo = new_repo();
        write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
        write(repo.path(), "privacy-terms.local.txt", "Brackenfold\n");
        let out = xtask(repo.path())
            .args(["privacy", "--stdin", "u.txt"])
            .write_stdin(bytes)
            .output()
            .unwrap();
        let text = output_text(&out);
        assert_eq!(out.status.code(), Some(1), "{name}: {text}");
        assert!(
            text.contains("LocalTerm") && text.contains("HomePath"),
            "{text}"
        );
    }
    // Text with no NUL (three Greek capitals) gives no evidence for either
    // order, so the unmarked order is read as well.
    let term = "\u{3A9}\u{3A8}\u{39E}";
    let t = Terms::generic().with_local(term);
    let mut le_mark_be_text = vec![0xFF, 0xFE];
    le_mark_be_text.extend(term.encode_utf16().flat_map(u16::to_be_bytes));
    let mut be_mark_le_text = vec![0xFE, 0xFF];
    be_mark_le_text.extend(term.encode_utf16().flat_map(u16::to_le_bytes));
    for bytes in [le_mark_be_text, be_mark_le_text] {
        assert!(!bytes[2..].contains(&0));
        let f = xtask::privacy::scan_bytes("u.txt", &bytes, &t);
        assert_eq!(f.len(), 1, "{bytes:?}: {f:?}");
    }
}

#[test]
fn an_option_shaped_range_is_an_error_not_an_empty_range() {
    let u = user();
    let repo = new_repo();
    write(repo.path(), "a.md", &format!("see /{}/{}/x\n", "home", u));
    commit_all(repo.path(), "add");
    for range in [
        "--stdin",
        "--glob=refs/zzz",
        "--all",
        "-n1",
        "--remotes=zzz",
    ] {
        let arg = format!("--range={range}");
        let (code, text) = run(repo.path(), &["privacy", &arg]);
        assert_eq!(code, 2, "{arg}: {text}");
        assert!(text.contains("privacy: error"), "{arg}: {text}");
        assert!(!text.contains("commits scanned"), "{arg}: {text}");
    }
    // An ordinary range still works.
    assert_eq!(run(repo.path(), &["privacy", "--range", "HEAD"]).0, 1);
}

#[test]
fn range_checks_author_and_committer_names_against_local_terms() {
    let repo = new_repo();
    write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
    write(repo.path(), "privacy-terms.local.txt", "Brackenfold\n");
    write(repo.path(), "a.md", "harmless\n");
    commit_all(repo.path(), "first");
    assert_eq!(run(repo.path(), &["privacy", "--range", "HEAD"]).0, 0);
    // The author's name holds the term; the committer is the plain test user.
    write(repo.path(), "a.md", "two\n");
    git(repo.path(), &["add", "-A"]);
    git(
        repo.path(),
        &[
            "commit",
            "-q",
            "--no-verify",
            "--author",
            "Ann Brackenfold <ann@example.com>",
            "-m",
            "second",
        ],
    );
    let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD~1..HEAD"]);
    assert_eq!(code, 1, "{text}");
    // Since fix round 3 the whole identity line is checked, under the
    // `author` and `committer` labels.
    assert!(text.contains("author:1: LocalTerm"), "{text}");
    assert!(!text.contains("committer:1: LocalTerm"), "{text}");
    // The committer's name holds it.
    write(repo.path(), "a.md", "three\n");
    git(repo.path(), &["add", "-A"]);
    git(
        repo.path(),
        &[
            "-c",
            "user.name=Ann Brackenfold",
            "commit",
            "-q",
            "--no-verify",
            "--author",
            "Test User <test@example.com>",
            "-m",
            "third",
        ],
    );
    let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD~1..HEAD"]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("committer:1: LocalTerm"), "{text}");
    assert!(!text.contains("author:1: LocalTerm"), "{text}");
}

#[test]
fn a_term_split_by_an_invisible_character_is_found_in_every_mode() {
    let chars = [
        '\u{00AD}', '\u{034F}', '\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{2061}',
        '\u{2062}', '\u{2063}', '\u{2064}', '\u{FEFF}', '\u{200E}', '\u{200F}', '\u{202A}',
        '\u{202E}', '\u{2066}', '\u{2069}', '\u{061C}', '\u{180E}',
    ];
    let body: String = chars.iter().map(|c| format!("Bracken{c}fold\n")).collect();
    let want: Vec<i64> = (1..=chars.len() as i64).collect();
    let local_lines = |stdout: &str| -> Vec<i64> {
        let v: Vec<serde_json::Value> = serde_json::from_str(stdout).expect("json array");
        let mut lines: Vec<i64> = v
            .iter()
            .filter(|f| f["rule"] == "LocalTerm")
            .map(|f| f["line"].as_i64().unwrap())
            .collect();
        lines.sort();
        lines
    };
    let setup = || {
        let repo = new_repo();
        write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
        write(repo.path(), "privacy-terms.local.txt", "Brackenfold\n");
        write(repo.path(), "a.md", &body);
        repo
    };
    let repo = setup();
    git(repo.path(), &["add", "-A"]);
    for args in [
        vec!["privacy", "--json", "--staged"],
        vec!["privacy", "--json", "--stdin", "a.md"],
    ] {
        let out = xtask(repo.path())
            .args(&args)
            .write_stdin(body.clone())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {stdout}");
        assert_eq!(local_lines(&stdout), want, "{args:?}: {stdout}");
    }
    git(repo.path(), &["commit", "-q", "--no-verify", "-m", "add"]);
    for args in [
        vec!["privacy", "--json", "--all"],
        vec!["privacy", "--json", "--range", "HEAD"],
    ] {
        let out = xtask(repo.path()).args(&args).output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {stdout}");
        assert_eq!(local_lines(&stdout), want, "{args:?}: {stdout}");
    }
    // In a path name too.
    let t = Terms::generic().with_local("Brackenfold\n");
    assert_eq!(scan_path_name("docs/bracken\u{200B}fold.md", &t).len(), 1);
}

#[test]
fn colon_hex_runs_that_are_not_hardware_addresses_are_clean() {
    let pairs = |n: usize, sep: &str| -> String {
        (0..n)
            .map(|i| format!("{:02x}", (i * 37 + 11) % 256))
            .collect::<Vec<_>>()
            .join(sep)
    };
    let mut wrongly: Vec<String> = Vec::new();
    for ok in [
        // TLS and SSH fingerprints: a longer run of hexadecimal pairs
        format!("SHA256 Fingerprint={}", pairs(32, ":")),
        format!("fp {} end", pairs(16, ":")),
        format!("fp {} end", pairs(12, ":")),
        pairs(20, "-"),
        // all-zero and broadcast
        ["00"; 6].join(":"),
        ["ff"; 6].join(":"),
        ["FF"; 6].join("-"),
        // the documentation blocks (unicast and multicast)
        format!("{}:{}", "00:00:5e:00:53", "2a"),
        format!("{}-{}", "00-00-5E-00-53", "FF"),
        format!("{}:{}", "01:00:5e:90:10", "00"),
    ] {
        if all_rules(&ok).contains(&RuleId::PrivateAddress) {
            wrongly.push(ok);
        }
    }
    assert!(wrongly.is_empty(), "flagged: {wrongly:?}");
    for bad in [
        format!(
            "ether {} brd",
            ["02", "42", "ac", "11", "00", "02"].join(":")
        ),
        format!("{} next", ["0a", "1b", "2c", "3d", "4e", "5f"].join("-")),
        // next to the documentation block, not in it
        format!("{}:{}", "00:00:5e:00:54", "01"),
        format!("{}:{}", "00:00:00:00:00", "01"),
        // a colon and text after it that is not a hexadecimal pair
        format!("mac={}: up", ["02", "42", "ac", "11", "00", "02"].join(":")),
        // a word that ends in two hexadecimal letters is not a pair
        format!("mac:{}", ["02", "42", "ac", "11", "00", "02"].join(":")),
        format!("{}:cafe", ["02", "42", "ac", "11", "00", "02"].join(":")),
    ] {
        assert!(all_rules(&bad).contains(&RuleId::PrivateAddress), "{bad}");
    }
}

#[test]
fn a_local_name_next_to_a_non_ascii_letter_is_flagged_by_either_reading() {
    // Unicode boundaries start the name at the dash, ASCII ones at the word
    // that holds an underscore (which reads as code): either one flagging is
    // enough.
    let text = format!("\u{e9}{}.{}", "a_b-printer", "local");
    assert!(all_rules(&text).contains(&RuleId::PrivateHost), "{text}");
    // ASCII boundaries find a name glued to a non-ASCII letter.
    let text = format!("\u{e9}{}.{}", "printer", "local");
    assert!(all_rules(&text).contains(&RuleId::PrivateHost), "{text}");
}

#[test]
fn a_local_field_on_a_settings_object_is_code() {
    for ok in [
        "let x = cfg.local;",
        "foo(opts.local)",
        "if (process.env.LOCAL) {",
        "from myapp.settings.local import *",
        "config.lan = 1",
        "return args.local, ctx.lan",
    ] {
        assert!(all_rules(ok).is_empty(), "flagged: {ok}");
    }
    for bad in [
        format!("ping {}.{}", "wrens-laptop", "local"),
        format!("({}.{})", "nas", "lan"),
        format!("{}.{};", "box", "local"),
        format!("server_name {}.{};", "kiosk", "local"),
    ] {
        assert!(all_rules(&bad).contains(&RuleId::PrivateHost), "{bad}");
    }
}

#[test]
fn an_ipv6_address_glued_to_a_word_by_a_colon_is_flagged() {
    // `~` splits each prefix so this file holds no address literally.
    for bad in [
        "addr:fd~12:3456::1",
        "v=spf1 ip6:fd~00::1 -all",
        "listen:fe~80::1%en0",
    ] {
        let bad = bad.replace('~', "");
        assert_eq!(all_rules(&bad), vec![RuleId::PrivateAddress], "{bad}");
    }
    for ok in [
        "addr:2001:db8::1",
        "time:12:34:56",
        "std::fmt::Display",
        "a.b::c",
    ] {
        assert!(all_rules(ok).is_empty(), "flagged: {ok}");
    }
}

#[test]
fn double_and_partial_percent_encoding_of_a_home_path_is_read() {
    let u = user();
    for bad in [
        format!("https://example.com/?p=%252F{}%252F{}%252Fx", "Users", u),
        format!("https://example.com/?p=/{}%2F{}", "Users", u),
        format!("https://example.com/?p=%2F{}/{}/x", "Users", u),
        format!("https://example.com/?p=%2f{}%252f{}", "home", u),
        format!("https://example.com/?p=C%253A%255C{}%255C{}", "Users", u),
        format!("p=c%3A%255c{}%5C{}", "users", u),
    ] {
        assert_eq!(all_rules(&bad), vec![RuleId::HomePath], "{bad}");
    }
    // A placeholder name stays clean in every spelling.
    for ok in [
        format!("p=%252F{}%252F{}", "Users", "example"),
        format!("p=C%253A%255C{}%255C{}", "Users", "user"),
    ] {
        assert!(all_rules(&ok).is_empty(), "flagged: {ok}");
    }
}

#[test]
fn a_stray_nul_in_the_term_list_is_named_as_one() {
    for (list, want, not) in [
        (
            b"Zebra\0corp\n".to_vec(),
            "a NUL byte at offset 5",
            "UTF-16",
        ),
        (b"\0".to_vec(), "a NUL byte at offset 0", "UTF-16"),
        // one pair in two with a NUL is not most of them
        (b"a\0bc".to_vec(), "a NUL byte at offset 1", "UTF-16"),
        (
            utf16le("Zebracorp\n", false),
            "looks like UTF-16",
            "a NUL byte at",
        ),
    ] {
        let repo = new_repo();
        write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
        fs::write(repo.path().join("privacy-terms.local.txt"), &list).unwrap();
        write(repo.path(), "a.md", "harmless\n");
        commit_all(repo.path(), "add");
        let (code, text) = run(repo.path(), &["privacy", "--all"]);
        assert_eq!(code, 2, "{text}");
        assert!(text.contains(want) && !text.contains(not), "{text}");
    }
}

#[test]
fn the_git_config_helper_file_does_not_pile_up() {
    // One fixed file per test binary, written once, never one per run.
    let name = empty_gitconfig()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_eq!(name, "xtask-mut-gitconfig");
}

// ---- fix round 2, item 1: rule 5 sees through quotes, brackets, keys and
// every Unicode space separator ----

#[test]
fn a_spaced_absolute_path_in_quotes_brackets_or_after_a_key_is_flagged() {
    let u = user();
    let tail = format!("{}/{}/notes.txt", "home", u);
    let mut missed: Vec<String> = Vec::new();
    for bad in [
        format!("/mnt/Backup Disk/{tail}"),
        format!("\"/media/{u}/Backup Disk/{tail}\""),
        format!("'/media/{u}/Backup Disk/{tail}'"),
        format!("{{\"path\": \"/mnt/Backup Disk/{tail}\"}}"),
        format!("path=/mnt/Backup Disk/{tail}"),
        format!("src:/mnt/Backup Disk/{tail}"),
        format!("(/mnt/Backup Disk/{tail})"),
        format!("[/mnt/Backup Disk/{}/{}]", "home", u),
        format!("x=\"/srv/old disk/{}/{}\"", "home", u),
        format!("path = \"/Volumes/Backup Disk/{tail}\""),
        format!("\"/mnt/My Drive/{tail}\""),
        format!("--dir=/mnt/Backup Disk/{tail}"),
        format!("-v \"/mnt/Backup Disk/{}/{}:/data\"", "home", u),
        format!("dir: /mnt/Backup Disk/{}/{}", "home", u),
        format!("<~/My Files/{tail}>"),
        format!("a,/mnt/Backup Disk/{tail}"),
        format!("k;/mnt/Backup Disk/{tail}"),
        format!("\u{201C}/mnt/Backup Disk/{tail}\u{201D}"),
        // a space separator that is not the ASCII space
        format!("/mnt/Backup\u{a0}Disk/{tail}"),
        format!("\"/mnt/Backup\u{a0}Disk/{tail}\""),
        format!("/mnt/Backup\u{3000}Disk/{tail}"),
        format!("/mnt/Backup\u{2009}Disk/{tail}"),
        format!("/mnt/Backup\u{202F}Disk/{tail}"),
        // the drive test stays on the whole token
        format!("D:/Backup Disk/{tail}"),
        format!("\"D:/Backup Disk/{tail}\""),
    ] {
        if !home_flagged(&bad) {
            missed.push(bad);
        }
    }
    assert!(missed.is_empty(), "not flagged: {missed:#?}");
}

#[test]
fn prose_before_a_relative_module_path_stays_clean() {
    // The delimiter cut only flags when an absolute path follows the
    // delimiter; a word after a colon, a quote or a bracket does not.
    let mut wrongly: Vec<String> = Vec::new();
    for ok in [
        format!("see the docs in src/{}/mod.rs", "home"),
        format!("\"see src/{}/mod.rs\"", "home"),
        format!("see: src/{}/mod.rs", "users"),
        format!("(see src/{}/mod.rs)", "home"),
        format!("Note:docs src/{}/mod.rs", "home"),
        format!("x=docs src/{}/mod.rs", "users"),
        format!("\u{201C}see src/{}/mod.rs\u{201D}", "home"),
        // a quoted absolute path that is closed before the space
        format!("\"/etc/hosts\" src/{}/mod.rs", "home"),
        // a tab or two spaces are not a single space separator
        format!("/srv\tsrc/{}/mod.rs", "home"),
        format!("/srv  src/{}/mod.rs", "home"),
    ] {
        if home_flagged(&ok) {
            wrongly.push(ok);
        }
    }
    assert!(wrongly.is_empty(), "flagged: {wrongly:#?}");
}

// ---- fix round 2, item 4: residuals closed cheaply ----

#[test]
fn a_parent_folder_holding_a_delimiter_or_a_timestamp_is_flagged() {
    let u = user();
    let tail = format!("{}/{}/notes.txt", "home", u);
    let mut missed: Vec<String> = Vec::new();
    for bad in [
        format!("/backups/2026-11-18T10:20:30/{tail}"),
        format!("/backups/2026-11-18T10:20:30+00:00/{tail}"),
        format!("\"/backups/2026-11-18T10:20:30/{tail}\""),
        format!("(/snap/a,b/{tail})"),
        format!("/srv/a,b/{tail}"),
        format!("/opt/x:y/{tail}"),
        format!("/srv/a=b/{tail}"),
        format!("~/snap/10:20/{tail}"),
        format!("C:/backups/10:20/{tail}"),
        // a timestamp that holds a space
        format!("/snap/2026-11-18 10:20/{tail}"),
    ] {
        if !home_flagged(&bad) {
            missed.push(bad);
        }
    }
    assert!(missed.is_empty(), "not flagged: {missed:#?}");
    // A module path after a key, a colon, a quote or a bracket stays clean.
    for ok in [
        format!("x=src/{}/y", "home"),
        format!("a:src/{}/x", "users"),
        format!("(src/{}/x)", "home"),
        format!("\"src/{}/x\"", "users"),
        format!("[src/{}/x]", "home"),
        format!("<src/{}/x>", "home"),
    ] {
        assert!(!home_flagged(&ok), "flagged: {ok}");
    }
    // Accepted false positives: a relative entry after an absolute one in a
    // `PATH`-like list, and a module path right after a typographic quote
    // (rule 3's delimiters are ASCII, so the run does not end there).
    assert!(home_flagged(&format!("/usr/bin:src/{}/mod.rs", "home")));
    assert!(home_flagged(&format!("\u{201C}src/{}/x\u{201D}", "home")));
}

#[test]
fn a_mirrored_home_folder_under_a_relative_prefix_is_flagged() {
    let u = user();
    let mut missed: Vec<String> = Vec::new();
    for bad in [
        format!("mirror/{}/{}/.bashrc", "home", u),
        format!("rootfs/{}/{}/.ssh/config", "home", u),
        format!("./{}/{}/.profile", "home", u),
        format!("backup/{}/{}/Documents/x", "users", u),
        format!("src/{}/{}/Desktop", "home", u),
        format!("a/{}/{}/Library/Keychains", "home", u),
        format!("x/{}/{}/AppData/Roaming", "users", u),
        format!("x/{}/{}/downloads/a", "home", u),
        format!("x/{}/{}/Pictures", "home", u),
        format!("x/{}/{}/Music/a", "home", u),
        format!("x/{}/{}/Movies/a", "home", u),
        format!("x/{}/{}/Videos/a", "home", u),
        format!("x/{}/{}/Public/a", "home", u),
        // the segment ends at a quote, a space or a backslash
        format!("\"x/{}/{}/Desktop\"", "home", u),
        format!("x/{}/{}/Music files", "home", u),
        format!("x/{}/{}/Desktop\\x", "home", u),
    ] {
        if !home_flagged(&bad) {
            missed.push(bad);
        }
    }
    assert!(missed.is_empty(), "not flagged: {missed:#?}");
    for ok in [
        format!("src/{}/mod.rs", "home"),
        format!("src/{}/handlers.rs", "users"),
        format!("src/{}/widgets/button.rs", "home"),
        format!("src/{}/{}/desktop_view.rs", "home", u),
        format!("src/{}/{}/Documents.rs", "users", u),
        // still a residual: a folder that is not on the list
        format!("mirror/{}/{}/notes.txt", "home", u),
    ] {
        assert!(!home_flagged(&ok), "flagged: {ok}");
    }
    // As a staged path name too.
    let repo = new_repo();
    let name = format!("mirror/{}/{}/.bashrc", "home", u);
    let p = repo.path().join(&name);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, "harmless\n").unwrap();
    git(repo.path(), &["add", "-A"]);
    let (code, text) = run(repo.path(), &["privacy", "--staged"]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains(&format!("{name}: HomePath")), "{text}");
}

#[test]
fn json_escaped_unc_and_cygwin_home_paths_are_flagged() {
    let u = user();
    let mut wrong: Vec<String> = Vec::new();
    // (input, how many home findings): each path is reported once.
    for (text, want) in [
        // JSON-escaped slashes
        (format!("{{\"cwd\":\"\\/{}\\/{}\\/x\"}}", "home", u), 1),
        (format!("\"\\/{}\\/{}\"", "Users", u), 1),
        // UNC paths to a home share, single and doubled backslashes
        (format!("\\\\fileserver\\{}\\{}\\Documents", "Users", u), 1),
        (format!("\\\\wsl$\\Ubuntu\\{}\\{}\\x", "home", u), 1),
        (format!("\\\\wsl.localhost\\Ubuntu\\{}\\{}", "home", u), 1),
        (format!("\"\\\\\\\\server\\\\{}\\\\{}\"", "home", u), 1),
        (format!("\\\\?\\UNC\\server\\{}\\{}\\x", "Users", u), 1),
        // a device path is a drive path, reported once by the drive rule
        (format!("\\\\?\\C:\\{}\\{}\\x", "Users", u), 1),
        // Cygwin and MSYS2 homes, backslash and forward slash
        (format!("C:\\cygwin64\\{}\\{}\\x", "home", u), 1),
        (format!("C:\\cygwin\\{}\\{}", "home", u), 1),
        (format!("C:\\msys64\\{}\\{}\\.bashrc", "home", u), 1),
        (format!("D:\\tools\\msys64\\{}\\{}\\x", "home", u), 1),
        (format!("C:\\\\msys64\\\\{}\\\\{}", "home", u), 1),
        (format!("C:/msys64/{}/{}/x", "home", u), 1),
        (format!("C:/cygwin64/{}/{}/x", "home", u), 1),
        (format!("/cygdrive/c/{}/{}/x", "Users", u), 1),
        // clean: a share that is not a home folder, a placeholder name, a
        // Cygwin folder that is not `home`
        ("\\\\server\\share\\docs\\x".to_string(), 0),
        (format!("\\\\server\\{}\\Public\\x", "Users"), 0),
        (format!("C:\\msys64\\usr\\bin\\{}", "bash.exe"), 0),
    ] {
        let got = all_rules(&text)
            .iter()
            .filter(|r| **r == RuleId::HomePath)
            .count();
        if got != want {
            wrong.push(format!("{text:?}: {got} findings, want {want}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

// ---- fix round 2, item 2: UTF-16 without a mark is found again ----

/// Bytes that look random and hold no NUL.
fn nonzero_bytes(n: usize) -> Vec<u8> {
    (0..n as u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 11) as u8 | 0x80)
        .collect()
}

fn le16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn be16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

#[test]
fn a_short_utf16_path_inside_a_binary_is_found_across_a_window_edge() {
    // 14 characters, 7 on each side of the 64-pair boundary (byte 128).
    let path = format!("/{}/{}/", "home", "kestrel");
    assert_eq!(path.chars().count(), 14);
    for (name, enc) in [("le", le16(&path)), ("be", be16(&path))] {
        let mut bytes = nonzero_bytes(256);
        bytes.splice(114..114 + enc.len(), enc);
        let f = xtask::privacy::scan_bytes("b.bin", &bytes, &Terms::generic());
        assert!(
            f.iter().any(|x| x.rule == RuleId::HomePath),
            "{name}: {f:?}"
        );
    }
    // A five-letter term inside 4,000 random bytes, at an even offset.
    let t = Terms::generic().with_local("Quorv\n");
    let mut bytes = nonzero_bytes(4000);
    let enc = le16("quorv");
    bytes.splice(2000..2000 + enc.len(), enc);
    let f = xtask::privacy::scan_bytes("b.bin", &bytes, &t);
    assert!(f.iter().any(|x| x.rule == RuleId::LocalTerm), "{f:?}");
}

#[test]
fn utf16_text_in_a_non_latin_script_without_a_mark_is_found() {
    // Prose of CJK characters (no character with a zero byte), a newline
    // every 40 characters, holding an invented term of three characters.
    let term = "\u{9EBB}\u{96C0}\u{6797}";
    let t = Terms::generic().with_local(term);
    let line: String = (0..40u32)
        .map(|i| char::from_u32(0x4E01 + i * 3).unwrap())
        .collect();
    let mut text = String::new();
    for i in 0..6 {
        text.push_str(&line);
        if i == 3 {
            text.push_str(term);
        }
        text.push('\n');
    }
    for (name, enc) in [("le", le16(&text)), ("be", be16(&text))] {
        let f = xtask::privacy::scan_bytes("u.txt", &enc, &t);
        assert!(
            f.iter().any(|x| x.rule == RuleId::LocalTerm),
            "{name}: {f:?}"
        );
        // and in every mode
        let repo = new_repo();
        write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
        write(repo.path(), "privacy-terms.local.txt", &format!("{term}\n"));
        fs::write(repo.path().join("u.txt"), &enc).unwrap();
        git(repo.path(), &["add", "-A"]);
        assert_eq!(run(repo.path(), &["privacy", "--staged"]).0, 1, "{name}");
        git(repo.path(), &["commit", "-q", "--no-verify", "-m", "add"]);
        assert_eq!(run(repo.path(), &["privacy", "--all"]).0, 1, "{name}");
        assert_eq!(
            run(repo.path(), &["privacy", "--range", "HEAD"]).0,
            1,
            "{name}"
        );
        let out = xtask(repo.path())
            .args(["privacy", "--stdin", "u.txt"])
            .write_stdin(enc.clone())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{name}: {}", output_text(&out));
    }
    // An ASCII term inside such prose is found too.
    let t = Terms::generic().with_local("Brackenfold\n");
    let mixed = format!("{line}\n{line}\n{line}brackenfold{line}{line}{line}\n");
    let f = xtask::privacy::scan_bytes("u.txt", &le16(&mixed), &t);
    assert!(f.iter().any(|x| x.rule == RuleId::LocalTerm), "{f:?}");
}

// ---- fix round 2, item 3: Unicode's default-ignorable set ----

#[test]
fn default_ignorable_characters_are_refused_in_the_list_and_stripped_from_text() {
    for c in [
        '\u{FE0F}',
        '\u{E0001}',
        '\u{115F}',
        '\u{3164}',
        '\u{17B4}',
        '\u{1D173}',
    ] {
        let name = format!("U+{:04X}", c as u32);
        // Inside a term in the list: an error, in every mode.
        let repo = term_repo(format!("Bracken{c}fold\n").as_bytes());
        for args in EVERY_MODE {
            let (code, text) = run_mode(repo.path(), args);
            assert_eq!(code, 2, "{name} {args:?}: {text}");
            assert!(text.contains("offset 7;"), "{name} {args:?}: {text}");
            assert!(!text.to_lowercase().contains("bracken"), "{name}: {text}");
        }
        // Inside a term in scanned text: still found.
        let t = Terms::generic().with_local("Brackenfold\n");
        let f = scan_text("f.md", &format!("about Bracken{c}fold here"), &t);
        assert_eq!(f.len(), 1, "{name}: {f:?}");
        assert_eq!(f[0].rule, RuleId::LocalTerm, "{name}");
    }
}

// ---- fix round 2, item 5: commit identities are read whole ----

fn git_stdout(dir: &Path, args: &[&str], input: Option<&[u8]>) -> String {
    use std::io::Write;
    let mut child = isolated(std::process::Command::new("git"))
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.unwrap_or_default())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A repository whose term list is `list` and whose last commit has the
/// given raw author and committer identity bytes (written as a commit
/// object by hand, so any byte can be in them).
fn repo_with_identities(list: &str, author: &[u8], committer: &[u8]) -> TempDir {
    let repo = new_repo();
    write(repo.path(), ".gitignore", "privacy-terms.local.txt\n");
    write(repo.path(), "privacy-terms.local.txt", list);
    write(repo.path(), "a.md", "harmless\n");
    git(repo.path(), &["add", "-A"]);
    let tree = git_stdout(repo.path(), &["write-tree"], None);
    let mut object = format!("tree {tree}\n").into_bytes();
    for (key, who) in [("author", author), ("committer", committer)] {
        object.extend_from_slice(format!("{key} ").as_bytes());
        object.extend_from_slice(who);
        object.extend_from_slice(b" 1700000000 +0000\n");
    }
    object.extend_from_slice(b"\nan ordinary message\n");
    let oid = git_stdout(
        repo.path(),
        &[
            "hash-object",
            "-t",
            "commit",
            "--literally",
            "-w",
            "--stdin",
        ],
        Some(&object),
    );
    git(repo.path(), &["update-ref", "HEAD", &oid]);
    repo
}

#[test]
fn identities_that_are_not_utf8_or_hold_a_nul_are_still_checked() {
    let plain = b"Test User <test@example.com>".to_vec();
    let mail = format!("{}@{}", "ann", "corp.example");
    // (term list, author bytes, committer bytes, lines the report must hold)
    type Case<'a> = (&'a str, Vec<u8>, Vec<u8>, &'a [&'a str]);
    let cases: Vec<Case> = vec![
        // a Latin-1 name and address with an accented term
        (
            "Brackenf\u{eb}ld\n",
            b"Ann Brackenf\xebld <ann@example.com>".to_vec(),
            plain.clone(),
            &["author:1: LocalTerm"],
        ),
        (
            "Brackenf\u{eb}ld\n",
            plain.clone(),
            b"Test User <ann@brackenf\xebld.example.com>".to_vec(),
            &["committer:1: LocalTerm"],
        ),
        // a NUL inside the name cannot empty the fields
        (
            "Brackenfold\n",
            [b"Ann\0Brackenfold <".as_slice(), mail.as_bytes(), b">"].concat(),
            plain.clone(),
            &["author:1: LocalTerm", "author:1: Email"],
        ),
        // a control character inside the address
        (
            "Brackenfold\n",
            plain.clone(),
            [b"Test User <x\x01".as_slice(), mail.as_bytes(), b">"].concat(),
            &["committer:1: Email"],
        ),
        // no angle brackets at all: the identity is taken whole
        (
            "Brackenfold\n",
            b"Ann Brackenfold".to_vec(),
            plain.clone(),
            &["author:1: LocalTerm"],
        ),
    ];
    for (list, author, committer, want) in cases {
        let repo = repo_with_identities(list, &author, &committer);
        let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD"]);
        assert_eq!(code, 1, "{author:?} {committer:?}: {text}");
        for w in want {
            assert!(text.contains(w), "{w} in {author:?} {committer:?}: {text}");
        }
    }
    // Control: plain identities stay clean.
    let repo = repo_with_identities("Brackenfold\n", &plain, &plain);
    let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD"]);
    assert_eq!(code, 0, "{text}");
}

#[test]
fn a_comment_marker_is_not_the_start_of_an_absolute_path() {
    // Controller's ruling, fix round 2: a token that is exactly `//`, `///`,
    // `//!`, `/*`, `/**` or `/*!` does not begin an absolute path for rule 5.
    for ok in [
        format!("// src/{}/mod.rs", "home"),
        format!("/// (src/{}/x)", "home"),
        format!("//! src/{}/x", "users"),
        format!("/* src/{}/x */", "home"),
        format!("/** src/{}/x */", "home"),
        format!("/*! src/{}/x", "users"),
    ] {
        assert!(!home_flagged(&ok), "flagged: {ok}");
    }
    // Anything longer that begins with `/` still counts.
    let u = user();
    for bad in [
        format!("//server/Backup Disk/{}/{}/x", "home", u),
        format!("/* /mnt/Backup Disk/{}/{}/x */", "home", u),
        format!("// /mnt/Backup Disk/{}/{}/x", "home", u),
        format!("//x src/{}/{}/x", "home", u),
    ] {
        assert!(home_flagged(&bad), "not flagged: {bad}");
    }
}

// ---- fix round 3, item 2: no punctuation before a spaced absolute path hides it ----

/// The printable ASCII punctuation characters.
fn ascii_punctuation() -> Vec<char> {
    (0x21u8..0x7F)
        .map(char::from)
        .filter(char::is_ascii_punctuation)
        .collect()
}

/// Every Unicode space separator (general category Zs).
const SPACE_SEPARATORS: [char; 17] = [
    ' ', '\u{A0}', '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}',
    '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200A}', '\u{202F}', '\u{205F}',
    '\u{3000}',
];

#[test]
fn no_punctuation_before_a_spaced_absolute_path_hides_it() {
    // For every printable ASCII punctuation character p, the prefixes p, pp
    // and p glued to a letter (as in `-I`), before an absolute path whose
    // parent folder holds one space separator of any kind, with a lowercase
    // `home` and a capitalised `Users`: flagged.
    //
    // Named exception (the contract keeps it clean): a prefix made only of
    // `.` or `_`, alone or with a letter (`.`, `_`, `__`, `.I`, `_I`), makes
    // the path relative (`./mnt/Backup Disk/home/...`), so a lowercase `home`
    // there is a module folder by rules 3 and 5. `..` climbs and is flagged.
    // A capitalised `Users` is flagged whatever the prefix (rule 2).
    let u = user();
    let mut wrong: Vec<String> = Vec::new();
    let mut checked = 0;
    for p in ascii_punctuation() {
        for prefix in [p.to_string(), format!("{p}{p}"), format!("{p}I")] {
            let relative_prefix = matches!(prefix.as_str(), "." | "_" | "__" | ".I" | "_I");
            for sp in SPACE_SEPARATORS {
                for parent in [
                    format!("/Volumes/Backup{sp}Disk"),
                    format!("/mnt/Backup{sp}Disk"),
                    format!("/media/{u}/My{sp}Drive"),
                ] {
                    for folder in ["home", "Users"] {
                        let line = format!("{prefix}{parent}/{folder}/{u}/notes.txt");
                        let want = !(relative_prefix && folder == "home");
                        checked += 1;
                        if home_flagged(&line) != want {
                            wrong.push(format!("{line:?} should be flagged={want}"));
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 9000, "{checked}");
    assert!(
        wrong.is_empty(),
        "{} wrong, first ones: {:#?}",
        wrong.len(),
        &wrong[..wrong.len().min(20)]
    );
}

#[test]
fn a_relative_module_path_after_any_punctuation_keeps_its_contract() {
    // The same prefixes before a genuinely relative module path, glued and
    // after a space.
    //
    // Glued (`{prefix}src/home/mod.rs`): clean when p is one of rule 3's
    // delimiters or `_`. Deliberately flagged, harmless shapes (rule 3): a run
    // that starts with `.`, `-` or `/`, and a run glued to a character that
    // is neither a path character nor a delimiter (a module path glued to
    // the `+` of a patch line is a known false positive).
    //
    // After a space (`{prefix} src/home/mod.rs`): clean, except the
    // deliberately flagged shapes of rule 5: a lone absolute start (`/`,
    // `/I`, `~`, `~~`, `~I`) or a climb (`..`) before the module path. The
    // comment marker `//` (p doubled) is clean by the controller's ruling.
    const RUN_DELIMS: &str = "\"'`=,;|<>()[]{}:";
    let mut wrong: Vec<String> = Vec::new();
    for p in ascii_punctuation() {
        for prefix in [p.to_string(), format!("{p}{p}"), format!("{p}I")] {
            let glued_clean = RUN_DELIMS.contains(p) || p == '_';
            let spaced_clean = !matches!(prefix.as_str(), "/" | "/I" | "~" | "~~" | "~I" | "..");
            for module in [
                format!("src/{}/mod.rs", "home"),
                format!("./src/{}/mod.rs", "users"),
            ] {
                for (line, want_clean) in [
                    (format!("{prefix}{module}"), glued_clean),
                    (format!("{prefix} {module}"), spaced_clean),
                ] {
                    if home_flagged(&line) == want_clean {
                        wrong.push(format!("{line:?} should be clean={want_clean}"));
                    }
                }
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} wrong, first ones: {:#?}",
        wrong.len(),
        &wrong[..wrong.len().min(20)]
    );
    // Prose and plain relative tokens before a module path stay clean.
    for ok in [
        format!("docs/a src/{}/mod.rs", "home"),
        format!("see the docs in src/{}/mod.rs", "home"),
        format!("\"/etc/hosts\" src/{}/mod.rs", "home"),
    ] {
        assert!(!home_flagged(&ok), "flagged: {ok}");
    }
}

// ---- fix round 3, items 3 and 4 ----

#[test]
fn round_three_closures_of_named_residuals() {
    let u = user();
    let mut missed: Vec<String> = Vec::new();
    for bad in [
        // M3: a backslash before a percent-encoded home path
        format!("\\%2F{}%2F{}", "home", u),
        format!("x\\%2F{}%2F{}%2Fx", "home", u),
        // a keyed or compact-JSON timestamped parent folder
        format!(
            "path=/backups/2026-11-18T10:20:30/{}/{}/notes.txt",
            "home", u
        ),
        format!(
            "{{\"p\":\"/backups/2026-11-18T10:20:30/{}/{}\"}}",
            "home", u
        ),
        format!("\"-I/backups/10:20/{}/{}/include\"", "home", u),
        // a UNC path with more folders before the home folder
        format!("\\\\srv\\a\\b\\c\\d\\e\\{}\\{}\\x", "home", u),
        format!("\\\\srv\\a\\b\\c\\d\\{}\\{}", "Users", u),
    ] {
        if !home_flagged(&bad) {
            missed.push(bad);
        }
    }
    // M4: the standard home folders of every platform
    for folder in [
        "Templates",
        "Applications",
        "Sites",
        "Favorites",
        "Links",
        "Contacts",
        "Searches",
        "Saved Games",
        "3D Objects",
        "snap",
    ] {
        let bad = format!("mirror/{}/{}/{}/x", "home", u, folder);
        if !home_flagged(&bad) {
            missed.push(bad);
        }
    }
    assert!(missed.is_empty(), "not flagged: {missed:#?}");
    // Module paths that only look like those stay clean, and a `PATH` with
    // an absolute entry before a relative one is an accepted false positive.
    for ok in [
        format!("src/{}/{}/snapshots.rs", "home", u),
        format!("src/{}/{}/Saved.rs", "home", u),
        format!("x=src/{}/mod.rs", "home"),
    ] {
        assert!(!home_flagged(&ok), "flagged: {ok}");
    }
    assert!(home_flagged(&format!(
        "PATH=/usr/bin:src/{}/mod.rs",
        "home"
    )));
}

// ---- fix round 3, item 1: the whole identity line is checked ----

#[test]
fn an_address_in_an_identity_name_is_flagged() {
    let mail = format!("{}@{}", "ann", "corp.example");
    let plain = b"Test User <test@example.com>".to_vec();
    // Hand-made identities: a name that is an address, a name that holds an
    // address, and two bracketed parts (judged whole, as git shows the
    // first).
    for author in [
        format!("{mail} <p@example.com>"),
        format!("Ann {mail} <p@example.com>"),
        format!("Ann ({mail}) <p@example.com>"),
        format!("Ann <{mail}> <p@example.com>"),
    ] {
        let repo = repo_with_identities("Brackenfold\n", author.as_bytes(), &plain);
        let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD"]);
        assert_eq!(code, 1, "{author}: {text}");
        assert!(text.contains("author:1: Email"), "{author}: {text}");
    }
    // Through porcelain git: `user.name` set to an address, in the author and
    // the committer line.
    let repo = new_repo();
    write(repo.path(), "a.md", "harmless\n");
    git(repo.path(), &["add", "-A"]);
    let name = format!("user.name={mail}");
    git(
        repo.path(),
        &["-c", &name, "commit", "-q", "--no-verify", "-m", "add"],
    );
    let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("author:1: Email") && text.contains("committer:1: Email"),
        "{text}"
    );
    // An identity that is not UTF-8 is read twice (lossy and Latin-1); an
    // address found in both readings is reported once.
    let author = [b"Ann Brackenf\xebld <".as_slice(), mail.as_bytes(), b">"].concat();
    let repo = repo_with_identities("Quorvale\n", &author, &plain);
    let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD"]);
    assert_eq!(code, 1, "{text}");
    assert_eq!(text.matches("author:1: Email").count(), 1, "{text}");
    // The hosting service's no-reply addresses stay exempt exactly as before
    // (real domain on purpose: the rule is about that address).
    for who in [
        "Some One <12345+someone@users.noreply.github.com>",
        "GitHub <noreply@github.com>",
    ] {
        let repo = repo_with_identities("Brackenfold\n", who.as_bytes(), who.as_bytes());
        let (code, text) = run(repo.path(), &["privacy", "--range", "HEAD"]);
        assert_eq!(code, 0, "{who}: {text}");
    }
}

// ---- fix round 4 ----

#[test]
fn a_long_line_of_doubled_backslashes_is_scanned_in_linear_time() {
    // R2: 200 KB of escaped-string backslash segments with one real UNC home
    // path in the middle: one finding, within the linear-time bound.
    let bs = "\\\\";
    let k = 33_000;
    let name = format!("{}{}", "k", "v");
    let line = format!(
        "{bs}s{}{bs}{}{bs}{name}{}",
        format!("{bs}a").repeat(k),
        "home",
        format!("{bs}x").repeat(k)
    );
    let started = std::time::Instant::now();
    let f = scan_text("f.json", &line, &Terms::generic());
    let took = started.elapsed();
    let homes: Vec<&xtask::privacy::Finding> =
        f.iter().filter(|x| x.rule == RuleId::HomePath).collect();
    assert_eq!(homes.len(), 1, "{} findings", homes.len());
    assert!(
        homes[0].value.starts_with(&format!("{bs}s{bs}a")),
        "{}",
        homes[0].excerpt
    );
    assert!(
        homes[0]
            .value
            .ends_with(&format!("{bs}{}{bs}{name}", "home"))
    );
    assert!(took < LINEAR_BOUND, "took {took:?}");
    // A minified JSON array of escaped Windows and UNC paths with no home
    // folder: clean, and fast.
    let items: Vec<String> = (0..4000)
        .map(|i| {
            format!("\"D:{bs}data{bs}proj{bs}file{i}.txt\",\"{bs}{bs}srv{bs}share{bs}a{bs}b{i}\"")
        })
        .collect();
    let json = format!("[{}]", items.join(","));
    let started = std::time::Instant::now();
    let f = scan_text("f.json", &json, &Terms::generic());
    let took = started.elapsed();
    assert!(f.is_empty(), "{:?}", &f[..f.len().min(3)]);
    assert!(took < LINEAR_BOUND, "took {took:?}");
}

#[test]
fn text_in_another_script_glued_to_a_spaced_absolute_path_does_not_hide_it() {
    // R1: in rule 5, only ASCII letters, digits and path punctuation are path
    // characters; any non-ASCII character ends the token.
    let u = user();
    let tail = format!("/mnt/Backup Disk/{}/{}/notes.txt", "home", u);
    for prefix in ["\u{8DEF}\u{5F84}", "\u{3164}", "\u{FFA0}", "\u{e9}"] {
        let bad = format!("{prefix}{tail}");
        assert!(home_flagged(&bad), "not flagged: {bad:?}");
    }
    // A relative module path after prose in another script and a space
    // stays clean.
    for ok in [
        format!("\u{8DEF}\u{5F84} src/{}/mod.rs", "home"),
        format!("\u{65E5}\u{672C}\u{8A9E}\u{306E} ./src/{}/mod.rs", "users"),
        format!("caf\u{e9} src/{}/mod.rs", "home"),
    ] {
        assert!(!home_flagged(&ok), "flagged: {ok:?}");
    }
}

#[test]
fn non_ascii_prefixes_before_a_spaced_absolute_path_are_flagged() {
    // The generated test of round 3, for a sample of non-ASCII prefixes:
    // CJK and Japanese text, accented Latin, the two invisible Hangul
    // fillers, a CJK comma and a section sign; alone, doubled and glued to a
    // letter, before every spaced parent with every space separator. All
    // flagged. Before a spaced relative module path: clean. Glued to a
    // module path: flagged (named, rule 3: the run must start with an ASCII
    // letter, digit or `_`, or end at a delimiter).
    let u = user();
    let mut wrong: Vec<String> = Vec::new();
    for p in [
        "\u{8DEF}\u{5F84}",
        "\u{65E5}\u{672C}\u{8A9E}\u{306E}",
        "\u{e9}",
        "\u{dc}n\u{ef}c\u{f6}d\u{e9}",
        "\u{3164}",
        "\u{FFA0}",
        "\u{3001}",
        "\u{a7}",
    ] {
        for prefix in [p.to_string(), format!("{p}{p}"), format!("{p}I")] {
            for sp in SPACE_SEPARATORS {
                for parent in [
                    format!("/mnt/Backup{sp}Disk"),
                    format!("/media/{u}/My{sp}Drive"),
                ] {
                    for folder in ["home", "Users"] {
                        let line = format!("{prefix}{parent}/{folder}/{u}/notes.txt");
                        if !home_flagged(&line) {
                            wrong.push(format!("not flagged: {line:?}"));
                        }
                    }
                }
            }
            for module in [
                format!("src/{}/mod.rs", "home"),
                format!("./src/{}/mod.rs", "users"),
            ] {
                let spaced = format!("{prefix} {module}");
                if home_flagged(&spaced) {
                    wrong.push(format!("flagged: {spaced:?}"));
                }
                let glued = format!("{prefix}{module}");
                if !home_flagged(&glued) {
                    wrong.push(format!("not flagged: {glued:?}"));
                }
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} wrong, first ones: {:#?}",
        wrong.len(),
        &wrong[..wrong.len().min(20)]
    );
}

// ---- fix round 5: a non-ASCII letter in the parent folder hides nothing ----

/// The prefixes of the generated tests: every printable ASCII punctuation
/// character and a sample of non-ASCII text, each alone, doubled and glued
/// to a letter.
fn generated_prefixes() -> Vec<String> {
    let non_ascii = [
        "\u{8DEF}\u{5F84}",
        "\u{65E5}\u{672C}\u{8A9E}\u{306E}",
        "\u{e9}",
        "\u{dc}n\u{ef}c\u{f6}d\u{e9}",
        "\u{3164}",
        "\u{FFA0}",
        "\u{3001}",
        "\u{a7}",
    ];
    let mut bases: Vec<String> = ascii_punctuation().iter().map(char::to_string).collect();
    bases.extend(non_ascii.iter().map(|s| s.to_string()));
    bases
        .iter()
        .flat_map(|p| [p.clone(), format!("{p}{p}"), format!("{p}I")])
        .collect()
}

#[test]
fn a_parent_folder_holding_a_non_ascii_letter_hides_no_spaced_absolute_path() {
    // Every prefix class of the generated tests before spaced parents whose
    // folder name before the space holds a non-ASCII letter (accented Latin,
    // kana, CJK, Cyrillic), with every space separator, `home` and `Users`:
    // flagged. The named exception of the round-3 test still holds: a prefix
    // made only of `.` or `_` (alone or with a letter) makes the path
    // relative, so a lowercase `home` there is a module folder.
    let u = user();
    let mut wrong: Vec<String> = Vec::new();
    let mut checked = 0;
    for prefix in generated_prefixes() {
        let relative_prefix = matches!(prefix.as_str(), "." | "_" | "__" | ".I" | "_I");
        for sp in SPACE_SEPARATORS {
            for parent in [
                format!("/mnt/B\u{e4}ckup{sp}Disk"),
                format!("/media/{u}/Cl\u{e9}{sp}USB"),
                format!("/mnt/\u{30D0}\u{30C3}\u{30AF}{sp}Disk"),
                format!("/srv/\u{6570}\u{636E}{sp}old"),
                format!("/srv/\u{414}\u{430}\u{43D}\u{43D}\u{44B}\u{435}{sp}old"),
            ] {
                for folder in ["home", "Users"] {
                    let line = format!("{prefix}{parent}/{folder}/{u}/notes.txt");
                    let want = !(relative_prefix && folder == "home");
                    checked += 1;
                    if home_flagged(&line) != want {
                        wrong.push(format!("{line:?} should be flagged={want}"));
                    }
                }
            }
        }
    }
    assert!(checked > 20_000, "{checked}");
    assert!(
        wrong.is_empty(),
        "{} wrong, first ones: {:#?}",
        wrong.len(),
        &wrong[..wrong.len().min(20)]
    );
}

// ---- the maintainer's decision on identity names (2026-11-18) ----

#[test]
fn a_name_equal_to_its_own_noreply_account_is_not_a_term_finding() {
    // The account name is invented and is also a term of the invented list.
    // The no-reply host is real on purpose: the decision is about exactly
    // the hosting service's no-reply address.
    let account = format!("{}{}", "bracken", "fold");
    let host = "users.noreply.github.com";
    let list = "Brackenfold\nQuorvale\n";
    let plain = b"Test User <test@example.com>".to_vec();
    let line = |who: String| who.into_bytes();
    let run_range = |author: Vec<u8>, committer: Vec<u8>| {
        let repo = repo_with_identities(list, &author, &committer);
        run(repo.path(), &["privacy", "--range", "HEAD"])
    };
    // Clean: the name is the account of its own no-reply address, in both
    // address forms and in another letter case.
    for who in [
        format!("{account} <12345+{account}@{host}>"),
        format!("{account} <{account}@{host}>"),
        format!("Brackenfold <12345+BrackenFold@{host}>"),
        format!("  {account}  <{account}@{host}>"),
    ] {
        let (code, text) = run_range(line(who.clone()), line(who.clone()));
        assert_eq!(code, 0, "{who}: {text}");
    }
    // Flagged: everything else stays checked whole.
    for who in [
        // the same name with an ordinary address
        format!("{account} <{account}@example.com>"),
        // the same name with a look-alike host (the no-reply host as a prefix)
        format!("{account} <12345+{account}@{host}.example.org>"),
        // the name followed by another word
        format!("{account} dev <12345+{account}@{host}>"),
        // a different listed term as the name, with the no-reply address
        format!("Quorvale <12345+{account}@{host}>"),
        // no digits before the plus: not the numbered form, so the account
        // is the whole local part
        format!("{account} <+{account}@{host}>"),
    ] {
        let (code, text) = run_range(line(who.clone()), plain.clone());
        assert_eq!(code, 1, "{who}: {text}");
        assert!(text.contains("author:1: LocalTerm"), "{who}: {text}");
    }
    // An identity that is not valid UTF-8 keeps both readings and no
    // exemption.
    let mut odd = account.clone().into_bytes();
    odd.push(0xff);
    odd.extend_from_slice(format!(" <12345+{account}@{host}>").as_bytes());
    let (code, text) = run_range(odd, plain.clone());
    assert_eq!(code, 1, "{text}");
    // Text after the address is still checked.
    let tail = format!("{account} <12345+{account}@{host}> Quorvale");
    let (code, text) = run_range(line(tail.clone()), plain.clone());
    assert_eq!(code, 1, "{tail}: {text}");
    assert!(text.contains("author:1: LocalTerm"), "{text}");
    // The author qualifies, the committer does not: one finding, on the
    // committer line.
    let (code, text) = run_range(
        line(format!("{account} <12345+{account}@{host}>")),
        line(format!("{account} <{account}@example.com>")),
    );
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("committer:1: LocalTerm"), "{text}");
    assert!(!text.contains("author:1:"), "{text}");
    assert!(text.contains("privacy: 1 finding(s)"), "{text}");
}

// ---- the maintainer's second decision on identity names (2026-11-18) ----

#[test]
fn the_repository_s_own_commit_name_with_a_noreply_address_is_not_a_term_finding() {
    // The repository's configured name is an invented two-word name that is
    // also a term of the invented list. The global and system git
    // configuration of every command here are empty files in the test root
    // (see `isolated` and `xtask`), so nothing depends on the real user's.
    let own = format!("{} {}", "Merrin", "Tallowby");
    let host = "users.noreply.github.com";
    let list = format!("{own}\nQuorvale\n");
    let plain = b"Test User <test@example.com>".to_vec();
    let run_range = |author: String, committer: Vec<u8>, configured: bool| {
        let repo = repo_with_identities(&list, author.as_bytes(), &committer);
        if configured {
            git(repo.path(), &["config", "user.name", &own]);
        } else {
            git(repo.path(), &["config", "--unset", "user.name"]);
        }
        run(repo.path(), &["privacy", "--range", "HEAD"])
    };
    // The configured name never shows in the output except inside a local
    // term's own excerpt (which is the term as listed, lowercased).
    let shows_name = |text: &str| {
        text.lines()
            .map(|l| l.split(": LocalTerm").next().unwrap_or(l))
            .any(|l| l.to_lowercase().contains(&own.to_lowercase()))
    };
    // Clean: the configured name with the no-reply address, both forms.
    for who in [
        format!("{own} <12345+mtallowby@{host}>"),
        format!("{own} <mtallowby@{host}>"),
    ] {
        let (code, text) = run_range(who.clone(), who.clone().into_bytes(), true);
        assert_eq!(code, 0, "{who}: {text}");
    }
    // Flagged: everything else is checked as before.
    let lower = own.to_lowercase();
    for (who, configured) in [
        (format!("{own} <merrin@example.com>"), true),
        (format!("{own} <12345+mtallowby@{host}.example.org>"), true),
        (format!("{lower} <12345+mtallowby@{host}>"), true),
        (format!("{own} Jr <12345+mtallowby@{host}>"), true),
        (format!("Quorvale <12345+mtallowby@{host}>"), true),
        // no name configured: the rule does not apply
        (format!("{own} <12345+mtallowby@{host}>"), false),
    ] {
        let (code, text) = run_range(who.clone(), plain.clone(), configured);
        assert_eq!(code, 1, "{who} configured={configured}: {text}");
        assert!(text.contains("author:1: LocalTerm"), "{who}: {text}");
        assert!(!shows_name(&text), "{who}: {text}");
        assert!(!text.contains(&own), "{who}: {text}");
    }
    // The author qualifies, the committer does not: one finding, on the
    // committer line, and an address in a name is still an Email finding.
    let (code, text) = run_range(
        format!("{own} <12345+mtallowby@{host}>"),
        format!("{own} <merrin@example.com>").into_bytes(),
        true,
    );
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("committer:1: LocalTerm") && !text.contains("author:1:"),
        "{text}"
    );
    let mail = format!("{}@{}", "merrin", "corp.example");
    let (code, text) = run_range(
        format!("{own} {mail} <12345+mtallowby@{host}>"),
        plain.clone(),
        true,
    );
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("author:1: Email"), "{text}");
}
