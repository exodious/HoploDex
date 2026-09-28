---

description: "Task list for feature 003: Database Protection, Portability & Management"
---

# Tasks: Database Protection, Portability & Management

**Input**: Design documents from `/specs/003-database-protection-management/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (`tauri-commands.md`, `database-file.md`, `ui-databases.md`), quickstart.md (all present)

**Tests**: The project constitution's Testing Standards principle is NON-NEGOTIABLE. Every phase that builds behaviour, the foundational one included, starts with test tasks, written first and expected to fail, and implementation then makes them pass. Rust tests run against real SQLCipher files in temp directories, created through `db::create_database` with the fixed test passphrase and the production cipher settings (research §20). They never use a mock connection, the OS keyring (only the `mock-keyring` store), or the real config, data or documents directory (constitution 1.2.0, research §21).

**Organization**: Tasks are grouped by user story, in spec.md's priority order (P1–P6). This feature replaces how the application opens its database, so Phase 2 is larger than usual: it swaps the keyring-held random key for a passphrase, turns `DbHandle` into a `Session` that may hold no database, and moves every existing command and test onto it. After Phase 2 the backend test suite passes again, but the app has no way to open a database until User Story 1 adds the chooser.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Which user story this task belongs to (US1–US6)
- Exact file paths follow plan.md's Project Structure

## Path Conventions

This is the existing Tauri desktop app: Rust backend in `src-tauri/`, React/TypeScript frontend in `src/`, WebdriverIO E2E suite in `e2e/`. The feature adds two backend modules, `src-tauri/src/session/` and `src-tauri/src/platform/`, and two frontend feature folders, `src/features/session/` and `src/features/databases/` (plan.md's Structure Decision). Run tests, lint and audits through `scripts/dev-container.sh` (CLAUDE.md). Run E2E specs one at a time, after `npm run build`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Dependencies and their license record. Nothing here changes behaviour.

- [X] T001 Add the Rust dependencies to `src-tauri/Cargo.toml` per plan.md's Technical Context. New: `unicode-normalization`, `gethostname`, `fs4`. Promoted from transitive to direct, at the versions already in `Cargo.lock`: `zeroize`, `same-file`. Target-specific: `zbus` 5 under `[target.'cfg(target_os = "linux")'.dependencies]`; `objc2`, `objc2-foundation`, `objc2-app-kit` under `cfg(target_os = "macos")`; `windows-sys` 0.61 under `cfg(windows)` with features `Win32_System_Power`, `Win32_System_RemoteDesktop`, `Win32_System_Shutdown`, `Win32_UI_WindowsAndMessaging` (research §14). Confirm with `cargo tree -d` that the promoted crates don't add a second version
- [X] T002 [P] Add the npm dependencies with `npm@11` (DEVELOPMENT.md), updating `package.json` and `package-lock.json`: `@zxcvbn-ts/core`, `@zxcvbn-ts/language-common`, `@zxcvbn-ts/language-en`, `@radix-ui/react-dropdown-menu` (research §18, contracts/ui-databases.md §0)
- [X] T003 [P] Record the license exception in DEVELOPMENT.md's "License audit" section, next to the existing OFL-1.1 one (research §18). It covers the ODC-BY `commonWords.json` in `@zxcvbn-ts/language-en` 4.1.1, generated from OpenSubtitles 2024 via OPUS (Helsinki-NLP). The exception is scoped to that package, with the reason (data the estimate looks words up in, not code combined with the program) and the attribution the release notices must carry. Add a note that `wikipedia.json`, `firstnames.json`, `lastnames.json`, `wordSequences.json` and `@zxcvbn-ts/language-common` 4.1.3's `passwords.json`, `diceware.json` and `adjacencyGraphs.json` state no source and are a manual license check before the first release
- [X] T004 Run `npm run audit` (vulnerabilities and `audit:licenses`) and the Rust dependency audit described in DEVELOPMENT.md's "Dependency audit", in the dev container. They must pass with no new advisory exception, and every new or promoted package must declare MIT, Apache-2.0 or both (depends on T001, T002, T003)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The passphrase-keyed file format, the new schema, the `Session` that replaces `DbHandle`, the machine-local settings file, and the test and seed infrastructure that every story's tests stand on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Tests for the foundation (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation.** The Rust tests use the `tests/support` API that T025 describes (`TestDb`, `TEST_PASSPHRASE`, `test_machine()`/`other_machine()`, `TestEvents`). They fail, first to compile and then to pass, until T011–T025 are done. Each story later extends these files with its own cases.

- [X] T005 [P] Write `src-tauri/tests/passphrase_protection_test.rs` with the file-format cases, through `db::create_database`/`db::open_database` (FR-002, FR-003, FR-006, FR-011; research §1, §1a, §2):
  - create, write a row, drop the connection and reopen with `TEST_PASSPHRASE`: the row is there
  - a wrong passphrase gives `OpenError::PassphraseIncorrect`, and the file is byte-identical before and after
  - `validate_new_passphrase` refuses an 11-character passphrase and one containing NUL with `fieldErrors.passphrase`
  - the NFC-composed and decomposed forms of the same passphrase open the same file
  - after create: the file's first 16 bytes are not the SQLite header, the pinned `PRAGMA`s read back as in contracts/database-file.md, the `app_state` row has a 32-hex `database_id` and `changes_waiting = 0`, and `collection_settings` holds the defaults
  - `create_database` refuses an existing path with `Exists` and leaves that file byte-identical
  - an SQLCipher file without `app_state` (a foreign database) gives `PassphraseIncorrect`
  - `verify_passphrase` returns true for the right passphrase and false for a wrong one, never writes the main file, and leaves no probe file in the scratch directory
- [X] T006 [P] Write `src-tauri/tests/database_open_test.rs` with the open outcomes and the normal close (research §2; data-model.md "Session states and transitions"):
  - a missing file gives `NotFound` (`DATABASE_NOT_FOUND` through `From<OpenError>`)
  - unix permissions `000` give `Unreadable`
  - a second `open_database` while the first connection is open gives `InUse`, even with a wrong passphrase, and the first connection still reads and writes
  - a truncated file gives `Damaged`
  - `lifecycle::open` installs the database and adds or refreshes its recent entry, most recent first
  - `close_normal` clears the open marker, drops the connection, deletes this session's decrypted document copies (FR-022), and emits `session:closing` then `session:closed`
- [X] T007 [P] Write `src-tauri/tests/machine_settings_test.rs` with the cases for T019 (research §6, §11), using temp config directories only:
  - the first load creates a 32-hex `machineId` that stays stable across loads
  - `touch_recent` orders most recent first and refreshes `name`
  - writes are atomic (no temp file left behind)
  - a corrupt `machine.json` is renamed to `machine.json.bad` and a fresh file started
  - notices are returned once and then gone
  - `set_unfinished_backup` survives a reload, and `clear_unfinished_backup` removes it
- [X] T008 [P] Write `src-tauri/tests/backup_due_tracking_test.rs` (FR-025, research §5), covering:
  - a guard: every table in `sqlite_schema` is either on the housekeeping list (`app_state`, `pending_changes`, `schema_migrations`, `firearms_fts` and its shadow tables) or carries all three `_marks_backup_due_after_*` triggers
  - an insert, update and delete on each collection table sets `changes_waiting`
  - writing the marker, the backup record, a dismissed note or `pending_changes` does not
  - a create followed by an open and close without changes leaves `changes_waiting = 0`
  - `sqlcipher_export` into an attached copy does not fire the triggers (a permanent spike finding)
- [X] T009 [P] Write `src-tauri/tests/session_test.rs` (research §13; data-model.md "In memory: the session"):
  - with nothing open, `read` and `write` fail with `DATABASE_CLOSED` and never run the closure
  - after `install`, both run the closure against the open connection, and `take` empties the session
  - `Operations::begin` returns a guard; a second `begin` while it lives gives `OPERATION_IN_PROGRESS { operation }`; dropping the guard unregisters it
  - `stop_running` returns the running kind, makes `is_cancelled()` true and interrupts a long query on the registered connection (`SQLITE_INTERRUPT`); `is_running` follows the registry
- [X] T010 [P] Write `src/services/tauriClient.test.ts`: a rejection carrying `details` becomes a `CommandFailure` with the same `details`; `listen` subscribes through `@tauri-apps/api/event` (mocked) with a typed payload and returns a function that unsubscribes

### Implementation for the foundation

- [X] T011 [P] Create `src-tauri/src/db/cipher.rs` with one `CIPHER_SETTINGS` constant and `apply_cipher_settings(conn: &Connection, schema: &str)`. It applies, straight after the key and before the first read, `cipher_page_size = 4096`, `kdf_iter = 1000000`, `cipher_kdf_algorithm = PBKDF2_HMAC_SHA512`, `cipher_hmac_algorithm = HMAC_SHA512`, `cipher_plaintext_header_size = 0`, to `main` or an attached schema (contracts/database-file.md, research §1, FR-011). Add `silence_cipher_log()`, which sets `PRAGMA cipher_log_level = NONE` once per process in release builds only. Declare `pub mod cipher;` in `src-tauri/src/db/mod.rs`
- [X] T012 [P] Create `src-tauri/src/services/passphrase.rs` (register it in `src-tauri/src/services/mod.rs`) per research §1 and data-model.md's validation table. A `Passphrase` newtype over `zeroize::Zeroizing<String>`, built by `Passphrase::from_input(String)`, which moves the string in at once and normalizes it to Unicode NFC. It is never trimmed and has no `Debug`/`Display` that reveals the value. `validate_new_passphrase(&Passphrase)` requires "≥ 12 Unicode scalar values; no NUL" and returns `VALIDATION_ERROR` with `fieldErrors.passphrase` = "Use at least 12 characters." (or a NUL message). `as_str()` is for `pragma_update`/`ATTACH … KEY ?` only (FR-003, FR-007)
- [X] T013 [P] Create `src-tauri/src/models/database.rs` (register it in `src-tauri/src/models/mod.rs`) with serde camelCase types mirroring contracts/tauri-commands.md "Shared types": `RecentDatabase`, `ChooserNotice`, `CloseReason` (`closed`, `switched`, `quit`, `lockedByUser`, `idle`, `screenLocked`, `sleep`, `shutdown`, `takenOver`), `OperationKind` (`backup`, `passphraseChange`, `restore`, `import`, `export`, `deleteBackups`), `CollectionSettings`, `PendingSummary`, `Draft`, `DatabaseStatus` (with `notes`), `CloseOutcome` and `BackupInfo`. Use the existing `text_enum!` pattern for the enums. No validation yet: each story adds its own `validate_*_input`
- [X] T014 [P] Create `src/features/databases/types.ts` mirroring T013's types exactly as written in contracts/tauri-commands.md "Shared types" (TypeScript unions for `CloseReason`, `OperationKind`, `ChooserNotice`)
- [X] T015 [P] Extend `src/services/tauriClient.ts`: `CommandError`/`CommandFailure` gain `details?: Record<string, unknown>`, and add a typed `listen<T>(event: string, handler: (payload: T) => void): () => void` helper over `@tauri-apps/api/event`, used by every backend event this feature adds (contracts/tauri-commands.md "Changes to the common shape", "Events")
- [X] T016 Add the new tables and triggers to `src-tauri/src/db/migrations/0001_initial.sql`, editing it in place (CLAUDE.md, data-model.md "Inside the database"):
  - `collection_settings`: `id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1)`, `backups_enabled INTEGER NOT NULL DEFAULT 1 CHECK (backups_enabled IN (0,1))`, `backup_keep_count INTEGER NOT NULL DEFAULT 5 CHECK (backup_keep_count BETWEEN 1 AND 100)`, `backup_location TEXT NOT NULL DEFAULT 'default'`, `idle_lock_enabled INTEGER NOT NULL DEFAULT 1 CHECK (idle_lock_enabled IN (0,1))`, `idle_lock_minutes INTEGER NOT NULL DEFAULT 10 CHECK (idle_lock_minutes BETWEEN 1 AND 240)`, `lock_on_screen_lock INTEGER NOT NULL DEFAULT 0 CHECK (lock_on_screen_lock IN (0,1))`
  - `app_state`: `id INTEGER PRIMARY KEY CHECK (id = 1)`, `database_id TEXT NOT NULL` (32 lowercase hex digits), `created_at TEXT NOT NULL`, `open_machine_id TEXT`, `open_machine_name TEXT` (≤ 255 chars), `open_since TEXT`, `changes_waiting INTEGER NOT NULL DEFAULT 0 CHECK (changes_waiting IN (0,1))`, `last_backup_at TEXT`, `disk_encryption_note_dismissed INTEGER NOT NULL DEFAULT 0 CHECK (disk_encryption_note_dismissed IN (0,1))`, and the table CHECK `((open_machine_id IS NULL) = (open_machine_name IS NULL) AND (open_machine_id IS NULL) = (open_since IS NULL))`. The backup-stamp columns `backup_made_at` and `backup_of_name` are added by T080 in User Story 3, together with the seeded backup that covers them
  - `pending_changes`: `id INTEGER PRIMARY KEY CHECK (id = 1)`, `kind TEXT NOT NULL CHECK (kind IN ('firearm','policy'))`, `mode TEXT NOT NULL CHECK (mode IN ('add','edit','dispose','restore','coverage'))` with `CHECK (mode <> 'coverage' OR kind = 'firearm')`, `target_id INTEGER` (NULL only for `mode = 'add'`, not a foreign key), `label TEXT NOT NULL` (≤ 200 chars), `form_version INTEGER NOT NULL`, `values_json TEXT NOT NULL CHECK (length(values_json) <= 1048576)`, `saved_at TEXT NOT NULL`
  - The change-tracking triggers `<T>_marks_backup_due_after_insert`, `_after_update` and `_after_delete` on each of `firearms`, `photos`, `document_attachments`, `disposition_history`, `insurance_policies`, `firearm_types` and `collection_settings` (21 triggers), each `BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;`. None on `app_state`, `pending_changes`, `schema_migrations` or the FTS5 tables (research §5, FR-025)
- [X] T017 Rewrite `src-tauri/src/db/mod.rs` for the passphrase model (research §1, §1a, §2; contracts/database-file.md). Remove `KEYRING_SERVICE`, `KEYRING_USER`, `DB_FILE_NAME`, `DbHandle`, `generate_key_hex`, both `get_or_create_passphrase`s, `open_encrypted` and `init_app_db`, and add no code that names the `sqlcipher-key` entry (research §10). Add the following (depends on T011, T012, T016):
  - `configure(conn, &Passphrase)`: `pragma_update("key", …)`, `apply_cipher_settings(conn, "main")`, `foreign_keys = ON`, `secure_delete = ON`, `locking_mode = EXCLUSIVE`
  - `create_database(path, &Passphrase, &MachineIdentity) -> Result<Connection, DbError>`: refuses an existing path (`Exists`), configures, applies the migrations, inserts the default `collection_settings` row, and then inserts the `app_state` row: random 32-hex `database_id` from `getrandom`, `created_at` now UTC, open marker set to the machine, `changes_waiting = 0`. That order keeps seeding `firearm_types` and the settings row from marking the new database due for a backup
  - `open_database(path, &Passphrase) -> Result<Connection, OpenError>`, in research §2's order, with nothing written before step 5. Metadata failure gives `NotFound`/`Unreadable`. `configure`, then the first read `SELECT count(*) FROM sqlite_schema`: `SQLITE_BUSY` gives `InUse`, `SQLITE_NOTADB` gives `PassphraseIncorrect`, `SQLITE_CORRUPT` gives `Damaged`. `schema_migrations` missing, or no `app_state` row, also gives `PassphraseIncorrect`. Then apply missing migrations. Steps 3 and 4 (newer version, open marker) are added in User Story 2 (T059)
  - `verify_passphrase(path, &Passphrase, scratch_dir) -> Result<bool, DbError>`, the page-1 probe of research §1a: copy the first 4096 bytes to a temp file in `scratch_dir`, open it with the candidate and the pinned settings, read `PRAGMA schema_version`. `SQLITE_NOTADB` means wrong. Delete the probe file whatever the result. The main file is never written
  - `OpenError` enum: `NotFound`, `Unreadable`, `InUse`, `PassphraseIncorrect`, `NewerVersion`, `OpenElsewhere { machine_name, since }`, `Damaged`
  - Rewrite the module's unit tests (`opens_migrates_and_reopens_a_real_temp_database`, `wrong_key_on_reopen_fails`) for `create_database`/`open_database`
- [X] T018 Extend `src-tauri/src/commands/error.rs` (depends on T017). `CommandError` gains `details: Option<serde_json::Value>`, skipped when `None`. Add a constructor for each code in contracts/tauri-commands.md: `DATABASE_CLOSED`, `PENDING_CHANGES_UNRESOLVED`, `DATABASE_TAKEN_OVER`, `DATABASE_DAMAGED { backupsAvailable }`, `PASSPHRASE_INCORRECT { savedPassphraseFailed?, backupsAvailable? }`, `DATABASE_NOT_FOUND { path }`, `DATABASE_UNREADABLE { path }`, `DATABASE_IN_USE`, `DATABASE_NEWER_VERSION`, `DATABASE_OPEN_ELSEWHERE { machineName, since }`, `DATABASE_EXISTS { path }`, `INSUFFICIENT_SPACE { bytesNeeded, bytesAvailable, path }`, `BACKUP_LOCATION_UNAVAILABLE { path, reason }`, `KEYRING_UNAVAILABLE`, `OPERATION_STOPPED { operation, importedCount?, deletedCount? }`, `OPERATION_IN_PROGRESS { operation }`, `REPLACE_FAILED { path }`, `DATABASE_UNAVAILABLE { path }`. Use the summary messages from contracts/tauri-commands.md "Error codes added by this feature". `from_db` maps `SQLITE_CORRUPT` to `DATABASE_DAMAGED`. Add `From<OpenError>`. No message or detail may contain a passphrase
- [X] T019 [P] Create `src-tauri/src/services/machine_settings.rs` (research §6, §11; data-model.md "Machine-local: machine.json"). `MachineSettings` is loaded from a config directory passed in, never resolved inside the service, and holds the exact JSON shape (`version: 1`, `machineId`, `recentDatabases[]` with `path`, `name`, `lastOpenedAt`, `databaseId`, `backupFolder`, `passphraseSaved`; `unfinishedBackup`; `notices[]`). The `machineId` is random 32 hex, made on first run. Writes are atomic (temp file, flush, rename). A corrupt or unreadable file is renamed to `machine.json.bad` and a new one started. Methods: `touch_recent` (add or refresh, most recent first), `recent`, `push_notice`, `take_notices`, `set_unfinished_backup`/`clear_unfinished_backup`. `MachineIdentity { id, display_name }`, where `display_name` is `gethostname` with a trailing `.local` removed on macOS. It holds paths and names only, never collection data or secrets
- [X] T020 Create `src-tauri/src/session/mod.rs` (declare `pub mod session;` in `src-tauri/src/lib.rs`) per research §13 and data-model.md "In memory: the session" (depends on T017, T018). `Session(Mutex<Option<OpenDatabase>>)`, where `OpenDatabase` holds `conn`, `path`, `name`, `database_id`, `interrupt: InterruptHandle`, `staged_draft: Option<Draft>` and `pending_unresolved: bool`. `read(|conn| …)` and `write(|conn| …)` both fail with `DATABASE_CLOSED` when nothing is open. `write` gains the take-over and unreachable-storage checks in T061, and both `read` and `write` gain the pending-changes check in T121. Add `is_open()`, `take()` (removes the open database for a close) and `install(OpenDatabase)`
- [X] T021 [P] Create `src-tauri/src/session/operations.rs` (research §13; data-model.md "Operations registry"). `Operations` allows at most one running long operation `{ kind: OperationKind, cancel: AtomicBool, interrupt: Option<InterruptHandle> }`. `begin(kind)` returns an RAII guard that unregisters on drop, or `OPERATION_IN_PROGRESS { operation }`. It also offers `is_cancelled()` for chunk and row loops, `stop_running() -> Option<OperationKind>` (sets the flag and calls `interrupt()`), and `is_running()` for the idle clock
- [X] T022 Create `src-tauri/src/session/lifecycle.rs` with the open and normal-close procedures as ordered steps (data-model.md "Session states and transitions", depends on T019, T020):
  - `open(...)`: `db::open_database`, then install the `OpenDatabase` and `touch_recent` with its name, `databaseId` and resolved backup folder
  - `create(...)`: `db::create_database`, then the same
  - `close_normal(app, reason)`: emit `session:closing { reason }`; the backup step (inserted by T085); clear the open marker (housekeeping); drop the connection; delete this session's decrypted document copies through `commands::documents::ops::clear_opened_documents` (FR-022); emit `session:closed { reason, databasePath, outcome }`
  
  Emit events through a small `SessionEvents` trait, so integration tests can record them without a Tauri app
- [X] T023 Move every existing command from `State<DbHandle>` + `.0.lock()` to `State<Session>` + `session.read(...)` (queries) or `session.write(...)` (anything that changes the database), leaving every `ops` signature unchanged, in `src-tauri/src/commands/firearms.rs`, `src-tauri/src/commands/insurance.rs`, `src-tauri/src/commands/photos.rs`, `src-tauri/src/commands/documents.rs` and `src-tauri/src/commands/import_export.rs` (depends on T020)
- [X] T024 Update `src-tauri/src/main.rs` `setup` (depends on T019–T023). It no longer opens a database. It manages `Session` (empty), `Operations` and `MachineSettings` (loaded from `app.path().app_config_dir()`), calls `db::cipher::silence_cipher_log()`, and keeps the startup sweep of decrypted document copies and the existing signal handling. `ImportSessionStore` stays
- [X] T025 Rewrite `src-tauri/tests/support/mod.rs` (research §20; depends on T017). `pub const TEST_PASSPHRASE`, a fixed ≥ 12-character string. `TestDb::new()` calls `db::create_database(<tempdir>/test.hoplodex, TEST_PASSPHRASE, test machine identity)` and keeps the public `conn` field. Add `TestDb::path()`, `TestDb::reopen()` (drop and `open_database` again), a `test_machine()` / `other_machine()` identity pair, and a `TestEvents` recorder implementing `SessionEvents`. Every existing integration test must compile and pass unchanged in logic
- [X] T026 [P] Create `src-tauri/examples/support/sandbox.rs` and its test `src-tauri/tests/seed_sandbox_test.rs` (write the test first; the test includes the module via `#[path]`) per research §21. `check_sandbox(target, env)` accepts a directory that does not exist yet, or one holding the `.hoplodex-sandbox` marker the seed created (and writes the marker into a new one). It refuses a non-empty directory without the marker, and any target that is, or lies inside, the data, config or documents directory resolved from the environment given (`XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_DOCUMENTS_DIR`/`user-dirs.dirs`, with their `$HOME` fallbacks). Cover each case with a temp `HOME`, and never resolve the developer's real directories
- [X] T027 Rewrite `src-tauri/examples/human_seed.rs` and `scripts/human-testing.sh` for the new layout (plan.md Project Structure, research §21; depends on T025, T026). `--dir` is checked with `sandbox::check_sandbox`, which replaces `refuse_real_data_dir`. The seed never touches the keyring. It creates `<dir>/HoploDex/Main collection.hoplodex` through `db::create_database` with a fixed passphrase it prints, runs the existing `seed(conn, extra)`, and sets `collection_settings` to backups on, a keep count of 3, a custom location `<dir>/Backups`, and the idle lock on at 15 minutes. It sets the main database's `disk_encryption_note_dismissed = 1` and its backup record to `changes_waiting = 0` with `last_backup_at` set (T095 later puts real backups behind it). It creates a second database, `<dir>/HoploDex/Shared collection.hoplodex`, with its own seeded firearms, so `changes_waiting = 1`, the note not dismissed, backups off, the idle lock off and `lock_on_screen_lock = 1`, a `pending_changes` row for a seeded firearm (`mode = 'edit'`) and its open marker left set by a fictitious machine ("Workshop PC"). Between them the two databases hold both values of every `CHECK IN (0,1)` column. It clears the main database's marker before closing, and writes `machine.json` with both entries into `<dir>/config/com.hoplodex.app/`. `scripts/human-testing.sh` launches with `XDG_CONFIG_HOME="$dir/config"` and `XDG_DATA_HOME`/`XDG_CACHE_HOME` in the sandbox, writes a `user-dirs.dirs` pointing `XDG_DOCUMENTS_DIR` at `$dir`, and prints the passphrase
- [X] T028 Update `src-tauri/tests/human_seed_coverage_test.rs` for the new tables (CLAUDE.md "Keep the human-testing seed in step"; depends on T027). Check the single-row tables (`collection_settings`, `app_state`, `pending_changes`) across both seeded databases, since one row cannot hold both values of a `CHECK IN (0,1)` column. List `pending_changes.kind` and `pending_changes.mode` in `PARTIAL_VALUES_OK` with the reason "at most one row per database (FR-039)". Fix any other gap by seeding, not by loosening the test
- [X] T029 Run `cargo test`, `cargo clippy --all-targets` and `cargo fmt --check` for `src-tauri/Cargo.toml`, and `npm run test`, in the dev container. The foundation tests above and all existing integration tests must pass on the passphrase-keyed `TestDb` (depends on T011–T028)

**Checkpoint**: The file format, schema, session and test infrastructure are in place, and the backend suite passes. The app starts with no database open and has no UI to open one yet. User stories can begin.

---

## Phase 3: User Story 1 - Protect My Collection With My Own Passphrase (Priority: P1) 🎯 MVP

**Goal**: The user creates a database with a passphrase they choose, is warned it cannot be recovered, sees a strength hint, and must enter the passphrase before anything from the collection is shown. No key or passphrase is stored on the machine.

**Independent Test**: Start with no database, create one with a passphrase, add a firearm, quit and relaunch. The collection appears only after the correct passphrase; a wrong one is refused with the FR-006 message and the file is unchanged; nothing is written to the keyring.

### Tests for User Story 1 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T030 [P] [US1] Extend `src-tauri/tests/passphrase_protection_test.rs` with the command cases, through `commands::databases::ops`:
  - create a database, add a firearm through `ops`, close, and reopen with `TEST_PASSPHRASE`: the firearm is there
  - a wrong passphrase gives `PASSPHRASE_INCORRECT`, and the file is byte-identical before and after
  - a short passphrase is refused with `fieldErrors.passphrase`, and `acknowledgedUnrecoverable: false` with `fieldErrors.acknowledgedUnrecoverable` (FR-004)
  - the mock keyring holds no entry after create and open (SC-002)
  - `dismiss_note("diskEncryption")` sets `disk_encryption_note_dismissed` and leaves `changes_waiting = 0`
  - a foreign SQLCipher file gives `PASSPHRASE_INCORRECT`
  - `validate_create_database_input` enforces each rule from data-model.md's validation table (name, folder, `DATABASE_EXISTS`)
- [X] T031 [P] [US1] Write `src/components/PassphraseField.test.tsx`: the input is `type="password"` with `spellcheck="false"` and `autocapitalize="off"`; the Show toggle sets `aria-pressed` and switches the type; the value is never in React state (read through the ref on submit, and the input is empty after `reset()`); with `strength` set, the lazily loaded hint shows a five-step meter, a text label, zxcvbn's suggestion and "Longer is stronger: several unrelated words make a good passphrase."; the hint never blocks; field errors use the standard slot (contracts/ui-databases.md §0)
- [X] T032 [P] [US1] Write `src/features/databases/CreateDatabaseDialog.test.tsx`: the defaults come from `suggested`; the help reads "Saved as <folder>/<name>.hoplodex"; a short passphrase shows "Use at least 12 characters." and a mismatched confirmation shows "The passphrases don't match."; the backup disclosure panel names `<folder>/HoploDex backups`, "at most once a day" and "keeping the latest 5" (FR-024); **Create database** stays disabled until the acknowledgement checkbox is ticked (FR-004); server `fieldErrors` land on the right fields; success calls `create_database` with the passphrase read from the field; pressing **Create database** puts it in the `Button`'s `pending` state ("Creating…") at once, before the command resolves, with the fields and **Cancel** disabled until it does (constitution IV, contracts/ui-databases.md §2)
- [X] T033 [P] [US1] Write `src/features/databases/DatabaseChooser.test.tsx` (US1 cases): with no known databases it shows the welcome text and the two large page actions; with a recent entry the most recent row is selected with its focused "Passphrase for <name>" field and **Open**; a `PASSPHRASE_INCORRECT` failure shows "That passphrase didn't open <name>. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged." and keeps the field for another try (FR-006); pressing **Open** puts it in the `pending` state ("Opening…") at once, before the command resolves, disables the other rows and announces "Opening <name>…" in the live region (SC-003, constitution IV); no collection data is rendered (contracts/ui-databases.md §1)
- [X] T034 [P] [US1] Write `src/features/session/SessionProvider.test.tsx` (US1 cases): with no open database only the chooser renders, and no `CollectionProvider` or `AppShell`; after a successful open or create the app shell renders; the disk-encryption note renders once while `status.notes.diskEncryption` is true and is dismissed through `dismiss_note` (FR-008)
- [X] T035 [P] [US1] Write the US1 flow in `e2e/specs/us7-databases.e2e.ts`: first run shows the welcome; create "Test" in a typed scratch folder (a short passphrase is blocked, and Create is disabled until the acknowledgement); the disk-encryption note appears and stays away after dismissal; add a firearm; relaunch the session; a wrong passphrase shows the FR-006 message; the right one shows the firearm

### Implementation for User Story 1

- [X] T036 [US1] Add `validate_create_database_input` to `src-tauri/src/models/database.rs` with data-model.md's rules verbatim. Database name: "1–120 chars and at most 200 bytes of UTF-8; no path separator or character invalid on any supported OS (`<>:"/\|?*`, control characters); not `.`/`..`; not ending in a space or dot" (`fieldErrors.name`). Database folder: "absolute; a writable folder, or a path that does not exist yet, which create makes" (`fieldErrors.folder`). Create target: "`<folder>/<name>.hoplodex` must not exist" (`DATABASE_EXISTS`). Also check `acknowledgedUnrecoverable` is true (`fieldErrors.acknowledgedUnrecoverable`), and run `validate_new_passphrase` for `fieldErrors.passphrase`
- [X] T037 [US1] Create `src-tauri/src/commands/databases.rs` (declare it in `src-tauri/src/commands/mod.rs`), with thin commands over a `pub mod ops` that takes explicit paths and `&MachineSettings` so tests call it directly:
  - `get_chooser_state` → `{ recent, selectedPath, keyringAvailable, screenLockSupported, suggested, notices }`. `keyringAvailable` and `screenLockSupported` are `false` until T108 and T130. `suggested` is `{ folder: <Documents>/HoploDex, name: "My collection" }` from `app.path().document_dir()` (which honours `user-dirs.dirs`), with `$HOME/Documents` as the fallback and never the app data directory (research §19)
  - `create_database` 🔑
  - `open_database` 🔑 with a typed `passphrase` (the saved-passphrase and `takeOver` inputs come in US5 and US2)
  - `get_database_status`
  - `dismiss_note` (`diskEncryption` sets `app_state.disk_encryption_note_dismissed`, a housekeeping write)
  
  Every passphrase argument goes into `Passphrase::from_input` at once and is dropped when the command returns (FR-007, contracts/tauri-commands.md). Depends on T036
- [X] T038 [US1] Register `get_chooser_state`, `create_database`, `open_database`, `get_database_status` and `dismiss_note` in `generate_handler!` in `src-tauri/src/main.rs` (depends on T037)
- [X] T039 [P] [US1] Create `src/components/StrengthHint.tsx`: on first render it dynamically `import()`s `@zxcvbn-ts/core`, `@zxcvbn-ts/language-common` and `@zxcvbn-ts/language-en`, then shows the five-step meter with the label ("Very weak" … "Very strong"), zxcvbn's suggestion and "Longer is stronger: several unrelated words make a good passphrase." (research §18). No keystroke crosses IPC
- [X] T040 [US1] Create `src/components/PassphraseField.tsx`, uncontrolled per contracts/ui-databases.md §0: `forwardRef` exposing `read(): string` and `reset()`; `type="password"` with a Show toggle (`aria-pressed`); an `autocomplete` prop (`current-password`/`new-password`); `spellcheck="false"`, `autocapitalize="off"`; an optional `strength` flag that renders `StrengthHint` from the input's own `onInput` without keeping the value in state; the standard field-error slot. Export both components from `src/components/index.ts` (depends on T039)
- [X] T041 [P] [US1] Create `src/features/session/sessionService.ts` and `src/features/databases/databasesService.ts`, with typed `invoke` wrappers (one per command) for `get_chooser_state`, `create_database`, `open_database`, `get_database_status` and `dismiss_note`, returning T014's types. Later stories add their commands to these files
- [X] T042 [US1] Create `src/features/session/SessionProvider.tsx`. It holds the session state (`chooser` | `open` with `DatabaseStatus`), exposes `useSession()` (`status`, `openDatabase`, `createDatabase`, `refreshStatus`), and renders the chooser when no database is open. Rewrite `src/App.tsx` to `SessionProvider` → `DatabaseChooser` | (`CollectionProvider` + `AppShell`), keyed by the open database's path and open count, so a close unmounts all collection state (plan.md, contracts/ui-databases.md §3) (depends on T041)
- [X] T043 [US1] Create `src/features/databases/DatabaseChooser.tsx` and `src/features/databases/RecentDatabaseRow.tsx`, plus `src/features/databases/databases.css`, per contracts/ui-databases.md §1:
  - header with the brand and the theme toggle
  - the recent list (name, middle-truncated folder with the full path in the tooltip and accessible name)
  - the selected row's inline `PassphraseField` "Passphrase for <name>", focused, with **Open**
  - the page actions **Create a new database…** and **Open another database file…** (a native open dialog filtered to `*.hoplodex` with "All files"; the chosen file becomes the selected row)
  - the first-run welcome "HoploDex keeps your collection in an encrypted database file that only your passphrase opens."
  - the `PASSPHRASE_INCORRECT` inline error text
  - the opening busy state: **Open** in the `Button`'s `pending` state ("Opening…") from the moment it is pressed until the command returns, with the other rows disabled (SC-003)
  - an `aria-live="polite"` region, with focus on the primary control (§14)
  
  Depends on T040, T042
- [X] T044 [US1] Create `src/features/databases/CreateDatabaseDialog.tsx` per contracts/ui-databases.md §2. It is a `Dialog size="lg"` titled "Create a database", in the `hd-form-grid` layout:
  - **Name** (help "This is also the file name.")
  - **Folder**: a text field plus **Choose…** (native folder picker), with the "Saved as …" help
  - **Passphrase** (`PassphraseField` with `strength`, `new-password`) and **Confirm passphrase**
  - the read-only backups disclosure (FR-024)
  - the required acknowledgement `Checkbox`
  
  The footer has **Cancel** and **Create database**, which shows the `pending` state ("Creating…") while the command runs. It checks the length and the match in the frontend, reads both fields once on submit and resets them, and maps server `fieldErrors`. Follow the field widths and grouping of the existing forms (CLAUDE.md UI consistency) (depends on T040, T042)
- [X] T045 [US1] Create `src/features/databases/DatabaseNotes.tsx` with the disk-encryption banner in the existing notice style: "Your collection is encrypted with your passphrase. For extra protection, also turn on your computer's disk encryption: BitLocker on Windows, FileVault on macOS, or LUKS on Linux." with a **Why?** link (a no-op until the guide exists, T137) and dismiss → `dismiss_note("diskEncryption")` (FR-008, contracts/ui-databases.md §10). Add a notes banner slot at the top of the collection page in `src/features/app/AppShell.tsx` and render `DatabaseNotes` there (depends on T042)
- [X] T046 [US1] Rework the E2E harness for the passphrase model in `e2e/wdio.conf.ts` and `e2e/support/ui.ts` (research §10, §19):
  - drop `HOPLODEX_E2E_DB_KEY` everywhere
  - write a scratch `user-dirs.dirs` into the scratch `XDG_CONFIG_HOME`, setting `XDG_DOCUMENTS_DIR` to a scratch folder
  - run `human_seed` with `--dir` inside the sandbox and read its fixed passphrase
  - add `createDatabase({ folder, name, passphrase })` and `unlock(passphrase)` helpers that type locations rather than using native pickers
- [X] T047 [US1] Update the existing E2E specs to start by creating or unlocking a database: `e2e/specs/us1-record-firearm.e2e.ts`, `us2-browse-search.e2e.ts`, `us3-value-insurance.e2e.ts`, `us4-photos-documents.e2e.ts`, `us5-export-import.e2e.ts`, `us6-identification.e2e.ts`, `ui-review.e2e.ts`, and `e2e/screenshots/screens.e2e.ts` (unlock the seeded "Main collection"). Run each spec one at a time after `npm run build` (depends on T046)
- [X] T048 [US1] Add the screens `14-chooser`, `15-chooser-first-run`, `16-create-database` and `24-disk-encryption-note` (light and dark) to `e2e/screenshots/screens.e2e.ts` (contracts/ui-databases.md §15). `15-chooser-first-run` and `24-disk-encryption-note` need a sandbox with no databases, so they are in `e2e/screenshots/first-run.e2e.ts`, which `npm run screenshots` also runs (depends on T047)

**Checkpoint**: User Story 1 is fully functional. A database is created with a passphrase and opens only with it. This is the MVP.

---

## Phase 4: User Story 2 - Keep Several Databases, Anywhere, and Take Them to Another Computer (Priority: P2)

**Goal**: Several databases in folders of the user's choosing, a recent list with unavailable entries, switching and closing (with the save, discard or cancel question), refusal of newer, in-use and open-elsewhere databases with take-over, and a file that opens on any OS with its passphrase alone.

**Independent Test**: Create two databases with different passphrases in two folders, add different firearms to each, and switch between them. Remove one from the list and check the file is untouched. Open the committed portability fixture and check every record, photo and document.

### Tests for User Story 2 (mandatory per constitution)

- [X] T049 [P] [US2] Extend `src-tauri/tests/database_open_test.rs` with the US2 cases:
  - a file whose `schema_migrations` names an unknown migration gives `DATABASE_NEWER_VERSION`, and the file is byte-identical (FR-014)
  - opening a second database closes the first with reason `switched`
  - `get_chooser_state` marks an entry whose file is missing as unavailable, and selects the database just closed (FR-012, FR-033)
- [X] T050 [P] [US2] Write `src-tauri/tests/take_over_test.rs` (FR-032, SC-009), covering:
  - a marker set by `other_machine()` gives `DATABASE_OPEN_ELSEWHERE` with `details { machineName, since }`, and the file is byte-identical
  - `takeOver: true` opens it and sets the marker to this machine
  - this machine's own stale marker (left by a dropped connection) opens without a warning
  - replacing the file at the path while open (another file renamed over it) makes the next `session.write` fail with `DATABASE_TAKEN_OVER`, writes nothing to either file, and emits `session:closed { reason: "takenOver" }`
  - a close after a take-over writes nothing (no marker clear)
  - reads still work until then
  - the fingerprint is refreshed after our own writes, so they never trip the check
  - with the file made unreachable while open (renamed away, then a directory made unreadable), the next `session.write` fails with `DATABASE_UNAVAILABLE { path }`, not `DATABASE_TAKEN_OVER`: nothing is written, no `session:closed` is emitted, and the session stays open
  - after that, every write is refused the same way, even once the file is back at its path
  - the close that follows writes nothing (no marker clear), and with the file back, the next open here succeeds without a warning (its own marker, FR-032)
- [X] T051 [P] [US2] Extend `src-tauri/tests/machine_settings_test.rs` with the US2 cases: `locate` replaces `path` and keeps the other fields; `remove` drops only the entry, and the database file is untouched. Use temp config directories only
- [X] T052 [P] [US2] Create `src-tauri/examples/portable_fixture.rs`, which regenerates `src-tauri/tests/fixtures/portable-v1.hoplodex` with a known passphrase, one firearm, one photo and one document, through `db::create_database` and `ops`. Commit the generated fixture. Then write `src-tauri/tests/portability_test.rs` (SC-001, SC-002, research §20):
  - open a copy of the fixture with the known passphrase, and compare the firearm fields and the photo and document bytes to the known values
  - assert the first 16 bytes are a salt, not the SQLite header
  - assert opening consults no machine-local state (no config dir, empty mock keyring)
  - a wrong passphrase fails
- [X] T053 [P] [US2] Extend `src/components/ConfirmDialog.test.tsx` for the optional third action: `alternativeLabel`/`onAlternative` render a third button with destructive style, and each of the three buttons calls only its own handler (contracts/ui-databases.md §0)
- [X] T054 [P] [US2] Write `src/components/Menu.test.tsx`: menu roles, arrow-key navigation, Escape closes and returns focus to the trigger (WCAG 2.1 AA, contracts/ui-databases.md §0)
- [X] T055 [P] [US2] Extend `src/features/databases/DatabaseChooser.test.tsx` (US2 cases), covering:
  - recent rows in order, with the folder tooltip
  - an unavailable row dimmed with "Not found at this location", **Locate…** and **Remove from list**
  - every row's overflow **Remove from list** with "The database file is not deleted."
  - **Open another database file…** (with `@tauri-apps/plugin-dialog`'s `open` mocked) calls it filtered to `*.hoplodex` with "All files", makes the chosen file the selected row with its focused "Passphrase for <name>" field, and opening it calls `open_database` with that path; a cancelled picker changes nothing (US2-2)
  - the open-failure texts for `DATABASE_IN_USE`, `DATABASE_NEWER_VERSION`, `DATABASE_NOT_FOUND`, `DATABASE_UNREADABLE` and `DATABASE_OPEN_ELSEWHERE` (machine name in bold and the local date/time), with **Go back** and **Take over…**
  - **Take over…** opens the destructive confirm "Take over <name>?", and confirming resends with `takeOver: true`, with the row in its opening busy state until it returns
  - the `takenOver` notice text (contracts/ui-databases.md §1)
- [X] T056 [P] [US2] Write `src/features/databases/DatabaseMenu.test.tsx`: the top-bar button shows the database name with `aria-haspopup="menu"`, and **Switch database…** and **Close database** go through the unsaved-changes check and then `close_database` (contracts/ui-databases.md §4)
- [X] T057 [P] [US2] Extend `src/features/session/SessionProvider.test.tsx` (US2 cases, FR-010, US2-4a), covering:
  - close, switch and `app:quit-requested` with a registered dirty form show "Save changes to <label>?" with **Save changes**, **Discard changes** and **Cancel**
  - Save runs the form's submit; if it fails validation, the dialog closes, the form keeps its errors and nothing closes
  - Discard closes
  - Cancel keeps everything open
  - a clean form closes without asking
  - `session:closed` unmounts the collection tree and selects `databasePath` in the chooser
  - a save that fails with `DATABASE_UNAVAILABLE` shows contracts/ui-databases.md §1's "can't reach" text in the form, keeps its input and keeps the collection open
  - quit calls `quit_application`
- [X] T058 [P] [US2] Write the US2 flows in `e2e/specs/us7-databases.e2e.ts`: create two databases in two scratch folders with different passphrases and a distinct firearm each; switch through the database menu; the chooser lists both, most recent first; closing with an unsaved firearm form asks save, discard or cancel; remove one from the list and check its file still exists

### Implementation for User Story 2

- [X] T059 [US2] Add research §2 steps 3 and 4 to `db::open_database` in `src-tauri/src/db/mod.rs`. Step 3 is read-only: a `schema_migrations` name this build does not know gives `NewerVersion`. Step 4 is read-only: `open_machine_id` set and not this machine's id gives `OpenElsewhere { machine_name, since }`, unless `take_over` is set. Step 5 sets the marker to this machine's id, display name and the current UTC time, replacing a stale own marker. `open_database` gains `machine: &MachineIdentity` and `take_over: bool`; update the callers (depends on T017)
- [X] T060 [P] [US2] Create `src-tauri/src/session/fingerprint.rs`, per research §6. `FileFingerprint { identity: same_file::Handle-derived key, len, mtime }` has `capture(path)` and `check(path) -> Same | Replaced | Unreachable`, where `Unreachable` means the path is missing or `stat` fails with an I/O or permission error. It costs one `stat` plus the handle identity (device/inode on Unix, volume serial/file index on Windows)
- [X] T061 [US2] Wire the take-over check into `src-tauri/src/session/mod.rs` and `src-tauri/src/session/lifecycle.rs` (FR-032, data-model.md "TakenOver"; depends on T060). `OpenDatabase` gains `fingerprint`, captured after open. `write()` compares it before running the closure and refreshes it after. `Replaced` refuses with `DATABASE_TAKEN_OVER`, emits `session:closed { reason: "takenOver" }`, pushes a `takenOver` notice, and drops the connection without writing anything more. `Unreachable`, or an `SQLITE_IOERR`/`SQLITE_CANTOPEN` from the write, refuses with `DATABASE_UNAVAILABLE { path }` and sets `OpenDatabase.storage_lost`, keeping the session open; while it is set, every write is refused with that code whatever the fingerprint says (research §6). `close_normal` checks first too: on `Replaced` or `storage_lost` it skips the backup and the marker clear and writes nothing
- [X] T062 [US2] Add switch and take-over to `src-tauri/src/session/lifecycle.rs` and `src-tauri/src/commands/databases.rs` (contracts/tauri-commands.md; depends on T059, T061):
  - `open_database` accepts `takeOver`; if another database is open, it closes it first as a `switched` close
  - add `close_database { reason: "closed" | "switched" } → CloseOutcome` (with `backup: "notAttempted"` until US3)
  - add `quit_application`: a normal close with reason `quit` if a database is open, then `app.exit(0)`
  - add `remove_recent_database { path } → { removed: true }`, which never touches the file (the keyring part is added in T108)
  - add `locate_database { path, newPath } → RecentDatabase`
  - `get_chooser_state` reports `available` from the path's existence and sets `selectedPath` to the database just closed (FR-012, FR-033)
- [X] T063 [US2] Update `src-tauri/src/main.rs` (research §17; depends on T062). `WindowEvent::CloseRequested` and `RunEvent::ExitRequested`, when not OS-initiated, are prevented and emit `app:quit-requested`. Register `close_database`, `quit_application`, `remove_recent_database` and `locate_database` in `generate_handler!`
- [X] T064 [P] [US2] Add the optional `alternativeLabel`/`onAlternative` third action (destructive style) to `src/components/ConfirmDialog.tsx`, keeping the existing props and behaviour unchanged (contracts/ui-databases.md §0)
- [X] T065 [P] [US2] Create `src/components/Menu.tsx` on `@radix-ui/react-dropdown-menu` (items, separators, shortcut hints), styled with the tokens in `src/styles/tokens.css`, and export it from `src/components/index.ts`
- [X] T066 [US2] Create the dirty-form registry in `src/features/session/usePendingDraft.ts` (plan.md). `useDirtyForm({ label, isDirty, submit })` registers the one open firearm or policy form, and `getDirtyForm()` is used by the session layer. Register `src/features/firearms/FirearmForm.tsx`, `DisposeDialog.tsx`, `RestoreDialog.tsx`, `src/features/insurance/InsurancePolicyForm.tsx` and `CoverageDialog.tsx`, each with a label like "Glock 19 (edit)" or "New firearm" (research §16). The draft staging is added in US6 (T133)
- [X] T067 [US2] Create `src/features/session/UnsavedChangesPrompt.tsx`: a `ConfirmDialog` with the third action, titled "Save changes to <label>?", with **Save changes** (primary), **Discard changes** (alternative) and **Cancel**, per contracts/ui-databases.md §6. In `src/features/session/SessionProvider.tsx`, route close, switch and `app:quit-requested` through it, then call `close_database`/`quit_application`. Listen for `session:closing` and `session:closed` (drop all collection state, select `databasePath`, show the notice) (depends on T064, T066)
- [X] T068 [US2] Create `src/features/databases/DatabaseMenu.tsx`, the top-bar button at the left of `hd-topbar__tools` showing the name with a chevron, opening `Menu` with **Switch database…** and **Close database** (the other items come in later stories). Mount it in `src/features/app/AppShell.tsx` (contracts/ui-databases.md §4) (depends on T065, T067)
- [X] T069 [US2] Complete `src/features/databases/DatabaseChooser.tsx` and `RecentDatabaseRow.tsx` for US2 (contracts/ui-databases.md §1):
  - unavailable rows with **Locate…** (native file picker) and **Remove from list**
  - the overflow **Remove from list** with its note
  - the open-failure table rows for `DATABASE_IN_USE`, `DATABASE_NEWER_VERSION`, `DATABASE_NOT_FOUND`, `DATABASE_UNREADABLE` and `DATABASE_OPEN_ELSEWHERE`
  - the `closed` and `takenOver` notices
  - create `src/features/databases/TakeOverConfirm.tsx` (destructive `ConfirmDialog`, confirm label "Take over", description as in §1); the resent open shows the row's busy state
  - `DATABASE_UNAVAILABLE` from any save shows §1's "can't reach" text in the saving form's error slot, keeping the input, and does not close the session
  
  Add `close_database`, `quit_application`, `remove_recent_database` and `locate_database` wrappers to the services (depends on T067)
- [X] T070 [US2] Add the screens `17-open-elsewhere` (the seeded "Shared collection") and `25-unsaved-changes` (the save / discard / cancel prompt over an edited firearm form, contracts/ui-databases.md §6) to `e2e/screenshots/screens.e2e.ts`, and re-run the E2E spec `us7-databases.e2e.ts` (depends on T069)

**Checkpoint**: User Stories 1 and 2 work: several databases, switching, the recent list, the unsaved-changes question, and refusals with take-over.

---

## Phase 5: User Story 3 - Automatic Backups and Restoring From One (Priority: P3)

**Goal**: A backup when a changed database is closed, at most once a day, rotated to the keep count, with progress and skip; settings to turn them off, move them and delete them all; restore from any backup, including for a damaged database.

**Independent Test**: With a simulated date, change and close a database across several days. Check that one backup appears per day with changes, none after an unchanged session, and never more than the keep count. Restore an earlier backup and check its content returns exactly. Turn backups off and check none are made.

### Follow-ups from User Story 2 (settle before T071)

- [X] T148 [US3] Stop `std::fs` reads of an open database from dropping its lock. On Linux and macOS, closing any file descriptor on a file cancels every POSIX lock the process holds on it, SQLite's exclusive lock included (research §2, §6). `db::verify_passphrase` (T017) opens and closes a descriptor to copy page 1, and the planned `copy_chunked` backup and restore copy (research §3) would do the same, after which another copy of HoploDex could open a database that is in use. First write a failing test that shows it across processes (the second opener must be another process, since SQLite shares lock state within one), then choose and record a fix in research §3, and apply it to `verify_passphrase` before the backup code is built on the same approach. The take-over fingerprint (T060) already avoids it by using `stat` only
- [X] T149 [US2] Take-over asks for the passphrase again rather than keeping the one that found the marker, which FR-007 forbids: `TakeOverConfirm` carries its own `PassphraseField`, read once and cleared on confirm, and an empty one keeps the dialog open. `DatabaseChooser` holds no passphrase between the refusal and the resend. contracts/ui-databases.md §0 and §1 say so, and `DatabaseChooser.test.tsx` covers the retyped, empty and wrong cases

### Tests for User Story 3 (mandatory per constitution)

- [X] T071 [P] [US3] Write `src-tauri/tests/backup_test.rs` with an injected clock and time zone. Close triggers:
  - a close after changes makes one backup named `<name> <YYYY-MM-DD HHMMSS> <id8>.hoplodex` in `<db dir>/HoploDex backups/`, then sets `changes_waiting = 0` and `last_backup_at`
  - a second close with changes the same day makes none, but the next day's close with no new changes makes one (US3-1a)
  - an unchanged session makes none
  - with backups off none is made, and existing ones are kept (US3-8)
  - a dropped connection (simulated crash) leaves `changes_waiting = 1`, and the next allowed close backs it up (US3-1b)
  
  Rotation and listing:
  - rotation keeps `backup_keep_count`, secure-deletes the oldest, and deletes nothing before the new backup is complete
  - listing selects by `<id8>` and the timestamp pattern, ignores `.partial` files and other databases' backups in a shared custom folder, and sorts newest first
  
  Contents of a backup:
  - it opens with the passphrase, has its marker cleared and `pending_changes` empty, and has `backup_made_at` and `backup_of_name` set
  - opening it directly gives it a new `database_id`, clears the stamp and sets `notes.openedBackup` (research §9)
  
  Failures, skips and progress:
  - a missing or unwritable location, or a full one (free space below the file size + 5%), still closes, leaves the changes waiting, returns `CloseOutcome.backup = "failed"` and pushes a `backupFailed` notice
  - skip during the copy removes the `.partial` and leaves the changes waiting
  - a close with `storage_lost` set makes no backup, writes nothing, returns `failed` with `failureReason: "databaseUnreachable"` and pushes a `backupFailed` notice with that reason (research §6)
  - a leftover `unfinishedBackup` at the next launch removes the partial file and pushes the "did not finish" notice (US3-7)
  - the first `backup:progress` event arrives within 100 ms, with `showNow` from the 50 MiB/s estimate (SC-005)
  
  Interruption points (SC-004), each through `Operations::stop_running` or a dropped connection: at the start (before the first chunk), midway through the copy, and at `finalize` (after the stamp, before the rename). After each, the database opens with its passphrase and all its content, with `changes_waiting` still 1, no file that `list` would return has been added, no `.partial` is left once the next launch's sweep has run, and the next allowed close makes the backup
  
  Deleting all backups (FR-029, US3-4), against real backup files made by earlier closes:
  - `delete_all_backups { confirmed: false }` gives `CONFIRMATION_REQUIRED` and deletes nothing
  - confirmed, it deletes every backup `list` returns for this database, each removed through `secure_delete` (its progress callback reports each overwrite), returns `deletedCount` equal to their number with `failedPaths` empty, and emits `backups_delete:progress` per file
  - in a shared custom folder, another database's backups and unrelated files are untouched
  - a backup that cannot be deleted (read-only file) is listed in `failedPaths` and the rest are still deleted
  - the database itself is byte-identical before and after, and `changes_waiting` and `last_backup_at` are unchanged (deleting backups is not a change, FR-025)
  - the next due close makes a new backup as usual
- [X] T072 [P] [US3] Write `src-tauri/tests/file_swap_test.rs` (research §4), covering:
  - the hard-link path replaces the file and leaves `.<file>.old` for secure deletion
  - the fallback (hard links disabled through a test hook) recovers at the next open from each gap: the original missing with `.new` present completes the swap; only `.old` present renames it back
  - a final rename that keeps failing removes `.new`, keeps the original byte-identical and gives `REPLACE_FAILED`
  - the free-space check reports `bytesNeeded` as the file size + 5%
- [X] T073 [P] [US3] Write `src-tauri/tests/restore_test.rs` (FR-028, SC-004, SC-007), covering:
  - restoring an earlier backup reproduces exactly that content (row counts and blobs)
  - a "before restoring" backup is made even when one was already made today
  - the backup restored from is never removed by the rotation that follows
  - the restored database has `changes_waiting = 0`, the backup stamp cleared and its identity kept
  - it opens with the backup's passphrase, and the status has `notes.restoredWithPassphraseOf`
  - a wrong backup passphrase gives `PASSPHRASE_INCORRECT` and changes nothing
  - an interruption during the copy, during the check and just before the rename leaves the database opening with its old passphrase and content, with no `.new` left
  - restoring a damaged database (`databasePath` given, nothing open, backup folder from the recent entry's cached `backupFolder`/`databaseId`) keeps the damaged file as `<name> damaged <YYYY-MM-DD HHMMSS>.hoplodex` and returns `notes.damagedFileKeptAt`
  - `DATABASE_DAMAGED` from a truncated file carries `backupsAvailable`
  
  Preconditions and failures (FR-028, research §8):
  - too little space for the restored copy, or for the "before restoring" backup (test hook on `disk_space`, including both folders on one volume), gives `INSUFFICIENT_SPACE` before anything is written: the database is byte-identical and no `.new` or `.partial` exists
  - a missing or unwritable backup location gives `BACKUP_LOCATION_UNAVAILABLE` the same way
  - with backups turned off, the "before restoring" backup is still made
  - a "before restoring" backup that fails or is stopped abandons the restore: the database is byte-identical, and its `.partial` and the `.new` copy are gone
  - restoring a damaged database checks only the space for the restored copy
- [X] T074 [P] [US3] Extend `src-tauri/tests/deletion_wipe_test.rs` for the secure-delete extension (research §12): a file larger than 1 MiB is overwritten in 1 MiB chunks with a progress callback, and a failed punch-hole or discard step is ignored
- [X] T075 [P] [US3] Write `src/features/session/ClosingScreen.test.tsx` (contracts/ui-databases.md §5, SC-005): "Closing <name>…" appears on `session:closing`; with `showNow` the "Backing up <name>…" bar appears at once with `aria-valuenow`; without it the bar appears only if the close is still running 1 s later (fake timers); **Skip this backup** calls `skip_backup` and shows "Its changes will be backed up next time."
- [X] T076 [P] [US3] Write `src/features/databases/DatabaseSettingsDialog.test.tsx` (Backups section, contracts/ui-databases.md §7), covering:
  - **Make automatic backups**, **Keep the latest** (`hd-field--quarter`, 1–100) and the location with "(next to the database)", **Change…** and **Use the default**
  - "Not available on this computer" for an unavailable custom location
  - the four FR-029 statements
  - **Delete all backups…** opens the destructive confirm "Delete all <n> backups of <name>? …"
  - **Save** sends `update_backup_settings`, and `VALIDATION_ERROR` lands on the fields
- [X] T077 [P] [US3] Write `src/features/databases/RestoreBackupDialog.test.tsx` (contracts/ui-databases.md §9), covering:
  - a radio list of backups, newest first ("25 September 2026, 14:30 — 212 MB")
  - an empty or unavailable folder is named
  - the passphrase note "Enter the passphrase <name> had on <date>…"
  - the "backed up first" statement, and the damaged-file statement when restoring a damaged database
  - **Restore** asks "Replace <name> with the backup from <date>?"
  - progress phase labels, and the dialog cannot be dismissed while it runs
  - `PASSPHRASE_INCORRECT` for the backup lands on the field
  - the refusals `INSUFFICIENT_SPACE` and `BACKUP_LOCATION_UNAVAILABLE` (with **Change backup location…**), and the cancelled-restore text when the "before restoring" backup fails, each keep the dialog open and say "Nothing has been changed."
  
  Also extend `src/features/databases/DatabaseChooser.test.tsx` (US3 cases, US3-6, FR-028): `DATABASE_DAMAGED` shows "<name> is damaged and can't be opened." with **Restore from a backup…** when `backupsAvailable` and without it otherwise; `PASSPHRASE_INCORRECT` with `backupsAvailable` also offers it; choosing it opens `RestoreBackupDialog` in damaged-database mode with the row's `databasePath`
- [X] T078 [P] [US3] Extend `src/features/import-export/ExportDialog.test.tsx` for the FR-031 wording: "Exports the collection to a spreadsheet. The file is **not encrypted**: anyone who can open it can read it. For encrypted backups of the whole database, see Database settings." (contracts/ui-databases.md §12)
- [X] T079 [P] [US3] Write `e2e/specs/us8-backups.e2e.ts`: change a firearm and close; a `HoploDex backups` folder appears next to the file with one backup; change and close again: still one; restore that backup with its passphrase; a "before restoring" backup appears and the change is gone

### Implementation for User Story 3

- [X] T080 [US3] Add `backup_made_at TEXT` ("UTC ISO-8601; set only in backup copies") and `backup_of_name TEXT` ("set only in backup copies") to `app_state` in `src-tauri/src/db/migrations/0001_initial.sql`, in place (data-model.md, research §9)
- [X] T081 [P] [US3] Create `src-tauri/src/services/disk_space.rs` with `available_space(dir)` via `fs4` and `check_room_for_copy(file_len, dir) -> Result<(), InsufficientSpace { bytes_needed, bytes_available, path }>`, which requires the file size + 5% (research §4, FR-016)
- [X] T082 [P] [US3] Extend `src-tauri/src/services/secure_delete.rs` (research §12). Large files are overwritten in 1 MiB chunks (the 64 KiB chunk stays for small document copies), with an optional progress callback and cancel check. After the overwrite and flush, ask the storage to discard the blocks, best effort with failures ignored: `fallocate(FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE)` on Linux, `fcntl(F_PUNCHHOLE)` on macOS, `FSCTL_SET_ZERO_DATA` on Windows
- [X] T083 [P] [US3] Create `src-tauri/src/services/file_swap.rs` per research §4 and contracts/database-file.md "Replacing the file". `replace(original, new_copy)` works with the connection already closed: `.<file>.new` in the same folder, hard-link the original to `.<file>.old`, `rename(.new → original)` with a directory fsync where supported, then secure-delete `.old` and report whether it went. The fallback without hard links is two renames. A failing final rename is retried for about 2 s, then `.new` is removed and `REPLACE_FAILED` returned. Add `recover(path)`, called at every open of a path before `db::open_database`: it completes or undoes an interrupted swap
- [X] T084 [US3] Create `src-tauri/src/services/backups.rs` (research §7, data-model.md "Backup files"; depends on T081, T082):
  - `is_due(app_state, settings, now_local)`: backups on, `changes_waiting = 1`, and `last_backup_at` not today in local time
  - `resolve_folder(db_path, location)`: `'default'` → `<db dir>/HoploDex backups`
  - `backup_file_name(name, now_local, database_id)`
  - `list(folder, database_id)`: matches the pattern and `<id8>`, never `.partial`, newest first
  - `copy_chunked(src, dst_partial, cancel, progress)`: 1 MiB chunks, cancellable, reporting bytes
  - `stamp_backup(conn, partial_path, name, now)`: `ATTACH` without a key, clear the marker, empty `pending_changes`, set `backup_made_at`/`backup_of_name`, `DETACH`
  - `finalize` (flush, rename)
  - `rotate(folder, database_id, keep, protected: Option<&Path>)`, with secure deletion
  - `estimate_seconds(len)` at 50 MiB/s
- [X] T085 [US3] Insert the backup step into `close_normal` in `src-tauri/src/session/lifecycle.rs` (FR-025, FR-027; depends on T084, T021):
  1. If `is_due`: check the location and space (on failure, `CloseOutcome.backup = "failed"`, a `backupFailed` notice and no write)
  2. Record `unfinishedBackup` in `machine.json`
  3. Register a `backup` operation and copy, emitting `backup:progress { processed, total, showNow }`
  4. Stamp, finalize and rotate
  5. In one transaction in the main database, set `changes_waiting = 0` and `last_backup_at = now` and clear the open marker
  6. Clear `unfinishedBackup`
  
  A skip removes the partial file and leaves the changes waiting (`skipped`). With `storage_lost` set, none of the steps run and the close returns `failed` with `failureReason: "databaseUnreachable"` (research §6). Return `made`, `notDue`, `alreadyToday` or `off`. The clock is injectable for tests
- [X] T086 [US3] Update `open` in `src-tauri/src/session/lifecycle.rs` (depends on T083):
  - call `file_swap::recover(path)` first
  - a file carrying `backup_made_at` gets a new random `database_id` and its stamp cleared (housekeeping), and `notes.openedBackup` is reported once (research §9)
  - cache the resolved backup folder in the recent entry
  
  In `src-tauri/src/main.rs` `setup`, run the unfinished-backup sweep: remove the partial file and push the "did not finish" notice
- [X] T087 [US3] Add `validate_backup_settings_input` to `src-tauri/src/models/database.rs`: `backup_keep_count` "1–100"; `backup_location` "`default`, or an absolute path; a path that does not exist is accepted … but is reported on save as currently unavailable" (`VALIDATION_ERROR`). Add `update_backup_settings` (a collection change through `session.write`, returning `CollectionSettings` with `location.available`) and `skip_backup` to `src-tauri/src/commands/databases.rs`, and fill `DatabaseStatus.settings` from `collection_settings`
- [X] T088 [US3] Create `src-tauri/src/commands/backups.rs` (declare it in `src-tauri/src/commands/mod.rs`) with thin commands over `pub mod ops` (depends on T083–T086):
  - `list_backups { databasePath? }` → `{ folder, available, backups }` (from the recent entry's cached folder and id when nothing is open)
  - `restore_backup` 🔑, following research §8 steps 1–6. Before anything is written, check the backup location (`BACKUP_LOCATION_UNAVAILABLE`) and the free space for the restored copy and the "before restoring" backup, summed when on one volume (`INSUFFICIENT_SPACE`); a damaged database needs only the first. Then copy to `.<file>.new` with `restore:progress` phases `copying`, `checking`, `savingCurrent`, `replacing`; open with the backup's passphrase; verify with `cipher_integrity_check`, `integrity_check` and the migration check; stamp (`changes_waiting = 0`, stamp cleared, identity kept); make the "before restoring" backup ignoring the once-a-day limit and whether backups are on, with the restored-from file protected from rotation, and abandon the restore (removing its `.partial` and `.new`) if it fails or is stopped; close; `file_swap::replace`; reopen with the backup's passphrase and set `notes.restoredWithPassphraseOf`. For a damaged database, rename it to `<name> damaged <YYYY-MM-DD HHMMSS>.hoplodex` and set `notes.damagedFileKeptAt`
  - `delete_all_backups { confirmed }`, with `CONFIRMATION_REQUIRED` when false and `backups_delete:progress`. It deletes only the files `backups::list` returns for this database, each with `secure_delete`, and collects the ones that fail in `failedPaths`. It checks `is_cancelled()` between files and passes the cancel check into `secure_delete`; when stopped it returns `OPERATION_STOPPED { operation: "deleteBackups", deletedCount }`, and the file whose overwrite was under way is removed without finishing it (FR-037)
  
  Each long command registers in `Operations`. `open_database` failures `DATABASE_DAMAGED` and `PASSPHRASE_INCORRECT` set `backupsAvailable` from the listing
- [X] T089 [US3] Register `update_backup_settings`, `skip_backup`, `list_backups`, `restore_backup` and `delete_all_backups` in `src-tauri/src/main.rs` (depends on T087, T088)
- [X] T090 [P] [US3] Create `src/features/session/ClosingScreen.tsx` per contracts/ui-databases.md §5. `SessionProvider` shows it within 100 ms of `session:closing`, in place of the app shell. It has a `ProgressBar` from `backup:progress` (immediate on `showNow`, otherwise after 1 s) and **Skip this backup**. Add the `skip_backup` wrapper to `sessionService.ts`
- [X] T091 [US3] Create `src/features/databases/DatabaseSettingsDialog.tsx` with its Backups section (contracts/ui-databases.md §7): `Dialog size="lg"` titled "<name> settings"; fieldsets; **Keep the latest** with `hd-field--quarter`; the location row with **Change…** (native folder picker) and **Use the default**; the FR-029 statements; **Restore from a backup…**; **Delete all backups…** with a destructive `ConfirmDialog` and progress. The footer has **Save**. Add **Database settings…** to `DatabaseMenu.tsx` and the wrappers to `databasesService.ts`
- [X] T092 [US3] Create `src/features/databases/RestoreBackupDialog.tsx` per contracts/ui-databases.md §9. It takes the backup list, a `PassphraseField` for the backup, the statements, the destructive confirm step, and progress from `restore:progress` with the phase labels. It shows §9's refusal texts for `INSUFFICIENT_SPACE`, `BACKUP_LOCATION_UNAVAILABLE` and a failed "before restoring" backup, keeping the dialog open. Afterwards the restored database is open, and a notice says it now opens with the passphrase from <date>. Add **Restore from a backup…** to `DatabaseMenu.tsx`, and to the chooser's `PASSPHRASE_INCORRECT` and `DATABASE_DAMAGED` failures when `backupsAvailable` (the damaged-database mode passes `databasePath`) (US3-6)
- [X] T093 [US3] Add the chooser's `backupFailed` notice ("<name> was not backed up: <reason>. Its changes will be backed up at the next close.", including the reason "its file could not be reached") with **Change backup location…**, which opens the settings after the next successful open. Add the opened-backup and restored banners to `src/features/databases/DatabaseNotes.tsx` (contracts/ui-databases.md §1, §10), and handle the unfinished-backup notice. Files: `src/features/databases/DatabaseChooser.tsx`, `src/features/databases/DatabaseNotes.tsx`
- [X] T094 [P] [US3] Change the export wording in `src/features/import-export/ExportDialog.tsx` to the FR-031 text (contracts/ui-databases.md §12)
- [X] T095 [US3] Seed backups in `src-tauri/examples/human_seed.rs`: two backups of "Main collection" through `services::backups` into its custom location (so `backup_made_at` and `backup_of_name` are populated in a seeded backup), the newer one matching the `last_backup_at` T027 set. Extend `src-tauri/tests/human_seed_coverage_test.rs` to check the backup-only columns in the seeded backup file (plan.md) (depends on T080, T084)
- [X] T096 [US3] Add the screens `18-closing-backup`, `19-database-settings` and `21-restore-backup` to `e2e/screenshots/screens.e2e.ts`, and run the E2E spec `us8-backups.e2e.ts` (depends on T090–T093)

**Checkpoint**: User Stories 1–3 work. Backups are made at close and can be restored.

---

## Phase 6: User Story 4 - Change My Passphrase Safely (Priority: P4)

**Goal**: The passphrase is changed by copy, verify and replace, with a space check, progress, secure deletion of the old file and a completion notice. An interruption at any point leaves the old file opening with the old passphrase.

**Independent Test**: Change a database's passphrase. It opens only with the new one, its content is identical, and the previous file is gone. Interrupting the change leaves it opening with the old passphrase and its content intact.

### Tests for User Story 4 (mandatory per constitution)

- [X] T097 [P] [US4] Write `src-tauri/tests/passphrase_change_test.rs` (FR-015, FR-016, SC-004), covering:
  - after a change the database opens with the new passphrase and refuses the old one
  - row counts for every table and the photo and document blobs are identical
  - `.old` and `.new` are gone and `oldFileRemoved = true`
  - `changes_waiting = 1` in the new file, so the next allowed close backs it up (US3-9)
  - a wrong current passphrase gives `PASSPHRASE_INCORRECT` with `fieldErrors.currentPassphrase`, detected by the page-1 probe, with the file byte-identical
  - a short new passphrase gives `fieldErrors.newPassphrase`
  - too little space (test hook on `disk_space`) gives `INSUFFICIENT_SPACE` before anything is written
  - an interrupt through `Operations::stop_running` during the export, during the check and just before the rename leaves the database opening with the old passphrase and all content, with no `.new` left
  - an old file that cannot be securely deleted gives `oldFileRemoved = false` with `oldFilePath`
  - `passphrase_change:progress` reports `copying` with growing `processed`, then `checking` and `replacing`
  - the session is open again afterwards and the fingerprint is refreshed (writes succeed)
- [X] T098 [P] [US4] Write `src/features/databases/ChangePassphraseDialog.test.tsx` (contracts/ui-databases.md §8), covering:
  - the fields **Current passphrase**, **New passphrase** (with strength) and **Confirm new passphrase**
  - the frontend length and match checks
  - while running, the body is replaced by a `ProgressBar` with "Making a copy with the new passphrase…", "Checking the new copy…" and "Replacing the database…", and the dialog cannot be dismissed
  - the completion text, and the "could not be deleted. It is at <path>…" text
  - the `INSUFFICIENT_SPACE` text "Changing the passphrase needs <size> free on the database's drive; <available> is free."
  - `fieldErrors.currentPassphrase` lands on the field

### Implementation for User Story 4

- [X] T099 [US4] Implement `ops::change_passphrase` in `src-tauri/src/commands/backups.rs` (research §3, §4; FR-015, FR-016):
  1. Validate the new passphrase
  2. `db::verify_passphrase` for the current one, using the app cache directory as scratch
  3. `disk_space::check_room_for_copy`
  4. Register a `passphraseChange` operation with the connection's `InterruptHandle`
  5. `ATTACH DATABASE ?tmp AS rekey KEY ?newPassphrase` to `.<file>.new`, then `apply_cipher_settings(conn, "rekey")` and `SELECT sqlcipher_export('rekey')`, with a monitor thread polling the destination size every 100 ms and emitting `passphrase_change:progress` against `page_count × page_size`
  6. Stamp the copy: marker cleared, `pending_changes` emptied, `changes_waiting = 1`. Then `DETACH`
  7. Verify with a fresh connection using the new passphrase: `cipher_integrity_check`, `integrity_check`, the migration check, and a row count for every table equal to the source's
  8. Close the session's connection and `file_swap::replace`
  9. Reopen with the new passphrase, held for this operation only
  
  Any failure removes `.new` and leaves the original untouched. Return `{ oldFileRemoved, oldFilePath?, passphraseSaved }`. The keyring update is added in T109
- [X] T100 [US4] Add the `change_passphrase` 🔑 command to `src-tauri/src/commands/backups.rs` and register it in `src-tauri/src/main.rs` (depends on T099)
- [X] T101 [US4] Create `src/features/databases/ChangePassphraseDialog.tsx` per contracts/ui-databases.md §8, with three `PassphraseField`s read once on submit and then reset, progress from `passphrase_change:progress`, and the completion and failure texts. Add **Change passphrase…** to `DatabaseMenu.tsx` and the wrapper to `databasesService.ts` (depends on T100)
- [X] T102 [US4] Add the screen `20-change-passphrase` to `e2e/screenshots/screens.e2e.ts` (depends on T101)

**Checkpoint**: User Stories 1–4 work. The passphrase can be changed safely.

---

## Phase 7: User Story 5 - Optionally Let This Computer Remember My Passphrase (Priority: P5)

**Goal**: An opt-in, per-database, per-computer saved passphrase in the OS keyring, with its disclosure, "forget saved passphrase", refresh when it goes stale, and an "unavailable" state when there is no keyring.

**Independent Test**: Opt in for one database and relaunch: it opens without a prompt while another database still asks. Choose "forget saved passphrase": the prompt returns and the keyring entry is gone.

### Tests for User Story 5 (mandatory per constitution)

- [X] T103 [P] [US5] Write `src-tauri/tests/keyring_test.rs`, run with `--features mock-keyring` (FR-017–FR-019, SC-008), covering:
  - saving with a checked passphrase creates service `com.hoplodex.app`, user `passphrase:<database_id>`, with the NFC passphrase
  - `open_database { useSavedPassphrase: true }` opens without a typed passphrase, while another database has no entry
  - `rememberPassphrase` on a typed open writes the entry
  - `forget_saved_passphrase` removes it
  - `remove_recent_database` removes it
  - a stale saved passphrase gives `PASSPHRASE_INCORRECT { savedPassphraseFailed: true }`, and the next successful typed open overwrites the entry
  - `change_passphrase` updates it, and `restore_backup` sets it to the backup's
  - a backup opened directly (new id) does not inherit it
  - `save_passphrase` with a wrong passphrase gives `PASSPHRASE_INCORRECT` through the page-1 probe
  - `HOPLODEX_E2E_KEYRING=unavailable` makes the probe report unavailable and `save_passphrase` give `KEYRING_UNAVAILABLE`, while everything else works
  - a pre-seeded mock `sqlcipher-key` entry is never read, changed or deleted by any of the above (research §10)
- [X] T104 [P] [US5] Extend `src/features/databases/DatabaseChooser.test.tsx` (US5 cases, contracts/ui-databases.md §1, §8), covering:
  - **Remember on this computer** is off by default
  - ticking it opens the FR-017 confirmation (confirm label "Remember passphrase"), and the box stays unticked unless confirmed
  - a saved row shows "Opens without a passphrase on this computer" and only **Open**
  - `savedPassphraseFailed` shows "The saved passphrase no longer opens <name>. Enter its passphrase; the saved copy will be updated." with the prompt
  - with `keyringAvailable: false` the box is disabled with "Not available: this computer has no keyring service."
- [X] T105 [P] [US5] Extend `src/features/databases/DatabaseSettingsDialog.test.tsx` (This computer section): the saved state is shown; turning it on shows the confirmation and then a `PassphraseField`; **Forget saved passphrase** calls `forget_saved_passphrase`; the unavailable text is shown
- [X] T106 [P] [US5] Add the US5 flows to `e2e/specs/us7-databases.e2e.ts` (the E2E build uses `mock-keyring`): remember on open, relaunch, and the database opens with **Open** alone; the second database still asks; forget in settings, relaunch, and the prompt returns. Add a variant launched with `HOPLODEX_E2E_KEYRING=unavailable` in which the option is disabled

### Implementation for User Story 5

- [X] T107 [US5] Create `src-tauri/src/services/keyring.rs` (research §10; data-model.md "Keyring: saved passphrase"). It offers `save(database_id, &Passphrase)`, `load(database_id) -> Option<Passphrase>`, `forget(database_id)`, and `probe()`, which reads a known-absent entry: `NoEntry` means available; `PlatformFailure`/`NoStorageAccess` means unavailable, logged without the raw reason reaching the UI, and the result is cached for the session. It uses service `com.hoplodex.app` and user `passphrase:<database_id>` only. Under `mock-keyring`, `HOPLODEX_E2E_KEYRING=unavailable` forces unavailable. It never names `sqlcipher-key`
- [X] T108 [US5] Extend `src-tauri/src/commands/databases.rs` (depends on T107):
  - `open_database` accepts `useSavedPassphrase` (loads from the keyring through the recent entry's cached `databaseId`; a failure gives `savedPassphraseFailed`) and `rememberPassphrase`
  - a typed success writes or refreshes the entry and sets `passphraseSaved` in `machine.json`
  - `get_chooser_state` reports `keyringAvailable` from `probe()`
  - add `save_passphrase` 🔑 (checked with `db::verify_passphrase`) and `forget_saved_passphrase { path? }`
  - `remove_recent_database` also forgets the saved passphrase (FR-018)
  
  Register the two new commands in `src-tauri/src/main.rs`
- [X] T109 [US5] Hook the keyring into `ops::change_passphrase` (update the saved copy to the new passphrase, US4-7) and `ops::restore_backup` (set it to the backup's passphrase, research §8) in `src-tauri/src/commands/backups.rs`, returning `passphraseSaved` (depends on T107)
- [X] T110 [P] [US5] Create the FR-017 confirmation `src/features/databases/RememberPassphraseConfirm.tsx`, a non-destructive `ConfirmDialog` with confirm label "Remember passphrase" and the exact facts in contracts/ui-databases.md §8
- [X] T111 [US5] Add the saved-passphrase flow to the chooser in `src/features/databases/DatabaseChooser.tsx` and `RecentDatabaseRow.tsx` (contracts/ui-databases.md §1): the **Remember on this computer** checkbox, the confirmation, the saved row with **Open** only (focused), the stale-passphrase text, and the unavailable state. Add the `save_passphrase` and `forget_saved_passphrase` wrappers (depends on T108, T110)
- [X] T112 [US5] Add the "This computer" section to `src/features/databases/DatabaseSettingsDialog.tsx`: the current state, turning it on (confirmation, then a `PassphraseField` → `save_passphrase`), **Forget saved passphrase**, and the unavailable text (contracts/ui-databases.md §7) (depends on T110)

**Checkpoint**: User Stories 1–5 work. A passphrase can be remembered on this computer and forgotten again.

---

## Phase 8: User Story 6 - Lock the Application When I Step Away (Priority: P6)

**Goal**: "Lock now", the idle lock, the lock at sleep and the optional lock at screen lock all close the database completely. Unsaved form input is kept as pending changes inside the database and offered at the next open. OS shutdown keeps pending changes too. Long operations are stopped at sleep. Passphrase fields clear on sleep and screen lock.

**Independent Test**: Open a database and choose "lock now". The chooser shows it selected with its passphrase prompt open, no collection data is shown or held, decrypted document copies are gone, and a due backup was made. Enter the passphrase and the collection opens again.

### Tests for User Story 6 (mandatory per constitution)

- [X] T113 [P] [US6] Write `src-tauri/tests/pending_changes_test.rs` (FR-039, research §16), covering:
  - `stage_pending_changes` keeps the draft in memory only (no row written) and refuses a `values` of more than 1 MiB serialized, or an invalid `kind`/`mode` pair, with `VALIDATION_ERROR`
  - a lock writes the staged draft to `pending_changes` before the connection closes, and leaves `changes_waiting` unchanged; at sleep and shutdown (`close_immediate`) the same write also clears the marker, and with no draft staged the marker is still cleared, with no `pending_changes` row written
  - the next open reports `pendingChanges` (`resumable = false` when the target firearm or policy no longer exists)
  - collection commands give `PENDING_CHANGES_UNRESOLVED` until `resolve_pending_changes`
  - `resume` returns the exact draft and removes the row; `discard` removes it
  - a backup made after a lock carries no pending changes
  - pending changes that cannot be written (read-only folder) still lock and push `pendingChangesLost`
  - a write refused by a take-over during the finish-on-wake is lost and reported (spec edge case)
- [X] T114 [P] [US6] Write `src-tauri/tests/lock_test.rs` (FR-033–FR-038, SC-010). Lock now:
  - `lock_database { draft }` saves the draft as pending, then runs the normal close with reason `lockedByUser` (a due backup made, marker cleared, document copies gone), and `selectedPath` is the locked database
  
  Idle clock (injected wall and monotonic clocks):
  - it locks between 10:00 and 10:01 after the last `note_activity`
  - activity restarts it
  - a registered operation (a deletion of all backups included) or `set_idle_paused { nativeDialog }` pauses it, and resuming starts from zero
  - a wall-clock jump longer than the duration locks at the next tick
  - the idle lock off never locks
  - `update_lock_settings` restarts the clock
  
  Sleep, with the idle lock on:
  - `session:closed { reason: "sleep" }` is emitted before the connection closes
  - the pending draft is written before the connection is dropped
  - no backup is started, and `changes_waiting` stays 1
  - document copies are deleted and partial files removed, in FR-037's order
  - a running import stops after the current row, keeps the imported rows and reports `OPERATION_STOPPED { importedCount }`
  - a passphrase change is interrupted and the old file opens with the old passphrase
  - a backup or restore is abandoned and the original is unchanged
  - an export's partial file is removed
  - a deletion of all backups stops between files: the backups already deleted are gone, the rest are still listed, none is left half overwritten (the file in progress is removed), and it reports `OPERATION_STOPPED { operation: "deleteBackups", deletedCount }`
  - an `operationStopped` notice is pushed
  - the open marker is cleared once the lock finishes, with and without a staged draft
  
  Sleep with the idle lock off does not lock a database that is open and not closing.
  
  Sleep during a close, and finishing on wake (FR-037, FR-038, research §14):
  - with the idle lock off and `lock_on_screen_lock` on, a sleep during the screen-lock backup stops it (`.partial` removed, `changes_waiting` stays 1) and the close finishes as `close_immediate`; the same for a sleep during a user close's backup
  - a sleep lock interrupted after step 1 is completed by `finish_on_wake`, steps 1–3 before any command is accepted, then the rest
  - the wake watchdog (injected wall and monotonic clocks) reports `Woke` when wall time runs more than 5 s ahead of monotonic time, and not otherwise
  
  Screen lock with `lock_on_screen_lock`: a normal close with a due backup. Off: nothing happens.
  
  Shutdown:
  - the staged draft is saved before document copies are deleted, and no backup is made
  - `system:clear-passphrase-fields` is emitted on sleep and screen lock whether or not a database is open
  - `validate_lock_settings_input` enforces `idle_lock_minutes` "1–240"
- [X] T115 [P] [US6] Extend `src-tauri/tests/import_export_test.rs`: with the cancel flag set after N rows, `import_collection` stops after the row in progress, keeps each imported row complete and returns `OPERATION_STOPPED { operation: "import", importedCount: N }`; a stopped `export_collection` removes its partial file and returns `OPERATION_STOPPED { operation: "export" }`
- [X] T116 [P] [US6] Write `src/features/session/useIdleActivity.test.ts` (research §15), covering:
  - `keydown`, `pointerdown`, `pointermove`, `wheel` and `touchstart` call `note_activity` at most once per second, on the leading and trailing edge
  - `withIdlePaused(fn)` sends `set_idle_paused { paused: true }` before and `false` after, even when `fn` throws
  - `pauseIdleForFileInput(input)` sends `paused: true` on the input's `click`, and `false` on its `change`, on its `cancel`, or, when neither fires, on the window's next `focus`; it sends `false` only once per `click`, and removes its listeners on unmount
- [X] T117 [P] [US6] Write `src/features/session/usePendingDraft.test.tsx` (research §16), covering:
  - edits stage a draft with `{ formVersion, kind, mode, targetId, label, values }`, debounced to 250 ms and flushed at once on blur
  - a clean or saved form stages `null`
  - a form given a resumed draft starts with those values as unsaved input, and its dirty state is true
- [X] T118 [P] [US6] Write `src/features/session/PendingChangesDialog.test.tsx` (contracts/ui-databases.md §13), covering:
  - the non-dismissable dialog "Unsaved changes to <label>" with "<name> locked on <date, time> while you were editing <label>. Your changes were kept."
  - **Resume editing** calls `resolve_pending_changes("resume")`, navigates to the target and opens its form with the draft
  - **Discard changes** asks for destructive confirmation first
  - with `resumable: false`, only Discard is offered, with the "no longer exists" sentence
- [X] T119 [P] [US6] Extend the frontend tests for locking:
  - `src/components/PassphraseField.test.tsx`: `system:clear-passphrase-fields` resets every mounted field (FR-007)
  - `src/features/databases/DatabaseMenu.test.tsx`: **Lock now** in the menu, the lock icon button (`aria-label` "Lock now") and Ctrl/⌘+L from any focus, including inside a dialog, call `lock_database` with the current draft and no confirmation (FR-035)
  - `src/features/databases/DatabaseSettingsDialog.test.tsx`, Locking section: **Lock after a period without use**, and the minutes `Select` (1, 2, 5, 10, 15, 30, 60, 120, 240; `hd-field--third`) enabled only when checked, with the help "Also locks when the computer goes to sleep. Turning this off stops both."; **Lock when the computer's screen locks**, disabled with "Not available: this computer doesn't tell applications when the screen locks." when unsupported; the lock statement; the FR-036 sentence when the passphrase is saved
  - `src/features/databases/DatabaseChooser.test.tsx`: the notices "HoploDex locked <name>." (with " after <n> minutes without use" for idle), the stopped-operation text (the import variant with the row count, and the deletion-of-backups variant with the number deleted), and "Unsaved changes could not be kept when <name> locked."; after a lock, focus is on the passphrase field, or on **Open** when the passphrase is saved
- [X] T120 [P] [US6] Write `e2e/specs/us9-locking.e2e.ts`, covering:
  - Ctrl+L while editing a firearm locks, with no collection DOM left
  - the chooser shows the database selected with the locked notice
  - unlock, and the pending-changes dialog names the firearm; Resume brings back the exact input
  - lock again, reopen and Discard; the form is clean
  - set the idle lock to 1 minute through settings, wait, and it locks

### Implementation for User Story 6

- [X] T121 [US6] Create `src-tauri/src/session/pending.rs` (FR-039, research §16):
  - `validate_draft`: "`values_json` ≤ 1 MiB; `kind`/`mode` pair valid", with `coverage` only when `kind = 'firearm'` and `target_id` NULL only for `add`
  - `stage(session, Option<Draft>)`, in memory only
  - `write_pending(conn, draft, clear_marker: bool)`, in one transaction
  - `summary(conn)` with `resumable` (whether the target still exists)
  - `resolve(conn, action)`
  
  Set `OpenDatabase.pending_unresolved` at open when a row exists. Make `Session::write` and `Session::read`, for collection commands only, refuse with `PENDING_CHANGES_UNRESOLVED` while it is set, leaving `get_database_status`, `resolve_pending_changes` and the lifecycle commands allowed. Report `pendingChanges` in `DatabaseStatus`
- [X] T122 [US6] Create `src-tauri/src/session/idle.rs` (research §15). `IdleClock { last_input_wall, paused_by: set<operation | nativeDialog> }` runs on an injectable wall clock. `note_activity()`, `set_paused(reason, bool)` (resuming restarts from zero), and `tick(now, settings) -> bool` (lock when `now − last_input ≥ idle_lock_minutes`, idle lock on, not paused, and `Operations::is_running()` false). A 1 s tick thread spawned by `main.rs` calls the idle lock through the lifecycle
- [X] T123 [US6] Add the lock procedures to `src-tauri/src/session/lifecycle.rs` (FR-033, FR-037, FR-038, FR-039; data-model.md "Closing(immediate)"; depends on T121, T122):
  - `lock(reason: lockedByUser | idle | screenLocked, draft)`: write the draft (or the staged one) as pending changes, then `close_normal(reason)`
  - `close_immediate(reason: sleep | shutdown)`, strictly in this order:
    1. `Operations::stop_running()` and emit `session:closed { reason, databasePath, stoppedOperation }`
    2. write the staged draft as pending, clearing the marker in the same write (with no draft staged, clear the marker alone); on failure push `pendingChangesLost`
    3. drop the connection
    4. delete decrypted document copies
    5. remove the stopped operation's partial files (`.partial`, `.new`, the export file, or the backup whose secure overwrite was under way)
    
    It never makes a backup, and pushes `operationStopped`
  - `close_immediate` also takes over a close already under way, whatever its reason and the idle-lock setting: it stops the running backup and continues from its step 2 (research §14)
  - `finish_on_wake()`: completes any unfinished steps, 1–3 before anything else
  
  The `Session` mutex is taken only after stopping the operation, so a running copy releases it
- [X] T124 [US6] Add cancellation to `src-tauri/src/commands/import_export.rs` (research §13). `import_collection` and `export_collection` register in `Operations` (so the idle clock pauses) and check `is_cancelled()` between rows. An import keeps the rows already committed and returns `OPERATION_STOPPED { operation: "import", importedCount }`. An export removes its partial output file and returns `OPERATION_STOPPED { operation: "export" }`
- [X] T125 [P] [US6] Create `src-tauri/src/platform/mod.rs` (declare `pub mod platform;` in `src-tauri/src/lib.rs`) per research §14. `SystemEvent { WillSleep { ack }, Woke, ScreenLocked, ScreenUnlocked, WillShutDown { ack } }`, where `ack` is a drop guard. `spawn_listener(sender)` picks the OS backend, and `screen_lock_supported()` reports availability. The wake watchdog is a 1 s tick comparing wall and monotonic time: a jump of more than 5 s is treated as `Woke` after an unseen sleep
- [X] T126 [P] [US6] Create `src-tauri/src/platform/linux.rs` using `zbus` (research §14):
  - logind `PrepareForSleep`, holding a `delay` inhibitor for `sleep` that is released when the lock finishes and re-taken on wake
  - `PrepareForShutdown`, with a `shutdown` delay inhibitor
  - the session object's `Lock` signal and `LockedHint`
  - `org.freedesktop.ScreenSaver`/`org.gnome.ScreenSaver` `ActiveChanged`
  - screen lock reported as supported only when the session object or a ScreenSaver interface is reachable at startup
- [X] T127 [P] [US6] Create `src-tauri/src/platform/macos.rs` (research §14):
  - an `extern "C"` block for `IORegisterForSystemPower`, `IOAllowPowerChange`, `IODeregisterForSystemPower` and `IONotificationPortGetRunLoopSource`
  - `kIOMessageSystemWillSleep` answered once the lock finishes, and `kIOMessageSystemHasPoweredOn`
  - the distributed notification `com.apple.screenIsLocked`
  - `NSWorkspaceWillPowerOffNotification`, through `objc2`/`objc2-foundation`/`objc2-app-kit`
  
  The container compiles only Linux code, so build and lint this file on macOS (`cargo clippy --all-targets`) before calling the task done; T144 runs the full gates there
- [ ] T128 [P] [US6] Create `src-tauri/src/platform/windows.rs` (research §14):
  - one hidden, never-shown top-level window (not `HWND_MESSAGE`) on its own thread with its own message loop, via `windows-sys`
  - `WM_POWERBROADCAST` with `PBT_APMSUSPEND` and `PBT_APMRESUMEAUTOMATIC`
  - `WTSRegisterSessionNotification`, then `WM_WTSSESSION_CHANGE`/`WTS_SESSION_LOCK`
  - `WM_QUERYENDSESSION`/`WM_ENDSESSION`, with `ShutdownBlockReasonCreate` while pending changes are saved
  
  The container compiles only Linux code, so build and lint this file on Windows (`cargo clippy --all-targets`) before calling the task done; T144 runs the full gates there
- [X] T129 [US6] Wire the system events in `src-tauri/src/main.rs` (depends on T123, T125–T128):
  - spawn the listener and the idle tick, and manage `IdleClock`
  - on `WillSleep` and `ScreenLocked`, always emit `system:clear-passphrase-fields`
  - `WillSleep` with the idle lock on, or while a close is under way whatever the setting → `close_immediate(sleep)`, then drop `ack`
  - `ScreenLocked` with `lock_on_screen_lock` → `lock(screenLocked)`
  - `Woke` → `finish_on_wake()`
  - `WillShutDown`, the unix SIGTERM/SIGHUP/SIGINT handler (replacing the direct `app.exit(0)` in `exit_on_termination_signals`) and the OS-initiated exit request → `close_immediate(shutdown)`, then exit. Pending changes are saved before document copies are deleted
- [X] T130 [US6] Add the lock commands to `src-tauri/src/commands/databases.rs` and register them in `src-tauri/src/main.rs` (depends on T123, T129):
  - `lock_database { draft } → CloseOutcome`
  - `stage_pending_changes { draft }`
  - `resolve_pending_changes { action } → { draft }`
  - `note_activity`
  - `set_idle_paused { reason: "nativeDialog", paused }`
  - `update_lock_settings`, with `validate_lock_settings_input` in `src-tauri/src/models/database.rs`: `idle_lock_minutes` "1–240". A collection change; it restarts the idle clock
  - `get_chooser_state` reports `screenLockSupported` from `platform::screen_lock_supported()`
- [X] T131 [US6] Make `PassphraseField` listen for `system:clear-passphrase-fields` and reset, whether or not a database is open (FR-007). File: `src/components/PassphraseField.tsx`
- [X] T132 [P] [US6] Create `src/features/session/useIdleActivity.ts`, with the throttled window listeners → `note_activity`, and `withIdlePaused(fn)`. Mount it in `SessionProvider` while a database is open. Wrap every `@tauri-apps/plugin-dialog` call in `withIdlePaused`: `src/features/import-export/ImportDialog.tsx`, `ExportDialog.tsx`, `src/features/databases/CreateDatabaseDialog.tsx` (Choose…), `DatabaseChooser.tsx` (Open another…, Locate…) and `DatabaseSettingsDialog.tsx` (Change…). The photo and document pickers are `<input type="file">` elements with no promise to wrap, so add `pauseIdleForFileInput(input)` to the same hook (research §15) and attach it to the inputs in `src/features/media/PhotoGallery.tsx` and `src/features/media/DocumentList.tsx`
- [X] T133 [US6] Add draft staging to `src/features/session/usePendingDraft.ts` (research §16). Registered forms stage `{ formVersion, kind, mode, targetId, label, values }` via `stage_pending_changes`, debounced to 250 ms, flushed on blur, and `null` when clean. Expose `currentDraft()` for `lock_database`. Make `FirearmForm.tsx`, `DisposeDialog.tsx`, `RestoreDialog.tsx`, `InsurancePolicyForm.tsx` and `CoverageDialog.tsx` accept a resumed draft as their initial unsaved state, each with a `FORM_VERSION` constant (depends on T066)
- [X] T134 [US6] Create `src/features/session/PendingChangesDialog.tsx` per contracts/ui-databases.md §13. `SessionProvider` shows it after any open whose status reports `pendingChanges`, before the collection can be used. **Resume editing** navigates to the record or policy and opens its form with the draft (via `src/features/app/navigation.ts`). **Discard changes** asks for destructive confirmation first (depends on T133)
- [X] T135 [US6] Add locking to the database UI (contracts/ui-databases.md §1, §4, §7, §14):
  - **Lock now** (with the Ctrl+L/⌘L hint) at the top of `DatabaseMenu.tsx`, plus the lock icon button beside the menu
  - a global Ctrl/⌘+L handler that works from any focus, including inside dialogs, and calls `lock_database` with `currentDraft()` and no confirmation
  - the Locking section in `DatabaseSettingsDialog.tsx`, with the FR-036 sentence when the passphrase is saved
  - the chooser notices for locks, stopped operations and lost pending changes in `DatabaseChooser.tsx`
  - focus moves to the passphrase field or **Open** after a lock
  - add the wrappers to `sessionService.ts`
- [X] T136 [US6] Add the screen `22-pending-changes` (the seeded "Shared collection" after take-over, or a lock during an edit) to `e2e/screenshots/screens.e2e.ts`, and run the E2E spec `us9-locking.e2e.ts` (depends on T134, T135)

**Checkpoint**: All six user stories work on their own.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: The in-app guide, performance budgets, documentation, amendments to feature 001's documents, and the PR gates.

- [X] T137 [P] Write `src/features/databases/DatabaseGuide.test.tsx`, then create `src/features/databases/DatabaseGuide.tsx` (FR-030, contracts/ui-databases.md §11). It is a `Dialog` with headed sections in the style of `src/features/firearms/OriginGuide.tsx`, covering:
  - passphrases (why length matters, no recovery, a copied file protected only by the passphrase)
  - saving the passphrase on this computer
  - locking
  - backups (location, contents, the passphrase they open with, deleted records in older backups, same-disk risk, cloud-synced folders)
  - secure deletion (best effort on SSDs, journaling and copy-on-write filesystems, snapshots and cloud folders; an old copy is still protected by its old passphrase)
  - using a database from more than one computer (one at a time, the open marker, take-over, and the bare-window-manager screen-lock caveat)
  - whole-disk encryption
  
  Link it from **About databases and security** in `DatabaseMenu.tsx`, **Why?** in `DatabaseNotes.tsx`, and the "see About databases and security" texts in `ChangePassphraseDialog.tsx` and `DatabaseSettingsDialog.tsx`. Add the screen `23-database-guide` to `e2e/screenshots/screens.e2e.ts`
- [X] T138 [P] Extend `src-tauri/tests/performance_test.rs`: opening a database of 10,000 firearms takes at most 1 s including key derivation (SC-003, constitution IV); the first `backup:progress`, `passphrase_change:progress` and `restore:progress` event each arrives within 100 ms of its operation starting (SC-005); 10,000 `session.write` fingerprint checks add no measurable cost against the existing 500 ms search and 1 s action budgets
- [X] T139 [P] Update DEVELOPMENT.md:
  - "Test isolation": the passphrase model; no DB key environment variable; `HOPLODEX_E2E_KEYRING=unavailable`; the scratch `user-dirs.dirs`; the seed's sandbox marker and refusals; what counts as real application data now (every database, its backups, `machine.json`, the suggested documents folder and the `passphrase:<id>` keyring entries, plus the pre-feature DB and key)
  - "Human testing": two seeded databases, the printed passphrase, "Shared collection" open on "Workshop PC"
  - the `mock-keyring` feature comment in `src-tauri/Cargo.toml`, which no longer describes a random key
- [X] T140 [P] Add the one-line "amended by 003" pointers to feature 001's documents, at each anchor the spec's "Relationship to Feature 001" lists:
  - `specs/001-firearms-inventory/research.md` §5
  - `quickstart.md`'s first-run flow
  - `spec.md` FR-021, FR-035, SC-005 (reworded to call the spreadsheet a data export, FR-031) and the Assumptions
  - `plan.md`'s Constitution rows V and Security & Data Handling, and its Project Structure keyring note
  - `data-model.md`'s storage note and entity list
  - `contracts/tauri-commands.md`'s introduction and error shape
- [X] T141 [P] Reconcile `specs/003-database-protection-management/contracts/tauri-commands.md`, `contracts/ui-databases.md` and `data-model.md` with any command shape, event, text or column that changed during implementation, and check that code comments cite this feature's requirement and research IDs accurately
- [X] T142 Check that nothing touches real application data (constitution 1.2.0, research §21). Grep `src-tauri/tests/`, `src-tauri/examples/`, `e2e/` and `scripts/` for `app_config_dir`, `app_data_dir`, `document_dir`, `dirs::` and keyring use outside `mock-keyring`, and confirm every hit resolves only inside a temp or sandbox directory
- [X] T143 Run the full gates in the dev container:
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - `cargo test --manifest-path src-tauri/Cargo.toml --features mock-keyring --test keyring_test`
  - `npm run test`
  - `cargo clippy --all-targets`, `cargo fmt --check`, `npm run lint`, `npm run format:check`
  - `npm run audit`
  - `npm run build`, then each E2E spec (`us1`–`us9`, `ui-review`) one at a time
  
  Fix every failure (depends on all earlier tasks)
- [ ] T144 On macOS and on Windows, outside the container, run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `cargo test --manifest-path src-tauri/Cargo.toml` and the `mock-keyring` keyring test. This is the only place `platform/macos.rs`, `platform/windows.rs` and the other `cfg(target_os = "macos")`/`cfg(windows)` code are compiled and linted (constitution I), and `portability_test` opening the Linux-made fixture there is SC-001's cross-platform check (research §20). Fix every failure and record the results for the PR (quickstart.md platform checks)
  - macOS done (2026-09-27, macOS 26.7, Rust 1.98.1): clippy, fmt, the full `cargo test` (run with `--no-fail-fast`, since a failing test binary stops the rest, `portability_test` among them) and the keyring test pass. `portability_test` opened the Linux-made fixture. Windows is still to do
- [X] T145 Run `npm run screenshots` and collect the before (main) and after images for the new screens 14–25 and the changed top bar and export dialog, for the PR's UI evidence (contracts/ui-databases.md §15)
- [ ] T146 Walk through quickstart.md's six walkthroughs with `scripts/human-testing.sh` (by hand, against scratch data), timing walkthrough 1's create and a switch to a second database against SC-006's 2 minutes, and do the per-OS platform checks table on each available OS. Record the results for the PR description
- [ ] T147 Draft the PR description: the before/after screenshots, the platform-check results (including T144's Rust gates on macOS and Windows), the security and data-handling note (the attack surface listed in spec.md's Assumptions and how each constraint is met: no passphrase held between commands, pinned cipher settings, refused opens never write, copy-verify-replace, secure deletion, local-only backups, keyring opt-in, test isolation), and the performance note (SC-003 open time and SC-005 progress latency as measured by T138)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies. Start immediately.
- **Foundational (Phase 2)**: Depends on Setup (T001 for the crates, T002 for the npm packages). **Blocks all user stories.**
- **US1 (Phase 3)**: Depends on Foundational. The MVP.
- **US2 (Phase 4)**: Depends on Foundational and on US1's chooser, services and E2E harness (T037–T047).
- **US3 (Phase 5)**: Depends on US2's normal close path and database menu (T061, T062, T068), because backups are made at close.
- **US4 (Phase 6)**: Depends on US3's `file_swap`, `disk_space` and `commands/backups.rs` (T081, T083, T088).
- **US5 (Phase 7)**: Depends on US1 and US2. T109 hooks into US4's `change_passphrase` and US3's `restore_backup` if they are done; otherwise defer T109 until they are.
- **US6 (Phase 8)**: Depends on US2 (close path, dirty-form registry T066). The lock-with-backup and stop-a-backup cases in T114 need US3; the stop-a-passphrase-change case needs US4.
- **Polish (Phase 9)**: Depends on every story you intend to ship.

### User Story Dependencies

```text
Setup → Foundational → US1 (MVP) → US2 ─┬→ US3 → US4
                                        ├→ US5 (T109 after US3/US4)
                                        └→ US6 (sleep-stop cases after US3/US4)
```

### Within Each User Story

- Tests are written first and must fail before implementation
- Backend: schema/models → services → session/lifecycle → commands → `main.rs` registration
- Frontend: shared components → services → provider → screens and dialogs
- Screenshots and E2E runs close each story

### Parallel Opportunities

- Setup: T002 and T003 run alongside T001.
- Foundational: the tests T005, T006, T007, T008, T009 and T010 come first and run together. Then T011, T012, T013, T014, T015, T019, T021 and T026 touch different files and run together. T016 runs alongside them. T017 waits for T011, T012 and T016.
- Within each story, every test task marked [P] runs together, and so do independent new files (for example T039 with T041 in US1; T060, T064 and T065 in US2; T081, T082 and T083 in US3; T125–T128 across the three OS backends in US6).
- After US2, US5 and the US6 backend work (T121–T128) can go alongside US3 by different people. `main.rs`, `commands/databases.rs` and `DatabaseMenu.tsx` are shared files, so merge those tasks one at a time.

---

## Parallel Example: User Story 1

```bash
# Tests first, together:
Task: "T030 passphrase_protection_test.rs in src-tauri/tests/"
Task: "T031 PassphraseField.test.tsx in src/components/"
Task: "T032 CreateDatabaseDialog.test.tsx in src/features/databases/"
Task: "T033 DatabaseChooser.test.tsx in src/features/databases/"
Task: "T034 SessionProvider.test.tsx in src/features/session/"
Task: "T035 us7-databases.e2e.ts US1 flow in e2e/specs/"

# Then independent new files, together:
Task: "T039 StrengthHint.tsx in src/components/"
Task: "T041 sessionService.ts and databasesService.ts"
```

## Parallel Example: User Story 6 (platform backends)

```bash
Task: "T126 platform/linux.rs (zbus: logind, ScreenSaver)"
Task: "T127 platform/macos.rs (IOKit, screenIsLocked, WillPowerOff)"
Task: "T128 platform/windows.rs (hidden top-level window)"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational. The backend suite must pass on the passphrase-keyed `TestDb` (T029)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Run US1's independent test and walkthrough 1 in quickstart.md
5. The app is usable again, now protected by a passphrase

### Incremental Delivery

1. Setup + Foundational → the backend is ready (the app shows nothing yet)
2. US1 → create and unlock with a passphrase (MVP)
3. US2 → several databases, switching, take-over, portability
4. US3 → automatic backups and restore
5. US4 → change passphrase
6. US5 → remember on this computer
7. US6 → lock now, idle, sleep, screen lock, pending changes
8. Polish → guide, performance, docs, the PR gates

Each story adds value without breaking the ones before it. Re-run the whole backend suite and the E2E specs of earlier stories at each checkpoint.

---

## Notes

- [P] tasks touch different files and depend on no incomplete task
- Every passphrase that crosses IPC (commands marked 🔑) goes straight into `Passphrase` (zeroized) and is never logged, returned or put in an error (FR-007)
- Never open, modify or delete the developer's real database at `~/.local/share/com.hoplodex.app/hoplodex.db` or its `sqlcipher-key` keyring entry, and never run the app outside the isolated tooling (CLAUDE.md)
- The schema is edited in place in `0001_initial.sql`. Existing development databases become incompatible, and the user is told, not converted
- Carry any form-pattern change to every form and dialog listed in CLAUDE.md's UI-consistency rule
- Commit after each task or logical group, and stop at any checkpoint to validate the story on its own

---

## Phase 10: Convergence

- [X] T150 [US3] Write `src-tauri/tests/backup_location_change_test.rs` first, against real backups made by earlier closes (temp directories only), per FR-026, US3-4a, US3-4b (missing):
  - a changed location with backups of this database at the old folder gives `BACKUPS_AT_OLD_LOCATION { folder, count, totalBytes }` and saves nothing. With none there, or with the same folder named another way (`same_path`), it saves at once with `existingBackups: null`
  - `leave`: saved; the old backups are no longer returned by `list_backups`, the rotation after the next backup or `delete_all_backups` touch none of them, and setting the location back to their folder lists them again
  - `delete`: only this database's backups at the old folder are removed, each through `secure_delete`, with `backups_delete:progress`; one that can't be deleted (read-only) gives `BACKUPS_NOT_ALL_DELETED { deletedCount, failedPaths }` and keeps the old location
  - `move`: every backup arrives byte-identical and the old folder no longer holds them. With hard links forced to fail (test hook), each copy is verified before its original is securely deleted. A missing custom new folder (`BACKUP_LOCATION_UNAVAILABLE`) and too little space (`disk_space` hook, `INSUFFICIENT_SPACE`) are refused with nothing moved or saved. A name already taken at the new folder is left behind (`reason: "nameTaken"`) and the file there stays byte-identical. A failure part way keeps the new location, leaves the rest in the old folder and reports `leftBehind { count, folder, reason }`. Nothing is rotated, even above `keepCount`, until the next backup completes (SC-007)
  - an unreadable old custom folder gives `OLD_BACKUP_LOCATION_UNAVAILABLE { folder }`; `move` and `delete` are refused the same way, and `leave` saves
  - the recent entry's cached `backupFolder` follows every saved change
  - a leftover `unfinishedBackupMove` at the next launch removes the partial copy, clears the record, and pushes `backupsLeftBehind { databasePath, folder, count }` when any are left
  - the first `backups_move:progress` carries `total` (twice the bytes to copy) and `showNow`
- [X] T151 [US3] Add the shared shapes for moving backups per FR-026, FR-037 (missing):
  - `OperationKind::MoveBackups` (`moveBackups`) in `src-tauri/src/models/database.rs`
  - `ChooserNotice::OperationStopped` gains `left_behind_count` and `folder`, and there is a new `ChooserNotice::BackupsLeftBehind { database_path, folder, count }`
  - `ExistingBackupsOutcome` (`leave` | `delete { deletedCount }` | `move { movedCount, leftBehind }`, with reasons `nameTaken`, `locationUnavailable`, `insufficientSpace`, `io`)
  - constructors for `BACKUPS_AT_OLD_LOCATION`, `BACKUPS_NOT_ALL_DELETED` and `OLD_BACKUP_LOCATION_UNAVAILABLE` in `src-tauri/src/commands/error.rs`, and `OPERATION_STOPPED` gaining `leftBehindCount` and `folder`
  - `stopped_notice` in `src-tauri/src/session/lifecycle.rs` filling them from the operation's `done()`
  - the TypeScript mirrors in `src/features/databases/types.ts`, as in contracts/tauri-commands.md "Shared types"
- [X] T152 [US3] Add `move_backups` to `src-tauri/src/services/backups.rs` per FR-026, plan: research §22 (missing):
  - the space check sums the backups to move (leaving out names already taken at the new folder) plus 5%
  - move oldest first, skipping a taken name and counting it as left behind
  - `fs::hard_link` then remove the old name. When linking fails, `copy_chunked` to `<final name>.partial`, flush it, compare it byte for byte with the original, `finalize` it (which refuses a taken name), then `secure_delete` the original
  - a copy that is stopped or fails removes its `.partial` and keeps the original
  - stop at the first failure, and never rotate
  - report `backups_move:progress` in bytes (the copy and the read-back each count once), check the cancel flag between files and chunks, and record the number not yet moved through `record_done`
  - add a test hook that forces the link to fail
- [X] T153 [US3] Rework `ops::update_backup_settings` and its command in `src-tauri/src/commands/databases.rs` per FR-026, US3-4a, US3-4b, contracts/tauri-commands.md (missing):
  - add an `existingBackups?: "move" | "leave" | "delete"` input, and detect a changed resolved folder with `same_path`
  - refuse with `BACKUPS_AT_OLD_LOCATION` or `OLD_BACKUP_LOCATION_UNAVAILABLE` (only `leave` accepted for the latter) before anything is saved
  - `leave` saves
  - `delete` runs the `delete_all_backups` code on the old folder and saves only when every backup went
  - `move` runs `check_location` and the space check, saves through `session.write`, registers `MoveBackups` in `Operations`, records and clears `unfinishedBackupMove` around `move_backups`, and reports `leftBehind`
  - return `{ settings, existingBackups }`, and refresh the recent entry's cached `backupFolder` whenever the location is saved (depends on T151, T152)
- [X] T154 [P] [US3] Write the frontend tests first per FR-026, US3-4a, US3-4b, contracts/ui-databases.md §7 (missing):
  - `src/features/databases/ExistingBackupsDialog.test.tsx`: the "Backups at the old location" question with its count, size and folder; the radio group with **Move them to the new location** selected first, plus the Leave and Delete help texts; **Delete them** asks FR-029's destructive confirm first and cancelling it returns to the question; **Cancel** saves nothing and resets the location while the other fields keep their input; the `ProgressBar` ("Moving the backups…" / "Deleting the backups…", at once on `showNow`, otherwise after 1 s) can't be dismissed and has no lock button; `INSUFFICIENT_SPACE` and `BACKUP_LOCATION_UNAVAILABLE` show "Nothing has been changed." above the footer with the question left open; `BACKUPS_NOT_ALL_DELETED` shows the paths with **Close**
  - `DatabaseSettingsDialog.test.tsx`: **Save** sends the backup settings first and holds back the lock settings until they are through; `OLD_BACKUP_LOCATION_UNAVAILABLE` shows the non-destructive **Continue** confirm, which resends with `leave`; the toasts after a move or delete; and the left-behind warning banner above the footer
  - `DatabaseChooser.test.tsx`: the stopped-move and `backupsLeftBehind` notice texts (contracts/ui-databases.md §1)
- [X] T155 [US3] Create `src/features/databases/ExistingBackupsDialog.tsx` and wire it in per FR-026, US3-4a, US3-4b, FR-037 (missing):
  - it is a `Dialog` with a radio group, the pattern `RestoreBackupDialog` uses
  - **Save** in `DatabaseSettingsDialog.tsx` goes through it, with the old-location-unavailable `ConfirmDialog`, progress from `backups_move:progress`/`backups_delete:progress`, the refusal texts, the toasts, the left-behind banner and `BACKUPS_NOT_ALL_DELETED`
  - update the `update_backup_settings` wrapper in `src/features/databases/databasesService.ts` for `existingBackups` and the new output
  - add the stopped-move and `backupsLeftBehind` notices to `src/features/databases/DatabaseChooser.tsx`
  - carry field widths and grouping from the other dialogs (CLAUDE.md UI consistency) (depends on T153)
- [X] T156 [US6] Extend `src-tauri/tests/lock_test.rs` and `src-tauri/tests/performance_test.rs` per FR-037, US6-4c, SC-010, SC-005 (missing):
  - a sleep during a move stops it between files: the partial copy is removed and its original kept, the moved ones are at the new folder and the rest at the old one, the new location is kept, and the notice is `operationStopped { operation: "moveBackups", leftBehindCount, folder }`
  - a sleep during a delete made at a location change keeps the old location
  - a running move pauses the idle clock
  - the first `backups_move:progress` arrives within 100 ms of the move starting
- [X] T157 [US3] Add `unfinishedBackupMove { databasePath, databaseId, fromFolder, partialPath }` to `src-tauri/src/services/machine_settings.rs`, with set, update and clear, per plan: research §22 (missing):
  - extend `backups::sweep_unfinished`, called from `src-tauri/src/main.rs` setup: remove the partial file, count this database's backups still in `fromFolder` by the cached id, and push `backupsLeftBehind` when there are any
  - add the `machine_settings_test.rs` round-trip case
- [X] T158 [US3] Add the move flows to `e2e/specs/us8-backups.e2e.ts`: change the location to a scratch folder and **Move** (the backups are listed from the new folder and gone from the old one); change it back and **Leave** (the restore list is empty until that folder is chosen again). Add the screen `27-backup-location-change` (light and dark) to `e2e/screenshots/screens.e2e.ts`. Per FR-026, contracts/ui-databases.md §15, Constitution III (missing)
- [X] T159 Re-run T143's gates in the dev container for the convergence work (cargo test, including the `mock-keyring` keyring test; clippy; fmt; `npm run test`, lint and format; `npm run audit`; `npm run build`, then `us8-backups.e2e.ts` and `us9-locking.e2e.ts` one at a time), then reconcile contracts/tauri-commands.md and contracts/ui-databases.md with any shape or text that changed, per Constitution (quality gates) (missing)

---

## Phase 11: Convergence

- [ ] T160 On Windows, outside the container, run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `cargo test --manifest-path src-tauri/Cargo.toml --no-fail-fast` and the `mock-keyring` keyring test. This first compiles and lints `platform/windows.rs` and the `cfg(windows)` code in `services/disk_space.rs`, `services/secure_delete.rs`, `services/file_swap.rs`, `services/backups.rs` and `session/fingerprint.rs`, and `portability_test` opening the Linux-made fixture is SC-001's Windows check. Fix every failure, record the results for the PR, then tick T128 and T144, whose macOS part is done, per T144, T128, Constitution I, SC-001 (partial)
- [ ] T161 On Linux (GNOME or KDE), run the Linux column of quickstart.md's "Platform checks" table with `scripts/human-testing.sh`: sleep with the idle lock on, sleep during a large backup, a passphrase change and a move of backups, screen lock with and without a half-typed passphrase, and log out while editing. On a bare window manager without logind session locking, check the screen-lock option shows as unavailable. Record the results for the PR, per T146, FR-033, FR-034, FR-037, FR-038, FR-039 (partial)
- [ ] T162 On macOS, run the macOS column of quickstart.md's "Platform checks" table (its Rust-gates row is already recorded in T144): Apple menu → Sleep with the idle lock on, sleep during a large backup, a passphrase change and a move of backups, Ctrl+⌘+Q with and without a half-typed passphrase, and log out while editing. `scripts/human-testing.sh` is Linux only, so use scratch databases in a scratch folder on a machine or account that holds no real HoploDex data. Record the results for the PR, per T146, FR-033, FR-034, FR-037, FR-038, FR-039 (partial)
- [ ] T163 On Windows, run the Windows column of quickstart.md's "Platform checks" table (its Rust-gates row is T160): Start → Power → Sleep with the idle lock on, sleep during a large backup, a passphrase change and a move of backups, Win+L with and without a half-typed passphrase, and sign out while editing. `scripts/human-testing.sh` is Linux only, so use scratch databases in a scratch folder on a machine or account that holds no real HoploDex data. Record the results for the PR, per T146, FR-033, FR-034, FR-037, FR-038, FR-039 (partial)
- [ ] T164 On Linux, walk through quickstart.md's walkthroughs 1 and 3–6 with `scripts/human-testing.sh`, timing walkthrough 1's create and a switch to a second database against SC-006's 2 minutes. Record the results for the PR, per T146, SC-006 (partial)
- [ ] T165 On macOS, do quickstart.md's walkthrough 2 in the direction `portability_test` doesn't cover: create a database there with a firearm, a photo and a document (scratch folder, as in T162), copy it to Linux, open it with **Open another database file…** and its passphrase, and check every record, photo and document. Record the result for the PR, per T146, SC-001, US2-6 (partial)
- [ ] T166 On Windows, do quickstart.md's walkthrough 2 the same way: create a database there with a firearm, a photo and a document (scratch folder, as in T163), copy it to Linux, open it with **Open another database file…** and its passphrase, and check every record, photo and document. Record the result for the PR, then, once T161–T165 are done too, tick T146, per T146, SC-001, US2-6 (partial)
