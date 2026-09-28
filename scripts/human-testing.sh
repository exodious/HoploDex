#!/usr/bin/env bash
# Human testing: seeds sample collections and launches the app against them,
# for poking around by hand (look and feel, workflows), not for automation.
#
#   scripts/human-testing.sh                 seed on first use, then launch
#   scripts/human-testing.sh --reset         throw the data away and reseed
#   scripts/human-testing.sh --extra 200     also generate 200 plain firearms
#   scripts/human-testing.sh --seed-only     seed without launching
#
# Everything lives in a sandbox, .human-testing/ (or $HUMAN_TESTING_DIR):
#   HoploDex/Main collection.hoplodex     the full collection
#   HoploDex/Shared collection.hoplodex   left open by "Workshop PC", with
#                                         pending changes
#   config/com.hoplodex.app/machine.json  the recent list naming both
#   import-samples/                       spreadsheets to import
# Both databases open with the passphrase this script prints. The app is
# pointed at the sandbox through XDG_*_HOME and a user-dirs.dirs whose
# documents folder is the sandbox, so your real collections, their recent
# list and your Documents folder are never used. Linux only: other
# platforms have no equivalent override. The seed refuses a directory it
# did not make, and never touches the keyring.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dir="${HUMAN_TESTING_DIR:-$repo/.human-testing}"
db="$dir/HoploDex/Main collection.hoplodex"
seed=(cargo run --quiet --manifest-path "$repo/src-tauri/Cargo.toml" --example human_seed --)

launch=1
reset=0
seed_args=()
for arg in "$@"; do
  case "$arg" in
    --seed-only) launch=0 ;;
    --reset) reset=1; seed_args+=("$arg") ;;
    *) seed_args+=("$arg") ;;
  esac
done

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "human-testing.sh only supports Linux (it relies on XDG_*_HOME)." >&2
  exit 1
fi

if [[ $reset -eq 1 || ! -f "$db" ]]; then
  "${seed[@]}" --dir "$dir" "${seed_args[@]}"
else
  echo "Using the existing test data in $dir (pass --reset to start over)."
fi

# The documents folder the app suggests for a new database.
mkdir -p "$dir/config"
printf 'XDG_DOCUMENTS_DIR="%s"\n' "$dir" > "$dir/config/user-dirs.dirs"

echo "Passphrase for both databases: $("${seed[@]}" --print-passphrase)"

if [[ $launch -eq 1 ]]; then
  cd "$repo"
  XDG_DATA_HOME="$dir/data" XDG_CONFIG_HOME="$dir/config" XDG_CACHE_HOME="$dir/cache" \
    exec npm run tauri dev
fi
