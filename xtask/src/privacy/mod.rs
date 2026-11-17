//! The privacy gate: scans a repository for personal information.
//!
//! Exit codes: 0 clean, 1 findings, 2 usage or environment error.

mod git;
mod rules;
mod scan;

pub use rules::RuleId;
pub use scan::{Finding, Terms, scan_bytes, scan_path_name, scan_text};

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

fn value_matches(value: &str, f: &Finding) -> bool {
    value == f.excerpt || scan::shorten(value) == f.excerpt
}

/// Loads the local term list from this worktree, else from the main worktree.
fn load_local(root: &Path) -> Result<Option<String>, String> {
    let mut places = vec![root.join(LOCAL_LIST)];
    if let Some(main) = git::main_worktree_root(root) {
        let p = main.join(LOCAL_LIST);
        if !places.contains(&p) {
            places.push(p);
        }
    }
    for p in places {
        match fs::read(&p) {
            Ok(b) => return Ok(Some(String::from_utf8_lossy(&b).into_owned())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("cannot read the local term list: {e}")),
        }
    }
    Ok(None)
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
    let terms = match load_local(&root)? {
        Some(text) => Terms::generic().with_local(&text),
        None => Terms::generic(),
    };
    if opts.require_local && !terms.has_local() {
        return Err(format!(
            "--require-local: no local term list found ({LOCAL_LIST} is missing or empty)"
        ));
    }
    if !terms.has_local() {
        eprintln!("privacy: local terms are not being checked (no {LOCAL_LIST} found)");
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

    match &opts.mode {
        Mode::Stdin(label) => {
            let mut buf = Vec::new();
            std::io::stdin()
                .read_to_end(&mut buf)
                .map_err(|e| format!("cannot read stdin: {e}"))?;
            if opts.git_message {
                let text = clean_git_message(&String::from_utf8_lossy(&buf));
                add(None, false, scan_text(label, &text, &terms));
            } else {
                add(None, false, scan_bytes(label, &buf, &terms));
            }
        }
        Mode::All => {
            local_list_check(&root, &mut add)?;
            for path in git::list_all(&root)? {
                if path == LOCAL_LIST {
                    continue;
                }
                add(None, true, scan_path_name(&path, &terms));
                let full = root.join(&path);
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
                add(
                    None,
                    true,
                    scan_bytes(&b.path, &git::read_blob(&root, &b.oid)?, &terms),
                );
            }
        }
        Mode::Range(range) => {
            let mut seen: HashSet<(String, String)> = HashSet::new();
            for c in git::commits(&root, range)? {
                let short = c.sha.chars().take(10).collect::<String>();
                let label = format!("commit {short} message");
                let msg = git::commit_message(&root, &c.sha)?;
                add(Some(&short), false, scan_bytes(&label, &msg, &terms));
                let (author, committer) = git::commit_identities(&root, &c.sha)?;
                for (who, email) in [("author", author), ("committer", committer)] {
                    let label = format!("commit {short} {who}");
                    let found = scan_text(&label, &email, &terms)
                        .into_iter()
                        .filter(|f| matches!(f.rule, RuleId::Email | RuleId::LocalTerm));
                    add(Some(&short), false, found.collect());
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
                            }],
                        );
                        continue;
                    }
                    if !seen.insert((b.oid.clone(), b.path.clone())) {
                        continue;
                    }
                    add(Some(&short), true, scan_path_name(&b.path, &terms));
                    add(
                        Some(&short),
                        true,
                        scan_bytes(&b.path, &git::read_blob(&root, &b.oid)?, &terms),
                    );
                }
            }
        }
    }

    report(&found, opts.json);
    Ok(if found.is_empty() { 0 } else { 1 })
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

fn report(found: &[Reported], json: bool) {
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
        eprintln!("privacy: clean");
    } else {
        eprintln!("privacy: {} finding(s)", found.len());
    }
}
