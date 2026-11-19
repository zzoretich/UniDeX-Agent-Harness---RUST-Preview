//! Task 1d: the pre-push hook and the hook installer's refusals.
//!
//! Assumptions (see the task report): `cargo xtask hooks install` also writes a
//! `pre-push` hook; the hook reads git's pre-push standard input (one line per
//! ref: local ref, local id, remote ref, remote id) and its arguments (remote
//! name, remote URL), and runs `cargo xtask privacy --range <from>..<local id>`.
//! For a new remote branch the range starts at the merge base of the pushed
//! commit with the remote's existing branches (`refs/remotes/<remote name>/*`).
//! Hook tests run on Unix; a stand-in `cargo` first on PATH runs the binary
//! under test, so nothing outside the temporary repositories is used.
//!
//! Every key-shaped value comes from `common::vectors()`, built at run time.

mod common;

#[cfg(unix)]
mod unix {
    use super::common::*;
    use std::fs;
    use std::path::Path;

    /// A repository with the stand-in manifest, one clean commit pushed to a
    /// bare remote named `origin` (before any hook exists), and the hooks installed
    /// through the tool's own installer.
    struct Fixture {
        repo: tempfile::TempDir,
        remote: tempfile::TempDir,
        _bin: tempfile::TempDir,
        path: std::ffi::OsString,
    }

    fn fixture(before_install: impl FnOnce(&Path)) -> Fixture {
        let repo = new_repo();
        let remote = new_bare();
        add_stand_in_manifest(repo.path());
        write(repo.path(), "a.md", "clean\n");
        commit_all(repo.path(), "first");
        before_install(repo.path());
        git(
            repo.path(),
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        );
        git(repo.path(), &["push", "-q", "origin", "main"]);
        xtask(repo.path())
            .args(["hooks", "install"])
            .assert()
            .code(0);
        let hook = repo.path().join(".git/hooks/pre-push");
        assert!(hook.exists(), "`hooks install` must write a pre-push hook");
        let (bin, path) = stand_in_cargo();
        Fixture {
            repo,
            remote,
            _bin: bin,
            path,
        }
    }

    impl Fixture {
        fn push(&self, refspec: &str) -> (bool, String) {
            let out = isolated(std::process::Command::new("git"))
                .args(["push", "origin", refspec])
                .current_dir(self.repo.path())
                .env("PATH", &self.path)
                .env_remove("CI")
                .output()
                .expect("git runs");
            (out.status.success(), output_text(&out))
        }

        fn remote_ref(&self, name: &str) -> Option<String> {
            let out = isolated(std::process::Command::new("git"))
                .args(["rev-parse", "--verify", "-q", name])
                .current_dir(self.remote.path())
                .output()
                .unwrap();
            out.status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
    }

    fn secret_text() -> String {
        // The first vector is the anthropic-style key; any family would do.
        file_with(&vectors().remove(0).text)
    }

    #[test]
    fn pre_push_scans_pushed_range() {
        let fx = fixture(|_| {});
        let repo = fx.repo.path();

        // Control: a clean commit goes through, so the refusal below comes from
        // the scan and not from a hook that cannot run.
        write(repo, "b.md", "also clean\n");
        commit_all(repo, "clean");
        let (ok, text) = fx.push("main");
        assert!(ok, "a clean push must pass: {text}");

        // A commit made with the commit hooks bypassed is caught at push.
        write(repo, "secret.txt", &secret_text());
        commit_all(repo, "oops"); // commit_all passes --no-verify
        let bad = git_out(repo, &["rev-parse", "HEAD"]);
        let before = fx.remote_ref("refs/heads/main");
        let (ok, text) = fx.push("main");
        assert!(!ok, "the push must be refused: {text}");
        assert!(text.contains("Secret"), "{text}");
        assert!(text.contains("secret.txt"), "{text}");
        assert_eq!(
            fx.remote_ref("refs/heads/main"),
            before,
            "nothing was pushed"
        );
        assert_ne!(before.as_deref(), Some(bad.as_str()));
    }

    #[test]
    fn pre_push_new_branch_scans_from_merge_base() {
        // An old secret already sits in the remote's main (pushed before the
        // hooks existed).
        let fx = fixture(|repo| {
            write(repo, "old.txt", &secret_text());
            commit_all(repo, "an old mistake");
        });
        let repo = fx.repo.path();
        assert!(fx.remote_ref("refs/heads/main").is_some());

        // A new branch with one clean commit: only the commits since the merge
        // base are scanned, so the old secret does not block the push.
        git(repo, &["checkout", "-q", "-b", "clean-branch"]);
        write(repo, "c.md", "fine\n");
        commit_all(repo, "clean work");
        let (ok, text) = fx.push("clean-branch");
        assert!(ok, "history already on the remote is not rescanned: {text}");
        assert!(fx.remote_ref("refs/heads/clean-branch").is_some());

        // A new branch whose first commit adds a secret and whose tip removes
        // it: the range holds the commit that added it, so the push is refused.
        git(repo, &["checkout", "-q", "main"]);
        git(repo, &["checkout", "-q", "-b", "leaky-branch"]);
        write(repo, "new.txt", &secret_text());
        commit_all(repo, "add a secret");
        fs::remove_file(repo.join("new.txt")).unwrap();
        commit_all(repo, "remove it again");
        let (ok, text) = fx.push("leaky-branch");
        assert!(
            !ok,
            "a secret anywhere in the new commits must refuse: {text}"
        );
        assert!(text.contains("Secret"), "{text}");
        assert!(text.contains("new.txt"), "{text}");
        assert!(fx.remote_ref("refs/heads/leaky-branch").is_none());
    }

    fn run_install(repo: &Path, global_config: Option<&Path>) -> (i32, String) {
        let mut cmd = xtask(repo);
        if let Some(cfg) = global_config {
            cmd.env("GIT_CONFIG_GLOBAL", cfg);
        }
        let out = cmd.args(["hooks", "install"]).output().unwrap();
        (out.status.code().unwrap(), output_text(&out))
    }

    fn assert_nothing_installed(dir: &Path) {
        for name in ["pre-commit", "commit-msg", "pre-push"] {
            assert!(
                !dir.join(name).exists(),
                "{} must not be written",
                dir.join(name).display()
            );
        }
    }

    #[test]
    fn installer_refuses_global_hooks_path() {
        let outside = temp_dir();
        let shared_hooks = outside.path().join("shared-hooks");
        fs::create_dir_all(&shared_hooks).unwrap();

        // The usual case: the user's global configuration sets core.hooksPath.
        let repo = new_repo();
        let cfg = outside.path().join("global-gitconfig");
        fs::write(
            &cfg,
            format!("[core]\n\thooksPath = {}\n", shared_hooks.display()),
        )
        .unwrap();
        let (code, text) = run_install(repo.path(), Some(&cfg));
        assert_eq!(code, 1, "{text}");
        assert!(text.to_lowercase().contains("hookspath"), "{text}");
        assert_nothing_installed(&shared_hooks);
        assert_nothing_installed(&repo.path().join(".git/hooks"));

        // A repository-local setting that points outside the repository.
        let repo = new_repo();
        git(
            repo.path(),
            &["config", "core.hooksPath", shared_hooks.to_str().unwrap()],
        );
        let (code, text) = run_install(repo.path(), None);
        assert_eq!(code, 1, "{text}");
        assert!(text.to_lowercase().contains("hookspath"), "{text}");
        assert_nothing_installed(&shared_hooks);
        assert_nothing_installed(&repo.path().join(".git/hooks"));
    }

    #[test]
    fn installer_keeps_unreadable_hook() {
        use std::os::unix::fs::PermissionsExt;
        let repo = new_repo();
        let hooks = repo.path().join(".git/hooks");
        fs::create_dir_all(&hooks).unwrap();
        let hook = hooks.join("pre-commit");
        fs::write(&hook, "#!/bin/sh\necho mine\n").unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read(&hook).is_ok() {
            // Running as a user for whom the mode makes no difference (root).
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o644)).unwrap();
            return;
        }
        let (code, text) = run_install(repo.path(), None);
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(code, 1, "an unreadable hook stops the installer: {text}");
        assert!(text.contains("pre-commit"), "{text}");
        assert_eq!(
            fs::read_to_string(&hook).unwrap(),
            "#!/bin/sh\necho mine\n",
            "the hook is untouched"
        );
        assert!(!hooks.join("commit-msg").exists());
        assert!(!hooks.join("pre-push").exists());
    }

    #[test]
    fn installer_keeps_non_utf8_hook() {
        let repo = new_repo();
        let hooks = repo.path().join(".git/hooks");
        fs::create_dir_all(&hooks).unwrap();
        let hook = hooks.join("pre-commit");
        let mine: Vec<u8> = b"#!/bin/sh\n# caf\xe9 hook, not UTF-8\necho mine\n".to_vec();
        fs::write(&hook, &mine).unwrap();
        let (code, text) = run_install(repo.path(), None);
        assert_eq!(code, 1, "a non-UTF-8 hook stops the installer: {text}");
        assert!(text.contains("pre-commit"), "{text}");
        assert_eq!(fs::read(&hook).unwrap(), mine, "the hook is untouched");
        assert!(!hooks.join("commit-msg").exists());
        assert!(!hooks.join("pre-push").exists());
    }
}

// ---- builder's tests: the installer ----
//
// These run the installer only (no hook is executed), so they need no
// stand-in `cargo`; they are written to run on Windows too (first shown by
// CI).
mod installer {
    use super::common::*;
    use std::fs;
    use std::path::Path;

    fn install(repo: &Path, extra: &[&str]) -> (i32, String) {
        let out = xtask(repo)
            .args(["hooks", "install"])
            .args(extra)
            .output()
            .unwrap();
        (out.status.code().unwrap(), output_text(&out))
    }

    fn hook(repo: &Path, name: &str) -> String {
        fs::read_to_string(repo.join(".git/hooks").join(name)).unwrap()
    }

    #[test]
    fn installer_writes_three_hooks_and_passes_require_local_on_request() {
        let repo = new_repo();
        assert_eq!(install(repo.path(), &[]).0, 0);
        for name in ["pre-commit", "commit-msg", "pre-push"] {
            let body = hook(repo.path(), name);
            assert!(body.contains("# xtask-privacy-hook"), "{name}: {body}");
            assert!(!body.contains("--require-local"), "{name}: {body}");
        }
        assert!(
            hook(repo.path(), "pre-push")
                .contains("exec cargo xtask hooks pre-push -- \"$1\" \"$2\"")
        );
        // Reinstalling over its own hooks works, and the flag reaches all three.
        let (code, text) = install(repo.path(), &["--require-local"]);
        assert_eq!(code, 0, "{text}");
        assert!(
            hook(repo.path(), "pre-commit")
                .contains("exec cargo xtask privacy --staged --require-local\n")
        );
        assert!(
            hook(repo.path(), "commit-msg").contains(
                "exec cargo xtask privacy --stdin commit-message --git-message --require-local < \"$1\"\n"
            )
        );
        assert!(
            hook(repo.path(), "pre-push")
                .contains("exec cargo xtask hooks pre-push --require-local -- \"$1\" \"$2\"\n")
        );
        // No temporary file is left behind.
        let left: Vec<String> = fs::read_dir(repo.path().join(".git/hooks"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("xtask-new"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn installer_refuses_a_hooks_path_in_the_work_tree_but_not_its_own_folder() {
        let repo = new_repo();
        git(repo.path(), &["config", "core.hooksPath", ".githooks"]);
        let (code, text) = install(repo.path(), &[]);
        assert_eq!(code, 1, "{text}");
        assert!(text.contains("core.hooksPath"), "{text}");
        for name in ["pre-commit", "commit-msg", "pre-push"] {
            assert!(!repo.path().join(".githooks").join(name).exists());
            assert!(!repo.path().join(".git/hooks").join(name).exists());
        }
        // Pointing at the repository's own hooks folder is fine.
        let own = repo.path().join(".git").join("hooks");
        git(
            repo.path(),
            &["config", "core.hooksPath", own.to_str().unwrap()],
        );
        let (code, text) = install(repo.path(), &[]);
        assert_eq!(code, 0, "{text}");
        assert!(own.join("pre-push").exists());
    }

    #[test]
    fn installer_refuses_a_global_hooks_path() {
        let outside = temp_dir();
        let shared = outside.path().join("shared-hooks");
        fs::create_dir_all(&shared).unwrap();
        let cfg = outside.path().join("global-gitconfig");
        fs::write(&cfg, "").unwrap();
        // Written through git, so a Windows path is quoted correctly.
        git(
            outside.path(),
            &[
                "config",
                "--file",
                cfg.to_str().unwrap(),
                "core.hooksPath",
                shared.to_str().unwrap(),
            ],
        );
        let repo = new_repo();
        let out = xtask(repo.path())
            .env("GIT_CONFIG_GLOBAL", &cfg)
            .args(["hooks", "install"])
            .output()
            .unwrap();
        let text = output_text(&out);
        assert_eq!(out.status.code(), Some(1), "{text}");
        assert!(text.contains("core.hooksPath"), "{text}");
        assert_eq!(fs::read_dir(&shared).unwrap().count(), 0);
        assert!(!repo.path().join(".git/hooks/pre-commit").exists());
    }

    #[test]
    fn installer_keeps_a_foreign_or_non_utf8_hook() {
        for mine in [
            &b"#!/bin/sh\necho mine\n"[..],
            &b"#!/bin/sh\n# caf\xe9\n"[..],
        ] {
            let repo = new_repo();
            let hooks = repo.path().join(".git/hooks");
            fs::create_dir_all(&hooks).unwrap();
            fs::write(hooks.join("commit-msg"), mine).unwrap();
            let (code, text) = install(repo.path(), &[]);
            assert_eq!(code, 1, "{text}");
            assert!(text.contains("commit-msg"), "{text}");
            assert_eq!(fs::read(hooks.join("commit-msg")).unwrap(), mine);
            assert!(!hooks.join("pre-commit").exists());
            assert!(!hooks.join("pre-push").exists());
        }
    }

    #[test]
    fn installer_keeps_a_foreign_hook_that_only_mentions_the_marker() {
        let repo = new_repo();
        let hooks = repo.path().join(".git/hooks");
        fs::create_dir_all(&hooks).unwrap();
        let mine = "#!/bin/sh\n# runs other checks too; see # xtask-privacy-hook\necho mine\n";
        fs::write(hooks.join("pre-push"), mine).unwrap();
        let (code, text) = install(repo.path(), &[]);
        assert_eq!(code, 1, "{text}");
        assert!(text.contains("pre-push"), "{text}");
        assert_eq!(fs::read_to_string(hooks.join("pre-push")).unwrap(), mine);
        assert!(!hooks.join("pre-commit").exists());
    }

    #[test]
    fn installer_in_a_linked_worktree_writes_the_shared_hooks_folder() {
        let repo = new_repo();
        write(repo.path(), "a.md", "clean\n");
        commit_all(repo.path(), "first");
        let place = temp_dir();
        let wt = place.path().join("wt");
        git(
            repo.path(),
            &["worktree", "add", "-q", wt.to_str().unwrap()],
        );
        let (code, text) = install(&wt, &[]);
        assert_eq!(code, 0, "{text}");
        assert!(repo.path().join(".git/hooks/pre-push").exists());
    }
}

// ---- builder's tests: the installer and the hooks, Unix only ----
#[cfg(unix)]
mod unix_builder {
    use super::common::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn installer_refuses_a_symbolic_link_hook() {
        let repo = new_repo();
        let hooks = repo.path().join(".git/hooks");
        fs::create_dir_all(&hooks).unwrap();
        let outside = temp_dir();
        let target = outside.path().join("pre-commit");
        // Even one that holds the marker: writing would follow the link.
        let mine = "#!/bin/sh\n# xtask-privacy-hook\necho mine\n";
        fs::write(&target, mine).unwrap();
        std::os::unix::fs::symlink(&target, hooks.join("pre-commit")).unwrap();
        let (code, text) = run(repo.path(), &["hooks", "install"]);
        assert_eq!(code, 1, "{text}");
        assert!(text.contains("pre-commit"), "{text}");
        assert!(text.contains("not a regular file"), "{text}");
        assert_eq!(fs::read_to_string(&target).unwrap(), mine);
        assert!(!hooks.join("commit-msg").exists());
        // A dangling link too: writing would create its target.
        fs::remove_file(hooks.join("pre-commit")).unwrap();
        let gone = outside.path().join("never");
        std::os::unix::fs::symlink(&gone, hooks.join("pre-commit")).unwrap();
        let (code, text) = run(repo.path(), &["hooks", "install"]);
        assert_eq!(code, 1, "{text}");
        assert!(!gone.exists());
    }

    #[test]
    fn installer_stops_when_a_hook_cannot_be_inspected() {
        // `.git/hooks` is a file, so looking at a hook inside it fails with
        // an error other than "not found": the installer stops there,
        // before it writes anything.
        let repo = new_repo();
        let hooks = repo.path().join(".git/hooks");
        fs::remove_dir_all(&hooks).unwrap();
        fs::write(&hooks, "not a folder\n").unwrap();
        let (code, text) = run(repo.path(), &["hooks", "install"]);
        assert_eq!(code, 1, "{text}");
        assert!(text.contains("cannot inspect"), "{text}");
        assert!(text.contains("pre-commit"), "{text}");
        assert_eq!(fs::read_to_string(&hooks).unwrap(), "not a folder\n");
    }

    #[test]
    fn installer_never_writes_through_a_link_at_its_temporary_name() {
        let repo = new_repo();
        let hooks = repo.path().join(".git/hooks");
        fs::create_dir_all(&hooks).unwrap();
        let outside = temp_dir();
        let target = outside.path().join("victim");
        fs::write(&target, "untouched\n").unwrap();
        std::os::unix::fs::symlink(&target, hooks.join(".pre-commit.xtask-new")).unwrap();
        let (code, text) = run(repo.path(), &["hooks", "install"]);
        assert_eq!(code, 0, "{text}");
        assert_eq!(fs::read_to_string(&target).unwrap(), "untouched\n");
        let meta = fs::symlink_metadata(hooks.join("pre-commit")).unwrap();
        assert!(meta.is_file(), "the hook is a regular file, not a link");
        assert!(!hooks.join(".pre-commit.xtask-new").exists());
    }

    /// A repository with the stand-in manifest and the hooks installed, whose
    /// `origin` fetches from `private` (which holds `secret-branch`, with a
    /// secret, and `main`) and whose push goes wherever `point` sets it.
    fn two_remotes(
        point: impl FnOnce(&Path, &Path),
    ) -> (tempfile::TempDir, [tempfile::TempDir; 2]) {
        let repo = new_repo();
        let (private, public) = (new_bare(), new_bare());
        add_stand_in_manifest(repo.path());
        write(repo.path(), "a.md", "clean\n");
        commit_all(repo.path(), "first");
        git(repo.path(), &["checkout", "-q", "-b", "secret-branch"]);
        write(
            repo.path(),
            "old.txt",
            &file_with(&vectors().remove(0).text),
        );
        commit_all(repo.path(), "an old mistake");
        git(repo.path(), &["checkout", "-q", "main"]);
        let p = private.path().to_str().unwrap();
        git(repo.path(), &["remote", "add", "origin", p]);
        git(
            repo.path(),
            &["push", "-q", "origin", "main", "secret-branch"],
        );
        git(public.path(), &["fetch", "-q", p, "main:main"]);
        point(repo.path(), public.path());
        xtask(repo.path())
            .args(["hooks", "install"])
            .assert()
            .code(0);
        (repo, [private, public])
    }

    fn push_with_hooks(repo: &Path, path: &std::ffi::OsString, refspec: &str) -> (bool, String) {
        let out = isolated(std::process::Command::new("git"))
            .args(["push", "origin", refspec])
            .current_dir(repo)
            .env("PATH", path)
            .env_remove("CI")
            .output()
            .expect("git runs");
        (out.status.success(), output_text(&out))
    }

    #[test]
    fn pre_push_asks_the_push_target_not_the_tracking_refs() {
        let (_bin, path) = stand_in_cargo();
        // `origin` re-pointed to another repository after fetching.
        let (repo, [_private, public]) = two_remotes(|repo, public| {
            git(
                repo,
                &["remote", "set-url", "origin", public.to_str().unwrap()],
            );
        });
        let (ok, text) = push_with_hooks(repo.path(), &path, "secret-branch");
        assert!(
            !ok,
            "a re-pointed remote's tracking refs are not trusted: {text}"
        );
        assert!(text.contains("old.txt:3: Secret"), "{text}");
        let listed = git_out(public.path(), &["for-each-ref", "refs/heads/secret-branch"]);
        assert!(listed.is_empty(), "nothing was pushed");
        // A push URL that differs from the fetch URL.
        let (repo, [_private, public]) = two_remotes(|repo, public| {
            git(
                repo,
                &["config", "remote.origin.pushurl", public.to_str().unwrap()],
            );
        });
        let (ok, text) = push_with_hooks(repo.path(), &path, "secret-branch");
        assert!(!ok, "the push URL is asked, not the fetch URL: {text}");
        assert!(text.contains("old.txt:3: Secret"), "{text}");
        let listed = git_out(public.path(), &["for-each-ref", "refs/heads/secret-branch"]);
        assert!(listed.is_empty(), "nothing was pushed");
        // Control: a clean new branch on top of what the target has passes.
        git(repo.path(), &["checkout", "-q", "-b", "clean"]);
        write(repo.path(), "c.md", "fine\n");
        commit_all(repo.path(), "clean work");
        let (ok, text) = push_with_hooks(repo.path(), &path, "clean");
        assert!(ok, "{text}");
        assert!(text.contains("privacy: clean (1 commit scanned)"), "{text}");
    }

    fn commit_with_hooks(repo: &Path, path: &std::ffi::OsString, msg: &str) -> (bool, String) {
        let out = isolated(std::process::Command::new("git"))
            .args(["commit", "-q", "-m", msg])
            .current_dir(repo)
            .env("PATH", path)
            .env_remove("CI")
            .output()
            .expect("git runs");
        (out.status.success(), output_text(&out))
    }

    #[test]
    fn hooks_with_require_local_stop_without_a_list_and_default_hooks_warn() {
        let (_bin, path) = stand_in_cargo();
        for require in [false, true] {
            let repo = new_repo();
            add_stand_in_manifest(repo.path());
            write(repo.path(), "a.md", "clean\n");
            commit_all(repo.path(), "first");
            let mut install = xtask(repo.path());
            install.args(["hooks", "install"]);
            if require {
                install.arg("--require-local");
            }
            install.assert().code(0);
            write(repo.path(), "a.md", "still clean\n");
            git(repo.path(), &["add", "a.md"]);
            let (ok, text) = commit_with_hooks(repo.path(), &path, "ordinary message");
            if require {
                assert!(!ok, "a missing list must stop the commit: {text}");
                assert!(text.contains("--require-local"), "{text}");
            } else {
                assert!(ok, "{text}");
                assert!(
                    text.contains("privacy: warning: local terms are not being checked"),
                    "{text}"
                );
            }
        }
    }
}

// ---- builder's tests: what the pre-push hook runs ----
//
// `xtask hooks pre-push -- <remote> <url>` is given git's pre-push input
// directly, so the ref logic is tested without a shell; these tests are
// written to run on Windows too (first shown by CI). The Unix tests above run
// it through the installed hook and a real `git push`. The URL is always a
// bare repository in the test root, or a path that does not exist: nothing
// on a network is contacted.
mod pre_push {
    use super::common::*;
    use std::path::Path;

    const ZERO: &str = "0000000000000000000000000000000000000000";

    /// A repository whose `main` (one clean commit, plus whatever `before`
    /// adds) is pushed to a bare remote named `origin`, with no hooks.
    fn setup(before: impl FnOnce(&Path)) -> (tempfile::TempDir, tempfile::TempDir) {
        let repo = new_repo();
        let remote = new_bare();
        write(repo.path(), "a.md", "clean\n");
        commit_all(repo.path(), "first");
        before(repo.path());
        git(
            repo.path(),
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
        );
        git(repo.path(), &["push", "-q", "origin", "main"]);
        (repo, remote)
    }

    fn rev(dir: &Path, what: &str) -> String {
        git_out(dir, &["rev-parse", what])
    }

    fn url(remote: &tempfile::TempDir) -> String {
        remote.path().to_str().unwrap().to_string()
    }

    fn pre_push(dir: &Path, url: &str, input: &str, extra: &[&str]) -> (i32, String) {
        let out = xtask(dir)
            .args(["hooks", "pre-push"])
            .args(extra)
            .args(["--", "origin", url])
            .write_stdin(input.to_string())
            .output()
            .unwrap();
        (out.status.code().unwrap(), output_text(&out))
    }

    fn line(remote_ref: &str, local: &str, remote: &str) -> String {
        format!("refs/heads/x {local} {remote_ref} {remote}\n")
    }

    fn secret_text() -> String {
        file_with(&vectors().remove(0).text)
    }

    #[test]
    fn pre_push_deleting_a_ref_scans_nothing() {
        let (repo, remote) = setup(|repo| {
            write(repo, "old.txt", &secret_text());
            commit_all(repo, "an old mistake");
        });
        let old = rev(repo.path(), "HEAD");
        let input = format!("(delete) {ZERO} refs/heads/main {old}\n");
        let (code, text) = pre_push(repo.path(), &url(&remote), &input, &[]);
        assert_eq!(code, 0, "{text}");
        assert!(
            text.contains("privacy: clean (0 commits scanned)"),
            "{text}"
        );
    }

    #[test]
    fn pre_push_several_refs_in_one_push() {
        let (repo, remote) = setup(|_| {});
        let (r, u) = (repo.path(), url(&remote));
        git(r, &["checkout", "-q", "-b", "clean"]);
        write(r, "c.md", "fine\n");
        commit_all(r, "clean work");
        let clean = rev(r, "HEAD");
        git(r, &["checkout", "-q", "main"]);
        git(r, &["checkout", "-q", "-b", "mail"]);
        write(r, "mail.md", "write to jane.roe@corp.example\n");
        commit_all(r, "mail");
        let mail = rev(r, "HEAD");
        git(r, &["checkout", "-q", "main"]);
        git(r, &["checkout", "-q", "-b", "leaky"]);
        write(r, "leak.txt", &secret_text());
        commit_all(r, "leak");
        let leaky = rev(r, "HEAD");
        // Three refs, two with a finding: both are reported.
        let input = line("refs/heads/clean", &clean, ZERO)
            + &line("refs/heads/leaky", &leaky, ZERO)
            + &line("refs/heads/mail", &mail, ZERO);
        let (code, text) = pre_push(r, &u, &input, &[]);
        assert_eq!(code, 1, "{text}");
        assert!(text.contains("leak.txt:3: SecretAnthropicKey"), "{text}");
        assert!(text.contains("mail.md:1: Email"), "{text}");
        assert!(text.contains("privacy: 2 finding(s)"), "{text}");
        assert!(text.contains("the push is refused"), "{text}");
        let (code, text) = pre_push(r, &u, &line("refs/heads/c", &clean, ZERO), &[]);
        assert_eq!(code, 0, "{text}");
        assert!(text.contains("privacy: clean (1 commit scanned)"), "{text}");
        // Two refs naming one new commit: it is scanned once.
        let input = line("refs/heads/one", &clean, ZERO) + &line("refs/heads/two", &clean, ZERO);
        let (code, text) = pre_push(r, &u, &input, &[]);
        assert_eq!(code, 0, "{text}");
        assert!(text.contains("privacy: clean (1 commit scanned)"), "{text}");
    }

    #[test]
    fn pre_push_without_a_merge_base_scans_every_commit_of_the_ref() {
        let (repo, remote) = setup(|_| {});
        let r = repo.path();
        git(r, &["checkout", "-q", "--orphan", "island"]);
        git(r, &["rm", "-rq", "--cached", "."]);
        write(r, "island.txt", &secret_text());
        commit_all(r, "island root");
        std::fs::remove_file(r.join("island.txt")).unwrap();
        write(r, "b.md", "clean\n");
        commit_all(r, "island tip");
        let tip = rev(r, "HEAD");
        let input = line("refs/heads/island", &tip, ZERO);
        let (code, text) = pre_push(r, &url(&remote), &input, &[]);
        assert_eq!(code, 1, "{text}");
        assert!(text.contains("island.txt:3: Secret"), "{text}");
        assert!(text.contains("privacy: 1 finding(s)"), "{text}");
    }

    #[test]
    fn pre_push_when_the_remote_cannot_be_listed_scans_the_whole_ref() {
        // An old secret is on the remote's main. Listed, the remote's commits
        // are not scanned again; unlisted, the new branch's whole history is.
        let (repo, remote) = setup(|repo| {
            write(repo, "old.txt", &secret_text());
            commit_all(repo, "an old mistake");
        });
        let r = repo.path();
        git(r, &["checkout", "-q", "-b", "next"]);
        write(r, "c.md", "fine\n");
        commit_all(r, "clean work");
        let tip = rev(r, "HEAD");
        let input = line("refs/heads/next", &tip, ZERO);
        let (code, text) = pre_push(r, &url(&remote), &input, &[]);
        assert_eq!(code, 0, "listed: only the new commit: {text}");
        assert!(text.contains("(1 commit scanned)"), "{text}");
        // A URL that cannot be listed, and URLs shaped like options, which
        // reach git after `--end-of-options` and so are never read as one
        // (an `--upload-pack` would create the marker file).
        let place = temp_dir();
        let marker = place.path().join("marker");
        let missing = place.path().join("no-such-repository");
        let upload_pack = format!("--upload-pack=touch {}", marker.display());
        for bad in [missing.to_str().unwrap(), "-x", upload_pack.as_str()] {
            let (code, text) = pre_push(r, bad, &input, &[]);
            assert_eq!(code, 1, "{bad}: {text}");
            assert!(text.contains("old.txt:3: Secret"), "{bad}: {text}");
        }
        assert!(
            !marker.exists(),
            "an option-shaped URL was read as an option"
        );
        // Without a URL argument, the same.
        let out = xtask(r)
            .args(["hooks", "pre-push", "--", "origin"])
            .write_stdin(input.clone())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{}", output_text(&out));
    }

    #[test]
    fn pre_push_never_trusts_remote_tracking_refs() {
        let (repo, remote) = setup(|_| {});
        let (r, u) = (repo.path(), url(&remote));
        // A branch with a secret, pushed, then deleted on the remote from
        // elsewhere: the local tracking ref stays behind (stale).
        git(r, &["checkout", "-q", "-b", "old"]);
        write(r, "old.txt", &secret_text());
        commit_all(r, "an old mistake");
        git(r, &["push", "-q", "origin", "old"]);
        git(remote.path(), &["branch", "-q", "-D", "old"]);
        assert!(!git_out(r, &["for-each-ref", "refs/remotes/origin/old"]).is_empty());
        git(r, &["checkout", "-q", "-b", "on-top"]);
        write(r, "c.md", "fine\n");
        commit_all(r, "clean work");
        let input = line("refs/heads/on-top", &rev(r, "HEAD"), ZERO);
        let (code, text) = pre_push(r, &u, &input, &[]);
        assert_eq!(code, 1, "a stale tracking ref is not trusted: {text}");
        assert!(text.contains("old.txt:3: Secret"), "{text}");
        // A tracking ref set by hand to a commit never pushed.
        git(r, &["checkout", "-q", "main"]);
        git(r, &["checkout", "-q", "-b", "local-only"]);
        write(r, "local.txt", &secret_text());
        commit_all(r, "never pushed");
        let local = rev(r, "HEAD");
        git(r, &["update-ref", "refs/remotes/origin/local-only", &local]);
        let input = line("refs/heads/local-only", &local, ZERO);
        let (code, text) = pre_push(r, &u, &input, &[]);
        assert_eq!(code, 1, "a hand-made tracking ref is not trusted: {text}");
        assert!(text.contains("local.txt:3: Secret"), "{text}");
    }

    #[test]
    fn pre_push_over_a_remote_commit_it_does_not_have_compares_with_what_the_remote_lists() {
        let (repo, remote) = setup(|repo| {
            write(repo, "old.txt", &secret_text());
            commit_all(repo, "an old mistake");
        });
        let (r, u) = (repo.path(), url(&remote));
        write(r, "c.md", "fine\n");
        commit_all(r, "clean work");
        let tip = rev(r, "HEAD");
        let unknown = "1234567890abcdef1234567890abcdef12345678";
        let (code, text) = pre_push(r, &u, &line("refs/heads/main", &tip, unknown), &[]);
        assert_eq!(code, 0, "{text}");
        assert!(text.contains("(1 commit scanned)"), "{text}");
        // And a known remote commit is the range's start.
        let input = line("refs/heads/main", &tip, &rev(r, "HEAD~1"));
        let (code, text) = pre_push(r, &u, &input, &[]);
        assert_eq!(code, 0, "{text}");
        assert!(text.contains("(1 commit scanned)"), "{text}");
        let input = line("refs/heads/main", &tip, &rev(r, "HEAD~2"));
        let (code, text) = pre_push(r, &u, &input, &[]);
        assert_eq!(code, 1, "the old commit is in this range: {text}");
    }

    #[test]
    fn pre_push_scans_annotated_tags_and_the_commits_of_any_tag() {
        let (repo, remote) = setup(|_| {});
        let (r, u) = (repo.path(), url(&remote));
        // An annotated tag on a commit the remote has: its message is scanned.
        git(
            r,
            &[
                "tag",
                "-a",
                "v1",
                "-m",
                "release\n\nask jane.roe@corp.example",
            ],
        );
        let v1 = rev(r, "v1");
        let short: String = v1.chars().take(10).collect();
        let (code, text) = pre_push(r, &u, &line("refs/tags/v1", &v1, ZERO), &[]);
        assert_eq!(code, 1, "{text}");
        assert!(
            text.contains(&format!("{short} tag {short} message:3: Email")),
            "{text}"
        );
        assert!(text.contains("privacy: 1 finding(s)"), "{text}");
        // A tag of that tag: both messages are scanned.
        git(r, &["tag", "-a", "v1-again", "-m", "plain", "v1"]);
        let again = rev(r, "v1-again");
        let (code, text) = pre_push(r, &u, &line("refs/tags/v1-again", &again, ZERO), &[]);
        assert_eq!(code, 1, "{text}");
        assert!(
            text.contains(&format!("tag {short} message:3: Email")),
            "{text}"
        );
        // A tagger line is an identity line.
        git(r, &["tag", "-a", "v2", "-m", "plain"]);
        let (code, text) = pre_push(r, &u, &line("refs/tags/v2", &rev(r, "v2"), ZERO), &[]);
        assert_eq!(code, 0, "{text}");
        // A lightweight tag on a new commit with a secret: the commit is scanned.
        write(r, "leak.txt", &secret_text());
        commit_all(r, "leak");
        git(r, &["tag", "v3"]);
        let (code, text) = pre_push(r, &u, &line("refs/tags/v3", &rev(r, "v3"), ZERO), &[]);
        assert_eq!(code, 1, "{text}");
        assert!(text.contains("leak.txt:3: Secret"), "{text}");
    }

    #[test]
    fn pre_push_refuses_a_ref_that_names_a_blob_or_a_tree() {
        let (repo, remote) = setup(|_| {});
        let (r, u) = (repo.path(), url(&remote));
        write(r, "loose.txt", &secret_text());
        let blob = git_out(r, &["hash-object", "-w", "loose.txt"]);
        let tree = rev(r, "HEAD^{tree}");
        git(r, &["tag", "-a", "on-a-blob", "-m", "plain", &blob]);
        let annotated = rev(r, "on-a-blob");
        for (name, id, kind) in [
            ("refs/tags/blob", blob.as_str(), "blob"),
            ("refs/tags/tree", tree.as_str(), "tree"),
            ("refs/tags/on-a-blob", annotated.as_str(), "blob"),
        ] {
            let (code, text) = pre_push(r, &u, &line(name, id, ZERO), &[]);
            assert_eq!(code, 2, "{name}: {text}");
            assert!(
                text.contains(&format!("{name} names a {kind}, not a commit")),
                "{text}"
            );
        }
    }

    #[test]
    fn pre_push_with_a_fetch_mirror_lists_the_push_target_or_scans_everything() {
        // `insteadOf` sends fetches (and so `ls-remote`) to a mirror that
        // holds a secret commit; `pushInsteadOf` keeps pushes on the target.
        // The hook must not take the mirror's commits as published.
        let repo = new_repo();
        let (mirror, target) = (new_bare(), new_bare());
        let r = repo.path();
        write(r, "a.md", "clean\n");
        commit_all(r, "first");
        git(r, &["checkout", "-q", "-b", "internal"]);
        write(r, "old.txt", &secret_text());
        commit_all(r, "only on the mirror");
        let secret = rev(r, "HEAD");
        let (m, t) = (url(&mirror), url(&target));
        git(r, &["push", "-q", &m, "main", "internal"]);
        git(r, &["push", "-q", &t, "main"]);
        git(r, &["config", &format!("url.{m}.insteadOf"), &t]);
        git(r, &["config", &format!("url.{t}.pushInsteadOf"), &t]);
        let (code, text) = pre_push(r, &t, &line("refs/heads/leak", &secret, ZERO), &[]);
        assert_eq!(code, 1, "the mirror's listing is not trusted: {text}");
        assert!(text.contains("old.txt:3: Secret"), "{text}");
        // Without the rewrite the target itself is listed: only what it lacks
        // is scanned.
        git(r, &["config", "--unset", &format!("url.{m}.insteadOf")]);
        git(r, &["checkout", "-q", "main"]);
        write(r, "c.md", "fine\n");
        commit_all(r, "clean work");
        let input = line("refs/heads/next", &rev(r, "HEAD"), ZERO);
        let (code, text) = pre_push(r, &t, &input, &[]);
        assert_eq!(code, 0, "{text}");
        assert!(text.contains("(1 commit scanned)"), "{text}");
    }

    #[test]
    fn pre_push_scans_a_tag_objects_whole_raw_text() {
        let (repo, remote) = setup(|_| {});
        let (r, u) = (repo.path(), url(&remote));
        let head = rev(r, "HEAD");
        let token = file_with(&vectors().remove(0).text);
        let token = token.lines().nth(2).unwrap().to_string();
        let tagger = "Test User <test@example.com> 1700000000 +0000";
        for (what, raw) in [
            (
                "an extra header",
                format!(
                    "object {head}\ntype commit\ntag plain\ntagger {tagger}\nx-note {token}\n\nplain\n"
                ),
            ),
            (
                "the tag's own name",
                format!("object {head}\ntype commit\ntag {token}\ntagger {tagger}\n\nplain\n"),
            ),
        ] {
            std::fs::write(r.join("raw-tag"), raw).unwrap();
            let id = git_out(
                r,
                &["hash-object", "--literally", "-t", "tag", "-w", "raw-tag"],
            );
            std::fs::remove_file(r.join("raw-tag")).unwrap();
            let short: String = id.chars().take(10).collect();
            let (code, text) = pre_push(r, &u, &line("refs/tags/plain", &id, ZERO), &[]);
            assert_eq!(code, 1, "{what}: {text}");
            assert!(
                text.contains(&format!("tag {short} object:")),
                "{what}: {text}"
            );
            assert!(text.contains("SecretAnthropicKey"), "{what}: {text}");
        }
        // A secret in the tag's message is reported once.
        git(
            r,
            &["tag", "-a", "v9", "-m", &format!("release\n\n{token}")],
        );
        let (code, text) = pre_push(r, &u, &line("refs/tags/v9", &rev(r, "v9"), ZERO), &[]);
        assert_eq!(code, 1, "{text}");
        assert_eq!(text.matches("SecretAnthropicKey").count(), 1, "{text}");
        assert!(text.contains("message:3: SecretAnthropicKey"), "{text}");
    }

    #[test]
    fn pre_push_scans_the_remote_ref_name() {
        let (repo, remote) = setup(|_| {});
        let r = repo.path();
        let head = rev(r, "HEAD");
        let name = format!("refs/heads/{}{}", "fix-for-", "jane.roe@corp.example");
        let (code, text) = pre_push(r, &url(&remote), &line(&name, &head, ZERO), &[]);
        assert_eq!(code, 1, "{text}");
        assert!(text.contains(&format!("{name}: Email")), "{text}");
    }

    #[test]
    fn pre_push_refuses_input_it_cannot_read_and_a_missing_list_on_request() {
        let (repo, remote) = setup(|_| {});
        let (r, u) = (repo.path(), url(&remote));
        for bad in ["refs/heads/x HEAD refs/heads/x HEAD\n", "one two three\n"] {
            let (code, text) = pre_push(r, &u, bad, &[]);
            assert_eq!(code, 2, "{bad}: {text}");
            assert!(text.contains("pre-push: error: line 1"), "{text}");
        }
        let input = line("refs/heads/main", &rev(r, "HEAD"), ZERO);
        let (code, text) = pre_push(r, &u, &input, &["--require-local"]);
        assert_eq!(code, 2, "{text}");
        assert!(text.contains("--require-local"), "{text}");
        let (code, text) = pre_push(r, &u, "", &[]);
        assert_eq!(code, 0, "{text}");
        // A local ref written with spaces (`HEAD@{1 minute ago}`) is read.
        let input = format!(
            "HEAD@{{1 minute ago}} {} refs/heads/y {ZERO}\n",
            rev(r, "HEAD")
        );
        let (code, text) = pre_push(r, &u, &input, &[]);
        assert_eq!(code, 0, "{text}");
    }
}
