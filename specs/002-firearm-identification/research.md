# Phase 0 Research: Firearm Identification & Markings

The Technical Context has no open unknowns: the stack is feature 001's and
no dependency is added. This document records the design decisions the spec
deliberately left to the plan (the "Follow-ups handed to the plan" in the
spec's Source Request) and the rationale for each, so nothing is left as
NEEDS CLARIFICATION going into Phase 1.

The regulatory research (ATF Ruling 2013-3, pre-1968 serial practice,
wartime duplicate serials) is recorded in the spec's Assumptions and is
taken as given here. It was not re-verified for this plan, and nothing in
the design depends on it being exactly right: FR-006 means the app records
what the owner states and never checks it against any era's rules.

## 1. Where the new values live: columns on `firearms`

- **Decision**: Seven nullable columns on `firearms`: `origin`,
  `year_of_manufacture`, `country_of_manufacture`, `importer_name`,
  `original_make`, `original_model`, `original_serial_number`. Migrations
  `0001`/`0002` are edited in place.
- **Rationale**: Every value belongs to exactly one firearm and is never
  shared or looked up across firearms (spec Key Entities: "not shared or
  looked up across firearms"). Columns make the FTS triggers, export, and
  the existing `Firearm`/`FirearmInput` shapes a straight extension. The
  app is unreleased and the spec's Assumptions say an existing development
  database may be recreated, as with earlier schema changes.
- **Alternatives considered**: An `importers` table with a foreign key
  (rejected: importer names are stored as entered and consolidating
  spellings is the planned classification-vocabularies feature's suggestion
  mechanism, so a table would be built ahead of its requirement); a separate `original_marks` table
  (rejected: one row per firearm at most, no independent lifecycle, and it
  would make the FTS trigger and export joins heavier for no gain); a new
  numbered migration (rejected: the repo's convention for unreleased-schema
  changes is in-place edits, and a migration would have to carry dead
  compatibility code for databases nobody has).

## 2. Origin: a three-value text enum, blank means unspecified

- **Decision**: `origin TEXT` with `CHECK (origin IS NULL OR origin IN
  ('domestic', 'imported', 'reimported'))`. The Rust `Origin` enum uses the
  existing `text_enum!` macro (like `Condition`) and has a `label()`:
  "Domestic", "Imported", "Re-imported". Blank/absent is `NULL`, never a
  fourth value.
- **Stored country for Re-imported**: `NULL`. The United States is what
  the record *displays and searches* for a re-imported firearm (FR-002,
  FR-012), not something the user entered; storing it would create a value
  that has to be kept in step with the origin and would make an
  Imported→Re-imported change look like a data edit.
- **Related-field CHECKs**: the importer name and the three original marks
  may be non-null only when `origin IN ('imported', 'reimported')`; the
  country only when `origin = 'imported'`. These are backstops for FR-002,
  FR-004 and FR-014; the command layer produces the readable field
  errors first.
- **Rationale**: Matches how `condition` and `status` are modeled, keeps the
  human-testing coverage test satisfied (it requires every value of a
  `CHECK ... IN` list to appear in the seed), and makes "not specified"
  trivially distinguishable from every explicit answer.
- **Alternatives considered**: Two booleans (`is_imported`,
  `is_reimported`) (rejected: allows an impossible fourth state); storing
  "United States" for Re-imported (rejected as above); a separate
  `origin_kind` plus `origin_country` model (rejected: the spec's fixed
  three-way list is the requirement).

## 3. The identity rule and its database backstop

001's FR-032 is enforced twice: `check_uniqueness` in the command layer
(readable message naming the other record) and a partial **unique** index
`idx_firearms_active_identity` on `(make, model, serial_number)` for active
rows with a serial. The year exception (FR-008) makes the index wrong: a
pair is allowed only when both years exist and differ, and a unique index
cannot express that. SQLite treats NULLs as distinct in a unique index, so
adding `year_of_manufacture` to the index would silently allow two records
with no year — the exact case that must stay blocked — and it could still
not express "one record has a year and the other does not".

- **Decision**:
  1. The command layer stays the authority and produces the message.
  2. `idx_firearms_active_identity` becomes a plain (non-unique) lookup
     index with the same columns and `WHERE` clause.
  3. A pair of `BEFORE INSERT` and `BEFORE UPDATE` triggers on `firearms`
     `RAISE(ABORT, ...)` when the new/updated row is active with a serial
     and another active row has the same make, model and serial
     (`COLLATE NOCASE`) unless both years are non-null and differ. The
     lookup uses the index above, so import stays fast.
- **Rationale**: SC-008 requires 100% of create, edit, import and restore
  cases; a backstop that covers only some of the pairs would be a silent
  hole. The trigger states the same predicate as the application rule in
  one `WHERE`, and a test that bypasses the command layer (raw `INSERT`
  through the connection) proves it holds for null/null, equal-year and
  year/no-year pairs. `from_db` maps a raised ABORT to `INTERNAL_ERROR`,
  which is right for a backstop: reaching it means a bug, not user error.
- **Alternatives considered**: Two partial unique indexes (null-year pair,
  equal-year pair) (rejected: leaves the year/no-year pair unenforced,
  which is a third of the rule); dropping the DB backstop and relying on the
  command layer (rejected: 001 deliberately had a backstop and the same
  reasoning applies; import, restore and edit are separate code paths that
  a future change could bypass); an application-level lock or
  transaction-only check (rejected: the single connection is already
  serialized by `DbHandle`, but that guards races, not bugs).
- **Note on whitespace**: the index and trigger compare stored values, and
  `normalized()` already trims serial numbers. It does not trim `make` or
  `model`, which is why the command-layer query uses `lower(trim())`. That
  asymmetry already exists in 001 and is unchanged; the trigger uses the same
  column comparison the old unique index did, so it is no weaker than what it
  replaces.

## 4. The year predicate in the identity query and import matching

- **Decision**: One predicate, used in three places. Two firearms with the
  same main make, model and serial (ignoring case and surrounding
  whitespace) **conflict** unless both `year_of_manufacture` values are
  non-null and different. Expressed in SQL as
  `NOT (a.year IS NOT NULL AND b.year IS NOT NULL AND a.year <> b.year)`.
  It is applied by (a) `find_identity_clash` (create, edit, restore, and
  the import `duplicate_allowed` test through `check_uniqueness`), (b)
  `import_matching::find_match` (so an import row that differs by year is a
  *new record*, not a conflict — spec scenario US4-5a), and (c) the trigger
  in §3.
- **Message (FR-008)**: the existing text ("… already has this make, model
  and serial number. Change one of them, or dispose of or delete the other
  record.") gains a closing sentence: "Or record a year of manufacture on
  each firearm: two firearms with the same marks are accepted when both
  have a year and the years differ." It stays on the `serialNumber` field,
  where 001's message already appears, so the form needs no new wiring.
- **`find_match` and disposed rows**: unchanged in shape. It still prefers
  an active match and still returns a disposed match when no active one
  exists; the year predicate applies to both, so a disposed 1943 record does
  not "conflict" with an imported 1944 row.
- **Original-maker marks never take part**: none of the three sites reads
  `original_*` (FR-005, FR-014).
- **Alternatives considered**: Adding year to the match key as a plain
  fourth column (rejected: "both have a year and they differ" is not
  equality, and a missing year must still match); comparing only when the
  *existing* record has a year (rejected: FR-008 needs a year on both).

## 5. The original-marks warning (FR-009): transport, scope, cost

- **Decision — transport**: `create_firearm` and `update_firearm` gain an
  optional `confirmedWarnings: boolean` argument (default false), and
  `ReverseDispositionInput` gains the same optional field. When the record
  being saved is active, has original maker, model and serial all recorded,
  and another active firearm has the same three (ignoring case; stored
  values are trimmed by `normalized()`), the command returns the domain
  error code `ORIGINAL_MARKS_MATCH` whose `message` names the other record
  and nothing is saved. The frontend shows the message in a `ConfirmDialog`
  ("Save anyway") and resends with `confirmedWarnings: true`.
- **Rationale**: The check and the save happen in one backend call, so
  nothing can change between "warned" and "saved" (a separate pre-check
  command would leave a gap and a second code path to keep in step). It
  reuses the confirm-and-resend shape of `delete_firearm(confirmed)` and
  the existing `CommandError` type (no new field: code + message is
  enough, since the dialog only needs the sentence). Restore already runs
  through `update_firearm` inside a transaction, so a returned error rolls
  back both the history insert and the status change as it does for a
  FR-032 clash.
- **Decision — scope**: The check runs on every save of the record form
  (create, edit), on reversing a disposition, and on import rows. It does
  **not** run for the partial updates that start from the stored record and
  touch nothing identifying: `dispose_firearm` (the record leaves active
  status, and FR-009 considers only active firearms) and
  `assign_firearm_coverage`. Those call the internal save with the check
  disabled.
- **Trade-off, stated**: an edit that changes only the notes on a record
  whose original marks already match another active firearm warns again,
  because FR-009 says the check runs on edit and scenario US3-5 gives no
  "unchanged" carve-out. One click confirms it. Suppressing the warning when
  the three original values are unchanged would be a small later change and
  is not needed by any requirement.
- **Decision — import**: rows never prompt. The check runs before each row
  is saved and, on a match, adds `{ row, message }` to a new `warnings` list
  on `ImportResult` (and on `ResolveResult` for rows saved by a conflict
  resolution). The row still imports (FR-009, US4-6). Rows earlier in the
  same file count, because rows are saved in order.
- **Decision — cost**: The lookup is
  `original_serial_number = :s COLLATE NOCASE AND lower(original_make) …`
  against a new partial index `idx_firearms_original_serial`
  `(original_serial_number COLLATE NOCASE) WHERE status = 'active' AND
  original_serial_number IS NOT NULL`. Make and model are then compared on
  the few rows the serial selects. Stored values are trimmed and blank is
  `NULL`, so an exact `NOCASE` comparison equals the spec's "ignoring case
  and surrounding whitespace". A test at 10,000 records guards the
  Principle IV budget (import runs it once per row).
- **Alternatives considered**: A standalone `check_warnings` command the
  form calls before saving (rejected: TOCTOU gap, a second call per save,
  and import/restore would still need the in-line path); returning the
  warning in a success payload (rejected: the save would already have
  happened, but the spec requires confirmation *before* saving); a
  `fieldErrors`-style validation error (rejected: it reads as a block, and
  the form must distinguish "fix this" from "confirm this").

## 6. Search (FR-012): index the value as displayed

- **Decision**: Add seven columns to `firearms_fts`: `origin`,
  `year_of_manufacture`, `country_of_manufacture`, `importer_name`,
  `original_make`, `original_model`, `original_serial_number`. The insert,
  update and delete triggers write, for `origin`, the display label via a
  `CASE` ("Domestic", "Imported", "Re-imported", `NULL`), and for the
  country, `'United States'` when `origin = 'reimported'` and the stored
  country otherwise. The year is written as text.
- **Why the label gives the spec's behavior with no special code**: The
  index is trigram-tokenized (issue #46; it was FTS5's word tokenizer with
  a trailing prefix `*` here). The search box sends its text as a quoted
  phrase matched as contiguous text, so "imported" matches both "Imported"
  and "Re-imported", "re-imported" matches only Re-imported, and
  "domestic" matches only Domestic, exactly as spec Clarifications and
  US1-4 require. A firearm with no origin writes `NULL`, so there is nothing
  to match.
- **Consistency guard**: the origin label exists in three places (the SQL
  `CASE`, `Origin::label()` in Rust, `ORIGIN_OPTIONS` in TypeScript). One
  Rust test creates a firearm per origin and asserts that searching each
  `label()` finds it, so the SQL and Rust copies cannot drift. A Vitest
  test asserts the TypeScript labels equal the strings the record page
  shows. A shared abstraction for three short string tables would cost more
  than it saves.
- **External-content note**: like `firearm_type_name` today, some indexed
  values are not columns of `firearms` under that name. `delete` commands
  supply the old values explicitly, so the index stays consistent; a
  `rebuild` was already unsupported for the joined type name and remains so.
- **Not searchable, per spec**: nothing. Year of manufacture is searchable
  but not groupable (FR-012).

## 7. Year of manufacture validation

- **Decision**: An integer, `1400 <= year <= current local year`, stored in
  `year_of_manufacture INTEGER CHECK (year_of_manufacture IS NULL OR
  year_of_manufacture BETWEEN 1400 AND 9999)`. The upper bound in the DB is
  only a sanity ceiling because "the current year" changes; the command layer
  applies the real bound against `chrono::Local` (as acquisition and
  disposition dates already do, FR-003). One message covers every failure:
  "Year of manufacture must be a four-digit year from 1400 to {current
  year}." A value that is not four digits (for example `43` or `19430`) and
  one that is out of range are the same error, since the range already
  implies four digits.
- **Input handling**: The form uses a numeric text input with the same
  numeric-only handling as `capacity`; the backend receives `number | null`.
  On import the cell goes through the existing `parse_scaled_decimal(…, 0)`
  used by `capacity`, so `1943`, a numeric spreadsheet cell `1943`, and
  `1943.0` are accepted, and `circa 1943`, `43`, or `1943.5` are row errors
  naming `year_of_manufacture` (FR-014, US4-7).
- **Rationale**: Reuses the established validation shape (`checked_*` helpers
  and `parse_scaled_decimal`), needs no new date logic, and keeps
  uncertainty ("circa") in free-form notes as the spec's Edge Cases require.
- **Alternatives considered**: A `TEXT` year (rejected: the FR-008 comparison
  and range check are numeric); a full date (rejected: the spec asks for a
  single year).

## 8. Changing origin discards hidden values (FR-010)

- **Decision**: The confirmation is a frontend `ConfirmDialog`, shown at the
  moment the user picks a new origin when at least one value that the new
  origin would not offer is non-blank (importer name, country, or any
  original mark; for Imported→Re-imported, the country only; between Imported
  and Re-imported the importer and original marks carry over silently).
  Confirming clears those form fields at once; cancelling leaves the origin
  as it was. Saving then sends a consistent record.
- **Backend stance**: `validate_firearm_input` rejects, with field errors,
  any importer, country or original-marks value that the record's origin does
  not allow (and the DB CHECKs back it). So the backend never has to guess
  whether a cleared field was intentional, and a stale or hand-built request
  cannot keep hidden data. It does not require a separate confirmation flag:
  the discard is the user's edit of the form, visible before Save, not a
  delete action on a stored record.
- **Rationale**: Confirming at selection time means the form never shows or
  submits values the current origin hides, and matches how a destructive
  choice in a form is normally confirmed. It uses the shared confirmation
  pattern (Principle III).
- **Alternatives considered**: Hiding the fields but keeping their values in
  form state until Save, and confirming at Save (rejected: hidden state is
  easy to lose track of and makes the record's contents differ from what the
  form shows); a backend `confirmedDiscard` flag comparing the stored record
  with the request (rejected: extra contract surface for something the
  spec's scenario 6 places at the form, and it would break import, where
  clearing is not a user action).

## 9. Spreadsheet columns and origin values

- **Decision**: Seven columns after `condition` and before `photo_filenames`,
  named like the existing ones: `origin`, `year_of_manufacture`,
  `country_of_manufacture`, `importer_name`, `original_make`,
  `original_model`, `original_serial_number`. Export writes the origin as its
  display label (`Domestic`, `Imported`, `Re-imported`, as `condition` is
  written) and a blank country for Re-imported; the year as plain digits.
  Import matches the origin case-insensitively against exactly those three
  labels (FR-014); anything else, including `re_imported` or `reimported`, is
  a row error naming the value. The original marks and the importer are
  rejected on a row whose origin does not offer them, and the country is
  also rejected on a Re-imported row.
- **Rationale**: A fixed, documented vocabulary keeps the spec's row-error
  rule simple and testable. `condition` accepts stored-style spellings
  because its stored values are unlike its labels; here the label and the
  intent are the same words, so no alias is needed. No column is required;
  every one may be blank (FR-014, Edge Cases).
- **Alternatives considered**: Accepting `imported`/`reimported` aliases
  (rejected: the spec lists three values and one spelling each); exporting
  the stored value (rejected: `reimported` is not what a user reads).

## 10. Grouping by origin

- **Decision**: `GroupBy` gains `origin`. The group key is the origin label,
  with `Not specified` for firearms with no origin. Origin groups are
  returned in a fixed order — Domestic, Imported, Re-imported, Not
  specified — instead of the alphabetical sort used for type, caliber and
  make, so the "no origin" group is always last rather than between
  Imported and Re-imported. Only origins present in the result appear.
- **Rationale**: FR-012 requires a distinct group for firearms with none;
  putting it last is the ordinary reading of "everything else", and the
  fixed order costs one small `match` in `list_firearms`.
- **Alternatives considered**: Alphabetical order (rejected: puts "Not
  specified" in the middle); a `NULL` key (rejected: the frontend renders
  keys as headings).

## 11. The "how to record it" guide (FR-015)

- **Decision**: A static React component, `OriginGuide.tsx`, opened in the
  shared `Dialog` from a "How do I record this?" button beside the origin
  control (and reachable by keyboard from it). Its worked examples are
  authored as data in the component (title, the paperwork/stamps the user
  sees, and the fields to fill in), not fetched or generated. The required
  cases are in [contracts/ui-identification.md](./contracts/ui-identification.md):
  a re-imported M1 Carbine; importer-adopted marks (an Austrian pistol
  stamped by its U.S. importer); an importer-assigned serial with the
  original serial kept as an original mark; a pre-1968 surplus import with
  only the maker's marks; wartime duplicate serials (record the actual
  manufacturer as the make, and any suffix as stamped); and a pre-1968
  domestic revolver whose maker restarted numbering (record a year on each).
- **Rationale**: The guide is fixed reference text with no data behind it,
  must work offline, and must be reachable from the control (FR-015); a
  component satisfies all three and can be unit-tested for its content and
  focus behavior. It is guidance only and never blocks, rejects or
  second-guesses a choice.
- **Alternatives considered**: A bundled markdown file rendered at runtime
  (rejected: adds a markdown renderer or a fetch path for one screen); an
  external web page (rejected: the app is offline and Principle V allows no
  network by default); tooltips (rejected: not reachable by keyboard or
  screen reader as reliably, and too long).

## 12. Human-testing seed and coverage

- **Decision**: `examples/human_seed.rs` gains records exercising every new
  column and every `origin` value: a re-imported M1 Carbine, an imported
  pistol with importer-adopted marks, an imported firearm with an
  importer-assigned serial and full original marks, a matching original-marks
  pair (so the warning can be seen), a pair of pre-1968 revolvers with the
  same marks and differing years, and import-sample rows carrying all seven
  new columns. `NEVER_SEEDED` stays empty.
- **Rationale**: `human_seed_coverage_test` fails when a column is empty in
  every row, when a `CHECK ... IN` value never appears, or when an import
  sample leaves a spreadsheet column blank. Without this, a person testing
  by hand would never see the feature.

## 13. Documenting the amendments to 001

- **Decision**: The 002 data model and both contracts are written as deltas
  naming the 001 anchor they change (FR-030, FR-032, FR-026, FR-033,
  FR-012/013, the `Firearm` table, `list_firearms`, `create_firearm`,
  `update_firearm`, `reverse_disposition`, `import_collection`, the
  spreadsheet columns). 001's files are not rewritten. The last task adds a
  one-line "amended by 002" pointer at each of those anchors.
- **Rationale**: Corrections to 001 edit it in place; new-capability specs
  *extend* it and, per the planning convention in the spec's
  Source Request, must list which 001 requirements and decisions they
  amend, which the spec's Relationship section does.
  Keeping deltas next to the spec that owns them means each feature's folder
  stays a faithful record of what that feature decided.
- **Alternatives considered**: Rewriting 001's data model and contracts in
  place (rejected: it would hide which rules belong to which feature and
  make 001's tasks.md, which is complete, disagree with its own design
  documents).
