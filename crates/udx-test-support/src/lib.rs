//! Helpers shared by the tests of the UniDeX crates.
//!
//! This crate depends on no workspace crate (A 2.2), so a crate can use it in
//! its own tests without a dependency cycle. Helpers that need a workspace
//! type live in that crate's own `testing` module.

mod stand_in;

use std::path::{Path, PathBuf};

pub use stand_in::{HttpStandIn, RecordedRequest, StandInResponse, StandInRoute, TestCert};

/// The variable cargo sets for every test process (`.cargo/config.toml`).
const TEST_ROOT: &str = "UNIDEX_TEST_ROOT";

/// A fresh folder inside the process's test root, removed on drop.
pub struct TestRoot {
    dir: tempfile::TempDir,
}

impl TestRoot {
    /// Panics if `UNIDEX_TEST_ROOT` is missing: run the tests through cargo.
    pub fn new() -> TestRoot {
        let root = std::env::var(TEST_ROOT)
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                panic!("{TEST_ROOT} is not set: run the tests through cargo (cargo test)")
            });
        // Checked before anything is created, so a relative value never makes a folder in the working folder.
        assert!(
            Path::new(&root).is_absolute(),
            "{TEST_ROOT} must be an absolute path"
        );
        std::fs::create_dir_all(&root).expect("create the test root");
        let dir = tempfile::Builder::new()
            .prefix("udx-")
            .tempdir_in(&root)
            .expect("create a folder inside the test root");
        TestRoot { dir }
    }

    /// The fresh folder. It exists on disk.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// A path below the folder. Creates nothing.
    pub fn child(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
}

impl Default for TestRoot {
    fn default() -> Self {
        TestRoot::new()
    }
}

/// A fake home folder under a [`TestRoot`], with files a sandbox must hide.
/// All contents are invented, low-entropy and match no secret rule.
pub struct TempHome {
    home: PathBuf,
}

impl TempHome {
    /// Make `<root>/home` and plant the files: a shell history, a private key
    /// file, a token file and a shell rc file.
    pub fn new(root: &TestRoot) -> TempHome {
        let home = TempHome {
            home: root.child("home"),
        };
        std::fs::create_dir_all(&home.home).expect("create the fake home");
        home.plant(".bash_history", b"echo invented-history-0001\n");
        home.plant(".ssh/id_ed25519", b"invented-key-file-0001\n");
        home.plant(".config/tool/token", b"invented-token-0001\n");
        home.plant(".zshrc", b"# invented rc file\n");
        home
    }

    /// Write `contents` to `rel` below the home (parents are created, an
    /// existing file is replaced) and return the path. Panics if `rel` is
    /// absolute or leaves the home.
    pub fn plant(&self, rel: &str, contents: &[u8]) -> PathBuf {
        let rel_path = Path::new(rel);
        assert!(
            !rel_path.is_absolute()
                && rel_path
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
            "plant needs a plain relative path, got {rel:?}"
        );
        let path = self.home.join(rel_path);
        std::fs::create_dir_all(path.parent().expect("a file has a parent"))
            .expect("create the parents");
        std::fs::write(&path, contents).expect("write the planted file");
        path
    }

    /// The fake home.
    pub fn path(&self) -> &Path {
        &self.home
    }
}

/// Every file below `root` that holds `canary` or its base64 (both alphabets),
/// percent or hex encoding. Symlinks are not followed; a folder that cannot be
/// read yields nothing. Sorted.
pub fn scan_for_canary(root: &Path, canary: &str) -> Vec<PathBuf> {
    if canary.is_empty() {
        return Vec::new();
    }
    let needles = encodings(canary);
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file()
                && std::fs::read(&path).is_ok_and(|bytes| holds_any(&bytes, &needles))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn holds_any(hay: &[u8], needles: &[Vec<u8>]) -> bool {
    needles
        .iter()
        .any(|n| hay.windows(n.len()).any(|w| w == n.as_slice()))
}

/// The plain value and each encoding a leak might use.
fn encodings(canary: &str) -> Vec<Vec<u8>> {
    let raw = canary.as_bytes();
    let mut all = vec![
        canary.to_string(),
        percent(raw, false),
        percent(raw, true),
        hex(raw, false),
        hex(raw, true),
    ];
    for url_safe in [false, true] {
        all.extend(base64_needles(raw, url_safe));
    }
    all.sort();
    all.dedup();
    all.into_iter().map(String::into_bytes).collect()
}

/// The characters of the canary's base64 form that do not depend on its neighbours, for each of the three alignments
/// (0, 1 or 2 bytes of other data before it). The characters that mix in a neighbouring byte are dropped: the leading
/// ones that hold bits of the byte before, and the last one when the canary does not end on a three-byte boundary.
/// A needle shorter than `MIN_NEEDLE` would match unrelated text, so it is skipped.
fn base64_needles(canary: &[u8], url_safe: bool) -> Vec<String> {
    const MIN_NEEDLE: usize = 6;
    let mut out = Vec::new();
    for before in 0..3usize {
        let mut data = vec![0u8; before];
        data.extend_from_slice(canary);
        let encoded = base64(&data, url_safe);
        let lead = [0, 2, 3][before];
        let trail = usize::from(data.len() % 3 != 0);
        let chars: Vec<char> = encoded.chars().collect();
        if chars.len() < lead + trail {
            continue;
        }
        let needle: String = chars[lead..chars.len() - trail].iter().collect();
        if needle.len() >= MIN_NEEDLE {
            out.push(needle);
        }
    }
    out
}

/// Base64 without padding, so a value matches whether or not the writer padded it.
fn base64(input: &[u8], url_safe: bool) -> String {
    let alphabet: &[u8; 64] = if url_safe {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
    } else {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    };
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |a, (i, b)| a | (u32::from(*b) << (16 - 8 * i)));
        for k in 0..=chunk.len() {
            out.push(alphabet[((n >> (18 - 6 * k)) & 63) as usize] as char);
        }
    }
    out
}

/// Every byte but the unreserved ones escaped, in upper or lower case hex.
fn percent(input: &[u8], lower: bool) -> String {
    let mut out = String::new();
    for &b in input {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else if lower {
            out.push_str(&format!("%{b:02x}"));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn hex(input: &[u8], upper: bool) -> String {
    input
        .iter()
        .map(|b| {
            if upper {
                format!("{b:02X}")
            } else {
                format!("{b:02x}")
            }
        })
        .collect()
}
