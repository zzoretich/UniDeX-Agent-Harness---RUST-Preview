//! Scanning text, bytes and path names with the generic rules and local terms.

use super::rules::{RuleId, generic_matches};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub path: String,
    pub line: usize,
    pub rule: RuleId,
    pub excerpt: String,
}

/// The generic rules plus an optional list of local terms.
#[derive(Debug, Clone, Default)]
pub struct Terms {
    local: Vec<String>,
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::new();
    let mut last_space = true;
    for c in s.trim().chars() {
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.extend(c.to_lowercase());
            last_space = false;
        }
    }
    out
}

impl Terms {
    pub fn generic() -> Terms {
        Terms::default()
    }

    /// Adds local terms, one per line. Blank lines and `#` comments are skipped.
    /// Whitespace inside a term is collapsed to single spaces.
    pub fn with_local(mut self, list_text: &str) -> Terms {
        for line in list_text.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let t = collapse_ws(t);
            if !self.local.contains(&t) {
                self.local.push(t);
            }
        }
        self
    }

    pub fn has_local(&self) -> bool {
        !self.local.is_empty()
    }

    /// Local-term hits in `text` as (line, term). Matching is case-insensitive
    /// and runs on the whole text with whitespace runs collapsed, so a term
    /// split across lines or padded with extra whitespace is still found. The
    /// line is the one where the match starts. With `separators`, a term also
    /// matches when its spaces are `-`, `_` or removed (file names).
    fn local_hits(&self, text: &str, separators: bool) -> Vec<(usize, String)> {
        if self.local.is_empty() {
            return Vec::new();
        }
        // Normalised text, with the source line of every byte of it.
        let mut hay = String::new();
        let mut lines: Vec<usize> = Vec::new();
        let mut last_space = true;
        let mut line = 1;
        for c in text.chars() {
            if c.is_whitespace() {
                if !last_space {
                    hay.push(' ');
                    lines.push(line);
                }
                last_space = true;
            } else {
                for l in c.to_lowercase() {
                    let before = hay.len();
                    hay.push(l);
                    lines.extend(std::iter::repeat_n(line, hay.len() - before));
                }
                last_space = false;
            }
            if c == '\n' {
                line += 1;
            }
        }
        let mut hits: Vec<(usize, String)> = Vec::new();
        for term in &self.local {
            let mut variants = vec![term.clone()];
            if separators && term.contains(' ') {
                for sep in ["-", "_", ""] {
                    variants.push(term.replace(' ', sep));
                }
            }
            for v in variants.iter().filter(|v| !v.is_empty()) {
                let mut from = 0;
                while let Some(i) = hay[from..].find(v.as_str()) {
                    let at = from + i;
                    let hit = (lines[at], term.clone());
                    if !hits.contains(&hit) {
                        hits.push(hit);
                    }
                    from = at + hay[at..].chars().next().map_or(1, char::len_utf8);
                }
            }
        }
        hits.sort();
        hits
    }
}

pub(crate) fn shorten(s: &str) -> String {
    const MAX: usize = 100;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        let cut: String = s.chars().take(MAX).collect();
        format!("{cut}...")
    }
}

fn push_generic(label: &str, line_no: usize, line: &str, out: &mut Vec<Finding>) {
    for (rule, text) in generic_matches(line) {
        out.push(Finding {
            path: label.to_string(),
            line: line_no,
            rule,
            excerpt: shorten(&text),
        });
    }
}

fn push_local(label: &str, hits: Vec<(usize, String)>, line_zero: bool, out: &mut Vec<Finding>) {
    for (line, term) in hits {
        out.push(Finding {
            path: label.to_string(),
            line: if line_zero { 0 } else { line },
            rule: RuleId::LocalTerm,
            excerpt: shorten(&term),
        });
    }
}

pub fn scan_text(label: &str, text: &str, terms: &Terms) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        push_generic(label, i + 1, line, &mut out);
    }
    push_local(label, terms.local_hits(text, false), false, &mut out);
    out.sort_by_key(|f| f.line);
    out
}

/// Scans arbitrary bytes. Invalid UTF-8 is decoded lossily; never panics.
pub fn scan_bytes(label: &str, bytes: &[u8], terms: &Terms) -> Vec<Finding> {
    scan_text(label, &String::from_utf8_lossy(bytes), terms)
}

/// Scans a path name. Findings carry line 0.
pub fn scan_path_name(path: &str, terms: &Terms) -> Vec<Finding> {
    let mut out = Vec::new();
    push_generic(path, 0, path, &mut out);
    push_local(path, terms.local_hits(path, true), true, &mut out);
    out
}
