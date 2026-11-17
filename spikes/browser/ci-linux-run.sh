#!/bin/sh
# CI only (Linux): runs the headless smoke check of the CEF helper installed in DIR under a
# virtual display, with Chromium's sandbox required (--expect-sandbox: every process the rule in
# protocol/src/tree.rs requires reports a seccomp-bpf filter, and a renderer exists). While the
# smoke check holds the helper, ci-while-holding.sh lists the trial's processes and listening
# sockets; after that the smoke check prints Chromium's own account of its GPU process
# (--gpu-info, reported, not checked). Then the smoke log and the tail of CEF's own log are
# printed. No switch that turns Chromium's sandbox off or weakens it is ever passed.
#
#   ci-linux-run.sh LABEL DIR PROFILE_DIR
#
# Exits with the smoke check's status, or 1 if a process of the trial listened on TCP.
set -u
label=$1 dir=$2 profile=$3
here=$(cd "$(dirname "$0")" && pwd)
log="${RUNNER_TEMP:-/tmp}/smoke-$label.log"
mkdir -p "$profile"
xvfb-run -a "$dir/spike-smoke" --frames 1 --timeout-s 90 --hold-s 8 --expect-sandbox --gpu-info -- \
  "$dir/spike-cef-helper" --profile-dir "$profile" >"$log" 2>&1 &
smoke=$!
sockets=0
"$here/ci-while-holding.sh" "$log" "$label" || sockets=$?
status=0
wait "$smoke" || status=$?
echo "--- smoke log ($label) ---"
cat "$log"
echo "--- CEF log ($label), last 80 lines ---"
if [ -f "$profile/cef.log" ]; then tail -n 80 "$profile/cef.log"; else echo "(no CEF log)"; fi
echo "smoke exit status ($label): $status; socket check: $sockets"
if [ "$status" -ne 0 ]; then exit "$status"; fi
exit "$sockets"
