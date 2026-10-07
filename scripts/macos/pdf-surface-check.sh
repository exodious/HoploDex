#!/usr/bin/env bash
# The PDF surface check on macOS (specs/007-document-preview/research.md §23).
# Run from a Mac that has the tart VM set up (scripts/tart-vm.sh setup), it
# starts the macOS 26 VM, copies this checkout in, runs the check in the test
# user's desktop session (it needs a window and may post input), and copies the
# log and screenshot back to e2e/screenshots-out/pdf-surface/ here. Run in the
# VM's own desktop session, it does the check there:
#
#   scripts/macos/pdf-surface-check.sh             the hostile PDF, truncated and
#                                                  bit-flipped PDFs, a 10 MB PDF,
#                                                  the reach probe, real input
#   scripts/macos/pdf-surface-check.sh --hud-on    FR-003a: WebKit's PDF HUD left
#                                                  on; passes only if the watch
#                                                  deletes Open in Preview's copy,
#                                                  closes the surface and sets the
#                                                  hold
#   scripts/macos/pdf-surface-check.sh shows       the 3-page PDF, a screenshot
#
# The VM's Terminal (or sshd) needs Accessibility and Screen Recording for
# the input and the screenshot (scripts/tart-vm.sh setup grants them where SIP
# is off). The check runs as the VM's test user, in a fresh scratch folder, with
# the app's folders taken from the E2E variables (an `e2e` build); it never
# reaches a real database. Exits with the example's code.
#
# Untested: written before surface/macos.rs (T065) existed.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=$ROOT/e2e/screenshots-out/pdf-surface

if [ "${HOPLODEX_PDF_CHECK_IN_VM:-}" != 1 ]; then
  TART=$ROOT/scripts/tart-vm.sh
  "$TART" start || exit 1
  "$TART" sync || exit 1
  "$TART" gui env HOPLODEX_PDF_CHECK_IN_VM=1 bash scripts/macos/pdf-surface-check.sh "$@"
  CODE=$?
  "$TART" fetch e2e/screenshots-out/pdf-surface || true
  exit $CODE
fi

# Inside the VM, in the desktop session.
MODE=check
EXTRA=()
for arg in "$@"; do
  case "$arg" in
    shows) MODE=shows ;;
    --hud-on) EXTRA+=(--hud-on) ;;
    *) EXTRA+=("$arg") ;;
  esac
done
mkdir -p "$OUT"
cd "$ROOT/src-tauri" || exit 1
cargo build --example pdf_surface_check --features e2e || exit 1

S=$(mktemp -d "${TMPDIR:-/tmp}/surface-XXXX")
mkdir -p "$S"/{config,cache,documents,home,input}
swiftc -O -o "$S/input/input" "$ROOT/src-tauri/examples/pdf_surface_input.swift" || exit 1
SUFFIX=
[ "${EXTRA[*]:-}" = --hud-on ] && SUFFIX=-hud-on
LOG=$OUT/$MODE$SUFFIX-macos.log
# Whatever the document could be copied to: the temp and cache folders (the
# parent of $TMPDIR), /tmp, the test user's Library and the run's scratch.
TEMP_PARENT=$(dirname "$TMPDIR")
FLAGS=()
if [ "$MODE" = check ]; then
  FLAGS=(--input "$S/input/input" --scan "$TEMP_PARENT" --scan /tmp --scan /private/var/tmp
    --scan "$HOME/Library" --scan "$S"
    --skip "$ROOT/src-tauri/target" --skip "$ROOT/node_modules" --skip "$OUT")
fi
env HOPLODEX_E2E_CONFIG_HOME="$S/config" HOPLODEX_E2E_CACHE_HOME="$S/cache" \
  HOPLODEX_E2E_DOCUMENTS="$S/documents" HOPLODEX_E2E_HOME="$S/home" \
  "$ROOT/src-tauri/target/debug/examples/pdf_surface_check" "$MODE" \
  --scratch "$S/scratch" --screenshot "$OUT/$MODE$SUFFIX-macos.png" \
  ${FLAGS[@]+"${FLAGS[@]}"} ${EXTRA[@]+"${EXTRA[@]}"} > "$LOG" 2>&1
CODE=$?
echo "exit $CODE" >> "$LOG"
grep -a "CHECK\|^exit" "$LOG" | grep -av "protocol request\|CHECK report\|page-load"
rm -rf "$S"
exit $CODE
