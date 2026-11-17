#!/bin/bash
# Phase B, macOS: run one trial scenario several times and keep each run's log.
#
#   ./measure-macos.sh OUT_DIR NAME RUNS [spike-app options...]
#
# Example (the scale-2 display was GPUI's display 1 on the trial machine):
#   ./measure-macos.sh /tmp/m scroll 3 --page long --display 1 --scenario scroll --measure-seconds 10
#
# Every run gets a hard time limit (the app's own --seconds, and a kill timer 30 s later), a
# fresh browser profile under target/release/cef-profiles, and the re-launch guard's log in
# OUT_DIR. After each run the script checks that no trial process is left and stops if one is.
# The `app: summary {json}` lines are what the decision record's tables are made from.
set -u
cd "$(dirname "$0")"
out=$1 name=$2 runs=$3
shift 3
mkdir -p "$out"
export UDX_SPIKE_GUARD_FILE="$out/launches.log"
for i in $(seq 1 "$runs"); do
  log="$out/$name-$i.log"
  set -m
  target/release/spike-app --helper cef --seconds 90 "$@" >"$log" 2>&1 &
  pid=$!
  ( sleep 120; kill -TERM -- "-$pid" 2>/dev/null; sleep 2; kill -KILL -- "-$pid" 2>/dev/null;
    pkill -KILL -u "$(id -u)" -f spike-cef-helper 2>/dev/null ) &
  timer=$!
  wait "$pid"
  status=$?
  kill "$timer" 2>/dev/null
  wait "$timer" 2>/dev/null
  set +m
  # Chromium's sub-processes exit on their own once the browser process is gone; give them up
  # to 5 s and report how long it took.
  waited=0
  left=$(pgrep -u "$(id -u)" -f 'spike-cef-helper|spike-app' || true)
  while [ -n "$left" ] && [ "$waited" -lt 50 ]; do
    sleep 0.1
    waited=$((waited + 1))
    left=$(pgrep -u "$(id -u)" -f 'spike-cef-helper|spike-app' || true)
  done
  summaries=$(grep -c '^app: summary' "$log")
  echo "$name run $i: exit $status, $summaries summary line(s), trial processes gone after $((waited * 100)) ms, leftover: ${left:-none}"
  if [ -n "$left" ]; then
    echo "stopping: a trial process is still running" >&2
    exit 1
  fi
  sleep 2
done
