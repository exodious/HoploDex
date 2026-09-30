# Phase 1 Data Model: Cartridges, Action Types & Entry Suggestions

Derived from the Key Entities and functional requirements of
[spec.md](./spec.md). This document is a **delta** against
[the 001 data model](../001-firearms-inventory/data-model.md) and
[002's](../002-firearm-identification/data-model.md): it lists what changes on
the `Firearm` entity, the two new lookup tables, their triggers, the FTS5
table, and the one entity that is deliberately **not** in the database (the
cartridge catalog). Everything not mentioned is unchanged.

The schema is edited in place in `0001_initial.sql`, `0002_fts5.sql` and
`0003_seed_firearm_types.sql` (research.md §10); an existing development
database must be recreated.

## Entity: Firearm (extended)

| Field | Type | Notes / validation |
|---|---|---|
| `make` | TEXT NOT NULL (unchanged) | FR-015: now **trimmed** on save; 1–100 characters, no control characters (research.md §9). Suggestions and snapping (FR-009, FR-013) |
| `model` | TEXT NOT NULL (unchanged) | FR-015: as `make` |
| `caliber` | TEXT NOT NULL (unchanged) | FR-001: stays required. FR-015: as `make`. May be derived from the cartridge on the form or on import (FR-003, FR-005, FR-025); always stored as text on the record, never looked up (FR-004) |
| `cartridge` | TEXT, **new**, nullable | FR-001: free text, e.g. "9x19mm Parabellum". Blank → `NULL`. When present: at most 100 characters, no control characters. No reference to the catalog (FR-004). Searchable (FTS), groupable (FR-008) |
| `action_type_id` | INTEGER, **new**, nullable, `REFERENCES action_types (id)` | FR-017: how the firearm operates. `NULL` = not specified; always allowed (FR-021). Must be allowed for `firearm_type_id` (see *Rule: action allowed for type*). Groupable, searched by the action's name (FR-020) |

The length cap applies to a value as it is entered: `create_firearm` checks
all four text fields, `update_firearm` only those whose trimmed value differs
from the stored one, and import every cell (research.md §9). No SQL `CHECK`
carries the cap, because an existing record over it stays valid (spec
Assumptions).

### Indexes

- `idx_firearms_cartridge ON firearms (cartridge)`: **new**, beside the
  existing `idx_firearms_caliber` and `idx_firearms_make`; covers
  `suggest_entries`' `GROUP BY cartridge` (research.md §5).
- No index on `(make, model)` unless `performance_test.rs` shows the model
  query needs one.

## Entity: Action Type (new lookup table)

```sql
CREATE TABLE action_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    sort_order INTEGER NOT NULL UNIQUE
);
```

A fixed list shipped with the application (FR-017): seeded with fixed ids,
never changed at run time, no command writes it. `name` is what the user sees,
what export writes, what import matches (ignoring case and surrounding
whitespace), and what search indexes. `sort_order` orders the choice and the
groups.

**Seed** (FR-018, in order):

| id | name |
|---:|---|
| 1 | Semi-automatic |
| 2 | Revolver |
| 3 | Bolt action |
| 4 | Lever action |
| 5 | Pump action |
| 6 | Break action |
| 7 | Falling block |
| 8 | Rolling block |
| 9 | Single shot (other) |
| 10 | Flintlock |
| 11 | Percussion |
| 12 | Inline muzzleloader |

## Entity: Firearm Type ↔ Action Type mapping (new join table)

```sql
CREATE TABLE firearm_type_actions (
    firearm_type_id INTEGER NOT NULL REFERENCES firearm_types (id) ON DELETE CASCADE,
    action_type_id INTEGER NOT NULL REFERENCES action_types (id),
    PRIMARY KEY (firearm_type_id, action_type_id)
) WITHOUT ROWID;
```

Seeded from FR-018's table for Handgun (1), Rifle (2) and Shotgun (3):

- **Handgun**: every action except Pump action, Falling block and Inline
  muzzleloader (9 rows).
- **Rifle**: all 12.
- **Shotgun**: every action except Rolling block (11 rows).
- **Other** (4), and any type added later: **no rows**, which means every
  action is allowed (FR-017).

### Rule: action allowed for type (FR-017, FR-019, SC-008)

A firearm's `action_type_id` is allowed when it is `NULL`, or its
`firearm_type_id` has no rows in `firearm_type_actions`, or it has a row for
that action. Enforced by:

1. **The command layer** (`check_action_allowed`), on create, update and each
   import row: a `VALIDATION_ERROR` with a field error on `actionTypeId`,
   "Pump action doesn't apply to a Handgun." Import reports it as a row
   error on `action_type`.
2. **A trigger pair** on `firearms`, the backstop (research.md §10):

```sql
CREATE TRIGGER firearms_action_allowed_insert BEFORE INSERT ON firearms
WHEN NEW.action_type_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'action type not allowed for this firearm type')
    WHERE EXISTS (SELECT 1 FROM firearm_type_actions WHERE firearm_type_id = NEW.firearm_type_id)
      AND NOT EXISTS (
          SELECT 1 FROM firearm_type_actions
          WHERE firearm_type_id = NEW.firearm_type_id AND action_type_id = NEW.action_type_id
      );
END;
-- firearms_action_allowed_update: the same, BEFORE UPDATE OF action_type_id, firearm_type_id.
```

`from_db` maps the raised ABORT to `INTERNAL_ERROR`: reaching it means a bug
bypassed the command layer.

### Backup tracking

`action_types` and `firearm_type_actions` get the three
`*_marks_backup_due_after_*` triggers, like `firearm_types`
(`backup_due_tracking_test` requires them; they never fire in practice).

## Virtual table: firearms_fts (extended)

Two columns are added after `original_serial_number`:

| FTS column | Indexed value |
|---|---|
| `cartridge` | `new.cartridge` |
| `action_type_name` | `(SELECT name FROM action_types WHERE id = new.action_type_id)` |

All three FTS triggers (`after_insert`, `after_delete`, `after_update`) carry
the two values, the same way they carry `firearm_type_name`. Action names are
fixed at run time, so a delete entry always matches what was indexed.

## Catalog Cartridge (not in the database)

A read-only list compiled into the application (FR-002, research.md §1),
parsed from `src-tauri/src/services/cartridges/catalog.tsv`:

| Field | Meaning |
|---|---|
| `rank` | Commonness, 1 = most common; orders catalog-only suggestions (FR-012) |
| `name` | The spelling the catalog offers and snaps to, e.g. "9x19mm Parabellum" |
| `caliber` | The bore class (FR-004a, research.md §2), e.g. "9mm" |
| `aliases` | Other names that narrow the list to this entry and derive its caliber (never snap targets), e.g. "9mm Luger", "9x19" |

The **catalog calibers** are the distinct `caliber` values; each ranks by its
best-ranked cartridge. Invariants are listed in research.md §2 and checked by
a unit test over the parsed file.

Nothing from the catalog is ever stored: picking an entry copies its name
into `firearms.cartridge` and its caliber into `firearms.caliber` (FR-003),
and from then on the record is self-contained (FR-004).

## Suggestion (computed, never stored)

The value `suggest_entries` returns (contracts/tauri-commands.md). Computed
per request from the `firearms` rows present at that moment (active and
disposed) and the catalog, deduplicated by entry key (research.md §3–§4):

| Field | Meaning |
|---|---|
| `value` | The display spelling: the catalog's when it has the key, otherwise the most-used record spelling (earliest recorded on a tie) |
| `inCatalog` | Whether the catalog has it (FR-016) |
| `useCount` | Firearms on record using any spelling with this key; `0` = catalog only |
| `caliber` | For a catalog cartridge only: its class, shown as a hint in the list |

## Derived values (not stored)

- **Entry key** (research.md §3): the comparison form behind deduplication,
  snapping and matching.
- **Derived caliber** (research.md §7): `(caliber, catalog | guess)` or none,
  for a cartridge.
- **Browse caliber text** (FR-027): `"{cartridge} ({caliber})"` when a
  cartridge is recorded, else `caliber`.

## State on the firearm form (not stored, except in a pending draft)

research.md §8. Beside the four text fields, the form keeps `caliberMode`
(`derived` | `edited`), `caliberSource` (`catalog` | `guess` | none),
`caliberSuggestion`, the last settled text of each suggestion field, and the
chosen `actionTypeId`. These are part of a pending-changes draft's
`values_json`, so `FORM_VERSION` becomes 2.

## Validation rules (additions)

| Rule | Where | Error |
|---|---|---|
| Make, model, caliber non-empty after trim (001, unchanged) | command, form | "Make is required." etc. |
| Make, model, cartridge, caliber ≤ 100 characters after trim (FR-015) | command (create: all; update: changed fields; import: all), form | "Make can be at most 100 characters." |
| No control characters in those four (FR-015) | same | "Make can't contain control characters." |
| `actionTypeId` names an action (FK) and is allowed for the type (FR-017) | command, trigger | "Pump action doesn't apply to a Handgun." |
| Import: `caliber` blank and no caliber can be derived (FR-025) | import | "caliber: Caliber is required; it couldn't be worked out from the cartridge "…"." or "caliber: Caliber is required." |
| Import: `action_type` unknown (FR-024) | import | "action_type: unknown action type "…"" |
| Import: a header names a column twice (research.md §12) | import (file) | `VALIDATION_ERROR`, "The import file has two "caliber" columns." |

## Relationship to 001 and 002

Amends 001's Firearm table (two columns; make/model/caliber trimming and
cap), its FTS table, its `FirearmSummary` (cartridge, action type name), and
its seed file (action types and mapping). 002's identity trigger and query
are untouched; they now see trimmed make and model (research.md §9).
