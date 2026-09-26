# Developing HoploDex

A Tauri 2.x desktop app: a Rust backend (`src-tauri/`) owns persistence,
encryption, and business logic; a React + TypeScript frontend (`src/`)
owns the UI.

## Development container (Linux, recommended)

The repo's `Dockerfile` (Debian trixie) has everything below already
installed: Rust, Node 24 LTS with npm 12, the Tauri/WebKitGTK and SQLCipher
build dependencies, `tauri-driver` and `WebKitWebDriver`, `cargo-deny`, Xvfb,
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

The container has none of the host's app data, so nothing run inside it can
reach your real collection (see [Test isolation](#test-isolation)).

To set up a host directly instead (or on macOS/Windows), install the
prerequisites below.

## Prerequisites

**Rust** (stable, 1.75+), via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Linux/macOS
```

On Windows, download and run [`rustup-init.exe`](https://win.rustup.rs).

**Node.js 22+** (tested on 22 and 24 LTS) and **npm 11+** — via
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

  SQLCipher also needs OpenSSL here (macOS uses CommonCrypto and Linux uses
  `libssl-dev`, so neither needs anything extra). Install it, for example
  with [vcpkg](https://vcpkg.io) — `vcpkg install openssl:x64-windows-static-md` —
  and set `OPENSSL_DIR` (e.g. `C:\vcpkg\installed\x64-windows-static-md`) and
  `OPENSSL_STATIC=1` before building. The build stops with "Missing
  environment variable OPENSSL_DIR" otherwise.

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

On Windows/macOS, E2E tests use their platform's own native WebView driver
instead (no extra install beyond the prerequisites above).

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
```

## Test

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # Rust unit + integration tests, real temp SQLCipher DB
npm test                                          # Vitest frontend unit tests (jsdom)
npm run build && npm run test:e2e                 # WebdriverIO E2E, driven against the built app
```

To run part of a suite:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test firearm_lifecycle_test         # one integration-test file
cargo test --manifest-path src-tauri/Cargo.toml --test firearm_lifecycle_test <name>  # tests whose name contains <name>
npx vitest run src/features/firearms/FirearmForm.test.tsx                            # one Vitest file
npm run test:e2e -- --spec e2e/specs/us1-record-firearm.e2e.ts                       # one E2E spec
```

`npm run test:e2e` builds a release binary with `cargo build --release
--features custom-protocol,mock-keyring` and drives it via `tauri-driver`.
That binary embeds whatever is in `dist/`, so **run `npm run build` first**
(the same goes for `npm run screenshots`). On Linux it runs under an isolated
`xvfb` virtual display (via `xvfb-run`), so it never touches your real
desktop, and it self-heals after an interrupted prior run (killing anything
left over on its ports before starting).

The E2E suite can't watch the app exit (WebKitWebDriver ends a session by
killing it), so `e2e/scripts/quit-cleanup.py` checks that decrypted document
copies are removed when the app quits — window closed, SIGTERM, SIGHUP or
SIGINT — against the built binary: `xvfb-run -a python3
e2e/scripts/quit-cleanup.py` (Linux; needs Xvfb, no other packages).

### Test isolation

Your real collection lives in an encrypted database at
`~/.local/share/com.hoplodex.app/hoplodex.db` (on Linux), with its key in the
OS keyring. None of the tooling here opens it:

- Rust tests use `tests/support::TestDb`, a real SQLCipher database in a temp
  directory, created by `db::create_database` with a fixed test passphrase
  and the production cipher settings. Tests never mock the database, and
  take every path (database, config directory) as a parameter.
- `e2e/wdio.conf.ts` gives each session throwaway `XDG_*` directories and a
  stub `xdg-open`. E2E builds use the `mock-keyring` feature, which generates
  a fresh key per launch, so a database left over from one spec would break
  the next.
- `scripts/human-testing.sh` and `src-tauri/examples/human_seed.rs` point the
  app at `.human-testing/` via `XDG_*_HOME`. The seed writes only into a
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
at a fixed 1200×800 window. It opens the
[human-testing collection](#human-testing), seeded into the session's
throwaway sandbox, and walks the main screens and dialogs (collection list
and tiles, a full record, the record scrolled so its pinned strip shows, the
edit/coverage/dispose dialogs, add firearm, insurance, a policy, import and
export) in light and dark mode, writing
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

The seed and the app share a database key via `HOPLODEX_E2E_DB_KEY`, which
only `mock-keyring` (E2E) builds read.

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
policy expiring soon. Policy dates are relative to the day it is seeded. The
data lives in `.human-testing/` (git-ignored): two databases in `HoploDex/`,
"Main collection" (the full collection) and "Shared collection" (left open
by "Workshop PC", with pending changes), both opened with the passphrase the
script prints; the `machine.json` listing them; and three spreadsheets in
`import-samples/` (clean, conflicting and invalid rows) to try File > Import
with. The app is pointed at it through `XDG_*_HOME` and a `user-dirs.dirs`
whose documents folder is the sandbox, so your real collections are never
opened. Linux only. A `.human-testing/` made before the passphrase model has
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
the advisory databases, so it needs network access. `cargo-deny` comes with the
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
  OpenSSL found through `OPENSSL_DIR`, whose DLL ships with the app. OpenSSL
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
  (`src/components/Icon.tsx`) and `src-tauri/icons/` are the project's own
  work, under the project license. Record the source and license of any
  third-party artwork, font or data file before adding it.

Compatible doesn't mean there's nothing to do. MIT, BSD, Apache-2.0, ISC and
OFL all require their copyright and license notices to ship with the app. The
minified bundle and the stripped release binary don't carry them, so a release
needs a third-party notices file. `cargo about` can generate one for the Rust
side.

## Continuous integration

The CI definition lives in `.github/workflows-disabled/ci.yml` and is
**currently disabled**; move it to `.github/workflows/` to enable it. Until
then, run the lint and test commands above locally. When enabled it runs on
every push to `main`/`develop` and on every pull request, on Windows, macOS
and Linux: it builds the frontend, then runs `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test`, `eslint`,
`prettier --check`, `vitest`, and finally `tauri build --no-bundle`.
The Rust crate embeds the built frontend, which is why the frontend builds
first. The WebdriverIO E2E suite isn't part of CI: it needs a display and a
platform WebDriver, so run `npm run test:e2e` locally.
