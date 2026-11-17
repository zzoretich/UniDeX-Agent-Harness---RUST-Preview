//! Checks a PNG against the trial's scene: `frame-check <png> [--clicked] [--json <out>]`.
//!
//! Used for captures the app did not make itself (the virtual display on Linux, the desktop on Windows). Exit code 0
//! when the check passes, 1 when it fails, 2 when the file cannot be read.

use render_ci::check_frame;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        eprintln!("usage: frame-check <png> [--clicked] [--json <out>]");
        std::process::exit(2);
    };
    let clicked = args.iter().any(|a| a == "--clicked");
    let json_out = args
        .iter()
        .position(|a| a == "--json")
        .and_then(|i| args.get(i + 1));
    let img = match image::open(path) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            eprintln!("frame-check: cannot read {path}: {e}");
            std::process::exit(2);
        }
    };
    let report = check_frame(&img, clicked);
    let text = serde_json::to_string_pretty(&report.json).unwrap_or_default();
    println!("{text}");
    if let Some(out) = json_out {
        let _ = std::fs::write(out, &text);
    }
    println!(
        "frame-check {path}: {}",
        if report.ok { "PASSED" } else { "FAILED" }
    );
    std::process::exit(if report.ok { 0 } else { 1 });
}
