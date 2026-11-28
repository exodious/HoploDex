# Research: Blank Import Spreadsheet

## §1 Where it lives

**Decision**: a "Get a blank spreadsheet…" button in the Import dialog's first step opens a dialog of its own, above the Import dialog. Nothing is added to the database chooser or the Export dialog (owner, 2026-11-28, spec.md "Clarifications").

**Rationale**: import needs an open database, and a new database's empty collection page already leads to Import ("Import a spreadsheet"), so someone starting from scratch creates the database they will keep and gets the template there. A command that runs with nothing open would join `UNSCOPED_COMMANDS`, which `docs/attack-surface.md` asks to keep minimal, and it would need the fixed lists from a copy in code rather than the open database.

**Alternatives considered**: the chooser as well (rejected above); a third "What to include" card in the Export dialog (hard to find from Import, and it sits under Export's "not encrypted" description); format and folder inline in the Import dialog (two forms in one dialog would break the One Enter rule).

## §2 Reusing export

**Decision**: the dialog picks its folder with `choose_export_folder` (`PickKind::ExportFolder`), and the new command `save_import_template` takes that token through `ops::export_to_chosen_folder`. That helper refuses a token that isn't one, checks the folder is absolute, within reach (Flatpak) and a directory, and puts the pick back when the save fails (FR-009). The files are written by `spreadsheet::write_csv_table` and by a `rust_xlsxwriter::Workbook`, as export's are, and created with export's `create_new` (`O_EXCL`/`CREATE_NEW`: nothing existing is opened, truncated or followed).

**Rationale**: the folder the template goes to is the same kind of answer as an export's ("the folder a spreadsheet is written to"), so a new `PickKind` would add a second chooser command with the same behaviour. Reusing the helper keeps one path for writing to a picked folder.

**Alternatives considered**: a native save-file dialog (one file name): it can't name the CSV template's second file, and inside a Flatpak the portal grants only the file picked, so the sibling couldn't be written.

## §3 Names

**Decision**: `{base}` is `hoplodex-import-template`; the files are `{base}.xlsx`, or `{base}.csv` and `{base}-accessories.csv`. When any of them exists (a file, a folder, a link), the next of `{base}-2`, `{base}-3`, … whose files are all free is used. Every file of a name is created before anything is written, and a name only partly free has its new files removed before moving on. A save that fails after that removes the files it made. There is no photos folder and no timestamp in the name: a template holds nothing that changes over time.

## §4 Notes and text cells

**Decision**: in the workbook, each header cell carries a `rust_xlsxwriter::Note` (no author prefix, sized to its text), and every column of both sheets is given a column format with the number format `@` (Text), so a spreadsheet tool stores what is typed into an empty cell as text. Each column is as wide as its header. The notes' text lives in `services::import_template`, keyed by table and column. The accepted values of `firearm_type`, `action_type`, `kind` and `registered_as` are read from the open database's fixed lists (every firearm type and action; the accessory kinds and classifications still offered), so a note can't name a value the import refuses. The fixed values of `status`, `no_serial_attested`, `disposition_type`, `condition` and `origin` come from the models' own labels where the model has them.

**Rationale**: a note is a separate part of the workbook (`xl/commentsN.xml` and a VML drawing), not a cell. The import helper reads only cells (`calamine`'s cell reader), so a note can never become a record or a row error. A column format writes no cell either, so a blank template has no data row. The number of parts stays far under the import's 1,000-entry limit (008 FR-006).

The text format is what keeps a typed date or serial number as typed: with the tool's default (General), `2024-03-01` is stored as the day number 45352 and fails its row, and `00123` is stored as 123 and imports as `123` with no error. Numbers typed as text (`450`, `16.25`, `TRUE`) import as before, since import reads every cell as text. Reading date cells in any workbook is issue #282, not this feature.

**Alternatives considered**: a "Read me" sheet (import stops on a sheet that is neither table, 006 FR-022; skipping such sheets is issue #283); an example row (import would have to recognise and skip it, a rule a user can trip on); dropdown lists (a second copy of each list, owner chose against).

## §5 Privacy and the attack surface

The template holds no record, value, photo, policy name or the database's name: its only content from the database is the four fixed lists, which every database at the same schema version holds. So it carries no "not encrypted" disclosure and asks nothing before a network folder (Constitution V: nothing of the collection leaves the device). The command is scoped to the open session like `export_collection`, and writes only into a folder the user picked in the chooser Rust opened: a compromised web view can make it write a header-only file into a folder the user picked, under a fixed name it can't choose, which it could already do with an export. `docs/attack-surface.md` gains the command in `files`' question about picked folders and the new module in `import-export`'s **Code**. No new **Accepted** risk.

## §6 Performance

A save runs four small `SELECT`s over the fixed lists and writes two short files: well under 1 s whatever the collection's size, since no record is read (SC-005). No search, list or record path changes.

## §7 The column list

**Decision**: `spreadsheet::EXPORT_ONLY_COLUMNS` (`photo_filenames`) is the one column export writes and import ignores (001 FR-019). The template's columns are `FIREARM_COLUMNS` and `ACCESSORY_COLUMNS` without it, in order. A unit test reads a row with a distinct value in every template column through `spreadsheet::read_table` and checks every value reaches the import row. `RawImportRow::from_cells` panics on a name not in the list, so a column import reads is always in the list; the test then fails if the template has a column import doesn't read.
