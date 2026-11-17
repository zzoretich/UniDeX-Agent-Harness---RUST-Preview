#!/usr/bin/env bash
# Runs the automation round trip (render-ci-client driving render-ci --automation) on a CI runner.
#
#   ci/automation.sh <label> [atspi]
#
# With `atspi` (Linux), the run happens inside a private D-Bus session whose AT-SPI bus reports
# org.a11y.Status.IsEnabled = true, which is what makes AccessKit build GPUI's accessibility tree on Linux (as a screen
# reader being on would). Writes everything to out/<label>/. Exit code: the client's (0 = every required step passed).
set -u

label="$1"
mode="${2:-}"
out="out/${label}"
mkdir -p "$out"
client=./target/release/render-ci-client
app=./target/release/render-ci
if [ -x "${client}.exe" ]; then
  client="${client}.exe"
  app="${app}.exe"
fi

echo "=== automation ${label} ${mode}"
started=$SECONDS
rc=0
if [ "$mode" = atspi ]; then
  # The script below runs inside the new session; it receives only fixed values from here.
  dbus-run-session -- bash -c '
    set -u
    echo "session bus: set"
    dbus-send --session --print-reply --dest=org.a11y.Bus /org/a11y/bus \
      org.freedesktop.DBus.Properties.Set string:org.a11y.Status string:IsEnabled variant:boolean:true \
      && echo "AT-SPI IsEnabled set to true" || echo "could not set AT-SPI IsEnabled"
    dbus-send --session --print-reply --dest=org.a11y.Bus /org/a11y/bus \
      org.freedesktop.DBus.Properties.Get string:org.a11y.Status string:IsEnabled | tail -1
    "$0" --app "$1" --out "$2" --log debug --a11y-wait-ms 8000
  ' "$client" "$app" "$out" > "$out/client.stdout" 2> "$out/app.log" || rc=$?
else
  "$client" --app "$app" --out "$out" --log debug --a11y-wait-ms 8000 > "$out/client.stdout" 2> "$out/app.log" || rc=$?
fi
echo "client exit code: ${rc} after $((SECONDS - started)) s"
echo "----- client (${label}) -----"
cat "$out/client.stdout"
echo "----- app log (${label}) -----"
cat "$out/app.log"

# Each optional step that did not pass becomes a visible warning, not a silent pass.
grep '^\[WARN\]' "$out/client.stdout" | cut -c1-400 | while IFS= read -r line; do
  esc=$(printf '%s' "$line" | sed -e 's/%/%25/g')
  echo "::warning title=Automation optional step (${label})::${esc}"
done

{
  echo "### Automation ${label}"
  echo
  echo '```text'
  grep -E '^\[(PASS|FAIL|WARN)\]' "$out/client.stdout" | cut -c1-200
  echo '```'
  echo "- client exit code: ${rc}; duration $((SECONDS - started)) s"
} >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
exit "$rc"
