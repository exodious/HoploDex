# Developing HoploDex

A Tauri 2.x desktop app: a Rust backend (`src-tauri/`) owns persistence,
encryption, and business logic; a React + TypeScript frontend (`src/`)
owns the UI.

## Build and run it locally

**On Linux with [podman](https://podman.io), that's all you need.** The dev
container has the rest:

```bash
scripts/dev-container.sh --gui npm run tauri dev   # run the app, with its window on your desktop
scripts/dev-container.sh npm run bundle:linux      # build the Linux bundles
```

**Otherwise, install these on the computer itself:**

1. **Rust** stable, 1.97 or newer, from [rustup](https://rustup.rs).
2. **Node.js 24 LTS** from [nodejs.org](https://nodejs.org), then npm 12:
   `npm install --global npm@12`.
3. **What your OS needs to build a Tauri app:**
   - **Debian/Ubuntu:**

     ```bash
     sudo apt install build-essential file pkg-config perl libwebkit2gtk-4.1-dev \
       libjavascriptcoregtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libssl-dev \
       libayatana-appindicator3-dev librsvg2-dev xdg-utils
     ```

   - **Arch:**

     ```bash
     sudo pacman -S --needed base-devel curl wget file openssl appmenu-gtk-module \
       libappindicator-gtk3 librsvg webkit2gtk-4.1 xdg-utils
     ```

   - **macOS:** `xcode-select --install`
   - **Windows** (10 or 11 Pro, Enterprise or Education, or Server 2025): skip
     steps 1 and 2. These two scripts install everything, including Git, Rust,
     Node and the OpenSSL the build needs. The first also turns on Remote
     Desktop and SSH for that user, so read [Windows](docs/development/windows.md)
     first.

     ```powershell
     # From an elevated PowerShell:
     powershell -ExecutionPolicy Bypass -File scripts\windows\setup-system.ps1 -User <your user>
     # Then as that user, from a new PowerShell that isn't elevated:
     powershell -ExecutionPolicy Bypass -File scripts\windows\setup-user.ps1
     ```

**Then, in your checkout:**

```bash
npm install
npm run tauri dev      # run the app, with hot reload
npm run tauri build    # build the installers for this OS, in src-tauri/target/release/bundle/
npm run bundle:linux   # on Linux, use this instead of tauri build
```

`npm run tauri dev` uses your real HoploDex data, as an installed copy would. To
try the app on a throwaway collection, on Linux or macOS, run
`scripts/human-testing.sh` instead. Coding agents always use that or the
tooling below (`CLAUDE.md`, "Never touch the real databases").

## Where to find what you need

Open only the row you need. The quoted names are this file's old section names,
which older specs and commits still cite.

| To | Read | Old section names |
|---|---|---|
| Use the dev container: its options, volumes and `--gui` | [dev-container.md](docs/development/dev-container.md) | "Development container (Linux, recommended)" |
| Set up a host by hand: E2E's display, a keyring over SSH | [prerequisites.md](docs/development/prerequisites.md) | "Prerequisites", "Headless/SSH sessions only" |
| Work on macOS: what's ported, the VM, driving it over SSH | [macos.md](docs/development/macos.md) | "macOS (partly ported, #28)", "macOS in a VM", "Driving the Mac from Linux, over SSH" |
| Set up Windows, drive it over SSH, run E2E there | [windows.md](docs/development/windows.md) | "Windows", "Driving a test machine from Linux, over SSH", "E2E on Windows" |
| Build, clear `target`, make an AppImage | [build.md](docs/development/build.md) | "Install dependencies", "Build", "Linux AppImage" |
| Build or check the Flatpak, update its source lists | [flatpak.md](docs/development/flatpak.md) | "Flatpak", "Where it runs", "Without podman", "What the Flatpak reaches", "The source lists", "The Flatpak check", "On a GitHub-hosted runner" |
| Run the tests, or one file or test; add a Rust test; run the release-only tests | [testing.md](docs/development/testing.md) | "Test" |
| Write or debug an E2E spec, choose its file | [e2e.md](docs/development/e2e.md) | "Test" (its E2E part), "E2E files" |
| Keep tooling away from the real data | [test-isolation.md](docs/development/test-isolation.md) | "Test isolation" |
| Reproduce a bug WebDriver's input can't | [real-input.md](docs/development/real-input.md) | "Real keyboard and mouse input" |
| Drive the app step by step | [wdio-session.md](docs/development/wdio-session.md) | "Drive the app with wdio session" |
| Change the PDF surface, the window's browser controls or WebKit's sandbox | [surface-checks.md](docs/development/surface-checks.md) | "PDF surface check", "Window controls check", "WebKit's sandbox on a Linux host" |
| Cite a spec, or update the attack surface list | [citations-and-attack-surface.md](docs/development/citations-and-attack-surface.md) | "Spec citations", "Attack surface list" |
| Change the schema | [data-layout.md](docs/development/data-layout.md) | "Changing the data layout", "Writing a step", "Testing a step", "`released.txt`", "The stored release databases" |
| Take screenshots, put them in a pull request, retake the store shots | [screenshots.md](docs/development/screenshots.md) | "Screenshots", "Screenshots in a pull request", "The Flatpak's store shots" |
| Try the app by hand on seeded data, or keep the seed in step | [human-testing.md](docs/development/human-testing.md) | "Human testing" |
| Tune the chooser's drawing, regenerate the program icon | [plate-and-icon.md](docs/development/plate-and-icon.md) | "Tuning the chooser's drawing", "Program icon" |
| Lint and format | [lint.md](docs/development/lint.md) | "Linting & formatting" |
| Run the dependency audit, answer a finding, add an exception | [dependency-audit.md](docs/development/dependency-audit.md) | "Dependency audit", "Deprecated npm packages", "Unmaintained crates", "Yanked crates", "The pinned OpenSSL", "Before a release" |
| Run the license audit, answer a notices failure | [licenses.md](docs/development/licenses.md) | "License audit", "System libraries", "Assets that aren't packages", "C code built inside a crate", "Exceptions and clarifications that are behind", "License notices", "What ships", "Answering a failure" |
| See what CI runs, reproduce a job, understand a timing budget | [ci.md](docs/development/ci.md) | "Continuous integration", "The CI workflow", "Timing budgets under CI", "Reproducing a job locally", "The vcpkg pin" |
| Understand how the code is organised | [docs/architecture.md](docs/architecture.md) | |
| See what counts as attack surface | [docs/attack-surface.md](docs/attack-surface.md) | |
