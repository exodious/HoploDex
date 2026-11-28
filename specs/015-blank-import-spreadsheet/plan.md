# Implementation Plan: Blank Import Spreadsheet

**Branch**: `fm/hoplodex-058-blank-import-spreadsheet` | **Date**: 2026-11-28 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/015-blank-import-spreadsheet/spec.md`

## Summary

The Import dialog gains "Get a blank spreadsheet…", which opens a small dialog: a format (Excel first, or CSV), a folder picked in the chooser Rust opens, and Save. A new scoped command, `save_import_template`, takes the folder's token through export's `export_to_chosen_folder`. It writes the two tables' header rows with export's writers: the columns import reads (`FIREARM_COLUMNS` and `ACCESSORY_COLUMNS` without the export-only `photo_filenames`), under `hoplodex-import-template`, each file created new. In a workbook, every header carries a note and every column is formatted as text. The import is unchanged.

## Technical Context

**Language/Version**: Rust (stable, edition per `src-tauri/Cargo.toml`), TypeScript with React 18

**Primary Dependencies**: `rust_xlsxwriter` 0.100 (notes and column formats are in its default features) and `csv`, both already used by export; Tauri 2

**Storage**: none new. The template is written to a user-picked folder. The open database is read only for its fixed lists (firearm types, actions, accessory kinds, registration classifications), never a record.

**Testing**: `cargo nextest` integration tests on real SQLCipher databases and real files, read back through the real import helper (`tests/sheets`); Vitest for the dialogs; WebdriverIO E2E in `e2e/specs/export-import.e2e.ts`

**Target Platform**: Linux (incl. Flatpak), macOS, Windows desktop

**Project Type**: desktop app (Tauri)

**Performance Goals**: a save under 1 s on a local folder, whatever the collection's size (SC-005): four small lookup queries and two files of one line each

**Constraints**: no new dependency or bundled asset; import unchanged; nothing from the collection in the file (FR-004)

**Scale/Scope**: one command, one service module, one dialog, one link

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality**: no new crate or layer. One service module, `services/import_template.rs`, holds the template's notes and its workbook, beside `spreadsheet.rs`, whose writers it reuses. PASS.
- **II. Testing**: every scenario is placed below. The template is read back through the real import helper and imported into a real database. Tests write only to temporary folders. PASS.
- **III. UX Consistency**: the dialog is built from Export's parts (`ChoiceCards`, `ChosenPathField`, the result's `hd-paths` list) and is a `<form>` with a submit button (One Enter rule). It is added to the screenshot walk. PASS.
- **IV. Performance**: research.md §6. No list, search or record path changes. PASS.
- **V. User Privacy**: the template holds no record, value, policy name or database name (FR-004). Saving it leaves nothing of the collection outside the database, so it carries no unencrypted-data warning and asks nothing for a network folder (research.md §5). PASS.
- **Security gates**: attack surface below; the per-PR AI review runs before the PR. Persistence: no schema or migration change, and no record is read or written.

**Attack surface** (`docs/attack-surface.md`): `paths` (filesystem locations from the web view): `save_import_template` joins the commands that take a token for a picked folder (`choose_export_folder`'s), with the same questions (absolute, an existing directory, created exclusively, no symlink followed). `import-export`: new **Code** path `src-tauri/src/services/import_template.rs`. `ipc-commands`: one new **scoped** command, so `UNSCOPED_COMMANDS` is unchanged. No new **Accepted** risk. The reasoning is in research.md §5. The list is updated in this feature's pull request.

## Test Levels

| Story | E2E journey | Other E2E (error mechanism, real app, security) | Scenarios tested in Rust or Vitest |
|-------|-------------|-------------------------------------------------|------------------------------------|
| US1 | `export-import.e2e.ts` (its database and work folder): Import → "Get a blank spreadsheet…" → Excel selected → choose a folder → Save → the result names `hoplodex-import-template.xlsx`, which exists; again as CSV, naming both files; Done returns to the Import dialog | Security: `save_import_template` with a folder path in place of a token is refused and writes nothing (the IPC boundary, as export's test) | US1-3 headers, US1-4 a second name, a failed save removes what it made → Rust `tests/import_template_test.rs`; US1-1, US1-5, US1-6, FR-009 error kept in the dialog → Vitest `BlankSpreadsheetDialog.test.tsx`, `ImportDialog.test.tsx` |
| US2 | Same file: the CSV template untouched imports nothing; the CSV template filled with a firearm and an accessory mounted on it by `record_id` imports both, with the mount made | — | US2-1 to US2-4 in both formats (the workbook filled through the template's own `Workbook`) → Rust `tests/import_template_test.rs` |
| US3 | Same file: the workbook template untouched imports with nothing added and nothing failed | — | US3-1 (every header has a note; lists read from the database) → Rust unit tests in `services/import_template.rs` and `tests/import_template_test.rs`; US3-2 → Rust; US3-3 (every column's cells formatted as text) → Rust, reading the workbook's parts |

The template's headers matching what import reads (FR-003, SC-004) is a Rust unit test in `services/import_template.rs`: a row with a distinct value in every template column is read through `spreadsheet::read_table`, and every value reaches the import row. That fails if a column is read but missing, or listed but unread.

## Project Structure

### Documentation (this feature)

```text
specs/015-blank-import-spreadsheet/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── tauri-commands.md      # save_import_template
│   ├── spreadsheet-format.md  # the template's layout, notes and formats
│   └── ui.md                  # the Import dialog's link and the new dialog
└── tasks.md
```

### Source Code (repository root)

```text
src-tauri/
├── src/
│   ├── services/import_template.rs      # new: notes, the template workbook, the column lists
│   ├── services/spreadsheet.rs          # EXPORT_ONLY_COLUMNS; write_csv_table and export_error opened to the crate
│   ├── services/mod.rs                  # pub mod import_template
│   ├── commands/import_export.rs        # ops::save_import_template, the save_import_template command
│   ├── commands_list.rs                 # COMMANDS
│   └── main.rs                          # generate_handler!
├── capabilities/default.json            # allow the command
└── tests/
    ├── import_template_test.rs          # new
    └── integration.rs                   # mod import_template_test
src/features/import-export/
├── BlankSpreadsheetDialog.tsx           # new
├── BlankSpreadsheetDialog.test.tsx      # new
├── ImportDialog.tsx                     # the link
├── ImportDialog.test.tsx
├── importExportService.ts               # saveImportTemplate
└── types.ts                             # ImportTemplateResult
e2e/specs/export-import.e2e.ts           # 015 US1–US3
e2e/screenshots/screens.e2e.ts           # the new dialog
docs/attack-surface.md, docs/architecture/models-and-services.md, docs/architecture/frontend.md, docs/architecture/commands-and-session.md
specs/006-accessory-links/contracts/ui-accessories.md   # one _Amended by_ note (§11)
```

**Structure Decision**: the existing Tauri layout. The backend owns the template's content, so the column lists and notes live in one place beside import.

## Close-out (at pull request time)

- Issue #58 is closed by the pull request's `Closes #58` line; check `closingIssuesReferences` lists it.
- File the issue for an easier way to link a hand-entered accessory to a new firearm (D7, owner 2026-11-28): no milestone, in the project's issue style, and report its URL to firstmate.
- #282 (date cells) and #283 (unrecognised sheets) are cited in the spec but not settled by this work: no comment.

## Complexity Tracking

None.
