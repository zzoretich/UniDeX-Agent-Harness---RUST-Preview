#!/usr/bin/env bash
# Runs `render-ci --scenario` once on a CI runner, captures the display while the window is held open, and checks
# both frames (the app's own readback and the display capture) with `frame-check`.
#
#   ci/scenario.sh <label> <capture>     capture: x11 | wayland | windows | none
#
# Run from spikes/render-ci after `cargo build --release --features automation`. Writes everything to out/<label>/.
# Exit code: 0 when the app's own check passed and the display capture (if one was taken) passed its check. On
# Wayland the app cannot read its own window back (GPUI has no readback there and a client cannot capture the
# screen), so only the display capture decides.
set -u

label="$1"
capture="$2"
out="out/${label}"
mkdir -p "$out"
bin=./target/release/render-ci
check=./target/release/frame-check
if [ -x "${bin}.exe" ]; then
  bin="${bin}.exe"
  check="${check}.exe"
fi

echo "=== scenario ${label} (capture: ${capture})"
started=$SECONDS
"$bin" --scenario "$out" --hold-ms "${HOLD_MS:-8000}" --exit-after 150 --log debug > "$out/app.stdout" 2> "$out/app.log" &
pid=$!

# Wait for the app's own capture to finish (it writes `ready`), or for the app to exit, for at most 120 s.
waited=0
while [ ! -f "$out/ready" ] && kill -0 "$pid" 2> /dev/null && [ "$waited" -lt 1200 ]; do
  sleep 0.1
  waited=$((waited + 1))
done
echo "ready after $((SECONDS - started)) s: $([ -f "$out/ready" ] && echo yes || echo no)"

capture_rc=none
if [ -f "$out/ready" ]; then
  case "$capture" in
    x11)
      # Standard X tools: dump the virtual screen (it holds nothing but this window), convert to PNG.
      xwd -root -silent -display "$DISPLAY" > "$out/display.xwd" && xwdtopnm "$out/display.xwd" 2> /dev/null | pnmtopng > "$out/display.png"
      capture_rc=$?
      rm -f "$out/display.xwd"
      ;;
    wayland)
      grim "$out/display.png"
      capture_rc=$?
      ;;
    windows)
      pwsh -NoProfile -File ci/capture-window.ps1 -Scenario "$out/scenario.json" -Out "$out/display.png"
      capture_rc=$?
      ;;
    none) ;;
    *)
      echo "unknown capture kind: $capture" >&2
      exit 2
      ;;
  esac
fi

app_rc=0
wait "$pid" || app_rc=$?
echo "app exit code: ${app_rc} after $((SECONDS - started)) s"
echo "----- app log (${label}) -----"
cat "$out/app.log"
echo "----- scenario.json (${label}) -----"
cat "$out/scenario.json" 2> /dev/null || echo "(none)"
echo

display_rc=none
if [ "$capture_rc" = 0 ] && [ -s "$out/display.png" ]; then
  echo "----- display capture check (${label}) -----"
  display_rc=0
  "$check" "$out/display.png" --json "$out/display-check.json" || display_rc=$?
elif [ "$capture_rc" != none ]; then
  echo "display capture failed with exit code ${capture_rc}"
  display_rc=failed
fi

{
  echo "### Scenario ${label}"
  echo
  echo "- app exit code (0 = its own frame check passed): ${app_rc}"
  echo "- display capture (${capture}): exit ${capture_rc}; check: ${display_rc}"
  echo "- duration: $((SECONDS - started)) s"
} >> "${GITHUB_STEP_SUMMARY:-/dev/null}"

if [ "$app_rc" != 0 ]; then
  if [ "$capture" = wayland ]; then
    echo "::warning title=Scenario ${label}::the app's own readback is not available on Wayland (exit ${app_rc}); judged on the display capture only"
  else
    exit 1
  fi
fi
if [ "$capture" = wayland ] && [ "$display_rc" != 0 ]; then
  echo "Wayland: no display capture passed its check (capture: ${capture_rc}, check: ${display_rc})"
  exit 1
fi
case "$display_rc" in
  none | 0) exit 0 ;;
  *) exit 1 ;;
esac
