# Contract: Spreadsheet Format (delta)

This is a **delta** against
[the 001 spreadsheet contract](../../001-firearms-inventory/contracts/spreadsheet-format.md),
as amended by 002, 004 and 005. It adds a second table, the accessory table,
and two columns to the firearm table. Every rule not named here is
unchanged:
- cells are formatted as before;
- amounts are whole dollars with no separators, and at most $99,999,999 (_amended 2026-10-09 (#67): a larger one is a row error naming the column_);
- dates are `YYYY-MM-DD`;
- columns are read by header, trimmed and in any letter case;
- unknown headers are ignored;
- a duplicate known header stops the import, and the message now names
  the file (and the sheet of a workbook that holds several) as well as the
  column, since one import can take two files: `accessories.csv: the header
  has two "make" columns.` (issue #56);
- text is protected against formula injection (below).

## Formula injection (CSV)

A spreadsheet program reads a cell that starts with `=`, `+`, `-`, `@`, a
tab or a carriage return as a formula. The two formats differ:

- **CSV export** writes a `'` before any cell that starts with one of those
  characters, in every column of both tables.
- **CSV import** removes one leading `'` from a cell when it is followed by
  one of those characters. Export then import returns the exact text of
  every cell (SC-002). A `'` followed by anything else is text and is kept.
- **XLSX** is unchanged: every cell is a typed string cell, which a
  spreadsheet program does not evaluate, so the text is written and read
  as it is.
- **One accepted ambiguity**: a CSV cell that genuinely starts with `'` and
  then one of those characters (`'=1`) loses its `'` on import, whether
  the file is an export of this application (a record whose text starts
  `'=`) or one written by hand. Text that starts with a `'` and anything
  else is unaffected.

## Two tables

An export writes firearms and accessories as **separate tables** (FR-020):

| Format | Firearm table | Accessory table (only when the export includes an accessory) |
|---|---|---|
| XLSX | sheet **"Firearms"** of `{base}.xlsx` | sheet **"Accessories"** of the same workbook |
| CSV | `{base}.csv` | `{base}-accessories.csv`, beside it |

_Amended 2026-10-09 (#74, #68): `{base}` is `hoplodex-export-YYYYMMDD-HHMMSS`, or that with `-2`, `-3`, … added when the destination folder already holds an export of that name; an export reserves its spreadsheet, accessory file and `{base}_photos/` folder as new (never reusing, overwriting or clearing an existing one), so the names to use are the ones in `ExportResult`. A photo's file name is its stored name reduced to a basename (no folders, drive prefix or control characters), with `-2`, `-3`, … before the extension when two photos of one record share a name._

- `{base}` is `hoplodex-export-YYYYMMDD-HHMMSS`, as before.
- Photos go to `{base}_photos/` as before. A firearm's photos are named
  `{firearm id}_{original filename}`, and an accessory's
  `a{accessory id}_{original filename}`.
- `photo_filenames` lists them, separated by `;`, as before. Import ignores
  photos (001 FR-019).

**Which records** (FR-020):
- **Whole collection**: every firearm and every accessory, active and
  disposed.
- **Filtered**: the firearms the collection page's filter matches, plus
  everything mounted on them at any depth, firearms and accessories alike.
  A firearm mounted on an exported firearm is exported even when the
  filter doesn't match it. Nothing else is exported: no unmounted
  accessory, and no host of an exported record unless the filter or this
  rule includes it.

A row's `mounted_on` names its host even when the host is not in the
export (US5-9).

## Recognising a table on import (FR-022)

Import takes one workbook, one CSV file, or two files picked together (two
single-sheet workbooks, two CSV files, or one of each). Every non-blank
sheet of every file is recognised by its header row:

| Header row has | It is |
|---|---|
| `firearm_type`, no `kind` | the firearm table |
| `kind`, no `firearm_type` | the accessory table |
| both, or neither | not a table: **the import stops** |

- **The import also stops**, before any row is saved, when two sheets or
  files hold the same table, or when more than two files are picked.
- **The stop message** names the file, and the sheet in a workbook:
  - "{file}: this isn't a HoploDex firearm or accessory table. Its header
    needs a firearm_type or a kind column."
  - "{file} and {other file} both hold firearms. Pick one firearm table
    and at most one accessory table."
- A sheet with no header row (an empty sheet) is ignored.
- A sheet exported before this feature has `firearm_type`, so it is the
  firearm table (US5-7).
- **Either table may be imported alone.** With no accessory table, no
  accessory is imported or changed. With no firearm table, no firearm is.

## Firearm table: new columns

| Column | Position | Export | Import |
|---|---|---|---|
| `record_id` | **first** | The firearm's identifier (a lowercase UUID) | Optional. See "Record ID" |
| `mounted_on` | after `registered_to`, before `photo_filenames` | The direct host's identifier, a firearm's or an accessory's; blank when not mounted | Optional. See "Mounted on" |

All other columns are unchanged, in their order after `record_id`.
`FIREARM_COLUMNS` (renamed from `COLUMNS`) is the canonical list. The
free-text `accessories` column is unchanged (FR-007).

## Accessory table: columns

In export order (`ACCESSORY_COLUMNS`):

| # | Column | Export | Import rule (FR-024) |
|---|---|---|---|
| 1 | `record_id` | identifier | as "Record ID" below |
| 2 | `kind` | the kind's name, e.g. `Optic` | **required**: a kind name, matched ignoring letter case and surrounding whitespace, against every kind, including one no longer offered |
| 3 | `make` | text | **required** (FR-001); 004's entry rules; snapped |
| 4 | `model` | text | **required** (FR-001); 004's entry rules; snapped |
| 5 | `serial_number` | text | optional; trimmed |
| 6 | `caliber` | text | optional; 004's entry rules; snapped; derived from `cartridge` when blank, reported as for a firearm |
| 7 | `cartridge` | text | optional; 004's entry rules; snapped |
| 8 | `notes` | text | optional |
| 9 | `status` | `active` / `disposed` | as the firearm table |
| 10 | `estimated_value` | whole dollars | as the firearm table |
| 11 | `acquisition_source` | text | optional; trimmed (research.md §16) |
| 12 | `acquisition_date` | `YYYY-MM-DD` | as the firearm table |
| 13 | `acquisition_price` | whole dollars | as the firearm table |
| 14 | `disposition_type` | as the firearm table | as the firearm table |
| 15 | `disposition_recipient` | text | as the firearm table |
| 16 | `disposition_date` | `YYYY-MM-DD` | as the firearm table |
| 17 | `disposition_price` | whole dollars | optional, even when disposed (below) |
| 18 | `insurance_policy_name` | the policy's name | as the firearm table |
| 19 | `scheduled_coverage_amount` | whole dollars | as the firearm table |
| 20 | `mounted_on` | host identifier or blank | as "Mounted on" below |
| 21 | `photo_filenames` | `;`-separated | ignored |

## Record ID (`record_id`, FR-019, FR-022)

- **Export** writes the record's identifier. It is never blank.
- **Import**: a blank cell, or no column at all, means the row has none.
  A value is trimmed and lowercased, and must be a version 4 UUID. Any
  other value is a **row error**: "record_id: "{value}" is not a record
  ID".
- **Matching**: when the identifier belongs to an existing record of the
  row's own kind, active or disposed, that record is the row's match, and
  the conflict choice applies (skip, overwrite or duplicate; 001 FR-026).
  The identifier is matched ahead of the make, model and serial number key.
- **Otherwise**:
  - *firearm row*: 001's make, model and serial number key is compared, as
    before (001 FR-030, 002 FR-008).
  - *accessory row*: there is no other key, so the row is new.
- **Row errors**:
  - "record_id: {value} is also used by {Table}, row {n}" (one used by an
    earlier row of the same import, in either table; `{Table}` is
    "Firearms" or "Accessories", as in "also used by Accessories, row 3");
  - "record_id: {value} belongs to an accessory" (on a firearm row, and the
    reverse).
- **A record created from a row** keeps the row's identifier, or gets a new
  one when the row has none. A **duplicate** created by the conflict choice
  always gets a new one. **Overwrite** keeps the existing record's
  identifier (FR-019: editing never changes it).
- **Overwrite with a disposed row**: when the row marks an active record
  disposed, confirming the overwrite asks which of the records mounted on
  it are disposed of with it, as the dispose dialog does; they take the
  row's disposition type, recipient and date and no price (FR-014, issue
  #56).

## Mounted on (`mounted_on`, FR-023)

- **Export** writes the direct host's identifier, or blank.
- **Import**: resolved after every row of both tables is settled
  (research.md §18), for each row that created a record, and for each
  conflict resolved as overwrite or duplicate.
  - The host is looked up first among this import's own rows (the record
    each row became), then among the database's firearms and accessories,
    by identifier.
  - A mount that can't be made never fails the row. The record is saved
    unmounted, and a **warning** names the row and the reason (wording in
    contracts/tauri-commands.md). The reasons:
    - not a record ID;
    - no such record;
    - a disposed item or host;
    - it would be mounted on itself through others. In that case, of the
      rows forming the loop, the last one in file order (firearm table
      first) is the one left unmounted.
- **Overwrite** takes the row's mount, or unmounts the record when the cell
  is blank (Edge Cases). A sheet with no `mounted_on` column reads as blank
  in every row, like any missing column.
- **Skip** leaves the record and its mount untouched.

## Disposition price

A `disposed` row, in either table, may have a blank `disposition_price`.
That records no price. This changes 001's rule, which made it a row error
(research.md §9). The disposition type, recipient and date are still
required for a disposed row.

## The import report

- **Table names**: every row error, warning, snapped value, derived caliber
  and conflict names its table and its row within that table, e.g.
  "Accessories, row 4". When only one table is imported, the table name is
  still given.
- **Snapping**: text in either table is snapped to spellings already on
  record in firearms and accessories together, and to spellings used
  earlier in the same import (004 FR-026). Snapped values are listed.
- **Counts**: imported, updated and skipped rows are counted for both
  tables. The accessory count is also given on its own.

## Column layout check

`export_test.rs` holds the exact header row of each table, and
`human_seed_coverage_test.rs` checks that the import samples fill every
column of both tables.
