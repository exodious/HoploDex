# Phase 1 Data Model: Firearm Identification & Markings

Derived from the Key Entities and functional requirements of
[spec.md](./spec.md). This document is a **delta** against
[the 001 data model](../001-firearms-inventory/data-model.md): it lists what
changes on the `Firearm` entity, its validation rules, its indexes and
triggers, and the FTS5 table, and names the 001 rule each change amends.
Everything not mentioned is unchanged. As in 001, all of this is real SQL in
the single encrypted SQLCipher database, and `cargo test` runs against a
real temporary database with this schema applied.

The schema is edited in place in migrations `0001_initial.sql` and
`0002_fts5.sql` (research.md §1); an existing development database must be
recreated.

## Entity: Firearm (extended)

Seven nullable columns are added. Feature 001's *Importer identification*
and *Original-maker marks* key entities (spec) are not separate tables: each
belongs to exactly one firearm, so each is a group of columns on it
(research.md §1).

| Field | Type | Notes / validation |
|---|---|---|
| `origin` | TEXT, nullable | FR-001: `domestic`, `imported`, `reimported` (shown "Domestic", "Imported", "Re-imported"); `CHECK (origin IS NULL OR origin IN ('domestic', 'imported', 'reimported'))`. `NULL` = not specified, always allowed. Groupable (FR-012), searched by its label |
| `year_of_manufacture` | INTEGER, nullable | FR-003: any firearm, any origin. `CHECK (year_of_manufacture IS NULL OR year_of_manufacture BETWEEN 1400 AND 9999)`; the real upper bound is the user's current local year, checked in the command layer. Searchable, not groupable. Joins the identity rule (FR-008) |
| `country_of_manufacture` | TEXT, nullable | FR-002: free text; only when `origin = 'imported'`. Never stored for `reimported` (displayed and searched as "United States") or for `domestic`/unspecified. Blank stored as `NULL` |
| `importer_name` | TEXT, nullable | FR-002: the importer's **name only**; no city, state or address is stored anywhere (spec Clarifications). Only when `origin` is import-marked. Blank stored as `NULL` |
| `original_make` | TEXT, nullable | FR-004: the original manufacturer's name. Only when import-marked; for `reimported` this is the U.S. maker. Trimmed; blank stored as `NULL` |
| `original_model` | TEXT, nullable | FR-004: as `original_make` |
| `original_serial_number` | TEXT, nullable | FR-004: as `original_make`. Distinct from the main `serial_number`; the "no serial number" attestation (001 FR-029) applies to the main serial only (FR-005) |

**Table-level CHECKs** (backstops for FR-002, FR-004, FR-014; the command
layer reports the same conditions as readable field errors first):

```sql
CHECK (
    (importer_name IS NULL AND original_make IS NULL
     AND original_model IS NULL AND original_serial_number IS NULL)
    OR origin IN ('imported', 'reimported')
),
CHECK (country_of_manufacture IS NULL OR origin = 'imported')
```

**Validation rules** (enforced in `models::firearm::validate_firearm_input`
and the command layer, not only in the database, so import produces per-row
messages, 001 FR-020):

- **Origin (FR-001)**: optional; a value outside the three is impossible
  through the typed API and is a row error on import (FR-014).
- **Fields the origin does not offer (FR-002, FR-004, FR-014)**: a non-blank
  `importer_name`, `original_*`, or, for a record whose origin is not
  `imported`, `country_of_manufacture` is a validation error with a
  `fieldErrors` entry naming the field and saying which origin allows it
  (for example "Country of manufacture applies only to imported firearms; a
  re-imported firearm is made in the United States."). The form clears such
  fields before sending (FR-010, research.md §8); the rule exists so no other
  caller can persist them.
- **Year of manufacture (FR-003)**: when present, a whole number with
  `1400 <= year <= current year`, judged against the user's local date.
  Message: "Year of manufacture must be a four-digit year from 1400 to
  {current year}." Checked on create, update, dispose, restore, and every
  import row.
- **Partial original marks (FR-004)**: any subset is accepted as entered.
- **No judgment (FR-006)**: nothing above, or anywhere else, tests a mark
  against the year, an import date, or a regulation.
- **Normalization**: `FirearmInput::normalized()` trims `importer_name`,
  `country_of_manufacture`, `original_make`, `original_model` and
  `original_serial_number`, and stores blank as `NULL`, as it already does
  for `nickname`, `serial_number` and `finish`.
- **Identity uniqueness (FR-007, FR-008)** — *amends 001 FR-032*. Among other
  `status = 'active'` firearms with a non-null `serial_number`, two records
  **conflict** when their `(make, model, serial_number)` match ignoring case
  and surrounding whitespace **unless** both `year_of_manufacture` values are
  non-null and different:

  ```text
  conflict(a, b) = same_main_marks(a, b)
                   AND NOT (a.year IS NOT NULL AND b.year IS NOT NULL AND a.year <> b.year)
  ```

  A conflict blocks the save with `VALIDATION_ERROR` on `serialNumber`, the
  message naming the other record and saying that a year of manufacture on
  each record is what would distinguish them. Unchanged from 001: disposed
  rows are never compared; a record with no serial number is never compared;
  origin is irrelevant. Applied on create, edit, import (as a row error) and
  reversing a disposition.
- **Original-marks warning (FR-009)** — *new, non-blocking*. When the saved
  record is `active` and has `original_make`, `original_model` and
  `original_serial_number` all non-null, and another **active** firearm has
  the same three (ignoring case), the save is stopped with the domain error
  `ORIGINAL_MARKS_MATCH` naming that firearm, until the caller resends with
  `confirmedWarnings: true`. A partial set never triggers it; disposed
  firearms are never considered. Applied on create, edit, and reversing a
  disposition; on import it is reported as a warning and never fails the row.
  It never blocks, is not part of the identifying key, and never affects
  import matching (FR-005, research.md §5).

**Identifying key (FR-005)** — *restates 001 FR-030*: `(make, model,
serial_number)`, plus `year_of_manufacture` only through the conflict rule
above. `original_*`, `importer_name`, `country_of_manufacture` and `origin`
take no part. The "no serial number" attestation and its `CHECK ((serial_number
IS NOT NULL) <> (no_serial_attested = 1))` are unchanged.

**State transitions**: unchanged (`active ↔ disposed`, 001 FR-033). Disposal
and restoration never change identification data (spec Edge Cases). Restoring
re-runs the identity rule and the original-marks warning against the firearms
active at that moment, inside the same transaction; either aborts the whole
reversal with nothing changed (the warning until it is confirmed).

**Deleting** a firearm removes all seven values with the row, through the
same `DELETE` + `secure_delete` + `VACUUM` path as every other column
(Constitution V). The FTS triggers remove the indexed copies.

### Origin transitions on edit (FR-010)

The form, not the backend, applies these when the origin control changes
(backend rules above then hold for the saved result):

| From → To | Carries over | Discarded (needs confirmation if non-blank) |
|---|---|---|
| none / Domestic → Imported or Re-imported | — (no import-only values can exist) | nothing |
| Imported ↔ Re-imported | importer name, original marks | country, only Imported → Re-imported |
| Imported / Re-imported → Domestic or none | nothing | importer name, country, original make, model, serial |
| Domestic ↔ none | — | nothing |

Year of manufacture is independent of origin and is never discarded.

## Indexes and triggers (amends 001 "Identity uniqueness" backstop)

| Object | Change |
|---|---|
| `idx_firearms_active_identity` | Was `CREATE UNIQUE INDEX` on `(make COLLATE NOCASE, model COLLATE NOCASE, serial_number COLLATE NOCASE) WHERE status = 'active' AND serial_number IS NOT NULL`. Now the same index **without `UNIQUE`**: a plain lookup index serving the trigger below. A unique index cannot express the year exception (research.md §3) |
| `firearms_active_identity_insert` / `_update` (new triggers) | `BEFORE INSERT` / `BEFORE UPDATE ON firearms`, `WHEN NEW.status = 'active' AND NEW.serial_number IS NOT NULL`: `RAISE(ABORT, 'active firearm with the same make, model and serial number exists')` when another active row (`id <> NEW.id`) has equal make, model and serial (`COLLATE NOCASE`) **and** the conflict predicate above holds. This is the exact backstop for FR-007/FR-008 and is expected never to fire in normal use because the command layer checks first |
| `idx_firearms_original_serial` (new) | `ON firearms (original_serial_number COLLATE NOCASE) WHERE status = 'active' AND original_serial_number IS NOT NULL`, serving the FR-009 lookup (research.md §5). Non-unique: the warning never blocks |
| `idx_firearms_active_nickname` | Unchanged |

## Virtual table: `firearms_fts` (extended) — *amends 001 FR-013*

Seven columns are appended to the external-content FTS5 table, and the
`AFTER INSERT`, `AFTER UPDATE` and `AFTER DELETE` triggers write them. Values
are written **as displayed** (FR-012):

| FTS column | Value written |
|---|---|
| `origin` | `CASE origin WHEN 'domestic' THEN 'Domestic' WHEN 'imported' THEN 'Imported' WHEN 'reimported' THEN 'Re-imported' END` (`NULL` for no origin) |
| `year_of_manufacture` | the year as text |
| `country_of_manufacture` | `'United States'` when `origin = 'reimported'`, otherwise the stored country |
| `importer_name`, `original_make`, `original_model`, `original_serial_number` | the stored value |

Because the index is trigram-tokenized (issue #46), the search box's
quoted-phrase query matches any run of text inside a value, which gives exactly
the spec's behavior with no special handling: "imported" finds imported and
re-imported, "re-imported" finds only re-imported, "domestic" finds only
domestic, and a firearm with no origin has nothing to match on origin
(research.md §6).

## Entity relationships (summary)

Unchanged from 001. The new values are columns of `Firearm`; no relationship
is added.

```text
FirearmType (1) ──< (many) Firearm
Firearm (1) ──< (many) Photo / DocumentAttachment / DispositionHistory
Firearm ──< firearms_fts (FTS5 shadow index; now also origin, year, country,
                          importer, original make/model/serial)
```

## Derived display values (not stored)

| Value | Rule |
|---|---|
| Country of manufacture shown | Stored country for `imported`; "United States" for `reimported`; not shown otherwise |
| Origin shown | The label; "Not specified" when `NULL` |
| Original maker's marks block | Shown on the detail view only when at least one of the three original values is recorded (US2-2); always labeled "Original maker's marks", separate from the main make/model/serial (FR-013) |

## Import row shape (not persisted)

Seven columns are added to the row shape of 001 (see
[contracts/spreadsheet-format.md](./contracts/spreadsheet-format.md)).
Import matching keys on `(make, model, serial_number)` when
`no_serial_attested = FALSE`, treating two rows as the same firearm unless
both have a year of manufacture and the years differ (FR-014, research.md
§4); `original_*` never takes part. Rows with `no_serial_attested = TRUE` are
still always new records.
