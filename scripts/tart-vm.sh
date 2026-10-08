#!/usr/bin/env bash
# Sets up a macOS VM with tart (https://tart.run) for running HoploDex's tests
# as a standard (non-admin) user, and runs them there. See "macOS in a VM" in
# DEVELOPMENT.md; the first run's findings are on #28.
#
#   scripts/tart-vm.sh setup        clone the VM, create the test user, install
#                                   the toolchain in its home, log it in
#   scripts/tart-vm.sh start [--tall]
#                                   start the VM (with its window) and wait for
#                                   SSH; --tall gives it the 1920x4200 screen
#                                   the screenshot walk needs (see below)
#   scripts/tart-vm.sh stop         shut it down (from inside, as admin)
#   scripts/tart-vm.sh sync         copy this checkout in (~/HoploDex, no .git)
#   scripts/tart-vm.sh test         sync, then lint, unit and Rust tests over SSH,
#                                   and the E2E suite in the user's desktop
#   scripts/tart-vm.sh ssh [CMD]    a shell, or CMD, as the test user in ~/HoploDex
#   scripts/tart-vm.sh gui CMD      run CMD in the test user's desktop session
#                                   (through Terminal), print its output, and
#                                   exit with its status
#   scripts/tart-vm.sh fetch PATH   copy PATH (relative to the checkout) back
#                                   from the VM, e.g. e2e/screenshots-out
#
# The VM comes from a Cirrus Labs image with Xcode, whose `admin` user (password
# `admin`, passwordless sudo) is used only during setup: to create the test
# user, turn off sleep, skip the user's Setup Assistant, log it in
# automatically at boot, turn off macOS's "bypass the private window picker"
# question and, where SIP is off, give Terminal and SSH sessions Accessibility
# and Screen Recording. Everything else runs as the test user, with the
# toolchain (rustup, cargo-nextest, cargo-deny, Node and npm at the Dockerfile's versions)
# in its home, not Homebrew, which belongs to `admin` on the image.
#
# A GUI app started over SSH lands in launchd's Background session and never
# gets a window, so `gui` and the E2E step of `test` start their command
# through Terminal in the logged-in desktop and follow its log over SSH.
# Interrupting them here leaves the command running in the VM.
#
# setup is safe to run again: each step checks what is already done.
#
# Environment:
#   HOPLODEX_VM            VM name (default hoplodex-macos)
#   HOPLODEX_VM_IMAGE      image to clone (default
#                          ghcr.io/cirruslabs/macos-tahoe-xcode:latest)
#   HOPLODEX_VM_USER       the test user (default hoplotest)
#   HOPLODEX_VM_KEY        SSH private key for it (default
#                          ~/.ssh/hoplodex_tart, created if missing)
#   HOPLODEX_VM_CPUS       CPUs (default 4)
#   HOPLODEX_VM_MEMORY     memory in MB (default 8192)
#   HOPLODEX_VM_DISPLAY    the VM's screen, for manual testing and the tests
#                          (default 1920x1080px)
#   HOPLODEX_VM_PASSWORD   the test user's password, for a user that already
#                          exists (default: generated for a new one)
#   HOPLODEX_VM_ADMIN_PASSWORD
#                          the image's admin password (default admin)
#
# The screen: tart sets it only while the VM is stopped, so start shuts a
# running VM down and starts it again when it has the other size. The usual
# one is a desktop to use by hand; the screenshot walk's full-page shots need
# --tall, since macOS keeps a window within the screen and they grow to 4000
# px, as Linux's Xvfb screen is (e2e/support/display.ts). That one is too tall
# for tart's window to show at a usable scale, so start without --tall again
# when the walk is done.
#
# The test user's password is kept, with the VM's SSH host keys and the logs,
# in ${XDG_STATE_HOME:-~/.local/state}/hoplodex-tart/<vm>/.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
vm="${HOPLODEX_VM:-hoplodex-macos}"
image="${HOPLODEX_VM_IMAGE:-ghcr.io/cirruslabs/macos-tahoe-xcode:latest}"
user="${HOPLODEX_VM_USER:-hoplotest}"
key="${HOPLODEX_VM_KEY:-$HOME/.ssh/hoplodex_tart}"
cpus="${HOPLODEX_VM_CPUS:-4}"
memory="${HOPLODEX_VM_MEMORY:-8192}"
# px, not pt: on a Retina host pt gives the VM a 2x framebuffer. Either needs
# room for the E2E window (1200x800): with tart's default 1024x768 the
# screenshot walk's resizeWindow times out.
display="${HOPLODEX_VM_DISPLAY:-1920x1080px}"
tall_display=1920x4200px
admin_password="${HOPLODEX_VM_ADMIN_PASSWORD:-admin}"
state="${XDG_STATE_HOME:-$HOME/.local/state}/hoplodex-tart/$vm"
checkout=HoploDex # in the test user's home

die() { echo "tart-vm: $*" >&2; exit 1; }
say() { echo "==> $*" >&2; }

usage() { sed -n '2,/^set -euo/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//' >&2; exit "${1:-0}"; }

command -v tart >/dev/null || die "tart isn't installed (brew install cirruslabs/cli/tart)"
[[ "$(uname -s)" == Darwin ]] || die "tart runs macOS VMs on a Mac only"

# The Dockerfile's versions, so the VM builds with what the container does.
dockerfile_arg() {
  sed -n "s/^ARG $1=//p" "$repo/Dockerfile" | head -1
}

vm_state() {
  tart list --format json 2>/dev/null |
    python3 -c 'import json,sys; vm=sys.argv[1]; print(next((v["State"] for v in json.load(sys.stdin) if v["Source"]=="local" and v["Name"]==vm), "missing"))' "$vm"
}

vm_ip() { tart ip "$vm" --wait 180; }

ssh_opts() {
  echo -o UserKnownHostsFile="$state/known_hosts" -o StrictHostKeyChecking=accept-new \
    -o ConnectTimeout=5 -o ServerAliveInterval=30 -o LogLevel=ERROR
}

# SSH as the test user, by key.
user_ssh() {
  # shellcheck disable=SC2046
  ssh $(ssh_opts) -i "$key" -o IdentitiesOnly=yes -o BatchMode=yes "$@"
}

# SSH as the image's admin, by password, which ssh reads from SSH_ASKPASS.
admin_ssh() {
  local askpass="$state/askpass"
  printf '#!/bin/sh\nprintf "%%s\\n" %q\n' "$admin_password" > "$askpass"
  chmod 700 "$askpass"
  # shellcheck disable=SC2046
  SSH_ASKPASS="$askpass" SSH_ASKPASS_REQUIRE=force \
    ssh $(ssh_opts) -o PubkeyAuthentication=no \
    -o PreferredAuthentications=password,keyboard-interactive "$@"
}

wait_for_ssh() { # $1: user_ssh or admin_ssh, $2: login
  local ip i
  ip="$(vm_ip)"
  for i in $(seq 1 60); do
    if "$1" "$2@$ip" true 2>/dev/null; then return 0; fi
    sleep 5
  done
  die "no SSH answer from $2@$ip after 5 minutes"
}

# Starts the VM with the screen $1 (default $display). A running VM with a
# different screen, or one this script didn't record, is shut down first.
start_vm() {
  local want="${1:-$display}"
  mkdir -p "$state"
  case "$(vm_state)" in
    missing) die "no VM named $vm; run: $0 setup" ;;
    running)
      [[ "$(cat "$state/display" 2>/dev/null)" == "$want" ]] && return 0
      say "restarting $vm with a $want screen"
      stop_vm
      ;;
  esac
  # Refit off, or tart reshapes the screen to its window, whose frame it
  # remembers from earlier runs. In the same call: tart set clears a refit
  # setting it isn't given.
  tart set "$vm" --display "$want" --no-display-refit
  echo "$want" > "$state/display"
  say "starting $vm (log: $state/run.log)"
  nohup tart run "$vm" > "$state/run.log" 2>&1 &
}

# tart stop cuts a macOS guest's power at once, so whatever the guest hasn't
# written to disk yet is lost, such as the files setup writes just before it
# restarts the VM. So shut the guest down from inside, as admin, and fall back
# to tart stop only if that doesn't finish.
stop_vm() {
  [[ "$(vm_state)" == running ]] || return 0
  say "shutting $vm down"
  mkdir -p "$state"
  local ip i
  if ip="$(tart ip "$vm" 2>/dev/null)"; then
    # The connection may drop before ssh sees the command's status.
    admin_ssh "admin@$ip" 'sudo -n shutdown -h now' >/dev/null 2>&1 || true
    for i in $(seq 1 60); do
      [[ "$(vm_state)" == running ]] || return 0
      sleep 2
    done
  fi
  say "$vm didn't shut down; stopping it"
  tart stop "$vm" --timeout 30
}

# Whether the test user has a desktop session (logged in at the VM's screen).
has_desktop() {
  user_ssh "$user@$(vm_ip)" 'launchctl print "gui/$(id -u)" >/dev/null 2>&1'
}

need_desktop() {
  has_desktop || die "$user isn't logged in at the VM's desktop; log in as $user in the VM's window and leave it there"
}

cmd_setup() {
  mkdir -p "$state"
  chmod 700 "$state"

  if [[ "$(vm_state)" == missing ]]; then
    say "cloning $image as $vm"
    tart clone "$image" "$vm"
    rm -f "$state/known_hosts"
  fi
  if [[ "$(vm_state)" != running ]]; then
    tart set "$vm" --cpu "$cpus" --memory "$memory"
  fi

  if [[ ! -f "$key" ]]; then
    say "creating SSH key $key"
    mkdir -p "$(dirname "$key")"
    ssh-keygen -q -t ed25519 -N '' -C "hoplodex-tart" -f "$key"
  fi
  if [[ -n "${HOPLODEX_VM_PASSWORD:-}" ]]; then
    (umask 077; printf '%s' "$HOPLODEX_VM_PASSWORD" > "$state/password")
  elif [[ ! -f "$state/password" ]]; then
    (umask 077; openssl rand -hex 12 > "$state/password")
  fi

  start_vm
  local ip
  ip="$(vm_ip)"
  say "waiting for SSH on $ip"
  wait_for_ssh admin_ssh admin

  say "configuring the VM as admin: user $user, sleep, Setup Assistant, auto-login"
  {
    printf 'user=%q password=%q pubkey=%q\n' "$user" "$(cat "$state/password")" "$(cat "$key.pub")"
    cat <<'ADMIN'
set -euo pipefail
home="/Users/$user"
if ! id "$user" >/dev/null 2>&1; then
  # A standard user: no -admin.
  sysadminctl -addUser "$user" -fullName "HoploDex tests" -password "$password" \
    -home "$home" -shell /bin/zsh 2>&1 | grep -v '^$' || true
  id "$user" >/dev/null 2>&1 || { echo "creating $user failed" >&2; exit 1; }
  createhomedir -c -u "$user" >/dev/null
fi
if dseditgroup -o checkmember -m "$user" admin >/dev/null 2>&1; then
  echo "$user is an administrator; the tests are meant to run as a standard user" >&2
  exit 1
fi

# Remote Login, if it's limited to a list of users.
if dscl . -read /Groups/com.apple.access_ssh >/dev/null 2>&1; then
  dseditgroup -o edit -a "$user" -t user com.apple.access_ssh
fi
install -d -m 700 -o "$user" -g staff "$home/.ssh"
touch "$home/.ssh/authorized_keys"
grep -qxF "$pubkey" "$home/.ssh/authorized_keys" || printf '%s\n' "$pubkey" >> "$home/.ssh/authorized_keys"
chown "$user":staff "$home/.ssh/authorized_keys"
chmod 600 "$home/.ssh/authorized_keys"

# No sleep: a sleeping or locked screen gives WKWebView no animation frames,
# and every E2E step times out.
pmset -a sleep 0 displaysleep 0 disksleep 0 >/dev/null

# The user's Setup Assistant, which would otherwise greet its first login.
version="$(sw_vers -productVersion)"
build="$(sw_vers -buildVersion)"
sa() { sudo -u "$user" defaults write com.apple.SetupAssistant "$@"; }
for pane in DidSeeAccessibility DidSeeActivationLock DidSeeAppearanceSetup \
  DidSeeApplePaySetup DidSeeAvatarSetup DidSeeCloudSetup DidSeeiCloudLoginForStorageServices \
  DidSeeIntelligence DidSeeLockdownMode DidSeePrivacy DidSeeScreenTime DidSeeSiriSetup \
  DidSeeSyncSetup DidSeeSyncSetup2 DidSeeTermsOfService DidSeeTouchIDSetup \
  DidSeeTrueTonePrivacy DidSeeWalletSetup SkipFirstLoginOptimization; do
  sa "$pane" -bool true
done
for seen in LastSeenCloudProductVersion LastSeenDiagnosticsProductVersion \
  LastSeenSiriProductVersion LastSeenIntelligenceProductVersion \
  LastSeenAgeRangeSelectionProductVersion LastPreLoginTasksPerformedVersion; do
  sa "$seen" -string "$version"
done
sa LastSeenBuddyBuildVersion -string "$build"
sa LastPreLoginTasksPerformedBuild -string "$build"
sa LastPrivacyBundleVersion -string 9999

# Log the user in at boot, so the VM comes up at its desktop. That needs the
# user's password, which for a user made before is the one it was given then.
if ! dscl /Local/Default -authonly "$user" "$password" >/dev/null 2>&1; then
  echo "$user's password isn't the one in the state directory, so it isn't logged in at boot;" \
    "put its password there (or in HOPLODEX_VM_PASSWORD) and run setup again" >&2
else
  # sysadminctl -autologin set names the user but fails to store the password
  # here (SACSetAutoLoginPassword error:22), so write /etc/kcpassword as it
  # would: the password and a NUL, padded to 12 bytes, XORed with Apple's key.
  perl -e 'my @k = (0x7D, 0x89, 0x52, 0x23, 0xD2, 0xBC, 0xDD, 0xEA, 0xA3, 0xB9, 0x1F);
    my $p = shift . "\0"; $p .= "\0" while length($p) % 12;
    print join "", map { chr(ord(substr($p, $_, 1)) ^ $k[$_ % @k]) } 0 .. length($p) - 1' \
    "$password" > /etc/kcpassword
  chmod 600 /etc/kcpassword
  defaults write /Library/Preferences/com.apple.loginwindow autoLoginUser "$user"
fi
if [ "$(defaults read /Library/Preferences/com.apple.loginwindow autoLoginUser 2>/dev/null)" = "$user" ]; then
  echo "AUTOLOGIN $user"
fi

# Accessibility and Screen Recording for Terminal, which runs the desktop
# commands (`gui`), and for SSH sessions (sshd-keygen-wrapper is the process
# TCC holds responsible for them), so real input and screenshots work from
# either without a prompt over the middle of the screen. TCC.db can only be
# written with SIP off, as it is on the Cirrus images; otherwise grant them by
# hand.
if csrutil status | grep -q disabled; then
  tcc="/Library/Application Support/com.apple.TCC/TCC.db"
  now="$(date +%s)"
  for client in 'com.apple.Terminal 0' '/usr/libexec/sshd-keygen-wrapper 1'; do
    set -- $client # the bundle ID or path, and its client_type
    for service in kTCCServiceAccessibility kTCCServiceScreenCapture kTCCServicePostEvent; do
      sqlite3 "$tcc" "INSERT OR REPLACE INTO access
        (service, client, client_type, auth_value, auth_reason, auth_version, flags, last_modified)
        VALUES ('$service', '$1', $2, 2, 4, 1, 0, $now);"
    done
  done
else
  echo "SIP is on: grant Terminal and sshd-keygen-wrapper Accessibility and Screen Recording in the VM's Privacy & Security settings" >&2
fi

# Screen Recording granted, macOS still asks each app that captures the screen
# directly (screencapture, from Terminal or over SSH) whether it may go on
# "bypassing the private window picker", at its first capture and every 30
# days, over the middle of the screen. forceBypassScreenCaptureAlert is the
# restriction an MDM profile sets to stop that (macOS 15.1+); replayd ignores
# it as a user preference, so it goes in as a managed one. On a Mac with no
# MDM, macOS empties /Library/Managed Preferences at boot, so a daemon puts
# the file back whenever that folder changes (and at boot), written as a file
# (defaults won't write there) and read once cfprefsd starts again.
daemon=io.github.exodious.hoplodex-tart.screen-capture-alert
cat > "/Library/LaunchDaemons/$daemon.plist" <<'DAEMON'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>io.github.exodious.hoplodex-tart.screen-capture-alert</string>
<key>ProgramArguments</key><array><string>/bin/sh</string><string>-c</string><string>
f="/Library/Managed Preferences/com.apple.applicationaccess.plist"
[ "$(/usr/libexec/PlistBuddy -c "Print :forceBypassScreenCaptureAlert" "$f" 2>/dev/null)" = true ] &amp;&amp; exit 0
mkdir -p "/Library/Managed Preferences"
[ -f "$f" ] || plutil -create xml1 "$f"
plutil -replace forceBypassScreenCaptureAlert -bool YES "$f"
chmod 644 "$f"
killall cfprefsd
</string></array>
<key>RunAtLoad</key><true/>
<key>WatchPaths</key><array><string>/Library/Managed Preferences</string></array>
</dict></plist>
DAEMON
chmod 644 "/Library/LaunchDaemons/$daemon.plist"
launchctl bootout "system/$daemon" 2>/dev/null || true
launchctl bootstrap system "/Library/LaunchDaemons/$daemon.plist"
ADMIN
  } | admin_ssh "admin@$ip" 'sudo -n bash -s' > "$state/admin.log" 2>&1 || {
    cat "$state/admin.log" >&2
    die "admin setup failed"
  }
  grep -v '^AUTOLOGIN ' "$state/admin.log" >&2 || true
  wait_for_ssh user_ssh "$user"

  say "installing the toolchain in $user's home"
  {
    printf 'node_version=%q npm_version=%q nextest_version=%q deny_version=%q rust_toolchain=%q\n' \
      "$(dockerfile_arg NODE_VERSION)" "$(dockerfile_arg NPM_VERSION)" \
      "$(dockerfile_arg CARGO_NEXTEST_VERSION)" "$(dockerfile_arg CARGO_DENY_VERSION)" \
      "$(dockerfile_arg RUST_TOOLCHAIN)"
    cat <<'USER'
set -euo pipefail
export PATH="$HOME/.cargo/bin:$HOME/.local/node/bin:$PATH"

if [ ! -x "$HOME/.cargo/bin/rustup" ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
    sh -s -- -y --no-modify-path --profile minimal \
      --default-toolchain "$rust_toolchain" --component rustfmt,clippy
fi
rustup update "$rust_toolchain" --no-self-update >/dev/null 2>&1 || true

# Node, the official build, checksum-verified, as in the Dockerfile.
if [ "$(node --version 2>/dev/null)" != "v$node_version" ]; then
  tarball="node-v${node_version}-darwin-arm64.tar.xz"
  tmp="$(mktemp -d)"
  (
    cd "$tmp"
    curl -fsSLO "https://nodejs.org/dist/v${node_version}/${tarball}"
    curl -fsSL "https://nodejs.org/dist/v${node_version}/SHASUMS256.txt" |
      grep " ${tarball}\$" | shasum -a 256 -c -
  )
  rm -rf "$HOME/.local/node"
  mkdir -p "$HOME/.local/node"
  tar -xJf "$tmp/$tarball" -C "$HOME/.local/node" --strip-components=1
  rm -rf "$tmp"
fi
case "$(npm --version)" in
  "$npm_version".*) ;;
  *) npm install -g --no-fund --no-audit "npm@${npm_version}" ;;
esac

case "$(cargo nextest --version 2>/dev/null | head -1)" in
  *" $nextest_version"*) ;;
  *) cargo install --locked cargo-nextest --version "$nextest_version" ;;
esac
case "$(cargo deny --version 2>/dev/null | head -1)" in
  *" $deny_version"*) ;;
  *) cargo install --locked cargo-deny --version "$deny_version" ;;
esac

# For non-interactive SSH commands and Terminal's .command files alike.
if ! grep -q 'hoplodex-tart' "$HOME/.zshenv" 2>/dev/null; then
  cat >> "$HOME/.zshenv" <<'ENV'
# hoplodex-tart: the toolchain installed by scripts/tart-vm.sh
export PATH="$HOME/.cargo/bin:$HOME/.local/node/bin:$PATH"
export NPM_CONFIG_UPDATE_NOTIFIER=false
ENV
fi

# No screen saver, and so no lock behind it.
defaults -currentHost write com.apple.screensaver idleTime -int 0

# Terminal starts with no windows: it doesn't reopen the last session's, which
# after a `gui` run cut short would come back as a pile of shells.
defaults write com.apple.Terminal ApplePersistenceIgnoreState -bool true
defaults write com.apple.Terminal NSQuitAlwaysKeepsWindows -bool false

echo "rust $(rustc --version | cut -d' ' -f2), $(cargo nextest --version | head -1), node $(node --version), npm $(npm --version)"
USER
  } | user_ssh "$user@$ip" 'bash -s'

  if ! has_desktop && grep -qxF "AUTOLOGIN $user" "$state/admin.log"; then
    say "restarting $vm so $user logs in automatically"
    stop_vm
    start_vm
    wait_for_ssh user_ssh "$user"
    local i
    for i in $(seq 1 24); do
      has_desktop && break
      sleep 5
    done
  fi
  if has_desktop; then
    say "$user is logged in at the VM's desktop"
  else
    say "$user isn't logged in at the desktop; log in as $user in the VM's window and leave it there"
  fi
  say "done. Next: $0 test"
  echo "Leave the VM's window open; check it for any Setup Assistant pane still waiting for a click." >&2
}

cmd_sync() {
  local ip
  ip="$(vm_ip)"
  say "copying the checkout to $user@$ip:$checkout/"
  # The ignored build outputs are excluded by name as well as through
  # .gitignore: macOS's rsync (openrsync) doesn't protect files matched only
  # by a dir-merge filter from --delete, and would delete the VM's own builds.
  # shellcheck disable=SC2046
  rsync -a --delete \
    -e "ssh $(ssh_opts) -i $key -o IdentitiesOnly=yes -o BatchMode=yes" \
    --exclude=/.git/ --exclude=/node_modules/ --exclude=/src-tauri/target/ \
    --exclude=/dist/ --exclude=/dist-tuner/ --exclude=/e2e/screenshots-out/ \
    --exclude=/e2e/wdio-logs/ --exclude=/.human-testing/ \
    --filter=':- .gitignore' \
    "$repo/" "$user@$ip:$checkout/"
}

cmd_fetch() {
  [[ $# -eq 1 ]] || die "usage: $0 fetch PATH"
  local path="${1%/}" ip
  ip="$(vm_ip)"
  mkdir -p "$repo/$(dirname "$path")"
  rsync -a -e "ssh $(ssh_opts) -i $key -o IdentitiesOnly=yes -o BatchMode=yes" \
    "$user@$ip:$checkout/$path" "$repo/$(dirname "$path")/"
  say "copied to $repo/$path"
}

cmd_ssh() {
  local ip
  ip="$(vm_ip)"
  if [[ $# -eq 0 ]]; then
    user_ssh -t "$user@$ip" "cd $checkout 2>/dev/null; exec zsh -l"
  else
    user_ssh "$user@$ip" "cd $checkout && $(printf '%q ' "$@")"
  fi
}

# Runs a shell command line in the test user's desktop session through
# Terminal, in a window that closes when it's done, follows its output, and
# exits with its status.
cmd_gui() {
  [[ $# -gt 0 ]] || die "usage: $0 gui CMD [ARG...]"
  need_desktop
  local line
  line="$(printf '%q ' "$@")"
  user_ssh "$user@$(vm_ip)" "bash -s -- $(printf '%q' "$line")" <<REMOTE
set -u
dir="\$(mktemp -d /tmp/hoplodex-gui.XXXXXX)"
: > "\$dir/log"
{
  echo '#!/bin/zsh'
  # The cd inside, so a missing checkout still writes a status to wait for.
  printf '{ cd ~/$checkout && %s; } >> %q 2>&1\n' "\$1" "\$dir/log"
  printf 'echo \$? > %q\n' "\$dir/status"
} > "\$dir/run.command"
chmod +x "\$dir/run.command"
# Opened through a settings file, not as a .command: Terminal's profiles keep
# a window open after its shell exits, so every run left a dead window
# behind. This one closes its window (shellExitAction 0) once the command and
# then the shell exit, and Terminal doesn't keep it among its profiles.
cat > "\$dir/run.terminal" <<TERMINAL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>type</key><string>Window Settings</string>
<key>CommandString</key><string>\$dir/run.command; exit</string>
<key>RunCommandAsShell</key><false/>
<key>shellExitAction</key><integer>0</integer>
</dict></plist>
TERMINAL
open -a Terminal "\$dir/run.terminal"
tail -n +1 -f "\$dir/log" &
tail=\$!
until [ -s "\$dir/status" ]; do sleep 1; done
sleep 1
kill \$tail 2>/dev/null
wait \$tail 2>/dev/null
status=\$(cat "\$dir/status")
rm -rf "\$dir"
exit "\$status"
REMOTE
}

cmd_test() {
  cmd_sync
  local ip failed=0
  ip="$(vm_ip)"
  say "lint, unit and Rust tests over SSH"
  user_ssh "$user@$ip" "cd $checkout && bash -s" <<'REMOTE' || failed=1
set -u
failures=()
step() {
  echo "==> $*"
  "$@" || failures+=("$*")
}
# node_modules, reinstalled when the lockfile changes.
lock="$(shasum -a 256 package-lock.json | cut -d' ' -f1)"
if [ "$(cat node_modules/.hoplodex-lock 2>/dev/null)" != "$lock" ]; then
  npm ci --no-fund --no-audit && echo "$lock" > node_modules/.hoplodex-lock || { echo "npm ci failed"; exit 1; }
fi
step npm run lint
step npm run format:check
step cargo fmt --check --manifest-path src-tauri/Cargo.toml
step cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
step npm test
step cargo nextest run --manifest-path src-tauri/Cargo.toml
step cargo nextest run --manifest-path src-tauri/Cargo.toml --features mock-keyring -E 'binary(keyring_test)'
step npm run build
step npm run audit
if [ ${#failures[@]} -gt 0 ]; then
  printf 'FAILED: %s\n' "${failures[@]}"
  exit 1
fi
REMOTE
  say "E2E in $user's desktop"
  cmd_gui npm run test:e2e || failed=1
  if [[ $failed -ne 0 ]]; then
    say "some checks failed (see above)"
    exit 1
  fi
  say "all checks passed"
}

[[ $# -gt 0 ]] || usage 1
sub="$1"
shift
case "$sub" in
  setup) cmd_setup ;;
  start)
    case "${1:-}" in
      --tall) start_vm "$tall_display" ;;
      "") start_vm ;;
      *) usage 1 ;;
    esac
    wait_for_ssh user_ssh "$user"
    say "$vm is up at $(vm_ip)"
    ;;
  stop) stop_vm ;;
  sync) cmd_sync ;;
  test) cmd_test ;;
  ssh) cmd_ssh "$@" ;;
  gui) cmd_gui "$@" ;;
  fetch) cmd_fetch "$@" ;;
  -h | --help | help) usage 0 ;;
  *) usage 1 ;;
esac
