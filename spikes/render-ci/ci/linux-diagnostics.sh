#!/usr/bin/env bash
# Prints what the Linux runner offers for rendering without a GPU: package versions, the virtual display's
# extensions, and the Vulkan and OpenGL devices the software rasterisers expose. Diagnostics only: a missing tool is
# reported, never fatal.
set -u
echo "----- kernel and distribution"
uname -srm
grep -E '^(NAME|VERSION)=' /etc/os-release
echo "----- packages"
dpkg-query -W -f='${Package} ${Version}\n' xvfb mesa-vulkan-drivers libvulkan1 libgl1-mesa-dri libegl-mesa0 \
  libegl1 fonts-dejavu-core at-spi2-core sway grim 2> /dev/null
echo "----- environment (selected)"
for v in DISPLAY WAYLAND_DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR VK_DRIVER_FILES VK_ICD_FILENAMES LIBGL_ALWAYS_SOFTWARE; do
  echo "${v}=${!v:-<unset>}"
done
echo "----- Vulkan ICD files"
ls /usr/share/vulkan/icd.d/ 2> /dev/null || echo "(none)"
if [ -n "${DISPLAY:-}" ]; then
  echo "----- xdpyinfo (summary)"
  xdpyinfo -display "$DISPLAY" 2>&1 | grep -E 'version number|vendor string|vendor release|dimensions|depth of root|number of extensions' || true
  echo "extensions:"
  xdpyinfo -display "$DISPLAY" -queryExtensions 2> /dev/null | sed -n '/number of extensions/,/default screen/p' | tr -s ' ' | head -40 || true
fi
echo "----- vulkaninfo --summary"
vulkaninfo --summary 2>&1 | grep -E 'Vulkan Instance Version|deviceName|deviceType|driverName|driverInfo|apiVersion|GPU[0-9]' || echo "(vulkaninfo failed)"
echo "----- glxinfo -B"
glxinfo -B 2>&1 | grep -E 'OpenGL (renderer|version|vendor)|Accelerated|direct rendering' || echo "(glxinfo failed)"
echo "----- eglinfo -B"
eglinfo -B 2>&1 | grep -E 'platform|OpenGL.*(renderer|version)' | head -20 || echo "(eglinfo failed)"
exit 0
