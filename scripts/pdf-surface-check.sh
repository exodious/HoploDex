#!/usr/bin/env bash
# The PDF surface check on Linux (specs/007-document-preview/research.md §23):
# builds src-tauri/examples/pdf_surface_check.rs and runs it under Xvfb with
# throwaway HOME and XDG folders and real XTest input. It shows the surface a
# hostile PDF, a truncated and a bit-flipped one and a 10 MB one, probes what a
# frame can reach, clicks, scrolls and presses keys, and fails if anything
# reached the network, a command or the disk. It prints the 10 MB PDF's time to
# first paint. Run it in the dev container:
#
#   scripts/dev-container.sh scripts/pdf-surface-check.sh [shows] [EXAMPLE FLAGS...]
#
# `shows` runs the example's first mode (the 3-page PDF at the given bounds, a
# screenshot) instead. The log and the screenshot go to
# e2e/screenshots-out/pdf-surface/{check,shows}-linux.{log,png}. Exits with the
# example's code (0 passed, 1 no window, 2 no load, 3 usage, 4 a check failed).
#
# WebKit's own sandbox can't be tested in the container (podman blocks the /proc
# mount bwrap needs, and WebKit skips bwrap where /run/.containerenv exists),
# so run it on a host for the sandbox on; there the disk scan covers only the
# temp folders and the run's scratch folder, never HOME.
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=$ROOT/e2e/screenshots-out/pdf-surface
MODE=check
if [ "${1:-}" = shows ]; then
  MODE=shows
  shift
fi
mkdir -p "$OUT"
cd "$ROOT/src-tauri" || exit 1
cargo build --example pdf_surface_check || exit 1

S=$(mktemp -d /tmp/surface-XXXX)
mkdir -p "$S"/{home,data,cache,config,state,runtime}
chmod 700 "$S/runtime"
cc -shared -fPIC -o "$S/dnslog.so" "$ROOT/src-tauri/examples/pdf_surface_dns.c" -ldl || exit 1
: > "$S/dns.log"

# Every folder the run could write a copy of the document to. In the container
# that is the whole filesystem (HOME is the run's own), on a host only the
# temp folders: the build, the dependencies and the outputs are left out.
if [ "$(hostname)" = hoplodex-dev ]; then SCAN=(/); else SCAN=(/tmp /var/tmp /dev/shm); fi
SCAN_ARGS=()
for dir in "${SCAN[@]}"; do SCAN_ARGS+=(--scan "$dir"); done
SKIP_ARGS=(--skip "$ROOT/src-tauri/target" --skip "$ROOT/node_modules" --skip "$OUT")

LOG=$OUT/$MODE-linux.log
if [ "$MODE" = check ]; then
  FLAGS=(--input python3 --input "$ROOT/src-tauri/examples/pdf_surface_input.py"
    --dns-log "$S/dns.log" "${SCAN_ARGS[@]}" "${SKIP_ARGS[@]}")
else
  FLAGS=()
fi
env -i PATH="$PATH" HOME="$S/home" XDG_DATA_HOME="$S/data" XDG_CACHE_HOME="$S/cache" \
  XDG_CONFIG_HOME="$S/config" XDG_STATE_HOME="$S/state" XDG_RUNTIME_DIR="$S/runtime" \
  xvfb-run -a -s "-screen 0 1280x1000x24" \
  env HD_DNS_LOG="$S/dns.log" LD_PRELOAD="$S/dnslog.so" \
  "$ROOT/src-tauri/target/debug/examples/pdf_surface_check" "$MODE" \
  --scratch "$S/scratch" --screenshot "$OUT/$MODE-linux.png" "${FLAGS[@]}" "$@" > "$LOG" 2>&1
CODE=$?
echo "exit $CODE" >> "$LOG"
grep -a "CHECK\|^exit" "$LOG" | grep -av "protocol request\|CHECK report\|page-load"
rm -rf "$S"
exit $CODE
