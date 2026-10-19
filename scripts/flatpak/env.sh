#!/usr/bin/env bash
# Sourced first by every Flatpak script (011 research.md §15, §16): `source "$(dirname "$0")/env.sh" "$@"`.
#   1. runs the host guard (guard.mjs) before any `flatpak` command, so a host's own
#      Flatpak installation is never used (011 US5-3, 011 SC-007);
#   2. exports FLATPAK_USER_DIR, the accepted installation, and marks a throwaway one;
#      and HOPLODEX_FLATPAK_STATE, the container's state volume or, on a host, the one
#      the caller named or else a folder beside the throwaway installation (011 T104);
#   3. starts a private system bus and exports DBUS_SYSTEM_BUS_ADDRESS (Debian's
#      `flatpak` asks the system bus before it installs, and the container has none);
#   4. adds the Flathub remote to that installation, if it isn't there;
#   5. stops the bus when the script exits.
# The arguments the caller passes on (`"$@"`) reach the guard, which looks for
# --i-know-this-is-a-host.
set -euo pipefail

_hd_flatpak_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v node >/dev/null 2>&1; then
  echo "node is needed to run the Flatpak scripts' host guard, and it is not on PATH." >&2
  return 1 2>/dev/null || exit 1
fi

# The guard prints the accepted FLATPAK_USER_DIR, then `marker` or `no-marker`, then the
# state folder.
if ! _hd_guard_out="$(node "$_hd_flatpak_dir/guard.mjs" "$@")"; then
  # The guard has already printed its message on stderr.
  return 1 2>/dev/null || exit 1
fi
_hd_user_dir="$(printf '%s\n' "$_hd_guard_out" | sed -n 1p)"
_hd_write_marker="$(printf '%s\n' "$_hd_guard_out" | sed -n 2p)"
_hd_state="$(printf '%s\n' "$_hd_guard_out" | sed -n 3p)"

mkdir -p "$_hd_user_dir"
export FLATPAK_USER_DIR="$_hd_user_dir"
export HOPLODEX_FLATPAK_STATE="$_hd_state"
if [ "$_hd_write_marker" = "marker" ]; then
  # (the name is guard.mjs's MARKER_NAME)
  printf '%s\n' 'Made by HoploDex scripts/flatpak/env.sh: a throwaway Flatpak installation, safe to delete.' \
    >"$_hd_user_dir/.hoplodex-throwaway-flatpak"
fi

# The private system bus: a minimal config in a temporary folder, a socket in it.
_hd_bus_dir="$(mktemp -d "${TMPDIR:-/tmp}/hoplodex-flatpak-bus.XXXXXX")"
_hd_bus_pid=""

hoplodex_flatpak_stop_bus() {
  if [ -n "${_hd_bus_pid:-}" ]; then
    kill "$_hd_bus_pid" 2>/dev/null || true
    _hd_bus_pid=""
  fi
  rm -rf "${_hd_bus_dir:-}"
}

# Keep an EXIT trap the caller already set.
_hd_previous_trap="$(trap -p EXIT | sed -e "s/^trap -- '//" -e "s/' EXIT\$//")"
# shellcheck disable=SC2064
trap "hoplodex_flatpak_stop_bus; ${_hd_previous_trap}" EXIT

cat >"$_hd_bus_dir/system.conf" <<CONF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
  "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>system</type>
  <listen>unix:path=$_hd_bus_dir/system_bus_socket</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
CONF

# --fork detaches; --print-address and --print-pid give its two lines.
_hd_bus_out="$(dbus-daemon --config-file="$_hd_bus_dir/system.conf" --fork --print-address --print-pid)"
DBUS_SYSTEM_BUS_ADDRESS="$(printf '%s\n' "$_hd_bus_out" | sed -n 1p)"
_hd_bus_pid="$(printf '%s\n' "$_hd_bus_out" | sed -n 2p)"
export DBUS_SYSTEM_BUS_ADDRESS

# The remote is added to FLATPAK_USER_DIR's installation, never the system's.
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
