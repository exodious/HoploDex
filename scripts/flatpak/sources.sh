#!/usr/bin/env bash
# Regenerates flatpak/cargo-sources.json and flatpak/node-sources.json from the
# lockfiles (npm run flatpak:sources). 011 research.md §3.
#
# Runs in the dev container only (`scripts/dev-container.sh --flatpak npm run
# flatpak:sources`): it sources the host guard's container check (guard.mjs), not
# env.sh, because it needs no Flatpak installation, only the two generators the
# image pins at /opt/flatpak-builder-tools. Each list is written to a temporary
# file and moved into place only when its generator succeeds, so a failed run
# leaves the committed lists as they were.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
tools="${HOPLODEX_FLATPAK_BUILDER_TOOLS:-/opt/flatpak-builder-tools}"

# The guard refuses outside the dev container (and its message says what to do).
node "$here/guard.mjs" "$@" >/dev/null

if [ ! -f "$tools/cargo/flatpak-cargo-generator.py" ] || [ ! -d "$tools/node/flatpak_node_generator" ]; then
  echo "sources.sh: the generators are not at $tools; rebuild the dev container image (scripts/dev-container.sh --build)." >&2
  exit 1
fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/hoplodex-flatpak-sources.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

echo "sources: cargo-sources.json from src-tauri/Cargo.lock"
python3 "$tools/cargo/flatpak-cargo-generator.py" "$repo/src-tauri/Cargo.lock" -o "$tmp/cargo-sources.json" \
  || { echo "sources.sh: flatpak-cargo-generator.py failed; flatpak/cargo-sources.json is unchanged." >&2; exit 1; }

echo "sources: node-sources.json from package-lock.json"
PYTHONPATH="$tools/node" python3 -m flatpak_node_generator npm "$repo/package-lock.json" -o "$tmp/node-sources.json" \
  || { echo "sources.sh: flatpak-node-generator failed; flatpak/node-sources.json is unchanged." >&2; exit 1; }

mkdir -p "$repo/flatpak"
mv "$tmp/cargo-sources.json" "$repo/flatpak/cargo-sources.json"
mv "$tmp/node-sources.json" "$repo/flatpak/node-sources.json"
echo "sources: ok (flatpak/cargo-sources.json, flatpak/node-sources.json)"
