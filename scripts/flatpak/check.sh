#!/usr/bin/env bash
# Runs the automated Flatpak check in its sandbox (npm run test:flatpak).
# 011 research.md §15, §16, §18; 011 contracts/flatpak-bundle.md "Project commands".
#
# Runs in `scripts/dev-container.sh --flatpak` only (env.sh's host guard refuses
# anywhere else unless FLATPAK_USER_DIR names a throwaway installation and
# --i-know-this-is-a-host is passed), after `npm run build` (the check builds the
# Flatpak from the checkout, and the e2e build for the cold-start comparison embeds dist/).
#
#   1. builds the check variant (scripts/flatpak/checkManifest.mjs) offline and
#      installs it into the container's Flatpak installation;
#   2. the bus checks, with the permissions the Flatpak ships with: the stand-in
#      Secret Service, both screen savers and the portal answer on a private
#      session bus, logind answers on the private system bus, and a name the
#      manifest does not grant is refused (011 FR-005, 011 FR-006);
#   3. the no-network check (011 FR-021): the shipped permissions have no network;
#   4. builds the `e2e` binary outside the sandbox and runs
#      e2e/specs/flatpak.e2e.ts through WebDriver, whose cold-start test compares
#      the two (011 SC-006, 011 research.md §18);
#   5. uninstalls the check variant whatever happened (a trap), and fails unless the
#      installation lists the same apps as before.
#
# Logs: flatpak/out/check/. The run's HOME is a throwaway: nothing here makes a
# ~/.var/app in the container user's real home (011 SC-007).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cd "$repo"

guard_args=()
for arg in "$@"; do
  case "$arg" in
    --fresh-install)
      # The shipped .flatpak into an empty installation (011 tasks.md T067).
      echo "check.sh: --fresh-install is not yet implemented." >&2
      exit 2
      ;;
    --i-know-this-is-a-host) guard_args+=("$arg") ;;
    *)
      echo "check.sh: unknown argument '$arg' (use --fresh-install, once implemented, or --i-know-this-is-a-host)." >&2
      exit 2
      ;;
  esac
done

# env.sh: the host guard (it refuses here, before any `flatpak` command, anywhere
# but the dev container), FLATPAK_USER_DIR, and the private system bus.
# shellcheck source=scripts/flatpak/env.sh
source "$here/env.sh" ${guard_args[@]+"${guard_args[@]}"}
# What support/flatpak.ts requires: this script has run the guard.
export HOPLODEX_FLATPAK_CHECK=1

app_id=io.github.exodious.HoploDex
state="${HOPLODEX_FLATPAK_STATE:?HOPLODEX_FLATPAK_STATE is not set; run through scripts/dev-container.sh --flatpak}"
log_dir="$repo/flatpak/out/check"
mkdir -p "$log_dir"
rm -f "$log_dir"/*.log
exec > >(tee "$log_dir/check.log") 2>&1

current_step=""
step() {
  current_step="$1"
  printf '\n==> %s\n' "$current_step"
}
fail() {
  printf 'check.sh: FAILED at step "%s": %s\n' "$current_step" "$1" >&2
  exit 1
}

scratch="$(mktemp -d "${TMPDIR:-/tmp}/hoplodex-flatpak-check.XXXXXX")"
# The bus checks' own session bus and stand-ins (the E2E run starts its own).
check_bus_pid=""
mock_pids=()
installed_before="__unset__"

stop_standins() {
  local pid
  for pid in ${mock_pids[@]+"${mock_pids[@]}"}; do kill "$pid" 2>/dev/null || true; done
  mock_pids=()
  if [ -n "$check_bus_pid" ]; then kill "$check_bus_pid" 2>/dev/null || true; check_bus_pid=""; fi
}

list_apps() { flatpak list --user --app --columns=application | sort; }

# Always, whatever happened: stop the stand-ins and the sandbox, uninstall the check
# variant and its remote, and compare the installation's apps with what they were. The
# bus env.sh started is stopped last (an uninstall may want it). This replaces env.sh's
# own EXIT trap, so it ends with that trap's function.
cleanup() {
  local status=$?
  trap - EXIT ERR
  set +e
  stop_standins
  flatpak kill "$app_id" >/dev/null 2>&1
  flatpak uninstall --user --noninteractive "$app_id" >>"$log_dir/uninstall.log" 2>&1
  flatpak remote-delete --user --force hoplodex-check >>"$log_dir/uninstall.log" 2>&1
  if [ "$installed_before" != "__unset__" ]; then
    local after
    after="$(list_apps 2>>"$log_dir/uninstall.log")"
    if [ "$after" != "$installed_before" ]; then
      echo "check.sh: the installation's apps differ from before the check:" >&2
      echo "  before: $(printf '%s' "${installed_before:-none}" | tr '\n' ' ')" >&2
      echo "  after:  $(printf '%s' "${after:-none}" | tr '\n' ' ')" >&2
      status=1
    else
      echo "uninstalled: the installation lists the same apps as before ($(printf '%s' "${installed_before:-none}" | tr '\n' ' '))"
    fi
  fi
  rm -rf "$scratch"
  hoplodex_flatpak_stop_bus
  if [ "$status" -ne 0 ]; then
    echo "check.sh: FAILED (step: ${current_step:-none}); logs are in flatpak/out/check/" >&2
  fi
  exit "$status"
}
trap cleanup EXIT
set -E
trap 'echo "check.sh: command failed at step: $current_step" >&2' ERR

# A `flatpak run` of the check variant under a throwaway home with a private session
# bus, as the E2E run starts it: no XDG_* folders of this shell, no network.
check_home="$scratch/home"
mkdir -p "$check_home/.config" "$check_home/Documents"
sandbox_run() {
  env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u XDG_CACHE_HOME -u XDG_STATE_HOME \
    HOME="$check_home" DBUS_SESSION_BUS_ADDRESS="${check_session_bus:-}" \
    flatpak run "$@"
}

# ---------------------------------------------------------------------------
step "the installation before the check"
# Whatever HoploDex build is installed (build.sh leaves one) is replaced by the check
# variant and removed at the end, so it is not part of "before".
if flatpak info --user "$app_id" >/dev/null 2>&1; then
  echo "a build of $app_id is installed in this installation; the check replaces and removes it"
  flatpak uninstall --user --noninteractive "$app_id"
fi
installed_before="$(list_apps)"
echo "apps installed: $(printf '%s' "${installed_before:-none}" | tr '\n' ' ')"

# ---------------------------------------------------------------------------
step "build and install the check variant (011 research.md §16)"
node --import tsx --input-type=module -e '
  const checkout = await import("./e2e/support/flatpak.ts");
  checkout.buildAndInstallCheck();
'
# The E2E run installs nothing of its own.
export HOPLODEX_FLATPAK_CHECK_INSTALLED=1

# ---------------------------------------------------------------------------
step "the private session bus and the stand-in services"
cat >"$scratch/session.conf" <<CONF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
  "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path=$scratch/session_bus_socket</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
CONF
bus_out="$(dbus-daemon --config-file="$scratch/session.conf" --fork --print-address --print-pid)"
check_session_bus="$(printf '%s\n' "$bus_out" | sed -n 1p)"
check_bus_pid="$(printf '%s\n' "$bus_out" | sed -n 2p)"

name_owned() { # bus-address name
  gdbus call --address "$1" --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.NameHasOwner "$2" 2>/dev/null | grep -q true
}

# start_mock <bus-address> <bus-flag> <name> <dbusmock arguments...>
start_mock() {
  local address="$1" flag="$2" name="$3"
  shift 3
  local var=DBUS_SESSION_BUS_ADDRESS
  [ "$flag" = --system ] && var=DBUS_SYSTEM_BUS_ADDRESS
  env "$var=$address" python3 -m dbusmock "$flag" "$@" >"$log_dir/standin-check-$name.log" 2>&1 &
  mock_pids+=("$!")
  local tries=0
  until name_owned "$address" "$name"; do
    tries=$((tries + 1))
    [ "$tries" -lt 300 ] || fail "the stand-in for $name never took its name (flatpak/out/check/standin-check-$name.log)"
    sleep 0.05
  done
  echo "stand-in up: $name"
}

system_bus="$DBUS_SYSTEM_BUS_ADDRESS"
start_mock "$system_bus" --system org.freedesktop.login1 -t logind
gdbus call --address "$system_bus" --dest org.freedesktop.login1 --object-path /org/freedesktop/login1 \
  --method org.freedesktop.DBus.Mock.AddSession auto seat0 "$(id -u)" "$(id -un)" true >/dev/null
start_mock "$check_session_bus" --session org.freedesktop.secrets org.freedesktop.secrets /org/freedesktop/secrets org.freedesktop.Secret.Service
start_mock "$check_session_bus" --session org.gnome.ScreenSaver org.gnome.ScreenSaver /org/gnome/ScreenSaver org.gnome.ScreenSaver
start_mock "$check_session_bus" --session org.freedesktop.ScreenSaver org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver
start_mock "$check_session_bus" --session org.freedesktop.portal.Desktop org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop org.freedesktop.portal.OpenURI
start_mock "$check_session_bus" --session org.example.NotGranted org.example.NotGranted /org/example/NotGranted org.example.NotGranted

# ---------------------------------------------------------------------------
step "the bus checks, from inside the sandbox with the shipped permissions (011 research.md §16)"
ping_inside() { # name path
  sandbox_run --command=gdbus "$app_id" call --session --dest "$1" --object-path "$2" \
    --method org.freedesktop.DBus.Peer.Ping
}
for pair in \
  "org.freedesktop.secrets /org/freedesktop/secrets" \
  "org.gnome.ScreenSaver /org/gnome/ScreenSaver" \
  "org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver" \
  "org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop"; do
  # shellcheck disable=SC2086
  set -- $pair
  ping_inside "$1" "$2" >/dev/null || fail "$1 does not answer on the session bus from inside the sandbox"
  echo "answers on the session bus: $1"
done

can_suspend="$(sandbox_run --command=gdbus "$app_id" call --system --dest org.freedesktop.login1 \
  --object-path /org/freedesktop/login1 --method org.freedesktop.login1.Manager.CanSuspend)" ||
  fail "logind does not answer on the system bus from inside the sandbox"
echo "logind answers on the system bus: $can_suspend"
sandbox_run --command=gdbus "$app_id" call --system --dest org.freedesktop.login1 \
  --object-path /org/freedesktop/login1/session/auto --method org.freedesktop.DBus.Properties.Get \
  org.freedesktop.login1.Session LockedHint >/dev/null ||
  fail "logind's session/auto does not answer from inside the sandbox"
echo "logind's session/auto answers"
# The delay inhibitor's file descriptor passes through the bus proxy (011 research.md §6).
sandbox_run --command=gdbus "$app_id" call --system --dest org.freedesktop.login1 \
  --object-path /org/freedesktop/login1 --method org.freedesktop.login1.Manager.Inhibit \
  sleep HoploDex "the Flatpak check" delay >/dev/null ||
  fail "logind's Inhibit, which returns a file descriptor, failed from inside the sandbox"
echo "logind's Inhibit returns a descriptor through the proxy"

# A name the manifest doesn't grant: it answers outside the sandbox, and not inside.
gdbus call --address "$check_session_bus" --dest org.example.NotGranted --object-path /org/example/NotGranted \
  --method org.freedesktop.DBus.Peer.Ping >/dev/null ||
  fail "the stand-in org.example.NotGranted does not answer outside the sandbox, so its refusal shows nothing"
if ping_inside org.example.NotGranted /org/example/NotGranted >"$scratch/not-granted.out" 2>&1; then
  fail "org.example.NotGranted, which the manifest does not grant, answered from inside the sandbox"
fi
echo "refused inside the sandbox, answering outside: org.example.NotGranted ($(tr '\n' ' ' <"$scratch/not-granted.out"))"

# ---------------------------------------------------------------------------
step "the no-network check (011 FR-021, 011 research.md §16)"
address="$(ip -4 -o addr show scope global 2>/dev/null | awk '{print $4}' | cut -d/ -f1 | head -n 1)"
[ -n "$address" ] ||
  fail "the container has no non-loopback IPv4 address, so a missing network inside the sandbox would show nothing; start it with a network"
python3 - >"$scratch/port" <<'PY' &
import socket, sys, time
server = socket.socket()
server.bind(("0.0.0.0", 0))
server.listen(5)
print(server.getsockname()[1], flush=True)
time.sleep(120)
PY
mock_pids+=("$!")
tries=0
until [ -s "$scratch/port" ]; do
  tries=$((tries + 1))
  [ "$tries" -lt 100 ] || fail "the listener did not start"
  sleep 0.05
done
port="$(cat "$scratch/port")"
connect='
import socket, sys
try:
    socket.create_connection((sys.argv[1], int(sys.argv[2])), 2).close()
    print("connected")
except OSError as error:
    print("not connected: %s" % error)
'
outside="$(python3 -I -c "$connect" "$address" "$port")"
[ "$outside" = connected ] || fail "the listener at $address:$port refused a connection from the container itself ($outside)"
echo "from the container: $address:$port $outside"
inside="$(sandbox_run --command=python3 "$app_id" -c "
import socket, sys
print('interfaces=' + ','.join(name for _, name in socket.if_nameindex()))
$connect" "$address" "$port")"
echo "$inside" | sed 's/^/from the sandbox: /'
echo "$inside" | grep -qx 'interfaces=lo' || fail "the sandbox, with the shipped permissions, has an interface other than the loopback"
echo "$inside" | grep -q '^not connected' || fail "the sandbox, with the shipped permissions, connected to $address:$port"
stop_standins

# ---------------------------------------------------------------------------
step "the e2e build outside the sandbox, for the cold-start comparison (011 SC-006, 011 research.md §18)"
[ -f dist/index.html ] || fail "dist/ is empty: run 'npm run build' first (the e2e build embeds it)"
node --import tsx --input-type=module -e '
  const build = await import("./e2e/support/build.ts");
  process.exit(build.buildApp() ? 0 : 1);
'

# ---------------------------------------------------------------------------
step "the Flatpak journey: HOPLODEX_E2E_FLATPAK=1 wdio run e2e/wdio.conf.ts"
HOPLODEX_E2E_FLATPAK=1 node e2e/run-e2e.mjs

echo
echo "check.sh: ok (the check variant is uninstalled next)"
