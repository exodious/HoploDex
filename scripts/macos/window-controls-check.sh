#!/usr/bin/env bash
# The window controls check on macOS (specs/008-hardening-batch/research.md
# §13, §18, 008 quickstart.md). Run from a Mac that has the tart VM set up
# (scripts/tart-vm.sh setup), it starts the macOS 26 VM, copies this checkout
# in, runs the check in the test user's desktop session (it needs a window and
# posts input), and copies the log and screenshots back to
# e2e/screenshots-out/window-controls/ here. Run in the VM's own desktop
# session, it does the check there:
#
#   scripts/macos/window-controls-check.sh
#
# It opens the main window with the app's own `window_controls::apply` (the
# hook on WryWebView) and a test page, right-clicks a blank area, a row, a
# button, an image, a text field and a selection and logs every item each menu
# offered, presses every 008 FR-012 key and the mouse's back and forward buttons,
# and presses control-command-D on a selection (Look Up). It fails if a menu
# offered an item outside 008 FR-011's list or appeared where none belongs, the
# page reloaded or navigated, or a Look Up panel opened. AppKit's AutoFill is
# not in the log: it is added outside the menu's items and kept by the owner's
# decision (008 spec.md Clarifications 2026-10-11).
#
# The VM's Terminal (or sshd) needs Accessibility and Screen Recording for the
# input and the screenshots (scripts/tart-vm.sh setup grants them where SIP is
# off). The check builds with the E2E profile, since a development build
# installs no filters (008 FR-014), and runs as the VM's test user in a fresh
# scratch folder; it never reaches a real database. Exits with the example's
# code (0 passed, 1 no window, 3 usage, 4 a check failed).
#
# Run on the macOS 26 VM for 008 T084.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=$ROOT/e2e/screenshots-out/window-controls

if [ "${HOPLODEX_WINDOW_CHECK_IN_VM:-}" != 1 ]; then
  TART=$ROOT/scripts/tart-vm.sh
  "$TART" start || exit 1
  "$TART" sync || exit 1
  "$TART" gui env HOPLODEX_WINDOW_CHECK_IN_VM=1 bash scripts/macos/window-controls-check.sh "$@"
  CODE=$?
  "$TART" fetch e2e/screenshots-out/window-controls || true
  exit $CODE
fi

# Inside the VM, in the desktop session.
mkdir -p "$OUT"
cd "$ROOT/src-tauri" || exit 1
cargo build --profile e2e --example window_controls_check --features e2e || exit 1

S=$(mktemp -d "${TMPDIR:-/tmp}/window-controls-XXXX")
mkdir -p "$S"/{config,cache,documents,home,input}
swiftc -O -o "$S/input/input" "$ROOT/src-tauri/examples/real_input.swift" || exit 1
LOG=$OUT/window-controls-macos.log
env HOPLODEX_E2E_CONFIG_HOME="$S/config" HOPLODEX_E2E_CACHE_HOME="$S/cache" \
  HOPLODEX_E2E_DOCUMENTS="$S/documents" HOPLODEX_E2E_HOME="$S/home" \
  "$ROOT/src-tauri/target/e2e/examples/window_controls_check" \
  --input "$S/input/input" --scratch "$S/scratch" --out "$OUT" "$@" > "$LOG" 2>&1
CODE=$?
echo "exit $CODE" >> "$LOG"
grep -a "CHECK\|^exit" "$LOG"
rm -rf "$S"
exit $CODE
