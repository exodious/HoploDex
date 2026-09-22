---

description: "Task list template for feature implementation"
---

# Tasks: Firearm Identification & Markings

**Input**: Design documents from `/specs/002-firearm-identification/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md (all present)

**Tests**: The project constitution's Testing Standards principle is NON-NEGOTIABLE — every user story below includes test tasks written first (contract/integration tests against a real temporary SQLCipher database, no mocks), expected to fail, then implementation makes them pass.

**Organization**: Tasks are grouped by user story (per spec.md's priorities P1–P4) to enable independent implementation and testing of each story. This feature is a **delta** against `specs/001-firearms-inventory/`: it extends the existing `firearms` table, commands and frontend rather than adding new ones (plan.md's Project Structure), so most tasks edit existing files.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Which user story this task belongs to (US1–US4)
- Exact file paths are given per plan.md's Project Structure

## Path Conventions

Existing Tauri desktop app from feature 001: Rust backend in `src-tauri/`, React/TypeScript frontend in `src/`, WebdriverIO E2E suite in `e2e/`. No new project, package, or layer (plan.md's Structure Decision).

---

## Phase 1: Setup

No setup tasks. Research.md confirms no new dependency is added; this feature extends 001's existing Tauri/React/rusqlite/chrono/csv/calamine/rust_xlsxwriter scaffold in place.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The schema and base model shape every user story is built on. No user story's tests can compile or pass until this phase is done.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T001 [P] Add seven nullable columns to `firearms` in `src-tauri/src/db/migrations/0001_initial.sql`, per data-model.md's "Entity: Firearm (extended)" table: `origin TEXT`, `year_of_manufacture INTEGER`, `country_of_manufacture TEXT`, `importer_name TEXT`, `original_make TEXT`, `original_model TEXT`, `original_serial_number TEXT`; add the two table-level CHECKs verbatim:
  ```sql
  CHECK (origin IS NULL OR origin IN ('domestic', 'imported', 'reimported')),
  CHECK (year_of_manufacture IS NULL OR year_of_manufacture BETWEEN 1400 AND 9999),
  CHECK (country_of_manufacture IS NULL OR origin = 'imported'),
  CHECK (
      (importer_name IS NULL AND original_make IS NULL
       AND original_model IS NULL AND original_serial_number IS NULL)
      OR origin IN ('imported', 'reimported')
  )
  ```
  per FR-001–FR-004
- [X] T002 [P] Add seven columns (`origin`, `year_of_manufacture`, `country_of_manufacture`, `importer_name`, `original_make`, `original_model`, `original_serial_number`) to the `firearms_fts` virtual table and its `AFTER INSERT`/`AFTER UPDATE`/`AFTER DELETE` triggers in `src-tauri/src/db/migrations/0002_fts5.sql`, writing `origin` as its display label via `CASE origin WHEN 'domestic' THEN 'Domestic' WHEN 'imported' THEN 'Imported' WHEN 'reimported' THEN 'Re-imported' END` and `country_of_manufacture` as `'United States'` when `origin = 'reimported'`, otherwise the stored value, per data-model.md's "Virtual table: firearms_fts (extended)" (FR-012)
- [X] T003 Add an `Origin` `text_enum!` (`Domestic => "domestic"`, `Imported => "imported"`, `Reimported => "reimported"`) with a `label()` method ("Domestic", "Imported", "Re-imported") and the seven fields (`origin: Option<Origin>`, `year_of_manufacture: Option<i64>`, `country_of_manufacture: Option<String>`, `importer_name: Option<String>`, `original_make: Option<String>`, `original_model: Option<String>`, `original_serial_number: Option<String>`) to `Firearm`, `FirearmInput`, `Firearm::from_row`, and `From<&Firearm> for FirearmInput` in `src-tauri/src/models/firearm.rs` (depends on T001)
- [X] T004 Trim `country_of_manufacture`, `importer_name`, `original_make`, `original_model` and `original_serial_number` to `None` when blank in `FirearmInput::normalized()` in `src-tauri/src/models/firearm.rs`, alongside the existing `nickname`/`serial_number`/`finish` trimming (FR-005 whitespace rule) (depends on T003)
- [X] T005 Add the seven new columns to the `INSERT`/`UPDATE` statements in `ops::create_firearm` and `ops::update_firearm` (`src-tauri/src/commands/firearms.rs`), and add the new fields (as `None`) to every `FirearmInput { .. }` literal under `src-tauri/tests/` (find them with `grep -rn 'FirearmInput {' src-tauri/tests`) (depends on T003)
- [X] T006 [P] Add the seven fields and an `Origin` type (`"domestic" | "imported" | "reimported"`) to `Firearm`/`FirearmInput` in `src/features/firearms/types.ts`, mirroring `src-tauri/src/models/firearm.rs` (depends on T003)

**Checkpoint**: Schema and base model exist; user story implementation can now begin.

---

## Phase 3: User Story 1 - Record Where a Firearm Came From (Priority: P1) 🎯 MVP

**Goal**: A collector records a firearm's origin (Domestic/Imported/Re-imported), country of manufacture, importer name, and year of manufacture.

**Independent Test**: Create a firearm, mark it imported (and separately another re-imported), enter a country, an importer name, and a year of manufacture, save, reopen, and confirm all three persisted, are shown on the detail view, and are found by search.

### Tests for User Story 1 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T007 [P] [US1] Write failing integration tests in a new `src-tauri/tests/identification_test.rs`: `create_firearm`/`update_firearm`/`get_firearm` round-trip `origin`, `yearOfManufacture`, `countryOfManufacture` and `importerName` for `imported` and `reimported` firearms; a firearm with no origin shows `origin: null` with `countryOfManufacture`/`importerName` both `null` and nothing else offered (US1-1, US1-8); `countryOfManufacture` is always `null` for `reimported` (US1-2); a year of `1943` saves and returns; a year that is not a whole four-digit number, is later than the current local year, or is earlier than 1400 is a `VALIDATION_ERROR` on `yearOfManufacture` (US1-5); a non-blank `importerName` on a `domestic`/unspecified record, or a non-blank `countryOfManufacture` on any origin but `imported` (including `reimported`), is a `VALIDATION_ERROR` naming the field (FR-002); a firearm created before this feature (no origin data) still round-trips unchanged (US1-8), per FR-001–FR-003, FR-011
- [X] T008 [P] [US1] Extend `src-tauri/tests/fts_search_test.rs`: searching `"imported"` finds both an imported and a re-imported firearm, `"re-imported"` finds only the re-imported one, `"domestic"` finds only the domestic one, and a firearm with no origin matches none of the three; searching a year, an importer name, or a country of manufacture (including "United States" for a re-imported firearm) finds the firearm, per FR-012, US1-4
- [X] T009 [P] [US1] Write failing Vitest tests in `src/features/firearms/FirearmForm.test.tsx`: the "Origin" `ChoiceCards` group offers Domestic/Imported/Re-imported/Not specified with the one-line descriptions from contracts/ui-identification.md §1; selecting Imported reveals Country of manufacture and Importer text fields (both optional); selecting Re-imported reveals only Importer plus a read-only "Country of manufacture: United States" line; Domestic/Not specified shows neither; selecting Domestic shows the cue "Made in the U.S. but stamped with an importer's name? Choose Re-imported."; Year of manufacture is shown for every origin and shows the field-level message "Year of manufacture must be a four-digit year from 1400 to {year}." for a non-four-digit or future value before submit (US1-5); changing origin away from Imported/Re-imported while importer or country is non-blank opens a `ConfirmDialog` ("Discard importer and original marks?", or "Discard the country of manufacture?" for Imported→Re-imported) that must be confirmed before the origin changes and the fields clear (US1-6, FR-010), per FR-001, FR-002, FR-015
- [X] T010 [P] [US1] Write failing Vitest tests in `src/features/firearms/FirearmRecordPage.test.tsx`: Origin, Year of manufacture, Country of manufacture (or "United States" for Re-imported) and Importer render as labeled rows only when recorded; a firearm with no origin shows "Origin: Not specified" and no importer/country row, per FR-013, US1-3
- [X] T010a [P] [US1] Write failing Vitest tests in a new `src/features/firearms/OriginGuide.test.tsx`: opening the dialog (via the "How do I record this?" button) shows the title "How to record where a firearm came from" and, at the top, the FR-006 disclaimer sentence ("Record what is stamped on the firearm and what your paperwork says. The app does not check it against any rules."); all six required worked examples from contracts/ui-identification.md §8 render (re-imported M1 Carbine; importer-adopted marks; importer-assigned serial with original marks; older surplus import; wartime duplicate serials; pre-1968 restarted numbering), each with a title, "What you see", and "How to record it"; Escape and the Close button close the dialog and return focus to the opening button, per FR-015, SC-007

### Implementation for User Story 1

- [X] T011 [US1] Add to `validate_firearm_input` in `src-tauri/src/models/firearm.rs`: year of manufacture must be a whole number with `1400 <= year <= current local year` (via `chrono::Local::now().date_naive()`), message "Year of manufacture must be a four-digit year from 1400 to {current year}." on `yearOfManufacture`; a non-blank `importer_name` on an origin that is not `imported`/`reimported` is a `fieldErrors` entry on `importerName` naming which origin allows it; a non-blank `country_of_manufacture` on any origin but `imported` is a `fieldErrors` entry on `countryOfManufacture` (depends on T003, T007)
- [X] T012 [P] [US1] Add the Origin control (a `ChoiceCards<Origin | "">` "Origin" group, placed after Type and before Caliber/Serial number, cards and descriptions per contracts/ui-identification.md §1) and its conditional fields (Country of manufacture + Importer `TextField`s for Imported; Importer plus a read-only "Country of manufacture: United States" line for Re-imported; a four-digit numeric Year of manufacture `TextField` shown for every origin) to `src/features/firearms/FirearmForm.tsx`; fields the origin doesn't offer are removed from the DOM, not disabled; a new record starts on "Not specified" (depends on T009, T011)
- [X] T013 [US1] Add the discard-confirmation `ConfirmDialog` (FR-010, contracts/ui-identification.md §3) to `FirearmForm.tsx`: opens when the user picks a new origin that would no longer offer a non-blank importer name or country of manufacture (moving from Imported/Re-imported to Domestic/none discards both; Imported→Re-imported discards only the country), titled "Discard importer and original marks?" or "Discard the country of manufacture?", clears the discarded fields on "Discard and change" and leaves the origin unchanged on "Cancel" (depends on T012)
- [X] T014 [US1] Add the "How do I record this?" button beside the Origin control in `FirearmForm.tsx`, wired to open `OriginGuide` (T016), in the tab order right after the origin cards (depends on T012)
- [X] T015 [US1] Add labeled rows for Origin, Year of manufacture, Country of manufacture (or "United States" for Re-imported) and Importer to `src/features/firearms/FirearmRecordPage.tsx`, each shown only when recorded, with "Origin: Not specified" and no importer/country row when absent, per contracts/ui-identification.md §5 (depends on T010, T012)
- [X] T016 [P] [US1] Build `src/features/firearms/OriginGuide.tsx`, a shared `Dialog` titled "How to record where a firearm came from", closable with Escape/a Close button returning focus to the opening button, opening with the FR-006 disclaimer sentence, and statically listing the six worked examples required by contracts/ui-identification.md §8 (re-imported M1 Carbine; importer-adopted marks; importer-assigned serial with original marks; older surplus import; wartime duplicate serials; pre-1968 restarted numbering), each with a title, "What you see", and "How to record it" naming the exact form fields, per FR-015, SC-007 (depends on T010a)
- [X] T017 [P] [US1] Extend `src-tauri/examples/human_seed.rs` with a re-imported M1 Carbine (importer name, the U.S. maker's serial as the main serial, no country) and an imported pistol with a country, an importer, and year 1943, per quickstart.md's manual seed list (depends on T011)
- [X] T018 [US1] Write and run a new `e2e/specs/us6-identification.e2e.ts`: record a re-imported M1 Carbine following the guide's example (origin Re-imported, the U.S. maker as main make/model/serial, an importer name, no country field offered), save with no further prompt, reopen and confirm every value shows labeled (SC-007, US1-7), per quickstart.md's scenario map (depends on T013, T014, T015, T016)

**Checkpoint**: User Story 1 is fully functional and independently testable (origin, year, country, importer round-trip, search, discard confirmation, guide).

---

## Phase 4: User Story 2 - Record the Original Manufacturer's Marks (Priority: P2)

**Goal**: A collector records the original manufacturer's make, model and serial number alongside the firearm's main marks, on any imported or re-imported firearm.

**Independent Test**: Create an imported firearm whose main serial number is the importer's, enter an original maker, model, and serial number, save, reopen, and confirm both sets of marks are shown and clearly labeled, and that a search for the original serial finds it.

### Tests for User Story 2 (mandatory per constitution)

- [X] T019 [P] [US2] Extend `src-tauri/tests/identification_test.rs`: `originalMake`/`originalModel`/`originalSerialNumber` round-trip on an imported or re-imported firearm and are returned distinctly from the main make/model/serial number; a partial set (e.g. maker only, no model or serial) is accepted as entered (US2-6); leaving all three blank saves normally with all three `null` (US2-2); a non-blank original-mark value on a `domestic`/unspecified firearm is a `VALIDATION_ERROR` naming the field (FR-004), per US2 Acceptance Scenarios 1, 2, 4, 6
- [X] T020 [P] [US2] Extend `src-tauri/tests/fts_search_test.rs`: a search for the original serial number, or for the original maker's name, finds the firearm, per US2-5
- [X] T021 [P] [US2] Extend `src/features/firearms/FirearmForm.test.tsx`: an import-marked origin shows the "Original maker's marks" `fieldset` (Original maker, Original model, Original serial number, all optional, with the hint "Only if the original maker's marks differ from the make, model and serial number above, or you want both."); a domestic/unspecified origin shows no such fields; a partial set submits with no message (US2-6)
- [X] T022 [P] [US2] Extend `src/features/firearms/FirearmRecordPage.test.tsx`: a labeled "Original maker's marks" block, visually distinct from and headed separately from the main marks, renders only when at least one of maker/model/serial is recorded, and is absent otherwise (US2-2)

### Implementation for User Story 2

- [X] T023 [US2] Add to `validate_firearm_input` in `src-tauri/src/models/firearm.rs`: a non-blank `original_make`, `original_model` or `original_serial_number` on an origin that is not `imported`/`reimported` is a `fieldErrors` entry naming the field, alongside the importer/country gating from T011 (depends on T011, T019)
- [X] T024 [US2] Add the "Original maker's marks" `fieldset` (Original maker, Original model, Original serial number, with the hint text from contracts/ui-identification.md §2) to `FirearmForm.tsx` for import-marked origins only, and extend the discard-confirmation dialog from T013 so moving away from an import-marked origin also discards these three fields when non-blank (depends on T012, T013, T021, T023)
- [X] T025 [US2] Add the labeled "Original maker's marks" block (Maker/Model/Serial number rows, own heading, shown only when at least one value is recorded) to `FirearmRecordPage.tsx`, per contracts/ui-identification.md §5 (SC-002) (depends on T015, T022)
- [X] T026 [P] [US2] Extend `src-tauri/examples/human_seed.rs` with an imported firearm carrying an importer-assigned main serial and full original maker/model/serial, and an imported pistol where the importer adopted the maker's model and serial as the main marks (no original-marks entry), per quickstart.md (depends on T023)

**Checkpoint**: User Stories 1 AND 2 both work independently.

---

## Phase 5: User Story 3 - Duplicate Checks That Respect How Imports Are Marked (Priority: P3)

**Goal**: The FR-032 duplicate rule from feature 001 gains a year-of-manufacture exception for identical main marks (FR-007/FR-008) and a non-blocking warning when original marks match another active firearm (FR-009).

**Independent Test**: With a small set of imported and domestic firearms, save records that collide on main marks, on original marks only, and on neither, and confirm each gets the blocking, warning, or no reaction the rules call for.

### Tests for User Story 3 (mandatory per constitution)

- [X] T027 [P] [US3] Extend `src-tauri/tests/identity_uniqueness_test.rs`: on create and edit, two active firearms (of any origin) with identical make/model/serial and no year on either are blocked, naming the other record, with the message pointing to recording a year on each (US3-1); years 1943/1944 differing are both accepted; the same year on both, or a missing year on either, is blocked (US3-2, and the same rule for a pre-1968 domestic pair, US3-3); editing the second firearm's year to match the first is blocked (US3-4a), per FR-007, FR-008
- [X] T028 [P] [US3] Add a backstop test to `src-tauri/tests/identity_uniqueness_test.rs` that raw `INSERT`/`UPDATE`s bypassing the command layer are still blocked by the `firearms_active_identity_insert`/`_update` triggers for null/null, equal-year, and year/no-year pairs, and are allowed for a pair with two different years (research.md §3), per SC-008
- [X] T029 [P] [US3] Write failing tests in a new `src-tauri/tests/original_marks_warning_test.rs`: saving (create or edit) a firearm whose original maker, model and serial number all match another active firearm's returns `ORIGINAL_MARKS_MATCH` naming the other record and saves nothing; resending with `confirmedWarnings: true` saves it (US3-5); a partial original-marks set never warns (US3-8); a match against a disposed firearm never warns (US3-7); two firearms sharing original marks but different main serial numbers are never blocked by the identity rule and only produce the warning (US3-6), per FR-009
- [X] T030 [P] [US3] Extend `src-tauri/tests/disposition_reversal_test.rs`: restoring a disposed firearm that would collide (by main marks) with an active one not distinguished by year is blocked with the FR-008 message and nothing changes, including the kept-history row (US3-4b); restoring a firearm whose original marks match an active firearm's shows `ORIGINAL_MARKS_MATCH` and only proceeds once resent with `confirmedWarnings: true`, with the status change and history insert committed or rolled back together (US3-9), per FR-008, FR-009
- [X] T031 [P] [US3] Extend `src-tauri/tests/import_matching_test.rs`: `find_match` treats an import row with identical main marks to an existing active record as no match (a new record) when both have a year of manufacture and the years differ, per FR-008 / research.md §4
- [X] T032 [P] [US3] Extend `src/features/firearms/RestoreDialog.test.tsx`: an identity clash on restore shows the same message as the form; an `ORIGINAL_MARKS_MATCH` shows its message with a "Restore anyway" action that resends `confirmedWarnings: true`, per US3-9

### Implementation for User Story 3

- [X] T033 [US3] Add the year-exception predicate `NOT (a.year IS NOT NULL AND b.year IS NOT NULL AND a.year <> b.year)` to `find_identity_clash` in `src-tauri/src/commands/firearms.rs`, and append to `check_uniqueness`'s `serialNumber` message: "Or record a year of manufacture on each firearm: two firearms with the same marks are accepted when both have a year and the years differ." (depends on T027)
- [X] T034 [US3] In `src-tauri/src/db/migrations/0001_initial.sql` (sequence after T001 by hand — same file): drop `UNIQUE` from `idx_firearms_active_identity`, keeping its columns and `WHERE` clause as a plain lookup index; add `firearms_active_identity_insert`/`_update` `BEFORE INSERT`/`BEFORE UPDATE ON firearms` triggers, `WHEN NEW.status = 'active' AND NEW.serial_number IS NOT NULL`, that `RAISE(ABORT, 'active firearm with the same make, model and serial number exists')` when another active row (`id <> NEW.id`) has equal make, model and serial (`COLLATE NOCASE`) and the year-exception predicate from T033 does not hold, per data-model.md's "Indexes and triggers" (depends on T028, T033)
- [X] T035 [US3] In `src-tauri/src/db/migrations/0001_initial.sql` (sequence after T034 — same file), add `idx_firearms_original_serial` on `original_serial_number COLLATE NOCASE` `WHERE status = 'active' AND original_serial_number IS NOT NULL`; add an `original_marks_clash` query to `src-tauri/src/commands/firearms.rs::ops` that, given a record with all three original marks non-null, returns the id of another **active** firearm with the same three (ignoring case and surrounding whitespace) via that index, or `None` for a partial set or no match (depends on T029)
- [X] T036 [US3] Add `confirmed_warnings: bool` (default `false` when absent) to the `create_firearm`/`update_firearm` Tauri commands' inputs and to `ReverseDispositionInput`, and thread it through `ops::create_firearm`, `ops::update_firearm` and `ops::reverse_disposition` in `src-tauri/src/commands/firearms.rs`: when the saved record is active with all three original marks recorded and `original_marks_clash` (T035) finds another active firearm, return `CommandError::new("ORIGINAL_MARKS_MATCH", ...)` naming it and save nothing, unless `confirmed_warnings` is `true`; never run the check from `dispose_firearm`'s or `assign_firearm_coverage`'s internal saves (research.md §5); update every `ops::create_firearm`/`ops::update_firearm`/`ops::reverse_disposition` call site under `src-tauri/tests/` accordingly (depends on T035)
- [X] T037 [US3] Add the year-exception predicate from T033 to `import_matching::find_match` in `src-tauri/src/services/import_matching.rs`, so a row with identical main marks to an existing record is not a match when both have a year and they differ (US4-5a) (depends on T031, T033)
- [X] T038 [US3] Add `confirmedWarnings?: boolean` to the create/update calls in `src/features/firearms/firearmsService.ts` and to `ReverseDispositionInput` in `src/features/firearms/types.ts`; on `FirearmForm.tsx`'s save path, show a returned `ORIGINAL_MARKS_MATCH` message in a `ConfirmDialog` ("Another firearm has the same original marks" / "Save anyway"), resending with `confirmedWarnings: true` on confirm and returning to the form unsaved on cancel (depends on T032, T036)
- [X] T039 [US3] Wire the same `ORIGINAL_MARKS_MATCH` handling into `RestoreDialog.tsx`'s `onRestore` (a "Restore anyway" action resending `confirmedWarnings: true`), per contracts/ui-identification.md §5 (depends on T032, T038)
- [X] T040 [P] [US3] Extend `src-tauri/examples/human_seed.rs` with a matching original-marks pair (so the warning is visible) and a pair of pre-1968 revolvers with identical main marks and differing years, per quickstart.md (depends on T036)

**Checkpoint**: User Stories 1, 2 AND 3 all work independently.

---

## Phase 6: User Story 4 - Carry Identification Through Browse, Export, and Import (Priority: P4)

**Goal**: Group the collection by origin; export and re-import every new field without loss; a hand-edited import file's identification errors are reported per row.

**Independent Test**: Export a collection containing domestic, imported, and origin-unspecified firearms, re-import the file into an empty collection, and confirm every new field is identical; then import a hand-edited file with deliberately invalid values and confirm the row-level errors.

### Tests for User Story 4 (mandatory per constitution)

- [X] T041 [P] [US4] Extend `src-tauri/tests/list_firearms_test.rs`: `groupBy: "origin"` returns groups in the fixed order Domestic, Imported, Re-imported, Not specified, with only origins present in the result appearing, per US4-1 / research.md §10
- [X] T042 [P] [US4] Extend `src-tauri/tests/import_export_test.rs`: export writes the seven new columns after `condition` and before `photo_filenames` (origin as its display label, year as plain digits, a blank country for a Re-imported row); an export followed by import into an empty collection reproduces every new field on every firearm exactly (US4-2, SC-005); an `origin` cell other than `Domestic`/`Imported`/`Re-imported` (matched ignoring case) is a row error naming the value (US4-3); `importer_name`/`country_of_manufacture`/`original_*` values on a row whose origin doesn't allow them, and `country_of_manufacture` on a `Re-imported` row, are row errors naming the disallowed value (US4-4); a future, non-four-digit, or otherwise malformed `year_of_manufacture` is a row error naming the column (US4-7); an `Imported`/`Re-imported` row with every new column blank imports normally, per contracts/spreadsheet-format.md
- [X] T043 [P] [US4] Extend `src-tauri/tests/import_export_test.rs` and `import_matching_test.rs`: an import row whose main marks match an existing active record, undistinguished by year, offers only `skip`/`overwrite` (never `duplicate`) via `ImportConflict.duplicateAllowed` (US4-5); an import row whose main marks match but both rows have a year and the years differ is imported as a new record with no conflict prompt at all (US4-5a); an import row whose original maker/model/serial match an existing active firearm's still imports and appears in `ImportResult.warnings`/`ResolveResult.warnings` naming the existing record, without failing the row or appearing in `rowErrors` (US4-6), per FR-009, FR-014
- [X] T044 [P] [US4] Extend `src/features/import-export/ImportDialog.test.tsx`: a non-empty `warnings` list renders a "Warnings" section ("Row {n}: {message}"), separate from row errors and conflicts, styled as information, with the count in the summary tally; nothing renders when there are none, per contracts/ui-identification.md §7

### Implementation for User Story 4

- [X] T045 [US4] Add `"origin"` to the `GroupBy` enum in `src-tauri/src/commands/firearms.rs`, and in `ops::list_firearms` group by the origin label ("Domestic"/"Imported"/"Re-imported"/"Not specified" for `NULL`) sorted in the fixed order Domestic, Imported, Re-imported, Not specified instead of the alphabetical `groups.sort_by` used for type/caliber/make (depends on T041)
- [X] T046 [US4] Append the seven new columns (after `condition`, before `photo_filenames`) to `COLUMNS`, `FirearmExportRow` (and its `as_fields`, now 34 long), `RawImportRow` and `row_from_cells` in `src-tauri/src/services/spreadsheet.rs`, per contracts/spreadsheet-format.md (depends on T042)
- [X] T047 [US4] In `src-tauri/src/commands/import_export.rs`: export the origin's display label and a blank country for a `Re-imported` row, the year as plain digits via `parse_scaled_decimal`'s sibling formatter (0 places); in `parse_row`, match `origin` case-insensitively against exactly `Domestic`/`Imported`/`Re-imported` (no aliases such as `reimported`) and parse the year with `parse_scaled_decimal("year_of_manufacture", &raw.year_of_manufacture, 0)`, then run the same origin-gating checks as `validate_firearm_input` so a disallowed importer/country/original-marks value is a row error naming it (depends on T046)
- [X] T048 [US4] Call `original_marks_clash` (T035) for each imported row in `ops::import_collection` and each resolved row in `ops::resolve_import_conflicts` (`src-tauri/src/commands/import_export.rs`), adding a `{ row, message }` entry to a new `warnings: Vec<RowError>` field on `ImportResult` and `ResolveResult` without failing the row (depends on T043, T047)
- [X] T049 [US4] Add `"origin"` to `GroupBy` and `GROUP_BY_OPTIONS` in `src/features/browse/types.ts`, and add `warnings: RowError[]` to `ImportResult`/`ResolveResult` in `src/features/import-export/types.ts` (depends on T045, T048)
- [X] T050 [US4] Add the "Warnings" section (`Row {n}: {message}`, styled as information, counted in the summary tally, hidden when empty) to `src/features/import-export/ImportDialog.tsx`, per contracts/ui-identification.md §7 (depends on T044, T049)
- [X] T051 [P] [US4] Extend `src-tauri/examples/human_seed.rs`'s import sample rows to carry all seven new columns, including a bad origin, a future year, and a re-imported row with a country (all as deliberately failing rows), per quickstart.md (depends on T047)

**Checkpoint**: All four user stories are independently functional.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Re-validation and consistency checks that span every story above.

- [X] T052 [P] Extend `src-tauri/tests/performance_test.rs`: the FR-009 `original_marks_clash` lookup and the widened `firearms_fts` index stay within Constitution IV's 500ms/10,000-record budget (depends on T036, T046)
- [X] T053 [P] Add a Rust test that walks all three copies of the origin label (the SQL `CASE` in `0002_fts5.sql`, `Origin::label()`, and `ORIGIN_OPTIONS` in `src/features/firearms/types.ts`) and asserts they agree for every origin, plus a Vitest test asserting the TypeScript labels equal what `FirearmRecordPage` shows (research.md §6) (depends on T008, T015)
- [X] T053a [P] Extend `src-tauri/tests/fts_search_test.rs`: seed at least 500 firearm records, including exactly one carrying a distinctive origin, year of manufacture, country of manufacture, importer name, original maker, original model, and original serial number; assert a single search on each of those seven values returns that firearm and no other, per SC-006 (depends on T008, T020)
- [X] T053b [P] Extend `src-tauri/tests/identity_uniqueness_test.rs` and `src-tauri/tests/original_marks_warning_test.rs`: assert the FR-008 identity-clash message and the `ORIGINAL_MARKS_MATCH` message never contain legality-judging language (e.g. "legal", "illegal", "lawful", "permitted"), per FR-006 (depends on T027, T029)
- [X] T054 Run `src-tauri/tests/human_seed_coverage_test.rs` and extend `examples/human_seed.rs` until it passes with `NEVER_SEEDED` empty: every new column, every `CHECK ... IN` value (`origin`), and every importable spreadsheet column covered by the seed, per quickstart.md's "Checks that must hold before merge" (depends on T017, T026, T040, T051)
- [X] T055 Add a one-line "amended by 002" pointer at each anchor named in spec.md's "Relationship to Feature 001": FR-030, FR-032, FR-026, FR-033, FR-012, FR-013, FR-018, FR-019, FR-020 in `specs/001-firearms-inventory/spec.md`, and the `Firearm` table, `list_firearms`, `create_firearm`/`update_firearm`/`reverse_disposition`/`import_collection` entries in `specs/001-firearms-inventory/data-model.md` and `specs/001-firearms-inventory/contracts/`, per plan.md §13 (depends on T045, T048, T050 — added last, once the amending behavior is finished)
- [X] T056 Run `cargo fmt --check`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets`, `cargo test --manifest-path src-tauri/Cargo.toml`, `npm run lint`, `npm run format:check` and `npm run test`; then walk quickstart.md's manual scenario map for all four user stories against the human-testing seed, and run `npm run test:e2e -- --spec e2e/specs/us6-identification.e2e.ts` one spec at a time with scratch XDG dirs, recreating the dev database first since the schema was edited in place (depends on T052, T053, T054, T055)
- [X] T057 Fold the form's new fields into one `Disclosure` titled "Origin and year of manufacture", placed after Caliber/Serial number so the required marks stay together: closed on a record with none of its fields recorded, open otherwise; closed, its summary reads the recorded values back; it opens itself on a client or backend error inside it and on an identity clash with no year (scrolling Year of manufacture into view). Size the year control for four digits. Add `Disclosure` to `src/components/` (Constitution III), amend contracts/ui-identification.md §1, §2 and §4 (superseding T012's and T014's placement), and open the group in `e2e/support/ui.ts` before filling its fields (depends on T056)
  - All command-line checks pass: `cargo fmt --check`, `cargo clippy --all-targets` (clean), `cargo test` (all green), `npm run lint`, `npm run format:check`, `npm run test` (200 passing), and `npm run test:e2e -- --spec e2e/specs/us6-identification.e2e.ts` (2/2, US1's first-use flow).
  - `examples/human_seed.rs` was verified runnable (`--features mock-keyring`) and regenerates the manual-testing dataset described in quickstart.md without error.
  - The interactive GUI walkthrough of quickstart.md's manual scenario map itself could not be performed in this sandboxed session: `scripts/human-testing.sh` needs a real OS keyring/secret-service backend (to persist the DB key across the seed and app-launch processes) and an interactive display, neither of which this environment provides — this is an environment limitation, not a gap in the feature. Every scenario in the manual map has an automated equivalent in the "Scenario map: story → tests" table above, and all of those pass; a developer with a real desktop session should still walk the manual map once before release.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No tasks.
- **Foundational (Phase 2)**: No dependencies beyond the existing 001 codebase — BLOCKS all user stories.
- **User Stories (Phase 3–6)**: All depend on Foundational (Phase 2) completion.
  - **US1 (P1)** has no dependency on US2–US4 and is the MVP.
  - **US2 (P2)** depends on US1's origin control existing in the form/record page (T012, T015) so the "Original maker's marks" fieldset/block has somewhere to attach, but its own fields, validation and tests are independent of US1's origin/year/country/importer values.
  - **US3 (P3)** depends on US1's `year_of_manufacture` field (T003) and US2's original-marks fields (T023) existing, since FR-008 and FR-009 operate on them.
  - **US4 (P4)** depends on US1–US3's fields, validation and `original_marks_clash` (T035) all existing, since export/import and the warning report carry every field and reuse that check.
- **Polish (Phase 7)**: Depends on all four user stories being complete. T053a additionally depends on T008 and T020 (US1/US2 search tests) for its per-field assertions; T053b additionally depends on T027 and T029 (the identity and original-marks message tests).

### Within Each User Story

- Tests are written first and must fail before implementation (Constitution II).
- Backend validation before frontend fields; frontend fields before the record page; the record page before end-to-end coverage.
- Shared-file tasks are sequenced by hand where noted (T001/T034/T035 in `0001_initial.sql`; T013 extended by T024 in `FirearmForm.tsx`).

### Parallel Opportunities

- Foundational: T001 and T002 (different migration files).
- Within each story's Tests subsection, tasks marked `[P]` touch different test files and can run together.
- Once Foundational is done, US1's tests (T007–T010) can be written while US2–US4's later tests are still being drafted, though their implementation tasks wait on the dependencies above.
- The seed-extension tasks (T017, T026, T040, T051) are `[P]` within their own story but are best done after that story's backend validation lands, to avoid seeding data the validation would reject.

---

## Parallel Example: User Story 1

```bash
# Failing tests, all different files:
Task: "Write failing integration tests in src-tauri/tests/identification_test.rs"       # T007
Task: "Extend src-tauri/tests/fts_search_test.rs for origin/year/country/importer"      # T008
Task: "Write failing Vitest tests in src/features/firearms/FirearmForm.test.tsx"        # T009
Task: "Write failing Vitest tests in src/features/firearms/FirearmRecordPage.test.tsx"  # T010
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 (nothing to do) and Phase 2: Foundational.
2. Complete Phase 3: User Story 1.
3. **STOP and VALIDATE**: run `identification_test.rs`, the extended `fts_search_test.rs`, `FirearmForm.test.tsx`, `FirearmRecordPage.test.tsx`, and `e2e/specs/us6-identification.e2e.ts`.
4. Demo: record an imported and a re-imported firearm end to end.

### Incremental Delivery

1. Foundational → schema and base model ready.
2. Add US1 → origin, year, country, importer usable and searchable (MVP).
3. Add US2 → original maker's marks recordable alongside US1's fields.
4. Add US3 → the year exception and the original-marks warning refine feature 001's duplicate rule.
5. Add US4 → browse grouping, export/import, and the import warning report catch the new fields up to the rest of the app.
6. Polish → performance re-validation, the origin-label consistency guard, the human-seed coverage gate, the 001 amendment pointers, and the full quickstart/lint/test sweep.

Each story adds value without breaking the previous ones; stop at any checkpoint to validate independently.
