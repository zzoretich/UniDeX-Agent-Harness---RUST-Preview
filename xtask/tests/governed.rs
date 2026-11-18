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

/// Run with raw list bytes and extra arguments.
fn run_raw(bytes: &[u8], extra: &[&str], summary: bool) -> Run {
    let dir = TempDir::new().unwrap();
    let list = dir.path().join("changed.list");
    fs::write(&list, bytes).unwrap();
    let summary_path = dir.path().join("summary.md");
    let mut cmd = Command::cargo_bin("xtask").unwrap();
    cmd.current_dir(dir.path())
        .arg("governed")
        .arg("--files")
        .arg(&list)
        .args(extra);
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

fn nul(names: &[&str]) -> Vec<u8> {
    let mut b = Vec::new();
    for n in names {
        b.extend_from_slice(n.as_bytes());
        b.push(0);
    }
    b
}

/// The first cell of every data row of the summary table (header and
/// separator rows left out), as written.
fn summary_names(summary: &str) -> BTreeSet<String> {
    summary
        .lines()
        .filter(|l| l.starts_with('|'))
        .skip(2)
        .map(|l| {
            l.trim_start_matches('|')
                .split(" | ")
                .next()
                .unwrap()
                .trim()
                .to_string()
        })
        .collect()
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
        "xtask/src/release_check.rs",
        "xtask/src/main.rs",
        "xtask/src/lib.rs",
        "xtask/Cargo.toml",
        "xtask/src/privacy.rs",
        "xtask/src/privacy_extra/mod.rs",
        "xtask/src/privacyx/mod.rs",
        "xtask/src/hooks_extra.rs",
        "xtask/src/hook.rs",
        "xtask/src/governed_extra.rs",
        "xtask/tests/xprivacy.rs",
        "xtask/tests/privacy.txt",
        "xtask/tests/secrets_extra.rs",
        "xtask/tests/other.rs",
        "xtask/tests/privacy.rs",
        "xtask/tests/privacy_mutants.rs",
        "xtask/tests/secrets.rs",
        "xtask/tests/hooks.rs",
        "xtask/tests/github_committer.rs",
        "xtask/tests/governed.rs",
        "privacy-allow.toml",
        ".gitleaks.toml",
        ".gitleaksignore",
        ".github/workflows/ci.yml",
        ".github/workflows/nested/extra.yml",
        "deny.toml",
        ".deny.toml",
        ".cargo/config.toml",
        ".cargo/nested/deeper/x.toml",
        ".gitattributes",
        "crates/x/.gitattributes",
        ".gitmodules",
        "Cargo.toml",
        "rust-toolchain.toml",
        "rust-toolchain",
        "CODEOWNERS",
        "docs/CODEOWNERS",
        "crates/udx-core/src/paths.rs",
        "crates/udx-core/src/lib.rs",
        "docs/decisions/0025-governed-paths-review.md",
        // Rulings of the controller: a `.cargo` folder, file or link at any
        // depth, and everything under `.github/`.
        "crates/x/.cargo/config.toml",
        "crates/udx-core/.cargo/config.toml",
        "crates/x/.cargo",
        ".cargo",
        ".github/CODEOWNERS",
        ".github/ISSUE_TEMPLATE/bug.md",
        ".github/actions/setup/nested/action.yml",
        // Letter case is folded.
        ".CARGO/config",
        "crates/x/.Cargo/config.toml",
        "DENY.TOML",
        "xtask/src/Privacy/mod.rs",
        ".GitHub/workflows/ci.yml",
    ];
    // Near misses: paths just outside a pattern.
    let clean = [
        "docs/build/LEDGER.md",
        "docs/decisions/0024-x.md",
        "crates/udx-core/tests/hooks.rs",
        "crates/udx-core/src/config.rs",
        "crates/udx-core/Cargo.toml",
        "cargo",
        "privacy-allow.toml.bak",
        ".gitleaks.toml.old",
        "crates/udx-core/deny.toml",
        "deny.toml.sample",
        "github/workflows/ci.yml",
        "crates/x/.github/workflows/ci.yml",
        ".cargo-extra/config.toml",
        "crates/x/.cargo-extra/config.toml",
        "CODEOWNERS.bak",
        "crates/x/CODEOWNERS",
        "Cargo.lock",
        "README.md",
    ];
    // Run 1: only the paths that must be flagged. (Run apart from the near
    // misses, because a near miss such as `github/workflows/ci.yml` is a plain
    // substring of a flagged path and could not otherwise be searched for.)
    let r = run_with(&governed, true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);

    let want: BTreeSet<String> = governed.iter().map(|s| s.to_string()).collect();
    assert_eq!(flagged(&r.stdout), want);

    let summary = r.summary.expect("job summary file was not written");
    for p in governed {
        assert!(summary.contains(p), "summary lacks {p}:\n{summary}");
    }
    // A table: a header row, a separator row, and one row per flagged file.
    let rows = summary.lines().filter(|l| l.starts_with('|')).count();
    assert_eq!(rows, governed.len() + 2, "summary:\n{summary}");

    // Run 2: only the near misses. Nothing is flagged, nothing is named.
    let r = run_with(&clean, true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    assert!(
        !r.stdout.lines().any(|l| l.starts_with("::")),
        "annotation for a near miss: {:?}",
        r.stdout
    );
    assert!(flagged(&r.stdout).is_empty());
    // The tool writes a heading and a plain line, and no table, for an empty result.
    let summary = r.summary.expect("job summary file was not written");
    assert!(summary.contains("## Governed paths"), "summary:\n{summary}");
    assert_eq!(
        summary.lines().filter(|l| l.starts_with('|')).count(),
        0,
        "table row for a near miss:\n{summary}"
    );
    for p in clean {
        assert!(!summary.contains(p), "summary names clean {p}:\n{summary}");
        assert!(!r.stdout.contains(p), "stdout names clean {p}");
    }

    // Run 3: both together. The flagged names are read back from the
    // annotations and from the summary rows and compared as exact sets, so no
    // substring search is needed and an ordering fault shows.
    let mut mixed: Vec<&str> = Vec::new();
    for i in 0..clean.len().max(governed.len()) {
        if let Some(c) = clean.get(i) {
            mixed.push(c);
        }
        if let Some(g) = governed.get(i) {
            mixed.push(g);
        }
    }
    let r = run_with(&mixed, true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    assert_eq!(flagged(&r.stdout), want);
    let summary = r.summary.expect("job summary file was not written");
    assert_eq!(summary_names(&summary), want, "summary:\n{summary}");
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

#[test]
fn governed_flags_previous_names() {
    // A governed file renamed out of the set: the list holds its new name and
    // its previous name; the previous name is flagged.
    let r = run_with(&["docs/x.toml", "deny.toml"], true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    let want: BTreeSet<String> = ["deny.toml".to_string()].into();
    assert_eq!(flagged(&r.stdout), want);
}

#[test]
fn governed_reads_name_status_output() {
    // `git diff --name-status -z --find-renames`: a status, then one path, or
    // two (old, new) for a rename or a copy; each ended by a NUL byte.
    let bytes = b"R100\0deny.toml\0docs/x.toml\0M\0README.md\0A\0xtask/src/new.rs\0C75\0docs/a.md\0.cargo/b.toml\0D\0docs/gone.md\0";
    let r = run_raw(bytes, &["--name-status"], true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    let want: BTreeSet<String> = ["deny.toml", "xtask/src/new.rs", ".cargo/b.toml"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(flagged(&r.stdout), want);
    assert_eq!(summary_names(&r.summary.unwrap()), want);
    // A rename that lacks its second path is an error, not a clean result.
    let r = run_raw(b"R100\0deny.toml\0", &["--name-status"], true);
    assert!(
        matches!(r.code, Some(c) if c != 0),
        "truncated: {:?}",
        r.code
    );
    assert!(flagged(&r.stdout).is_empty());
    // An empty path is an error too.
    let r = run_raw(b"M\0\0", &["--name-status"], true);
    assert!(
        matches!(r.code, Some(c) if c != 0),
        "empty path: {:?}",
        r.code
    );
    // Without the flag the same bytes are plain names, and the status words
    // are names that are not governed.
    let r = run_raw(b"M\0README.md\0", &[], true);
    assert_eq!(r.code, Some(0));
    assert!(flagged(&r.stdout).is_empty());
}

#[test]
fn governed_reports_an_incomplete_list() {
    let list = nul(&["README.md"]);
    // Fewer entries listed than the pull request changes: a warning without a
    // file, a summary line, and still exit 0.
    let r = run_raw(&list, &["--changed", "3500", "--listed", "3000"], true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    let warn: Vec<&str> = r.stdout.lines().filter(|l| l.starts_with("::")).collect();
    assert_eq!(warn.len(), 1, "stdout: {:?}", r.stdout);
    assert!(warn[0].starts_with("::warning "), "{:?}", warn[0]);
    assert!(
        warn[0].contains("incomplete (3000 of 3500)"),
        "{:?}",
        warn[0]
    );
    assert!(warn[0].contains("treat this pull request as governed"));
    let summary = r.summary.unwrap();
    assert!(
        summary.contains("incomplete (3000 of 3500)")
            && summary.contains("treat this pull request as governed"),
        "summary:\n{summary}"
    );
    // The same numbers: no incomplete warning.
    let r = run_raw(&list, &["--changed", "1", "--listed", "1"], true);
    assert_eq!(r.code, Some(0));
    assert!(!r.stdout.contains("incomplete"), "{:?}", r.stdout);
    assert!(!r.summary.unwrap().contains("incomplete"));
    // More listed than changed (a count that raced a push) is not incomplete.
    let r = run_raw(&list, &["--changed", "1", "--listed", "2"], true);
    assert!(!r.stdout.contains("incomplete"), "{:?}", r.stdout);
    // The two counts come together, and must be numbers.
    let r = run_raw(&list, &["--changed", "1"], true);
    assert!(matches!(r.code, Some(c) if c != 0));
    let r = run_raw(&list, &["--changed", "x", "--listed", "1"], true);
    assert!(matches!(r.code, Some(c) if c != 0));
}

#[test]
fn governed_prints_commit_ids_and_refuses_bad_ones() {
    let list = nul(&["deny.toml"]);
    let base = "a".repeat(40);
    let head = "b".repeat(64);
    let r = run_raw(&list, &["--base", &base, "--head", &head], true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    let summary = r.summary.unwrap();
    assert!(
        summary.contains(&base) && summary.contains(&head),
        "{summary}"
    );
    for l in summary.lines() {
        assert!(
            l.is_empty() || l.starts_with('|') || l.starts_with('#') || l.starts_with("Run for"),
            "stray summary line: {l:?}"
        );
    }
    // Not 40 or 64 hexadecimal characters: refused, nothing flagged.
    for bad in [
        "a".repeat(39),
        "a".repeat(41),
        format!("{}g", "a".repeat(39)),
        "main".to_string(),
        format!("{}\n::error::x", "a".repeat(40)),
    ] {
        let r = run_raw(&list, &["--base", &bad, "--head", &head], true);
        assert!(matches!(r.code, Some(c) if c != 0), "accepted {bad:?}");
        assert!(flagged(&r.stdout).is_empty());
    }
}

#[test]
fn governed_folds_case_and_flags_non_ascii_names() {
    // Built at run time: no such literal path is a file name in this repository.
    let long_s = '\u{17f}';
    let kelvin = '\u{212a}';
    let shadow = format!("xta{long_s}k/src/privacy/mod.rs");
    let shadow_k = format!("xtas{kelvin}/src/hooks.rs");
    let shadow_deny = format!("DENY.TOML{long_s}");
    let ordinary = format!("docs/caf{}.md", '\u{e9}');
    let r = run_with(&[&shadow, &shadow_k, &ordinary, "docs/plain.md"], true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    // Three annotations: the two shadows (a governed path once folded) and the
    // ordinary non-ASCII name (its own reason).
    let lines: Vec<&str> = r
        .stdout
        .lines()
        .filter(|l| l.starts_with("::warning "))
        .collect();
    assert_eq!(lines.len(), 3, "stdout: {:?}", r.stdout);
    assert!(!r.stdout.contains("plain"), "{:?}", r.stdout);
    let ord_line = lines.iter().find(|l| l.contains("caf")).unwrap();
    assert!(
        ord_line
            .contains("non-ASCII name: may shadow a governed path on a case-folding file system"),
        "{ord_line:?}"
    );
    // A shadow of a governed path carries that path's reason, not the
    // non-ASCII one alone.
    let sh_line = lines.iter().find(|l| l.contains("xta")).unwrap();
    assert!(sh_line.contains("privacy gate"), "{sh_line:?}");
    // Non-ASCII characters are written as escapes, never raw, so a name cannot
    // pass for another path in the annotations or the summary.
    assert!(
        r.stdout.is_ascii(),
        "raw non-ASCII in output: {:?}",
        r.stdout
    );
    let summary = r.summary.unwrap();
    assert!(summary.is_ascii(), "raw non-ASCII in summary:\n{summary}");
    assert!(
        summary.contains("\\u{17f}") && summary.contains("\\u{e9}"),
        "{summary}"
    );
    // A governed name folded from a non-ASCII letter alone is flagged.
    let r = run_with(&[&shadow_deny], true);
    assert_eq!(
        r.stdout
            .lines()
            .filter(|l| l.starts_with("::warning "))
            .count(),
        1
    );
}

#[test]
fn governed_annotation_message_names_reason_and_record() {
    let r = run_with(&["deny.toml", ".cargo/x"], false);
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{:?}", r.stdout);
    for l in lines {
        let msg = l
            .split_once("::")
            .and_then(|(_, rest)| rest.split_once("::"))
            .unwrap()
            .1;
        assert!(msg.contains("Governed file: "), "{msg:?}");
        assert!(msg.contains("decision 0025"), "{msg:?}");
        assert!(msg.contains("extra security review"), "{msg:?}");
    }
    let d = r.stdout.lines().find(|l| l.contains("deny.toml")).unwrap();
    assert!(d.contains("dependency policy"), "{d:?}");
}

#[test]
fn governed_control_and_markup_characters_in_summary() {
    let name = ".cargo/a\tb\u{1b}c[x](y)![i](j)*e*~s~";
    let r = run_with(&[name], true);
    let summary = r.summary.unwrap();
    assert!(
        !summary.contains('\t') && !summary.contains('\u{1b}'),
        "{summary:?}"
    );
    let row = summary.lines().find(|l| l.contains(".cargo/a")).unwrap();
    assert!(row.contains("a?b?c"), "{row:?}");
    for c in ['[', ']', '(', ')', '!', '*', '~'] {
        assert!(!row.contains(c), "raw {c} in {row:?}");
    }
    // The annotation holds the escape for the control characters too.
    assert!(!r.stdout.contains('\u{1b}'), "{:?}", r.stdout);
}

#[test]
fn governed_escaped_names_cannot_collide() {
    // The U+017F shadow of a governed path, and an ASCII decoy that spells out
    // the escape the shadow is printed with. Built at run time.
    let shadow = format!("xta{}k/src/privacy/mod.rs", '\u{17f}');
    let decoy = "xta\\u{17f}k/src/privacy/mod.rs";
    assert!(shadow.is_ascii() != decoy.is_empty() || !shadow.is_ascii());
    let r = run_with(&[&shadow, decoy], true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    // Print each alone to see what it is called.
    let lines: Vec<&str> = r
        .stdout
        .lines()
        .filter(|l| l.starts_with("::warning "))
        .collect();
    let alone = run_with(&[&shadow], true);
    let alone_decoy = run_with(&[decoy], true);
    let a = flagged(&alone.stdout);
    let d = flagged(&alone_decoy.stdout);
    assert_eq!(a.len(), 1);
    assert!(
        !a.iter().any(|n| d.contains(n)),
        "same printed name: {a:?} {d:?}"
    );
    // The decoy's backslash is printed as an escape of its own, so the only
    // `\u{17f}` in the output is the shadow's.
    for n in &d {
        assert!(n.contains("\\u{5c}u{17f}"), "{n}");
    }
    for n in &a {
        assert!(n.contains("xta\\u{17f}k") && !n.contains("u{5c}"), "{n}");
    }
    // In one run the shadow is flagged under its own printed name.
    assert!(
        lines.iter().any(|l| l.contains("file=xta\\u{17f}k/")),
        "{lines:?}"
    );
    // The summary rows differ too, and each maps back to one name.
    let rows = summary_names(&r.summary.unwrap());
    assert!(rows.contains("xta\\u{17f}k/src/privacy/mod.rs"), "{rows:?}");
    assert!(
        rows.iter()
            .all(|n| !n.contains('\\') || n.contains("u{5c}")),
        "{rows:?}"
    );
}

#[test]
fn governed_flags_a_list_at_the_listing_limit() {
    let list = nul(&["README.md"]);
    // Exactly the documented limit listed, with the event's count equal: the
    // count may be capped as well, so this still warns.
    let r = run_raw(&list, &["--changed", "3000", "--listed", "3000"], true);
    assert_eq!(r.code, Some(0), "stdout: {}", r.stdout);
    let warn: Vec<&str> = r.stdout.lines().filter(|l| l.starts_with("::")).collect();
    assert_eq!(warn.len(), 1, "stdout: {:?}", r.stdout);
    assert!(
        warn[0].contains("incomplete (3000 of 3000)"),
        "{:?}",
        warn[0]
    );
    assert!(
        r.summary
            .unwrap()
            .contains("treat this pull request as governed")
    );
    // Just under it, with matching counts: no warning.
    let r = run_raw(&list, &["--changed", "2999", "--listed", "2999"], true);
    assert_eq!(r.code, Some(0));
    assert!(!r.stdout.contains("incomplete"), "{:?}", r.stdout);
    assert!(!r.summary.unwrap().contains("incomplete"));
}

#[test]
fn governed_name_status_refuses_other_input() {
    // A plain list read as name-status: `deny.toml` is not a status word.
    let r = run_raw(&nul(&["deny.toml", "README.md"]), &["--name-status"], true);
    assert!(
        matches!(r.code, Some(c) if c != 0),
        "plain list: {:?}",
        r.code
    );
    assert!(flagged(&r.stdout).is_empty());
    // Status words git does not emit.
    for bad in ["Z", "m", "MM", "M1x", "RR100", "100", " M"] {
        let mut b = Vec::new();
        b.extend_from_slice(bad.as_bytes());
        b.extend_from_slice(b"\0deny.toml\0docs/x.toml\0");
        let r = run_raw(&b, &["--name-status"], true);
        assert!(
            matches!(r.code, Some(c) if c != 0),
            "accepted status {bad:?}: {:?}",
            r.code
        );
    }
    // Every letter git documents, with a score where git gives one.
    for ok in ["A", "C", "D", "M", "R", "T", "U", "X", "R100", "C75", "M"] {
        let two = ok.starts_with('R') || ok.starts_with('C');
        let mut b = Vec::new();
        b.extend_from_slice(ok.as_bytes());
        b.extend_from_slice(b"\0deny.toml\0");
        if two {
            b.extend_from_slice(b"docs/x.toml\0");
        }
        let r = run_raw(&b, &["--name-status"], true);
        assert_eq!(r.code, Some(0), "refused status {ok:?}");
        assert_eq!(flagged(&r.stdout).len(), 1, "{ok:?}");
    }
}

/// The patterns of the governed table in `xtask/src/governed.rs`, in order.
fn code_patterns() -> Vec<String> {
    let src = include_str!("../src/governed.rs");
    let start = src.find("const GOVERNED").unwrap();
    let end = start + src[start..].find("\n];").unwrap();
    let lines: Vec<&str> = src[start..end].lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        let first =
            t.starts_with("(\"") || (i > 0 && lines[i - 1].trim() == "(" && t.starts_with('"'));
        if first {
            let q = t.trim_start_matches('(').trim_start_matches('"');
            out.push(q.split('"').next().unwrap().to_string());
        }
    }
    out
}

#[test]
fn governed_table_matches_the_record() {
    let rec = include_str!("../../docs/decisions/0025-governed-paths-review.md");
    let table = rec.split("| Pattern | Why |").nth(1).expect("record table");
    let record: Vec<String> = table
        .lines()
        .skip(2)
        .take_while(|l| l.starts_with('|'))
        .map(|l| {
            let cell = l
                .trim_start_matches('|')
                .split(" | ")
                .next()
                .unwrap()
                .trim();
            cell.trim_matches('`').to_string()
        })
        .collect();
    let code = code_patterns();
    assert!(code.len() > 10, "{code:?}");
    assert_eq!(record, code);
}
