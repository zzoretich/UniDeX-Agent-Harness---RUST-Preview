#!/bin/sh
# Runs the chat trial's scripted scenarios on macOS and writes one JSON file per run into a results folder.
#
#   sh run-macos.sh [results-folder] [runs] [extra cargo flags...]
#
# Variants: TARGET_DIR picks a separate target folder and CHAT_ARGS adds flags to every launch, for example
#   TARGET_DIR=target/kit CHAT_ARGS="--renderer kit" sh run-macos.sh results-kit 3 --features kit
#   TARGET_DIR=target/profiler sh run-macos.sh results-profiler 1 --features profiler
#
# Each scenario runs in a fresh process. The idle scenario is also sampled from outside with `top` (IDLEW, the
# kernel's count of wake-ups from idle) and `ps` (CPU time). Nothing here needs elevated rights, and nothing inspects
# or controls another application. Every window this opens closes itself when its scenario ends.
set -eu
cd "$(dirname "$0")"
OUT="${1:-target/results}"
RUNS="${2:-3}"
shift 2 2>/dev/null || shift $#
mkdir -p "$OUT"
TARGET_DIR="${TARGET_DIR:-target}"
CHAT_ARGS="${CHAT_ARGS:-}"
cargo build --release --locked --target-dir "$TARGET_DIR" -p chat-term-app --bin chat "$@"
BIN="$TARGET_DIR/release/chat"

# Wait for a process to end, killing it after a limit in seconds.
wait_or_kill() {
  pid=$1; limit=$2; n=0
  while kill -0 "$pid" 2>/dev/null; do
    sleep 1; n=$((n + 1))
    if [ "$n" -ge "$limit" ]; then kill "$pid" 2>/dev/null || true; echo "killed $pid after ${limit}s"; fi
  done
}

echo "cold start: 1 warm-up launch, then 10"
"$BIN" $CHAT_ARGS --scenario coldstart --out "$OUT/coldstart-warmup.json" >/dev/null 2>&1
for i in 1 2 3 4 5 6 7 8 9 10; do
  "$BIN" $CHAT_ARGS --scenario coldstart --out "$OUT/coldstart-$i.json" >/dev/null 2>&1
  sleep 1
done

r=1
while [ "$r" -le "$RUNS" ]; do
  echo "run $r: idle"
  "$BIN" $CHAT_ARGS --scenario idle --seconds 30 --out "$OUT/idle-$r.json" >/dev/null 2>&1 &
  pid=$!
  sleep 5
  ps -o time= -p "$pid" > "$OUT/idle-$r.ps-start" || true
  top -l 26 -s 1 -pid "$pid" -stats pid,idlew,cpu,time > "$OUT/idle-$r.top" 2>/dev/null || true
  ps -o time= -p "$pid" > "$OUT/idle-$r.ps-end" || true
  wait_or_kill "$pid" 60
  for s in scroll follow stream-up resize; do
    echo "run $r: $s"
    "$BIN" $CHAT_ARGS --scenario "$s" --seconds 30 --out "$OUT/$s-$r.json" >/dev/null 2>&1 &
    wait_or_kill $! 240
  done
  r=$((r + 1))
done
echo "results in $OUT"
