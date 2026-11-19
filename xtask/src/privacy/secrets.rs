//! The secret rules (provider keys, cloud access keys, the hosting service's
//! tokens, private key blocks and bearer values) and the masking that keeps
//! a secret's body out of everything the gate prints.
//!
//! # The rules
//!
//! Each rule matches a prefix, which may be printed, and a body, which never
//! is. Every rule but `SecretPrivateKey` begins with a word and matches only
//! where the character before it is not an ASCII letter or digit (the text
//! start, a separator, `_`, a quote), so a word that merely holds a prefix
//! (`disk-usage-...` holds `sk-`) is not a key.
//!
//! | Rule | Matches | Deliberately does not match |
//! |---|---|---|
//! | `SecretAnthropicKey` | `sk-ant-` then 32 or more of `A-Z a-z 0-9 _ -` (the key's tag, as in `api03-`, is part of the body) | a shorter body (`sk-ant-api03-EXAMPLE`) |
//! | `SecretOpenAiKey` | `sk-proj-`, `sk-svcacct-` or `sk-admin-` then 32 or more of `A-Z a-z 0-9 _ -`; or `sk-` then 32 or more ASCII letters and digits | a shorter body (`sk-proj-EXAMPLE`); `sk-` inside a word |
//! | `SecretCloudAccessKey` | `AKIA`, `ASIA`, `ABIA`, `ACCA`, or `A3T` and one more letter or digit, then exactly 16 uppercase letters and digits, not followed by a letter or digit | a longer run of letters and digits; the cloud provider's documented example key, exactly |
//! | `SecretHostingToken` | `ghp_`, `gho_`, `ghu_`, `ghs_` or `ghr_` then 36 or more ASCII letters and digits; or `github_pat_` then 36 or more of `A-Z a-z 0-9 _` | a shorter body (`ghp_EXAMPLE`) |
//! | `SecretPrivateKey` | a header that names a private key, in any letter case: PEM (five `-`, `BEGIN`, up to 100 of `A-Z 0-9 _ -` and spaces, `PRIVATE KEY` or `PRIVATE KEY BLOCK`, five `-`), RFC 4716 (four `-` and a space each side), or a PuTTY key file's first line. The header alone is a finding | a certificate or a public key header |
//! | `SecretBearer` | `Bearer` (any letter case), then any number of spaces, tabs and `%20` with at most one line break or one written escape (`\n`, `\r`, `\t`) among them, then 20 or more bearer-token characters (`A-Z a-z 0-9 - . _ ~ + /`, then any `=`) | a shorter value (`YOUR_TOKEN`); a value that holds any other character, as a placeholder does (`<token>`, `$TOKEN`, `{token}`); a value after a blank line |
//!
//! The only exemptions, both narrow: a placeholder body, which once one
//! leading `EXAMPLE` (in capitals) is removed and trailing `=` padding is
//! ignored is one character repeated (`xxxx...`, `EXAMPLE0000...`); and the
//! cloud provider's documented example access key, exactly
//! (`DOCUMENTED_EXAMPLE`). The allow file can never exempt a secret finding
//! (`Allow::load` refuses the rule names).
//!
//! # Masking
//!
//! A secret finding's excerpt is its prefix and the number of characters it
//! hides (`ghp_[36 characters hidden]`), or a key block's header, whose middle
//! is shown only when it is a short label (`RSA`, `OPENSSH`) and is otherwise
//! masked as `[..]`; its `value`
//! is a digest of the matched text, never the text. `Secrets::merge` replaces
//! the excerpt of every other finding on a line that holds a secret, or
//! inside a private key block (header to footer, or to the end of the text
//! when there is no footer), with `HIDDEN`, so a home path or an address that
//! overlaps a key cannot print part of it. `redact` hides the body of every
//! match in text the gate prints as it is: a path and an error message.

use super::rules::RuleId;
use super::scan::{Finding, text_views};
use regex::Regex;
use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::LazyLock;

/// The family of a secret rule. Each is its own rule name, `Secret...`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind {
    AnthropicKey,
    OpenAiKey,
    CloudAccessKey,
    HostingToken,
    PrivateKey,
    Bearer,
}

impl SecretKind {
    pub const ALL: [SecretKind; 6] = [
        SecretKind::AnthropicKey,
        SecretKind::OpenAiKey,
        SecretKind::CloudAccessKey,
        SecretKind::HostingToken,
        SecretKind::PrivateKey,
        SecretKind::Bearer,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SecretKind::AnthropicKey => "SecretAnthropicKey",
            SecretKind::OpenAiKey => "SecretOpenAiKey",
            SecretKind::CloudAccessKey => "SecretCloudAccessKey",
            SecretKind::HostingToken => "SecretHostingToken",
            SecretKind::PrivateKey => "SecretPrivateKey",
            SecretKind::Bearer => "SecretBearer",
        }
    }
}

/// What another finding's excerpt becomes on a line a secret covers.
pub(crate) const HIDDEN: &str = "[excerpt hidden: this line holds a secret]";

/// The cloud provider's documented example access key, the one value of the
/// cloud rule's shape that is not a finding. Written in parts so this file
/// holds no key-shaped literal.
const DOCUMENTED_EXAMPLE: &str = concat!("AKIA", "IOSFODNN7", "EXAMPLE");

/// Before a rule that begins with a word: the text start, a character that
/// is not an ASCII letter or digit, or an escape that ends in one (`\n`,
/// `\r`, `\t` and `\u` with four hexadecimal digits, as written inside a
/// quoted string, and a percent code such as `%3D` or `%20`). It is part of
/// the match, outside the prefix.
const START: &str = r"(?:\A|[^A-Za-z0-9]|\\[nrt]|\\u[0-9A-Fa-f]{4}|%[0-9A-Fa-f]{2})";

/// A key header's middle that is a real label, shown as it is: up to four
/// short capital words (`RSA `, `SSH2 ENCRYPTED `), or a short version number.
static LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ ?(?:[A-Z0-9]{1,10} ){0,4}$|^[0-9]{1,2}$").unwrap());

/// One pattern of a rule. Group `p` is the prefix, which may be printed;
/// group `m`, in a key header, is its middle (see `LABEL`);
/// group `s`, when present, separates it from the body and is shown as one
/// space; group `b`, when present, the body, which never is; group `t`, when
/// present, must be empty (the body is not followed by a letter or digit).
struct Pattern {
    kind: SecretKind,
    re: Regex,
}

static PATTERNS: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
    let pattern = |kind, re: &str| Pattern {
        kind,
        re: Regex::new(re).unwrap(),
    };
    vec![
        pattern(
            SecretKind::AnthropicKey,
            &format!(r"{START}(?P<p>sk-ant-)(?P<b>[A-Za-z0-9_-]{{32,}})"),
        ),
        pattern(
            SecretKind::OpenAiKey,
            &format!(r"{START}(?P<p>sk-(?:proj|svcacct|admin|None)-)(?P<b>[A-Za-z0-9_-]{{32,}})"),
        ),
        pattern(
            SecretKind::OpenAiKey,
            &format!(r"{START}(?P<p>sk-)(?P<b>[A-Za-z0-9]{{32,}})"),
        ),
        pattern(
            SecretKind::CloudAccessKey,
            &format!(
                r"{START}(?P<p>AKIA|ASIA|ABIA|ACCA|A3T[A-Z0-9])(?P<b>[A-Z0-9]{{16}})(?P<t>[A-Za-z0-9]*)"
            ),
        ),
        pattern(
            SecretKind::HostingToken,
            &format!(r"{START}(?P<p>gh[pousr]_)(?P<b>[A-Za-z0-9]{{36,}})"),
        ),
        pattern(
            SecretKind::HostingToken,
            &format!(r"{START}(?P<p>github_pat_)(?P<b>[A-Za-z0-9_]{{36,}})"),
        ),
        pattern(
            SecretKind::PrivateKey,
            r"(?i)(?P<p>-----BEGIN(?P<m>[ A-Z0-9_-]{0,100}?)PRIVATE KEY(?: BLOCK)?-----)",
        ),
        // RFC 4716 (four dashes and spaces) and a PuTTY key file.
        pattern(
            SecretKind::PrivateKey,
            r"(?i)(?P<p>---- BEGIN (?P<m>[ A-Z0-9_-]{0,100}?)PRIVATE KEY ----)",
        ),
        pattern(
            SecretKind::PrivateKey,
            r"(?P<p>PuTTY-User-Key-File-(?P<m>[0-9]+):)",
        ),
        pattern(
            SecretKind::Bearer,
            &format!(
                r"(?i){START}(?P<p>bearer)(?P<s>(?:[ \t]|%20)+|(?:[ \t]|%20)*(?:\\[nrt]|\r?\n)(?:[ \t]|%20)*)(?P<b>[A-Za-z0-9._~+/-]{{20,}}=*)"
            ),
        ),
    ]
});

/// The footer that ends a private key block. The header's and the footer's
/// middle parts are lazy, so a header never runs on into a footer on its own
/// line.
static KEY_FOOTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)-----END[ A-Z0-9_-]{0,100}?PRIVATE KEY(?: BLOCK)?-----|---- END [ A-Z0-9_-]{0,100}?PRIVATE KEY ----|(?-i:Private-MAC:)",
    )
    .unwrap()
});

/// One match of a rule.
struct Hit {
    kind: SecretKind,
    /// Where the prefix starts and the match ends, in bytes.
    start: usize,
    end: usize,
    /// Where the body starts and ends; `None` for a key block's header.
    body: Option<(usize, usize)>,
    /// A placeholder (see the module notes), not a finding.
    placeholder: bool,
    /// The prefix, which may be printed (a key header's middle only when it
    /// is a label), and whether a separator follows it.
    prefix: (usize, usize),
    spaced: bool,
    /// A key header's middle.
    middle: Option<(usize, usize)>,
}

impl Hit {
    /// What a finding shows: the prefix and how much is hidden, or the
    /// header, its middle masked as `[..]` unless it is a label.
    fn shown(&self, text: &str) -> String {
        let prefix = &text[self.prefix.0..self.prefix.1];
        match self.body {
            Some((start, end)) => {
                let space = if self.spaced { " " } else { "" };
                format!("{prefix}{space}[{} characters hidden]", end - start)
            }
            None => match self.middle {
                Some((start, end)) if !LABEL.is_match(&text[start..end]) => format!(
                    "{}[..]{}",
                    &text[self.prefix.0..start],
                    &text[end..self.prefix.1]
                ),
                _ => prefix.to_string(),
            },
        }
    }
}

/// A placeholder body: one character repeated, once one leading `EXAMPLE`
/// is removed and trailing `=` padding is ignored.
fn placeholder_body(body: &str) -> bool {
    let body = body.trim_end_matches('=');
    let rest = body.strip_prefix("EXAMPLE").unwrap_or(body);
    let mut chars = rest.chars();
    let first = chars.next();
    chars.all(|c| Some(c) == first)
}

/// Every match of every rule in `text`, placeholders included.
fn hits(text: &str) -> Vec<Hit> {
    let mut out = Vec::new();
    for pattern in PATTERNS.iter() {
        for caps in pattern.re.captures_iter(text) {
            if caps.name("t").is_some_and(|t| !t.is_empty()) {
                continue;
            }
            let p = caps.name("p").expect("every pattern has a prefix group");
            let body = caps.name("b");
            let end = body.map_or(p.end(), |b| b.end());
            out.push(Hit {
                kind: pattern.kind,
                start: p.start(),
                end,
                body: body.map(|b| (b.start(), b.end())),
                placeholder: body.is_some_and(|b| placeholder_body(b.as_str()))
                    || &text[p.start()..end] == DOCUMENTED_EXAMPLE,
                prefix: (p.start(), p.end()),
                spaced: caps.name("s").is_some(),
                middle: caps.name("m").map(|m| (m.start(), m.end())),
            });
        }
    }
    out
}

/// `text` with the body of every match hidden (placeholders too), for text
/// the gate prints as it is.
pub fn redact(text: &str) -> String {
    let mut spans: Vec<(usize, usize)> = hits(text).iter().filter_map(|h| h.body).collect();
    spans.sort_unstable();
    let mut out = String::new();
    let mut at = 0;
    for (start, end) in spans {
        if end <= at {
            continue; // inside a body already hidden
        }
        let start = start.max(at);
        out.push_str(&text[at..start]);
        out.push_str(&format!("[{} characters hidden]", end - start));
        at = end;
    }
    out.push_str(&text[at..]);
    out
}

/// The secret findings in one text, and the lines where other findings'
/// excerpts must be hidden.
#[derive(Default)]
pub(crate) struct Secrets {
    findings: Vec<Finding>,
    /// `covered[n]` is true when line `n` holds a secret or lies inside a
    /// private key block. Line 0 is a path name.
    covered: Vec<bool>,
}

impl Secrets {
    /// `found` (another rule's findings in the same text) with every excerpt
    /// on a covered line hidden, followed by the secret findings.
    pub(crate) fn merge(self, mut found: Vec<Finding>) -> Vec<Finding> {
        for f in &mut found {
            if self.covered.get(f.line).copied().unwrap_or(false) {
                f.excerpt = HIDDEN.to_string();
            }
        }
        found.extend(self.findings);
        found
    }

    /// Adds the findings of one reading of a text, without repeats.
    fn add(&mut self, label: &str, text: &str, name: bool, seen: &mut HashSet<Finding>) {
        let found = hits(text);
        if found.iter().all(|h| h.placeholder) {
            return;
        }
        // Where each line ends (just after its newline); a position's line is
        // one more than the number of lines that end at or before it.
        let ends: Vec<usize> = text
            .split_inclusive('\n')
            .scan(0, |end, line| {
                *end += line.len();
                Some(*end)
            })
            .collect();
        let line_of = |pos: usize| {
            if name {
                0
            } else {
                ends.partition_point(|&e| e <= pos) + 1
            }
        };
        let footers: Vec<usize> = KEY_FOOTER.find_iter(text).map(|m| m.start()).collect();
        // Covered line ranges, kept as a difference array so many blocks
        // stay linear.
        let last = line_of(text.len());
        let mut depth = vec![0i64; last + 2];
        for h in found.iter().filter(|h| !h.placeholder) {
            let from = line_of(h.start);
            let to = if h.kind == SecretKind::PrivateKey {
                footers
                    .get(footers.partition_point(|&f| f < h.end))
                    .map_or(last, |&f| line_of(f))
            } else {
                // A bearer value can sit on the line after `Bearer`.
                line_of(h.end)
            };
            depth[from] += 1;
            depth[to + 1] -= 1;
            let finding = Finding {
                path: label.to_string(),
                line: from,
                rule: RuleId::Secret(h.kind),
                excerpt: h.shown(text),
                value: digest(&text[h.start..h.end]),
            };
            if seen.insert(finding.clone()) {
                self.findings.push(finding);
            }
        }
        self.covered
            .resize(self.covered.len().max(depth.len()), false);
        let mut open = 0;
        for (line, d) in depth.iter().enumerate() {
            open += d;
            self.covered[line] |= open > 0;
        }
    }
}

/// A stand-in for a matched secret in `Finding::value`: equal for equal
/// text, so a secret read twice (two readings of a file) is reported once,
/// and never the text itself.
fn digest(text: &str) -> String {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    format!("digest:{:016x}", h.finish())
}

/// The secret findings in `bytes`, on every reading of them (see
/// `scan::text_views`), after `prep` has been applied to each.
pub(crate) fn scan_bytes(label: &str, bytes: &[u8], prep: &dyn Fn(&str) -> String) -> Secrets {
    let mut out = Secrets::default();
    let mut seen = HashSet::new();
    for view in text_views(bytes) {
        out.add(label, &prep(&view), false, &mut seen);
    }
    out
}

/// The secret findings in a path name, at line 0.
pub(crate) fn scan_name(path: &str) -> Secrets {
    let mut out = Secrets::default();
    out.add(path, path, true, &mut HashSet::new());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Key-shaped text from parts, so no literal here matches a rule.
    fn key(prefix_a: &str, prefix_b: &str, body: &str) -> String {
        format!("{prefix_a}{prefix_b}{body}")
    }

    fn body(n: usize) -> String {
        (0..n)
            .map(|i| char::from(b"Q7wE3rT9yU1iO5pA"[i % 16]))
            .collect()
    }

    fn rules_in(text: &str) -> Vec<&'static str> {
        let s = scan_bytes("f", text.as_bytes(), &str::to_string);
        s.findings.iter().map(|f| f.rule.name()).collect()
    }

    #[test]
    fn each_body_length_threshold_is_exact() {
        let anthropic = |n| key("sk-", "ant-", &body(n));
        assert_eq!(rules_in(&anthropic(32)), ["SecretAnthropicKey"]);
        assert!(rules_in(&anthropic(31)).is_empty());
        let project = |n| key("sk-", "proj-", &body(n));
        assert_eq!(rules_in(&project(32)), ["SecretOpenAiKey"]);
        assert!(rules_in(&project(31)).is_empty());
        let legacy = |n| key("s", "k-", &body(n));
        assert_eq!(rules_in(&legacy(32)), ["SecretOpenAiKey"]);
        assert!(rules_in(&legacy(31)).is_empty());
        let hosting = |n| key("gh", "s_", &body(n));
        assert_eq!(rules_in(&hosting(36)), ["SecretHostingToken"]);
        assert!(rules_in(&hosting(35)).is_empty());
        let fine = |n| key("github_", "pat_", &body(n));
        assert_eq!(rules_in(&fine(36)), ["SecretHostingToken"]);
        assert!(rules_in(&fine(35)).is_empty());
        let bearer = |n| key("Bear", "er ", &body(n));
        assert_eq!(rules_in(&bearer(20)), ["SecretBearer"]);
        assert!(rules_in(&bearer(19)).is_empty());
        // The cloud rule is exact: 16, and not 15 or 17.
        let upper = "QWERTYUIOPASDFGHJ";
        let cloud = |n: usize| key("AK", "IA", &upper[..n]);
        assert_eq!(rules_in(&cloud(16)), ["SecretCloudAccessKey"]);
        assert!(rules_in(&cloud(15)).is_empty());
        assert!(rules_in(&cloud(17)).is_empty());
        assert!(rules_in(&format!("{}x", cloud(16))).is_empty());
        assert_eq!(
            rules_in(&format!("{}-x", cloud(16))),
            ["SecretCloudAccessKey"]
        );
    }

    #[test]
    fn a_rule_needs_a_boundary_before_its_prefix() {
        let k = key("gh", "p_", &body(36));
        assert!(rules_in(&format!("x{k}")).is_empty());
        assert!(rules_in(&format!("7{k}")).is_empty());
        for before in ["", " ", "_", "\"", "=", "/", "\n"] {
            assert_eq!(
                rules_in(&format!("{before}{k}")),
                ["SecretHostingToken"],
                "{before:?}"
            );
        }
        // The key block header needs none.
        let header = key("x-----BEGIN ", "RSA PRIVATE KEY", "-----");
        assert_eq!(rules_in(&header), ["SecretPrivateKey"]);
    }

    #[test]
    fn placeholders_are_one_repeated_character_after_example() {
        assert!(placeholder_body("xxxxxxxxxxxxxxxx"));
        assert!(placeholder_body("EXAMPLE000000000"));
        assert!(placeholder_body("EXAMPLExxxxxxx=="));
        assert!(!placeholder_body("xxxxxxxxxxxxxxxy"));
        assert!(!placeholder_body("EXAMPLE00000001"));
        assert!(!placeholder_body("example000000000"));
        assert!(!placeholder_body("0EXAMPLE00000000"));
        assert!(!placeholder_body("Q7wE3rT9yU1iO5pA"));
        let k = key("gh", "p_", &"x".repeat(36));
        assert!(rules_in(&k).is_empty());
        // Only the documented cloud value, not one that differs by a letter.
        let mut near = DOCUMENTED_EXAMPLE.to_string();
        assert!(rules_in(&near).is_empty());
        near.replace_range(4..5, "J");
        assert_eq!(rules_in(&near), ["SecretCloudAccessKey"]);
    }

    #[test]
    fn redact_hides_every_body_and_keeps_the_rest() {
        let k = key("gh", "o_", &body(36));
        assert_eq!(
            redact(&format!("a {k} b")),
            "a gho_[36 characters hidden] b"
        );
        assert_eq!(redact("plain text"), "plain text");
        // Two bodies, and a body inside another (the legacy pattern inside
        // an Anthropic body): each character is hidden once.
        let inner = key("s", "k-", &body(40));
        let outer = key("sk-", "ant-", &format!("{}-{inner}", body(10)));
        let two = format!("{outer} and {k}");
        assert_eq!(
            redact(&two),
            "sk-ant-[54 characters hidden] and gho_[36 characters hidden]"
        );
        // A token inside a bearer value.
        let overlap = key(
            "Bearer ",
            "",
            &format!("{}.{}", body(20), key("gh", "p_", &body(36))),
        );
        assert_eq!(redact(&overlap), "Bearer [61 characters hidden]");
    }

    #[test]
    fn a_key_block_covers_the_lines_up_to_its_footer() {
        let header = key("-----BEGIN ", "EC PRIVATE KEY", "-----");
        let footer = key("-----END ", "EC PRIVATE KEY", "-----");
        let text = format!("a\n{header}\nb\nc\n{footer}\nd\n");
        let s = scan_bytes("f", text.as_bytes(), &str::to_string);
        assert_eq!(
            s.covered,
            [false, false, true, true, true, true, false, false, false]
        );
        // Without a footer, to the end.
        let text = format!("a\n{header}\nb\n");
        let s = scan_bytes("f", text.as_bytes(), &str::to_string);
        assert_eq!(s.covered, [false, false, true, true, true, false]);
        // A footer before the header does not end it.
        let text = format!("{footer}\n{header}\nb\n");
        let s = scan_bytes("f", text.as_bytes(), &str::to_string);
        assert_eq!(s.covered, [false, false, true, true, true, false]);
        // A footer on the header's own line ends the block there, even right
        // after it.
        for gap in [" ", ""] {
            let text = format!("a\n{header}{gap}{footer}\nb\n");
            let s = scan_bytes("f", text.as_bytes(), &str::to_string);
            assert_eq!(
                s.covered,
                [false, false, true, false, false, false],
                "{gap:?}"
            );
        }
        // A key at the very start of the text is on line 1.
        let s = scan_bytes("f", header.as_bytes(), &str::to_string);
        assert_eq!(
            (s.findings[0].line, s.covered.as_slice()),
            (1, &[false, true, true, false][..])
        );
    }

    #[test]
    fn a_token_covers_its_own_line_and_a_name_line_zero() {
        let k = key("gh", "u_", &body(36));
        let text = format!("a\n{k}\nb\n");
        let s = scan_bytes("f", text.as_bytes(), &str::to_string);
        assert_eq!(s.covered, [false, false, true, false, false, false]);
        assert_eq!(s.findings[0].line, 2);
        let s = scan_name(&format!("dir/{k}.txt"));
        assert_eq!(s.covered, [true, false]);
        assert_eq!(s.findings[0].line, 0);
        // A placeholder covers nothing.
        let s = scan_name(&format!("dir/{}.txt", key("gh", "u_", &"0".repeat(36))));
        assert!(s.findings.is_empty() && s.covered.is_empty());
    }

    #[test]
    fn a_finding_shows_the_prefix_and_a_digest_never_the_body() {
        let b = body(36);
        let k = key("gh", "r_", &b);
        let s = scan_bytes("f", k.as_bytes(), &str::to_string);
        assert_eq!(s.findings.len(), 1);
        assert_eq!(s.findings[0].excerpt, "ghr_[36 characters hidden]");
        assert!(s.findings[0].value.starts_with("digest:"));
        assert!(!s.findings[0].value.contains(&b[..12]));
        // A bearer value is shown after one space, whatever separated it.
        for gap in [" ", "\n", "%20"] {
            let text = format!("{}{gap}{}", "Bearer", body(25));
            let s = scan_bytes("f", text.as_bytes(), &str::to_string);
            assert_eq!(s.findings[0].excerpt, "Bearer [25 characters hidden]");
        }
        // Two secrets on one line stay two findings.
        let other: String = b.chars().rev().collect();
        let two = format!("{k} {}", key("gh", "r_", &other));
        assert_eq!(rules_in(&two).len(), 2);
        // One secret that both readings see on the same line (UTF-8 at the
        // start, a UTF-16 copy after it) stays one finding.
        let utf16: Vec<u8> = k.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let mut alone = vec![0xFF, 0xFE];
        alone.extend(&utf16);
        assert_eq!(scan_bytes("f", &alone, &str::to_string).findings.len(), 1);
        let mut both = format!("{k}  ").into_bytes();
        both.extend(&utf16);
        assert_eq!(text_views(&both).len(), 2);
        assert_eq!(scan_bytes("f", &both, &str::to_string).findings.len(), 1);
    }

    #[test]
    fn merge_hides_excerpts_on_covered_lines_only() {
        let k = key("gh", "p_", &body(36));
        let text = format!("x\n{k}\ny\n");
        let s = scan_bytes("f", text.as_bytes(), &str::to_string);
        let other = |line| Finding {
            path: "f".into(),
            line,
            rule: RuleId::Email,
            excerpt: "shown".into(),
            value: "shown".into(),
        };
        let merged = s.merge(vec![other(1), other(2), other(3), other(99)]);
        let excerpts: Vec<&str> = merged.iter().map(|f| f.excerpt.as_str()).collect();
        assert_eq!(
            excerpts,
            [
                "shown",
                HIDDEN,
                "shown",
                "shown",
                "ghp_[36 characters hidden]"
            ]
        );
    }

    /// No window of 12 characters of `hidden` appears in `shown`.
    fn never_shown(hidden: &str, shown: &str) {
        let chars: Vec<char> = hidden.chars().collect();
        for w in chars.windows(12) {
            let piece: String = w.iter().collect();
            assert!(!shown.contains(&piece), "{shown}");
        }
    }

    #[test]
    fn a_key_header_shows_only_a_label_of_short_capital_words() {
        let token_body = body(36);
        let cloud_body = "QWERTYUIOPASDFGH";
        // Anything but short capital words in the middle is masked: a token
        // or a cloud key (with or without a boundary before it), lower case,
        // or a long run.
        for (inner, hidden) in [
            (key("gh", "p_", &token_body), token_body.clone()),
            (key("Xgh", "p_", &token_body), token_body.clone()),
            (key("AK", "IA", cloud_body), cloud_body.to_string()),
            (key("7AK", "IA", cloud_body), cloud_body.to_string()),
            ("rsa".to_string(), "rsa".to_string()),
            ("QWERTYUIOPA".to_string(), "QWERTYUIOPA".to_string()),
            ("A B C D E".to_string(), "A B C D E".to_string()),
        ] {
            let header = format!("{}{inner}{}", "-----BEGIN ", " PRIVATE KEY-----");
            let s = scan_bytes("f", header.as_bytes(), &str::to_string);
            let key_finding = s
                .findings
                .iter()
                .find(|f| f.rule == RuleId::Secret(SecretKind::PrivateKey))
                .expect("the header is a finding");
            assert_eq!(key_finding.excerpt, "-----BEGIN[..]PRIVATE KEY-----");
            for f in &s.findings {
                never_shown(&hidden, &f.excerpt);
            }
        }
        // A real label is shown as it is.
        for (header, shown) in [
            (key("-----BEGIN ", "RSA PRIVATE KEY", "-----"), None),
            (key("-----BEGIN ", "PRIVATE KEY", "-----"), None),
            (key("-----BEGIN PGP ", "PRIVATE KEY BLOCK", "-----"), None),
            (
                key("---- BEGIN SSH2 ", "ENCRYPTED PRIVATE KEY", " ----"),
                None,
            ),
            (key("PuTTY-User-", "Key-File-3:", ""), None),
            (
                key("PuTTY-User-", "Key-File-", "123:"),
                Some("PuTTY-User-Key-File-[..]:"),
            ),
            (key("-----BEGIN ", "A B C D PRIVATE KEY", "-----"), None),
        ] {
            let s = scan_bytes("f", header.as_bytes(), &str::to_string);
            assert_eq!(s.findings[0].excerpt, shown.unwrap_or(&header), "{header}");
        }
    }

    #[test]
    fn a_key_after_an_escape_or_a_percent_code_is_found() {
        let k = key("gh", "p_", &body(36));
        for before in [r"\n", r"\t", r"\r", r"\u0020", "%3D", "%20", "%2f"] {
            let text = format!("\"a{before}{k}\"");
            assert_eq!(rules_in(&text), ["SecretHostingToken"], "{before}");
        }
        // A letter before an escape-shaped text is still no boundary for
        // the letter itself; only the escape is.
        assert!(rules_in(&format!("x{k}")).is_empty());
        let bearer_value = body(30);
        for gap in [
            "%20", "%20%20", r"\n", r"\t", "\n", "\r\n", " \n ", " \t%20",
        ] {
            let text = format!("{}{gap}{bearer_value}", "Bearer");
            assert_eq!(rules_in(&text), ["SecretBearer"], "{gap:?}");
        }
        // At most one line break: prose that ends a line with the word,
        // then a blank line, then a path, is not a finding.
        for gap in ["\n\n", "\n \n    ", "\r\n\r\n", " \n\n ", r"\n\n", "\n\\t"] {
            let text = format!(
                "{}{gap}{}",
                "as a bearer", "crates/udx-provider/src/auth.rs"
            );
            assert!(rules_in(&text).is_empty(), "{gap:?}");
        }
        // A value on the line after `Bearer` covers that line too.
        let text = format!("a\n{}\n{bearer_value} jane\nb\n", "Bearer");
        let s = scan_bytes("f", text.as_bytes(), &str::to_string);
        assert_eq!(s.findings[0].line, 2);
        assert_eq!(s.covered[..5], [false, false, true, true, false]);
    }

    #[test]
    fn the_none_project_form_and_two_more_key_formats_are_found() {
        assert_eq!(
            rules_in(&key("sk-", "None-", &body(48))),
            ["SecretOpenAiKey"]
        );
        let rfc = key("---- BEGIN SSH2 ", "ENCRYPTED PRIVATE KEY", " ----");
        let rfc_end = key("---- END SSH2 ", "ENCRYPTED PRIVATE KEY", " ----");
        let text = format!("a\n{rfc}\nb\n{rfc_end}\nc\n");
        let s = scan_bytes("f", text.as_bytes(), &str::to_string);
        assert_eq!(s.findings.len(), 1);
        assert_eq!(s.findings[0].line, 2);
        assert_eq!(
            s.covered,
            [false, false, true, true, true, false, false, false]
        );
        let putty = key("PuTTY-User-Key-", "File-3:", " ssh-ed25519");
        let mac = key("Private-", "MAC:", " 00");
        let text = format!("{putty}\nPrivate-Lines: 1\nb\n{mac}\nc\n");
        let s = scan_bytes("f", text.as_bytes(), &str::to_string);
        assert_eq!(s.findings.len(), 1);
        assert_eq!(s.findings[0].rule, RuleId::Secret(SecretKind::PrivateKey));
        assert_eq!(
            s.covered,
            [false, true, true, true, true, false, false, false]
        );
        // Public forms are not findings.
        assert!(rules_in(&key("---- BEGIN SSH2 ", "PUBLIC KEY", " ----")).is_empty());
        assert!(rules_in(&key("PuTTY-User-Key-", "File-", "x")).is_empty());
    }
}
