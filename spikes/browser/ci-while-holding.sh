#!/bin/sh
# CI only: waits until spike-smoke says it is holding the helper (all checks done, every process
# still running), then lists listening TCP sockets with COMMAND and keeps the trial's lines.
# Threat model P32: none may belong to the app or the helper tree.
#
#   ci-while-holding.sh SMOKE_LOG "COMMAND THAT LISTS LISTENING TCP SOCKETS"
log=$1 command=$2
i=0
while [ $i -lt 180 ] && ! grep -q "holding the helper" "$log" 2>/dev/null; do
  sleep 0.5
  i=$((i + 1))
done
if grep -q "holding the helper" "$log" 2>/dev/null; then
  echo "listening TCP sockets of the trial while it runs (P32, expected none):"
  sh -c "$command" | grep -i spike || echo "none"
else
  echo "the smoke check never reached its hold; no socket check"
fi
