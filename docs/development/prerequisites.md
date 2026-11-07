# Prerequisites

_Part of the [development docs](../../DEVELOPMENT.md)._

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

- **Windows**: see [Windows](windows.md).

**End-to-end (E2E) testing extras — Linux only:** an isolated virtual
display. The E2E build carries its own WebDriver server (see [E2E tests](e2e.md)), so
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
