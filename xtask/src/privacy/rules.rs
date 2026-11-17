//! The generic privacy rules: what each one flags and what it allows.

use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleId {
    Email,
    HomePath,
    PrivateAddress,
    PrivateHost,
    SessionLink,
    LocalTerm,
    LocalFileTracked,
}

impl RuleId {
    pub fn name(self) -> &'static str {
        match self {
            RuleId::Email => "Email",
            RuleId::HomePath => "HomePath",
            RuleId::PrivateAddress => "PrivateAddress",
            RuleId::PrivateHost => "PrivateHost",
            RuleId::SessionLink => "SessionLink",
            RuleId::LocalTerm => "LocalTerm",
            RuleId::LocalFileTracked => "LocalFileTracked",
        }
    }

    pub fn from_name(name: &str) -> Option<RuleId> {
        [
            RuleId::Email,
            RuleId::HomePath,
            RuleId::PrivateAddress,
            RuleId::PrivateHost,
            RuleId::SessionLink,
            RuleId::LocalTerm,
            RuleId::LocalFileTracked,
        ]
        .into_iter()
        .find(|r| r.name() == name)
    }
}

static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}").unwrap()
});

// Characters that never belong to an account name: separators, whitespace and
// placeholder or syntax characters.
const NAME_STOP: &str = r#"/\\\s<>%$:{}*?"'|,;()\[\]=&#`"#;

static HOME_UNIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)/(?:users|home)/([^{NAME_STOP}]+)")).unwrap());
// With a following backslash the name may hold single spaces.
static HOME_WINDOWS_SPACED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)[a-z]:\\{{1,2}}users\\{{1,2}}([^{NAME_STOP}]+(?: [^{NAME_STOP}]+)*)\\"
    ))
    .unwrap()
});
static HOME_WINDOWS_PLAIN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)[a-z]:\\{{1,2}}users\\{{1,2}}([^{NAME_STOP}]+)"
    ))
    .unwrap()
});
static URL_SCHEME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z][A-Za-z0-9+.-]*)://").unwrap());
static RETINA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)@[0-9]+x\.(?:png|jpe?g|gif|webp|svg|ico|icns|pdf)$").unwrap()
});

static NUMBER_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9]+(?:\.[0-9]+)*").unwrap());

static PRIVATE_HOST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:[a-z0-9-]+\.)+ts\.net\b").unwrap());

static SESSION_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)claude\.ai/(?:code/session_[A-Za-z0-9_-]*|code/artifact/[^\s)>\]"'`,]*|artifact/[^\s)>\]"'`,]*)"#)
        .unwrap()
});

const PLACEHOLDER_HOMES: [&str; 11] = [
    "runner",
    "runneradmin",
    "you",
    "me",
    "user",
    "username",
    "name",
    "example",
    "shared",
    "public",
    "default",
];

fn email_allowed(m: &str) -> bool {
    if RETINA.is_match(m) {
        return true;
    }
    let m = m.to_ascii_lowercase();
    if m.ends_with("@users.noreply.github.com")
        || m == "noreply@anthropic.com"
        || m == "git@github.com"
    {
        return true;
    }
    let domain = m.rsplit('@').next().unwrap_or("");
    ["example.com", "example.org", "example.net"]
        .iter()
        .any(|d| domain == *d || domain.ends_with(&format!(".{d}")))
}

fn home_allowed(name: &str) -> bool {
    PLACEHOLDER_HOMES.contains(&name.trim().to_lowercase().as_str())
}

/// True when the match sits inside a URL whose scheme is not `file`.
fn inside_url(line: &str, start: usize) -> bool {
    let token_start = line[..start].rfind(char::is_whitespace).map_or(0, |i| {
        i + line[i..].chars().next().map_or(1, char::len_utf8)
    });
    URL_SCHEME
        .captures_iter(&line[token_start..start])
        .any(|c| !c[1].eq_ignore_ascii_case("file"))
}

/// Home paths on one line. Matches may overlap, so each search resumes one
/// character after the previous match started.
fn home_matches(line: &str, out: &mut Vec<(RuleId, String)>) {
    for re in [&*HOME_UNIX, &*HOME_WINDOWS_SPACED] {
        let mut pos = 0;
        while pos <= line.len() {
            let Some(caps) = re.captures_at(line, pos) else {
                break;
            };
            let whole = caps.get(0).unwrap();
            let name = caps.get(1).unwrap();
            // The spaced Windows form ends at the backslash; keep it out of the text.
            let trimmed = name.as_str().trim_end_matches('.');
            if trimmed.is_empty() {
                pos = whole.start() + 1;
                continue;
            }
            let end = name.start() + trimmed.len();
            if !home_allowed(trimmed) && !inside_url(line, whole.start()) {
                out.push((RuleId::HomePath, line[whole.start()..end].to_string()));
            }
            pos = whole.start()
                + line[whole.start()..]
                    .chars()
                    .next()
                    .map_or(1, char::len_utf8);
        }
    }
    // Windows path whose name is not followed by a backslash.
    let mut pos = 0;
    while pos <= line.len() {
        let Some(caps) = HOME_WINDOWS_PLAIN.captures_at(line, pos) else {
            break;
        };
        let whole = caps.get(0).unwrap();
        let rest = &line[whole.end()..];
        let covered_by_spaced = rest.starts_with('\\')
            || line[whole.end()..].starts_with(' ')
                && HOME_WINDOWS_SPACED
                    .captures_at(line, whole.start())
                    .is_some_and(|c| c.get(0).unwrap().start() == whole.start());
        let name = caps[1].trim_end_matches('.');
        if !covered_by_spaced
            && !name.is_empty()
            && !home_allowed(name)
            && !inside_url(line, whole.start())
        {
            let end = caps.get(1).unwrap().start() + name.len();
            out.push((RuleId::HomePath, line[whole.start()..end].to_string()));
        }
        pos = whole.start()
            + line[whole.start()..]
                .chars()
                .next()
                .map_or(1, char::len_utf8);
    }
}

fn private_v4(parts: &[u8; 4]) -> bool {
    matches!(
        (parts[0], parts[1]),
        (10, _) | (172, 16..=31) | (192, 168) | (100, 64..=127) | (169, 254)
    )
}

/// Every generic-rule match on one line, as (rule, matched text), in rule order.
pub(crate) fn generic_matches(line: &str) -> Vec<(RuleId, String)> {
    let mut out = Vec::new();
    for m in EMAIL.find_iter(line) {
        if !email_allowed(m.as_str()) {
            out.push((RuleId::Email, m.as_str().to_string()));
        }
    }
    home_matches(line, &mut out);
    for run in NUMBER_RUN.find_iter(line) {
        let parts: Vec<&str> = run.as_str().split('.').collect();
        if parts.len() != 4 || parts.iter().any(|p| p.len() > 3) {
            continue;
        }
        let nums: Vec<u8> = parts.iter().filter_map(|p| p.parse().ok()).collect();
        if let Ok(v4) = <[u8; 4]>::try_from(nums)
            && private_v4(&v4)
        {
            out.push((RuleId::PrivateAddress, run.as_str().to_string()));
        }
    }
    for m in PRIVATE_HOST.find_iter(line) {
        out.push((RuleId::PrivateHost, m.as_str().to_string()));
    }
    for m in SESSION_LINK.find_iter(line) {
        out.push((RuleId::SessionLink, m.as_str().to_string()));
    }
    out
}
