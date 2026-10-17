# Quickstart: validating feature 009

How to show that the feature works. Commands run through the dev container
(CLAUDE.md, "Run tests in the dev container"). DEVELOPMENT.md's "Test" section
has the full forms, including single files.

## Automated

```sh
scripts/dev-container.sh bash -c 'cd src-tauri && cargo test --features test-kdf'
scripts/dev-container.sh npm test
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/databases.e2e.ts --spec e2e/specs/value-insurance.e2e.ts'
scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'
```

Expected: every test passes, and the screenshot walk shows contracts/ui.md
§10's new screens.

The E2E and screenshot runs open `src-tauri/tests/fixtures/releases/0.1.0.hoplodex`
(passphrase `release fixture 0.1.0`), copied into their sandbox. The `e2e`
build's extra step (research.md §14) makes it older than the app.

### Where each scenario and criterion is tested

| Scenario / criterion | Test | Level |
|---|---|---|
| 009 US1-1, US1-2, US1-4, SC-001, SC-002 (identifiers: distinct, refused when reused or missing or malformed, unchanged by edit, reorder, disposal and reversal of the owner, new after the highest id is deleted) | `tests/record_identifier_test.rs` | Rust, real database |
| 009 US1-3, SC-001 (same identifiers after backup, restore, passphrase change and upgrade) | `tests/record_identifier_test.rs`, `tests/release_upgrade_test.rs` | Rust |
| 009 FR-003 (no identifier in any IPC type or export) | `tests/record_identifier_test.rs` (serializes every policy, photo and document type and an export and finds no `uid`) | Rust |
| 009 US2-1, SC-003 (a duplicate name, in any A–Z case and with spaces, refused at the name field; the edited policy itself not counted; expired and future policies counted) | `tests/insurance_policy_test.rs`; `InsurancePolicyForm.test.tsx` shows the field error | Rust, Vitest |
| 009 US2-1 journey | `value-insurance.e2e.ts`: rename a policy to another's name in other letter case, see the error at Name, then save a distinct name | E2E |
| 009 US2-2, US2-3, FR-006, SC-003 (import resolves a name in other letter case and with spaces to its one policy; an unknown name is a row error) | `tests/import_export_test.rs`, for firearm and accessory rows | Rust |
| 009 FR-007 backstop (the index refuses what ops would) | `tests/insurance_policy_test.rs` (a raw insert) | Rust |
| 009 US3-1, FR-009 to FR-011 (asked first; nothing written before the answer; Cancel leaves the file's bytes unchanged; backup at the location before any write) | `tests/upgrade_test.rs` | Rust |
| 009 US3-2, FR-015, FR-017 (every value intact; the note; pending changes kept) | `tests/release_upgrade_test.rs`, `tests/upgrade_test.rs` | Rust |
| 009 US3-3, SC-005 (failure forced at each point; interruption simulated at each point; the next open asks again; no partial backup left or taken for a complete one) | `tests/upgrade_test.rs` | Rust |
| 009 US3-4, FR-018 (a newer database refused unchanged, with the backup folder; its wording) | `tests/database_open_test.rs`; `DatabaseChooser.test.tsx` | Rust, Vitest |
| 009 US3-5 (backups off, and already one today: the backup is still made, and the backup record untouched) | `tests/upgrade_test.rs` | Rust |
| 009 FR-012 (location missing, not writable, full; a chosen folder; a chosen folder that also fails; the waiting database kept) | `tests/upgrade_test.rs`; `DatabaseChooser.test.tsx` | Rust, Vitest |
| 009 FR-013, FR-014 (the name and stamp; listed with `beforeUpgradeTo`; rotation keeps the newest and doesn't count it; the once-a-day rule ignores it; delete all removes it) | `tests/backup_test.rs` | Rust |
| 009 FR-016 (cancel during the backup removes the partial file and leaves the file unchanged; a stop during the upgrade is ignored) | `tests/upgrade_test.rs` | Rust |
| 009 FR-019 (restoring an older backup: `check_backup`, refused without `upgrade`, upgraded copy, a failure leaves the database as it was) | `tests/restore_test.rs`; `RestoreBackupDialog.test.tsx` | Rust, Vitest |
| 009 US4-1, FR-020 (a test-only rebuild of `photos` and a new required unique value: rows, identifiers and links kept; an orphan abandons the upgrade) | `tests/upgrade_steps_test.rs` | Rust |
| 009 US4-2, FR-021, SC-006 (an altered, removed or reordered released step fails) | `tests/released_migrations_test.rs` (checks a doctored manifest as well as the real one) | Rust |
| 009 US4-3, FR-023, SC-004 (the stored 0.1.0 database opens upgraded with every value; the backup equals it) | `tests/release_upgrade_test.rs` | Rust |
| 009 FR-022, research.md §7 and §13 (no forbidden statement in any step; the columns an older layout must keep) | `tests/upgrade_steps_test.rs` | Rust |
| 009 US3 journey (open the 0.1.0 database, see the question, upgrade, see the note, find the backup in the restore list) | `databases.e2e.ts` | E2E |
| 009 SC-007 (progress within 1 s; the upgrade's time at 10,000 items) | `tests/performance_test.rs` | Rust |
| research.md §15 (an unknown notice kind or field in `machine.json` keeps the recent list) | `tests/machine_settings_test.rs` | Rust |
| Edge: several steps skipped are one upgrade (two test-only steps, a failure in the second leaves neither) | `tests/upgrade_test.rs` | Rust |
| Edge: a backup opened directly that needs an upgrade (Cancel leaves it a backup; Upgrade gives it its own id and backup) | `tests/upgrade_test.rs` | Rust |
| Edge: the database open on another computer (the take-over comes first; a marker set during the question refuses the upgrade) | `tests/upgrade_test.rs` | Rust |

## Manual checks (best effort before a release, not merge gates)

### Setup S1 (Linux or macOS)

1. From the checkout, run `scripts/human-testing.sh --reset --release --older`.
   It seeds `.human-testing/`, adds **Older collection** (a copy of the stored
   0.1.0 database, in `.human-testing/Older/`) to the recent list, and
   launches the E2E-profile build, whose extra step makes Older collection
   need an upgrade.
2. In the chooser, select **Older collection**.
3. Type the passphrase `release fixture 0.1.0` and press **Open**.
   Expected: the question "Upgrade “Older collection” for HoploDex 0.1.0?"
   with **Upgrade** focused.

### M1: The whole round trip, by keyboard

1. Do Setup S1.
2. Press **Enter**. Expected: the progress dialog may flash by, then the
   collection opens with the note "“Older collection” was upgraded for
   HoploDex 0.1.0. A backup of it from before the upgrade is
   …/Older/HoploDex backups/Older collection <date> <time> <id8> before
   0.1.0.hoplodex. …".
3. Press **Ctrl+L** (⌘L on macOS) to lock it. Expected: the chooser.
4. Quit HoploDex.
5. Run `scripts/human-testing.sh` (without `--release`: the `tauri dev`
   build, which lacks the extra step, so it stands in for the earlier
   version).
6. Select **Older collection**, type `release fixture 0.1.0` and press
   **Open**. Expected: refused, with "“Older collection” was last used by a
   newer version of HoploDex, which upgraded it. … look for one whose name
   ends in “before” and a version number, in …/Older/HoploDex backups."
7. Choose **Open another database file…** and pick the backup named in step 2.
8. Type `release fixture 0.1.0` and press **Open**. Expected: it opens, with
   the note "This is a backup of Older collection made on <date>, before it
   was upgraded to HoploDex 0.1.0. …".

### M2: The backup location can't be written

1. Run `scripts/human-testing.sh --reset --release --older --seed-only`.
2. Run `mkdir -p ".human-testing/Older/HoploDex backups"` and then
   `chmod a-w ".human-testing/Older/HoploDex backups"`, so its default backup
   folder can't be written. The database's own folder stays writable, as
   SQLite needs it for its journal.
3. Run `scripts/human-testing.sh --release`.
4. Select **Older collection**, type `release fixture 0.1.0`, press
   **Open**, then press **Upgrade**. Expected: "Save the backup somewhere
   else?", saying HoploDex isn't allowed to save files in
   …/Older/HoploDex backups.
5. Press **Choose a folder…** and choose `.human-testing/HoploDex`.
   Expected: the collection opens, and its note names the backup in
   `.human-testing/HoploDex` and says it isn't in the database's list of
   backups.
6. Afterwards, run `chmod u+w ".human-testing/Older/HoploDex backups"`.

### M3: A screen reader hears the phases (Linux with Orca, or macOS with VoiceOver)

1. Do Setup S1 with the screen reader on.
2. Press **Enter** on **Upgrade**. Expected: the question's title and
   description are read when it opens, and "Upgrading “Older collection”…"
   is announced when the phase changes (a 0.1.0-sized database may finish
   before the backup phase is announced; that is expected).
