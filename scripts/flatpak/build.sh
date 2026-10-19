#!/usr/bin/env bash
# Builds the Flatpak offline, checks it and installs it (npm run bundle:flatpak).
# 011 research.md §2, §13, §15; 011 contracts/flatpak-bundle.md "Project commands".
#
# Runs in `scripts/dev-container.sh --flatpak` only (env.sh's host guard refuses
# anywhere else unless FLATPAK_USER_DIR names a throwaway installation and
# --i-know-this-is-a-host is passed). Each step is printed as it starts, and the
# script stops at the first step that fails, naming it.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cd "$repo"

# An ERR trap, not an EXIT one: env.sh adds its bus clean-up to the EXIT trap, ahead
# of any trap set before it, and that would hide the failing status.
current_step=""
set -E
trap 'printf "\nbuild.sh: FAILED at step: %s\n" "$current_step" >&2' ERR

# shellcheck source=scripts/flatpak/env.sh
source "$here/env.sh" "$@"

app_id=io.github.exodious.HoploDex
manifest="$repo/flatpak/$app_id.yml"
arch=x86_64
state="${HOPLODEX_FLATPAK_STATE:?HOPLODEX_FLATPAK_STATE is not set; run through scripts/dev-container.sh --flatpak}"
build_dir="$state/build-dir"
local_repo="$state/repo"
# GNOME 51 is built on Freedesktop SDK 26.08, and so are both SDK extensions
# (011 research.md §1, 011 T012).
runtime_ref="org.gnome.Platform//51"
required_refs=(
  "$runtime_ref"
  "org.gnome.Sdk//51"
  "org.freedesktop.Sdk.Extension.rust-stable//26.08"
  "org.freedesktop.Sdk.Extension.node24//26.08"
)

step() {
  current_step="$1"
  printf '\n==> %s\n' "$current_step"
}

install_missing_refs() {
  local ref missing=()
  for ref in "${required_refs[@]}"; do
    if flatpak info --user "$ref" >/dev/null 2>&1; then
      echo "$ref: installed"
    else
      missing+=("$ref")
    fi
  done
  if [ "${#missing[@]}" -gt 0 ]; then
    echo "fetching ${missing[*]} from Flathub"
    flatpak install --user --noninteractive --assumeyes --arch="$arch" flathub "${missing[@]}"
  fi
}

# --disable-rofiles-fuse: the container has no /dev/fuse (rofiles-fuse only speeds up
# flatpak-builder's cache by hard-linking; the build is the same without it).
builder() {
  flatpak-builder --user --arch="$arch" --state-dir="$state/state" --disable-rofiles-fuse "$@"
}

# The notices the Flatpak ships equal the other Linux builds' (011 FR-019,
# 011 research.md §13): the generator run here, outside the Flatpak, with the
# same lockfiles. The one line that may differ is the copyright years, because
# git is not in the build and 010 then uses the build's own year.
compare_notices() {
  local other shipped
  other="$(mktemp -d "${TMPDIR:-/tmp}/hoplodex-notices.XXXXXX")"
  node scripts/licenses/notices.mjs --out "$other" --target x86_64-unknown-linux-gnu
  shipped="$build_dir/files/share/licenses/$app_id/NOTICES.txt"
  local years='^Copyright \(C\) .* exodious and the HoploDex contributors$'
  if diff -u <(grep -v -E "$years" "$other/NOTICES.txt") <(grep -v -E "$years" "$shipped"); then
    rm -rf "$other"
    echo "notices: same as the other Linux builds"
  else
    rm -rf "$other"
    echo "notices: DIFFER from the other Linux builds (diff above)" >&2
    return 1
  fi
}

mkdir -p "$state/state" "$repo/flatpak/out"

step "install the runtime, SDK and extensions (network, only if missing)"
install_missing_refs

step "flatpak-builder --download-only (fetches every source; network)"
builder --download-only --force-clean "$build_dir" "$manifest"

step "flatpak-builder --disable-download (the build; no network)"
builder --disable-download --force-clean --repo="$local_repo" "$build_dir" "$manifest"

step "contents check (research.md §13)"
node scripts/flatpak/contents.mjs "$build_dir/files"

step "desktop-file-validate"
desktop-file-validate "$build_dir/files/share/applications/$app_id.desktop"
echo "desktop-file-validate: ok"

step "appstreamcli validate --no-net"
appstreamcli validate --no-net "$build_dir/files/share/metainfo/$app_id.metainfo.xml"
echo "appstreamcli validate: ok"

step "notices comparison (011 FR-019)"
compare_notices

step "install the build from the local repository"
flatpak remote-add --user --if-not-exists --no-gpg-verify hoplodex-local "$local_repo"
flatpak install --user --reinstall --noninteractive --assumeyes hoplodex-local "$app_id"
echo "installed: $app_id (from $local_repo)"

# ---------------------------------------------------------------------------
# 011 T066 (US4) goes here: Flathub's linter on the manifest and the repository,
# then `flatpak build-bundle` into flatpak/out/. Nothing below this line yet.
# ---------------------------------------------------------------------------

echo
echo "build.sh: ok (installed build; the linter and the .flatpak file are not made yet)"
