#!/usr/bin/env bash
# Entrypoint for the HoploDex development container (see Dockerfile). Sets up
# what a desktop login would normally provide, then runs the given command:
#
# - a private XDG_RUNTIME_DIR and session D-Bus, with gnome-keyring answering
#   the Secret Service API, so the app's real (non-mock) keyring works for
#   `tauri dev` and scripts/human-testing.sh. The keyring lives in the home
#   volume and is unlocked with a blank password, like the headless-host setup
#   in DEVELOPMENT.md.
# - the X pointer size, when HOPLODEX_XCURSOR_SIZE is set (see below).
# - first-run ownership of the named volumes, and `npm ci` into an empty
#   node_modules volume.
set -euo pipefail

if [[ -z "${XDG_RUNTIME_DIR:-}" ]]; then
  export XDG_RUNTIME_DIR="/tmp/runtime-$(id -u)"
fi
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

if [[ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ]]; then
  DBUS_SESSION_BUS_ADDRESS="$(dbus-daemon --session --fork --print-address=1)"
  export DBUS_SESSION_BUS_ADDRESS
  # --login only unlocks; a daemon that sees no --start within 120 seconds
  # exits, and the app's first keyring call (after a long cold build) would
  # then D-Bus-activate a fresh daemon with no login collection. --start
  # completes the session like a desktop login does.
  set -a
  eval "$(printf '\n' | gnome-keyring-daemon --login --daemonize --components=pkcs11,secrets 2>/dev/null)" || true
  eval "$(gnome-keyring-daemon --start --components=pkcs11,secrets 2>/dev/null)" || true
  set +a
fi

# An X server that sets no Xcursor.size (XQuartz) leaves libXcursor to size the
# pointer from the screen height, which is enormous on a 4K screen. Xlib merges
# the file XENVIRONMENT names into the X resources, so this needs no change to
# the server (an `xrdb -merge` would touch the host's other X apps).
if [[ -n "${HOPLODEX_XCURSOR_SIZE:-}" ]]; then
  printf 'Xcursor.size: %s\n' "$HOPLODEX_XCURSOR_SIZE" > "$XDG_RUNTIME_DIR/xresources"
  export XENVIRONMENT="$XDG_RUNTIME_DIR/xresources"
fi

# Named volumes can come up root-owned on first use, depending on the engine.
for dir in "$HOME" /workspace/node_modules /workspace/src-tauri/target; do
  if [[ -d "$dir" && ! -w "$dir" ]]; then
    sudo chown "$(id -u):$(id -g)" "$dir"
  fi
done

if [[ -f /workspace/package-lock.json && ! -f /workspace/node_modules/.package-lock.json ]]; then
  echo "hoplodex-dev: node_modules is empty, running npm ci (first run only)..." >&2
  (cd /workspace && npm ci --no-audit --no-fund) >&2
fi

exec "$@"
