# Implementation Plan: Stable Record Identifiers, Unique Policy Names and Upgradable Databases

**Branch**: `009-upgrade-readiness` | **Date**: 2026-10-17 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/009-upgrade-readiness/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Two issues and a question, all about the 0.1.0 data layout. The first two
stories are the last layout changes made in place; the last two build the
machinery that every later layout change goes through.

- **#53, record identifiers (P1)**: `insurance_policies`, `photos` and
  `document_attachments` get 006's `uid`.
  - It is a lowercase v4 UUID, `NOT NULL UNIQUE`, checked by a `CHECK`, fixed
    by a trigger, and set in `ops` at the one insert of each table.
  - It is never shown or exported. Disposition history and the built-in lists
    get none.
- **#54's question, unique policy names (P2)**: a unique index on
  `trim(name) COLLATE NOCASE` (A–Z folded, surrounding spaces ignored).
  - The policy form's save checks first and refuses at the name field, naming
    the other policy.
  - Import's lookup uses the same comparison, so a name finds one policy or is
    a row error.
- **#115, upgrading a database (P3)**:
  - **The question first**: an open that finds steps due stops with the
    database keyed and locked but unwritten, waiting in a new session slot,
    and the chooser asks. Cancel drops it untouched. Upgrade makes a backup
    (cancellable, with progress, named `… before <version>.hoplodex`, kept
    out of rotation while it is the latest) and then runs every due step in
    one transaction.
  - **All or nothing**: foreign keys are off around the steps and a
    `foreign_key_check` follows them. A failure rolls back; an interruption
    leaves a hot journal that the next open rolls back before asking again.
  - **Restoring an older backup** upgrades the restored copy before the swap,
    after a pre-check that lets the restore's one confirmation say so.
  - **The newer-version refusal** names where a backup from before the upgrade
    can be found. It ships in 0.1.0.
- **#115, keeping upgrades trustworthy (P4)**:
  - `migrations/released.txt` records each released step's SHA-256, and a
    test fails when one changes, disappears or moves.
  - A stored database made by 0.1.0 is opened, upgraded and compared value by
    value in every test run.
  - Test-only steps (a rebuild of `photos`, a new required unique value)
    exercise rebuilds, the link check, forced failures at each point, and
    simulated power loss.

**Decisions taken at planning**: the spec's six clarifications (2026-10-16)
are the owner's. Everything else is listed under "Design choices worth a
look" at the end.

## Technical Context

**Language/Version**: Rust 1.97+ (edition 2024, `src-tauri`), TypeScript 5.x /
React 18 (`src`), unchanged

**Primary Dependencies**: no new runtime dependency. `sha2` 0.10 becomes a
dev-dependency for the released-steps test (research.md §11, §18). It is
already in `Cargo.lock` through `secret-service` and `tauri-codegen`. SQLite
behaviours the design depends on (research.md §4, §7, §2) were probed at
planning and become the first tests against SQLCipher (research.md §14).

**Storage**: the existing encrypted SQLCipher file.
- **The last in-place edit.** `0001_initial.sql` is edited in place one last
  time:
  - `uid` and `*_uid_fixed` on three tables;
  - `idx_insurance_policies_name`;
  - `app_state.backup_before_upgrade_to`.
- **Development databases** must be created again (CLAUDE.md "Schema
  changes"). The developer's databases are not touched.
- **Upgrade steps.** `db::apply_migrations` becomes `db::upgrade`, the one
  runner for creating and upgrading.
- **Backup names** gain the ` before <version>` form (data-model.md).
- **`machine.json`** gains `UnfinishedBackup.beforeUpgrade`, and is read
  leniently (research.md §15).

**Testing**:
- **Rust integration tests on real databases** (`cargo test --features
  test-kdf`).
  - New: `upgrade_test.rs`, `upgrade_steps_test.rs`,
    `released_migrations_test.rs`, `release_upgrade_test.rs`.
  - Amended: `record_identifier_test.rs`, `insurance_policy_test.rs`,
    `import_export_test.rs`, `backup_test.rs`, `restore_test.rs`,
    `database_open_test.rs`, `machine_settings_test.rs`,
    `performance_test.rs`, plus every test that inserts photos, documents or
    policies directly (research.md §1).
- **Test-only steps and fault injection** (`db::upgrade::testing`) are
  compiled only with `test-kdf` and debug assertions (research.md §14).
- **The stored database made by 0.1.0** is
  `tests/fixtures/releases/0.1.0.hoplodex`, made by
  `examples/release_fixture.rs` from the human seed's Main collection, with
  passphrase `release fixture 0.1.0`.
- **Vitest**: `DatabaseChooser`, the new upgrade dialogs,
  `RestoreBackupDialog`, `DatabaseNotes`, `InsurancePolicyForm`.
- **E2E**: the US3 journey in `databases.e2e.ts`, using the stored 0.1.0
  database and the `e2e` build's test-only step. The US2 journey is in
  `value-insurance.e2e.ts`. The screenshot walk gets the new screens.

**Target Platform**: desktop: Windows 10+, macOS 26+, Linux (WebKitGTK 2.40+),
unchanged. The upgrade is SQL and file I/O, so it is the same on each OS. The
Rust suite and the E2E journey run on all three (constitution II; CLAUDE.md:
a failure seen on one OS is verified there).

**Project Type**: desktop application (Tauri: React frontend + Rust backend in
`src-tauri/`).

**Performance Goals** (constitution IV):
- An open with no step due keeps 003 SC-003's 1 s; the new check is one read
  that already happens.
- Progress shows within 1 s whenever the backup and upgrade take longer
  (SC-007).
- Feedback within 100 ms through the row's busy state.
- The interface stays responsive, since `upgrade_database` is an `async`
  command.

**Constraints**:
- **Never write before the answer** (FR-010).
- **Never upgrade without a backup** (FR-012).
- **All or nothing across every due step**, power loss included (FR-015).
- **Released steps are frozen from the 0.1.0 tag** (FR-021).
- **Steps are pure SQL** that depend only on their starting layout (FR-022).
- **What an older layout must keep** for the open to read and the stamp to
  write (research.md §13).
- **The passphrase is not kept** while the user decides (003 FR-007;
  research.md §4).
- **No identifier is shown or exported** (FR-003).

**Scale/Scope**: four user stories (P1 to P4), 23 functional requirements, 7
success criteria. Three new commands (`upgrade_database`, `cancel_upgrade`,
`check_backup`), one new event, five new or amended error codes, five new
dialog states in the chooser and restore dialog, and one note.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Lint/static analysis, review, small single-purpose modules, no speculative abstraction | **One runner**: `db::upgrade` both creates and upgrades a database, and the restore uses it too. No second path applies steps. **One comparison** for policy names: the index's expression, used by the form's check and import's lookup. **One backup routine** with a `kind`, not a second copy of `make_backup`. **Refactor, not workaround**: `open_database` returns `Opened` and every caller follows; `make_backup` takes `BackupKind` everywhere; `restore_backup` gains `upgrade`; no `_with_upgrade` siblings. The upgrade flow is its own module (`session/upgrade.rs`) rather than growing `lifecycle.rs`. `clippy`, `rustfmt`, `eslint`, `prettier`, the citations check |
| II. Testing (NON-NEGOTIABLE) | Tests first, real persistence, one test per acceptance scenario, isolation | Every scenario and criterion maps to a named test (quickstart.md). Upgrades are tested against a stored 0.1.0 database and test-only steps, with failure forced at each point and power loss simulated by copying the file and its hot journal mid-transaction. The manifest test fails on a doctored copy (red check). The fixture is made by a refusing-`test-kdf` example; E2E and human testing copy it into their sandbox, never near real data. Three manual procedures, numbered, naming "Older collection" and `release fixture 0.1.0` |
| III. UX Consistency | One component set and pattern, WCAG 2.1 AA | The question is the standard `ConfirmDialog`, non-destructive with focus on Upgrade (One Enter rule). The progress dialog follows the closing screen's show-at-once-or-after-a-second rule and its `ProgressBar`. The folder choice uses the folder picker the settings dialog uses. The note is in the existing notes area, and the failure in the existing open-failure slot. The policy name error uses the form's field-error pattern. The phase line is a live region, so a screen reader hears the change |
| IV. Performance | 100 ms feedback / 1 s completion, 500 ms search, no UI-thread blocking | No step due: one existing read, timed in `performance_test.rs`. The upgrade is an `async` command with progress from its first chunk; SC-007 is measured at 10,000 items with the test-only rebuild. The policy-name check and the lookup use the new index. **Performance-sensitive** (the open path, a rebuild over 10,000 items): the PR reports the timings |
| V. User Privacy | Local only, real deletion, no hidden copies | The backup is local, at the location the user set or a folder they choose, and is named in the question and the note (Principle V: "make it clear what leaves the device and where it goes"; nothing leaves). A cancelled or failed backup's partial file is securely deleted; dropped tables' pages are zeroed (`secure_delete`). Identifiers are random and never shown |
| Security & Data Handling | Encryption at rest, vetted dependencies, no cipher change | No cipher change. The backup is a byte copy, protected by the passphrase current when made. The waiting database holds the derived key, not the passphrase, and sleep and screen lock drop it. A chosen folder is validated and written exclusively. The new commands are on the main web view only, and reach the waiting database only by its id. PR notes come from research.md §17 |
| Licensing | GPLv3-compatible dependencies and assets with recorded source | `sha2` is MIT/Apache-2.0 and dev-only. The stored 0.1.0 database is generated by the project from its own seed data (research.md §18) |

**Result**: PASS, with the entries in Complexity Tracking.

## Test Levels

*Every acceptance scenario needs a test (constitution II), but not an E2E one.
E2E is the slowest layer and runs on every OS, some of them small VMs, so place
each scenario at the cheapest level that can show it, and say why wherever the
plan departs from these defaults.*

- **E2E, one journey per user story**: the story's main path through the real
  app, keyboard-first where the story calls for it, as a
  `describe("009 US1 - [the story's heading]")` in the existing E2E file
  whose setup it shares, whatever spec that file began with. Every file adds
  start-up time on every OS, so a new file needs a reason here (DEVELOPMENT.md,
  "E2E files").
- **E2E, one test per error mechanism per form or dialog**, not one per rule.
  The policy form's field-error mechanism already has one (001 US3-12's
  overlap on Coverage starts), so the name rule adds none. The upgrade
  question, progress and folder dialogs are new; their refusals are walked in
  the US3 journey where they need the real app, and the rest in Vitest.
- **E2E, what only the real app shows**: IPC wiring of the three new commands
  and the event, the waiting database's lock, and the screenshot walk.
- **Security**: the waiting database is unreachable by collection commands
  (Rust, `session_scope_test.rs` and `upgrade_test.rs`), and the chosen-folder
  checks run in Rust. The ACL and capability entries are covered by
  `acl_manifest_test.rs`.
- **Everything else** at the Rust `ops` level on real databases, or in Vitest.

| Story | E2E journey | Other E2E (error mechanism, real app, security) | Scenarios tested in Rust or Vitest |
|-------|-------------|-------------------------------------------------|------------------------------------|
| US1 | None. The identifiers are never shown (FR-003), so no path through the app shows them; the story is entirely backend behaviour | — | US1-1 to US1-4 → Rust (`record_identifier_test.rs`, `release_upgrade_test.rs`) |
| US2 | In `value-insurance.e2e.ts` (its setup: a database with policies): rename "Collector Schedule" to "range bag rider " (another policy's name in other case, with a space), see the error at Name naming “Range Bag Rider”, then save "Collector Schedule 2026" | None new: the field-error mechanism exists | US2-1 → Rust (`insurance_policy_test.rs`) and Vitest (`InsurancePolicyForm.test.tsx`); US2-2, US2-3 → Rust (`import_export_test.rs`) |
| US3 | In `databases.e2e.ts` (its setup: the chooser and opening): copy the stored 0.1.0 database into the sandbox; open it; the question, with Upgrade focused; Enter; the collection with the upgraded note; open Restore from a backup and see the backup listed "before upgrading to HoploDex 0.1.0". Then a second copy with a file where its backup folder would be (so the location is missing on every OS): Upgrade shows the "Save the backup somewhere else?" dialog, and Cancel returns to the chooser. The native folder picker can't be driven by WebDriver, so choosing a folder is tested in Rust (`upgrade_database` with `backupFolder`) and in Vitest (the picker mocked) | The question's Cancel, and the folder dialog's, leave the file's bytes unchanged (SHA-256 before and after, read by the spec): it needs the real chooser and IPC | US3-1 to US3-5, FR-011 to FR-019, the edge cases → Rust (`upgrade_test.rs`, `backup_test.rs`, `restore_test.rs`, `database_open_test.rs`); the dialogs' text, focus and phases → Vitest |
| US4 | None. The story is the maintainer's: its journey is the automated checks themselves | — | US4-1 → Rust (`upgrade_steps_test.rs`); US4-2 → Rust (`released_migrations_test.rs`); US4-3 → Rust (`release_upgrade_test.rs`) |

## Project Structure

### Documentation (this feature)

```text
specs/009-upgrade-readiness/
├── plan.md              # This file
├── research.md          # Phase 0: decisions and their reasons (§1–§19)
├── data-model.md        # Phase 1: schema, backup files, machine.json, IPC types, states
├── quickstart.md        # Phase 1: automated coverage and manual checks
├── contracts/
│   ├── tauri-commands.md      # commands, errors, events
│   ├── ui.md                  # the question, progress, folder choice, notes, wording
│   ├── database-file.md       # layout version, in-place upgrade, backup names
│   └── spreadsheet-format.md  # insurance_policy_name lookup
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
src-tauri/
├── Cargo.toml                        # sha2 dev-dependency
├── src/
│   ├── db/
│   │   ├── mod.rs                    # open_database → Opened (Ready | UpgradeDue); check_copy; create via upgrade
│   │   ├── upgrade.rs                # NEW: the runner, step list (+ e2e step), pre-upgrade columns, testing hooks
│   │   └── migrations/
│   │       ├── 0001_initial.sql      # last in-place edit: uid ×3, uid triggers, name index, backup_before_upgrade_to
│   │       ├── released.txt          # NEW: released steps' and fixtures' SHA-256
│   │       └── README.md             # NEW: the rules for a step (research.md §7)
│   ├── session/
│   │   ├── mod.rs                    # awaiting_upgrade slot; dropped with every other open, quit, sleep, screen lock
│   │   ├── lifecycle.rs              # open → UpgradeDue handling; sleep/screen lock/shutdown drop the slot
│   │   └── upgrade.rs                # NEW: upgrade_database's steps 5a–5d, cancel, stamp-taking for a backup opened directly
│   ├── services/
│   │   ├── backups.rs                # BackupKind, ` before <version>` names, rotation, once-a-day, stamp, silent sweep
│   │   └── machine_settings.rs       # UnfinishedBackup.beforeUpgrade; lenient notices and enums
│   ├── commands/
│   │   ├── databases.rs              # UPGRADE_NEEDED, upgrade_database, cancel_upgrade, NEWER_VERSION details, dismiss "upgraded"
│   │   ├── backups.rs                # restore_backup { upgrade }, check_backup, upgrading phase
│   │   ├── insurance.rs              # uid at insert; name check (FR-008)
│   │   ├── photos.rs, documents.rs   # uid at insert
│   │   ├── import_export.rs          # policy lookup by trim + NOCASE
│   │   └── error.rs                  # UPGRADE_NEEDED, UPGRADE_FAILED, UPGRADE_BACKUP_FAILED
│   ├── models/database.rs            # OperationKind::Upgrade, BackupInfo, OpenedBackupNote, DatabaseNotes, UpgradedNote
│   ├── commands_list.rs, main.rs     # three new commands, unscoped
│   └── capabilities/default.json     # allow-upgrade-database, allow-cancel-upgrade, allow-check-backup
├── examples/
│   ├── human_seed.rs                 # uid on its direct document insert; `--older` recent entry
│   └── release_fixture.rs            # NEW: makes tests/fixtures/releases/0.1.0.hoplodex
└── tests/
    ├── upgrade_test.rs               # NEW
    ├── upgrade_steps_test.rs         # NEW
    ├── released_migrations_test.rs   # NEW
    ├── release_upgrade_test.rs       # NEW
    ├── fixtures/releases/0.1.0.hoplodex  # NEW
    └── (amended, see Technical Context)

src/features/
├── databases/
│   ├── DatabaseChooser.tsx           # UPGRADE_NEEDED → the dialogs; NEWER_VERSION text; UPGRADE_FAILED text
│   ├── UpgradeDialogs.tsx            # NEW: question, progress, "Save the backup somewhere else?"
│   ├── RestoreBackupDialog.tsx       # check_backup first; the upgrade sentence; upgrading phase; beforeUpgradeTo label
│   ├── DatabaseNotes.tsx             # upgraded note; opened-backup note's "before" form
│   ├── databasesService.ts, types.ts # upgradeDatabase, cancelUpgrade, checkBackup, onUpgradeProgress; types
└── insurance/InsurancePolicyForm.tsx # Name hint

e2e/
├── specs/databases.e2e.ts            # 009 US3 journey
├── specs/value-insurance.e2e.ts      # 009 US2 journey
├── screenshots/screens.e2e.ts        # contracts/ui.md §10
└── support/sandbox.ts                # copy the stored 0.1.0 database into a sandbox

scripts/human-testing.sh              # --older
CLAUDE.md, DEVELOPMENT.md             # the schema rule from 0.1.0 on; "Changing the data layout"; --older
```

**Structure Decision**: no new project or layer.
- **Rust** owns the decision to upgrade, the backup, the transaction, the
  checks and the rules.
- **React** asks, shows progress, offers the folder and shows the results.
- **The amendments to earlier specs** are made at implementation, as one-line
  italic notes (CLAUDE.md "Spec Kit workflow"):
  - 001 FR-019, FR-020, FR-027 (unique names and the lookup);
  - 003 FR-014, FR-025, FR-028 and its database-file, tauri-commands and
    ui-databases contracts (the refusal's wording, rotation, the restore,
    backup names, `open_database`);
  - 006 FR-019 (the identifier extended).

**First tasks**, because later ones depend on them holding:
1. **SQLCipher probes as tests** (research.md §14). Under SQLCipher with this
   app's connection settings:
   - `BEGIN EXCLUSIVE; COMMIT` keeps the exclusive lock;
   - a copied file and hot journal from mid-transaction (cache spilled) roll
     back on a keyed open;
   - the `trim(name) COLLATE NOCASE` index refuses and is used by the lookup.

   If the first fails, take the lock with the marker write instead and tell
   the owner, since Cancel would then not be byte-for-byte untouched. If the
   second fails, stop and ask: the in-place upgrade would need the
   copy-and-swap alternative (research.md §7).
2. **The runner replaces `apply_migrations`** for creation, with the whole
   existing suite passing, before any upgrade path uses it.
3. **The layout changes** (Stories 1 and 2), then the stored 0.1.0 database
   made from the settled layout (spec Assumptions: Story 4's database comes
   after Stories 1 and 2).

### Close-out (at pull request time)

The final phase of tasks.md, done as part of opening the pull request. Check
each issue's state first (`gh issue view <n> --json state`), and don't comment
on a closed one unless the owner asks.

- **#53**: the pull request closes it ("Fixes #53"). Comment on what was
  settled:
  - `uid` on `insurance_policies`, `photos` and `document_attachments`, on
    006's terms;
  - none on `disposition_history` or the built-in lists;
  - uniqueness across kinds only between firearms and accessories;
  - the tests that cover it.
- **#115**: the pull request closes it ("Fixes #115"). Comment item by item:
  - released steps frozen by `released.txt` and its test, and the CLAUDE.md
    rule;
  - the backup before upgrading, and its rotation;
  - one transaction with foreign keys off and the link check;
  - the stored 0.1.0 database, and the tests that open it.
- **#54**: comment that names are now unique (A–Z case and surrounding spaces
  ignored), and that rows keep naming their policy by name. Update its
  "Identity and matching" question: names are unique and `uid` exists, so
  matching an imported policy row is the open part. Everything else stays.
- **#24**: comment that the 0.x upgrade machinery exists (frozen steps,
  backup, upgrade question, stored release databases). The post-1.0 policy
  stays there: breaking changes, a fuller dialog, squashing the 0.x steps,
  showing the last-saved version.
- **#19** (release process): comment that tagging a release must add its
  steps' and stored database's lines to `released.txt`, add the next stored
  database once a step exists, and run `release_upgrade_test.rs`. 0.1.0 is
  the first.
- **#16**: comment that adding `location` to `pending_changes.kind` is now
  possible after 0.1.0, as a rebuild step (research.md §7), with an example
  of the order.
- **#30, #31**: comment that policies, photos and documents now carry
  identifiers that hold across copies of a database.
- **#118**: comment that, after 0.1.0, letting policies own documents is a
  rebuild of `document_attachments` (its owner `CHECK`), which the runner now
  supports.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| A new session slot, `awaiting_upgrade`, beside the open database, dropped by quit, sleep, screen lock, shutdown and any other open | FR-010's question must come before anything is written, and 003 FR-007 keeps the passphrase from being held while the user decides; only a keyed connection held by the backend does both (research.md §4) | Refuse and resend: the passphrase must then be kept in the chooser or typed again, and a field in the question takes FR-010's focus from Upgrade. Open first and then ask: Cancel would have written the marker |
| Test-only hooks in the library (`db::upgrade::testing`), compiled only with `test-kdf` and debug assertions | SC-005 needs a failure at each point and power loss mid-transaction, against the real runner; FR-020 needs a rebuild step before any real one exists | Faults from outside (killing a process, filling a disk) can't hit each point reliably, and a real rebuild step would change 0.1.0's layout |
| One step that only `e2e` builds apply (`9000_e2e_upgrade_check`) | The US3 journey needs a database older than the app, and until 0.2.0 no shipped step makes one | Building a second app binary for one spec would double the E2E build time on every OS |
| Lenient reading of `machine.json` (research.md §15), beyond the spec's database layout | The downgrade the spec supports (FR-013, FR-018) runs 0.1.0 against a later `machine.json`; today that loses the recent list. It must ship in 0.1.0 to help at all | Leaving it means the first downgrade a beta tester makes empties their recent list |

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (research.md, data-model.md, contracts/,
quickstart.md).*

- **Code Quality**: each rule has one home.
  - Steps: the runner.
  - Names: the index's expression.
  - Backups: `make_backup` with a kind.
  - The waiting database: one slot.
  - What an older layout must keep: one list with its test.

  Signatures change and callers follow. Still PASS.
- **Testing**: every scenario, criterion and edge case has a named test
  (quickstart.md). Failure and interruption at each point are automated. The
  stored 0.1.0 database makes US4-3 a standing check. Three manual
  procedures, none a gate. Still PASS.
- **UX Consistency**: one new dialog component file holding three standard
  dialogs. Every text is in contracts/ui.md, and focus follows the One Enter
  rule. Still PASS.
- **Performance**: unchanged for opens with nothing due; the upgrade is
  measured and reported. Still PASS.
- **User Privacy**: the only new copy is the user-visible backup, at a
  location the user set or chose. Still PASS.

**Result**: PASS. The design adds no violations beyond those justified above.

## Design choices worth a look

The owner settled the spec's six clarifications (2026-10-16). These were
decided at planning, inside what the spec allows:

- **The database waits, keyed and locked, while the question is up**, in a
  session slot of its own. Nothing is written until **Upgrade**, and Cancel
  only closes the connection (research.md §4). A passphrase to be remembered
  is kept, zeroized, until the upgrade ends.
- **All due steps in one transaction**, relying on SQLite's rollback journal
  for power loss, rather than upgrading a copy and swapping it in. That avoids
  a third copy's worth of disk space (research.md §7).
- **"Ignoring letter case" folds A–Z only**, as import does today, because
  folding all of Unicode needs a collation the file can't carry
  (research.md §2). "Église" and "église" are two names.
- **The backup's name ends in ` before <version>`**, so listings and rotation
  find it without decrypting. The same marker goes in its stamp for when it
  is opened directly (research.md §6). 0.1.0 already knows both.
- **The backup before an upgrade is stamped like any backup**: no open marker
  and no pending changes. Its collection is identical, and the pending
  changes survive in the upgraded database (research.md §6).
- **A backup opened directly has its stamp taken inside the upgrade**, so
  Cancel leaves it a backup, and its backup belongs to the database it
  becomes (research.md §8).
- **Restoring an older backup checks it first** (`check_backup`), so the
  restore's one confirmation can say it will be upgraded. That also moves the
  wrong-passphrase error before the confirmation. The backup restored from
  rotates as an ordinary backup (research.md §9).
- **The stored 0.1.0 database is the human seed's Main collection**, with
  pending changes added, kept under 2 MB if it can be (research.md §12).
- **`machine.json` is read leniently**, so going back to 0.1.0 keeps the
  recent list (research.md §15). This is an addition the spec doesn't ask
  for.
- **`human-testing.sh` gains `--older`**, which puts a copy of the stored
  0.1.0 database in the sandbox as "Older collection" (quickstart.md).
