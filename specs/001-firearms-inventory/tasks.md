---

description: "Task list template for feature implementation"
---

# Tasks: Firearms Collection Inventory

**Input**: Design documents from `/specs/001-firearms-inventory/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md (all present)

**Tests**: The project constitution's Testing Standards principle is NON-NEGOTIABLE — every user story below includes test tasks written first (contract/integration tests against a real temporary SQLCipher database, no mocks), expected to fail, then implementation makes them pass.

**Organization**: Tasks are grouped by user story (per spec.md's priorities P1–P5) to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Which user story this task belongs to (US1–US5)
- Exact file paths are given per plan.md's Project Structure

## Path Conventions

Tauri desktop app per plan.md: Rust backend in `src-tauri/`, React/TypeScript
frontend in `src/`, WebdriverIO E2E suite in `e2e/`.

<!-- Sample tasks from the template have been replaced with the actual task breakdown below. -->

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [X] T001 Create Tauri 2.x project scaffold (`src-tauri/` Rust crate + `src/` React+TypeScript app) per plan.md's Project Structure
- [X] T002 Add backend dependencies to `src-tauri/Cargo.toml`: `tauri`, `rusqlite` (`bundled-sqlcipher`, `fts5` features), `keyring`, `rust_xlsxwriter`, `calamine`, `csv`, `serde`/`serde_json`, `tokio`
- [X] T003 [P] Add frontend dependencies to `package.json`: `react`, `typescript`, `vite`, Radix UI primitives (shadcn/ui pattern), `vitest`, `@testing-library/react`
- [X] T004 [P] Configure linting/formatting: `rustfmt.toml` + `clippy` lint config for `src-tauri/`, `eslint`/`prettier` config for `src/`
- [X] T005 [P] Scaffold WebdriverIO + `tauri-driver` E2E harness in `e2e/wdio.conf.ts`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T006 Implement SQLCipher-backed `rusqlite` connection manager with `keyring`-based passphrase generation/unlock in `src-tauri/src/db/mod.rs`
- [X] T007 Write initial schema migration (FirearmType, Firearm, Photo, DocumentAttachment, InsurancePolicy tables per data-model.md) in `src-tauri/src/db/migrations/0001_initial.sql` (depends on T006)
- [X] T008 Write `firearms_fts` FTS5 virtual table + sync triggers migration in `src-tauri/src/db/migrations/0002_fts5.sql` (depends on T007)
- [X] T009 [P] Seed `FirearmType` lookup rows (Handgun/Rifle/Shotgun/Other + `generic_thumbnail_key`) in `src-tauri/src/db/migrations/0003_seed_firearm_types.sql`
- [X] T010 [P] Implement shared `CommandError` type (code/message/fieldErrors) and error mapping in `src-tauri/src/commands/error.rs`
- [X] T011 Wire Tauri app bootstrap (DB init on startup, plugin registration, command-handler scaffolding) in `src-tauri/src/main.rs` (depends on T006)
- [X] T012 [P] Bundle generic thumbnail image assets per firearm type as Tauri app resources in `src-tauri/resources/thumbnails/`
- [X] T013 [P] Build shared accessible component-library primitives (Button, Dialog, ConfirmDialog, TextField, Select) per WCAG 2.1 AA in `src/components/`
- [X] T014 [P] Implement typed Tauri `invoke()` wrapper service in `src/services/tauriClient.ts`
- [X] T015 [P] Build real-temp-SQLCipher-DB test harness (no mocks) in `src-tauri/tests/support/mod.rs`

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 1 - Record a Firearm (Priority: P1) 🎯 MVP

**Goal**: A collector can create, edit, dispose, and delete a firearm record capturing identifying details, condition notes, acquisition/disposition info, and estimated value, including the serial-number attestation rule.

**Independent Test**: Add a new firearm record with all core fields, save it, reopen it to confirm persistence, edit a field, and mark it disposed — independent of search, photos, or export.

### Tests for User Story 1 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T016 [P] [US1] Integration test: create/edit/dispose/delete firearm lifecycle against a real SQLCipher DB (Acceptance Scenarios 1–5) in `src-tauri/tests/firearm_lifecycle_test.rs`
- [X] T017 [P] [US1] Integration test: serial-number attestation validation — blank+attested saves, blank+unattested blocked (Scenarios 6–7) in `src-tauri/tests/serial_attestation_test.rs`
- [X] T018 [P] [US1] Vitest unit test for `FirearmForm` serial-attestation UI rule in `src/features/firearms/FirearmForm.test.tsx`
- [X] T019 [P] [US1] E2E test covering all User Story 1 acceptance scenarios in `e2e/specs/us1-record-firearm.e2e.ts`

### Implementation for User Story 1

- [X] T020 [P] [US1] Implement `Firearm` model struct + validation rules from data-model.md in `src-tauri/src/models/firearm.rs`
- [X] T021 [US1] Implement `create_firearm` command in `src-tauri/src/commands/firearms.rs` (depends on T020)
- [X] T022 [US1] Implement `update_firearm` command in `src-tauri/src/commands/firearms.rs` (depends on T021)
- [X] T023 [US1] Implement `dispose_firearm` command in `src-tauri/src/commands/firearms.rs` (depends on T021)
- [X] T024 [US1] Implement `delete_firearm` command with cascade delete of Photos/DocumentAttachments in `src-tauri/src/commands/firearms.rs` (depends on T021)
- [X] T025 [US1] Implement `get_firearm` command in `src-tauri/src/commands/firearms.rs` (depends on T021)
- [X] T026 [US1] Register firearm commands in the Tauri invoke handler in `src-tauri/src/main.rs` (depends on T021-T025)
- [X] T027 [P] [US1] Build `FirearmForm` component (create/edit, serial-attestation checkbox) in `src/features/firearms/FirearmForm.tsx`
- [X] T028 [P] [US1] Build `FirearmDetail` view (edit, dispose, delete via shared `ConfirmDialog`) in `src/features/firearms/FirearmDetail.tsx`
- [X] T029 [US1] Wire `FirearmForm`/`FirearmDetail` to `create_firearm`/`update_firearm`/`dispose_firearm`/`delete_firearm`/`get_firearm` via `tauriClient` in `src/features/firearms/` (depends on T026, T027, T028)

**Checkpoint**: User Story 1 fully functional and testable independently

---

## Phase 4: User Story 2 - Browse, Search, and Group the Collection (Priority: P2)

**Goal**: A collector can view the collection as list or tiles, group by structured fields, and search all recorded information including free-form notes.

**Independent Test**: Populate several firearm records, switch list/tile views, group by a structured field, and search a term found only in free-form notes — independent of photos, valuation, or export.

### Tests for User Story 2 (mandatory per constitution)

- [X] T030 [P] [US2] Integration test for `list_firearms` search/group/`includeDisposed` behavior against a real DB (Scenarios 1–5) in `src-tauri/tests/list_firearms_test.rs`
- [X] T031 [P] [US2] Integration test confirming FTS5 search matches both free-form notes and structured fields in `src-tauri/tests/fts_search_test.rs`
- [X] T032 [P] [US2] E2E test covering all User Story 2 acceptance scenarios in `e2e/specs/us2-browse-search.e2e.ts`

### Implementation for User Story 2

- [X] T033 [US2] Implement `list_firearms` command (query/groupBy/includeDisposed/view) in `src-tauri/src/commands/firearms.rs` (depends on T020, T008)
- [X] T034 [US2] Register `list_firearms` in `src-tauri/src/main.rs` (depends on T033)
- [X] T035 [P] [US2] Build `BrowseList` (list-layout) component in `src/features/browse/BrowseList.tsx`
- [X] T036 [P] [US2] Build `BrowseTiles` (tile/thumbnail-layout) component in `src/features/browse/BrowseTiles.tsx`
- [X] T037 [P] [US2] Build `GroupByControl` (type/caliber/make) in `src/features/browse/GroupByControl.tsx`
- [X] T038 [P] [US2] Build `SearchBar` component in `src/features/browse/SearchBar.tsx`
- [X] T039 [US2] Wire view-switching/grouping/search state to `list_firearms` in `src/features/browse/BrowsePage.tsx` (depends on T034-T038)

**Checkpoint**: User Stories 1 AND 2 both work independently

---

## Phase 5: User Story 3 - Track Value and Insurance Coverage (Priority: P3)

**Goal**: A collector records insurance coverage per firearm, views an always-current value summary broken down per policy, and sees uninsured/under-insured/policy-expiry warnings.

**Independent Test**: Set estimated value and insurance coverage on a mix of firearms, view the value summary, then add/edit/dispose/delete firearms and confirm the summary and warnings update automatically — independent of photos or export.

### Tests for User Story 3 (mandatory per constitution)

- [X] T040 [P] [US3] Integration test for insurance-policy CRUD and delete-blocked-while-assigned behavior in `src-tauri/tests/insurance_policy_test.rs`
- [X] T041 [P] [US3] Integration test for insurance-status calculations — uninsured, under-insured, blanket-limit-exceeded, expired-policy override (Scenarios 1–4, 7–8) in `src-tauri/tests/insurance_status_test.rs`
- [X] T042 [P] [US3] Integration test for value-summary auto-recompute-on-mutation and per-policy/unassigned breakdown (Scenarios 5–6) in `src-tauri/tests/valuation_test.rs`
- [X] T043 [P] [US3] E2E test covering all User Story 3 acceptance scenarios in `e2e/specs/us3-value-insurance.e2e.ts`

### Implementation for User Story 3

- [X] T044 [P] [US3] Implement `InsurancePolicy` model + validation (end date after start date) in `src-tauri/src/models/insurance_policy.rs`
- [X] T045 [US3] Implement `services::insurance_status` (under/uninsured, blanket-exceeded, expiry/expired override per FR-024/028) in `src-tauri/src/services/insurance_status.rs` (depends on T044, T020)
- [X] T046 [US3] Implement `services::valuation` (collection total + per-policy breakdown + unassigned group per FR-015) in `src-tauri/src/services/valuation.rs` (depends on T045)
- [X] T047 [US3] Implement `create_insurance_policy`/`update_insurance_policy` commands in `src-tauri/src/commands/insurance.rs` (depends on T044)
- [X] T048 [US3] Implement `delete_insurance_policy` command (`ON DELETE RESTRICT` + `POLICY_HAS_FIREARMS` error) in `src-tauri/src/commands/insurance.rs` (depends on T047)
- [X] T049 [US3] Implement `assign_firearm_coverage` command in `src-tauri/src/commands/insurance.rs` (depends on T047)
- [X] T050 [US3] Implement `get_value_summary` command in `src-tauri/src/commands/insurance.rs` (depends on T046)
- [X] T051 [US3] Register insurance/valuation commands in `src-tauri/src/main.rs` (depends on T047-T050)
- [X] T052 [P] [US3] Build `InsurancePolicyForm` component in `src/features/insurance/InsurancePolicyForm.tsx`
- [X] T053 [P] [US3] Build `CoverageAssignment` control (policy/kind/amount) on the firearm detail view in `src/features/insurance/CoverageAssignment.tsx`
- [X] T054 [P] [US3] Build `ValueSummaryPanel` (collection total, per-policy breakdown, unassigned group) in `src/features/insurance/ValueSummaryPanel.tsx`
- [X] T055 [P] [US3] Build shared `InsuranceWarningBadge` (uninsured/under-insured/policy-expiring/expired) in `src/components/InsuranceWarningBadge.tsx`
- [X] T056 [US3] Wire `ValueSummaryPanel`/`InsuranceWarningBadge` to re-invoke `get_value_summary` after every mutating command in `src/features/insurance/` (depends on T051-T055)

**Checkpoint**: User Stories 1, 2, AND 3 all work independently

---

## Phase 6: User Story 4 - Attach Photos and Documents (Priority: P4)

**Goal**: A collector adds photos to a firearm, designates a thumbnail, and attaches documents (e.g., a PDF).

**Independent Test**: Add two photos, confirm the first becomes the thumbnail, explicitly select the second instead, attach a PDF, and confirm a firearm with no photos shows its type's generic thumbnail — independent of export.

### Tests for User Story 4 (mandatory per constitution)

- [X] T057 [P] [US4] Integration test for `add_photo`/`set_thumbnail_photo`/`delete_photo` including thumbnail-fallback behavior (Scenarios 1–3) in `src-tauri/tests/photo_test.rs`
- [X] T058 [P] [US4] Integration test for `add_document`/`get_document`/`delete_document` (Scenario 4) in `src-tauri/tests/document_test.rs`
- [X] T059 [P] [US4] E2E test covering all User Story 4 acceptance scenarios in `e2e/specs/us4-photos-documents.e2e.ts`

### Implementation for User Story 4

- [X] T060 [P] [US4] Implement `Photo` model + thumbnail-generation helper in `src-tauri/src/models/photo.rs`
- [X] T061 [P] [US4] Implement `DocumentAttachment` model in `src-tauri/src/models/document_attachment.rs`
- [X] T062 [P] [US4] Implement `add_photo`/`set_thumbnail_photo`/`delete_photo` commands in `src-tauri/src/commands/photos.rs` (depends on T060)
- [X] T063 [P] [US4] Implement `add_document`/`get_document`/`delete_document` commands in `src-tauri/src/commands/documents.rs` (depends on T061)
- [X] T064 [US4] Register photo/document commands in `src-tauri/src/main.rs` (depends on T062, T063)
- [X] T065 [P] [US4] Implement generic-thumbnail fallback resolution (FirearmType lookup, research.md §10) in `src-tauri/src/services/thumbnails.rs`
- [X] T066 [P] [US4] Build `PhotoGallery` + thumbnail-picker component in `src/features/media/PhotoGallery.tsx`
- [X] T067 [P] [US4] Build `DocumentList` component (attach/reopen) in `src/features/media/DocumentList.tsx`
- [X] T068 [US4] Wire `PhotoGallery`/`DocumentList` to photo/document commands and update `BrowseList`/`BrowseTiles` thumbnail display in `src/features/media/` (depends on T064-T067)

**Checkpoint**: User Stories 1–4 all work independently

---

## Phase 7: User Story 5 - Export and Import Records (Priority: P5)

**Goal**: A collector exports the full collection (with photos) to a spreadsheet backup and imports firearm records (without photos) from a spreadsheet, with per-row error reporting and conflict resolution.

**Independent Test**: Export a populated collection to a spreadsheet and a photo folder, inspect for completeness, and import a separately prepared spreadsheet to confirm new/updated records appear correctly — independent of UI browsing or valuation.

### Tests for User Story 5 (mandatory per constitution)

- [X] T069 [P] [US5] Integration test: `export_collection` produces a spreadsheet + photos folder matching contracts/spreadsheet-format.md (Scenario 1) in `src-tauri/tests/export_test.rs`
- [X] T070 [P] [US5] Integration test: `import_collection` creates/updates records and reports failing rows without discarding successful ones (Scenarios 2–3) in `src-tauri/tests/import_export_test.rs`
- [X] T071 [P] [US5] Integration test: import matching/conflict resolution — make+model+serial key, no-serial-always-new, apply-to-remaining (FR-026, FR-030) in `src-tauri/tests/import_matching_test.rs`
- [X] T072 [P] [US5] E2E test covering all User Story 5 acceptance scenarios in `e2e/specs/us5-export-import.e2e.ts`

### Implementation for User Story 5

- [X] T073 [US5] Implement `services::spreadsheet` CSV/XLSX writer (`rust_xlsxwriter`/`csv`) per contracts/spreadsheet-format.md in `src-tauri/src/services/spreadsheet.rs`
- [X] T074 [US5] Implement `services::spreadsheet` CSV/XLSX reader (`calamine`/`csv`) + per-row validation in `src-tauri/src/services/spreadsheet.rs` (depends on T073)
- [X] T075 [P] [US5] Implement `services::import_matching` (make+model+serial key matching, no-serial-always-new) in `src-tauri/src/services/import_matching.rs`
- [X] T076 [US5] Implement `export_collection` command (`scope: all|filtered`, progress events) in `src-tauri/src/commands/import_export.rs` (depends on T073)
- [X] T077 [US5] Implement `import_collection` command (progress events, per-row error report) in `src-tauri/src/commands/import_export.rs` (depends on T074, T075)
- [X] T078 [US5] Implement `resolve_import_conflicts` command (per-row + apply-to-remaining) in `src-tauri/src/commands/import_export.rs` (depends on T077)
- [X] T079 [US5] Register import/export commands in `src-tauri/src/main.rs` (depends on T076-T078)
- [X] T080 [P] [US5] Build `ExportWizard` (format/destination/scope picker + progress bar) in `src/features/import-export/ExportWizard.tsx`
- [X] T081 [P] [US5] Build `ImportWizard` (file picker + progress bar + row-error report) in `src/features/import-export/ImportWizard.tsx`
- [X] T082 [P] [US5] Build `ImportConflictResolver` (per-row skip/overwrite/duplicate + apply-to-remaining) in `src/features/import-export/ImportConflictResolver.tsx`
- [X] T083 [P] [US5] Build shared `ProgressBar` component consuming Tauri progress events in `src/components/ProgressBar.tsx`
- [X] T084 [US5] Wire `ExportWizard`/`ImportWizard`/`ImportConflictResolver` to import/export commands via `ProgressBar` in `src/features/import-export/` (depends on T079-T083)

**Checkpoint**: All 5 user stories independently functional

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [X] T085 [P] Run `clippy`/`rustfmt` and `eslint`/`prettier` across the full codebase and fix violations
- [X] T086 [P] Accessibility audit of the shared component library against WCAG 2.1 AA in `src/components/`
- [X] T087 [P] Performance validation: confirm `list_firearms`/`get_value_summary` complete within the 500ms budget at a 10,000-record fixture in `src-tauri/tests/performance_test.rs`
- [X] T088 Execute the full quickstart.md validation walkthrough across all 5 user stories
- [X] T089 [P] Update `README.md` with build/run/test instructions

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories
- **User Stories (Phase 3-7)**: All depend on Foundational phase completion
  - Can proceed in parallel if staffed, or sequentially in priority order (P1 → P2 → P3 → P4 → P5)
- **Polish (Phase 8)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: No dependency on other stories — the MVP
- **User Story 2 (P2)**: Depends on the `Firearm` model (T020, from US1) existing; independently testable once list_firearms exists
- **User Story 3 (P3)**: Depends on the `Firearm` model (T020, from US1); independently testable via its own policy/coverage/summary commands
- **User Story 4 (P4)**: Depends on the `Firearm` model (T020, from US1); independently testable via its own photo/document commands
- **User Story 5 (P5)**: Depends on the `Firearm` model (T020, from US1) and benefits from `InsurancePolicy` (T044, from US3) for full-fidelity export/import of policy assignments, but row-level export/import is testable against firearms alone

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Models before services/commands
- Commands before command registration in `main.rs`
- Commands before frontend wiring
- Story complete before moving to the next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel (within Phase 2)
- Once Foundational completes, all 5 user stories can start in parallel (if team capacity allows)
- All tests for a user story marked [P] can run in parallel
- Independent-file model/component tasks within a story marked [P] can run in parallel

---

## Parallel Example: User Story 1

```bash
# Launch all tests for User Story 1 together:
Task: "Integration test: create/edit/dispose/delete firearm lifecycle in src-tauri/tests/firearm_lifecycle_test.rs"
Task: "Integration test: serial-number attestation validation in src-tauri/tests/serial_attestation_test.rs"
Task: "Vitest unit test for FirearmForm serial-attestation rule in src/features/firearms/FirearmForm.test.tsx"
Task: "E2E test for User Story 1 in e2e/specs/us1-record-firearm.e2e.ts"

# Launch independent-file implementation tasks together:
Task: "Implement Firearm model in src-tauri/src/models/firearm.rs"
Task: "Build FirearmForm component in src/features/firearms/FirearmForm.tsx"
Task: "Build FirearmDetail view in src/features/firearms/FirearmDetail.tsx"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Run T016-T019 and the US1 quickstart.md scenarios independently
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 (Browse/Search/Group) → Test independently → Deploy/Demo
4. Add User Story 3 (Value/Insurance) → Test independently → Deploy/Demo
5. Add User Story 4 (Photos/Documents) → Test independently → Deploy/Demo
6. Add User Story 5 (Export/Import) → Test independently → Deploy/Demo
7. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (then User Story 2, which depends on its model)
   - Developer B: User Story 3
   - Developer C: User Story 4
   - Developer D: User Story 5 (spreadsheet/import-matching services can start immediately; full-fidelity policy export waits on US3's model)
3. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependency on an incomplete task
- [Story] label maps task to specific user story for traceability
- Every user story is independently completable and testable per its Independent Test statement in spec.md
- Verify tests fail before implementing (red-green per constitution Principle II)
- Commit after each task or logical group
- Stop at any checkpoint to validate a story independently
- All integration tests run against a real temporary SQLCipher database (`src-tauri/tests/support/mod.rs`) — no mocks, per the constitution's Testing Standards

---

## Phase 9: Convergence

**Purpose**: Remaining work found by `/speckit-converge` after the 2026-09-19/20 spec revisions. The app is unreleased, so schema changes edit `0001`/`0002` directly with no data migration. Within each group, write the failing tests first (Constitution II).

- [X] T090 CRITICAL: Write a failing regression test that a decrypted `open_document` copy is deleted (contents overwritten, then removed) by the exit cleanup and by the startup sweep, including retry of anything that could not be deleted, in `src-tauri/tests/document_test.rs` per Constitution V / FR-035 (contradicts)
- [X] T091 CRITICAL: Implement a best-effort secure-delete helper (overwrite then unlink) in `src-tauri/src/services/secure_delete.rs` and use it to clear `OPENED_DOCUMENTS_DIR` in `src-tauri/src/commands/documents.rs` per Constitution V / FR-035 (contradicts) (depends on T090)
- [X] T092 CRITICAL: Run the opened-documents cleanup on normal application exit (Tauri run-event exit handler) and replace the startup `remove_dir_all` with the secure sweep in `src-tauri/src/main.rs` per FR-035 / SC-010 (contradicts) (depends on T091)
- [ ] T093 CRITICAL: Extend `e2e/specs/us4-photos-documents.e2e.ts` to assert no decrypted copy remains after the app exits and is relaunched per US4/AC5 (missing)
- [X] T094 [P] Write failing tests for future acquisition/disposition dates and disposition-before-acquisition (create, update, dispose, import row error; today accepted) in `src-tauri/tests/firearm_lifecycle_test.rs` and `src-tauri/tests/import_export_test.rs` per US1/AC13–14 (missing)
- [X] T095 Implement date validation against the local current date in `src-tauri/src/models/firearm.rs`, `dispose_firearm` in `src-tauri/src/commands/firearms.rs`, and the import row validator in `src-tauri/src/commands/import_export.rs` per FR-003 / FR-004 (missing) (depends on T094)
- [X] T096 [P] Enforce the same date rules in `src/features/firearms/FirearmForm.tsx` and `src/features/firearms/DisposeDialog.tsx` (max date = today, field-level messages, disposition ≥ acquisition) with Vitest coverage in `FirearmForm.test.tsx` and `DisposeDialog.test.tsx` per FR-003 / FR-004 (missing) (depends on T095)
- [X] T097 [P] Write failing nickname tests (case/whitespace-insensitive uniqueness among active firearms, released on disposal, blank stored as null, searchable, blocked on create/edit/import/reversal) in `src-tauri/tests/nickname_test.rs` per US1/AC8–9 (missing)
- [X] T098 Add `nickname` to `firearms` with a partial unique NOCASE index on active rows in `src-tauri/src/db/migrations/0001_initial.sql` and to the FTS table and triggers in `src-tauri/src/db/migrations/0002_fts5.sql` per FR-031 (missing) (depends on T097)
- [X] T099 Add `nickname` to `Firearm`, `FirearmInput` and `FirearmSummary`, trimming blanks to null, with the uniqueness check naming the conflicting record, in `src-tauri/src/models/firearm.rs` and `src-tauri/src/commands/firearms.rs` per FR-031 (missing) (depends on T098)
- [X] T100 [P] Add the `nickname` column to export and import (duplicate nickname is a row error) in `src-tauri/src/services/spreadsheet.rs` and `src-tauri/src/commands/import_export.rs` per FR-031 / contracts/spreadsheet-format.md (missing) (depends on T099)
- [X] T101 [P] Add the nickname field to `src/features/firearms/FirearmForm.tsx` and a shared display-name helper that shows nickname alongside make and model in `BrowseList.tsx`, `BrowseTiles.tsx`, `FirearmRecordPage.tsx`, the value summary, insurance warnings and `CoverageDialog.tsx`, with Vitest coverage, per FR-031 / US1/AC8 (missing) (depends on T099)
- [ ] T102 [P] Write failing tests for make+model+serial uniqueness (non-exempt duplicate blocked and named, exempt duplicate saves with a warning, disposed match never blocks or warns, no-serial never compared, case/whitespace-insensitive, re-checked on edit) in `src-tauri/tests/identity_uniqueness_test.rs` per US1/AC10–11 (missing)
- [ ] T103 Add the partial unique NOCASE index on `(make, model, serial_number)` for active, non-exempt, serial-bearing rows in `src-tauri/src/db/migrations/0001_initial.sql` per FR-032 (missing) (depends on T102)
- [ ] T104 Implement the FR-032 check in `src-tauri/src/commands/firearms.rs` and return `warnings: string[]` from `create_firearm` and `update_firearm` per FR-032 / contracts/tauri-commands.md (missing) (depends on T103)
- [ ] T105 Apply FR-032 to every import row and offer only skip/overwrite (reject `duplicate`) where it would block, with tests in `src-tauri/tests/import_matching_test.rs`, in `src-tauri/src/commands/import_export.rs` and `src-tauri/src/services/import_matching.rs` per FR-026 / FR-032 (missing) (depends on T104)
- [ ] T106 [P] Show returned warnings after save and hide the "duplicate" option in `src/features/import-export/ImportDialog.tsx` where FR-032 would block it, in `src/features/firearms/FirearmForm.tsx` and `src/features/firearms/firearmsService.ts` per FR-026 / FR-032 (missing) (depends on T105)
- [ ] T107 [P] Write failing tests for `reverse_disposition` (keep stores history, discard stores none, status active with disposition fields null, blocked with a named record on nickname or make/model/serial clash and nothing changed, repeated dispose/restore accumulates history, history removed with the firearm) in `src-tauri/tests/disposition_reversal_test.rs` per US1/AC12 (missing)
- [ ] T108 Add the `disposition_history` table (`ON DELETE CASCADE`) in `src-tauri/src/db/migrations/0001_initial.sql` per FR-033 / data-model.md (missing) (depends on T107)
- [ ] T109 Implement `reverse_disposition` (one transaction, re-runs FR-031/FR-032, optional rename) in `src-tauri/src/commands/firearms.rs`, return retained history from `get_firearm`, and register the command in `src-tauri/src/main.rs` per FR-033 (missing) (depends on T099, T104, T108)
- [ ] T110 [P] Build a keep/discard reversal dialog on the shared `ConfirmDialog` pattern, with rename-on-clash, and a retained-history list on `src/features/firearms/FirearmRecordPage.tsx`; refresh the value summary afterwards, per FR-033 / Constitution III (missing) (depends on T109)
- [ ] T111 [P] Write failing policy tests for an optional blanket limit, overlapping blanket policies blocked with the other policy named, a shared boundary day accepted, and edits or renewals re-checked, in `src-tauri/tests/insurance_policy_test.rs` per US3/AC12 (missing)
- [ ] T112 [P] Rewrite the insurance-status and valuation tests for implicit blanket coverage (unscheduled covered by the blanket policy in force, uninsured when none, blanket total vs limit, expired scheduled policy uninsured, later-starting policy wins on the boundary day, FR-028 expiring/expired suppression, new-shape summary with `blanket`/`byPolicy`/`uninsured`) and drop `coverage_kind` from all test fixtures in `src-tauri/tests/` per US3/AC1, 4, 6, 8, 11, 13–14 (contradicts)
- [ ] T113 Remove `coverage_kind` and make `blanket_coverage_limit` nullable in `src-tauri/src/db/migrations/0001_initial.sql` per FR-027 / FR-036 (contradicts) (depends on T111, T112)
- [ ] T114 Make the blanket limit optional and enforce at most one blanket policy in force (overlap of more than one shared boundary day is blocked, message names the other policy) on create and update in `src-tauri/src/models/insurance_policy.rs` and `src-tauri/src/commands/insurance.rs` per FR-036 / FR-027 (contradicts) (depends on T113)
- [ ] T115 Remove `CoverageKind` from the firearm model and commands and make `assign_firearm_coverage` schedule-only (`policyId` null unschedules) in `src-tauri/src/models/firearm.rs`, `src-tauri/src/commands/firearms.rs` and `src-tauri/src/commands/insurance.rs` per FR-014 / FR-036 (contradicts) (depends on T113)
- [ ] T116 Rewrite `src-tauri/src/services/insurance_status.rs` around `blanket_in_force` (implicit coverage of unscheduled firearms, uninsured when none is in force, expired scheduled policy uninsured, FR-028 expiring/expired suppression for superseded blanket policies) per FR-016 / FR-017 / FR-024 / FR-028 (contradicts) (depends on T115)
- [ ] T117 Rewrite `src-tauri/src/services/valuation.rs` to return `collectionTotal`, `blanket`, `byPolicy` and `uninsured` per contracts/tauri-commands.md per FR-015 (contradicts) (depends on T116)
- [ ] T118 [P] Remove the `coverage_kind` column from export and import in `src-tauri/src/services/spreadsheet.rs` and `src-tauri/src/commands/import_export.rs` so an unscheduled row means blanket-covered, per contracts/spreadsheet-format.md (contradicts) (depends on T115)
- [ ] T119 [P] Update the frontend coverage types, services and helpers (`src/features/firearms/types.ts`, `src/features/insurance/types.ts`, `insuranceService.ts`, `coverage.ts`, `coverage.test.ts`) to schedule-only coverage and the new value-summary shape per FR-014 / FR-015 (contradicts) (depends on T117)
- [ ] T120 [P] Rework `CoverageDialog.tsx`, `CoverageCell.tsx` and `FirearmRecordPage.tsx` so a firearm is scheduled under a policy or left unscheduled (showing "covered by <blanket policy>" or "uninsured"), with `CoverageDialog.test.tsx` updated, per FR-014 / FR-036 (contradicts) (depends on T119)
- [ ] T121 [P] Rework `InsurancePolicyForm.tsx`, `PolicyCard.tsx`, `InsurancePage.tsx` and the value-summary panel: optional blanket limit, blanket block for the policy in force, scheduled firearms per policy, distinct uninsured group, expiring/expired warnings with FR-028 suppression, per FR-015 / FR-027 / FR-028 (contradicts) (depends on T119)
- [ ] T122 Update `e2e/specs/us3-value-insurance.e2e.ts` for implicit blanket coverage, overlap blocking, boundary-day acceptance and warning suppression per US3/AC11–14 (partial) (depends on T120, T121)
- [ ] T123 [P] Write failing tests for `get_policy_deletion_impact` and `delete_insurance_policy` (move keeps scheduled amounts, unschedule falls to the blanket policy in force or uninsured, non-expired unschedule needs `confirmUnschedule`, expired needs only a warning, unresolved deletion refused with `POLICY_HAS_FIREARMS`, in-force blanket deletion reports `blanketFirearmCount`, one transaction) in `src-tauri/tests/insurance_policy_test.rs` per US3/AC9–10, 15 (missing)
- [ ] T124 Implement `get_policy_deletion_impact` and the move/unschedule resolution in `delete_insurance_policy` (FK stays `ON DELETE RESTRICT` as a backstop) in `src-tauri/src/commands/insurance.rs`, and register the impact command in `src-tauri/src/main.rs`, per FR-034 (missing) (depends on T115, T123)
- [ ] T125 [P] Build the policy-deletion dialog (move to another policy with a reminder to confirm coverage, or leave unscheduled with the stronger confirmation for a non-expired policy, the expired-policy warning, and the in-force blanket warning) on the shared `ConfirmDialog` pattern in `src/features/insurance/` and wire it into `PolicyCard.tsx`, per FR-034 / Constitution III (missing) (depends on T124)
- [ ] T126 [P] Add `.github/workflows/ci.yml` running `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `eslint`, `prettier --check`, `vitest` and the app build on Windows, macOS and Linux, and document it in `README.md`, per plan: Testing / FR-022 / Constitution I (missing)
- [ ] T127 [P] Re-validate the 500 ms `list_firearms` and `get_value_summary` budget at the 10,000-record fixture with nicknames and blanket computation in `src-tauri/tests/performance_test.rs` per Constitution IV (partial) (depends on T117)
- [ ] T128 Update `e2e/specs/us1-record-firearm.e2e.ts` for nickname, duplicate blocking/warning, date validation and disposition reversal (US1/AC8–14), then re-run the full quickstart.md walkthrough across all five user stories, per T088 / US1 (partial) (depends on T101, T106, T110, T122, T125)
