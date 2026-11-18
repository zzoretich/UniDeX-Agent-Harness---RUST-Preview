//! Tests of the helpers in `udx-test-support` (A 3.14). They do not compile until `TestRoot`, `TempHome` and
//! `scan_for_canary` exist (T1b's builder), and they run inside the process's test root.

use std::collections::BTreeSet;
use std::path::PathBuf;

use udx_test_support::{TempHome, TestRoot, scan_for_canary};

#[test]
fn test_root_is_inside_the_process_root_and_removed_on_drop() {
    let path;
    {
        let root = TestRoot::new();
        path = root.path().to_path_buf();
        assert!(path.is_dir());
        assert_eq!(root.child("a/b"), path.join("a/b"));
        let process_root =
            std::fs::canonicalize(std::env::var("UNIDEX_TEST_ROOT").unwrap()).unwrap();
        assert!(
            std::fs::canonicalize(&path)
                .unwrap()
                .starts_with(process_root)
        );
    }
    assert!(!path.exists());
}

#[test]
fn temp_home_plants_files() {
    let root = TestRoot::new();
    let home = TempHome::new(&root);
    assert!(home.path().starts_with(root.path()));
    // The files the brief names are planted by default.
    for rel in [".bash_history", ".ssh/id_ed25519", ".zshrc"] {
        assert!(home.path().join(rel).is_file(), "{rel} is planted");
    }
    // A token file is planted too: at least one more file than the three named exists.
    let mut count = 0;
    let mut stack = vec![home.path().to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                count += 1;
            }
        }
    }
    assert!(count >= 4, "only {count} files planted");
    // plant() creates parents, returns the path and writes the bytes.
    let p = home.plant("deep/er/note.txt", b"invented-0001");
    assert_eq!(p, home.path().join("deep/er/note.txt"));
    assert_eq!(std::fs::read(&p).unwrap(), b"invented-0001");
    // Planting again replaces the contents.
    let again = home.plant("deep/er/note.txt", b"invented-0002");
    assert_eq!(again, p);
    assert_eq!(std::fs::read(&p).unwrap(), b"invented-0002");
}

const B64_STD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(input: &[u8], url_safe: bool) -> String {
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |a, (i, b)| a | (u32::from(*b) << (16 - 8 * i)));
        for k in 0..=chunk.len() {
            let c = B64_STD[((n >> (18 - 6 * k)) & 63) as usize] as char;
            out.push(match (c, url_safe) {
                ('+', true) => '-',
                ('/', true) => '_',
                _ => c,
            });
        }
        while !url_safe && !out.len().is_multiple_of(4) {
            out.push('=');
        }
    }
    out
}

fn percent(input: &str) -> String {
    input
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn hex(input: &str) -> String {
    input.bytes().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn scan_for_canary_finds_each_encoding() {
    // Nine bytes, a multiple of three, so the standard form has no padding; the bytes `?`, `>` and `~` make the two
    // base64 alphabets differ and need percent escapes. Invented, low entropy, and no rule's shape.
    let canary = "c4n??>>~~";
    let std_b64 = base64(canary.as_bytes(), false);
    let url_b64 = base64(canary.as_bytes(), true);
    assert_ne!(std_b64, url_b64, "the canary must tell the alphabets apart");
    assert!(std_b64.contains('+') || std_b64.contains('/'));

    let root = TestRoot::new();
    let write = |rel: &str, body: String| -> PathBuf {
        let p = root.child(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, format!("before {body} after")).unwrap();
        p
    };
    let plain = write("a/plain.txt", canary.to_string());
    let b64 = write("b/deeper/std.txt", std_b64);
    let b64u = write("b/url.txt", url_b64);
    let pct = write("c/percent.txt", percent(canary));
    let hx = write("d/hex.txt", hex(canary));
    let clean = write("e/clean.txt", "nothing to see here".to_string());

    let found: BTreeSet<PathBuf> = scan_for_canary(root.path(), canary).into_iter().collect();
    let want: BTreeSet<PathBuf> = [plain, b64, b64u, pct, hx].into_iter().collect();
    assert_eq!(found, want, "clean file {clean:?} must not be reported");

    // A different canary finds none of them.
    assert!(scan_for_canary(root.path(), "other-0000").is_empty());
    // A missing folder yields nothing and does not panic.
    assert!(scan_for_canary(&root.child("no-such-folder"), canary).is_empty());
}

/// Base64 of a whole payload, padded or not.
fn base64_payload(input: &[u8], url_safe: bool, pad: bool) -> String {
    let mut out = base64(input, url_safe);
    if !pad {
        out.retain(|c| c != '=');
    }
    out
}

#[test]
fn scan_for_canary_finds_base64_inside_larger_payloads() {
    // Two canaries, one whose length is a multiple of three and one that is not, so every alignment and every kind of
    // ending is met. Both are invented and low entropy.
    for canary in [
        "c4n??>>~~",
        "c4n??>>~~x",
        "c4n??>>~~xy",
        "invented-canary-0000?>",
    ] {
        for before in 0..3usize {
            for after in [0usize, 1, 2, 5] {
                for url_safe in [false, true] {
                    for pad in [false, true] {
                        let mut payload = b"user".to_vec();
                        payload.truncate(before);
                        payload.extend_from_slice(canary.as_bytes());
                        payload.extend(std::iter::repeat_n(b'~', after));
                        let blob = base64_payload(&payload, url_safe, pad);
                        let root = TestRoot::new();
                        let p = root.child("blob.txt");
                        std::fs::write(&p, format!("Authorization: Basic {blob}\n")).unwrap();
                        assert_eq!(
                            scan_for_canary(root.path(), canary),
                            vec![p],
                            "canary {canary:?}, {before} bytes before, {after} after, url_safe {url_safe}, pad {pad}: {blob}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn scan_for_canary_finds_hex_and_percent_inside_larger_data() {
    let canary = "c4n??>>~~";
    let lower_hex = hex(canary);
    let upper_hex = lower_hex.to_uppercase();
    let upper_pct = percent(canary);
    let lower_pct = upper_pct.to_lowercase();
    let root = TestRoot::new();
    let mut want = BTreeSet::new();
    for (name, body) in [
        ("hex_lower", format!("00ff{lower_hex}aa11")),
        ("hex_upper", format!("00FF{upper_hex}AA11")),
        ("pct_upper", format!("q=pre{upper_pct}&x=1")),
        ("pct_lower", format!("q=pre{lower_pct}&x=1")),
    ] {
        let p = root.child(name);
        std::fs::write(&p, body).unwrap();
        want.insert(p);
    }
    let found: BTreeSet<PathBuf> = scan_for_canary(root.path(), canary).into_iter().collect();
    assert_eq!(found, want);
}

#[test]
fn scan_for_canary_accepts_the_shortest_canary_and_finds_it_in_base64() {
    // Five bytes is the shortest canary whose base64 form has a needle at every alignment.
    let canary = "k3y-9";
    let root = TestRoot::new();
    for (before, name) in [(0usize, "a"), (1, "b"), (2, "c")] {
        let mut payload = b"xy"[..before].to_vec();
        payload.extend_from_slice(canary.as_bytes());
        payload.extend_from_slice(b"zz");
        std::fs::write(root.child(name), base64_payload(&payload, false, true)).unwrap();
    }
    assert_eq!(scan_for_canary(root.path(), canary).len(), 3);
}

#[test]
#[should_panic(expected = "canary")]
fn scan_for_canary_refuses_a_canary_too_short_to_search_for() {
    let root = TestRoot::new();
    scan_for_canary(root.path(), "k3y-");
}

#[test]
#[should_panic(expected = "canary")]
fn scan_for_canary_refuses_an_empty_canary() {
    let root = TestRoot::new();
    scan_for_canary(root.path(), "");
}
