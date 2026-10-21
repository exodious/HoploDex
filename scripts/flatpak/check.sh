#!/usr/bin/env bash
# Runs the automated Flatpak check in its sandbox (npm run test:flatpak).
# 011 research.md §15, §16, §18; 011 contracts/flatpak-bundle.md "Project commands".
#
# Runs in `scripts/dev-container.sh --flatpak` only: the guard is asked for the
# container alone (--container-only) before anything else, so a host is refused even
# with a throwaway FLATPAK_USER_DIR (011 T104; the check needs the container's
# stand-ins), after `npm run build` (the check builds the
# Flatpak from the checkout, and the e2e build for the cold-start comparison embeds dist/).
#
#   1. builds the check variant (scripts/flatpak/checkManifest.mjs) offline and
#      installs it into the container's Flatpak installation;
#   2. the bus checks, with the permissions the Flatpak ships with: both screen savers and
#      the desktop portal (scripted: openuri_mock.py, with its OpenURI and Secret
#      interfaces) answer on a private session bus, logind answers on the private system
#      bus, and a name the manifest does not grant is refused (011 FR-005, 011 FR-006).
#      The Secret Service (a real, headless gnome-keyring-daemon: keyring-daemon.sh) owns
#      its name on that bus and is refused to the sandbox as well (011 SC-011);
#   3. the no-network check (011 FR-021): the shipped permissions have no network;
#   4. builds the `e2e` binary and the keyring file reader outside the sandbox and runs
#      e2e/specs/flatpak.e2e.ts through WebDriver, whose cold-start test compares
#      the two (011 SC-006, 011 research.md §18);
#   5. uninstalls the check variant whatever happened (a trap), and fails unless the
#      installation lists the same apps as before.
#
# `--fresh-install` (network; before each release) runs a different check instead
# (011 US4-2, 011 SC-010, 011 FR-009): the shipped .flatpak file that `npm run
# bundle:flatpak` wrote, installed into a second, empty installation with no remotes,
# then started once under a throwaway home.
#
# Logs: flatpak/out/check/. The run's HOME is a throwaway: nothing here makes a
# ~/.var/app in the container user's real home (011 SC-007).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cd "$repo"

# The container alone, asked before anything else (a host's refusal says why).
node "$here/guard.mjs" --container-only >/dev/null

fresh_install=0
for arg in "$@"; do
  case "$arg" in
    --fresh-install) fresh_install=1 ;;
    *)
      echo "check.sh: unknown argument '$arg' (use --fresh-install)." >&2
      exit 2
      ;;
  esac
done

# env.sh: the host guard again (it refuses here, before any `flatpak` command, anywhere
# but the dev container), FLATPAK_USER_DIR, and the private system bus.
# shellcheck source=scripts/flatpak/env.sh
source "$here/env.sh"
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
# What the stand-in Secret portal hands the app (the journey makes its own).
check_secret="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
sandbox_run() {
  env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u XDG_CACHE_HOME -u XDG_STATE_HOME \
    HOME="$check_home" DBUS_SESSION_BUS_ADDRESS="${check_session_bus:-}" \
    flatpak run "$@"
}

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
  last_mock_pid="$!"
  mock_pids+=("$last_mock_pid")
  local tries=0
  until name_owned "$address" "$name"; do
    tries=$((tries + 1))
    [ "$tries" -lt 300 ] || fail "the stand-in for $name never took its name (flatpak/out/check/standin-check-$name.log)"
    sleep 0.05
  done
  echo "stand-in up: $name"
}

# start_service <bus-address> <name> <command...>: a real service (not a dbusmock stand-in)
# that takes <name> on the bus, started in the background and stopped with the stand-ins.
start_service() {
  local address="$1" name="$2"
  shift 2
  "$@" >"$log_dir/standin-check-$name.log" 2>&1 &
  last_mock_pid="$!"
  mock_pids+=("$last_mock_pid")
  local tries=0
  until name_owned "$address" "$name"; do
    tries=$((tries + 1))
    kill -0 "$last_mock_pid" 2>/dev/null || fail "the service for $name exited at start (flatpak/out/check/standin-check-$name.log)"
    [ "$tries" -lt 300 ] || fail "the service for $name never took its name (flatpak/out/check/standin-check-$name.log)"
    sleep 0.05
  done
  echo "service up: $name"
}

# The private session bus and the services both checks start the app against: dbusmock
# stand-ins for logind, both screen savers and a name the manifest does not grant, a real
# gnome-keyring-daemon for the Secret Service (which the Flatpak is not granted, 011 SC-011),
# and the scripted desktop portal with OpenURI and Secret (011 research.md §6, §7, T102).
start_standins() {
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

  system_bus="$DBUS_SYSTEM_BUS_ADDRESS"
  start_mock "$system_bus" --system org.freedesktop.login1 -t logind
  gdbus call --address "$system_bus" --dest org.freedesktop.login1 --object-path /org/freedesktop/login1 \
    --method org.freedesktop.DBus.Mock.AddSession auto seat0 "$(id -u)" "$(id -un)" true >/dev/null
  # The real Secret Service, unlocked, on the private bus only, its data in the check's
  # scratch folder (keyring-daemon.sh refuses the container's own session bus).
  start_service "$check_session_bus" org.freedesktop.secrets \
    "$here/keyring-daemon.sh" "$check_session_bus" "$scratch/keyring"
  start_mock "$check_session_bus" --session org.gnome.ScreenSaver org.gnome.ScreenSaver /org/gnome/ScreenSaver org.gnome.ScreenSaver
  start_mock "$check_session_bus" --session org.freedesktop.ScreenSaver org.freedesktop.ScreenSaver /org/freedesktop/ScreenSaver org.freedesktop.ScreenSaver
  start_mock "$check_session_bus" --session org.freedesktop.portal.Desktop \
    -t "$here/openuri_mock.py" -p "{\"log\": \"$log_dir/openuri-check.log\", \"opened\": \"$log_dir/openuri-check-opened.log\", \"secret\": \"$check_secret\"}"
  start_mock "$check_session_bus" --session org.example.NotGranted org.example.NotGranted /org/example/NotGranted org.example.NotGranted
}

# ---------------------------------------------------------------------------
# --fresh-install (011 US4-2, 011 SC-010, 011 FR-009)
# ---------------------------------------------------------------------------
fresh_dir="$state/fresh-flatpak"
xvfb_pid=""
shipped_pid=""

fresh_cleanup() {
  local status=$?
  trap - EXIT ERR
  set +e
  if [ -n "$shipped_pid" ]; then kill "$shipped_pid" 2>/dev/null; fi
  FLATPAK_USER_DIR="$fresh_dir" flatpak kill "$app_id" >/dev/null 2>&1
  stop_standins
  if [ -n "$xvfb_pid" ]; then kill "$xvfb_pid" 2>/dev/null; fi
  # The throwaway installation goes whatever happened; the container's own is not touched.
  rm -rf "$fresh_dir"
  if [ -e "$fresh_dir" ]; then
    echo "check.sh: could not remove $fresh_dir" >&2
    status=1
  else
    echo "removed: the throwaway installation $fresh_dir"
  fi
  rm -rf "$scratch"
  hoplodex_flatpak_stop_bus
  if [ "$status" -ne 0 ]; then
    echo "check.sh: FAILED (step: ${current_step:-none}); logs are in flatpak/out/check/" >&2
  fi
  exit "$status"
}

# The shipped file into an empty installation with no remotes, as a user's computer
# without Flathub gets it (the network is used: the runtime comes from Flathub).
fresh_install_check() {
  trap fresh_cleanup EXIT
  local version bundle
  version="$(node -p "require('./package.json').version")"
  bundle="$repo/flatpak/out/HoploDex-$version-x86_64.flatpak"

  step "the shipped file"
  [ -s "$bundle" ] || fail "$bundle is missing: run 'npm run bundle:flatpak' first"
  echo "file: $bundle ($(du -h "$bundle" | cut -f1))"

  step "an empty installation with no remotes"
  # env.sh has added Flathub to the container's installation (its default); the second
  # one is made after that, so it stays empty. The guard accepts it all the same.
  rm -rf "$fresh_dir"
  mkdir -p "$fresh_dir"
  FLATPAK_USER_DIR="$fresh_dir" node "$here/guard.mjs" >/dev/null ||
    fail "the guard does not accept $fresh_dir"
  export FLATPAK_USER_DIR="$fresh_dir"
  [ -z "$(flatpak remotes --user --columns=name)" ] || fail "the new installation has remotes"
  [ -z "$(flatpak list --user --columns=ref)" ] || fail "the new installation is not empty"
  echo "installation: $fresh_dir (no remotes, nothing installed)"

  step "flatpak install --user --bundle (network)"
  flatpak install --user --noninteractive --assumeyes --bundle "$bundle"
  echo
  echo "remotes now: $(flatpak remotes --user --columns=name | tr '\n' ' ')"
  flatpak remotes --user --columns=name | grep -qx flathub || fail "the installation did not gain the flathub remote"
  echo "installed refs:"
  flatpak list --user --columns=ref,origin | sed 's/^/  /'
  flatpak list --user --runtime --columns=ref | grep -q '^org\.gnome\.Platform/x86_64/51$' ||
    fail "the installation did not gain org.gnome.Platform//51"
  flatpak info --user "$app_id" >/dev/null || fail "$app_id is not installed"
  echo "the installation gained the flathub remote and org.gnome.Platform//51"

  step "no runtime library came from the file (011 FR-018)"
  local location
  location="$(flatpak info --user --show-location "$app_id")"
  node "$here/contents.mjs" --installed "$location/files" || fail "the installed app's files are not what contents.mjs allows"

  step "start the shipped build under a throwaway home and the stand-in buses"
  # A display of its own (the container has none), as the E2E run has.
  Xvfb -displayfd 5 -screen 0 1280x1024x24 -nolisten tcp -ac 5>"$scratch/display" >"$log_dir/xvfb.log" 2>&1 &
  xvfb_pid="$!"
  local tries=0
  until [ -s "$scratch/display" ]; do
    tries=$((tries + 1))
    [ "$tries" -lt 100 ] || fail "Xvfb did not start (flatpak/out/check/xvfb.log)"
    sleep 0.05
  done
  local display=":$(tr -d '\n' <"$scratch/display")"
  start_standins
  local app_log="$log_dir/shipped-start.log"
  env -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u XDG_CACHE_HOME -u XDG_STATE_HOME -u WAYLAND_DISPLAY \
    HOME="$check_home" DBUS_SESSION_BUS_ADDRESS="$check_session_bus" DISPLAY="$display" \
    flatpak run "$app_id" >"$app_log" 2>&1 &
  shipped_pid="$!"

  # Its log line that tells where it runs (the log reaches standard error, 011 research.md §16),
  # then the folders startup makes: machine.json when the settings are loaded, and the main
  # web view's data folder when the main window is built.
  local data="$check_home/.var/app/$app_id"
  local config="$data/config/$app_id/machine.json"
  local webview="$data/cache/$app_id/main-webview"
  tries=0
  until grep -q '^\[INFO\] running in a Flatpak; reach: ' "$app_log" 2>/dev/null && [ -f "$config" ] && [ -d "$webview" ]; do
    tries=$((tries + 1))
    kill -0 "$shipped_pid" 2>/dev/null || fail "the shipped build exited before its window was up (flatpak/out/check/shipped-start.log)"
    [ "$tries" -lt 1200 ] || fail "the shipped build's window did not come up in 60 s (flatpak/out/check/shipped-start.log)"
    sleep 0.05
  done
  # The page is loading into that web view; give it a moment before it is killed.
  sleep 3
  kill -0 "$shipped_pid" 2>/dev/null || fail "the shipped build exited while starting (flatpak/out/check/shipped-start.log)"
  echo "started: the shipped build is running and logged: $(grep '^\[INFO\] running in a Flatpak' "$app_log" | head -n 1)"
  FLATPAK_USER_DIR="$fresh_dir" flatpak kill "$app_id"
  wait "$shipped_pid" 2>/dev/null || true
  shipped_pid=""

  step "where the shipped build keeps its own data (011 FR-009, 011 research.md §11)"
  [ -f "$config" ] || fail "no machine.json at $config"
  echo "exists: ~/.var/app/$app_id/config/$app_id/machine.json"
  [ -d "$webview" ] || fail "no main web view data folder at $webview"
  echo "exists: ~/.var/app/$app_id/cache/$app_id/main-webview"
  for outside in ".config/$app_id" ".cache/$app_id"; do
    [ ! -e "$check_home/$outside" ] || fail "~/$outside exists: the Flatpak wrote outside its own data folder"
    echo "does not exist: ~/$outside"
  done

  echo
  echo "check.sh: ok (--fresh-install; the throwaway installation is removed next)"
}

if [ "$fresh_install" = 1 ]; then
  fresh_install_check
  exit 0
fi

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
# ---------------------------------------------------------------------------
step "the private session bus and the stand-in services"
start_standins

# ---------------------------------------------------------------------------
step "the bus checks, from inside the sandbox with the shipped permissions (011 research.md §16)"
ping_inside() { # name path
  sandbox_run --command=gdbus "$app_id" call --session --dest "$1" --object-path "$2" \
    --method org.freedesktop.DBus.Peer.Ping
}
for pair in \
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

# Whether the sandbox's bus proxy says a name has an owner.
name_has_owner_inside() { # name
  sandbox_run --command=gdbus "$app_id" call --session --dest org.freedesktop.DBus \
    --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.NameHasOwner "$1"
}

# The Secret portal answers from inside the sandbox, with no permission for it: its
# interface reads a version, which is what the app's availability probe asks (011 FR-006).
version="$(sandbox_run --command=gdbus "$app_id" call --session --dest org.freedesktop.portal.Desktop \
  --object-path /org/freedesktop/portal/desktop --method org.freedesktop.DBus.Properties.Get \
  org.freedesktop.portal.Secret version)" ||
  fail "the Secret portal's version does not read from inside the sandbox"
echo "the Secret portal answers inside the sandbox: version $version"

# The Secret Service (011 SC-011): a real daemon owns the name, and the shipped
# permissions are refused it. It answers outside the sandbox, and not inside.
gdbus call --address "$check_session_bus" --dest org.freedesktop.secrets --object-path /org/freedesktop/secrets \
  --method org.freedesktop.DBus.Peer.Ping >/dev/null ||
  fail "the real Secret Service does not answer outside the sandbox, so its refusal shows nothing"
if ping_inside org.freedesktop.secrets /org/freedesktop/secrets >"$scratch/secrets.out" 2>&1; then
  fail "org.freedesktop.secrets answered from inside the sandbox, which the manifest does not grant (011 SC-011)"
fi
echo "refused inside the sandbox, answering outside: org.freedesktop.secrets ($(tr '\n' ' ' <"$scratch/secrets.out"))"
# What the proxy says of who owns the name is recorded, not asserted.
owned="$(name_has_owner_inside org.freedesktop.secrets)" ||
  fail "NameHasOwner for org.freedesktop.secrets failed from inside the sandbox"
echo "org.freedesktop.secrets, as the sandbox's bus proxy answers who owns it: $owned"

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
# The example that reads the app's keyring file back with the run's key (the journeys'
# saved-passphrase checks); it opens a file and makes no connection.
node --import tsx --input-type=module -e '
  const flatpak = await import("./e2e/support/flatpak.ts");
  process.exit(flatpak.buildKeyringReader() ? 0 : 1);
'

# ---------------------------------------------------------------------------
step "the Flatpak journey: HOPLODEX_E2E_FLATPAK=1 wdio run e2e/wdio.conf.ts"
HOPLODEX_E2E_FLATPAK=1 node e2e/run-e2e.mjs

echo
echo "check.sh: ok (the check variant is uninstalled next)"
