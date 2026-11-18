//! The choke-point rule (A 3.1.1, "The choke-point rule"): every route to the operating system's user folders goes
//! through `udx_core::paths`. The rule is a small token scanner kept in this file, tested on the `.txt` fixtures under
//! `tests/fixtures/choke_point/{allowed,refused}/` (which the tree scan never reads, since it reads only `.rs` files)
//! and then run over every `.rs` file under `crates/`.
//!
//! `xtask/` lies outside `crates/` and is not scanned: it is the build tool, never shipped, and T1a moves its own
//! tests onto the test root.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------- lexer

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Id(String),
    Str(String),
    P(String),
}

impl Tok {
    fn is_p(&self, s: &str) -> bool {
        matches!(self, Tok::P(p) if p == s)
    }
    fn is_id(&self, s: &str) -> bool {
        matches!(self, Tok::Id(p) if p == s)
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Tokens of Rust source with comments removed. Strings keep their text; lifetimes and character literals vanish.
fn lex(src: &str) -> Vec<Tok> {
    let b: Vec<char> = src.chars().collect();
    let n = b.len();
    let at = |i: usize| b.get(i).copied().unwrap_or('\0');
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '/' && at(i + 1) == '/' {
            while i < n && b[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && at(i + 1) == '*' {
            let mut depth = 1;
            i += 2;
            while i < n && depth > 0 {
                if b[i] == '/' && at(i + 1) == '*' {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && at(i + 1) == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        } else if c == '"' {
            let (s, ni) = plain_string(&b, i + 1);
            out.push(Tok::Str(s));
            i = ni;
        } else if c == '\'' {
            if at(i + 1) == '\\' {
                i += 3;
                while i < n && b[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if at(i + 2) == '\'' {
                i += 3;
            } else {
                i += 1;
                while i < n && is_ident_char(b[i]) {
                    i += 1;
                }
            }
        } else if is_ident_char(c) {
            let st = i;
            while i < n && is_ident_char(b[i]) {
                i += 1;
            }
            let id: String = b[st..i].iter().collect();
            let raw_prefix = matches!(id.as_str(), "r" | "br" | "cr");
            let plain_prefix = matches!(id.as_str(), "b" | "c");
            if (raw_prefix && (at(i) == '"' || at(i) == '#')) || (plain_prefix && at(i) == '"') {
                if raw_prefix {
                    let mut h = 0;
                    while at(i) == '#' {
                        h += 1;
                        i += 1;
                    }
                    i += 1; // the opening quote
                    let st = i;
                    while i < n {
                        if b[i] == '"' && (0..h).all(|k| at(i + 1 + k) == '#') {
                            break;
                        }
                        i += 1;
                    }
                    out.push(Tok::Str(b[st..i.min(n)].iter().collect()));
                    i += 1 + h;
                } else {
                    let (s, ni) = plain_string(&b, i + 1);
                    out.push(Tok::Str(s));
                    i = ni;
                }
            } else if id == "b" && at(i) == '\'' {
                i += 1;
                i += if at(i) == '\\' { 2 } else { 1 };
                while i < n && b[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else {
                out.push(Tok::Id(id));
            }
        } else if c == ':' && at(i + 1) == ':' {
            out.push(Tok::P("::".to_string()));
            i += 2;
        } else {
            out.push(Tok::P(c.to_string()));
            i += 1;
        }
    }
    out
}

fn plain_string(b: &[char], mut i: usize) -> (String, usize) {
    let mut s = String::new();
    while i < b.len() {
        match b[i] {
            '\\' => {
                if let Some(&next) = b.get(i + 1) {
                    s.push(next);
                }
                i += 2;
            }
            '"' => return (s, i + 1),
            c => {
                s.push(c);
                i += 1;
            }
        }
    }
    (s, i)
}

// ---------------------------------------------------------------- use trees

type UsePath = (Vec<String>, Option<String>);

fn use_tree(t: &[Tok], i: &mut usize, prefix: Vec<String>, out: &mut Vec<UsePath>) {
    let mut path = prefix;
    if t.get(*i).is_some_and(|x| x.is_p("::")) {
        *i += 1;
    }
    loop {
        match t.get(*i) {
            Some(Tok::Id(s)) if s != "as" => {
                path.push(s.clone());
                *i += 1;
            }
            Some(x) if x.is_p("{") => {
                *i += 1;
                loop {
                    use_tree(t, i, path.clone(), out);
                    if t.get(*i).is_some_and(|x| x.is_p(",")) {
                        *i += 1;
                        if t.get(*i).is_some_and(|x| x.is_p("}")) {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                if t.get(*i).is_some_and(|x| x.is_p("}")) {
                    *i += 1;
                }
                return;
            }
            Some(x) if x.is_p("*") => {
                *i += 1;
                path.push("*".to_string());
                out.push((path, None));
                return;
            }
            _ => return,
        }
        if t.get(*i).is_some_and(|x| x.is_p("::")) {
            *i += 1;
        } else {
            break;
        }
    }
    let mut rename = None;
    if t.get(*i).is_some_and(|x| x.is_id("as")) {
        *i += 1;
        if let Some(Tok::Id(name)) = t.get(*i) {
            rename = Some(name.clone());
            *i += 1;
        }
    }
    if path.last().is_some_and(|l| l == "self") {
        path.pop();
    }
    out.push((path, rename));
}

// ---------------------------------------------------------------- the rule

const OS_LOOKUPS: [&str; 8] = [
    "getpwuid",
    "getpwuid_r",
    "getpwnam",
    "getpwnam_r",
    "SHGetKnownFolderPath",
    "SHGetFolderPathW",
    "GetUserProfileDirectoryW",
    "NSHomeDirectory",
];
const USER_CRATES: [&str; 3] = ["dirs", "directories", "home"];
const USER_VARS: [&str; 10] = [
    "HOME",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "TMPDIR",
    "TEMP",
    "TMP",
    "USER",
    "LOGNAME",
    "SHELL",
];

fn is_home_or_temp(s: &str) -> bool {
    s == "home_dir" || s == "temp_dir"
}

fn is_user_literal(s: &str) -> bool {
    USER_VARS.contains(&s) || s.starts_with("XDG_")
}

/// Every route to the OS's user folders in `src` (comments removed), one message each.
fn violations(src: &str) -> Vec<String> {
    let t = lex(src);
    let mut v = Vec::new();
    for (i, tok) in t.iter().enumerate() {
        let prev = |k: usize| i.checked_sub(k).and_then(|j| t.get(j));
        let next = |k: usize| t.get(i + k);
        let Tok::Id(id) = tok else { continue };

        // 1. `env::home_dir`, `env::temp_dir`, however the path is spelled.
        if id == "env"
            && next(1).is_some_and(|x| x.is_p("::"))
            && matches!(next(2), Some(Tok::Id(f)) if is_home_or_temp(f))
        {
            v.push(format!("std lookup: env::{}", ident_at(&t, i + 2)));
        }
        // 2. A path whose first segment is one of the user-folder crates.
        if USER_CRATES.contains(&id.as_str()) && next(1).is_some_and(|x| x.is_p("::")) {
            let first = match prev(1) {
                None => true,
                Some(p) if p.is_p("::") => !matches!(prev(2), Some(Tok::Id(_))),
                Some(p) if p.is_p(".") => false,
                Some(_) => true,
            };
            if first {
                v.push(format!("user-folder crate: {id}::"));
            }
        }
        // 2. The operating systems' own home lookups, by name.
        if OS_LOOKUPS.contains(&id.as_str()) {
            v.push(format!("OS lookup: {id}"));
        }
        // 3. A read of a user variable by its literal name.
        let is_call = (id == "var" || id == "var_os") && next(1).is_some_and(|x| x.is_p("("));
        let is_macro = (id == "env" || id == "option_env")
            && next(1).is_some_and(|x| x.is_p("!"))
            && next(2).is_some_and(|x| x.is_p("("));
        let arg = if is_call {
            next(2)
        } else if is_macro {
            next(3)
        } else {
            None
        };
        if let Some(Tok::Str(s)) = arg
            && is_user_literal(s)
        {
            v.push(format!("variable read: {id}({s:?})"));
        }
    }
    // 1. Use items: any flattened path into `std::env` that reaches the lookups, or renames `std::env`.
    let mut i = 0;
    while i < t.len() {
        if t[i].is_id("use") {
            let mut j = i + 1;
            let mut paths = Vec::new();
            use_tree(&t, &mut j, Vec::new(), &mut paths);
            for (p, rename) in paths {
                let std_env = p.len() >= 2 && p[0] == "std" && p[1] == "env";
                if std_env && p.len() == 2 && rename.is_some() {
                    v.push("use renames std::env".to_string());
                }
                if p.last().is_some_and(|l| is_home_or_temp(l))
                    && (std_env || p.windows(2).any(|w| w[0] == "env"))
                {
                    v.push(format!("use imports {}", p.join("::")));
                }
                if p.first().is_some_and(|f| USER_CRATES.contains(&f.as_str())) {
                    v.push(format!("use of user-folder crate {}", p[0]));
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    v
}

fn ident_at(t: &[Tok], i: usize) -> String {
    match t.get(i) {
        Some(Tok::Id(s)) => s.clone(),
        _ => String::new(),
    }
}

/// Source with every item under `#[cfg(test)]` removed; a file that begins `#![cfg(test)]` is entirely test code.
fn without_cfg_test(t: Vec<Tok>) -> Vec<Tok> {
    let is_cfg_test = |i: usize| {
        let want = ["#", "[", "cfg", "(", "test", ")", "]"];
        want.iter().enumerate().all(|(k, w)| match t.get(i + k) {
            Some(Tok::P(p)) => p == w,
            Some(Tok::Id(s)) => s == w,
            _ => false,
        })
    };
    let inner_cfg_test = t.len() > 8
        && t[0].is_p("#")
        && t[1].is_p("!")
        && t[2].is_p("[")
        && t[3].is_id("cfg")
        && t[5].is_id("test");
    if inner_cfg_test {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < t.len() {
        if is_cfg_test(i) {
            i += 7;
            // further attributes on the same item
            while t.get(i).is_some_and(|x| x.is_p("#")) && t.get(i + 1).is_some_and(|x| x.is_p("["))
            {
                let mut depth = 0;
                i += 1;
                while i < t.len() {
                    if t[i].is_p("[") {
                        depth += 1;
                    } else if t[i].is_p("]") {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    i += 1;
                }
            }
            // the item: up to a `;` or through its braces
            let mut paren = 0;
            while i < t.len() {
                if t[i].is_p("(") || t[i].is_p("[") {
                    paren += 1;
                } else if t[i].is_p(")") || t[i].is_p("]") {
                    paren -= 1;
                } else if paren == 0 && t[i].is_p(";") {
                    i += 1;
                    break;
                } else if paren == 0 && t[i].is_p("{") {
                    let mut depth = 0;
                    while i < t.len() {
                        if t[i].is_p("{") {
                            depth += 1;
                        } else if t[i].is_p("}") {
                            depth -= 1;
                            if depth == 0 {
                                i += 1;
                                break;
                            }
                        }
                        i += 1;
                    }
                    break;
                }
                i += 1;
            }
        } else {
            out.push(t[i].clone());
            i += 1;
        }
    }
    out
}

// ---------------------------------------------------------------- tree helpers

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/udx-core has a workspace root two levels up")
        .to_path_buf()
}

fn rs_files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let ft = e.file_type().unwrap();
        if ft.is_dir() {
            if e.file_name() != "target" {
                rs_files_under(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

/// (file suffix, message part) pairs for sanctioned exceptions to the choke-point rule. Empty at merge; it grows only
/// with the controller's agreement.
const CHOKE_POINT_ALLOW: &[(&str, &str)] = &[];

/// (file suffix) entries for `remove_dir_all` outside test code: temp-folder cleanups only (P3; A D14). Empty at merge.
const REMOVE_DIR_ALL_ALLOW: &[&str] = &[];

fn fixture_dir(kind: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/choke_point")
        .join(kind)
}

fn fixtures(kind: &str) -> Vec<(String, String)> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(fixture_dir(kind)).expect("the fixture folder exists") {
        let p = e.unwrap().path();
        assert_eq!(p.extension().and_then(|x| x.to_str()), Some("txt"), "{p:?}");
        v.push((
            p.file_name().unwrap().to_string_lossy().into_owned(),
            std::fs::read_to_string(&p).unwrap(),
        ));
    }
    v.sort();
    v
}

// ---------------------------------------------------------------- tests

#[test]
fn choke_point_rule_accepts_sanctioned_accessor() {
    let allowed = fixtures("allowed");
    let names: BTreeSet<&str> = allowed.iter().map(|(n, _)| n.as_str()).collect();
    for must in [
        "udx_core_paths_home_dir.txt",
        "crate_paths_home_dir.txt",
        "use_then_bare_home_dir.txt",
        "udx_core_paths_user_var.txt",
        "crate_paths_user_var.txt",
        "use_then_bare_user_var.txt",
        "method_var_reads_test_root.txt",
        "home_as_map_key.txt",
    ] {
        assert!(names.contains(must), "missing allowed fixture {must}");
    }
    for (name, src) in &allowed {
        let found = violations(src);
        assert!(found.is_empty(), "allowed/{name} was refused: {found:?}");
    }
}

#[test]
fn choke_point_rule_refuses_direct_lookups() {
    let refused = fixtures("refused");
    let names: BTreeSet<&str> = refused.iter().map(|(n, _)| n.as_str()).collect();
    for must in [
        "std_env_home_dir.txt",
        "env_home_dir_after_use_std_env.txt",
        "use_tree_home_dir.txt",
        "use_tree_temp_dir.txt",
        "use_tree_split_over_lines.txt",
        "use_tree_renamed_home_dir.txt",
        "use_std_env_renamed.txt",
        "std_env_temp_dir.txt",
        "dirs_path.txt",
        "directories_path.txt",
        "home_crate_path.txt",
        "os_lookup_getpwuid.txt",
        "os_lookup_SHGetKnownFolderPath.txt",
        "std_env_var_home.txt",
        "env_var_userprofile_method.txt",
        "std_env_var_os_xdg_config_home.txt",
        "env_macro_home.txt",
    ] {
        assert!(names.contains(must), "missing refused fixture {must}");
    }
    for (name, src) in &refused {
        assert!(
            !violations(src).is_empty(),
            "refused/{name} was accepted by the rule"
        );
    }
}

#[test]
fn choke_point_no_direct_user_folder_access() {
    // The scanner's own helper must be sound before the tree is judged by it.
    assert!(!violations("let h = std::env::home_dir();").is_empty());
    assert!(violations("let h = home_dir(env);").is_empty());

    let root = workspace_root();
    let mut files = Vec::new();
    rs_files_under(&root.join("crates"), &mut files);
    assert!(files.len() >= 3, "found only {files:?}");
    let mut bad = Vec::new();
    for f in files {
        let r = rel(&root, &f);
        if r == "crates/udx-core/src/paths.rs" || r.starts_with("crates/udx-cookbook/") {
            continue;
        }
        let src = std::fs::read_to_string(&f).unwrap();
        for msg in violations(&src) {
            let allowed = CHOKE_POINT_ALLOW
                .iter()
                .any(|(file, pat)| r.ends_with(file) && msg.contains(pat));
            if !allowed {
                bad.push(format!("{r}: {msg}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "routes to the OS's user folders outside udx_core::paths:\n{}",
        bad.join("\n")
    );
}

#[test]
fn choke_point_no_remove_dir_all() {
    // The scanner must see a use outside test code and ignore one inside.
    let probe = "fn a() { std::fs::remove_dir_all(p); }\n#[cfg(test)]\nmod tests { fn b() { std::fs::remove_dir_all(p); } }";
    let seen = without_cfg_test(lex(probe))
        .iter()
        .filter(|t| t.is_id("remove_dir_all"))
        .count();
    assert_eq!(seen, 1);

    let root = workspace_root();
    let mut files = Vec::new();
    rs_files_under(&root.join("crates"), &mut files);
    let mut bad = Vec::new();
    for f in files {
        let r = rel(&root, &f);
        let in_test_folder = r.split('/').any(|c| c == "tests") || r.ends_with("/tests.rs");
        if in_test_folder || r.starts_with("crates/udx-cookbook/") {
            continue;
        }
        if REMOVE_DIR_ALL_ALLOW.iter().any(|a| r.ends_with(a)) {
            continue;
        }
        let toks = without_cfg_test(lex(&std::fs::read_to_string(&f).unwrap()));
        if toks.iter().any(|t| t.is_id("remove_dir_all")) {
            bad.push(r);
        }
    }
    assert!(
        bad.is_empty(),
        "remove_dir_all outside test code (P3; A D14): {bad:?}"
    );
}

#[test]
fn choke_point_no_direct_chrono() {
    // A dependency on chrono is named at the start of a line, in a dependency table header, or as a renamed package.
    fn names_chrono(manifest: &str) -> bool {
        manifest.lines().any(|l| {
            let l = l.trim();
            if l.starts_with('#') {
                return false;
            }
            let key_is_chrono = l.strip_prefix("chrono").is_some_and(|rest| {
                !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '-')
            });
            let table_is_chrono =
                l.starts_with('[') && l.trim_end_matches(']').ends_with(".chrono");
            key_is_chrono || table_is_chrono || l.replace(' ', "").contains("package=\"chrono\"")
        })
    }
    assert!(names_chrono("[dependencies]\nchrono = \"0.4\"\n"));
    assert!(names_chrono("[dependencies.chrono]\nversion = \"0.4\"\n"));
    assert!(names_chrono(
        "x = { package = \"chrono\", version = \"0.4\" }\n"
    ));
    assert!(!names_chrono(
        "# chrono is forbidden\n[dependencies]\nchrono-tz-like = \"1\"\n"
    ));

    let root = workspace_root();
    let mut manifests = vec![root.join("Cargo.toml"), root.join("xtask/Cargo.toml")];
    for e in std::fs::read_dir(root.join("crates")).unwrap() {
        let m = e.unwrap().path().join("Cargo.toml");
        if m.is_file() {
            manifests.push(m);
        }
    }
    let mut bad = Vec::new();
    for m in manifests {
        if let Ok(text) = std::fs::read_to_string(&m)
            && names_chrono(&text)
        {
            bad.push(rel(&root, &m));
        }
    }
    assert!(
        bad.is_empty(),
        "chrono is not allowed (decision 0007): {bad:?}"
    );
}
