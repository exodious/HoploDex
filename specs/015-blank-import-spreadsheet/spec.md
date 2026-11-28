# Feature Specification: Blank Import Spreadsheet

**Feature Branch**: `fm/hoplodex-058-blank-import-spreadsheet`

**Created**: 2026-11-28

**Status**: Draft

**Input**: GitHub issue #58, "Get a blank import spreadsheet without exporting an empty database". A spreadsheet with the import's columns and no records, which the user can fill in and import, got without creating a throwaway database and exporting it.

## Source Request

This feature was filed as GitHub issue #58 (labels `enhancement`, `needs spec`; milestone 0.2.0). The owner asked for it as "Let's start work on Hoplodex 58. It needs a spec", then "Or refinement of an existing spec". It is a new feature spec rather than a change to an older one, because CLAUDE.md treats a merged spec as the record of what was decided then: the spreadsheet format stays recorded in specs 001, 002, 004, 005, 006 and 008, and this spec adds a template made from it. The issue had no comments when this spec was written. It is reproduced verbatim:

### #58: "Get a blank import spreadsheet without exporting an empty database"

> ## Request
>
> There's no way to get a blank import spreadsheet without creating a new empty database and exporting it.
>
> ## Current behaviour
>
> The only way to get a spreadsheet with the right column headers is to export a collection (`export_collection` in `src-tauri/src/commands/import_export.rs`). Export needs an open database, so someone starting from scratch, or who wants to fill in records offline before importing them, has to create a throwaway database, export it empty, then close it and open the real one. Exporting the real collection instead and deleting its rows works, but leaves a file of the collection's data lying around.
>
> ## Open questions
>
> - **Where it lives:** in the Import dialog (e.g. "Get a blank spreadsheet"), the Export dialog, the database chooser, or more than one of these. Should it be available with no database open?
> - **Formats:** a workbook with both the Firearms and Accessories sheets, the two CSV files, or both, matching export's format choice.
> - **Contents:** headers only, or headers plus guidance such as an example row, a notes sheet describing each column (accepted values, date and money formats, how `record_id` and `mounted_on` link an accessory to its host), or data validation lists for enum columns. Any example row must not be imported by mistake.
> - **Source of truth:** the headers should come from `FIREARM_COLUMNS` and `ACCESSORY_COLUMNS` so the template can't drift from what import reads.

The issue settles the last point: the template's headers come from the same column lists import reads (FR-003).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Get a blank spreadsheet to fill in (Priority: P1)

A user wants to enter their collection in a spreadsheet tool, or prepare records away from HoploDex, and import them later. They ask HoploDex for a blank spreadsheet, choose a format and a folder, and get a file whose headers are the import's columns, with no records in it.

**Why this priority**: This is the request. Today the only way to get the headers is to export, which needs a throwaway database or leaves a copy of the real collection's data on disk.

**Independent Test**: Save a blank spreadsheet in each format and read it back: its tables and headers are the import's, and it holds no row of data.

**Acceptance Scenarios**:

1. **Given** the place this feature is offered (FR-001), **When** the user asks for a blank spreadsheet, chooses a format and a folder and saves, **Then** the folder holds the blank spreadsheet, and HoploDex names the file or files it wrote.
2. **Given** a blank spreadsheet was saved, **When** the user opens it in a spreadsheet tool, **Then** it holds a firearm table and an accessory table whose header rows are the import's columns, and no data rows.
3. **Given** the folder already holds a blank spreadsheet of the same name, **When** the user saves another, **Then** the new one gets a new name and the old one is untouched.
4. **Given** the user cancels the folder chooser or closes the dialog, **When** nothing was saved, **Then** no file is written.

---

### User Story 2 - Import a filled-in template (Priority: P1)

The user fills in the blank spreadsheet with firearms and accessories, some accessories mounted on firearms, and imports it. The rows import as they would from an export, with the same checks and report.

**Why this priority**: A template that doesn't import cleanly is worse than none. The template is only useful because import reads it.

**Independent Test**: Fill a saved template's tables with rows (including an accessory whose `mounted_on` names a firearm row's `record_id`) and import it: every row is added and the accessory is mounted.

**Acceptance Scenarios**:

1. **Given** a blank spreadsheet with nothing added, **When** the user imports it, **Then** the import finishes with nothing added, updated, skipped or failed, and the collection is unchanged.
2. **Given** a template filled in with valid firearm and accessory rows, **When** the user imports it, **Then** each row becomes a record, and an accessory whose `mounted_on` names a row's `record_id` is mounted on that record.
3. **Given** a filled-in template with an invalid row, **When** the user imports it, **Then** that row is reported as failed with its reason and the others import, as for any import.
4. **Given** a template in CSV, **When** the user picks its firearm file and accessory file together, **Then** both tables import, as two export files do.

---

### User Story 3 - Guidance that is never imported (Priority: P2)

A user filling in the template by hand needs to know what each column takes: which columns are required, accepted values, the date and money formats, and how `record_id` and `mounted_on` link an accessory to its host. Whatever guidance the template carries is never imported as a record and never stops an import.

**Why this priority**: Without guidance, a hand-filled template fails row by row and the user learns the format from the import's error messages. With careless guidance (an example row), an import adds a record the user never meant to keep.

**Independent Test**: Save a template, import it untouched and filled in, and check that no guidance became a record, a row error or a stop.

**Acceptance Scenarios**:

1. **Given** the template's guidance (FR-005), **When** the user imports the template untouched, **Then** nothing is added and nothing is reported as failed.
2. **Given** the user types a date or a serial number with leading zeros into the workbook template, **When** they import it, **Then** the value is read as typed, not as a spreadsheet tool's date number or a number with the zeros dropped.

---

### Edge Cases

- **The import's columns change** in a later feature: the template changes with them, since both come from the same column lists (FR-003). A test fails if they part.
- **`photo_filenames`**: it is export only and ignored on import (001 FR-019). Whether the template carries it is decided with the contents (FR-005).
- **A folder that can't be written** (read only, gone, out of a Flatpak's reach, a full disk): the save fails with the message export gives for the same cause, and leaves no partial file.
- **A folder on a network share**: export asks first because it writes the collection's records there (`confirm_export_folder`). A blank spreadsheet holds none of the collection's data, so it does not ask (FR-008).
- **A record ID typed by hand**: `record_id` must be a version 4 UUID (006 contracts/spreadsheet-format.md "Record ID"). Linking a new accessory to a new firearm in the template means typing the same made-up identifier in both rows; leaving `mounted_on` blank and mounting in HoploDex afterwards also works. The template's guidance says so if it has guidance (FR-005).
- **The database locks or closes** while the dialog is open: the save is refused as any scoped command is, and nothing is written.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The user MUST be able to save a blank import spreadsheet without exporting a collection and without creating a database for the purpose. [NEEDS CLARIFICATION: where it is offered, and whether it works with no database open]
- **FR-002**: The blank spreadsheet MUST be offered in [NEEDS CLARIFICATION: the formats: a workbook with Firearms and Accessories sheets, the two CSV files, or both, chosen as export's format is]. Its tables MUST be laid out as an export's are (006 contracts/spreadsheet-format.md "Two tables"): in a workbook, a "Firearms" sheet and an "Accessories" sheet; in CSV, a firearm file and an accessory file named `{base}.csv` and `{base}-accessories.csv`.
- **FR-003**: Each table's header row MUST come from the column lists the import reads (`FIREARM_COLUMNS`, `ACCESSORY_COLUMNS`), in their order, so that the template cannot drift from the import. A test MUST fail if a column import reads is missing from the template.
- **FR-004**: The blank spreadsheet MUST hold no data row and nothing from the collection: no record, value, photo, policy name, or name of the database. Saving it MUST NOT need, read or change any record.
- **FR-005**: The template MUST carry [NEEDS CLARIFICATION: its guidance: headers only; notes on each header; a notes sheet; an example row; dropdown lists for fixed-value columns]. Whatever it carries MUST NOT be imported as a record, reported as a failed row, or stop the import.
- **FR-006**: The blank spreadsheet MUST be written with export's writers and file handling: the folder is picked in the chooser Rust opens and held as a token, as export's is; each file is created new and exclusively, never overwriting or following an existing file; `{base}` is `hoplodex-import-template`, or that with `-2`, `-3`, … when the folder already holds one; a save that fails removes what it made and nothing else.
- **FR-007**: Importing the blank spreadsheet untouched MUST finish with nothing added, updated, skipped or failed, and leave the collection unchanged. Importing it filled in MUST behave exactly as importing an export with the same rows.
- **FR-008**: Saving a blank spreadsheet MUST NOT show export's warning about unencrypted records, and MUST NOT ask before writing to a network folder, since it holds none of the collection's data (FR-004).
- **FR-009**: After a save, HoploDex MUST name the file or files written, as export's result does, and MUST report a failure in the dialog without closing it, so the user can choose another folder.
- **FR-010**: The new controls MUST follow the Import and Export dialogs' patterns (format cards, the folder field, the result's list of paths, One Enter rule: the dialog is a form with a submit button) and keep the shared components' labels, roles and keyboard operation.

### Key Entities

- **Blank import spreadsheet (template)**: a workbook or a pair of CSV files holding the firearm table and the accessory table with their header rows and no data rows, plus any guidance FR-005 settles. It is not stored in HoploDex; it is written to the folder the user picks and is theirs.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can save a blank spreadsheet in at most four steps from the place it is offered (open it, choose a format, choose a folder, save), with no database created or exported for the purpose.
- **SC-002**: A blank spreadsheet, imported untouched, adds, updates and fails zero rows in every format offered.
- **SC-003**: A template filled in with firearms and linked accessories imports every valid row, with each `mounted_on` link made, in every format offered.
- **SC-004**: Every column the import reads appears in the template's headers, checked by an automated test that fails when a column is added to the import without reaching the template.
- **SC-005**: Saving a blank spreadsheet takes under one second on a local folder, whatever the collection's size, since it reads no record.

## Assumptions

- Writing the template reuses export's spreadsheet writers (`rust_xlsxwriter` and `csv`, already dependencies), its folder chooser and its file reservation. No new dependency or bundled asset is needed.
- The import is unchanged unless the decided guidance needs it (a notes sheet or an example row would).
- The template is written by the backend, like an export, so the header lists exist in one place.
