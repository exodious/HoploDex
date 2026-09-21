# Contract: Export/Import Spreadsheet Format

Defines the CSV/XLSX column contract shared by `export_collection` and
`import_collection` (see [tauri-commands.md](./tauri-commands.md)), so a
file produced by export is always re-importable, and a user-authored file
following this shape imports cleanly (FR-018, FR-019, FR-020).

One row = one Firearm. No photo or document data is included in the
spreadsheet (FR-019); photos are exported as separate files alongside the
spreadsheet, matched by a `photo_filenames` column referencing files in the
sibling photos folder.

## Columns

| Column | Required on import? | Maps to | Notes |
|---|---|---|---|
| `make` | yes | `Firearm.make` | |
| `model` | yes | `Firearm.model` | |
| `nickname` | no | `Firearm.nickname` | free text; blank → none; never used for matching; must be unique among active firearms, otherwise row error (FR-031) |
| `serial_number` | no | `Firearm.serial_number` | blank only with `no_serial_attested = TRUE`, and must be blank with it (FR-029): a row with both is a row error |
| `no_serial_attested` | no (default FALSE) | `Firearm.no_serial_attested` | `TRUE`/`FALSE` |
| `caliber` | yes | `Firearm.caliber` | |
| `firearm_type` | yes | `FirearmType.name` | matched case-insensitively; unknown type → row error |
| `notes` | no | `Firearm.notes` | free text |
| `accessories` | no | `Firearm.accessories` | semicolon-delimited list |
| `status` | no (default `active`) | `Firearm.status` | `active` / `disposed` |
| `estimated_value` | no | `Firearm.estimated_value` | whole dollars, e.g. `450` (see Amounts below) |
| `acquisition_source` | no | `Firearm.acquisition_source` | |
| `acquisition_date` | no | `Firearm.acquisition_date` | `YYYY-MM-DD`; a future date is a row error (FR-003) |
| `acquisition_price` | no | `Firearm.acquisition_price` | whole dollars |
| `disposition_type` | required if `status=disposed` | `Firearm.disposition_type` | `sold`/`traded`/`gifted`/`destroyed`/`lost_stolen` |
| `disposition_recipient` | required if `status=disposed` | `Firearm.disposition_recipient` | |
| `disposition_date` | required if `status=disposed` | `Firearm.disposition_date` | `YYYY-MM-DD`; a future date, or a date before `acquisition_date`, is a row error (FR-004) |
| `disposition_price` | no | `Firearm.disposition_price` | whole dollars |
| `insurance_policy_name` | no | `InsurancePolicy.name` (by lookup) | schedules the firearm under that policy; blank ⇒ unscheduled (covered by the blanket policy in force); unknown name on import → row error |
| `scheduled_coverage_amount` | required if `insurance_policy_name` set; must be blank otherwise | `Firearm.scheduled_coverage_amount` | whole dollars |
| `barrel_length_in` | no | `Firearm.barrel_length_hundredths` | decimal inches, up to two decimal places, `> 0` (FR-039), e.g. `16.25`; see Physical details below |
| `overall_length_in` | no | `Firearm.overall_length_hundredths` | as `barrel_length_in` |
| `weight_oz` | no | `Firearm.weight_tenths_oz` | decimal ounces, up to one decimal place, `> 0`, e.g. `40.5` |
| `capacity` | no | `Firearm.capacity` | whole number `>= 1` |
| `finish` | no | `Firearm.finish` | free text |
| `condition` | no | `Firearm.condition` | one of `New in box`, `Like new`, `Excellent`, `Good`, `Fair`, `Poor`; matched case-insensitively, and `new_in_box` style values are accepted; unknown value → row error (as `firearm_type`) |
| `photo_filenames` | no (export only; ignored on import per FR-019) | — | semicolon-delimited filenames in the sibling photos folder |

## Amounts (FR-037)

`estimated_value`, `acquisition_price`, `disposition_price` and
`scheduled_coverage_amount` are whole U.S. dollars. Export writes plain
digits (`1250`, no `$`, no thousands separator, no decimals). Import accepts
a whole number, a numeric cell with a whole value, or text with a zero
fraction (`450.00`), and drops a leading `$`, thousands commas, and
surrounding whitespace; a value with non-zero cents (`450.50`), a negative
value, or any other text is a row error naming the column (FR-020). Values
are never rounded.

## Physical details (FR-039)

The six physical-detail columns follow the same rules as Amounts: blank is
allowed; export writes plain numbers with no unit text and no trailing zeros
(`16.25`, `18`, `40.5`); import accepts a number with at most the allowed
decimal places (a zero fraction beyond that, e.g. `16.250`, is accepted) and a
numeric cell; a value with more places (`16.255`), a value `<= 0`, a
non-numeric value, a fractional or `< 1` capacity, or an unknown `condition`
is a row error naming the column (FR-020). Values are never rounded. The
columns sit after `scheduled_coverage_amount` and before `photo_filenames`.

## Export behavior

- Retained `DispositionHistory` rows (FR-033) are not represented in the
  spreadsheet (one row = one firearm, current state only); only the
  firearm's current disposition columns are exported.

- One spreadsheet file (`.csv` or `.xlsx`, per user's chosen format) plus
  one sibling folder (`<export-name>_photos/`) containing every stored
  photo in its original format and filename, deduplicated per firearm.
- `scope` (all vs. currently-filtered) is resolved by the frontend passing
  the active filter explicitly to `export_collection` — see
  tauri-commands.md.

## Import behavior

- Each row is validated independently; a failing row is recorded in
  `rowErrors` with a 1-based row number and human-readable reason, and
  does **not** block other rows from importing (FR-020).
- Matching key for update-vs-create: `(make, model, serial_number)` when
  `no_serial_attested = FALSE`. Rows with `no_serial_attested = TRUE` are
  always inserted as new records (FR-030) — they can never match an
  existing row, even if `make`/`model` coincide.
- A row whose key matches an existing record produces an `ImportConflict`
  (see `resolve_import_conflicts` in tauri-commands.md) rather than a
  silent overwrite.
- Every row is also checked against FR-032: a row that would be blocked
  (serial matching an *active* record) is not offered the
  "duplicate" resolution. A disposed-only match is not a conflict for
  blocking purposes.
