# Phase 0 Research: Accessory Records and Mounting

The Technical Context has no open unknowns: the stack is feature 001's and
no dependency is added. This document records the decisions the spec left
to planning, the ones the codebase forced, and the two places where the
spec and the existing application disagree (§16, §9), each with its
rationale and the alternatives considered.

The spec's own release question (issue #50's 0.1.0 plan) is answered first,
in §1, because every later decision feeds it.

## 1. Does the design change existing tables? Yes, so it is built in 0.1.0

- **Decision**: The design changes five existing tables and adds four. The
  owner's plan on issue #50 says to build the feature in 0.1.0 in that
  case, before migrations start to count (#24).

  | Table | Change | Why |
  |---|---|---|
  | `firearms` | `+ uid TEXT NOT NULL UNIQUE` | FR-019: the record identifier (§6) |
  | `photos`, `document_attachments`, `disposition_history` | `firearm_id` becomes nullable, `+ accessory_id`, one owner `CHECK` | FR-007a, FR-006: accessories have them too (§3) |
  | `pending_changes` | `kind` allows `'accessory'`; `coverage` allowed for it | FR-027 (§19) |
  | `accessory_kinds` *(new)* | seeded lookup | FR-002 (§15) |
  | `accessories` *(new)* | the records | FR-001 (§2) |
  | `mounts` *(new)* | the current mounts | FR-010 (§4) |
  | `accessories_fts` *(new)* | trigram search | FR-018 (§12) |

  The free-text `firearms.accessories` column is untouched (FR-007).
- **Rationale**: Even the smallest design changes `firearms`, because
  FR-019 needs the identifier on every firearm, and `pending_changes`,
  because its `CHECK` names the kinds of form. In SQLite, adding a
  `NOT NULL UNIQUE` column or changing a `CHECK` after release means
  rebuilding the table. Doing it now, while `0001_initial.sql` is edited in
  place (CLAUDE.md), costs nothing.
- **Alternatives considered**: An add-only design (parallel
  `accessory_photos`, `accessory_documents` and
  `accessory_disposition_history` tables, with the identifier left to #53).
  It still needs the `pending_changes` change, so it doesn't actually avoid
  touching existing tables. It also doubles every photo, document and
  history code path. Rejected.

## 2. Accessories are their own table, not firearm rows and not a supertable

- **Decision**: A new `accessories` table holds the fields FR-001 lists,
  plus status, disposition, insurance scheduling, the thumbnail and the
  identifier, using the same column names and `CHECK`s as `firearms`. The
  kind is `accessory_kind_id`, which references `accessory_kinds`. Make,
  model, serial number, caliber and cartridge are nullable, because only the
  kind is required (FR-001).
- **Rationale**: The spec makes accessories a separate kind of record, with
  none of a firearm's identity rules (FR-004). A separate table keeps every
  firearm `CHECK` and trigger (serial or attestation, nickname, identity,
  fields-apply, registration) exactly as it is. Using the same column names
  means the shared pieces (amount checks, the disposition rules,
  `insurance_status`, export cell formatting) can take a column's value
  from either table without translation.
- **Alternatives considered**:
  - *Accessory rows in `firearms` with a kind flag.* Every firearm
    constraint would need an "unless accessory" escape, and
    `list_firearms` would need to filter them out everywhere. Rejected.
  - *A `records` supertable* (`id`, `uid`, `kind`), with
    `firearms.id` and `accessories.id` referencing it, and photos,
    documents, history and mounts keyed by record id. It would give one
    id space and a simple self-referencing mount. But every firearm insert
    becomes two, `photos.firearm_id` is renamed in about 140 places, and
    deleting a firearm has to go through the supertable. That is the
    largest change for a benefit that §3 and §4 get more cheaply. Rejected
    under constitution I (complexity must be justified by a current need).

## 3. Photos, documents and disposition history: one table each, owned by a firearm or an accessory

- **Decision**: `photos`, `document_attachments` and `disposition_history`
  each keep `firearm_id`, now nullable, and gain
  `accessory_id INTEGER REFERENCES accessories (id) ON DELETE CASCADE`,
  with `CHECK ((firearm_id IS NULL) <> (accessory_id IS NULL))` and an
  index on `accessory_id`. In Rust, an `Owner`/`RecordRef` enum picks the
  column. The photo and document commands take `owner: RecordRef` instead of
  `firearmId` (contracts/tauri-commands.md). `set_thumbnail_photo` updates
  `firearms` or `accessories` according to the owner.
- **Rationale**: FR-007a asks for "exactly as a firearm", and one table per
  concern means one code path. That is the only way to guarantee "exactly"
  as the photo and document behavior evolves. `ON DELETE CASCADE` from both
  owners keeps constitution V's deletion with no extra code, and
  `secure_delete` plus the existing `VACUUM` reclaim covers the bytes
  (§10).
- **Alternatives considered**: Parallel tables (rejected in §1). A
  polymorphic `owner_kind`, `owner_id` pair with no foreign key: it loses
  `ON DELETE CASCADE` and needs delete triggers instead. Rejected.

## 4. Mounts: a `mounts` table with an item pair and a host pair

- **Decision**:

  ```sql
  CREATE TABLE mounts (
      id INTEGER PRIMARY KEY,
      item_firearm_id   INTEGER UNIQUE REFERENCES firearms (id)   ON DELETE CASCADE,
      item_accessory_id INTEGER UNIQUE REFERENCES accessories (id) ON DELETE CASCADE,
      host_firearm_id   INTEGER REFERENCES firearms (id)   ON DELETE CASCADE,
      host_accessory_id INTEGER REFERENCES accessories (id) ON DELETE CASCADE,
      CHECK ((item_firearm_id IS NULL) <> (item_accessory_id IS NULL)),
      CHECK ((host_firearm_id IS NULL) <> (host_accessory_id IS NULL)),
      CHECK (item_firearm_id IS NULL OR item_firearm_id IS NOT host_firearm_id),
      CHECK (item_accessory_id IS NULL OR item_accessory_id IS NOT host_accessory_id)
  );
  ```

  Each `UNIQUE` item column enforces at most one host per item (FR-010).
  SQLite treats `NULL`s as distinct, so the other kind's rows don't collide.
  Deleting either record cascades its mount rows away (FR-015), so a
  deleted host's items are left unmounted with no application code.
  - **Backstops** (data-model.md): a trigger pair refuses a mount row whose
    item or host is not active, and a trigger on each record table refuses
    setting `status = 'disposed'` while the record is in a mount.
  - **Loops** are prevented in the command layer only (§5). SQLite does not
    allow a `WITH` clause inside a trigger, so a recursive check has no
    backstop. The same-record `CHECK`s catch the one-step loop, and
    `mount_test.rs` holds the rest with a randomized sequence of operations
    (SC-004).
- **Rationale**: The table is additive. A mount change doesn't rewrite the
  record's row, so it doesn't re-index the firearm's 21 search columns or
  touch `updated_at`. Loading the whole graph is one narrow scan (§5).
  "No history" (FR-013) holds by construction: unmounting deletes the row,
  and `secure_delete` overwrites it.
- **Alternatives considered**:
  - *`mounted_on_firearm_id` and `mounted_on_accessory_id` columns on both
    record tables*: four columns on two tables, an FTS re-index on every
    mount, and the `UNIQUE` item rule comes for free but nothing else does.
    Rejected.
  - *A `records` supertable* (§2): rejected.

## 5. The mount graph is loaded once per call and walked in Rust

- **Decision**: A pure module, `services::mounts`, holds a `MountGraph`
  built from `SELECT item_firearm_id, item_accessory_id, host_firearm_id,
  host_accessory_id FROM mounts`. It is a `HashMap<RecordRef, RecordRef>`
  from item to host, plus its reverse, a map from host to its items. It
  answers:
  - `host_of(item)`;
  - `chain(item)`: host, host's host, and so on, for FR-013's "Mounted on
    BCM upper, on LaRue receiver";
  - `below(host)`: depth-first, each entry with its direct host, for the
    Mounted section and the dispose dialog;
  - `count_below(host)`: memoized, for every firearm on the collection page
    (FR-016a);
  - `would_loop(item, host)`: true when `host == item`, or when `host` is in
    `below(item)` (FR-010).

  Mounting runs `would_loop` and then the backstopped insert or update,
  inside `session.write`.
- **Rationale**: It follows `insurance_status::load_context`'s "load once,
  decide in plain functions" pattern, so the rules are unit-testable
  without IPC. At the spec's scale (10,000 firearms and 10,000 accessories,
  at most 20,000 mount rows of four integers) the scan and map build take a
  few milliseconds, well inside the 1 s and 500 ms budgets. That is
  measured in `performance_test.rs` (§22). Walking the graph in memory
  can't overflow on a deep chain, because the walk is iterative.
- **Alternatives considered**: Recursive CTEs per question. They work for
  `below` and `would_loop`, but counting below each of 10,000 firearms
  would be 10,000 recursive queries, or one hard-to-read
  `GROUP BY` over a transitive closure. Rejected for the counts. Using
  the graph for everything keeps one implementation.

## 6. The record identifier: a UUID in `uid`, on firearms and accessories

- **Decision**: `firearms` and `accessories` each gain
  `uid TEXT NOT NULL UNIQUE`, with a `CHECK` on its shape (data-model.md
  gives it). It holds a random **version 4 UUID**, lowercase and hyphenated
  (`3f2a9c1e-5b7d-4e8a-9c0f-1a2b3c4d5e6f`). `services::record_id` generates
  it from `getrandom`, which is already a dependency, and parses
  spreadsheet values. It is set in `ops::create_firearm` and
  `ops::create_accessory`, and never in an `UPDATE`. A trigger pair
  refuses a `uid` already used by the other table, because FR-022 makes a
  cross-kind clash a row error. The model structs carry `uid` as
  `#[serde(skip)]`: FR-019 shows it nowhere but the spreadsheet.
- **Rationale**:
  - A hyphenated UUID can't be read as a number or a date by any
    spreadsheet application. A bare 32-digit hex string can: one made
    only of digits, or of digits around an `e`, would be read as a number
    and lose its digits, and a CSV opened and saved in a spreadsheet
    application would corrupt it.
  - The format is widely recognised, so a user can see what the column is.
  - Import accepts any letter case and surrounding whitespace, and stores
    lowercase.
  - Issue #53 leaves the format open. This decision proposes UUIDs for #53's
    other tables too, so the application has one identifier format.
    Whichever of #53 and 006 is built first adds the two record tables'
    columns, as #53 says.
- **Alternatives considered**: 32 lowercase hex digits, like
  `app_state.database_id`. It is consistent with that column but at risk in
  spreadsheets, as above. A 16-byte `BLOB`: unreadable in the spreadsheet
  and in tests. The `uuid` crate: not needed for generating and checking one
  format. Rejected.

## 7. `RecordRef` over IPC

- **Decision**: Anything that can be either kind of record is named over
  IPC by `RecordRef = { kind: "firearm" | "accessory", id: number }`.
  Lists that show a record use `RecordLabel`, which is a `RecordRef` plus
  the pieces the frontend names it from: make, model, nickname, type or kind
  name, and status (contracts/tauri-commands.md). Naming stays in the
  frontend, as it is for firearms today (`FirearmName`). The new
  `RecordName` component calls `FirearmName` for a firearm and FR-005's
  rule for an accessory.
- **Rationale**: Integer ids are per table, so a bare id is ambiguous. One
  shape for "a record" keeps the mount, candidate, value-summary and
  policy-deletion shapes uniform.

## 8. How a mount is set: on the forms, and by one command from a host's page

- **Decision**:
  - **The forms.** `FirearmInput` and `AccessoryInput` gain
    `mountedOn: RecordRef | null`. Create and update set the record's mount
    in the same transaction as the save. The output `Firearm` and
    `Accessory` gain `mountedOn` (the direct host), so that
    `From<&Firearm> for FirearmInput` (used by dispose, reverse and
    coverage) carries the current mount and never drops it by accident.
    Dispose and reverse set it to `null`, as FR-014 requires (§9). The form
    always sends the field.
  - **`mount_record(item, host)`**, where `host` may be `null` to unmount,
    serves the host page's Mount choice ("Existing accessory or
    firearm…") and the Unmount action on a direct entry in the Mounted
    section. Moving takes one call (FR-012).
  - **The move confirmation** ("Move it from …?") is the frontend's
    `ConfirmDialog`, shown when the chosen record's label carries a current
    host. The backend doesn't refuse a move, since the item's own form
    moves with no confirmation (FR-012).
  - **`list_mount_candidates`** returns the choices for both directions:
    - *hosts* for an item (or for a new record): active firearms and
      accessories, minus the item and everything below it;
    - *items* for a host: active records, minus the host, its chain, and
      what is already directly on it.

    Each candidate comes with its current direct host. Matching is
    case-insensitive substring on make, model, nickname and serial number,
    limited to 50 results and ordered by name (FR-012).
  - **"New accessory…"** opens `AccessoryForm` in add mode with `mountedOn`
    preset to the host. Saving creates the accessory and its mount in one
    transaction.
- **Rationale**: Making the mount part of the form's input is what FR-027
  requires ("a firearm's mount choices are part of its own form's input").
  Doing it in one transaction means no record is ever saved with its mount
  half-applied. Using the overridable-warning pattern
  (`confirmedWarnings`) for the move would make the item's form, which needs
  no confirmation, pass a flag just to skip it. Rejected.

## 9. Disposing with mounted items, and the disposition price

- **Decision**:
  - **The commands.** `dispose_firearm` and `dispose_accessory` take
    `withMounted: [{ record: RecordRef, price: number | null }]`, the
    records to dispose of with the host. Everything else below the host is
    kept. One transaction:
    1. Check that every listed record is below the host now. If not, fail
       with `VALIDATION_ERROR` on `withMounted` ("What is mounted has
       changed. Close the dialog and try again.").
    2. Delete every mount whose item or host is the host or a listed
       record.
    3. Save each listed record as disposed, with the host's type,
       recipient and date and its own price, which may be null.
    4. Save the host as disposed.

    A kept record mounted on a kept record keeps its mount. A kept record
    mounted on a disposed one loses it in step 2 (FR-014). The status
    triggers (§4) check the result.
  - **A disposed record may have no price.** FR-014 makes an item's price
    optional (blank means no price), and SC-002 requires its export to
    re-import. So `validate_firearm_input` and `validate_accessory_input`
    no longer require `dispositionPrice` when disposed: type, recipient and
    date stay required. The host's own price stays required, as today:
    the dispose dialog still asks for it ("Enter the price, or 0 if nothing
    was received."), and `DisposeInput.price` stays a number.
  - This changes 001's import behavior in one way: a disposed row with a
    blank `disposition_price` is now accepted, not a row error. That is
    unavoidable for the round trip. It is listed under "Findings to
    confirm" in plan.md.
  - Reversing a disposition restores no mount (FR-014). Nothing has to be
    done for that, because the mounts were deleted at disposal.
- **Rationale**: One command keeps the host and its items consistent. A
  half-applied disposal could leave an item mounted on a disposed host,
  which SC-004 forbids. Listing only the records to dispose of, and
  checking them against the current graph, means a stale dialog (for
  example, resumed from pending changes after another computer edited the
  file) can't dispose of the wrong record.
- **Alternatives considered**: One call per item from the frontend: not
  atomic. Rejected. Keeping the price required and writing `0` for a blank
  item price: FR-014 says blank means no price, so "0" would record
  something the user didn't say. Rejected.

## 10. Deletion

- **Decision**: `delete_accessory` deletes the row. `ON DELETE CASCADE`
  removes its photos, documents, history and mounts (both as item and as
  host). It then runs the same reclaim as a firearm deletion: optimize
  `accessories_fts`, then `VACUUM` (`db::reclaim_deleted_record`, renamed
  from `reclaim_deleted_firearm`). `delete_firearm` is unchanged apart from
  the cascade now also covering `mounts`. Each delete confirmation names the
  records directly mounted on it, from the detail's `mounted` list, because
  those are the ones left unmounted. Records further down stay on their own
  hosts (FR-015, US3-5, US3-9).
- **Rationale**: Constitution V and SC-006. `deletion_wipe_test.rs` gains an
  accessory with a unique serial number, note, photo and document, and an
  accessory host with items, and checks the raw file for none of those
  bytes.

## 11. Value and insurance include accessories

- **Decision**:
  - `insurance_status::load_context` computes the blanket total over
    unscheduled active firearms **and** accessories. It also counts each
    kind.
  - `firearm_warning` is renamed `record_warning`. Its logic is unchanged,
    because it only reads value, policy and amount.
  - `valuation::get_value_summary` reads both tables. `ValueSummary` gains
    `firearmsTotal` and `accessoriesTotal` (FR-008's subtotal). Each
    scheduled or uninsured entry is keyed by `record: RecordRef`.
  - `assign_accessory_coverage` mirrors `assign_firearm_coverage`.
  - Policy deletion's impact and its "move" or "leave unscheduled" updates
    cover both tables, counted together (FR-009).
- **Rationale**: SC-005 holds by construction: the collection total is the
  sum of both tables' active values in one function. The warning stays one
  function, so the collection page, the Accessories page and the value
  summary can't disagree.

## 12. The Accessories page: listing, grouping and search

- **Decision**:
  - `list_accessories(query, groupBy, includeDisposed)` mirrors
    `list_firearms`. It is one query over `accessories` joined to
    `accessory_kinds`, plus the mount graph and the host labels for
    `mountedOn`.
  - **Search** uses `accessories_fts`, an external-content FTS5 table with
    the trigram tokenizer. It indexes the kind's name, make, model, serial
    number, caliber, cartridge, acquisition source and notes, every text
    field FR-018 means. A query of one or two characters uses `LIKE` over
    the same values, as `list_firearms` does.
  - **Grouping by kind** follows the kind list's order, with groups in
    alphabetical order otherwise. "Unspecified" (no value) and
    "Not mounted" come last.
  - **Grouping by "Mounted on"** is keyed by the host's `RecordRef`, never
    by its name, because two hosts can share a name. Each group carries its
    `host: RecordLabel`. Groups are sorted by the host's name, then by kind
    and id for a stable order.
  - **Group building** uses a `HashMap` from key to group index.
    `list_firearms` used to find the group with a linear search for each
    row. That is fine for its few groups, but grouping 10,000 accessories by
    host could produce thousands of groups. `list_firearms` has the same
    map, which is harmless.
- **Rationale**: FR-016 to FR-018 and US4. Mirroring `list_firearms` keeps
  the two pages' behaviors identical (constitution III). The collection
  page's search is unchanged: `firearms_fts` gains nothing from mounts
  (FR-018, US4-6).

## 13. The collection page's mount details

- **Decision**: `FirearmSummary` gains `mountedOn: RecordLabel | null` (the
  direct host) and `mountedCounts: RecordCounts` (everything below it, by kind; issue #56).
  `list_firearms` loads the graph once and fetches the labels of the hosts
  it needs in one query per table
  (`WHERE id IN (SELECT value FROM json_each(:ids))`). Grouping and search
  are unchanged (FR-016a).
- **Rationale**: One graph load and two small label queries keep the
  collection page's 500 ms budget at 10,000 firearms with up to 20,000
  mounts (`performance_test.rs`).

## 14. Presenting nested mounts: a flat outline with "on …" lines

The spec leaves the presentation to the plan, with the condition that it
stays readable at any depth without assuming an indent per level (FR-013).

- **Decision**: The Mounted section lists everything below the record in
  depth-first order, as one flat list with a single fixed indent:
  - **Direct entries** sit at the list's edge. Each shows the record's name,
    its type or kind, and an Unmount button.
  - **Each deeper entry** sits one fixed step in, under the direct entry
    whose subtree it belongs to. Its name is followed by a muted second
    line, "on {host name}". It has no Unmount button, because this page is
    not its host's.
  - Every name is a link to the record (001 FR-040: back returns here).

  The dispose dialog uses the same rows, with the keep or dispose choice and
  the price field on each row. contracts/ui-accessories.md §5 and §7 give
  the mockups, roles and copy. It is designed with the frontend design
  skill, as 005's grouping menu was.
- **Rationale**: The depth never widens the layout, so a five-level chain
  reads like a two-level one. The "on …" line says exactly what each entry
  is mounted on, which FR-013 requires. The single indent still shows
  which direct entry an item travels with, which is the question the owner
  asks ("what goes with the upper?").
- **Alternatives considered**: An indent per level (excluded by the
  spec's concern). A tree widget with expanders hides nested items, but
  the dispose dialog must show every choice. A breadcrumb on every row
  ("on Scope, on Upper"): it repeats the chain on each row and gets longer
  with depth. Rejected.

## 15. Accessory kinds: a seeded lookup, and a drawing per kind

- **Decision**:
  - `accessory_kinds (id, name, generic_thumbnail_key, sort_order,
    offered)` is seeded in `0003_seed_firearm_types.sql` with fixed ids
    1–12 in FR-002's first order, then 13 Trigger and 14 Bipod, added
    later, which `sort_order` lists beside their neighbours. Other is
    last, and all are offered. Like
    `registration_classes`, the `offered` flag lets a later version stop
    offering a kind without removing it (FR-002).
  - `list_accessory_kinds` exposes the list, so the frontend never hard-codes
    it, following 005's `list_firearm_types`.
  - Each kind gets its own drawing in `typeDrawings.ts`, keyed by
    `generic_thumbnail_key` (`optic`, `light`, `magazine`, `stock`,
    `upper`, `barrel`, `trigger`, `muzzle`, `conversion`, `mount`,
    `bipod`, `sling`, `case`, `accessory`). The drawings use the same
    320×200 box, muzzle-right convention and line roles. They are drawn
    for the project from the published dimensions of common patterns,
    each at a stated scale, with no brand marks, and their source note is
    recorded in the file (Licensing; as 005's suppressor). Each must be
    recognisable for what it is: the bipod is seen from the front so both
    legs show, the case lies flat, the trigger is an AR-15 drop-in
    cassette (hammer on the front pin, blade between the pins), and
    Other's drawing (an open box of spare parts, seen from a little above
    so its flaps hinge on its walls) suggests no particular accessory
    (redrawn 2026-10-02, from manual testing).
  - Like a firearm type, a kind carries no rules (spec Assumptions). Every
    kind has the same fields.
- **Rationale**: FR-002, and FR-007a's "a generic picture for its kind".
- **Alternatives considered**: One shared accessory drawing for every
  kind. It contradicts FR-007a and makes the tile view useless for telling
  kinds apart. Rejected.

## 16. Entry rules, suggestions and caliber derivation for accessories

- **Decision**:
  - Make, model, caliber and cartridge reuse `EntryField` unchanged:
    `check_entry_text` on save and import, and `settle_entry`/`snap` on the
    form.
  - `FieldVocabulary::load` reads `firearms UNION ALL accessories` for each
    of the four fields. The model suggestions' "same make" rule reads both
    tables. Partial indexes on `accessories (make)`, `(make, model)`,
    `(caliber)` and `(cartridge)` keep `suggest_entries` within 004's 50 ms.
    _Amended 2026-10-07: `FieldVocabulary::load` reads one query per table and
    merges the groups, and the make and make-model indexes are not partial,
    because make and model are `NOT NULL` (amended 2026-10-02), and SQLite then
    drops `IS NOT NULL` from the query and can't use a partial index (the
    models over both tables took 51 ms on Windows until then). The caliber and
    cartridge indexes stay partial._
  - Make and model are required on an accessory, as on a firearm
    (amended 2026-10-02); the caliber is optional, so `check_entry_text`
    for an accessory treats a blank caliber as "none". The caps and
    character rules still apply.
  - A blank caliber is derived from the cartridge with 004's
    `derive_caliber`, on the form and on import, for every kind. There is no
    per-kind `caliber_from_cartridge` flag, because no kind's caliber means
    anything else (FR-003).
- **Acquisition source and serial number stay plain text** (resolved with
  the owner, 2026-10-01). FR-003 first listed acquisition source among the
  fields that suggest and snap, and both it and the serial number among
  those under 004's entry rules. A firearm's have never had either: they
  are plain text in `FirearmForm` and not `EntryField`s. Giving the
  accessory form behavior the firearm form lacks would break constitution
  III, and changing the firearm form is outside this spec. So both are
  plain text on an accessory, trimmed, blank stored as null, exactly as on
  a firearm, and FR-003 now says so. Whether "acquisition source" is the
  right field at all, and whether it should suggest, is examined for both
  kinds of record in issue #55.

## 17. Spreadsheet: two tables, recognised by header

Full details are in contracts/spreadsheet-format.md. The decisions:

- **Columns.**
  - Both tables start with `record_id` and end with `mounted_on`, then
    `photo_filenames`.
  - The accessory table's columns are `record_id`, `kind`, `make`,
    `model`, `serial_number`, `caliber`, `cartridge`, `notes`, `status`,
    the value, acquisition and disposition columns as on the firearm table,
    `insurance_policy_name`, `scheduled_coverage_amount`, `mounted_on` and
    `photo_filenames`.
  - `services::spreadsheet` gains `ACCESSORY_COLUMNS` beside `COLUMNS`,
    which is renamed `FIREARM_COLUMNS`.
- **Recognising a table.**
  - A header row with `kind` and without `firearm_type` is the accessory
    table. One with `firearm_type` and without `kind` is the firearm table.
  - Anything else, or a second table of a kind already read, stops the
    import before any row is saved, naming the file (and the sheet, in a
    workbook) (FR-022).
  - In a workbook, every sheet with a non-empty header row must be one of
    the two tables. A blank sheet is ignored.
  - Old sheets have `firearm_type`, so they are still recognised (US5-7).
- **File names on export.**
  - CSV: `{base}.csv` for firearms, as today, plus `{base}-accessories.csv`
    when the export includes an accessory.
  - XLSX: one workbook with sheets named "Firearms" and "Accessories".
  - Accessory photos are written as `a{id}_{filename}`, so they can't
    collide with a firearm's `{id}_{filename}`.
- **Import order.**
  1. Read and recognise every file and sheet.
  2. Firearm rows, then accessory rows, each through its own `parse_row`,
     matching and create-or-conflict, as today.
  3. Mount resolution, once every row is settled (§18).

  One `ImportResult` covers both tables. Each row's place in the report is
  named by its table and its row number ("Accessories, row 4").
- **Matching (FR-022).**
  - When the row has an identifier that belongs to a record of the same
    kind, active or disposed, that record is the match.
  - Otherwise, for a firearm row, 001's make, model and serial number key
    is the match, as today.
  - For an accessory row with no identifier, or one that matches nothing,
    there is never a match: the row is created.
  - A created record keeps the row's identifier, or gets a new one when
    the row has none. A duplicate gets a new one.
  - A malformed identifier, one repeated in the same import, or one that
    belongs to the other kind, is a row error.
  - `ImportConflict` gains `kind` and `existingRecord: RecordRef`. For
    accessories, `duplicateAllowed` is always true.
- **The disposition price** may be blank on a disposed row (§9).

## 18. Resolving "mounted on" on import

- **Decision**: After every row is settled, a `RowOutcome` map runs from
  each row's identifier to the record the row became:
  - *created*: the new record;
  - *conflict*: the existing record it matched;
  - *row error*: none.

  Then, in file order (firearm table first), for each row that was
  **created**, with a non-blank `mounted_on`:
  1. Parse the identifier. Malformed: warning, left unmounted.
  2. Resolve it to a host:
     - through the outcome map first, so a host row in the same import
       counts even when it matched an existing record by make, model and
       serial number;
     - then through `uid` in both tables.

     No host found: warning ("No firearm or accessory has record ID …").
  3. A disposed item or host: warning.
  4. `would_loop`: warning ("would be mounted on itself through …"), and
     that mount is dropped.
  5. Otherwise mount.

  Conflict rows keep their `mounted_on` in the import session, and
  `resolve_import_conflicts` applies the same steps when the choice is
  overwrite or duplicate:
  - **overwrite**: the row's mount, or unmounted when the cell is blank
    (FR-023, Edge Cases). A sheet with no `mounted_on` column reads as
    blank in every row, as any missing column does;
  - **skip**: nothing changes.

  The warnings are added to `ImportResult.warnings` and
  `ResolveResult.warnings`, which already exist.
- **Rationale**: Resolving after every row has been saved lets a row name a
  host that appears later in the file, or in the other table (US5-3). The
  outcome map covers the case the spec's edge cases imply, where a host row
  in the same file matched an existing record. An absent column is not
  told apart from a blank cell: until the first release there are no
  exports made before this feature to protect (owner's decision of
  2026-10-01).

## 19. Pending changes and unsaved-changes questions

- **Decision**:
  - `pending_changes.kind` allows `'accessory'`, with the modes add, edit,
    dispose, restore and coverage. The `coverage` `CHECK` becomes
    `kind IN ('firearm', 'accessory')`. `DraftKind` gains `Accessory`, and
    `validate_draft` allows the same modes as for a firearm.
  - `AccessoryForm`, the accessory's dispose, restore and coverage dialogs
    register with `SessionProvider`'s unsaved-changes tracking, as the
    firearm's do. Each draft's label names the accessory by FR-005
    ("Leupold VX-5HD 3-15x44 · Optic — edit").
  - The firearm form's `FORM_VERSION` goes up, because its values gain
    `mountedOn`. The dispose dialog's goes up, because its values gain the
    per-item choices and prices. An older draft is offered as today's
    version rule handles it (003 research.md §16).
- **Rationale**: FR-027.

## 20. Navigation, pages and forms

- **Decision**:
  - `Route` gains `{ page: "accessories" }` and
    `{ page: "accessory"; id; from }`. The shell's tab row gains
    "Accessories" between Collection and Insurance, with its own "Add
    accessory" action on the page. `ShellDialog` gains `"addAccessory"`.
  - `features/accessories/` holds `AccessoriesPage` (list and tiles,
    grouping menu, search, disposed toggle), `AccessoryForm`,
    `AccessoryRecordPage`, `accessoriesService.ts` and `types.ts`.
  - The shared pieces move to where both kinds of record use them:
    - `features/mounts/` holds `MountedSection`, `MountChooser` (the
      Combobox-based picker used for "Mounted on" and "Existing accessory
      or firearm…"), `MountedOnChain` and `RecordName`;
    - `PhotoGallery` and `DocumentList` take an `owner: RecordRef`;
    - `DisposeDialog`, `RestoreDialog` and `CoverageDialog` take a record
      of either kind, not a separate copy.
  - `collectionStore` keeps `accessories` and `accessoriesById` beside
    `firearms`, refreshed after any accessory or mount change, so names
    resolve without extra calls.
- **Rationale**: Constitution III. Each dialog and form pattern exists once
  and serves both kinds of record. CLAUDE.md's UI-consistency rule then
  carries any later change to both.

## 21. Media commands take an owner

- **Decision**: `list_photos`, `add_photo`, `add_photo_from_path`,
  `set_thumbnail_photo`, `list_documents`, `add_document` and
  `add_document_from_path` replace their `firearmId` argument with
  `owner: RecordRef`. `delete_photo`, `get_photo_*`, `delete_document` and
  `open_document` take the photo or document id as today. Decrypted
  document copies go to the same `OPENED_DOCUMENTS_DIR`, cleared at the
  same times (FR-007a, 001 FR-035).
- **Rationale**: One command set, as one table (§3). The application is
  unreleased, so changing the argument breaks nothing outside the repo.

## 22. Performance plan (FR-026, SC-007)

`performance_test.rs` seeds 10,000 firearms and 10,000 accessories, with
5,000 mounts that include chains five deep, and holds each operation to its
budget:

| Operation | Budget | Expected cost |
|---|---|---|
| `get_firearm` / `get_accessory` with chain and Mounted list | 1 s | graph load (about 5 ms) + label queries |
| `mount_record` (move with a subtree), unmount | 1 s | graph load + `would_loop` + one write |
| `dispose_firearm` with 20 items below | 1 s | one transaction |
| create or update with `mountedOn` | 1 s | as today + graph check |
| `list_accessories` search, and each grouping | 500 ms | FTS5 + graph load + label lookups |
| `list_firearms` with `mountedOn` and `mountedCounts` | 500 ms | as today + graph load + one label query per table |
| `list_mount_candidates` | 500 ms | `LIKE` over 20,000 rows, limit 50 |
| `suggest_entries` over both tables | 50 ms | two indexed `GROUP BY`s |

`delete_*` keeps 001's `VACUUM` reclaim. Its time is a property of the
database's size, which this feature doesn't change materially.

## 23. Things checked that need no decision

- **Backups** copy the whole file, so the identifier survives backup and
  restore unchanged (FR-019). `record_identifier_test.rs` checks it.
- **Backup due**: the four new tables get the `*_marks_backup_due_*`
  triggers. `backup_due_tracking_test.rs` fails otherwise.
  `accessories_fts` is a virtual table and is excluded from that rule, as
  `firearms_fts` is.
- **The human seed**: it gains accessories of every kind, mounts of every
  shape in the user stories, accessory photos and documents, an accessory
  scheduled under a policy, a disposed accessory with history, and import
  samples for both tables. `human_seed_coverage_test.rs` checks every new
  column, `photos.accessory_id` included.
- **Cipher settings** are untouched.
