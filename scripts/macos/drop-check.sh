#!/usr/bin/env bash
# The drop check on macOS (code scanning alert `paths/cwe-73` on
# `add_document_from_path`). Run from a Mac that has the tart VM set up
# (scripts/tart-vm.sh setup), it starts the macOS 26 VM, copies this checkout
# in, runs the check in the test user's desktop session (it needs a window and
# posts input), and copies the log and screenshot back to
# e2e/screenshots-out/drop-check/ here. Run in the VM's own desktop session,
# it does the check there:
#
#   scripts/macos/drop-check.sh [EXAMPLE FLAGS...]
#
# It builds src-tauri/examples/drop_check.rs without the `e2e` feature, so
# drops arrive as a shipped build gets them, drags a file from another
# process's window (examples/drag_source.swift) onto the main window, which
# must be attachable, and links out of the page and a second web view, which
# must not be. Then src-tauri/examples/app_drop_check.rs runs the app itself
# as a release builds it (`npm run build`, then `cargo build --release
# --features custom-protocol`), or the binary DROP_CHECK_APP names, with a
# HOME of its own: it opens a database of one firearm, presses its way to
# that firearm's page, and drags a photo and a PDF there from that other
# process's window; both must be attached (#207). The VM's Terminal (or sshd)
# needs Accessibility and Screen Recording for the input and the screenshot
# (scripts/tart-vm.sh setup grants them where SIP is off). It runs as the VM's
# test user in a fresh scratch folder and never reaches a real database.
# Exits 0 if both parts passed, else the first failure's code (1 no window,
# database or way to the page, 3 usage, 4 a check failed).
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=$ROOT/e2e/screenshots-out/drop-check

if [ "${HOPLODEX_DROP_CHECK_IN_VM:-}" != 1 ]; then
  TART=$ROOT/scripts/tart-vm.sh
  "$TART" start || exit 1
  "$TART" sync || exit 1
  "$TART" gui env HOPLODEX_DROP_CHECK_IN_VM=1 bash scripts/macos/drop-check.sh "$@"
  CODE=$?
  "$TART" fetch e2e/screenshots-out/drop-check || true
  exit $CODE
fi

# Inside the VM, in the desktop session.
mkdir -p "$OUT"
cd "$ROOT/src-tauri" || exit 1
cargo build --profile e2e --example drop_check --example app_drop_check || exit 1
APP=${DROP_CHECK_APP:-}
if [ -z "$APP" ]; then
  (cd "$ROOT" && npm run build) || exit 1
  cargo build --release --features custom-protocol || exit 1
  APP=$ROOT/src-tauri/target/release/hoplodex
fi

S=$(mktemp -d "${TMPDIR:-/tmp}/drop-check-XXXX")
mkdir -p "$S"/{config,cache,documents,home,input,scratch,app,apphome}
swiftc -O -o "$S/input/input" "$ROOT/src-tauri/examples/real_input.swift" || exit 1
swiftc -O -o "$S/input/drag_source" "$ROOT/src-tauri/examples/drag_source.swift" || exit 1
LOG=$OUT/drop-macos.log
env HOPLODEX_E2E_CONFIG_HOME="$S/config" HOPLODEX_E2E_CACHE_HOME="$S/cache" \
  HOPLODEX_E2E_DOCUMENTS="$S/documents" HOPLODEX_E2E_HOME="$S/home" \
  "$ROOT/src-tauri/target/e2e/examples/drop_check" \
  --input "$S/input/input" --source "$S/input/drag_source" \
  --scratch "$S/scratch" --out "$OUT" "$@" > "$LOG" 2>&1
CODE=$?
echo "exit $CODE" >> "$LOG"
grep -a "CHECK\|^exit" "$LOG"

# The app finds its settings and data under HOME.
LOG=$OUT/app-drop-macos.log
env HOME="$S/apphome" "$ROOT/src-tauri/target/e2e/examples/app_drop_check" \
  --app "$APP" --config "$S/apphome/Library/Application Support/io.github.exodious.HoploDex" \
  --input "$S/input/input" --source "$S/input/drag_source" --source-window \
  --scratch "$S/app" --out "$OUT" > "$LOG" 2>&1
APP_CODE=$?
echo "exit $APP_CODE" >> "$LOG"
grep -a "CHECK\|^exit" "$LOG"
rm -rf "$S"
[ "$CODE" -ne 0 ] && exit $CODE
exit $APP_CODE
