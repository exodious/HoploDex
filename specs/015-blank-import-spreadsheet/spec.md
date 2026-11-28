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

## Clarifications

### Session 2026-11-28

The owner answered the decisions put to them (the issue's four open questions and three of clarify's) with "All recommended on 58", and asked for two issues to be filed for what the investigation found: #282 (import dates from workbook date cells and other unambiguous formats) and #283 (skip workbook sheets import doesn't recognize instead of stopping the import). Neither is fixed here.

- Q: Where is the blank spreadsheet offered, and does it work with no database open? → A: In the Import dialog only, so a database is open. A line under the file field, "No spreadsheet yet? Get a blank spreadsheet…", opens a small dialog of its own. A new database's empty collection page already leads to Import ("Import a spreadsheet"), so someone starting from scratch creates their real database and gets the template there. The database chooser and the Export dialog are unchanged (FR-001).
- Q: Which formats, and which is selected first? → A: Both, with Export's two format cards, Excel selected first, since only a workbook carries the template's notes and text cells (FR-002).
- Q: What guidance does the template carry? → A: In a workbook, a note on each header cell: whether the column is required, what it accepts, its format and an example value. No example row and no notes sheet: both would need the import to skip something (a notes sheet stops the import today; #283 is the issue for that). A CSV template is header lines only (FR-005).
- Q: Dropdown lists on the fixed-value columns? → A: No. The header notes name the accepted values (FR-005).
- Q: Are the workbook template's cells formatted as text? → A: Yes, every column, so what is typed is what import reads: a date typed into a cell formatted as a date is stored as a day number (`2024-03-01` becomes `45352`, a failed row), and a serial number typed as `00123` into a number cell becomes `123` with no error. Reading date cells is #282's (FR-006).
- Q: Does the template include `photo_filenames`? → A: No. It is export only and ignored on import (001 FR-019); the template has exactly the columns import reads (FR-003).
- Q: How does a hand-entered accessory link to a new firearm? → A: As import already allows: the same made-up version 4 UUID in the firearm's `record_id` and the accessory's `mounted_on`, or `mounted_on` left blank and the accessory mounted in HoploDex after the import. The `record_id` and `mounted_on` notes say so, and an issue for an easier link is filed when this feature's pull request opens (FR-005).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Get a blank spreadsheet to fill in (Priority: P1)

A user wants to enter their collection in a spreadsheet tool, or prepare records away from HoploDex, and import them later. In the Import dialog they choose "Get a blank spreadsheet…", pick a format and a folder, and save a file whose headers are the import's columns, with no records in it.

**Why this priority**: This is the request. Today the only way to get the headers is to export, which needs a throwaway database or leaves a copy of the real collection's data on disk.

**Independent Test**: Save a blank spreadsheet in each format and read it back: its tables and headers are the import's, and it holds no row of data.

**Acceptance Scenarios**:

1. **Given** the Import dialog is open, **When** the user chooses "Get a blank spreadsheet…", **Then** a dialog opens offering the format (Excel selected) and the folder, with Cancel and Save.
2. **Given** that dialog, **When** the user picks a format and a folder and saves, **Then** the folder holds the blank spreadsheet, and the dialog names the file or files it wrote.
3. **Given** a blank spreadsheet was saved, **When** the user opens it in a spreadsheet tool, **Then** it holds a firearm table and an accessory table whose header rows are the columns import reads, and no data rows.
4. **Given** the folder already holds a blank spreadsheet of the same name, **When** the user saves another, **Then** the new one gets a new name and the old one is untouched.
5. **Given** the user cancels the folder chooser or closes the dialog, **When** nothing was saved, **Then** no file is written, and the Import dialog is still open beneath it.
6. **Given** no folder is chosen, **When** the user presses Save, **Then** the dialog asks for the folder and writes nothing.

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

A user filling in the workbook template by hand can read what each column takes from a note on its header: whether it is required, accepted values, the date and money formats, and how `record_id` and `mounted_on` link an accessory to its host. The notes are never imported and never stop an import, and what the user types into a cell is what import reads.

**Why this priority**: Without guidance, a hand-filled template fails row by row and the user learns the format from the import's error messages.

**Independent Test**: Save a workbook template, check every header has a note, import it untouched and filled in, and check that no note became a record, a row error or a stop.

**Acceptance Scenarios**:

1. **Given** a workbook template, **When** the user rests the pointer on a header, **Then** its note says whether the column is required and what it takes, with an example value.
2. **Given** a workbook template with its notes, **When** the user imports it untouched, **Then** nothing is added and nothing is reported as failed.
3. **Given** the user types a date as `2024-03-01`, or a serial number as `00123`, into the workbook template, **When** they import it, **Then** the value is read as typed.

---

### Edge Cases

- **The import's columns change** in a later feature: the template changes with them, since both come from the same column lists (FR-003). A test fails if they part.
- **`photo_filenames`**: export only and ignored on import (001 FR-019), so the template leaves it out (FR-003).
- **A folder that can't be written** (read only, gone, out of a Flatpak's reach, a full disk): the save fails with the message export gives for the same cause, leaves no partial file, and keeps the dialog open so another folder can be chosen.
- **A folder on a network share**: export asks first because it writes the collection's records there. A blank spreadsheet holds none of the collection's data, so it does not ask (FR-008).
- **A record ID typed by hand**: `record_id` must be a version 4 UUID (006 contracts/spreadsheet-format.md "Record ID"). Linking a new accessory to a new firearm means typing the same made-up identifier in both rows; leaving `mounted_on` blank and mounting in HoploDex afterwards also works. The notes say so (FR-005).
- **A workbook the user saved with an extra sheet** (their own notes, a summary): the import stops, as for any workbook today (006 FR-022). The template adds no such sheet; #283 is the issue for skipping one.
- **A date cell or a number cell in a workbook the user changed the format of**: read as import reads any workbook today. #282 is the issue for date cells.
- **The database locks or closes** while the dialog is open: the save is refused as any command on the open database is, and nothing is written.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The user MUST be able to save a blank import spreadsheet from the Import dialog, without exporting a collection and without creating a database for the purpose. The Import dialog's first step MUST offer it as "Get a blank spreadsheet…", which opens a dialog of its own above the Import dialog. Closing that dialog MUST return to the Import dialog as it was.
- **FR-002**: The blank spreadsheet MUST be offered in both of export's formats, chosen with the same format cards, Excel (.xlsx) selected first. Its tables MUST be laid out as an export's are (006 contracts/spreadsheet-format.md "Two tables"): in a workbook, a "Firearms" sheet and an "Accessories" sheet; in CSV, a firearm file and an accessory file named `{base}.csv` and `{base}-accessories.csv`. Both tables are always written.
- **FR-003**: Each table's header row MUST be the columns the import reads, taken from `FIREARM_COLUMNS` and `ACCESSORY_COLUMNS` in their order, leaving out only the export-only `photo_filenames`. A test MUST fail if a column import reads is missing from the template, or the template has one import doesn't read.
- **FR-004**: The blank spreadsheet MUST hold no data row and nothing from the collection: no record, value, photo, policy name, or name of the database. Saving it MUST NOT read or change any record.
- **FR-005**: In a workbook, every header cell MUST carry a note saying whether the column is required, what it accepts (the accepted values of a column with a fixed list, read from the open database's lists; the date, amount and measurement formats), and an example value where one helps. The `record_id` and `mounted_on` notes MUST say how to link an accessory to its host by hand and that mounting after the import also works. A CSV template MUST be the header lines only. The template MUST carry no example row and no sheet other than the two tables, and nothing in it may be imported as a record, reported as a failed row, or stop the import.
- **FR-006**: In a workbook, every column of both tables MUST be formatted as text, so a value typed into it is stored and imported as typed.
- **FR-007**: The blank spreadsheet MUST be written with export's writers and file handling: the folder is picked in the chooser Rust opens and held as a token, as export's is; each file is created new and exclusively, never overwriting or following an existing file; `{base}` is `hoplodex-import-template`, or that with `-2`, `-3`, … when the folder already holds one of its files; a save that fails removes what it made and nothing else.
- **FR-008**: Saving a blank spreadsheet MUST NOT show export's warning about unencrypted records, and MUST NOT ask before writing to a network folder, since it holds none of the collection's data (FR-004).
- **FR-009**: After a save, the dialog MUST name the file or files written, as export's result does. A failure MUST be shown in the dialog without closing it, keeping the folder chosen, so the user can try again or choose another folder.
- **FR-010**: Importing the blank spreadsheet untouched MUST finish with nothing added, updated, skipped or failed, and leave the collection unchanged. Importing it filled in MUST behave exactly as importing an export with the same rows.
- **FR-011**: The new dialog MUST follow the Export dialog's patterns (format cards, the folder field, the result's list of paths) and the One Enter rule (it is a form with a submit button), and keep the shared components' labels, roles and keyboard operation. The link in the Import dialog MUST be a button reachable by keyboard.

### Key Entities

- **Blank import spreadsheet (template)**: a workbook, or a pair of CSV files, holding the firearm table and the accessory table with their header rows and no data rows; in a workbook, a note on each header and every column formatted as text. It is not stored in HoploDex; it is written to the folder the user picks and is theirs.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can save a blank spreadsheet in at most four steps from the Import dialog (open it, choose a folder, save, with the format already chosen), with no database created or exported for the purpose.
- **SC-002**: A blank spreadsheet, imported untouched, adds, updates and fails zero rows in both formats.
- **SC-003**: A template filled in with firearms and linked accessories imports every valid row, with each `mounted_on` link made, in both formats.
- **SC-004**: Every column the import reads appears in the template's headers, and every header has a note in the workbook, checked by automated tests that fail when a column is added to the import without reaching the template.
- **SC-005**: Saving a blank spreadsheet takes under one second on a local folder, whatever the collection's size, since it reads no record.

## Assumptions

- Writing the template reuses export's spreadsheet writer (`rust_xlsxwriter`, whose notes and column formats need no new feature or dependency), its CSV writer, its folder chooser and its exclusive file creation. No new dependency or bundled asset is needed.
- The import is unchanged.
- The template is written by the backend, like an export, so the column lists and the notes live in one place.
