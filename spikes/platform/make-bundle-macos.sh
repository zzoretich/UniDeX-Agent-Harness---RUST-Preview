#!/bin/sh
# Builds "target/UniDeX Platform Trial.app" from the release binary and bundle/Info.plist, so the optional link
# scheme steps in CHECKLIST.md can be tried. It registers nothing by itself: macOS registers the bundle's
# `unidex://` scheme with LaunchServices when the bundle is first opened (CHECKLIST.md says how to undo it).
set -eu
cd "$(dirname "$0")"
cargo build --release --locked
app="target/UniDeX Platform Trial.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp bundle/Info.plist "$app/Contents/Info.plist"
cp target/release/platform-trial "$app/Contents/MacOS/platform-trial"
plutil -lint "$app/Contents/Info.plist"
# An ad hoc signature for the bundle as a whole (no identity, nothing installed).
codesign --force --sign - "$app"
echo "built $app"
