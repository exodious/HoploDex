#!/usr/bin/env bash
# The drop check on Linux (code scanning alert `paths/cwe-73` on
# `add_document_from_path`): builds src-tauri/examples/drop_check.rs without
# the `e2e` feature, so drops arrive as a shipped build gets them, and runs it
# under Xvfb with throwaway HOME and XDG folders and real XTest input. It drags
# a file from another process's window onto the main window, which must be
# attachable, and a link out of the page itself, which must not be. Run it in
# the dev container:
#
#   scripts/dev-container.sh scripts/drop-check.sh [EXAMPLE FLAGS...]
#
# The log and the screenshot go to e2e/screenshots-out/drop-check/
# drop-linux.{log,png}. Exits with the example's code (0 passed, 1 no window,
# 3 usage, 4 a check failed).
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=$ROOT/e2e/screenshots-out/drop-check
mkdir -p "$OUT"
cd "$ROOT/src-tauri" || exit 1
cargo build --profile e2e --example drop_check || exit 1

S=$(mktemp -d /tmp/drop-check-XXXX)
mkdir -p "$S"/{home,data,cache,config,state,runtime,scratch}
chmod 700 "$S/runtime"
LOG=$OUT/drop-linux.log
BIN=$ROOT/src-tauri/target/e2e/examples/drop_check
env -i PATH="$PATH" HOME="$S/home" XDG_DATA_HOME="$S/data" XDG_CACHE_HOME="$S/cache" \
  XDG_CONFIG_HOME="$S/config" XDG_STATE_HOME="$S/state" XDG_RUNTIME_DIR="$S/runtime" \
  xvfb-run -a -s "-screen 0 1280x1000x24" \
  "$BIN" \
  --input python3 --input "$ROOT/src-tauri/examples/real_input.py" \
  --source "$BIN" --source --drag-source \
  --scratch "$S/scratch" --out "$OUT" "$@" > "$LOG" 2>&1
CODE=$?
echo "exit $CODE" >> "$LOG"
grep -a "CHECK\|^exit" "$LOG"
rm -rf "$S"
exit $CODE
