#!/usr/bin/env bash
# Runs examples/pdf_spike.rs on Linux under Xvfb with throwaway HOME and XDG
# dirs, clicks the PDF's link and the viewer's Save button with real input,
# then lists every file written during the run and any that holds the PDF's
# marker. Run it in the dev container (specs/007-document-preview/
# spike-webview-pdf.md):
#
#   scripts/dev-container.sh bash src-tauri/examples/pdf_spike.sh NAME [VAR=1 ...]
#
# WebKit's own sandbox can't be tested in the container (podman blocks the
# /proc mount bwrap needs, and WebKit skips bwrap where /run/.containerenv
# exists), so for WEBKIT_FORCE_SANDBOX=1 run the same command on the host.
# On the host the disk check covers only the temp dirs, never HOME.
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

S=$(mktemp -d /tmp/spike-XXXX)
mkdir -p "$S"/{home,data,cache,config,state,runtime,dl}
chmod 700 "$S/runtime"
cc -shared -fPIC -o "$S/dnslog.so" "$ROOT/src-tauri/examples/pdf_spike_dns.c" -ldl
echo "canary" > "$S/home/CANARY.txt"
touch "$S/stamp"
sleep 1

# The window sits at 0,0 under Xvfb: the link is at 290,317 and the viewer's
# Save button at 950,16.
env -i PATH="$PATH" HOME="$S/home" XDG_DATA_HOME="$S/data" XDG_CACHE_HOME="$S/cache" \
  XDG_CONFIG_HOME="$S/config" XDG_STATE_HOME="$S/state" XDG_RUNTIME_DIR="$S/runtime" \
  SPIKE_DOWNLOAD_DIR="$S/dl" LD_PRELOAD="$S/dnslog.so" "$@" \
  xvfb-run -a -s "-screen 0 1280x1000x24" bash -c "
    $ROOT/src-tauri/target/debug/examples/pdf_spike &
    P=\$!
    sleep 9; import -window root $OUT/$NAME-1.png
    bash $ROOT/src-tauri/examples/pdf_spike_procs.sh \$P $S/home/CANARY.txt
    python3 $ROOT/e2e/scripts/x11-input.py move 290 317; python3 $ROOT/e2e/scripts/x11-input.py click
    echo SPIKE clicked link
    sleep 3; import -window root $OUT/$NAME-2.png
    python3 $ROOT/e2e/scripts/x11-input.py move 950 16; python3 $ROOT/e2e/scripts/x11-input.py click
    echo SPIKE clicked save
    sleep 4; import -window root $OUT/$NAME-3.png
    wait \$P
  " > "$OUT/$NAME.log" 2>&1
echo "exit $?" >> "$OUT/$NAME.log"

PRUNE=(\( -path /proc -o -path /sys -o -path "$ROOT/src-tauri/target" -o -path "$ROOT/node_modules" -o -path "$OUT" \) -prune -o)
if [ "$(hostname)" = hoplodex-dev ]; then SCAN=(/); else SCAN=(/tmp /var/tmp /dev/shm); fi
{
  echo "== files written during the run =="
  find "${SCAN[@]}" "${PRUNE[@]}" -type f -newer "$S/stamp" -print 2>/dev/null | grep -v mesa_shader_cache_db/part
  echo "== of those, files holding the marker =="
  find "${SCAN[@]}" "${PRUNE[@]}" -type f -newer "$S/stamp" -print0 2>/dev/null | xargs -0 grep -l -a HDSPIKEMARKER 2>/dev/null
} > "$OUT/$NAME.disk.txt"
grep -a "SPIKE" "$OUT/$NAME.log" | grep -v "protocol request\|SPIKE DNS"
echo "== hostnames looked up (all processes) =="
grep -a "SPIKE DNS" "$OUT/$NAME.log" | awk '{print $4}' | sort | uniq -c
sed -n '/holding the marker/,$p' "$OUT/$NAME.disk.txt"
