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

// A home folder: the account name is restricted to plain name characters, so
// placeholders such as a shell variable or an angle-bracket name never match.
static HOME_UNIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[^A-Za-z0-9._~-])(?P<p>/(?:Users|home)/(?P<n>[A-Za-z0-9._-]+)/)").unwrap()
});
static HOME_WINDOWS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?P<p>[a-z]:\\{1,2}Users\\{1,2}(?P<n>[A-Za-z0-9._-]+)\\)").unwrap()
});

static NUMBER_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9]+(?:\.[0-9]+)*").unwrap());

static PRIVATE_HOST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:[a-z0-9-]+\.)+ts\.net\b").unwrap());

static SESSION_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)claude\.ai/(?:code/session_[A-Za-z0-9_-]*|code/artifact/[^\s)>\]]*|artifact/[^\s)>\]]*)")
        .unwrap()
});

const PLACEHOLDER_HOMES: [&str; 7] = ["runner", "you", "me", "user", "username", "name", "example"];

fn email_allowed(m: &str) -> bool {
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
    PLACEHOLDER_HOMES.contains(&name.to_ascii_lowercase().as_str())
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
    for caps in HOME_UNIX.captures_iter(line) {
        if !home_allowed(&caps["n"]) {
            out.push((RuleId::HomePath, caps["p"].to_string()));
        }
    }
    for caps in HOME_WINDOWS.captures_iter(line) {
        if !home_allowed(&caps["n"]) {
            out.push((RuleId::HomePath, caps["p"].to_string()));
        }
    }
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
