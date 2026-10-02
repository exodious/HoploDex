---

description: "Task list for feature 006: accessory records and mounting"
---

# Tasks: Accessory Records and Mounting

**Input**: Design documents from `/specs/006-accessory-links/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (tauri-commands.md, spreadsheet-format.md, ui-accessories.md), quickstart.md (all present)

**Tests**: The project constitution's Testing Standards principle is NON-NEGOTIABLE: every user story below includes test tasks written first (integration tests run the real `ops` against a temporary SQLCipher database, no mocks), expected to fail, then implementation makes them pass.

**Organization**: Tasks are grouped by user story (spec.md's priorities P1–P5) so each story can be implemented and tested on its own. This feature is a **delta** against `specs/001-firearms-inventory/` as amended by 002 to 005: most tasks edit existing files (plan.md's Project Structure), and the schema is edited in place in migrations 0001–0003 (CLAUDE.md). No new migration file, dependency, layer or error code.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Which user story this task belongs to (US1–US5)
- Exact file paths are given per plan.md's Project Structure
- Work that must be built, tested or run on a specific platform gets one task per platform

## Path Conventions

Existing Tauri desktop app: Rust backend in `src-tauri/`, React/TypeScript frontend in `src/`, WebdriverIO E2E suite in `e2e/`. Run tests, lint and screenshots through `scripts/dev-container.sh` (CLAUDE.md; commands in quickstart.md). Never open the real databases (CLAUDE.md, "Never touch the real databases").

## How the stories divide the shared pieces

- **Foundational** owns the whole schema (every table, column, index, FTS table and trigger of data-model.md), the record identifier on firearms (`services::record_id`, `firearms.uid` set on create), the shared shapes `RecordRef`/`RecordLabel`/`MountedEntry`/`MountDetail`, and the frontend's mirrors of them, so every story writes records that hold them.
- **US1** owns the accessory record end to end: kinds, the nine accessory commands, validation, the media owner (`owner: RecordRef` on photos, documents and history), value and insurance over both tables, suggestions over both tables, accessory drafts, `RecordName`, the 12 drawings, the form, the record page, a plain Accessories page (list, tiles, disposed toggle), and the dialogs serving either kind.
- **US2** owns mounting: `MountGraph`, `mount_record`, `list_mount_candidates`, `mountedOn` on both inputs and outputs, `MountDetail` on both record pages, the Mounted section, the chooser, the chain, the collection page's mount details, and the Accessories page's Mounted on column. Its dispose commands only remove the disposed record's own mounts, so the status backstops hold.
- **US3** owns disposing with mounted records (`withMounted`, the optional disposition price, the dispose dialog's Mounted group) and the delete confirmations, plus SC-004's random sequence.
- **US4** owns the Accessories page's search and grouping (`accessories_fts`, `groupBy`, the grouping menu).
- **US5** owns the spreadsheet: `record_id` and `mounted_on` in both tables, the accessory table, `get_export_scope`, two-table import and mount resolution.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Empty modules so later tasks in different files can proceed in parallel.

- [X] T001 Create the new backend modules and declare them: `src-tauri/src/models/record.rs`, `src-tauri/src/models/accessory.rs` and `src-tauri/src/models/accessory_kind.rs` in `src-tauri/src/models/mod.rs`; `src-tauri/src/services/mounts.rs` and `src-tauri/src/services/record_id.rs` in `src-tauri/src/services/mod.rs`; `src-tauri/src/commands/accessories.rs` and `src-tauri/src/commands/mounts.rs` (each with an empty `pub mod ops`) in `src-tauri/src/commands/mod.rs`. Give each a module doc comment citing its source: `record` research.md §7, `accessory` data-model.md "Entity: Accessory", `accessory_kind` research.md §15, `mounts` research.md §5 ("every mount rule, as plain functions over one loaded graph"), `record_id` research.md §6, `commands::accessories` contracts/tauri-commands.md "Accessories (new)", `commands::mounts` "Mounts (new)". `cargo build --manifest-path src-tauri/Cargo.toml` must pass
- [X] T002 [P] Create the frontend feature folders with empty, compiling modules: `src/features/accessories/` (`types.ts`, `accessoriesService.ts`, `accessories.css`) and `src/features/mounts/` (`types.ts`, `mountsService.ts`, `mounts.css`), each `.ts` file with a header comment naming its contract section (contracts/tauri-commands.md, contracts/ui-accessories.md). `npm run lint` must pass

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The schema, the record identifier on firearms, and the shared record shapes that every story builds on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Schema (migrations edited in place; no new migration file)

- [X] T003 In `src-tauri/src/db/migrations/0001_initial.sql`, per data-model.md:
  - add to `firearms` the column `uid TEXT NOT NULL UNIQUE CHECK (length(uid) = 36 AND uid GLOB '????????-????-4???-????-????????????' AND substr(uid, 20, 1) IN ('8', '9', 'a', 'b') AND NOT (replace(uid, '-', '') GLOB '*[^0-9a-f]*'))`, verbatim from "Entity: Record Identifier";
  - create `accessory_kinds (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, generic_thumbnail_key TEXT NOT NULL, sort_order INTEGER NOT NULL UNIQUE, offered INTEGER NOT NULL DEFAULT 1 CHECK (offered IN (0, 1)))` with the comment "FR-002: 0 = no longer offered for new choices. The row is never deleted or renamed while a record can hold it.";
  - create `accessories` exactly as data-model.md "Entity: Accessory (new)" gives it: the same `uid` column and `CHECK`, `accessory_kind_id INTEGER NOT NULL REFERENCES accessory_kinds (id)`, the optional text columns with **no length `CHECK`** (004's entry rules apply on entry only), `status` `CHECK (status IN ('active', 'disposed'))`, the money columns each `CHECK (… IS NULL OR … >= 0)`, `disposition_type` `CHECK (disposition_type IS NULL OR disposition_type IN ('sold', 'traded', 'gifted', 'destroyed', 'lost_stolen'))`, `thumbnail_photo_id … REFERENCES photos (id) ON DELETE SET NULL`, `insurance_policy_id … REFERENCES insurance_policies (id) ON DELETE RESTRICT`, timestamps, and `CHECK ((insurance_policy_id IS NULL) = (scheduled_coverage_amount IS NULL))`; no nickname, no serial-or-attestation `CHECK`, no identity index, no fields-apply trigger (FR-004);
  - add its seven indexes verbatim: `idx_accessories_kind`, `idx_accessories_status`, `idx_accessories_insurance_policy`, and the partial `idx_accessories_make`, `idx_accessories_make_model`, `idx_accessories_caliber`, `idx_accessories_cartridge` (research.md §16);
  - add the triggers `firearms_uid_fixed` and `accessories_uid_fixed` (`BEFORE UPDATE OF uid … WHEN NEW.uid IS NOT OLD.uid`, `RAISE(ABORT, 'a record identifier never changes')`) and `accessories_uid_distinct` / `firearms_uid_distinct` (`BEFORE INSERT`, `RAISE(ABORT, 'record identifier already used by a firearm')` and `'… by an accessory'`), each placed after both tables exist;
  - give `accessory_kinds` and `accessories` the three `*_marks_backup_due_after_*` triggers each, copied from `registration_classes`'
- [X] T004 In `src-tauri/src/db/migrations/0001_initial.sql` (after T003), per data-model.md "Entity: Photo, Document Attachment, Disposition History": in each of `photos`, `document_attachments` and `disposition_history`, make `firearm_id INTEGER REFERENCES firearms (id) ON DELETE CASCADE` (drop `NOT NULL`), add `accessory_id INTEGER REFERENCES accessories (id) ON DELETE CASCADE` and `CHECK ((firearm_id IS NULL) <> (accessory_id IS NULL))`, and add `idx_photos_accessory`, `idx_document_attachments_accessory` and `idx_disposition_history_accessory` on `accessory_id` `WHERE accessory_id IS NOT NULL`, keeping the `firearm_id` indexes. In `pending_changes`, make the kind `CHECK (kind IN ('firearm', 'policy', 'accessory'))` and the coverage rule `CHECK (mode <> 'coverage' OR kind IN ('firearm', 'accessory'))` (data-model.md "Entity: Pending changes")
- [X] T005 In `src-tauri/src/db/migrations/0001_initial.sql` (after T004), create `mounts` verbatim from data-model.md "Entity: Mount (new)" (the item pair `UNIQUE`, the host pair, every `ON DELETE CASCADE`, the two exactly-one `CHECK`s and the two one-step self-mount `CHECK`s), `idx_mounts_host_firearm` and `idx_mounts_host_accessory` (partial), the backstop triggers `mounts_active_insert` and `mounts_active_update` (`RAISE(ABORT, 'a mount needs an active item and an active host')`), `firearms_disposed_unmounted` and `accessories_disposed_unmounted` (`BEFORE UPDATE OF status … WHEN NEW.status = 'disposed'`, `RAISE(ABORT, 'a disposed record cannot be mounted or carry mounts')`), and the three `mounts_marks_backup_due_after_*` triggers (a mount is collection data)
- [X] T006 [P] In `src-tauri/src/db/migrations/0002_fts5.sql`, create `accessories_fts USING fts5(kind_name, make, model, serial_number, caliber, cartridge, acquisition_source, notes, content = 'accessories', content_rowid = 'id', tokenize = 'trigram remove_diacritics 1')` and its `accessories_fts_after_insert`, `_after_delete` and `_after_update` triggers shaped like `firearms_fts`'s, with `kind_name` looked up as `(SELECT name FROM accessory_kinds WHERE id = new.accessory_kind_id)` (and `old.` in the delete half) (data-model.md "Virtual table: accessories_fts"; research.md §12)
- [X] T007 [P] In `src-tauri/src/db/migrations/0003_seed_firearm_types.sql`, seed `accessory_kinds` with the fixed ids of data-model.md, `sort_order` equal to the id, all `offered = 1`: 1 Optic `optic`, 2 Light or laser `light`, 3 Magazine `magazine`, 4 Stock or brace `stock`, 5 Upper receiver `upper`, 6 Barrel `barrel`, 7 Muzzle device `muzzle`, 8 Conversion kit `conversion`, 9 Mount or rail `mount`, 10 Sling `sling`, 11 Case `case`, 12 Other `accessory`. No Suppressor kind (FR-002). Update the header comment to say the file also seeds the accessory kinds
- [X] T008 [P] In `src-tauri/tests/human_seed_coverage_test.rs`, extend `is_user_table` to skip `accessory_kinds` (a seeded lookup) and the `accessories_fts` shadow tables (`name.starts_with("accessories_fts")`), updating its doc comment to cite research.md §23; do not add any new column to `NEVER_SEEDED`. In `src-tauri/tests/backup_due_tracking_test.rs`, add insert, update and delete cases for `accessory_kinds` (`id` 99), `accessories` (a raw row with a valid lowercase v4 `uid` and kind 1) and `mounts` (an accessory mounted on a firearm), beside the `registration_classes` ones, and give the file's raw `INSERT INTO firearms` a valid `uid`

### Record identifier on firearms (FR-019)

- [X] T009 [P] Write failing `src-tauri/tests/record_identifier_test.rs` (firearm part):
  - `services::record_id::generate()` returns 36 characters matching `^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`, and 1,000 calls are distinct;
  - `services::record_id::parse` accepts a valid identifier with surrounding whitespace and in upper case, returning it trimmed and lowercased, and rejects `""`, a v1 UUID, 32 hex digits without hyphens, a variant digit outside `8 9 a b`, and a non-hex letter;
  - `ops::create_firearm` stores a `uid` the column `CHECK` accepts; two firearms get distinct ones;
  - `update_firearm`, `dispose_firearm`, `reverse_disposition` (keep and discard) and `assign_firearm_coverage` leave it unchanged;
  - deleting the highest-id firearm and creating another (which reuses the id) gives a new `uid`;
  - a raw `UPDATE firearms SET uid = …` aborts (`firearms_uid_fixed`); a raw insert with a malformed `uid` fails the `CHECK`;
  - `serde_json::to_value` of `Firearm` and of `get_firearm`'s detail has no `uid` key and no value equal to the identifier (FR-019)
- [X] T010 Implement `src-tauri/src/services/record_id.rs`: `pub fn generate() -> String` from 16 bytes of `getrandom`, setting the version nibble to 4 and the variant bits to `10`, formatted lowercase and hyphenated; `pub fn parse(cell: &str) -> Option<String>` (trim, any letter case, must be that v4 form, returns lowercase). Make T009's service cases pass (research.md §6)
- [X] T011 [P] In `src-tauri/src/models/record.rs`, add per contracts/tauri-commands.md "Shared shapes": `RecordKind { Firearm, Accessory }` (serde lowercase), `RecordRef` (Rust `enum RecordRef { Firearm(i64), Accessory(i64) }`, serialized over IPC as `{ kind, id }` through a custom serde or `#[serde(tag = "kind", content = "id")]`-equivalent), with `Hash`/`Eq`/`Copy` and helpers `kind()`, `id()` and `owner_columns()` returning `(Option<i64>, Option<i64>)` for a `(firearm_id, accessory_id)` pair; `RecordLabel { record, make, model, nickname, type_name, serial_number, status }`; `MountedEntry { label, host: RecordRef, depth: u32 }`; `MountDetail { chain: Vec<RecordLabel>, mounted: Vec<MountedEntry> }` with `MountDetail::default()` empty; all camelCase
- [X] T012 Add `uid: String` with `#[serde(skip)]` to `Firearm` and `Firearm::from_row` in `src-tauri/src/models/firearm.rs` (not to `FirearmInput`); in `ops::create_firearm` in `src-tauri/src/commands/firearms.rs`, generate it with `record_id::generate()` and insert it; never write it in an `UPDATE`. Give every raw `INSERT INTO firearms` under `src-tauri/tests/` a valid identifier (`suppressor_test.rs`, `registration_test.rs`, `action_type_test.rs`, `identity_uniqueness_test.rs`, `performance_test.rs`; find them with `grep -rn 'INSERT INTO firearms' src-tauri/tests`). In `src-tauri/tests/backup_test.rs`, assert every firearm's `uid` is equal after a backup and a restore of it (research.md §23). Make T009 pass (depends on T003, T010)

### Frontend shapes

- [X] T013 [P] Mirror the shapes in TypeScript: in `src/features/mounts/types.ts`, `RecordKind`, `RecordRef`, `RecordLabel`, `MountedEntry` and `MountDetail` exactly as contracts/tauri-commands.md "Shared shapes"; in `src/features/accessories/types.ts`, `AccessoryKind`, `AccessoryInput`, `Accessory`, `AccessoryDetail`, `ListAccessoriesInput`, `AccessorySummary` and `AccessoryGroup` exactly as "Accessories (new)", importing the shared ones; re-export `RecordRef` from `src/features/firearms/types.ts`
- [X] T014 [P] In `src/test/collectionFixtures.ts`, export `ACCESSORY_KINDS: AccessoryKind[]` with the 12 seeded kinds of data-model.md (ids, names, `genericThumbnailKey`, `sortOrder`, all `offered: true`), so every frontend test builds its state from one copy of the seed (depends on T013)

**Checkpoint**: The schema exists and every firearm has an identifier; `cargo test` compiles and every pre-existing test passes (`backup_due_tracking_test` with the new triggers; `human_seed_coverage_test` fails until US1–US3 seed the new tables, which is expected). A development database created before this point must be recreated.

---

## Phase 3: User Story 1 - Record an Accessory (Priority: P1) 🎯 MVP

**Goal**: Accessories are a second kind of record with a kind from a fixed list of 12, optional make, model, serial number, caliber, cartridge, value, acquisition details and notes, photos and documents, disposal and restore, insurance scheduling, and a place in the value summary; an Accessories page lists them.

**Independent Test**: Create, edit, dispose, restore and delete accessories of several kinds, with and without each optional field, photos and documents; confirm they save, reopen intact, appear on the Accessories page, and are counted in the value summary, the blanket total and the coverage warnings exactly when they are active (spec US1).

### Tests for User Story 1 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T015 [P] [US1] Write failing `src-tauri/tests/accessory_test.rs` through `ops` against a temporary database:
  - `list_accessory_kinds` returns the 12 kinds in `sortOrder` (Other last), with no "Suppressor", all `offered: true` (US1-2, FR-002);
  - an accessory with only kind Sling (10) saves and reopens with every optional field `null` (US1-5);
  - an Optic "Leupold" "VX-5HD 3-15x44" with serial number, value 1000, price 1100, acquired 2025-11-02 and a note saves and `get_accessory` returns every value (US1-3); a Magazine "Walther" "P38 magazines, pair" value 180 saves (US1-4);
  - a missing kind or kind 99 fails with `VALIDATION_ERROR` and exactly `fieldErrors.accessoryKindId = "Choose a kind."`;
  - after a raw `UPDATE accessory_kinds SET offered = 0 WHERE id = 8`, the list returns it with `offered: false`, an accessory holding it reopens and saves an edit with its kind unchanged, and a new accessory may still be created with it (FR-002);
  - two accessories with the same make, model and serial number both save, with no attestation (FR-004);
  - make, model, caliber and cartridge follow 004's entry rules ("Make can be at most 100 characters." for 101 characters, control characters refused), a blank make or model is accepted, and an unchanged over-long stored value passes on update; serial number and acquisition source are only trimmed, blank stored as `null` (FR-003);
  - an acquisition date of tomorrow (local) and a negative amount fail with the firearm's messages; disposed without type, recipient or date fails as for a firearm; a policy without an amount fails (data-model.md "Validation rules");
  - `dispose_accessory` (sold, recipient, date, price) leaves the default `list_accessories` and is listed with `includeDisposed`; `reverse_accessory_disposition` with `keep` retains a history entry and with `discard` none, with no nickname or identity re-check (US1-9, FR-006);
  - `delete_accessory` with `confirmed: false` is refused as `delete_firearm`'s is, and with `true` removes the row (US1-10);
  - a firearm whose free-text `accessories` is "Leupold scope" keeps it unchanged after accessories are created, and no accessory is created from it (US1-11, FR-007);
  - `serde_json::to_value` of `Accessory` and of the detail has no `uid` key (FR-019)
- [X] T016 [P] [US1] In `src-tauri/tests/record_identifier_test.rs`, add the accessory part: `create_accessory` stores a distinct valid `uid`; edit, dispose, restore and coverage keep it; a raw `UPDATE accessories SET uid = …` aborts; a raw `INSERT` of an accessory with a firearm's `uid` aborts (`accessories_uid_distinct`) and a raw firearm insert with an accessory's `uid` aborts (`firearms_uid_distinct`) (FR-019, FR-022)
- [X] T017 [P] [US1] Write failing `src-tauri/tests/list_accessories_test.rs` (the plain list): with no `groupBy`, one group "All" ordered by make, model, then kind; disposed accessories only with `includeDisposed: true`; each `AccessorySummary` carries `kindName`, `genericThumbnailKey`, `thumbnailPhotoId`, `estimatedValue`, `insurancePolicyId`, `scheduledCoverageAmount` and `insuranceWarning` (`uninsured` with no policy in force, `under_insured` with a scheduled amount below the value, `none` otherwise) (FR-016)
- [X] T018 [P] [US1] In `src-tauri/tests/entry_suggestions_test.rs`, add (US1-6, US1-10, FR-003): a make recorded only on an accessory is suggested for `make`, and `settle_entry` snaps a variant to it with `changedBy: "record"`; model suggestions narrowed by a make include models recorded on accessories of that make; caliber and cartridge vocabularies read both tables; after the only accessory using a make is deleted, it is no longer suggested
- [X] T019 [P] [US1] In `src-tauri/tests/photo_test.rs` and `src-tauri/tests/document_test.rs`, add the accessory owner (US1-12, FR-007a): `add_photo`/`add_photo_from_path` with `owner: { kind: "accessory", id }` makes the first photo the accessory's thumbnail; `set_thumbnail_photo` switches it and returns `{ thumbnailPhotoId }`; `list_photos`/`list_documents` by owner return only that owner's; naming a photo with the wrong owner is `NOT_FOUND`; `add_document`/`open_document` work for an accessory; a raw `INSERT` into `photos`, `document_attachments` or `disposition_history` with both owners or neither fails the `CHECK`; deleting the accessory removes its photos and documents. Update the existing firearm calls to pass `owner: { kind: "firearm", id }`
- [X] T020 [P] [US1] In `src-tauri/tests/valuation_test.rs`, `src-tauri/tests/insurance_status_test.rs` and `src-tauri/tests/policy_deletion_test.rs`, add accessories (US1-7, US1-8, FR-008, FR-009, SC-005): `collectionTotal == firearmsTotal + accessoriesTotal` over active records only, with active accessories of 1000 and 180 giving `accessoriesTotal` 1180; the blanket total includes unscheduled active accessories, with `firearmCount` and `accessoryCount`; an accessory scheduled below its value is under-insured and one with no policy in force is listed in `uninsured` by `record: { kind: "accessory", id }`; a disposed accessory counts nowhere; `get_policy_deletion_impact` returns `scheduledRecords`, `scheduledRecordCount` and `blanketRecordCount` counting both kinds, and `delete_insurance_policy` moves or unschedules the policy's accessories with its firearms in one transaction; rename uses of `firearm_warning` to `record_warning`
- [X] T021 [P] [US1] In `src-tauri/tests/disposition_reversal_test.rs`, add an accessory disposed and restored with history kept and discarded, with history entries owned by the accessory (`owner`), and no nickname or identity check on restore (FR-006)
- [X] T022 [P] [US1] In `src-tauri/tests/deletion_wipe_test.rs`, add an accessory with a unique serial number, a unique note, a photo and a document; after `delete_accessory`, assert none of those bytes is found in the raw database file (read through `db/raw_file.rs`) and the photo and document rows are gone (SC-006, constitution V)
- [X] T023 [P] [US1] In `src-tauri/tests/pending_changes_test.rs`, add `kind: "accessory"` drafts in each mode (`add`, `edit`, `dispose`, `restore`, `coverage`), each kept and resumed with its label; a policy draft in `coverage` mode is still refused; a resumed edit whose accessory was deleted is not resumable (FR-027, research.md §19)
- [X] T024 [P] [US1] In `src-tauri/tests/backup_test.rs`, add an accessory with a photo: it, its photo and its `uid` survive a backup and a restore of it (FR-025, research.md §23)
- [X] T025 [P] [US1] Write failing `src/features/mounts/RecordName.test.tsx`: a firearm label renders as `FirearmName` does (make, model, “nickname”, and the type where asked); an accessory with make and model reads "Leupold VX-5HD 3-15x44 · Optic", with only one of them that one ("Walther · Magazine"), with neither the kind alone ("Sling") (FR-005)
- [X] T026 [P] [US1] Write failing `src/features/accessories/AccessoryForm.test.tsx` per contracts/ui-accessories.md §3: rows in order Kind, Make and Model, Cartridge and Caliber, Serial number, Estimated value, Acquired, Notes (Mounted on is US2's); Kind is a required `Select` of the offered kinds in list order with the hint "A suppressor is recorded as a firearm."; a saved record holding a no-longer-offered kind shows it as selected; there is no quantity field and no "no serial number" box; the value hint reads "The value of this record as a whole, everything it describes included. Value each record on its own."; picking the cartridge "5.56x45mm NATO" with an empty caliber fills the caliber with 004's hint (US1-6); a save with no kind focuses Kind and shows "Choose a kind."; titles "Add accessory" and "Edit {name}"; closing with changes asks save, discard or cancel; a lock keeps a draft labelled with the accessory's name (FR-027)
- [X] T027 [P] [US1] Write failing `src/features/accessories/AccessoryRecordPage.test.tsx` per contracts/ui-accessories.md §12: bar with Back, the name and Edit, Mark disposed (Restore when disposed) and Delete; main column in order photo gallery, Details (kind, make, model, serial number, caliber, cartridge), Value and acquisition, Notes, Documents, Disposition history; side column Coverage; the delete confirmation's wording matches the firearm's with "accessory"
- [X] T028 [P] [US1] Write failing `src/features/accessories/AccessoriesPage.test.tsx` (plain list) per contracts/ui-accessories.md §2: the same List | Tiles `SegmentedControl` and "Show disposed" checkbox as the collection page; list columns Accessory (a `RecordName` link), Value and Coverage (`InsuranceWarningBadge`); tiles show the thumbnail, or the kind's `TypeDrawing` with no photo; "No accessories recorded yet." with the Add accessory button when empty; disposed accessories appear only with Show disposed; a name link calls `navigation.open` with `{ page: "accessory", id, from }`
- [X] T029 [P] [US1] In `src/features/browse/TypeDrawing.test.tsx`, assert every `genericThumbnailKey` in `ACCESSORY_KINDS` has an entry in `DRAWINGS`, and that `TypeDrawing` renders the `optic` drawing (FR-007a)
- [X] T030 [P] [US1] In `src/features/firearms/DisposeDialog.test.tsx`, `src/features/firearms/RestoreDialog.test.tsx` and `src/features/insurance/CoverageDialog.test.tsx`, add the accessory variant: each dialog opened with an accessory names it by `RecordName`, calls `dispose_accessory`, `reverse_accessory_disposition` or `assign_accessory_coverage`, and keeps its draft as `kind: "accessory"` (FR-006, FR-009, FR-027)
- [X] T031 [P] [US1] In `src/features/insurance/PolicyCard.test.tsx` and `src/features/insurance/PolicyDeleteDialog.test.tsx`, and a new `src/features/insurance/InsurancePage.test.tsx`: the value summary shows "Accessories {accessoriesTotal}" under the collection total beside the firearms subtotal, and the blanket line counts "{n} firearms and {m} accessories"; a policy card lists scheduled accessories with the scheduled firearms, each by `RecordName`; the delete dialog counts records of both kinds from `scheduledRecords` (contracts/ui-accessories.md §10)
- [X] T032 [P] [US1] In `src/features/app/AppShell.test.tsx`, add: the tab row reads Collection · Accessories · Insurance; the Accessories tab opens the Accessories page; "Add accessory" opens `AccessoryForm`; a resumed `kind: "accessory"` draft opens the form in add or edit, or the dispose, restore or coverage dialog for its accessory (contracts/ui-accessories.md §1; FR-027)

### Implementation for User Story 1

- [X] T033 [US1] In `src-tauri/src/models/accessory_kind.rs`, add `AccessoryKind { id, name, generic_thumbnail_key, sort_order, offered: bool }` and `AccessoryKindsOutput { kinds }` (camelCase). In `src-tauri/src/models/accessory.rs`, add `Accessory` (every column, `uid` `#[serde(skip)]`, `from_row`), `AccessoryInput` (every column except `id`, `uid`, `thumbnail_photo_id` and the timestamps), `AccessoryInput::normalized()` (trim, blank → `None`, as `FirearmInput`'s) and `From<&Accessory> for AccessoryInput` (depends on T011)
- [X] T034 [US1] Extract the disposition, amount, date and insurance-pair rules of `validate_firearm_input` in `src-tauri/src/models/firearm.rs` into shared helpers (in `src-tauri/src/models/mod.rs` or a `models::rules` submodule) with the firearm's messages unchanged, and add `validate_accessory_input(input, stored)` to `src-tauri/src/models/accessory.rs` per data-model.md's "Validation rules" table: `make`, `model`, `caliber`, `cartridge` under 004's entry rules only when changed from the stored value, with a blank make or model allowed (add an optional-field variant of `check_entry_text` in `src-tauri/src/services/entry_text.rs` that skips the required check); `serialNumber`, `acquisitionSource`, `notes` free text; amounts whole dollars not negative; `acquisitionDate` `YYYY-MM-DD` not after today (local); disposition type, recipient and date required when disposed and none of the four when active (the price stays required here until US3's T086); `dispositionDate` not before `acquisitionDate`; policy and amount both or neither; messages naming the record say "accessory". Existing firearm tests must still pass (depends on T033)
- [X] T035 [US1] In `src-tauri/src/commands/accessories.rs`, add `ops::create_accessory` (normalize, validate, check the kind exists in `accessory_kinds`, offered or not, else "Choose a kind." on `accessoryKindId`, generate `uid`, insert), `ops::update_accessory`, `ops::get_accessory` (→ `AccessoryDetail` with `dispositionHistory` and `mount: MountDetail::default()` until US2) and their `#[tauri::command]`s through `session.write`/`session.read`; add `ops::list_accessory_kinds` and its command to `src-tauri/src/commands/entries.rs`; register all four in `generate_handler!` in `src-tauri/src/main.rs`. Make T015's create, update, kind and validation cases pass (depends on T034)
- [X] T036 [US1] In `src-tauri/src/commands/accessories.rs`, add `ops::list_accessories(conn, ListAccessoriesInput)` with `includeDisposed` and one "All" group ordered by make, model, then kind (query and `groupBy` are US4's), each `AccessorySummary` joined to `accessory_kinds` with `insuranceWarning` from `insurance_status::record_warning` (T040), `mountedOn: None` until US2; and the command, registered in `main.rs`. Make T017 pass (depends on T035, T040)
- [X] T037 [US1] In `src-tauri/src/commands/accessories.rs`, add `ops::dispose_accessory` (the same `DisposeInput` checks and history as `dispose_firearm`; `withMounted` is US3's), `ops::reverse_accessory_disposition` (`keep` | `discard` history, no nickname or identity re-check, FR-006) and `ops::delete_accessory` (`confirmed` as `delete_firearm`, then `db::reclaim_deleted_record`), with commands registered in `main.rs`. In `src-tauri/src/db/mod.rs`, rename `reclaim_deleted_firearm` to `reclaim_deleted_record` and have it optimize both `firearms_fts` and `accessories_fts` before the `VACUUM`; update `delete_firearm`'s call. Make T015's dispose, restore and delete cases, T021 and T022 pass (depends on T035, T038)
- [X] T038 [US1] Give photos, documents and history an owner (research.md §3, §21): in `src-tauri/src/models/photo.rs`, `document_attachment.rs` and `disposition_history.rs`, replace `firearm_id: i64` with `owner: RecordRef` (serialized as `owner`, read from the column pair); in `src-tauri/src/commands/photos.rs` and `src-tauri/src/commands/documents.rs`, make `list_photos`, `add_photo`, `add_photo_from_path`, `set_thumbnail_photo`, `list_documents`, `add_document` and `add_document_from_path` take `owner: RecordRef` per contracts/tauri-commands.md "Photos and documents (amended)"; `set_thumbnail_photo` updates `firearms` or `accessories` by owner and returns `{ thumbnailPhotoId }`; the first photo becomes the owner's thumbnail; a photo of a different owner is `NOT_FOUND`; write firearm history rows with the owner pair in `src-tauri/src/commands/firearms.rs`. Decrypted copies still go to `OPENED_DOCUMENTS_DIR`. Update every caller in `src-tauri/tests/` and `src-tauri/examples/human_seed.rs` (`grep -rn 'firearm_id\|firearmId' src-tauri/tests src-tauri/examples`). Make T019 pass (depends on T011)
- [X] T039 [US1] In `src-tauri/src/commands/insurance.rs`, add `ops::assign_accessory_coverage` mirroring `assign_firearm_coverage` and its command, registered in `main.rs`; make `get_policy_deletion_impact` return `scheduledRecords: RecordLabel[]`, `scheduledRecordCount` and `blanketRecordCount`, and `delete_insurance_policy` move or unschedule accessories with firearms in the same transaction (FR-009) (depends on T035, T040)
- [X] T040 [US1] In `src-tauri/src/services/insurance_status.rs`, compute the blanket total over unscheduled active firearms **and** accessories with a count of each in `load_context`, and rename `firearm_warning` to `record_warning` (logic unchanged; update callers in `commands/firearms.rs`, the tests and the comment in `src/features/insurance/coverage.ts`); in `src-tauri/src/services/valuation.rs`, return the `ValueSummary` of contracts/tauri-commands.md "Insurance (amended)" (`firearmsTotal`, `accessoriesTotal`, `blanket.accessoryCount`, `record: RecordRef` in `individuallyScheduled` and `uninsured`), active records only. Make T020 pass together with T039 (depends on T033)
- [X] T041 [US1] In `src-tauri/src/services/suggestions.rs`, make `FieldVocabulary::load` read `make`, `model`, `caliber` and `cartridge` from `firearms UNION ALL accessories`, and the model suggestions' same-make rule read both tables (research.md §16); `settle_entry` in `src-tauri/src/commands/entries.rs` needs no other change. Make T018 pass (depends on T035)
- [X] T042 [US1] Add `Accessory` to `DraftKind` in `src-tauri/src/models/database.rs`; in `src-tauri/src/session/pending.rs`, let `validate_draft` allow an accessory every mode a firearm has, and check an accessory target against `accessories` in the resumable check. Make T023 pass (depends on T004)
- [X] T043 [US1] Frontend plumbing: in `src/features/accessories/accessoriesService.ts`, one typed wrapper over `invoke` per accessory command (`listAccessoryKinds`, `createAccessory`, `updateAccessory`, `getAccessory`, `listAccessories`, `disposeAccessory`, `reverseAccessoryDisposition`, `deleteAccessory`, `assignAccessoryCoverage`); in `src/features/media/mediaService.ts` and `src/features/media/types.ts`, take and return `owner: RecordRef`; in `src/features/insurance/types.ts` and `insuranceService.ts`, the new `ValueSummary` and policy-deletion shapes; in `src/features/app/collectionStore.ts`, add `accessories`, `accessoriesById`, `accessoryKinds` and a `useAccessoryKinds()` hook shaped like `useFirearmTypes()`, loaded in `src/features/app/CollectionProvider.tsx` beside the type list and refreshed after any accessory change (research.md §20) (depends on T013, T035–T040)
- [X] T044 [US1] In `src/features/app/navigation.ts`, add the routes `{ page: "accessories" }` and `{ page: "accessory"; id: number; from: "accessories" | "collection" | "insurance" | "firearm" | "accessory" }` and the `ShellDialog` `"addAccessory"`; in `src/features/app/AppShell.tsx`, put the Accessories tab between Collection and Insurance, render `AccessoriesPage` and `AccessoryRecordPage`, own the add-accessory dialog, and resume `kind: "accessory"` drafts into `AccessoryForm` or the dispose, restore or coverage dialog. Make T032 pass (depends on T043, T048, T049, T050)
- [X] T045 [P] [US1] Create `src/features/mounts/RecordName.tsx`: given a `RecordLabel` (or a firearm or accessory record), render `FirearmName` for a firearm and FR-005's "{make} {model} · {kind}" for an accessory, one of make and model alone as it is, the kind alone when both are blank; an optional `link` prop renders it as a link through `navigation.open`. Make T025 pass (depends on T013)
- [X] T046 [P] [US1] Add 12 entries to `DRAWINGS` in `src/features/browse/typeDrawings.ts`, keyed `optic`, `light`, `magazine`, `stock`, `upper`, `barrel`, `muzzle`, `conversion`, `mount`, `sling`, `case` and `accessory`, in the same 320×200 box, muzzle-right convention and part/open/detail line roles as the firearm drawings, with separate subpaths per stroke, no brand marks; record in the file's header comment that they were drawn for the project, traced from side-on photographs for proportion, and are GPL-3.0-only with the rest of the source (research.md §15; constitution, Licensing). Make T029 pass
- [X] T047 [US1] In `src/features/media/PhotoGallery.tsx` and `src/features/media/DocumentList.tsx`, take `owner: RecordRef` instead of a firearm or `firearmId`, including the file-drop path (`add_*_from_path` with the owner, `listenForFileDrops` unchanged); update `src/features/firearms/FirearmRecordPage.tsx` to pass `{ kind: "firearm", id }`. Existing firearm tests must still pass (depends on T043)
- [X] T048 [US1] Create `src/features/accessories/AccessoryForm.tsx` per contracts/ui-accessories.md §3 (the Mounted on row is US2's): a large-layout `Dialog` following `FirearmForm`'s grid classes (`hd-form-grid--*`, `hd-field--third` for serial number, `hd-field--quarter` for estimated value) and `EntryField`s with 004's caliber derivation row (`caliberDerivation.ts`); Kind a required `Select` of offered kinds plus the record's own when no longer offered, with its hint; the value hint; the Acquired group as the firearm form's; field errors as the firearm form shows them; unsaved-changes handling and `usePendingDraft` with `kind: "accessory"` and a label by `RecordName`, `FORM_VERSION = 1`. Make T026 pass (depends on T043, T045)
- [X] T049 [US1] Create `src/features/accessories/AccessoryRecordPage.tsx` per contracts/ui-accessories.md §12, using `FirearmRecordPage`'s layout classes from `src/features/firearms/record.css`, `PhotoGallery` and `DocumentList` with the accessory owner (drops work as on a firearm's record), `DispositionHistoryList`, and the Coverage side panel; Edit opens `AccessoryForm`; Mark disposed, Restore and Delete use the shared dialogs (T050). Make T027 pass (depends on T047, T048, T050)
- [X] T050 [US1] Make `src/features/firearms/DisposeDialog.tsx`, `src/features/firearms/RestoreDialog.tsx` and `src/features/insurance/CoverageDialog.tsx` take a record of either kind (`record: { ref: RecordRef; label: RecordLabel; … }` or a discriminated firearm/accessory prop), naming it by `RecordName`, calling the matching `dispose_*`, `reverse_*` or `assign_*_coverage` command and keeping drafts with the record's kind; keep their `FORM_VERSION`s (no value changes yet). Update their callers in `FirearmRecordPage.tsx`. Make T030 pass (depends on T043, T045)
- [X] T051 [US1] Create `src/features/accessories/AccessoriesPage.tsx` and `accessories.css` per contracts/ui-accessories.md §2 without search and grouping (US4): the layout switch, "Show disposed" checkbox, Add accessory button, list columns Accessory, Value and Coverage, tiles with the thumbnail or the kind's `TypeDrawing`, the empty message, disposed rows marked as on the collection page; state remembered for the session as the collection page's is. Make T028 pass (depends on T043, T045, T046)
- [X] T052 [US1] Insurance UI: in `src/features/insurance/InsurancePage.tsx`, add the "Accessories {accessoriesTotal}" line and the blanket line's "{n} firearms and {m} accessories"; in `PolicyCard.tsx` and `coverage.ts`, key scheduled and uninsured entries by `RecordRef` and list accessories by `RecordName`; in `PolicyDeleteDialog.tsx`, read `scheduledRecords`, `scheduledRecordCount` and `blanketRecordCount`. Make T031 pass (depends on T043, T045)
- [X] T053 [US1] In `src-tauri/examples/human_seed.rs`, seed through `ops` (data-model.md "Seed and coverage"): one accessory of every kind, with and without each optional field; the pair-of-magazines record; an accessory scheduled under a policy; disposed accessories with retained history covering every `disposition_type` value the `CHECK` allows; accessories with photos (one with two, thumbnail switched) and a document. `human_seed_coverage_test` must pass for every `accessories` column and for `photos.accessory_id`, `document_attachments.accessory_id` and `disposition_history.accessory_id`, fixed by seeding, not by loosening the test (depends on T037, T038, T039)

**Checkpoint**: Accessories are recorded, valued, insured, disposed, restored, deleted and listed, with photos and documents; `accessory_test`, `record_identifier_test`, `list_accessories_test`, the media, value, insurance, reversal, wipe, pending and backup tests, and the frontend tests above pass.

---

## Phase 4: User Story 2 - Mount an Item on a Firearm (Priority: P2)

**Goal**: Any active accessory or firearm can be mounted on any active firearm or accessory, one host at a time, nested without loops, from the record's form or from the host's Mount menu; both records, the collection page and the Accessories page show it.

**Independent Test**: Mount accessories and firearms on firearms and on accessories, nested several deep, from either record, move them between hosts (with what is mounted on them), unmount them, and confirm both records show the current mount, that no item is ever on two hosts or mounted on itself through others, and that every rule in FR-010 to FR-013 blocks or allows as stated (spec US2).

### Tests for User Story 2 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T054 [P] [US2] Write failing `src-tauri/tests/mount_test.rs` through `ops`:
  - setting an Optic's `mountedOn` to a Rifle on update makes `get_accessory`'s `mount.chain` the Rifle's label and `get_firearm`'s `mount.mounted` list the Optic at depth 1 (US2-1); `mount_record` of a Suppressor firearm on the Rifle shows on both (US2-2);
  - mounting the Suppressor on an AR moves it in one step, leaving one `mounts` row (US2-3); unmounting (`host: null`) leaves no row and no record of it (US2-5, FR-013);
  - a Receiver with an Upper carrying a Scope (carrying a Red dot) and a Light: the Receiver's `mounted` is Upper (1), Scope (2, host Upper), Red dot (3, host Scope), Light (2, host Upper), depth-first in that order; the Red dot's `chain` is Scope, Upper, Receiver (US2-10, US2-12);
  - unmounting the Upper keeps the Optic on it; mounting it on a second Receiver moves both; moving the Optic to a second Upper removes it from the first (US2-9, US2-13, US2-14);
  - nothing is judged: Optic on Optic, Upper on Sling, Suppressor firearm on Upper, a .22 WMR Suppressor on a .308 Rifle, a pair-of-magazines record on a pistol, all mount with no warning (US2-7, US2-8, US2-15, FR-011);
  - loops: mounting a record on itself, or the Upper on its own Optic, through the form or `mount_record`, fails with `VALIDATION_ERROR` and exactly "A firearm can't be mounted on itself, or on something mounted on it." on `mountedOn` (form) or `host` (`mount_record`); a disposed or missing host fails with "Choose an active firearm or accessory."; a missing item is `NOT_FOUND` (US2-16, FR-010);
  - `list_mount_candidates` role `host` for the Upper excludes the Upper, the Optic and anything below it and includes active firearms and accessories only (US2-6, US2-16); role `item` for the AR excludes the AR, its chain and what is directly on it, and gives the Optic mounted on the Rifle with `mountedOn` the Rifle's label (US2-4, US2-3a); matching is case-insensitive, inside a value, on make, model, nickname and serial number; ordered by name then id; `query: ""` returns the first 50, `limit` caps at 100;
  - `dispose_firearm` on a Rifle with records mounted leaves them active and unmounted, and disposing a mounted Optic unmounts it (FR-014's default);
  - backstops by raw SQL: a mount row with a disposed item or host aborts; a second row for the same item fails `UNIQUE`; a one-step self-mount fails the `CHECK`; `UPDATE firearms SET status = 'disposed'` on a mounted firearm aborts, and the same for an accessory host; deleting either record cascades its rows away (data-model.md "Rules")
- [X] T055 [P] [US2] In `src-tauri/tests/list_firearms_test.rs`, add (US2-11, FR-016a): a Suppressor mounted on a Rifle nicknamed "Deer rifle" has `mountedOn` the Rifle's `RecordLabel` (with nickname); the Rifle with the Suppressor, an Optic and a Light has `mountedCount` 3; the Receiver of T054 has 4; a firearm neither mounted nor carrying has `mountedOn: null` and `mountedCount: 0`; groups and search are unchanged
- [X] T056 [P] [US2] In `src-tauri/tests/list_accessories_test.rs`, add: an Optic on an Upper on a Receiver has `mountedOn` the Upper's label only (direct host, FR-013); an unmounted accessory has `null`
- [X] T057 [P] [US2] In `src-tauri/tests/backup_test.rs`, add a mount of an accessory on a firearm and of a firearm on an accessory: both survive a backup and a restore of it (FR-025)
- [X] T058 [P] [US2] Write failing `src/features/mounts/MountChooser.test.tsx` per contracts/ui-accessories.md §4: labelled "Mounted on", empty reads "Not mounted"; typing calls `list_mount_candidates` with role `host`, debounced; each option shows the `RecordName`, then muted type or kind and serial number, and "Mounted on {host}" on a second line for a mounted candidate; "Not mounted" at the top and the × button clear it; choosing a mounted candidate asks nothing; ↑ ↓ Enter Esc work; the listbox has `aria-label="Mounted on"`
- [X] T059 [P] [US2] Write failing `src/features/mounts/MountedSection.test.tsx` per contracts/ui-accessories.md §5: a `<ul>` depth-first; direct entries at the edge with **Unmount** (accessible name "Unmount {name}"); deeper entries one fixed indent in with "on {host}" as the link's `aria-describedby`; every name a link; "Nothing mounted." beside **Mount ▾** when empty; the menu offers **New accessory…** and **Existing accessory or firearm…**; the latter opens a `Dialog` "Mount on {name}" whose `Combobox` calls role `item`; choosing an unmounted record mounts it at once with the toast "{name} mounted on {this record}."; choosing one with "Mounted on X" first shows a `ConfirmDialog` "Move {name}?" with body "It is mounted on {X}. Moving it takes everything mounted on it along." and **Move** / **Cancel**, and Cancel mounts nothing (US2-3a); Unmount asks nothing and toasts "{name} unmounted."; the words "item" and "host" appear nowhere in the rendered text
- [X] T060 [P] [US2] In `src/features/firearms/FirearmForm.test.tsx`, add: a **Mounted on** `MountChooser` after Type and Action with the hint "Only if this firearm is mounted on another firearm or an accessory, such as a suppressor on a rifle."; it is sent as `mountedOn` on save (`null` when not mounted); an edit shows the current host; a `mountedOn` field error shows on it; `FORM_VERSION` is 4 and a version 3 draft is discarded (research.md §19)
- [X] T061 [P] [US2] In `src/features/firearms/FirearmRecordPage.test.tsx`, add: the header facts show "Mounted on {host}, on {its host}" with each a link when the firearm is mounted, and nothing when not (§6); the Mounted section sits after Notes and before the free-text Accessories subsection, which is unchanged ("No accessories recorded." with its Add link when empty) (US2-10); a disposed firearm has no Mounted section; Mount → New accessory… opens `AccessoryForm` with Mounted on preset to this firearm
- [X] T062 [P] [US2] In `src/features/accessories/AccessoryRecordPage.test.tsx` and `src/features/accessories/AccessoryForm.test.tsx`, add: the chain in Details ("Mounted on BCM upper · Upper receiver, on LaRue PredatAR · Rifle", US2-12); the Mounted section after Notes and before Documents; no Mounted section when disposed; the form's **Mounted on** row 5 with the hint "The firearm or accessory it is on now, if any."; opened from Mount → New accessory… the title is still "Add accessory" with Mounted on preset and changeable (US2-4)
- [X] T063 [P] [US2] In `src/features/browse/BrowseList.test.tsx` and `src/features/browse/BrowseTiles.test.tsx`, add per contracts/ui-accessories.md §9: a mounted firearm shows a muted "Mounted on {host's RecordName}" line under its name with the host as a link; a carrying firearm shows "{n} mounted", not linked; both lines, "Mounted on …" first, when both; neither when neither. In `src/features/accessories/AccessoriesPage.test.tsx`, add the Mounted on column (a link, or "—") and the tile's "Mounted on {host}" link
- [X] T064 [P] [US2] Write failing `e2e/specs/us12-accessories.e2e.ts` using real keyboard input (`e2e/support/realInput.ts`; DEVELOPMENT.md, "Real keyboard and mouse input"): from the keyboard only, open a seeded Rifle, open Mount → New accessory…, add an Optic (kind, make, model, serial, value) and save, and see it in the Mounted section; mount a Suppressor firearm on the Rifle through Existing accessory or firearm…; then mount the Suppressor on an AR from the AR's page and confirm the move; assert the whole path took well under a minute (a guard, SC-001)

### Implementation for User Story 2

- [X] T065 [US2] Implement `src-tauri/src/services/mounts.rs` per research.md §5: `MountGraph::load(conn)` from one `SELECT item_firearm_id, item_accessory_id, host_firearm_id, host_accessory_id FROM mounts` into `HashMap<RecordRef, RecordRef>` (item → host) and its reverse (host → items, in id order); `host_of`, `chain` (iterative), `below` (iterative depth-first, each `(RecordRef, host, depth)`), `count_below` (memoized for many hosts), `would_loop(item, host)` (true when `host == item` or `host` is in `below(item)`); `set_mount(conn, graph, item, host: Option<RecordRef>, field: &str)`, which checks both records are active ("Choose an active firearm or accessory."), then `would_loop` ("A firearm can't be mounted on itself, or on something mounted on it."), then deletes, updates or inserts the item's one row; `labels(conn, refs) -> HashMap<RecordRef, RecordLabel>` with one query per table (`WHERE id IN (SELECT value FROM json_each(?))`), firearms joined to their type and accessories to their kind; and `detail(conn, record) -> MountDetail`. Nothing reads a kind, type, caliber or cartridge (FR-011) (depends on T011, T005)
- [X] T066 [US2] In `src-tauri/src/commands/mounts.rs`, add `ops::mount_record` (`{ item, host }` → `{ item: RecordLabel, host: RecordLabel | null }`; `NOT_FOUND` for a missing item; field `host`) and `ops::list_mount_candidates` (roles `host` and `item` as contracts/tauri-commands.md "list_mount_candidates" defines them, case-insensitive substring match on make, model, nickname and serial number over active records of both tables, ordered by name then id, `limit` default 50 and at most 100, each with its current direct host's label), read-only through `session.read`; register both in `main.rs`. Make T054's `mount_record` and candidate cases pass (depends on T065)
- [X] T067 [US2] Add `mounted_on: Option<RecordRef>` (`#[serde(default)]`) to `FirearmInput` and to `Firearm` (read through the graph or a join on `mounts`) in `src-tauri/src/models/firearm.rs`, copied by `From<&Firearm> for FirearmInput`; add `mounted_on: None` to every `FirearmInput { .. }` literal under `src-tauri/tests/`, in `src-tauri/examples/human_seed.rs` and in `src-tauri/src/commands/import_export.rs` (`grep -rn 'FirearmInput {' src-tauri`). In `src-tauri/src/commands/firearms.rs`: `create_firearm`/`update_firearm` call `mounts::set_mount` with field `mountedOn` in the save's transaction; `FirearmDetail` gains `mount: MountDetail`; `list_firearms` loads the graph once and adds `mountedOn: Option<RecordLabel>` and `mounted_count` to `FirearmSummary` with one label query per table, and builds groups with a `HashMap` from key to group index (research.md §12, §13); `dispose_firearm` deletes every mount whose item or host is the firearm before setting `status`, and `reverse_disposition` restores none. Make T054's form, dispose and backstop cases and T055 pass (depends on T065)
- [X] T068 [US2] The same for accessories in `src-tauri/src/models/accessory.rs` and `src-tauri/src/commands/accessories.rs`: `mountedOn` on `AccessoryInput` and `Accessory`, set in `create_accessory`/`update_accessory`'s transaction; `get_accessory`'s real `mount: MountDetail`; `list_accessories`' `mountedOn` label (direct host only); `dispose_accessory` deletes its mounts first. Make T056 and the rest of T054 pass (depends on T065, T067)
- [X] T069 [US2] Frontend plumbing: `mountRecord` and `listMountCandidates` in `src/features/mounts/mountsService.ts`; `mountedOn: RecordRef | null` on `Firearm` and `FirearmInput` in `src/features/firearms/types.ts` and on the accessory types; `mount: MountDetail` on both details; `mountedOn: RecordLabel | null` and `mountedCount` on `FirearmSummary` in `src/features/browse/types.ts`; the collection store refreshes firearms and accessories after any mount change (depends on T066, T067, T068)
- [X] T070 [P] [US2] Create `src/features/mounts/MountChooser.tsx` over the shared `Combobox` per contracts/ui-accessories.md §4, with `record: RecordRef | null` (the record being placed) and `role` props so the host page's dialog reuses it with role `item`. Make T058 pass (depends on T069)
- [X] T071 [US2] Create `src/features/mounts/MountedList.tsx` (the flat depth-first outline of research.md §14, shared with the dispose dialog in US3), `src/features/mounts/MountedSection.tsx` (the list, **Mount ▾** `Menu`, the "Mount on {name}" dialog with `MountChooser` role `item`, the move `ConfirmDialog`, Unmount and the toasts) and `src/features/mounts/MountedOnChain.tsx`, styled in `src/features/mounts/mounts.css` with one fixed indent whatever the depth, designed with the `frontend-design` skill as research.md §14 says. Make T059 pass (depends on T045, T070)
- [X] T072 [US2] In `src/features/firearms/FirearmForm.tsx`, add the Mounted on `MountChooser` after Type and Action with its hint, carry `mountedOn` in `FormState` and in the input, show its field error, and set `FORM_VERSION = 4`. Make T060 pass (depends on T070)
- [X] T073 [US2] In `src/features/accessories/AccessoryForm.tsx`, add the Mounted on row 5 with its hint and a `presetMountedOn` prop. Make T062's form cases pass (depends on T070)
- [X] T074 [US2] In `src/features/firearms/FirearmRecordPage.tsx` and `src/features/accessories/AccessoryRecordPage.tsx`, add `MountedOnChain` (firearm: in the header facts beside the type; accessory: in Details) and the `MountedSection` on active records (firearm: after Notes and before the free-text Accessories subsection; accessory: after Notes and before Documents), with New accessory… opening `AccessoryForm` with `presetMountedOn` and every link following `navigation.open`. Make T061 and T062's page cases pass (depends on T071, T073)
- [X] T075 [P] [US2] In `src/features/browse/BrowseList.tsx` and `src/features/browse/BrowseTiles.tsx`, add the "Mounted on {host}" line (a `RecordName` link) and "{n} mounted" line per contracts/ui-accessories.md §9, styled in `src/features/browse/collection.css`. Make T063's collection cases pass (depends on T069, T045)
- [X] T076 [P] [US2] In `src/features/accessories/AccessoriesPage.tsx`, add the Mounted on column ("—" when not mounted) and the tile's "Mounted on {host}" link. Make T063's Accessories cases pass (depends on T069)
- [X] T077 [US2] In `src-tauri/examples/human_seed.rs`, add the user stories' mounts (data-model.md "Seed and coverage"): an Optic and a Suppressor on a Rifle nicknamed "Deer rifle"; a Receiver with an Upper carrying a Scope (carrying a Red dot) and a Light; an unmounted Upper carrying its own Optic; and a firearm mounted on an accessory, so every `mounts` column is seeded. `human_seed_coverage_test` must pass for `mounts` (depends on T067, T068)
- [X] T078 [US2] Build and run `scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us12-accessories.e2e.ts'` and make T064 pass (depends on T044, T072, T074)

**Checkpoint**: Mounting, moving, unmounting and nesting work from both records; both record pages, the collection page and the Accessories page show the mount; `mount_test`, `list_firearms_test`, `list_accessories_test`, the mount UI tests and the us12 mount path pass.

---

## Phase 5: User Story 3 - Dispose and Delete With Items Mounted (Priority: P3)

**Goal**: Disposing of a record lists everything mounted on it, each kept or disposed with it at its own optional price; deleting a record leaves what was mounted on it in the collection, unmounted, and says so first.

**Independent Test**: With items mounted, dispose, restore and delete hosts and items, choosing each option, and confirm the resulting records, mounts, value summary and database contents (spec US3).

### Tests for User Story 3 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T079 [P] [US3] Write failing `src-tauri/tests/mount_lifecycle_test.rs` through `ops`:
  - a Rifle with an Optic and a Suppressor: `dispose_firearm` with price 1200 and `withMounted: [{ Optic, 300 }]` disposes both with the same type, recipient and date and prices 1200 and 300; the Suppressor is active and unmounted; no `mounts` row involves any of them; with the Optic's price `null` it is disposed with no price (US3-1, US3-2);
  - disposing a mounted Optic unmounts it (US3-3); restoring the Rifle or the Optic restores it unmounted and leaves the other unchanged (US3-4);
  - a Rifle with a Launcher (firearm) carrying a Light: keeping both leaves the Launcher unmounted and the Light on the Launcher; disposing the Launcher with the Rifle and keeping the Light leaves the Light active and unmounted (US3-7);
  - a Rifle with an Upper carrying an Optic: the same outcomes; disposing the Upper alone unmounts it from the Rifle and lets its Optic be disposed with it or kept (US3-8); `dispose_accessory` takes `withMounted` the same way;
  - a `withMounted` record that is not below the host now fails with `VALIDATION_ERROR` and exactly `fieldErrors.withMounted = "What is mounted has changed. Close the dialog and try again."`, with nothing changed (stale dispose);
  - deleting a Rifle with records mounted leaves them in the collection unmounted, and a record mounted further down stays on its own host; deleting a mounted accessory removes it and its mount, and its host's `mount.mounted` no longer lists it; deleting an Upper with an Optic leaves the Optic unmounted (US3-5, US3-6, US3-9, FR-015)
- [X] T080 [P] [US3] In `src-tauri/tests/mount_test.rs`, add SC-004's property-style test: from a fixed seed, run 2,000 random operations over a dozen firearms and accessories (form mount, `mount_record` move or unmount, dispose with a random `withMounted` subset, restore, delete, create), and after every step assert that no item has two rows, no row involves a disposed or deleted record, `MountGraph` has no loop, and everything that was below a moved record is still below it
- [X] T081 [P] [US3] In `src-tauri/tests/import_export_test.rs`, change 001's assertion: a `disposed` row with type, recipient and date and a blank `disposition_price` now imports with no price instead of being a row error; a disposed row missing its type, recipient or date is still a row error (research.md §9)
- [X] T082 [P] [US3] In `src-tauri/tests/deletion_wipe_test.rs`, add an accessory host carrying records: after deleting it, no `mounts` row names it, every record that was on it is present and unmounted, and its unique values are not in the file (SC-006)
- [X] T083 [P] [US3] In `src/features/firearms/DisposeDialog.test.tsx`, add per contracts/ui-accessories.md §7: a mounted record shows "{name} will be unmounted from {host}." under the title; a record carrying others shows a **Mounted** group after its own price with the shared flat list; each row's `SegmentedControl` reads **Keep | Dispose with it** with Keep selected and is one tab stop changed by arrow keys; choosing Dispose with it adds an optional `MoneyField` "Price for {name}" (`hd-field--quarter`) with the hint "Leave blank if none was received separately."; the statements "Kept records mounted on {name} will be unmounted." and "Records kept with what they are mounted on stay mounted." appear as the choices make them true; Confirm sends one call with `withMounted` (blank price as `null`); the stale-mount error is shown and the list reloaded; the choices and prices are in the draft's values and resumed; `FORM_VERSION` is 2 and a version 1 draft is discarded
- [X] T084 [P] [US3] In `src/features/firearms/FirearmRecordPage.test.tsx` and `src/features/accessories/AccessoryRecordPage.test.tsx`, add per contracts/ui-accessories.md §8: with records mounted directly on it, the delete confirmation adds "{n} records mounted on it will stay in the collection, unmounted:" and their names as a list, deeper records not listed; with none, the wording is unchanged
- [X] T085 [P] [US3] In `e2e/specs/us12-accessories.e2e.ts`, add from the keyboard: Mark disposed on the Rifle, set its Optic to Dispose with it with arrow keys, enter both prices, confirm, and see the Optic disposed and the Rifle's other records unmounted

### Implementation for User Story 3

- [X] T086 [US3] In the shared disposition helper (T034), stop requiring `dispositionPrice` when `status` is `disposed` for both `validate_firearm_input` and `validate_accessory_input`; type, recipient and date stay required; `DisposeInput.price` stays a required number. Make T081 pass (research.md §9) (depends on T034)
- [X] T087 [US3] Add the shared disposal with mounted records (in `src-tauri/src/services/mounts.rs` or a helper both `ops` call) and give `DisposeInput` `with_mounted: Vec<{ record: RecordRef, price: Option<i64> }>` (`#[serde(default)]`) in `src-tauri/src/commands/firearms.rs`; `dispose_firearm` and `dispose_accessory` run in one transaction per contracts/tauri-commands.md: check every listed record is below the host now (else the stale error on `withMounted`), delete every mount whose item or host is the host or a listed record, save each listed record disposed with the host's type, recipient and date and its own price, then the host. Make T079 and T080 pass (depends on T067, T068, T086)
- [X] T088 [US3] In `src/features/firearms/DisposeDialog.tsx`, add the mounted note, the **Mounted** group using `MountedList` with a Keep | Dispose with it `SegmentedControl` and the conditional price field per row, the kept-record statements, `withMounted` in the call, the stale error with a reload of the record's `mount`, the choices and prices in the draft values, and `FORM_VERSION = 2`; add `withMounted` to the dispose wrappers in `firearmsService.ts` and `accessoriesService.ts`. Make T083 pass (depends on T071, T087)
- [X] T089 [US3] In `src/features/firearms/FirearmRecordPage.tsx` and `src/features/accessories/AccessoryRecordPage.tsx`, add the mounted records (depth 1 of `mount.mounted`) to the delete `ConfirmDialog` per §8. Make T084 pass (depends on T074)
- [X] T090 [US3] In `src-tauri/examples/human_seed.rs`, add a Rifle carrying a Launcher (firearm) with a Light, and a disposed host whose Optic was disposed with it with no price (data-model.md "Seed and coverage") (depends on T077, T087)
- [X] T091 [US3] Run `scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us12-accessories.e2e.ts'` and make T085 pass (depends on T088)

**Checkpoint**: No mount ever outlives a disposed or deleted record; the dispose dialog lists nested records with their choices and prices; the delete confirmation names what stays; SC-004's sequence holds.

---

## Phase 6: User Story 4 - Browse, Group and Search Accessories (Priority: P4)

**Goal**: The Accessories page groups by kind, make, caliber, cartridge and Mounted on, and searches every accessory text field.

**Independent Test**: With accessories of every kind, mounted and unmounted, active and disposed, group by each grouping field and search by each recorded field, and check membership and the disposed toggle (spec US4).

### Tests for User Story 4 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T092 [P] [US4] In `src-tauri/tests/list_accessories_test.rs`, add per contracts/tauri-commands.md "Group order": `kind` groups follow the kind list's order; `make`, `caliber` and `cartridge` are alphabetical with "Unspecified" last (US4-1, US4-3); `mounted_on` has one group per host record (firearm or accessory) with `host` its `RecordLabel`, sorted by name, "Not mounted" last, and two hosts with the same name are two groups (US4-2); within a group, make, model, then kind; a search matches inside the kind name, make, model, serial number, caliber, cartridge, acquisition source and notes, one- and two-character queries through `LIKE` (US4-4, FR-018); disposed accessories only with `includeDisposed` (US4-5)
- [X] T093 [P] [US4] In `src-tauri/tests/fts_search_test.rs`, add US4-6: a Rifle with an Optic mounted is not found by the Optic's model, but is found by its own free-text accessories
- [X] T094 [P] [US4] In `src/features/accessories/AccessoriesPage.test.tsx`, add: the search bar ("Search accessories…") and the "No accessories match "{query}"." message; the "Group by" `Menu` with radio items No grouping, Kind, Make, Caliber, Cartridge and Mounted on (the collection page's control); grouped by Mounted on, each heading is the host's `RecordName` link, followed by its serial number in muted text when two hosts share a name; the column that is the group heading is left out; the state is remembered for the session
- [X] T095 [P] [US4] In `e2e/specs/us12-accessories.e2e.ts`, add from the keyboard: open Accessories, group by Mounted on through the grouping menu, and see the Rifle's group and "Not mounted" last

### Implementation for User Story 4

- [X] T096 [US4] In `src-tauri/src/commands/accessories.rs`, add `query` and `groupBy` to `ops::list_accessories`: three or more characters through `accessories_fts MATCH` (quoted as `list_firearms` quotes), one or two through `LIKE` over the same eight values; grouping per the "Group order" rules with a `HashMap` from key (a `RecordRef` for Mounted on) to group index. Make T092 pass (depends on T068)
- [X] T097 [US4] In `src/features/accessories/AccessoriesPage.tsx`, add the `SearchBar` and the grouping `Menu` with `MenuRadioGroup`/`MenuRadioItem` exactly as `CollectionPage`'s, host headings as links, the duplicate-name serial number, and the omitted heading column; remember the state for the session. Make T094 pass (depends on T096)
- [X] T098 [US4] Run `scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us12-accessories.e2e.ts'` and make T095 pass (depends on T097)

**Checkpoint**: The Accessories page searches and groups within its budgets; the collection page's search is unchanged.

---

## Phase 7: User Story 5 - Accessories and Mounts Through Export and Import (Priority: P5)

**Goal**: Export writes firearms and accessories as two tables with `record_id` and `mounted_on`; import reads one or two tables, matches by identifier first, and reproduces the mounts.

**Independent Test**: Export a collection with accessories of every kind and mounts of both kinds of item, import it into an empty database and compare; re-import it into the source database with each conflict choice; import sheets with each error and warning case and check the report (spec US5).

### Tests for User Story 5 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T099 [P] [US5] In `src-tauri/tests/export_test.rs`, add per contracts/spreadsheet-format.md: the firearm header is `FIREARM_COLUMNS` exactly, with `record_id` first and `mounted_on` after `registered_to` and before `photo_filenames`; the accessory header is the 21 `ACCESSORY_COLUMNS` in order; XLSX has sheets "Firearms" and "Accessories"; CSV writes `{base}.csv` and `{base}-accessories.csv`, the second only when an accessory is exported (`accessorySpreadsheetPath` null otherwise); `record_id` is never blank; `mounted_on` is the direct host's identifier, firearm's or accessory's; accessory photos are named `a{id}_{filename}` and counted in `exportedPhotoCount`; US5-9's three filtered cases (the Rifle with Optic and Suppressor but not the unmounted Light; the Rifle, Launcher, Light and Laser; the Launcher, Light and Laser without the Rifle, the Launcher's `mounted_on` naming the Rifle); the free-text `accessories` column is unchanged (FR-007); `get_export_scope` returns the matching counts and `includesAccessories`
- [X] T100 [P] [US5] Write failing `src-tauri/tests/accessory_spreadsheet_test.rs`:
  - SC-002: a collection with accessories of every kind, scheduled and unscheduled, active and disposed (one with no price), and mounts of firearms and accessories on both, exported and imported into an empty database with the same policies by name, reproduces every record, field, `uid` and mount, as a workbook with both sheets, as two single-sheet workbooks and as two CSV files (US5-3);
  - SC-003: re-importing its own export with `skip` for every conflict creates nothing, including a firearm with no serial number and every accessory; `duplicate` gives a new `uid`; `overwrite` keeps the existing `uid` and takes the row's mount, and a blank `mounted_on` unmounts it (US5-4);
  - mount warnings with the exact wording of contracts/tauri-commands.md: unknown identifier, disposed host, not a record ID, and a two-row loop where the later row in file order (firearm table first) is left unmounted; each record imports unmounted (US5-5, FR-023);
  - row errors: blank `kind`, unknown `kind`, a bad amount, a bad date (US5-6); "record_id: "{value}" is not a record ID", "record_id: {value} is also used by row {n}" and "record_id: {value} belongs to an accessory" (and the reverse); a kind matched ignoring case and surrounding whitespace, including one with `offered = 0`; accessory text snapped to spellings on record in both tables and listed;
  - partial imports: a pre-feature firearm sheet (no `record_id` or `mounted_on`) imports as before (US5-7); the firearm table alone, with a firearm row mounted on an accessory already in the database, imports its mounts and changes no accessory; the accessory table alone resolves against the database (US5-8);
  - stops before any row: a header with both `firearm_type` and `kind`, or neither ("{file}: this isn't a HoploDex firearm or accessory table. Its header needs a firearm_type or a kind column."), two firearm tables ("{file} and {other file} both hold firearms. Pick one firearm table and at most one accessory table."), three files; a blank sheet is ignored;
  - every report entry names its table ("Accessories, row 4")
- [X] T101 [P] [US5] In `src-tauri/tests/import_matching_test.rs`, add: a firearm row whose `record_id` matches one record and whose make, model and serial number match another matches the first (identifier first); with no `record_id`, or one matching nothing, the make, model and serial number key is used as before; an accessory row's only match is its identifier
- [X] T102 [P] [US5] In `src/features/import-export/ExportDialog.test.tsx`, add per contracts/ui-accessories.md §11: counts "{n} firearms and {m} accessories" from `get_export_scope`; with the filtered scope the hint "Includes everything mounted on these firearms."; the disclosure names "accessories, with their serial numbers, values and photos" when `includesAccessories`, in the same sentence as the registration disclosure; a CSV result lists both file names
- [X] T103 [P] [US5] In `src/features/import-export/ImportDialog.test.tsx`, add: the picker allows one or two files ("Choose one file, or the firearm and accessory files together") and lists them, a third replacing the selection; the report names each row's table ("Accessories, row 4") and lists mount warnings with the other warnings; an accessory conflict is shown by its `RecordName`; the stop message naming a file is shown

### Implementation for User Story 5

- [X] T104 [US5] In `src-tauri/src/services/spreadsheet.rs`, rename `COLUMNS` to `FIREARM_COLUMNS` (update every user, including `src-tauri/tests/human_seed_coverage_test.rs`) and insert `record_id` first and `mounted_on` before `photo_filenames`; add `ACCESSORY_COLUMNS`; add `record_id` and `mounted_on` to `RawImportRow` and a `RawAccessoryRow`; add table recognition by header (`firearm_type` without `kind`, `kind` without `firearm_type`), multi-sheet XLSX reading that ignores blank sheets, and writers for a two-sheet workbook and two CSV files with the existing formula-safe cells (research.md §17)
- [X] T105 [US5] In `src-tauri/src/services/import_matching.rs`, match by `uid` first among records of the row's kind, active or disposed, then by 001's make, model and serial number key for a firearm row; an accessory row has no other key. Make T101 pass (depends on T104)
- [X] T106 [US5] Let `ops::create_firearm` and `ops::create_accessory` take an optional identifier (`Option<&str>`, already parsed) used instead of a generated one; every other caller passes `None` (research.md §17) (depends on T012, T035)
- [X] T107 [US5] In `src-tauri/src/commands/import_export.rs`, add `ops::get_export_scope` (filtered: the matching firearms plus everything below them at any depth from `MountGraph`; all: every record) and its command, registered in `main.rs`; make `export_collection` write that record set as two tables, `record_id` and `mounted_on` in both, accessory rows with kind name, policy name and scheduled amount, accessory photos as `a{id}_{filename}`, and `ExportResult.accessorySpreadsheetPath` and `exportedAccessoryCount`. Make T099 pass (depends on T104, T065)
- [X] T108 [US5] In `src-tauri/src/commands/import_export.rs`, make `import_collection` take `files: { filePath, format }[]`: read and recognise every file and sheet and stop with `VALIDATION_ERROR` naming the file (and sheet) on the cases of T100; import firearm rows, then accessory rows (an accessory `parse_row` matching `kind` by trimmed name `COLLATE NOCASE` among all kinds, `validate_accessory_input`, 004's entry rules, caliber derivation and snapping over both tables); apply the `record_id` row errors (malformed, repeated in either table, other kind's) and identifier-first matching; create with the row's identifier (T106), a duplicate with a new one; keep a `RowOutcome` map and resolve `mounted_on` after every row is settled per research.md §18 with the four warnings; `ImportConflict` gains `table`, `existingRecord: RecordRef` (replacing `existingFirearmId`) and `kindName`; `RowError`, warnings and `SnappedValue` gain `table`; `importedAccessoryCount`; `resolve_import_conflicts` applies the row's mount on overwrite and duplicate. Make T100 pass (depends on T105, T106, T107, T087)
- [X] T109 [P] [US5] In `src/features/import-export/types.ts` and `importExportService.ts`, add `getExportScope`, the new `ExportResult`, `ImportResult` and `ImportConflict` fields and the `files` input; in `src/features/import-export/ExportDialog.tsx`, read the counts and disclosure from `get_export_scope` instead of `list_firearms` and show both CSV file names. Make T102 pass (depends on T107)
- [X] T110 [US5] In `src/features/import-export/ImportDialog.tsx`, allow one or two files, list them, name each report entry's table, list mount warnings, show accessory conflicts by `RecordName`, and show the stop message. Make T103 pass (depends on T108, T109)
- [X] T111 [US5] In `src-tauri/examples/human_seed.rs`'s `write_import_samples`, add an accessories-only CSV that fills every `ACCESSORY_COLUMNS` column but `photo_filenames`, a workbook with both sheets, a sample with each mount warning and accessory row error, and keep a pre-feature firearm sheet (no `record_id` or `mounted_on`); in `src-tauri/tests/human_seed_coverage_test.rs`, extend `the_import_samples_use_every_spreadsheet_column` to accept the accessory header and the pre-006 firearm header and to require every column of both tables to be filled by some sample (contracts/spreadsheet-format.md "Column layout check"). Fix it by seeding, not by loosening the test (depends on T108)

**Checkpoint**: An export round-trips every record, identifier and mount in both formats; re-imports match by identifier; every warning and error case is reported by table and row.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T112 Extend `src-tauri/tests/performance_test.rs` with every row of research.md §22, seeding 10,000 firearms and 10,000 accessories with 5,000 mounts including chains five deep: `get_firearm`/`get_accessory` with chain and Mounted list, `mount_record` moving a subtree and unmounting, `dispose_firearm` with 20 records below, create and update with `mountedOn`, each within 1 s; `list_accessories` search and each grouping, `list_firearms` with `mountedOn` and `mountedCount`, and `list_mount_candidates`, each within 500 ms; `suggest_entries` over both tables within 50 ms (FR-026, SC-007)
- [X] T113 Run `scripts/dev-container.sh cargo test --manifest-path src-tauri/Cargo.toml --release --test performance_test` (as DEVELOPMENT.md runs it) and record each timing of T112 for the PR's performance note; fix any operation over its budget (constitution IV)
- [X] T114 [P] Add to the walk in `e2e/screenshots/screens.e2e.ts`, continuing the numbering, in light and dark, every screen of contracts/ui-accessories.md §13 (the Accessories page as a list grouped by kind, as tiles, grouped by Mounted on; the accessory form and at minimum width; an accessory record page with its chain; a firearm record page with a nested Mounted section; the Mount menu; the existing-record dialog with a mounted candidate; the move confirmation; the dispose dialog with one record set to Dispose with it; the delete confirmation naming mounted records; the collection list and tiles with "Mounted on" and "3 mounted"; the value summary; the export dialog's disclosure; the import report with a mount warning). Run `scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'` and keep before/after images for the PR
- [X] T115 [P] Add a one-line "Amended by 006 (`specs/006-accessory-links/`)" pointer at each amended anchor listed in plan.md's Project Structure: in `specs/001-firearms-inventory/` (spec.md FR-002, FR-004, FR-007 to FR-011, FR-014 to FR-020, FR-023 to FR-026, FR-030, FR-033 to FR-036 and the Key Entities; data-model.md's Firearm, Photo, DocumentAttachment and DispositionHistory tables; contracts/spreadsheet-format.md's columns and import matching; contracts/tauri-commands.md's anchors listed in 006's contracts/tauri-commands.md); in `specs/003-database-protection-management/` (FR-010, FR-039 and the pending-changes table); in `specs/004-cartridges-action-types/` (the suggestion sources, FR-009 to FR-013); in `specs/005-regulated-item-types/` (the clarification and edge case on suppressor links, and the Assumption "No links between records")
- [X] T116 [P] Update `CLAUDE.md`'s Architecture section: `commands/accessories.rs` and `commands/mounts.rs` (feature 006); `services/` gains `mounts` (`MountGraph`, every mount rule) and `record_id` (the identifier format); `models/record.rs`'s `RecordRef`/`RecordLabel` as the one shape for "a record"; photos, documents and history take an `owner`; `db::reclaim_deleted_record`; `spreadsheet`'s `FIREARM_COLUMNS` and `ACCESSORY_COLUMNS` (the two tables recognised by header); frontend `features/accessories/` and `features/mounts/` (shared by both record pages and the dispose dialog); add `AccessoryForm` to the UI-consistency list of forms
- [X] T117 Check the cross-cutting rules in the code and fix anything found: FR-011, nothing in `src-tauri/src/services/mounts.rs`, `commands/mounts.rs` or the mount paths of the dispose and import code reads a kind, type, caliber or cartridge; FR-019, `grep -rn '"uid"\|uid:' src` finds no identifier in the frontend and every Rust struct carrying `uid` marks it `#[serde(skip)]`; FR-012 wording, `grep -rniE '\b(item|host)s?\b' src/features/{accessories,mounts}` and the dispose dialog find those words only in code identifiers and comments, never in rendered text; no new migration file exists (`ls src-tauri/src/db/migrations`)
- [X] T118 Run the full gates through `scripts/dev-container.sh`: `cargo test --manifest-path src-tauri/Cargo.toml`, `npm test`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `npm run lint`, `npm run format:check`, `npm run audit`, and `npm run build && npm run test:e2e` one spec at a time (us1–us12); fix every failure (constitution, Development Workflow)
- [X] T119 Check quickstart.md end to end: every row of its scenario map has its test, and `human_seed_coverage_test` passes with `NEVER_SEEDED` unchanged; tell the user their development databases must be recreated (the schema was edited in place); draft the PR notes: before/after screenshots from T114; how the persistence changes meet Security & Data Handling (accessories, media and mounts only in the encrypted database; deletion cascades with `secure_delete` and `VACUUM`, checked by `deletion_wipe_test`; the export disclosure; no dependency, cipher or network change; plan.md's Constitution Check); and T113's figures against Principle IV's budgets
- [X] T120 On Linux, with Orca, do quickstart.md's manual check **M1** (the Mounted section and the dispose dialog with a screen reader) with `scripts/dev-container.sh --gui scripts/human-testing.sh`, and record the result for the PR (best effort, not a merge gate)
  - **Deferred** (2026-10-02) to #48, with 004's and 005's M1: not run before the PR. The ARIA it depends on is covered by `MountedSection.test.tsx`, `mountedStatements.test.ts`, `DisposeDialog.test.tsx` and `us12-accessories.e2e.ts`
- [X] T121 On Linux, do quickstart.md's manual check **M2** (the 12 accessory drawings at every size, both themes, minimum window width) with `scripts/dev-container.sh --gui scripts/human-testing.sh`, and record the result for the PR (best effort, not a merge gate)

### Close-out (at pull request time)

Done when the pull request is opened, not before (CLAUDE.md; plan.md "Close-out").

- [X] T122 Comment on issue **#53** with what 006 settled (`uid` on `firearms` and `accessories` as a lowercase v4 UUID from `services::record_id`, set at creation and fixed by trigger; the spreadsheet's `record_id` column and identifier-first matching; a created record keeps the row's identifier and a duplicate gets a new one, a data point for #31; the tests and seed for those two tables) and what is left (`insurance_policies.uid`, required for #54; `photos.uid` and `document_attachments.uid` for #30; whether `disposition_history` needs one; confirming the lookup tables need none; whether cross-table uniqueness matters beyond firearms and accessories; the acceptance tests and seed for those tables), and update the issue body's "To decide" list to match, with `gh issue comment 53` and `gh issue edit 53`
- [X] T123 [P] Link the pull request on issue **#50** with `gh issue comment 50`
- [X] T124 [P] If anything about FR-003's acquisition source changed during implementation, note it on issue **#55** with `gh issue comment 55`; otherwise record in the PR notes that nothing changed
  - Nothing changed; recorded in #61's notes

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: depends on Setup; BLOCKS every user story.
- **US1 (Phase 3)**: depends on Foundational only. The MVP.
- **US2 (Phase 4)**: depends on Foundational; its accessory parts (T056, T062, T068, T073, T076) need US1's accessory commands and pages. A firearm-only slice (Suppressor on a Rifle) needs Foundational alone.
- **US3 (Phase 5)**: depends on US2 (the graph, `set_mount`, `MountedList`) and US1 (`dispose_accessory`, the shared disposition helper).
- **US4 (Phase 6)**: depends on US1 (`list_accessories`); its Mounted on grouping needs US2's labels.
- **US5 (Phase 7)**: depends on US1–US3 (accessory validation, `MountGraph`, the optional disposition price for the round trip).
- **Polish (Phase 8)**: depends on all five stories. Close-out happens when the pull request is opened.

### Within Each User Story

- Tests first, failing, then implementation (constitution II).
- Backend `ops` before the frontend services that call them; services and store before components; shared components (`RecordName`, `MountChooser`, `MountedList`) before the pages that use them.
- Same-file tasks are sequenced by hand:
  - `0001_initial.sql`: T003 → T004 → T005
  - `models/firearm.rs`: T012 → T034 → T067 → T086
  - `models/accessory.rs`: T033 → T034 → T068
  - `commands/firearms.rs`: T012 → T038 → T067 → T087
  - `commands/accessories.rs`: T035 → T036 → T037 → T068 → T096
  - `commands/entries.rs`: T035 → T041
  - `commands/import_export.rs`: T067 → T107 → T108
  - `main.rs`: T035 → T036 → T037 → T039 → T066 → T107
  - `record_identifier_test.rs`: T009 → T016
  - `list_accessories_test.rs`: T017 → T056 → T092
  - `mount_test.rs`: T054 → T080
  - `backup_test.rs`: T012 → T024 → T057
  - `deletion_wipe_test.rs`: T022 → T082
  - `us12-accessories.e2e.ts`: T064 → T085 → T095
  - `AccessoryForm.tsx`: T048 → T073
  - `AccessoryForm.test.tsx`: T026 → T062
  - `AccessoryRecordPage.tsx`: T049 → T074 → T089
  - `AccessoryRecordPage.test.tsx`: T027 → T062 → T084
  - `AccessoriesPage.tsx`: T051 → T076 → T097
  - `AccessoriesPage.test.tsx`: T028 → T063 → T094
  - `FirearmRecordPage.tsx`: T047 → T050 → T074 → T089
  - `FirearmRecordPage.test.tsx`: T061 → T084
  - `DisposeDialog.tsx`: T050 → T088
  - `DisposeDialog.test.tsx`: T030 → T083
  - `human_seed.rs`: T038 → T053 → T067 → T077 → T090 → T111
  - `human_seed_coverage_test.rs`: T008 → T104 → T111

### Parallel Opportunities

- Foundational: T006, T007, T008 alongside T003–T005; T009, T011, T013, T014 alongside the schema.
- US1 tests T015–T032 are all different files and can be written together; T045 (`RecordName`) and T046 (the drawings) run alongside the backend T033–T042.
- US2 tests T054–T064 together; T070 (`MountChooser`), T075 and T076 alongside T071–T074.
- US3 tests T079–T085 together.
- US4 tests T092–T095 together.
- US5 tests T099–T103 together; T109 alongside T108.
- Polish T114–T116 together; T123 and T124 together.

---

## Parallel Example: User Story 1

```bash
# Failing tests, all different files:
Task: "Write accessory_test.rs"                                         # T015
Task: "Extend record_identifier_test.rs for accessories"                # T016
Task: "Write list_accessories_test.rs (plain list)"                     # T017
Task: "Extend entry_suggestions_test.rs over both tables"               # T018
Task: "Extend photo_test.rs and document_test.rs with the owner"        # T019
Task: "Extend valuation, insurance_status and policy_deletion tests"    # T020
Task: "Write RecordName.test.tsx"                                       # T025
Task: "Write AccessoryForm.test.tsx"                                    # T026
Task: "Write AccessoriesPage.test.tsx"                                  # T028

# Then, alongside the backend T033–T042:
Task: "RecordName.tsx"                                                  # T045
Task: "The 12 kind drawings in typeDrawings.ts"                         # T046
```

## Parallel Example: User Story 2

```bash
Task: "Write mount_test.rs"                                             # T054
Task: "Extend list_firearms_test.rs with mountedOn and mountedCount"    # T055
Task: "Write MountChooser.test.tsx"                                     # T058
Task: "Write MountedSection.test.tsx"                                   # T059
Task: "Extend BrowseList and BrowseTiles tests"                         # T063
Task: "Write the failing us12 E2E spec"                                 # T064

# After T065–T069:
Task: "MountChooser.tsx"                                                # T070
Task: "BrowseList and BrowseTiles mount lines"                          # T075
Task: "AccessoriesPage Mounted on column"                               # T076
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 Setup and Phase 2 Foundational.
2. Phase 3: User Story 1.
3. **STOP and VALIDATE**: run `accessory_test`, `record_identifier_test`, `list_accessories_test`, the media, value, insurance, reversal, wipe, pending and backup tests, and the accessory, record page, page, drawing, dialog, insurance and shell frontend tests.
4. Demo: record an Optic and a pair of magazines, add photos and a receipt, schedule one under a policy, see the accessories subtotal, dispose and restore one.

### Incremental Delivery

1. Foundational → schema, identifier and shared shapes.
2. US1 → accessory records (MVP).
3. US2 → mounting, nested, shown on both records and both pages.
4. US3 → disposing and deleting with records mounted.
5. US4 → grouping and searching accessories.
6. US5 → the two-table spreadsheet with identifiers and mounts.
7. Polish → performance, screenshots, amendment pointers, CLAUDE.md, the cross-cutting checks, the full gates and the manual checks; close-out with the pull request.

Each story adds value without breaking the previous ones; stop at any checkpoint to validate.

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps a task to its user story for traceability
- Verify tests fail before implementing
- Commit after each task or logical group; never touch the real databases

---

## Phase 9: Convergence

Decisions recorded with the user on 2026-10-01: US2 scenario 9 is amended (nothing is enforced about uppers; FR-011 wins); a record disposed with its host is checked in the dialog and named by the backend; CSV export is protected against formula injection with an exact round trip; the mount search also matches the kind name.

- [X] T125 Fix `assignAccessoryCoverage` in `src/features/accessories/accessoriesService.ts` to send `{ accessoryId, input: { policyId, scheduledCoverageAmount } }` as `assign_accessory_coverage` in `src-tauri/src/commands/insurance.rs` expects (scheduling an accessory fails in the real app today); amend the `assign_accessory_coverage` row in contracts/tauri-commands.md to the `input` shape; add a regression test that pins the `invoke` arguments, as `insuranceService`'s firearm call is pinned per FR-009, US1/AC8 (partial)
- [X] T126 Add `accessory/add`, `accessory/edit`, `accessory/dispose`, `accessory/restore` and `accessory/coverage` to `FORM_VERSIONS` in `src/features/session/PendingChangesDialog.tsx` with each form's `FORM_VERSION`, so an accessory draft kept on a lock can be resumed; add a regression test resuming an accessory draft of each mode per FR-027 (partial)
- [X] T127 Protect CSV export against formula injection per contracts/spreadsheet-format.md ("protected against formula injection"), Constitution V: in `src-tauri/src/services/spreadsheet.rs`, prefix `'` to any CSV text cell starting with `=`, `+`, `-`, `@`, tab or carriage return on export, and on CSV import remove one leading `'` when it is followed by one of those characters, so export then import returns the exact text (SC-002); XLSX stays as is (typed string cells). Record the rule and its one accepted ambiguity (an imported CSV cell that genuinely starts with `'` and a trigger character loses the `'`) in contracts/spreadsheet-format.md; add tests for each trigger character, a value genuinely starting with `'`, a firearm and an accessory note through a full CSV round trip, and that XLSX is unchanged (contradicts)
- [X] T128 Make a record disposed with its host that fails its own checks name itself per FR-014, US3/AC2: in `src/features/firearms/DisposeDialog.tsx`, check the disposal date against each chosen record's acquisition date per row before submit, as the host's is checked (carry the acquisition date in the mounted list's payload if it isn't there); in `commands/mounts.rs` `dispose_with_mounted`, report such a failure as `fieldErrors.withMounted` naming the record and the reason, with nothing changed; amend the `dispose_*` contract in contracts/tauri-commands.md; add backend and dialog tests (partial)
- [X] T129 Amend specs/006-accessory-links/spec.md User Story 2 scenario 9 and the short-barreled rifle edge case so nothing is enforced about uppers: both can be mounted on the receiver and the owner unmounts the one not in use, consistent with FR-010 and FR-011; check that no test asserts the old behaviour per US2/AC9 (contradicts)
- [X] T130 In `list_mount_candidates` (`src-tauri/src/commands/mounts.rs`), also match the accessory kind name and the combined "make model" text (typing "Leupold VX" or "sling" finds the record); amend FR-012 in spec.md and the command's contract; add tests per FR-012 (partial)
- [X] T131 Add a US1 part to `e2e/specs/us12-accessories.e2e.ts`, from the keyboard: add an accessory, schedule it under a policy below its value and see the under-insured warning, add a photo, edit it; run it alone with `scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us12-accessories.e2e.ts'` per SC-001, US1/AC8, Constitution II (missing)
- [X] T132 Show the "Mounted on" chain in the record header facts on both record pages (move it out of the accessory page's Details panel in `src/features/accessories/AccessoryRecordPage.tsx`), and make contracts/ui-accessories.md §6 and §12 agree; update the page tests per Constitution III (partial)
- [X] T133 Word the self-or-below mount error by the item's kind ("An accessory can't be mounted on itself…" for an accessory) in `src-tauri/src/services/mounts.rs`, and amend the two prescribed messages in contracts/tauri-commands.md; update tests per FR-010 (partial)
- [X] T134 Reword the Insurance page's empty-state text in `src/features/insurance/InsurancePage.tsx` to cover firearms and accessories; update its test per FR-009 (partial)
- [X] T135 Name the table in the import's duplicate record ID message ("also used by Accessories, row 3") in `src-tauri/src/commands/import_export.rs`, and amend contracts/spreadsheet-format.md's "Record ID" wording; update tests per FR-022 (partial)
- [X] T136 Amend contracts/ui-accessories.md §7 to give "Price for {name}" `hd-field--third`, as the standard dispose dialog's "Price received" has per Constitution III (contradicts)
- [X] T137 Add a searched `list_firearms` case to the 10,000 firearms and 10,000 accessories scale test in `src-tauri/tests/performance_test.rs` per FR-026, SC-007 (partial)
- [X] T138 Remove `#[allow(clippy::too_many_arguments)]` from `export_rows` in `src-tauri/src/commands/import_export.rs` by bundling its arguments into a struct per Constitution I (unrequested)

## Phase 10: Review of issue #56

Decisions recorded with the user on 2026-10-01 in issue #56: saving a record as disposed still unmounts it and everything on it, but an import that replaces an active host with a disposed row asks about each record mounted on it, as the dispose dialog does (each kept by default; one disposed with it takes the row's type, recipient and date and no price); there are no workaround variants of the create functions; the duplicate-column error names its file; the missing amendment pointers are added; choosing files to import never allows more than the two an import takes.

- [X] T139 Fold `create_firearm_with_uid` and `create_accessory_with_uid` into `ops::create_firearm` and `ops::create_accessory`, which take the identifier as T106 says, and pass `None` at every other caller, tests included (issue #56)
- [X] T140 Name the file (and the sheet of a workbook that holds several) in the duplicate-column error in `src-tauri/src/services/spreadsheet.rs`; amend contracts/spreadsheet-format.md and 004's data-model.md; add tests for the second file of a two-file import and for a workbook sheet per FR-022 (issue #56)
- [X] T141 Add the "Amended by 006" pointers for `stage_pending_changes` (003's contract) and `suggest_entries` and `settle_entry` (004's contract), list them in plan.md, and fix the four links in 001's data-model.md that pointed one directory too far up (issue #56)
- [X] T142 Import replacing an active host with a disposed row per FR-014: `ImportConflict.mounted` lists everything below the record; `ConflictResolution.withMounted` disposes of the chosen records with it through `mounts::ops::dispose_chain`, the steps `dispose_with_mounted` takes; a failure leaves the conflict open with nothing changed. The Replace confirmation shows `MountedChoices` (moved to `src/features/mounts/`, shared with the dispose dialog) for each such row. Amend spec.md FR-014 and the edge cases, contracts/tauri-commands.md, contracts/spreadsheet-format.md and contracts/ui-accessories.md §11 and §13; add backend and dialog tests (issue #56)
- [X] T143 Add `import-dispose-receiver.csv` to the human-testing seed's import samples, with a coverage test against the seeded database, and shoot the Replace confirmation in `e2e/screenshots/screens.e2e.ts` (65) per contracts/ui-accessories.md §13 (issue #56)
- [X] T144 Keep the import picker within its two files in `src/features/import-export/ImportDialog.tsx`: Choose… (and, with two picked, the path field) is disabled once two are chosen, a typed path counting as one, with a hint saying so; a pick that would make more than two adds nothing and says so, where a third pick used to replace the selection. Amend contracts/ui-accessories.md §11; replace the third-pick test in `ImportDialog.test.tsx` (issue #56)
- [X] T145 Say what a count or a set of firearms and accessories holds instead of "record" (issue #56): add `RecordCounts` (`src-tauri/src/models/record.rs`, `src/features/mounts/recordCounts.ts`) with `describe`/`noun` and their frontend twins; `FirearmSummary.mountedCount` becomes `mountedCounts` and `get_policy_deletion_impact`'s `scheduledRecordCount`/`blanketRecordCount` become `scheduledCounts`/`blanketCounts`; reword the collection page's mount line, the delete note, the dispose statements (naming each disposed record kept ones were on), the import report, Replace confirmation and warnings, the export dialog, the coverage overview, policy card and policy delete dialog, the accessory value hint and the policy delete refusals. Amend spec.md FR-005, FR-012, FR-016a and US2-11, contracts/tauri-commands.md and contracts/ui-accessories.md §7–§11; update and add tests (issue #56)

## Phase 11: Manual testing, 2026-10-02

Decisions recorded with the user on 2026-10-02 (spec.md "Session 2026-10-02"): an accessory's make and model are required, as a firearm's are, so none is named by its kind alone; the Accessories page counts what is mounted on an accessory, as the collection page does for a firearm; unmounting from a Mounted section asks first, and staging the section's changes until the record is saved is left to issue #17.

- [X] T146 Require an accessory's make and model (FR-001, FR-005, FR-024): `make` and `model` `NOT NULL` in `0001_initial.sql`, `String` in `Accessory`, `AccessoryInput`, `AccessorySummary`, `RecordLabel` and `ImportConflict` and in their TypeScript twins; "Make is required." / "Model is required." in `validate_accessory_input`, a blank cell an import row error, and the form's Make and Model required ("Enter the make." / "Enter the model.") as `FirearmForm`'s; drop the kind-alone name and the make grouping's "Unspecified". Give every human-seed accessory and import sample a make and model, with a blank-make and a blank-model row in `import-accessory-errors.csv`; amend spec.md, data-model.md, contracts/tauri-commands.md, contracts/spreadsheet-format.md and contracts/ui-accessories.md §2, §3; update and add tests
- [X] T147 Count what is mounted on an accessory on the Accessories page (FR-016): `AccessorySummary.mountedCounts` from `MountGraph::count_below` in `list_accessories`, shown by `MountLines` under the name in the list and on the tile; `MountLines.mountedCounts` is no longer firearms-only. Amend contracts/tauri-commands.md and contracts/ui-accessories.md §2; add tests
- [X] T148 Ask before unmounting from a Mounted section (FR-012): `MountedSection` asks "Unmount {name}?" with the standard `ConfirmDialog`, naming the record it is on and saying what stays mounted on it; Cancel leaves it mounted. Shoot it in `e2e/screenshots/screens.e2e.ts` (66); amend contracts/ui-accessories.md §5 and §13; replace the unmount tests. Comment on issue #17 that the Mounted section's changes could be staged until the record is saved
