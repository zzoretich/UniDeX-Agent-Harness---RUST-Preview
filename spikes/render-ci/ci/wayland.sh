#!/usr/bin/env bash
# The Wayland trial: a headless wlroots compositor (sway, headless back end, pixman software renderer), the app as a
# Wayland client, and `grim` to capture the compositor's output (it holds nothing but this window).
#
#   ci/wayland.sh       run from spikes/render-ci after the build; writes out/linux-wayland/
set -u
mkdir -p out/linux-wayland
runtime=$(mktemp -d)
chmod 700 "$runtime"
export XDG_RUNTIME_DIR="$runtime"
printf 'output HEADLESS-1 resolution 1280x800\ndefault_border none\n' > "$runtime/sway.conf"
sway --version || { echo "sway is not installed"; exit 1; }
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
  sway -c "$runtime/sway.conf" > out/linux-wayland/sway.log 2>&1 &
sway_pid=$!
socket=""
for _ in $(seq 1 150); do
  socket=$(cd "$runtime" && ls wayland-[0-9] 2> /dev/null | head -1)
  [ -n "$socket" ] && break
  kill -0 "$sway_pid" 2> /dev/null || break
  sleep 0.1
done
if [ -z "$socket" ]; then
  echo "sway did not create a Wayland socket"
  cat out/linux-wayland/sway.log
  kill "$sway_pid" 2> /dev/null
  exit 1
fi
echo "compositor socket: $socket"
export WAYLAND_DISPLAY="$socket"
unset DISPLAY
rc=0
bash ci/scenario.sh linux-wayland wayland || rc=$?
kill "$sway_pid" 2> /dev/null
wait "$sway_pid" 2> /dev/null
echo "----- sway log (tail)"
tail -40 out/linux-wayland/sway.log
exit "$rc"
