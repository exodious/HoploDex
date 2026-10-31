#!/usr/bin/env bash
# The Flatpak check's real Secret Service (011 research.md §6, T102): a headless
# gnome-keyring-daemon, unlocked, on the check's private session bus, with its data
# under the check's throwaway home. It replaces the stand-in the check used before, so
# the app's real Secret Service backend (keyring 4, zbus) talks to a real keyring over
# the same permissions the Flatpak ships with.
#
#   keyring-daemon.sh <session-bus-address> <data-folder>
#
# It runs in the foreground (the caller starts it in the background and kills it to
# stop the keyring) and becomes the daemon, so the caller's process id is the daemon's.
#
# It must never reach the developer's keyring:
#   - it runs only with HOPLODEX_FLATPAK_CHECK=1 (check.sh sets it after the host guard);
#   - the daemon starts under `env -i` with nothing but PATH, HOME, XDG_DATA_HOME,
#     XDG_RUNTIME_DIR and the bus address it was given, so no GNOME_KEYRING_CONTROL,
#     SSH_AUTH_SOCK or DISPLAY of this shell reaches it, and its control socket and
#     keyrings are under <data-folder>;
#   - it refuses an address that is the session bus of the environment it was started
#     from (the container's own), and any that is not a unix socket path.
# The keyring is a "login" keyring with a fixed throwaway password: gnome-keyring
# creates it only for a password that is not empty, and nothing here is secret.
set -euo pipefail

if [ "${HOPLODEX_FLATPAK_CHECK:-}" != 1 ]; then
  echo "keyring-daemon.sh: run through scripts/flatpak/check.sh (npm run test:flatpak)." >&2
  exit 2
fi
if [ "$#" -ne 2 ]; then
  echo "usage: keyring-daemon.sh <session-bus-address> <data-folder>" >&2
  exit 2
fi
address="$1"
data="$2"

case "$address" in
  unix:path=/*) ;;
  *)
    echo "keyring-daemon.sh: '$address' is not the address of a private bus (unix:path=/...)." >&2
    exit 2
    ;;
esac
if [ "$address" = "${DBUS_SESSION_BUS_ADDRESS:-}" ]; then
  echo "keyring-daemon.sh: '$address' is the session bus this shell was started on; the check uses a private one." >&2
  exit 2
fi
case "$data" in
  /*) ;;
  *)
    echo "keyring-daemon.sh: the data folder must be an absolute path." >&2
    exit 2
    ;;
esac

mkdir -p "$data/share" "$data/runtime"
chmod 700 "$data/runtime"

# The password comes in on standard input, as gnome-keyring-daemon --unlock reads it.
exec env -i \
  PATH="$PATH" \
  HOME="$data" \
  XDG_DATA_HOME="$data/share" \
  XDG_RUNTIME_DIR="$data/runtime" \
  DBUS_SESSION_BUS_ADDRESS="$address" \
  gnome-keyring-daemon --unlock --components=secrets --foreground \
  < <(printf '%s' 'hoplodex-flatpak-check')
