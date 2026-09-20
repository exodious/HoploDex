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
| `serial_number` | no | `Firearm.serial_number` | blank allowed only with `no_serial_attested = TRUE` |
| `no_serial_attested` | no (default FALSE) | `Firearm.no_serial_attested` | `TRUE`/`FALSE` |
| `caliber` | yes | `Firearm.caliber` | |
| `firearm_type` | yes | `FirearmType.name` | matched case-insensitively; unknown type → row error |
| `notes` | no | `Firearm.notes` | free text |
| `accessories` | no | `Firearm.accessories` | semicolon-delimited list |
| `status` | no (default `active`) | `Firearm.status` | `active` / `disposed` |
| `estimated_value` | no | `Firearm.estimated_value` | decimal currency, e.g. `450.00` |
| `acquisition_source` | no | `Firearm.acquisition_source` | |
| `acquisition_date` | no | `Firearm.acquisition_date` | `YYYY-MM-DD` |
| `acquisition_price` | no | `Firearm.acquisition_price` | decimal currency |
| `disposition_type` | required if `status=disposed` | `Firearm.disposition_type` | `sold`/`traded`/`gifted`/`destroyed`/`lost_stolen` |
| `disposition_recipient` | required if `status=disposed` | `Firearm.disposition_recipient` | |
| `disposition_date` | required if `status=disposed` | `Firearm.disposition_date` | `YYYY-MM-DD` |
| `disposition_price` | no | `Firearm.disposition_price` | decimal currency |
| `insurance_policy_name` | no | `InsurancePolicy.name` (by lookup) | unknown name on import → row error |
| `coverage_kind` | required if `insurance_policy_name` set | `Firearm.coverage_kind` | `individually_scheduled`/`blanket` |
| `scheduled_coverage_amount` | required if `coverage_kind=individually_scheduled` | `Firearm.scheduled_coverage_amount` | decimal currency |
| `photo_filenames` | no (export only; ignored on import per FR-019) | — | semicolon-delimited filenames in the sibling photos folder |

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
  (non-exempt serial matching an *active* record) is not offered the
  "duplicate" resolution; a row for a serial-exempt firearm that matches
  produces a warning in the report but still imports. A disposed-only
  match is not a conflict for blocking purposes.
