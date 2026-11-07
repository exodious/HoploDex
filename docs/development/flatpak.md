# Flatpak

_Part of the [development docs](../../DEVELOPMENT.md)._

`npm run bundle:flatpak` builds the Flatpak from this checkout, offline, checks
it and installs it (`011 US3-6`, `011 FR-014`; the reasons are in
[011's research.md §2, §13 and §15](../../specs/011-flatpak/research.md)):

```bash
scripts/dev-container.sh --flatpak npm run bundle:flatpak       # build, check, install
scripts/dev-container.sh --flatpak npm run flatpak:sources      # after any lockfile change
```

It prints each step as it starts and stops at the first one that fails,
naming it:

1. Installs the GNOME 51 runtime and SDK and the two SDK extensions (Rust and
   Node) from Flathub, only the ones missing (network).
2. `flatpak-builder --download-only` fetches every source the manifest names
   (network): the checkout, and the crates and npm packages in
   `flatpak/cargo-sources.json` and `flatpak/node-sources.json`.
3. `flatpak-builder --disable-download` builds with the build sandbox's
   network off (`011 SC-004`): `npm ci --offline`, `npm run build`, a release
   `cargo build --offline --locked --features custom-protocol`,
   `scripts/check-no-webdriver.mjs` on the binary, and the installs. The
   notices step of `npm run build` runs its `cargo fetch --locked`,
   `cargo metadata` and `cargo tree` against the vendored crates, offline.
4. `scripts/flatpak/contents.mjs` lists what the build carries and fails on
   any file or shared library not in
   [011's data-model.md "The bundle's contents"](../../specs/011-flatpak/data-model.md)
   (`011 FR-017`, `011 FR-018`).
5. `desktop-file-validate` and `appstreamcli validate --no-net`.
6. Compares the notices the Flatpak carries with those the other Linux builds
   make from the same lockfiles (`011 FR-019`); only the copyright years may
   differ, because `git` isn't in the build.
7. Installs the build into the container's Flatpak installation.
8. Installs Flathub's linter (`org.flatpak.Builder`, network, only if missing)
   and runs `flatpak-builder-lint` on the manifest and on the repository with
   `flatpak/linter-exceptions.json` (`--exceptions --user-exceptions`; the
   repository also gets the two screenshot rules a local build cannot satisfy,
   `011 research.md` §4).
9. Writes `flatpak/out/HoploDex-<version>-x86_64.flatpak` with
   `flatpak build-bundle --runtime-repo=<Flathub's>` and prints its path. It
   attaches and publishes nothing (`011 SC-009`).

The build is of the checkout as it is, uncommitted changes included, except
`node_modules`, `dist`, `src-tauri/target`, `.git`, `.claude`, `flatpak/out`,
`.human-testing` and `.wdio`. `flatpak/out/` is git-ignored, so the host can
reach the `.flatpak` file there.

## Where it runs

**Where it runs.** Only in the dev container started with `--flatpak`
(`011 FR-023`), apart from the host and runner exceptions below.
`flatpak-builder` and a running Flatpak use `bwrap`, which needs the real
`/proc`, and a rootless container masks parts of it, so `--flatpak` unmasks `/proc` for that run only
(`--security-opt unmask=/proc/*`). It also mounts a volume for
`flatpak-builder`'s state (downloads, build folders; several gigabytes) per
checkout, named in `HOPLODEX_FLATPAK_STATE`. The runtime, SDK and extensions
(about 2 GB) go in the container's own user installation in the shared
`/home/dev` volume, so the first run in a container fetches them and later
ones, in any checkout, don't; the first run also fetches every crate and npm
package. Run it in the background with a log (the first run is long), and
see [Development container](dev-container.md) for
`--reset-volumes`, which also deletes the state volume.

## Without podman

**Without podman,** a Linux host needs `flatpak`, `flatpak-builder`,
`appstream` (`appstreamcli`), `desktop-file-utils`, `binutils` (`readelf`),
`dbus` (`dbus-daemon`), `python3-dbusmock`, and the generators' Python modules
(`aiohttp`, `tomlkit`, `PyYAML`) with
[flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools) at
the commit the `Dockerfile` pins (`FLATPAK_BUILDER_TOOLS_COMMIT`, cloned to
`/opt/flatpak-builder-tools`, or wherever `HOPLODEX_FLATPAK_BUILDER_TOOLS`
says). The scripts must never use your own Flatpak installation, so they
refuse on a host unless `FLATPAK_USER_DIR` names an absolute, empty or
throwaway folder and `--i-know-this-is-a-host` follows the command
(`FLATPAK_USER_DIR=/var/tmp/hoplodex-flatpak npm run bundle:flatpak -- --i-know-this-is-a-host`);
the guard (`scripts/flatpak/guard.mjs`) says what to change otherwise. The
build's state folder (downloads, build folders and the repository)
defaults to a folder beside it, `<FLATPAK_USER_DIR>-state`
(`/var/tmp/hoplodex-flatpak-state` above), so no other variable is needed;
set `HOPLODEX_FLATPAK_STATE` to an absolute path to put it elsewhere. The
first run then fetches the 2 GB into those two folders, and deleting both
removes everything it made. (A GitHub-hosted runner that opted in needs
neither the flag nor the default: "The Flatpak check" below.) **`npm run test:flatpak` does not run on a
host, with or without that flag** (`011 FR-023`; only a GitHub-hosted runner
set up as in "The Flatpak check" is an exception): it refuses before it
touches anything, saying so. The check needs what only the dev container
has: a tmpfs at `/run/media/dev/hoplodex-check` standing in for a removable
drive (`--flatpak` mounts it; the Flatpak reaches `/run/media`), two folders
outside what it reaches for the check to show refused (`/mnt/hoplodex-check`,
another tmpfs `--flatpak` mounts, and `/srv/hoplodex-check`, which the
`Dockerfile` makes), private D-Bus buses with scripted portals and a headless
keyring daemon started by the script, and an Xvfb display of its own.

## What the Flatpak reaches

**What the Flatpak reaches, and where it keeps passphrases.** It is built with
`--filesystem=xdg-documents/HoploDex`, `/media` and `/run/media` and no home
folder, so a database in `<Documents>/HoploDex` or on a removable drive works,
and a location anywhere else (`/mnt`, `/srv`, a Downloads folder) is refused
before anything is written, with the `flatpak override --user
--filesystem=<folder>` command that would grant it (`011 FR-003`, `011 FR-007`;
`011 research.md` §5). Saved passphrases do not use the Secret Service, which the
Flatpak is not granted: inside it they are kept in the app's own file keyring
under `$XDG_DATA_HOME/keyrings/`, encrypted with a key the Secret portal
returns, and saving is unavailable where the desktop has no Secret portal
(`011 FR-006`; `011 research.md` §7).

## The source lists

**The source lists.** The offline build can only use what the lists name, so
`flatpak/cargo-sources.json` (from `src-tauri/Cargo.lock`) and
`flatpak/node-sources.json` (from `package-lock.json`) are generated and
committed. After any change to either lockfile, run
`scripts/dev-container.sh --flatpak npm run flatpak:sources` and commit both
lists. Until you do, `scripts/flatpak/sources.test.ts` fails in every
`npm test`, naming the list and the first package that differs (`011 US3-2`,
`011 FR-015`). That includes a dependency update pull request, Dependabot's or
one by hand: regenerate the lists in the dev container and commit them to its
branch before `npm test` passes.

## The Flatpak check

`npm run test:flatpak` builds the Flatpak's check variant, installs it into the
dev container's Flatpak installation and drives it through WebDriver in its
sandbox (`011 US5-1`, `011 FR-021`, `011 FR-023`; the reasons are in
[011's research.md §15, §16 and §18](../../specs/011-flatpak/research.md)). It runs
only in the dev container started with `--flatpak` (or on a GitHub-hosted runner
set up as below), after `npm run build`, and **ordinary test runs skip it**:

```bash
scripts/dev-container.sh --flatpak bash -c 'npm run build && npm run test:flatpak'
scripts/dev-container.sh --flatpak bash -c 'npm run build && npm run test:flatpak -- --fresh-install'
```

What it does, in order:

1. Removes any HoploDex build already installed (`npm run bundle:flatpak`
   leaves the shipped one there) and notes which apps the installation lists.
2. Builds the check variant offline and installs it. It is the shipped manifest
   with only the `e2e` and `flatpak-check` cargo features and the profile
   changed, so it runs with the permissions the Flatpak ships with
   (`manifest.test.ts` compares the two). `flatpak-check` is the `e2e` build
   with the real routes: its `AppOpener` hands a document to the OpenURI
   portal and its keyring is the real one (inside the Flatpak, the file keyring
   keyed by the Secret portal) instead of the `e2e` build's log and in-memory
   store. The consent and passphrase dialogs are still the
   `e2e` build's stand-ins, which no WebDriver could click (the passphrase one
   answers from the queue the spec fills). Only the Flatpak check
   builds it; the main E2E suite and every shipped build are unchanged.
3. Starts a private session bus with a scripted desktop portal
   (`scripts/flatpak/openuri_mock.py`, a dbusmock template with two interfaces:
   OpenURI, which records each `OpenFile`'s descriptor and answers `Response`,
   and Secret, whose `RetrieveSecret` hands back a fixed key for the run, the
   stand-in for the Secret portal; started without the key it has no Secret
   interface, as a desktop with no Secret portal backend), stand-ins for both
   screen savers and a name the manifest does not grant, and a stand-in logind
   on the private system bus. A real, headless `gnome-keyring-daemon`
   (unlocked, its data in the run's throwaway folder, started by
   `scripts/flatpak/keyring-daemon.sh`, which refuses the container's own bus)
   stays on the same bus for one reason only: to show that
   `org.freedesktop.secrets` is refused to the sandbox while it answers
   outside it. From inside the sandbox the check also shows that each granted
   name answers, that logind's `Inhibit` returns a descriptor through the bus
   proxy, and that the stand-in name the manifest does not grant is refused
   while it answers outside the sandbox.
4. Shows the shipped permissions have no network: it listens on the container's
   own non-loopback address, then a `python3` run in the sandbox without
   `--share=network` sees only the loopback interface and cannot connect to it.
5. Builds the `e2e` binary outside the sandbox and runs
   `e2e/specs/flatpak.e2e.ts` (`HOPLODEX_E2E_FLATPAK=1`), whose journeys run the
   check variant with the run's throwaway home, buses and stand-ins, and add
   `--share=network` only for the WebDriver port. They show that the sandbox
   sees the host's user directories without the home folder; that a database
   is made, backed up and reopened under the stand-in drive; that `/mnt` and
   `/srv` are refused on the folder field with nothing written; that a photo
   dropped from `~/Pictures` is refused with the advice to attach it from the
   file chooser; and that a saved passphrase lands in the app's keyring file,
   which the check reads back with the stand-in portal's key
   (`src-tauri/examples/flatpak_keyring_read.rs`) and which opens the database
   from the recent list after a restart; and that the web process the Flatpak
   portal starts for WebKit runs with core dumps off, soft and hard, as the
   app sets them at each page load (#166). One of its tests prints the
   cold start to the chooser for the Flatpak and for the `e2e` build outside it
   and fails on a difference over a second (`011 SC-006`).
6. Uninstalls the check variant whatever happened, and fails unless the
   installation lists the same apps as at the start.

Its logs are in `flatpak/out/check/` (`check.log`, one per stand-in and per app
start, which holds what the app logged to standard error: whether WebKit's
sandbox is on and what the helpers are confined with, see `src-tauri/src/logging.rs`;
and `openuri.log`, one JSON line for each descriptor the scripted portal was handed).
`lock-times.log` has each lock's time from the signal, in the Flatpak and outside
the sandbox for a sleep, a screen lock, a shutdown and a SIGTERM, each Flatpak
time within a second of its outside time (`011 SC-002`), and
`webkit-sandbox.txt` the `flatpak-spawn --sandbox` command line, what the web
process sees of its sandbox and what a process started with WebKit's flags
reaches (`011 research.md` §9).

`-- --fresh-install` runs a different check (`011 US4-2`, `011 SC-010`; it needs
the network and the file `npm run bundle:flatpak` wrote). It installs that file
into a second, empty installation with no remotes under the state volume
(`flatpak install --user --bundle`), shows that the installation gained the
`flathub` remote and `org.gnome.Platform//51` and that the installed app's
`files/` pass `contents.mjs` (no runtime library came from the file), starts the
shipped build once under a throwaway home on a virtual display with the stand-in
buses, and finds where it keeps its own data: `machine.json` and the main web
view's data folder under `~/.var/app/io.github.exodious.HoploDex/`, and nothing
under the home's own `.config` or `.cache` (`011 FR-009`). The second
installation is removed whatever happened; the shipped build has no WebDriver
and no log line for its window, so the check waits for the log line that says it
runs in a Flatpak and for the two folders.

Run it before each release and for a pull request that changes any of these
paths (`011 FR-021`): `flatpak/`, `scripts/flatpak/`,
`e2e/specs/flatpak.e2e.ts`, `e2e/support/flatpak.ts`,
`src-tauri/src/platform/linux.rs`, `src-tauri/src/platform/core_dumps.rs`,
`src-tauri/src/main.rs`, `src-tauri/src/app_dirs.rs`,
`src-tauri/src/session/lifecycle.rs`; under
`src-tauri/src/services/`: `sandbox.rs`, `keyring.rs`, `file_swap.rs`,
`backups.rs`, `scratch.rs`, `helper/`, `import_reader/`, `preview/helper.rs`,
`preview/sandbox_probe.rs`; under `src-tauri/src/commands/`: `databases.rs`,
`backups.rs`, `import_export.rs`, `documents.rs`, `photos.rs`; and
`src/features/media/useFileDrop.ts`.

It never touches your Flatpak installation, your `~/.var/app`, your buses or
your keyring: the container has its own (`011 SC-007`), and the script refuses
anywhere else but a GitHub-hosted runner set up as below (the guard described under "Flatpak" above).

### On a GitHub-hosted runner

**On a GitHub-hosted runner** (#25) the check runs outside the container, once
the workflow has made the container's stand-ins itself. The guard
(`scripts/flatpak/guard.mjs`) accepts such a runner only with all three markers:
`GITHUB_ACTIONS=true` (also true on a self-hosted runner, which may be someone's
own machine), `RUNNER_ENVIRONMENT=github-hosted` (set on GitHub's own VMs) and
the explicit opt-in `HOPLODEX_CI_FLATPAK=1`, so a shell that happens to export
the first two doesn't pass by accident. It is an accident guard, not a security
boundary. As on a host, it then needs both `FLATPAK_USER_DIR` and
`HOPLODEX_FLATPAK_STATE` named, each empty or holding the marker the scripts
write, outside any real Flatpak installation. No script mounts or uses `sudo`;
the workflow does, before the check:

```bash
# .github/actions/apt's flatpak and flatpak-check sets
sudo apt-get install -y --no-install-recommends flatpak flatpak-builder desktop-file-utils \
  appstream binutils dbus python3 python3-dbusmock gnome-keyring libsecret-tools xvfb \
  libwebkit2gtk-4.1-dev
sudo mkdir -p /run/media/dev/hoplodex-check /mnt/hoplodex-check
sudo mount -t tmpfs -o mode=1777 tmpfs /run/media/dev/hoplodex-check
sudo mount -t tmpfs -o mode=1777 tmpfs /mnt/hoplodex-check
sudo install -d -m 0755 -o "$USER" /srv/hoplodex-check
sudo install -d -o "$USER" /mnt/hoplodex-flatpak /mnt/hoplodex-flatpak-state
export HOPLODEX_CI_FLATPAK=1 FLATPAK_USER_DIR=/mnt/hoplodex-flatpak HOPLODEX_FLATPAK_STATE=/mnt/hoplodex-flatpak-state
npm ci && npm run build && npm run bundle:flatpak && npm run test:flatpak
```

`npm run bundle:flatpak` must run first in the same job: it installs the
runtime and SDK, without which the check fails at once
(`org.gnome.Sdk/x86_64/51 not installed`). Never set these variables on your own
machine.
