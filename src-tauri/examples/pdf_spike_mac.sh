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
#
# SPIKE_MAC_FLOW (from the environment, like SPIKE_SCAN_HOME) runs the
# cleanup tests instead, with the HUD and the menu on. Each also records what
# Preview shows and remembers (its windows, File > Open Recent with a
# screenshot, the two Recent Documents lists' strings, the files it holds
# open) and scans the QuickLook cache, Preview's container and Spotlight:
# - names: the default clicks, then that record (what the copy is named).
# - close: the default clicks, then the record, then it closes the preview
#   (SPIKE_CLOSE_FILE; pass SPIKE_SWEEP=1 to delete the copies on close)
#   and records again, after paging through the document in Preview, after
#   quitting Preview and after starting it again.
# - watch-hud, watch-menu: only the HUD's Open in Preview, or only the
#   menu's Open with Preview, logging when (Unix ms) the click and Preview's
#   process happen (pass SPIKE_WATCH=1), then the record.
# SPIKE_MAC_RESET=1 first quits Preview, deletes every WebKitPDFs-* dir and
# clears Recent Documents and Preview's recent list: the VM's, never a real
# one.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=$ROOT/e2e/screenshots-out/pdf-spike
NAME=${1:-default}
shift || true
mkdir -p "$OUT"
FLOW=${SPIKE_MAC_FLOW:-}
SFL=$HOME/Library/Application\ Support/com.apple.sharedfilelist
RECENTS=("$SFL/com.apple.LSSharedFileList.RecentDocuments.sfl4"
  "$SFL/com.apple.LSSharedFileList.ApplicationRecentDocuments/com.apple.preview.sfl4")
if [ "${SPIKE_MAC_RESET:-0}" = 1 ]; then
  pkill -x Preview; sleep 1
  rm -rf "$TMPDIR"/WebKitPDFs-*
  killall sharedfilelistd 2>/dev/null
  rm -f "${RECENTS[@]}"
fi
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
EXTRA=()
[ "$FLOW" = close ] && EXTRA=(SPIKE_CLOSE_FILE="$S/close")
RUN=(env SPIKE_DOWNLOAD_DIR="$S/dl" SPIKE_EXIT_SECONDS=60 ${EXTRA[@]+"${EXTRA[@]}"} "$@"
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
ms() { perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000'; }
if [ -n "$FLOW" ]; then
  : > "$OUT/$NAME.record.txt"
  # When Preview's process starts (or that it was already running).
  { while ! pgrep -x Preview > /dev/null; do sleep 0.005; done
    echo "SPIKE Preview process seen at $(ms)" >> "$LOG"; } &
  WATCHER=$!
fi
sleep 10
# Points from the window's top left: the window is 1000x800 and the page's
# layout is fixed, so these hold wherever macOS puts the window.
read -r X Y _ < <(input bounds $P)
at() { echo $((X + $1)) $((Y + $2)); }
input click $(at 840 700) # focus, on the window's margin
input text $P > /dev/null # the first query turns on the full tree
shot 1

# What Preview shows and remembers, under the label $1.
record() {
  local pv; pv=$(pgrep -x Preview | head -1)
  {
    echo "== $1 at $(ms) =="
    echo "-- WebKitPDFs dirs made during the run"
    for d in $(ls -d "$TMPDIR"/WebKitPDFs-* 2>/dev/null | grep -vxFf "$S/pdfs-before"); do ls -la "$d"; done
    echo "-- Preview: ${pv:-not running}"
    if [ -n "$pv" ]; then
      echo "-- Preview's windows and their text"
      input text "$pv" | grep -v "AXMenuItem\|AXMenuBarItem\|AXMenu \|HDSPIKEMARKER-PAD" | head -40
      echo "-- File > Open Recent"
      input menu "$pv" | grep "Open Recent"
      echo "-- files Preview holds open"
      lsof -p "$pv" 2> /dev/null | grep -i "WebKitPDFs\|\.pdf"
    fi
    echo "-- Recent Documents lists: strings naming a PDF, WebKitPDFs or a marker"
    for f in "${RECENTS[@]}"; do
      echo "$(basename "$f"):"
      [ -f "$f" ] && strings "$f" | grep -n "WebKitPDFs\|\.pdf\|MARKER\|$TOKEN"
    done
  } >> "$OUT/$NAME.record.txt" 2>&1
  [ -n "$pv" ] || return 0
  open -a Preview; sleep 1.5
  shot "$1"
  local at; at=$(input find "$pv" AXMenuBarItem File) || return 0
  input click $at; sleep 0.8
  at=$(input find "$pv" AXMenuItem "Open Recent") && { input move $at; sleep 1.5; }
  shot "$1-recent"
  input key 53; input key 53; sleep 0.3
}
quit_preview() {
  local pv; pv=$(pgrep -x Preview | head -1)
  [ -n "$pv" ] || return 0
  echo "SPIKE quitting Preview: $(input press "$pv" "Quit Preview")" >> "$LOG"
  sleep 3
}
TOKEN=$(grep -o "pdf_path=[0-9a-f]*" "$LOG" | cut -d= -f2)
TOKEN=${TOKEN:-NO-TOKEN}

case "$FLOW" in
watch-hud | watch-menu)
  if [ "$FLOW" = watch-hud ]; then
    input move $(at 490 600); sleep 0.3; input move $(at 500 720); sleep 0.8
    echo "SPIKE clicked the HUD Open in Preview at $(input clickt $(at 530 728))" >> "$LOG"
  else
    input rclick $(at 640 400); sleep 1.5
    echo "SPIKE clicked the menu Open with Preview at $(input clickt $(at 740 410))" >> "$LOG"
  fi
  sleep 4
  shot 2-after-click
  record watch
  quit_preview
  ;;
*)
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
  ;;
esac
case "$FLOW" in
names) record names; quit_preview ;;
close)
  record open
  touch "$S/close"; sleep 3
  record closed
  # Page through the document in Preview, as someone still reading would.
  pv=$(pgrep -x Preview | head -1)
  if [ -n "$pv" ]; then
    open -a Preview; sleep 1
    read -r PX PY PW PH < <(input bounds "$pv")
    input click $((PX + PW / 2)) $((PY + PH / 2))
    for _ in 1 2 3 4; do input key 121; sleep 0.5; done # Page Down
    shot closed-paged
    input key 115; sleep 0.5 # Home
  fi
  quit_preview
  shot closed-quit
  record closed-quit
  pkill -x Preview; sleep 1
  open -a Preview; sleep 4
  record relaunched
  quit_preview
  ;;
esac
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
if [ -n "$FLOW" ]; then
  kill "$WATCHER" 2>/dev/null
  T=$(dirname "$TMPDIR")
  PREVIEW_DATA=$HOME/Library/Containers/com.apple.Preview
  {
    echo "== QuickLook caches: files new or changed during the run =="
    find "$T"/C/com.apple.quicklook.* "$T"/T/com.apple.quicklook.* -newer "$S/stamp" -ls 2>/dev/null
    echo "== Preview's container: files new or changed during the run =="
    find "$PREVIEW_DATA" -newer "$S/stamp" -ls 2>/dev/null
    echo "== Preview's container: any file holding the marker =="
    grep -rl -a HDSPIKEMARKER "$PREVIEW_DATA" 2>/dev/null
    echo "== Spotlight =="
    mdutil -s / 2>&1
    echo "mdfind HDSPIKEMARKER:"; mdfind HDSPIKEMARKER 2>&1
    echo "mdfind -name doc.pdf / document.pdf / HDNAMEMARKER:"
    mdfind -name doc.pdf 2>&1; mdfind -name document.pdf 2>&1; mdfind -name HDNAMEMARKER 2>&1
  } >> "$OUT/$NAME.disk.txt"
fi
grep -a "SPIKE" "$LOG" | grep -v "protocol request"
sed -n '/holding the marker/,$p' "$OUT/$NAME.disk.txt"
