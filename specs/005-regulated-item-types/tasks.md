---

description: "Task list for feature 005: regulated item types, suppressors and NFA registration"
---

# Tasks: Regulated Item Types: Suppressors and NFA Registration

**Input**: Design documents from `/specs/005-regulated-item-types/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (tauri-commands.md, spreadsheet-format.md, ui-registration.md), quickstart.md (all present)

**Tests**: The project constitution's Testing Standards principle is NON-NEGOTIABLE: every user story below includes test tasks written first (integration tests run the real `ops` against a temporary SQLCipher database, no mocks), expected to fail, then implementation makes them pass.

**Organization**: Tasks are grouped by user story (spec.md's priorities P1–P4) so each story can be implemented and tested on its own. This feature is a **delta** against `specs/001-firearms-inventory/` as amended by 002 and 004: most tasks edit existing files (plan.md's Project Structure), and the schema is edited in place in migrations 0001–0003 (CLAUDE.md). No new migration file, dependency, layer or error code.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Which user story this task belongs to (US1–US4)
- Exact file paths are given per plan.md's Project Structure
- Work that must be built, tested or run on a specific platform gets one task per platform

## Path Conventions

Existing Tauri desktop app: Rust backend in `src-tauri/`, React/TypeScript frontend in `src/`, WebdriverIO E2E suite in `e2e/`. Run tests, lint and screenshots through `scripts/dev-container.sh` (CLAUDE.md; commands in quickstart.md). Never open the real databases (CLAUDE.md, "Never touch the real databases").

## How the stories divide the shared pieces

- **Foundational** owns every schema change except the new action (the four `firearms` columns, their `CHECK`, the type flags and their trigger pair, the `registration_classes` table and seed, the FTS columns) and the model fields, so every story can write records that hold them.
- **US1** owns the fields rule (`check_fields_apply`), `list_firearm_types` and the frontend's switch from the hard-coded type list to the store, the suppressor drawing, and the form's and record page's type-dependent fields.
- **US2** owns everything about registration above the schema: validation, `list_registration_classes`, the two new `EntryField`s and the built-in form names, grouping and search, the Registration section and panel, the guide, and the grouping menu (with the shared `Menu` radio items).
- **US3** is only the new action: its seed row, sort orders and mappings. It is independent of US1 and US2 apart from the Foundational schema.
- **US4** extends export and import, and reuses US1's fields rule and US2's validation and snapping.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Empty modules so later tasks in different files can proceed in parallel.

- [X] T001 Create the new backend modules and declare them: `src-tauri/src/models/firearm_type.rs` and `src-tauri/src/models/registration.rs` in `src-tauri/src/models/mod.rs`, and `src-tauri/src/services/registration.rs` in `src-tauri/src/services/mod.rs`, each with a module doc comment citing its source (research.md §3 for `firearm_type`, §5 for `models::registration`, §7 for `services::registration`: "the built-in form names, used only as suggestions for `registration_form`; nothing here is stored"). `cargo build --manifest-path src-tauri/Cargo.toml` must pass

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The schema and the base model shape that every story is built on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Schema (migrations edited in place; no new migration file)

- [X] T002 In `src-tauri/src/db/migrations/0001_initial.sql`, per data-model.md:
  - add to `firearm_types` the three columns `action_type_applies INTEGER NOT NULL DEFAULT 1 CHECK (action_type_applies IN (0, 1))`, `barrel_length_applies INTEGER NOT NULL DEFAULT 1 CHECK (barrel_length_applies IN (0, 1))` and `capacity_applies INTEGER NOT NULL DEFAULT 1 CHECK (capacity_applies IN (0, 1))`, with the comment "FR-003: 0 = the field doesn't apply to this type: the form doesn't offer it and no firearm of the type may hold a value.";
  - create `registration_classes (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, sort_order INTEGER NOT NULL UNIQUE, offered INTEGER NOT NULL DEFAULT 1 CHECK (offered IN (0, 1)))` before `firearms`, with the comment "FR-007: 0 = no longer offered for new choices. The row is never deleted or renamed while a record can hold it.", and give it the three `registration_classes_marks_backup_due_after_*` triggers, copied from `action_types`';
  - add to `firearms` the nullable columns `registration_class_id INTEGER REFERENCES registration_classes (id)`, `registration_form TEXT`, `registration_approved TEXT` (`YYYY-MM-DD`) and `registered_to TEXT`, with no SQL length `CHECK` (004's entry rules apply on entry only), and the table constraint `CHECK (registration_class_id IS NOT NULL OR (registration_form IS NULL AND registration_approved IS NULL AND registered_to IS NULL))`;
  - add `CREATE INDEX idx_firearms_registered_to ON firearms (registered_to) WHERE registered_to IS NOT NULL` and `CREATE INDEX idx_firearms_registration_form ON firearms (registration_form) WHERE registration_form IS NOT NULL` beside `idx_firearms_cartridge`; no index on `registration_class_id`;
  - add the trigger pair `firearms_fields_apply_insert` (`BEFORE INSERT ON firearms`) and `firearms_fields_apply_update` (`BEFORE UPDATE OF firearm_type_id, action_type_id, barrel_length_hundredths, capacity ON firearms`), each `WHEN NEW.action_type_id IS NOT NULL OR NEW.barrel_length_hundredths IS NOT NULL OR NEW.capacity IS NOT NULL`, raising `RAISE(ABORT, 'a field that does not apply to this firearm type has a value')` verbatim from data-model.md's "Rule: fields apply to the type". Leave 004's `firearms_action_allowed_*` triggers unchanged
- [X] T003 [P] In `src-tauri/src/db/migrations/0002_fts5.sql`, add `registered_as`, `registration_form` and `registered_to` to `firearms_fts` after `action_type_name`, and carry them in the `after_insert`, `after_delete` and `after_update` triggers: `(SELECT name FROM registration_classes WHERE id = new.registration_class_id)` (and `old.` in the delete half), `new.registration_form` and `new.registered_to`, the same way `action_type_name` is carried. Do not index `registration_approved` (data-model.md, "Virtual table: firearms_fts (extended)"; FR-017)
- [X] T004 [P] In `src-tauri/src/db/migrations/0003_seed_firearm_types.sql`, seed `firearm_types` with explicit ids (research.md §4): 1 Handgun `handgun`, 2 Rifle `rifle`, 3 Shotgun `shotgun`, 4 Other `other`, all three flags 1, and **5 Suppressor `suppressor` with `action_type_applies`, `barrel_length_applies` and `capacity_applies` all 0**; seed `registration_classes` with fixed ids and `sort_order` equal to the id, all `offered = 1`: 1 Suppressor, 2 Short-barreled rifle, 3 Short-barreled shotgun, 4 Any other weapon, 5 Machine gun, 6 Destructive device. Suppressor (5) gets no `firearm_type_actions` rows. Update the header comment to say the file seeds the type, action and classification lists (the action seed itself changes in US3, T054)
- [X] T005 [P] Add `|| name == "registration_classes"` beside `"firearm_types"` in `is_user_table` in `src-tauri/tests/human_seed_coverage_test.rs` (a seeded lookup, not user data; research.md §14). Do not add any new `firearms` column to `NEVER_SEEDED`. In `src-tauri/tests/backup_due_tracking_test.rs`, add "registration_classes insert" (`INSERT INTO registration_classes (id, name, sort_order) VALUES (99, 'Test class', 99)`), "registration_classes update" and "registration_classes delete" cases beside the `action_types` ones, so the new table's three triggers are tested

### Base model

- [X] T006 Add `registration_class_id: Option<i64>`, `registration_form: Option<String>`, `registration_approved: Option<String>` and `registered_to: Option<String>` to `Firearm`, `FirearmInput` (all `#[serde(default)]` so callers built before this feature may omit them; contracts/tauri-commands.md, "Shared shape: registration"), `Firearm::from_row` and `From<&Firearm> for FirearmInput` in `src-tauri/src/models/firearm.rs`; in `FirearmInput::normalized()` trim `registration_form` and `registered_to` and make a blank one `None`, as for `cartridge` (research.md §6) (depends on T002)
- [X] T007 Add the four columns to the `INSERT`/`UPDATE` statements in `ops::create_firearm`/`ops::update_firearm` and to the `SELECT` column lists that feed `Firearm::from_row` in `src-tauri/src/commands/firearms.rs`, and add `registration_class_id: None, registration_form: None, registration_approved: None, registered_to: None` to every `FirearmInput { .. }` literal under `src-tauri/tests/` and in `src-tauri/examples/human_seed.rs` (find them with `grep -rn 'FirearmInput {' src-tauri`) (depends on T006)
- [X] T008 [P] Add `FirearmTypeInfo { id: i64, name: String, generic_thumbnail_key: String, action_type_applies: bool, barrel_length_applies: bool, capacity_applies: bool }` and `FirearmTypesOutput { types: Vec<FirearmTypeInfo> }` to `src-tauri/src/models/firearm_type.rs`, and `RegistrationClass { id: i64, name: String, offered: bool }` and `RegistrationClassesOutput { classes: Vec<RegistrationClass> }` to `src-tauri/src/models/registration.rs`, all serde camelCase, per contracts/tauri-commands.md's `list_firearm_types` and `list_registration_classes` (depends on T001)
- [X] T009 [P] In `src/features/firearms/types.ts`, add `registrationClassId: number | null`, `registrationForm: string | null`, `registrationApproved: string | null` and `registeredTo: string | null` to `Firearm` and `FirearmInput`, and the types `FirearmTypeInfo`, `FirearmTypesOutput`, `RegistrationClass` and `RegistrationClassesOutput` mirroring T008; default the four fields to `null` wherever `src/features/firearms/FirearmForm.tsx` builds a `FirearmInput` so the app still compiles (depends on T006, T008)
- [X] T010 [P] Create `src/test/collectionFixtures.ts` exporting `FIREARM_TYPES: FirearmTypeInfo[]` (the five seeded types of data-model.md with their flags and drawing keys), `REGISTRATION_CLASSES: RegistrationClass[]` (the six, all offered) and `ACTION_TYPES` (004's twelve actions with their real ids; US3 adds id 13), so every frontend test builds its `CollectionState` from one copy of the seed (depends on T009)

**Checkpoint**: Schema and model shape exist; `cargo test` compiles and every pre-existing test passes (`backup_due_tracking_test` with the new triggers; `human_seed_coverage_test` fails until US1–US3 seed the new columns, which is expected). A development database created before this point must be recreated.

---

## Phase 3: User Story 1 - Record a Suppressor (Priority: P1) 🎯 MVP

**Goal**: Suppressor is a fifth firearm type with its own drawing and no action type, barrel length or capacity; its caliber reads "Caliber rating"; changing to it announces what will be cleared.

**Independent Test**: Create a Suppressor with make, model, serial number and caliber rating; confirm the form offers no action, barrel length or capacity, the record saves, reopens intact, shows the suppressor drawing, groups under Suppressor, and is subject to the same serial number rules as any firearm (spec US1).

### Tests for User Story 1 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T011 [P] [US1] Write failing `src-tauri/tests/suppressor_test.rs` through `ops` against a temporary database:
  - `list_firearm_types` returns five types in id order, Suppressor (id 5, `genericThumbnailKey` "suppressor") with all three flags false and the other four with all true (US1-1, FR-001);
  - a Suppressor with make "SilencerCo", model "Omega 300", serial "ABC123", caliber ".30", overall length, weight, finish and condition saves and `get_firearm` returns every value (US1-3);
  - creating a Suppressor with an action, a barrel length or a capacity fails with `VALIDATION_ERROR` and exactly the field errors `actionTypeId` "Action doesn't apply to a Suppressor.", `barrelLengthHundredths` "Barrel length doesn't apply to a Suppressor.", `capacity` "Capacity doesn't apply to a Suppressor.", one per field set, all three when all are set; a Suppressor with action 1 gets the fields message, not 004's action-allowed one (the check runs before `check_action_allowed`);
  - updating a Rifle with action 3 (Bolt action), barrel 20 in and capacity 5 to type 5 with those values still set is refused with the three field errors; the same update with them `None` saves; changing it back to Rifle saves with them empty (US1-5, FR-004);
  - on a Suppressor, `settle_entry` for cartridge "9x19mm Parabellum" returns the derived caliber as for any type, and a Suppressor with no cartridge saves (US1-7, FR-002);
  - the backstop: a raw `INSERT` of a type-5 firearm with each of the three fields, and a raw `UPDATE` of `firearm_type_id` from 2 to 5 on a row with a barrel length, each abort; the same raw writes on types 1–4 succeed (FR-003, SC-005)
- [X] T012 [P] [US1] In `src-tauri/tests/identity_uniqueness_test.rs`, add: an active Suppressor "SilencerCo" / "Omega 300" / "ABC123" blocks saving another active firearm with the same make, model and serial (001 FR-032), and still blocks it when either record has a classification; a Suppressor recorded with the "no serial number" attestation saves (US1-6, FR-005)
- [X] T013 [P] [US1] In `src-tauri/tests/list_firearms_test.rs`, add: grouped by type, a Suppressor is in a group keyed "Suppressor" and its summary's `genericThumbnailKey` is "suppressor" and `actionType` is null (US1-4)
- [X] T014 [P] [US1] In `src/features/firearms/FirearmForm.test.tsx`, build the test `CollectionState` from `src/test/collectionFixtures.ts` and add, per contracts/ui-registration.md §1:
  - the Type cards are Handgun, Rifle, Shotgun, Other, Suppressor in id order (US1-1);
  - with type Suppressor, no Action field is rendered; Physical details, opened, has no Barrel length or Capacity but has Overall length, Weight, Finish and Condition; the closed summary omits the hidden fields; the caliber field is labelled "Caliber rating" with the hint "The largest bore the suppressor is rated for." (US1-2);
  - a Rifle with Bolt action, 20 in and 5 rounds, changed to Suppressor, shows exactly "A Suppressor has no action, barrel length or capacity, so Bolt action, 20 in and 5 rounds will be cleared when you save."; with only a barrel length recorded the note names only that; changing back to Rifle removes the note and shows the three values again; submitting as Suppressor calls `onSubmit` with `actionTypeId`, `barrelLengthHundredths` and `capacity` all `null`; the polite live region announces the note once per type change that clears something (US1-5, FR-004, spec Edge Cases);
  - when `firearmTypesFailed` is true, a note under Type says the list couldn't be loaded, worded like `ACTION_LIST_FAILED`
- [X] T015 [P] [US1] In `src/features/firearms/FirearmRecordPage.test.tsx`, add: a Suppressor's title block labels its caliber "Caliber rating" and lists no Action, Barrel length or Capacity; a Rifle is unchanged (US1-3, contracts/ui-registration.md §4)
- [X] T016 [P] [US1] Write failing `src/features/browse/TypeDrawing.test.tsx`: every `genericThumbnailKey` in `FIREARM_TYPES` (T010) has an entry in `DRAWINGS`, and `TypeDrawing` renders the suppressor drawing; in `src/features/browse/BrowseTiles.test.tsx`, a Suppressor summary with no photo shows the suppressor drawing (US1-4, FR-001)

### Implementation for User Story 1

- [X] T017 [US1] Add `pub fn check_fields_apply(conn: &Connection, input: &FirearmInput) -> Result<(), CommandError>` to `ops` in `src-tauri/src/commands/firearms.rs`: read the type's three flags and name, and return `VALIDATION_ERROR` with one field error per offending field, "<Field> doesn't apply to a <type name>." (research.md §2). Call it in `create_firearm` and `update_firearm` after `validate_firearm_input` and **before** `check_action_allowed` (contracts/tauri-commands.md, "Order of checks"). The backend never clears the values itself. Confirm `CommandError::from_db` maps the trigger's ABORT to `INTERNAL_ERROR`. Make T011's command-layer and trigger cases pass (depends on T007)
- [X] T018 [US1] Add `ops::list_firearm_types(conn) -> Result<FirearmTypesOutput, CommandError>` (rows by id) and a `#[tauri::command] list_firearm_types` that goes through `session.read(...)` to `src-tauri/src/commands/entries.rs`, and register it in `generate_handler!` in `src-tauri/src/main.rs`. Make T011's list case pass (depends on T008)
- [X] T019 [US1] Add `listFirearmTypes()` to `src/features/firearms/firearmsService.ts`; add `firearmTypes: FirearmTypesOutput` and `firearmTypesFailed: boolean` to the state in `src/features/app/collectionStore.ts` with a `useFirearmTypes()` hook shaped like `useActionTypes()`; load it once per open database in `src/features/app/CollectionProvider.tsx`, beside `list_action_types`, with the same failure handling (research.md §3) (depends on T009, T018)
- [X] T020 [US1] In `src/features/firearms/types.ts`, remove `FIREARM_TYPE_OPTIONS`; make `firearmTypeOption` take the store's type list and an id, still falling back to "Other" with the `other` drawing for an unknown id; add `caliberLabel(typeName)` returning "Caliber rating" for "Suppressor" and "Caliber" otherwise (data-model.md, "Derived values"). Update the callers in `FirearmForm.tsx` and `FirearmRecordPage.tsx`, and give every existing test that renders them (`FirearmForm.test.tsx`, `FirearmRecordPage.test.tsx`, `src/features/app/AppShell.test.tsx`, `src/features/insurance/*.test.tsx`, `src/features/import-export/*.test.tsx`) the fixture's types through `CollectionContext` where they need them (depends on T010, T019)
- [X] T021 [P] [US1] Add a `suppressor` entry to `DRAWINGS` in `src/features/browse/typeDrawings.ts` per research.md §4: a side elevation, mount end left, in the same 320×200 box and part/open/detail roles as the others; a tube of about 6:1 length to diameter centred on the bore axis, a threaded or quick-detach mount collar, wrench flats near the mount, and a front cap with the bore opening; no brand marks; separate subpaths per stroke and round caps started past 1 (branding notes). Record in the file's header comment that it was drawn for the project, traced from side-on photographs for proportion, and is GPL-3.0-only with the rest of the source (constitution, Licensing). Make T016 pass
- [X] T022 [US1] In `src/features/firearms/FirearmForm.tsx`, per contracts/ui-registration.md §1: build the Type `ChoiceCards` from `useFirearmTypes()` in id order (five cards wrap at `minCardWidth` 140); hide Action when the type's `actionTypeApplies` is false, and Barrel length and Capacity in Physical details when theirs are false, leaving them out of the closed summary; label the caliber with `caliberLabel` and give it the hint "The largest bore the suppressor is rated for." for a Suppressor; keep hidden values in the form state (and a pending draft) and leave them out of the input at save when the type in effect omits them; show the clearing note under Type naming only the recorded fields, in the hint style of 004's cleared-action note, and announce it through the existing polite live region once per type change; for a type that omits the action, this hold-until-save note replaces 004's immediate action clearing (research.md §2); show the types-failed note. Make T014 pass (depends on T020)
- [X] T023 [P] [US1] In `src/features/firearms/FirearmRecordPage.tsx`, use `caliberLabel` in the title block and omit the Action, Barrel length and Capacity rows for a type whose flag is false. Make T015 pass (depends on T020)
- [X] T024 [US1] In `src-tauri/examples/human_seed.rs`, add a Suppressor with no classification (quickstart.md "Human-testing seed"), and run `cargo run --example human_seed` through `scripts/human-testing.sh`'s scratch location to confirm it seeds (depends on T017)

**Checkpoint**: A Suppressor can be recorded, shown, grouped and refused any inapplicable field on every path; `suppressor_test`, `identity_uniqueness_test`, `list_firearms_test`, and the form, record page and drawing tests pass.

---

## Phase 4: User Story 2 - Record an Item's Registration (Priority: P2)

**Goal**: Any firearm can carry an optional "Registered as" classification and, with it, a form, an approved date and "Registered to", with suggestions, grouping, search, a guide and a standing "records only" note, and nothing about legal status is ever judged.

**Independent Test**: On a Suppressor and on a Rifle, record a classification with each combination of registration details (none, some, all), save, reopen, confirm the values and that no status is derived from them, search and group by them, and clear a classification; confirm that no screen shows a hint or warning about legal status (spec US2).

### Tests for User Story 2 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T025 [P] [US2] Write failing `src-tauri/tests/registration_test.rs` through `ops`:
  - `list_registration_classes` returns the six classifications in list order, all `offered: true` (US2-1);
  - on a Suppressor and on a Rifle: a classification with no details, with some, and with all ("Suppressor", "Form 4", approved 2026-02-10, "Smith Family Trust") saves and reopens intact; nothing derived is returned (US2-3, US2-4, FR-011);
  - details with no classification fail with `VALIDATION_ERROR` "Choose what the firearm is registered as first." on each set detail (`registrationForm`, `registrationApproved`, `registeredTo`); a raw `INSERT` that does the same fails the table `CHECK`;
  - `registrationClassId` 99 fails with "Choose a classification from the list." on `registrationClassId`;
  - an approved date of tomorrow (local) fails with "Approved date can't be in the future."; "2026-13-40" fails with "Approved date must be a date in YYYY-MM-DD format." (US2-6, FR-010);
  - a 101-character form fails with "Form can be at most 100 characters."; a "Registered to" with a tab fails with "Registered to can't contain control characters."; on update, an unchanged over-long stored value passes (004 FR-015, on entry only); values are trimmed;
  - changing to another classification keeps the details; an update with classification and details all `None` clears them (FR-012);
  - `dispose_firearm`, `reverse_disposition` and `assign_firearm_coverage` keep the details; `delete_firearm` removes them (US2-12, FR-013);
  - SC-004: after a raw `UPDATE registration_classes SET offered = 0 WHERE id = 5`, the list returns it with `offered: false`, a firearm holding it opens and saves an edit to another field with its classification unchanged, and a new firearm may still be created with it (research.md §5);
  - FR-008 / FR-014 / SC-002: every type 1–5 × (no classification and each of the six), plus Rifles and Shotguns with barrels of 10.5 in and 14 in with no classification, all save, and the stored classification is exactly what was given
- [X] T026 [P] [US2] In `src-tauri/tests/entry_text_test.rs`, add: `EntryField::ALL` has six fields; `ipc_name()` is "make", "model", "cartridge", "caliber", "registrationForm", "registeredTo"; `column()` is "registration_form" and "registered_to" for the new two; their serde JSON names are "registrationForm" and "registeredTo" and the four existing names are unchanged; labels are "Form" and "Registered to"; `check_entry_text` messages use those labels; neither is `required()` (research.md §7)
- [X] T027 [P] [US2] In `src-tauri/tests/entry_suggestions_test.rs`, add (US2-5, FR-009, FR-013):
  - `registrationForm` with "F" or "Form" suggests "Form 4" first, then the other built-in names in rank order (Form 4, Form 1, Form 5), each `inCatalog: true`, then forms on record such as "eForm 4";
  - `settle_entry` for "form 4" returns "Form 4" with `changedBy: "catalog"`;
  - `registeredTo` offers only values on record, and "smith family trust" settles to "Smith Family Trust" with `changedBy: "record"`;
  - a "Registered to" value used only by a deleted firearm is not suggested (SC-007);
  - `caliber` and `derivedCaliber` are `null` for both fields
- [X] T028 [P] [US2] In `src-tauri/tests/list_firearms_test.rs`, add: grouped by `registered_as`, groups follow the list's `sort_order` with "Unspecified" last (US2-9); grouped by `registered_to`, groups are alphabetical with "Unspecified" last and holding every firearm with no classification (US2-10, FR-016); each summary carries `registeredAs` (the classification's name or null)
- [X] T029 [P] [US2] In `src-tauri/tests/fts_search_test.rs`, add: "smith family" finds a firearm registered to "Smith Family Trust", "form 1" finds "Form 1" (and "Form 10"), "short-barreled" finds one registered as Short-barreled rifle (US2-11, FR-017); a one- and a two-character query matching only a "Registered to" value (e.g. "QZ Holdings", query "qz") finds it through the `LIKE` branch; changing a firearm's registration updates what search finds; the approved date is not searched
- [X] T030 [P] [US2] In `src-tauri/tests/deletion_wipe_test.rs`, give the deleted firearm a unique "Registered to" value and a unique form, and assert neither is found in the database file's bytes after deletion (SC-007, constitution V)
- [X] T031 [P] [US2] In `src-tauri/tests/disposition_reversal_test.rs`, add a registered firearm whose classification and three details are unchanged after disposal and after reversal (FR-013)
- [X] T032 [P] [US2] In `src-tauri/tests/performance_test.rs`, at 10,000 firearms with a share registered: `list_firearms` grouped by `registered_as` and by `registered_to` each within 1 s, a search for "smith family" within 500 ms, and `suggest_entries` for `registrationForm` and `registeredTo` each within 50 ms (SC-006, research.md §7, §8)
- [X] T033 [P] [US2] In `src/components/Menu.test.tsx`, add for `MenuRadioGroup`, `MenuRadioItem` and `MenuLabel` (contracts/ui-registration.md §6): items have `role="menuitemradio"` and `aria-checked`; a group has `role="group"` labelled by its `MenuLabel`; one value shared across groups checks exactly one item; the menu opens with focus on the checked item; Up/Down cross groups, Home/End reach the ends, a typed letter jumps (typeahead); Enter or Space chooses and closes; Escape closes, chooses nothing and returns focus to the trigger
- [X] T034 [P] [US2] Write failing `src/features/browse/CollectionPage.test.tsx`: the trigger reads "Group by None" when ungrouped and "Group by Caliber" when grouped by caliber, with accessible name "Group by, Caliber" and `aria-haspopup="menu"`; the menu shows None alone, then "The firearm" (Type, Action, Caliber, Cartridge), "Its maker" (Make, Origin) and "Registration" (Registered as, Registered to); choosing "Registered as" regroups with `groupBy: "registered_as"` and the trigger reads "Group by Registered as"; Escape changes nothing; List/Tiles is still a segmented control (FR-016)
- [X] T035 [P] [US2] In `src/features/firearms/FirearmForm.test.tsx`, add per contracts/ui-registration.md §2–§3:
  - the Registration section sits after "Origin and year of manufacture" and before "Physical details"; closed, its summary reads "Optional: what the firearm is registered as, and the approval." with nothing recorded and "Registered as Suppressor. Form 4, approved Feb 10, 2026. Registered to Smith Family Trust." with everything, leaving out missing parts; a save error inside it opens it;
  - open, it shows the standing note (`role="note"`) "HoploDex records what you enter here. It doesn't decide what is regulated or needs registering, and the law changes." and a **How to record registrations** button that opens the guide at "Registered items" (US2-13);
  - **Registered as** offers Unspecified then the six classifications (US2-1); Form, Approved and Registered to appear only once one is chosen (US2-2), none marked required, with the hints "The date on the approved form (the tax stamp date)." and "A person, trust or company, as named on the form."; Approved's `max` is today; Form's suggestions mark the five names "Built-in" (US2-5);
  - choosing Unspecified with details opens a `ConfirmDialog` titled "Discard the registration details?" whose body names only the recorded parts ("Clearing what the firearm is registered as will discard the form "Form 4", the approved date and Registered to "Smith Family Trust". They can't be recovered once saved."), with **Discard details** and **Keep them**; Keep them leaves the classification; Discard details clears all four; choosing another classification keeps the details with no question; Unspecified with no details clears at once (US2-7, FR-012);
  - a record holding a classification with `offered: false` is offered it, in its list position; another record is not (SC-004);
  - a version 2 draft is discarded (`FORM_VERSION` 3);
  - FR-014 / SC-002: for a Rifle with a 10.5 in barrel and no classification, a Suppressor with none, and a Rifle registered as Machine gun with a Semi-automatic action, the rendered text contains no "regulat", "NFA", "pending" or "required", and "register" only in the Registration section's own labels and note (US2-8)
- [X] T036 [P] [US2] In `src/features/firearms/FirearmRecordPage.test.tsx`, add: with a classification, a Registration panel after "Original maker's marks" and before "Physical details" lists Registered as, Form, Approved ("Feb 10, 2026") and Registered to, omitting rows with no value and showing no status row; with none, no panel; the panel's Edit link opens the form on the Registration section; the FR-014 text search of T035 over the rendered page (US2-3, US2-4, FR-018)
- [X] T037 [P] [US2] `git mv src/features/firearms/OriginGuide.test.tsx src/features/firearms/IdentificationGuide.test.tsx` and rewrite it for `IdentificationGuide` (contracts/ui-registration.md §7): the dialog is titled "How to record where a firearm came from and how it's registered"; the disclaimer reads "Record what is stamped on the firearm and what your paperwork says. HoploDex doesn't check it against any rules or decide what is regulated."; part 1 "Where it came from" keeps 002's six examples; part 2 "Registered items" has the two examples of research.md §12 with "What you have" / "How to record it" pairs; opened for the origin it starts at the top; opened for registration, part 2's heading is scrolled into view and focused (US2-13)
- [X] T038 [P] [US2] Write failing `e2e/specs/us11-regulated-items.e2e.ts` using real keyboard input (DEVELOPMENT.md, "Real keyboard and mouse input"): from the keyboard only, add a Suppressor with make, model, serial and caliber rating, open Registration, choose Suppressor, pick "Form 4" from the suggestion list, type an approved date and "Registered to", save and reopen; assert no Action, Barrel length or Capacity was offered and the run took well under 2 minutes (a guard, SC-001); then group the collection by "Registered as" through the grouping menu from the keyboard and see "Unspecified" last

### Implementation for User Story 2

- [X] T039 [US2] In `src-tauri/src/services/entry_text.rs`, add `EntryField::RegistrationForm` and `EntryField::RegisteredTo`; grow `ALL` to six; `column()` returns "registration_form" and "registered_to" and its doc comment now says it is the database and spreadsheet name only; add `ipc_name()` ("registrationForm", "registeredTo", and the column name for the other four); `label()` "Form" and "Registered to"; `required()` false; change `#[serde(rename_all = "lowercase")]` to `"camelCase"`. Make T026 pass
- [X] T040 [US2] Add `pub const BUILT_IN_FORMS: &[&str] = &["Form 4", "Form 1", "Form 5"];` (rank order, data-model.md "Built-in Form Name") to `src-tauri/src/services/registration.rs`; in `src-tauri/src/services/suggestions.rs`, make `known_spelling` and `suggest` treat it as `RegistrationForm`'s catalog (offered with `in_catalog: true`, snapped to first with `ChangedBy::Catalog`), give `RegisteredTo` no catalog, and have `FieldVocabulary::load` read both new columns; in `src-tauri/src/commands/entries.rs`, leave `caliber`/`derivedCaliber` `None` for both in `settle_entry`. Make T027 pass (depends on T039)
- [X] T041 [US2] In `src-tauri/src/models/firearm.rs`'s `validate_firearm_input`: key every entry-rule field error by `EntryField::ipc_name()`; when `registration_class_id` is `None`, add "Choose what the firearm is registered as first." on each set detail; check `registration_approved` through the existing `checked_date` with the label "Approved date" (not after the user's local today, FR-010). In `src-tauri/src/commands/firearms.rs`, look up `registration_class_id` in `registration_classes` on create and update, before the foreign key could fail, and fail with "Choose a classification from the list." on `registrationClassId`; accept a classification with `offered = 0` (research.md §5). Make T025's validation cases pass (depends on T007, T039)
- [X] T042 [US2] Add `ops::list_registration_classes(conn) -> Result<RegistrationClassesOutput, CommandError>` (every row, `ORDER BY sort_order`) and a `#[tauri::command] list_registration_classes` through `session.read(...)` to `src-tauri/src/commands/entries.rs`, and register it in `generate_handler!` in `src-tauri/src/main.rs`. Make T025's list case pass (depends on T008, T018)
- [X] T043 [US2] In `src-tauri/src/commands/firearms.rs`'s `list_firearms`: add `GroupBy::RegisteredAs` and `GroupBy::RegisteredTo` (serde "registered_as", "registered_to"); `LEFT JOIN registration_classes rc`; order "Registered as" groups by `rc.sort_order` and "Registered to" groups alphabetically by stored text, each with `UNSPECIFIED` last (for "Registered to", including firearms with no classification); add `registered_as: Option<String>` (serde `registeredAs`) to `FirearmSummary`; add `rc.name`, `f.registration_form` and `f.registered_to` to the short-query `LIKE` branch. Make T028 and T029 pass (depends on T041)
- [X] T044 [P] [US2] Frontend plumbing: `listRegistrationClasses()` in `src/features/firearms/firearmsService.ts`; `registrationClasses` / `registrationClassesFailed` and a `useRegistrationClasses()` hook in `src/features/app/collectionStore.ts`, loaded in `src/features/app/CollectionProvider.tsx` beside the type list; in `src/features/browse/types.ts`, `GroupBy` gains "registered_as" and "registered_to", `FirearmSummary` gains `registeredAs: string | null`, and `GROUP_BY_OPTIONS` gains a `section: "firearm" | "maker" | "registration"` field (Type, Action, Caliber, Cartridge → firearm; Make, Origin → maker) and the entries "Registered as" and "Registered to"; the entry field union used by `suggestEntries`/`settleEntry` gains "registrationForm" and "registeredTo" (depends on T042, T043)
- [X] T045 [P] [US2] Add `MenuRadioGroup`, `MenuRadioItem` and `MenuLabel` to `src/components/Menu.tsx` over Radix `DropdownMenu.RadioGroup`, `RadioItem` + `ItemIndicator` and `Label`, exported from `src/components/index.ts`; in `src/components/components.css`, draw the leading-gutter indicator on every item (an empty ring in `--rule-strong`, a Niter-filled dot when checked) using the existing `hd-menu__item` height and `--niter-wash` highlight, and style `MenuLabel` at `--text-sm`, `--ink-2`, weight 650, sentence case (contracts/ui-registration.md §6). Make T033 pass
- [X] T046 [US2] In `src/features/browse/CollectionPage.tsx`, replace the "Group by" `SegmentedControl` with a small `Button` trigger reading "Group by" (`--ink-2`) and the chosen label (`--ink`, 650) with a chevron, accessible name "Group by, <label>", that opens a `Menu` aligned to the trigger's start: None in an unlabelled group of its own, then the three sections built from `GROUP_BY_OPTIONS`' `section` with the fixed headings "The firearm", "Its maker", "Registration"; choosing updates the browse state at once; no animation beyond the menu's own open and close. Keep List/Tiles as a `SegmentedControl`. Adjust `src/features/browse/collection.css` for the trigger. Make T034 pass (depends on T044, T045)
- [X] T047 [US2] `git mv src/features/firearms/OriginGuide.tsx src/features/firearms/IdentificationGuide.tsx` and rename the component; set the new title and disclaimer; add part 2 "Registered items" with research.md §12's two examples (a suppressor bought on a Form 4 and registered to a trust; a rifle made into a short-barreled rifle on a Form 1, uppers in notes); add a prop choosing the part to open at, focusing part 2's heading with `placeFocus`; update the import and state in `FirearmForm.tsx` and the exact dialog name in `FirearmForm.test.tsx`; confirm `e2e/specs/us6-identification.e2e.ts`'s partial title match still holds. Make T037 pass
- [X] T048 [US2] In `src/features/firearms/FirearmForm.tsx`, add the Registration `Disclosure` per contracts/ui-registration.md §2: its closed summary; the standing note and the guide button; the **Registered as** `Select` (Unspecified, the offered classifications, plus the record's own if no longer offered, in list position); once a classification is chosen, one `hd-form-grid hd-form-grid--registration` row with Form (`EntryField` `registrationForm`, `hd-field--third`), Approved (`DateField`, `hd-field--third`, `max` today) and Registered to (`EntryField` `registeredTo`, full row below), with the snap notes of 004; the `ConfirmDialog` on clearing with details; `FormState` gains the four fields and the two fields' last settled text; `FORM_VERSION` becomes 3; the registration fields join the focus order used after a failed submit. Add `hd-form-grid--registration` to `src/features/firearms/forms.css`. Make T035 pass (depends on T044, T047)
- [X] T049 [P] [US2] In `src/features/firearms/FirearmRecordPage.tsx`, add the Registration `hd-panel` after "Original maker's marks" and before "Physical details", only when a classification is recorded: a definition list of Registered as (name from `useRegistrationClasses()`), Form, Approved (`formatDate`) and Registered to, rows with no value omitted, and an Edit link that opens the form on the Registration section (contracts/ui-registration.md §4). Make T036 pass (depends on T044)
- [X] T050 [US2] In `src-tauri/examples/human_seed.rs`, add the registration records of quickstart.md's "Human-testing seed": a Suppressor registered as Suppressor on Form 4, approved, to "Smith Family Trust", with an approved-form document attached; a Rifle registered as Short-barreled rifle on Form 1 to the owner with a 10.5 in barrel; a Rifle with a 10.5 in barrel and no classification; a Shotgun registered as Short-barreled shotgun with no approved date; a disposed firearm registered to a unique name; "Smith family trust" typed on one more firearm. `human_seed_coverage_test` must pass for `registration_class_id`, `registration_form`, `registration_approved` and `registered_to` (depends on T041)
- [X] T051 [US2] Build and run `scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us11-regulated-items.e2e.ts'` and make T038 pass (depends on T022, T046, T048)

**Checkpoint**: Registration is recorded, validated, suggested, grouped, searched, shown and guided; no screen judges legal status. US1 and US2 both work on their own.

---

## Phase 5: User Story 3 - Record a Firearm That Fires Automatically (Priority: P3)

**Goal**: The fixed action list gains "Automatic or select-fire" for Handgun, Rifle and Shotgun (and Other), unrelated to any classification.

**Independent Test**: Create a Rifle, a Handgun and a Shotgun with action "Automatic or select-fire", with and without a Machine gun classification; each saves, groups and searches by the action, and the action is not offered for a Suppressor (spec US3).

### Tests for User Story 3 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T052 [P] [US3] In `src-tauri/tests/action_type_test.rs`, add and update: `list_action_types` lists id 13 "Automatic or select-fire" seventh, after Break action and before Falling block, with sort orders 1–6 unchanged, 13 → 7 and ids 7–12 → 8–13; `allowedByFirearmType` lists 13 for types 1, 2 and 3, type 4 has no entry and allows it, type 5 is absent; a Handgun, Rifle, Shotgun and Other with action 13 each save with no classification and with Machine gun (US3-1, US3-2); a Rifle registered as Machine gun saves with Semi-automatic (US3-3, FR-008); a Suppressor with action 13 is refused by the fields rule
- [X] T053 [P] [US3] In `src-tauri/tests/list_firearms_test.rs`, grouped by action, the "Automatic or select-fire" group comes after Break action and before Falling block; in `src-tauri/tests/fts_search_test.rs`, "select-fire" finds a firearm with that action
- [X] T054 [P] [US3] In `src/features/firearms/FirearmForm.test.tsx`, add id 13 to the action fixture (from `ACTION_TYPES` in `src/test/collectionFixtures.ts`, ordered by sort order) and assert "Automatic or select-fire" is offered for Handgun, Rifle, Shotgun and Other and that choosing it shows no prompt about a classification; for a Suppressor, no Action field exists (US3-1, US3-2)

### Implementation for User Story 3

- [X] T055 [US3] In `src-tauri/src/db/migrations/0003_seed_firearm_types.sql`, seed action id 13 "Automatic or select-fire" with `sort_order` 7, move Falling block to Inline muzzleloader (ids 7–12) to sort orders 8–13, and add `firearm_type_actions` rows `(1, 13)`, `(2, 13)` and `(3, 13)` (data-model.md, "Entity: Action Type (extended)"; research.md §13). Add id 13 to `ACTION_TYPES` in `src/test/collectionFixtures.ts`. Make T052–T054 pass (depends on T004)
- [X] T056 [US3] In `src-tauri/examples/human_seed.rs`, add a Rifle with action "Automatic or select-fire" registered as Machine gun (quickstart.md) (depends on T050, T055)

**Checkpoint**: The new action works for every type that allows it and is unrelated to classification.

---

## Phase 6: User Story 4 - Suppressors and Registrations Through Export and Import (Priority: P4)

**Goal**: Export writes the four registration columns and discloses registration details; import reads them, checks them, snaps form and "Registered to", and reports every bad row.

**Independent Test**: Export a collection with suppressors, classifications and every registration field, import it into an empty database and compare; import a sheet with the error cases and check the report (spec US4).

### Tests for User Story 4 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T057 [P] [US4] In `src-tauri/tests/import_export_test.rs`, add per contracts/spreadsheet-format.md:
  - the header has 40 columns, with `registered_as`, `registration_form`, `registration_approved`, `registered_to` after `original_serial_number` and before `photo_filenames` (US4-1);
  - SC-003 round trip: a collection with Suppressors, every classification (one set to `offered = 0` by raw SQL) and every registration field, exported and imported into an empty database, reproduces every type, classification and detail exactly; a second test with same-notation variants of a form and a name merges them and lists them in `snappedValues` with fields `registrationForm` and `registeredTo`;
  - `registered_as` "SHORT-BARRELED RIFLE" (and with surrounding spaces) matches Short-barreled rifle, and a non-offered classification matches by name (US4-4, FR-021);
  - row errors with the exact messages: `registered_as: unknown classification "Short barrel rifle"`; `registered_as: Registration details need a classification.` (US4-5); `registration_approved: Approved date can't be in the future.`; `registration_approved: Approved date must be a date in YYYY-MM-DD format.`; `registered_to: Registered to can be at most 100 characters.`; for a Suppressor row, `action_type: Action doesn't apply to a Suppressor.`, `barrel_length_in: Barrel length doesn't apply to a Suppressor.` and `capacity: Capacity doesn't apply to a Suppressor.`, all three joined by "; " on one row, and never an action-allowed message (US4-6, FR-022); other rows still import;
  - `registration_form` snaps to a built-in name, then to forms on record at import start, then to the sheet's majority spelling; `registered_to` to values on record, then the sheet's majority;
  - a sheet without the four columns imports with no classification (US4-7)
- [X] T058 [P] [US4] In `src-tauri/tests/export_test.rs`, add: `registered_as` is the classification's name, `registration_approved` is `YYYY-MM-DD`, all four are blank on a firearm with no classification, and a Suppressor's `firearm_type` is "Suppressor" with blank `action_type`, `barrel_length_in` and `capacity` (US4-1)
- [X] T059 [P] [US4] In `src/features/import-export/ExportDialog.test.tsx`, add: the note ends "…including serial numbers, values and registration details." when any firearm in the chosen scope has `registeredAs`, and "…including serial numbers and values." otherwise, for both "all" and "filtered", recomputed when the scope changes (US4-2, FR-020, contracts/ui-registration.md §5)
- [X] T060 [P] [US4] In `src/features/import-export/ImportDialog.test.tsx`, add: snapped values with fields `registrationForm` and `registeredTo` are listed under "Form" and "Registered to" (contracts/ui-registration.md §9)

### Implementation for User Story 4

- [X] T061 [US4] In `src-tauri/src/services/spreadsheet.rs`, insert the four columns into `COLUMNS` after `original_serial_number` (40 entries), and add them to the export row and the raw import row (depends on T006)
- [X] T062 [US4] In `src-tauri/src/commands/import_export.rs`: export writes the classification's name, the form, the approved date and "Registered to"; `parse_row` matches `registered_as` by trimmed name `COLLATE NOCASE` among all classifications (offered or not), reports an unknown one and details with a blank `registered_as`, reads `registration_approved` as `acquisition_date` is read (same cell reading and `checked_date`), and calls `check_fields_apply` before `check_action_allowed`, mapping `actionTypeId` → `action_type`, `barrelLengthHundredths` → `barrel_length_in`, `capacity` → `capacity` in the messages; `settle_row` adds `RegistrationForm` and `RegisteredTo` to its loop of optional fields so `Snapping` covers them. Make T057 and T058 pass (depends on T017, T040, T041, T061)
- [X] T063 [P] [US4] In `src/features/import-export/ExportDialog.tsx`, add "and registration details" to the note when any firearm in the scope has `registeredAs`: for "all" from the collection store's summaries (active and disposed), for "filtered" from the `list_firearms` result the dialog already fetches (research.md §10). Make T059 pass (depends on T044)
- [X] T064 [P] [US4] In `src/features/import-export/types.ts`, widen `snappedValues[].field` to include "registrationForm" and "registeredTo"; in `src/features/import-export/ImportDialog.tsx`, label them "Form" and "Registered to". Make T060 pass
- [X] T065 [US4] In `src-tauri/examples/human_seed.rs`, add the import samples of quickstart.md: a round-trip sheet; a sheet with an unknown `registered_as`, details with no classification, a future approved date and a Suppressor row with an action, a barrel length and a capacity; and a sheet without the four columns. `human_seed_coverage_test` must pass for all four spreadsheet columns (depends on T056, T062)

**Checkpoint**: All four stories work together; export and import carry, check, snap and report every new value.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T066 [P] Run `src-tauri/tests/performance_test.rs` at 10,000 firearms through `scripts/dev-container.sh` and record T032's timings (grouping by Registered as and Registered to, the registration search, both suggestion fields) for the PR's performance note (Principle IV, SC-006)
- [X] T067 [P] Add to the walk in `e2e/screenshots/screens.e2e.ts` the screens of contracts/ui-registration.md §10: the form with type Suppressor (Caliber rating, no Action, Physical details open without barrel length or capacity) at wide and minimum width; the type-change clearing note; the Registration section closed with a summary, open with only "Registered as", and open with all details and the Form list showing built-in names; the clear-classification confirmation; the record page's Registration panel; the guide at "Registered items"; the grouping menu open; the list grouped by Registered as; the suppressor drawing in tiles; the export dialog with the registration note. Run `scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'` and keep before/after images, including the grouping control, for the PR
- [X] T068 [P] Add a one-line "Amended by 005 (`specs/005-regulated-item-types/`)" pointer at each amended anchor listed in plan.md's Project Structure: in `specs/001-firearms-inventory/` (spec.md's Assumptions on types, FR-001, FR-009, FR-012, FR-013, FR-018 to FR-020, FR-039; data-model.md's FirearmType and Firearm tables and FTS table; contracts/spreadsheet-format.md's columns; contracts/tauri-commands.md's `FirearmSummary`); in `specs/002-firearm-identification/` (spec.md FR-006, FR-015 and the Assumption on registration detail; contracts/ui-identification.md §8, the guide); in `specs/004-cartridges-action-types/` (spec.md FR-017, FR-018, FR-024 and the clarification on automatic fire; data-model.md's action seed; the `EntryField` list)
- [X] T069 [P] Update `CLAUDE.md`'s Architecture section: `services/` gains `registration` (the built-in form names); `commands/entries.rs` also holds `list_firearm_types` and `list_registration_classes`; the frontend reads firearm types from the backend (no hard-coded list); `src/components/Menu.tsx` has radio items; `OriginGuide` is now `IdentificationGuide`
- [X] T070 Check FR-014 across the code: `grep -rniE 'regulat|NFA|pending|needs? regist' src` finds only the Registration section's note, the guide and tests; `grep -rn 'barrel_length\|overall_length' src-tauri/src` shows no logic that reads the lengths beyond storage, validation, display and spreadsheet I/O; no code derives a classification from a type, action or length (FR-008). Fix anything found
- [X] T071 Run the full gates through `scripts/dev-container.sh`: `cargo test --manifest-path src-tauri/Cargo.toml`, `npm test`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `npm run lint`, `npm run format:check`, `npm run audit`, and `npm run build && npm run test:e2e` one spec at a time (us1–us11); fix every failure (constitution, Development Workflow)
- [X] T072 Check quickstart.md's "Checks that must hold before merge": no new migration file exists (`ls src-tauri/src/db/migrations`); `human_seed_coverage_test` passes with `NEVER_SEEDED` unchanged; the suppressor drawing's source note is in `typeDrawings.ts`; tell the user their development databases must be recreated (the schema was edited in place); draft the PR notes: before/after screenshots from T067, how the persistence changes meet Security & Data Handling (research.md §14: no storage outside `firearms` and its FTS entry, the deletion wipe test extended, the export disclosure, no cipher or dependency change, no network access), and T066's performance figures against Principle IV's budgets
- [X] T073 On Linux, with Orca, do quickstart.md's manual check **M1** (the grouping menu with a screen reader) with `scripts/dev-container.sh --gui scripts/human-testing.sh`, and record the result for the PR (best effort, not a merge gate)
  - **Deferred** (2026-09-30) to #48, with 004's M1: not run before the PR. The ARIA it depends on is covered by `Menu.test.tsx`, `CollectionPage.test.tsx` and `us11-regulated-items.e2e.ts`
- [ ] T074 On Linux, do quickstart.md's manual check **M2** (the suppressor drawing at every size, both themes, minimum window width) with `scripts/dev-container.sh --gui scripts/human-testing.sh`, and record the result for the PR (best effort, not a merge gate)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: depends on Setup; BLOCKS every user story.
- **US1 (Phase 3)**: depends on Foundational only. The MVP.
- **US2 (Phase 4)**: depends on Foundational for its backend and tests. Its frontend follows US1's store and form work (T044 after T019; T048 after T022) only because they edit the same files; its E2E run (T051) needs T022.
- **US3 (Phase 5)**: depends on Foundational only (T055 after T004, same file). T054 follows T014/T035 and T056 follows T050 only because they edit the same files.
- **US4 (Phase 6)**: depends on US1 (`check_fields_apply`, T017) and US2 (validation T041, snapping T040).
- **Polish (Phase 7)**: depends on all four stories.

### Within Each User Story

- Tests first, failing, then implementation (constitution II).
- Backend `ops` before the frontend services that call them; services and store before components; shared components (`Menu`) before the page that uses them.
- Same-file tasks are sequenced by hand:
  - `0003_seed_firearm_types.sql`: T004 → T055
  - `models/firearm.rs`: T006 → T041
  - `commands/firearms.rs`: T007 → T017 → T041 → T043
  - `commands/entries.rs`: T018 → T040 → T042
  - `main.rs`: T018 → T042
  - `src/features/firearms/types.ts`: T009 → T020
  - `firearmsService.ts`: T019 → T044
  - `collectionStore.ts` / `CollectionProvider.tsx`: T019 → T044
  - `FirearmForm.tsx`: T009 → T020 → T022 → T047 → T048
  - `FirearmForm.test.tsx`: T014 → T020 → T035 → T047 → T054
  - `FirearmRecordPage.tsx`: T020 → T023 → T049
  - `FirearmRecordPage.test.tsx`: T015 → T036
  - `list_firearms_test.rs`: T013 → T028 → T053
  - `fts_search_test.rs`: T029 → T053
  - `human_seed.rs`: T007 → T024 → T050 → T056 → T065
  - `src/test/collectionFixtures.ts`: T010 → T055

### Parallel Opportunities

- Foundational: T003, T004, T005 after T002 is drafted; T008, T009 and T010 alongside T006/T007.
- US1 tests T011–T016 are all different files and can be written together; T021 (the drawing) and T023 run in parallel with T022.
- US2 tests T025–T038 are all different files (T035 and T036 wait for US1's edits to the same test files); T045 (`Menu`) runs alongside the backend T039–T043; T049 alongside T048.
- US3's backend (T055) can proceed in parallel with US2 once Foundational is done.
- US4 tests T057–T060 together; T063 and T064 alongside T061/T062.
- Polish T066–T069 together.

---

## Parallel Example: User Story 1

```bash
# Failing tests, all different files:
Task: "Write suppressor_test.rs"                                       # T011
Task: "Extend identity_uniqueness_test.rs for a Suppressor"            # T012
Task: "Extend list_firearms_test.rs for the Suppressor group"          # T013
Task: "Extend FirearmForm.test.tsx for type-dependent fields"          # T014
Task: "Extend FirearmRecordPage.test.tsx for Caliber rating"           # T015
Task: "Write TypeDrawing.test.tsx; extend BrowseTiles.test.tsx"        # T016

# Then:
Task: "check_fields_apply in commands/firearms.rs"                     # T017
Task: "list_firearm_types in commands/entries.rs"                      # T018
Task: "The suppressor drawing in typeDrawings.ts"                      # T021
```

## Parallel Example: User Story 2

```bash
Task: "Write registration_test.rs"                                     # T025
Task: "Extend entry_text_test.rs"                                      # T026
Task: "Extend entry_suggestions_test.rs"                               # T027
Task: "Extend deletion_wipe_test.rs"                                   # T030
Task: "Extend performance_test.rs"                                     # T032
Task: "Extend Menu.test.tsx"                                           # T033
Task: "Write CollectionPage.test.tsx"                                  # T034
Task: "Write the failing us11 E2E spec"                                # T038
Task: "Menu radio items in src/components/Menu.tsx"                    # T045 (after T033), alongside backend T039–T043
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 Setup and Phase 2 Foundational.
2. Phase 3: User Story 1.
3. **STOP and VALIDATE**: run `suppressor_test`, `identity_uniqueness_test`, `list_firearms_test`, `FirearmForm.test.tsx`, `FirearmRecordPage.test.tsx`, `TypeDrawing.test.tsx` and `BrowseTiles.test.tsx`.
4. Demo: record a Suppressor; change a Rifle to a Suppressor and see the clearing note; browse its drawing and group by type.

### Incremental Delivery

1. Foundational → schema and model.
2. US1 → suppressors as first-class records (MVP).
3. US2 → registration, its grouping menu, search, guide and record-only note.
4. US3 → "Automatic or select-fire".
5. US4 → export and import catch up with every new field and rule.
6. Polish → performance, screenshots, amendment pointers, CLAUDE.md, the FR-014 check, the full gates and the manual checks.

Each story adds value without breaking the previous ones; stop at any checkpoint to validate.

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps a task to its user story for traceability
- Verify tests fail before implementing
- Commit after each task or logical group; never touch the real databases

---

## Phase 8: Convergence

- [X] T075 [P] Add a one-line "Amended by 005 (`specs/005-regulated-item-types/`)" pointer beside each `field: "make" | "model" | "cartridge" | "caliber"` union in `specs/004-cartridges-action-types/contracts/tauri-commands.md` (`SuggestEntriesInput`, `SettleEntryInput` and `ImportResult.snappedValues`), saying the union gains `"registrationForm"` and `"registeredTo"` (see 005's contracts/tauri-commands.md) per plan: amendment pointers, "the `EntryField` list" in 004's documents (partial)
- [X] T076 [P] In `specs/005-regulated-item-types/contracts/ui-registration.md` §2, change "suggestions marked "Built-in" for the five form names" to the three built-in form names (Form 1, Form 4, Form 5), matching the spec's clarification and `BUILT_IN_FORMS` in `src-tauri/src/services/registration.rs`, per spec Clarifications (built-in form names) and FR-009 (partial)
