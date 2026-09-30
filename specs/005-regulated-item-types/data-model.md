# Phase 1 Data Model: Regulated Item Types, Suppressors and NFA Registration

Derived from the Key Entities and functional requirements of
[spec.md](./spec.md). This document is a **delta** against
[the 001 data model](../001-firearms-inventory/data-model.md) as amended by
[002](../002-firearm-identification/data-model.md) and
[004](../004-cartridges-action-types/data-model.md). It lists what changes on
`FirearmType`, `ActionType` and `Firearm`, the one new lookup table, its
triggers, the FTS5 table, and the one list that is deliberately **not** in
the database (the built-in form names). Everything not mentioned is
unchanged.

The schema is edited in place in `0001_initial.sql`, `0002_fts5.sql` and
`0003_seed_firearm_types.sql`, and an existing development database must be
recreated (CLAUDE.md; spec Assumptions).

## Entity: Firearm Type (extended)

```sql
CREATE TABLE firearm_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    generic_thumbnail_key TEXT NOT NULL,
    -- FR-003: 0 = the field doesn't apply to this type: the form doesn't
    -- offer it and no firearm of the type may hold a value.
    action_type_applies INTEGER NOT NULL DEFAULT 1 CHECK (action_type_applies IN (0, 1)),
    barrel_length_applies INTEGER NOT NULL DEFAULT 1 CHECK (barrel_length_applies IN (0, 1)),
    capacity_applies INTEGER NOT NULL DEFAULT 1 CHECK (capacity_applies IN (0, 1))
);
```

**Seed** (now with fixed ids, research.md §4):

| id | name | generic_thumbnail_key | action / barrel / capacity apply |
|---:|---|---|---|
| 1 | Handgun | handgun | 1 / 1 / 1 |
| 2 | Rifle | rifle | 1 / 1 / 1 |
| 3 | Shotgun | shotgun | 1 / 1 / 1 |
| 4 | Other | other | 1 / 1 / 1 |
| 5 | **Suppressor** | **suppressor** | **0 / 0 / 0** |

### Rule: fields apply to the type (FR-003, FR-022, SC-005)

A firearm may hold an `action_type_id`, `barrel_length_hundredths` or
`capacity` only when its type's matching flag is 1. The rule is enforced by:

1. **The command layer** (`check_fields_apply`) on create, update and each
   import row, **before** `check_action_allowed`. It raises a
   `VALIDATION_ERROR` with one field error per offending field:
   `actionTypeId` "Action doesn't apply to a Suppressor.",
   `barrelLengthHundredths` "Barrel length doesn't apply to a Suppressor.",
   `capacity` "Capacity doesn't apply to a Suppressor." Import names the
   spreadsheet column instead (contracts/spreadsheet-format.md).
2. **A trigger pair**, the backstop:

```sql
CREATE TRIGGER firearms_fields_apply_insert BEFORE INSERT ON firearms
WHEN NEW.action_type_id IS NOT NULL OR NEW.barrel_length_hundredths IS NOT NULL OR NEW.capacity IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a field that does not apply to this firearm type has a value')
    FROM firearm_types t
    WHERE t.id = NEW.firearm_type_id
      AND ((NEW.action_type_id IS NOT NULL AND t.action_type_applies = 0)
        OR (NEW.barrel_length_hundredths IS NOT NULL AND t.barrel_length_applies = 0)
        OR (NEW.capacity IS NOT NULL AND t.capacity_applies = 0));
END;
-- firearms_fields_apply_update: the same, BEFORE UPDATE OF firearm_type_id,
-- action_type_id, barrel_length_hundredths, capacity.
```

`from_db` maps the raised ABORT to `INTERNAL_ERROR`: reaching it means a bug
bypassed the command layer. 004's `firearms_action_allowed_*` triggers are
unchanged. A Suppressor has no `firearm_type_actions` rows, which would allow
every action, and this trigger is what refuses one.

## Entity: Action Type (extended)

One row is added (FR-006, research.md §13). Ids are unchanged, and the sort
orders from Revolver on move down one:

| id | name | sort_order |
|---:|---|---:|
| 1 | Semi-automatic | 1 |
| **13** | **Automatic or select-fire** | **2** |
| 2 | Revolver | 3 |
| 3 … 12 | *(unchanged names)* | 4 … 13 |

`firearm_type_actions` gains `(1, 13)`, `(2, 13)` and `(3, 13)`. Other (4)
and Suppressor (5) have no rows. For Other that still means every action is
allowed. For Suppressor it doesn't matter, because the fields rule refuses
any action.

## Entity: Registration Classification (new lookup table)

```sql
CREATE TABLE registration_classes (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    sort_order INTEGER NOT NULL UNIQUE,
    -- FR-007: 0 = no longer offered for new choices. The row is never
    -- deleted or renamed while a record can hold it.
    offered INTEGER NOT NULL DEFAULT 1 CHECK (offered IN (0, 1))
);
```

A fixed list shipped with the application (FR-007). It is seeded with fixed
ids, never changed at run time, and no command writes it. It carries no rule
about what is regulated (spec Key Entities). `name` is what the user sees,
what export writes, what import matches (ignoring case and surrounding
whitespace, offered or not) and what search indexes. `sort_order` orders the
choice and the groups.

**Seed** (in order):

| id | name | offered |
|---:|---|---:|
| 1 | Suppressor | 1 |
| 2 | Short-barreled rifle | 1 |
| 3 | Short-barreled shotgun | 1 |
| 4 | Any other weapon | 1 |
| 5 | Machine gun | 1 |
| 6 | Destructive device | 1 |

Gets the three `registration_classes_marks_backup_due_after_*` triggers, like
the other lookups.

## Entity: Firearm (extended)

| Field | Type | Notes / validation |
|---|---|---|
| `registration_class_id` | INTEGER, **new**, nullable, `REFERENCES registration_classes (id)` | FR-007: what the item is registered as. `NULL` = none, always allowed. Any known classification is accepted, offered or not (research.md §5). Independent of the type and every other field (FR-008). Groupable; searched by its name |
| `registration_form` | TEXT, **new**, nullable | FR-009: free text, e.g. "Form 4". Blank → `NULL`, trimmed. At most 100 characters, no control characters (004 FR-015, on entry only). Suggestions from the built-in form names and the forms on record; snapped. Searchable |
| `registration_approved` | TEXT (`YYYY-MM-DD`), **new**, nullable | FR-009, FR-010: the date on the approved form. Not after the user's local today |
| `registered_to` | TEXT, **new**, nullable | FR-009: e.g. "Smith Family Trust". Rules as `registration_form`. Suggestions from the values on record only; snapped. Groupable, searchable |

**Table constraint** (FR-009: "a record MUST NOT hold any" details without a
classification):

```sql
CHECK (
    registration_class_id IS NOT NULL
    OR (registration_form IS NULL AND registration_approved IS NULL AND registered_to IS NULL)
)
```

Unchanged: `barrel_length_hundredths`, `capacity` and `action_type_id` keep
their columns and checks. The fields rule above adds the type condition.

### Indexes

- `idx_firearms_registered_to ON firearms (registered_to) WHERE registered_to
  IS NOT NULL`: **new**. It covers the vocabulary's `GROUP BY` for
  `suggest_entries` (research.md §7).
- `idx_firearms_registration_form ON firearms (registration_form) WHERE
  registration_form IS NOT NULL`: **new**, for the same reason.

No index on `registration_class_id`: grouping reads it in the one list query
already made, and six values need no lookup.

## Virtual table: firearms_fts (extended)

Three columns are added after `action_type_name`:

| FTS column | Indexed value |
|---|---|
| `registered_as` | `(SELECT name FROM registration_classes WHERE id = new.registration_class_id)` |
| `registration_form` | `new.registration_form` |
| `registered_to` | `new.registered_to` |

All three FTS triggers carry the three values, the same way they carry
`action_type_name`. Classification names are fixed at run time, so a delete
entry always matches what was indexed. `list_firearms`' short-query `LIKE`
branch gains `rc.name`, `f.registration_form` and `f.registered_to`. The
approved date is not indexed (FR-017 doesn't name it).

## Built-in Form Name (not in the database)

A read-only list in `src-tauri/src/services/registration.rs`, used only as
suggestions and snap targets for `registration_form`, like 004's cartridge
catalog (spec Key Entities):

| rank | name |
|---:|---|
| 1 | Form 4 |
| 2 | Form 1 |
| 3 | Form 3 |
| 4 | Form 5 |
| 5 | Form 10 |

Ranked by how often an owner records each (a transfer to an individual, then
making, then the dealer and tax-exempt transfers), so that a bare "F" or
"Form" suggests Form 4 first. Nothing from it is stored. Picking one copies
its text into `firearms.registration_form`.

## Derived values (not stored)

- **Caliber label**: "Caliber rating" when the type's name is Suppressor,
  otherwise "Caliber" (FR-002). The frontend decides this from the type list.
- **Registration summary line** on the closed form section and in the record
  page's Registration panel (contracts/ui-registration.md §2, §4).
- **Export disclosure flag**: whether any firearm in the export's scope has a
  `registration_class_id` (research.md §10).

## State on the firearm form (not stored, except in a pending draft)

`FormState` gains `registrationClassId`, `registrationForm`,
`registrationApproved` and `registeredTo`, and the last settled text of the
two suggestion fields. Barrel length, capacity and the action stay in the
state while the type hides them, and are left out of the input at save when
the type in effect omits them (research.md §2). `FORM_VERSION` becomes 3.

## Validation rules (additions)

| Rule | Where | Error (field) |
|---|---|---|
| Action, barrel length or capacity on a type that omits it (FR-003) | command, trigger, import | "Action doesn't apply to a Suppressor." (`actionTypeId`, `barrelLengthHundredths`, `capacity`) |
| Registration details with no classification (FR-009) | command, CHECK, import | "Choose what the firearm is registered as first." (each detail); import: "registered_as: Registration details need a classification." |
| `registrationClassId` names no classification | command, FK | "Choose a classification from the list." (`registrationClassId`) |
| `registrationApproved` in the future or not a date (FR-010) | command, form, import | "Approved date can't be in the future." / "Approved date must be a date in YYYY-MM-DD format." (`registrationApproved`) |
| `registrationForm`, `registeredTo` ≤ 100 characters, no control characters (004 FR-015) | command (create: all; update: changed only), form, import | "Form can be at most 100 characters.", "Registered to can't contain control characters." |
| Import: `registered_as` not a known classification (FR-021) | import | "registered_as: unknown classification "…"" |

## Relationship to 001, 002 and 004

Amends 001's `FirearmType` (a fifth seeded type and the three flags), its
Firearm table (four columns, one check, two indexes), its FTS table, its
`FirearmSummary` (`registeredAs`), and its seed file (type ids, the new
action and its mappings, the classifications). Amends 004's action list and
mapping and its FR-017 reading of "no rows" (a type can now omit the action
entirely). 002's identity triggers and queries are untouched (FR-005).
