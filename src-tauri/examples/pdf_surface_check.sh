#!/usr/bin/env bash
# Runs examples/pdf_surface_check.rs in "shows" mode on Linux under Xvfb with
# throwaway HOME and XDG dirs. Run it in the dev container:
#
#   scripts/dev-container.sh bash src-tauri/examples/pdf_surface_check.sh
#
# The screenshot goes to e2e/screenshots-out/pdf-surface/shows-linux.png and
# the log to shows-linux.log next to it. Exits with the example's code.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=$ROOT/e2e/screenshots-out/pdf-surface
mkdir -p "$OUT"
cd "$ROOT/src-tauri"
cargo build --example pdf_surface_check || exit 1
S=$(mktemp -d /tmp/surface-XXXX)
mkdir -p "$S"/{home,data,cache,config,state,runtime}
chmod 700 "$S/runtime"
env -i PATH="$PATH" HOME="$S/home" XDG_DATA_HOME="$S/data" XDG_CACHE_HOME="$S/cache" \
  XDG_CONFIG_HOME="$S/config" XDG_STATE_HOME="$S/state" XDG_RUNTIME_DIR="$S/runtime" "$@" \
  xvfb-run -a -s "-screen 0 1280x1000x24" \
  "$ROOT/src-tauri/target/debug/examples/pdf_surface_check" shows \
  --scratch "$S/scratch" --screenshot "$OUT/shows-linux.png" > "$OUT/shows-linux.log" 2>&1
CODE=$?
echo "exit $CODE" >> "$OUT/shows-linux.log"
grep -a "CHECK\|^exit" "$OUT/shows-linux.log" | grep -v "protocol request"
rm -rf "$S"
exit $CODE
