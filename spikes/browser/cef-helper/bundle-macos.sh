#!/bin/sh
# Phase B, macOS only: assemble the CEF helper's app bundle beside the built binaries.
#
#   cef-helper/bundle-macos.sh <extracted CEF folder> <target folder>
#
# <extracted CEF folder> holds "Chromium Embedded Framework.framework" (the cef_macos_aarch64
# folder the bindings' build script extracts). <target folder> holds the built spike-cef-helper
# (for example target/release). The result is <target folder>/spike-cef-helper.app:
#
#   Contents/MacOS/spike-cef-helper                         CEF's browser process (started by the app)
#   Contents/Frameworks/Chromium Embedded Framework.framework
#   Contents/Frameworks/spike-cef-helper Helper.app         utility sub-processes
#   Contents/Frameworks/spike-cef-helper Helper (GPU).app
#   Contents/Frameworks/spike-cef-helper Helper (Renderer).app
#   Contents/Frameworks/spike-cef-helper Helper (Plugin).app
#   Contents/Frameworks/spike-cef-helper Helper (Alerts).app
#
# Chromium finds its sub-process executables by these names, next to the framework. Each helper
# is a copy of the same binary; it sees `--type=...` and runs the sub-process. Every bundle sets
# LSUIElement, so nothing appears in the Dock. Nothing is signed beyond the linker's ad hoc
# signature; that is enough to run locally, not to ship.
set -eu

cef_dir=$1
target=$2
name=spike-cef-helper
app="$target/$name.app"
framework="Chromium Embedded Framework.framework"

[ -d "$cef_dir/$framework" ] || { echo "no $framework in $cef_dir" >&2; exit 2; }
[ -x "$target/$name" ] || { echo "no built $name in $target" >&2; exit 2; }

plist() {
  # plist <file> <executable> <identifier suffix>
  cat > "$1" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>$2</string>
  <key>CFBundleIdentifier</key><string>com.example.unidex-browser-trial$3</string>
  <key>CFBundleName</key><string>$2</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>0.0.0</string>
  <key>CFBundleShortVersionString</key><string>0.0.0</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>LSUIElement</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
EOF
}

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks" "$app/Contents/Resources"
cp "$target/$name" "$app/Contents/MacOS/$name"
plist "$app/Contents/Info.plist" "$name" ""
# A copy-on-write clone where the file system allows it (APFS), a plain copy otherwise.
cp -cR "$cef_dir/$framework" "$app/Contents/Frameworks/" 2>/dev/null ||
  cp -R "$cef_dir/$framework" "$app/Contents/Frameworks/"

for suffix in "" " (GPU)" " (Renderer)" " (Plugin)" " (Alerts)"; do
  helper="$name Helper$suffix"
  id=$(printf '%s' "$suffix" | tr -d ' ()' | tr '[:upper:]' '[:lower:]')
  dir="$app/Contents/Frameworks/$helper.app/Contents"
  mkdir -p "$dir/MacOS"
  cp "$target/$name" "$dir/MacOS/$helper"
  plist "$dir/Info.plist" "$helper" ".helper${id:+.$id}"
done

du -sh "$app"
echo "bundled $app"
