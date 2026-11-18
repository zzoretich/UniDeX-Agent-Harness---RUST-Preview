//! The governed paths: the repository's own safety checks (decision 0025).
//!
//! `cargo xtask governed --files <list-file>` reads the changed files of a
//! pull request, prints one warning annotation for each file that belongs to
//! the governed set and writes a job summary table of them. It flags and never
//! fails for a governed change: it exits nonzero only on its own error (an
//! unreadable list, an unwritable summary, a malformed argument). Every file
//! name is data and is escaped before it is printed.

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// The governed list: one pattern per row, with the reason it is governed.
///
/// Patterns are anchored at the repository root, with `*` never crossing a
/// folder separator and letter case ignored, except where a pattern starts
/// with `**/` (any depth). A reason holds no markup characters, because it is
/// written into a table cell. The record (decision 0025) carries the same table.
pub const GOVERNED: &[(&str, &str)] = &[
    (
        "xtask/**",
        "the tool that hosts the privacy gate, the hooks and this check, with their tests",
    ),
    (
        "privacy-allow.toml",
        "every exemption from the privacy rules lives here",
    ),
    (
        ".gitleaks.toml",
        "the secret scanner's rules and its exemptions",
    ),
    (
        ".gitleaksignore",
        "the secret scanner reads findings to ignore from this file",
    ),
    (
        ".github/**",
        "workflows, composite actions, dependency-update configuration and a code-owners file all steer CI",
    ),
    (
        "deny.toml",
        "the dependency policy: licences, advisories and sources",
    ),
    (".deny.toml", "a second spelling of the dependency policy"),
    (
        "**/.cargo",
        "a file or link named .cargo at any depth can stand for a build configuration folder",
    ),
    (
        "**/.cargo/**",
        "build-tool configuration at any depth can redirect sources or run programs",
    ),
    (
        "**/.gitattributes",
        "attributes at any depth can hide a file's changes from a review or change what the privacy gate reads",
    ),
    (
        ".gitmodules",
        "a submodule brings code in from outside the repository",
    ),
    (
        "Cargo.toml",
        "the workspace manifest can patch or replace any dependency",
    ),
    (
        "rust-toolchain.toml",
        "the compiler and cargo for every check come from this file",
    ),
    ("rust-toolchain", "the older spelling of the toolchain file"),
    ("CODEOWNERS", "the code-owners file decides who must review"),
    (
        "docs/CODEOWNERS",
        "the code-owners file decides who must review",
    ),
    (
        "crates/udx-core/src/paths.rs",
        "the path guard that confines every test to its test root",
    ),
    (
        "docs/decisions/0025-governed-paths-review.md",
        "the record that defines the governed set and the review rule",
    ),
];

/// The reason for a name that holds a non-ASCII character and matches no row.
pub const NON_ASCII_REASON: &str =
    "non-ASCII name: may shadow a governed path on a case-folding file system";

/// The compiled governed list, in the order of [`GOVERNED`].
pub struct Matcher {
    set: GlobSet,
}

impl Default for Matcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Fold a path the way a case-folding file system can: upper-case, then
/// lower-case, so characters that fold to ASCII letters become those letters.
pub fn fold(name: &str) -> String {
    name.to_uppercase().to_lowercase()
}

impl Matcher {
    pub fn new() -> Self {
        let mut b = GlobSetBuilder::new();
        for (pattern, _) in GOVERNED {
            b.add(
                GlobBuilder::new(pattern)
                    .literal_separator(true)
                    .case_insensitive(true)
                    .build()
                    .expect("the governed patterns are valid"),
            );
        }
        Self {
            set: b.build().expect("the governed patterns compile"),
        }
    }

    /// The reason of the first governed row the name matches, as given or
    /// folded; else the non-ASCII reason for a name with a non-ASCII character.
    pub fn reason(&self, path: &str) -> Option<&'static str> {
        let folded = fold(path);
        let first = self
            .set
            .matches(path)
            .into_iter()
            .chain(self.set.matches(&folded))
            .min();
        match first {
            Some(i) => Some(GOVERNED[i].1),
            None if !path.is_ascii() => Some(NON_ASCII_REASON),
            None => None,
        }
    }
}

/// Write a character that is not printable ASCII as `\u{hex}`.
fn push_escape(out: &mut String, c: char) {
    let _ = write!(out, "\\u{{{:x}}}", c as u32);
}

/// Escape a workflow-command property value (`%`, CR, LF, `:` and `,`; any
/// other control or non-ASCII character is written as `\u{hex}`).
pub fn escape_property(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            ':' => out.push_str("%3A"),
            ',' => out.push_str("%2C"),
            c if c.is_control() || !c.is_ascii() => push_escape(&mut out, c),
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

/// Make text safe inside one cell of a markdown table: one line, no raw HTML,
/// no link, image or emphasis markup, no pipe that could end the cell, no
/// control or non-ASCII character (written as `\u{hex}`).
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
            '[' => out.push_str("&#91;"),
            ']' => out.push_str("&#93;"),
            '(' => out.push_str("&#40;"),
            ')' => out.push_str("&#41;"),
            '!' => out.push_str("&#33;"),
            '*' => out.push_str("&#42;"),
            '~' => out.push_str("&#126;"),
            '\r' => out.push_str("&#13;"),
            '\n' => out.push_str("&#10;"),
            c if c.is_control() => out.push('?'),
            c if !c.is_ascii() => push_escape(&mut out, c),
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

/// Read the output of `git diff --name-status -z --find-renames`: a status
/// word, then one path, or two (old, new) for a rename or a copy, each ended
/// by a NUL byte. Every path, old and new, becomes a name.
pub fn parse_name_status(bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut toks = bytes.split(|b| *b == 0).collect::<Vec<_>>();
    if toks.last().is_some_and(|t| t.is_empty()) {
        toks.pop();
    }
    let mut names = Vec::new();
    let mut it = toks.into_iter();
    while let Some(status) = it.next() {
        let two = matches!(status.first(), Some(b'R' | b'C'));
        if status.is_empty() {
            return Err("name-status input: an empty status".into());
        }
        for _ in 0..(if two { 2 } else { 1 }) {
            match it.next() {
                Some(p) if !p.is_empty() => names.push(String::from_utf8_lossy(p).into_owned()),
                _ => return Err("name-status input: a status without its path".into()),
            }
        }
    }
    Ok(names)
}

/// A commit id: 40 or 64 hexadecimal characters.
pub fn check_commit_id(s: &str) -> Result<String, String> {
    if (s.len() == 40 || s.len() == 64) && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(s.to_string())
    } else {
        Err("a commit id is 40 or 64 hexadecimal characters".into())
    }
}

/// What to check and how to report it.
pub struct Request<'a> {
    pub list: &'a Path,
    /// The list is `git diff --name-status -z` output, not plain names.
    pub name_status: bool,
    /// The pull request's changed-file count and the number of entries listed.
    pub counts: Option<(u64, u64)>,
    pub base: Option<&'a str>,
    pub head: Option<&'a str>,
    pub summary: Option<&'a Path>,
}

/// Run the check. Annotations go to `out`; the summary is appended to the
/// summary file when given. Returns the number of governed files.
pub fn run(req: &Request, out: &mut dyn Write) -> Result<usize, String> {
    let bytes =
        fs::read(req.list).map_err(|e| format!("cannot read {}: {e}", req.list.display()))?;
    let names = if req.name_status {
        parse_name_status(&bytes)?
    } else {
        parse_list(&bytes)
    };
    let matcher = Matcher::new();
    let mut hits: Vec<(String, &'static str)> = Vec::new();
    for name in names {
        if let Some(reason) = matcher.reason(&name) {
            hits.push((name, reason));
        }
    }
    let incomplete = match req.counts {
        Some((changed, listed)) if listed < changed => Some(format!(
            "The changed-file list is incomplete ({listed} of {changed}); treat this pull request as governed."
        )),
        _ => None,
    };
    let emit = |out: &mut dyn Write, line: String| {
        writeln!(out, "{line}").map_err(|e| format!("cannot write annotations: {e}"))
    };
    if let Some(msg) = &incomplete {
        emit(
            out,
            format!("::warning title=Governed path::{}", escape_message(msg)),
        )?;
    }
    for (name, reason) in &hits {
        let message = format!(
            "Governed file: {reason}. This pull request gets an extra security review of the governed files (decision 0025)."
        );
        emit(
            out,
            format!(
                "::warning file={},title=Governed path::{}",
                escape_property(name),
                escape_message(&message)
            ),
        )?;
    }
    if let Some(path) = req.summary {
        let mut text = String::from("## Governed paths\n\n");
        if let (Some(base), Some(head)) = (req.base, req.head) {
            let _ = writeln!(text, "Examined base {base} and head {head}.\n");
        }
        if let Some(msg) = &incomplete {
            let _ = writeln!(text, "{msg}\n");
        }
        if let Some((changed, listed)) = req.counts {
            let _ = writeln!(
                text,
                "{} governed entries; {listed} entries listed of {changed} changed.\n",
                hits.len()
            );
        }
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
