# Quickstart: validating feature 011

How to show that the feature works. Commands run through the dev container
(CLAUDE.md, "Run tests in the dev container"); the Flatpak ones need its
`--flatpak` option (contracts/flatpak-bundle.md). DEVELOPMENT.md's "Flatpak"
section has the full forms once it exists.

## Automated

```sh
# Every test run: the source lists, manifest, contents, metadata and guard checks (Vitest),
# and the reach and refusal tests (Rust)
scripts/dev-container.sh npm test
scripts/dev-container.sh bash -c 'cargo nextest run --manifest-path src-tauri/Cargo.toml --features test-kdf -E "binary(sandbox_reach_test) | binary(portal_opener_test) | test(/sandbox/) | test(/sandbox_probe/)"'

# The build, with its checks and validators; writes flatpak/out/HoploDex-<version>-x86_64.flatpak
scripts/dev-container.sh --flatpak npm run bundle:flatpak

# The automated Flatpak check (Stories 1, 2 and 5), and before a release the fresh install (Story 4)
scripts/dev-container.sh --flatpak bash -c 'npm run build && npm run test:flatpak'
scripts/dev-container.sh --flatpak npm run test:flatpak -- --fresh-install
```

The first `bundle:flatpak` in a container fetches the GNOME 51 runtime and
SDK, the two SDK extensions and `org.flatpak.Builder` from Flathub (about
2 GB, into the shared `/home/dev` volume), then every crate and npm package
the lists name. Give it a generous timeout, or run it in the background with
its output in a log (CLAUDE.md). Later runs fetch only what changed.

Expected:

- `npm test` and the Rust run pass.
- `bundle:flatpak` ends with the `.flatpak` path. Its log shows the download
  phase, then the build phase with `--disable-download`, then
  `contents: ok`, `check-no-webdriver: ok`, `desktop-file-validate: ok`,
  `appstreamcli validate: ok`, `notices: same as the other Linux builds` and
  the linter passing, on the manifest with the two recorded exceptions
  (`flatpak-builder-lint manifest: ok`) and on the repository with those two
  and the two screenshot rules `build.sh` adds (`flatpak-builder-lint repo: ok`).
  _Amended 2026-10-19 (011 T078): the notices line and the repository lint's
  screenshot rules were added during implementation (research.md §4)._
- `test:flatpak` passes every test in `e2e/specs/flatpak.e2e.ts`, the bus
  checks and the no-network check, prints the cold-start times of the Flatpak and the non-Flatpak
  build (SC-006), and leaves no app installed (`flatpak list --app` in the
  container is empty afterwards).
- `--fresh-install` reports that the empty installation gained the
  `flathub` remote and the GNOME 51 runtime, that HoploDex started, and
  that its config and web view data folders are under
  `~/.var/app/io.github.exodious.HoploDex/` with nothing in the home's own
  `.config` or `.cache`.

### Red checks

Run once each while implementing, to see each check fail for the right
reason, then undo the change:

1. Bump a crate's version in `src-tauri/Cargo.lock` by hand: `npm test`
   fails in `sources.test.ts`, naming `flatpak/cargo-sources.json`, the
   crate and `npm run flatpak:sources`.
2. Add `--share=network` to the manifest's `finish-args`: `manifest.test.ts`
   fails, naming the permission.
3. Add a line installing any `.so` file into `/app/lib` to the module:
   `bundle:flatpak` fails at `contents`, naming the file.
4. Set the metainfo's newest release to another version: `metainfo.test.ts`
   fails, naming `APP_VERSION`.
5. Run `npm run test:flatpak` on the host (not in the container):
   it refuses before doing anything, naming the dev container and
   `FLATPAK_USER_DIR`.

### Where each scenario and criterion is tested

| Scenario or criterion | Test |
|---|---|
| US1-1, US1-2, US1-3, US1-4, US1-6 | `flatpak.e2e.ts`, "011 US1" journey; US1-2's folder in the shipped build: manual "Setup" step 7 |
| US1-5 | `flatpak.e2e.ts` (a drop within reach); `sandbox_reach_test.rs` (a drop outside it) |
| US1-7 | `flatpak.e2e.ts` (opener log, the copy on disk and gone after the close); `portal_opener_test.rs` (the portal's answers) |
| US1-8, SC-003 | `sandbox_reach_test.rs` (every command, nothing written); `flatpak.e2e.ts` (a typed `/srv` location); Vitest for each place's wording |
| US2-1, US2-2, US2-3, US2-4, US2-6, SC-002 | `flatpak.e2e.ts`, "011 US2" journey, with the stand-in logind and screen saver, and SIGTERM for a log-out; a real log-out: manual M7 |
| FR-009 | `test:flatpak -- --fresh-install` (the shipped build's folders); manual "Setup" step 7 |
| US2-5 | the existing 003 keyring-unavailable tests; the bus check (no Secret Service, no owner) |
| US2-7 | research.md §9, §20 (read by the release review, #21) |
| US3-1, US3-3, US3-4, SC-004, SC-005 | `bundle:flatpak` and its checks; `contents.test.ts` |
| US3-2 | `sources.test.ts` |
| US3-5 | `manifest.test.ts` (one module, no other sources) |
| US3-6 | DEVELOPMENT.md's "Flatpak", followed by this file |
| US4-2, SC-010 | `test:flatpak -- --fresh-install` |
| US4-3, SC-008 | `flatpak.e2e.ts`, the install-over step |
| US4-4, US4-5 | `metainfo.test.ts`; `appstreamcli validate` and the linter in `bundle:flatpak` |
| US4-1, US4-6, SC-009 | #19's written process; no command here publishes (review) |
| US5-1 | DEVELOPMENT.md's path list for when the check is required |
| US5-2, US5-3, SC-007 | `test:flatpak` itself; `guard.test.ts` |
| US5-4 | the manual steps below |
| SC-001 | the US1 journey |
| SC-006 | the check's cold-start times; the pull request's performance note |

## Manual (best effort before a release, #37)

The steps the automated check can't take: a real sleep, a real screen lock,
the desktop's own file manager, file chooser, keyring and document viewer,
and a real log-out. They run on a real Linux desktop (GNOME), in a local
user account kept for testing, never the account that holds your own
HoploDex data, so the Flatpak's installation, `~/.var/app`, Documents
folder and keyring are all that account's.

### Setup

1. On the Linux test computer, create a local user account for testing, if
   there isn't one, named `hoplodex-test` here, and log in to its GNOME
   desktop. Every step below runs in that account unless it says the dev
   container.
2. In the dev container, in your own account's checkout, build the bundle:
   `scripts/dev-container.sh --flatpak npm run bundle:flatpak`.
3. In the dev container, seed the two test databases:
   `scripts/dev-container.sh scripts/human-testing.sh --reset --seed-only`.
4. Copy `flatpak/out/HoploDex-<version>-x86_64.flatpak` from your checkout
   into `hoplodex-test`'s home folder, and copy
   `.human-testing/HoploDex/Main collection.hoplodex` and
   `.human-testing/HoploDex/Shared collection.hoplodex` into
   `hoplodex-test`'s `~/Documents/HoploDex/` (create the folder). Make
   `hoplodex-test` their owner.
5. As `hoplodex-test`, in a terminal, install the bundle:
   `flatpak install --user ~/HoploDex-<version>-x86_64.flatpak`. Accept
   adding Flathub and the GNOME 51 runtime.
6. Start HoploDex from the desktop's application list. Expected: its own
   name and icon in the list and the window, and the database chooser with
   an empty recent list.
7. Click **Create…**. Expected: the suggested location is
   `/home/hoplodex-test/Documents/HoploDex`, not a folder under
   `~/.var/app`. Cancel. In a terminal, run
   `ls ~/.var/app/io.github.exodious.HoploDex/config/io.github.exodious.HoploDex/ ~/.config/io.github.exodious.HoploDex`.
   Expected: `machine.json` in the first, and the second doesn't exist.
8. Click **Open…**, and in the file chooser pick
   `~/Documents/HoploDex/Main collection.hoplodex`. Enter the passphrase
   `human testing passphrase`. Expected: it opens, and once closed, the
   recent list names it at `/home/hoplodex-test/Documents/HoploDex/Main collection.hoplodex`,
   not a path under `/run/user`.
9. Do the same for `~/Documents/HoploDex/Shared collection.hoplodex`,
   passphrase `human testing passphrase`. It says it was left open on
   "Workshop PC": take it over, resolve its pending changes with
   **Discard**, and close it.

### M1. A real sleep is held back until the database locks

1. Open "Main collection" (passphrase `human testing passphrase`).
2. In a terminal, run `systemd-inhibit --list`. Expected: a line for
   HoploDex, "Locking the open database", `sleep`, `delay`.
3. Suspend the computer from the system menu, wait ten seconds, and wake it.
4. Unlock the desktop. Expected: HoploDex shows "Main collection" locked,
   asking for its passphrase.
5. Enter `human testing passphrase`. Expected: it unlocks where it was.

### M2. A real screen lock locks the database

1. With "Main collection" open, open **Database settings** and make sure
   **Lock when the screen locks** is on and available (not marked
   unavailable).
2. Press Super+L, wait five seconds, and unlock the desktop.
3. Expected: "Main collection" is locked, asking for
   `human testing passphrase`.

### M3. Drops from the desktop's file manager

1. Copy any JPEG image to `~/Pictures/drop-test.jpg`, and another to
   `/tmp/outside.jpg`.
2. Open "Main collection" (passphrase `human testing passphrase`) and open
   any firearm record.
3. From Files, drag `~/Pictures/drop-test.jpg` onto the record's photos.
   Expected: it is attached.
4. From Files (Other Locations > Computer > tmp), drag `/tmp/outside.jpg`
   onto the photos. Expected: it is refused with a message that names
   `/tmp/outside.jpg`, says this copy of HoploDex is a Flatpak that can only
   reach the home folder and removable drives, and gives the
   `flatpak override` command. Nothing is attached.

### M4. Opening a document in its real application

1. Open "Main collection" (passphrase `human testing passphrase`) and a
   record that has a PDF document.
2. Choose to open the PDF in another application, and confirm the native
   question. Expected: the desktop's PDF viewer opens it.
3. Close the PDF viewer, then close the database from HoploDex.
4. In a terminal, run
   `ls ~/.var/app/io.github.exodious.HoploDex/cache/io.github.exodious.HoploDex/`.
   Expected: no opened document's copy remains.

### M5. A saved passphrase in the desktop's keyring

1. Open "Main collection" (passphrase `human testing passphrase`), and in
   **Database settings** turn on remembering the passphrase on this
   computer.
2. Quit HoploDex, start it again, and open "Main collection" from the recent
   list. Expected: it opens without asking.
3. Open Passwords and Keys (Seahorse). Expected: a login keyring item for
   `io.github.exodious.HoploDex`, user `passphrase:` followed by the
   database's id.
4. Back in **Database settings**, forget the saved passphrase. Expected:
   the item is gone from Seahorse.

### M6. A folder outside reach, and the override that fixes it

1. In a terminal: `sudo mkdir -p /srv/hoplodex-test` and
   `sudo chown hoplodex-test: /srv/hoplodex-test`.
2. Open "Main collection" (passphrase `human testing passphrase`). Open
   **Database settings**. Expected: its backup location,
   `/workspace/.human-testing/Backups` (where the seed made it), is shown as
   out of reach for this copy of HoploDex, with the override command.
3. Set the backup location to a custom folder and type
   `/srv/hoplodex-test`. Expected: refused, naming `/srv/hoplodex-test` and
   giving `flatpak override --user --filesystem=/srv/hoplodex-test io.github.exodious.HoploDex`.
4. Quit HoploDex. Run that command. Start HoploDex, open "Main collection"
   (passphrase `human testing passphrase`), and set the backup location to
   `/srv/hoplodex-test` again. Expected: accepted.
5. Close the database. Expected: a backup appears in `/srv/hoplodex-test`.
6. Clean up: `flatpak override --user --reset io.github.exodious.HoploDex`
   and `sudo rm -r /srv/hoplodex-test`.

### M7. A real log-out keeps unsaved changes

1. Open "Main collection" (passphrase `human testing passphrase`), open any
   firearm for editing, and type `logout test` into its notes without
   saving.
2. Log out of the desktop from the system menu, and log back in as
   `hoplodex-test`.
3. Start HoploDex and open "Main collection" (passphrase
   `human testing passphrase`). Expected: it offers the pending changes,
   and resuming them shows `logout test` in the notes.

### Afterwards

As `hoplodex-test`: `flatpak uninstall --user --delete-data io.github.exodious.HoploDex`.
