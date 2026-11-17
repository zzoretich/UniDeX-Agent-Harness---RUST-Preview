#!/bin/sh
# Phase B, macOS: build the CEF helper against the CEF distribution already unpacked under
# target/cef/<version>/cef_macos_aarch64 (see decision 0012 for how it got there: downloaded
# once, SHA-256 checked, unpacked by hand), then bundle it next to spike-app.
#
#   ./build-cef-macos.sh
#
# CEF_DOWNLOAD_URL points at a host that does not exist, so if the bindings' build script ever
# tried to download anything (it should not: the unpacked copy is complete), the build fails
# instead of fetching.
set -eu
cd "$(dirname "$0")"
cef="$PWD/target/cef/156.0.30/cef_macos_aarch64"
[ -f "$cef/archive.json" ] || { echo "no unpacked CEF at $cef" >&2; exit 2; }
CEF_PATH="$PWD/target/cef" CEF_DOWNLOAD_URL="https://cef-download-disabled.example.com" \
  cargo build --release --locked -p spike-cef-helper --features cef
cargo build --release --locked
cef-helper/bundle-macos.sh "$cef" target/release
