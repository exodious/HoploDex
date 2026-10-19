# Contract: The Flatpak Bundle and Its Commands

What the Flatpak promises to users and to the release process (#19), and
the project commands that build and check it. data-model.md gives the
files' shapes; research.md gives the reasons.

## Identity

| | |
|---|---|
| App id | `io.github.exodious.HoploDex` (the Tauri bundle identifier; never changes, since it names the keyring entries and `~/.var/app`) |
| Runtime | `org.gnome.Platform//51`, from Flathub (research.md §1) |
| Architecture | x86-64 only (FR-025) |
| Command | `hoplodex` |
| Network | none (no `--share=network`) |

## Permissions (FR-002)

Exactly research.md §4's table, in the manifest's `finish-args`, each with
its reason beside it:

```text
--share=ipc
--socket=wayland
--socket=fallback-x11
--device=dri
--filesystem=home
--filesystem=/media
--filesystem=/run/media
--filesystem=/mnt
--talk-name=org.freedesktop.secrets
--talk-name=org.freedesktop.ScreenSaver
--talk-name=org.gnome.ScreenSaver
--system-talk-name=org.freedesktop.login1
```

Adding, removing or changing one is a change to this contract, research.md
§4's record and `manifest.test.ts`, and needs the automated check (FR-021).

## Where things live inside the Flatpak

| What | Where | As on a `.deb`? |
|---|---|---|
| Databases, backups | wherever the user keeps them within reach | Yes |
| Suggested new-database folder | `<Documents>/HoploDex` (the host's own Documents folder) | Yes (FR-010) |
| `machine.json` (settings, recent list) | `~/.var/app/io.github.exodious.HoploDex/config/io.github.exodious.HoploDex/` | No: separate from a `.deb`'s (FR-009) |
| Caches, opened documents' copies, web view data | `~/.var/app/io.github.exodious.HoploDex/cache/io.github.exodious.HoploDex/` | No: separate |
| Saved passphrases | the user's keyring, service `io.github.exodious.HoploDex`, user `passphrase:<database id>` | Yes: the same entries (FR-006) |

## Bundle contents (FR-017, FR-018)

data-model.md's list, and nothing else. In particular, no shared library.
The plain-text notices are at
`/app/share/licenses/io.github.exodious.HoploDex/NOTICES.txt`.

## The file a release carries (FR-020)

`HoploDex-<version>-x86_64.flatpak`, naming Flathub as its runtime's
source. A user installs it with their software centre, or:

```bash
flatpak install --user HoploDex-<version>-x86_64.flatpak
```

which adds Flathub if it isn't set up and fetches the runtime from it. A
newer version is installed the same way, over the old one, keeping
everything (FR-024). `flatpak update` updates the runtime but not HoploDex.
#19 publishes this text with the releases.

## Project commands

All three run in the dev container with `--flatpak`
(`scripts/dev-container.sh --flatpak npm run bundle:flatpak`). Outside it
they refuse, unless `FLATPAK_USER_DIR` names an empty or throwaway
installation and `--i-know-this-is-a-host` is passed (research.md §15,
SC-007).

| Command | Does | Fails when |
|---|---|---|
| `npm run bundle:flatpak` | Installs the SDK, runtime, extensions and linter into the container's installation if missing (network); `flatpak-builder --download-only`; `flatpak-builder --disable-download` (no network; `check-no-webdriver.mjs` is one of the manifest's build commands); the contents check; `desktop-file-validate`; `appstreamcli validate --no-net`; the notices comparison; installs the build from the local repository; the linter on the manifest and the repository; writes the `.flatpak` | any step fails, naming it |
| `npm run flatpak:sources` | Regenerates `flatpak/cargo-sources.json` and `flatpak/node-sources.json` from the lockfiles | a generator fails |
| `npm run test:flatpak` | Builds the check variant (data-model.md), installs it, starts the private buses and stand-ins, runs `e2e/specs/flatpak.e2e.ts` (US4-3's install-over among it) and the bus checks, uninstalls it. `-- --fresh-install` also installs the shipped `.flatpak` into an empty installation with no remotes and starts it (network) | any test fails |

`npm test` (every run) includes `scripts/flatpak/*.test.ts`, which need no
Flatpak tooling: the source lists against the lockfiles, the manifest's
rules, the contents allow list on fixtures, the metadata's rules and the host
guard.

_Amended 2026-10-19 (011 T078): what the steps do that the table leaves out._

- _The linter is `flatpak run --command=flatpak-builder-lint
  org.flatpak.Builder`, run with `--exceptions --user-exceptions
  flatpak/linter-exceptions.json`: on the manifest with exactly the two recorded
  exceptions, and on the built repository with those two plus
  `appstream-missing-screenshots` and
  `appstream-screenshots-not-mirrored-in-ostree`, which `build.sh` adds in a
  file under the state volume (the screenshots' addresses are at the release's
  tag, so a local build can't mirror them; research.md §4)._
- _The notices comparison builds `NOTICES.txt` outside the Flatpak with the same
  lockfiles and fails if it differs from the bundle's, apart from the copyright
  years line (FR-019)._
- _`flatpak build-bundle` is passed `--runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo`
  and ends the run with the file's path._

## Output locations

| | |
|---|---|
| The `.flatpak` file | `flatpak/out/` (git-ignored; reachable from the host) |
| `flatpak-builder` state and build folders | the `--flatpak` volume, per checkout |
| The container's Flatpak installation (runtime, SDK, extensions, linter, the check's installed app) | the container's user installation in the shared `/home/dev` volume; never the host's |
| The check's logs | `flatpak/out/check/` |
