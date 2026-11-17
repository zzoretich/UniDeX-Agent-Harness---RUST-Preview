//! Drives `render-ci --automation` through its socket and checks the three things design 10.3 asks of it: an action
//! changes the app's state, the element tree can be read back, and a frame can be captured.
//!
//!   render-ci-client --app <path to render-ci> --out <dir> [--log <level>] [--a11y-wait-ms <n>] [--frame-optional]
//!
//! It starts the app as its own child, reads the endpoint from the child's stdout, and runs:
//! hello, wait for a first render, tree, a11y_activate, tree again, frame (before), click "count", wait until the
//! screen shows the new state, tree, frame (after), quit. Frames are saved as PNG and checked with
//! `render_ci::check_frame`. Results go to `<dir>/automation.json`; the exit code is 0 only if every required step
//! passed. The AccessKit tree is reported but not required (it depends on the platform activating accessibility); a
//! missing one prints a `[WARN]` line.

use base64::Engine as _;
use render_ci::{MAX_LINE, check_frame, read_line_limited};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Conn {
    reader: BufReader<Box<dyn Read>>,
    writer: Box<dyn Write>,
    transcript: Vec<Value>,
}

impl Conn {
    fn request(&mut self, request: Value) -> Value {
        let started = Instant::now();
        let response = (|| -> Result<Value, String> {
            writeln!(self.writer, "{request}").map_err(|e| e.to_string())?;
            self.writer.flush().map_err(|e| e.to_string())?;
            let line = read_line_limited(&mut self.reader, MAX_LINE)
                .map_err(|e| e.to_string())?
                .ok_or("the app closed the connection")?;
            serde_json::from_str(&line).map_err(|e| e.to_string())
        })()
        .unwrap_or_else(|e| json!({ "ok": false, "error": format!("transport: {e}") }));
        let mut logged = response.clone();
        if let Some(png) = logged.get("png_base64").and_then(|v| v.as_str()) {
            logged["png_base64"] = json!(format!("<{} base64 characters>", png.len()));
        }
        self.transcript.push(json!({
            "request": request,
            "response": logged,
            "ms": started.elapsed().as_millis(),
        }));
        response
    }
}

#[cfg(unix)]
fn connect(endpoint: &str) -> std::io::Result<(Box<dyn Read>, Box<dyn Write>)> {
    let stream = std::os::unix::net::UnixStream::connect(endpoint)?;
    Ok((Box::new(stream.try_clone()?), Box::new(stream)))
}

#[cfg(windows)]
fn connect(endpoint: &str) -> std::io::Result<(Box<dyn Read>, Box<dyn Write>)> {
    let mut last = None;
    for _ in 0..50 {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(endpoint)
        {
            Ok(file) => return Ok((Box::new(file.try_clone()?), Box::new(file))),
            Err(e) => {
                last = Some(e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    Err(last.unwrap())
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn wait_for_endpoint(child: &mut Child) -> Result<String, String> {
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    let _ = tx.send(None);
                    return;
                }
                Ok(_) => {
                    if let Some(rest) = line.trim_end().strip_prefix("RENDER_CI_ENDPOINT=") {
                        let _ = tx.send(Some(rest.to_string()));
                        // Keep draining so the child never blocks on a full pipe.
                        let mut sink = Vec::new();
                        let _ = reader.read_to_end(&mut sink);
                        return;
                    }
                }
            }
        }
    });
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Some(e)) => Ok(e),
        Ok(None) => Err("the app exited (or closed stdout) before printing its endpoint".into()),
        Err(_) => Err("no endpoint within 120 s".into()),
    }
}

fn find_text(v: &Value, needle: &str) -> bool {
    match v {
        Value::String(s) => s.contains(needle),
        Value::Array(a) => a.iter().any(|x| find_text(x, needle)),
        Value::Object(o) => o.values().any(|x| find_text(x, needle)),
        _ => false,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(app) = arg_value(&args, "--app") else {
        eprintln!(
            "usage: render-ci-client --app <render-ci> --out <dir> [--log <level>] [--a11y-wait-ms <n>] [--frame-optional]"
        );
        std::process::exit(2);
    };
    let out = PathBuf::from(arg_value(&args, "--out").unwrap_or_else(|| "automation-out".into()));
    let log_level = arg_value(&args, "--log").unwrap_or_else(|| "info".into());
    // Where the app cannot read its own window back (Wayland), the frame steps are reported but not required.
    let frame_required = !args.iter().any(|a| a == "--frame-optional");
    let a11y_wait = Duration::from_millis(
        arg_value(&args, "--a11y-wait-ms")
            .and_then(|v| v.parse().ok())
            .unwrap_or(5000),
    );
    std::fs::create_dir_all(&out).expect("create the output folder");
    let started = Instant::now();

    let mut child = Command::new(&app)
        .args(["--automation", "--exit-after", "150", "--log", &log_level])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("render-ci-client: cannot start {app}: {e}");
            std::process::exit(2);
        });

    let mut steps = Steps::default();

    let endpoint = match wait_for_endpoint(&mut child) {
        Ok(e) => e,
        Err(e) => {
            steps.record("endpoint", false, true, json!({ "error": e }));
            let _ = child.kill();
            let _ = child.wait();
            write_results(&out, &steps.list, &[], false, started);
            std::process::exit(1);
        }
    };
    steps.record(
        "endpoint",
        true,
        true,
        json!({ "endpoint_kind": if cfg!(windows) { "named pipe" } else { "unix socket" } }),
    );

    let (reader, writer) = match connect(&endpoint) {
        Ok(p) => p,
        Err(e) => {
            steps.record("connect", false, true, json!({ "error": e.to_string() }));
            let _ = child.kill();
            let _ = child.wait();
            write_results(&out, &steps.list, &[], false, started);
            std::process::exit(1);
        }
    };
    let mut conn = Conn {
        reader: BufReader::new(reader),
        writer,
        transcript: Vec::new(),
    };

    let hello = conn.request(json!({ "cmd": "hello" }));
    steps.record(
        "hello",
        hello["ok"] == true,
        true,
        json!({ "window": hello["window"], "platform": hello["platform"] }),
    );

    // Unknown verbs are refused.
    let refused = conn.request(json!({ "cmd": "shell", "line": "echo hi" }));
    steps.record(
        "unknown_verb_refused",
        refused["ok"] == false,
        true,
        refused.clone(),
    );

    // Wait for a first render.
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut state = conn.request(json!({ "cmd": "state" }));
    while state["render_count"].as_u64().unwrap_or(0) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        state = conn.request(json!({ "cmd": "state" }));
    }
    steps.record(
        "first_render",
        state["render_count"].as_u64().unwrap_or(0) > 0,
        true,
        state.clone(),
    );
    std::thread::sleep(Duration::from_millis(500));

    // The element tree, before and after asking the platform to activate accessibility.
    let tree0 = conn.request(json!({ "cmd": "tree" }));
    let accesskit_before = !tree0["accesskit"]["tree"].is_null();
    let activate = conn.request(json!({ "cmd": "a11y_activate" }));
    let mut tree1 = conn.request(json!({ "cmd": "tree" }));
    let deadline = Instant::now() + a11y_wait;
    while tree1["accesskit"]["tree"].is_null() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
        tree1 = conn.request(json!({ "cmd": "tree" }));
    }
    let registry = tree1["registry"]["elements"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let ids: Vec<&str> = registry.iter().filter_map(|e| e["id"].as_str()).collect();
    let wanted = ["red", "green", "blue", "state", "line", "count", "clicks"];
    let missing: Vec<&&str> = wanted.iter().filter(|w| !ids.contains(w)).collect();
    steps.record(
        "tree_registry",
        missing.is_empty() && find_text(&tree1["registry"], "Clicks: 0"),
        true,
        json!({ "elements": registry, "missing": missing }),
    );
    let accesskit = &tree1["accesskit"];
    let accesskit_ok = !accesskit["tree"].is_null()
        && find_text(&accesskit["tree"], "Count")
        && find_text(&accesskit["tree"], "Clicks: 0");
    steps.record(
        "tree_accesskit",
        accesskit_ok,
        false,
        json!({
            "present_before_activation": accesskit_before,
            "activation": activate,
            "a11y_enabled": accesskit["a11y_enabled"],
            "a11y_active": accesskit["a11y_active"],
            "node_count": accesskit["tree"]["nodes"].as_object().map(|n| n.len()),
        }),
    );
    let _ = std::fs::write(
        out.join("tree.json"),
        serde_json::to_string_pretty(&tree1).unwrap_or_default(),
    );

    // Frame before the click.
    let frame_step = |conn: &mut Conn, name: &str, clicked: bool| -> (bool, Value) {
        let frame = conn.request(json!({ "cmd": "frame" }));
        let Some(b64) = frame["png_base64"].as_str() else {
            return (false, json!({ "error": frame["error"] }));
        };
        let bytes = match base64::engine::general_purpose::STANDARD.decode(b64) {
            Ok(b) => b,
            Err(e) => return (false, json!({ "error": format!("base64: {e}") })),
        };
        let path = out.join(format!("{name}.png"));
        let _ = std::fs::write(&path, &bytes);
        match image::load_from_memory(&bytes) {
            Ok(img) => {
                let report = check_frame(&img.to_rgba8(), clicked);
                (
                    report.ok,
                    json!({ "method": frame["method"], "file": path.file_name().map(|f| f.to_string_lossy().to_string()), "check": report.json }),
                )
            }
            Err(e) => (false, json!({ "error": format!("not a PNG: {e}") })),
        }
    };
    let (ok, detail) = frame_step(&mut conn, "frame-before", false);
    steps.record("frame_before", ok, frame_required, detail);

    // The action.
    let click = conn.request(json!({ "cmd": "click", "id": "count" }));
    steps.record("click_sent", click["ok"] == true, true, click.clone());
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut state = conn.request(json!({ "cmd": "state" }));
    while !(state["clicks"] == 1 && state["rendered_clicks"] == 1) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
        state = conn.request(json!({ "cmd": "state" }));
    }
    steps.record(
        "state_changed",
        state["clicks"] == 1 && state["rendered_clicks"] == 1,
        true,
        state.clone(),
    );
    // Give the renderer a moment to present the new frame before reading it back.
    std::thread::sleep(Duration::from_millis(500));
    let tree2 = conn.request(json!({ "cmd": "tree" }));
    steps.record(
        "tree_after_click",
        find_text(&tree2["registry"], "Clicks: 1"),
        true,
        json!({ "accesskit_shows_clicks_1": find_text(&tree2["accesskit"]["tree"], "Clicks: 1") }),
    );
    let (ok, detail) = frame_step(&mut conn, "frame-after", true);
    steps.record("frame_after", ok, frame_required, detail);

    let quit = conn.request(json!({ "cmd": "quit" }));
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut status = None;
    while Instant::now() < deadline {
        if let Ok(Some(s)) = child.try_wait() {
            status = Some(s);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if status.is_none() {
        let _ = child.kill();
        status = child.wait().ok();
    }
    steps.record(
        "app_exited",
        status.as_ref().is_some_and(|s| s.success()),
        true,
        json!({ "quit": quit, "status": status.map(|s| format!("{s}")) }),
    );

    let all_ok = steps.all_ok;
    write_results(&out, &steps.list, &conn.transcript, all_ok, started);
    println!(
        "automation round trip: {}",
        if all_ok { "PASSED" } else { "FAILED" }
    );
    std::process::exit(if all_ok { 0 } else { 1 });
}

struct Steps {
    list: Vec<Value>,
    all_ok: bool,
}

impl Default for Steps {
    fn default() -> Self {
        Self {
            list: Vec::new(),
            all_ok: true,
        }
    }
}

impl Steps {
    fn record(&mut self, name: &str, ok: bool, required: bool, detail: Value) {
        let tag = if ok {
            "[PASS]"
        } else if required {
            "[FAIL]"
        } else {
            "[WARN]"
        };
        println!(
            "{tag} {name}: {}",
            serde_json::to_string(&detail).unwrap_or_default()
        );
        if required && !ok {
            self.all_ok = false;
        }
        self.list
            .push(json!({ "step": name, "ok": ok, "required": required, "detail": detail }));
    }
}

fn write_results(
    out: &std::path::Path,
    steps: &[Value],
    transcript: &[Value],
    ok: bool,
    started: Instant,
) {
    let results = json!({
        "ok": ok,
        "elapsed_ms": started.elapsed().as_millis(),
        "steps": steps,
        "transcript": transcript,
    });
    let _ = std::fs::write(
        out.join("automation.json"),
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    );
}
