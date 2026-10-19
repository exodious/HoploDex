# Implementation Plan: Build and Distribute a Flatpak

**Branch**: `011-flatpak` | **Date**: 2026-10-19 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/011-flatpak/spec.md`

## Summary

HoploDex gets a Flatpak, the self-contained Linux bundle 010 FR-017 chose in
place of the AppImage. It is built from the checkout by `flatpak-builder`,
with no network during the build, on Flathub's GNOME 51 runtime (the last
with WebKitGTK 4.1, supported until about September 2027; research.md §1).
A `.flatpak` file that gets its runtime from Flathub is the output; nothing
is published or sent to Flathub (FR-020).

Most of the app already works in the sandbox as it is. The changes:

- **Reach** (research.md §5): a new `services/sandbox.rs` reads
  `/.flatpak-info` and says whether a location is within reach. Every
  command that names a location asks it before writing anything, and refuses
  with a new `LOCATION_OUT_OF_REACH` (or `BACKUP_LOCATION_UNAVAILABLE`'s new
  reason `outOfReach`) that says how to give access. Documents-portal paths
  from the file chooser are turned back into host paths where they can be.
  Outside Flatpak it reports everything within reach, so no other install
  changes.
- **Opening documents** (§8): inside Flatpak, through the OpenURI portal's
  `OpenFile`, reading its answer, since the runtime's `xdg-open` reports
  success before it knows.
- **The logind session** (§6): always `session/auto`, since the sandbox's own
  PIDs make `GetSessionByPID` wrong.
- **WebKit's sandbox** (§9): the Flatpak runs 007's probe instead of
  skipping it, so web processes get WebKit's own sub-sandbox.
- **The guide** (§11): one paragraph for Linux about separate installs.

Around them: the manifest with its recorded permissions (§4), the offline
source lists with a check on every test run (§3), the desktop entry,
AppStream metadata, icons and notices with a contents allow list (§13),
validators and Flathub's linter as gates, dev container support (§15), an
automated check of the real Flatpak in its sandbox (§16) and the manual steps
the check can't take (§17).

## Technical Context

**Language/Version**: Rust 1.97+ (edition 2024) for the backend; TypeScript
5 / React 18 for the frontend; Bash and Node 24 (ESM `.mjs`) for the build
and check scripts; YAML (the manifest), XML (AppStream), INI (desktop entry).

**Primary Dependencies**: no new shipped dependency. `zbus` 5 (already a
Linux dependency) makes the OpenURI and Documents portal calls. Build and
test only, in the dev container: `org.gnome.Sdk//51`,
`org.freedesktop.Sdk.Extension.rust-stable` and `node24` on the 26.08
branch, `flatpak`, `flatpak-builder`, flatpak-builder-tools' cargo and npm
generators (pinned commit), Flathub's `flatpak-builder-lint`
(`org.flatpak.Builder`), `appstream`, `desktop-file-utils`,
`python3-dbusmock` (research.md §21).

**Storage**: no change to the database, its format or `machine.json`. In
the Flatpak, the config and cache directories live under
`~/.var/app/io.github.exodious.HoploDex/` (research.md §11). New committed
files: the manifest, two generated source lists, the desktop entry, the
AppStream metadata, `flathub.json`, the linter exceptions and the metadata's
screenshots, all under `flatpak/`.

**Testing**: Rust integration tests (`cargo nextest --features test-kdf`) for
reach and the refusals on real files and databases, and for the portal
opener against a test portal on a private bus; Vitest for the UI's messages
and for the script checks (source lists, manifest, contents allow list,
metadata, the host guard); the automated Flatpak check (`npm run
test:flatpak`, a WebDriver journey against the installed Flatpak in the dev
container, with stand-in logind, screen saver and Secret Service); manual
steps before a release (quickstart.md).

**Target Platform**: Linux x86-64, as a Flatpak on any distribution with
Flatpak and access to Flathub. Nothing changes on macOS, Windows or a
non-Flatpak Linux install.

**Project Type**: desktop app (Tauri 2), plus build tooling.

**Performance Goals**: the constitution's budgets hold unchanged (same code);
cold start within 1 s of the same computer's non-Flatpak start (SC-006,
research.md §18). `/.flatpak-info` is read once at startup; a reach check is
a few path comparisons, plus one D-Bus round trip for a Documents-portal
path.

**Constraints**: no network during the build (SC-004), none in the shipped
sandbox (research.md §4); no runtime library in the bundle (FR-018); every
permission recorded (FR-002); Flathub's linter passes with recorded
exceptions (FR-026); nothing published (FR-020); the check never touches the
developer's data or Flatpak installation (SC-007).

**Scale/Scope**: about 560 crates and 820 npm packages in the source lists;
one backend module, five command areas and three platform files touched;
four frontend components; about ten new scripts and checks.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Lint/static analysis, review, small single-purpose modules, no speculative abstraction | **One place decides reach** (`services/sandbox.rs`), read from the file Flatpak writes, not a second list in the code; the commands ask it, they don't each judge paths. **One opener per platform**, the Flatpak's chosen at construction rather than a `_flatpak` sibling. `GetSessionByPID` removed rather than worked around. **One manifest**: the check's is derived from it, and a test confines the difference to the features. Cargo directly rather than the Tauri CLI, as the E2E build already does. `eslint`, `prettier`, `clippy`, `rustfmt`, the citations check |
| II. Testing (NON-NEGOTIABLE) | Tests first, real persistence, one test per acceptance scenario, isolation | Every scenario maps to a test (Test Levels). Each refusal has a red Rust test on real files that also shows nothing was written (SC-003). The source-list, manifest and contents checks each have red fixtures. The automated check runs the real Flatpak in its sandbox, in the dev container, whose home, keyring, buses and Flatpak installation are its own; its scripts refuse a host (SC-007). The `e2e` build's keyring and opener doubles stay, and what the sandbox changes about each (its bus access) is shown with the shipped permissions (research.md §16) |
| III. UX Consistency | One component set and pattern, WCAG 2.1 AA | No new screen or dialog. The refusal arrives through the existing mechanisms: a field error in the create dialog, the chooser's open-failure line, Database settings' "Not available on this computer", the close's backup notice, the import, export and attach errors. One wording for all of them (contracts/tauri-commands.md). The guide's paragraph uses the guide's existing section pattern |
| IV. Performance | 100 ms feedback / 1 s completion, 500 ms search, no UI-thread blocking | Same code paths; reach is read once. The check measures cold start against the non-Flatpak build in the same container, and the pull request records it (research.md §18) |
| V. User Privacy | Local only, no telemetry, nothing leaves without consent | The Flatpak has no network at all (research.md §4), stronger than a `.deb`. Opening a document keeps 007's native consent; the portal hands only that copy to the chosen app. Nothing is sent to Flathub or attached to a release |
| Security & Data Handling | Encryption at rest, vetted dependencies, no cipher change | No cipher, key, database format or keyring entry changes. No new shipped dependency. Each permission is recorded with what a compromised process could reach (research.md §4), and the residual risks are listed for #21 (§20). WebKit's sandbox comes on inside Flatpak (§9). The persistence PR note comes from research.md §5 and §20 |
| Licensing | GPLv3-compatible shipped components; release carries notices | The bundle carries `NOTICES.txt` (010 FR-011) and no runtime library (010 FR-017), enforced by an allow list of the bundle's files. Same binary, same lockfiles, so the audit already covers what it ships; a test fails if the manifest gains a module (research.md §13). Build tools ship nothing (§21) |

**Result**: PASS, with the entries in Complexity Tracking.

## Test Levels

*Every acceptance scenario needs a test (constitution II), but not an E2E
one. Each is placed at the cheapest level that can show it. The automated
Flatpak check is this feature's E2E: it is the only way to run the real app
in the real sandbox. It is a new E2E file, `e2e/specs/flatpak.e2e.ts`,
because its setup (a Flatpak build, installation and stand-in buses) is
shared with no other file, and it is left out of `npm run test:e2e` and run
by `npm run test:flatpak` instead (DEVELOPMENT.md, "E2E files").*

- **E2E, one journey per user story**: Stories 1 and 2 are each a
  `describe("011 US<n> - …")` in `flatpak.e2e.ts`. Stories 3 and 4 are the
  maintainer's: their journey is the build command itself, whose checks fail
  it. Story 5 is the check.
- **E2E, error mechanisms**: the refusal reaches the UI through existing
  mechanisms that already have their E2E tests in the main suite (a field
  error in the create dialog, a refusal's message). One check test shows the
  real refusal end to end (a typed location under `/srv`); Vitest shows each
  place's wording.
- **Security**: reach and the refusals are tested where they are enforced
  (Rust, on real files), and in the real sandbox by the check. The bus
  checks run with the shipped permissions. Neither moves to a cheaper level.

| Story | E2E journey (`flatpak.e2e.ts`) | Other checks (real app, security, build) | Scenarios tested in Rust or Vitest |
|-------|-------------|-------------------------------------------------|------------------------------------|
| US1 | Start, chooser shown; create in the suggested Documents folder; close (backup beside it); reopen from the recent list; change passphrase; restore; import and export; attach by drop; preview a PDF, a TIFF, an image and a text file; open a document (opener log; its copy on disk, then gone after the close); a process outside the sandbox can't take the open database's lock; save the passphrase and reopen without it; the same on a database under `/mnt`; a typed location under `/srv` refused with nothing written | US1-1's name and icon: `desktop-file-validate`, `appstreamcli validate` and the contents check in the build. US1-2's folder: the check reads `xdg-user-dir DOCUMENTS` inside the sandbox and compares | US1-8 → Rust `tests/sandbox_reach_test.rs` (each command and need: create, open, a recent entry, backup location, export, import, attach, drop; refused before writing, no file left; a Documents-portal path turned back into a host path, or used, or refused per need), and `services/sandbox.rs` unit tests (parsing `/.flatpak-info`: grants, overrides, `:ro`, `!`, unknown grants). US1-5's outside-reach drop → Rust. US1-7 → Rust `tests/portal_opener_test.rs` (responses 0, 1, 2, no portal, the time-out). US1-8's wording in each place → Vitest (`CreateDatabaseDialog`, `DatabaseChooser`, `DatabaseSettingsDialog`, the close notice) |
| US2 | With a database open: logind's `PrepareForSleep(true)` locks it and the inhibitor's descriptor is released after; the screen saver's `ActiveChanged(true)` locks it; `PrepareForShutdown(true)` closes it keeping pending changes; a log-out's SIGTERM closes it at once, keeping pending changes and clearing the copies; started without the screen-saver stand-ins, the setting shows unavailable; an import and a TIFF preview each log their helper's confinement, and both helpers are gone once the app is killed; the log shows WebKit's sandbox on | Bus checks with the shipped permissions (`flatpak run --command=gdbus`): the Secret Service, both screen savers, logind and the OpenURI portal answer; a name not granted is refused. With only the shipped permissions, the sandbox has no network beyond loopback (the journey itself adds network for WebDriver). US2-5: with no Secret Service on the bus, the name has no owner | US2-5's unavailable keyring → the existing 003 tests (`Keyring::off()`). The `session/auto` change → Rust unit test of the path the listener uses. The probe's decision with `/.flatpak-info` present → `sandbox_probe` unit tests. US2-7 → research.md §9, §20 (read by the release review, #21) |
| US3 | The build command from a clean checkout: download, then build with the network off, then the checks | The contents allow list, `check-no-webdriver.mjs`, the validators and the linter, all failing the build | US3-2 → Vitest `scripts/flatpak/sources.test.ts` (the real lockfiles and lists, and red fixtures: a version bump, a checksum change, an extra source, a `git` source). US3-3 → Vitest `contents.test.ts` (red fixtures: a `.so`, a stray file, a missing icon). US3-5 → Vitest `manifest.test.ts` (one module, its three sources, no network build argument, the finish-args list, the check manifest's difference). US3-6 → DEVELOPMENT.md, followed in quickstart.md |
| US4 | — | US4-2: `npm run test:flatpak -- --fresh-install` installs the `.flatpak` into an empty installation with no remotes and starts it (needs the network; before each release), and finds the shipped build's config and web view data under `~/.var/app` (FR-009, which the `e2e` build's seams can't show; FR-010's suggested folder is the manual setup's step 7). US4-3: the check installs a second build over the first and finds settings, the recent list and the saved passphrase kept, and the database opening | US4-4 → Vitest `metainfo.test.ts` (license, source address, description, screenshot files exist, newest release equals `APP_VERSION`). US4-5 → the linter in the build. US4-6 → no command in the repository publishes (review; SC-009) |
| US5 | The check itself | US5-3: the scripts' host guard → Vitest `guard.test.ts` (refuses outside the dev container without a throwaway `FLATPAK_USER_DIR` and the explicit flag) | US5-1 → DEVELOPMENT.md's path list; US5-4 → quickstart.md's manual steps |

## Project Structure

### Documentation (this feature)

```text
specs/011-flatpak/
├── spec.md              # amended at planning (Clarifications 2026-10-18 and 2026-10-19)
├── plan.md              # this file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1: validation, automated and manual
├── contracts/
│   ├── tauri-commands.md    # the out-of-reach refusals, command by command
│   └── flatpak-bundle.md    # the manifest, permissions, bundle contents, commands
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

Also amended: `specs/007-document-preview/research.md` §9 (and §1's
residual-risk line) gets an `_Amended by spec 011_` note: WebKit's sandbox
does run inside Flatpak, and the Flatpak turns it on where the probe works.

### Source Code (repository root)

```text
flatpak/                                      # new
├── io.github.exodious.HoploDex.yml           # the manifest (research.md §2, §4)
├── io.github.exodious.HoploDex.desktop       # §13
├── io.github.exodious.HoploDex.metainfo.xml  # §13
├── flathub.json                              # {"only-arches": ["x86_64"]} (§14)
├── linter-exceptions.json                    # the two recorded exceptions (§4)
├── cargo-sources.json                        # generated (§3)
├── node-sources.json                         # generated (§3)
├── screenshots/                              # the metainfo's images (§13)
└── out/                                      # the built .flatpak (git-ignored)

scripts/
├── flatpak/                                  # new
│   ├── build.sh                # npm run bundle:flatpak: download, build offline, check, bundle
│   ├── sources.sh              # npm run flatpak:sources: both generators
│   ├── check.sh                # npm run test:flatpak: check build, install, buses, E2E, uninstall
│   ├── env.sh                  # the private system bus, the installation, the host guard
│   ├── guard.mjs               # the host guard's rules (tested)
│   ├── contents.mjs            # the bundle's allow list and its readelf check
│   ├── checkManifest.mjs       # derives the check variant's manifest
│   ├── sources.test.ts         # lockfiles ↔ source lists (runs in npm test)
│   ├── manifest.test.ts        # modules, sources, finish-args, the variant
│   ├── contents.test.ts
│   ├── metainfo.test.ts
│   └── guard.test.ts
├── dev-container.sh            # --flatpak (research.md §15)
└── build-appimage.sh           # unchanged

Dockerfile                      # flatpak, flatpak-builder, appstream, desktop-file-utils,
                                # python3-dbusmock, the generators (research.md §15)
package.json                    # bundle:flatpak, flatpak:sources, test:flatpak
.gitignore                      # flatpak/out/

src-tauri/src/
├── services/sandbox.rs         # new: Reach, read from /.flatpak-info; Documents-portal host paths
├── services/mod.rs             # pub mod sandbox
├── commands/error.rs           # CommandError::out_of_reach (LOCATION_OUT_OF_REACH)
├── commands/databases.rs       # create, open, a recent entry; backup location (outOfReach)
├── commands/backups.rs         # list_backups' and settings' availability (outOfReach)
├── commands/import_export.rs   # export destination, import files
├── commands/photos.rs          # add_photo_from_path and drops
├── commands/documents.rs       # add_document_from_path; the portal Opener (research.md §8)
├── session/lifecycle.rs        # the close's backup: failureReason outOfReach
├── platform/linux.rs           # session/auto (research.md §6)
├── services/preview/sandbox_probe.rs   # /.flatpak-info no longer skips the probe (§9)
└── main.rs                     # read Reach once and manage it; choose the opener

src-tauri/tests/
├── sandbox_reach_test.rs       # new (US1-8, US1-5, SC-003)
└── portal_opener_test.rs       # new (US1-7), Linux only, private bus

src/features/databases/
├── DatabaseGuide.tsx           # the Linux paragraph (research.md §11)
├── DatabaseChooser.tsx         # close notice: outOfReach
├── DatabaseSettingsDialog.tsx  # backup location: outOfReach
├── CreateDatabaseDialog.tsx    # nothing new: the refusal is a field error
└── types.ts                    # failureReason, availability reason

e2e/
├── specs/flatpak.e2e.ts        # new; excluded from test:e2e
├── support/flatpak.ts          # new: launch through flatpak run, the stand-in buses
├── support/app.ts              # launchApp takes the Flatpak launcher
└── wdio.conf.ts                # exclude flatpak.e2e.ts unless HOPLODEX_E2E_FLATPAK

DEVELOPMENT.md                  # "Flatpak" under Build; the check under Test; the PR path list
CLAUDE.md                       # points to them; the new error code; services/sandbox.rs
```

**Structure Decision**: the existing Tauri layout. The Flatpak's own files
get a top-level `flatpak/`, as Flathub's layout expects a manifest beside its
metadata, and its scripts go under `scripts/flatpak/`, beside
`scripts/licenses/`.

### Close-out (at pull request time)

The final phase of tasks.md, done as part of opening the pull request. Check
each issue's state first (`gh issue view <n> --json state`), and don't
comment on a closed one unless the owner asks. All five were open on
2026-10-19.

- **#42**: the pull request closes it ("Fixes #42"). Comment with what was
  settled against the issue's list: the reach (home and removable drives,
  overrides honoured, the refusal), the keyring, the notices' bus names and
  `session/auto`, drops (host paths only; outside reach refused), config and
  data locations and the guide's note, the copies and the portal opener,
  WebKit's sandbox now on, the runtime (GNOME 51 and its end of life, with a
  link to the new issue below), the source build and its lists, the
  metadata, the command, the check, and the manual steps. Left: a Flathub
  submission (research.md §19).
- **New issue**: "Linux self-contained bundle after GNOME 51's end of life
  (September 2027)", labelled `release`, linking #42 and this spec. Its body
  is research.md §1's options, the Tauri issues to watch, and the decision
  it needs before about June 2027. Opened before #42's comment so the
  comment can link it.
- **#19** (release process): comment with what the written process takes
  from this feature: build the `.flatpak` with `npm run bundle:flatpak` in
  the dev container and attach `flatpak/out/HoploDex-<version>-x86_64.flatpak`;
  run `npm run test:flatpak` and its `--fresh-install` before the release;
  the install instructions (one command, the software centre, and that
  `flatpak update` doesn't update a copy installed from a file); check the
  SDK extensions' branches are stable; the follow-up issue's deadline.

  _Amended 2026-10-19 (011 T109): the comment also says that each release
  updates the metainfo's `<release>` date and notes (the newest release's
  version must equal `APP_VERSION`, research.md §13); that the `v<version>`
  tag must carry `flatpak/screenshots/`, since the metainfo's screenshot
  addresses are at that tag; and that the release reviews the pinned
  flatpak-builder-tools commit (`FLATPAK_BUILDER_TOOLS_COMMIT` in the
  `Dockerfile`, research.md §21) and Flathub's linter (`org.flatpak.Builder`,
  installed unpinned by `scripts/flatpak/build.sh`)._
- **#21** (release security review): comment with research.md §4's
  permission table, §9's web-process reach with WebKit's sandbox on and off,
  and §20's residual risks, for the review's attack surface.
- **#36** (OS event checks): comment that `scripts/flatpak/env.sh` and
  `e2e/support/flatpak.ts` start a stand-in logind and screen saver with
  `python-dbusmock` and send their signals, which the general check can
  reuse outside Flatpak.
- **#37** (manual procedures): comment with quickstart.md's manual Flatpak
  steps (M1–M7), for the release procedures.

## Complexity Tracking

| Addition | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A reach module consulted by five command areas | FR-004 requires the check before anything is written, with the fix | Turning the I/O error into the message after the fact can't tell "out of reach" from "missing", and comes after scratch files exist (research.md §5) |
| A second opener on Linux (the OpenURI portal) | FR-008's "no application" failure inside Flatpak | `xdg-open` in the sandbox exits 0 before it knows (research.md §8) |
| A check variant of the manifest | WebDriver needs the `e2e` build | A shipped build has no driver; a separate hand-kept manifest would drift, so it is derived and its difference tested (§16) |
| `--flatpak` in `dev-container.sh` (unmasking `/proc`) | bwrap under rootless podman | Always unmasking weakens every run's container (§15) |
| Python generators and the linter in the image | Offline source lists and FR-026 | Writing an npm generator ourselves (research.md §3) |

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (research.md, data-model.md, contracts/,
quickstart.md).*

- **Code Quality**: reach has one home and one input (`/.flatpak-info`); the
  three needs are an enum the commands name, not three checks
  (data-model.md). The portal opener replaces `xdg-open` only where it is
  wrong. The check manifest is derived. Still PASS.
- **Testing**: every scenario has a named test at a stated level; every
  check script has red fixtures; the manual steps are numbered procedures
  naming the database and passphrase (quickstart.md). Still PASS.
- **UX Consistency**: one refusal wording through existing mechanisms; no
  new screen, so the screenshot walk is unchanged (the guide's new paragraph
  is in the existing guide shot). Still PASS.
- **Performance**: unchanged paths; cold start measured. Still PASS.
- **User Privacy**: no network in the sandbox; nothing published. Still
  PASS.
- **Security**: permissions recorded with their reach; residual risks
  listed; WebKit's sandbox on. Still PASS.
- **Licensing**: the bundle's allow list enforces "no runtime library";
  notices carried. Still PASS.

**Result**: PASS. The design adds no violations beyond those justified above.

## Decisions for the owner

Settled at planning, each reversible before tasks start:

1. **GNOME 51**, with a follow-up issue for its end of life: the owner's
   direction of 2026-10-19 (spec, Clarifications).
2. **Drops outside reach are refused**: the owner's decision of 2026-10-18,
   now in the spec (FR-007).
3. **WebKit's sandbox on inside Flatpak** (research.md §9), following the
   spike: a security gain with the existing probe as its fallback.
4. **A database picked outside reach isn't opened** (spec, Clarifications
   2026-10-19), since opening writes beside it.
5. **Flathub's linter from `org.flatpak.Builder`**, a Flathub-built tool
   used only to lint (research.md §15). The alternative is building it from
   source at a pinned commit; say so and the plan changes there only.
6. **The check keeps the `e2e` build's keyring and opener doubles**, and
   shows the sandbox's effect on them by bus checks with the shipped
   permissions (research.md §16), rather than a WebDriver build that could
   reach a real keyring.
