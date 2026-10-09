# Developing HoploDex

A Tauri 2.x desktop app: a Rust backend (`src-tauri/`) owns persistence,
encryption, and business logic; a React + TypeScript frontend (`src/`)
owns the UI.

## Development container (Linux, recommended)

The repo's `Dockerfile` (Debian trixie) has everything below already
installed: Rust, Node 24 LTS with npm 12, the Tauri/WebKitGTK and SQLCipher
build dependencies, `cargo-deny`, `cargo-nextest`, Xvfb,
gnome-keyring, the GitHub CLI, Claude Code, Spec Kit's `specify`, and
`python3-gi` for GTK drag-and-drop tests. It's built for rootless [podman](https://podman.io) and
runs as a non-root `dev` user. The only thing to install on the host is
podman.

```bash
scripts/dev-container.sh                      # shell in /workspace (builds the image on first use)
scripts/dev-container.sh npm test             # or run one command and exit
scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'   # several commands
scripts/dev-container.sh --build              # rebuild the image, e.g. after pulling Dockerfile changes
scripts/dev-container.sh --reset-volumes      # start over with empty node_modules and target volumes
scripts/dev-container.sh --gui npm run tauri dev   # show the app's window on your desktop
scripts/dev-container.sh --git-config --ssh-agent  # a shell you can commit and push from
```

Every command in the sections below works unchanged inside the container.
Inside it, the working directory is `/workspace` and the hostname is
`hoplodex-dev`. Your checkout is bind-mounted at `/workspace`, and
`--userns=keep-id` maps your host user onto `dev`, so files the container
writes stay owned by you. Some things are kept in named volumes rather than
in the checkout, so they don't collide with host builds:

- `node_modules` and `src-tauri/target`, one pair per checkout. The container
  links against its own system libraries, so its builds can't share these
  with the host. On first run the container fills `node_modules` with
  `npm ci`, and the first cargo build starts from scratch, so the first run
  of anything is slow.
- `/home/dev`, shared by all checkouts: shell history, the cargo crate cache,
  the keyring, and `gh` / `claude` logins. Log in once with `gh auth login` and
  `claude` inside the container.

Nothing from your host's identity is shared by default: no git config, SSH
agent or tokens. To commit and push from inside, pass `--git-config` to mount
your `~/.gitconfig` read-only (or `--git-config=FILE` for another one) and
`--ssh-agent` to forward your SSH agent socket. To use tokens instead of
logging in, export them on the host and pass `--gh-token` (`GH_TOKEN` /
`GITHUB_TOKEN`) or `--anthropic-api-key` (`ANTHROPIC_API_KEY`). Options go
before the command. Tests and screenshots run
headless on Xvfb and need no display. `--gui` forwards your Wayland or X11
socket and `/dev/dri`, for `tauri dev` and human testing. `podman volume ls |
grep hoplodex` lists the volumes. `--reset-volumes` deletes this checkout's
`node_modules` and `target` volumes before starting, so they're rebuilt from
scratch; `--reset-volumes=all` also deletes the shared home volume, which
logs you out of `gh` and `claude`.
`CONTAINER_ENGINE=docker` works too. Docker has no `keep-id`, though, so files
end up owned by uid 1000, which is fine if that's your uid.

On macOS the container runs in podman's Linux VM, which can't see XQuartz's
Unix socket, so `--gui` connects to XQuartz over TCP instead and renders in
software, with a 24 px pointer (XQuartz gives clients no size, so GTK would pick
48 px on a 4K screen). One-time setup: install XQuartz, tick "Allow connections from
network clients" in its Settings > Security (or `defaults write
org.xquartz.X11 nolisten_tcp -bool false`), quit and reopen it, and run
`xhost +localhost` (the VM's connection arrives as a local one, so this admits
only your own machine). The script checks that XQuartz is listening and says
what to change if it isn't.

The container has none of the host's app data, so nothing run inside it can
reach your real collection (see [Test isolation](#test-isolation)).

To set up a host directly instead (or on macOS/Windows), install the
prerequisites below.

## Prerequisites

**Rust** (stable, 1.97+), via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Linux/macOS
```

On Windows, download and run [`rustup-init.exe`](https://win.rustup.rs).

**cargo-nextest**, the Rust test runner (`cargo install cargo-nextest --locked`,
or [a prebuilt binary](https://nexte.st/docs/installation/pre-built-binaries/)).
`cargo test` still works without it, only slower.

**Node.js 24 LTS** and **npm 12+** (npm 12 writes the lockfile format the
repo uses) — via
[nodejs.org](https://nodejs.org), [nvm](https://github.com/nvm-sh/nvm), or
your platform's package manager.

**Platform system dependencies** (required by Tauri/WebView, and by
`rusqlite`'s bundled-SQLCipher build):

- **Linux** (Debian/Ubuntu and Arch package names below — see
  [Tauri's Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux)
  for other distros):

  Debian/Ubuntu:

  ```bash
  sudo apt update
  sudo apt install -y \
    build-essential \
    pkg-config \
    perl \
    libwebkit2gtk-4.1-dev \
    libjavascriptcoregtk-4.1-dev \
    libgtk-3-dev \
    libsoup-3.0-dev \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    xdg-utils
  ```

  Arch:

  ```bash
  sudo pacman -S --needed \
    base-devel \
    curl \
    wget \
    file \
    openssl \
    appmenu-gtk-module \
    libappindicator-gtk3 \
    librsvg \
    webkit2gtk-4.1 \
    xdg-utils
  ```

- **macOS**: Xcode Command Line Tools —

  ```bash
  xcode-select --install
  ```

- **Windows**: see [Windows](#windows) below.

**End-to-end (E2E) testing extras — Linux only:** an isolated virtual
display. The E2E build carries its own WebDriver server (see "Test"), so
there is no driver to install.

```bash
sudo apt install -y xvfb xauth               # Debian/Ubuntu
sudo pacman -S --needed xorg-server-xvfb     # Arch
```

**Headless/SSH sessions only:** HoploDex reads/writes its SQLCipher database
key via the OS credential store (the `keyring` crate), which on Linux talks
to the Secret Service D-Bus API. A bare SSH login has a D-Bus session bus
but usually no Secret Service provider registered on it, so `test:e2e` fails
with `keyring error: ... org.freedesktop.DBus.Error.ServiceUnknown: The name
is not activatable`. Fix by installing `gnome-keyring` and starting it
against your session's bus before running tests:

```bash
sudo pacman -S --needed gnome-keyring   # apt install gnome-keyring on Debian/Ubuntu
printf '\n' | gnome-keyring-daemon --login --daemonize --components=pkcs11,secrets
gnome-keyring-daemon --start --components=pkcs11,secrets
```

Use `--login` (not `--start --unlock` — this daemon version rejects that
combination), which unlocks the login keyring with the blank password from
stdin in one step, creating it on first run. Follow it with `--start`, which
finishes the session like a desktop login. Without it the `--login` daemon
exits after 120 seconds, and the next keyring call D-Bus-activates a fresh
daemon that has no login keyring, which fails with `SS error: result not
returned from SS API`. Verify it registered correctly
with `busctl --user list | grep org.freedesktop.secrets` before re-running
`npm run test:e2e`.

**macOS (partly ported, #28):** the harness gives the app a `HOME` inside
the sandbox (only the app; cargo keeps the real one), since macOS builds
Application Support, Caches and Documents from `HOME` and ignores `XDG_*`.
It runs on the host, with the app's window on the real desktop. It needs an
unlocked, logged-in session: while the screen is locked, WKWebView reports
every page as hidden and gives it no animation frames, so every step times
out with "The app never settled". The tests that use real input
(`e2e/support/realInput.ts`, X11 only: us7's owl beak, us8's restore-dialog
regressions, all of us10, us11 and us12) are skipped, and reported as
pending. `quit-cleanup.py` runs there too (see below), and so does the
screenshot walk, but macOS keeps a window within the screen, so the
full-page shots are cut off at the screen's height unless the display is
taller than the page (`scripts/tart-vm.sh start --tall` gives its VM
1920×4200, as Linux's Xvfb screen).

**macOS in a VM:** to test macOS without your own account's data nearby,
use a [tart](https://tart.run) VM made from a Cirrus Labs image with Xcode
(`ghcr.io/cirruslabs/macos-tahoe-xcode`). `scripts/tart-vm.sh` sets one up
and runs the tests in it as a standard (non-admin) user:

```bash
scripts/tart-vm.sh setup                   # clone, create hoplotest, install the toolchain, log it in at boot
scripts/tart-vm.sh start                   # start it with a 1920×1080 screen, for the tests and manual testing
scripts/tart-vm.sh test                    # copy the checkout in, lint and test over SSH, E2E in its desktop
scripts/tart-vm.sh ssh npx vitest run src/features/firearms/FirearmForm.test.tsx   # one command
scripts/tart-vm.sh gui npm run test:e2e -- --spec e2e/specs/us1-record-firearm.e2e.ts
scripts/tart-vm.sh start --tall            # restart it with a 1920×4200 screen, for the screenshot walk
scripts/tart-vm.sh gui bash -c 'npm run build && npm run screenshots'
scripts/tart-vm.sh fetch e2e/screenshots-out   # copy results back
scripts/tart-vm.sh start                   # back to 1920×1080
```

The VM's screen is 1920×1080 unless you ask for the tall one. The screenshot
walk needs 1920×4200, since macOS keeps a window within the screen and the
full-page shots grow to 4000, but tart's window shrinks a screen that tall
until the desktop is too small to use by hand. tart changes the screen only
while the VM is stopped, so `start` shuts a running VM down and starts it
again when it has the other size. It also turns off tart's display refit
(`--no-display-refit`), which otherwise reshapes the VM's screen to fit tart's
window (one that remembered an earlier size gave a 1920×2012 screen for
1080), so the screen is the size asked for and tart scales it into the
window. Set `HOPLODEX_VM_DISPLAY` (such as
`1440x900px`) for a usual screen that suits your own; keep it at least
1200×800 for the E2E window.

`setup` uses the image's `admin` only to create the user, turn off sleep,
skip the user's Setup Assistant, log it in automatically at boot and give
Terminal and SSH sessions Accessibility and Screen Recording, and turn off
the "bypass the private window picker" question macOS otherwise asks each of
them at the first screenshot and every 30 days (`forceBypassScreenCaptureAlert`,
as a managed preference that a small LaunchDaemon puts back after each boot).
The toolchain (rustup, cargo-nextest, Node and npm at the `Dockerfile`'s
versions) goes in the user's home. Running it again finishes what's left.
Shut the VM down with `scripts/tart-vm.sh stop`, not `tart stop`, which cuts a macOS guest's power
and loses whatever it hasn't written to disk yet. The script's header lists
its settings (`HOPLODEX_VM` and friends, for a VM you already have). The rest
of this paragraph is what it does for you. To set a VM up by hand, install the
prerequisites in it and leave it logged in at its desktop. Copy the checkout in with
`rsync -a --exclude .git --exclude src-tauri/target --exclude node_modules
--exclude dist --exclude e2e/screenshots-out ./ <user>@$(tart ip <vm>):HoploDex/`
(with `--delete`, exclude those by name even if `.gitignore` is a filter too:
macOS's rsync deletes files a dir-merge filter alone matches).
Over SSH, mind where a GUI app starts. A program started from an SSH session
runs in launchd's `Background` session (`launchctl managername` says so) and
never gets a window: the app starts, logs, and waits forever. Run the E2E
suite from a Terminal in the VM, or from SSH start the app through Terminal
(`open -a Terminal run.command`) and drive it from SSH, as
`scripts/macos/pdf-surface-check.sh` does. Whatever posts mouse input or
takes screenshots (`sshd-session` over SSH, or Terminal) needs Accessibility
and Screen Recording in the VM's Privacy & Security settings. Until someone
answers it, the first request's prompt sits over the middle of the screen
and catches the clicks. The tart VM and podman's VM together can need more
memory than a host has; if so, run the dev container's checks on another
computer.

### Windows

x64 Windows 10 or 11 (Pro, Enterprise or Education), or Windows Server
2025 with Desktop Experience. Two scripts install everything in this
section, skipping what's already there, so they can be run again:

```powershell
# 1. Once per computer, from an elevated PowerShell; -AutoLogon (test machines) and -GitHubCli optional
powershell -ExecutionPolicy Bypass -File scripts\windows\setup-system.ps1 -User alice
# 2. Signed in as that user, from a new PowerShell that isn't elevated;
#    -ClaudeCode or -ClaudeRemoteControl <checkout> optional
powershell -ExecutionPolicy Bypass -File scripts\windows\setup-user.ps1
```

The repository is private, so on a new computer copy the two scripts over
first; the system script installs Git, and `-GitHubCli` adds `gh` to clone
with. Run the user script as the account that builds: rustup, npm 12 and
the cargo tools install into that account's profile, so an elevated shell
signed in as another administrator sets up the wrong account.

What they install, to set it up by hand instead:

**Machine-wide** (`setup-system.ps1`):

- **The MSVC C++ build tools and a Windows SDK.** Rust's
  `x86_64-pc-windows-msvc` toolchain links with Microsoft's linker, and
  `rusqlite`'s `bundled-sqlcipher` compiles SQLCipher's C with `cl.exe`.
  Install [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  2026 with the "Desktop development with C++" workload, whose recommended
  parts include the SDK:

  ```powershell
  winget install --id Microsoft.VisualStudio.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  ```

  A Visual Studio (Community or another edition, 2022 or 2026) with that
  workload works as well, and so do the 2022 Build Tools
  (`Microsoft.VisualStudio.2022.BuildTools`). The GNU toolchain
  (`x86_64-pc-windows-gnu`) doesn't: the OpenSSL below is built with MSVC.
- **Git** (`winget install --id Git.Git -e --scope machine`).
- **Node.js 24 LTS.** `OpenJS.NodeJS.LTS` follows whichever release is LTS,
  so install a 24.x with `--version` and keep it there with
  `winget pin add --id OpenJS.NodeJS.LTS --version 24.*`.
- **Python 3.13**, the dev container's version, for all users and on the
  machine `PATH`, with the `py` launcher:
  `winget install --id Python.Python.3.13 -e --scope machine --override "/quiet InstallAllUsers=1 PrependPath=1 Include_launcher=1 InstallLauncherAllUsers=1 Include_test=0"`.
  On Windows it's `python` or `py -3.13`; `python3` is the Microsoft Store's
  alias.
- **WebView2**, the engine the app's window uses. Windows 10 and 11 include
  it; otherwise get it from the
  [WebView2 runtime page](https://developer.microsoft.com/microsoft-edge/webview2/)
  or `winget install --id Microsoft.EdgeWebView2Runtime -e`.
- **OpenSSL 3, from [vcpkg](https://vcpkg.io).** SQLCipher needs it on
  Windows (macOS uses CommonCrypto and Linux uses `libssl-dev`). It must be
  OpenSSL 3: 1.1 and older aren't GPLv3-compatible (see
  [License audit](#license-audit)). Use a vcpkg of its own, cloned from
  GitHub; the one that comes with Visual Studio has no classic mode, so
  `vcpkg install` refuses there.

  ```powershell
  git clone https://github.com/microsoft/vcpkg C:\vcpkg
  C:\vcpkg\bootstrap-vcpkg.bat -disableMetrics
  C:\vcpkg\vcpkg install openssl:x64-windows-static-md
  ```

  Then set `OPENSSL_DIR=C:\vcpkg\installed\x64-windows-static-md` and
  `OPENSSL_STATIC=1`, machine-wide or for the user. The build stops with
  "Missing environment variable OPENSSL_DIR" otherwise.
- **Long paths:** `LongPathsEnabled` = 1 under
  `HKLM\SYSTEM\CurrentControlSet\Control\FileSystem`, and
  `git config --system core.longpaths true`, for deep `node_modules` and
  `target` paths.
- **Remote access for `-User`:** Remote Desktop on, with Network Level
  Authentication, and open in the firewall; the OpenSSH server
  (`sshd`, built into Server 2025, an optional capability elsewhere)
  started automatically and open on port 22; and the user in the "Remote
  Desktop Users" and "OpenSSH Users" groups. Windows Home has no Remote
  Desktop host, so the script refuses it. SSH is restricted to that group
  with `AllowGroups "openssh users"` in `%ProgramData%\ssh\sshd_config`,
  before its first `Match` block (Windows' `sshd` wants names in lower
  case), so an administrator who isn't in the group can't sign in over
  SSH. The script keeps the old file as `sshd_config.before-hoplodex`.
- **Windows Server:** Server Manager doesn't open at sign-in (its
  scheduled task is off, and the `DoNotOpenAtLogon` policy is set).
- **Auto-logon (`-AutoLogon`, test machines only):** E2E and screenshot
  runs need a desktop session that stays signed in and drawing. Windows
  signs the user in at startup (`AutoAdminLogon`, `DefaultUserName` and
  `DefaultDomainName` under `HKLM\...\Winlogon`). The password is kept
  as the `DefaultPassword` LSA secret, as Sysinternals Autologon does,
  rather than in the registry, where any local user can read it. The
  session never locks, blanks or sleeps when idle: display, sleep and
  hibernate timeouts are off, no password on wakeup, no machine
  inactivity limit, and the user's screen saver is off by policy. The
  user's "Show animations in Windows" is turned on (a bit of
  `UserPreferencesMask`): Windows Server starts with it off, and WebView2
  then reports reduced motion, so us7's catalogue-plate test fails. Both
  live in the user's own registry, so if they've never signed in, restart (auto-logon signs them in) and run the script again.
  Avoid connecting with Remote Desktop as that user before a test run: it
  takes over the console session, which stops drawing once you
  disconnect.

**Per user** (`setup-user.ps1`), after a new shell picks up the new `PATH`:

- **rustup** (`winget install --id Rustlang.Rustup -e`, or
  [`rustup-init.exe`](https://win.rustup.rs)), then
  `rustup toolchain install stable --component rustfmt --component clippy`.
  Install it after the build tools, so its default host is
  `x86_64-pc-windows-msvc`.
- **npm 12:** `npm install --global npm@12`. It lands in `%APPDATA%\npm`,
  and Node's own `npm.cmd` hands off to it, for this account only.
- **cargo-nextest and cargo-deny:** `cargo install --locked cargo-nextest cargo-deny`.

**Claude Code on a test machine (`-ClaudeRemoteControl <checkout>`).**
Claude can't live in an SSH session on Windows: `sshd` ends every process
in a session when it disconnects (there's no `tmux` or `nohup` to escape
it), and the session has no desktop, so the app's window can't open for
E2E. So it runs in the auto-logon desktop session instead, and you reach
it through [Remote Control](https://code.claude.com/docs/en/remote-control)
from claude.ai/code, the Claude app or another Claude session. The option
installs Claude Code, copies `scripts/windows/claude-remote-control.ps1`
to `%LOCALAPPDATA%\HoploDex`, and adds a Startup-folder shortcut that runs
it minimized at each sign-in. The script keeps
`claude --remote-control --name <host>-hoplodex --permission-mode auto`
running in the checkout (`<host>` is the machine's short host name, in lower
case): it starts it again whenever it stops, 30 seconds later. Auto mode
matches the lead's: a session in another mode holds each message from the
lead for your approval. The lead sends per-OS batches, and the session runs
each with the `speckit-os-batch` skill and reports back. Before the first
sign-in, answer Claude Code's one-time questions over SSH, since the
minimized window would wait on them: in the checkout, run `claude` and
`/login`, then `claude --remote-control --permission-mode auto`, trust the
folder, enable Remote Control, accept auto mode if asked, and `/exit`. After
changing the script, rerun `setup-user.ps1 -ClaudeRemoteControl` (or copy it
to `%LOCALAPPDATA%\HoploDex`) and restart it with `-Restart`.

Claude Code downloads updates in the background, but a running `claude`
keeps its version until it restarts. So during one hour a day (`-UpdateHour`
on the script, 5 a.m. by default, `-1` for never) the script restarts it
if a newer version is installed. To restart it now, from SSH:

```powershell
powershell -File $env:LOCALAPPDATA\HoploDex\claude-remote-control.ps1 -Restart
```

Either way it stops `claude` with Ctrl+C, and forcibly only if that
doesn't work within 30 seconds, and the new `claude` starts a new
conversation under the same name. Whatever the old one was doing stops, so
restart between tasks. To stop it until the next sign-in, close its window.

**E2E on Windows:** `npm run test:e2e` runs there, through the embedded
WebDriver server, with no driver to install, and is isolated from your real
data like everywhere else (see [Test isolation](#test-isolation)). The
real-input tests skip themselves, by decision (see
[Real keyboard and mouse input](#real-keyboard-and-mouse-input)).
`npm run screenshots` runs there too, with the window on the desktop. Its
frame takes some of the 1200×800, so the shots are 1184×761. Full-page shots
grow the window past the bottom of the screen, which Windows allows. So take
before/after pairs on one platform.
With Windows' "Show animations in Windows" off (Settings › Accessibility ›
Visual effects), WebView2 reports reduced motion and us7's catalogue-plate
test fails; `setup-system.ps1 -AutoLogon` turns it on.
A WebDriver screenshot leaves out the PDF surface (a child web view), so where
Linux takes the X display with `import -window root`, us13's pixel checks and
the walk's viewer shots (`shotDisplay`) run `e2e/scripts/window-shot.ps1` on
Windows. It saves the app window's client area, so a point in the page is the
same point in the picture, and the window must be on top and uncovered.

## Install dependencies

```bash
npm install
```

## Build

```bash
npm run build                                              # tsc typecheck + vite build -> dist/
cargo build --manifest-path src-tauri/Cargo.toml            # backend, debug profile
npm run tauri dev                                           # run the full app in dev mode (hot reload)
npm run tauri build                                         # production installer/bundle for this OS
npm run bundle:linux                                        # the same on Linux, with a working AppImage (below)
```

Cargo never deletes old build outputs. Each change to a test or a dependency
leaves the previous test programs and incremental caches behind in
`src-tauri/target`, so it grows by gigabytes a week. Clear it every so often:

```bash
cargo clean --manifest-path src-tauri/Cargo.toml                                # on the host
scripts/dev-container.sh find src-tauri/target -mindepth 1 -delete             # the container's target volume
```

In the container, `src-tauri/target` is the volume's mount point. `cargo
clean` empties it, then fails because it can't remove the mount point
itself, so the container command empties the volume with `find` instead.

### Linux AppImage

On Linux, build the bundles with `npm run bundle:linux` (in the dev
container: `scripts/dev-container.sh npm run bundle:linux`), not
`npm run tauri build`. Arguments go through to `tauri build`, so
`npm run bundle:linux -- --bundles appimage` builds only the AppImage.

A plain `tauri build` AppImage carries the build machine's display-stack
libraries (`libwayland-*`, `libxkbcommon`, `libxcb-*`, `libXau`, `libXdmcp`)
and puts them ahead of the host's. On a host with a newer Mesa than the build
machine (Arch and CachyOS, Fedora 44), WebKitGTK then can't create an EGL
display: the window opens grey, and the terminal shows
`Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...`
([tauri-apps/tauri#15976](https://github.com/tauri-apps/tauri/issues/15976)).
`scripts/build-appimage.sh` sets `LINUXDEPLOY_EXCLUDED_LIBRARIES` so
linuxdeploy leaves those libraries out and the host's are used, then extracts
the AppImage and fails if any are still inside. Only the linuxdeploy that
Tauri CLI 2.12 and later downloads reads that variable. The `.deb` and `.rpm`
use the system's libraries anyway.

To check an AppImage on a machine with a newer Mesa without a desktop session
or your real data, run it under `xvfb-run` with throwaway `HOME` and `XDG_*`
directories and no session D-Bus. The broken build aborts at once with the EGL error, and the
fixed one shows the chooser.

## Test

```bash
cargo nextest run --manifest-path src-tauri/Cargo.toml   # Rust unit + integration tests, real temp SQLCipher DB
cargo test --manifest-path src-tauri/Cargo.toml --doc    # Rust doctests (nextest doesn't run them; there are none today)
npm test                                                 # Vitest frontend unit tests (jsdom)
npm run build && npm run test:e2e                        # WebdriverIO E2E, driven against the built app
```

To run part of a suite:

```bash
cargo nextest run --manifest-path src-tauri/Cargo.toml -E 'binary(firearm_lifecycle_test)'                 # one integration-test file
cargo nextest run --manifest-path src-tauri/Cargo.toml -E 'binary(firearm_lifecycle_test) & test(<name>)'  # tests in it whose name contains <name>
cargo nextest run --manifest-path src-tauri/Cargo.toml <name>                                              # tests anywhere whose name contains <name>
npx vitest run src/features/firearms/FirearmForm.test.tsx                                                 # one Vitest file
npm run test:e2e -- --spec e2e/specs/us1-record-firearm.e2e.ts                                            # one E2E spec

cargo test --manifest-path src-tauri/Cargo.toml --test firearm_lifecycle_test <name>  # the same without nextest
```

The performance budgets (search 500 ms, actions 1 s, suggestions 50 ms at
10,000 records) are timed only in a release build, so `performance_test.rs`
is `#[ignore]`d and the default run skips it. **Run it before opening
any pull request that touches search, listing, persistence or the `ops` layer
(the pull request must note its impact against the budgets, per the
constitution), and before every release.** Run the tests one at a time so that
none is timed while another copies a database, and read the timings with
`--nocapture`:

```bash
CARGO_PROFILE_RELEASE_PANIC=unwind cargo test --manifest-path src-tauri/Cargo.toml \
  --release --test performance_test -- --ignored --nocapture --test-threads=1
```

`panic=unwind` is needed because `cargo test --release` builds the library
for the tests with unwinding and for the binary with the profile's
`panic = "abort"`, and the two then fail to link.

This command stays on `cargo test` on purpose: nextest runs each test in its
own process, so it would repeat the 10,000-record seeding that
`performance_test.rs` does once per process under `cargo test`. If you do run
it with `cargo nextest run --release --run-ignored all`, `.config/nextest.toml`
puts `performance_test` in a one-at-a-time group that also holds every CPU
slot, so no other test runs beside it.

`npm run test:e2e` builds the app with `cargo build --profile e2e --features
custom-protocol,e2e` and drives it through the WebDriver server compiled
into it. That binary embeds whatever is in `dist/`, so **run `npm run build`
first** (the same goes for `npm run screenshots`).

Spec files run in parallel, each in a WebdriverIO worker of its own: up to
`HOPLODEX_E2E_WORKERS` at once, by default half the CPUs and at most 4. Each
worker runs its own app (a few hundred MB for WebKitGTK), so more workers
than that gain little and slow every step. `wdio.conf.ts` builds the app and
the human-testing seed once, in `onPrepare`, before any worker starts. For
debugging, run one spec file at a time, so the log isn't interleaved:

```bash
HOPLODEX_E2E_WORKERS=1 npm run test:e2e
```

The `e2e` profile (`[profile.e2e]` in `src-tauri/Cargo.toml`) is the release
profile without fat LTO, with 16 codegen units and incremental builds, so
touching the backend rebuilds in seconds instead of a minute. It keeps
`panic = "abort"`. The trade-off: E2E doesn't drive a byte-identical copy of
the shipping binary. (It never did, because of the in-memory keyring.) Before
a release, run the suite once against the shipping profile:

```bash
npm run build && HOPLODEX_E2E_PROFILE=release npm run test:e2e
```

`HOPLODEX_E2E_PROFILE` is `e2e` (the default) or `release`; it picks the
profile `wdio.conf.ts` builds the app and the seed with, and the binary it
launches (`target/<profile>/hoplodex`).

The `e2e` Cargo feature is for builds that must never reach a shipped one. It
compiles in `tauri-plugin-wdio-webdriver`, a W3C WebDriver server on
`127.0.0.1` at `TAURI_WEBDRIVER_PORT` (#29). It has no authentication and runs
any script in the page, so `scripts/check-no-webdriver.mjs` checks that a
shipped build has none of it: the dependency graph in `npm run audit`, and the
release binary in `scripts/build-appimage.sh`. The feature also implies
`mock-keyring`, and lets
`HOPLODEX_E2E_IDLE_MINUTE_SECONDS` set how many seconds the idle lock counts
as one minute (default 60). The harness sets it to 3 for
`us9-locking.e2e.ts`, so its idle-lock test waits seconds, not a real minute;
the notice and the settings still say "1 minute". The scaling's unit tests
(`session/idle.rs`) run in the default Rust test run.

The harness, not a driver, owns the app's process (`e2e/support/app.ts`):
`wdio.conf.ts` launches the app before each spec file's session, with the
sandbox's environment and `TAURI_WEBDRIVER_PORT`, waits for its server to
answer, and kills it afterwards. Each worker has a port of its own: 4445 for
the run's first worker, and one up for each later one (worker `0-5` uses
4450). A spec that relaunches the app calls
`relaunchApp()`, which kills it (SIGKILL, so no exit handler runs, like a
crash), starts it again on the same sandbox, display and port, and opens a
new session.
Ending a WebDriver session alone doesn't close the app. The server runs
scripts and dispatches keys in the page, so key presses are synthetic
(untrusted) `KeyboardEvent`s; `e2e/support/ui.ts` already clicks and fills
through page scripts. For genuine input, see "Real keyboard and mouse
input" below.

On Linux each worker starts an isolated Xvfb virtual display of its own
(`e2e/support/display.ts`) and points `DISPLAY` at it, so the app never
touches your real desktop and parallel workers never share a screen, a
pointer or a keyboard. The harness self-heals after an interrupted prior run,
killing anything left over on a worker's port before launching the app.

The E2E suite kills the app rather than quitting it, so a script checks
that decrypted document copies are removed when the app quits, against the
built E2E binary:

- **Linux:** `xvfb-run -a python3 e2e/scripts/quit-cleanup.py` (needs Xvfb,
  no other packages). Window closed, SIGTERM, SIGHUP and SIGINT.
- **macOS:** `python3 e2e/scripts/quit-cleanup.py`, in a logged-in desktop
  session (in a tart VM, `scripts/tart-vm.sh gui python3
  e2e/scripts/quit-cleanup.py`). Window closed, Cmd+Q, SIGTERM, SIGHUP and
  SIGINT. It presses the close button through the Accessibility API and
  posts Cmd+Q to the app, so whatever runs it needs Accessibility.
- **Windows:** `powershell -ExecutionPolicy Bypass -File
  e2e/scripts/quit-cleanup.ps1`, in a desktop session. Window closed,
  `taskkill` without /F, and a log-off (`WM_QUERYENDSESSION` and
  `WM_ENDSESSION` to every window, then the process ended at once, as
  Windows may).

None needs the real directories: each launch gets a scratch sandbox. An
E2E build never hands an opened document to the OS either, so no real
viewer starts: `open_document` writes the copy's path to
`HOPLODEX_E2E_OPENED_LOG` instead, which us4 checks.

Feature 007 adds two more E2E-only seams, both read only by a build with the
`e2e` feature. `HOPLODEX_E2E_CONSENT` answers the native confirmation before a
document goes to another app, which no WebDriver can click: `open` answers
"Open in another app", anything else (or nothing) "Cancel". Each request's
title is appended to the file `HOPLODEX_E2E_CONSENT_LOG` names, which the
harness points into the sandbox. `wdio.conf.ts` sets the answer to `open` for
every spec, and a spec that tests the dialog sets its own and calls
`relaunchApp()`, since the app reads it at start-up.
`HOPLODEX_E2E_PDF_PREVIEW=off` makes the start-up check say the computer's PDF
viewer can't be used, so PDFs aren't previewable while TIFF and text still
are (a spec file named `*-pdf-off.e2e.ts` gets it for its whole session). Both
variable names are among the markers `scripts/check-no-webdriver.mjs` looks
for, so a shipped binary that carries either one fails `build-appimage.sh`'s
check, just as one with the WebDriver server does.

WebDriver drives only the main web view. What goes on inside the PDF surface,
a child web view, is the PDF surface check's.

### PDF surface check

The PDF surface (specs/007-document-preview/research.md §4 and §23) is a web
view that shows a document the app doesn't trust, and each OS builds it
differently, so one script per OS builds `src-tauri/examples/pdf_surface_check.rs`
(which uses the app's own `services::preview::surface`, not a copy) and drives
it with real input. It shows hostile, truncated, bit-flipped and 10 MB PDFs,
probes what a frame can reach (network, IPC, commands), clicks every toolbar and
context-menu item and presses the viewer's shortcuts, and fails if anything
reached the network, a command or the disk. It prints the 10 MB PDF's time to
first paint. Run it on every OS before a pull request that touches
`services/preview/surface/` or the viewer's page area, and whenever the web view
engine is updated (Tauri, wry, WebKitGTK, WebView2):

- **Linux:** `scripts/dev-container.sh scripts/pdf-surface-check.sh`, under Xvfb
  with XTest input and throwaway `HOME` and `XDG_*` folders.
- **macOS:** `scripts/macos/pdf-surface-check.sh`, from a Mac with the tart VM
  set up (it starts the VM, copies the checkout in, runs the check in the VM's
  desktop session and copies the log back). The `--hud-on` variant leaves
  WebKit's PDF toolbar on and passes only if the watch on the Preview copy
  deletes it, closes the surface and sets the PDF preview hold; run it too.
- **Windows:** `powershell -ExecutionPolicy Bypass -File
  scripts\windows\pdf-surface-check.ps1`, in the signed-in desktop session, at
  100% scaling.

Each takes `shows` to run the 3-page PDF and take a screenshot instead. Logs and
screenshots go to `e2e/screenshots-out/pdf-surface/`. Exit codes: 0 passed, 1 no
window, 2 no load, 3 usage, 4 a check failed.

### WebKit's sandbox on a Linux host

On Linux the app turns on WebKit's web-process sandbox (bubblewrap) for the
whole app, but only where a probe shows it works: at start-up it runs itself
once as `hoplodex --webkit-sandbox-probe` and sets `WEBKIT_FORCE_SANDBOX=1`
only if that child loads a page. A host needs `bubblewrap` and `xdg-dbus-proxy`
(and no AppArmor rule that restricts user namespaces for it); without them the app runs unsandboxed and logs why. The
dev container never gets the sandbox: podman blocks the `/proc` mount bubblewrap
needs, and WebKit skips its sandbox where `/run/.containerenv` exists, so the
probe doesn't run there either. So the container's E2E and surface-check runs
are unsandboxed. Before merging a change to the preview, run the E2E suite once
on a Linux host (`npm run build && npm run test:e2e`) and the surface check
there (`scripts/pdf-surface-check.sh`, without the container) and note the
result in the pull request.

A checkout you also use with the container has an empty `node_modules` on the
host, since the container keeps its own in a volume mounted there, and the
host can't write to it. Run the host's E2E from a second copy of the checkout
(`git ls-files` plus `dist/`, then `npm ci` there; point its `src-tauri/target`
at a host build folder), not from this one. To see that a run is sandboxed,
look at the main window's `WebKitWebProcess`: its `/proc/<pid>/ns/pid` and
`ns/mnt` differ from the shell's. The app's log line isn't in the E2E output.
The probe's child takes about 165 ms (the first run after boot about 315 ms),
against 158 ms for the same child with the sandbox off.

### Real keyboard and mouse input

WebDriver's clicks and keys don't reach WebKitGTK the way a person's do, so
some bugs a person sees don't reproduce under them. Examples: a focus ring
that shows or doesn't (`:focus-visible` after a script moves focus), or a
dialog whose layout only corrects itself on the next real key press. When a
report says "after a mouse click" or "when I press Tab" and a spec can't
reproduce it, send real X11 input instead. `e2e/scripts/x11-input.py` sends
it through XTest to the worker's Xvfb display (`DISPLAY`), and `e2e/support/realInput.ts`
wraps it for specs:

```ts
import { realClick, realKey } from "../support/realInput";

await realClick("button.hd-db-menu"); // a real pointer click
await realClick('[role="menuitem"]*=Restore from a backup'); // WebdriverIO's text match
await realKey("Tab"); // X keysym names
await realKey("Shift_L+Tab"); // + for a chord
```

It needs `libX11`, `libXtst` and `python3` (all in the dev container) and
runs on Linux only. The first call moves the pointer once to find where the
window sits on the screen. Keys go to the window under the pointer.

A test that sends real input starts with `skipWithoutRealInput(this)` (from
`realInput.ts`, in an `async function`, not an arrow, for Mocha's `this`),
which skips it on other platforms. A block whose tests all do calls it from a
`before` hook. So does a test that needs what such a test did (a record it
added, a database it left open). Off Linux, `realClick` and `realKey` throw
if a test reaches them without it.

Real input stays on Linux only, by decision (#27): Windows and macOS skip
these tests rather than port the helper. Two kinds need it. WebKitGTK
regressions (a focus ring, a first layout) only matter on Linux. The
keyboard-only flows (us10, us11, us12) need it on every platform, because
the embedded WebDriver's key presses are synthetic: they don't move focus on
Tab or type text. So use real input only for what needs it, and keep it out
of anything else's way. A check that needs it goes in a test of its own,
which leaves the database as it found it, and the other tests don't depend
on it. us8's restore-dialog regressions are an example. The screenshot walk
uses no real input, so it runs everywhere.

To track down a bug of this kind, write a throwaway spec that opens the
screen with real input and logs what you need from the page with
`browser.execute`: `getBoundingClientRect()` of the elements involved,
`document.activeElement.matches(":focus-visible")`, and, to catch a first
layout that differs from the settled one, a snapshot taken from a
`MutationObserver` the moment the element appears. Then change one thing
at a time, by injecting a `<style>` before opening the screen, and compare.
Run it with `npm run test:e2e -- --screenshots=<dir> --spec <file>`, look
at the screenshots, and delete the spec before committing.

### Test isolation

Your real application data is more than one file. It is every `.hoplodex`
database you have created or opened, wherever you keep it, and its backups
(in `HoploDex backups` next to it, or the folder you chose);
`machine.json`, with the recent-databases list, in the app's config
directory (`~/.config/io.github.exodious.HoploDex/` on Linux); the suggested
`<Documents>/HoploDex` folder; and each saved passphrase, a
`passphrase:<database id>` entry under `io.github.exodious.HoploDex` in the
OS keyring.
The database from before the passphrase model,
`~/.local/share/com.hoplodex.app/hoplodex.db`, and its key in the keyring
count too; nothing reads them any more. None of the tooling here touches any
of it:

- Rust tests use `tests/support::TestDb`, a real SQLCipher database in a temp
  directory, created by `db::create_database` with a fixed test passphrase
  and the production cipher settings. Tests never mock the database, and
  take every path (database, config directory) as a parameter.
  `MachineSettings::load` starts with the keyring off, so they never reach the
  OS keyring. The saved-passphrase tests in `tests/keyring_test.rs` run only
  with `--features mock-keyring`, against keyring-core's in-memory store:
  `cargo nextest run --manifest-path src-tauri/Cargo.toml --features mock-keyring -E 'binary(keyring_test)'`.
- `e2e/wdio.conf.ts` gives each session (one spec file, in a worker of its
  own) a throwaway sandbox. An E2E build takes its config and cache
  directories and its documents folder (the suggested place for a new
  database) from `HOPLODEX_E2E_CONFIG_HOME`, `HOPLODEX_E2E_CACHE_HOME` and
  `HOPLODEX_E2E_DOCUMENTS`, never from the OS, and won't start without them
  (`src-tauri/src/app_dirs.rs`; every command gets its directories there).
  That's what isolates Windows, whose known folders ignore `APPDATA` and the
  like. The webview's own data goes into the sandbox too: through `XDG_*`
  directories on Linux (with a `user-dirs.dirs`) and `HOME` on macOS. On
  Windows the app puts each web view's data folder (`main-webview` and
  `preview-webview2`) under `HOPLODEX_E2E_CACHE_HOME`, so no
  `WEBVIEW2_USER_DATA_FOLDER` is set. Each spec starts at
  a first run and creates its database by typing a location in the sandbox
  (`createDatabase()` in `e2e/support/ui.ts`), or unlocks the seeded one
  with its passphrase (`unlock()`). There is no database key in the
  environment: a database opens with its passphrase alone, as in the app.
  E2E builds use the
  `e2e` feature, which includes `mock-keyring`, an in-memory keyring for saved
  passphrases, since a headless session can't unlock a real one. The harness keeps it in
  `keyring.json` in the sandbox (`HOPLODEX_E2E_KEYRING_FILE`) so a remembered
  passphrase survives a relaunch, and launches any `*-no-keyring.e2e.ts` spec
  with `HOPLODEX_E2E_KEYRING=unavailable`, a computer without a keyring.
- E2E steps don't sleep. The frontend counts its backend calls and pending
  search debounces (`src/lib/busy.ts`, `window.__hoplodexBusy`), and the
  helpers in `e2e/support/ui.ts` call `settle()` after each action, which
  waits for that count to reach 0, for finite animations shorter than a
  second to end, and for two painted frames. Anything it can't see (a smooth
  scroll, a thing appearing) gets a `waitUntil` on that condition, not a
  `browser.pause()`.
- `scripts/human-testing.sh` and `src-tauri/examples/human_seed.rs` point the
  app at `.human-testing/`: via `XDG_*_HOME` on Linux, and on macOS via a
  `HOME` of its own there (cargo, rustup and npm keep the real one). The seed writes only into a
  directory that is new, empty or holds the `.hoplodex-sandbox` marker it
  left there, refuses anything inside the real data, config or documents
  directory, and never touches the keyring.
- The [development container](#development-container-linux-recommended) has
  none of the host's app data at all.

## Screenshots

For pull requests that change the UI, take screenshots of the real app
(WebKitGTK, the engine users get), not of a browser:

```bash
npm run build
npm run screenshots                            # -> e2e/screenshots-out/ (git-ignored)
npm run screenshots -- --screenshots=/tmp/pr   # somewhere else
```

This runs `e2e/screenshots/screens.e2e.ts` through the E2E harness, under Xvfb
at a fixed 1200×800 window (for Windows, see [Windows](#windows)). It starts
at the database chooser listing the
[human-testing databases](#human-testing), seeded into the session's
throwaway sandbox, shoots it and the create dialog, unlocks "Main collection"
with the seed's passphrase, and walks the main screens and dialogs
(collection list and tiles, a full record, the record scrolled so its pinned
strip shows, the edit/coverage/dispose dialogs, the unsaved-changes question,
add firearm, insurance, a policy, import and export), then the database menu's
dialogs (settings, change passphrase, restore from a backup, the guide), the
pending-changes question after a lock, the closing screen with its backup
bar, and "Shared collection" refused as open on another computer. Then `e2e/screenshots/first-run.e2e.ts`, in an
unseeded sandbox, shoots the first-run chooser and a new database's
disk-encryption note. Every screen is taken in light and dark mode, writing
`<nn>-<screen>-<theme>.png`. Long pages and dialogs are captured whole. The
names don't change between runs, so for before/after pairs, run it on the base
branch and then on yours:

```bash
git switch develop && npm run build && npm run screenshots -- --screenshots=/tmp/before
git switch my-branch && npm run build && npm run screenshots -- --screenshots=/tmp/after
```

When a change adds a screen, add it to the walk. For a one-off shot inside
any other spec, call `shot("name")` from `e2e/support/screenshots.ts`. It
saves only when the run was started with `--screenshots`, so a normal
`test:e2e` is unaffected:

```bash
npm run test:e2e -- --screenshots --spec e2e/specs/us1-record-firearm.e2e.ts
```

The seed writes into a new `seed` folder in the sandbox, and the harness
moves the app's config directory there so its recent list names the seeded
databases. Specs read the seed's passphrase from
`HOPLODEX_E2E_SEED_PASSPHRASE`, and the sandbox's documents folder from
`HOPLODEX_E2E_DOCUMENTS`.

## Tuning the chooser's drawing

The database chooser's catalogue plate (the hoplon with its owl, the Greek
key and the firearm drawings) draws itself in stroke by stroke, then cycles
entry 2 through the firearm drawings until a database is opened. Every time
it uses, and how one drawing gives way to the next (`CYCLE_STYLE`), is in
`src/features/databases/plate/timing.ts`, one value per line. The app builds
in whatever that file holds.

To judge a change, watch it in the tuner rather than editing numbers blind:

```bash
npm run tuner         # http://localhost:1430: sliders, a scrubbable timeline, presets, Save
npm run tuner:build   # -> dist-tuner/plate-tuner.html, one file to open anywhere (no Save)
```

The tuner plays the app's own plate and animation code. Paused, it shows the
moment on the timeline with the new values; playing, it starts again when
you let go of a slider. "The whole loop" stretches the timeline over the
cycle, and the presets under "Changing drawings" each play one way of
changing drawings from just before the first change. **Save to timing.ts**
(served by `npm run tuner` only) writes what you changed into the file, and
the page reloads with them as its values; the built page lists your changes
to copy into it instead. A link can open it on a preset and paused at a
moment: `?preset=2&view=cycle&at=8.5`.

Inside the [development container](#development-container-linux-recommended)
the tuner's server can't be reached from your browser, so there use
`npm run tuner:build` and open `dist-tuner/plate-tuner.html` from your
checkout.

Entry 2's drawings, their captions and their real lengths (which size the
scale bars) are in `plate/entries.ts`; the artwork is `plate/PlateArt.tsx`.
The app shows the drawings in an order shuffled once at startup, and the
draw-in draws whichever comes first; each keeps its catalogue number (2 to
5, from its place in `PLATE_ENTRIES`). The tuner always plays them in
`PLATE_ENTRIES`' order, starting with the rifle.

## Program icon

`src-tauri/icons/` is generated from `tools/icons/AppIcon.tsx`, which draws
the chooser plate's shield and owl on a blued-steel tile. After changing
either, regenerate every size (16 to 1024 px), `icon.ico` and `icon.icns`:

```bash
npm run icons
```

At 32 px and below the icon has its own drawing, a solid owl on a plain rim,
as the top bar's mark does (`src/features/app/BrandMark.tsx`).
`src-tauri/icons/source/` keeps the two drawings as SVG, for reference.

## Human testing

To poke at the app by hand (look and feel, workflows) against a realistic
collection instead of an empty one:

```bash
scripts/human-testing.sh                # seed on first use, then launch `tauri dev`
scripts/human-testing.sh --reset        # throw the data away and reseed
scripts/human-testing.sh --extra 200    # also generate 200 plain firearms
```

The seed (`src-tauri/examples/human_seed.rs`) goes through the app's own
command layer, so it covers photos, documents, dispositions and every
insurance state: healthy, under-insured, uninsured (expired policy), and a
policy expiring soon. Policy dates are relative to the day it is seeded.
Some firearms carry real photos from `src-tauri/examples/seed-photos/`, all
public domain or freely licensed, whose `README.md` records each one's
source, licence and the credit line to use if they appear on a web page; one
record carries ten generated images for size and count testing. The
data lives in `.human-testing/` (git-ignored): two databases in `HoploDex/`,
"Main collection" (the full collection) and "Shared collection" (left open
by "Workshop PC", with pending changes), both opened with the passphrase the
script prints; two backups of "Main collection" in `Backups/`, its custom
backup folder; the `machine.json` listing both databases; and three
spreadsheets in
`import-samples/` (clean, conflicting and invalid rows) to try File > Import
with. The app is pointed at it, so your real collections are never opened:
on Linux through `XDG_*_HOME` and a `user-dirs.dirs` whose documents folder
is the sandbox; on macOS through a `HOME` of its own, `.human-testing/home/`,
whose `Library/Application Support` is `config/` and whose `Documents` is the
sandbox (the web view's data and caches land in it too). Linux and macOS
only. A `.human-testing/` made before the passphrase model has
no sandbox marker, and the seed refuses it: delete it and run the script
again.

**Changing the data model?** Update the seed in the same change. The seed
compiles when a new field is simply left out, so
`src-tauri/tests/human_seed_coverage_test.rs` runs it into a temporary
sandbox and fails if any column is empty in every row of both databases, if a
`CHECK ... IN` value never appears in either, or if no import sample fills a
spreadsheet column. Fix a
failure by seeding a record that uses the new field (and adding it to the
import samples), not by loosening the test.

In the [development container](#development-container-linux-recommended),
run it as `scripts/dev-container.sh --gui scripts/human-testing.sh`. The
container keeps the data in `~/human-testing` in its home volume (via
`HUMAN_TESTING_DIR`), not in the checkout's `.human-testing/`.

## Linting & formatting

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
npm run lint             # eslint, --max-warnings 0
npm run format:check     # prettier
```

## Dependency audit

```bash
npm run audit            # all three of the below
npm run audit:npm        # audit-ci over `npm audit`: dependencies and devDependencies
npm run audit:cargo      # cargo-deny against the RustSec advisory database
npm run audit:licenses   # the license audit, see below
```

Run it with the lint and test commands before every pull request. It fetches
the advisory databases, so it needs network access. The `Dependency audit`
workflow (`.github/workflows/audit.yml`) also runs it on every pull request to
`main`, Dependabot's included, and daily on `main`. A pull request that changes
only Markdown (`*.md`) passes without the audit running. `cargo-deny` comes with the
[development container](#development-container-linux-recommended). On a host,
install it with `cargo install cargo-deny --locked`.

What fails the audit:

- **npm**: any high or critical advisory, in devDependencies too. Moderate and
  low ones show up in `npm audit` but don't fail it.
- **Rust**: any vulnerability advisory, whatever its severity (many RustSec
  advisories have no CVSS score), and any unmaintained or unsound notice.

Advisories are published all the time, so the audit can start failing on a
branch that changed no dependencies. Fix that in its own commit.

To fix a finding, update the dependency (`npm audit fix`, `npm update <pkg>`,
`cargo update -p <crate>`). If a parent pins a vulnerable transitive
dependency, try an npm `overrides` entry, and test the path that uses it.

Add an exception only when there is no fix to take: no patched release, or
a parent that won't accept it. A critical advisory needs both, no patch
anywhere and an analysis. A high one needs a mitigation or a justification.
Record it next to the tool's config:

- **npm**: an `allowlist` entry in `audit-ci.jsonc`, scoped to the dependency
  path you analysed (`"GHSA-…|*parent>vulnerable-pkg*"`) rather than the bare
  advisory ID, so the same advisory arriving another way fails again.
- **Rust**: an `ignore` entry in `src-tauri/deny.toml`, with a `reason`.

Above the entry, write the advisory, why it can't be fixed yet, whether and how
it reaches HoploDex (shipped app or dev tooling only, and whether the
vulnerable code path is ever called), and a date to review it. Neither tool
expires entries, so that date is what brings it back. Remove the entry once the
fix is available.

## License audit

HoploDex is released under GPL-3.0-only (`LICENSE`), so everything shipped
with it must be under a GPLv3-compatible license. `npm run audit:licenses`
checks the dependency trees, and `npm run audit` includes it. It works offline
once the Cargo registry is fetched. Run it again whenever you add or update a
dependency, and before every release.

- **npm**: `scripts/check-npm-licenses.mjs` reads `package-lock.json` and
  checks every package that isn't dev-only, since Vite bundles those into the
  app. devDependencies aren't distributed, so their licenses don't matter
  for the release. `node scripts/check-npm-licenses.mjs --all` lists them too,
  without failing on them.
- **Rust**: `cargo deny check licenses`, under `[licenses]` in
  `src-tauri/deny.toml`. It covers every crate for every target platform and
  feature, build and proc-macro crates included.

Both use the same allow list of GPLv3-compatible SPDX licenses (`ALLOWED` in
the script, `allow` in `deny.toml`). Keep the two in step. Before adding a
license, check it against the FSF's
[list of GPL-compatible licenses](https://www.gnu.org/licenses/license-list.html).
A license that's only acceptable for particular packages goes in the script's
`EXCEPTIONS` (npm) or a `[[licenses.exceptions]]` entry (Rust), with the
reason. Today there is one: OFL-1.1, for the `@fontsource` fonts only.

The passphrase strength hint's English dictionary is a second exception, which
the tools can't see because the package declares MIT. `@zxcvbn-ts/language-en`
4.1.1 bundles `commonWords.json`, which its `THIRD_PARTY_LICENSES.md` and
`NOTICE.md` say is derived from the OpenSubtitles 2024 dataset (via OPUS,
Helsinki-NLP) under ODC-BY, a data license whose only condition is
attribution. It's accepted for that package only: the list is data the
strength estimate looks words up in, not code combined with the program. The
release's third-party notices must carry the attribution, including the
package's `NOTICE.md`, which asks to be kept on redistribution: "commonWords.json
contains data derived from OpenSubtitles 2024 (https://opus.nlpl.eu/),
provided by Helsinki-NLP / OPUS, under the Open Data Commons Attribution
License (ODC-BY)."

The tools can't check some things, so check these by hand when they change and
before a release:

- **The strength hint's other word lists**: `wikipedia.json`,
  `firstnames.json`, `lastnames.json` and `wordSequences.json` in
  `@zxcvbn-ts/language-en` 4.1.1, and `passwords.json`, `diceware.json` and
  `adjacencyGraphs.json` in `@zxcvbn-ts/language-common` 4.1.3, state no
  source. Find out where each comes from and under what license before the
  first release.

- **MPL-2.0 crates**: a file carrying MPL's Exhibit B notice ("Incompatible
  With Secondary Licenses") can't be combined with GPL code. When a new
  MPL-2.0 crate turns up (`cargo deny --manifest-path src-tauri/Cargo.toml list -l license`),
  grep its sources in `~/.cargo/registry/src/` for that phrase, leaving out
  the `LICENSE` file, which quotes it.
- **C code built inside a crate**: `libsqlite3-sys` compiles SQLCipher
  (BSD-3-Clause, `sqlcipher/LICENSE` in the crate) and SQLite (public domain),
  but the crate declares only its own MIT license.
- **SQLCipher's crypto library** (`bundled-sqlcipher`): on Linux it links the
  system's OpenSSL `libcrypto`, on macOS CommonCrypto, and on Windows the
  OpenSSL found through `OPENSSL_DIR`, linked into the app statically (the
  `x64-windows-static-md` triplet builds no DLL). OpenSSL
  3.x is Apache-2.0, which is compatible. 1.1.x and older use the
  OpenSSL/SSLeay license, which isn't, so build releases against OpenSSL 3.
  Switching to `bundled-sqlcipher-vendored-openssl` would pull in
  `openssl-src`, and the Rust check would cover it.
- **System libraries**: WebKitGTK, GTK and their stack on Linux (LGPL),
  WebView2 on Windows and WKWebView on macOS are system components. A bundle
  that ships LGPL libraries inside it (an AppImage, for example) must also
  make their source available.
- **Assets that aren't packages**: the type drawings
  (`src/features/browse/typeDrawings.ts`), the icon set
  (`src/components/Icon.tsx`), the chooser's catalogue plate
  (`src/features/databases/plate/`), and the program icon (`tools/icons/`,
  `src-tauri/icons/`) are the project's own work, under the project license.
  The plate's owl is drawn after the owl on the Athenian tetradrachm, a
  5th-century BC coin whose design is in the public domain. Record the source and license of any
  third-party artwork, font or data file before adding it.

Compatible doesn't mean there's nothing to do. MIT, BSD, Apache-2.0, ISC and
OFL all require their copyright and license notices to ship with the app. The
minified bundle and the stripped release binary don't carry them, so a release
needs a third-party notices file. `cargo about` can generate one for the Rust
side.

## Continuous integration

Two focused workflows run on GitHub: CodeQL (`.github/workflows/codeql.yml`)
on every pull request to `main`, on pushes to it and weekly, and the
[dependency audit](#dependency-audit) (`.github/workflows/audit.yml`). In both,
third-party actions are pinned to a commit (#72) and GitHub's own to a release
tag. Dependabot (`.github/dependabot.yml`) opens a weekly pull request to
update them.

Full CI, the format, lint and test commands above and the app build on
Windows, macOS and Linux, **isn't implemented yet** (#25). Run them locally
before every pull request. The WebdriverIO E2E suite needs a display and a
platform WebDriver, so it runs locally too (`npm run test:e2e`).
