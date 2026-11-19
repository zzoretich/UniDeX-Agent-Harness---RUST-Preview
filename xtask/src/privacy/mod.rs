//! The privacy gate: scans a repository for personal information and
//! secrets.
//!
//! Exit codes: 0 clean, 1 findings, 2 usage or environment error.
//!
//! Every mode (`--staged`, `--all`, `--range`, `--stdin` and a push) runs the
//! generic rules, the local terms and the secret rules (`secrets.rs`) on each
//! text it reads (`scan_content`, `scan_name`, `identity_scan`). The public
//! `scan_text`, `scan_bytes` and `scan_path_name` run the generic rules and
//! the local terms only. Nothing the gate prints holds a secret's body: a
//! secret finding shows its prefix, other findings on its line are hidden,
//! and paths and error messages pass through `secrets::redact`.

pub(crate) mod git;
mod rules;
mod scan;
mod secrets;

pub use rules::RuleId;
pub use scan::{Finding, Terms, scan_bytes, scan_path_name, scan_text};
use scan::{decode_term_list, scan_views};
pub use secrets::{SecretKind, redact};

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
    /// The refs of a push (the pre-push hook): every commit and annotated tag
    /// they would publish, and their remote names.
    Push(Vec<PushRange>),
}

/// One ref of a push. The commits scanned are those reachable from `tip` and
/// from none of `exclude` (commits the remote already has); when `tip` is an
/// annotated tag, every tag object of its chain (message and tagger) is
/// scanned too. `name`, the remote ref name the push creates or moves, is
/// scanned like a file name and never given to git. `tip` and `exclude` are
/// object ids (`hooks::push_ranges` checks them); they reach git only after
/// `--end-of-options`, so a value that is not one is a git error, never an
/// option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushRange {
    pub name: String,
    pub tip: String,
    pub exclude: Vec<String>,
}

/// A full object id: 40 (SHA-1) or 64 (SHA-256) hexadecimal characters.
pub fn is_object_id(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Runs git in `dir` with `input` on its standard input and returns its
/// standard output, or an error holding its standard error.
pub(crate) fn git_input(dir: &Path, args: &[&str], input: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run git: {e}"))?;
    // Written from another thread, so a large input and a large output
    // cannot block each other.
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let input = input.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let out = child
        .wait_with_output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    let written = writer
        .join()
        .map_err(|_| "cannot write to git".to_string())?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    written.map_err(|e| format!("cannot write to git: {e}"))?;
    Ok(out.stdout)
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
    // With their place in the file, so an error can name the line without
    // quoting the text (which could hold a secret).
    path: toml::Spanned<String>,
    rule: toml::Spanned<String>,
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
        let parsed: AllowFile = toml::from_str(&text).map_err(|e| parse_error(&text, e.span()))?;
        let mut entries = Vec::new();
        for e in parsed.allow {
            // Each error names the line, never the entry's text.
            let at = |field: &toml::Spanned<String>, what: &str| {
                let (line, _) = position(&text, field.span().start);
                format!("{ALLOW_FILE} line {line}: the entry {what} (the text is not shown)")
            };
            let rule = RuleId::from_name(e.rule.get_ref())
                .ok_or_else(|| at(&e.rule, "names an unknown rule"))?;
            if matches!(
                rule,
                RuleId::LocalTerm | RuleId::LocalFileTracked | RuleId::Secret(_)
            ) {
                // A known rule's name, so safe to show.
                return Err(format!(
                    "{ALLOW_FILE}: `{}` can never be exempted",
                    rule.name()
                ));
            }
            if e.reason.trim().is_empty() {
                return Err(at(&e.path, "has no reason"));
            }
            if e.values.is_empty() || e.values.iter().any(|v| v.is_empty()) {
                return Err(at(&e.path, "needs a non-empty `values` list"));
            }
            let glob = GlobBuilder::new(e.path.get_ref())
                .literal_separator(true)
                .build()
                .map_err(|_| at(&e.path, "has a path that is not a valid glob"))?;
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

/// An allow-file parse error: where it is, never the text there (toml's own
/// message quotes the line, and a schema error quotes the value, either of
/// which could hold a secret).
fn parse_error(text: &str, span: Option<std::ops::Range<usize>>) -> String {
    match span {
        Some(span) => {
            let (line, column) = position(text, span.start);
            format!(
                "invalid {ALLOW_FILE}: a TOML or schema error at line {line}, column {column} \
                 (the text there is not shown)"
            )
        }
        None => format!("invalid {ALLOW_FILE}: a TOML or schema error (the text is not shown)"),
    }
}

/// The line and column (both from 1) of a byte offset in `text`.
fn position(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..text.floor_char_boundary(offset)];
    let line = before.matches('\n').count() + 1;
    let column = before.chars().rev().take_while(|c| *c != '\n').count() + 1;
    (line, column)
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
            // A message can quote a path or an argument; never a secret.
            eprintln!("privacy: error: {}", secrets::redact(&msg));
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
            eprintln!(
                "privacy: warning: local terms are not being checked (no {LOCAL_LIST} found)"
            );
        } else {
            eprintln!(
                "privacy: warning: local terms are not being checked (no list found has any term)"
            );
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
            let prep: &dyn Fn(&str) -> String = if opts.git_message {
                &clean_git_message
            } else {
                &str::to_string
            };
            add(None, false, scan_content(label, &buf, &terms, prep));
        }
        Mode::All => {
            local_list_check(&root, &mut add)?;
            for entry in git::list_all(&root)? {
                let path = entry.name;
                if path == LOCAL_LIST {
                    continue;
                }
                add(None, true, scan_name(&path, &terms));
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
                add(
                    None,
                    true,
                    scan_content(&path, &bytes, &terms, &str::to_string),
                );
            }
        }
        Mode::Staged => {
            local_list_check(&root, &mut add)?;
            for b in git::staged(&root)? {
                if b.path == LOCAL_LIST {
                    continue;
                }
                add(None, true, scan_name(&b.path, &terms));
                if !b.submodule {
                    let blob = git::read_blob(&root, &b.oid)?;
                    add(
                        None,
                        true,
                        scan_content(&b.path, &blob, &terms, &str::to_string),
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
            // The repository's own commit name, read once (the maintainer's
            // second decision, see `identity_findings`); never printed.
            let own_name = git::configured_user_name(&root);
            let commits = git::commits(&root, range)?;
            scanned = Some(commits.len());
            scan_commits(&root, commits, &terms, own_name.as_deref(), &mut add)?;
        }
        Mode::Push(refs) => {
            let own_name = git::configured_user_name(&root);
            let mut commits = Vec::new();
            let mut listed: HashSet<String> = HashSet::new();
            for r in refs {
                add(None, false, scan_name(&r.name, &terms));
                // Only commits are scanned, so a ref that names a blob or a
                // tree (directly or through tags) is refused, not passed.
                let kind = scan_tags(&root, &r.tip, &terms, own_name.as_deref(), &mut add)?;
                if kind != "commit" {
                    return Err(format!(
                        "push: {} names a {kind}, not a commit; the gate scans commits, \
                         so such a push is refused",
                        r.name
                    ));
                }
                for c in pushed_commits(&root, r)? {
                    if listed.insert(c.sha.clone()) {
                        commits.push(c);
                    }
                }
            }
            scanned = Some(commits.len());
            scan_commits(&root, commits, &terms, own_name.as_deref(), &mut add)?;
        }
    }

    report(&found, opts.json, scanned);
    Ok(if found.is_empty() { 0 } else { 1 })
}

/// Every finding in one text: the generic rules and the local terms on each
/// reading of it (`scan_views`), and the secret rules on each reading, with
/// the other findings' excerpts hidden on the lines a secret covers.
fn scan_content(
    label: &str,
    bytes: &[u8],
    terms: &Terms,
    prep: &dyn Fn(&str) -> String,
) -> Vec<Finding> {
    secrets::scan_bytes(label, bytes, prep).merge(scan_views(label, bytes, terms, prep))
}

/// Every finding in a path name (line 0), the secret rules included.
fn scan_name(path: &str, terms: &Terms) -> Vec<Finding> {
    secrets::scan_name(path).merge(scan_path_name(path, terms))
}

/// The findings in one identity line (`identity_findings`) and its secrets.
fn identity_scan(label: &str, bytes: &[u8], terms: &Terms, own_name: Option<&str>) -> Vec<Finding> {
    secrets::scan_bytes(label, bytes, &str::to_string)
        .merge(identity_findings(label, bytes, terms, own_name))
}

/// Scans each commit: its message, its author and committer lines, and the
/// name and content of every file it adds or changes (each file version
/// once). A commit that adds or changes the local term list is a finding.
fn scan_commits(
    root: &Path,
    commits: Vec<git::CommitInfo>,
    terms: &Terms,
    own_name: Option<&str>,
    add: &mut impl FnMut(Option<&str>, bool, Vec<Finding>),
) -> Result<(), String> {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for c in commits {
        let short = c.sha.chars().take(10).collect::<String>();
        let label = format!("commit {short} message");
        let msg = git::commit_message(root, &c.sha)?;
        let mut text_found = scan_content(&label, &msg, terms, &str::to_string);
        // The whole identity line (name and address together) is checked
        // for addresses and local terms, except a name that is the account
        // of its own no-reply address (the maintainer's decision, see
        // `identity_findings`).
        for id in git::commit_identities(root, &c.sha)? {
            let label = format!("commit {short} {}", id.who);
            text_found.extend(identity_scan(&label, &id.line, terms, own_name));
        }
        // The whole raw commit object too: extra headers and message text
        // after a NUL byte are published but are in neither of the above.
        let raw = git::git(root, &["cat-file", "commit", &c.sha])?;
        let object = format!("commit {short} object");
        add(
            Some(&short),
            false,
            with_raw_secrets(text_found, &object, &raw),
        );
        for b in git::commit_blobs(root, &c)? {
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
            add(Some(&short), true, scan_name(&b.path, terms));
            if !b.submodule {
                let blob = git::read_blob(root, &b.oid)?;
                add(
                    Some(&short),
                    true,
                    scan_content(&b.path, &blob, terms, &str::to_string),
                );
            }
        }
    }
    Ok(())
}

/// `found` (the findings in an object's message and identity lines) and the
/// secret findings in the object's whole raw text, `raw`, under `label`; a
/// secret already in `found` is not repeated (the same text has the same
/// digest).
fn with_raw_secrets(mut found: Vec<Finding>, label: &str, raw: &[u8]) -> Vec<Finding> {
    let reported: HashSet<String> = found
        .iter()
        .filter(|f| matches!(f.rule, RuleId::Secret(_)))
        .map(|f| f.value.clone())
        .collect();
    let mut raw_found = secrets::scan_bytes(label, raw, &str::to_string).merge(Vec::new());
    raw_found.retain(|f| !reported.contains(&f.value));
    found.extend(raw_found);
    found
}

/// The commits a push of `r` would publish, oldest first: reachable from
/// its tip and from none of its exclusions, which go to git on standard input
/// (any number of them, never on the command line).
fn pushed_commits(root: &Path, r: &PushRange) -> Result<Vec<git::CommitInfo>, String> {
    // On standard input, which `rev-list --stdin` would also read pseudo
    // options from, so every line is checked to be an object id first.
    if let Some(bad) = std::iter::once(&r.tip)
        .chain(&r.exclude)
        .find(|id| !is_object_id(id))
    {
        return Err(format!("push: not an object id: {bad:?}"));
    }
    let mut input = format!("{}\n", r.tip);
    for e in &r.exclude {
        input.push_str(&format!("^{e}\n"));
    }
    let out = git_input(
        root,
        &["rev-list", "--parents", "--reverse", "--stdin"],
        input.as_bytes(),
    )?;
    Ok(String::from_utf8_lossy(&out)
        .lines()
        .filter_map(|l| {
            let mut it = l.split(' ');
            let sha = it.next()?.to_string();
            Some(git::CommitInfo {
                sha,
                parents: it.map(str::to_string).collect(),
            })
        })
        .collect())
}

/// When `oid` is an annotated tag: scans the message and the tagger line of
/// it and of every tag it leads to (a tag can name another tag), and each
/// whole raw tag object with the secret rules, as text that the allow file
/// never covers. A tag's chain ends at a non-tag object,
/// whose type it returns (`commit`, `tree` or `blob`); a tag that names no
/// object id is an error.
fn scan_tags(
    root: &Path,
    oid: &str,
    terms: &Terms,
    own_name: Option<&str>,
    add: &mut impl FnMut(Option<&str>, bool, Vec<Finding>),
) -> Result<String, String> {
    let mut oid = oid.to_string();
    loop {
        let kind = git::git(root, &["cat-file", "-t", "--end-of-options", &oid])?;
        if kind != b"tag\n" {
            return Ok(String::from_utf8_lossy(&kind).trim_end().to_string());
        }
        let raw = git::git(root, &["cat-file", "tag", "--end-of-options", &oid])?;
        let short: String = oid.chars().take(10).collect();
        // The header ends at the first empty line; the message follows.
        let at = raw
            .windows(2)
            .position(|w| w == b"\n\n")
            .unwrap_or(raw.len());
        let (head, message) = (&raw[..at], raw.get(at + 2..).unwrap_or_default());
        let mut next = None;
        let mut found = Vec::new();
        for line in head.split(|b| *b == b'\n') {
            if let Some(rest) = line.strip_prefix(b"tagger ") {
                let label = format!("tag {short} tagger");
                found.extend(identity_scan(&label, rest, terms, own_name));
            } else if let Some(rest) = line.strip_prefix(b"object ") {
                next = Some(String::from_utf8_lossy(rest).into_owned());
            }
        }
        let label = format!("tag {short} message");
        found.extend(scan_content(&label, message, terms, &str::to_string));
        // The whole raw tag object too: its own name and any extra header.
        let object = format!("tag {short} object");
        add(Some(&short), false, with_raw_secrets(found, &object, &raw));
        oid = next
            .filter(|n| is_object_id(n))
            .ok_or_else(|| format!("tag {short} names no object id"))?;
    }
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
                    "path": secrets::redact(&f.path),
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
            let path = secrets::redact(&f.path);
            let at = if f.line == 0 {
                path
            } else {
                format!("{path}:{}", f.line)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushed_commits_takes_object_ids_only() {
        // Checked before git runs: `rev-list --stdin` would read `--all` as
        // a pseudo option.
        let ok = "a".repeat(40);
        let cases = [
            ("--all".to_string(), Vec::new()),
            (ok.clone(), vec!["--all".to_string()]),
            (ok.clone(), vec![ok.clone(), "HEAD".to_string()]),
        ];
        for (tip, exclude) in cases {
            let r = PushRange {
                name: "refs/heads/x".into(),
                tip,
                exclude,
            };
            let e = pushed_commits(Path::new("."), &r).err().unwrap_or_default();
            assert!(e.starts_with("push: not an object id"), "{e}");
        }
    }
}
