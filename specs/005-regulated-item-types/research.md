# Phase 0 Research: Regulated Item Types, Suppressors and NFA Registration

The Technical Context has no open unknowns: the stack is feature 001's and
no dependency is added. This document records the decisions the spec left to
planning (how a type marks the fields that don't apply to it, where the
classification list lives, how registration is stored, the grouping control's
design), the ones the codebase forced (the frontend's hard-coded type list,
§3; `EntryField` naming its IPC field by its column, §6), and the legal
background the spec asked to be checked (§1), each with its rationale, so
nothing is left as NEEDS CLARIFICATION going into Phase 1.

## 1. Legal background, checked at planning (spec Assumptions)

Not legal advice. Checked on 2026-09-30 so the plan knows what the
application must stay neutral about.

- **Public Law 119-21** (the 2025 budget reconciliation act) set the NFA's
  making and transfer tax to $0 for suppressors, short-barreled rifles,
  short-barreled shotguns and any other weapons from 1 January 2026. Machine
  guns and destructive devices keep the $200 tax. This matches the spec's
  assumption.
- **Since the spec was written, the law has moved again.** On 5 August 2026,
  in *Silencer Shop Foundation v. ATF* (N.D. Tex., consolidated with *Jensen
  v. ATF*), the court held that the NFA's registration and approval
  requirements can't stand for suppressors, short-barreled rifles and
  short-barreled shotguns once their tax is $0. The injunction took effect on
  13 August 2026. It is **party-specific, not nationwide**: it protects the
  plaintiffs and the members and customers of named organizations and
  companies, and 15 plaintiff states' agencies. The Department of Justice
  has said it won't appeal. Other suits (including one by Missouri, filed in
  September 2026) and bills to remove these items from the Act are pending.
- **What this means for the design**: nothing changes, and the spec's
  record-only stance (FR-014) is confirmed. The same suppressor may now be
  registered for one owner and unregistered for another, depending on
  whether the owner is covered by an injunction. The design already records
  a suppressor with no classification as a valid record (Edge Cases). It
  keeps a registration recorded before the ruling unchanged (FR-015). It
  never infers a classification from the type (FR-008). The application
  holds no rule that a court ruling could make wrong: only the names of the
  classifications and the forms. The standing note's "laws change" wording
  (contracts/ui-registration.md §3) stays as the spec gives it. It names no
  law, ruling or date, because any such detail would go out of date inside
  the application.
- **The approved date's hint** keeps "(the tax stamp date)" as FR-009 says,
  because owners know the date by that name. The field itself is labelled
  "Approved", not "Tax stamp date", because a $0 approval has no tax to
  stamp (spec Assumptions).

Sources: [Brown v. ATF (Wikipedia)](https://en.wikipedia.org/wiki/Brown_v._ATF),
[American Suppressor Association](https://americansuppressorassociation.com/news/federal-court-strikes-down-nfa-registration-for-suppressors),
[Silencer Shop Foundation](https://ssf.org/blogs/news/what-silencer-shop-foundation-v-atf-win-means-for-you),
[Wiley](https://www.wiley.law/alert-NFA-Court-Ruling-Reshapes-Compliance-Following-Wileys-Successful-Constitutional-Challenge),
[Texas Gun Rights](https://texasgunrights.org/trump-directs-doj-not-to-appeal-nfa-ruling-leaving-major-suppressor-injunction-intact),
[AmmoLand, Missouri suit](https://www.ammoland.com/2026/09/missouri-sues-atf-nfa-registry-suppressors-sbrs/).

## 2. Fields that don't apply to a type: three flags on `firearm_types` (FR-003)

- **Decision**: `firearm_types` gains three columns,
  `action_type_applies`, `barrel_length_applies` and `capacity_applies`,
  each `INTEGER NOT NULL DEFAULT 1 CHECK (… IN (0, 1))`. Suppressor is
  seeded with all three at 0, and every other type with 1. The rule is
  enforced three times, the same way as 004's action rule:
  1. **The command layer**, in `check_fields_apply(conn, input)`. It runs in
     `create_firearm` and `update_firearm` before `check_action_allowed`, and
     on each import row. It returns a `VALIDATION_ERROR` with a field error
     per offending field: "Action doesn't apply to a Suppressor."
  2. **A trigger pair** on `firearms`, `firearms_fields_apply_insert` and
     `firearms_fields_apply_update`, as the backstop. `from_db` maps them to
     `INTERNAL_ERROR`, as for the other backstops.
  3. **The form**, which doesn't offer the fields (contracts/ui-registration.md
     §1).
- **Why the check runs before the action mapping**: Suppressor has no rows in
  `firearm_type_actions`, which 004 reads as "every action allowed"
  (004 FR-017). The spec keeps that meaning for a type with no mapping and
  asks for a separate "no action at all" case. Checking the flag first means
  the mapping never has to express "none".
- **Rationale**: Three named flags are the smallest structure that the
  trigger, the command and the frontend can all read without interpretation.
  The spec names exactly three fields, and overall length, weight, finish and
  condition apply to every type. A general table of omitted fields
  (`firearm_type_omitted_fields (type, field)`) would describe fields that
  never need it, and it would force the trigger to compare field names as
  strings. The flags default to 1, so a type added later accepts everything
  unless it opts out.
- **The form clears on save, not on change** (FR-004, spec Edge Cases). The
  hidden values stay in the form state until save, so switching Rifle →
  Suppressor → Rifle before saving brings them back. The save builds the
  input with them cleared when the type in effect at save omits them. This
  differs from 004's action clearing, which clears at once, because here the
  spec asks for a notice of what "will be cleared" and for the check "against
  the type in effect at save".
- **Alternatives considered**: Encoding "no action" as a special
  `firearm_type_actions` row (rejected: it changes 004's reading of the
  table, and barrel length and capacity have no table to encode anything
  in). Hard-coding "type 5 has no action" in Rust and TypeScript (rejected:
  it duplicates a rule the database can hold, which 004 avoided for the
  action mapping). Silently dropping the values in the backend (rejected:
  FR-022 wants import to report them, and the form tells the user).

## 3. The frontend reads firearm types from the backend (`list_firearm_types`)

- **Decision**: A new read command, `list_firearm_types`, returns each type's
  id, name, drawing key and three flags. `CollectionProvider` loads it once
  per open database, beside `list_action_types`, with the same failure flag.
  `FIREARM_TYPE_OPTIONS` in `src/features/firearms/types.ts` is removed.
  `firearmTypeOption` reads the store, and still falls back to "Other" and its
  drawing for an id it doesn't know.
- **Rationale**: The type list is hard-coded in the frontend today (four
  entries, three users). Adding Suppressor there would copy the seed, and
  the flags of §2 would copy a rule. 004 added `list_action_types` "so the
  frontend never copies the mapping", and the same reasoning applies here.
  The command is cheap: five rows, loaded once.
- **Alternatives considered**: Adding a Suppressor entry with its flags to
  `FIREARM_TYPE_OPTIONS` (rejected: two copies of the seed and the rule).
  Folding the types into `list_action_types`' output (rejected: it mixes two
  lists, and the output's name would no longer say what it holds).

## 4. The Suppressor type and its drawing (FR-001, FR-002)

- **Decision**: `0003_seed_firearm_types.sql` seeds the types with explicit
  ids that never change, and gives `firearm_types` a `sort_order` that lists
  them: 1 Handgun, 2 Rifle, 3 Shotgun, 4 Suppressor, 5 Other (the
  catch-all last). Suppressor keeps **id 5** and Other **id 4**, with drawing
  key `suppressor`. The caliber is
  labelled "Caliber rating" for a Suppressor on the form and the record
  page. On the form it has the hint "The largest bore the suppressor is rated
  for." The label and hint depend on the type's name being Suppressor, not on
  a flag. That is a display choice for one seeded type, and it has no rule
  behind it.
- **The drawing**: a new `suppressor` entry in `DRAWINGS`
  (`src/features/browse/typeDrawings.ts`). It is a side elevation, mount end
  to the left, in the same 320×200 box and the same part/open/detail roles
  as the others, but drawn as a **partial section**: the tube is cut away
  between two break lines to show a hatched wall, a blast chamber and a
  generic cone-baffle stack (a sleeve of skirts with a cone at each joint,
  the blast baffle heavier than the rest). The mount end has a threaded or
  quick-detach collar with wrench flats; the front end is a flush rounded
  end cap with no front cap drawn. The tube's length-to-diameter ratio is
  about 6:1, typical of a .30-caliber rifle can, centred vertically on the
  bore axis. Like the others, it is traced from side-on
  photographs of real models for proportion (spec FR-001: drawn for the
  project). It carries no brand marks. The file's header comment records
  that. The drawing is licensed GPL-3.0-only with the rest of the source
  (constitution, Licensing). The empty-state draw-in animation
  (`TypeDrawing`) uses it unchanged. The branding notes' cautions apply:
  separate subpaths for each stroke, and round caps started past 1.
- **Why the cutaway**: a plain tube with a cap is indistinguishable from a
  flashlight or a pipe at thumbnail size. The baffle stack is what makes a
  suppressor recognizable, and the break lines keep it a drawing of the
  object, not a schematic.
- **Alternatives considered**: Reusing the "other" drawing (rejected by
  FR-001). An exterior-only elevation with a front cap (the first draft;
  superseded by the cutaway for the reason above).

## 5. The classification list: a seeded lookup table with an `offered` flag (FR-007, FR-015)

- **Decision**: A new table, `registration_classes (id, name UNIQUE,
  sort_order UNIQUE, offered 0/1)`, is seeded with fixed ids in
  `0003_seed_firearm_types.sql`: 1 Suppressor, 2 Short-barreled rifle,
  3 Short-barreled shotgun, 4 Any other weapon, 5 Machine gun,
  6 Destructive device, all offered. `firearms.registration_class_id`
  references it. No command writes it. A later build adds an entry or stops
  offering one through a migration, which is also what makes an older build
  refuse the file as `NewerVersion` instead of meeting an id it doesn't
  know.
- **`offered` is a choice filter only**: The backend accepts any known
  classification on create, update and import, offered or not (FR-021
  explicitly matches entries no longer offered). The form offers the offered
  entries plus the one the record already holds (FR-007). That is enough for
  "saving never forces a change" (SC-004). Refusing a non-offered entry on
  create would add a rule the spec doesn't ask for, and it would make import
  and the form disagree.
- **Rationale**: This is the same shape as `action_types`: fixed ids, a sort
  order for the choice and the groups, and a name that export writes, import
  matches and search indexes. The foreign key makes an unknown value
  impossible, and the table carries no "regulated" flag (spec Key Entities).
  A Rust enum stored as text, like origin, was the other candidate. It would
  let a newer build add a value with no migration, but then an older build
  would fail to read the record instead of refusing the file cleanly.
- **SC-004 is testable**: a test sets `offered = 0` with raw SQL, then
  opens, edits and saves a record holding that classification.
- **Alternatives considered**: A text enum (rejected, above). A
  `FirearmType`-style table users can extend (rejected by the spec's Source
  Request answer: the list is fixed).

## 6. Registration details: four columns on `firearms` (FR-009 to FR-013)

- **Decision**: `firearms` gains `registration_class_id` (INTEGER, FK, NULL =
  none), `registration_form` (TEXT), `registration_approved` (TEXT,
  `YYYY-MM-DD`) and `registered_to` (TEXT). A table `CHECK` requires the
  three details to be NULL when the classification is NULL.
- **Validation** (in `validate_firearm_input`, so import shares it):
  - Details with no classification: a field error on each detail, "Choose
    what the firearm is registered as first." (Only reachable through a
    bad caller or import; the form doesn't offer them.)
  - `registrationApproved`: through the existing `checked_date`, so "Approved date
    can't be in the future." and the format message, judged against the
    local date (FR-010).
  - `registrationForm` and `registeredTo`: 004's entry rules
    (`check_entry_text`: at most 100 characters, no control characters),
    applied on update only when the value changed, as for make and model.
  - `registrationClassId`: an id that names no classification gives "Choose
    a classification from the list." from a command-layer lookup, before the
    FK could fail.
- **Normalization**: `FirearmInput::normalized` trims both text details and
  makes a blank one `None`, as for cartridge.
- **Kept through disposition and restore** (FR-013): the details are ordinary
  `Firearm` fields, and `From<&Firearm> for FirearmInput` carries them, so
  `dispose_firearm`, `reverse_disposition` and `assign_coverage` keep them
  with no extra code. This is tested.
- **Deleted with the firearm** (FR-013, SC-007): they live in the firearm's
  row and its FTS entry, which the existing delete, `secure_delete` and
  reclaim path already wipes. `deletion_wipe_test.rs` gains a unique
  "Registered to" value that must be gone from the file's bytes.
- **Alternatives considered**: A separate `registrations` table, one row per
  firearm (rejected: the spec settles one registration per record, the
  firearm row already holds every other per-firearm detail including 002's
  importer details, and a second table would need its own triggers,
  cascade and backup tracking for no gain).

## 7. Form and "Registered to": two more suggestion fields (FR-009, FR-013, FR-021)

- **Decision**: `EntryField` gains `RegistrationForm` and `RegisteredTo`.
  `FieldVocabulary::load`, `snap`, `suggest`, `SheetSpellings` and
  `snap_for_import` then work for them unchanged. The built-in form names
  (Form 1, Form 4, Form 5) are a `const` slice in a new
  `services::registration` module. `known_spelling` and `suggest` treat them
  as the catalog for `RegistrationForm`: they are offered with the "built-in"
  marker and snapped to first (`ChangedBy::Catalog`). "Registered to" has no
  built-in list.
- **Two naming fixes the codebase forces**:
  - `EntryField::column()` is documented as "the `firearms` column, which is
    also the IPC field name and the spreadsheet column". That holds for the
    four one-word fields but not for `registration_form`. A new
    `EntryField::ipc_name()` returns `registrationForm` and `registeredTo`,
    and `validate_firearm_input` keys field errors by it. `column()` stays the
    database and spreadsheet name.
  - `#[serde(rename_all = "lowercase")]` becomes `"camelCase"`, which leaves
    `make`, `model`, `cartridge` and `caliber` unchanged on the wire and gives
    `registrationForm` and `registeredTo`.
- **`EntryField::ALL`** grows to six. Its users, `validate_firearm_input` and
  the import's `Snapping`, cover the new fields with no further change.
  `settle_row` adds them to its loop of optional fields. Caliber keeps its
  own branch. `required()` stays false for both.
- **Indexes**: `idx_firearms_registered_to` and
  `idx_firearms_registration_form` are partial indexes (`WHERE … IS NOT
  NULL`). They cover the vocabulary's `GROUP BY` for a handful of rows among
  10,000, and the grouping pass in `list_firearms`. The performance test
  holds `suggest_entries` for both fields to 004's 50 ms.
- **Rationale**: FR-009 asks for the suggestions "in the same way" as 004's.
  Reusing the one entry key and snapping path means the form, `settle_entry`
  and import can't drift. That was 004's reason for building them around one
  function.
- **Alternatives considered**: A separate suggestion command for
  registration (rejected: it duplicates 004's ranking). Treating the form as
  a fixed choice (rejected by FR-009: free text, because ATF form numbering
  has changed before and eForms have their own names).

## 8. Grouping and search (FR-016, FR-017)

- **Decision — grouping**: `GroupBy` gains `RegisteredAs` and `RegisteredTo`
  (`"registered_as"`, `"registered_to"`). `list_firearms` reads `rc.name`
  and `f.registered_to` through a `LEFT JOIN registration_classes`.
  - "Registered as" groups are in the list's `sort_order`, with "Unspecified"
    last, as for action types.
  - "Registered to" groups are keyed by the stored text (spelling variants
    already on record stay apart, as for cartridge, and snapping keeps new
    ones from arising), sorted alphabetically, with "Unspecified" last. A
    firearm with no classification has no "Registered to" and so falls under
    "Unspecified" (FR-016).
- **Decision — summary**: `FirearmSummary` gains `registeredAs:
  string | null`. The browse list and tiles don't show it (FR-018). It is
  there for the export dialog's disclosure (§10), which already has the
  summaries in hand.
- **Decision — search**: `firearms_fts` gains `registered_as` (the
  classification's name, by subquery, as for the action's name),
  `registration_form` and `registered_to`. The short-query `LIKE` branch in
  `list_firearms` gains the same three. "short-barreled" finds "Short-barreled
  rifle", "form 1" finds "Form 1", and "smith family" finds "Smith Family
  Trust" (US2-11). The approved date is not searched (FR-017 doesn't name
  it), and nothing else about search changes.
- **Performance**: grouping reuses the one list query with one more
  `LEFT JOIN` on a six-row table. Search adds three columns to the trigram
  index. `performance_test.rs` gains grouping by both fields and a
  registration search at 10,000 records against the 1 s and 500 ms budgets
  (SC-006).

## 9. The grouping control: a sectioned radio menu (FR-016, spec Assumptions)

The spec requires this to be designed with the frontend design skill rather
than by adding two more buttons. The collection page is the only screen that
offers a grouping choice, so there is nothing else to carry it to.

- **Problem**: the toolbar's `SegmentedControl` already holds seven joined
  buttons (None + six fields). Nine won't fit beside the search box at the
  window's minimum width, and a row that long is read left to right, one
  button at a time, to find one word.
- **Design brief, as the design pass framed it**: the job is a one-in-nine
  choice made occasionally and read often. The current choice has to be
  visible at a glance. The other choices only need to be findable. The app's
  identity is a museum catalogue of arms: blueprint grid, Niter-blue
  controls, a bronze owl. The control lives inside that system (constitution
  III). It gets no palette or typeface of its own. What makes it this app's
  control is how the choices are sorted. A catalogue has indexes, so the
  fields are grouped by what they describe instead of listed flat.
- **Decision**: a toolbar button that reads **Group by: Caliber** (label in
  `--ink-2`, value in `--ink` at weight 650, a chevron). It opens a menu of
  radio items in three sections headed in sentence case:
  - **The firearm**: Type, Action, Caliber, Cartridge
  - **Its maker**: Make, Origin
  - **Registration**: Registered as, Registered to

  **None** heads the menu on its own, above the first section. It is the one
  way to ungroup. The chosen item carries a Niter radio dot in the leading
  gutter, and the button reads "Group by: None" when nothing is chosen. The
  full layout, roles and keys are in contracts/ui-registration.md §6.
- **Reviewed against the generic default**: a plain `<select>` would work,
  and a dropdown is the conventional answer to "too many options". What this
  design adds is the sectioning, which is specific to what these fields are.
  That is also the part that keeps the menu scannable as more fields arrive.
  The design deliberately adds no icons per field, no counts per option and
  no animation beyond the menu's own open and close. Those would be
  decoration on a utility control.
- **Built on shared primitives**: the existing `Menu` (Radix
  `DropdownMenu`) gains `MenuRadioGroup`, `MenuRadioItem` and `MenuLabel` in
  `src/components/Menu.tsx`. Radix gives `menuitemradio` roles, arrow keys,
  typeahead and Escape back to the button. The segmented control stays for
  short choices (List/Tiles), so no pattern is lost (constitution III).
- **Alternatives considered**: A segmented row with a "More" overflow
  (rejected: it hides the choice the user made whenever that choice is in
  the overflow). Two rows of buttons (rejected: it takes vertical space from
  the list on every visit). A native `<select>` with `<optgroup>` (workable,
  but it renders differently on each platform's WebView, and its popup
  ignores the app's theme in WebKitGTK).

## 10. Export and import (FR-019 to FR-022)

- **Columns**: `registered_as`, `registration_form`,
  `registration_approved`, `registered_to`, placed after
  `original_serial_number` and before `photo_filenames`, so the
  identification and registration columns sit together. Import reads by
  header (004), so position matters for export only. `COLUMNS` becomes 40.
- **Export**: `registered_as` is the classification's name, and
  `registration_approved` is `YYYY-MM-DD` as the other dates are. All four
  are blank when there is no value.
- **Disclosure** (FR-020): the export dialog's note adds "and registration
  details" when any firearm in the chosen scope has a classification. For
  "all", that comes from the collection store's summaries (active and
  disposed). For "filtered", it comes from the `list_firearms` result the
  dialog already fetches. The dialog counts a classification alone as
  registration details: the spec's trigger is "registration details", and a
  classification such as "Machine gun" is at least as sensitive as a form
  number. Disclosing on the broader condition fails safe.
- **Import** (in `parse_row` and `settle_row`):
  - `registered_as`: matched by name `COLLATE NOCASE` after trimming, among
    all classifications, offered or not. Unknown gives "registered_as:
    unknown classification "…"".
  - Details with a blank `registered_as`: "registered_as: Registration
    details need a classification."
  - `registration_approved`: read as `acquisition_date` is read (the same
    cell reading and `checked_date`), so a future date is a row error.
  - `registration_form` and `registered_to`: checked with
    `check_entry_text`, then snapped (catalog, the database's values when the
    import started, the sheet) and listed in `snappedValues` (FR-021).
  - FR-022: before `check_action_allowed`, a row whose type omits a field and
    has a value in it fails with a message naming the spreadsheet column:
    "action_type: Action doesn't apply to a Suppressor.",
    "barrel_length_in: …", "capacity: …". `parse_row` maps
    `check_fields_apply`'s IPC field names to column names for this.
- **A pre-feature sheet** has none of the four columns. They read as blank,
  and the rows import with no classification (US4-7), with no new code.

## 11. The form: where registration sits, and what clearing asks (FR-009, FR-012, FR-014)

- **Decision**: a new folded section, **Registration**, after "Origin and year
  of manufacture" and before "Physical details". It follows the same
  `Disclosure` rules: optional, closed unless something is recorded, opened
  by an error inside it, and a one-line summary when closed
  (contracts/ui-registration.md §2). It
  sits next to identification because registration describes the item's
  paperwork, as origin does. The "Registered as" select is always there. The
  form, approved date and "Registered to" appear only once a classification
  is chosen (FR-009). The standing note and the guide link sit at the top of
  the section's body. Layout details are in contracts/ui-registration.md §2.
- **Clearing** (FR-012): choosing "Unspecified" while any detail has a value
  opens the shared `ConfirmDialog`, worded like origin's discard ("Clearing
  the classification will discard the form "Form 4", the approved date and
  "Registered to". It can't be recovered once saved."). Changing to another
  classification keeps the details with no question.
- **Draft version**: `FormState` gains `registrationClassId`,
  `registrationForm`, `registrationApproved` and `registeredTo`, so
  `FORM_VERSION` becomes 3. Older drafts are discarded, as before.
- **UI consistency** (constitution III, CLAUDE.md): the section reuses the
  folded-section, field-width (`hd-field--third` for the date,
  `hd-form-grid--*`) and confirm patterns already in FirearmForm. No other
  form gains fields, so nothing needs carrying over. DisposeDialog,
  RestoreDialog and CoverageDialog keep the details through
  `From<&Firearm>` without showing them.

## 12. The guide gains a registration part (FR-014)

- **Decision**: `OriginGuide` becomes `IdentificationGuide`. It is still one
  `Dialog`, now titled "How to record where a firearm came from and how it's
  registered", with two headed parts: the six origin examples, and
  **Registered items** with two examples:
  1. *Suppressor bought on a Form 4, registered to a trust*: type Suppressor;
     caliber rating ".30"; registered as Suppressor; form "Form 4"; approved
     is the date on the approved form; registered to the trust's name as it
     appears on the form; attach the approved form as a document.
  2. *Rifle made into a short-barreled rifle on a Form 1*: type stays Rifle;
     make and serial number as marked on the firearm; registered as
     Short-barreled rifle; form "Form 1"; approved date; registered to the
     owner. If several uppers are used on the receiver, list them in notes.

  The guide opens scrolled to the part it was opened from: the origin
  trigger opens it at the top, and the registration section's link opens it
  at "Registered items". The disclaimer at the top now says: "Record what is
  stamped on the firearm and what your paperwork says. HoploDex doesn't
  check it against any rules or decide what is regulated."
- **Rationale**: FR-014 requires 002's guide to gain the examples, not a
  second guide. Opening at the right part keeps it one click from either
  section.

## 13. "Automatic or select-fire" (FR-006)

- **Decision**: seeded as action id **13** with `sort_order` 7: after the
  six common actions (Semi-automatic, Revolver, Bolt, Lever, Pump and Break
  action) and before the single-shot and muzzleloading ones. Few collectors
  own one, so it shouldn't sit above the actions most firearms have. The
  sort orders from Falling block on move down one (ids unchanged, so an id
  still means the same action in every build; the first six keep 004's
  order). It is mapped to Handgun, Rifle and Shotgun. Other has no
  rows and allows everything. Suppressor omits the action (§2).
- Nothing relates it to a classification (FR-008), and a test saves every
  combination.

## 14. Privacy, deletion and backups (FR-013, FR-023, constitution V)

- Every new value lives in the encrypted database, in `firearms` and its FTS
  entry. No new file, cache or setting holds any of it, and the feature adds
  no network access.
- `registration_classes` gets the three `*_marks_backup_due_after_*` triggers
  (`backup_due_tracking_test` requires them), and
  `human_seed_coverage_test`'s `is_user_table` skips it as a seeded lookup.
  A firearm's registration change is already covered by the `firearms`
  triggers.
- Deleting a firearm removes its details through the existing path. The
  suggestion list reads `firearms` at request time, so a "Registered to"
  value used only by deleted firearms is never suggested (FR-013, SC-007).
- The export disclosure (§10) covers the one route by which details leave
  the database unencrypted. Backups stay encrypted (feature 003).
