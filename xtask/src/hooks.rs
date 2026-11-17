//! `xtask hooks install`: writes the git hooks that run the privacy gate.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MARKER: &str = "# xtask-privacy-hook";

const PRELUDE: &str = r#"if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi
top=$(git rev-parse --show-toplevel) || exit 1
cd "$top" || exit 1
if [ ! -f xtask/Cargo.toml ]; then
    echo "privacy gate: no xtask/Cargo.toml in $top, so the gate cannot run; refusing to continue" >&2
    exit 1
fi
"#;

fn script(last_line: &str) -> String {
    format!("#!/bin/sh\n{MARKER} (written by `cargo xtask hooks install`)\n{PRELUDE}{last_line}\n")
}

fn hooks_dir(cwd: &Path) -> Result<PathBuf, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--git-path", "hooks"])
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "not inside a git repository: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim_end_matches(['\n', '\r']));
    Ok(if p.is_absolute() { p } else { cwd.join(p) })
}

/// Installs `pre-commit` and `commit-msg`. Refuses to replace a hook it did not write.
pub fn install() -> Result<Vec<PathBuf>, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("no working directory: {e}"))?;
    let dir = hooks_dir(&cwd)?;
    let hooks = [
        ("pre-commit", script("exec cargo xtask privacy --staged")),
        (
            "commit-msg",
            script("exec cargo xtask privacy --stdin commit-message < \"$1\""),
        ),
    ];
    for (name, _) in &hooks {
        if let Ok(existing) = fs::read_to_string(dir.join(name))
            && !existing.contains(MARKER)
        {
            return Err(format!(
                "{} already exists and was not written by xtask; move it aside and run again",
                dir.join(name).display()
            ));
        }
    }
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let mut written = Vec::new();
    for (name, body) in hooks {
        let path = dir.join(name);
        fs::write(&path, body).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("cannot make {} executable: {e}", path.display()))?;
        }
        written.push(path);
    }
    Ok(written)
}
