---
description: "Task list for the blank import spreadsheet"
---

# Tasks: Blank Import Spreadsheet

**Input**: Design documents from `/specs/015-blank-import-spreadsheet/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Every story's tests are written first and expected to fail, at the level plan.md's Test Levels gives them.

## Format: `[ID] [P?] [Story] Description`

Bare IDs here mean this feature's. In code, cite them as `015 FR-003`, `015 US2-1`, `015 T005`, `015 research.md §4`.

---

## Phase 1: Setup

- [ ] T001 Confirm the baseline passes in the Linux dev container: `scripts/dev-container.sh npm test` and the Rust integration tests, in the background with a log.

---

## Phase 2: Foundational

**Purpose**: the template's columns, notes and workbook, which every story uses.

- [ ] T002 Write failing unit tests in src-tauri/src/services/import_template.rs: each table's template columns are its column list without `photo_filenames`, in order (015 FR-003); a row with a distinct value in every template column, read through `spreadsheet::read_table`, brings every value into the import row (015 SC-004); every template column of both tables has a non-empty note (015 FR-005); the `firearm_type`, `action_type`, `kind` and `registered_as` notes name the lists they are given.
- [ ] T003 Add `EXPORT_ONLY_COLUMNS` (`photo_filenames`) to src-tauri/src/services/spreadsheet.rs, and open `write_csv_table` and `export_error` to the crate.
- [ ] T004 Implement src-tauri/src/services/import_template.rs (and `pub mod import_template` in src-tauri/src/services/mod.rs): `TemplateLists` (the four fixed lists, read from a connection per data-model.md: every firearm type and action by `sort_order`; the accessory kinds and registration classifications where `offered`, by `sort_order`), the columns of each table, `note(table, column, lists)` with the text in contracts/spreadsheet-format.md, `workbook(lists)` (sheets "Firearms" and "Accessories", the header row, a note on every header with no author prefix, every column formatted as text `@`, each column as wide as its header) and `write_csv(firearm_out, accessory_out)` (header lines only, through `write_csv_table`).

**Checkpoint**: T002's tests pass.

---

## Phase 3: User Story 1 - Get a blank spreadsheet to fill in (Priority: P1) 🎯 MVP

**Goal**: from the Import dialog, save a blank spreadsheet in either format into a picked folder.

**Independent Test**: save a template in each format and read it back; a second save takes a new name.

- [ ] T005 [P] [US1] Write failing Rust integration tests in src-tauri/tests/import_template_test.rs (registered in src-tauri/tests/integration.rs) through `import_export::ops::save_import_template(conn, folder, format, reach)`: a workbook has exactly the sheets "Firearms" and "Accessories" with the template headers and no data row, and CSV is the two files of header lines (015 US1-3, FR-002); a second save in the same folder makes `hoplodex-import-template-2.*` and leaves the first byte for byte (015 US1-4); a name taken by something else (a folder named `hoplodex-import-template-accessories.csv`) is skipped, nothing of it touched, and the save's files made under that name are removed (015 FR-007); a save reads no record and changes nothing in the database (015 FR-004).
- [ ] T006 [US1] Implement `ops::save_import_template` and `ImportTemplateResult` in src-tauri/src/commands/import_export.rs: the lists from the connection, the base name `hoplodex-import-template` with `-2`, `-3`, … reserved by `create_new` for every file of a name before anything is written, the files written, and everything made removed when writing fails (015 FR-007; research.md §3). One debug line naming the format and `logging::place` of the folder.
- [ ] T007 [US1] Add the `save_import_template` command (scoped; input `{ format, folderToken }`; through `ops::export_to_chosen_folder` and `session.read`, on a blocking thread, as `export_collection`) in src-tauri/src/commands/import_export.rs; register it in src-tauri/src/main.rs `generate_handler!`, src-tauri/src/commands_list.rs `COMMANDS` and src-tauri/capabilities/default.json (contracts/tauri-commands.md).
- [ ] T008 [P] [US1] Add `ImportTemplateResult` and `SaveImportTemplateInput` to src/features/import-export/types.ts and `saveImportTemplate` to src/features/import-export/importExportService.ts, with its test in importExportService.test.ts.
- [ ] T009 [P] [US1] Write failing Vitest tests in src/features/import-export/BlankSpreadsheetDialog.test.tsx: Excel selected first; Save with no folder shows "Choose the folder to save the spreadsheet in." and calls nothing (015 US1-6); a save sends the format and the folder's token and shows the paths ("Spreadsheet", or "Firearms" and "Accessories") and Done (015 US1-2); a failure shows the message in an alert and keeps the dialog and the folder (015 FR-009); no privacy note (015 FR-008); Enter in the form submits Save (One Enter rule, 015 FR-011).
- [ ] T010 [US1] Implement src/features/import-export/BlankSpreadsheetDialog.tsx per contracts/ui.md §2, built from `Dialog`, `ChoiceCards`, `ChosenPathField`, `Button` and the `hd-outcome`/`hd-paths` result as ExportDialog.tsx uses them.
- [ ] T011 [US1] Write a failing Vitest test in src/features/import-export/ImportDialog.test.tsx, then add the link to src/features/import-export/ImportDialog.tsx per contracts/ui.md §1: "No spreadsheet yet?" and the **Get a blank spreadsheet…** button, disabled while importing, opening the dialog above the Import dialog, which keeps its chosen files when it closes; the link is a button found by its role and name (015 US1-1, US1-5, FR-011).
- [ ] T012 [US1] Add `describe("015 US1 - Get a blank spreadsheet to fill in")` to e2e/specs/export-import.e2e.ts: from Import, Get a blank spreadsheet…, Excel selected, choose the work folder, Save, the result names `hoplodex-import-template.xlsx` and the file exists; again as CSV with both paths; Done returns to the Import dialog. Also `save_import_template` with a folder path in place of a token is refused with `VALIDATION_ERROR` and writes nothing (`invokeFailure`, as export's test). A save to the stand-in network folder (`HOPLODEX_E2E_NETWORK_FOLDER`) writes its file without the native question export asks there (015 FR-008). Add `chooseTemplateFolder` or reuse `chooseExportFolder` in e2e/support/importExport.ts.

**Checkpoint**: US1's tests pass.

---

## Phase 4: User Story 2 - Import a filled-in template (Priority: P1)

**Goal**: a blank template imports cleanly, and a filled one imports as an export would.

**Independent Test**: import the template untouched, then filled with a firearm and an accessory mounted on it.

- [ ] T013 [US2] Add Rust tests to src-tauri/tests/import_template_test.rs, read through the real import helper (`sheets::import`): both formats' blank template imports with 0 added, updated, skipped and failed, no conflict or warning, and the collection unchanged, in an empty database and in one with records (015 US2-1, FR-010); the workbook template filled through `import_template::workbook`'s own `Workbook` (a firearm row with a version 4 `record_id`, an accessory row whose `mounted_on` names it, an invalid firearm row) imports 1 firearm and 1 accessory, mounts the accessory, and reports the invalid row as failed (015 US2-2, US2-3); the CSV template with the same rows appended to its two files, picked together, does the same (015 US2-4).
- [ ] T014 [US2] Add `describe("015 US2 - Import a filled-in template")` to e2e/specs/export-import.e2e.ts: the saved CSV template imported untouched (both files) adds 0 and fails 0; the same files with a firearm row and an accessory row mounted on it by `record_id` add 2 and fail 0, and the firearm shows "1 accessory mounted".

**Checkpoint**: US2's tests pass.

---

## Phase 5: User Story 3 - Guidance that is never imported (Priority: P2)

**Goal**: the workbook's notes and text cells guide a hand-filled template without being imported.

**Independent Test**: read the workbook's parts for a note per header and a text format per column; import it untouched.

- [ ] T015 [US3] Add Rust tests to src-tauri/tests/import_template_test.rs: the saved workbook's comments parts hold one note per header of each sheet, each with its column's text, and its sheets give every template column a style whose number format is text (`numFmtId="49"`) (015 US3-1, FR-005, FR-006); the notes name the open database's lists (a kind no longer offered is left out) (015 FR-005).
- [ ] T016 [US3] Add `describe("015 US3 - Guidance that is never imported")` to e2e/specs/export-import.e2e.ts: the saved workbook template imported untouched adds 0 and fails 0 (015 US3-2).

**Checkpoint**: every story's tests pass.

---

## Phase 6: Docs and surface

- [ ] T017 Update docs/attack-surface.md: `files` (the picked-folder question names `save_import_template`; Since), `import-export` (**Code**: `src-tauri/src/services/import_template.rs`; the export question covers the template; Since).
- [ ] T018 [P] Update docs/architecture/models-and-services.md (`import_template`), docs/architecture/frontend.md (the dialog, `ChosenPathField`'s users) and docs/architecture/commands-and-session.md (the token-taking commands).
- [ ] T019 [P] Add an `_Amended by [spec 015]…_` note to specs/006-accessory-links/contracts/ui-accessories.md §11 beside the import dialog's file picker.
- [ ] T020 Add the dialog to e2e/screenshots/screens.e2e.ts (contracts/ui.md §3).

---

## Phase 7: Validation on every environment

- [ ] T021 In the Linux dev container, run format, lint, `npm test` and the dependency and license audit, in the background with logs.
- [ ] T022 In the Linux dev container, run the Rust tests, then `npm run build` and the E2E specs (all), in the background with logs.
- [ ] T023 On macOS (the VM, under its lock, in the task's folder on the external volume), run the Rust tests and the E2E specs.
- [ ] T024 On Windows (the VM, under its lock, in the task's folder), run the Rust tests and the E2E specs.
- [ ] T025 In the Linux dev container, take the before and after screenshots of the Import dialog and the new dialog.
- [ ] T026 On macOS, take window-only before and after screenshots of the Import dialog and the new dialog.
- [ ] T027 On Windows, take window-only before and after screenshots of the Import dialog and the new dialog.
- [ ] T028 Report every test that failed then passed on rerun, and every skip, as they happen. SC-005 is shown by design (no record is read, research.md §6) and said so in the PR, rather than by a timing assertion.

---

## Phase 8: Close-out

- [ ] T029 Push the screenshots to `evidence/015-blank-import-spreadsheet-screenshots`.
- [ ] T030 Run `/ai-security-review pr` before the PR; keep its three lines in `## Security review`.
- [ ] T031 Write the PR title and description: `Closes #58` near the top, screenshots, the persistence and performance notes, what wasn't run. Check `closingIssuesReferences`.
- [ ] T032 When the PR opens, file the issue for an easier way to link a hand-entered accessory to a new firearm (no milestone) and report its URL to firstmate (plan.md "Close-out").

---

## Dependencies & Execution Order

- Phase 2 blocks every story. US1 (T005–T012) comes before US2 and US3, which use `save_import_template` to make the files they import.
- Within US1: T005 before T006–T007; T008 and T009 in parallel with the backend; T010 after T009; T011 after T010; T012 after T007 and T011.
- US2 and US3 are independent of each other once US1 is done.
- Phase 6 can run once US1 is done; Phase 7 after everything else.

### Parallel opportunities

- T005 (Rust tests), T008 (service and types) and T009 (dialog tests) touch different files.
- T018 and T019 alongside T017.

## Implementation Strategy

US1 is the MVP: the template can be saved. US2 proves it imports, and US3 proves its guidance is never imported. All three ship together in one pull request.
