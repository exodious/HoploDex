#!/usr/bin/env bash
# The window controls check on Linux (specs/008-hardening-batch/research.md §18,
# 008 quickstart.md): builds src-tauri/examples/window_controls_check.rs with the
# E2E profile (a development build installs none of the filters, 008 FR-014) and
# runs it under Xvfb with throwaway HOME and XDG folders and real XTest input.
# It opens the main window with the app's own `window_controls::apply` and a
# test page, right-clicks a blank area, a row, a button, an image, a text field
# and a selection and logs every item each menu offered, presses every 008 FR-012
# key and the mouse's back and forward buttons, and fails if a menu offered an
# item outside 008 FR-011's list or appeared where none belongs, or the page
# reloaded or navigated. Run it in the dev container:
#
#   scripts/dev-container.sh scripts/window-controls-check.sh [EXAMPLE FLAGS...]
#
# The log and the screenshot go to e2e/screenshots-out/window-controls/
# window-controls-linux.{log,png}. Exits with the example's code (0 passed, 1 no
# window, 3 usage, 4 a check failed).
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=$ROOT/e2e/screenshots-out/window-controls
mkdir -p "$OUT"
cd "$ROOT/src-tauri" || exit 1
cargo build --profile e2e --example window_controls_check --features e2e || exit 1

S=$(mktemp -d /tmp/window-controls-XXXX)
mkdir -p "$S"/{home,data,cache,config,state,runtime}
chmod 700 "$S/runtime"
LOG=$OUT/window-controls-linux.log
env -i PATH="$PATH" HOME="$S/home" XDG_DATA_HOME="$S/data" XDG_CACHE_HOME="$S/cache" \
  XDG_CONFIG_HOME="$S/config" XDG_STATE_HOME="$S/state" XDG_RUNTIME_DIR="$S/runtime" \
  xvfb-run -a -s "-screen 0 1280x1000x24" \
  "$ROOT/src-tauri/target/e2e/examples/window_controls_check" \
  --input python3 --input "$ROOT/src-tauri/examples/real_input.py" \
  --scratch "$S/scratch" --out "$OUT" "$@" > "$LOG" 2>&1
CODE=$?
echo "exit $CODE" >> "$LOG"
grep -a "CHECK\|^exit" "$LOG"
rm -rf "$S"
exit $CODE
