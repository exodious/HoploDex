# Contract: Export/Import Spreadsheet Format (delta)

This is a **delta** against
[the 001 spreadsheet contract](../../001-firearms-inventory/contracts/spreadsheet-format.md),
as amended by [002](../../002-firearm-identification/contracts/spreadsheet-format.md)
(FR-018 to FR-020 and FR-026 in 001; FR-022 to FR-026 here). One row is still
one firearm, and a file produced by export is always re-importable. Only the
columns and rules below change.

## New columns

Two columns, both optional, placed **directly after `caliber`** (FR-022):

| Column | Required on import? | Maps to | Notes |
|---|---|---|---|
| `cartridge` | no | `Firearm.cartridge` | free text, at most 100 characters, no control characters; blank = none. Snapped on import (below) |
| `action_type` | no | `ActionType.name` (by lookup) | one of the action names (data-model.md), matched ignoring letter case and surrounding whitespace; blank = none. Unknown, or not allowed for the row's `firearm_type`, is a row error (FR-024) |

The header row becomes:

```text
make, model, nickname, serial_number, no_serial_attested, caliber, cartridge,
action_type, firearm_type, notes, …, original_serial_number, photo_filenames
```

so `COLUMNS` in `services::spreadsheet` becomes 36 entries (34 + 2).

## Changed column: `caliber`

- **Required on import**: yes, **unless** `cartridge` is given and a caliber
  can be derived from it (FR-025). A blank `caliber` is filled from the
  catalog when the cartridge is a catalog name or alias, and otherwise by the
  caliber guess (research.md §7). Every filled row is listed in the import
  report with the caliber used and whether it came from the catalog or was
  guessed. When nothing can be derived, or both columns are blank, the row is
  a row error: "caliber: Caliber is required; it couldn't be worked out from
  the cartridge "Wildcat Special"." / "caliber: Caliber is required."
- A caliber given in the sheet is always used as given, after snapping.

## Columns are read by header (changes 001's reading)

Import now finds each column by its **header name** (trimmed, any letter
case), not by its position (research.md §12):

- A column in `COLUMNS` that the file does not have reads as blank on every
  row. A sheet exported before this feature, with no `cartridge` or
  `action_type` column, imports with no cartridge and no action (FR-023,
  US4-8), and its later columns still land in the right fields.
- Columns not in `COLUMNS` are ignored, as `photo_filenames` already is.
- The columns may be in any order.
- A header that names the same column twice is a file error: nothing is
  imported, and the message names the column.

## Entry rules and snapping on import (FR-015, FR-026)

For `make`, `model`, `cartridge` and `caliber`:

1. **Checked**: after trimming, at most 100 characters and no control
   characters; `make`, `model` and (after derivation) `caliber` non-blank. A
   violation is a row error naming the column.
2. **Snapped** (FR-013): a value that is a same-notation variant (differs only
   in letter case, spacing, the separators `-` `/` `.` `x` `×` `&`, or `&`
   versus `and`; research.md §3) of
   - a catalog spelling (cartridge names for `cartridge`, calibers for
     `caliber`), becomes that spelling; otherwise
   - a value **on record when the import started**, becomes the spelling most
     firearms use (the earliest recorded on a tie); otherwise
   - other cells of the **same sheet**, becomes the spelling most rows use
     (the earliest row on a tie).

   Different notations are never snapped ("9mm", "9x19", "9x19mm Parabellum";
   "S&W", "Smith & Wesson").
3. **Reported**: every changed value is listed in the import report (row,
   column, value in the sheet, value recorded), and the report counts them.

Matching of existing records (001 FR-026) uses the snapped `make` and
`model`.

## Export behavior (additions)

- `cartridge` is the recorded text; `action_type` is the action's name
  (`Bolt action`). Both blank when none.
- **Round trip (SC-006, as revised)**: exporting a collection with cartridges
  and actions and importing the file into an empty database reproduces every
  cartridge, caliber and action, except that same-notation variants of one
  value are merged on import. No exported caliber is blank, so none is
  derived, and a value that is the only spelling of its key is left as it is;
  a collection holding two spellings of one value ("Springfield Armory" on
  three firearms, "Springfield armory" on one; or "9X19mm Parabellum" beside
  the catalog's "9x19mm Parabellum", spec Edge Cases) is re-imported with them
  merged, by the sheet pass or the catalog, and each merge is listed in the
  report (SC-007). One test checks exactness on a collection without such
  pairs; a second checks the merge and its report (research.md §12).
