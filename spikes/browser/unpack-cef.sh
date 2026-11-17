#!/bin/sh
# Unpacks a CEF minimal distribution, already downloaded and SHA-256 checked, into the layout the
# CEF bindings' build script expects under CEF_PATH, so that the build script uses it and fetches
# nothing (not even CEF's index.json):
#
#   unpack-cef.sh ARCHIVE CEF_PATH VERSION FOLDER
#   unpack-cef.sh cef_binary_..._macosarm64_minimal.tar.bz2 target/cef 156.0.30 cef_macos_aarch64
#
# This does by hand what `download_cef::extract_target_archive` does: Release/ becomes
# CEF_PATH/VERSION/FOLDER/, Resources/* move into it (not on macOS, where they live in the
# framework), and CMakeLists.txt, cmake/, include/, libcef_dll/, CREDITS.html move next to them.
# An archive.json names the archive; the build script reads only the version from it. The
# listing is checked first: no absolute path and no ".." component.
set -eu
archive=$1 cef_path=$2 version=$3 folder=$4
base=$(basename "$archive" .tar.bz2)
mkdir -p "$cef_path/$version"
dest=$(cd "$cef_path/$version" && pwd)
if [ -e "$dest/$folder" ]; then
  echo "already unpacked: $dest/$folder" >&2
  exit 0
fi
# tar runs from the archive's folder with a relative name: GNU tar (Git Bash on Windows) would
# read "D:/..." as a remote host.
cd "$(dirname "$archive")"
name=$(basename "$archive")
if tar -tjf "$name" | grep -qE '(^/|(^|/)\.\.(/|$))'; then
  echo "refusing: the archive holds an absolute path or a .. component" >&2
  exit 1
fi
tar -xjf "$name" -C "$dest"
mv "$dest/$base/Release" "$dest/$folder"
case "$folder" in
  cef_macos_*) ;;
  *) for f in "$dest/$base/Resources/"*; do mv "$f" "$dest/$folder/"; done ;;
esac
for f in CMakeLists.txt cmake include libcef_dll CREDITS.html LICENSE.txt; do
  mv "$dest/$base/$f" "$dest/$folder/$f"
done
printf '{\n  "type": "minimal",\n  "name": "%s.tar.bz2",\n  "sha1": ""\n}\n' "$base" >"$dest/$folder/archive.json"
echo "unpacked into $dest/$folder"
