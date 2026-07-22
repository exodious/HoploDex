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

- **Linux** (Debian/Ubuntu package names — see
  [Tauri's Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux)
  for other distros):

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

```bash
sudo apt install -y webkit2gtk-driver xvfb   # provides WebKitWebDriver + an isolated virtual display
cargo install tauri-driver
```

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
cargo fmt --check && cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml
npm run lint
npm run format:check
```
