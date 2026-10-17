#!/usr/bin/env bash
# Human testing: seeds sample collections and launches the app against them,
# for poking around by hand (look and feel, workflows), not for automation.
#
#   scripts/human-testing.sh                 seed on first use, then launch
#   scripts/human-testing.sh --reset         throw the data away and reseed
#   scripts/human-testing.sh --extra 200     also generate 200 plain firearms
#   scripts/human-testing.sh --seed-only     seed without launching
#   scripts/human-testing.sh --pending-from 1.2.0
#                                            seed (with --reset) the Shared
#                                            collection's pending changes as
#                                            kept by HoploDex 1.2.0, in a form
#                                            this build can't resume
#   scripts/human-testing.sh --older         seed (with --reset) "Older
#                                            collection" too: a copy of the
#                                            stored 0.1.0 database, which
#                                            needs an upgrade to open (009
#                                            quickstart.md Setup S1)
#   scripts/human-testing.sh --release       launch the E2E-profile build (the
#                                            browser controls on: no reload,
#                                            no Inspect Element) instead of
#                                            `tauri dev`
#
# Everything lives in a sandbox, .human-testing/ (or $HUMAN_TESTING_DIR):
#   HoploDex/Main collection.hoplodex     the full collection
#   HoploDex/Shared collection.hoplodex   left open by "Workshop PC", with
#                                         pending changes
#   config/io.github.exodious.HoploDex/machine.json
#                                         the recent list naming both
#   Older/Older collection.hoplodex       with --older: the stored 0.1.0
#                                         database, first in the recent list;
#                                         its passphrase is `release fixture
#                                         0.1.0` (the seed prints it)
#   import-samples/                       spreadsheets to import
# Both databases open with the passphrase this script prints. The app is
# pointed at the sandbox, so your real collections, their recent list and
# your Documents folder are never used: on Linux through XDG_*_HOME and a
# user-dirs.dirs whose documents folder is the sandbox, and on macOS through
# a HOME of its own, home/, whose Library/Application Support is config/ and
# whose Documents is the sandbox. (cargo, rustup and npm keep the real home.)
# Linux and macOS only: Windows has no such override. The seed refuses a
# directory it did not make, and never touches the keyring.
#
# --release runs `npm run build` and `cargo build --profile e2e --features
# custom-protocol,e2e` (as the E2E suite does) and launches that binary in the
# same sandbox. An E2E build takes its config, cache and documents folders from
# HOPLODEX_E2E_* variables (set here, to the sandbox), keeps saved passphrases
# in a file in the sandbox instead of the OS keyring, answers the "open in
# another app" confirmation "open" itself, and runs the E2E suite's WebDriver
# server on 127.0.0.1 (TAURI_WEBDRIVER_PORT, default 4445; it has no
# authentication, so don't leave it running).
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dir="${HUMAN_TESTING_DIR:-$repo/.human-testing}"
db="$dir/HoploDex/Main collection.hoplodex"
seed=(cargo run --quiet --manifest-path "$repo/src-tauri/Cargo.toml" --example human_seed --)

launch=1
release=0
reset=0
seed_args=()
for arg in "$@"; do
  case "$arg" in
    --seed-only) launch=0 ;;
    --release) release=1 ;;
    --reset) reset=1; seed_args+=("$arg") ;;
    *) seed_args+=("$arg") ;;
  esac
done

os="$(uname -s)"
if [[ "$os" != "Linux" && "$os" != "Darwin" ]]; then
  echo "human-testing.sh supports Linux and macOS only." >&2
  exit 1
fi

if [[ $reset -eq 1 || ! -f "$db" ]]; then
  "${seed[@]}" --dir "$dir" ${seed_args[@]+"${seed_args[@]}"}
else
  echo "Using the existing test data in $dir (pass --reset to start over)."
fi

# The documents folder the app suggests for a new database.
mkdir -p "$dir/config"
printf 'XDG_DOCUMENTS_DIR="%s"\n' "$dir" > "$dir/config/user-dirs.dirs"

echo "Passphrase for both databases: $("${seed[@]}" --print-passphrase)"

if [[ $launch -eq 1 ]]; then
  cd "$repo"
  if [[ $release -eq 1 ]]; then
    # The E2E-profile build, as e2e/support/build.ts makes it. It embeds dist/.
    npm run build
    cargo build --manifest-path "$repo/src-tauri/Cargo.toml" --profile e2e \
      --features custom-protocol,e2e --bin hoplodex
    app="$repo/src-tauri/target/e2e/hoplodex"
    # Every place an E2E build looks is the sandbox (src-tauri/src/app_dirs.rs
    # refuses to start without them), and the keyring is a file in it.
    sandbox_env=(
      HOPLODEX_E2E_CONFIG_HOME="$dir/config"
      HOPLODEX_E2E_CACHE_HOME="$dir/cache"
      HOPLODEX_E2E_DOCUMENTS="$dir"
      HOPLODEX_E2E_KEYRING_FILE="$dir/keyring.json"
      HOPLODEX_E2E_OPENED_LOG="$dir/opened.log"
      HOPLODEX_E2E_CONSENT_LOG="$dir/consent.log"
      HOPLODEX_E2E_CONSENT=open
    )
    mkdir -p "$dir/cache"
    if [[ "$os" == "Darwin" ]]; then
      home="$dir/home"
      mkdir -p "$home/Library"
      ln -sfn "$dir/config" "$home/Library/Application Support"
      ln -sfn "$dir" "$home/Documents"
      exec env "${sandbox_env[@]}" HOPLODEX_E2E_HOME="$home" HOME="$home" \
        CFFIXED_USER_HOME="$home" "$app"
    fi
    exec env "${sandbox_env[@]}" XDG_DATA_HOME="$dir/data" XDG_CONFIG_HOME="$dir/config" \
      XDG_CACHE_HOME="$dir/cache" "$app"
  fi
  if [[ "$os" == "Darwin" ]]; then
    # macOS ignores XDG_*: Application Support, Caches, Documents and the
    # web view's data all come from HOME.
    home="$dir/home"
    mkdir -p "$home/Library"
    ln -sfn "$dir/config" "$home/Library/Application Support"
    ln -sfn "$dir" "$home/Documents"
    CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" \
      npm_config_cache="$(npm config get cache)" HOME="$home" CFFIXED_USER_HOME="$home" \
      exec npm run tauri dev
  fi
  XDG_DATA_HOME="$dir/data" XDG_CONFIG_HOME="$dir/config" XDG_CACHE_HOME="$dir/cache" \
    exec npm run tauri dev
fi
