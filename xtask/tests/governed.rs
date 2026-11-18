//! Tests for `cargo xtask governed --files <list-file>` (task 1c, decision 0025).
//!
//! List-file format assumed by these tests: the changed paths, each followed by
//! one NUL byte (so a name holding a newline or a carriage return survives, as
//! the hosting service's API gives names as JSON strings). Annotations go to
//! standard output as `::warning file=<escaped path>,...::<message>`; the job
//! summary goes to the file named by `GITHUB_STEP_SUMMARY`.

use assert_cmd::Command;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

struct Run {
    code: Option<i32>,
    stdout: String,
    summary: Option<String>,
}

/// Write `names` as a NUL-terminated list and run the subcommand on it.
fn run_with(names: &[&str], summary: bool) -> Run {
    let dir = TempDir::new().unwrap();
    let list = dir.path().join("changed.list");
    let mut bytes = Vec::new();
    for n in names {
        bytes.extend_from_slice(n.as_bytes());
        bytes.push(0);
    }
    fs::write(&list, bytes).unwrap();
    run_on(dir.path(), &list, summary)
}

fn run_on(dir: &Path, list: &Path, summary: bool) -> Run {
    let summary_path = dir.join("summary.md");
    let mut cmd = Command::cargo_bin("xtask").unwrap();
    cmd.current_dir(dir)
        .arg("governed")
        .arg("--files")
        .arg(list);
    if summary {
        cmd.env("GITHUB_STEP_SUMMARY", &summary_path);
    } else {
        cmd.env_remove("GITHUB_STEP_SUMMARY");
    }
    let out = cmd.output().unwrap();
    Run {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        summary: fs::read_to_string(&summary_path).ok(),
    }
}

/// The `file=` property of every `::warning` line, as printed (still escaped).
fn flagged(stdout: &str) -> BTreeSet<String> {
    stdout
        .lines()
        .filter_map(|l| l.strip_prefix("::warning "))
        .map(|rest| {
            let props = rest.split("::").next().unwrap();
            props
                .split(',')
                .find_map(|p| p.strip_prefix("file="))
                .unwrap_or_else(|| panic!("no file= property in: {l}", l = rest))
                .to_string()
        })
        .collect()
}

#[test]
fn governed_list_matches_record() {
    // Every row of the governed list, with at least one concrete path each
    // (a nested path for each `**` row).
    let governed = [
        "xtask/src/privacy/mod.rs",
        "xtask/src/privacy/rules/deep/terms.rs",
        "xtask/src/hooks.rs",
        "xtask/src/governed.rs",
        "xtask/tests/privacy.rs",
        "xtask/tests/privacy_mutants.rs",
        "xtask/tests/secrets.rs",
        "xtask/tests/hooks.rs",
        "xtask/tests/github_committer.rs",
        "xtask/tests/governed.rs",
        "privacy-allow.toml",
        ".gitleaks.toml",
        ".github/workflows/ci.yml",
        ".github/workflows/nested/extra.yml",
        "deny.toml",
        ".cargo/config.toml",
        ".cargo/nested/deeper/x.toml",
        // Widenings ruled by the controller (record 0025): a `.cargo` folder
        // at any depth, and everything under `.github/`.
        "crates/x/.cargo/config.toml",
        "crates/udx-core/.cargo/config.toml",
        ".github/CODEOWNERS",
        ".github/ISSUE_TEMPLATE/bug.md",
        ".github/actions/setup/nested/action.yml",
    ];
    // Near misses: the brief's two, and paths just outside a pattern.
    let clean = [
        "xtask/src/release_check.rs",
        "docs/build/LEDGER.md",
        "xtask/src/privacy.rs",
        "xtask/src/privacy_extra/mod.rs",
        "xtask/src/privacyx/mod.rs",
        "xtask/src/hooks_extra.rs",
        "xtask/src/hook.rs",
        "xtask/src/governed_extra.rs",
        "xtask/src/main.rs",
        "xtask/tests/xprivacy.rs",
        "xtask/tests/privacy.txt",
        "xtask/tests/secrets_extra.rs",
        "xtask/tests/other.rs",
        "crates/udx-core/tests/hooks.rs",
        "privacy-allow.toml.bak",
        ".gitleaks.toml.old",
        "crates/udx-core/deny.toml",
        "deny.toml.sample",
        "github/workflows/ci.yml",
        ".cargo-extra/config.toml",
        "Cargo.toml",
        "README.md",
    ];
    let all: Vec<&str> = governed.iter().chain(clean.iter()).copied().collect();
    let r = run_with(&all, true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);

    let want: BTreeSet<String> = governed.iter().map(|s| s.to_string()).collect();
    assert_eq!(flagged(&r.stdout), want);

    let summary = r.summary.expect("job summary file was not written");
    for p in governed {
        assert!(summary.contains(p), "summary lacks {p}:\n{summary}");
    }
    for p in clean {
        assert!(!summary.contains(p), "summary names clean {p}:\n{summary}");
        assert!(!r.stdout.contains(p), "stdout names clean {p}");
    }
    // A table: a header row, a separator row, and one row per flagged file.
    let rows = summary.lines().filter(|l| l.starts_with('|')).count();
    assert_eq!(rows, governed.len() + 2, "summary:\n{summary}");
}

#[test]
fn governed_names_escaped_in_annotations() {
    let nasty_a = ".github/workflows/a\n::error::pwn\r%41:x,y=z.yml";
    let nasty_b = ".cargo/a,line=1,title=T::b";
    let r = run_with(&[nasty_a, nasty_b, "deny.toml"], true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);

    // No carriage return and no stray workflow command in the output: every
    // line that starts with `::` is one of the tool's own warnings.
    assert!(!r.stdout.contains('\r'), "raw CR in output: {:?}", r.stdout);
    let command_lines: Vec<&str> = r.stdout.lines().filter(|l| l.starts_with("::")).collect();
    assert_eq!(command_lines.len(), 3, "stdout: {:?}", r.stdout);
    for l in &command_lines {
        assert!(l.starts_with("::warning "), "foreign command: {l:?}");
    }
    assert!(
        !r.stdout.contains("a\n::error"),
        "raw name leaked: {:?}",
        r.stdout
    );

    // Each annotation has exactly one file= property, escaped as the workflow
    // command syntax requires for properties (% CR LF : and ,).
    for l in &command_lines {
        let props = l
            .strip_prefix("::warning ")
            .unwrap()
            .split("::")
            .next()
            .unwrap();
        let files = props.split(',').filter(|p| p.starts_with("file=")).count();
        assert_eq!(files, 1, "properties of {l:?}");
    }
    let got = flagged(&r.stdout);
    assert!(got.contains(".github/workflows/a%0A%3A%3Aerror%3A%3Apwn%0D%2541%3Ax%2Cy=z.yml"));
    assert!(got.contains(".cargo/a%2Cline=1%2Ctitle=T%3A%3Ab"));
    assert!(got.contains("deny.toml"));
    assert_eq!(got.len(), 3);

    // The summary stays one table: nothing but rows, headings and blank lines,
    // and no raw carriage return, so a name cannot add text outside the table.
    let summary = r.summary.expect("job summary file was not written");
    assert!(!summary.contains('\r'), "raw CR in summary");
    for l in summary.lines() {
        assert!(
            l.is_empty() || l.starts_with('|') || l.starts_with('#'),
            "stray summary line: {l:?}"
        );
    }
}

#[test]
fn governed_exits_zero_when_flagged() {
    let names = ["deny.toml", ".github/workflows/ci.yml", "README.md"];
    // With the summary variable set.
    let r = run_with(&names, true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    assert_eq!(flagged(&r.stdout).len(), 2);
    assert!(r.summary.expect("summary written").contains("deny.toml"));
    // With the summary variable unset: still exit 0, annotations still printed.
    let r = run_with(&names, false);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    assert_eq!(flagged(&r.stdout).len(), 2);
    // Nothing governed, and an empty list: exit 0, no warnings.
    let r = run_with(&["README.md", "docs/build/LEDGER.md"], true);
    assert_eq!(r.code, Some(0));
    assert!(flagged(&r.stdout).is_empty());
    let r = run_with(&[], true);
    assert_eq!(r.code, Some(0));
    assert!(flagged(&r.stdout).is_empty());
}

#[test]
fn governed_fails_on_unreadable_list() {
    // The subcommand exists and reads a good list (an unknown subcommand would
    // also exit nonzero, which must not count as passing).
    let ok = run_with(&["deny.toml"], false);
    assert_eq!(ok.code, Some(0), "stdout: {}", ok.stdout);
    let dir = TempDir::new().unwrap();
    // A list file that does not exist.
    let r = run_on(dir.path(), &dir.path().join("missing.list"), true);
    assert!(matches!(r.code, Some(c) if c != 0), "missing: {:?}", r.code);
    assert!(flagged(&r.stdout).is_empty());
    // A path that is a folder, not a file.
    let r = run_on(dir.path(), dir.path(), true);
    assert!(matches!(r.code, Some(c) if c != 0), "folder: {:?}", r.code);
    assert!(flagged(&r.stdout).is_empty());
}

#[test]
fn governed_names_escaped_in_summary() {
    let nasty = ".cargo/a|b`c`<img src=x>\nd";
    let r = run_with(&[nasty, "deny.toml"], true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    let summary = r.summary.expect("job summary file was not written");
    assert!(!summary.contains('<'), "raw < in summary:\n{summary}");
    assert!(!summary.contains('\r'), "raw CR in summary");
    for l in summary.lines() {
        assert!(
            l.is_empty() || l.starts_with('|') || l.starts_with('#'),
            "stray summary line: {l:?}"
        );
    }
    // Header, separator and one row for each of the two flagged names.
    let rows: Vec<&str> = summary.lines().filter(|l| l.starts_with('|')).collect();
    assert_eq!(rows.len(), 4, "summary:\n{summary}");
    // Each row has exactly its two cells: three unescaped pipes, no more.
    for row in rows {
        assert_eq!(row.matches('|').count(), 3, "row: {row:?}");
    }
    assert!(!summary.contains('`'), "raw backtick in summary");
}
