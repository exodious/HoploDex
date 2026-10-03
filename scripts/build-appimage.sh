#!/usr/bin/env bash
# Builds the Linux bundles (`tauri build`) without the build machine's
# display-stack libraries inside the AppImage. See "Linux AppImage" in
# DEVELOPMENT.md.
#
#   scripts/build-appimage.sh                     same as `npm run tauri build`
#   scripts/build-appimage.sh --bundles appimage  only the AppImage
#
# Arguments are passed to `tauri build`.
#
# Why: linuxdeploy copies libwayland-*, libxkbcommon, libxcb-* and friends
# from the build machine into the AppImage, and they shadow the host's copies.
# A host with a newer Mesa then can't set up EGL against them, and WebKitGTK
# aborts with "Could not create default EGL display: EGL_BAD_PARAMETER",
# leaving an empty grey window (tauri-apps/tauri#15976). linuxdeploy leaves out
# anything matching LINUXDEPLOY_EXCLUDED_LIBRARIES; the linuxdeploy that
# Tauri CLI 2.12 downloads is the first to read it, and older ones ignore it
# silently, so the build is checked afterwards.

set -euo pipefail

# The host provides these; a bundled copy only breaks EGL. Globs, matched
# against the library's file name.
EXCLUDED=(
  'libwayland-*'
  'libxkbcommon*'
  'libxcb*'
  'libXau*'
  'libXdmcp*'
)

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"

LINUXDEPLOY_EXCLUDED_LIBRARIES=$(IFS=';'; echo "${EXCLUDED[*]}")
export LINUXDEPLOY_EXCLUDED_LIBRARIES
check=$(mktemp -d)
trap 'rm -rf "$check"' EXIT
touch "$check/started"
npx tauri build "$@"
# The E2E-only WebDriver server must never ship (#29).
scripts/check-no-webdriver.sh src-tauri/target/release/hoplodex

# Check each AppImage this build made (not older ones left in the folder).
shopt -s nullglob
status=0
for appimage in src-tauri/target/release/bundle/appimage/*.AppImage; do
  [ "$appimage" -nt "$check/started" ] || continue
  rm -rf "$check/squashfs-root"
  (cd "$check" && "$repo/$appimage" --appimage-extract >/dev/null)
  found=()
  for pattern in "${EXCLUDED[@]}"; do
    found+=("$check"/squashfs-root/usr/lib/$pattern)
  done
  if [ ${#found[@]} -gt 0 ]; then
    echo "build-appimage.sh: $appimage still bundles these, so it won't start on newer Mesa:" >&2
    printf '  %s\n' "${found[@]##*/}" >&2
    echo "Is @tauri-apps/cli 2.12 or later installed? (npm ci)" >&2
    status=1
  else
    echo "Checked $appimage: no display-stack libraries bundled"
  fi
done
exit $status
