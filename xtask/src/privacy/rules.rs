//! The generic privacy rules: what each one flags and what it allows.

use super::secrets::SecretKind;
use regex::Regex;
use std::collections::HashSet;
use std::net::Ipv6Addr;
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
    /// A secret rule (`secrets.rs`); never exempted through the allow file.
    Secret(SecretKind),
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
            RuleId::Secret(kind) => kind.name(),
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
        .chain(SecretKind::ALL.map(RuleId::Secret))
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
// The same, percent-encoded once or twice, whole or in part (`%2FUsers%2Fname`,
// `%252Fhome%252Fname`, `/Users%2Fname`, `%2Fhome/name`, `C%3A%5CUsers%5Cname`,
// `C%253A%255CUsers%255Cname`), as it appears in a URL's query string, or
// with JSON-escaped slashes (a backslash before each `/`). A drive written
// with `%2F` is read by the first of them. Each also matches the plain
// spelling, which the plain rules report, so `home_matches` uses a match of
// these only when it holds a `%` or a backslash.
static HOME_UNIX_ENCODED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)(?:/|%2F|%252F|\\/)(?:users|home)(?:/|%2F|%252F|\\/)([^{NAME_STOP}]+)"
    ))
    .unwrap()
});
// A UNC path to a home share (`\\server\Users\name`, `\\wsl$\Ubuntu\home\name`,
// the long `\\?\UNC\` form, and the doubled backslashes of an escaped
// string). A device path (`\\?\C:\...`) is a drive path, which the drive
// rules report.
static HOME_UNC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\\{{2,4}}(?:\?\\UNC\\)?[^\\\s/?.][^\\\s/]*(?:\\{{1,2}}[^\\\s/]+)*?\\{{1,2}}(?:users|home)\\{{1,2}}([^{NAME_STOP}]+)"
    ))
    .unwrap()
});
// A Cygwin or MSYS2 home under a drive, in backslash spelling
// (`C:\cygwin64\home\name`, `C:\tools\msys64\home\name`); the forward-slash
// spelling is a plain Unix match.
static HOME_CYGWIN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)[a-z]:\\{{1,2}}(?:[^\\\s]+\\{{1,2}}){{0,2}}?(?:cygwin(?:64)?|msys(?:2|32|64)?)\\{{1,2}}home\\{{1,2}}([^{NAME_STOP}]+)"
    ))
    .unwrap()
});
static HOME_WINDOWS_ENCODED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)[a-z](?::|%3A|%253A)(?:\\|%5C|%255C){{1,2}}users(?:\\|%5C|%255C)+([^{NAME_STOP}]+)"
    ))
    .unwrap()
});
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
static RETINA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)@[0-9]+x\.(?:png|jpe?g|gif|webp|svg|ico|icns|pdf)$").unwrap()
});

static NUMBER_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9]+(?:\.[0-9]+)*").unwrap());

static PRIVATE_HOST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:[a-z0-9-]+\.)+ts\.net\b").unwrap());

// A name on a local network: a dotted name ending in `local`, `lan` or
// `home.arpa`. A dotted chain that is code or a file name is not one; see
// `local_names`. The word boundaries are ASCII ones, which keep the regex on
// its fast engine on non-ASCII text; `LOCAL_NAME_UNICODE` is the same with
// Unicode boundaries, run as well on the rare line where they could differ.
static LOCAL_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)[a-z0-9_-]+(?:\.[a-z0-9_-]+)*\.(?:local|lan|home\.arpa)(?-u:\b)")
        .unwrap()
});
static LOCAL_NAME_UNICODE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[a-z0-9_-]+(?:\.[a-z0-9_-]+)*\.(?:local|lan|home\.arpa)\b").unwrap()
});
static LOCAL_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(?:local|lan|home\.arpa)").unwrap());

// A shell prompt that names an account and a machine: `user@host:~$ `,
// `user@host:/srv# `, `[user@host dir]$ `.
static PROMPT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:(?-u:\b)|\[)[A-Za-z_][A-Za-z0-9_.-]*@([A-Za-z0-9][A-Za-z0-9.-]*)(?::[~/][^\s$#%]*\s?[$#%](?:\s|$)|\s+[^\]\s]+\][$#%])",
    )
    .unwrap()
});

// The default prompts of macOS, which name the short host name (no dots):
// zsh's `user@host dir % ` (or `$`, `#`) and the older bash `host:dir user$ `.
// A colon followed by `//` is a URL, not a bash prompt.
static PROMPT_ZSH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?-u:\b)[A-Za-z_][A-Za-z0-9_.-]*@([A-Za-z0-9][A-Za-z0-9-]*)[ \t]+[^\s$#%]+[ \t]+[$#%](?:\s|$)",
    )
    .unwrap()
});
static PROMPT_BASH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?-u:\b)([A-Za-z0-9][A-Za-z0-9-]*):(?:~[^\s$#%]*|/(?:[^/\s$#%][^\s$#%]*)?)[ \t]+[A-Za-z_][A-Za-z0-9_.-]*[$#](?:\s|$)",
    )
    .unwrap()
});

// Candidate IPv6 text: hexadecimal digits, colons, dots (an IPv4 tail) and a
// zone suffix (`%en0`). Only a token that holds a colon can be an address, so
// one without is never a match (the same tokens as `[0-9A-Za-z_:.%]+` that
// hold a colon, without parsing every word of a file).
static V6_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[0-9A-Za-z_.%]*:[0-9A-Za-z_:.%]*").unwrap());

// Six pairs of hexadecimal digits: a hardware (MAC) address. ASCII word
// boundaries: the first and last characters are ASCII, so these find every
// match Unicode boundaries would, and more next to a non-ASCII letter.
static HW_ADDR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?-u:\b)(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2}(?-u:\b)").unwrap());

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

/// The hosting service's no-reply address, in either of its forms
/// (`<digits>+<account>@users.noreply.github.com` or
/// `<account>@users.noreply.github.com`), compared without regard to case.
fn hosting_noreply(m: &str) -> bool {
    m.to_ascii_lowercase()
        .ends_with("@users.noreply.github.com")
}

/// The account name inside a hosting-service no-reply address (see
/// `hosting_noreply`): the local part, without a leading `<digits>+`.
pub(crate) fn noreply_account(address: &str) -> Option<&str> {
    if !hosting_noreply(address) {
        return None;
    }
    let (local, _) = address.rsplit_once('@')?;
    Some(match local.split_once('+') {
        Some((digits, account))
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) =>
        {
            account
        }
        _ => local,
    })
}

fn email_allowed(m: &str) -> bool {
    if RETINA.is_match(m) {
        return true;
    }
    let m = m.to_ascii_lowercase();
    if hosting_noreply(&m)
        || m == "noreply@anthropic.com"
        || m == "git@github.com"
        // GitHub's own committer address for merges made through its interface.
        || m == "noreply@github.com"
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

/// How far back, in bytes, a look-back reads before it gives up. A look-back
/// that cannot decide within it treats the match as flagged, so the work per
/// match is constant and a pathological line fails towards flagging.
const LOOK_BACK: usize = 1024;

/// Where the run that ends at `end` starts: just after the nearest character
/// before `end` for which `stop` holds, or 0 when the line start comes first.
/// `None` when neither lies within `LOOK_BACK` bytes: the caller cannot decide.
fn run_start(line: &str, end: usize, stop: impl Fn(char) -> bool) -> Option<usize> {
    let lo = line.floor_char_boundary(end.saturating_sub(LOOK_BACK));
    for (i, c) in line[lo..end].char_indices().rev() {
        if stop(c) {
            return Some(lo + i + c.len_utf8());
        }
    }
    (lo == 0).then_some(0)
}

/// True when the match lies inside an `http://` or `https://` URL's path: one
/// is reached scanning backwards before any URL-ending delimiter, with no `?`
/// or `#` after it (a query string or fragment can carry a real local path).
/// A URL segment that starts further back than the look-back is not read as
/// one, so the match stays flagged.
fn inside_url(line: &str, start: usize) -> bool {
    const DELIMS: &[char] = &[
        '"', '\'', ',', '<', '>', '(', ')', '[', ']', '{', '}', '|', ';', '\\', '`',
    ];
    let Some(seg_start) = run_start(line, start, |c| c.is_whitespace() || DELIMS.contains(&c))
    else {
        return false;
    };
    let seg = line[seg_start..start].to_ascii_lowercase();
    let after_scheme = ["http://", "https://"]
        .iter()
        .filter_map(|s| seg.find(s).map(|i| i + s.len()))
        .min();
    after_scheme.is_some_and(|at| !seg[at..].contains(['?', '#']))
}

/// Characters that end the run before a plain Unix home match: whitespace,
/// quotes, brackets, `=`, `,`, `;`, `|`, `<`, `>` and `:` (which also
/// separates the entries of a `PATH`).
const RUN_DELIMS: &[char] = &[
    '"', '\'', '`', '=', ',', ';', '|', '<', '>', '(', ')', '[', ']', '{', '}', ':',
];

/// A character of a relative path's run: a letter, a digit, `_`, `-`, `.`
/// or `/`.
fn path_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/')
}

/// The exemption for a relative path with a `home` or `users` module folder
/// (a `home` folder under `src`, or `./users/x`). `start` is where a plain Unix match
/// (`/home/...` or `/users/...`, the first rule's regex) begins. The contract,
/// from the controller's ruling: the match is exempt only when all of these
/// hold, and flagged in every other case.
///
/// 1. The match is the plain Unix form. The caller applies this to
///    `HOME_UNIX` matches only, never to the drive-letter, UNC or
///    percent-encoded forms.
/// 2. The folder name is lowercase `home` or `users`; any other casing
///    (`Users`, `Home`) is flagged.
/// 3. The run of path characters (letters, digits, `_`, `-`, `.`, `/`)
///    immediately before the match, back to the nearest delimiter
///    (`RUN_DELIMS`, whitespace or the line start), starts with an ASCII
///    letter, digit or `_`, optionally after one leading `./`. A run that
///    starts with `-`, `~`, `.` (other than that one `./`) or `/`, an empty
///    run, and a run that reaches a character that is neither a path
///    character nor a delimiter (`$`, `~`, `\`, a replacement character) do
///    not qualify.
/// 4. The run contains no `..` segment.
/// 5. The run is not the continuation of an absolute path across a space:
///    when the delimiter before the run is a single space separator (any
///    Unicode space separator: the ASCII space, a no-break space, the
///    ideographic space and the like, but not a tab) and the
///    whitespace-separated token before it, on the same line, begins an
///    absolute path, the match is flagged. The token begins one when it
///    starts with `/`, `~` or a drive letter and `:`; or when, once a glued
///    option is dropped (a run of `-` and letters, as in `-I/...`), it starts
///    with `/` or `~`, or does so right after its last quote, opening
///    bracket, `=`, `:`, `,` or `;`, or right after its last character that
///    cannot be part of a path (anything but a letter or digit of any
///    script, `_`, `-`, `.`, `/` or `~`; ASCII letters glued after such a
///    character are dropped like an option); or when it holds a `..` segment
///    (a climb). Before each of these tests, a leading word that begins with
///    a letter or digit outside ASCII is dropped (text in another script or
///    an invisible filler glued before the path, fix round 5). Examples:
///    `"/mnt/Backup Disk/home/...`, `path=/mnt/Backup Disk/home/...`,
///    `+/mnt/Backup Disk/home/...`, `-I/mnt/Backup Disk/home/...`,
///    `../mnt/Backup Disk/home/...` (fix rounds 2 and 3). When the run follows
///    a delimiter inside a token, the token that holds the run is judged too:
///    after dropping its leading quotes and opening brackets it must not
///    begin with a drive path (`C:/`, `C:\`), must not begin an absolute path
///    as above at its start or right after any character that cannot be part
///    of a path (`/backups/2026-11-18T10:20:30/home/...`,
///    `path=/backups/<timestamp>/home/...`), and must not climb; and the
///    space test above applies to the token before it. A token that is exactly a comment
///    marker (`COMMENT_MARKERS`: `//`, `///`, `//!`, `/*`, `/**`, `/*!`) is
///    not the start of an absolute path (controller's ruling, fix round 2);
///    a longer token that begins with `/` (`//server/share`) still is.
/// 6. The segment after the name is not a dotfile or dot-folder, nor one of
///    the standard home folders (`HOME_FOLDERS`): `mirror/home/<name>/.ssh`
///    is a copied home folder, not a module (fix round 2).
///
/// Every look-back reads at most `LOOK_BACK` bytes; one that cannot decide
/// within it flags.
fn relative_module_folder(line: &str, start: usize, name_end: usize) -> bool {
    // Rule 2.
    let folder = &line[start + 1..];
    if !(folder.starts_with("home/") || folder.starts_with("users/")) {
        return false;
    }
    // Rule 3: the run back to the first character that is not a path
    // character, which must be a delimiter (or the line start).
    let Some(from) = run_start(line, start, |c| !path_char(c)) else {
        return false;
    };
    if let Some(prev) = line[..from].chars().next_back()
        && !(prev.is_whitespace() || RUN_DELIMS.contains(&prev))
    {
        return false;
    }
    let run = &line[from..start];
    let body = run.strip_prefix("./").unwrap_or(run);
    let starts_well =
        run == "." || body.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_');
    // Rule 4.
    let climbs = run.split('/').any(|s| s == "..");
    // Rule 5: the token that holds the run, then the token before it.
    let Some(t) = run_start(line, from, char::is_whitespace) else {
        return false;
    };
    let head = line[t..from]
        .trim_start_matches(|c: char| quote_mark(c) || matches!(c, '(' | '[' | '{' | '<'));
    let drive_path = matches!(
        drop_foreign_word(head).as_bytes(),
        [d, b':', b'/' | b'\\', ..] if d.is_ascii_alphabetic()
    );
    let head_absolute = begins_absolute(head, false)
        || head
            .split(|c: char| !path_part(c))
            .skip(1)
            .any(|piece| begins_absolute(piece, true))
        || climbs_in(head);
    starts_well
        && !climbs
        && !head_absolute
        && !drive_path
        && !continues_absolute_path(&line[..t])
        // Rule 6.
        && !mirrored_home(&line[name_end..])
}

/// Folders every home folder has, by their usual names on macOS, Linux
/// (XDG and Ubuntu's `snap`) and Windows (matched without regard to case).
const HOME_FOLDERS: [&str; 20] = [
    "3D Objects",
    "AppData",
    "Applications",
    "Contacts",
    "Desktop",
    "Documents",
    "Downloads",
    "Favorites",
    "Library",
    "Links",
    "Movies",
    "Music",
    "Pictures",
    "Public",
    "Saved Games",
    "Searches",
    "Sites",
    "snap",
    "Templates",
    "Videos",
];

/// Rule 6 of `relative_module_folder`: `after` (the line after the name)
/// starts with a segment that only a home folder holds: a name that begins
/// with `.`, or one of `HOME_FOLDERS`.
fn mirrored_home(after: &str) -> bool {
    let Some(rest) = after.strip_prefix('/') else {
        return false;
    };
    let segment_end =
        |c: char| c == '/' || c == '\\' || c.is_whitespace() || RUN_DELIMS.contains(&c);
    // A folder name may hold a space (`Saved Games`): compare each name with
    // the text after the slash, then require the segment to end there.
    let named = HOME_FOLDERS.iter().any(|f| {
        rest.get(..f.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(f))
            && rest
                .get(f.len()..)
                .is_some_and(|tail| tail.is_empty() || tail.starts_with(segment_end))
    });
    rest.starts_with('.') || named
}

/// A Unicode space separator (general category Zs): the ASCII space, the
/// no-break spaces, the ideographic space and the fixed-width spaces. Not a
/// tab or a line break.
fn space_separator(c: char) -> bool {
    matches!(
        c,
        ' ' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// A quotation mark of any kind: ASCII, typographic, guillemets, corner
/// brackets and their full-width forms.
fn quote_mark(c: char) -> bool {
    matches!(
        c,
        '"' | '\''
            | '`'
            | '\u{00AB}'
            | '\u{00BB}'
            | '\u{2018}'..='\u{201F}'
            | '\u{2039}'
            | '\u{203A}'
            | '\u{300C}'..='\u{300F}'
            | '\u{301D}'..='\u{301F}'
            | '\u{FF02}'
            | '\u{FF07}'
            | '\u{FF62}'
            | '\u{FF63}'
    )
}

/// A character after which an absolute path's text can begin inside a
/// token: a quote, an opening bracket, `=`, `:`, `,` or `;`.
fn path_opener(c: char) -> bool {
    quote_mark(c) || matches!(c, '(' | '[' | '{' | '<' | '=' | ':' | ',' | ';')
}

/// A token that begins an absolute path: `/`, `~` or a drive letter and `:`.
fn absolute_start(token: &str) -> bool {
    matches!(token.as_bytes(), [b'/' | b'~', ..])
        || matches!(token.as_bytes(), [d, b':', ..] if d.is_ascii_alphabetic())
}

/// Tokens that open a comment, which rule 5 never reads as an absolute path.
const COMMENT_MARKERS: [&str; 6] = ["//", "///", "//!", "/*", "/**", "/*!"];

/// A character that can be part of a path in rule 5's view: a path
/// character (a letter or digit of any script, `_`, `-`, `.`, `/`) or `~`.
fn path_part(c: char) -> bool {
    path_char(c) || c == '~'
}

/// `s` without a leading word that begins with a letter or digit outside
/// ASCII (text in another script, an accented word, an invisible filler,
/// glued before a path): the whole leading run of letters and digits of any
/// script. A word that begins with an ASCII letter (`café/x`) is kept.
fn drop_foreign_word(s: &str) -> &str {
    if s.starts_with(|c: char| !c.is_ascii() && c.is_alphanumeric()) {
        s.trim_start_matches(char::is_alphanumeric)
    } else {
        s
    }
}

/// `part` starts an absolute path (`/` or `~`) once a leading foreign word
/// (`drop_foreign_word`) and a glued option are dropped: a leading run of
/// `-` and ASCII letters (`-I`, `--`, `-isystem`), or, when `glued` (the part
/// follows a character that cannot be part of a path, or a foreign word),
/// any leading ASCII letters as well.
fn begins_absolute(part: &str, glued: bool) -> bool {
    let bare = drop_foreign_word(part);
    let glued = glued || bare != part;
    let part = bare;
    let rest = if glued || part.starts_with('-') {
        part.trim_start_matches(|c: char| c == '-' || c.is_ascii_alphabetic())
    } else {
        part
    };
    rest.starts_with(['/', '~'])
}

/// `token` holds a `..` segment, between slashes or characters that cannot
/// be part of a path.
fn climbs_in(token: &str) -> bool {
    token
        .split(|c: char| c == '/' || !path_part(c))
        .any(|s| s == "..")
}

/// Rule 5 of `relative_module_folder`: `before` (the line up to the run)
/// ends in a single space separator after a token that begins an absolute
/// path (see the contract) and is not exactly a comment marker. A token
/// longer than the look-back counts as one.
fn continues_absolute_path(before: &str) -> bool {
    let Some(sep) = before.chars().next_back().filter(|c| space_separator(*c)) else {
        return false;
    };
    let rest = &before[..before.len() - sep.len_utf8()];
    // With no token (the line start, or a second separator), the token is
    // empty.
    let Some(t) = run_start(rest, rest.len(), char::is_whitespace) else {
        return true;
    };
    let token = &rest[t..];
    let after_opener = token
        .rsplit_once(path_opener)
        .is_some_and(|(_, part)| begins_absolute(part, true));
    let after_non_path = token
        .rsplit_once(|c: char| !path_part(c))
        .is_some_and(|(_, part)| begins_absolute(part, true));
    !COMMENT_MARKERS.contains(&token)
        && (absolute_start(drop_foreign_word(token))
            || begins_absolute(token, false)
            || after_opener
            || after_non_path
            || climbs_in(token))
}

/// Home paths on one line. Matches may overlap, so each search resumes one
/// character after the previous match started.
fn home_matches(line: &str, out: &mut Vec<(RuleId, String)>) {
    // Per form: whether it can be a relative path's module folder (only the
    // plain Unix form); the characters a match must hold to count (an encoded
    // form also matches the plain spelling, which the plain forms report);
    // and whether a match is the tail of a match already found, given the
    // character before it and its own first character (the slash after the
    // backslash of an escaped slash, the second backslash of a UNC prefix, a
    // drive's colon before doubled backslashes).
    const ANY: &[char] = &[];
    let never: fn(Option<char>, char) -> bool = |_, _| false;
    // The last field says where the next search starts: one character on
    // (matches may overlap), or, for the UNC form, past the match, whose
    // matches would otherwise restart at every backslash of one path and
    // make a line of doubled backslashes quadratic (fix round 4).
    for (re, plain_unix, needs, is_tail, past_match) in [
        (&*HOME_UNIX, true, ANY, never, false),
        (
            &*HOME_UNIX_ENCODED,
            false,
            &['%', '\\'][..],
            (|prev, first| prev == Some('\\') && first == '/') as fn(Option<char>, char) -> bool,
            false,
        ),
        (&*HOME_WINDOWS_ENCODED, false, &['%'][..], never, false),
        (&*HOME_WINDOWS_SPACED, false, ANY, never, false),
        (
            &*HOME_UNC,
            false,
            ANY,
            |prev, _| matches!(prev, Some('\\' | ':')),
            true,
        ),
        (&*HOME_CYGWIN, false, ANY, never, false),
    ] {
        let mut pos = 0;
        while pos <= line.len() {
            let Some(caps) = re.captures_at(line, pos) else {
                break;
            };
            let whole = caps.get(0).unwrap();
            let name = caps.get(1).unwrap();
            let path_start = whole.start();
            let next = if past_match {
                whole.end()
            } else {
                whole.start()
                    + line[whole.start()..]
                        .chars()
                        .next()
                        .map_or(1, char::len_utf8)
            };
            // The spaced Windows form ends at the backslash; keep it out of the text.
            let trimmed = name.as_str().trim_end_matches('.');
            if trimmed.is_empty() {
                pos = next;
                continue;
            }
            let end = name.start() + trimmed.len();
            let holds_needed = needs.is_empty() || line[path_start..end].contains(needs);
            let a_tail = is_tail(
                line[..path_start].chars().next_back(),
                line[path_start..].chars().next().unwrap_or(' '),
            );
            if holds_needed
                && !a_tail
                && !home_allowed(trimmed)
                && !inside_url(line, path_start)
                && !(plain_unix && relative_module_folder(line, path_start, end))
            {
                out.push((RuleId::HomePath, line[path_start..end].to_string()));
            }
            pos = next;
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

/// Receivers whose `local` or `lan` field is code (`cfg.local`,
/// `process.env.LOCAL`, `myapp.settings.local`). As host names these generic
/// words identify nobody.
const CODE_RECEIVERS: [&str; 14] = [
    "args", "cfg", "conf", "config", "context", "ctx", "env", "flags", "options", "opts", "params",
    "props", "settings", "state",
];

/// Names on a local network. A dotted chain
/// that reads as code or a file name is not one: it has an underscore, starts
/// with `self`, `this`, `crate` or `super`, has a settings-like receiver just
/// before the suffix (`CODE_RECEIVERS`), is followed by `::`, `(` or more
/// dotted text (a method chain, a file name with an extension), or follows a dot (a dotfile).
fn local_names(line: &str, out: &mut Vec<(RuleId, String)>) {
    // ASCII and Unicode boundaries can only disagree next to a non-ASCII
    // character; on a line that holds a local suffix both readings run and
    // every name either one flags is reported.
    // (On an ASCII line both readings agree, so running both there only
    // repeats work; the check is kept to the suffix, which is rare.)
    let unicode = LOCAL_SUFFIX.is_match(line).then_some(&*LOCAL_NAME_UNICODE);
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for m in std::iter::once(&*LOCAL_NAME)
        .chain(unicode)
        .flat_map(|re| re.find_iter(line))
    {
        if !seen.insert((m.start(), m.end())) {
            continue;
        }
        let text = m.as_str();
        let after = &line[m.end()..];
        let lower = text.to_ascii_lowercase();
        let first = lower.split('.').next().unwrap_or("");
        let stem = [".home.arpa", ".local", ".lan"]
            .iter()
            .find_map(|s| lower.strip_suffix(s))
            .unwrap_or(&lower);
        let receiver = stem.rsplit('.').next().unwrap_or("");
        let code = text.contains('_')
            || matches!(first, "self" | "this" | "crate" | "super")
            || CODE_RECEIVERS.contains(&receiver)
            || after.starts_with("::")
            || after.starts_with('(')
            || after.starts_with('.')
                && after[1..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
            || line[..m.start()].ends_with('.');
        if !code {
            out.push((RuleId::PrivateHost, text.to_string()));
        }
    }
}

/// A prompt names a real machine unless the host is a documentation placeholder.
fn prompt_host_allowed(host: &str) -> bool {
    let h = host.to_ascii_lowercase();
    matches!(
        h.as_str(),
        "host" | "hostname" | "localhost" | "myhost" | "example"
    ) || ["example.com", "example.org", "example.net"]
        .iter()
        .any(|d| h == *d || h.ends_with(&format!(".{d}")))
}

/// A six-pair match that is not a hardware address: part of a longer run of
/// hexadecimal pairs (a TLS or SSH fingerprint), the all-zero or broadcast
/// address, or a documentation block of RFC 7042 (`00-00-5E-00-53-xx`
/// unicast, `01-00-5E-90-10-xx` multicast).
fn hw_addr_allowed(line: &str, start: usize, end: usize) -> bool {
    // A neighbouring pair is two hexadecimal digits that are a whole token:
    // `mac:` or `:cafe` next to an address is a word, not a pair.
    let pair = |x: u8, y: u8, outer: Option<&u8>| {
        x.is_ascii_hexdigit()
            && y.is_ascii_hexdigit()
            && !outer.is_some_and(u8::is_ascii_alphanumeric)
    };
    let before = &line.as_bytes()[..start];
    let after = &line.as_bytes()[end..];
    let longer = matches!(before, [.., x, y, b':' | b'-'] if pair(*x, *y, before.iter().rev().nth(3)))
        || matches!(after, [b':' | b'-', x, y, ..] if pair(*x, *y, after.get(3)));
    let text = line[start..end].to_ascii_lowercase();
    let p: Vec<&str> = text.split([':', '-']).collect();
    longer
        || p.iter().all(|x| *x == "00")
        || p.iter().all(|x| *x == "ff")
        || p[..5] == ["00", "00", "5e", "00", "53"]
        || p[..5] == ["01", "00", "5e", "90", "10"]
}

/// IPv6 link-local and unique-local addresses (the two private ranges). A
/// token that does not parse is tried again without a leading word and colon
/// when that word is not hexadecimal (an `addr:` or `ip6:` prefix).
fn v6_matches(line: &str, out: &mut Vec<(RuleId, String)>) {
    for m in V6_TOKEN.find_iter(line) {
        let token = m.as_str().split('%').next().unwrap_or("");
        let token = token.trim_matches('.');
        let token = match token.strip_suffix(':') {
            Some(t) if !t.ends_with(':') => t,
            _ => token,
        };
        // (An empty word is all hexadecimal, so it is never stripped.)
        let glued = token
            .split_once(':')
            .filter(|(word, _)| !word.bytes().all(|b| b.is_ascii_hexdigit()));
        let Some((token, addr)) = token
            .parse::<Ipv6Addr>()
            .ok()
            .map(|a| (token, a))
            .or_else(|| glued.and_then(|(_, rest)| Some((rest, rest.parse::<Ipv6Addr>().ok()?))))
        else {
            continue;
        };
        let first = addr.segments()[0];
        if first & 0xffc0 == 0xfe80 || first & 0xfe00 == 0xfc00 {
            out.push((RuleId::PrivateAddress, token.to_string()));
        }
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
        if parts.len() != 4 {
            continue;
        }
        let nums: Vec<u8> = parts.iter().filter_map(|p| p.parse().ok()).collect();
        if let Ok(v4) = <[u8; 4]>::try_from(nums)
            && private_v4(&v4)
        {
            out.push((RuleId::PrivateAddress, run.as_str().to_string()));
        }
    }
    v6_matches(line, &mut out);
    for m in HW_ADDR.find_iter(line) {
        if !hw_addr_allowed(line, m.start(), m.end()) {
            out.push((RuleId::PrivateAddress, m.as_str().to_string()));
        }
    }
    for m in PRIVATE_HOST.find_iter(line) {
        out.push((RuleId::PrivateHost, m.as_str().to_string()));
    }
    local_names(line, &mut out);
    for re in [&*PROMPT, &*PROMPT_ZSH, &*PROMPT_BASH] {
        for c in re.captures_iter(line) {
            if !prompt_host_allowed(&c[1]) {
                out.push((RuleId::PrivateHost, c[0].to_string()));
            }
        }
    }
    for m in SESSION_LINK.find_iter(line) {
        out.push((RuleId::SessionLink, m.as_str().to_string()));
    }
    out
}
