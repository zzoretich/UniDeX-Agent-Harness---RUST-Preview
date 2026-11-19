//! Scanning text, bytes and path names with the generic rules and local terms.

use super::rules::{RuleId, generic_matches};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Finding {
    pub path: String,
    pub line: usize,
    pub rule: RuleId,
    /// The matched text, shortened for display.
    pub excerpt: String,
    /// The whole matched text. The allow file compares against this, never
    /// against the shortened excerpt.
    pub value: String,
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
    /// Whitespace inside a term is collapsed to single spaces. LF, CRLF and a
    /// lone CR all end a line.
    pub fn with_local(mut self, list_text: &str) -> Terms {
        // A UTF-8 byte-order mark would otherwise stick to the first term.
        let list_text = list_text.strip_prefix('\u{FEFF}').unwrap_or(list_text);
        for line in list_text.split(['\r', '\n']) {
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
    /// and runs on the whole text with whitespace runs collapsed and invisible
    /// characters (see `invisible`) removed, so a term split across lines,
    /// padded with extra whitespace or split by an invisible character is
    /// still found. The line is the one where the match starts. A multi-word term also matches
    /// with its spaces written as `-`, `_` or removed, in text and in names.
    fn local_hits(&self, text: &str) -> Vec<(usize, String)> {
        if self.local.is_empty() {
            return Vec::new();
        }
        // Normalised text, with the source line of every byte of it.
        let mut hay = String::new();
        let mut lines: Vec<usize> = Vec::new();
        let mut last_space = true;
        let mut line = 1;
        for c in text.chars() {
            // An invisible character inside a term would hide it.
            if invisible(c) {
                continue;
            }
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
        let mut seen: HashSet<(usize, &str)> = HashSet::new();
        for term in &self.local {
            let mut variants = vec![term.clone()];
            if term.contains(' ') {
                for sep in ["-", "_", ""] {
                    variants.push(term.replace(' ', sep));
                }
            }
            for v in variants.iter().filter(|v| !v.is_empty()) {
                let mut from = 0;
                while let Some(i) = hay[from..].find(v.as_str()) {
                    let at = from + i;
                    if seen.insert((lines[at], term.as_str())) {
                        hits.push((lines[at], term.clone()));
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
            value: text,
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
            value: term,
        });
    }
}

pub fn scan_text(label: &str, text: &str, terms: &Terms) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        push_generic(label, i + 1, line, &mut out);
    }
    push_local(label, terms.local_hits(text), false, &mut out);
    out.sort_by_key(|f| f.line);
    out
}

/// Whether each UTF-16 byte order is plausible for `bytes`. An order is
/// plausible when any of these holds for it:
///
/// - a window of up to 64 byte pairs, the windows overlapping by half, has at
///   least one pair in eight with a NUL on that order's high side only (ASCII
///   text in that order);
/// - four or more pairs in a row are printable ASCII in that order (a short
///   string inside a binary);
/// - in the whole input, at least two pairs have a NUL on that order's high
///   side only, and at least twice as many as on the other order's (text in
///   another script with line breaks; random bytes split evenly).
///
/// Random bytes almost never do any of these; zero padding (NUL on both
/// sides) never does. Linear, no decoding.
fn utf16_evidence(bytes: &[u8]) -> (bool, bool) {
    // Without a NUL no pair has one; skip the passes.
    if !bytes.contains(&0) {
        return (false, false);
    }
    let pairs = bytes.as_chunks::<2>().0;
    let (le_seen, le_count) = order_evidence(pairs, |[low, high]| (*low, *high));
    let (be_seen, be_count) = order_evidence(pairs, |[high, low]| (*low, *high));
    let majority = |own: usize, other: usize| own >= 2 && own >= 2 * other;
    (
        le_seen || majority(le_count, be_count),
        be_seen || majority(be_count, le_count),
    )
}

/// For one byte order, `side` giving each pair as (low byte, high byte):
/// whether a window or a printable run shows that order, and how many pairs
/// in the whole input have a NUL on the high side only.
fn order_evidence(pairs: &[[u8; 2]], side: impl Fn(&[u8; 2]) -> (u8, u8)) -> (bool, usize) {
    const WINDOW: usize = 64;
    const STEP: usize = 32;
    let one_sided = |p: &[u8; 2]| {
        let (low, high) = side(p);
        low != 0 && high == 0
    };
    let mut seen = false;
    for start in (0..pairs.len()).step_by(STEP) {
        let window = &pairs[start..pairs.len().min(start + WINDOW)];
        seen |= window.iter().filter(|p| one_sided(p)).count() * 8 >= window.len();
    }
    let mut count = 0;
    let mut run = 0;
    for p in pairs {
        count += usize::from(one_sided(p));
        let (low, high) = side(p);
        if high == 0 && (0x20..=0x7E).contains(&low) {
            run += 1;
            seen |= run >= 4;
        } else {
            run = 0;
        }
    }
    (seen, count)
}

/// The readings of `bytes` to scan: the lossy UTF-8 one first, then one per
/// UTF-16 byte order the bytes could be in. A byte-order mark names an order,
/// which is always read; the other order is read too when the marked one is
/// not plausible or the other one is (a mark that contradicts the text hides
/// nothing). Without a mark, an order is read only when `utf16_evidence`
/// finds it plausible, so a binary file is not decoded twice for nothing.
/// Never panics.
pub(crate) fn text_views(bytes: &[u8]) -> Vec<String> {
    let (le, be) = match bytes {
        [0xFF, 0xFE, rest @ ..] => {
            let (le, be) = utf16_evidence(rest);
            (Some(rest), (be || !le).then_some(rest))
        }
        [0xFE, 0xFF, rest @ ..] => {
            let (le, be) = utf16_evidence(rest);
            ((le || !be).then_some(rest), Some(rest))
        }
        _ => {
            let (le, be) = utf16_evidence(bytes);
            (le.then_some(bytes), be.then_some(bytes))
        }
    };
    let mut views = vec![String::from_utf8_lossy(bytes).into_owned()];
    if let Some(b) = le {
        views.push(decode_utf16(b, u16::from_le_bytes));
    }
    if let Some(b) = be {
        views.push(decode_utf16(b, u16::from_be_bytes));
    }
    views
}

fn decode_utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    char::decode_utf16(bytes.as_chunks::<2>().0.iter().map(|c| unit(*c)))
        .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// Scans every reading of `bytes` (see `text_views`), after `prep` has been
/// applied to each, and merges the findings without repeats (through a set,
/// so the merge stays linear in the number of findings).
pub(crate) fn scan_views(
    label: &str,
    bytes: &[u8],
    terms: &Terms,
    prep: &dyn Fn(&str) -> String,
) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let mut seen: HashSet<Finding> = HashSet::new();
    for view in text_views(bytes) {
        for f in scan_text(label, &prep(&view), terms) {
            if !seen.contains(&f) {
                seen.insert(f.clone());
                out.push(f);
            }
        }
    }
    out.sort_by_key(|f| f.line);
    out
}

/// Scans arbitrary bytes, as UTF-8 (invalid sequences decoded lossily) and as
/// UTF-16 where the bytes could be that. Never panics.
pub fn scan_bytes(label: &str, bytes: &[u8], terms: &Terms) -> Vec<Finding> {
    scan_views(label, bytes, terms, &str::to_string)
}

/// Unicode's Default_Ignorable_Code_Point set: characters that render as
/// nothing (soft hyphen, zero-width and joiner characters, bidirectional
/// marks and controls, variation selectors, Hangul fillers, tag characters
/// and the reserved code points of those blocks). Source: Unicode 16.0.0,
/// DerivedCoreProperties.txt. A unit test checks it against the regex
/// crate's table of the same property.
const DEFAULT_IGNORABLE: [(char, char); 17] = [
    ('\u{00AD}', '\u{00AD}'),
    ('\u{034F}', '\u{034F}'),
    ('\u{061C}', '\u{061C}'),
    ('\u{115F}', '\u{1160}'),
    ('\u{17B4}', '\u{17B5}'),
    ('\u{180B}', '\u{180F}'),
    ('\u{200B}', '\u{200F}'),
    ('\u{202A}', '\u{202E}'),
    ('\u{2060}', '\u{206F}'),
    ('\u{3164}', '\u{3164}'),
    ('\u{FE00}', '\u{FE0F}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FFA0}', '\u{FFA0}'),
    ('\u{FFF0}', '\u{FFF8}'),
    ('\u{1BCA0}', '\u{1BCA3}'),
    ('\u{1D173}', '\u{1D17A}'),
    ('\u{E0000}', '\u{E0FFF}'),
];

/// A character that is invisible in text and would split a term: one of
/// `DEFAULT_IGNORABLE`. Refused inside the term list, removed from text
/// before term matching.
pub(crate) fn invisible(c: char) -> bool {
    !c.is_ascii()
        && DEFAULT_IGNORABLE
            .iter()
            .any(|(lo, hi)| (*lo..=*hi).contains(&c))
}

/// Reads the private term list. UTF-16 (a NUL byte, or a UTF-16 byte-order
/// mark) and any other invalid UTF-8 are refused: read lossily, a term would
/// silently match nothing. So is, after one leading byte-order mark, any
/// invisible character (see `invisible`) or control character other than
/// tab, CR and LF, which would silently break the term that holds it. The
/// message names a byte offset, never the list's content.
pub(crate) fn decode_term_list(bytes: &[u8]) -> Result<String, String> {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return Err(
            "the local term list is not UTF-8 (it starts with a UTF-16 byte-order \
             mark, so it looks like UTF-16); save it as UTF-8 so its terms are not \
             silently ignored"
                .to_string(),
        );
    }
    if let Some(at) = bytes.iter().position(|b| *b == 0) {
        // UTF-16 text has a NUL on one side of most byte pairs; a stray NUL
        // in UTF-8 text is named as what it is.
        let pairs = bytes.as_chunks::<2>().0;
        let one_sided = pairs.iter().filter(|[a, b]| (*a == 0) != (*b == 0)).count();
        return Err(if one_sided * 2 > pairs.len() {
            "the local term list is not UTF-8 (NUL bytes fill one side of its byte pairs, \
             so it looks like UTF-16); save it as UTF-8 so its terms are not silently ignored"
                .to_string()
        } else {
            format!(
                "the local term list holds a NUL byte at offset {at}; remove it so the \
                 term that holds it is not silently ignored"
            )
        });
    }
    let text = String::from_utf8(bytes.to_vec()).map_err(|e| {
        format!(
            "the local term list is not valid UTF-8 (invalid byte at offset {}); \
             save it as UTF-8 so its terms are not silently ignored",
            e.utf8_error().valid_up_to()
        )
    })?;
    let from = if text.starts_with('\u{FEFF}') { 3 } else { 0 };
    let bad = text[from..]
        .char_indices()
        .find(|(_, c)| invisible(*c) || c.is_control() && !matches!(c, '\t' | '\r' | '\n'));
    if let Some((at, _)) = bad {
        return Err(format!(
            "the local term list holds an invisible or control character at byte offset {}; \
             remove it so the term that holds it is not silently ignored",
            from + at
        ));
    }
    Ok(text)
}

/// Scans a path name. Findings carry line 0.
pub fn scan_path_name(path: &str, terms: &Terms) -> Vec<Finding> {
    let mut out = Vec::new();
    push_generic(path, 0, path, &mut out);
    push_local(path, terms.local_hits(path), true, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::{invisible, text_views, utf16_evidence};

    #[test]
    fn the_invisible_table_is_unicodes_default_ignorable_set() {
        // The regex crate carries the same Unicode property; every scalar
        // value must agree with it.
        let all: String = (0..=0x10_FFFFu32).filter_map(char::from_u32).collect();
        let re = regex::Regex::new(r"\p{Default_Ignorable_Code_Point}").unwrap();
        let found: std::collections::HashSet<char> = re
            .find_iter(&all)
            .flat_map(|m| m.as_str().chars())
            .collect();
        let differ: Vec<String> = all
            .chars()
            .filter(|c| invisible(*c) != found.contains(c))
            .take(10)
            .map(|c| format!("U+{:04X}", c as u32))
            .collect();
        assert!(differ.is_empty(), "{differ:?}");
        assert!(found.len() > 4000, "{}", found.len());
    }

    /// `n` byte pairs of plain letters with no NUL, except a pair with a NUL
    /// on the high side (little-endian evidence) at each of `le`, one with a
    /// NUL on the low side (big-endian evidence) at each of `be`, and the
    /// letter `A` with a NUL after it at each of `printable`.
    fn pairs(n: usize, le: &[usize], be: &[usize], printable: &[usize]) -> Vec<u8> {
        let mut v: Vec<[u8; 2]> = vec![*b"QR"; n];
        for &i in le {
            v[i] = [1, 0];
        }
        for &i in be {
            v[i] = [0, 1];
        }
        for &i in printable {
            v[i] = [b'A', 0];
        }
        v.concat()
    }

    #[test]
    fn each_kind_of_utf16_evidence_has_its_threshold() {
        let le = |bytes: &[u8]| utf16_evidence(bytes).0;
        // A window: 8 one-sided pairs in 64 are evidence, 7 are not (the
        // other order holds enough pairs that the whole-file test is off).
        let be5 = [130, 200, 270, 340, 380];
        assert!(le(&pairs(400, &[0, 1, 2, 3, 4, 5, 6, 7], &be5, &[])));
        assert!(!le(&pairs(400, &[0, 1, 2, 3, 4, 5, 6], &be5, &[])));
        // Windows overlap: 8 pairs across a 64-pair edge are evidence, and so
        // are 8 pairs that two windows hold.
        let across: Vec<usize> = (60..68).collect();
        assert!(le(&pairs(400, &across, &be5, &[])));
        let twice: Vec<usize> = (40..48).collect();
        assert!(le(&pairs(400, &twice, &be5, &[])));
        // A run of four printable characters is evidence, three are not.
        let be3 = [130, 200, 270];
        assert!(le(&pairs(400, &[], &be3, &[20, 21, 22, 23])));
        assert!(!le(&pairs(400, &[], &be3, &[20, 21, 22])));
        assert!(!le(&pairs(400, &[], &be3, &[20, 21, 23, 24])));
        assert!(le(&pairs(
            400,
            &[],
            &[130, 200, 270, 340],
            &[20, 21, 22, 23, 24]
        )));
        // The whole file: at least two one-sided pairs, and at least twice as
        // many as the other order has.
        assert_eq!(
            utf16_evidence(&pairs(400, &[10, 150], &[], &[])),
            (true, false)
        );
        assert_eq!(utf16_evidence(&pairs(400, &[10], &[], &[])), (false, false));
        assert_eq!(
            utf16_evidence(&pairs(400, &[10, 150], &[290], &[])),
            (true, false)
        );
        assert_eq!(
            utf16_evidence(&pairs(400, &[10, 150, 290], &[80, 220], &[])),
            (false, false)
        );
        assert_eq!(
            utf16_evidence(&pairs(400, &[], &[10, 150], &[])),
            (false, true)
        );
    }

    /// Bytes that look random, with a NUL every 97 bytes.
    fn binary(n: usize) -> Vec<u8> {
        let mut bytes: Vec<u8> = (0..n as u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8 | 1)
            .collect();
        for i in (0..bytes.len()).step_by(97) {
            bytes[i] = 0;
        }
        bytes
    }

    #[test]
    fn a_binary_gets_a_utf16_reading_only_when_one_is_plausible() {
        // NULs here and there, or NUL on both sides of every pair (zero
        // padding), are no evidence of UTF-16: one reading.
        assert_eq!(text_views(&binary(4096)).len(), 1);
        assert_eq!(text_views(&[0u8; 4096]).len(), 1);
        let mut plain = b"see the notes here\n".repeat(20);
        plain[5] = 0;
        assert_eq!(text_views(&plain).len(), 1);
        // ASCII text in UTF-16, either order, no mark: one more reading.
        let text = "plain text\n".repeat(10);
        let le: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let be: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert_eq!(text_views(&le).len(), 2);
        assert_eq!(text_views(&be).len(), 2);
        // Even two characters.
        assert_eq!(text_views(b"a\0b\0").len(), 2);
        assert_eq!(text_views(b"\0a\0b").len(), 2);
        // The threshold: one pair in eight. Eight pairs with one NUL on one
        // side are enough; nine are not.
        assert_eq!(text_views(b"abcdefghijklmn\0p").len(), 2);
        assert_eq!(text_views(b"abcdefghijklmn\0pqr").len(), 1);
        // A UTF-16 stretch inside a binary.
        let mut mixed = binary(4096);
        mixed.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        mixed.extend(binary(4096));
        assert_eq!(text_views(&mixed).len(), 2);
    }
}
