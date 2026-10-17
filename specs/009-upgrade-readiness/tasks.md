---

description: "Task list for feature 009: stable record identifiers, unique policy names and upgradable databases"
---

# Tasks: Stable Record Identifiers, Unique Policy Names and Upgradable Databases

**Input**: Design documents from `/specs/009-upgrade-readiness/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (tauri-commands.md, ui.md, database-file.md, spreadsheet-format.md), quickstart.md (all present)

**Tests**: The project constitution's Testing Standards principle is NON-NEGOTIABLE: every user story below includes test tasks written first, expected to fail, then made to pass by the implementation. Backend tests run the real `ops`, the real `Session` and the real upgrade runner against temporary SQLCipher databases (no mocks of the DB). Failures and interruptions are forced through `db::upgrade::testing` (research.md §14), which is compiled only with the `upgrade-test-steps` feature: `test-kdf` turns it on, the release `performance_test` uses it alone, and `custom-protocol` and `e2e` builds refuse it.

**Organization**: Tasks are grouped by user story (spec.md's priorities P1–P4) so each story can be implemented and tested on its own. This feature is a **delta** against features 001 to 008: most tasks edit existing files (plan.md's Project Structure). The layout changes are made in place in `0001_initial.sql` for the last time (research.md §11); no new migration file.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Which user story this task belongs to (US1–US4)
- Exact file paths are given per plan.md's Project Structure
- In this file, bare IDs (FR-001, US1, T012) mean this feature's. In code, comments and test names, cite them with the feature's number: `009 FR-001`, `009 US3-2`, `009 T012`, `009 research.md §7` (DEVELOPMENT.md, "Spec citations"); `npm test` fails on a bare one
- Work that must be built, tested or run on a specific platform gets one task per platform, starting with the platform's name

## Path Conventions

Existing Tauri desktop app: Rust backend in `src-tauri/`, React/TypeScript frontend in `src/`, WebdriverIO E2E suite in `e2e/`. Run tests, lint, the audit and screenshots through `scripts/dev-container.sh` (CLAUDE.md; commands in quickstart.md). macOS work runs in the macOS 26 VM and Windows work on the Windows test machine (DEVELOPMENT.md's macOS and Windows sections). Never open the real databases, `machine.json` or keyring entries (CLAUDE.md, "Never touch the real databases"): the stored 0.1.0 database is made by an example into the repository and copied into each test's, E2E run's and human-testing sandbox, never next to real data.

## How the stories divide the shared pieces

- **Foundational** owns the runner: `db::upgrade` replacing `apply_migrations` (one transaction, foreign keys off, the link check, `changes_waiting` kept, `build_steps`), its test hooks (`db::upgrade::testing` and the `upgrade-test-steps` feature), and the SQLCipher probes the design rests on (plan.md "First tasks" 1 and 2), proved on each OS.
- **US1** owns `uid` on `insurance_policies`, `photos` and `document_attachments` (schema, `ops` inserts, every direct insert in tests and the seed).
- **US2** owns `idx_insurance_policies_name`, the policy form's name check and its field error, import's lookup, and the third probe (the index under SQLCipher).
- **US3** owns everything the user sees of an upgrade, and the stored 0.1.0 database, which its E2E journey needs: `app_state.backup_before_upgrade_to` (the last layout change, so the fixture comes after it and after US1 and US2), `examples/release_fixture.rs` and the fixture, the `e2e` build's step, `Opened`, the `awaiting_upgrade` slot, `session/upgrade.rs`, `BackupKind`, `machine.json`'s `beforeUpgrade` and lenient reading, the three new commands, the error codes, the restore's upgrade, every dialog and note, the `e2e` build's phase hold (`HOPLODEX_E2E_UPGRADE_PHASE_SECONDS`), the seeded backup made before an upgrade, and `human-testing.sh --older`.
- **US4** owns what keeps upgrades trustworthy: the rebuild and fill test steps (`testing::PHOTOS_REBUILD`, `testing::HISTORY_UID_FILL`), the step rules and their scan, the pre-upgrade columns' test, `released.txt` and its test, the full release upgrade comparison over the fixture, and the CLAUDE.md and DEVELOPMENT.md rules.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Empty modules and the one dev-dependency, so later tasks in different files can proceed in parallel.

- [X] T001 Create and declare the new backend files, each with a module doc comment citing its research.md section, empty but compiling:
  - `src-tauri/src/db/upgrade.rs` (research.md §7), declared in `src-tauri/src/db/mod.rs`, with an empty `pub mod testing` behind `#[cfg(feature = "upgrade-test-steps")]` (§14), and a `compile_error!` when `upgrade-test-steps` is combined with `custom-protocol` or `e2e`, as `db/cipher.rs` guards `test-kdf`;
  - `src-tauri/src/session/upgrade.rs` (§4, §8), declared in `src-tauri/src/session/mod.rs`;
  - `src-tauri/src/db/migrations/released.txt` holding only the two comment lines of data-model.md "`migrations/released.txt`";
  - `src-tauri/src/db/migrations/README.md` with a heading and a one-line pointer to research.md §7 (filled by T068);
  - `src-tauri/tests/upgrade_test.rs`, `upgrade_steps_test.rs`, `released_migrations_test.rs` and `release_upgrade_test.rs`, each with only a module doc comment, and a `mod` line for each in `src-tauri/tests/integration.rs` (every test file is a module of that one program; `test_files_test` fails without the line).

  In `src-tauri/Cargo.toml`'s `[features]`, add `upgrade-test-steps = []` with a comment saying what it compiles in and that shipped and E2E builds refuse it, and change `test-kdf = []` to `test-kdf = ["upgrade-test-steps"]` (research.md §14). Add `sha2 = "0.10"` under `[dev-dependencies]` in `src-tauri/Cargo.toml` (research.md §18; already in `Cargo.lock` through `secret-service` and `tauri-codegen`, so `Cargo.lock` gains no new package). `cargo build --manifest-path src-tauri/Cargo.toml` must pass in the dev container, and `cargo check --manifest-path src-tauri/Cargo.toml --features upgrade-test-steps,custom-protocol` must fail with the `compile_error!` (its red check)
- [X] T002 [P] Create `src/features/databases/UpgradeDialogs.tsx` as an empty, compiling module with a header comment naming contracts/ui.md §1–§3. `npm run lint` must pass

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The SQLite behaviours the design depends on, proved under SQLCipher on each OS, and the one runner every later task goes through (plan.md "First tasks" 1 and 2).

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### First task 1: the SQLCipher probes (research.md §4, §7, §14)

- [X] T003 Write the lock probe in a new `src-tauri/tests/upgrade_steps_test.rs`: create a database with `db::create_database`, open a second connection to it with the app's connection settings (`db/cipher.rs`'s `CIPHER_SETTINGS`, foreign keys, `secure_delete`, `locking_mode = EXCLUSIVE`) and its key, run `BEGIN EXCLUSIVE; COMMIT` with no statement between, and assert that (a) the file's bytes are unchanged (SHA-256 of the whole file, read through `db/raw_file.rs`), and (b) a third connection's first keyed read fails with "database is locked" until the probing connection is dropped, then succeeds
- [X] T004 Write the hot-journal probe in `src-tauri/tests/upgrade_steps_test.rs`: in a database with a 5,000-row table, set `cache_size = 1`, `BEGIN IMMEDIATE`, rebuild the table in research.md §7's order (create `new_x`, copy, drop `x`, rename) without committing; while the transaction is open, copy the database through `RawFile` (never `std::fs`) and its `-journal` file to another folder; drop nothing else. Open the copy with its passphrase and assert the original rows and `sqlite_schema` are back and `PRAGMA integrity_check` is `ok`
- [X] T005 On Linux, run T003 and T004 in the dev container (`scripts/dev-container.sh bash -c 'cargo test --manifest-path src-tauri/Cargo.toml --features test-kdf --test integration upgrade_steps_test::'`). If T003 fails, change research.md §4 step 2 to take the lock with the open marker's write instead and tell the owner, since Cancel would then not leave the file byte-for-byte untouched; if T004 fails, stop and ask the owner (the in-place upgrade would need research.md §7's copy-and-swap alternative). Record the outcome in research.md §14
- [ ] T006 [P] On macOS, in the macOS 26 VM, run T003 and T004 as T005 does and apply T005's rules to the outcome
- [ ] T007 [P] On Windows, on the Windows test machine, run T003 and T004 as T005 does and apply T005's rules to the outcome (Windows' file locks differ most from POSIX ones)

### First task 2: the runner replaces `apply_migrations` (research.md §7)

- [ ] T008 Write the runner's tests first in `src-tauri/tests/upgrade_steps_test.rs`; they fail to compile until T009 and T010, which is their red step. Each uses `testing::scope(dir).extra_steps(…)` with test-only steps defined in the test, on a database made by `create_database` in the scope's folder and then opened: two test-only steps apply in one transaction and add two `schema_migrations` rows; a failure in the second (`scope(dir).fail_at` after step 2, and a step whose SQL is invalid) leaves neither step, neither row and an unchanged `sqlite_schema` (spec Edge Cases, several releases skipped); a step that leaves an orphan row fails at the link check and is rolled back; `changes_waiting` is the same after a step that updates collection rows as before; `last_saved_version` is unchanged; `PRAGMA foreign_keys` is on again after success and after failure; a database in another folder, made while the scope is held, gets no extra step (the scope's isolation, research.md §14)
- [ ] T009 Implement `db::upgrade::testing` in `src-tauri/src/db/upgrade.rs` (research.md §14), compiled only with `upgrade-test-steps`: a process-wide registry behind a mutex, keyed by folder; `scope(dir) -> Scope`, whose `extra_steps(&[(&'static str, &'static str)])`, `fail_at(Point)` and `at_point(Point, impl Fn(&Connection) + Send + Sync)` apply to any database whose file is in `dir` (the database, the restore's `.<file>.new` copy), on whichever thread works on it, and are cleared when the `Scope` drops. `Point` covers: before the backup, during the copy, at the stamp, before the backup's rename, after step *n*, at the link check, before the commit. The runner and `services::backups` call `testing::reach(point, database_path, conn)` at each, a no-op that compiles to nothing without the feature. Make `src-tauri/src/main.rs`, `examples/human_seed.rs` and `examples/portable_fixture.rs` refuse to run with `upgrade-test-steps`, as they refuse `test-kdf`. The shared test steps (`PHOTOS_REBUILD`, `HISTORY_UID_FILL`) come in T067
- [ ] T010 Implement `db::upgrade` in `src-tauri/src/db/upgrade.rs`: `pub fn upgrade(conn: &Connection, steps: &[(&str, &str)], marker: Option<&OpenMarker>) -> Result<(), UpgradeError>` doing research.md §7's steps exactly: (1) `PRAGMA foreign_keys = OFF` outside any transaction; (2) `BEGIN IMMEDIATE`; (3) read `app_state.changes_waiting` when the table exists (a new database has none); (4) for each step in order, `execute_batch(sql)` then `INSERT INTO schema_migrations (name, applied_at)`; (5) write the open marker when `marker` is given (an open, not a create); (6) `PRAGMA foreign_key_check`, where any row abandons the upgrade, logging the table and row id, never the data; (7) put `changes_waiting` back; (8) `COMMIT`, then `PRAGMA foreign_keys = ON` on every path, error included. Any error before the commit rolls back. It calls `testing::reach` at T009's points. It never calls `record_saved_version()` (008 FR-015). `UpgradeError` names the failing step or the link check, never row values. Add `pub fn build_steps(database_path: &Path) -> Vec<(&'static str, &'static str)>`, which every caller uses: `MIGRATIONS`, plus, with `upgrade-test-steps`, the extra steps registered for the database's folder (T034 adds the `e2e` step); and `pub fn steps_due(conn, database_path) -> Result<Vec<&'static str>>` (the build's step names after the database's prefix, research.md §3). Remove `apply_migrations` from `src-tauri/src/db/mod.rs`: `create_database` calls `upgrade` with every step and no marker, and the open path calls it for any steps due (a stopgap, replaced by T046 in US3, so the suite keeps passing). No `_with_` sibling (CLAUDE.md memory "Refactor, not workaround"). T008's tests must now pass
- [ ] T011 On Linux, run the whole Rust suite (`scripts/dev-container.sh bash -c 'cd src-tauri && cargo test --features test-kdf'`) and `cargo clippy --all-targets -- -D warnings`; it must pass with the runner in place before any story starts

**Checkpoint**: the runner creates every database; the probes hold on all three OS.

---

## Phase 3: User Story 1 - Records Keep One Identity in Every Copy of a Database (Priority: P1) 🎯 MVP

**Goal**: every insurance policy, photo and document carries a permanent random identifier, on 006's terms, never shown or exported (FR-001 to FR-005).

**Independent Test**: spec.md US1 "Independent Test": create, edit, back up, restore, change the passphrase, delete the newest of each kind and create another; each identifier is unique, unchanged through all of it, the same in the backup and the restored database, and new for the new record.

### Tests for User Story 1 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T012 [US1] Write first, before T013 (it fails until T013–T016 are in): extend `src-tauri/tests/record_identifier_test.rs` for policies, photos and documents (cite `009 US1-1` to `009 US1-4`, `009 FR-001` to `009 FR-005`, `009 SC-001`, `009 SC-002`):
  - two policies with the same insurer and dates (distinct names) get distinct `uid`s; every photo and document created through `ops` has one matching `services::record_id::parse`;
  - a raw `INSERT` with a missing, malformed (uppercase, wrong version digit, wrong variant digit, 35 characters) or already used `uid` is refused, and a raw `UPDATE … SET uid` is refused by `<table>_uid_fixed` with "a record identifier never changes";
  - `uid`s are unchanged by editing a policy, a photo's caption, a document's details, reordering photos (`sort_order`), and by editing, disposing of and reversing the disposition of the owning firearm and accessory;
  - after `make_backup`, the backup's rows have the same `uid`s; after `restore_backup` and `change_passphrase`, the database's rows do;
  - after deleting the highest-`id` policy, photo and document and creating one of each, the new integer `id` may repeat but the new `uid` matches no earlier one;
  - a photo or document may have the same `uid` as a firearm (no cross-kind trigger, FR-005), set by raw insert;
  - `disposition_history`, `firearm_types`, `action_types`, `registration_classes` and `accessory_kinds` have no `uid` column (`PRAGMA table_info`, FR-004);
  - FR-003: serializing every policy, photo and document IPC type returned by `ops` (list, get, create) and an export's rows contains neither the key `uid` nor any of the records' `uid` values

### Implementation for User Story 1

- [ ] T013 [US1] In `src-tauri/src/db/migrations/0001_initial.sql` (edited in place), add to `insurance_policies`, `photos` and `document_attachments`, right after `id`, `uid TEXT NOT NULL UNIQUE CHECK (length(uid) = 36 AND uid GLOB '????????-????-4???-????-????????????' AND substr(uid, 20, 1) IN ('8', '9', 'a', 'b') AND NOT (replace(uid, '-', '') GLOB '*[^0-9a-f]*'))`, laid out exactly as `firearms.uid`'s; and the triggers `insurance_policies_uid_fixed`, `photos_uid_fixed` and `document_attachments_uid_fixed`, each `BEFORE UPDATE OF uid ON <table> WHEN NEW.uid IS NOT OLD.uid BEGIN SELECT RAISE(ABORT, 'a record identifier never changes'); END;`, beside `firearms_uid_fixed`. No `DEFAULT` and no `*_uid_distinct` trigger (research.md §1)
- [ ] T014 [P] [US1] Set `uid` from `services::record_id::generate()` at the one `INSERT INTO insurance_policies` in `src-tauri/src/commands/insurance.rs`'s `ops::create_policy`; it is not added to `InsurancePolicy` or any other IPC type (FR-003)
- [ ] T015 [P] [US1] Set `uid` from `record_id::generate()` at the one `INSERT INTO photos` in `src-tauri/src/commands/photos.rs`; not in `Photo` or any IPC type
- [ ] T016 [P] [US1] Set `uid` from `record_id::generate()` at the one `INSERT INTO document_attachments` in `src-tauri/src/commands/documents.rs`; not in `DocumentAttachment` or any IPC type
- [ ] T017 [US1] Add a shared helper `new_uid()` (wrapping `record_id::generate()`) to `src-tauri/tests/support/mod.rs` and give every direct `INSERT` into the three tables a `uid` with it: `amount_cap_test.rs`, `export_test.rs`, `firearm_lifecycle_test.rs`, `backup_due_tracking_test.rs`, `document_test.rs`, `list_accessories_test.rs`, `photo_test.rs`, `passphrase_change_test.rs`, `performance_test.rs` and `support/preview_support.rs` (find any others with `grep -rln 'INSERT INTO \(photos\|document_attachments\|insurance_policies\)' src-tauri/tests`)
- [ ] T018 [P] [US1] Give the direct `INSERT INTO document_attachments` in `src-tauri/examples/human_seed.rs` a `uid` from `record_id::generate()`; `tests/human_seed_coverage_test.rs` must pass (every seeded column non-null somewhere), by seeding, not by loosening the test
- [ ] T019 [US1] On Linux, run the whole Rust suite with `test-kdf`; T012 must now pass and nothing else fail. Tell the user their development databases must be created again (`0001_initial.sql` edited in place); never touch them

**Checkpoint**: identifiers on every kind #30, #31 and #54 will match; nothing visible changed.

---

## Phase 4: User Story 2 - A Policy Name Means One Policy (Priority: P2)

**Goal**: policy names are unique ignoring A–Z letter case and surrounding spaces, refused at the form's name field, enforced by the database, and import's lookup uses the same comparison (FR-006 to FR-008).

**Independent Test**: spec.md US2 "Independent Test": a second policy named alike is refused; firearm and accessory rows naming policies in other letter case resolve to the one policy, and an unknown name is a row error.

### Tests for User Story 2 (mandatory per constitution)

- [ ] T020 [P] [US2] Extend `src-tauri/tests/insurance_policy_test.rs` (cite `009 US2-1`, `009 FR-007`, `009 FR-008`, `009 SC-003`):
  - beside "Homeowners rider", `create_policy` refuses "Homeowners rider", "homeowners rider ", " HOMEOWNERS RIDER" and "Homeowners Rider" with `VALIDATION_ERROR` and exactly `fieldErrors.name` = "“Homeowners rider” is already the name of another policy. Give this one a different name, such as “Homeowners rider <current year>”." (contracts/tauri-commands.md), and the policy count is unchanged;
  - the same against an expired and a future policy;
  - `update_policy` refuses renaming one policy to another's name in other case, and allows saving a policy under its own name, with its own case changed, or with spaces added;
  - "Église" and "église" can both exist (A–Z only), and "Rider\t" beside "Rider" is allowed (only U+0020 is trimmed, research.md §2);
  - names are stored as typed (" Rider " reads back with its spaces);
  - a raw `INSERT` of a conflicting name is refused by `idx_insurance_policies_name` (FR-007's backstop; this is plan.md "First tasks" 1's third probe, the index under SQLCipher)
- [ ] T021 [P] [US2] Extend `src-tauri/tests/import_export_test.rs` (cite `009 US2-2`, `009 US2-3`, `009 FR-006`): with a policy "Range rider", a firearm row and an accessory row whose `insurance_policy_name` is "  range RIDER " are scheduled under it with their `scheduled_coverage_amount`; a row naming "Rang rider" is the row error `Unknown insurance policy: Rang rider` and nothing from it is saved, for both tables
- [ ] T022 [P] [US2] Extend `src-tauri/tests/query_plan_test.rs`: `EXPLAIN QUERY PLAN` of the form's name check and of import's two lookups each uses `idx_insurance_policies_name`, not a scan
- [ ] T023 [P] [US2] Extend `src/features/insurance/InsurancePolicyForm.test.tsx`: a `VALIDATION_ERROR` with `fieldErrors.name` from save is shown at **Name** in the form's field-error pattern with focus moved there, and no other field changes; the Name hint includes "Each policy needs its own name." (contracts/ui.md §9)
- [ ] T024 [P] [US2] E2E journey in `e2e/specs/value-insurance.e2e.ts`, as a new `describe("009 US2 - A Policy Name Means One Policy")` (its setup: a database with policies; plan.md Test Levels): add policies "InsE2E Homeowners rider" and "InsE2E Collector schedule" with the file's `addPolicy` helper; edit the second and rename it to " inse2e HOMEOWNERS rider " (other letter case, surrounding spaces); press **Save**; expect the error at **Name** naming “InsE2E Homeowners rider” and the policy card still reading "InsE2E Collector schedule"; rename it to "InsE2E Collector schedule 2026" and save; expect that card. Keyboard-first where the file's other journeys are

### Implementation for User Story 2

- [ ] T025 [US2] In `src-tauri/src/db/migrations/0001_initial.sql` (edited in place), add `CREATE UNIQUE INDEX idx_insurance_policies_name ON insurance_policies (trim(name) COLLATE NOCASE);` after the table (data-model.md)
- [ ] T026 [US2] In `src-tauri/src/commands/insurance.rs`, have `ops::create_policy` and `ops::update_policy` look the name up first with `SELECT name FROM insurance_policies WHERE trim(name) = trim(?1) COLLATE NOCASE AND id IS NOT ?2` (`?2` NULL on create) and refuse a match with `VALIDATION_ERROR` and `fieldErrors.name` = "“<other name as stored>” is already the name of another policy. Give this one a different name, such as “<other name> <current year>”." (FR-008). The check runs under the session's mutex with the write, so the index's refusal stays a backstop mapped by `from_db` to `INTERNAL_ERROR`. Names are not trimmed when stored
- [ ] T027 [US2] In `src-tauri/src/commands/import_export.rs`, change both `insurance_policy_name` lookups (the firearm row's, near line 1374, and the accessory row's, near line 1493) from `name = :name COLLATE NOCASE` to `trim(name) = trim(:name) COLLATE NOCASE`; the row error `Unknown insurance policy: <cell>` is unchanged (contracts/spreadsheet-format.md)
- [ ] T028 [P] [US2] Add the Name hint "Each policy needs its own name." to `src/features/insurance/InsurancePolicyForm.tsx` in the form's existing hint pattern (contracts/ui.md §9; carry nothing else, since no other form has names that must be unique)
- [ ] T029 [US2] Give distinct names to any test, seed or E2E setup that creates two policies with the same name compared as above (`grep -rn` for the policy names in `src-tauri/tests`, `src-tauri/examples/human_seed.rs`, `e2e/specs` and `src/**/*.test.tsx`). In `human_seed.rs`'s import samples, change one row's `insurance_policy_name` from "Vault Schedule" to " vault SCHEDULE " so the samples exercise the comparison (CLAUDE.md "Keep the human-testing seed in step")
- [ ] T030 [US2] On Linux, run the whole Rust suite, `npm test` and `npm run build && npm run test:e2e -- --spec e2e/specs/value-insurance.e2e.ts` in the dev container; all must pass

**Checkpoint**: a name finds one policy or none, everywhere it is compared.

---

## Phase 5: User Story 3 - A Beta Tester's Database Survives an Upgrade (Priority: P3)

**Goal**: an older database is upgraded only after the user agrees, with a backup made first, all or nothing; a newer one is refused with directions to the backup; an older backup is upgraded as it is restored (FR-009 to FR-019).

**Independent Test**: spec.md US3 "Independent Test", automated by `upgrade_test.rs` with test-only steps and by the E2E journey with the stored 0.1.0 database.

### First: the last layout change and the stored 0.1.0 database (research.md §6, §12, §14)

These come first because the E2E journey and US4 open the fixture, and the fixture must have the settled layout (spec Assumptions: after US1 and US2).

- [ ] T031 [US3] Test first, in `src-tauri/tests/backup_test.rs` (cite `009 FR-013`; it fails until the column exists): a database made now has `app_state.backup_before_upgrade_to` NULL; a backup whose stamp also sets it (by a raw `UPDATE` on the backup file) has it cleared when opened directly (`lifecycle::open`) and when restored (`restore_backup`). Then, in `src-tauri/src/db/migrations/0001_initial.sql` (edited in place), add `backup_before_upgrade_to TEXT` to `app_state` after `backup_of_name` (data-model.md: "Set only in a backup made before an upgrade … `NULL` everywhere else"), and clear it wherever the backup stamp is cleared: `take_backup_stamp` in `src-tauri/src/session/lifecycle.rs` and the restore's stamp clearing in `src-tauri/src/commands/backups.rs`. The test must now pass. `human_seed_coverage_test` then fails on the new column until T047 seeds a backup made before an upgrade; that is expected, and it is fixed by seeding, not by loosening the test (CLAUDE.md)
- [ ] T032 [US3] Write `src-tauri/examples/release_fixture.rs`, modelled on `portable_fixture.rs`: it refuses to run in a `test-kdf` or `upgrade-test-steps` build; it seeds a database at a path it is given with `human_seed.rs`'s "Main collection" seeding (move that seeding into `src-tauri/examples/support/` so both examples share it, with no change to what `human_seed` makes), with small images and no `--extra` firearms; it then adds a policy draft as pending changes, sets `changes_waiting` and `last_backup_at`, and keys it with the pinned KDF and the passphrase `release fixture 0.1.0`. Run it to write `src-tauri/tests/fixtures/releases/0.1.0.hoplodex`; report its size (aim under 2 MB, research.md §12) and stop and ask if it is over
- [ ] T033 [US3] Create `src-tauri/tests/release_upgrade_test.rs` with `release_fixture_matches_layout`: a copy of the fixture opened with a raw keyed connection (no steps run) has the same `sqlite_schema` and `schema_migrations` names as a database `create_database` makes now; its failure message says to rerun `cargo run --example release_fixture` (until 0.1.0 is released; research.md §12)
- [ ] T034 [US3] Add the `e2e` build's step, test first: add a test to `upgrade_steps_test.rs` (built without `e2e`) that the steps a new database gets are exactly `MIGRATIONS`' names (research.md §14), and make it go red once by appending a step without its `cfg`, then put that back. Then, in `src-tauri/src/db/upgrade.rs`, add `const E2E_STEPS: &[(&str, &str)] = &[("9000_e2e_upgrade_check", "CREATE TABLE e2e_upgrade_check (id INTEGER PRIMARY KEY);")]` behind `cfg(feature = "e2e")`, appended by T010's `build_steps()` after `MIGRATIONS` in an `e2e` build

### Tests for User Story 3 (mandatory per constitution)

- [ ] T035 [P] [US3] Write `src-tauri/tests/upgrade_test.rs`, the question and the answer (cite `009 US3-1`, `009 US3-2`, `009 US3-5`, `009 FR-009` to `009 FR-011`, `009 FR-015`, `009 FR-017`). Each test makes a database, then opens it through `lifecycle::open` under `testing::scope(dir).extra_steps(…)` (one test-only step) so it is older than the build:
  - the open fails with `UPGRADE_NEEDED` and details `{ awaitingId, version: APP_VERSION, backupFolder }`; the file's SHA-256 (through `RawFile`) is unchanged; a second connection's keyed read fails "database is locked"; every collection command and `get_database_status` see no open database;
  - `cancel_upgrade { awaitingId }` drops it: the bytes are unchanged, the lock is released, and no backup was made;
  - `upgrade_database { awaitingId }`: the backup `<name> <date> <time> <id8> before <APP_VERSION>.hoplodex` exists at the backup location before the transaction begins (`scope(dir).at_point` before the commit sees it, with the database's bytes still unchanged); it opens with the passphrase and has `backup_made_at`, `backup_of_name` and `backup_before_upgrade_to = APP_VERSION`, no open marker and no pending changes; the database opens upgraded with the step's `schema_migrations` row, `notes.upgraded = { version, backupPath, atBackupLocation: true }`, every collection row, `uid`, link, setting and pending change unchanged, `changes_waiting` as before and `last_saved_version` unchanged;
  - `rememberPassphrase` saves to the keyring (mock keyring) only after the upgrade succeeds, never after a cancel or failure;
  - US3-5: with automatic backups off and a regular backup already made today, the backup before the upgrade is still made, and `changes_waiting`, `last_backup_at` and `has_backup_made_on` are as before
- [ ] T036 [P] [US3] Add to `src-tauri/tests/upgrade_test.rs` the location problems (cite `009 FR-012`): a file where the backup folder would be (`missing`), a read-only folder (`notWritable`, `#[cfg(unix)]`), and too little room (with the free-space seam `backup_test.rs` already uses for `INSUFFICIENT_SPACE`; never by filling a real volume, CLAUDE.md memory "No filling real volumes") each refuse with the contract's code and `folder`, and the database is still waiting (the same `awaitingId` upgrades later). A chosen `backupFolder` then gets the backup and `atBackupLocation: false`; one that resolves to the backup location gets `atBackupLocation: true`; a relative path, a missing folder, a file, or an unwritable folder is refused the same way and is never created; an existing file of the backup's name is never overwritten (exclusive create); an I/O failure during the copy (`scope(dir).fail_at` during the copy) is `UPGRADE_BACKUP_FAILED` with the folder, no `.partial` left, and the database still waiting
- [ ] T037 [P] [US3] Add to `src-tauri/tests/upgrade_test.rs` failure and interruption at each point (cite `009 US3-3`, `009 FR-015`, `009 FR-016`, `009 SC-005`):
  - in the backup phase, `scope(dir).fail_at` before the backup, at the stamp and before the rename each return `UPGRADE_BACKUP_FAILED` with the folder; no `.partial` is left; the database is still waiting (contracts/tauri-commands.md, as T036's copy failure), and the same `awaitingId` then upgrades;
  - from the first step on, `scope(dir).fail_at` after each of two test-only steps, at the link check (an orphan planted by a step) and before the commit each return `UPGRADE_FAILED` with `backupPath` and `atBackupLocation`; the database's rows and `sqlite_schema` equal those before; the finished backup is still there; the slot is empty; the next open is `UPGRADE_NEEDED` again;
  - interruption: with `cache_size = 1`, at each point inside the transaction `scope(dir).at_point` copies the database through `RawFile` and its journal to another folder; opening the copy rolls back to the earlier rows and layout and is `UPGRADE_NEEDED`; a backup finished before the point is still there under its ` before ` name; a `.partial` left at a point in the copy is removed by the launch sweep with no chooser notice;
  - cancel: `cancel_upgrade` between backup chunks removes the partial file, `upgrade_database` returns `OPERATION_STOPPED` with `operation: "upgrade"`, the file's SHA-256 is unchanged, and the slot is empty; a shutdown (`close_immediate`) during the backup does the same; `cancel_upgrade` once the `upgrading` phase has begun is ignored and the upgrade completes
- [ ] T038 [P] [US3] Add to `src-tauri/tests/upgrade_test.rs` the slot's life and the edge cases (cite spec Edge Cases, `009 research.md §4`, `§8`):
  - a waiting database is dropped by another `open_database`, `create_database` or `restore_backup` beginning, by `quit_application`, and by sleep, screen lock and shutdown `SystemEvent`s; after each, `upgrade_database` with the old `awaitingId` is `DATABASE_CLOSED` and `cancel_upgrade` does nothing;
  - a database marked open by another computer is `DATABASE_OPEN_ELSEWHERE` first, and the take-over resend reaches `UPGRADE_NEEDED`; a marker written by another computer during the question (through a test hook on the held connection) makes `upgrade_database` refuse with `DATABASE_OPEN_ELSEWHERE`, nothing written, the slot empty;
  - a backup opened directly that needs an upgrade: Cancel leaves its bytes unchanged and its stamp in place; Upgrade gives it a new `database_id`, its backup before the upgrade is named with the new id's `id8`, stamped with the new id and written to the backup folder its own settings name, and the `opened_backup` note is shown;
  - two test-only steps upgrade in one go, and a failure in the second leaves neither (spec Edge Cases, several releases skipped)
- [ ] T039 [P] [US3] Extend `src-tauri/tests/backup_test.rs` (cite `009 FR-013`, `009 FR-014`): the ` before <version>` name for versions `0.2.0`, `1.0.0-beta.1` and `1.0.0+build.5`; `list_backups` gives `beforeUpgradeTo` for those and `null` for regular ones, including for a database named "Guns before 1.0.0"; rotation with keep counts 1 and 3 keeps the newest before-upgrade backup outside the count and rotates older ones normally; `has_backup_made_on` ignores before-upgrade backups; `delete_all_backups` removes them; moving the backup location moves them
- [ ] T040 [P] [US3] Extend `src-tauri/tests/restore_test.rs` (cite `009 FR-019`): `check_backup` gives `upgradeTo: APP_VERSION` for an older backup (made older by `testing::scope(dir).extra_steps(…)`) and `null` for a current one, `PASSPHRASE_INCORRECT` for a wrong passphrase and `DATABASE_NEWER_VERSION` for a newer one, and leaves the backup's bytes unchanged; with `databasePath`, it applies the damaged restore's authorization (#62); without it, it needs the session header. `restore_backup` of an older backup without `upgrade` is `UPGRADE_NEEDED { version }` with nothing copied, closed or written; with `upgrade: true` it restores and upgrades, `notes.upgraded.backupPath` is the backup restored from, that backup is unchanged and still rotates as a regular one, and `restore:progress` has an `upgrading` phase between `checking` and `savingCurrent`; a failure forced in the copy's upgrade is `UPGRADE_FAILED`, `.<file>.new` is removed, the database is as it was, and the "before restoring" backup exists; the chooser's damaged-database restore upgrades the same way
- [ ] T041 [P] [US3] Extend `src-tauri/tests/database_open_test.rs` (cite `009 US3-4`, `009 FR-018`): a database with an unknown step is `DATABASE_NEWER_VERSION`, unchanged, with `details.backupFolder` from its recent entry, and without `backupFolder` when it has none. Extend `src-tauri/tests/machine_settings_test.rs` (cite `009 research.md §15`): a `machine.json` with an unknown notice kind, an unknown field and an unknown `documentOpening` loads with its recent list, the unknown notice dropped and `documentOpening` the default; an `UnfinishedBackup` without `beforeUpgrade` reads as `false`; the launch sweep removes a `beforeUpgrade` partial with no notice and keeps today's notice for a regular one. Extend `src-tauri/tests/session_scope_test.rs` so `upgrade_database`, `cancel_upgrade` and `check_backup` are in `UNSCOPED_COMMANDS`
- [ ] T042 [P] [US3] Vitest (cite contracts/ui.md §1–§4, §8): in `src/features/databases/DatabaseChooser.test.tsx`, `UPGRADE_NEEDED` from open (and from the take-over resend) shows §1's question with its exact text and **Upgrade** focused; Enter calls `upgradeDatabase`; Cancel, Escape and `system:clear-passphrase-fields` call `cancelUpgrade` and leave the row selected with no message; the row's busy state shows while the question is up; each of §3's reasons shows its text with **Choose a folder…** focused; choosing a folder (the picker mocked) calls `upgradeDatabase` with `backupFolder`; a refusal there shows the dialog again naming that folder; dismissing the picker leaves it up; `UPGRADE_FAILED` shows §4's text in the row; `DATABASE_NEWER_VERSION` shows §8's text with and without `backupFolder`. In a new `src/features/databases/UpgradeDialogs.test.tsx` (fake timers): the progress dialog shows at once on `showNow` and otherwise only after a second; `backingUp` shows its line, a `ProgressBar` and **Cancel**, which shows "Stopping…"; `upgrading` shows its line, no buttons, and Escape does nothing; there is no close button; the phase line has `aria-live="polite"`
- [ ] T043 [P] [US3] Vitest (cite contracts/ui.md §5–§7): in `src/features/databases/RestoreBackupDialog.test.tsx`, Restore calls `checkBackup` before any confirmation; a wrong passphrase shows at the field with no confirmation; `DATABASE_NEWER_VERSION` shows above the footer; with `upgradeTo` the confirmation adds §6's sentence and confirming sends `upgrade: true`; the progress shows "Upgrading the restored copy…"; `UPGRADE_FAILED` shows its text with the dialog left open; a before-upgrade backup is listed as "<date> — <size> — before upgrading to HoploDex 0.2.0". In `src/features/databases/DatabaseNotes.test.tsx`: §5's upgraded note, with the extra sentence when `atBackupLocation` is false, and the restore variant after the restored note; §7's opened-backup note when `beforeUpgradeTo` is set, without the "may be removed" clause; dismissing sends `"upgraded"`. In `databasesService.test.ts`: the four new wrappers send their commands and listen to `upgrade:progress`
- [ ] T044 [P] [US3] E2E journey in `e2e/specs/databases.e2e.ts`, as a new `describe("009 US3 - A Beta Tester's Database Survives an Upgrade")` (plan.md Test Levels): copy the stored 0.1.0 database into the sandbox with T060's helper; record its SHA-256; open it with `release fixture 0.1.0`; expect the question with **Upgrade** focused; press **Cancel**; expect the chooser and an unchanged SHA-256; open it again and press **Enter**; expect the collection with the upgraded note; open **Restore from a backup** and expect the backup listed "before upgrading to HoploDex <version>". Then a second copy with a file where its "HoploDex backups" folder would be: **Upgrade** shows "Save the backup somewhere else?"; **Cancel** returns to the chooser and the file's SHA-256 is unchanged. Choosing a folder isn't driven (the native picker; covered by T036 and T042)

### Implementation for User Story 3

- [ ] T045 [P] [US3] In `src-tauri/src/models/database.rs` and `src/features/databases/types.ts`, add (contracts/tauri-commands.md "Shared types"; data-model.md): `OperationKind::Upgrade` (`"upgrade"`); `BackupInfo.beforeUpgradeTo: string | null`; `OpenedBackupNote.beforeUpgradeTo: string | null`, filled by `take_backup_stamp` in `src-tauri/src/session/lifecycle.rs` from T031's column; `UpgradedNote { version, backupPath, atBackupLocation }`; `DatabaseNotes.upgraded: UpgradedNote | null`; the `upgrade:progress` payload `{ phase: "backingUp" | "upgrading"; processed; total; showNow }`; `restore:progress`'s `"upgrading"` phase. In `src-tauri/src/commands/error.rs`, add `UPGRADE_NEEDED` (`{ awaitingId, version, backupFolder }` from an open, `{ version }` from a restore), `UPGRADE_FAILED` (`{ backupPath, atBackupLocation }`) and `UPGRADE_BACKUP_FAILED` (`{ folder }`), and `DATABASE_NEWER_VERSION`'s optional `backupFolder`
- [ ] T046 [US3] In `src-tauri/src/db/mod.rs`, make `open_database` return `Opened::Ready(Connection)` or `Opened::UpgradeDue { conn, steps_due, marker_seen }` (data-model.md): after today's read-only steps 1–4, when `steps_due` is non-empty it runs `BEGIN EXCLUSIVE; COMMIT` and returns without writing (research.md §4 step 2), replacing T010's stopgap. Every caller follows (`session/lifecycle.rs`, `commands/backups.rs`, the tests including `portability_test.rs`, the examples); `check_copy` is unchanged
- [ ] T047 [US3] In `src-tauri/src/services/backups.rs` (research.md §6; data-model.md "Backup files"): add `BackupKind::{Regular, BeforeUpgrade { to_version }}` to `BackupJob`, passed by every caller. For `BeforeUpgrade`: the name `<name> <YYYY-MM-DD HHMMSS> <id8> before <version>.hoplodex`; the stamp also sets `backup_before_upgrade_to`; it is made whether backups are on or off and ignores the once-a-day rule; it touches neither `changes_waiting` nor `last_backup_at`; its `UnfinishedBackup` has `beforeUpgrade: true`. `made_at_by_name` first strips a trailing ` before <version>` where `<version>` matches `[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?`, and `list_backups` fills `beforeUpgradeTo` from it. `rotate` sets aside the newest before-upgrade backup, then keeps the newest `keep` of the rest; `has_backup_made_on` counts regular backups only. Add a target folder chosen by the user: it must be absolute, an existing directory, writable and have room, is never created, and the file is created exclusively (#63's `scratch::create`); a folder that is `same_path` as the backup location is the backup location. Make the human-testing seed's newest backup of the main database (`src-tauri/examples/human_seed.rs`, or `examples/support/` if T032 moved the backup seeding there) a `BackupKind::BeforeUpgrade { to_version: APP_VERSION }` backup, made a minute after its two regular ones, so `human_seed_coverage_test` finds `backup_before_upgrade_to` set and a person testing by hand sees the "before upgrading" label; the test must pass again
- [ ] T048 [P] [US3] In `src-tauri/src/services/machine_settings.rs` (research.md §15; data-model.md "`machine.json`"): add `UnfinishedBackup.beforeUpgrade: bool` with `serde(default)`; make the launch sweep remove a `beforeUpgrade` partial securely with no chooser notice; read `notices` entry by entry, dropping and logging (without contents) one that doesn't parse; read an unknown `documentOpening` as the default
- [ ] T049 [US3] In `src-tauri/src/session/mod.rs`, add `SessionInner.awaiting_upgrade: Mutex<Option<AwaitingUpgrade>>` with data-model.md's fields (`id` from the session-id counter, `conn`, `path`, `name`, `database_id`, `steps_due`, `marker_seen`, `backup_stamp`, `save_passphrase: Option<Passphrase>` zeroized on drop, `backup_folder`); drop it when any open, create or restore begins and on `quit_application`. No collection, scoped or unscoped command can reach it (research.md §4)
- [ ] T050 [US3] In `src-tauri/src/session/lifecycle.rs`: on `Opened::UpgradeDue`, put the connection in the slot (keeping a passphrase to be saved or refreshed in the keyring only, 003 FR-017, FR-018) and fail with `UPGRADE_NEEDED` and its details; drop the slot on sleep, screen lock and shutdown with the existing `system:clear-passphrase-fields`; a shutdown or signal during an upgrade's backup stops the operation as `close_immediate` stops any
- [ ] T051 [US3] Implement `src-tauri/src/session/upgrade.rs` (research.md §4 step 5, §8; contracts/tauri-commands.md `upgrade_database` "Order"): take the waiting database by `awaitingId` (`DATABASE_CLOSED` otherwise); begin the `Upgrade` operation; re-read the open marker (`DATABASE_OPEN_ELSEWHERE` if another computer's is there now; dropped); for a backup opened directly, choose its new `database_id` first; check the backup folder or the chosen one and its room (refusals put the database back in the slot); make the `BeforeUpgrade` backup with `upgrade:progress` `backingUp` events, the first with `showNow` from the backup's estimate, stoppable between chunks (`OPERATION_STOPPED`, partial removed, dropped); an I/O failure is `UPGRADE_BACKUP_FAILED` and puts the database back; then emit `upgrading` (`processed = total = 0`, and after each step), mark the operation unstoppable, and run `db::upgrade` with the open marker and, for a backup opened directly, the stamp-taking with the new id inside the same transaction; a failure is `UPGRADE_FAILED { backupPath, atBackupLocation }` (or `DISK_FULL`), rolled back, dropped; on success continue as every open does (`prepare`, `install`, the recent list, the saved passphrase) with `notes.upgraded`. In an `e2e` build only, read `HOPLODEX_E2E_UPGRADE_PHASE_SECONDS` as `session/idle.rs` reads `HOPLODEX_E2E_IDLE_MINUTE_SECONDS`: when set, wait that long before the backup's first chunk (stoppable, as a chunk is) and before the upgrade's commit, so the screenshot walk and quickstart.md M3 can see each phase (research.md §14). `cancel_upgrade` drops a waiting database, stops a backup in progress, and does nothing once upgrading or for a stale id
- [ ] T052 [US3] In `src-tauri/src/commands/databases.rs`: add the unscoped `async` commands `upgrade_database { awaitingId, backupFolder? }` and `cancel_upgrade { awaitingId }`, thin over `session/upgrade.rs`; `dismiss_note` accepts `"upgraded"` (session-only, like `"openedBackup"`); `open_database`'s `DATABASE_NEWER_VERSION` carries `backupFolder` from the recent entry when there is one
- [ ] T053 [US3] In `src-tauri/src/commands/backups.rs`: add the unscoped `check_backup { backupPath, passphrase, databasePath? }` → `{ upgradeTo }` (contracts/tauri-commands.md), opening the backup read-only and wiping the passphrase before returning, with `databasePath` authorized as the damaged restore is (#62) and otherwise requiring the session header (`session::scoped::Target`); give `restore_backup` `upgrade: bool`: without it an older backup is `UPGRADE_NEEDED { version }` before anything is copied; with it the `.<file>.new` copy is upgraded by `db::upgrade` (no marker) after `check_copy` and the stamp and before the swap, with the `upgrading` phase, a failure removing the copy and returning `UPGRADE_FAILED`, and success setting `notes.upgraded` with the restored-from path; the chooser's damaged restore follows
- [ ] T054 [US3] Register `upgrade_database`, `cancel_upgrade` and `check_backup` in `src-tauri/src/main.rs`'s `generate_handler!`, in `src-tauri/src/commands_list.rs`'s `COMMANDS` and `UNSCOPED_COMMANDS`, and as `allow-upgrade-database`, `allow-cancel-upgrade` and `allow-check-backup` in `src-tauri/capabilities/default.json` for the main web view only; `acl_manifest_test` and `session_scope_test` must pass
- [ ] T055 [P] [US3] In `src/features/databases/databasesService.ts`, add `upgradeDatabase(awaitingId, backupFolder?)`, `cancelUpgrade(awaitingId)`, `checkBackup(backupPath, passphrase, databasePath?)` (through `tauriClient`'s `invoke`, as unscoped commands; `checkBackup` through the scope when no `databasePath` is given, as `restoreBackup` does) and `onUpgradeProgress(listener)`; `restoreBackup` gains `upgrade`
- [ ] T056 [US3] Implement `src/features/databases/UpgradeDialogs.tsx` (contracts/ui.md §1–§3): the question (a non-destructive `ConfirmDialog`, **Upgrade** focused, its three-sentence description); the progress `Dialog` (shown at once on `showNow` or after a second, the `backingUp` line with `ProgressBar` and **Cancel** → "Stopping…", the `upgrading` line with an indeterminate bar or a step count and no buttons, Escape inert, no close button, the phase line `aria-live="polite"`); and "Save the backup somewhere else?" (§3's text per reason, **Choose a folder…** focused, the folder picker the settings dialog uses, **Cancel**). Every dialog is a `<form>` with a `type="submit"` primary (One Enter rule)
- [ ] T057 [US3] In `src/features/databases/DatabaseChooser.tsx`: on `UPGRADE_NEEDED` from open or from the take-over resend, show T056's question with the row busy; wire Upgrade, Cancel, `system:clear-passphrase-fields` (closes as Cancel), the progress and the folder dialog's retry; on success continue as an open does; show §4's `UPGRADE_FAILED` text in the row's open-failure slot and §8's `DATABASE_NEWER_VERSION` text
- [ ] T058 [P] [US3] In `src/features/databases/RestoreBackupDialog.tsx` (contracts/ui.md §6, §7): call `checkBackup` on **Restore** with the button busy; show its refusals as §6 says; add the upgrade sentence to the confirmation when `upgradeTo` is set and send `upgrade: true`; show "Upgrading the restored copy…" for the `upgrading` phase and `UPGRADE_FAILED`'s text with the dialog left open; label before-upgrade backups in the list. Keep `useShownOnceLoaded` (#116)
- [ ] T059 [P] [US3] In `src/features/databases/DatabaseNotes.tsx` (contracts/ui.md §5, §7): the upgraded note and its two variants, dismissible with `"upgraded"`; the opened-backup note's `beforeUpgradeTo` wording
- [ ] T060 [P] [US3] In `e2e/support/sandbox.ts`, add a helper that copies `src-tauri/tests/fixtures/releases/0.1.0.hoplodex` into a sandbox folder under a given name and returns its path and SHA-256, and an `upgradePhaseSeconds` setting in `applySessionSettings` that sets `HOPLODEX_E2E_UPGRADE_PHASE_SECONDS` as `idleMinuteSeconds` sets its variable, with tests for both in `e2e/support/sandbox.test.ts`
- [ ] T061 [US3] Add `--older` to `scripts/human-testing.sh` and the seed (`src-tauri/examples/human_seed.rs`): copy the stored 0.1.0 database to `.human-testing/Older/Older collection.hoplodex` and add "Older collection" to the sandbox's recent list; document it in the script's header (quickstart.md Setup S1)
- [ ] T062 [US3] On Linux, run the whole Rust suite, `npm test`, and `npm run build && npm run test:e2e -- --spec e2e/specs/databases.e2e.ts` in the dev container; all must pass

**Checkpoint**: an older database is upgraded only after the question, with its backup, all or nothing; Story 3's two 0.1.0 parts (FR-013's naming, FR-018's wording) are in.

---

## Phase 6: User Story 4 - The Data Layout Can Still Change After Release (Priority: P4)

**Goal**: rebuilds work and keep every link, released steps can't change unnoticed, and the stored 0.1.0 database is opened, upgraded and compared value by value in every test run (FR-020 to FR-023).

**Independent Test**: spec.md US4 "Independent Test": the test-only rebuild over a full database, a doctored manifest, and the stored 0.1.0 database, each in `cargo test`.

### Tests for User Story 4 (mandatory per constitution)

- [ ] T063 [P] [US4] Add to `src-tauri/tests/upgrade_steps_test.rs` the tests of the shared rebuild steps (cite `009 US4-1`, `009 FR-020`, `009 research.md §7`, `§14`), written first: they fail to compile until T067 adds `testing::PHOTOS_REBUILD` and `testing::HISTORY_UID_FILL`. `PHOTOS_REBUILD`, applied with `testing::scope(dir).extra_steps(…)` over a database with firearms and accessories whose `thumbnail_photo_id` names photos, keeps every photo row, `uid`, owner and thumbnail link, changes the `CHECK`, and recreates every index and trigger of `photos` (compared in `sqlite_schema`); the same step plus a planted orphan abandons the upgrade at the link check, rolled back; `HISTORY_UID_FILL` gives every `disposition_history` row a distinct value; 10,000 values of research.md §7's expression pass 006's `CHECK` and `record_id::parse`
- [ ] T064 [P] [US4] Add to `src-tauri/tests/upgrade_steps_test.rs` the step rules (cite `009 FR-022`, `009 research.md §13`): no step in `MIGRATIONS` (or `E2E_STEPS`) contains `BEGIN`, `COMMIT`, `ROLLBACK`, `SAVEPOINT`, `RELEASE`, `PRAGMA foreign_keys`, `VACUUM`, `ATTACH` or `DETACH` as a statement (the scan also fails on a doctored step string, so it can go red); a new database has every column of research.md §13's list (`schema_migrations.name`; `app_state`'s `id`, `database_id`, `open_machine_id`, `open_machine_name`, `open_since`, `changes_waiting`, `backup_made_at`, `backup_of_name`, `backup_before_upgrade_to`; `collection_settings.backup_location`; the table `pending_changes`)
- [ ] T065 [P] [US4] Write `src-tauri/tests/released_migrations_test.rs` (cite `009 US4-2`, `009 FR-021`, `009 SC-006`): a checker that takes the manifest's text, the build's steps and a fixture reader, and returns every problem, run against the real `released.txt` (empty today, so it passes) and against doctored inputs that must each fail: a listed step whose bytes changed, a listed step missing from `MIGRATIONS`, one renamed, released steps out of order or not the list's first entries, a listed fixture whose bytes changed, a malformed line. Hash with `sha2`. Each failure message names CLAUDE.md's rule
- [ ] T066 [P] [US4] Extend `src-tauri/tests/release_upgrade_test.rs` (cite `009 US4-3`, `009 FR-023`, `009 SC-004`, `009 research.md §12`): for each stored release database (today `0.1.0.hoplodex`), read every table's rows from one copy with a raw keyed connection (blobs as SHA-256; `app_state`'s marker fields and `pending_changes` kept apart); open another copy through `lifecycle::open` and the upgrade's ops under `testing::scope(dir).extra_steps(&[PHOTOS_REBUILD, HISTORY_UID_FILL])` (T067); check the backup before the upgrade is at the backup location with its ` before ` name and stamp and its collection rows equal the first copy's; check the upgraded rows equal the first copy's after each step's declared change (a per-step function in the test, empty for released steps); and check that every table in the fixture has a row and every column a non-null value somewhere ("holds every kind" can't erode), except the columns a closed database that isn't a backup always has NULL, listed in the test with that reason: `app_state`'s `open_machine_id`, `open_machine_name`, `open_since`, `backup_made_at`, `backup_of_name` and `backup_before_upgrade_to` (the backup's stamp is checked on the backup instead). Also check `uid`s survive (`009 US1-3`)

### Implementation for User Story 4

- [ ] T067 [US4] Add to `db::upgrade::testing` (`src-tauri/src/db/upgrade.rs`) the shared test steps `PHOTOS_REBUILD` and `HISTORY_UID_FILL` (research.md §14), written in research.md §7's rebuild order with its fill expression, so T063, T066 and T072 use one text; T063 must then pass. Document in `src-tauri/src/db/upgrade.rs`'s module doc research.md §13's list of what an older layout must keep and why, and research.md §7's rules (pure SQL; the forbidden statements; the rebuild order; never rename `x` away first; the v4 fill expression; depending only on the starting layout)
- [ ] T068 [US4] Fill `src-tauri/src/db/migrations/README.md` with research.md §7's step rules, the rebuild order as a worked example (adding `location` to `pending_changes.kind`, #16), the fill expression, and how `released.txt` is updated at a release's tag (#19)
- [ ] T069 [US4] Change CLAUDE.md's "Schema changes edit the existing migrations in place" convention to research.md §11's rule: in place until 0.1.0 is tagged; from then on every change is a new step in `db/migrations/`, released ones never change (`released.txt`, `tests/released_migrations_test.rs`), and each new step is covered by `tests/release_upgrade_test.rs`; rerun `cargo run --example release_fixture` whenever the layout changes before the tag
- [ ] T070 [P] [US4] Add a "Changing the data layout" section to `DEVELOPMENT.md` (research.md §7, §11, §12): how to write a step and a rebuild, the test-only steps and the `upgrade-test-steps` feature, `released.txt`, the release fixture and `examples/release_fixture.rs`, `HOPLODEX_E2E_UPGRADE_PHASE_SECONDS`, and `human-testing.sh --older`. Add `--features upgrade-test-steps` to the "Test" section's release `performance_test` command, saying why (it can't take `test-kdf`)
- [ ] T071 [US4] On Linux, run the whole Rust suite in the dev container; T063–T067 must pass (T065's doctored inputs are the red check for the manifest, research.md §11)

**Checkpoint**: upgrades are tested against the stored release database and rebuilds; released steps are guarded.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Performance, screenshots, amendments, documentation, cross-cutting checks, the gates on every OS, the manual checks, and the pull request's notes.

### Performance (constitution IV, research.md §16)

- [ ] T072 Extend `src-tauri/tests/performance_test.rs` (cite `009 SC-007`), which is built in release with `upgrade-test-steps` (research.md §14, §16): an open with nothing due stays within 003 SC-003's 1 s at 10,000 items; on a 10,000-item database made older by `testing::scope(dir).extra_steps(&[PHOTOS_REBUILD])`, the first `upgrade:progress` arrives within 1 s of `upgrade_database` being called; the backup's and the rebuild's times are recorded
- [ ] T073 On Linux, run the release `performance_test` (as DEVELOPMENT.md runs it, now with `--features upgrade-test-steps`; quickstart.md) and record the figures for the PR: the open with nothing due, the first progress event, the backup and the rebuild at 10,000 items, and that search ≤ 500 ms and actions ≤ 1 s still hold; fix anything over budget
- [ ] T074 [P] On macOS, in the macOS 26 VM, run the release `performance_test` as T073 and record the same figures
- [ ] T075 [P] On Windows, on the Windows test machine, run the release `performance_test` as T073 and record the same figures

### Screens, docs and checks

- [ ] T076 [P] Add to the walk in `e2e/screenshots/screens.e2e.ts`, continuing its numbering, in light and dark, contracts/ui.md §10's screens: the upgrade question (from the stored 0.1.0 database, via T060's helper); the progress dialog in each phase; "Save the backup somewhere else?" with the backup location missing; the upgraded note; the restore confirmation with the upgrade sentence; the policy form with the duplicate-name error. The walk sets T060's `upgradePhaseSeconds` (and relaunches) for the upgrade's screens so each phase stays up long enough to capture. Run `scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'` and keep the before/after pairs for the PR
- [ ] T077 [P] Add a one-line `_Amended by [spec 009](../009-upgrade-readiness/<file>.md): …_` note at each amended anchor (plan.md "Structure Decision"): `specs/001-firearms-inventory/` FR-019, FR-020 and FR-027 (unique names, the lookup's comparison); `specs/003-database-protection-management/` FR-014 (the refusal's wording), FR-025 (rotation and the once-a-day rule), FR-028 (restoring an older backup), and its `contracts/database-file.md` (backup names, the in-place upgrade), `contracts/tauri-commands.md` (`open_database`, `restore_backup`, `list_backups`, `dismiss_note`, the errors) and `contracts/ui-databases.md` §1, §9 and §10; `specs/006-accessory-links/` FR-019 (the identifier extended to policies, photos and documents)
- [ ] T078 [P] Update `CLAUDE.md`'s Architecture section for 009: `db::upgrade` (the one runner; `Opened`; `build_steps()`; the `e2e` step and `HOPLODEX_E2E_UPGRADE_PHASE_SECONDS`; `db::upgrade::testing` only with the `upgrade-test-steps` feature, which `test-kdf` turns on, the release `performance_test` needs, and `custom-protocol` and `e2e` refuse); the session's `awaiting_upgrade` slot and `session/upgrade.rs`; `BackupKind` and the ` before <version>` names; the three new unscoped commands in the `UNSCOPED_COMMANDS` list; the codes `UPGRADE_NEEDED`, `UPGRADE_FAILED`, `UPGRADE_BACKUP_FAILED`; `uid` on policies, photos and documents in the `services::record_id` sentence; the policy name rule
- [ ] T079 Check the cross-cutting rules in the code and fix anything found: no `apply_migrations` left (`grep -rn apply_migrations src-tauri`); no `uid` in any IPC type or `src/` type (`grep -rn '\buid\b' src src-tauri/src/models`); no new `.sql` file in `src-tauri/src/db/migrations`; every command in `COMMANDS` and the capability (`acl_manifest_test`); no `_with_upgrade` or other sibling variants; no `std::fs` read of an open database's bytes in the new code (`db/raw_file.rs` only); every new citation names its spec (`node scripts/citations.mjs`); `cargo check --features upgrade-test-steps,custom-protocol` and `--features upgrade-test-steps,e2e` both fail to compile; the app and every example refuse to run with `upgrade-test-steps`
- [ ] T080 On Linux, run the full gates through `scripts/dev-container.sh`: `cargo test --features test-kdf`, `npm test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `npm run lint`, `npm run format:check`, `npm run audit`, and `npm run build && npm run test:e2e` one spec file at a time (every file in `e2e/specs`); fix every failure
- [ ] T081 On macOS, in the macOS 26 VM, run the build, `cargo test --features test-kdf`, `npm test`, lint, the audits, every E2E spec file and the screenshot walk; fix every failure and record the result
- [ ] T082 On Windows, on the Windows test machine, run the build, `cargo test --features test-kdf`, `npm test`, lint, the audits, every E2E spec file and the screenshot walk; fix every failure and record the result
- [ ] T083 Check quickstart.md end to end (every scenario-map row names a test that exists under that name; update the map where a name changed); tell the user their development databases must be created again; run `/security-review` and save its result under `.git` (CLAUDE.md memory "Security review before PR"); draft the PR notes: before/after screenshots from T076; how the persistence and security changes meet Security & Data Handling and User Privacy (research.md §17; no cipher change); the performance figures of T073–T075, and that the interface's responsiveness during an upgrade (SC-007) rests on `upgrade_database` being `async`, with no test measuring it; the residual risks of research.md §19; the fixture's size

### Manual checks (best effort before a release, not merge gates)

- [ ] T084 [P] On Linux, do quickstart.md's manual check **M1** (the round trip by keyboard, on Older collection, passphrase `release fixture 0.1.0`), and record the result for the PR
- [ ] T085 [P] On macOS, do quickstart.md's manual check **M1**, and record the result for the PR
- [ ] T086 [P] On Linux, do quickstart.md's manual check **M2** (the backup location can't be written, on Older collection, passphrase `release fixture 0.1.0`), and record the result for the PR
- [ ] T087 [P] On Linux, do quickstart.md's manual check **M3** with Orca, and record the result for the PR
- [ ] T088 [P] On macOS, do quickstart.md's manual check **M3** with VoiceOver, and record the result for the PR

### Close-out (at pull request time)

Done when the pull request is opened, not before (CLAUDE.md; plan.md "Close-out (at pull request time)"). Check each issue's state first (`gh issue view <n> --json state`) and don't comment on a closed one unless the owner asks.

- [ ] T089 Write the pull request body with "Fixes #53, fixes #115" (one keyword per issue), `/security-review`'s result (T083), research.md §17's data-handling notes, the performance note and the before/after screenshots (the repository's hook requires the review); after opening it, check `closingIssuesReferences` names both issues
- [ ] T090 [P] On issue **#53**, comment what was settled: `uid` on `insurance_policies`, `photos` and `document_attachments` on 006's terms; none on `disposition_history` or the built-in lists; uniqueness across kinds only between firearms and accessories; the tests that cover it (`record_identifier_test.rs`, `release_upgrade_test.rs`)
- [ ] T091 [P] On issue **#115**, comment item by item: released steps frozen by `released.txt`, its test and the CLAUDE.md rule; the backup before upgrading and its rotation; one transaction with foreign keys off and the link check; the stored 0.1.0 database and the tests that open it
- [ ] T092 [P] On issue **#54**, comment that names are now unique (A–Z case and surrounding spaces ignored) and rows keep naming their policy by name; update its "Identity and matching" question: names are unique and `uid` exists, so matching an imported policy row is the open part
- [ ] T093 [P] On issue **#24**, comment that the 0.x upgrade machinery exists (frozen steps, the backup, the question, stored release databases); the post-1.0 policy stays there (breaking changes, a fuller dialog, squashing the 0.x steps, showing the last-saved version)
- [ ] T094 [P] On issue **#19**, comment that tagging a release must add its steps' and stored database's lines to `released.txt`, add the next stored database once a step exists, and run `release_upgrade_test.rs`; 0.1.0 is the first
- [ ] T095 [P] On issue **#16**, comment that adding `location` to `pending_changes.kind` is possible after 0.1.0 as a rebuild step, pointing to `src-tauri/src/db/migrations/README.md`'s worked example
- [ ] T096 [P] On issues **#30** and **#31**, comment that policies, photos and documents now carry identifiers that hold across copies of a database
- [ ] T097 [P] On issue **#118**, comment that letting policies own documents is, after 0.1.0, a rebuild of `document_attachments` (its owner `CHECK`), which the runner now supports

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: depends on Setup; BLOCKS every story. T003 → T004 → T005–T007 (the probes must hold on all three OS, or the owner decides, before T008); T008 (tests, red) → T009 → T010 → T011.
- **US1 (Phase 3)**: depends on Foundational only. The MVP.
- **US2 (Phase 4)**: depends on Foundational only; it can run alongside US1 except for `0001_initial.sql` (T013 and T025 edit different tables, but commit them one after the other).
- **US3 (Phase 5)**: depends on Foundational, and its fixture (T031 → T032 → T033) on US1 and US2 being done, since the stored 0.1.0 database must have the settled layout (spec Assumptions). Its tests and implementation that don't open the fixture (T035–T043, T045–T059) need only Foundational and T031.
- **US4 (Phase 6)**: depends on Foundational, US3's T032–T034 (the fixture and `build_steps()`) and T046 (`Opened`, which T066 drives).
- **Polish (Phase 7)**: depends on every story; the close-out waits for the pull request.

### Same-file order across stories

- `src-tauri/src/db/migrations/0001_initial.sql`: T013 (US1) → T025 (US2) → T031 (US3), then T032 regenerates the fixture.
- `src-tauri/src/db/upgrade.rs`: T009 → T010 → T034 (US3) → T067 (US4).
- `src-tauri/src/db/mod.rs`: T010 → T046 (US3).
- `src-tauri/tests/upgrade_steps_test.rs`: T003 → T004 → T008 → T034 → T063 → T064.
- `src-tauri/tests/upgrade_test.rs`: T035 → T036 → T037 → T038 (one file, written in order).
- `src-tauri/tests/release_upgrade_test.rs`: T033 (US3) → T066 (US4).
- `src-tauri/src/services/backups.rs`: T031 → T047.
- `src-tauri/src/session/mod.rs`: T049 → T050 → T051.
- `src-tauri/src/session/lifecycle.rs`: T031 → T045 → T050.
- `src-tauri/tests/backup_test.rs`: T031 → T039.
- `src-tauri/src/commands/import_export.rs`: T027 only.
- `src-tauri/examples/human_seed.rs`: T018 (US1) → T029 (US2) → T032 (shared seeding moved) → T047 (the backup made before an upgrade) → T061.
- `src/features/databases/DatabaseChooser.tsx`: T056 → T057.
- `src/features/databases/types.ts`: T045 → T055.
- `CLAUDE.md`: T069 (US4) → T078.
- `src-tauri/tests/performance_test.rs`: T017 (US1) → T072.

### Within Each User Story

- Tests are written first and must fail before the implementation.
- Schema before `ops`; backend state and rules before the commands that use them; commands before the frontend that calls them.
- Each story ends with a Linux run of what it touches; the other OS run in Polish.

### Parallel Opportunities

- T002 alongside T001; T006 and T007 alongside each other once T003 and T004 are written.
- US1: T012 first (red), then T013, then T014–T016 and T018 together (different files).
- US2: T020–T024 together; T028 alongside T026 and T027.
- US3: T035–T043 are in different files except T035–T038 (one file, in order); T045, T048, T055, T058, T059 and T060 together.
- US4: T063–T066 (T063 and T064 in one file, in order) alongside T070.
- Once Foundational is done, US1 and US2 can proceed in parallel; US3's non-fixture work can start alongside them after T031.
- Per-OS runs (macOS, Windows) can go to their own sessions in parallel with Linux work (CLAUDE.md memory "Per-OS sessions").

---

## Parallel Example: User Story 1

```bash
# T012 first (it fails), then T013, then together:
Task: "T014 uid at create_policy in src-tauri/src/commands/insurance.rs"
Task: "T015 uid at the photos insert in src-tauri/src/commands/photos.rs"
Task: "T016 uid at the documents insert in src-tauri/src/commands/documents.rs"
Task: "T018 uid at human_seed.rs's document insert"
```

## Parallel Example: User Story 2

```bash
Task: "T020 insurance_policy_test.rs name rule"
Task: "T021 import_export_test.rs lookup"
Task: "T022 query_plan_test.rs index use"
Task: "T023 InsurancePolicyForm.test.tsx field error and hint"
Task: "T024 value-insurance.e2e.ts 009 US2 journey"
```

## Parallel Example: User Story 3

```bash
# Tests, together (upgrade_test.rs's four tasks in order within one agent):
Task: "T035–T038 src-tauri/tests/upgrade_test.rs"
Task: "T039 src-tauri/tests/backup_test.rs"
Task: "T040 src-tauri/tests/restore_test.rs"
Task: "T041 database_open_test.rs, machine_settings_test.rs, session_scope_test.rs"
Task: "T042 DatabaseChooser.test.tsx and UpgradeDialogs.test.tsx"
Task: "T043 RestoreBackupDialog.test.tsx, DatabaseNotes.test.tsx, databasesService.test.ts"
Task: "T044 databases.e2e.ts 009 US3 journey"

# Building blocks, together:
Task: "T045 models and error codes"
Task: "T048 machine_settings.rs"
Task: "T060 e2e/support/sandbox.ts fixture helper"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational, proving the probes on all three OS first.
3. Complete Phase 3: User Story 1.
4. **STOP and VALIDATE**: `record_identifier_test.rs` passes; the identifiers #30, #31 and #54 need are in the layout.

### Incremental Delivery

1. Setup + Foundational → one runner creates every database; the SQLite behaviours hold.
2. US1 → identifiers (#53).
3. US2 → unique policy names (#54's question).
4. US3 → the upgrade the user sees (#115), and the stored 0.1.0 database.
5. US4 → the checks that keep upgrades trustworthy (#115).
6. Polish → gates on every OS, the manual checks, the PR and its close-out.

All four ship in one pull request, before 0.1.0 is tagged (spec Assumptions): US1, US2 and US3's FR-013 and FR-018 parts must be in 0.1.0 itself.

---

## Notes

- [P] tasks = different files, no dependency on an incomplete task.
- [Story] maps a task to its user story for traceability.
- Verify each test fails before implementing it.
- Commit after each task or logical group (CLAUDE.md "Commit and push").
- Never open the real databases; every run goes through the sandboxed tooling.
- Stop at any checkpoint to validate a story on its own.
