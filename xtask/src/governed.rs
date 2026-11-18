//! The governed paths: the repository's own safety checks (decision 0025).
//!
//! `cargo xtask governed --files <list-file>` reads the changed files of a
//! pull request, prints one warning annotation for each file that belongs to
//! the governed set and writes a job summary table of them. It flags and never
//! fails for a governed change: it exits nonzero only on its own error (an
//! unreadable list, an unwritable summary). Every file name is data and is
//! escaped before it is printed.

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// The governed list: one pattern per row, with the reason it is governed.
///
/// Patterns are anchored at the repository root, with `*` never crossing a
/// folder separator, except two widenings decided with record 0025, because a
/// flag is cheap and a miss is not:
///
/// - `**/.cargo/**`: the build tool honours configuration in a `.cargo` folder
///   at any depth, so one in a sub-folder steers the build as well.
/// - `.github/**`: everything under `.github/` (workflows, composite actions,
///   dependency-update configuration, a future code-owners file) steers CI.
pub const GOVERNED: &[(&str, &str)] = &[
    (
        "xtask/src/privacy/**",
        "the privacy gate's code decides what counts as personal information",
    ),
    (
        "xtask/src/hooks.rs",
        "the commit hooks run the privacy gate before a commit",
    ),
    (
        "xtask/src/governed.rs",
        "this check's own code and its governed list",
    ),
    (
        "xtask/tests/privacy*.rs",
        "the privacy gate's tests define what it must catch",
    ),
    (
        "xtask/tests/secrets.rs",
        "the secret-scanner tests define what it must catch",
    ),
    (
        "xtask/tests/hooks.rs",
        "the hook tests define what the commit hooks must do",
    ),
    (
        "xtask/tests/github_committer.rs",
        "the committer-identity tests guard the commit author check",
    ),
    (
        "xtask/tests/governed.rs",
        "this check's own tests and its governed list",
    ),
    (
        "privacy-allow.toml",
        "every exemption from the privacy rules lives here",
    ),
    (
        ".gitleaks.toml",
        "every exemption from the secret scanner lives here",
    ),
    (
        ".github/**",
        "workflows, composite actions, dependency-update configuration and a code-owners file all steer CI (widened: the whole folder, at the root only)",
    ),
    (
        "deny.toml",
        "the dependency policy: licences, advisories and sources",
    ),
    (
        "**/.cargo/**",
        "build-tool configuration can redirect sources or run programs (widened: a .cargo folder at any depth)",
    ),
];

/// The compiled governed list, in the order of [`GOVERNED`].
pub struct Matcher {
    set: GlobSet,
}

impl Default for Matcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Matcher {
    pub fn new() -> Self {
        let mut b = GlobSetBuilder::new();
        for (pattern, _) in GOVERNED {
            let glob: Glob = GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .expect("the governed patterns are valid");
            b.add(glob);
        }
        Self {
            set: b.build().expect("the governed patterns compile"),
        }
    }

    /// The reason of the first governed row `path` matches, if any.
    pub fn reason(&self, path: &str) -> Option<&'static str> {
        self.set
            .matches(path)
            .into_iter()
            .min()
            .map(|i| GOVERNED[i].1)
    }
}

/// Escape a workflow-command property value (`%`, CR, LF, `:` and `,`).
pub fn escape_property(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            ':' => out.push_str("%3A"),
            ',' => out.push_str("%2C"),
            c => out.push(c),
        }
    }
    out
}

/// Escape a workflow-command message (`%`, CR and LF).
pub fn escape_message(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            c => out.push(c),
        }
    }
    out
}

/// Make text safe inside one cell of a markdown table: one line, no raw
/// markup characters, no pipe that could end the cell.
pub fn escape_cell(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '|' => out.push_str("&#124;"),
            '`' => out.push_str("&#96;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\\' => out.push_str("&#92;"),
            '\r' => out.push_str("&#13;"),
            '\n' => out.push_str("&#10;"),
            c if c.is_control() => out.push('?'),
            c => out.push(c),
        }
    }
    out
}

/// Split a list file: names each ended by a NUL byte; empty names are dropped.
pub fn parse_list(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|n| !n.is_empty())
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .collect()
}

/// Run the check. Annotations go to `out`; the summary table is appended to
/// `summary` when given. Returns the number of governed files.
pub fn run(list: &Path, summary: Option<&Path>, out: &mut dyn Write) -> Result<usize, String> {
    let bytes = fs::read(list).map_err(|e| format!("cannot read {}: {e}", list.display()))?;
    let matcher = Matcher::new();
    let mut hits: Vec<(String, &'static str)> = Vec::new();
    for name in parse_list(&bytes) {
        if let Some(reason) = matcher.reason(&name) {
            hits.push((name, reason));
        }
    }
    for (name, reason) in &hits {
        let message = format!(
            "Governed file: {reason}. This pull request gets an extra security review of the governed files (decision 0025)."
        );
        writeln!(
            out,
            "::warning file={},title=Governed path::{}",
            escape_property(name),
            escape_message(&message)
        )
        .map_err(|e| format!("cannot write annotations: {e}"))?;
    }
    if let Some(path) = summary {
        let mut text = String::from("## Governed paths\n\n");
        if hits.is_empty() {
            text.push_str("No governed file is changed.\n");
        } else {
            text.push_str("| File | Why it is governed |\n|---|---|\n");
            for (name, reason) in &hits {
                let _ = writeln!(text, "| {} | {} |", escape_cell(name), escape_cell(reason));
            }
        }
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("cannot open the summary {}: {e}", path.display()))?;
        f.write_all(text.as_bytes())
            .map_err(|e| format!("cannot write the summary: {e}"))?;
    }
    Ok(hits.len())
}
