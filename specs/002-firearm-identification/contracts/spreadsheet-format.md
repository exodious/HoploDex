# Contract: Export/Import Spreadsheet Format (delta)

This is a **delta** against
[the 001 spreadsheet contract](../../001-firearms-inventory/contracts/spreadsheet-format.md)
(FR-018, FR-019, FR-020 there; FR-014 here). One row is still one firearm,
no photo or document data is in the spreadsheet, and a file produced by
export is always re-importable. Only the columns and rules below change.

## New columns

Seven columns, all optional (blank allowed), placed after `condition` and
before `photo_filenames`, in this order:

| Column | Required on import? | Maps to | Notes |
|---|---|---|---|
| `origin` | no | `Firearm.origin` | one of `Domestic`, `Imported`, `Re-imported`, matched ignoring letter case; blank = not specified. Any other value is a row error naming the value (FR-014, US4-3) |
| `year_of_manufacture` | no | `Firearm.year_of_manufacture` | a four-digit year `1400`..current local year, e.g. `1943`; see Year below |
| `country_of_manufacture` | no | `Firearm.country_of_manufacture` | free text; allowed only when `origin` is `Imported`. Always blank on export for a `Re-imported` row |
| `importer_name` | no | `Firearm.importer_name` | free text, a name only; allowed only when `origin` is `Imported` or `Re-imported` |
| `original_make` | no | `Firearm.original_make` | as `importer_name` |
| `original_model` | no | `Firearm.original_model` | as `importer_name` |
| `original_serial_number` | no | `Firearm.original_serial_number` | as `importer_name`; not the main `serial_number` |

The header row lists the columns in the order above, so `COLUMNS` in
`services::spreadsheet` becomes 34 entries (27 in 001 + 7).

## Origin values

Export writes the display label (`Domestic`, `Imported`, `Re-imported`), as
`condition` is written. Import accepts exactly those three spellings, in any
letter case, plus blank. Aliases such as `reimported` or `re_imported` are
**not** accepted: the spec lists three values, each with one spelling
(research.md §9).

## Year (FR-003)

Export writes plain digits with no decoration (`1943`). Import accepts a
whole number, a numeric cell with a whole value, or text with a zero
fraction (`1943.0`), and drops surrounding whitespace, by the same routine
`capacity` uses. Anything else — `circa 1943`, `43`, `19430`, `1943.5`, a
value earlier than 1400 or later than the current year — is a row error
naming `year_of_manufacture` (US4-7). Values are never rounded or
guessed at.

## Fields the origin does not offer (FR-014, US4-4)

A row is a row error, with a message naming the value and the origin that
does not allow it, when:

- `origin` is blank or `Domestic` and any of `importer_name`,
  `country_of_manufacture`, `original_make`, `original_model`,
  `original_serial_number` is non-blank; or
- `origin` is `Re-imported` and `country_of_manufacture` is non-blank.

`origin = Imported` (or `Re-imported`) with every importer, country and
original-marks column blank imports normally (spec Edge Cases).

## Export behavior (additions)

- All seven values are exported for every firearm, active or disposed.
  Blank stays blank; the United States is never written for a re-imported
  firearm because it is not a stored value.
- An export followed by an import into an empty collection reproduces every
  new field on every firearm exactly (SC-005).

## Import behavior (changes)

- **Matching key** (amends 001 FR-026, FR-030): `(make, model,
  serial_number)` when `no_serial_attested = FALSE`, ignoring case and
  surrounding whitespace, **unless both the row and the existing record have
  a `year_of_manufacture` and the years differ**, in which case the row is a
  new record and there is no conflict prompt (US4-5a). `original_*`,
  `importer_name`, `country_of_manufacture` and `origin` never take part in
  matching. Rows with `no_serial_attested = TRUE` are still always new
  records.
- **Identity block** (amends 001 FR-032 as applied to import): a row that
  would be blocked by FR-007/FR-008 is not offered the "create a duplicate"
  resolution. Only skip and overwrite are offered (US4-5).
- **Original-marks warning** (FR-009, US4-6): a row whose original maker,
  model and serial all match another active firearm still imports; the
  import report lists it under `warnings` with a message naming the existing
  record. The report shows warnings separately from row errors and from
  conflicts. Rows earlier in the same file count.
- **Row errors** are as in 001 (1-based row number, human-readable reason,
  other rows unaffected).
