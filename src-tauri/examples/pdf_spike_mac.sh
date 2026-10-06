#!/usr/bin/env bash
# Runs examples/pdf_spike.rs on macOS with real input, as pdf_spike.sh does on
# Linux: takes screenshots, hovers for WebKit's PDF HUD, clicks its Download
# and Open in Preview, right-clicks for the context menu and clicks its Open
# with Preview, clicks the PDF's link. Then it lists every file written
# during the run and any that holds the PDF's marker, and any copy WebKit
# left in $TMPDIR/WebKitPDFs-*. Results:
# specs/007-document-preview/spike-webview-pdf.md.
#
#   bash src-tauri/examples/pdf_spike_mac.sh NAME [VAR=1 ...]
#
# Run it in a macOS VM (tart) with a logged-in desktop, over SSH or from a
# Terminal there. A GUI app started over SSH lands in launchd's Background
# session and never gets a window, so over SSH the script starts only the
# spike through Terminal (`open -a Terminal`) and posts input and takes
# screenshots itself. Whichever runs those (sshd-session or Terminal) needs
# Accessibility and Screen Recording. Open in Preview opens the document in
# Preview, and the unhardened run leaves a decrypted copy in $TMPDIR.
# SPIKE_SCAN_HOME=1 also searches HOME (the VM's, never a real one; the
# default is only the temp and cache dirs).
#
# NAME labels the outputs in e2e/screenshots-out/pdf-spike/; the VARs are the
# example's SPIKE_* settings.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=$ROOT/e2e/screenshots-out/pdf-spike
NAME=${1:-default}
shift || true
mkdir -p "$OUT"
cd "$ROOT/src-tauri"
cargo build --example pdf_spike 2>&1 | tail -1

S=$(mktemp -d "${TMPDIR:-/tmp}/spike-XXXX")
mkdir -p "$S/dl"
swiftc -O -o "$S/input" "$ROOT/src-tauri/examples/pdf_spike_input.swift"
input() { "$S/input" "$@"; }
shot() { screencapture -x "$OUT/$NAME-$1.png"; }
ls -d "$TMPDIR"/WebKitPDFs-* > "$S/pdfs-before" 2>/dev/null
touch "$S/stamp"
sleep 1

LOG=$OUT/$NAME.log
: > "$LOG"
RUN=(env SPIKE_DOWNLOAD_DIR="$S/dl" SPIKE_EXIT_SECONDS=60 "$@"
  "$ROOT/src-tauri/target/debug/examples/pdf_spike")
if [ "$(launchctl managername)" = Background ]; then
  { echo '#!/bin/bash'; printf '%q ' "${RUN[@]}"; printf '>> %q 2>&1 &\necho $! > %q\nwait\n' "$LOG" "$S/pid"; } > "$S/run.command"
  chmod +x "$S/run.command"
  open -a Terminal "$S/run.command"
  until [ -s "$S/pid" ]; do sleep 0.5; done
  P=$(cat "$S/pid")
else
  "${RUN[@]}" >> "$LOG" 2>&1 &
  P=$!
fi
sleep 10
# Points from the window's top left: the window is 1000x800 and the page's
# layout is fixed, so these hold wherever macOS puts the window.
read -r X Y _ < <(input bounds $P)
at() { echo $((X + $1)) $((Y + $2)); }
input click $(at 840 700) # focus, on the window's margin
input text $P > /dev/null # the first query turns on the full tree
shot 1
input move $(at 490 600); sleep 0.3; input move $(at 500 720); sleep 0.8
shot 2-hover
input click $(at 580 728); echo SPIKE clicked the HUD Download >> "$LOG"
sleep 2
input move $(at 490 600); sleep 0.3; input move $(at 500 720); sleep 0.8
input click $(at 530 728); echo SPIKE clicked the HUD Open in Preview >> "$LOG"
sleep 3
input click $(at 840 700)
input rclick $(at 640 400); sleep 1.5
shot 3-menu
input click $(at 740 410); echo SPIKE clicked the menu Open with Preview >> "$LOG"
sleep 3
input click $(at 840 700) # closes the menu, if it is still open
input click $(at 340 218); echo SPIKE clicked the link >> "$LOG"
sleep 2
shot 4
input text $P | grep -v "AXMenuItem\|AXMenuBarItem" | sed 's/^/SPIKE AX /' >> "$LOG"
osascript -e 'quit app "Preview"' 2>/dev/null
while kill -0 "$P" 2>/dev/null; do sleep 1; done
echo "SPIKE process ended" >> "$LOG"

SCAN=("$(dirname "$TMPDIR")" /tmp /private/var/tmp)
[ "${SPIKE_SCAN_HOME:-0}" = 1 ] && SCAN+=("$HOME")
PRUNE=(\( -path "$ROOT/src-tauri/target" -o -path "$ROOT/node_modules" -o -path "$OUT" -o -path "$S" \) -prune -o)
{
  echo "== files written during the run =="
  find "${SCAN[@]}" "${PRUNE[@]}" -type f -newer "$S/stamp" -print 2>/dev/null
  echo "== of those, files holding the marker =="
  find "${SCAN[@]}" "${PRUNE[@]}" -type f -newer "$S/stamp" -print0 2>/dev/null | xargs -0 grep -l -a HDSPIKEMARKER 2>/dev/null
  echo "== WebKitPDFs dirs made during the run =="
  ls -d "$TMPDIR"/WebKitPDFs-* 2>/dev/null | grep -vxFf "$S/pdfs-before"
} > "$OUT/$NAME.disk.txt"
grep -a "SPIKE" "$LOG" | grep -v "protocol request"
sed -n '/holding the marker/,$p' "$OUT/$NAME.disk.txt"
