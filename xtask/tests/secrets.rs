//! Task 1d: the privacy gate's local secret scan, driven through the command
//! line in the two modes the pre-commit hook and CI use (`--staged`, `--all`).
//!
//! Assumed output contract (see the task report): a secret finding's rule name
//! starts with `Secret`, it is reported at `path:line` like every other
//! finding, and the tool never prints the secret itself: any stretch of 12 or
//! more characters of the secret's body must be absent from stdout and stderr,
//! in text and in `--json` form. (A short prefix such as the provider's key
//! prefix may be shown.)
//!
//! Every key-shaped value here is built at run time from short parts; nothing
//! key-shaped is written whole in this file.

mod common;

use common::*;
use std::path::Path;

/// Stages `body` as `secret.txt` in a repository that already has a clean commit.
fn staged_repo(body: &str) -> tempfile::TempDir {
    let repo = new_repo();
    write(repo.path(), "readme.md", "nothing here\n");
    commit_all(repo.path(), "base");
    write(repo.path(), "secret.txt", body);
    git(repo.path(), &["add", "secret.txt"]);
    repo
}

fn committed_repo(body: &str) -> tempfile::TempDir {
    let repo = new_repo();
    write(repo.path(), "secret.txt", body);
    commit_all(repo.path(), "add");
    repo
}

fn assert_never_printed_in_full(hidden: &str, text: &str, what: &str) {
    let chars: Vec<char> = hidden.chars().collect();
    for w in chars.windows(12) {
        let piece: String = w.iter().collect();
        assert!(
            !text.contains(&piece),
            "{what}: output shows 12 or more characters of the secret's body\n{text}"
        );
    }
}

fn json_findings(dir: &Path, mode: &str) -> (i32, Vec<serde_json::Value>, String) {
    let out = xtask(dir)
        .args(["privacy", mode, "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let findings = match (stdout.find('['), stdout.rfind(']')) {
        (Some(s), Some(e)) => serde_json::from_str(&stdout[s..=e]).unwrap_or_default(),
        _ => Vec::new(),
    };
    (out.status.code().unwrap(), findings, output_text(&out))
}

#[test]
fn secret_patterns_flag_each_vector() {
    for vector in vectors() {
        let body = file_with(&vector.text);
        for mode in ["--staged", "--all"] {
            let repo = if mode == "--staged" {
                staged_repo(&body)
            } else {
                committed_repo(&body)
            };
            let what = format!("{} ({mode})", vector.name);

            // Text form: exit 1, the finding names a Secret rule and path:line.
            let (code, text) = run(repo.path(), &["privacy", mode]);
            assert_eq!(code, 1, "{what}: expected a finding\n{text}");
            assert!(
                text.contains("secret.txt:3"),
                "{what}: no path:line\n{text}"
            );
            assert!(text.contains("Secret"), "{what}: no rule name\n{text}");
            assert_never_printed_in_full(&vector.hidden, &text, &what);

            // JSON form: a Secret rule at line 3 of the file, and nothing else.
            let (code, findings, raw) = json_findings(repo.path(), mode);
            assert_eq!(code, 1, "{what}\n{raw}");
            assert!(
                findings.iter().any(|f| f["path"] == "secret.txt"
                    && f["line"] == 3
                    && f["rule"].as_str().is_some_and(|r| r.starts_with("Secret"))),
                "{what}: no Secret finding at secret.txt line 3\n{raw}"
            );
            assert!(
                findings
                    .iter()
                    .all(|f| f["rule"].as_str().is_some_and(|r| r.starts_with("Secret"))),
                "{what}: a vector must trip only the secret rules\n{raw}"
            );
            assert_never_printed_in_full(&vector.hidden, &raw, &format!("{what} json"));
        }
    }
}

#[test]
fn secret_patterns_ignore_look_alikes() {
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        body(50, 8, "0123456789abcdef"),
        body(51, 4, "0123456789abcdef"),
        body(52, 4, "0123456789abcdef"),
        body(53, 4, "0123456789abcdef"),
        body(54, 12, "0123456789abcdef"),
    );
    let look_alikes: Vec<(&str, String)> = vec![
        (
            "anthropic prefix then EXAMPLE",
            format!("{}{}EXAMPLE", "sk-", "ant-api03-"),
        ),
        (
            "openai prefix then EXAMPLE",
            format!("{}{}EXAMPLE", "sk-", "proj-"),
        ),
        (
            "cloud access key documentation value",
            format!("{}{}{}", "AKIA", "IOSFODNN7", "EXAMPLE"),
        ),
        ("hosting prefix then EXAMPLE", format!("{}EXAMPLE", "ghp_")),
        (
            "hosting prefix, body that starts with EXAMPLE",
            format!("{}EXAMPLE{}", "ghp_", "0".repeat(29)),
        ),
        ("sha-1 hash", body(60, 40, "0123456789abcdef")),
        (
            "sha-256 hash",
            format!("checksum = \"{}\"", body(61, 64, "0123456789abcdef")),
        ),
        ("uuid", format!("id: {uuid}")),
        (
            "integrity digest (base64)",
            format!("integrity: sha256-{}=", body(62, 43, BASE64)),
        ),
        (
            "base64 of plain words",
            "dGhpcyBpcyBqdXN0IHNvbWUgcGxhaW4gd29yZHMgZm9yIHRoZSBkb2N1bWVudGF0aW9u".to_string(),
        ),
        (
            "hyphenated words that contain the openai prefix",
            "disk-usage-report-for-the-entire-cluster-this-week".to_string(),
        ),
        (
            "bearer placeholder in angle brackets",
            format!("{}{}<token>", "Authorization: ", "Bearer "),
        ),
        (
            "bearer placeholder variable",
            format!("{}{}$TOKEN", "Authorization: ", "Bearer "),
        ),
        (
            "bearer with the word token",
            format!("curl -H \"{}{}YOUR_TOKEN\"", "Authorization: ", "Bearer "),
        ),
        (
            "certificate block (public)",
            format!("{}{}", "-----BEGIN ", "CERTIFICATE-----"),
        ),
        (
            "public key block",
            format!("{}{}", "-----BEGIN ", "PUBLIC KEY-----"),
        ),
    ];
    for (name, text) in look_alikes {
        let body = file_with(&text);
        for mode in ["--staged", "--all"] {
            let repo = if mode == "--staged" {
                staged_repo(&body)
            } else {
                committed_repo(&body)
            };
            let (code, out) = run(repo.path(), &["privacy", mode]);
            assert_eq!(code, 0, "{name} ({mode}) must stay clean\n{out}");
        }
    }
}

// ---- builder's tests: every mode, and nothing printed holds a secret ----

fn hosting_token(seed: u64) -> (String, String) {
    let b = body(seed, 36, ALNUM);
    (format!("{}{}", "gh", "p_") + &b, b)
}

/// A repository with a clean commit `base`, then a commit adding `secret.txt`
/// with `text` and a commit removing it again.
fn added_then_removed(text: &str) -> tempfile::TempDir {
    let repo = new_repo();
    write(repo.path(), "readme.md", "nothing here\n");
    commit_all(repo.path(), "base");
    git(repo.path(), &["tag", "base"]);
    write(repo.path(), "secret.txt", text);
    commit_all(repo.path(), "add");
    std::fs::remove_file(repo.path().join("secret.txt")).unwrap();
    commit_all(repo.path(), "remove");
    repo
}

#[test]
fn secret_rules_run_in_range_and_stdin_modes() {
    for vector in vectors() {
        let body = file_with(&vector.text);
        let what = vector.name;

        // A range whose last commit removes the secret still holds it.
        let repo = added_then_removed(&body);
        let (code, text) = run(repo.path(), &["privacy", "--range", "base..HEAD"]);
        assert_eq!(code, 1, "{what} (range)\n{text}");
        assert!(text.contains("secret.txt:3: Secret"), "{what}\n{text}");
        assert_never_printed_in_full(&vector.hidden, &text, what);
        let out = xtask(repo.path())
            .args(["privacy", "--range", "base..HEAD", "--json"])
            .output()
            .unwrap();
        assert_never_printed_in_full(&vector.hidden, &output_text(&out), what);

        // Standard input, plain and as a commit message.
        for extra in [&[][..], &["--git-message"][..]] {
            let out = xtask(repo.path())
                .args(["privacy", "--stdin", "msg"])
                .args(extra)
                .write_stdin(body.clone())
                .output()
                .unwrap();
            let text = output_text(&out);
            assert_eq!(
                out.status.code(),
                Some(1),
                "{what} (stdin {extra:?})\n{text}"
            );
            assert!(text.contains("msg:3: Secret"), "{what}\n{text}");
            assert_never_printed_in_full(&vector.hidden, &text, what);
        }
    }
}

#[test]
fn a_secret_in_a_commit_message_or_identity_is_found() {
    let (token, hidden) = hosting_token(80);
    let repo = new_repo();
    write(repo.path(), "a.md", "clean\n");
    commit_all(repo.path(), "base");
    git(repo.path(), &["tag", "base"]);
    write(repo.path(), "a.md", "clean again\n");
    commit_all(repo.path(), &format!("subject\n\nthe token was {token}"));
    let (code, text) = run(repo.path(), &["privacy", "--range", "base..HEAD"]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("message:3: SecretHostingToken"), "{text}");
    assert_never_printed_in_full(&hidden, &text, "message");

    // A token as the local part of the author's address: the address finding
    // on that line is hidden, the secret is named.
    let repo = new_repo();
    write(repo.path(), "a.md", "clean\n");
    commit_all(repo.path(), "base");
    git(repo.path(), &["tag", "base"]);
    write(repo.path(), "a.md", "clean again\n");
    git(repo.path(), &["add", "-A"]);
    git(
        repo.path(),
        &[
            "commit",
            "-q",
            "--no-verify",
            "-m",
            "x",
            &format!("--author=Pat Doe <{token}@corp.example>"),
        ],
    );
    let (code, text) = run(repo.path(), &["privacy", "--range", "base..HEAD"]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("author:1: SecretHostingToken"), "{text}");
    assert!(text.contains("author:1: Email: [excerpt hidden"), "{text}");
    assert_never_printed_in_full(&hidden, &text, "identity");
}

#[test]
fn other_findings_on_a_secret_line_never_print_it() {
    let (token, hidden) = hosting_token(81);
    let user_dir = format!("/{}/", "Users");
    // A home path whose folder name is the token, on the token's line.
    let line = format!("see {user_dir}{token}/x");
    // A key block whose body line holds a home path.
    let key_line = format!(
        "{}{user_dir}{}/{}",
        body(82, 20, ALNUM),
        body(83, 20, ALNUM),
        body(84, 20, ALNUM)
    );
    let block = format!(
        "{}{}\n{key_line}\n{}{}",
        "-----BEGIN ", "RSA PRIVATE KEY-----", "-----END ", "RSA PRIVATE KEY-----"
    );
    for (text, hidden) in [(line, hidden.clone()), (block, key_line)] {
        let repo = committed_repo(&file_with(&text));
        for json in [false, true] {
            let mut args = vec!["privacy", "--all"];
            if json {
                args.push("--json");
            }
            let (code, out) = run(repo.path(), &args);
            assert_eq!(code, 1, "{out}");
            assert!(out.contains("HomePath"), "{out}");
            assert!(out.contains("Secret"), "{out}");
            assert!(out.contains("excerpt hidden"), "{out}");
            assert_never_printed_in_full(&hidden, &out, "same line");
        }
    }
    // A home path on another line keeps its excerpt.
    let text = format!("{token}\nsee {user_dir}{}/x", "wrenfield");
    let repo = committed_repo(&text);
    let (_, out) = run(repo.path(), &["privacy", "--all"]);
    let shown = format!("secret.txt:2: HomePath: {user_dir}{}", "wrenfield");
    assert!(out.contains(&shown), "{out}");
}

#[test]
fn a_secret_in_a_file_name_is_never_printed() {
    let (token, hidden) = hosting_token(85);
    let name = format!("notes-{token}.md");
    let repo = new_repo();
    write(repo.path(), "readme.md", "nothing here\n");
    commit_all(repo.path(), "base");
    git(repo.path(), &["tag", "base"]);
    write(repo.path(), &name, "write to jane.roe@corp.example\n");
    commit_all(repo.path(), "add");
    for args in [
        &["privacy", "--all"][..],
        &["privacy", "--all", "--json"][..],
        &["privacy", "--range", "base..HEAD"][..],
    ] {
        let (code, out) = run(repo.path(), args);
        assert_eq!(code, 1, "{args:?}\n{out}");
        assert!(out.contains("SecretHostingToken"), "{out}");
        assert!(out.contains("Email"), "{out}");
        assert!(out.contains("notes-ghp_[36 characters hidden].md"), "{out}");
        assert_never_printed_in_full(&hidden, &out, "file name");
    }
}

#[test]
fn an_error_message_never_prints_a_secret() {
    let (token, hidden) = hosting_token(86);
    let repo = new_repo();
    write(repo.path(), "a.md", "clean\n");
    commit_all(repo.path(), "base");
    let (code, out) = run(repo.path(), &["privacy", "--range", &token]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("privacy: error:"), "{out}");
    assert!(out.contains("ghp_[36 characters hidden]"), "{out}");
    assert_never_printed_in_full(&hidden, &out, "error");
}

#[test]
fn the_allow_file_can_never_exempt_a_secret() {
    for kind in [
        "SecretAnthropicKey",
        "SecretOpenAiKey",
        "SecretCloudAccessKey",
        "SecretHostingToken",
        "SecretPrivateKey",
        "SecretBearer",
    ] {
        let repo = new_repo();
        write(repo.path(), "a.md", "harmless\n");
        write(
            repo.path(),
            "privacy-allow.toml",
            &format!(
                "[[allow]]\npath = \"a.md\"\nrule = \"{kind}\"\nvalues = [\"x\"]\nreason = \"test\"\n"
            ),
        );
        commit_all(repo.path(), "add");
        let (code, out) = run(repo.path(), &["privacy", "--all"]);
        assert_eq!(code, 2, "{kind}\n{out}");
        assert!(
            out.contains(&format!("`{kind}` can never be exempted")),
            "{out}"
        );
    }
}

#[test]
fn a_missing_local_list_is_a_warning_on_every_run() {
    let repo = committed_repo("clean\n");
    let (code, out) = run(repo.path(), &["privacy", "--all"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("privacy: warning: local terms are not being checked"),
        "{out}"
    );
}

// ---- fix round 1 ----

#[test]
fn an_allow_file_parse_error_names_the_place_and_never_the_text() {
    let line = body(90, 64, BASE64);
    let token = hosting_token(91).0;
    for (text, hidden, place) in [
        // Not TOML at all on line 3.
        (
            format!("# exceptions\n[[allow]]\n{line}\n"),
            line.clone(),
            "line 3, column 65",
        ),
        // TOML, but a value of the wrong type: the schema error would quote it.
        (
            format!(
                "[[allow]]\npath = \"a.md\"\nrule = \"Email\"\nvalues = \"x{token}\"\nreason = \"r\"\n"
            ),
            token.clone(),
            "line 4, column 10",
        ),
    ] {
        let repo = new_repo();
        write(repo.path(), "a.md", "harmless\n");
        write(repo.path(), "privacy-allow.toml", &text);
        commit_all(repo.path(), "add");
        let (code, out) = run(repo.path(), &["privacy", "--all"]);
        assert_eq!(code, 2, "{out}");
        assert!(
            out.contains(&format!(
                "invalid privacy-allow.toml: a TOML or schema error at {place}"
            )),
            "{out}"
        );
        assert_never_printed_in_full(&hidden, &out, "allow file error");
    }
}

#[test]
fn the_whole_raw_commit_object_is_scanned_for_secrets() {
    let (token, hidden) = hosting_token(92);
    let repo = new_repo();
    write(repo.path(), "a.md", "clean\n");
    commit_all(repo.path(), "base");
    let tree = git_out(repo.path(), &["rev-parse", "HEAD^{tree}"]);
    let parent = git_out(repo.path(), &["rev-parse", "HEAD"]);
    let who = "Test User <test@example.com> 1700000000 +0000";
    for (what, raw) in [
        (
            "an extra header",
            format!(
                "tree {tree}\nparent {parent}\nauthor {who}\ncommitter {who}\nx-note {token}\n\nplain\n"
            ),
        ),
        (
            "message text after a NUL",
            format!(
                "tree {tree}\nparent {parent}\nauthor {who}\ncommitter {who}\n\nplain\0 {token}\n"
            ),
        ),
    ] {
        let path = repo.path().join("raw-commit");
        std::fs::write(&path, raw).unwrap();
        let id = git_out(
            repo.path(),
            &[
                "hash-object",
                "--literally",
                "-t",
                "commit",
                "-w",
                "raw-commit",
            ],
        );
        std::fs::remove_file(&path).unwrap();
        let range = format!("{parent}..{id}");
        let (code, out) = run(repo.path(), &["privacy", "--range", &range]);
        assert_eq!(code, 1, "{what}\n{out}");
        assert!(out.contains("object:"), "{what}\n{out}");
        assert!(out.contains("SecretHostingToken"), "{what}\n{out}");
        assert_never_printed_in_full(&hidden, &out, what);
    }
    // A secret in the message is reported once, not again for the object.
    write(repo.path(), "a.md", "clean again\n");
    commit_all(repo.path(), &format!("subject\n\nthe token was {token}"));
    let (code, out) = run(repo.path(), &["privacy", "--range", "HEAD~1..HEAD"]);
    assert_eq!(code, 1, "{out}");
    assert_eq!(out.matches("SecretHostingToken").count(), 1, "{out}");
    assert!(out.contains("message:3: SecretHostingToken"), "{out}");
}

// ---- fix round 2 ----

#[test]
fn allow_entry_errors_name_the_line_and_never_the_text() {
    // A token after a letter is no secret to the rules, so only position
    // reporting keeps it out of the output.
    let token = format!("x{}", hosting_token(93).0);
    let hidden = hosting_token(93).1;
    for (entry, place, what) in [
        (
            format!("path = \"a.md\"\nrule = \"{token}\"\nvalues = [\"v\"]\nreason = \"r\"\n"),
            "line 4",
            "names an unknown rule",
        ),
        (
            format!("path = \"[{token}\"\nrule = \"Email\"\nvalues = [\"v\"]\nreason = \"r\"\n"),
            "line 3",
            "has a path that is not a valid glob",
        ),
        (
            format!("path = \"{token}\"\nrule = \"Email\"\nvalues = [\"v\"]\nreason = \" \"\n"),
            "line 3",
            "has no reason",
        ),
        (
            format!("path = \"{token}\"\nrule = \"Email\"\nvalues = []\nreason = \"r\"\n"),
            "line 3",
            "needs a non-empty `values` list",
        ),
    ] {
        let repo = new_repo();
        write(repo.path(), "a.md", "harmless\n");
        write(
            repo.path(),
            "privacy-allow.toml",
            &format!("# exceptions\n[[allow]]\n{entry}"),
        );
        commit_all(repo.path(), "add");
        let (code, out) = run(repo.path(), &["privacy", "--all"]);
        assert_eq!(code, 2, "{out}");
        let expected =
            format!("privacy-allow.toml {place}: the entry {what} (the text is not shown)");
        assert!(out.contains(&expected), "{out}");
        assert_never_printed_in_full(&hidden, &out, what);
    }
}
