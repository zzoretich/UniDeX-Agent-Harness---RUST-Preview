mod governed;

use clap::{Args, Parser, Subcommand};
use std::process::ExitCode;
use xtask::privacy::{Mode, Options};

#[derive(Parser)]
#[command(name = "xtask", about = "UniDeX repository tooling")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Scan for personal information. Exit 0 clean, 1 findings, 2 error.
    Privacy(PrivacyArgs),
    /// Flag the governed files among a pull request's changed files.
    /// Exit 0 whether or not any are flagged, 1 on an error.
    Governed(GovernedArgs),
    /// Manage git hooks.
    Hooks {
        #[command(subcommand)]
        action: HooksCmd,
    },
}

#[derive(Args)]
struct GovernedArgs {
    /// A file of changed paths, each ended by a NUL byte. With --name-status:
    /// the output of `git diff --name-status -z --find-renames BASE...HEAD`
    /// saved to a file, which names both paths of a rename.
    #[arg(long, value_name = "LIST_FILE")]
    files: std::path::PathBuf,
    /// Read the list as `git diff --name-status -z` output.
    #[arg(long)]
    name_status: bool,
    /// The changed-file count of the pull request (needs --listed).
    #[arg(long, value_name = "N", requires = "listed")]
    changed: Option<u64>,
    /// The number of entries the hosting service listed (needs --changed).
    #[arg(long, value_name = "N", requires = "changed")]
    listed: Option<u64>,
    /// The base commit id examined, 40 or 64 hexadecimal characters.
    #[arg(long, value_name = "ID", value_parser = governed::check_commit_id, requires = "head")]
    base: Option<String>,
    /// The head commit id examined, 40 or 64 hexadecimal characters.
    #[arg(long, value_name = "ID", value_parser = governed::check_commit_id, requires = "base")]
    head: Option<String>,
}

#[derive(Subcommand)]
enum HooksCmd {
    /// Write the pre-commit and commit-msg hooks.
    Install,
}

#[derive(Args)]
#[command(group(clap::ArgGroup::new("mode").required(true).args(["staged", "all", "range", "stdin"])))]
struct PrivacyArgs {
    /// Scan the staged version of each staged file.
    #[arg(long)]
    staged: bool,
    /// Scan every tracked and untracked-but-not-ignored file.
    #[arg(long)]
    all: bool,
    /// Scan every commit in A..B: files they touch and their messages.
    #[arg(long, value_name = "A..B")]
    range: Option<String>,
    /// Scan standard input under this label.
    #[arg(long, value_name = "LABEL")]
    stdin: Option<String>,
    /// Fail with exit 2 when there is no local term list.
    #[arg(long)]
    require_local: bool,
    /// With --stdin: clean the text as a git commit message file first.
    #[arg(long)]
    git_message: bool,
    /// Print findings as a JSON array.
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Privacy(a) => {
            let mode = if a.staged {
                Mode::Staged
            } else if a.all {
                Mode::All
            } else if let Some(r) = a.range {
                Mode::Range(r)
            } else {
                Mode::Stdin(a.stdin.unwrap_or_default())
            };
            let code = xtask::privacy::run(&Options {
                mode,
                require_local: a.require_local,
                json: a.json,
                git_message: a.git_message,
            });
            ExitCode::from(code as u8)
        }
        Cmd::Governed(a) => {
            let summary = std::env::var_os("GITHUB_STEP_SUMMARY")
                .filter(|v| !v.is_empty())
                .map(std::path::PathBuf::from);
            let mut out = std::io::stdout().lock();
            let req = governed::Request {
                list: &a.files,
                name_status: a.name_status,
                counts: a.changed.zip(a.listed),
                base: a.base.as_deref(),
                head: a.head.as_deref(),
                summary: summary.as_deref(),
            };
            match governed::run(&req, &mut out) {
                Ok(_) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("governed: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Cmd::Hooks {
            action: HooksCmd::Install,
        } => match xtask::hooks::install() {
            Ok(paths) => {
                for p in paths {
                    println!("installed {}", p.display());
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("hooks: {e}");
                ExitCode::from(1)
            }
        },
    }
}
