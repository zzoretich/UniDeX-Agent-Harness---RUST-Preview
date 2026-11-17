use clap::Parser;

/// UniDeX command line.
#[derive(Parser)]
#[command(name = "unidex", disable_version_flag = true)]
struct Cli {
    /// Print the version and exit.
    #[arg(long)]
    version: bool,

    /// Print machine-readable output.
    #[arg(long)]
    json: bool,
}

fn main() {
    let cli = Cli::parse();
    if cli.version {
        if cli.json {
            println!(
                "{{\"name\":\"unidex\",\"version\":\"{}\"}}",
                udx_core::VERSION
            );
        } else {
            println!("unidex {}", udx_core::VERSION);
        }
    }
}
