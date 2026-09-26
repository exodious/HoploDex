# Implementation Plan: Database Protection, Portability & Management

**Branch**: `003-database-protection-management` | **Date**: 2026-09-25 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/003-database-protection-management/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

This feature replaces the silent, keyring-held random key with a passphrase
the user chooses for each database, so that a database file plus its
passphrase opens on any supported computer. On top of that it lets the user
keep several databases wherever they like; backs each one up automatically,
at most once a day, when it is closed after changes; changes a passphrase by
copy, verify and replace; optionally remembers a passphrase in this computer's
keyring; and locks the application (by command, after inactivity, at sleep,
or optionally at screen lock) by closing the database completely, keeping
any unsaved form input as pending changes inside the encrypted file.

Technically, the passphrase becomes SQLCipher's key, with every cipher setting
pinned and the key-derivation work factor raised to 1,000,000 iterations
(336 ms to open, measured). Each database carries three new tables: its
backup and lock settings (collection data), its housekeeping state (identity,
open marker, backup record), and at most one set of pending changes.
Triggers on every collection table record "changes waiting" in the same
transaction as the change. Housekeeping stays trigger-free, so opening and
closing never make a backup due.

Copies rest on SQLCipher behaviour confirmed by a spike. Backups and restores
are chunked byte copies (exact progress, stoppable between chunks), stamped
by attaching them without a key, since they share the main file's salt. A
passphrase change uses `sqlcipher_export` into a newly keyed attachment,
stoppable with SQLite's interrupt. The backend therefore never holds a
passphrase after the command that needed it. Replacement is hard link plus
atomic rename, then secure deletion of the old contents.

`DbHandle` becomes a session that may hold no database. Every command goes
through `read`/`write` helpers, which refuse when closed, when pending
changes are unresolved, or, for writes, when the file's fingerprint shows
another computer replaced it. Exclusive locking mode tells "in use here" apart
from "wrong passphrase".

The idle clock runs in the backend on wall time. Per-OS listeners (logind
over `zbus`, IOKit and distributed notifications, a hidden Win32 top-level
window) deliver sleep, wake, screen-lock and shutdown notices, backed by a
wall-versus-monotonic watchdog. The frontend mirrors any dirty form draft to
backend memory, so a lock at sleep or shutdown never waits on the webview.

The frontend gains a database chooser, create, open and take-over flows, a
database menu, settings, change-passphrase and restore dialogs, a closing
screen with backup progress, the pending-changes prompt, a save / discard /
cancel prompt on close and quit, a shared `PassphraseField` with a lazily
loaded zxcvbn strength hint, and an in-app security guide.

## Technical Context

**Language/Version**: Rust 1.75+ (backend, `src-tauri`), TypeScript 5.x /
React 18 (frontend, `src`). Unchanged.

**Primary Dependencies**: Existing: Tauri 2, `rusqlite` 0.40
(`bundled-sqlcipher`, SQLCipher 4.14.0), `keyring` 4.1 (+ `keyring-core`
mock), `tauri-plugin-dialog`, `chrono`, `getrandom`, Radix. **New Rust**:
`unicode-normalization` (NFC, research §1), `gethostname` (open marker name,
§6), `fs4` (free space, §4). **Promoted from transitive to direct** (already
in `Cargo.lock`): `zeroize`, `zbus` (Linux notices), `same-file` (file
identity), `objc2`/`objc2-foundation`/`objc2-app-kit` (macOS),
`windows-sys` 0.61 with power, session and window features (Windows).
**New npm**: `@zxcvbn-ts/core`, `@zxcvbn-ts/language-common`,
`@zxcvbn-ts/language-en` (strength hint, lazy-loaded, §18),
`@radix-ui/react-dropdown-menu` (database menu). All are MIT or
MIT/Apache-2.0, have no network access and no telemetry, and must pass
`npm run audit` including `audit:licenses`.

**Storage**: One SQLCipher file per database, `<name>.hoplodex`, anywhere
the user chooses (default suggestion `<Documents>/HoploDex/`). New tables
`collection_settings`, `app_state` and `pending_changes`, plus
change-tracking triggers, added to `0001_initial.sql` in place (CLAUDE.md).
Existing development databases are incompatible (raw key, old schema) and
are not converted (spec Assumptions); the user is told, and their files are
not touched. Backups are `.hoplodex` files in `HoploDex backups/` next to
the database, or in a chosen folder. Machine-local state is in
`machine.json` in the app config directory. Saved passphrases are keyring
entries `passphrase:<database_id>`. Formats:
[contracts/database-file.md](./contracts/database-file.md) and
[data-model.md](./data-model.md).

**Testing**: `cargo test` integration tests calling `ops`/`session` functions
against real temp SQLCipher files, created with the production cipher
settings and a fixed test passphrase (no mocks, research §20), including
simulated dates, interrupted copies and a committed portability fixture.
The keyring tests run under `--features mock-keyring`. Vitest + RTL for every
new dialog, the chooser, the session provider and the hooks. WebdriverIO E2E
(`us7-databases`, `us8-backups`, `us9-locking`), one spec at a time, in
scratch `XDG_*` directories, with a scratch `user-dirs.dirs` so even the
suggested location stays inside the sandbox (§19). Manual per-OS checks for
the sleep, screen-lock and shutdown notices, which cannot be driven in the
container ([quickstart.md](./quickstart.md#platform-checks-manual-each-os-before-merge)).

**Target Platform**: Desktop: Windows 10+, macOS 12+, Linux (GNOME and KDE
fully; other desktops get sleep through logind, and screen lock where logind
or the ScreenSaver interface is present). Unchanged from 001.

**Project Type**: Desktop application (Tauri: React frontend + Rust backend
in one repo).

**Performance Goals**: Open ≤ 2 s at 10,000 firearms including key
derivation (SC-003; 0.34 s derivation measured). Progress shown within
100 ms for any backup, passphrase change or restore estimated over 1 s
(SC-005; estimate at a conservative 50 MiB/s against about 190 MiB/s
measured). Idle lock between 10:00 and 10:01 after the last input
(SC-010's 10:05 bound). Principle IV budgets for search (500 ms) and actions
(1 s) unchanged: the only new per-command cost is one `stat` for the
take-over check on writes.

**Constraints**: Fully offline, with nothing sent anywhere. The backend
never holds a passphrase between commands (FR-007). A refused open never
modifies the file (FR-006, FR-014). The original is untouched until a
verified copy replaces it in one rename (FR-015). No backup at sleep or OS
shutdown (FR-027, FR-037). The sleep lock stops work and hides data first,
under a delay of about 2 s (Windows) to 30 s (macOS). The pre-feature database and
keyring entry must never be read, changed or deleted (CLAUDE.md).

**Scale/Scope**: Six user stories (P1–P6), 39 functional requirements, 10
success criteria. 23 new IPC commands, 8 events and 17 error codes. 3
new tables, 21 triggers, 1 machine-local file, per-OS listeners on 3
platforms. About 12 new frontend components and hooks, 3 new E2E specs, 11
new screenshot screens.

No NEEDS CLARIFICATION remain: each open technical question is resolved in
[research.md](./research.md) (§1–§21).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Lint and static analysis; review; small single-purpose modules; complexity justified by a current requirement | New concerns each get one module: `db::cipher` (pinned settings), `session` (open database, read/write guards, lock procedure, idle clock, operations), `platform::system_events` (per-OS notices behind one enum), `services::{backups, file_swap, keyring, machine_settings, passphrase}`. Existing `ops` signatures are unchanged. Every new dependency answers a named requirement (Technical Context); three are new downloads, the rest are already in the lockfile. `PassphraseField` and `Menu` join the shared components because the passphrase field is used in five dialogs, which clears the "third occurrence" bar. `clippy`, `rustfmt`, `eslint` and `prettier` run locally; CI stays disabled by the owner's choice (a documented deviation) |
| II. Testing (NON-NEGOTIABLE) | Tests first; real persistence; a test per acceptance scenario; regression tests | Every scenario maps to a test in [quickstart.md](./quickstart.md), against real SQLCipher files with the production format. The spike's findings (copy shares salt, export skips triggers, interrupt works, BUSY before NOTADB, page-1 probe) become permanent tests, so a SQLCipher upgrade that changes them fails loudly. A guard test forces change-tracking triggers onto any future table. The only paths not automated are the OS sleep, screen-lock and shutdown notices themselves; their handlers are tested by calling the same entry points, and the notices are checked by hand per OS |
| III. UX Consistency | One component set; one confirmation pattern; WCAG 2.1 AA | Take-over, delete all backups, restore and discarding pending changes use the destructive `ConfirmDialog`. Save / discard / cancel extends `ConfirmDialog` with a third action instead of a one-off prompt. The settings dialog uses the existing `hd-form-grid`/`hd-field--quarter`/`--third` classes. All progress uses `ProgressBar`. Menu roles and focus rules are in [contracts/ui-databases.md](./contracts/ui-databases.md) §0 and §14. New screens join the screenshot walk (§15) |
| IV. Performance | 100 ms feedback / 1 s completion; no UI-thread blocking; progress on long work | All new commands are async, and long ones emit progress. The closing screen appears within 100 ms. The 1 s rule for backups is SC-005's. Open time is budgeted and measured (SC-003). Import and export gain a per-row cancel check (an atomic load). `performance_test.rs` gains open and progress timing |
| V. User Privacy | Local only; encryption at rest; clear disclosure of what goes where; real deletion | Nothing leaves the device. Backups are encrypted copies in a folder the user sees and chooses, disclosed at creation and in settings (FR-024, FR-029). Deleted records remaining in older backups is disclosed, with delete-all and secure rotation. The keyring holds only a passphrase, only on opt-in. `machine.json` holds paths and names only |
| Security & Data Handling | Platform-standard encryption; keys never logged or sent; network sync or backup opt-in and off by default; vetted dependencies | SQLCipher with pinned standard settings, and no custom cryptography (§1, §1a). Passphrases are zeroized and never logged, and SQLCipher's log is silenced in release. **Local backups are on by default**, which the spec's clarification reads as allowed because they never leave the device. The constitution's wording must be clarified by a PATCH amendment first (see Complexity Tracking). Dependencies are reviewed above |

**Result**: PASS with one recorded deviation, pending the PATCH amendment
(Complexity Tracking). Phase 0 may proceed.

## Project Structure

### Documentation (this feature)

```text
specs/003-database-protection-management/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output: §1–§21, including the SQLCipher spike findings
├── data-model.md        # Phase 1 output: tables, machine.json, keyring entry, session states
├── quickstart.md        # Phase 1 output: story → tests map, walkthroughs, per-OS checks
├── contracts/           # Phase 1 output
│   ├── tauri-commands.md     # deltas to 001's IPC contract: new commands, events, codes
│   ├── database-file.md      # the portable file format promise (FR-011)
│   └── ui-databases.md       # chooser, dialogs, menu, closing screen, guide, wording
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

Feature 001's documents are amended by pointer, as 002 did. The final task
adds a one-line "amended by 003" note at each anchor listed in the spec's
[Relationship to Feature 001](./spec.md#relationship-to-feature-001):
research.md §5, quickstart's first-run flow, FR-021, FR-035, SC-005, the
Assumptions, plan.md's Constitution rows and Project Structure note, and the
IPC contract's introduction and error shape.

### Source Code (repository root)

The existing Tauri layout from features 001 and 002. Only files that change or are
added are listed.

```text
src-tauri/
├── Cargo.toml                      # deps above; windows-sys/objc2/zbus target-specific
├── src/
│   ├── main.rs                     # setup: Session, Operations, IdleClock, MachineSettings,
│   │                               #  system-events listener, startup sweeps; CloseRequested/
│   │                               #  ExitRequested → app:quit-requested; signals → shutdown path;
│   │                               #  new commands in generate_handler!
│   ├── db/
│   │   ├── mod.rs                  # create_database / open_database / verify_passphrase,
│   │   │                           #  open-outcome classification, newer-version check;
│   │   │                           #  raw-key + keyring code removed
│   │   ├── cipher.rs               # NEW: pinned CipherSettings, apply_cipher_settings()
│   │   └── migrations/0001_initial.sql  # + collection_settings, app_state, pending_changes, triggers
│   ├── session/                    # NEW
│   │   ├── mod.rs                  # Session (Mutex<Option<OpenDatabase>>), read()/write() guards
│   │   ├── lifecycle.rs            # open/create/close/lock/sleep/shutdown procedures (ordered steps)
│   │   ├── fingerprint.rs          # take-over detection (research §6)
│   │   ├── idle.rs                 # wall-clock idle clock, pause reasons, 1 s tick
│   │   ├── operations.rs           # long-running operation registry, cancel flag, interrupt
│   │   └── pending.rs              # staged draft, pending_changes read/write/resolve
│   ├── platform/                   # NEW
│   │   ├── mod.rs                  # SystemEvent enum, listener spawn, wake watchdog
│   │   ├── linux.rs                # logind PrepareForSleep/PrepareForShutdown + delay inhibitors,
│   │   │                           #  session Lock/LockedHint, ScreenSaver ActiveChanged (zbus)
│   │   ├── macos.rs                # IOKit power, com.apple.screenIsLocked, will-power-off
│   │   └── windows.rs              # hidden top-level window: power, WTS session, end session
│   ├── commands/
│   │   ├── databases.rs            # NEW: chooser, create/open/close/lock/quit, settings, keyring,
│   │   │                           #  pending changes, activity (thin; logic in session/services)
│   │   ├── backups.rs              # NEW: list/restore/delete-all/change_passphrase (+ progress)
│   │   ├── error.rs                # + details field, new codes, SQLITE_CORRUPT → DATABASE_DAMAGED
│   │   ├── firearms.rs, insurance.rs, photos.rs, documents.rs
│   │   │                           # state.0.lock() → session.read()/write(); documents' cache
│   │   │                           #  cleared on every close (FR-022)
│   │   └── import_export.rs        # + cancel check between rows, OPERATION_STOPPED
│   ├── models/
│   │   ├── database.rs             # NEW: CollectionSettings, DatabaseStatus, PendingSummary,
│   │   │                           #  Draft, BackupInfo, CloseOutcome + validate_*_input
│   │   └── mod.rs
│   └── services/
│       ├── passphrase.rs           # NEW: NFC, length/NUL validation, Zeroizing wrapper
│       ├── backups.rs              # NEW: due rule, naming, listing, rotation, chunked copy + stamp
│       ├── file_swap.rs            # NEW: hard-link + rename replace, fallback, recovery sweep
│       ├── keyring.rs              # NEW: saved passphrases (real store / mock feature), probe
│       ├── machine_settings.rs     # NEW: machine.json (id, recent list, notices), atomic writes
│       ├── disk_space.rs           # NEW: free-space check (fs4)
│       └── secure_delete.rs        # + discard hint, 1 MiB chunks, progress for large files
├── examples/
│   ├── human_seed.rs               # creates the databases via db::create_database with a printed
│   │                               #  passphrase; seeds non-default settings, a pending change, two
│   │                               #  backups, and a second database marked open on another computer;
│   │                               #  writes machine.json's recent list into the scratch config dir
│   └── portable_fixture.rs         # NEW: regenerates tests/fixtures/portable-v1.hoplodex
└── tests/
    ├── support/mod.rs              # TestDb via db::create_database + TEST_PASSPHRASE; helpers to
    │                               #  simulate dates, interrupts and another machine's marker
    ├── fixtures/portable-v1.hoplodex   # NEW (research §20)
    ├── passphrase_protection_test.rs   # NEW (US1)
    ├── database_open_test.rs       # NEW (US2: outcomes, newer version, in use, recent list)
    ├── take_over_test.rs           # NEW (FR-032: marker, take-over, fingerprint)
    ├── portability_test.rs         # NEW (SC-001, SC-002)
    ├── machine_settings_test.rs    # NEW
    ├── backup_test.rs              # NEW (US3, FR-023–FR-027, FR-029)
    ├── backup_due_tracking_test.rs # NEW (FR-025 housekeeping vs changes, trigger guard)
    ├── restore_test.rs             # NEW (FR-028)
    ├── passphrase_change_test.rs   # NEW (US4, SC-004)
    ├── file_swap_test.rs           # NEW
    ├── keyring_test.rs             # NEW (US5; --features mock-keyring)
    ├── lock_test.rs                # NEW (US6, FR-033–FR-038, SC-010)
    ├── pending_changes_test.rs     # NEW (FR-039)
    ├── import_export_test.rs       # + stop between rows keeps imported rows
    ├── performance_test.rs         # + open ≤ 2 s at 10,000; progress within 100 ms
    ├── human_seed_coverage_test.rs # + new tables; backup-only columns checked in a seeded backup
    └── (every other test file)     # unchanged logic; TestDb now passphrase-keyed

src/
├── App.tsx                         # SessionProvider → DatabaseChooser | (CollectionProvider + AppShell),
│                                   #  keyed by session so closing unmounts all collection state
├── services/tauriClient.ts         # CommandFailure.details; listen() helper for events
├── components/
│   ├── PassphraseField.tsx         # NEW (uncontrolled, show/hide, strength, clears on system event)
│   ├── StrengthHint.tsx            # NEW (lazy zxcvbn)
│   ├── Menu.tsx                    # NEW (Radix dropdown menu)
│   ├── ConfirmDialog.tsx           # + alternative action (save / discard / cancel)
│   └── index.ts
├── features/
│   ├── session/                    # NEW
│   │   ├── SessionProvider.tsx     # chooser/open/closing state; session:closed, app:quit-requested
│   │   ├── sessionService.ts       # typed wrappers for the session commands
│   │   ├── ClosingScreen.tsx       # backup progress + skip
│   │   ├── PendingChangesDialog.tsx
│   │   ├── UnsavedChangesPrompt.tsx    # ConfirmDialog with the alternative action
│   │   ├── useIdleActivity.ts      # throttled note_activity; withIdlePaused for native dialogs
│   │   └── usePendingDraft.ts      # stage_pending_changes (debounced), dirty registry for FR-010
│   ├── databases/                  # NEW
│   │   ├── DatabaseChooser.tsx, RecentDatabaseRow.tsx, TakeOverConfirm.tsx
│   │   ├── CreateDatabaseDialog.tsx
│   │   ├── DatabaseMenu.tsx        # top-bar menu + lock button + Ctrl/⌘+L
│   │   ├── DatabaseSettingsDialog.tsx  # Backups · Locking · This computer
│   │   ├── ChangePassphraseDialog.tsx
│   │   ├── RestoreBackupDialog.tsx
│   │   ├── DatabaseNotes.tsx       # disk-encryption and opened-backup banners
│   │   ├── DatabaseGuide.tsx       # FR-030 in-app guide
│   │   ├── databasesService.ts, types.ts, databases.css
│   │   └── *.test.tsx
│   ├── app/AppShell.tsx            # database menu in the top bar; notes banner slot
│   ├── firearms/FirearmForm.tsx, DisposeDialog.tsx, RestoreDialog.tsx
│   │                               # usePendingDraft + accept a resumed draft as initial state
│   ├── insurance/InsurancePolicyForm.tsx, CoverageDialog.tsx   # same
│   └── import-export/ExportDialog.tsx, ImportDialog.tsx
│                                   # FR-031 wording; native dialogs wrapped in withIdlePaused
└── **/*.test.tsx

e2e/
├── wdio.conf.ts                    # drop HOPLODEX_E2E_DB_KEY; scratch user-dirs.dirs; unlock helper
├── support/ui.ts                   # createDatabase(), unlock(passphrase)
├── specs/us7-databases.e2e.ts      # NEW: US1, US2, US5 flows
├── specs/us8-backups.e2e.ts        # NEW: backup on close, restore
├── specs/us9-locking.e2e.ts        # NEW: lock now, pending changes resume and discard
├── specs/us1…us6                   # start by unlocking the seeded/created database
└── screenshots/screens.e2e.ts      # + screens 14–24 (contracts/ui-databases.md §15)

scripts/human-testing.sh            # new data layout (.human-testing/HoploDex/*.hoplodex), prints passphrase
DEVELOPMENT.md                      # Test isolation: passphrase model, no DB key env; human-testing notes
.specify/memory/constitution.md     # PATCH amendment (research §21), via /speckit-constitution
```

**Structure Decision**: No new project or package. It is the existing
two-part Tauri app with two new backend modules: `session`, for what "the
open database" now means, and `platform`, for OS notices. The rest is
services following the existing `ops`/`services` split, so business logic
stays in functions that take `&Connection` or paths and are tested directly.
The frontend gains two feature folders: `session` (lifecycle, cross-cutting)
and `databases` (screens and dialogs). The E2E specs continue the `usN`
numbering from 002's `us6`.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| Local backups on by default, while the constitution's Security & Data Handling section says "any network sync or backup feature MUST be opt-in, off by default" | The user's request and the spec's first clarification: backups protect against corruption only if they exist before the corruption, and they never leave the device | Off by default would leave most users without a backup when it matters. Resolution: a PATCH amendment clarifying that the rule covers backups that leave the device (research §21), made with `/speckit-constitution` before this feature merges. Until then this row records the deviation |

Additions that are not violations but are justified here, since
constitution I asks for it: per-OS system-event code (required by FR-037 and
FR-038; no single crate covers all three platforms and all four notices,
§14); a backend idle clock rather than a frontend timer (webview timer
throttling, §15); frontend draft mirroring (the sleep deadline, §16); three
new Rust crates and four npm packages (Technical Context).

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (research.md, data-model.md, contracts/, quickstart.md).*

- **Code Quality**: the design keeps business logic in `&Connection`/path
  functions (session, services), and commands stay thin. The copy
  mechanisms were narrowed to two (raw copy plus stamp, and
  `sqlcipher_export` for rekey) after the spike, rather than three. Still PASS.
- **Testing**: every acceptance scenario maps to a named test file in
  quickstart.md; interruption points (SC-004) are enumerated per operation;
  the portability fixture pins the format. The per-OS notices are the one
  manual area, with a checklist the PR must fill in. Still PASS.
- **UX Consistency**: all prompts reuse `ConfirmDialog` (one extended with a
  third action and pushed back into the shared set). The guide follows 002's
  guide pattern. Texts that state security facts are fixed in the UI
  contract. Still PASS.
- **Performance**: open measured at 0.34 s of key derivation, against a
  2 s budget. Progress rules are concrete (50 MiB/s estimate, 100 ms). The
  take-over check is one `stat` per write. Still PASS.
- **User Privacy / Security**: nothing is transmitted; the backend holds no
  passphrase between commands, which the salt-sharing finding makes possible;
  decrypted data never touches disk (the page-1 probe copies ciphertext
  only); the developer's pre-feature database and keyring entry are
  untouched by design. The on-by-default local backup remains the one
  recorded deviation, pending the PATCH amendment. PASS with that recorded.

**Result**: PASS, with one recorded deviation that has a scheduled resolution.
