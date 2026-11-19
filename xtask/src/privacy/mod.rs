//! The privacy gate: scans a repository for personal information.
//!
//! Exit codes: 0 clean, 1 findings, 2 usage or environment error.

mod git;
mod rules;
mod scan;

pub use rules::RuleId;
pub use scan::{Finding, Terms, scan_bytes, scan_path_name, scan_text};
use scan::{decode_term_list, scan_views};

use globset::{GlobBuilder, GlobMatcher};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::Path;

const LOCAL_LIST: &str = "privacy-terms.local.txt";
const ALLOW_FILE: &str = "privacy-allow.toml";

#[derive(Debug, Clone)]
pub enum Mode {
    Staged,
    All,
    Range(String),
    Stdin(String),
}

#[derive(Debug, Clone)]
pub struct Options {
    pub mode: Mode,
    pub require_local: bool,
    pub json: bool,
    /// For `--stdin`: treat the text as a raw git commit message file.
    pub git_message: bool,
}

/// A finding, with the commit it came from in `--range` mode.
struct Reported {
    commit: Option<String>,
    finding: Finding,
}

#[derive(Deserialize)]
struct AllowFile {
    #[serde(default)]
    allow: Vec<AllowEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowEntry {
    path: String,
    rule: String,
    #[serde(default)]
    values: Vec<String>,
    reason: String,
}

struct AllowItem {
    glob: GlobMatcher,
    rule: RuleId,
    values: Vec<String>,
}

struct Allow(Vec<AllowItem>);

impl Allow {
    fn load(root: &Path) -> Result<Allow, String> {
        let file = root.join(ALLOW_FILE);
        let text = match fs::read_to_string(&file) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Allow(Vec::new())),
            Err(e) => return Err(format!("cannot read {ALLOW_FILE}: {e}")),
        };
        let parsed: AllowFile =
            toml::from_str(&text).map_err(|e| format!("invalid {ALLOW_FILE}: {e}"))?;
        let mut entries = Vec::new();
        for e in parsed.allow {
            let rule = RuleId::from_name(&e.rule)
                .ok_or_else(|| format!("{ALLOW_FILE}: unknown rule `{}`", e.rule))?;
            if matches!(rule, RuleId::LocalTerm | RuleId::LocalFileTracked) {
                return Err(format!("{ALLOW_FILE}: `{}` can never be exempted", e.rule));
            }
            if e.reason.trim().is_empty() {
                return Err(format!(
                    "{ALLOW_FILE}: the entry for `{}` has no reason",
                    e.path
                ));
            }
            if e.values.is_empty() || e.values.iter().any(|v| v.is_empty()) {
                return Err(format!(
                    "{ALLOW_FILE}: the entry for `{}` needs a non-empty `values` list",
                    e.path
                ));
            }
            let glob = GlobBuilder::new(&e.path)
                .literal_separator(true)
                .build()
                .map_err(|err| format!("{ALLOW_FILE}: bad glob `{}`: {err}", e.path))?;
            entries.push(AllowItem {
                glob: glob.compile_matcher(),
                rule,
                values: e.values,
            });
        }
        Ok(Allow(entries))
    }

    /// A finding is exempt when its path, rule and matched text all match an entry.
    fn exempts(&self, f: &Finding) -> bool {
        let path = f.path.replace('\\', "/");
        self.0.iter().any(|a| {
            a.rule == f.rule
                && a.glob.is_match(&path)
                && a.values.iter().any(|v| value_matches(v, f))
        })
    }

    /// A finding inside the allow file whose text is one of its declared values.
    /// Only the generic rules, and only for the rule of the declaring entry.
    fn is_own_value(&self, f: &Finding) -> bool {
        f.path == ALLOW_FILE
            && !matches!(f.rule, RuleId::LocalTerm | RuleId::LocalFileTracked)
            && self.0.iter().any(|a| {
                a.rule == f.rule
                    && a.values
                        .iter()
                        .any(|v| value_matches(v, f) || value_matches(&toml_escaped(v), f))
            })
    }
}

/// How a value reads in the allow file's own text (backslashes doubled).
fn toml_escaped(v: &str) -> String {
    v.replace('\\', "\\\\")
}

/// An allow value exempts a finding only when it is the whole matched text,
/// never a shortened form of a longer one.
fn value_matches(value: &str, f: &Finding) -> bool {
    value == f.value
}

/// One private term list that was found, with the place it came from.
struct LocalList {
    /// "this checkout" or "the main checkout"; never a path, which would
    /// carry a home folder.
    place: &'static str,
    text: String,
}

/// Finds and reads every private term list that applies to `root`. Lookup
/// order:
///
/// 1. `<root>/privacy-terms.local.txt`, where `<root>` is the top of the
///    checkout being scanned (`git rev-parse --show-toplevel`): a linked
///    worktree, or the main checkout itself.
/// 2. When `<root>` is a linked worktree, `<main>/privacy-terms.local.txt`
///    in the main checkout, found through `git rev-parse --git-common-dir`.
///    Skipped when it is the same file as the first.
///
/// Every list found is read and the caller combines their terms, so a
/// missing or empty list in one place never hides the other. A list that
/// cannot be read (a dangling symbolic link included) or decoded is an error
/// naming its place, never its content.
fn load_local(root: &Path) -> Result<Vec<LocalList>, String> {
    let here = root.join(LOCAL_LIST);
    let mut places = vec![("this checkout", here.clone())];
    if let Some(main) = git::main_worktree_root(root) {
        let p = main.join(LOCAL_LIST);
        let real = |q: &Path| fs::canonicalize(q).unwrap_or_else(|_| q.to_path_buf());
        if real(&p) != real(&here) {
            places.push(("the main checkout", p));
        }
    }
    let mut lists = Vec::new();
    for (place, p) in places {
        match fs::read(&p) {
            Ok(b) => lists.push(LocalList {
                place,
                text: decode_term_list(&b).map_err(|e| format!("{LOCAL_LIST} ({place}): {e}"))?,
            }),
            // Missing is fine only when nothing is there at all: a list that
            // exists and cannot be read, a dangling symbolic link included,
            // is an error.
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound
                    && fs::symlink_metadata(&p).is_err() => {}
            Err(e) => return Err(format!("cannot read {LOCAL_LIST} ({place}): {e}")),
        }
    }
    Ok(lists)
}

/// For a git commit message file: drops the verbose diff git appends after
/// its scissors line (`git commit -v`) and nothing else. Comment-looking lines
/// and a scissors line without a diff after it are kept and scanned.
pub fn clean_git_message(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let Some((at, token)) = lines.iter().enumerate().find_map(|(i, l)| {
        let (tok, rest) = l.split_once(' ')?;
        let ok = (1..=8).contains(&tok.chars().count())
            && !tok.chars().any(char::is_whitespace)
            && rest.strip_prefix(&"-".repeat(24))?.strip_prefix(" >8 ")? == "-".repeat(24);
        ok.then(|| (i, tok.to_string()))
    }) else {
        return text.to_string();
    };
    let next = lines[at + 1..]
        .iter()
        .find(|l| !l.is_empty() && !l.starts_with(&token));
    if next.is_some_and(|l| l.starts_with("diff --git ")) {
        lines[..at].join("\n")
    } else {
        text.to_string()
    }
}

/// Runs the gate and returns the process exit code.
pub fn run(opts: &Options) -> i32 {
    match run_inner(opts) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("privacy: error: {msg}");
            2
        }
    }
}

fn run_inner(opts: &Options) -> Result<i32, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("no working directory: {e}"))?;
    let root = match git::repo_root(&cwd) {
        Ok(r) => r,
        // Reading a message from stdin needs no repository.
        Err(_) if matches!(opts.mode, Mode::Stdin(_)) => cwd.clone(),
        Err(e) => return Err(e),
    };

    let allow = Allow::load(&root)?;
    let lists = load_local(&root)?;
    let mut terms = Terms::generic();
    let mut empty: Vec<&str> = Vec::new();
    for l in &lists {
        if !Terms::generic().with_local(&l.text).has_local() {
            empty.push(l.place);
        }
        terms = terms.with_local(&l.text);
    }
    if opts.require_local {
        if lists.is_empty() {
            return Err(format!(
                "--require-local: no local term list found ({LOCAL_LIST} is missing)"
            ));
        }
        if let Some(place) = empty.first() {
            return Err(format!(
                "--require-local: {LOCAL_LIST} ({place}) has no terms \
                 (it is empty or holds only comments and blank lines)"
            ));
        }
    }
    for place in &empty {
        eprintln!(
            "privacy: warning: {LOCAL_LIST} ({place}) has no terms \
             (it is empty or holds only comments and blank lines)"
        );
    }
    if !terms.has_local() {
        if lists.is_empty() {
            eprintln!("privacy: local terms are not being checked (no {LOCAL_LIST} found)");
        } else {
            eprintln!("privacy: local terms are not being checked (no list found has any term)");
        }
    }

    let mut found: Vec<Reported> = Vec::new();
    // `exemptable` is true for file findings only; stdin text, commit messages
    // and identities are never covered by the allow file.
    let mut add = |commit: Option<&str>, exemptable: bool, fs: Vec<Finding>| {
        for finding in fs {
            if !(allow.is_own_value(&finding) || exemptable && allow.exempts(&finding)) {
                found.push(Reported {
                    commit: commit.map(str::to_string),
                    finding,
                });
            }
        }
    };

    // In range mode, how many commits the range held.
    let mut scanned: Option<usize> = None;
    match &opts.mode {
        Mode::Stdin(label) => {
            let mut buf = Vec::new();
            std::io::stdin()
                .read_to_end(&mut buf)
                .map_err(|e| format!("cannot read stdin: {e}"))?;
            if opts.git_message {
                add(
                    None,
                    false,
                    scan_views(label, &buf, &terms, &clean_git_message),
                );
            } else {
                add(None, false, scan_bytes(label, &buf, &terms));
            }
        }
        Mode::All => {
            local_list_check(&root, &mut add)?;
            for entry in git::list_all(&root)? {
                let path = entry.name;
                if path == LOCAL_LIST {
                    continue;
                }
                add(None, true, scan_path_name(&path, &terms));
                let full = root.join(&entry.file);
                let Ok(meta) = fs::symlink_metadata(&full) else {
                    continue; // tracked but deleted in the working tree
                };
                let bytes = if meta.file_type().is_symlink() {
                    fs::read_link(&full)
                        .map(|t| t.to_string_lossy().into_owned().into_bytes())
                        .unwrap_or_default()
                } else if meta.is_file() {
                    fs::read(&full).map_err(|e| format!("cannot read {path}: {e}"))?
                } else {
                    continue; // submodule or other non-file entry
                };
                add(None, true, scan_bytes(&path, &bytes, &terms));
            }
        }
        Mode::Staged => {
            local_list_check(&root, &mut add)?;
            for b in git::staged(&root)? {
                if b.path == LOCAL_LIST {
                    continue;
                }
                add(None, true, scan_path_name(&b.path, &terms));
                if !b.submodule {
                    add(
                        None,
                        true,
                        scan_bytes(&b.path, &git::read_blob(&root, &b.oid)?, &terms),
                    );
                }
            }
        }
        Mode::Range(range) => {
            // git would read a range that begins with `-` as an option
            // (`--stdin`, `--all`) and report an empty or a different range.
            if range.starts_with('-') {
                return Err("--range: a range cannot begin with `-`".to_string());
            }
            let mut seen: HashSet<(String, String)> = HashSet::new();
            // The repository's own commit name, read once (the maintainer's
            // second decision, see `identity_findings`); never printed.
            let own_name = git::configured_user_name(&root);
            let commits = git::commits(&root, range)?;
            scanned = Some(commits.len());
            for c in commits {
                let short = c.sha.chars().take(10).collect::<String>();
                let label = format!("commit {short} message");
                let msg = git::commit_message(&root, &c.sha)?;
                add(Some(&short), false, scan_bytes(&label, &msg, &terms));
                // The whole identity line (name and address together) is
                // checked for addresses and local terms, except a name that
                // is the account of its own no-reply address (the
                // maintainer's decision, see `identity_findings`).
                for id in git::commit_identities(&root, &c.sha)? {
                    let label = format!("commit {short} {}", id.who);
                    add(
                        Some(&short),
                        false,
                        identity_findings(&label, &id.line, &terms, own_name.as_deref()),
                    );
                }
                for b in git::commit_blobs(&root, &c)? {
                    if b.path == LOCAL_LIST {
                        add(
                            Some(&short),
                            false,
                            vec![Finding {
                                path: b.path.clone(),
                                line: 0,
                                rule: RuleId::LocalFileTracked,
                                excerpt: "a commit adds or changes the local term list".into(),
                                value: String::new(),
                            }],
                        );
                        continue;
                    }
                    if !seen.insert((b.oid.clone(), b.path.clone())) {
                        continue;
                    }
                    add(Some(&short), true, scan_path_name(&b.path, &terms));
                    if !b.submodule {
                        add(
                            Some(&short),
                            true,
                            scan_bytes(&b.path, &git::read_blob(&root, &b.oid)?, &terms),
                        );
                    }
                }
            }
        }
    }

    report(&found, opts.json, scanned);
    Ok(if found.is_empty() { 0 } else { 1 })
}

/// The address and local-term findings in one identity line: its lossy
/// UTF-8 reading and, when its bytes are not valid UTF-8, its Latin-1
/// reading too (a name written by a tool that did not use UTF-8).
///
/// The maintainer's decision of 2026-11-18, as a contract: the name is not
/// checked against the local terms when, and only when, the line is valid
/// UTF-8, its address (between the first `<` and the next `>`) is one the
/// gate already treats as the hosting service's no-reply address
/// (`rules::noreply_account`, in either form), and the name (the text before
/// the `<`, trimmed) equals that address's account name, compared ASCII
/// case-insensitively. That account name is already public in the address.
/// Then neither the name nor that address gives a local-term finding; the
/// rest of the line is still checked for terms, and the whole line for
/// addresses. Anything else (another word in the name, a look-alike host, an
/// address in the name, the other line of the same commit) is checked as
/// before.
///
/// The maintainer's second decision of 2026-11-18 (the maintainer keeps their
/// own commit name), as a contract: the same exemption also holds when the
/// name, trimmed, equals `own_name` exactly (case-sensitively): the commit
/// name configured for the repository being scanned, as
/// `git config --get user.name` resolves it there, read once per run and
/// never printed. With no configured name, or when git fails, this part does
/// not apply. Residual, stated plainly: whoever can change the repository's
/// configured commit name can make that name exempt, so this protects
/// against other people's and mistaken identities, not against a deliberate
/// change of the local configuration.
fn identity_findings(
    label: &str,
    bytes: &[u8],
    terms: &Terms,
    own_name: Option<&str>,
) -> Vec<Finding> {
    let mut readings = vec![String::from_utf8_lossy(bytes).into_owned()];
    if std::str::from_utf8(bytes).is_err() {
        readings.push(bytes.iter().map(|b| char::from(*b)).collect());
    }
    let rest = std::str::from_utf8(bytes)
        .ok()
        .and_then(|line| after_own_noreply_name(line, own_name));
    let no_terms = Terms::generic();
    let line_terms = if rest.is_some() { &no_terms } else { terms };
    let mut found: Vec<Finding> = readings
        .iter()
        .flat_map(|r| scan_text(label, r, line_terms))
        .collect();
    if let Some(rest) = rest {
        found.extend(scan_text(label, rest, terms));
    }
    let mut out: Vec<Finding> = Vec::new();
    for f in found {
        if matches!(f.rule, RuleId::Email | RuleId::LocalTerm) && !out.contains(&f) {
            out.push(f);
        }
    }
    out
}

/// For an identity line with a no-reply address whose name is that
/// address's account or the repository's own commit name (the maintainer's
/// decisions, see `identity_findings`): the text after that address. `None`
/// for every other line.
fn after_own_noreply_name<'a>(line: &'a str, own_name: Option<&str>) -> Option<&'a str> {
    let (name, tail) = line.split_once('<')?;
    let (address, rest) = tail.split_once('>')?;
    let account = rules::noreply_account(address)?;
    let name = name.trim();
    (name.eq_ignore_ascii_case(account) || own_name == Some(name)).then_some(rest)
}

fn local_list_check(
    root: &Path,
    add: &mut impl FnMut(Option<&str>, bool, Vec<Finding>),
) -> Result<(), String> {
    if let Some(why) = git::local_list_problem(root, LOCAL_LIST)? {
        add(
            None,
            false,
            vec![Finding {
                path: LOCAL_LIST.to_string(),
                line: 0,
                rule: RuleId::LocalFileTracked,
                excerpt: why.to_string(),
                value: String::new(),
            }],
        );
    }
    Ok(())
}

/// In CI a local term is never echoed: only where it was found.
fn shown_excerpt(f: &Finding) -> String {
    if f.rule == RuleId::LocalTerm && std::env::var_os("CI").is_some() {
        String::new()
    } else {
        f.excerpt.clone()
    }
}

fn report(found: &[Reported], json: bool, scanned: Option<usize>) {
    if json {
        let items: Vec<serde_json::Value> = found
            .iter()
            .map(|r| {
                let f = &r.finding;
                let mut o = serde_json::json!({
                    "path": f.path,
                    "line": f.line,
                    "rule": f.rule.name(),
                    "excerpt": shown_excerpt(f),
                });
                if let Some(c) = &r.commit {
                    o["commit"] = serde_json::Value::String(c.clone());
                }
                o
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".into())
        );
    } else {
        for r in found {
            let f = &r.finding;
            let at = if f.line == 0 {
                f.path.clone()
            } else {
                format!("{}:{}", f.path, f.line)
            };
            let who = r
                .commit
                .as_deref()
                .map(|c| format!("{c} "))
                .unwrap_or_default();
            let ex = shown_excerpt(f);
            if ex.is_empty() {
                println!("{who}{at}: {}", f.rule.name());
            } else {
                println!("{who}{at}: {}: {ex}", f.rule.name());
            }
        }
    }
    if found.is_empty() {
        // An empty range is not "clean" without saying so: nothing was scanned.
        match scanned {
            Some(1) => eprintln!("privacy: clean (1 commit scanned)"),
            Some(n) => eprintln!("privacy: clean ({n} commits scanned)"),
            None => eprintln!("privacy: clean"),
        }
    } else {
        eprintln!("privacy: {} finding(s)", found.len());
    }
}
