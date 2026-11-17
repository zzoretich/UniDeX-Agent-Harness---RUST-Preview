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

impl Terms {
    pub fn generic() -> Terms {
        Terms::default()
    }

    /// Adds local terms, one per line. Blank lines and `#` comments are skipped.
    pub fn with_local(mut self, list_text: &str) -> Terms {
        for line in list_text.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let t = t.to_lowercase();
            if !self.local.contains(&t) {
                self.local.push(t);
            }
        }
        self
    }

    pub fn has_local(&self) -> bool {
        !self.local.is_empty()
    }

    /// Terms found in `line`, matched case-insensitively. With `separators`,
    /// a term also matches when its spaces are `-`, `_` or removed (file names).
    fn local_hits(&self, line: &str, separators: bool) -> Vec<String> {
        let hay = line.to_lowercase();
        let mut hits = Vec::new();
        for term in &self.local {
            let mut variants = vec![term.clone()];
            if separators && term.contains(' ') {
                for sep in ["-", "_", ""] {
                    variants.push(term.replace(' ', sep));
                }
            }
            if variants.iter().any(|v| hay.contains(v.as_str())) {
                hits.push(term.clone());
            }
        }
        hits
    }
}

fn shorten(s: &str) -> String {
    const MAX: usize = 100;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        let cut: String = s.chars().take(MAX).collect();
        format!("{cut}...")
    }
}

fn scan_line(
    label: &str,
    line_no: usize,
    line: &str,
    terms: &Terms,
    seps: bool,
    out: &mut Vec<Finding>,
) {
    for (rule, text) in generic_matches(line) {
        out.push(Finding {
            path: label.to_string(),
            line: line_no,
            rule,
            excerpt: shorten(&text),
        });
    }
    for term in terms.local_hits(line, seps) {
        out.push(Finding {
            path: label.to_string(),
            line: line_no,
            rule: RuleId::LocalTerm,
            excerpt: shorten(&term),
        });
    }
}

pub fn scan_text(label: &str, text: &str, terms: &Terms) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        scan_line(label, i + 1, line, terms, false, &mut out);
    }
    out
}

/// Scans arbitrary bytes. Invalid UTF-8 is decoded lossily; never panics.
pub fn scan_bytes(label: &str, bytes: &[u8], terms: &Terms) -> Vec<Finding> {
    scan_text(label, &String::from_utf8_lossy(bytes), terms)
}

/// Scans a path name. Findings carry line 0.
pub fn scan_path_name(path: &str, terms: &Terms) -> Vec<Finding> {
    let mut out = Vec::new();
    scan_line(path, 0, path, terms, true, &mut out);
    out
}
