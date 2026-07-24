# HoploDex
Firearm Collection Inventory App

## Development

A Tauri 2.x desktop app: a Rust backend (`src-tauri/`) owns persistence,
encryption, and business logic; a React + TypeScript frontend (`src/`)
owns the UI.

### Prerequisites

**Rust** (stable, 1.75+), via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Linux/macOS
```

On Windows, download and run [`rustup-init.exe`](https://win.rustup.rs).

**Node.js 18+** and npm — via [nodejs.org](https://nodejs.org),
[nvm](https://github.com/nvm-sh/nvm), or your platform's package manager.

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
    librsvg2-dev
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
    webkit2gtk-4.1
  ```

- **macOS**: Xcode Command Line Tools —

  ```bash
  xcode-select --install
  ```

- **Windows**: the
  [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  (the "Desktop development with C++" workload) and WebView2 (preinstalled
  on Windows 10/11; otherwise get it from the
  [WebView2 runtime page](https://developer.microsoft.com/microsoft-edge/webview2/)).

**End-to-end (E2E) testing extras — Linux only:**

Debian/Ubuntu:

```bash
sudo apt install -y webkit2gtk-driver xvfb   # provides WebKitWebDriver + an isolated virtual display
cargo install tauri-driver
```

Arch:

```bash
sudo pacman -S --needed xorg-server-xvfb   # isolated virtual display
cargo install tauri-driver
```

Unlike Debian's `webkit2gtk-driver` package, Arch's `webkit2gtk-4.1` package
does **not** include the `WebKitWebDriver` binary, so it must be built from
source, matching the exact version of `webkit2gtk-4.1` you have installed:

```bash
sudo pacman -S --needed ninja cmake clang lld ruby gperf python unifdef
gem install getoptlong

ver=$(pacman -Q webkit2gtk-4.1 | awk '{print $2}' | cut -d- -f1)
curl -LO "https://webkitgtk.org/releases/webkitgtk-$ver.tar.xz"
tar xf "webkitgtk-$ver.tar.xz"
mkdir webkitgtk-build && cd webkitgtk-build
cmake "../webkitgtk-$ver" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DPORT=GTK -DENABLE_WEBDRIVER=ON \
  -DENABLE_MINIBROWSER=OFF -DENABLE_API_TESTS=OFF -DUSE_GTK4=OFF -DUSE_SOUP2=ON \
  -DENABLE_INTROSPECTION=OFF -DENABLE_SPEECH_SYNTHESIS=OFF -DUSE_LIBBACKTRACE=OFF
ninja WebKitWebDriver
sudo install -m755 bin/WebKitWebDriver /usr/local/bin/
```

Re-run this after every `pacman` update to `webkit2gtk-4.1`, since a
version-mismatched `WebKitWebDriver` will fail to drive the installed
library. `npm run test:e2e` locates the binary via `which WebKitWebDriver`
(falling back to a filesystem search), so anywhere on `$PATH` works.

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
```

Use `--login` (not `--start --unlock` — this daemon version rejects that
combination), which unlocks the login keyring with the blank password from
stdin in one step, creating it on first run. Verify it registered correctly
with `busctl --user list | grep org.freedesktop.secrets` before re-running
`npm run test:e2e`.

On Windows/macOS, E2E tests use their platform's own native WebView driver
instead (no extra install beyond the prerequisites above).

### Install dependencies

```bash
npm install
```

### Build

```bash
npm run build                                              # frontend only -> dist/
cargo build --manifest-path src-tauri/Cargo.toml            # backend, debug profile
npm run tauri dev                                           # run the full app in dev mode (hot reload)
npm run tauri build                                         # production installer/bundle for this OS
```

### Test

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # Rust unit + integration tests, real temp SQLCipher DB
npm test                                          # Vitest frontend unit tests
npm run test:e2e                                  # WebdriverIO E2E, driven against the built app
```

`npm run test:e2e` builds a release binary with `cargo build --release
--features custom-protocol` and drives it via `tauri-driver`. On Linux it
runs under an isolated `xvfb` virtual display (via `xvfb-run`), so it never
touches your real desktop, and it self-heals after an interrupted prior
run (killing anything left over on its ports before starting).

### Linting & formatting

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml && cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml
npm run lint
npm run format:check
```
