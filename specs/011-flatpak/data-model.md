# Data Model: Build and Distribute a Flatpak

**Feature**: 011-flatpak | **Date**: 2026-10-19

Nothing in the database, its layout, `machine.json` or the keyring entries
changes. This file describes the backend's new in-memory reach model, the
small additions to IPC types, and the files the build reads and writes.

## Backend: reach (`services/sandbox.rs`)

### `Reach`

What this run of HoploDex may reach on the filesystem. Read once at startup
by `main.rs` and kept as Tauri state (`Arc<Reach>`). Commands pass it to
their `ops`, which take `&Reach`, so tests build one for a temporary folder
(research.md §5).

| Variant | Meaning |
|---|---|
| `Unconfined` | Not in a Flatpak (no `/.flatpak-info`): every path is within reach and nothing is checked. Every non-Flatpak install, and every existing test, uses this. |
| `Flatpak { grants: Vec<Grant>, home: PathBuf }` | Inside a Flatpak, with the grants read from `/.flatpak-info`. |

**`Grant`**: `{ root: PathBuf, writable: bool }`. A path is within a grant
when it is the grant's root or below it, compared after making it absolute
and normalising `.` and `..` without following links. The longest matching
root decides, so a `!` grant below a wider one removes that part.

**Reading `/.flatpak-info`** (`Reach::from_flatpak_info(text, home, user_dirs)`,
a pure function): the `[Context]` group's `filesystems=` key, a
`;`-separated list. Each entry:

| Entry | Grant |
|---|---|
| `home`, `~` | `home`, writable |
| `host` | `/`, writable |
| `host-os`, `host-etc` | not counted: system folders, never a database's place |
| `~/<path>` | `home/<path>` |
| `xdg-documents`, `xdg-download`, `xdg-desktop`, … (with an optional `/<path>`) | the user folder from `user_dirs`, then the path |
| `/<absolute path>` | that path |
| any entry with `:ro` | the same root, not writable |
| any entry with `:create`, `:rw` | the same root, writable |
| `!<entry>` | removes what the entry would grant (the longest-match rule above) |
| anything else | not counted, and logged once |

A missing or unreadable file inside a Flatpak gives `Flatpak` with no
grants, so nothing is within reach and each operation is refused with the
fix, rather than failing part-way.

### `Need`

What an operation needs of a location. The command names it; `Reach::check`
judges it.

| Need | Used by | Within reach when |
|---|---|---|
| `DatabaseFolder` | `create_database` (the folder and name), `open_database`, opening from a recent entry, `locate_database` | the path's folder is within a writable grant (after resolving a Documents-portal path to its host path) |
| `WriteFolder` | `update_backup_settings`' custom location; the backup at close, a restore's and a passphrase change's "before" backup when the location is custom; `export_collection`'s destination | the folder is within a writable grant, or is a Documents-portal folder |
| `ReadFile` | `read_import_files` / `import_collection` files, `add_photo_from_path`, `add_document_from_path` (chooser and drop), `restore_backup`'s backup file, `check_backup` | the file exists and can be read in the sandbox (a Documents-portal path included), or, when it doesn't exist, it isn't outside every grant (then it is "not found" as today) |

`Reach::check(&self, path: &Path, need: Need) -> Result<PathBuf, CommandError>`
returns the path to use: the given one, or the host path a Documents-portal
path stands for when that host path is within reach. It refuses with
`CommandError::out_of_reach` (below) otherwise. It writes nothing and makes
no scratch file.

### Documents-portal paths

A path under `/run/user/<uid>/doc/` (or `$XDG_RUNTIME_DIR/doc/`) came from the
file chooser. `Reach` resolves it through a `HostPaths` trait:

- The app's: the Documents portal's `GetHostPaths(doc_ids)` over the session
  bus (`zbus`), falling back to the file's `user.document-portal.host-path`
  extended attribute.
- The tests': a fixed map.

| Resolved host path | `DatabaseFolder` | `WriteFolder` | `ReadFile` |
|---|---|---|---|
| within a writable grant | host path | host path | host path |
| within a read-only grant | refused | refused | host path |
| outside every grant, or not resolved | refused | the portal path (writing inside a granted folder works) | the portal path |

## IPC additions (camelCase; contracts/tauri-commands.md)

### Error `LOCATION_OUT_OF_REACH`

```ts
// CommandFailure.code === "LOCATION_OUT_OF_REACH"
details: {
  path: string;          // the location named, as the user gave it
  folder: string;        // the folder that would need access
  grants: string[];      // what this copy can reach now, for the message
}
fieldErrors?: { folder?: string; name?: string }  // create_database only
```

The message (one wording, built once in `commands/error.rs`):

> This copy of HoploDex is a Flatpak, and it can only reach your home folder
> and removable drives, so it can't use `<path>`. Move the file into your
> home folder or onto a removable drive, or give HoploDex access to
> `<folder>` by running:
> `flatpak override --user --filesystem=<folder> io.github.exodious.HoploDex`
> and then restarting HoploDex.

When the user has granted more, the first sentence lists `grants` instead of
"your home folder and removable drives".

### Backup location availability

- `BACKUP_LOCATION_UNAVAILABLE.details.reason` gains `"outOfReach"` beside
  `"missing" | "notWritable" | "insufficientSpace"`.
- `BackupLocation` (the collection settings' location), `BackupList` and
  the chooser's `RecentDatabase` each gain `unavailableReason?: "outOfReach"`
  beside their `available: boolean`, set only when `available` is false
  because of reach. A recent entry out of reach then says so instead of
  "not found". The settings dialog then
  says "This copy of HoploDex can't reach this folder" with the fix, in place
  of "Not available on this computer".
- `CloseOutcome.failureReason` gains `"outOfReach"`, which the chooser's
  backup notice words as "this copy of HoploDex (a Flatpak) can't reach the
  backup location".

## The portal opener (`commands/documents.rs`)

`AppOpener` on Linux holds `OpenerRoute::XdgOpen` or `OpenerRoute::Portal`,
chosen once from `Reach` (Flatpak or not). The portal route's answers:

| OpenURI `Response` | `Opener::open` returns |
|---|---|
| `0` | `Ok(())` |
| `1` (the user closed the portal's chooser) | `Ok(())` |
| `2` | `Err(OpenFailure::NoApp)` |
| no answer in 5 s | `Ok(())` (the portal is waiting on the user) |
| no portal on the bus, or the call refused | `Err(OpenFailure::NoApp)` |

## Build files (`flatpak/`)

### The manifest, `io.github.exodious.HoploDex.yml`

| Key | Value |
|---|---|
| `id` | `io.github.exodious.HoploDex` (the bundle identifier, FR-001) |
| `runtime`, `runtime-version`, `sdk` | `org.gnome.Platform`, `51`, `org.gnome.Sdk` |
| `sdk-extensions` | `org.freedesktop.Sdk.Extension.rust-stable`, `org.freedesktop.Sdk.Extension.node24` |
| `command` | `hoplodex` |
| `finish-args` | exactly research.md §4's list, each with a comment giving its reason |
| `build-options` | the extensions' `bin` on `PATH`; `CARGO_HOME` in the build folder; `CARGO_NET_OFFLINE=true`; `npm_config_offline=true`; no `build-args` |
| `modules` | one: `hoplodex`, `buildsystem: simple`, sources `[{ type: dir, path: .. }, cargo-sources.json, node-sources.json]`, the build commands of research.md §2 |

**Rules** (`manifest.test.ts`): one module; those three sources; no
`--share=network` anywhere; `finish-args` equal to the recorded list.

### The check manifest (derived, never committed)

`scripts/flatpak/checkManifest.mjs` writes it into the state folder from the
manifest. It differs only in the cargo build command's `--features`
(`custom-protocol,e2e`) and `--profile e2e`, in the binary's path that
follows from the profile, and in leaving out the `check-no-webdriver.mjs`
step, which would fail on the `e2e` build's driver by design. The test
compares the two parsed manifests and fails on any other difference.

### Offline source lists

`cargo-sources.json` and `node-sources.json`: arrays of `flatpak-builder`
sources (`archive`/`file` entries with `url`, `sha256` or `sha512`, `dest`,
plus the generators' `inline` config entries), as the generators write them.
They are regenerated, never edited.

**Rules** (`sources.test.ts`):

- For each `[[package]]` in `src-tauri/Cargo.lock` with a registry `source`,
  exactly one `archive` entry whose `url` names that crate and version and
  whose `sha256` equals the lock's `checksum`.
- For each entry in `package-lock.json`'s `packages` with `resolved` and
  `integrity`, exactly one source whose URL and hash match.
- No source for a package neither lockfile has.
- No `git` source in `Cargo.lock`.
- A failure names the list (`flatpak/cargo-sources.json` or
  `flatpak/node-sources.json`), the first package that differs, and
  `npm run flatpak:sources`.

### `linter-exceptions.json`

```json
{
  "io.github.exodious.HoploDex": {
    "finish-args-home-filesystem-access": "<why: research.md §4, §5>",
    "finish-args-login1-system-talk-name": "<why: research.md §4, §6>"
  }
}
```

The format is the linter's own for local exceptions, confirmed when it is
first run (research.md §4).

### `flathub.json`

`{ "only-arches": ["x86_64"] }` (FR-025).

### AppStream metadata, `io.github.exodious.HoploDex.metainfo.xml`

`component type="desktop-application"` with: `id`, `name` (HoploDex),
`summary`, `description` (what it is; everything stays on the computer; no
account, no network), `metadata_license` `CC0-1.0`, `project_license`
`GPL-3.0-only`, `developer id="io.github.exodious"`, `url` homepage,
`vcs-browser` and `bugtracker`, `launchable type="desktop-id"`,
`content_rating type="oars-1.1"` (empty), `branding` (light and dark
colours from `src/styles/tokens.css`), `screenshots` (addresses at the
release tag of files in `flatpak/screenshots/`), `releases` (newest first,
each with a version, a date and notes).

**Rules** (`metainfo.test.ts`, then `appstreamcli validate` and the linter):
the newest release's version equals `APP_VERSION`; every screenshot address
names a file under `flatpak/screenshots/` at `v<version>`; the license and
source address are as above.

### The desktop entry, `io.github.exodious.HoploDex.desktop`

`Type=Application`, `Name=HoploDex`, `Comment` (the metadata's summary),
`Exec=hoplodex`, `Icon=io.github.exodious.HoploDex`, `Terminal=false`,
`Categories=Office;Database;`, `StartupWMClass` (the main window's class).
Checked by `desktop-file-validate`.

### The bundle's contents (`contents.mjs`)

The built app's `files/` must hold exactly:

```text
bin/hoplodex
share/applications/io.github.exodious.HoploDex.desktop
share/metainfo/io.github.exodious.HoploDex.metainfo.xml
share/icons/hicolor/{32x32,64x64,128x128,256x256,512x512}/apps/io.github.exodious.HoploDex.png
share/licenses/io.github.exodious.HoploDex/NOTICES.txt
```

plus what `flatpak build-finish` itself adds (`manifest.json`, the
exported copies under `export/`). Any other path fails the check, naming it.
`readelf -d bin/hoplodex`'s `NEEDED` libraries must each be found in the
runtime (FR-018).

### The `.flatpak` file

`flatpak/out/HoploDex-<version>-x86_64.flatpak`, from `flatpak build-bundle`
with `--runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo`
(research.md §14). `<version>` is `APP_VERSION`.

## State transitions

None new. `Reach` is fixed for a run, as the sandbox itself is: an override
the user makes while HoploDex runs takes effect when it next starts, which is
why the message says to restart it.
