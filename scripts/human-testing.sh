#!/usr/bin/env bash
# Human testing: seeds a sample collection and launches the app against it,
# for poking around by hand (look and feel, workflows), not for automation.
#
#   scripts/human-testing.sh                 seed on first use, then launch
#   scripts/human-testing.sh --reset         throw the data away and reseed
#   scripts/human-testing.sh --extra 200     also generate 200 plain firearms
#   scripts/human-testing.sh --seed-only     seed without launching
#
# The data (database, webview storage, import sample files) lives in
# .human-testing/ (or $HUMAN_TESTING_DIR). The app is pointed at it through
# XDG_*_HOME, so your real collection is never opened. Linux only: other
# platforms have no equivalent override of the app data directory.
#
# The database is encrypted with the same OS-keyring key the app uses.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dir="${HUMAN_TESTING_DIR:-$repo/.human-testing}"
db="$dir/data/com.hoplodex.app/hoplodex.db"

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
  echo "human-testing.sh only supports Linux (it relies on XDG_DATA_HOME)." >&2
  exit 1
fi

if [[ $reset -eq 1 || ! -f "$db" ]]; then
  cargo run --manifest-path "$repo/src-tauri/Cargo.toml" --example human_seed -- \
    --dir "$dir" "${seed_args[@]}"
else
  echo "Using the existing test data in $dir (pass --reset to start over)."
fi

if [[ $launch -eq 1 ]]; then
  cd "$repo"
  XDG_DATA_HOME="$dir/data" XDG_CONFIG_HOME="$dir/config" XDG_CACHE_HOME="$dir/cache" \
    exec npm run tauri dev
fi
