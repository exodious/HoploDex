---

description: "Task list for feature 008: session isolation, safe import, browser controls and pending changes from another version"
---

# Tasks: Session Isolation, Safe Import, Browser Controls and Pending Changes from Another Version

**Input**: Design documents from `/specs/008-hardening-batch/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (tauri-commands.md, ui.md, spreadsheet-format.md), quickstart.md (all present)

**Tests**: The project constitution's Testing Standards principle is NON-NEGOTIABLE: every user story below includes test tasks written first, expected to fail, then made to pass by the implementation. Backend tests run the real `ops` and the real `Session` against temporary SQLCipher databases (no mocks of the DB); import tests run the real helper binary (`CARGO_BIN_EXE_hoplodex`), as `tiff_preview_test.rs` does. What only real input on a real web view can show is the window controls check, automated on each OS.

**Organization**: Tasks are grouped by user story (spec.md's priorities P1–P4) so each story can be implemented and tested on its own. This feature is a **delta** against features 001 to 007: most tasks edit existing files (plan.md's Project Structure). The one schema change is made in place in `0001_initial.sql` (CLAUDE.md "Schema changes"); no new migration file.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Which user story this task belongs to (US1–US4)
- Exact file paths are given per plan.md's Project Structure
- Work that must be built, tested or run on a specific platform gets one task per platform, starting with the platform's name

## Path Conventions

Existing Tauri desktop app: Rust backend in `src-tauri/`, React/TypeScript frontend in `src/`, WebdriverIO E2E suite in `e2e/`. Run tests, lint, the audit and screenshots through `scripts/dev-container.sh` (CLAUDE.md; commands in quickstart.md). macOS work runs in the macOS 26 VM and Windows work on the Windows test machine (DEVELOPMENT.md's macOS and Windows sections). Never open the real databases, `machine.json` or keyring entries (CLAUDE.md, "Never touch the real databases"); every new path (the import helper's input, the window controls check's scratch folders, the seed's `--pending-from` database) is taken from a parameter or the sandbox.

## How the stories divide the shared pieces

- **Foundational** owns the session id end to end, because every later task edits the files it changes: `SessionId` and the current id, `Session`'s accessors taking the id, `ScopedSession` and the `HoploDex-Session` header (plan.md's first task 1, proved on each OS), `UNSCOPED_COMMANDS` and its test, `DatabaseStatus.sessionId`, the session events' `sessionId`, the frontend `SessionScope` with its `invoke`, its lifetime in `SessionProvider`, and every `*Service.ts` taking the scope as its first parameter.
- **US1** owns what the scope holds and what ends with it: the thumbnail cache, staging, the resumed draft and its `PendingState` lifecycle (`resume`, `opened`, `notOpened`, `discard`), the pending dialog's "Opening…" and failure line, batches stopping at the scope's end, the toasts moved into the keyed tree, and the two-database regression tests.
- **US2** owns reading for import: the helper core moved to `services/helper/` (plan.md's first task 3), the `--import-helper` mode, `services/import_reader/` with its limits and frames, the two-phase `import_collection`, `cancel_import`, `IMPORT_LIMIT_EXCEEDED`, the reading phase in the import dialog, and the free-text field maximums (research.md §10), which size the limits.
- **US3** owns the browser controls: `browserControls.ts` in the page, `window_controls/` on each OS (plan.md's first task 2, the macOS hook, is its first task), the window controls check and its scripts, and `human-testing.sh --release`.
- **US4** owns the last-saved version (`app_state.last_saved_version`, `APP_VERSION`, `record_saved_version`), `PendingSummary`'s versions, the dialog's wording and its **Close the database** button, and `human_seed --pending-from`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Empty modules, so later tasks in different files can proceed in parallel.

- [ ] T001 Create and declare the new backend modules, each with a module doc comment citing its research.md section, empty but compiling:
  - `src-tauri/src/session/scoped.rs` (research.md §2), declared in `src-tauri/src/session/mod.rs`;
  - `src-tauri/src/services/helper/mod.rs` (§8, the shared helper core's new home), declared in `src-tauri/src/services/mod.rs`;
  - `src-tauri/src/services/import_reader/mod.rs` with `limits.rs`, `reader.rs`, `client.rs` and `frames.rs` (§8, §9), declared in `src-tauri/src/services/mod.rs`;
  - `src-tauri/src/window_controls/mod.rs` (§13–§15) with `linux.rs`, `macos.rs` and `windows.rs`, each behind its `#[cfg(target_os = …)]` as `platform/` does, declared from `src-tauri/src/lib.rs`.
  `cargo build --manifest-path src-tauri/Cargo.toml` must pass in the dev container
- [ ] T002 [P] Create the new frontend modules as empty, compiling files with a header comment naming their research.md section: `src/features/session/sessionScope.ts` (§3), `src/features/app/browserControls.ts` (§12), `src/lib/textLimits.ts` (§10). `npm run lint` must pass

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The session id, carried by every request and checked by the backend, with the frontend's scope that sends it. Every story's tasks edit the command files, services and `SessionProvider` this phase changes, so it goes first (plan.md "First tasks", 1).

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### First task 1: the header reaches commands, on each OS (research.md §2)

- [ ] T003 In `src-tauri/src/session/mod.rs`, rename `OpenDatabase::generation` to `session_id: SessionId` (a `SessionId(u64)` newtype, `Copy`, `Eq`, serialized as a number; `0` is never issued and means "none", data-model.md "SessionId") from the existing `GENERATIONS` counter, renamed to match; add `SessionInner.current: AtomicU64`, set by `install` and cleared to 0 by `take`/`forget_open`, and `Session::current_id() -> SessionId` reading it without the mutex (research.md §1). Add `session_id: SessionId` to `DatabaseStatus` in `src-tauri/src/models/database.rs` (IPC `sessionId`, never 0 while open) and fill it in `get_database_status` and the open/unlock/create results in `src-tauri/src/commands/databases.rs`; mirror it in `src/features/session/` types
- [ ] T004 Implement `ScopedSession` in `src-tauri/src/session/scoped.rs` (data-model.md "ScopedSession"): `{ session, id: SessionId }`, implementing `tauri::ipc::CommandArg` by taking the `Session` state as `State<Session>` does and parsing the `HoploDex-Session` header from the `InvokeMessage` as a decimal `u64`; a missing, malformed or `0` header fails with `CommandError` `DATABASE_CLOSED`; `is_current()` compares `id` with `Session::current_id()` without the mutex. Add unit tests in the same file for a good, a missing, a malformed and a zero header. Change one read command (`list_firearms` in `src-tauri/src/commands/firearms.rs`) to take `session: ScopedSession`, delegating to `Session::read` with a temporary check of the id, so T006–T008 can prove the path
- [ ] T005 In `src/services/tauriClient.ts`, give `invoke(command, args, options?: { session?: number })` that turns `session` into `InvokeOptions.headers` `{ "HoploDex-Session": String(session) }`; nothing else may set the header. Extend `src/services/tauriClient.test.ts` for a call with and without it. Temporarily send the open status's `sessionId` from `browseService.ts`'s `listFirearms` so the E2E suite exercises the header
- [ ] T006 On Linux, in the dev container, run `npm run build` and the full E2E suite one spec file at a time with T004/T005 in place, plus a throwaway probe spec that calls `list_firearms` through `browser.execute` with the header (served) and without it (`DATABASE_CLOSED`) over both Tauri IPC paths (the `ipc://` custom protocol, and the `postMessage` fallback forced as research.md §2 describes); record which paths were exercised and the result in research.md §2, then delete the probe spec
- [ ] T007 On macOS, in the macOS 26 VM, run the same probe and the E2E suite as T006; record the result in research.md §2
- [ ] T008 On Windows, on the Windows test machine, run the same probe and the E2E suite as T006; record the result in research.md §2. If the header fails to reach the command on any OS, stop and report to the owner before T009 (plan.md "First tasks")

### The backend check (FR-004, research.md §2)

- [ ] T009 Write `src-tauri/tests/session_scope_test.rs` (expected to fail until T010–T013): against real temporary databases through `Session`, every accessor (`read`, `read_stamped`, `write`, `write_open`, `write_housekeeping`, `inspect`, `inspect_mut`, `hold`) refuses with `DATABASE_CLOSED`, running nothing, for an ended session's id (after a close, a lock, a reopen of the same file, an open of a second file, a restore of the open database from a backup, and a take-over), for a never-issued id and for 0; and the current id is served. "A late close or lock leaves the next session open": `ops::close_database` and `ops::lock_database` (with a draft and with none) given an ended session's id return `DATABASE_CLOSED`, and the database opened after it stays open with no pending changes written. Add the command-list check: read `src-tauri/src/commands/*.rs` and fail when a `#[tauri::command]` takes `State<'_, Session>` but isn't in `UNSCOPED_COMMANDS`, or is in `UNSCOPED_COMMANDS` but takes `ScopedSession`, and when a name in `UNSCOPED_COMMANDS` isn't in `COMMANDS`
- [ ] T010 In `src-tauri/src/session/mod.rs`, make `read`, `read_stamped`, `write`, `write_open`, `write_housekeeping`, `write_checked`, `inspect`, `inspect_mut` and `hold` take a `SessionId` and compare it with the open database's `session_id` **under the session's mutex**, before the fingerprint check and before anything runs; a mismatch or nothing open is `DATABASE_CLOSED`. Fold `with_generation` into `read`/`inspect_mut` with the id and delete it, updating its callers (`src-tauri/src/commands/documents.rs`'s `open_document`, `src-tauri/src/commands/preview.rs`). Make `Session::take` take a `SessionId` too, comparing under the mutex and returning `None` (→ `DATABASE_CLOSED`) for another session, and make `lifecycle::close_normal` and `lifecycle::lock` take the `SessionId` they pass to `take` and the accessors, so a late close or lock can't end the next session (research.md §2 point 3); `hold_for_close` (the immediate close) stays as it is. Give `ScopedSession` the same accessors delegating with its id, with public `session` and `id` fields and no `Deref` to `Session`. No unscoped variants (CLAUDE.md memory "refactor, not workaround")
- [ ] T011 In `src-tauri/src/session/lifecycle.rs`, `src-tauri/src/session/idle.rs`, `src-tauri/src/session/pending.rs` (`lock_with_staged`) and `src-tauri/src/platform/`, pass `session.current_id()` wherever the backend acts on whatever is open (idle lock, screen lock, sleep, shutdown, quit, take-over, the open marker), through the same accessors and `close_normal`/`lock` (research.md §2 point 4)
- [ ] T012 Add `UNSCOPED_COMMANDS` to `src-tauri/src/commands_list.rs`: `get_chooser_state`, `create_database`, `open_database`, `remove_recent_database`, `locate_database`, `get_database_status`, `quit_application`, `note_activity`, `set_idle_paused`, `skip_backup`, `get_document_opening`, `set_document_opening`, `restore_backup` (contracts/tauri-commands.md "Every request names its session"; `cancel_import` joins it in US2, T058)
- [ ] T013 Replace `session: State<'_, Session>` with `session: ScopedSession` in every command not in `UNSCOPED_COMMANDS`, under the same parameter name, in `src-tauri/src/commands/firearms.rs`, `accessories.rs`, `mounts.rs`, `insurance.rs`, `photos.rs`, `documents.rs`, `preview.rs`, `entries.rs`, `import_export.rs`, `backups.rs` and `databases.rs` (including `close_database`, `lock_database`, `stage_pending_changes`, `resolve_pending_changes`, `dismiss_note`, `update_backup_settings`, `update_lock_settings`, `save_passphrase`, `forget_saved_passphrase`, `list_backups`, `delete_all_backups`, `change_passphrase`). The `ops` functions in `databases.rs` and `backups.rs` that reach the open database take `&Session` and the request's `SessionId` (the command passes `session.id`), and pass it to the accessors, `close_normal` and `lock`. In `restore_backup`, when there is no `databasePath` (restoring the open database), read the header itself and refuse a missing or non-current one with `DATABASE_CLOSED`. Remove T004's temporary check. `session_scope_test.rs` (T009) must pass
- [ ] T014 Update every Rust test and example that calls `Session`'s accessors to pass an id (`session.current_id()` after its open), in `src-tauri/tests/session_test.rs`, `lock_test.rs`, `lock_held_test.rs`, `pending_changes_test.rs`, `take_over_test.rs`, `open_document_test.rs`, `pdf_preview_test.rs`, `tiff_preview_test.rs`, `performance_test.rs`, `restore_test.rs`, `passphrase_change_test.rs`, `backup_test.rs`, `support/` and any other file `cargo test` names; `cargo test --manifest-path src-tauri/Cargo.toml` passes in the dev container
- [ ] T015 Add `sessionId` to the `session:closing` and `session:closed` payloads in `src-tauri/src/session/lifecycle.rs` (the id of the session that is ending) and to their TypeScript types in `src/features/session/` (contracts/tauri-commands.md "Events"); extend `src-tauri/tests/lock_test.rs`'s event assertions to check it

### The frontend scope (FR-001, FR-002, research.md §3)

- [ ] T016 [P] Write `src/features/session/sessionScope.test.ts` (expected to fail until T017): `invoke` sends the header with the scope's id; after `end()` it rejects with `CommandFailure { code: "DATABASE_CLOSED" }` without calling `tauriClient.invoke`; a response that resolves after `end()` rejects the same way and its value is never returned; `end()` is synchronous and idempotent and runs every registered clean-up once
- [ ] T017 Implement `SessionScope` in `src/features/session/sessionScope.ts` per data-model.md "SessionScope" (`id`, `ended`, `end()`, `invoke<T>(command, args)`, a clean-up registry for what US1 adds), with `SessionScopeContext` and `useSessionScope()` (throws outside a provider)
- [ ] T018 In `src/features/session/SessionProvider.tsx`, create one `SessionScope` per open from `DatabaseStatus.sessionId`, provide it to the collection's tree keyed by the open, and end it synchronously, before anything of the next session renders: on `session:closing` or `session:closed` whose `sessionId` is the scope's (a late event of an old session ends nothing); right after "Lock now" sends `lock_database`; when `opened()` installs a new status (ending the previous scope first); and on `toChooser`. Extend `src/features/session/SessionProvider.test.tsx` for each of these, and for an old session's late event leaving the new scope alive
- [ ] T019 [P] Make every wrapper in `src/features/firearms/firearmsService.ts` and `src/features/browse/browseService.ts` (including the entry commands, `suggest_entries`, `settle_entry`, `list_action_types`, `list_firearm_types`, `list_registration_classes`) take `scope: SessionScope` as its first parameter and call `scope.invoke`; remove T005's temporary header; update every caller in `src/features/firearms/` and `src/features/browse/` (components capture the scope with `useSessionScope()` when they render) and their tests' mocks
- [ ] T020 [P] Do the same as T019 for `src/features/accessories/accessoriesService.ts` and `src/features/mounts/mountsService.ts`, and their callers and tests in `src/features/accessories/` and `src/features/mounts/`
- [ ] T021 [P] Do the same as T019 for `src/features/insurance/insuranceService.ts` and its callers and tests in `src/features/insurance/` and `src/features/app/` (`CollectionProvider`, `collectionStore`)
- [ ] T022 [P] Do the same as T019 for `src/features/media/mediaService.ts` (photos, documents and the preview commands) and its callers and tests in `src/features/media/`
- [ ] T023 [P] Do the same as T019 for `src/features/import-export/importExportService.ts` and its callers and tests in `src/features/import-export/`
- [ ] T024 Do the same as T019 for the scoped commands in `src/features/session/sessionService.ts` and `src/features/databases/databasesService.ts` (`lock_database`, `close_database`, `stage_pending_changes`, `resolve_pending_changes`, `dismiss_note`, the settings, saved-passphrase, backup and passphrase-change commands, and `restore_backup` of the open database); the unscoped ones (T012's list) keep calling `tauriClient.invoke` with no scope. Update their callers in `src/features/session/` and `src/features/databases/` and their tests. `npm test` and `npm run lint` pass
- [ ] T025 On Linux, run `npm run build` and the full E2E suite one spec file at a time in the dev container (`us1`–`us13`, `us7-databases-no-keyring`, `ui-review`, `webdriver-plugin`) and fix every failure, so the scoped commands work end to end

**Checkpoint**: every collection request names its session and the backend refuses an ended one; the suite passes. User story work can begin.

---

## Phase 3: User Story 1 - Each Database's Data Stays With Its Own Session (Priority: P1) 🎯 MVP

**Goal**: Nothing of one session's collection (thumbnails, records, drafts, notifications, unfinished work) is shown in or written to another session, of another database or the same one (FR-001 to FR-005).

**Independent Test**: Two databases whose firearms, accessories, photos and documents share internal ids; in the first, view thumbnails, start a batch of attachments, resume a draft whose record fails to load and raise notifications; lock or switch to the second; nothing of the first is shown in the second and nothing begun in the first is written there (spec.md US1; quickstart.md rows US1-1 to US1-7, SC-001).

### Tests for User Story 1 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T026 [P] [US1] Extend `src-tauri/tests/session_scope_test.rs` with "colliding ids" (SC-001's backend half): two databases seeded through `ops` so that firearm, accessory, photo and document ids are the same in both; with A's session id after switching to B, `list_photos`, `get_photo`, `get_firearm`, `list_documents` and `open_preview` return `DATABASE_CLOSED` and nothing of B; `add_photo_from_path` and `add_document_from_path` with A's id write nothing to B (B's row counts and change-tracking unchanged) (US1-5, US1-7)
- [ ] T027 [P] [US1] Write `src-tauri/tests/pending_resume_test.rs` (data-model.md "PendingState"; contracts/tauri-commands.md `resolve_pending_changes`): each of `resume`, `opened`, `notOpened`, `discard` in each state, with a wrong-state action a `VALIDATION_ERROR` changing nothing; collection commands refused with `PENDING_CHANGES_UNRESOLVED` in `Unresolved` and served in `Resuming`; `opened` and `discard` delete the row; `notOpened` returns the `PendingSummary` and blocks again; "row kept across a lock while resuming" (lock in `Resuming` with nothing staged leaves the row, and the next open reports it in `pendingChanges` and starts in `Unresolved`); `close_database` served in `Unresolved` and `Resuming`; "opened stages the draft: a lock or sleep right after keeps the changes" (after `opened`, with nothing staged by a form, a lock and, separately, an immediate close write the draft back as pending changes)
- [ ] T028 [P] [US1] Write `src/features/browse/FirearmThumbnail.test.tsx`: "two scopes with colliding photo ids" (scope A's cached thumbnail for photo 7 is not shown under scope B, which loads its own); "a deferred response after the scope ended is dropped" (resolved after `end()`: not shown, not cached)
- [ ] T029 [P] [US1] Write `src/features/media/PhotoGallery.test.tsx` and extend `src/features/media/DocumentList.test.tsx`: "stops at the scope's end, keeps what was added" (a batch of three files, the scope ended after the first add resolves: exactly one add was sent); "a deferred response after the scope ended is dropped" (a list load resolved after `end()` doesn't reach the list)
- [ ] T030 [P] [US1] Extend `src/features/session/usePendingDraft.test.tsx`: "a draft resumed in one scope is not matched in another" (a form of the same kind, mode, version and target id under another scope doesn't take it); a debounced flush that fires after its scope ended sends nothing; `staged` resets when a scope ends, so the next session's first draft is staged
- [ ] T031 [P] [US1] Extend `src/features/session/SessionProvider.test.tsx`: "toasts go with the session; a late notify shows nothing; the chooser notice stays" (US1-6, FR-005); "a failed record load brings the dialog back with its error" (US1-4: `notOpened` sent, the dialog shows its three buttons and the error line, the collection is blocked); "lock while opening keeps the changes for the next open" (US1-3); "Resume editing shows Opening… until the form opens, then `opened` closes the dialog". Add the guard of research.md §4: `DatabaseChooser`, `ClosingScreen`, `PendingChangesDialog` and `SessionProvider` render no `useToast` consumer outside the tree keyed by the open
- [ ] T032 [P] [US1] Extend `src/features/firearms/FirearmRecordPage.test.tsx` and `src/features/accessories/AccessoryRecordPage.test.tsx`, and write `src/features/insurance/PolicyEditors.test.tsx`: "report notOpened" when the record fails to load (or the policy isn't in the collection) while a resumed draft for it is opening, and nothing when no draft is resuming
- [ ] T033 [P] [US1] Write `e2e/specs/us14-session-isolation.e2e.ts`: with the two databases `seedCollection()` seeds ("Main collection" and "Shared collection", each with its own id sequence; first confirm in `src-tauri/examples/human_seed.rs` that a firearm with a thumbnail in each has the same photo id, and seed so they do if not, keeping `human_seed_coverage_test` passing), open Main, view the collection with thumbnails, lock, open Shared, and assert "shows B's own thumbnail after locking A" (the thumbnail's data URL for the colliding id differs from A's); also switch through the database menu and restore a backup, asserting the same; and that no toast of A is shown after the switch

### Implementation for User Story 1

- [ ] T034 [US1] Replace `OpenDatabase.pending_unresolved: bool` with `pending: PendingState` (`None`, `Unresolved`, `Resuming`) in `src-tauri/src/session/mod.rs`, set from the open's row check; `write_checked` refuses collection commands with `PENDING_CHANGES_UNRESOLVED` only in `Unresolved`; `close_database` is served in every state (research.md §6)
- [ ] T035 [US1] In `src-tauri/src/session/pending.rs` and `src-tauri/src/commands/databases.rs`, make `resolve_pending_changes` take `{ action: "resume" | "opened" | "notOpened" | "discard" }` (`PendingAction` in `src-tauri/src/models/database.rs`) with the transitions and outputs of contracts/tauri-commands.md: `resume` returns the draft and moves to `Resuming` keeping the row; `opened` deletes the row and, under the same hold of the session's mutex, sets `OpenDatabase.staged_draft` to the draft (research.md §6); `notOpened` returns `{ pending }` and moves back to `Unresolved`; `discard` unchanged; a wrong state is `VALIDATION_ERROR`. Make the lock's `write_pending` leave the row when nothing is staged. `pending_resume_test.rs` (T027) and `pending_changes_test.rs` pass
- [ ] T036 [US1] Move `FirearmThumbnail`'s module-level `photoThumbnailCache` into the scope (`scope.thumbnails: Map<number, string>`, cleared by `end()`) in `src/features/session/sessionScope.ts` and `src/features/browse/FirearmThumbnail.tsx`; load through `scope.invoke`, so a late image is neither cached nor shown. T028 passes
- [ ] T037 [US1] In `src/features/session/usePendingDraft.ts`, keep each registration's scope and stage through it in `flush`; reset `staged` when a scope ends (a scope clean-up); move the module-level `resumed` into the scope as `resumed: { draft, state: "opening" } | null` with `resumeOpened()` (→ `resolve_pending_changes("opened")`, then marks the draft as staged, since the backend now holds it) and `resumeFailed(reason)` (→ `"notOpened"`, returning the summary); `useResumedDraftTaken` matches only the scope's own `resumed` and calls `resumeOpened()` when its form opens with it. Update `src/features/session/sessionService.ts` for the four actions. T030 passes
- [ ] T038 [US1] In `src/features/app/AppShell.tsx`, take the resumed route from `scope.resumed` instead of module state, so it opens only in the session that resumed
- [ ] T039 [US1] In `src/features/session/PendingChangesDialog.tsx` and `src/features/session/SessionProvider.tsx` (contracts/ui.md §1 "Resuming"): Resume editing calls `resolve(resume)`, the collection mounts beneath while the dialog stays up with Resume editing pending, labelled "Opening…"; the dialog closes when the scope reports opened; on `resumeFailed`, the dialog stays with Resume editing offered again and an `hd-field__error` line with `role="alert"`: "The form for <label> couldn't be opened. Your changes are still kept. Try again, close the database, or discard them."; the collection beneath is inert until then. (US4's T095 adds Close the database to the same footer)
- [ ] T040 [P] [US1] Call `scope.resumeFailed(...)` when a resumed draft's form can't open: in `src/features/firearms/FirearmRecordPage.tsx` and `src/features/accessories/AccessoryRecordPage.tsx` when the record fails to load; in `src/features/insurance/PolicyEditors.tsx` when the policy isn't in the collection; and in the collection's `FaultBoundary` (`src/features/session/FaultScreen.tsx`) when a render fails. T032 passes
- [ ] T041 [P] [US1] In `src/features/media/PhotoGallery.tsx` (`addAll`) and `src/features/media/DocumentList.tsx` (its batch), check `scope.ended` before each file and stop when it has ended; every add goes through `scope.invoke` (research.md §5). T029 passes
- [ ] T042 [US1] Move `ToastProvider` from `src/App.tsx` into `src/features/session/SessionProvider.tsx`, wrapping the collection's `FaultBoundary` inside the tree keyed by the open, so every toast of the session goes when the collection is replaced and a late `notify` reaches no provider (research.md §4); update tests that render `ToastProvider` around collection components (`src/features/media/*.test.tsx`, `src/features/databases/DatabaseNotes.test.tsx`, `DatabaseSettingsDialog.test.tsx`, `src/features/mounts/MountedSection.test.tsx`) only as needed. T031 passes
- [ ] T043 [US1] Check that research.md §4's list of module-level state is still complete: `grep -rnE '^(let|const) [a-zA-Z_]+ *(: *[^=]+)?= *(new (Map|Set)|\[\]|\{\}|null)' src --include='*.ts' --include='*.tsx'` and `grep -rn '^let ' src`, excluding tests; anything holding collection data moves into the scope; record the result in research.md §4
- [ ] T044 [US1] On Linux, run `npm run build` and `e2e/specs/us14-session-isolation.e2e.ts`, `us9-locking.e2e.ts`, `us4-photos-documents.e2e.ts` and `us13-document-preview.e2e.ts` one at a time in the dev container, plus `cargo test` and `npm test`; fix every failure

**Checkpoint**: User Story 1 is fully functional and testable on its own.

---

## Phase 4: User Story 2 - Import a Spreadsheet Without Risk to the Application (Priority: P2)

**Goal**: Any file given to import is either read within fixed limits, in a confined helper that holds no session lock and can be cancelled, or refused with a message naming the file and the limit; nothing ends the application (FR-006 to FR-009). Free-text fields get maximum lengths so the largest collection, and the limits sized against it, are defined (research.md §10).

**Independent Test**: Import the hostile corpus (impossible `uniqueCount`, a 1000× zip bomb, too many rows, columns, sheets, too much cell text, a CSV with one huge field, truncated and corrupted workbooks); each is refused naming the file within 5 s, the app keeps running with memory bounded, cancelling and locking stop the reading within 1 s; then the app's own largest export imports (quickstart.md rows US2-1 to US2-6, SC-002 to SC-004).

### First task 3: the helper core moved (research.md §8)

- [ ] T045 [US2] Move the TIFF-independent parts of 007's helper from `src-tauri/src/services/preview/` to `src-tauri/src/services/helper/`: `confine.rs` (Landlock and rlimits on Linux, `sandbox_init` and the kqueue parent watch on macOS, the job object on Windows), with the memory cap now a parameter of `confine()` (TIFF keeps 2 GiB); `helper_job.rs` → `job.rs`; the framing half of `helper_protocol.rs` → `frames.rs`; the generic half of `helper_handle.rs` (start, time limits, kill on drop) → `handle.rs`. `services/preview/` keeps `tiff.rs` and `helper.rs` (the TIFF mode's loop and its requests). Update `src-tauri/src/main.rs`, `src-tauri/tests/tiff_helper_test.rs`, `tiff_preview_test.rs` and `support/preview_support.rs` to the new paths, and rename the paths in place in `specs/007-document-preview/` (research.md, plan.md, tasks.md) as a mechanical rename (CLAUDE.md "Spec Kit workflow"). No behaviour change
- [ ] T046 [US2] On Linux, run `tiff_helper_test`, `tiff_preview_test` (including the confinement self-check) and `pdf_preview_test` in the dev container after T045; fix every failure
- [ ] T047 [US2] On macOS, in the macOS 26 VM, run the same tests as T046 after T045; fix every failure
- [ ] T048 [US2] On Windows, on the Windows test machine, run the same tests as T046 after T045; fix every failure

### Tests for User Story 2 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T049 [P] [US2] Write `src-tauri/tests/support/hostile_spreadsheets.rs` (declared in `support/mod.rs`), generating at test time into a temporary folder: a workbook whose shared strings declare `uniqueCount` and `count` of `18446744073709551615` and of 2,000,001 with few strings present; a workbook with cells at A1 and XFD1048576; a 1000× zip bomb (a sheet part of ≥1 GiB of repeated XML compressed to ≤1 MiB); a workbook with 1,001 zip entries; one with 17 sheets; one with 100,001 data rows; one with a 257-cell row; one with more than 5,000,000 cells within every other limit (sixteen sheets of 100,000 rows of 4 short cells); a sparse workbook whose cells at column IV of every row pad out past 5,000,000 cells; one with a 32,768-character cell; one passing 256 MiB of total text within the other limits; a CSV with 100,001 rows, a 257-field row, a 32,768-character field, a single 300 MB field, and 100,000 lines of 255 commas (25.6 million empty cells in a 25 MB file); a file over 256 MiB; truncated and bit-flipped copies of a valid export workbook; a valid archive that isn't a spreadsheet; a two-file import whose second file passes a limit; and a two-file import whose files are each under 5,000,000 cells but pass it together
- [ ] T050 [P] [US2] Write `src-tauri/tests/import_reader_test.rs` over T049's corpus through `import_reader::read_files` and the real helper binary: for each file, refused within 5 s with `IMPORT_LIMIT_EXCEEDED` (the right `ImportLimit`, `file`, and `sheet` for a workbook, with contracts/tauri-commands.md's message wording) or `VALIDATION_ERROR` "<file>: could not read the file." for unreadable ones; "uniqueCount beyond the limit is refused as unreadable, the helper ends, the app doesn't"; the test process's resident memory rises by no more than 512 MB during any one file (read from `/proc/self/status` on Linux, `task_info` on macOS, `GetProcessMemoryInfo` on Windows); the helper process is gone afterwards; the two-file case imports nothing; "cancel_import stops the reading within 1 s" (at several points in a slow, large valid file); "a lock goes ahead at once and the reading ends within 1 s" (`DATABASE_CLOSED`, nothing imported); an export of a small seeded collection, as a workbook and as two CSVs, reads back as `read_table` expects
- [ ] T051 [P] [US2] Extend `src-tauri/tests/lock_test.rs` with "screen lock, sleep and quit during reading": with an import reading a large valid file, a screen lock, a sleep and a quit (`quit_application`'s close) each go ahead without waiting for the reading, and the import ends with `DATABASE_CLOSED` (never `OPERATION_STOPPED`, which only `cancel_import` gives, research.md §11), with nothing imported and the helper gone
- [ ] T052 [P] [US2] Write `src-tauri/tests/text_limits_test.rs` (research.md §10, data-model.md "Validation: free-text field maximums"): for each field of the table "Firearm", "Accessory", "Disposition" and "Insurance policy" — the six entry fields stay at "100 (unchanged)"; `nickname`, `serialNumber`, `finish`, `acquisitionSource`, `dispositionRecipient`, `countryOfManufacture`, `importerName`, `originalMake`, `originalModel`, `originalSerialNumber`, an accessory's `serialNumber`, `acquisitionSource`, `dispositionRecipient`, the disposition `recipient`, and a policy's `name`, `policyNumber`, `insuranceCompany`, `companyContact`, `agentName`, `agentContact` take "200"; a firearm's `notes` and `accessories`, an accessory's `notes` and a policy's `notes` take "4,000" — a value at the maximum is saved, one character more is refused with `fieldErrors[<field>] = "<Label> can be at most <n> characters."`, counted in Unicode scalar values after trimming (a 4-byte character counts as one); and the same cells in an imported row give that message as a row error, not a refusal of the file
- [ ] T053 [P] [US2] Write `src-tauri/tests/import_largest_test.rs`, `#[ignore]` and release-only (`#[cfg(not(debug_assertions))]`): generate research.md §9's largest export (10,000 firearms and 10,000 accessories, every text field at its §10 maximum, ASCII) as one workbook and as two CSV files through the app's own export, import each into a fresh database through `import_reader::read_files` and `import_collection_stoppable`, assert nothing is refused and every row imports, and print the reading time, the import time and the parent's peak resident memory (SC-003)
- [ ] T054 [P] [US2] Write `src/lib/textLimits.test.ts` (the two maximums and the check, matching the backend's message and character counting) and extend `src/components/TextArea.test.tsx` (create it if absent) with "counter": no counter below 3,600 characters; "<n> of 4,000" from 3,600, linked through `aria-describedby` with `aria-live="polite"`; error style over 4,000
- [ ] T055 [P] [US2] Extend `src/features/firearms/FirearmForm.test.tsx`, `src/features/accessories/AccessoryForm.test.tsx`, `src/features/firearms/DisposeDialog.test.tsx` and `src/features/insurance/InsurancePolicyForm.test.tsx`: a short field over 200 and a note over 4,000 show "<Label> can be at most <n> characters." at once, the submit is refused, and no input has a `maxLength` attribute (contracts/ui.md §3)
- [ ] T056 [P] [US2] Extend `src/features/import-export/ImportDialog.test.tsx` (contracts/ui.md §2): while reading, the bar reads "Reading <file name>…" with a percentage and the secondary button is **Stop reading**, enabled, calling `cancel_import`; on `OPERATION_STOPPED` the banner says "The import was cancelled. Nothing was imported." and the files stay chosen; once rows are saved the button is **Cancel**, disabled; an `IMPORT_LIMIT_EXCEEDED` shows the backend's message in the `role="alert"` banner with the files still listed
- [ ] T057 [P] [US2] Extend `e2e/specs/us5-export-import.e2e.ts`: "a refused file" (import `import-samples`' over-limit workbook of T069 and see the 16-sheet message naming it; nothing imported) and "Stop reading" (import a large valid file generated into the sandbox, press **Stop reading**, see the cancelled banner within 1 s; nothing imported)

### Implementation for User Story 2

- [ ] T058 [US2] Add `cancel_import` (unscoped, no input, `null` output) to `src-tauri/src/commands/import_export.rs`, `generate_handler!` in `src-tauri/src/main.rs`, `COMMANDS` and `UNSCOPED_COMMANDS` in `src-tauri/src/commands_list.rs`, and `allow-cancel_import` in `src-tauri/capabilities/default.json` (main web view only); it stops the running operation only if it is an `Import` in its reading phase (a phase flag in `src-tauri/src/session/operations.rs`), marking the stop as the user's (a second flag beside it), and does nothing otherwise. `acl_manifest_test.rs` and `session_scope_test.rs` pass
- [ ] T059 [P] [US2] Implement `src-tauri/src/services/import_reader/limits.rs` with data-model.md's constants verbatim: `MAX_FILE_BYTES` 268,435,456; `MAX_UNPACKED_BYTES` 1,073,741,824 ("all parts of one workbook, measured by inflating"); `MAX_ZIP_ENTRIES` 1,000; `MAX_SHEETS` 16; `MAX_ROWS` 100,000 ("data rows per table"); `MAX_COLUMNS` 256; `MAX_CELLS` 5,000,000 ("cells in all the rows sent to the parent, per import, empty ones included"); `MAX_CELL_CHARS` 32,767; `MAX_TOTAL_TEXT_BYTES` 268,435,456 ("UTF-8 bytes of all cells read, per import"); `MAX_SHARED_STRINGS` 2,000,000 ("declared (`count`, `uniqueCount`) or present"); `HELPER_MEMORY_BYTES` 536,870,912; `HELPER_IDLE_TIMEOUT` 120 s; and `ImportLimit` (`fileSize`, `unpackedSize`, `zipEntries`, `sheets`, `rows`, `columns`, `cells`, `cellText`, `totalText`, `sharedStrings`, serialized camelCase) with the message text of contracts/tauri-commands.md for each
- [ ] T060 [P] [US2] Implement `src-tauri/src/services/import_reader/frames.rs` on `services/helper/frames.rs`'s framing: `Begin { format: csv | xlsx, file_size, file_name }`, `Chunk` (≤ 1 MiB), `End`; `Progress { done, total }`, `Sheet { name }`, `Rows(Vec<Vec<String>>)` (≤ 500 rows, ending early past 2 MiB, a larger single row alone), `Finished`, `Refused { limit, sheet }`, `Unreadable` (no reason); either side refuses a frame over `MAX_IMPORT_FRAME` (64 MiB). Unit tests for round trips and an oversized frame
- [ ] T061 [US2] Implement the helper side in `src-tauri/src/services/import_reader/reader.rs` (research.md §8): CSV parsed as chunks arrive (`csv::Reader` over the incoming bytes), applying the row, column, cell-count, cell-text and total-text limits per record; XLSX kept as received (bytes counted against `MAX_FILE_BYTES`), then: the zip's central directory (≤ 1,000 entries); each needed part inflated through a counting reader stopping at `MAX_UNPACKED_BYTES` (true sizes, not declared); the shared strings' `count` and `uniqueCount` pre-read with `quick-xml` and refused over `MAX_SHARED_STRINGS` before `calamine` sees them; ≤ 16 sheets; each sheet read with `worksheet_cells_reader` (no dense grid), applying the limits as cells arrive (a row padded up to its last cell, every padded cell counted against `MAX_CELLS`) and sending `Sheet` and `Rows` batches, `Progress` in bytes (declared sizes used only for the bar, clamped). Any parser error is `Unreadable`; nothing from the file is logged
- [ ] T062 [US2] Add the `--import-helper` process mode to `src-tauri/src/main.rs`, checked before anything else beside `--render-helper` and `--webkit-sandbox-probe`: confine with `services::helper::confine(HELPER_MEMORY_BYTES)` (512 MiB `RLIMIT_DATA` on Linux, `JobMemoryLimit` on Windows; no file or network access, no core dumps, dies with the parent), then run `reader.rs`'s loop on stdin/stdout
- [ ] T063 [US2] Implement the parent side in `src-tauri/src/services/import_reader/client.rs` with `read_files(paths, cancel, is_current, progress)`: refuse a file over `MAX_FILE_BYTES` from its metadata before starting; start one helper per file through `services::helper::handle` (the job object on Windows); stream the file in 1 MiB `Chunk`s, counting bytes (a file that grows is refused); turn each `Rows` batch into import rows as it arrives, through the sheet's header mapped with `services::spreadsheet::read_table`/`recognise` (changed to take rows batch by batch if need be), so the cells aren't held twice; count cells across both files against `MAX_CELLS`; check `cancel` and `is_current()` every 100 ms and kill the helper at once on either, returning `OPERATION_STOPPED` only when the stop was the user's (T058) and the session is still current, and `DATABASE_CLOSED` otherwise (research.md §11); treat the helper's death, 120 s with no frame, or a malformed frame as `Unreadable`; refuse the whole import if either of two files is refused. Remove `read_csv`/`read_xlsx` from `src-tauri/src/services/spreadsheet.rs`, keeping `read_table` and `recognise`
- [ ] T064 [US2] Rework `import_collection` in `src-tauri/src/commands/import_export.rs` into two phases (research.md §11): register the `Import` operation (reading phase), read through `import_reader::read_files` **without taking the session's mutex**, emitting `import_collection:progress` with `phase: "reading"` (bytes across the files); then save rows under `session.write_open(...)` with the request's `SessionId` (`phase: "importing"`, rows, as before). `import_collection_stoppable` takes the tables already read, not paths. Map outcomes to contracts/tauri-commands.md: `IMPORT_LIMIT_EXCEEDED` with `details { file, sheet?, limit }` (add the code to `src-tauri/src/commands/error.rs`), `VALIDATION_ERROR` "<file>: could not read the file.", `OPERATION_STOPPED { operation: "import", importedCount: 0 }` on cancel, `DATABASE_CLOSED` when the session ended. Update `src-tauri/tests/import_export_test.rs`, `accessory_spreadsheet_test.rs`, `import_matching_test.rs`, `import_binding_test.rs`, `record_identifier_test.rs` and any other caller to read through `import_reader::read_files`. T050 and T051 pass
- [ ] T065 [P] [US2] Add `MAX_SHORT_TEXT_CHARS = 200`, `MAX_LONG_TEXT_CHARS = 4_000` and `check_text_length(label, value, max)` (Unicode scalar values after trimming; message "<Label> can be at most <n> characters.") to `src-tauri/src/models/rules.rs`, and call it for every field of data-model.md's table from `validate_firearm_input` (`src-tauri/src/models/firearm.rs`), `validate_accessory_input` (`src-tauri/src/models/accessory.rs`), the policy validation (`src-tauri/src/models/insurance_policy.rs`) and the dispose inputs' `recipient` (`src-tauri/src/commands/firearms.rs`'s `DisposeInput` and the accessory's in `src-tauri/src/commands/accessories.rs`). Imported rows go through the same validation. T052 passes
- [ ] T066 [P] [US2] Implement `src/lib/textLimits.ts` mirroring the two maximums and the check (same message and counting as T065), and give `src/components/TextArea.tsx` an optional `limit` prop showing "<n> of 4,000" (formatted) under the field from 90% of the limit, through `aria-describedby` with `aria-live="polite"`, in the error style over it. T054 passes
- [ ] T067 [US2] Carry the maximums to every form with these fields (constitution III; contracts/ui.md §3): `src/features/firearms/FirearmForm.tsx`, `src/features/accessories/AccessoryForm.tsx`, `src/features/firearms/DisposeDialog.tsx` (recipient, for both kinds of record) and `src/features/insurance/InsurancePolicyForm.tsx`: an immediate field error over the maximum as the entry fields give, `limit` on the notes and accessories text areas, and no `maxLength` attribute. T055 passes
- [ ] T068 [US2] In `src/features/import-export/ImportDialog.tsx`, `importExportService.ts` and `types.ts`: `cancelImport()` (unscoped, through `tauriClient.invoke`), the progress event's `phase`, the reading phase's "Reading <file name>…" bar with a percentage, **Stop reading** while reading and **Cancel** (disabled) while rows are saved, the cancelled banner, and the limit message in the existing error banner (contracts/ui.md §2). T056 passes
- [ ] T069 [US2] In `src-tauri/examples/human_seed.rs`, add an over-limit sample to `import-samples` (a workbook with 20 sheets, each with `x` in A1, named so its purpose is clear), list it in the import samples' README if there is one, and make sure the seeded records stay within the field maximums; `human_seed_coverage_test.rs` and `seed_sandbox_test.rs` pass
- [ ] T070 [US2] On Linux, run `cargo test` (including `import_reader_test`, `lock_test`, `text_limits_test`), `npm test`, and `npm run build` then `e2e/specs/us5-export-import.e2e.ts`, `us1-record-firearm.e2e.ts`, `us3-value-insurance.e2e.ts` and `us12-accessories.e2e.ts` one at a time in the dev container; fix every failure
- [ ] T071 [US2] On macOS, in the macOS 26 VM, run `import_reader_test` and `lock_test` (the helper there has no enforced memory cap, research.md §8, so the limits must bound it) and `us5-export-import.e2e.ts`; fix every failure
- [ ] T072 [US2] On Windows, on the Windows test machine, run `import_reader_test` and `lock_test` (the job object's memory limit) and `us5-export-import.e2e.ts`; fix every failure

**Checkpoint**: User Story 2 is fully functional and testable on its own.

---

## Phase 5: User Story 3 - No Browser Controls in the Main Window (Priority: P3)

**Goal**: In release and E2E builds, right-click opens no menu except the system's trimmed editing and copy menus, and reload, navigation, print, save, find, view-source and developer-tools keys and the mouse's back and forward buttons do nothing; development builds keep them (FR-010 to FR-014).

**Independent Test**: In a release build on each OS, right-click a blank area, a row, a button, a text field and a selection, and press each reload and navigation key and mouse button with input typed; no browser menu except editing or copying, no reload, input intact (quickstart.md rows US3-1 to US3-6, SC-005; the window controls check).

### First task 2: the macOS menu hook (research.md §13)

- [ ] T073 [US3] On macOS, in the macOS 26 VM, implement `src-tauri/src/window_controls/macos.rs`: add `willOpenMenu:withEvent:` to wry's `WryWebView` class at startup (objc2, `class_addMethod`, calling `super` first) that keeps only items whose `identifier` is `WKMenuItemIdentifierCut`, `Copy`, `Paste`, `SpellingMenu` (with its submenu's Show Spelling and Grammar and Check Spelling While Typing), `SpellingGuess`, `IgnoreSpelling` or `LearnSpelling`, and AppKit's Emoji & Symbols (action `orderFrontCharacterPalette:`), then tidies separators; log every item seen. Add `quickLookWithEvent:` to the same class, doing nothing (the three-finger-tap Look Up). In the VM, also confirm that a page preventing `webkitmouseforcewillbegin` stops a force click's Look Up, and that the page's ⌃⌘D handling (or the hook) stops ⌃⌘D's; record each in research.md §13. Right-click a text field with typed text and a selection in a test page, and record in research.md §13 which items appear and whether each excluded one (Look Up, Translate, Search With Google, Share, Services, Speech, Font, Substitutions, Transformations, Writing Direction, Writing Tools, AutoFill, Inspect Element, Reload) was removed. **If any excluded item can't be removed, or a look-up can't be stopped, stop and ask the owner** to choose among keeping that item, HoploDex's own Radix menu, or no menu on macOS (spec.md Assumptions) before T082

### Tests for User Story 3 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T074 [P] [US3] Write `src/features/app/browserControls.test.ts` (research.md §12): the exported key table (a function of the platform) blocks each of F5, Shift+F5, Ctrl+F5, Ctrl/⌘+R, Ctrl/⌘+Shift+R, Alt+←/→ on Windows and Linux only, ⌘[ and ⌘] and ⌘←/→ outside editable text only on macOS, ⌃⌘D on macOS, `BrowserBack`, `BrowserForward`, `BrowserRefresh`, `BrowserStop`, `BrowserHome`, Ctrl/⌘+P, S, U, F, G, F3, F7, F12 and Ctrl/⌘+Shift+I, J, C; lets through "/", Ctrl/⌘+L, Escape, Ctrl/⌘+C, X, V, Z, Y, Shift+Z, A and the arrow, Home and End keys with Shift, and on macOS Option+←/→ with and without Shift in editable text and elsewhere ("Option+←/→ moves by word on macOS"); the context-menu decision allows `input` of text-like types, `textarea`, `[contenteditable]` other than `"false"`, `select`, and a click inside a non-empty selection (a read-only field with a selection included), and prevents it on a blank area, a row, a card, a button, an image and a dialog body; mouse buttons 3 and 4 are prevented on `mousedown`, `mouseup` and `auxclick`; "force click is prevented on macOS" (`webkitmouseforcewillbegin`); "not installed when DEV"
- [ ] T075 [P] [US3] Add a unit test in `src-tauri/src/window_controls/mod.rs`, "filters compiled for release and e2e only": `window_controls::INSTALLED` is `cfg!(not(debug_assertions))`; T083 runs it with `cargo test --profile e2e --manifest-path src-tauri/Cargo.toml window_controls` so the E2E profile reports it true
- [ ] T076 [P] [US3] Write `e2e/specs/us15-browser-controls.e2e.ts`: dispatch `contextmenu` at a blank area, a row, a card, a button, an image and a dialog body and assert `defaultPrevented`, and at a text field, a text area and a selection and assert not (US3-1 to US3-3, page side); on Linux, with real input through `e2e/support/realInput.ts` (XTest), type into a form, then press F5, Ctrl+R, Ctrl+Shift+R, Ctrl+F5, Alt+Left, Alt+Right and the mouse's back and forward buttons, and assert the window didn't reload (a `sessionStorage` marker) and the typed input is unchanged (US3-4); on every OS, "/" focuses search, Ctrl/⌘+L locks, Escape closes a dialog, and cut, copy, paste, undo and select all work in a field (US3-5, FR-013)

### Implementation for User Story 3

- [ ] T077 [US3] Implement `src/features/app/browserControls.ts` (research.md §12): capture-phase listeners on `window` for `contextmenu` (prevent unless editable or inside a non-empty selection), `keydown` (`preventDefault()` and `stopPropagation()` for the exported table for the platform read once from `navigator.userAgent`, ⌘←/→ only outside editable text), `webkitmouseforcewillbegin` (prevented, macOS) and `mousedown`/`mouseup`/`auxclick` with `button` 3 or 4; install it from `src/main.tsx` before React renders, only when `import.meta.env.PROD`. Confirm `SessionProvider`'s Ctrl/⌘+L capture listener still runs. T074 passes
- [ ] T078 [P] [US3] Implement `src-tauri/src/window_controls/linux.rs` (research.md §13): `connect_context_menu` on the main web view keeping only the stock actions `Cut`, `Copy`, `Paste`, `Delete`, `SelectAll`, `UnicodeInsertEmoji`, `SpellingGuess`, `NoGuessesFound`, `IgnoreSpelling`, `LearnSpelling`, `IgnoreGrammar` and `Custom` spelling items, removing everything else (Paste as Plain Text and the Input Methods submenu included); an emptied menu isn't shown (return `true`); log each item offered when the check (T081) asks
- [ ] T079 [P] [US3] Implement `src-tauri/src/window_controls/windows.rs` (research.md §13, §14): `ICoreWebView2_11::add_ContextMenuRequested` on the main web view keeping items named `cut`, `copy`, `paste`, `selectAll`, `undo`, `redo`, `emoji` and the spelling items (`spellCheck` command kind), removing every other name; an empty menu sets `Handled` with no menu; `ICoreWebView2Settings3::put_AreBrowserAcceleratorKeysEnabled(false)` and `ICoreWebView2Settings6::put_IsSwipeNavigationEnabled(false)`; log each item offered when the check asks
- [ ] T080 [US3] Implement `src-tauri/src/window_controls/mod.rs`'s `apply(webview)` dispatching to each OS's file, compiled for `not(debug_assertions)` (release and E2E profiles, research.md §15), and call it on the `main` web view in `setup()` in `src-tauri/src/main.rs`. T075 passes
- [ ] T081 [US3] Write `src-tauri/examples/window_controls_check.rs` (research.md §18): open the main window as E2E isolates one (scratch config, cache and data folders) with `window_controls::apply` and a test page (blank area, row, button, image, text field with text, a selection, a `sessionStorage` marker and a typed field); drive it with real input: rename `src-tauri/examples/pdf_surface_input.{py,swift,ps1}` to `real_input.*` and update `pdf_surface_check.rs`, `scripts/pdf-surface-check.sh`, `scripts/macos/pdf-surface-check.sh`, `scripts/windows/pdf-surface-check.ps1` and DEVELOPMENT.md; right-click each target and log every item each native menu offered; press every FR-012 key and the mouse's back and forward buttons, and on macOS ⌃⌘D on the selection, logging whether a Look Up panel opened; fail (exit 4) on a menu item outside FR-011's list, a menu where none belongs, a Look Up panel, or a reload or navigation; logs to `e2e/screenshots-out/window-controls/`; exit codes 0, 1, 3, 4 as quickstart.md says. Add `scripts/window-controls-check.sh` (Linux, Xvfb + XTest), `scripts/macos/window-controls-check.sh` (the tart VM, as the PDF surface check runs) and `scripts/windows/window-controls-check.ps1`
- [ ] T082 [US3] Add `--release` to `scripts/human-testing.sh`: launch the E2E-profile build in the sandbox instead of `tauri dev`, so a person can try the browser controls (quickstart.md Setup S1); update DEVELOPMENT.md's human-testing section
- [ ] T083 [US3] On Linux, run `scripts/dev-container.sh scripts/window-controls-check.sh`, T075's unit test under `--profile e2e`, and `npm run build` then `e2e/specs/us15-browser-controls.e2e.ts`; confirm the menus offer only FR-011's items and the mouse's buttons 8 and 9 don't navigate (research.md §14); record the logged items in research.md §13 and fix every failure
- [ ] T084 [US3] On macOS, in the macOS 26 VM, run `scripts/macos/window-controls-check.sh` and `us15-browser-controls.e2e.ts`; record the logged items in research.md §13 and fix every failure
- [ ] T085 [US3] On Windows, on the Windows test machine, run `scripts\windows\window-controls-check.ps1` and `us15-browser-controls.e2e.ts`; confirm the exact WebView2 item names (research.md §13) and that XButton1 and XButton2 don't navigate (§14); record them and fix every failure

**Checkpoint**: User Story 3 is fully functional and testable on its own.

---

## Phase 6: User Story 4 - Pending Changes Kept by Another Version (Priority: P4)

**Goal**: Each database records the HoploDex version that last saved it; the pending changes dialog names it when the changes can't be resumed, and offers **Close the database** in every case, leaving the changes for the next open (FR-015 to FR-017).

**Independent Test**: A test database whose pending changes were kept by another version: open it, see the dialog name the version with Close and Discard but no Resume; close and reopen, the changes are still there; open it with the matching version and resume (quickstart.md rows US4-1 to US4-6, FR-015, SC-006).

### Tests for User Story 4 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T086 [P] [US4] Write `src-tauri/tests/app_version_test.rs`: `package.json`'s `version`, `src-tauri/tauri.conf.json`'s `version` and `src-tauri/Cargo.toml`'s `package.version` are equal, and `hoplodex::APP_VERSION` is that value (research.md §16)
- [ ] T087 [P] [US4] Write `src-tauri/tests/last_saved_version_test.rs` (FR-015; data-model.md `app_state`): `last_saved_version` is "Set at creation; inside `write_pending`'s transaction whenever a draft is written (a lock, or the immediate close at sleep or shutdown); and after any `write`, `write_open` or `write_housekeeping` that succeeded and changed rows" — test each, overwriting a seeded older value — and "Never set by opening, the open marker, closing, a backup, a restore's own bookkeeping, a passphrase change, or a write that changed nothing" — test each against a seeded `1.2.0`; a backup carries the value; writing it doesn't make a backup due
- [ ] T088 [P] [US4] Extend `src-tauri/tests/pending_resume_test.rs` with "close leaves the row; backup has no pending changes": with pending changes unresolved, `close_database` makes the normal close and its automatic backup when due, the row is untouched and the next open reports it, and the backup's `pending_changes` table is empty (US4-2, US4-5); "resume after a close returns the draft as kept" (close from `Unresolved`, reopen, `resume` returns every field as written, US4-3); and `resolve_pending_changes` `opened` and `discard` record `APP_VERSION`
- [ ] T089 [P] [US4] Extend `src/features/session/PendingChangesDialog.test.tsx` (contracts/ui.md §1): one test per wording ("They were kept by HoploDex 1.2.0. Open the database with that version to resume them, or discard them here."; "They were kept by a different build of HoploDex <appVersion>. Open the database with that build to resume them, or discard them here."; "They were kept by another version of HoploDex. Open the database with that version to resume them, or discard them here."; and the unchanged record-gone sentence); "Close the database in all three cases" (resumable, another version, record gone) as a `secondary` button between Discard and Resume; focus defaults to Resume editing when offered, otherwise Close, never Discard; a same-form-version summary kept by another version still offers Resume (US4-6); Discard still asks first (existing test kept, US4-4)
- [ ] T090 [P] [US4] Extend `e2e/specs/us9-locking.e2e.ts` with "pending changes from another version: close, then reopen" (seeded with `human_seed --pending-from 1.2.0` through the sandbox): open Shared collection, take over if asked, see "They were kept by HoploDex 1.2.0" with Discard and Close and no Resume; press **Close the database**, see the closing screen then the chooser with Shared collection selected; reopen and see the same dialog. Add "close from the dialog, reopen and resume" on the default seed (Shared collection's pending changes kept by this version): press **Close the database**, reopen, press **Resume editing**, and see the firearm form with every kept field (US4-3, SC-006)

### Implementation for User Story 4

- [ ] T091 [US4] Add `pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");` to `src-tauri/src/lib.rs`, and align the three version files if T086 finds them different. T086 passes
- [ ] T092 [US4] Add `last_saved_version TEXT` (nullable, NULL when none is recorded) to `app_state` in `src-tauri/src/db/migrations/0001_initial.sql`, edited in place (no new migration); add `db::record_saved_version(conn)` in `src-tauri/src/db/mod.rs` (`UPDATE app_state SET last_saved_version = ?1 WHERE last_saved_version IS NOT ?1` with `APP_VERSION`) and call it in `create_database`'s initialization
- [ ] T093 [US4] Record the version in `src-tauri/src/session/pending.rs`'s `write_pending`, inside its transaction, whenever a draft is written; and in `Session::write_checked` (`src-tauri/src/session/mod.rs`) after a `write`, `write_open` or `write_housekeeping` that succeeded and changed rows (`conn.total_changes()` before and after). Leave the open marker, closing, backups, restore's bookkeeping and the passphrase change untouched. T087 and T088 pass
- [ ] T094 [US4] Add `saved_by_version: Option<String>` (`app_state.last_saved_version`) and `app_version: String` (`APP_VERSION`) to `PendingSummary` in `src-tauri/src/models/database.rs`, filled where the summary is built in `src-tauri/src/session/pending.rs`; mirror `savedByVersion: string | null` and `appVersion: string` in `src/features/session/` types; `canResume` stays decided by the form version
- [ ] T095 [US4] In `src/features/session/PendingChangesDialog.tsx` and `src/features/session/SessionProvider.tsx` (after US1's T039): the three wordings of contracts/ui.md §1 when the form version differs; **Close the database** (`secondary`) in every case, between **Discard changes** (`danger`) and **Resume editing** (`primary`, `autoFocus`), and still available while "Opening…"; Close calls `closeDatabase("closed")` through the scope (the closing screen, its backup when due, the chooser with this database selected); focus defaults to Resume, else Close. T089 passes
- [ ] T096 [US4] Add `--pending-from <version>` to `src-tauri/examples/human_seed.rs`: Shared collection's seeded pending changes ("Beretta 92FS — edit") get a form version other than the current one and `last_saved_version` set to `<version>`; without the option, the seeded pending changes record `APP_VERSION`. Pass it through `scripts/human-testing.sh --pending-from <version>` and through `e2e/support/sandbox.ts`'s `seedCollection()` as an option; keep `human_seed_coverage_test.rs` passing by seeding the new column (CLAUDE.md "Keep the human-testing seed in step"), not by loosening it
- [ ] T097 [US4] On Linux, run `cargo test`, `npm test`, and `npm run build` then `e2e/specs/us9-locking.e2e.ts` and `us7-databases.e2e.ts` in the dev container; fix every failure

**Checkpoint**: All four user stories are independently functional.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Performance, screenshots, documentation, cross-cutting checks, the gates on every OS, the manual checks, and the pull request's notes.

### Performance (constitution IV, research.md §19)

- [ ] T098 On Linux, run `scripts/dev-container.sh bash -c 'cd src-tauri && cargo test --release -- --ignored import_largest'` and the release `performance_test` (as DEVELOPMENT.md runs it); record for the PR: the largest export's reading and import times and the parent's peak memory as a workbook and as two CSVs, the slowest refusal in the hostile corpus against 5 s, the largest memory rise against 512 MB, cancel and lock times against 1 s, and that search ≤ 500 ms and actions ≤ 1 s still hold with the session check; fix anything over budget
- [ ] T099 On macOS, in the macOS 26 VM, run the release `import_largest` and `performance_test` as T098 and record the same figures
- [ ] T100 On Windows, on the Windows test machine, run the release `import_largest` and `performance_test` as T098 and record the same figures

### Screens, docs and checks

- [ ] T101 [P] Add to the walk in `e2e/screenshots/screens.e2e.ts`, continuing its numbering, in light and dark, every screen of contracts/ui.md §5: the pending changes dialog kept by another version (Shared collection seeded with `--pending-from 1.2.0`); the same dialog after a failed resume with its error line; the import dialog reading a large file with **Stop reading**; the import dialog after a refused file; FirearmForm with a note past 3,600 characters (the counter) and one over 4,000 (the error). Run `scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'` and keep the before/after pairs for the PR
- [ ] T102 [P] Add a one-line `_Amended by [spec 008](../008-hardening-batch/<file>.md): …_` note at each amended anchor listed in plan.md's Project Structure: `specs/003-database-protection-management/` (spec.md FR-033 and FR-039; `contracts/ui-databases.md` §13; `contracts/tauri-commands.md`'s `DatabaseStatus`, `PendingSummary`, `resolve_pending_changes`, `close_database`, the events and "Changes to every existing command"; `data-model.md`'s `app_state` and pending changes states; `research.md` §16); `specs/001-firearms-inventory/` and `specs/006-accessory-links/` (`import_collection` in their command contracts; "Import behavior" and "Recognising a table on import" in their spreadsheet contracts; 001 FR-019, FR-020 and 006 FR-022; the field maximums beside each form's field list); `specs/007-document-preview/research.md` §11 (the helper core moved to `services/helper/`)
- [ ] T103 [P] Update `CLAUDE.md`'s Architecture section for 008: the session id (`SessionId`, `current_id()`), `ScopedSession` and the `HoploDex-Session` header, `UNSCOPED_COMMANDS` and its test (a new command is scoped unless listed there); `PendingState` and the four resolve actions; the frontend `SessionScope` (services take it first; what it owns; `ToastProvider` inside `SessionProvider`); `services/helper/` (shared by `--render-helper` and `--import-helper`), `services/import_reader/` and its limits; `window_controls/` and `browserControls.ts` (release and E2E only); the free-text maximums in `models/rules.rs` and `src/lib/textLimits.ts`; `app_state.last_saved_version` and `APP_VERSION`; and the new process mode in `main.rs`'s list
- [ ] T104 [P] Update `DEVELOPMENT.md`: the window controls check on each OS (its three scripts, exit codes, when to run it), the `real_input.*` rename, `human-testing.sh --release` and `--pending-from`, and the `import_largest` release test
- [ ] T105 Check the cross-cutting rules in the code and fix anything found: no `State<'_, Session>` outside `UNSCOPED_COMMANDS` (`session_scope_test`); no `with_generation` or `generation` left (`grep -rn 'generation' src-tauri/src`); no `read_csv`/`read_xlsx` or `calamine` use outside `services/import_reader/reader.rs` (`grep -rn 'calamine' src-tauri/src`); no `tauriClient.invoke` of a scoped command outside `SessionScope` (`grep -rn "invoke(" src --include='*.ts' --include='*.tsx'`); no `maxLength` on the free-text fields; every `#[tauri::command]` is in `COMMANDS` and the capability (`acl_manifest_test`); no new migration file (`ls src-tauri/src/db/migrations`)
- [ ] T106 On Linux, run the full gates through `scripts/dev-container.sh`: `cargo test --manifest-path src-tauri/Cargo.toml`, `npm test`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `npm run lint`, `npm run format:check`, `npm run audit`, `scripts/window-controls-check.sh`, `scripts/pdf-surface-check.sh`, and `npm run build && npm run test:e2e` one spec file at a time (`us1`–`us15`, `us7-databases-no-keyring`, `ui-review`, `webdriver-plugin`); fix every failure
- [ ] T107 On macOS, in the macOS 26 VM, run the build, `cargo test`, `npm test`, lint, the audits, `scripts/macos/window-controls-check.sh`, `scripts/macos/pdf-surface-check.sh`, every E2E spec file and the screenshot walk; fix every failure and record the result
- [ ] T108 On Windows, on the Windows test machine, run the build, `cargo test`, `npm test`, lint, the audits, `scripts\windows\window-controls-check.ps1`, `scripts\windows\pdf-surface-check.ps1`, every E2E spec file and the screenshot walk; fix every failure and record the result
- [ ] T109 Check quickstart.md end to end (every scenario-map row has its named test, and the test exists under that name); tell the user their development databases must be recreated (`0001_initial.sql` was edited in place) and that records with text over the new maximums can't be saved until shortened; run `/security-review` and save its result under `.git` (CLAUDE.md memory "Security review before PR"); draft the PR notes: before/after screenshots from T101; how the persistence and security changes meet Security & Data Handling and User Privacy (research.md §20, §22; no cipher change); the performance figures of T098–T100; the residual risks of research.md §22

### Manual checks (best effort before a release, not merge gates)

- [ ] T110 [P] On macOS, do quickstart.md's manual check **M1** (right-click menus with a trackpad, on Main collection, passphrase `human testing passphrase`), and record the result for the PR
- [ ] T111 [P] On Linux, do quickstart.md's manual check **M2** (import workbooks saved by LibreOffice Calc, and by Excel if available, on Main collection), and record the result for the PR
- [ ] T112 [P] On Windows, do quickstart.md's manual check **M2** with Excel, and record the result for the PR
- [ ] T113 [P] On Linux, do quickstart.md's manual check **M3** (pending changes from another version, by hand, on Shared collection seeded with `--pending-from 1.2.0`), and record the result for the PR

### Close-out (at pull request time)

Done when the pull request is opened, not before (CLAUDE.md; plan.md "Close-out (at pull request time)").

- [ ] T114 Open a follow-up issue with `gh issue create`: "Workbook export fails for a cell over 32,767 characters", for `photo_filenames` on a record with more than about 270 photos (research.md §9)
- [ ] T115 [P] On issue **#69**, link the pull request (it closes the issue on merge) and reply to each finding: the thumbnail cache lives in the session's scope; the resumed draft stays in the database until its form opens and is matched only within its session; attachment batches stop at the session's end and the backend refuses them; toasts are keyed by the open; every request carries the session id and the backend checks it under the lock, reads included. Name the two-database regression tests (`session_scope_test.rs` "colliding ids", `us14-session-isolation.e2e.ts`, `FirearmThumbnail.test.tsx`, `PhotoGallery.test.tsx`, `usePendingDraft.test.tsx`)
- [ ] T116 [P] On issue **#66**, link the pull request (it closes the issue) and reply: the reader runs in the confined `--import-helper` with research.md §9's limits; calamine's `uniqueCount` and dense-grid paths are refused or end only the helper; the reading is outside the session lock and cancellable. Name the hostile corpus tests and give T098–T100's measured times and memory
- [ ] T117 [P] On issue **#40**, link the pull request (it closes the issue) and comment with what each OS does (research.md §13, §14), the window controls check's results on all three (T083–T085), and which manual checks were run (T110)
- [ ] T118 [P] On issue **#78**, link the pull request (it closes the issue) and comment that the version is recorded in `app_state.last_saved_version` independent of the pending changes, as the owner asked; that the dialog names it; and that Close the database is offered in every case
- [ ] T119 [P] On issue **#24**, comment that `app_state.last_saved_version` now exists, and that showing it elsewhere or warning about a newer saver is left to #24 (spec.md Assumptions)
- [ ] T120 [P] On issue **#73**, comment that reload and history keys, buttons and menu items are now blocked in the main window, but its navigation handling itself is unchanged, so #73 stays open
- [ ] T121 [P] On issue **#21**, comment what the release security review must cover: `ScopedSession` and `UNSCOPED_COMMANDS`; the frontend scope; the import helper, its confinement, limits and frames; `window_controls` on each OS; and list research.md §22's residual risks
- [ ] T122 Put `/security-review`'s result (T109) and research.md §20's data-handling notes in the pull request body, with the performance note and the before/after screenshots (the repository's hook requires the review)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: depends on Setup; BLOCKS every user story. Within it, first task 1 (T003 → T008) must hold on all three OS before T009; T010 → T011 → T013 → T014 in order (same accessors, then their callers); T016 → T017 → T018 before the service refactors T019–T024, which can run in parallel with each other; T025 last.
- **US1 (Phase 3)**: depends on Foundational only. The MVP.
- **US2 (Phase 4)**: depends on Foundational only (it uses `ScopedSession::is_current()` and the scoped `import_collection`). Its first task (T045 → T046–T048) must hold before T061–T063.
- **US3 (Phase 5)**: depends on Foundational only, and barely: its frontend layer and native filters don't touch the session. T073 (macOS hook) settles before T080 wires the filters, and an owner decision there may change T073's file.
- **US4 (Phase 6)**: depends on Foundational, and on US1's T034, T035 and T039, which change the same files (`session/mod.rs`, `pending.rs`, `PendingChangesDialog.tsx`, `SessionProvider.tsx`) first.
- **Polish (Phase 7)**: depends on every story it covers; the close-out waits for the pull request.

### Same-file order across stories

- `src-tauri/src/session/mod.rs`: T003 → T010 → T034 (US1) → T093 (US4).
- `src-tauri/src/session/pending.rs`: T011 → T035 (US1) → T093, T094 (US4).
- `src-tauri/src/models/database.rs`: T003 → T035 (US1) → T094 (US4).
- `src-tauri/src/commands_list.rs`: T012 → T058 (US2).
- `src-tauri/src/commands/import_export.rs`: T013 → T058 → T064 (US2).
- `src-tauri/src/session/lifecycle.rs`: T010 → T011 → T015.
- `src/features/session/sessionScope.ts`: T017 → T036 → T037 (US1).
- `src/features/app/browserControls.ts`: T077 (US3), after T073's findings on macOS.
- `src-tauri/src/main.rs`: T045 → T058 → T062 (US2) → T080 (US3).
- `src/features/session/SessionProvider.tsx`: T018 → T039, T042 (US1) → T095 (US4).
- `src/features/session/PendingChangesDialog.tsx`: T039 (US1) → T095 (US4).
- `src-tauri/examples/human_seed.rs`: T033's check (US1) → T069 (US2) → T096 (US4).
- `src-tauri/tests/lock_test.rs`: T014, T015 → T051 (US2).
- `src-tauri/tests/pending_resume_test.rs`: T027 (US1) → T088 (US4).
- `scripts/human-testing.sh`: T082 (US3) → T096 (US4).

### Within Each User Story

- Tests are written first and must fail before the implementation.
- Backend state and rules before the commands that use them; commands before the frontend that calls them.
- Each story's per-OS runs come last in its phase.

### Parallel Opportunities

- T002 alongside T001.
- T016 alongside T009; T019–T023 together once T018 is done.
- Every test task marked [P] within a story, together.
- US1: T040 and T041 together after T037.
- US2: T059, T060, T065 and T066 together; T049–T057 together.
- US3: T078 and T079 together; T074–T076 together.
- US4: T086–T090 together.
- Once Foundational is done, US1, US2 and US3 can proceed in parallel; US4 follows US1's T039.
- Per-OS runs (macOS, Windows) can go to their own sessions in parallel with Linux work (CLAUDE.md memory "Per-OS sessions").

---

## Parallel Example: User Story 1

```bash
# Tests for User Story 1, together:
Task: "T026 colliding ids in src-tauri/tests/session_scope_test.rs"
Task: "T027 src-tauri/tests/pending_resume_test.rs"
Task: "T028 src/features/browse/FirearmThumbnail.test.tsx"
Task: "T029 src/features/media/PhotoGallery.test.tsx and DocumentList.test.tsx"
Task: "T030 src/features/session/usePendingDraft.test.tsx"
Task: "T032 FirearmRecordPage, AccessoryRecordPage and PolicyEditors tests"
Task: "T033 e2e/specs/us14-session-isolation.e2e.ts"

# After T037:
Task: "T040 report notOpened from the record pages, PolicyEditors and FaultBoundary"
Task: "T041 batches stop at the scope's end in PhotoGallery.tsx and DocumentList.tsx"
```

## Parallel Example: User Story 2

```bash
# Tests for User Story 2, together:
Task: "T049 src-tauri/tests/support/hostile_spreadsheets.rs"
Task: "T052 src-tauri/tests/text_limits_test.rs"
Task: "T053 src-tauri/tests/import_largest_test.rs"
Task: "T054 src/lib/textLimits.test.ts and TextArea.test.tsx"
Task: "T055 the four forms' tests"
Task: "T056 src/features/import-export/ImportDialog.test.tsx"

# Implementation building blocks, together:
Task: "T059 services/import_reader/limits.rs"
Task: "T060 services/import_reader/frames.rs"
Task: "T065 models/rules.rs field maximums"
Task: "T066 src/lib/textLimits.ts and TextArea limit"
```

## Parallel Example: User Story 3

```bash
Task: "T074 src/features/app/browserControls.test.ts"
Task: "T076 e2e/specs/us15-browser-controls.e2e.ts"
Task: "T078 src-tauri/src/window_controls/linux.rs"
Task: "T079 src-tauri/src/window_controls/windows.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational, proving the header on all three OS first.
3. Complete Phase 3: User Story 1.
4. **STOP and VALIDATE**: the two-database tests and `us14` pass; #69's paths are closed.

### Incremental Delivery

1. Setup + Foundational → every request names its session.
2. US1 → session isolation (#69, security, needs no attacker).
3. US2 → safe import (#66, security).
4. US3 → browser controls (#40), after the owner's decision on macOS if T073 asks for one.
5. US4 → pending changes from another version (#78).
6. Polish → gates on every OS, the manual checks, the PR and its close-out.

Each story adds value without breaking the previous ones; all four ship in one pull request (spec.md Assumptions).

---

## Notes

- [P] tasks = different files, no dependency on an incomplete task.
- [Story] maps a task to its user story for traceability.
- Verify each test fails before implementing it.
- Commit after each task or logical group, by path (CLAUDE.md "Commit and push").
- Never open the real databases; every run goes through the sandboxed tooling.
- Stop at any checkpoint to validate a story on its own.
