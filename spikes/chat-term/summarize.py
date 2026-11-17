"""Summarise the chat and terminal trial's JSON results into Markdown tables.

    python3 summarize.py RESULTS_FOLDER [more folders...]

Reads the files run-macos.sh and run-term-macos.sh write. Values from separate runs are shown side by side
("run 1 / run 2 / run 3") so the spread stays visible; nothing is averaged away.
"""

import glob
import json
import os
import re
import statistics
import sys


def load(path):
    with open(path) as f:
        return json.load(f)


def runs(folder, prefix):
    files = sorted(glob.glob(os.path.join(folder, f"{prefix}-[0-9]*.json")), key=lambda p: int(re.findall(r"-(\d+)\.json$", p)[0]))
    return [load(p) for p in files]


def fmt(v):
    if v is None:
        return "-"
    if isinstance(v, float):
        return f"{v:.1f}" if abs(v) >= 10 else f"{v:.2f}"
    return str(v)


def joined(values):
    return " / ".join(fmt(v) for v in values)


def get(d, path):
    for key in path.split("."):
        if d is None:
            return None
        d = d.get(key) if isinstance(d, dict) else None
    return d


def chat_tables(folder):
    out = []
    cold = [load(p) for p in sorted(glob.glob(os.path.join(folder, "coldstart-[0-9]*.json")))]
    if cold:
        out.append("### Cold start (chat, release build)\n")
        out.append("| Milestone | Median | Range | Launches |\n|---|---|---|---|")
        for label, key in [
            ("process start to main", "startup.process_start_to_main_ms"),
            ("first render", "startup.first_render.since_process_start_ms"),
            ("first frame painted", "startup.first_frame_painted.since_process_start_ms"),
            ("next frame signal after it (presented by then)", "startup.next_frame_signal_after_first_paint.since_process_start_ms"),
        ]:
            vals = [get(c, key) for c in cold if get(c, key) is not None]
            if vals:
                out.append(f"| {label} | {statistics.median(vals):.1f} ms | {min(vals):.1f} to {max(vals):.1f} ms | {len(vals)} |")
        out.append("")

    out.append("### Chat scenarios (values per run: run 1 / run 2 / run 3)\n")
    header = (
        "| Scenario, phase | Frames | Paint interval median / p95 / p99 / max (ms) | Over 1.5x / 2x refresh | Over 16.7 ms "
        "| Draw work median / p99 (ms) | Token sent to painted p50 / p95 / p99 (ms) | CPU (s) | Peak RSS / footprint (MiB) |"
    )
    out.append(header)
    out.append("|" + "---|" * 9)
    for scenario in ["scroll", "follow", "stream-up", "resize", "idle"]:
        rs = runs(folder, scenario)
        if not rs:
            continue
        phases = [p["phase"] for p in rs[0]["phases"]]
        for name in phases:
            ps = [next((p for p in r["phases"] if p["phase"] == name), None) for r in rs]
            ps = [p for p in ps if p]
            col = lambda path: joined([get(p, path) for p in ps])
            paint = " ; ".join(
                "/".join(fmt(get(p, f"frames.paint_intervals.{k}")) for k in ["median_ms", "p95_ms", "p99_ms", "max_ms"]) for p in ps
            )
            slow = " ; ".join(
                f"{fmt(get(p, 'frames.paint_intervals.over_1_5x_refresh'))}/{fmt(get(p, 'frames.paint_intervals.over_2x_refresh'))}" for p in ps
            )
            work = " ; ".join(f"{fmt(get(p, 'frames.draw_work.median_ms'))}/{fmt(get(p, 'frames.draw_work.p99_ms'))}" for p in ps)
            tok = " ; ".join(
                "/".join(fmt(get(p, f"tokens.sent_to_frame_painted.{k}")) for k in ["median_ms", "p95_ms", "p99_ms"]) if get(p, "tokens") else "-"
                for p in ps
            )
            mem = " ; ".join(f"{fmt(get(p, 'process.peak_rss_mib'))}/{fmt(get(p, 'process.footprint_mib'))}" for p in ps)
            out.append(
                f"| {scenario}, {name} | {col('frames.frames_drawn')} | {paint} | {slow} | {col('frames.paint_intervals.over_16_7ms')} "
                f"| {work} | {tok} | {col('process.cpu_s')} | {mem} |"
            )
            if name == "random_jumps":
                jw = " ; ".join(
                    f"{fmt(get(p, 'jump_frames.draw_work.median_ms'))}/{fmt(get(p, 'jump_frames.draw_work.p99_ms'))}/{fmt(get(p, 'jump_frames.draw_work.max_ms'))}"
                    for p in ps
                )
                ji = " ; ".join(
                    f"{fmt(get(p, 'jump_frames.paint_interval_ending_here.over_1_5x_refresh'))}/{fmt(get(p, 'jump_frames.paint_interval_ending_here.count'))}"
                    for p in ps
                )
                out.append(f"| scroll, jump frames only: draw work median/p99/max ; late jump frames (over 1.5x) of all | | {jw} | {ji} | | | | | |")
    out.append("")

    idle = runs(folder, "idle")
    if idle:
        out.append("### Idle, 30 s with the thread open\n")
        out.append("| Run | Frames drawn | Renders | CPU (s, in-app) | top IDLEW delta (25 s) | Footprint (MiB) |\n|---|---|---|---|---|---|")
        for i, r in enumerate(idle, 1):
            p = next(p for p in r["phases"] if p["phase"] == "idle")
            idlew = top_idlew(os.path.join(folder, f"idle-{i}.top"))
            out.append(
                f"| {i} | {get(p, 'frames.frames_drawn')} | {get(p, 'renders')} | {fmt(get(p, 'process.cpu_s'))} | {fmt(idlew)} | {fmt(get(p, 'process.footprint_mib'))} |"
            )
        out.append("")
    return "\n".join(out)


def top_idlew(path):
    """IDLEW is cumulative in `top -l`: last sample minus first."""
    if not os.path.exists(path):
        return None
    vals = []
    with open(path) as f:
        for line in f:
            parts = line.split()
            # top marks a value that changed since the last sample with a trailing "+".
            if len(parts) >= 2 and parts[0].isdigit() and parts[1].rstrip("+").isdigit():
                vals.append(int(parts[1].rstrip("+")))
    return vals[-1] - vals[0] if len(vals) >= 2 else None


def term_tables(folder):
    out = []
    files = sorted(glob.glob(os.path.join(folder, "term-*.json")))
    if not files:
        return ""
    out.append("### Terminal\n")
    out.append("| File | Backend | Step | OK | Detail |\n|---|---|---|---|---|")
    for path in files:
        d = load(path)
        for s in d["steps"]:
            detail = []
            if get(s, "frames"):
                pi = get(s, "frames.paint_intervals")
                detail.append(
                    f"frames {get(s, 'frames.frames_drawn')}, paint median/p95/p99/max {fmt(pi.get('median_ms'))}/{fmt(pi.get('p95_ms'))}/{fmt(pi.get('p99_ms'))}/{fmt(pi.get('max_ms'))}, over 1.5x {pi.get('over_1_5x_refresh')}"
                )
                detail.append(f"draw work median/p99 {fmt(get(s, 'frames.draw_work.median_ms'))}/{fmt(get(s, 'frames.draw_work.p99_ms'))}")
            if get(s, "write_to_final_frame_painted_ms") is not None:
                detail.append(f"write to final frame {fmt(get(s, 'write_to_final_frame_painted_ms'))} ms")
            if get(s, "keystroke_to_echo_painted"):
                k = get(s, "keystroke_to_echo_painted")
                detail.append(f"echo p50/p95/p99/max {fmt(k.get('median_ms'))}/{fmt(k.get('p95_ms'))}/{fmt(k.get('p99_ms'))}/{fmt(k.get('max_ms'))} ({k.get('count')})")
            if get(s, "process"):
                detail.append(f"CPU {fmt(get(s, 'process.cpu_s'))} s")
            if s.get("grid_after"):
                detail.append(f"grid {s.get('grid_before')} to {s['grid_after']}")
            if s.get("copied") is not None:
                detail.append(f"copied {s['copied']!r}")
            out.append(f"| {os.path.basename(path)} | {d['backend']} | {s['step']} | {s['ok']} | {'; '.join(detail) or fmt(s.get('ms')) + ' ms'} |")
    out.append("")
    tops = sorted(glob.glob(os.path.join(folder, "term-*.top")))
    if tops:
        out.append("| Idle run | top IDLEW delta |\n|---|---|")
        for t in tops:
            out.append(f"| {os.path.basename(t)} | {fmt(top_idlew(t))} |")
        out.append("")
    return "\n".join(out)


if __name__ == "__main__":
    for folder in sys.argv[1:]:
        print(f"## {os.path.basename(folder.rstrip('/'))}\n")
        print(chat_tables(folder))
        print(term_tables(folder))
