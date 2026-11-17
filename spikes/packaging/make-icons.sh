#!/bin/sh
# Trial only (Task 14): write a plain placeholder icon (a solid square, no artwork) as target/packaging/icon/icon.png,
# and on macOS also icon.icns, for the candidates that insist on an icon. Run from the repository root.
set -eu
out=target/packaging/icon
mkdir -p "$out"
python3 - "$out/icon.png" <<'PY'
import struct, sys, zlib
size = 1024
row = b"\x00" + bytes([0x25, 0x63, 0xEB, 0xFF]) * size
raw = row * size
def chunk(tag, data):
    body = tag + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)
png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(raw, 9))
png += chunk(b"IEND", b"")
open(sys.argv[1], "wb").write(png)
PY
if [ "$(uname -s)" = "Darwin" ]; then
  set_dir="$out/icon.iconset"
  rm -rf "$set_dir"
  mkdir -p "$set_dir"
  for s in 16 32 128 256 512; do
    sips -z "$s" "$s" "$out/icon.png" --out "$set_dir/icon_${s}x${s}.png" >/dev/null
    d=$((s * 2))
    sips -z "$d" "$d" "$out/icon.png" --out "$set_dir/icon_${s}x${s}@2x.png" >/dev/null
  done
  iconutil -c icns "$set_dir" -o "$out/icon.icns"
fi
ls -l "$out"
