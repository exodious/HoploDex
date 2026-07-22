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
