#!/usr/bin/env bash
# Checks that a shipped build has none of the embedded WebDriver server
# (tauri-plugin-wdio-webdriver), which E2E builds carry behind the `e2e`
# Cargo feature (#29). The server has no authentication and runs any script
# in the page, so any local process could read an unlocked collection
# through it.
#
#   scripts/check-no-webdriver.sh            the dependency graph only
#   scripts/check-no-webdriver.sh BINARY...  also these built binaries
#
# The dependency check is part of `npm run audit`; build-appimage.sh checks
# the release binary it built. Each check is also run the other way round
# (the `e2e` graph, an E2E binary when one is built) so that it can't pass
# because the crate or its marker was renamed.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
crate=tauri-plugin-wdio-webdriver
# The variable the server reads its port from, compiled into it.
marker=TAURI_WEBDRIVER_PORT

has_crate() {
  cargo tree --manifest-path "$repo/src-tauri/Cargo.toml" --edges normal --prefix none \
    --features "$1" | grep -q "^$crate "
}

has_marker() {
  grep -aq "$marker" "$1"
}

status=0
if has_crate custom-protocol; then
  echo "check-no-webdriver: a shipped build (features: custom-protocol) depends on $crate" >&2
  status=1
fi
if ! has_crate custom-protocol,e2e; then
  echo "check-no-webdriver: an E2E build no longer depends on $crate; update this check" >&2
  status=1
fi

for binary in "$@"; do
  if has_marker "$binary"; then
    echo "check-no-webdriver: $binary contains the embedded WebDriver server ($marker)" >&2
    status=1
  fi
done
if [ $# -gt 0 ]; then
  e2e_binary="$repo/src-tauri/target/e2e/hoplodex"
  if [ -f "$e2e_binary" ] && ! has_marker "$e2e_binary"; then
    echo "check-no-webdriver: $e2e_binary has no $marker either; update this check" >&2
    status=1
  fi
fi

[ $status -eq 0 ] && echo "Checked: no embedded WebDriver server in a shipped build${*:+ (and in $*)}"
exit $status
