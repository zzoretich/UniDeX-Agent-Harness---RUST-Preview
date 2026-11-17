#!/bin/sh
# CI only: waits until spike-smoke says it is holding the helper (all checks done, every process
# still running), then lists the trial's processes and the listening TCP sockets that belong to
# them. Threat model P32: none may belong to the app or the helper tree.
#
#   ci-while-holding.sh SMOKE_LOG LABEL
#
# Each outcome is printed as a GitHub warning annotation (on its own line) and added to the step
# summary, so it is visible without opening the log. Exits 1 if a process of the trial listens
# on TCP, 0 otherwise (also when the smoke check never reached its hold; that is annotated).
# On Windows the listing comes from ci-windows-tree.ps1, which also reads each process's
# integrity level and writes a sandbox verdict for the workflow step.
log=$1 label=$2
here=$(dirname "$0")
summary=${GITHUB_STEP_SUMMARY:-/dev/null}
hits=$(mktemp)

note() {
  printf '\n::warning title=%s::%s\n' "$1" "$2"
  printf -- '- %s: %s\n' "$1" "$2" >>"$summary"
}

i=0
while [ $i -lt 180 ] && ! grep -q "holding the helper" "$log" 2>/dev/null; do
  sleep 0.5
  i=$((i + 1))
done
if ! grep -q "holding the helper" "$log" 2>/dev/null; then
  note "P32 socket check ($label)" "the smoke check never reached its hold, so no socket check ran"
  exit 0
fi

echo "processes and listening TCP sockets of the trial while it runs ($label):"
case "$(uname -s)" in
  Linux)
    ps -eo pid,ppid,user,stat,args >"$hits.ps"
    grep -E 'spike-|chrome-sandbox' "$hits.ps" | grep -v grep
    sudo ss -ltnpH >"$hits.ss"
    grep -i 'spike' "$hits.ss" >"$hits"
    ;;
  Darwin)
    ps -axo pid,ppid,user,stat,command >"$hits.ps"
    grep 'spike-' "$hits.ps" | grep -v grep
    lsof -nP -iTCP -sTCP:LISTEN >"$hits.ss"
    grep -i 'spike' "$hits.ss" >"$hits"
    ;;
  MINGW* | MSYS* | CYGWIN*)
    pwsh -NoProfile -NonInteractive -File "$here/ci-windows-tree.ps1" >"$hits.ps" 2>&1
    cat "$hits.ps"
    grep '^listening:' "$hits.ps" >"$hits"
    ;;
  *)
    note "P32 socket check ($label)" "no socket listing on $(uname -s)"
    exit 0
    ;;
esac

if [ -s "$hits" ]; then
  note "P32 socket check ($label)" "a process of the trial listens on TCP: $(tr '\n' ' ' <"$hits")"
  exit 1
fi
note "P32 socket check ($label)" "no process of the trial listened on TCP while it ran"
exit 0
