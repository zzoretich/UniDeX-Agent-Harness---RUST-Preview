#!/bin/sh
# Runs the terminal trial's scripted scenarios on macOS and writes one JSON file per run into a results folder.
#
#   sh run-term-macos.sh [results-folder] [runs] [extra cargo flags...]
#
# For each PTY backend: the correctness checks once, then the 200,000-line burst and the input-to-echo latency
# `runs` times; idle with and without a blinking cursor `runs` times on the alacritty backend and once on portable-pty,
# each also sampled from outside with `top` (IDLEW). Every window closes itself when its scenario ends.
set -eu
cd "$(dirname "$0")"
OUT="${1:-target/term-results}"
RUNS="${2:-3}"
shift 2 2>/dev/null || shift $#
mkdir -p "$OUT"
cargo build --release --locked -p chat-term-app --bin term "$@"
BIN=target/release/term

wait_or_kill() {
  pid=$1; limit=$2; n=0
  while kill -0 "$pid" 2>/dev/null; do
    sleep 1; n=$((n + 1))
    if [ "$n" -ge "$limit" ]; then kill "$pid" 2>/dev/null || true; echo "killed $pid after ${limit}s"; fi
  done
}

for b in alacritty portable; do
  echo "$b: check"
  "$BIN" --backend "$b" --scenario check --out "$OUT/term-check-$b.json" >/dev/null 2>&1 &
  wait_or_kill $! 120
  r=1
  while [ "$r" -le "$RUNS" ]; do
    for s in burst echo; do
      echo "$b: $s $r"
      "$BIN" --backend "$b" --scenario "$s" --out "$OUT/term-$s-$b-$r.json" >/dev/null 2>&1 &
      wait_or_kill $! 120
    done
    r=$((r + 1))
  done
done

idle_run() {
  b=$1; s=$2; r=$3
  echo "$b: $s $r"
  "$BIN" --backend "$b" --scenario "$s" --out "$OUT/term-$s-$b-$r.json" >/dev/null 2>&1 &
  pid=$!
  sleep 4
  top -l 26 -s 1 -pid "$pid" -stats pid,idlew,cpu,time > "$OUT/term-$s-$b-$r.top" 2>/dev/null || true
  wait_or_kill "$pid" 60
}
r=1
while [ "$r" -le "$RUNS" ]; do
  idle_run alacritty idle "$r"
  idle_run alacritty idle-blink "$r"
  r=$((r + 1))
done
idle_run portable idle 1
idle_run portable idle-blink 1
echo "results in $OUT"
