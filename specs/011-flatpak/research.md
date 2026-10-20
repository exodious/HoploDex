# Research: Build and Distribute a Flatpak

**Feature**: 011-flatpak | **Date**: 2026-10-19 | **Spec**: [spec.md](spec.md)

This file holds the decisions behind [plan.md](plan.md). Each section gives a
decision, why it was made and what else was considered. Facts read from
outside the repository carry the date they were read. The spike of
2026-10-18, run in a throwaway container while planning was paused, is
reported in full on #42 (comment of 2026-10-19, "Planning 011 paused"). The
sections below cite it as "the spike".

## §1 The runtime: GNOME 51, and what happens before its end of life (FR-001, FR-016)

- **Decision**: `org.gnome.Platform//51` and `org.gnome.Sdk//51` from Flathub,
  with the SDK extensions `org.freedesktop.Sdk.Extension.rust-stable` and
  `org.freedesktop.Sdk.Extension.node24` on the branch GNOME 51 is built on,
  Freedesktop SDK 26.08. The manifest names the branch (`51`), not one build
  of it, so Flathub's runtime updates go on under an installed HoploDex
  (spec, Edge Cases).
  - **What GNOME 51 gives HoploDex**: WebKitGTK 2.54 as `libwebkit2gtk-4.1`
    (the GTK 3 API that wry uses, with the `v2_40` features
    `src-tauri/Cargo.toml` asks for), GTK 3, glib, libsoup 3, OpenSSL (which
    the bundled SQLCipher links to on Linux, as it links the system's on a
    `.deb`), D-Bus, `xdg-dbus-proxy`'s client side and `flatpak-spawn`. On
    Flathub, GNOME 49, 50 and 51 all ship `libwebkit2gtk-4.1.so.0`; 48 is past
    its end of life (the spike).
  - **Rust**: `src-tauri/Cargo.toml` needs Rust 1.97 (`rust-version`) and
    edition 2024. The 26.08 branch of `rust-stable` carries Rust 1.98
    (tuna-os/compass#103, read 2026-10-19). The build fails if the
    extension's compiler is older than `rust-version`, so a lag shows at once.
  - **Node**: Node 24, as the dev container uses. The extension's own npm
    reads `package-lock.json`'s lockfile version 3, so npm 12 isn't needed
    inside the build.
  - **To confirm when implementation starts**: that the 26.08 branches of both extensions
    are stable on Flathub and not only `26.08beta` (GNOME's "This Week in
    GNOME" #263 still pointed developers at `rust-stable//26.08beta`). If one
    is still beta on the day, the manifest uses the beta branch until the
    stable one is out, and the release process (#19) checks it before
    0.1.0.
  - _Amended 2026-10-19 (011 T012):_ confirmed in the dev container
    (`scripts/dev-container.sh --flatpak`, after `env.sh`, with
    `flatpak remote-info --user flathub`). Both extensions are on the **stable**
    `26.08` branch (`Collection: org.flathub.Stable`): `rust-stable//26.08`
    (commit `14774ceb`, 2026-09-03) and `node24//26.08` (commit `eb3f0d47`,
    2026-09-19). Neither has a `26.08beta` ref on Flathub any more, so the
    manifest names `26.08` and no beta branch. `org.gnome.Platform//51` and
    `org.gnome.Sdk//51` are stable too (both from GNOME build 51.1-3, 2026-10-18).
    The `rust-stable` branch's compiler, from installing it into the
    container's own installation (`--no-deps`, under `/home/dev`, never the
    host's) and running its `bin/rustc`, is **rustc 1.98.0** (cargo 1.98.0),
    which is at least `src-tauri/Cargo.toml`'s `rust-version` 1.97, so the
    build's compiler is new enough. The extension's `bin` also has `cargo`,
    `cargo-clippy`, `cargo-fmt` and `ld.mold`. Its `remote-info` does not show
    the compiler, so installing it was the way to read it.
- **Rationale**: GNOME 51 is the newest branch with the web view Tauri 2
  needs, and the one supported longest: until about September 2027. GNOME
  removed `webkit2gtk-4.1` from its next runtime on 2026-09-28
  ([gnome-build-meta!4734](https://gitlab.gnome.org/GNOME/gnome-build-meta/-/merge_requests/4734)):
  "Any apps still relying on it should port to webkitgtk-6.0 or stick with
  the GNOME 51 runtime, which will be supported till September 2027." So
  FR-016's move to a newer branch can't happen until HoploDex's web view
  changes. The owner chose on 2026-10-19 to go ahead anyway: Tauri may
  change its Linux web view before then (spec, Clarifications 2026-10-19).
- **Before GNOME 51's end of life** (FR-016, as amended): a follow-up issue,
  opened with this feature's pull request (plan.md, "Close-out"), decides
  what the release process does by then. Its starting points, from #42's
  comment:
  - **Tauri's GTK 4 port**: not merged anywhere on 2026-10-18.
    tauri-apps/tauri#12562, tauri-apps/tauri#14684 (3.0 milestone),
    tauri-apps/wry#1767 and tauri-apps/tao#1104 are open. A maintainer's
    `tauri-apps/winit-gtk4` may replace porting tao. If it lands, the
    manifest moves to the newest GNOME branch with `webkitgtk-6.0`.
  - **Tauri's Chromium runtime** (`feat/cef`): takes WebKitGTK out, but
    bundles Chromium, prebuilt by a third party unless built here.
  - **Building `webkit2gtk-4.1` as a module of this manifest**, on a newer
    runtime: frees the branch, but the bundle would then carry an LGPL
    library (010 FR-017 forbids that without a decision on its source;
    FR-018 here forbids it as written), and HoploDex would own WebKit's
    security updates.
  - **Publishing only the `.deb` and `.rpm`** after September 2027.
  - What doesn't help: an AppImage escapes GNOME's schedule but not
    WebKitGTK's. It carries the build machine's WebKit, frozen until the
    next HoploDex release, and reopens 010 FR-017's source question.
- **Alternatives considered**:
  - **GNOME 50** (supported until about March 2027): the same web view,
    half a year less.
  - **The Freedesktop runtime with WebKitGTK as a module**: see above.
  - **Waiting for Tauri's port before shipping a Flatpak**: leaves Linux
    users without a self-contained bundle (010 FR-017) for an unknown time.

## §2 Building from source, offline (FR-014, FR-026)

- **Decision**: `flatpak-builder` builds one module, `hoplodex`, from the
  checkout, in two phases: `--download-only` fetches every source the
  manifest names (the checkout itself, plus the crates and npm packages in
  the offline source lists, §3); then `--disable-download` builds with the
  build sandbox's network off. The manifest asks for no
  `build-args: --share=network`, and a test fails if it ever does, so no
  step can reach the network during the build (SC-004).
  - **The module's commands**, in order:
    1. `npm ci --offline`, from the npm cache the node source list fills.
    2. `npm run build` (the `tsc` check, then `vite build`, which writes
       `dist/` with `notices.json` and `NOTICES.txt`; 010 FR-008, FR-011).
       The notices plugin runs `cargo fetch --locked`, `cargo metadata` and
       `cargo tree` (#42's comment from 010). With `CARGO_NET_OFFLINE=true`
       and the vendored crates in `CARGO_HOME` (§3), each runs offline. The
       plugin reads each crate's license files from its vendored folder,
       which holds the crate's whole published package. `git` isn't in the
       build, so the copyright's last year is the build's year, as 010
       documents.
    3. `cargo build --release --offline --locked --features custom-protocol
       --bin hoplodex`, the same features and profile as a release
       `tauri build`.
    4. `node scripts/check-no-webdriver.mjs` on the binary it built, as
       `build-appimage.sh` does for the AppImage's.
    5. Install the binary, the desktop entry, the AppStream metadata, the
       icons and `NOTICES.txt` (§13).
  - **Cargo directly, not the Tauri CLI**: `tauri build` would bring in
    `@tauri-apps/cli`'s prebuilt native binary and its bundlers, none of
    which the Flatpak needs. The E2E build already calls cargo directly
    (`e2e/support/build.ts`). Tauri's `build.rs` (`tauri-build`) needs
    nothing the SDK lacks.
  - **The source is the checkout** (`type: dir`, path `..`), so the build is
    of exactly what is checked out, uncommitted changes included, as every
    other local build is. A Flathub submission would swap it for a
    `type: git` source pinned to the release tag and commit (§19).
  - **State and output**: `flatpak-builder`'s state directory (its download
    cache and build directories, several gigabytes) goes in a per-checkout
    volume (§15). The `.flatpak` file goes to `flatpak/out/` in the
    checkout, which git ignores, so the host can reach it.
- **Rationale**: the spec's assumption ("built from source, offline", FR-014,
  FR-026) and Flathub's rules. Building inside `flatpak-builder`'s own
  sandbox, which has no network, makes "no network during the build" a
  property of the tool, not of a promise in a script.
- **Alternatives considered**:
  - **Repackaging the `.deb`** (the route Tauri's guide also shows): faster,
    but nothing would be built from source, and Flathub doesn't take it for
    an open source app.
  - **`npm run tauri build` inside the module**: needs the CLI's prebuilt
    binary, and builds bundles that would be thrown away.
  - **A `type: git` source for local builds too**: builds only what is
    committed, which makes trying a change slower, and gains nothing until
    a submission.

## §3 The offline source lists (FR-015)

- **Decision**: two generated files, committed beside the manifest:
  `flatpak/cargo-sources.json` from `src-tauri/Cargo.lock`, made by
  `flatpak-cargo-generator.py`, and `flatpak/node-sources.json` from
  `package-lock.json`, made by `flatpak-node-generator npm`. Both tools come
  from [flatpak/flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools),
  pinned to one commit and installed in the dev container's image (§15).
  `npm run flatpak:sources` regenerates both.
  - **The check** (`scripts/flatpak/sources.test.ts`, run by `npm test`, so
    by every test run): it reads both lockfiles and both lists and fails,
    naming the list to regenerate and the first package that differs, when a
    package in a lockfile has no source in its list with the same version
    and checksum, or the list has a source no lockfile package needs. It
    needs no Python and no network, so it is cheap enough for every run,
    unlike regenerating (SC-004, US3-2).
  - **What the lists cover**: every registry package in `Cargo.lock`
    (crates for every target, since `cargo tree --target` is asked about
    every OS by the notices check), and every package in
    `package-lock.json`, dev dependencies included, since `npm run build`
    needs them. The workspace's path dependency
    (`src-tauri/vendor/tauri-plugin-wdio-webdriver`) is in the checkout and
    needs no source. `Cargo.lock` has no `git` sources today; the check
    fails on one, since the generator would then need the network.
- **Rationale**: these are the generators Flathub and Tauri's own Flatpak
  guide use, and writing an npm one ourselves would mean reproducing npm's
  cache layout. The content check is what makes "a lockfile change without
  them fails the checks" hold on every run, not only when someone runs the
  Flatpak build.
- **Alternatives considered**:
  - **Our own generators in Node**: the cargo one would be small; the npm
    one wouldn't, and both would need testing against what
    `flatpak-builder` expects.
  - **A hash of each lockfile, stored with the lists**: cheaper still, but
    it can be refreshed without regenerating, and its failure can't say
    which package differs.
  - **A git submodule for flatpak-builder-tools** (Tauri's guide): every
    checkout then needs submodules, for a tool only the container runs.

_Amended 2026-10-19 (011 T002): the pinned commit is
`74697c75b630d7330e77250fc13cb5ea688d9479` (master, committed 2026-09-30,
the newest on the day), cloned into `/opt/flatpak-builder-tools` by the
`Dockerfile`'s `FLATPAK_BUILDER_TOOLS_COMMIT`. The generators' modules
(`aiohttp`, `tomlkit`, `PyYAML`) come from Debian (`python3-aiohttp`,
`python3-tomlkit`, `python3-yaml`), so there is no
`scripts/flatpak/requirements.txt`._

## §4 Permissions (FR-002, FR-003, FR-005, FR-006, FR-008, FR-026)

- **Decision**: the manifest's `finish-args` are exactly these, each with a
  comment in the manifest giving its reason. A test (`scripts/flatpak/
  manifest.test.ts`) compares them with the list below, so a permission
  can't be added or dropped without changing this record and the test.

| Permission | What needs it | What a compromised HoploDex process could reach with it |
|---|---|---|
| `--share=ipc` | X11's shared memory, which GTK uses under X11 | Shared memory with other X11 clients of the session |
| `--socket=wayland` | The window, on Wayland | The Wayland compositor, as any windowed app |
| `--socket=fallback-x11` | The window, on X11 only when there is no Wayland | The X server, where X11 lets any client read other windows' input and contents |
| `--device=dri` | WebKit's GPU rendering; without it, it renders on the CPU | The GPU's device nodes |
| `--filesystem=home` | Databases, backups beside them, typed locations, imports and exports anywhere in the home folder (FR-003) | Every file of the user's home folder, apart from what Flatpak keeps from every app (other apps' `~/.var/app` folders among them) |
| `--filesystem=/media`, `--filesystem=/run/media`, `--filesystem=/mnt` | Databases and backups on removable drives (FR-003) | Every file on a mounted removable drive |
| `--talk-name=org.freedesktop.secrets` | Saved passphrases, through the Secret Service (FR-006) | Every item in the user's unlocked keyring collections, as on a `.deb` install: the Secret Service doesn't separate one app's items from another's (§7) |
| `--talk-name=org.freedesktop.ScreenSaver`, `--talk-name=org.gnome.ScreenSaver` | Locking when the screen locks (003 FR-038; FR-005) | The screen saver's interface: whether the screen is locked, and its methods, among them inhibiting it |
| `--system-talk-name=org.freedesktop.login1` | The sleep and shutdown notices and their delay, and the session's lock signals (003 research.md §14; FR-005) | logind's interface on the system bus. Its actions are still checked by polkit as for any process of the user: inhibiting sleep, listing sessions and, where polkit allows a local user, suspending or powering off |

  - **No `--share=network`**: HoploDex is local-only. The sandbox has a
    network namespace with nothing but loopback, so neither the app nor
    any process it starts can send a byte off the computer. That is
    stronger than a `.deb` install, and is recorded with the residual
    risks (§20). Only the automated check adds network, at run time, for
    WebDriver (§16).
  - **No `--socket=pulseaudio`, `--device=all`, `--filesystem=host`,
    `--talk-name=org.freedesktop.Flatpak` or `--own-name`**: nothing uses
    them. The desktop portals HoploDex uses (the file chooser, OpenURI, the
    Documents portal, the Flatpak portal behind `flatpak-spawn --sandbox`)
    are reachable from every Flatpak without a permission.
  - **Flathub's linter**: `--filesystem=home` raises
    `finish-args-home-filesystem-access`, and the logind name raises
    `finish-args-login1-system-talk-name` (the spike). Both are kept with a
    recorded exception and its reason in
    `flatpak/linter-exceptions.json`, which the build passes to the
    linter. The rest raises nothing. FR-026 holds: the linter passes with
    each warning kept only by a recorded exception. The exact flag
    (`--user-exceptions`) and the file's format are confirmed when the
    linter is first run.
    _Amended 2026-10-19 (011 T066):_ confirmed on the linter from
    `org.flatpak.Builder` (stable). The flag is `--user-exceptions <file>`
    **together with `--exceptions`**: without `--exceptions` the linter reads no
    exceptions at all and reports both rules, and with `--exceptions` and a
    `--user-exceptions` file it asks Flathub for nothing. The file is JSON keyed
    by the app id, and the linter takes the rule names from it with
    `set(exceptions.get(appid, []))`, so `{appid: {rule: reason}}` (the shape of
    `flatpak/linter-exceptions.json`) and `{appid: [rule, ...]}` both work; the
    reasons are for people. `build.sh` runs it on the manifest and on the
    repository, which it must be allowed to see (`flatpak run
    --filesystem=<state>:ro`, since the linter's own sandbox does not see the
    state volume). The manifest passes with exactly the two exceptions. The
    repository passes with them plus two screenshot rules,
    `appstream-missing-screenshots` and
    `appstream-screenshots-not-mirrored-in-ostree`: Flathub's pipeline mirrors
    the metainfo's screenshots into the repository (`flatpak-builder
    --mirror-screenshots-url`, which downloads each address), and the addresses
    are at the release's tag (§13), which does not exist while the release is
    being made. `build.sh` therefore adds those two, for the repository lint
    only, in a file it writes under the state volume; the committed file stays at
    two (`manifest.test.ts`). A submission to Flathub gets the screenshots
    mirrored by its own pipeline (§19, item 7).
    _Amended 2026-10-19 (011 T078): checked against what was built.
    `flatpak/io.github.exodious.HoploDex.yml`'s `finish-args` are this table's
    rows, in this order (twelve arguments: the `/media`, `/run/media` and `/mnt`
    row is three of them and the screen savers' row two), with no
    `--share=network`, and `flatpak/linter-exceptions.json` holds exactly the two
    exceptions above, each with a reason citing §4 and §5 or §6. `build.sh`
    passes it with `--exceptions --user-exceptions`, as the T066 note says.
    Nothing in this section changed._
  _Amended 2026-10-19 (owner's decision; spec Clarifications 2026-10-19,
  FR-002, FR-003, FR-006, SC-011): secure defaults, after §9's "One limit"
  showed that whatever the Flatpak reaches, a compromised web process can reach
  with one D-Bus call. Three rows change:_

  | Permission | What needs it | What a compromised HoploDex process could reach with it |
  |---|---|---|
  | `--filesystem=xdg-documents/HoploDex:create` (replaces `--filesystem=home`) | Databases and their backups in the suggested folder, made if missing (FR-003, FR-010) | Every file under `<Documents>/HoploDex`, and nothing else of the home folder |
  | `--filesystem=/media`, `--filesystem=/run/media` (`/mnt` removed) | Databases and backups on removable drives (FR-003) | Every file on a mounted removable drive |
  | _(none; `--talk-name=org.freedesktop.secrets` removed)_ | Saved passphrases, through the Secret portal, which needs no permission (§7) | HoploDex's own saved passphrases, and no other keyring item |

  _That is ten arguments. A compromised process no longer reaches `~/.bashrc`,
  `~/.config/autostart` or any other file of the home folder, so it can't leave
  the sandbox by writing a file the desktop or a shell later runs.
  `finish-args-home-filesystem-access` leaves `linter-exceptions.json`, which
  keeps the logind exception only; if the build's linter warns about a
  narrower grant, the warning is answered by the grant or recorded with its
  reason, as before. `manifest.test.ts` gains SC-011's check: it fails on any
  `home`, `~`, `host` or `~/…` grant, any `xdg-*` grant other than
  `xdg-documents/HoploDex`, any mount root other than `/media` and
  `/run/media`, and any `--talk-name` of `org.freedesktop.secrets`. What a
  user grants later with `flatpak override` is their own choice, and `Reach`
  honours it (§5). The first alternative below, "`xdg-documents` only", was
  rejected because typed locations and backups beside a database need a
  folder; the scoped folder gives both inside it, and the refusal's override
  gives them anywhere else._
- **Rationale**: the spec settled the reach (Clarifications, FR-003) and the
  keyring route (FR-006). The others are what the existing code already
  uses on Linux, kept so the Flatpak runs the same code as the `.deb`.
- **Alternatives considered**:
  - **`--filesystem=xdg-documents` only**, with everything else through the
    file chooser: rejected in the spec's clarifications, since backups
    beside a database and typed locations need the folder.
  - **The Inhibit portal's session monitor** (`org.freedesktop.portal.Inhibit`,
    `CreateMonitor`) instead of the ScreenSaver names and logind: it
    reports screen-lock and the session ending, but offers no delay
    before sleep (spec, Assumptions), so logind is needed anyway. It is a
    second code path for the same notices.
  - **The Secret portal**: see §7.

## §5 What is within reach, and refusing what isn't (FR-003, FR-004, FR-007, FR-010)

- **Decision**: a new backend module, `services/sandbox.rs`, says whether a
  path is within reach, and every command that names a location asks it
  before anything is written.
  - **Reach** is read once at startup from `/.flatpak-info`: its
    `[Context]` group's `filesystems=` key, which holds the manifest's
    grants with the user's own overrides
    (`flatpak override --filesystem=…`) already merged. `home`, `host`,
    `~/…`, `xdg-*` folders and absolute paths are understood, `:ro` marks a
    read-only grant, and a `!` (an override that removes one) takes it
    away. A grant it doesn't understand is not counted, so a mistake
    narrows reach and never widens it. Outside Flatpak (no
    `/.flatpak-info`), everything is within reach and nothing changes for
    a `.deb`, `.rpm`, AppImage, macOS or Windows install. The parsing is a
    pure function of the file's text and the home folder, tested on its
    own (data-model.md, "Reach").
  - **Three needs**, since operations need different things:
    - **A database's folder** (`create_database`, `open_database`, opening
      from a recent entry): the database's folder must be within reach and
      writable. `change_passphrase` and `restore_backup` replace a database
      that is open, or was, and so already passed this check. Opening writes beside the file (the open
      marker, SQLite's journal, the replacement's `.<file>.new`, the
      backups' default folder), so a database the file chooser handed over
      without its folder is refused at open (spec, Clarifications
      2026-10-19).
    - **A folder to write into** (the backup location, an export's
      destination): within reach and writable, or a folder the file chooser
      handed over through the Documents portal, inside which writing is
      allowed.
    - **A file to read** (an import's files, a photo or document attached by
      the chooser or dropped, a backup being restored): any path that exists
      and can be read, a Documents-portal path included. A path that
      doesn't exist inside the sandbox and lies outside reach is out of
      reach, not "not found".
  - **Documents-portal paths** (`/run/user/<uid>/doc/<id>/…`): the file
    chooser returns one even for a file the app could reach directly
    (flatpak/xdg-desktop-portal#1120). The module asks the Documents portal
    for the file's host path (`org.freedesktop.portal.Documents.GetHostPaths`,
    with the `user.document-portal.host-path` extended attribute as a
    fallback where the portal is older). When the host path is within
    reach, it is used instead, so a database picked in the home folder
    opens from its real path, and its recent entry, backups and
    replacement work. Otherwise the portal path is used where reading or
    writing inside it is enough, and refused where a database's folder is
    needed.
  - **The refusal**: a new error code, `LOCATION_OUT_OF_REACH`, with
    `details: { path, grants }` and a message that names the location, says
    this copy of HoploDex is a Flatpak that can reach only the home folder
    and removable drives, and gives both fixes: move the file into the
    home folder or onto a removable drive, or give HoploDex access with
    `flatpak override --user --filesystem=<folder> io.github.exodious.HoploDex`
    and then restart HoploDex (FR-004). Because the module reads the
    overrides, that advice works: after the restart, the location is within
    reach. A running sandbox can't gain access, so the restart is needed. The backup location's
    existing `BACKUP_LOCATION_UNAVAILABLE` gains the reason `outOfReach`
    rather than a second code, and so do Database settings' "Not available
    on this computer" and the close's `failureReason`
    (contracts/tauri-commands.md). The check runs before any scratch file,
    probe or backup is made, so nothing is left behind (SC-003).
    _Amended 2026-10-19 (011 T078): `details` is `{ path, folder, grants }`
    (`folder` is the folder that would need access, which the override command
    names), and the built message names `grants` in its first sentence unless
    they are the shipped four, and shell-quotes the override's folder when a
    shell would split it (data-model.md, "Error `LOCATION_OUT_OF_REACH`";
    `CommandError::out_of_reach(path, folder, home, grants)`)._
  - **Drops**: WebKitGTK's GTK 3 drop target asks only for `text/uri-list`,
    so a drop always arrives as the file's host path (the spike). One within
    reach is attached as on a `.deb`; one outside it doesn't exist in the
    sandbox and gets `LOCATION_OUT_OF_REACH` (spec, Clarifications
    2026-10-18, FR-007 as amended).
  - **Network shares and synced folders** (spec, Edge Cases): one mounted
    under the home folder or a removable-drive folder is within reach. A
    desktop's own network mounts under `/run/user/<uid>/gvfs` are not, and
    get the refusal.
  - **The suggested location** (FR-010): `app_dirs::document_dir` asks the
    OS through Tauri, which reads `XDG_DOCUMENTS_DIR` from
    `$XDG_CONFIG_HOME/user-dirs.dirs`. Flatpak sets `XDG_CONFIG_HOME` to the
    app's own folder, but with home access it binds the host's
    `user-dirs.dirs` into it, so a localised Documents folder resolves to
    the same path as on the `.deb` (the spike). No code changes.
- **Rationale**: FR-004 asks for the check before anything is written, and
  "how to give access". Reading `/.flatpak-info` follows the manifest and
  the user's overrides without a second list in the code, and keeps the
  module inert everywhere else. A probe by trying to write would leave the
  very files FR-004 forbids.
- **Alternatives considered**:
  - **A list of the four granted roots in the code**: it would contradict
    the advice to run `flatpak override`.
  - **Reading `/proc/self/mountinfo`** to see what is bound in: closer to
    the truth, but harder to read reliably than the file Flatpak writes for
    exactly this.
  - **Letting the operation fail and turning the I/O error into the
    message**: an `ENOENT` or `EROFS` can't be told from an ordinary
    missing folder, and the failure comes after scratch files are made.
  - **The file-transfer portal for drops**: the web view never offers it
    (spec, Clarifications 2026-10-18).

_Amended 2026-10-19 (owner's decision; FR-003, FR-007, FR-010):_

- _**Flatpak's rule for `xdg-*` folders.** Flatpak grants nothing for an
  `xdg-*` folder that is unset or set to the home folder itself (xdg-user-dirs'
  way of turning one off). `UserDirs` applies the same rule, or `Reach` would
  count a grant the sandbox doesn't have: with Documents set to `$HOME`,
  `xdg-documents/HoploDex` would wrongly count `~/HoploDex`. Unit tests cover
  both cases._
- _**The shipped default is three grants**: `<Documents>/HoploDex`, `/media`
  and `/run/media`, each writable. `CommandError::out_of_reach` words the
  refusal for them when the grants are exactly these (the Documents one
  resolved through the same `UserDirs`), and lists any other set as before
  (contracts/tauri-commands.md)._
- _**No grantable Documents folder.** `suggested_folder`
  (`commands/databases.rs`) falls back to `~/Documents/HoploDex` when there is
  no Documents folder. In the Flatpak that folder isn't granted then, so
  creating there is refused with the override command; nothing else is
  suggested (spec, Clarifications)._
- _**A drop from outside reach.** The file chooser hands a file as a
  Documents-portal path, which is read wherever it is (`Need::ReadFile`), so a
  host path outside reach reaching `add_photo_from_path` or
  `add_document_from_path` can only be a drop. Their refusal therefore always
  says to attach the file with the file chooser instead; an import's and the
  other commands' refusals don't (contracts/tauri-commands.md)._
- _**To confirm first.** With no home folder in reach, whether the sandbox
  still sees the host's `user-dirs.dirs` (`Reach` and `app_dirs` read it to
  find Documents). The check's US1-2 test (`xdg-user-dir DOCUMENTS` inside the
  sandbox against the host's) settles it in the first run on the narrower
  manifest, before anything else is built on it._

## §6 Sleep, shutdown and screen-lock notices (FR-005)

- **Decision**: `platform/linux.rs` keeps its listeners, through the three
  bus names of §4, with one change: the logind session it listens to is
  always `/org/freedesktop/login1/session/auto`, and the
  `GetSessionByPID(std::process::id())` call goes.
  - **Why**: inside Flatpak the app has its own PID namespace (it runs as
    PID 2 under bwrap; the spike), so the PID it sends is not its PID on
    the host. logind would look up a different process, fail, or in the
    worst case answer with another session. `session/auto` is resolved by
    logind from the caller's own credentials on the bus, which the bus
    takes from the connection, not from anything the app says. On a GNOME
    or KDE desktop today, an app launched from the desktop runs in an app
    scope outside any logind session, so `GetSessionByPID` already fails
    there and the code already falls back to `auto`. Using it always
    removes a lookup that is wrong in one install and useless in most.
  - **The delay inhibitor** (003 research.md §14): logind's `Inhibit`
    returns a file descriptor, and `xdg-dbus-proxy` passes descriptors
    through. Read from the source in the spike, not yet run; the automated
    check (§16) shows it, and the manual procedure (§17) shows a real
    sleep held back.
    _Amended 2026-10-19 (011 T060): run, in the container's Flatpak 1.16.6
    with `xdg-dbus-proxy` 0.1.6. `e2e/specs/flatpak.e2e.ts` "011 US2-1": while
    the database is open, the stand-in logind lists a `sleep` and a
    `shutdown` delay inhibitor held by "HoploDex", so `Inhibit` reached the
    app's end through the proxy with its descriptor. The stand-in keeps
    the other end of each descriptor and drops an inhibitor only once the
    last end is closed, so the app's end is what its list shows going: after
    `PrepareForSleep(true)` the database is locked 70 to 80 ms after the
    signal is sent and the `sleep` inhibitor is gone 80 ms after (the
    `shutdown` one stays); after `PrepareForSleep(false)` the app takes the
    `sleep` one again. After `PrepareForShutdown(true)` the app has ended
    11 ms after the signal and neither inhibitor is left. A sleep locks in
    69 ms in the Flatpak against 68 ms for the same build outside it (SC-002;
    `flatpak/out/check/lock-times.log`)._
  - **Unavailable notices**: unchanged. When the ScreenSaver names have no
    owner and the session object can't be read, `spawn` returns false and
    the setting shows as unavailable (003 FR-038, US2-4). Without the
    system bus, the log says so and the wake watchdog still notices a
    sleep afterwards.
  - **Logout and shutdown**: when the session ends, systemd stops the
    session's processes, the Flatpak's included, with SIGTERM; the app
    receives it directly and closes at once, keeping pending changes, as
    on a `.deb` (§12).
- **Rationale**: same code, same behaviour, one fewer guess about PIDs.
- **Alternatives considered**: `GetSessionByPID` with the host PID, read
  from `/proc/self/status`'s `NSpid` (the namespace hides it); the Inhibit
  portal's monitor (§4).

_Amended 2026-10-19 (011 T102): the D-Bus permissions of §4 and §6 are
unchanged (`finish-args` stays the recorded list), but what answers behind
two of the names is now real. On the check's private session bus,
`org.freedesktop.secrets` is a headless `gnome-keyring-daemon` (unlocked, its
data under the run's throwaway folder) and `org.freedesktop.portal.Desktop` is
a scripted dbusmock `OpenURI` that records each `OpenFile`'s descriptor and
sends `Response` after its reply. The check variant is built with the
`flatpak-check` feature, so the app's `AppOpener` and keyring backend are the
shipped ones. Both services talk to the app through the same
`--talk-name=org.freedesktop.secrets` and `--talk-name=org.freedesktop.portal.Desktop`
grants, so the journeys now show that those grants are enough for a saved
passphrase to be written, read and used, and for a descriptor to pass the proxy
to the portal. The logind and screen-saver stand-ins are unchanged.
First run of the check (011 T111, `flatpak/out/check/lock-times.log`): a sleep
locked the database in 69 ms in the Flatpak and 74 ms outside it, a screen lock
in 73 and 77 ms, `PrepareForShutdown` ended the app in 11 and 17 ms, and SIGTERM
in 40 and 11 ms; the check fails when any Flatpak time is over a second more
than its outside time (SC-002)._

## §7 Saved passphrases (FR-006)

- **Decision**: unchanged code. The `keyring` crate's Secret Service backend
  talks to `org.freedesktop.secrets` through the bus name of §4, with the
  same service and user names as on every other install. Its availability
  probe (003 FR-019) answers false when the name isn't reachable, and
  saving a passphrase shows as unavailable (US2-5).
  - **What it reaches** (the permission record, FR-006): the Secret Service
    API has no per-application separation, so through it HoploDex, or
    anything that compromised it, can read every item in the user's
    unlocked collections, exactly as a `.deb` install can. The sandbox
    narrows nothing here.
  - **A `.deb` user moving to the Flatpak** (spec, Edge Cases): entries are
    keyed by the database's id (003 research.md §10), so the saved
    passphrase is found again once the database is opened from the
    Flatpak's recent list.
- **Alternatives considered**: **the Secret portal**
  (`org.freedesktop.portal.Secret`), which gives the app a secret of its
  own for a local encrypted file. It reaches nothing else, but the entries
  would then live in the Flatpak's own storage, not the user's keyring, so
  a `.deb` and the Flatpak wouldn't share them, and the `keyring` crate
  would need another backend. The spec chose the Secret Service
  (Clarifications 2026-10-18).

_Amended 2026-10-19 (owner's decision; FR-006): inside a Flatpak, saved
passphrases go through the Secret portal. The `.deb`, `.rpm`, macOS and
Windows keep the `keyring` crate, unchanged._

- _**Client**: `oo7` 0.6 (MIT; github.com/linux-credentials/oo7), with
  `default-features = false, features = ["async-std", "native_crypto"]`: pure
  Rust (RustCrypto: aes, cbc, hkdf, pbkdf2, sha2), no C and no OpenSSL. Its
  only `-sys` crate, linux-raw-sys, is already in the tree. It shares the app's
  one zbus 5 and adds about 25 crates, Linux only. Not its `tokio` feature,
  which turns on `zbus/tokio` for the whole graph and moves the app's
  `zbus::blocking` calls onto tokio._
- _**Never the Secret Service.** Not `oo7::Keyring::new()`, which falls back
  to the Secret Service when the portal is missing; FR-006 forbids any
  fallback. Only the portal path: `oo7::ashpd::desktop::secret::retrieve()`
  for the app's secret, then `oo7::file::UnlockedKeyring::load(path, secret)`
  on `$XDG_DATA_HOME/keyrings/default.keyring`. In the Flatpak that is
  `~/.var/app/io.github.exodious.HoploDex/data/keyrings/default.keyring`, in
  libsecret's file format. It is driven from the app's sync code with
  `zbus::block_on`. Nothing in this path names `org.freedesktop.secrets`._
- _**One API.** A small keyring-core `CredentialStoreApi` over that file
  keyring is installed with `set_default_store` when `Reach` is `Flatpak`, so
  `services::keyring` keeps one `Entry` path with the same service and user
  names. The `e2e` build's mock store stays for every other E2E run. The
  `flatpak-check` build uses the portal store, as it uses the real opener
  (T102)._
- _**Availability (003 FR-019)**: available when `org.freedesktop.portal.Desktop`
  has an owner and its `org.freedesktop.portal.Secret` `version` property
  reads. xdg-desktop-portal exports that interface only when a backend
  implements it. ashpd's `Secret::new()` answers Ok when the portal service
  itself is missing, so the owner is checked first. The probe never calls
  `RetrieveSecret`, whose first call makes the app's key and may prompt._
- _**Who provides it**: gnome-keyring 51 (`gnome-keyring.portal`, GNOME);
  KWallet's ksecretd (`kwallet.portal`, KDE Frameworks 6.2 and later); and
  oo7-portal (Fedora's oo7-daemon, which replaces both there). Where none is
  configured, saving shows as unavailable._
- _**Not checked**: Xfce, Cinnamon, MATE and wlroots desktops such as Sway.
  gnome-keyring's portal file says `UseIn=gnome`, so on those the portal
  likely finds no Secret backend unless the user's `portals.conf` names one,
  and saving shows as unavailable. Nothing in this is specific to one
  distribution: `oo7` is only the client, and any desktop with a Secret
  portal backend serves it. A manual check on one such desktop before a
  release would confirm it (quickstart.md M5)._
- _**What it reaches.** The portal takes the app id from the caller's
  `.flatpak-info`, never from its arguments. The backend keeps one random
  64-byte key per app id in the user's login keyring ("Application key for
  io.github.exodious.HoploDex"), and the file is encrypted with it (PBKDF2-SHA256
  at 100,000 iterations, AES-128-CBC with HMAC-SHA256). A compromised
  HoploDex process gets HoploDex's key and file, so its saved passphrases, and
  no other item. A process on the host, or an app granted the Secret Service,
  can still read that key, as it could read the entries before._
- _**Moving from a `.deb`**: entries aren't shared. The user saves the
  passphrase again in the Flatpak (spec, Edge Cases); nothing reads the
  Secret Service to bring them over, since that would need its permission._
- _**Alternatives**: libsecret through FFI (C and the runtime's library, with
  oo7's README citing its sandbox issues); ashpd alone (it gives only the key,
  so the encryption and the file format would be ours); the `keyring` crate's
  Secret Service stores (no portal support)._

## §8 Opening a document in another application (FR-008)

- **Decision**: inside Flatpak, `AppOpener` on Linux opens the copy through
  the OpenURI portal directly: `org.freedesktop.portal.OpenURI.OpenFile`
  with a read-only descriptor for the copy and `writable: false`, then
  waits for the request's `Response` signal. Outside Flatpak it runs
  `xdg-open` as now.
  - **Why not `xdg-open` in the sandbox**: the runtime's `xdg-open` hands
    the file to the portal and exits 0 as soon as the call is accepted,
    without waiting to see whether an app took it. With no portal at all
    it exits 4, which today's code already reads as "no application" (the
    spike). So "no application for this type" would be lost silently.
  - **The answer**: response 0 is opened; 2 (the portal couldn't open it,
    or no application handles the type) is `OpenFailure::NoApp`, which
    `open_document` already turns into `NO_APP_FOR_DOCUMENT`; 1 (the user
    closed the portal's own application chooser) is treated as opened,
    since the user made the choice; no portal service on the bus is
    `NoApp`. The wait is bounded as `xdg-open`'s is (5 s), after which the
    document counts as opened, since the portal's chooser may be waiting
    on the user.
  - **The copy** (001 FR-035): stays in the app's cache directory, which in
    Flatpak is inside `~/.var/app/io.github.exodious.HoploDex/cache`, and
    is deleted as on every other install. The portal shares it with the
    receiving app through the Documents portal; deleting the copy ends
    that app's access to its contents.
  - **What the Documents portal keeps**: its database records the copy's
    path, file name included, after the copy is deleted. On a `.deb`, the
    receiving app's recent-files list does the same. Recorded with the
    residual risks (§20).
  - **The E2E build** keeps `E2eOpener` (no app is started from a test),
    so the portal call is tested in Rust against a test portal on a private
    bus (plan.md, Test Levels).
- **Rationale**: the existing failure path (007 research.md §18) needs an
  honest "no app", and only the portal's response gives it.
- **Alternatives considered**: `gio open` (the same portal call, the same
  early exit); `tauri-plugin-opener`'s Linux path (`xdg-open` again).

## §9 WebKit's sandbox and the PDF preview (FR-011)

- **Decision**: the Flatpak runs 007's startup probe like any other Linux
  install. `/.flatpak-info` comes out of `sandbox_probe::SKIP_MARKERS`, and
  `/run/.containerenv` and `/.dockerenv` stay, since inside a Flatpak the
  sandbox's own `/run` and root hide the container's markers anyway.
  - **How WebKit sandboxes inside Flatpak**: with `WEBKIT_FORCE_SANDBOX=1`,
    WebKitGTK starts each web process through `flatpak-spawn --sandbox`
    (WebKit's `FlatpakLauncher`), which asks the Flatpak portal for a
    sub-sandbox. `flatpak-spawn` is in the GNOME 51 Platform, and it worked
    in the spike, contrary to 007 research.md §9, which assumed WebKit
    won't sandbox inside Flatpak. The probe is the same: a crash, a
    non-zero exit or a time-out leaves it off and is logged once.
  - **With the sandbox on** (the expected case): a web process, the PDF
    surface's and the main window's, runs in a sub-sandbox with none of the
    app's `--filesystem` grants, no network (it has none anyway, §4) and
    only the sockets WebKit asks `flatpak-spawn` to share for rendering
    (the display, the GPU, accessibility). The exact `flatpak-spawn`
    arguments WebKit 2.54 passes are read from the running process during
    implementation and recorded in this section before the pull request.
    _Amended 2026-10-19 (011 T060): read from the running check, with the
    container's Flatpak portal (1.16.6) and WebKitGTK 2.54 of GNOME 51
    (`e2e/specs/flatpak.e2e.ts` "011 US2-7" writes it to
    `flatpak/out/check/webkit-sandbox.txt`). For each web process WebKit runs
    `flatpak-spawn --forward-fd=<n> --expose-pids --watch-bus --sandbox
    --directory=/ --no-network --sandbox-flag=share-gpu
    --sandbox-flag=share-display --sandbox-flag=share-sound
    --sandbox-flag=allow-a11y --sandbox-flag=allow-dbus
    --sandbox-a11y-own-name=io.github.exodious.HoploDex.Sandboxed.WebProcess-<uuid>
    --env=<each variable of the app's environment> …
    /usr/libexec/webkit2gtk-4.1/WebKitWebProcess <fds>`, with
    `WEBKIT_FORCE_SANDBOX=1` among the variables. There is no `--sandbox-expose*`
    and no `--filesystem`, so no folder is shared. The portal then runs
    `bwrap --args <fd> -- WebKitWebProcess …` for it and a `bwrap … xdg-dbus-proxy`
    for its session bus; the `bwrap` arguments travel in a descriptor that is
    closed once they are read, so `/proc/<pid>/cmdline` shows only the
    descriptor. What the sub-sandbox shares was read from the web process
    itself (`/proc/<pid>/ns`, `mountinfo`, `net/dev`) and by starting a
    process with WebKit's own flags and looking from inside:
    - **Shared with the app**: the user, UTS and IPC namespaces (IPC is the
      app's own `--share=ipc`), the X11 socket and `Xauthority` (the display),
      the runtime and `/app` read-only, the fonts and icons. `share-gpu`
      and `share-sound` ask for the GPU's device nodes and the audio socket;
      the container has neither, so the check can't show them. HoploDex
      itself asks for no audio.
    - **Its own**: a PID namespace, a mount namespace and a network namespace
      with only `lo`.
    - **Not there**: the home folder (`$HOME` is an empty folder), Documents,
      `/media`, `/run/media` and `/mnt`, the app's `~/.var/app` folders, `/tmp`,
      `/var/data`, and the system bus socket. The database file can't be opened
      from it (ENOENT).
    - **The session bus**: through a proxy of its own (`allow-dbus`). The
      Secret Service and both screen savers were refused (`ServiceUnknown`)
      while their stand-ins ran; the desktop portals answer, as they do for
      every sandbox.
    - **One limit**: the portal that started it is among the names it reaches,
      and a process that the portal starts for it without `--sandbox`
      (`flatpak-spawn` run from inside) runs with the app's own permissions: it
      listed the throwaway home's Documents folder. So WebKit's sandbox makes
      a compromised web process do one more thing (speak D-Bus to the Flatpak
      portal and ask for a process) before it has the app's reach, and keeps
      from it everything it can't do without that step. It is not a barrier
      against a process that does. This is the portal's behaviour, which
      Flatpak versions may change, so the check records the result (it does
      not assert it).
    _Amended 2026-10-19 (011 T078): checked against the code. The probe runs
    in a Flatpak (`sandbox_probe::SKIP_MARKERS` is `/run/.containerenv` and
    `/.dockerenv`), and the startup lines the check reads, whether WebKit's
    sandbox is on and what `Reach` found, are `log` lines that
    `src-tauri/src/logging.rs` writes to standard error (our crates at `info`,
    the others at `warn`; the helpers and the probe log nothing, the parent logs
    for them). Before 011 nothing installed a logger, so those lines went
    nowhere on any OS (§20)._
  - **With the sandbox off** (the probe failed, for example on a desktop
    without the Flatpak portal): a compromised web process has everything
    of §4's table: the home folder and removable drives, the user's
    unlocked keyring through the Secret Service, the logind and screen
    saver interfaces, and the display and GPU. It still has no network.
    That is what a `.deb` install's web process has where WebKit's own
    sandbox fails, apart from the network, which the `.deb` has.
  - **007 research.md §9** gets an amendment note pointing here (plan.md,
    Project Structure).
  _Amended 2026-10-19 (011 T034): the probe child and the app both need a
  default `GApplication` with the app id (`sandbox_probe::
  name_default_application`). WebKit names a sandboxed web process's
  accessibility bus name after it, or after a hash when there is none, and the
  Flatpak portal behind `flatpak-spawn --sandbox` refuses a name that doesn't
  start with the Flatpak's app id ("Invalid sandbox a11y own name"). Without
  it the probe aborted after 2 s (sandbox off) and, once it passed, the app
  itself died at its first web view. Tauri's `enableGTKAppId` would give tao's
  application the id, but tao then makes a second HoploDex on the same session
  bus exit at once, so a stand-in application is made instead. The probe takes
  about 470 ms in the sandbox, against 270 ms without WebKit's sandbox._
- **Rationale**: FR-011, as amended, and 007 FR-004: where the computer
  offers a sandbox and HoploDex finds it works, the preview runs in it.
- **Alternatives considered**: keeping it off in Flatpak (007's earlier
  assumption): the web process would keep the app's whole reach for no
  reason now that it isn't needed.

_Amended 2026-10-19 (011 T102): the session bus the sandbox's proxies reach in
the check now carries a real Secret Service. §9's sub-sandbox test lists
`org.freedesktop.secrets` among the names a process started with WebKit's flags
must not reach, and that stays true with the real daemon behind the name: the
sub-sandbox's proxy is refused it, as it was refused the stand-in. The session bus still has
no service activation of the container's own, so a stopped keyring leaves the
name without an owner and does not start another._

_Amended 2026-10-19 (owner's decision):_

- _**`name_default_application` runs only inside a Flatpak** (`/.flatpak-info`),
  in the probe child and in the app. Only the Flatpak portal refuses WebKit's
  hash-named accessibility bus name, so a `.deb` or `.rpm` with WebKit's
  sandbox on keeps 007's behaviour exactly. A unit test pins the choice._
- _**With the narrower defaults (§4).** A process the sub-sandbox asks of the
  Flatpak portal ("One limit" above) now gets §4's narrower reach. The Secret
  portal answers the sub-sandbox too: it keeps the app id, and `allow-dbus`
  gives it the portals, so a compromised web process can fetch HoploDex's key.
  It can't reach the keyring file, since `--sandbox` mounts no `~/.var/app`
  (read from flatpak 1.16's `flatpak-run.c`; the check's sub-sandbox listing
  confirms it). Through the one D-Bus call it gets both, and so HoploDex's
  saved passphrases (§20)._

## §10 The helpers (FR-012)

- **Decision**: no change. The import reader and the render helper
  (`--import-helper`, `--render-helper`) start from `/proc/self/exe` inside
  the sandbox and confine themselves as on any Linux. In the spike,
  Landlock (the host kernel's ABI), `PR_SET_PDEATHSIG`,
  `PR_SET_DUMPABLE` and the rlimits all worked inside the sandbox, so
  each helper is confined at least as tightly as on a `.deb` on the same
  computer, with the sandbox around it as well. `unshare(CLONE_NEWUSER)`
  is refused inside Flatpak, and nothing in `services/helper/` uses it.
  - **Dying with HoploDex**: `PR_SET_PDEATHSIG` ties each helper to the
    app's thread that started it, as on a `.deb`; inside the sandbox, bwrap
    runs with `--die-with-parent` as well.
  - **Shown by the automated check** (§16): each helper's confinement line
    in the log and the helper gone once the app is killed.
- **Rationale**: the spike shows nothing the helpers need is missing.

## §11 Settings, recent list, caches and the guide (FR-009, FR-010)

- **Decision**: no change to where anything is kept. Tauri's
  `app_config_dir` and `app_cache_dir` follow `XDG_CONFIG_HOME` and
  `XDG_CACHE_HOME`, which Flatpak points into
  `~/.var/app/io.github.exodious.HoploDex/`, so `machine.json` (with the
  recent list), the passphrase check's scratch files, the opened
  documents' copies and the web views' data folders are all in the
  Flatpak's own storage. A `.deb` and a Flatpak on the same computer keep
  separate recent lists and settings (spec, Edge Cases).
  - **Shown by**: the automated check can't show these locations, because
    its `e2e` build takes its config, cache and Documents folders from the
    `HOPLODEX_E2E_*` variables the check sets (§16). The shipped build
    shows them instead: `test:flatpak -- --fresh-install` starts it under a
    throwaway home and finds its config folder (with `machine.json`, which
    startup writes) and its web view data folder under
    `~/.var/app/io.github.exodious.HoploDex/`, and nothing under the home's
    own `.config` or `.cache` (FR-009); and the manual setup's first start
    finds the create dialog suggesting `~/Documents/HoploDex` (FR-010,
    quickstart.md "Setup").
  - **The guide**: the database guide's section on using a database on
    several computers (`DatabaseGuide.tsx`) gains a paragraph shown on Linux
    only, saying that a Flatpak and a `.deb` or `.rpm` of HoploDex on one
    computer keep separate recent lists and settings, that saved
    passphrases are shared when both reach the same keyring, and that the
    Flatpak reaches only the home folder and removable drives unless given
    more. The frontend already knows the platform
    (`browserControls.ts`'s `platformFrom`), so no command changes. The
    text says "If you have installed HoploDex as a Flatpak", so it reads
    correctly on any Linux install.
- **Alternatives considered**: showing it only inside Flatpak, which needs
  the backend to report the install kind for one paragraph.

## §12 Quit, shutdown and signals (FR-013)

- **Decision**: no change. Inside Flatpak the app is a child of bwrap and
  receives SIGTERM, SIGHUP and SIGINT itself: from systemd when the
  session ends, from `flatpak kill`, and from the terminal for a
  `flatpak run` in one. Its handlers close at once, keeping unsaved input
  as pending changes, and clear the decrypted copies, as on a `.deb`.
  Startup's sweep of leftover copies and unfinished backups reads the same
  directories.
- **Shown by**: the automated check's US2 journey (§16), which sends
  SIGTERM to the app inside the sandbox with unsaved input in an open form
  and an opened document's copy on disk, then finds the pending changes
  offered at the next open and the copies folder empty. `flatpak kill`,
  which the helpers' check uses, ends the sandbox without counting on the
  app's handlers, so it shows the helpers dying with the app, not this
  path. A real log-out is quickstart.md's manual M7.

## §13 What the bundle carries (FR-017, FR-018, FR-019)

- **Decision**: the module installs exactly these into `/app`:
  - `bin/hoplodex`: the program.
  - `share/applications/io.github.exodious.HoploDex.desktop`: name
    HoploDex, `Exec=hoplodex`, `Icon=io.github.exodious.HoploDex`,
    categories `Office;Database;`, and `StartupWMClass` set to the window's
    class so the desktop matches the window to the entry.
    _Amended 2026-10-19: the window's class is `hoplodex`, not the identifier.
    The `e2e` build's main window, read with Xlib's `XGetClassHint` under
    Xvfb in the dev container (no `xprop` there), has `WM_CLASS`
    `hoplodex`, `Hoplodex`: GTK takes it from the program name, because
    Tauri leaves the GTK application id unset (`enableGtkAppId` is off). So
    `StartupWMClass=hoplodex`. On Wayland the app id comes from the same
    program name (read from tao's and Tauri's source, not run: no Wayland
    session in the dev container), so the same value matches there._
  - `share/metainfo/io.github.exodious.HoploDex.metainfo.xml`: name,
    summary, a description that says what HoploDex is and that it keeps
    everything on the computer, `project_license` `GPL-3.0-only`,
    `metadata_license` `CC0-1.0`, the developer, the homepage, VCS and
    issue tracker addresses, `launchable`, the content rating (OARS 1.1,
    nothing to declare), branding colours from the design tokens, the
    releases with their notes, newest first, and screenshots. The newest
    release's version must equal `APP_VERSION` (as
    `tests/app_version_test.rs` keeps the others equal).
  - **Screenshots**: AppStream wants addresses, which a software centre
    fetches. They are images from the screenshot walk
    (`e2e/screenshots/screens.e2e.ts`), copied into `flatpak/screenshots/`
    and addressed at the release's tag on GitHub
    (`https://raw.githubusercontent.com/exodious/HoploDex/v<version>/flatpak/screenshots/…`).
    A test checks every address names a file in the repository. Before the
    first release is tagged, the addresses don't resolve yet, which the
    validator's offline mode doesn't check.
    _Amended 2026-10-19 (T106; constitution Licensing): no screenshot shows a
    photo that needs a credit line (`src-tauri/examples/seed-photos/README.md`:
    CC BY, OGL). The collection shot was the first one to, with the Benelli
    M4's photo (OGL v3.0) in its second row, and it showed "$100,048,799" as
    the replacement value, which is the seed's one firearm at the $99,999,999
    maximum (#67). It is retaken in the walk's own setting (the dev
    container's Xvfb, 1200 x 800, light theme) from a throwaway sandbox with
    the seed, where that firearm's estimated replacement value was edited to
    $2,500 before the shot (the total reads $51,300; the counts are
    unchanged), grouped by Type so the Handgun group opens the list and the
    shotgun, with the Benelli, is below it. The seed is unchanged. What each
    shot shows, with the photos' licences:
    - `chooser.png`: no photo.
    - `collection.png`: the thumbnails of the Beretta 92FS
      (`beretta-92fs-atf.jpg`, public domain, US Government), the Colt 1911
      (`1911a1-field-stripped.jpg`, public domain, PD-self) and the Glock 19
      (`glock-19-gen3.jpg`, public domain, PD-self); the others in view
      (FN, Ridgeline Arms) have the type's drawing, not a photo.
    - `record.png`: the Glock 19's photos, `glock-19-gen3.jpg` (public
      domain, PD-self; the large one), and in the strip `glock-19-gen4-fde.jpg`
      (CC0 1.0) and `glock-19-atf.jpg` (public domain, US Government).
    - `document-preview.png`: no photo (the Dead Air Sandman-K has none).
    - `accessories.png`: no photo from the seed set; the Leupold's is a
      generated gradient.
    A retake of any of them must keep to this list: a photo that needs a
    credit line stays out of view._
    _Amended 2026-10-19 (owner's decision): the store shots must be
    reproducible, changing only when someone retakes them on purpose, for
    example to show a new feature. A scripted command
    (`npm run screenshots:store`, in the dev container) takes the five from the
    seed in the walk's setting. The script makes the one value edit (the
    $99,999,999 firearm's estimated replacement value set to $2,500 in the
    throwaway sandbox) and the grouping, and no text in view may depend on the
    day it runs. Where a shot would show such text, the script changes the view,
    or the seed gets a record with fixed dates for it. It writes
    `flatpak/screenshots/`; a build never retakes them. A test checks the
    recorded list above (records, photos, licences) against the script's._
  - `share/icons/hicolor/<size>x<size>/apps/io.github.exodious.HoploDex.png`
    at 32, 64, 128, 256 and 512, copied from `src-tauri/icons/`, which
    `npm run icons` generates from `tools/icons/AppIcon.tsx`, so the program
    icon has one source.
  - `share/licenses/io.github.exodious.HoploDex/NOTICES.txt`: the build's
    `dist/NOTICES.txt` (010 FR-011), which carries the copyright notice and
    the GPL-3.0 text; the folder Flatpak and software centres use for an
    app's license files.
  - **Nothing else**. The bundle check (`scripts/flatpak/contents.mjs`)
    lists the built app's files and fails on any path not in this list,
    which covers FR-018: no runtime library, and no shared library at all,
    can be inside without failing. It also confirms, with `readelf`, that
    the binary's needed libraries are all ones the runtime has.
- **The audit** (FR-019): the Flatpak builds the same binary from the same
  lockfiles with the same features as the other Linux bundles, and adds no
  module, so its notices list the same components, and nothing ships only
  in the Flatpak. A test fails if the manifest gains a module or a source
  other than the checkout and the two lists, so that statement can't go
  stale without someone extending the audit. The SDK extensions (the Rust
  and Node toolchains) are used only to build, like the dev container's
  toolchains, and ship nothing.
- **Validators**: `desktop-file-validate`, `appstreamcli validate` (the
  validator Flathub uses), and Flathub's linter on the manifest and the
  built repository (§4) all run in the build command and fail it.
- **Alternatives considered**: Tauri's generated `.desktop` from the `.deb`
  (it names the binary's own icon and lacks the Flatpak id); an SVG icon
  (`tauri icon`'s PNGs are drawn per size, which an SVG would lose at the
  small sizes).

## §14 The `.flatpak` file, upgrades and the architecture (FR-020, FR-024, FR-025)

- **Decision**: the build command ends with
  `flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo`,
  making `flatpak/out/HoploDex-<version>-x86_64.flatpak`. That
  `--runtime-repo` is what makes installing the file on a computer without
  Flathub add Flathub and fetch the runtime from it (the spike). Nothing is
  attached to a release, and nothing goes to Flathub (FR-020, SC-009): the
  release process (#19) attaches the file, and its written process says how
  the file is made (plan.md, "Close-out").
  - **Upgrades** (FR-024): installing the next version's file over the
    previous one keeps `~/.var/app/io.github.exodious.HoploDex` (the
    spike), so settings, the recent list and the cache stay, and the keyring
    is outside the app anyway. `flatpak update` doesn't update an app
    installed from a file; the install instructions say so, and #19 puts
    them with the releases.
  - **x86-64 only** (FR-025): the build passes `--arch=x86_64`, and
    `flatpak/flathub.json` holds `{"only-arches": ["x86_64"]}` for a later
    submission.
  - **Review that nothing publishes** (011 US4-6, 011 SC-009; 011 T069,
    2026-10-19): a search of `scripts/flatpak/`, `package.json`'s scripts,
    `.github/workflows/` (`audit.yml` and `codeql.yml`, neither of which
    mentions Flatpak or a release), `flatpak/`, `e2e/support/flatpak.ts` and
    `e2e/specs/flatpak.e2e.ts` for `gh release`, `gh pr`, `gh api`,
    `flat-manager`, `--gpg-sign`, `--sign`, `git push`, `curl`, `wget`,
    `upload` and `publish` found none. `flathub` appears only as the runtime
    repository's address (`env.sh`'s `remote-add`, `build.sh`'s `--runtime-repo`
    and the installs of the runtime, the SDK, its extensions and the linter
    from it, and `check.sh --fresh-install`'s expectation that the bundle adds
    it), all downloads. The linter is run with `--exceptions --user-exceptions
    <file>`, which reads the committed file and makes no lookup of Flathub's
    exceptions (§4). The three commands write only under `flatpak/out/`, the
    state volume and the container's own installation. Nothing was attached to
    a release or sent to Flathub by T062-T069 (no `gh` command was run).
- **Alternatives considered**: a repository of our own on GitHub Pages,
  which would give `flatpak update`, but is a publishing channel the spec
  doesn't ask for and HoploDex would have to sign and keep.

## §15 Running it in the dev container (FR-014, FR-023)

- **Decision**: the Flatpak build and check run in the dev container, with a
  new `--flatpak` option for `scripts/dev-container.sh`.
  - **What `--flatpak` adds**: `--security-opt unmask=/proc/*`, which bwrap
    needs under rootless podman (the spike), and a per-checkout volume for
    `flatpak-builder`'s state directory (its downloads and build folders).
    The container's own Flatpak installation (the runtime, SDK, extensions
    and linter, about 2 GB, fetched from Flathub once) is its user
    installation in `/home/dev`, the volume every checkout already shares,
    so worktrees don't each fetch it. It is opt-in because it relaxes the
    container's protection of `/proc`, which nothing else needs.
  - **The image** gains `flatpak`, `flatpak-builder`, `appstream`,
    `desktop-file-utils`, `python3-dbusmock` (for the check's stand-in
    services, §16), the Python packages the two generators need (from
    Debian, or pinned with hashes, §21), the two generators at one pinned
    commit (§3), and Flathub's linter (below).
  - **A private system bus**: Debian's `flatpak` asks the system bus
    (parental controls) before it installs an app, and the container has no
    system bus. The scripts start a private one and point
    `DBUS_SYSTEM_BUS_ADDRESS` at it (the spike). The check uses the same bus
    for its stand-in logind (§16).
  - **The linter**: `flatpak-builder-lint` from Flathub's `org.flatpak.Builder`
    (installed into the container's own Flatpak installation, run with
    `flatpak run --command=flatpak-builder-lint org.flatpak.Builder`), the
    way Flathub runs it, with the appstream version it expects. It is a
    build-time tool built by Flathub and never ships.
  - **Commands** (contracts/flatpak-bundle.md): `npm run bundle:flatpak`
    builds, checks and writes the `.flatpak`; `npm run flatpak:sources`
    regenerates the lists; `npm run test:flatpak` runs the automated check.
    Each refuses to run outside the dev container unless told to, since on a
    host it would use the developer's own Flatpak installation (SC-007).
  - **Without podman**: DEVELOPMENT.md lists what a host needs and says to
    point `FLATPAK_USER_DIR` at a throwaway installation.
- **Alternatives considered**:
  - **The linter from source** (`pip install` from
    flathub-infra/flatpak-builder-lint at a pinned commit): avoids a
    Flathub-built tool, but needs its own appstream and pinning by hand.
    The owner can choose it instead at no cost to the plan.
  - **`flatpak-builder` from `org.flatpak.Builder` too**: nests a Flatpak
    sandbox in a Flatpak sandbox in a container; Debian's package worked in
    the spike.
  - **Always unmasking `/proc`**: weakens every test run's container for one
    feature.

## §16 The automated Flatpak check (FR-021, SC-002, SC-007)

- **Decision**: `npm run test:flatpak` builds a check variant of the
  Flatpak, installs it into the container's Flatpak installation, and runs
  one E2E file, `e2e/specs/flatpak.e2e.ts`, against it through WebDriver,
  then uninstalls it.
  - **The check variant**: the build script derives a second manifest from
    the shipped one, changing only the cargo features (`custom-protocol,e2e`
    instead of `custom-protocol`), the profile (`e2e`) and the binary's
    path that follows from it, and leaving out the `check-no-webdriver.mjs`
    step, since the `e2e` build carries the driver by design; not the
    `finish-args`. A test compares the two manifests and fails on any
    other difference, so the check runs with the permissions the Flatpak
    ships with. It is installed but never written to a `.flatpak` file.
  - **What the run adds at `flatpak run`**: `--share=network` (WebDriver's
    port, which the test runner outside the sandbox must reach; the
    shipped Flatpak has no network) and the `HOPLODEX_E2E_*` variables the
    `e2e` build needs. The variables point at the paths a shipped Flatpak
    uses: `HOPLODEX_E2E_CONFIG_HOME` at `~/.var/app/<id>/config`,
    `HOPLODEX_E2E_CACHE_HOME` at `~/.var/app/<id>/cache`, and
    `HOPLODEX_E2E_DOCUMENTS` at the documents folder `user-dirs.dirs`
    names, which the check reads from inside the sandbox
    (`flatpak run --command=xdg-user-dir … DOCUMENTS`). So the `e2e` build's
    seams change nothing the sandbox affects, except that the shipped
    build's own derivation of those folders isn't exercised: §11 says
    where that is shown.
  - **No network in the shipped sandbox**: since the journey runs with
    network added, the check shows the shipped permissions' network
    separately: `flatpak run --command=python3` (without
    `--share=network`) finds only the loopback interface and fails to
    connect to the container's own non-loopback address, where the check
    listens on a port first so that a connection would succeed outside the
    sandbox.
  - _Amended 2026-10-19 (011 T102): the check variant no longer swaps them.
    It is built with `flatpak-check` as well as `e2e` (the cargo features
    `custom-protocol,e2e,flatpak-check`), which keeps the WebDriver server and
    the `HOPLODEX_E2E_*` seams but uses `AppOpener` and the OS keyring. The
    owner's decision of 2026-10-19: the check talks to a scripted OpenURI and a
    headless `gnome-keyring-daemon` on its private bus, never to anything
    the developer has. The paragraph below, and the "Alternatives" entry on a
    build with the real keyring and opener, are what 011 first decided._
  - **What the `e2e` build swaps**, by design: the keyring (an in-memory
    store) and the opener (a log). Both exist so that no test can reach a
    real keyring or start a real app (constitution II). What the sandbox
    changes about each is its bus access, so the check shows that with the
    shipped permissions: `flatpak run --command=gdbus` calls the stand-in
    Secret Service and the OpenURI portal's name from inside the sandbox,
    and is refused for a name the manifest doesn't grant. The portal call
    itself is tested in Rust (§8), and the keyring's Secret Service
    backend is the `.deb`'s, used in human testing in the same container.
  - **The stand-in services**: `python-dbusmock` on the private system bus
    as logind (its `logind` template, with `Inhibit`), and on the check's
    private session bus as `org.gnome.ScreenSaver` and
    `org.freedesktop.secrets`. The check sends `PrepareForSleep(true)`,
    `PrepareForShutdown(true)` and `ActiveChanged(true)` and confirms the
    database locks or closes, and that the inhibitor's descriptor is closed
    once it has (the `Inhibit` stand-in keeps the other end). Running
    without the ScreenSaver stand-ins shows the setting as unavailable
    (US2-4).
  - **The journey** (US5-2): create in the suggested location, close with a
    backup beside it, reopen from the recent list, change the passphrase,
    restore, import and export a spreadsheet, a file dropped onto a record
    (the native drop technique the E2E suite already uses), a document
    opened in another application (through the opener log), a saved
    passphrase reopening without typing, a database on a second mount under
    `/mnt` standing in for a removable drive, and a typed location outside
    reach refused with nothing written. The opened document's copy is on
    disk in the copies folder while the database is open and gone once it
    closes (FR-008, 001 FR-035). While the Flatpak has a database open, a
    process outside the sandbox can't take the file's lock (the exclusive
    lock holds across the sandbox, spec Edge Cases). It also checks the
    helpers' confinement lines and that both are gone after the app is
    killed (§10), WebKit's sandbox line (§9), and SIGTERM closing at once
    with pending changes kept and the copies cleared (§12).
  - **Not in the check**: sleep while a backup is being written (003's
    tests show the stop; the sandbox changes neither the notice nor the
    backup's code) and the search and action budgets (§18).
  - **Isolation** (SC-007): the whole check runs in the dev container,
    whose home, keyring, buses and Flatpak installation are the
    container's, never the developer's. The scripts refuse to run on a host
    that isn't the dev container unless `FLATPAK_USER_DIR` names a
    throwaway installation and `--i-know-this-is-a-host` is given.
  - **When it runs**: before each release, and for a pull request that
    changes any of the paths DEVELOPMENT.md lists (FR-021): `flatpak/`,
    `scripts/flatpak/`, `e2e/specs/flatpak.e2e.ts`, `e2e/support/flatpak.ts`,
    `src-tauri/src/platform/linux.rs`, `src-tauri/src/main.rs` (reach
    detection and the opener's route), `src-tauri/src/app_dirs.rs` (the
    config, cache and Documents folders), `src-tauri/src/session/lifecycle.rs`
    (the reaction to the OS notices, the close's backup and the signal
    path), and, under `src-tauri/src/services/`, `sandbox.rs`, `keyring.rs`,
    `file_swap.rs`, `backups.rs`, `scratch.rs`, `helper/`,
    `import_reader/`, `preview/helper.rs`, `preview/sandbox_probe.rs`;
    under `src-tauri/src/commands/`, `databases.rs`, `backups.rs` and
    `import_export.rs` (the reach checks), `documents.rs` and `photos.rs`
    (opening and drops); and `src/features/media/useFileDrop.ts` (drops).
    Ordinary test runs skip it.
- **Rationale**: a WebDriver journey is the only way to drive the real app
  in its sandbox. Swapping the two seams the `e2e` build already has keeps
  constitution II's guarantee, and the bus checks cover what the sandbox
  changes about them.
- **Alternatives considered**:
  - **A WebDriver build with the real keyring and opener**: a build whose
    WebDriver server can save to a real keyring is one mistake away from
    the developer's own; the bus checks give the same evidence about the
    sandbox without it.
  - **Driving the shipped build by real input only** (`x11-input.py`):
    possible, but there is nothing to read the app's state from without
    WebDriver.
  - **Waiting for #36**: the spec asks for the notices now; #36 can reuse
    the stand-ins.

_Amended 2026-10-19 (owner's decision): the check follows the narrower
defaults (§4, §5, §7):_

- _**Files in reach.** The throwaway home's `Documents/HoploDex` is the only
  home folder in reach, so the journey's own files (the scans, the export,
  "Protected scans") move inside it. A drop from `~/Pictures` becomes the
  out-of-reach drop (refused, with the message naming the file chooser), and a
  drop from inside reach is attached._
- _**The stand-in drive** moves from `/mnt/hoplodex-check` to a mount under
  `/run/media` (`dev-container.sh --flatpak`). A typed location under `/mnt`
  joins `/srv` as refused._
- _**The Secret portal** is added to the stand-in
  `org.freedesktop.portal.Desktop` (`openuri_mock.py`, which already serves the
  OpenURI): `version` 1, and `RetrieveSecret` writing a fixed per-run key to
  the descriptor. The app's real portal store (oo7's file keyring) runs against
  it. The saved-passphrase, reopen and install-over (US4-3) journeys look for
  the entry in the sandbox's `keyrings/default.keyring`, read with that key,
  not with `secret-tool`._
- _**The bus check.** The headless gnome-keyring-daemon stays on the private
  bus, so the check can show that the shipped permissions are refused
  `org.freedesktop.secrets` while a real daemon owns it (SC-011)._
- _**US2-5** becomes a run whose stand-in portal has no Secret interface
  (saving unavailable)._
- _**The sub-sandbox listing's expectations** change with §9's note._

## §17 Manual procedures (FR-022)

- **Decision**: quickstart.md's manual part, which #37's release procedures
  take over, holds the steps the check can't take, on a real desktop with
  the `.flatpak` installed: a real sleep held back until the database locks,
  a real screen lock, a drop from the desktop's own file manager, a file
  picked through the real file chooser (a Documents-portal path), a
  document opened in its real application, a saved passphrase in the
  desktop's own keyring, and the out-of-reach message for a folder outside
  the home folder. Each step names the database ("Main collection" or
  "Shared collection", from `scripts/human-testing.sh`) and its passphrase.

_Amended 2026-10-19 (owner's decision): quickstart.md's manual steps follow.
M3's drop from the file manager is made from `~/Pictures` (refused, the
message naming the file chooser) and from `~/Documents/HoploDex` (attached).
M5 saves a passphrase through the desktop's Secret portal, then checks in the
desktop's keyring manager that the only new item is the "Application key for
io.github.exodious.HoploDex" and no `passphrase:` entry. M6's folder outside
reach is one in the home folder outside `Documents/HoploDex`._

## §18 Performance (SC-006)

- **Decision**: no new budget work. Search and actions run the same code as
  on a `.deb`, on the same file: the home folder and removable drives are
  bind mounts with no layer between the app and the file, and a database
  is never opened through the Documents portal's FUSE folder (a portal
  path is turned back into its host path or refused, §5). So the release
  `performance_test` outside the sandbox measures what the Flatpak does. Cold start adds `bwrap` and the D-Bus proxy (tens of
  milliseconds) and, now, WebKit's sandbox probe (§9), which a `.deb`
  already pays where it isn't in a container (007 research.md §22). The
  automated check times the start to the chooser for the Flatpak and for
  the `e2e` build outside it in the same container, and the pull request
  records both against SC-006's one second. A difference over a second
  fails the check.
  _Amended 2026-10-19 (011 T034): the container skips the probe, so the
  outside figure adds one run of the probe, forced on as the app does, to
  model a `.deb` on a desktop. Unfair to neither: the first run of the check
  measured 2190 ms against 395 ms with the probe failing slowly (see §9's
  amendment), and 863 ms against 392 ms without the probe added._

## §19 What a Flathub submission would still need (FR-026)

Read from [Flathub's requirements](https://docs.flathub.org/docs/for-app-authors/requirements)
and its linter's documentation on 2026-10-18. This feature keeps the build
within Flathub's conventions (the app id, `flathub.json`, the metadata, the
permissions, the linter), and leaves these to a later decision:

1. **A manifest written without AI-generated or AI-assisted content.**
   Flathub's rule since 2026-09-21: "Flathub manifests must not contain
   AI-generated or AI-assisted content. Disclosure does not exempt
   manifests from this restriction." This feature's manifest is
   AI-assisted, so a submission needs one a person writes.
2. **Disclosure of the AI-generated material in the app**, and acceptance of
   reviewers' discretion over its extent.
3. **Evidence of a meaningful development history and of use.**
4. **A submission opened and answered by a person**: agents may not open
   or automate submissions.
5. **The home-folder exception**: Flathub's linter policy says it "will not
   be granted if there are signs of LLM usage in the software", so the
   `--filesystem=home` grant (§4) may have to narrow, with the reach
   design of §5 adjusted.
6. **A `type: git` source** pinned to the release tag and commit, in place
   of the checkout (§2).
7. **Screenshots** hosted where Flathub mirrors them, which the tag-based
   addresses of §13 already allow.

_Amended 2026-10-19 (011 T078): checked against what was built._

- _Item 1: the manifest's own header says it is AI-assisted, which is why it
  can't be submitted as it is._
- _Item 5 is two exceptions, not one: `finish-args-login1-system-talk-name`
  (§4, §6) is recorded beside `finish-args-home-filesystem-access`, and a
  submission asks Flathub for both. If the logind name is refused, the sleep
  delay (§6) has no other route (§4, "Alternatives considered")._
- _Item 7: the repository lint in `build.sh` sets aside
  `appstream-missing-screenshots` and
  `appstream-screenshots-not-mirrored-in-ostree` because a local build can't
  mirror the tag-based addresses (§4). Flathub's pipeline does mirror them
  (`--mirror-screenshots-url`), so a submission needs neither._

## §20 Residual risks for the release security review (#21)

- **A compromised web process, with WebKit's sandbox off** (§9): reaches the
  home folder, removable drives, the unlocked keyring and the logind and
  screen-saver interfaces. With it on: only what WebKit's sub-sandbox shares
  for rendering. Neither has the network.
  _Amended 2026-10-19 (011 T060): with it on, the arguments are in §9:
  nothing of the home or the removable drives, no network, no system bus and no
  keyring or screen saver on the session bus, but the display, the audio
  and GPU sockets WebKit asks for, and the desktop portals, the Flatpak portal
  among them. A process asked of the Flatpak portal from inside runs with the
  app's permissions (§9, "One limit"), so with the sandbox on a compromised web
  process is one D-Bus call from the "off" reach above, and not by default
  at it. The release review should count WebKit's sandbox here as a layer an
  attacker has to get through, not a wall._
- **A process asked of the Flatpak portal, from inside WebKit's sub-sandbox.**
  _Amended 2026-10-19 (011 T078):_ `flatpak-spawn` run without `--sandbox` from
  inside the sub-sandbox starts a process with the app's own permissions: all of
  §4's table, the home folder, removable drives, keyring and logind among
  it. In the check it listed the throwaway home's Documents folder (§9, "One
  limit"). The sub-sandbox can ask because the Flatpak portal is among the
  names every Flatpak sandbox reaches on the session bus. So WebKit's sandbox,
  with it on, does not keep a compromised web process from the app's reach; it
  adds the one D-Bus call that asks for a process. Judge it as a layer, not a
  wall. This is the portal's behaviour and may change with Flatpak versions, so
  the check records it and does not assert it.
- **A compromised HoploDex process**: everything in §4's table, which is
  what a `.deb` install has apart from the network and files outside the
  home folder and removable drives.
  _Amended 2026-10-19 (owner's decision): with the narrower defaults (§4), a
  compromised HoploDex process, or one the sub-sandbox has the Flatpak portal
  start, reaches `<Documents>/HoploDex`, the drives under `/media` and
  `/run/media`, HoploDex's own saved passphrases (§7), logind, the screen
  savers and the display. It no longer reaches the rest of the home folder or
  any other keyring item, and so can no longer leave the sandbox by writing a
  startup file. The databases in reach stay encrypted; the saved passphrases
  open those whose passphrase is saved. A user's `flatpak override` widens this
  by what they chose. The keyring bullet below now applies to a `.deb` only._
- **The keyring**: no narrower than on a `.deb` (§7).
- **The Documents portal** remembers the opened copy's path and name after
  the copy is deleted (§8).
- **Overrides the user makes.** The refusal's advice (§5) is a
  `flatpak override --user --filesystem=<folder>` command, and Reach honours
  whatever the user grants, `host` included. That widens what a compromised
  HoploDex process reaches by exactly what the user chose, as it would for any
  Flatpak; nothing in HoploDex limits it.
- **The app's log reaches standard error.** _Amended 2026-10-19 (011 T078):_
  `logging.rs` is new in 011. The `log` lines the app already had (about 190, of
  which about a third name a path) now go to the terminal HoploDex was started
  from, or to the journal for one started from the desktop, on every OS, not
  only in a Flatpak. A line can name a database, backup or document file's path
  and the operating system's error text. The lines that mention a passphrase,
  key or secret were read and none prints one; the rest were not reviewed line
  by line for collection data. The journal is local and the user's own; the
  release review should decide whether paths belong there.
  _Amended 2026-10-19: every line was then read for collection data. The owner
  accepts database, backup and app-file paths; nothing from inside a database
  is logged (no record values, no original photo or document file name, nor
  its source path or a copy named after it, no draft text), and the lines that
  could were changed (`logging.rs`, `logging::folder_of`, `logging::json_error`).
  A log a user can send, at a level they choose, is #128._
- **The runtime** is Flathub's, updated by Flathub, and ends with GNOME 51
  (§1). A WebKitGTK security fix reaches users when Flathub updates the
  runtime, without a HoploDex release.
- **The `.flatpak` file is unsigned**, as the other bundles are until #19
  decides signing. Installing it from anywhere but the release page is the
  user's risk.

## §21 Dependencies and tools

- **Shipped**: no new crate or npm package. The OpenURI and Documents
  portal calls use `zbus`, already a Linux dependency for the OS notices.
  The runtime's libraries are the runtime's (010 FR-017).
  _Amended 2026-10-19 (owner's decision): `oo7` 0.6 and its new crates
  ship on Linux, for the Flatpak's saved passphrases ([§7](#7-saved-passphrases-fr-006))._
- **Build and test only**, all in the dev container: the GNOME 51 SDK and
  the two SDK extensions, `flatpak`, `flatpak-builder`, the two
  flatpak-builder-tools generators, Flathub's linter (§15), `appstream`,
  `desktop-file-utils` and `python3-dbusmock`. None of them ships, so the
  license audit doesn't cover them (constitution, Licensing:
  "Development-only dependencies that are not distributed are exempt").
  None of them collects data.
- **Keeping the build tools current and free of known vulnerabilities**:
  as the image's other tools are (`CARGO_DENY_VERSION`,
  `CARGO_NEXTEST_VERSION` and the like), each is pinned in the
  `Dockerfile`: flatpak-builder-tools by its commit
  (`FLATPAK_BUILDER_TOOLS_COMMIT`), the rest as Debian packages. The
  generators' Python packages come from Debian; any that Debian doesn't
  have is installed from a committed `scripts/flatpak/requirements.txt`
  with exact versions and `--require-hashes`, which GitHub's dependency
  graph reads, so Dependabot's security alerts cover it as they cover the
  npm and Cargo dependencies, and it is updated by hand the same way
  (DEVELOPMENT.md, "Dependency audit"). The pinned commit and the SDK
  branches are reviewed with the release process's checks (#19). None of
  them touches collection data: the generators read only the lockfiles,
  and the linter only the manifest and the built repository.
- **Prebuilt by others, for the build only**: the GNOME SDK, the toolchain
  extensions and, if kept, `org.flatpak.Builder`'s linter, all from Flathub.
  The runtime the Flatpak runs on is Flathub's by the spec's own choice. The
  owner's preference against third-party-built binaries
  (plan.md, "Decisions for the owner") applies to what ships; the linter's
  alternative is in §15.
