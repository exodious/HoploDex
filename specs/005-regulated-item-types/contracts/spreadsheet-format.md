# Contract: Export/Import Spreadsheet Format (delta)

This is a **delta** against
[the 001 spreadsheet contract](../../001-firearms-inventory/contracts/spreadsheet-format.md),
as amended by [002](../../002-firearm-identification/contracts/spreadsheet-format.md)
and [004](../../004-cartridges-action-types/contracts/spreadsheet-format.md).
One row is still one firearm. A file produced by export is always
re-importable, and import still reads columns by header. Only the columns and
rules below change.

## New columns

Four columns, all optional, placed **after `original_serial_number` and
before `photo_filenames`** (FR-019):

| Column | Required on import? | Maps to | Notes |
|---|---|---|---|
| `registered_as` | no | `RegistrationClass.name` (by lookup) | One of the classification names, matched ignoring letter case and surrounding whitespace, **including those no longer offered**. Blank = none |
| `registration_form` | no | `Firearm.registration_form` | Free text, ≤ 100 characters, no control characters. Snapped (below) |
| `registration_approved` | no | `Firearm.registration_approved` | A date, read as `acquisition_date` is. Not in the future |
| `registered_to` | no | `Firearm.registered_to` | Free text, as `registration_form`. Snapped (below) |

The header row ends:

```text
…, original_make, original_model, original_serial_number,
registered_as, registration_form, registration_approved, registered_to,
photo_filenames
```

so `COLUMNS` in `services::spreadsheet` becomes 40 entries (36 + 4).

## Changed column: `firearm_type`

- May now be `Suppressor` (matched ignoring case, as before).

## Changed column: `action_type`

- May now be `Automatic or select-fire`, allowed for Handgun, Rifle, Shotgun
  and Other.

## Row errors (additions, FR-021, FR-022)

Each is a row error. Other rows still import (001 FR-020).

| Case | Message |
|---|---|
| `registered_as` not a known classification | `registered_as: unknown classification "Short barrel rifle"` |
| `registration_form`, `registration_approved` or `registered_to` set and `registered_as` blank | `registered_as: Registration details need a classification.` |
| `registration_approved` in the future | `registration_approved: Approved date can't be in the future.` |
| `registration_approved` not a date | `registration_approved: Approved date must be a date in YYYY-MM-DD format.` |
| `registration_form` / `registered_to` over 100 characters or with control characters | `registered_to: Registered to can be at most 100 characters.` |
| `firearm_type` Suppressor with an `action_type` | `action_type: Action doesn't apply to a Suppressor.` |
| … with `barrel_length_in` | `barrel_length_in: Barrel length doesn't apply to a Suppressor.` |
| … with `capacity` | `capacity: Capacity doesn't apply to a Suppressor.` |

A row with several of these reports them all, joined by "; " as before. The
fields-apply errors are checked before 004's action mapping, so a Suppressor
row with an action is never reported as "allowed".

## Snapping on import (FR-021)

`registration_form` and `registered_to` follow 004's three steps (the
spreadsheet contract's "Entry rules and snapping on import"), with these
sources:

- `registration_form`: the built-in form names, then the forms on record when
  the import started, then the sheet's own majority spelling.
- `registered_to`: the values on record when the import started, then the
  sheet's majority spelling. It has no built-in list.

Every changed value is listed in the import report's `snappedValues` with
field `registrationForm` or `registeredTo`. The report shows them as "Form"
and "Registered to".

## Export behavior (additions)

- `registered_as` is the classification's name, and
  `registration_approved` is `YYYY-MM-DD`. All four are blank when there is
  no value, including all four on a firearm with no classification.
- `firearm_type` is `Suppressor` for a suppressor. Its `action_type`,
  `barrel_length_in` and `capacity` are always blank.
- **Disclosure** (FR-020): when any firearm in the export's scope has a
  classification, the export dialog names registration details among what
  leaves the database unencrypted (contracts/ui-registration.md §5).

## A sheet without the new columns (US4-7)

A sheet exported before this feature has none of the four columns. They read
as blank on every row, and the rows import with no classification. This is
004's header-based reading, and needs no new code.

## Round trip (SC-003)

Exporting a collection with suppressors, every classification (offered or
not) and every registration field, and importing the file into an empty
database, reproduces every type, classification and registration detail
exactly. Same-notation variants of a form or a "Registered to" name are
merged on import and reported, as 004's revised SC-006 describes for make and
model. The round-trip test uses a collection without such pairs, and a second
test checks the merge and its report.
