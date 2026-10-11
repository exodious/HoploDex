# Contract: Spreadsheet Format (feature 008 delta)

A delta against 001's `contracts/spreadsheet-format.md` as amended by 002,
004, 005 and 006. Columns, recognition, formula protection and the import
report are unchanged.

## Import limits (FR-006, FR-007; research.md §9)

A file is read whole, within these limits, before any row is saved. A file
that passes any of them is refused as a whole, with `IMPORT_LIMIT_EXCEEDED`
naming the file, the sheet of a workbook, and the limit. Nothing from it,
or from the other file of a two-file import, is imported.

| Limit | Value | Applies to |
|---|---|---|
| File size | 256 MiB | each file, CSV or XLSX |
| Size once unpacked | 1 GiB, all parts together, measured by unpacking (declared sizes are not trusted) | XLSX |
| Parts | 1,000 | XLSX |
| Sheets | 16 | XLSX |
| Rows | 100,000 data rows per table | each sheet or CSV |
| Columns | 256 cells in a row | each sheet or CSV |
| Cell text | 32,767 characters | every cell |
| Text in all | 256 MiB (UTF-8) | all cells read in one import |
| Shared strings | 2,000,000, declared or present | XLSX |

A workbook whose declared counts or sizes don't match its content, or that
can't be read within these limits, is refused as unreadable (`<file>: could
not read the file.`), as is any file the reader fails on.

**What the export writes stays inside the limits**: at the largest
supported collection (10,000 firearms and 10,000 accessories, every text
field at its maximum below), about 170 MB as two CSV files, or about 100 MB
as one workbook (210 MB unpacked), with at most 42 columns, 2 sheets and
4,000 characters in a cell other than `photo_filenames` (SC-003).

## Free-text maximums (research.md §10)

Rows are validated as the forms are. A cell over its field's maximum is a
row error (`"<Label> can be at most <n> characters."`), not a refusal of the
file. The maximums, by column:

| Columns | Maximum |
|---|---|
| `make`, `model`, `cartridge`, `caliber`, `registration_form`, `registered_to` | 100 (unchanged) |
| `nickname`, `serial_number`, `finish`, `acquisition_source`, `disposition_recipient`, `country_of_manufacture`, `importer_name`, `original_make`, `original_model`, `original_serial_number` | 200 |
| `notes`, `accessories` | 4,000 |

The accessory table's columns of the same names take the same maximums.
`photo_filenames` is still ignored on import. A record with more than about
270 photos writes that cell over Excel's 32,767-character limit. That is an
existing limitation of the workbook export, recorded as a follow-up issue,
not changed here.
