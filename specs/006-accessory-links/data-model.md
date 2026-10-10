# Phase 1 Data Model: Accessory Records and Mounting

Derived from the Key Entities and functional requirements of
[spec.md](./spec.md). This document is a **delta** against
[the 001 data model](../001-firearms-inventory/data-model.md), as amended by
[002](../002-firearm-identification/data-model.md),
[003](../003-database-protection-management/data-model.md),
[004](../004-cartridges-action-types/data-model.md) and
[005](../005-regulated-item-types/data-model.md). Everything not mentioned is
unchanged. The reasons behind each choice are in [research.md](./research.md).

## The schema decision (issue #50's 0.1.0 plan)

**This feature changes existing tables, so it is built in 0.1.0.** It
changes `firearms`, `photos`, `document_attachments`, `disposition_history`
and `pending_changes`, and adds `accessory_kinds`, `accessories`, `mounts`
and `accessories_fts` (research.md §1). The free-text
`firearms.accessories` column is kept unchanged (FR-007).

The schema is edited in place in `0001_initial.sql`, `0002_fts5.sql` and
`0003_seed_firearm_types.sql`. An existing development database must be
recreated (CLAUDE.md; spec Assumptions). Nothing in this feature opens or
converts the developer's real databases.

## Shared shape: Record reference

Not a table. In Rust it is `enum RecordRef { Firearm(i64), Accessory(i64) }`,
and over IPC `{ kind: "firearm" | "accessory", id }` (research.md §7). Every
mount, owner and value-summary entry that can be either kind of record uses
it.

## Entity: Record Identifier (on Firearm and Accessory)

| Column | Type | Rules |
|---|---|---|
| `uid` | `TEXT NOT NULL UNIQUE` | A random version 4 UUID, lowercase and hyphenated. Set by `ops::create_*`, never updated. |

```sql
-- On both firearms and accessories:
uid TEXT NOT NULL UNIQUE CHECK (
    length(uid) = 36
    AND uid GLOB '????????-????-4???-????-????????????'
    AND substr(uid, 20, 1) IN ('8', '9', 'a', 'b')
    AND NOT (replace(uid, '-', '') GLOB '*[^0-9a-f]*')
),
```

**Backstops** (each expected never to fire; `from_db` maps them to
`INTERNAL_ERROR`):

```sql
-- FR-019: never changed.
CREATE TRIGGER firearms_uid_fixed BEFORE UPDATE OF uid ON firearms
WHEN NEW.uid IS NOT OLD.uid
BEGIN SELECT RAISE(ABORT, 'a record identifier never changes'); END;
-- (and accessories_uid_fixed)

-- FR-022: one identifier names one record across both tables.
CREATE TRIGGER accessories_uid_distinct BEFORE INSERT ON accessories
BEGIN
    SELECT RAISE(ABORT, 'record identifier already used by a firearm')
    WHERE EXISTS (SELECT 1 FROM firearms WHERE uid = NEW.uid);
END;
-- (and firearms_uid_distinct, against accessories)
```

**Validation**: `services::record_id::parse` accepts a spreadsheet cell
after trimming, in any letter case, and returns it lowercased. Anything
else is "not a record ID". The identifier is in no IPC output: the structs
mark it `#[serde(skip)]` (FR-019).

## Entity: Accessory Kind (new lookup)

```sql
CREATE TABLE accessory_kinds (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    generic_thumbnail_key TEXT NOT NULL,
    sort_order INTEGER NOT NULL UNIQUE,
    -- FR-002: 0 = no longer offered for new choices. The row is never
    -- deleted or renamed while a record can hold it.
    offered INTEGER NOT NULL DEFAULT 1 CHECK (offered IN (0, 1))
);
```

The seed (`0003_seed_firearm_types.sql`) uses fixed ids, so an id means the
same kind in every build. Trigger and Bipod were added after the first
twelve (2026-10-02), so they take the next ids, and `sort_order` lists them
beside their neighbours:

| id | name | generic_thumbnail_key | sort_order |
|---|---|---|---|
| 1 | Optic | `optic` | 1 |
| 2 | Light or laser | `light` | 2 |
| 3 | Magazine | `magazine` | 3 |
| 4 | Stock or brace | `stock` | 4 |
| 5 | Upper receiver | `upper` | 5 |
| 6 | Barrel | `barrel` | 6 |
| 13 | Trigger | `trigger` | 7 |
| 7 | Muzzle device | `muzzle` | 8 |
| 8 | Conversion kit | `conversion` | 9 |
| 9 | Mount or rail | `mount` | 10 |
| 14 | Bipod | `bipod` | 11 |
| 10 | Sling | `sling` | 12 |
| 11 | Case | `case` | 13 |
| 12 | Other | `accessory` | 14 |

All are offered. There is no Suppressor kind (FR-002). The table gets the
three `*_marks_backup_due_*` triggers.

## Entity: Accessory (new)

```sql
CREATE TABLE accessories (
    id INTEGER PRIMARY KEY,
    uid TEXT NOT NULL UNIQUE CHECK (...),            -- as above
    accessory_kind_id INTEGER NOT NULL REFERENCES accessory_kinds (id),
    -- FR-001: make and model required, as a firearm's, so every accessory
    -- has a name of its own (FR-005); the rest optional. 004's entry rules
    -- apply on entry only, so no length CHECK (an existing longer value
    -- stays valid).
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    serial_number TEXT,
    caliber TEXT,
    cartridge TEXT,
    notes TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disposed')),
    estimated_value INTEGER CHECK (estimated_value IS NULL OR estimated_value >= 0),
    acquisition_source TEXT,
    acquisition_date TEXT,
    acquisition_price INTEGER CHECK (acquisition_price IS NULL OR acquisition_price >= 0),
    disposition_type TEXT CHECK (disposition_type IS NULL OR disposition_type IN
        ('sold', 'traded', 'gifted', 'destroyed', 'lost_stolen')),
    disposition_recipient TEXT,
    disposition_date TEXT,
    disposition_price INTEGER CHECK (disposition_price IS NULL OR disposition_price >= 0),
    thumbnail_photo_id INTEGER REFERENCES photos (id) ON DELETE SET NULL,
    insurance_policy_id INTEGER REFERENCES insurance_policies (id) ON DELETE RESTRICT,
    scheduled_coverage_amount INTEGER CHECK (scheduled_coverage_amount IS NULL OR scheduled_coverage_amount >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK ((insurance_policy_id IS NULL) = (scheduled_coverage_amount IS NULL))
);

CREATE INDEX idx_accessories_kind ON accessories (accessory_kind_id);
CREATE INDEX idx_accessories_status ON accessories (status);
CREATE INDEX idx_accessories_insurance_policy ON accessories (insurance_policy_id);
-- research.md §16: suggest_entries' GROUP BY over both tables.
CREATE INDEX idx_accessories_make ON accessories (make) WHERE make IS NOT NULL;
CREATE INDEX idx_accessories_make_model ON accessories (make, model) WHERE model IS NOT NULL;
CREATE INDEX idx_accessories_caliber ON accessories (caliber) WHERE caliber IS NOT NULL;
CREATE INDEX idx_accessories_cartridge ON accessories (cartridge) WHERE cartridge IS NOT NULL;
```

_Amended 2026-10-07 (on 007's branch, with the suggestions warm-up): `idx_accessories_make` and `idx_accessories_make_model` are no longer partial, `ON accessories (make)` and `ON accessories (make, model)`. Make and model are `NOT NULL`, so SQLite drops `IS NOT NULL` from the query and can't use a partial index (the models over both tables took 51 ms on Windows, against 004's 50 ms). See research.md §16._

The table gets the three `*_marks_backup_due_*` triggers. It has no
nickname, no serial-or-attestation `CHECK`, no identity index and no
fields-apply trigger (FR-004).

**Rust model** (`models/accessory.rs`): `Accessory` (output; `uid` is
`#[serde(skip)]`; plus `mountedOn: RecordRef | null`), `AccessoryInput`
(every column except `id`, `uid`, `thumbnail_photo_id` and the
timestamps, plus `mountedOn`), `AccessoryInput::normalized()` (trim, blank
to `None`, as for `FirearmInput`) and `validate_accessory_input(input,
stored)`.

**Validation rules** (`validate_accessory_input`, shared by the commands
and import, FR-024):

| Field | Rule | Message (field key) |
|---|---|---|
| `accessoryKindId` | Required; must exist in `accessory_kinds` (offered or not; the command checks) | "Choose a kind." (`accessoryKindId`) |
| `make`, `model` | Required; trimmed; 004's entry rules (FR-015), checked only when changed from the stored value | "Make is required." / "Model is required." (`make`, `model`), else as 004 |
| `caliber`, `cartridge` | Optional; otherwise 004's entry rules (FR-015), checked only when changed from the stored value | as 004 |
| `serialNumber`, `acquisitionSource`, `notes` | Optional free text; trimmed; blank → `null` | none |
| `estimatedValue`, `acquisitionPrice`, `dispositionPrice`, `scheduledCoverageAmount` | Whole dollars, not negative (001 FR-037), at most $99,999,999 (_amended 2026-10-09 (#67): the cap is shared with the firearm through `models::rules`_) | as for a firearm ("… can't be more than $99,999,999.") |
| `acquisitionDate` | `YYYY-MM-DD`, not after today (local) | as for a firearm |
| disposition | When `disposed`: type, recipient and date required; price optional (research.md §9). When `active`: none of the four | as for a firearm |
| `dispositionDate` | Not before `acquisitionDate` | as for a firearm |
| insurance | Policy and amount both or neither | as for a firearm |
| `mountedOn` | `null`, or an allowed host (FR-010); checked by the command, not here | see Mount |

**Naming** (FR-005, done in the frontend's `RecordName`): "{make} {model} ·
{kind}". Both are required (FR-001), so an accessory is never named by its
kind alone.

## Entity: Mount (new)

```sql
CREATE TABLE mounts (
    id INTEGER PRIMARY KEY,
    item_firearm_id   INTEGER UNIQUE REFERENCES firearms (id)    ON DELETE CASCADE,
    item_accessory_id INTEGER UNIQUE REFERENCES accessories (id) ON DELETE CASCADE,
    host_firearm_id   INTEGER REFERENCES firearms (id)    ON DELETE CASCADE,
    host_accessory_id INTEGER REFERENCES accessories (id) ON DELETE CASCADE,
    -- Exactly one item and one host.
    CHECK ((item_firearm_id IS NULL) <> (item_accessory_id IS NULL)),
    CHECK ((host_firearm_id IS NULL) <> (host_accessory_id IS NULL)),
    -- FR-010: never mounted on itself (one step; longer loops: the command layer).
    CHECK (item_firearm_id IS NULL OR item_firearm_id IS NOT host_firearm_id),
    CHECK (item_accessory_id IS NULL OR item_accessory_id IS NOT host_accessory_id)
);

CREATE INDEX idx_mounts_host_firearm ON mounts (host_firearm_id) WHERE host_firearm_id IS NOT NULL;
CREATE INDEX idx_mounts_host_accessory ON mounts (host_accessory_id) WHERE host_accessory_id IS NOT NULL;
```

**Rules** (FR-010 to FR-015):

| Rule | Where it is enforced |
|---|---|
| At most one host per item | `UNIQUE` item columns; the command updates the item's row to move it |
| Item and host both active | `services::mounts` before writing; `mounts_active_insert`/`_update` triggers |
| Never on itself, directly or through others | `MountGraph::would_loop` in the command layer; the one-step `CHECK`s; `mount_test.rs` (SC-004) |
| No mount involves a disposed record | dispose commands delete the mounts first; `firearms_disposed_unmounted` / `accessories_disposed_unmounted` triggers |
| Deleting either record removes its mounts | `ON DELETE CASCADE` |
| No history | unmounting deletes the row; `secure_delete` is on |
| No kind or type is judged | nothing reads a kind, type, caliber or cartridge (FR-011) |

```sql
CREATE TRIGGER mounts_active_insert BEFORE INSERT ON mounts
BEGIN
    SELECT RAISE(ABORT, 'a mount needs an active item and an active host')
    WHERE COALESCE((SELECT status FROM firearms WHERE id = NEW.item_firearm_id),
                   (SELECT status FROM accessories WHERE id = NEW.item_accessory_id)) IS NOT 'active'
       OR COALESCE((SELECT status FROM firearms WHERE id = NEW.host_firearm_id),
                   (SELECT status FROM accessories WHERE id = NEW.host_accessory_id)) IS NOT 'active';
END;
-- mounts_active_update: the same, BEFORE UPDATE ON mounts.

CREATE TRIGGER firearms_disposed_unmounted BEFORE UPDATE OF status ON firearms
WHEN NEW.status = 'disposed'
BEGIN
    SELECT RAISE(ABORT, 'a disposed record cannot be mounted or carry mounts')
    WHERE EXISTS (SELECT 1 FROM mounts WHERE item_firearm_id = NEW.id OR host_firearm_id = NEW.id);
END;
-- accessories_disposed_unmounted: the same for accessories.
```

The table gets the three `*_marks_backup_due_*` triggers: a mount is
collection data.

**State transitions** (an item's mount):

```text
not mounted ──mount_record / form save──▶ mounted on H
mounted on H ──mount_record(H2) / form save──▶ mounted on H2   (one step; subtree follows)
mounted on H ──mount_record(null) / form save──▶ not mounted
mounted on H ──item or H disposed──▶ not mounted          (FR-014; the row is deleted)
mounted on H ──item or H deleted──▶ (row cascaded away)    (FR-015)
disposed ──reverse_disposition──▶ active, not mounted     (no mount restored)
```

**`MountGraph`** (`services::mounts`, research.md §5): an in-memory map built
from one scan of `mounts`. It answers `host_of`, `chain`, `below`
(depth-first, each entry with its direct host and depth), `count_below` and
`would_loop`.

## Entity: Firearm (extended)

- `+ uid` (above).
- The output `Firearm` gains `mountedOn: RecordRef | null`, and
  `FirearmInput` gains `mountedOn` (research.md §8).
  `From<&Firearm> for FirearmInput` copies it.
- `validate_firearm_input` no longer requires `dispositionPrice` for a
  disposed firearm (research.md §9). The dispose dialog and
  `DisposeInput.price` still require the host's own price.
- `FirearmSummary` gains `mountedOn: RecordLabel | null` and
  `mountedCounts` (FR-016a; by kind, issue #56).
- The free-text `accessories` column, its search and its spreadsheet column
  are unchanged (FR-007).
- `firearms_fts` is unchanged: no mount is searched (FR-018).

## Entity: Photo, Document Attachment, Disposition History (owner extended)

Each of `photos`, `document_attachments` and `disposition_history`:

```sql
firearm_id INTEGER REFERENCES firearms (id) ON DELETE CASCADE,        -- was NOT NULL
accessory_id INTEGER REFERENCES accessories (id) ON DELETE CASCADE,   -- new
CHECK ((firearm_id IS NULL) <> (accessory_id IS NULL)),
```

Each also gains an index on `accessory_id` (`WHERE accessory_id IS NOT
NULL`). The existing `firearm_id` index stays. The Rust models replace
`firearm_id: i64` with `owner: RecordRef` (serialized as `owner`).

## Virtual table: `accessories_fts` (new)

```sql
CREATE VIRTUAL TABLE accessories_fts USING fts5(
    kind_name, make, model, serial_number, caliber, cartridge, acquisition_source, notes,
    content = 'accessories',
    content_rowid = 'id',
    tokenize = 'trigram remove_diacritics 1'
);
```

Kept in step by `accessories_fts_after_insert`, `_after_delete` and
`_after_update` triggers, shaped like `firearms_fts`'s, with `kind_name`
looked up from `accessory_kinds`. A query of one or two characters uses
`LIKE` over the same values (research.md §12).

## Entity: Pending changes (extended)

```sql
kind TEXT NOT NULL CHECK (kind IN ('firearm', 'policy', 'accessory')),
...
CHECK (mode <> 'coverage' OR kind IN ('firearm', 'accessory')),
```

`DraftKind` gains `Accessory`. `validate_draft` allows an accessory the same
modes as a firearm (FR-027).

## Value summary (derived, not stored)

Recomputed on every call, as before (001 FR-015, research.md §11):

- `collectionTotal = firearmsTotal + accessoriesTotal`, over active records
  only (SC-005).
- The blanket total covers unscheduled active firearms **and** accessories,
  with a count of each.
- Each individually scheduled or uninsured entry is a `record: RecordRef`.

## Spreadsheet rows (import shape)

See [contracts/spreadsheet-format.md](./contracts/spreadsheet-format.md).
`RawImportRow` gains `record_id` and `mounted_on`, and `RawAccessoryRow`
mirrors it for the accessory table. A missing `mounted_on` column reads
as blank, as any missing column does (research.md §18).

## Seed and coverage

`examples/human_seed.rs` seeds, through `ops`:
- one accessory of every kind, with and without each optional field;
- a pair-of-magazines record;
- an accessory scheduled under a policy;
- a disposed accessory with retained history;
- accessories with photos (one with two, thumbnail switched) and a
  document;
- the user stories' mounts: an optic and a suppressor on a rifle, a
  receiver with an upper carrying a scope (carrying a red dot) and a light,
  a launcher with a light on a rifle, and an unmounted upper carrying its
  own optic;
- import samples for both tables, including an accessories-only CSV, a
  workbook with both sheets, and a pre-feature firearm sheet.

`tests/human_seed_coverage_test.rs` then holds every new column, and
`is_user_table` skips `accessory_kinds` as it skips the other lookups.
