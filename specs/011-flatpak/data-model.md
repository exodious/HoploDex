# Data Model: Build and Distribute a Flatpak

**Feature**: 011-flatpak | **Date**: 2026-10-19

Nothing in the database, its layout, `machine.json` or the keyring entries
changes. This file describes the backend's new in-memory reach model, the
small additions to IPC types, and the files the build reads and writes.

## Backend: reach (`services/sandbox.rs`)

### `Reach`

What this run of HoploDex may reach on the filesystem. Read once at startup
by `main.rs` and kept as Tauri state (`Arc<Reach>`), the same `Arc` that
`Session` holds (`Session::reach()`), so closes, upgrades and the "before"
backups started by OS events can ask it too. Commands pass it to their `ops`,
which take `&Reach`, so tests build one for a temporary folder (research.md §5).

| Variant | Meaning |
|---|---|
| `Unconfined` | Not in a Flatpak (no `/.flatpak-info`): every path is within reach and nothing is checked. Every non-Flatpak install, and every existing test, uses this. |
| `Flatpak { grants: Vec<Grant>, removed: Vec<PathBuf>, home: PathBuf, host_paths: Arc<dyn HostPaths> }` | Inside a Flatpak, with the grants read from `/.flatpak-info`, the roots an override removed (`!<entry>`), the home folder, and the lookup that turns a Documents-portal path into its host path. |

_Amended 2026-10-19 (011 T078): the variant was written with `grants` and
`home` only. `removed` and `host_paths` are fields of it, not separate state:
a removal beats a grant equally close to the path (so a mistake narrows reach
and never widens it), and the portal lookup travels with the reach it serves.
`Reach::detect(info_path, home, user_dirs, host_paths)` reads the file once
(no file: `Unconfined`; a file that can't be read: `Flatpak` with no grants,
logged), `Reach::from_flatpak_info(text, home, user_dirs)` is the pure parser,
and `Reach::describe()` is the one line the log carries at startup._

**`Grant`**: `{ root: PathBuf, writable: bool }`. A path is within a grant
when it is the grant's root or below it, compared after making it absolute
and normalising `.` and `..` without following links. The longest matching
root decides, so a `!` entry below a wider grant removes that part; a removal
as close as the closest grant, or closer, wins.

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

_Amended 2026-10-19 (owner's decision): an `xdg-*` entry whose folder is
unset, or is the home folder itself, is not counted, as Flatpak grants nothing
for it (research.md §5)._

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

_Amended 2026-10-19 (011 T107): a passphrase change makes no "before" backup,
so it is not a user of `WriteFolder`; only `close_database`'s backup, a
restore's and an upgrade's "before" backup (and `update_backup_settings` and
`export_collection`) are. See contracts/tauri-commands.md._

_Amended 2026-10-19 (011 T096): `ReadFile` accepts a path that is not a
Documents-portal one only where a grant covers it, whether or not it exists in
the sandbox, so a drop from outside reach is always refused (`011 FR-007` as
written). A Documents-portal path is read as the table below says._

_Amended 2026-10-19 (011 T097): a new `BackupFolder` need, for the folder
backups are written to now and at every later close: `update_backup_settings`'
custom location, `backups::check_location` (so the close's backup and the
"before" backups), `list_backups`' and the settings' `available`. It is `WriteFolder` for a path that is not a Documents-portal one;
for a Documents-portal path it is the host path when that is within a writable
grant and refused otherwise (the table below). `WriteFolder` stays for
`export_collection`'s destination and for the upgrade's chosen folder (the one
backup before an upgrade, "save this one in a folder you choose instead"), both
one-off writes; `check_location` and `BackupJob` take the `Need`. A chosen folder
that is the database's backup location itself is the stored one, `BackupFolder`._

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

_Amended 2026-10-19 (011 T094, T095, T097): the host path of a path below the
document is the document's host path with the rest below it joined on
(`doc/<id>/<name>/sub/x` is `<host path>/sub/x`; `<name>` is the picked file or
folder, whose host path `GetHostPaths` answers for), so a database inside a
picked folder resolves below that folder. A refusal after a successful lookup
names the host path and the host folder, so the override command names a folder
that helps; the portal's own path and folder are named only when nothing
resolves. A `BackupFolder` column is the `DatabaseFolder` one for a folder:
the host path within a writable grant, otherwise refused._

_Amended 2026-10-19 (011 T112): a path is compared with the grants after
following the links in the grant roots and in the path's folders above them
(`/home` linking to `/var/home` on Fedora Silverblue), so a typed
`/home/<user>/…` is judged against a grant on `/var/home/<user>`. A link inside
a grant is still judged by its own path, and a link outside that leads into one
is not followed._

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

`CommandError::out_of_reach(path, folder, home, grants)` builds it, in
`commands/error.rs`; `home` is the user's home folder, the first of the shipped
grants, which `Reach` holds.

_Amended 2026-10-19 (011 T078): the constructor was planned as
`out_of_reach(path, folder, grants)`. It takes `home` too, because "the
shipped four" is the home folder plus `/media`, `/run/media` and `/mnt`, and
the home folder is not the same path on every computer._

The message (one wording, built once in `commands/error.rs`):

> This copy of HoploDex is a Flatpak, and it can only reach your home folder
> and removable drives, so it can't use `<path>`. Move the file into your
> home folder or onto a removable drive, or give HoploDex access to
> `<folder>` by running:
> `flatpak override --user --filesystem=<folder> io.github.exodious.HoploDex`
> and then restarting HoploDex.

When the user has granted more, the first sentence lists `grants` instead of
"your home folder and removable drives".

_Amended 2026-10-19 (011 T078): what the built message does, which the
paragraph above leaves open:_

- _The first sentence says "your home folder and removable drives" only when
  `grants` are exactly the shipped four: the home folder, `/media`,
  `/run/media` and `/mnt`, each writable, in any order. Any other set (one
  added, one removed, home replaced, one made read-only) is the user's own and
  is listed, each root in backticks, `(read only)` after a read-only one, and
  "and" before the last. No grants at all reads "none of your folders"._
- _The override command's folder is shell-quoted, and only when a shell would
  split it (a folder with a space in it). `<folder>` in the prose stays plain._
- _`details.grants` is always the roots, whatever the first sentence says._

_Amended 2026-10-19 (owner's decision; FR-003, FR-007): the shipped default is
now three grants, `<Documents>/HoploDex` (Documents from `UserDirs`), `/media`
and `/run/media`, each writable. For exactly those the message reads (the
`<…>` filled in):_

> This copy of HoploDex is a Flatpak, and it can only reach the HoploDex
> folder in your Documents folder (`<Documents>/HoploDex`) and removable
> drives, so it can't use `<path>`. Move the file into
> `<Documents>/HoploDex` or onto a removable drive, or give HoploDex access
> to `<folder>` by running:
> `flatpak override --user --filesystem=<folder> io.github.exodious.HoploDex`
> and then restarting HoploDex.

_An attach command's refusal (`add_photo_from_path`, `add_document_from_path`:
a host path outside reach there can only be a drop) puts a sentence before the
two fixes: "To attach it from where it is, use Add photo (or Add document) and
choose it there." The constructor takes which advice to give
(contracts/tauri-commands.md); `details` are unchanged. Any other set of grants
is listed as before._

_Amended 2026-10-19 (011 T118): what was built. The constructor is
`out_of_reach(path, folder, documents, grants, advice)`: `documents` is the
Documents folder `UserDirs` names (`None` when it is unset or is the home
folder, which `Reach` holds), and the default is recognised as exactly
`<documents>/HoploDex`, `/media` and `/run/media`, each writable, in any order.
The attach sentence names the real button, not both: "To attach it from where
it is, use Add photos and choose it there." from `add_photo_from_path`
(`Need::AttachPhoto`), and "...use Attach and choose it there." from
`add_document_from_path` (`Need::AttachDocument`); each is read like
`Need::ReadFile`. For any other set of grants the move advice reads "Move the
file into a folder it can reach" (the default's folder and drives are not
promised there), followed by the same override fix._

_Amended 2026-10-20 (owner's decision): the backticks in the wordings above
are this document's formatting, not the message's. The message names its
paths, grants and the override command as plain text, as the frontend's own
out-of-reach text (`outOfReach.ts`) does; the app shows it as written, so
backticks appeared on screen._

## Saved passphrases inside a Flatpak (`services/keyring.rs`)

_Added 2026-10-19 (owner's decision; FR-006; research.md §7)._

| | Outside a Flatpak | Inside a Flatpak |
|---|---|---|
| Store | the `keyring` crate's OS store (unchanged) | a keyring-core `CredentialStoreApi` over oo7's file keyring, installed with `set_default_store` |
| Secret | the OS keyring's own | the Secret portal's per-app key (`RetrieveSecret`), fetched on first use in a run |
| File | the OS keyring's | `$XDG_DATA_HOME/keyrings/default.keyring` (`~/.var/app/io.github.exodious.HoploDex/data/keyrings/`) |
| Entry | service `io.github.exodious.HoploDex`, user `passphrase:<database id>` | the same, as the item's attributes |
| Available when | the OS keyring answers (003 FR-019) | `org.freedesktop.portal.Desktop` has an owner and its Secret interface's `version` reads; never by calling `RetrieveSecret` |
| When it isn't | saving shows unavailable | the same, with no fallback to the Secret Service |

_Amended 2026-10-19: the store is not installed with `set_default_store`. It
is held in `Keyring`'s own `Backend::Portal`, because `keyring::Entry::new`
makes the Secret Service the default store on first use and would replace it._

`e2e` builds keep the mock store, except `flatpak-check`, which uses the
portal store against the check's stand-in portal (research.md §16).

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

`AppOpener` on Linux holds `OpenerRoute::XdgOpen` or
`OpenerRoute::Portal { timeout }`, chosen once from `Reach::is_flatpak()` by
`main.rs`'s `opener_route` (the portal's timeout is 5 s) and passed to
`AppOpener::new(app, route)`. The portal route's answers:

| OpenURI `Response` | `Opener::open` returns |
|---|---|
| `0` | `Ok(())` |
| `1` (the user closed the portal's chooser) | `Ok(())` |
| `2` | `Err(OpenFailure::NoApp)` |
| no answer in 5 s | `Ok(())` (the portal is waiting on the user) |
| no portal on the bus, or the call refused | `Err(OpenFailure::NoApp)` |

_Amended 2026-10-19 (011 T078): the planned `AppOpener::new(app)` gained the
route as its second argument (a refactor of the one constructor, not a sibling
for Flatpak). `OpenerRoute` exists only outside the `e2e` build, which keeps
`E2eOpener`; on macOS and Windows it has `XdgOpen` only, which there means the
platform's own launcher._

## Build files (`flatpak/`)

### The manifest, `io.github.exodious.HoploDex.yml`

| Key | Value |
|---|---|
| `id` | `io.github.exodious.HoploDex` (the bundle identifier, FR-001) |
| `runtime`, `runtime-version`, `sdk` | `org.gnome.Platform`, `51`, `org.gnome.Sdk` |
| `sdk-extensions` | `org.freedesktop.Sdk.Extension.rust-stable`, `org.freedesktop.Sdk.Extension.node24` |
| `command` | `hoplodex` |
| `finish-args` | exactly research.md §4's list, each with a comment giving its reason |
| `build-options` | the extensions' `bin` on `PATH`; `CARGO_HOME` in the build folder; `CARGO_NET_OFFLINE=true`; `npm_config_offline=true`; `XDG_CACHE_HOME` and `npm_config_cache` in the node source list's cache; no `build-args` |
| `modules` | one: `hoplodex`, `buildsystem: simple`, sources `[{ type: dir, path: .., skip: [...] }, cargo-sources.json, node-sources.json]`, the build commands of research.md §2 |

_Amended 2026-10-19 (011 T078): the `dir` source skips `node_modules`,
`dist`, `src-tauri/target`, `flatpak/out`, `.human-testing`, `.claude`, `.git`
and `.wdio`, so the dev container's build output and data, the worktrees and
the history never enter the build. `XDG_CACHE_HOME` and `npm_config_cache`
point at the cache `node-sources.json` fills (flatpak-node-generator's layout),
which `npm ci --offline` and esbuild's binary read._

**Rules** (`manifest.test.ts`): one module; those three sources; no
`--share=network` anywhere; `finish-args` equal to the recorded list.

### The check manifest (derived, never committed)

`scripts/flatpak/checkManifest.mjs` writes it into the state folder from the
manifest. It differs only in the cargo build command's `--features`
(`custom-protocol,e2e`) and `--profile e2e`, in the binary's path that
follows from the profile, and in leaving out the `check-no-webdriver.mjs`
step, which would fail on the `e2e` build's driver by design. The test
compares the two parsed manifests and fails on any other difference.

_Amended 2026-10-19 (011 T078): the file written to the state volume also names
the module's `dir` source and the two source lists by absolute path
(`withAbsoluteSources`), because flatpak-builder resolves relative paths from
the manifest's own folder and the state volume is not the checkout. That is the
writer's business: the derivation `manifest.test.ts` compares still differs
from the shipped manifest only in the ways above._

_Amended 2026-10-19 (011 T102): the cargo `--features` are now
`custom-protocol,e2e,flatpak-check`. `flatpak-check` keeps the `e2e` build's
WebDriver server and `HOPLODEX_E2E_*` seams but uses the real `AppOpener` and
the OS keyring (see [research.md §16](research.md)); no other difference from
the shipped manifest._

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

_Amended 2026-10-19 (owner's decision): `finish-args-home-filesystem-access`
goes with `--filesystem=home`, so the committed file holds the logind exception
only, unless the linter warns about a narrower grant (research.md §4)._

The format is the linter's own for local exceptions, confirmed when it was
first run (research.md §4): the linter takes the rule names from the object's
keys, so the reasons are for people.

_Amended 2026-10-19 (011 T078): the linter is run with `--exceptions
--user-exceptions flatpak/linter-exceptions.json`; without `--exceptions` it
reads no exceptions. The committed file stays at these two. `build.sh` lints
the built repository with a copy that also sets aside
`appstream-missing-screenshots` and
`appstream-screenshots-not-mirrored-in-ostree`, written under the state volume
and never committed: Flathub's pipeline mirrors the metainfo's screenshots
(addresses at the release's tag, which does not exist while the release is
made), and a local build can't (research.md §4)._

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
(`contents.mjs --installed` checks an app Flatpak installed and sets aside the
two files installing adds, `files/.ref` and `export/bin/<app id>`.)

_Amended 2026-10-19 (011 T024): flatpak-builder's appstream compose also adds
`share/app-info/xmls/io.github.exodious.HoploDex.xml.gz` and
`share/app-info/icons/flatpak/{48x48,64x64,128x128}/io.github.exodious.HoploDex.png`,
which `scripts/flatpak/contents.mjs` allows by name._

_Amended 2026-11-01 (#148): flatpak-builder 1.4.8 also records the app's
license file at `share/licenses/io.github.exodious.HoploDex/hoplodex/LICENSE`
(1.4.4 does not), which `scripts/flatpak/contents.mjs` allows by name, present
or absent._
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
