# Implementation Plan: Accessory Records and Mounting

**Branch**: `006-accessory-links` | **Date**: 2026-10-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/006-accessory-links/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

**Accessories** become a second kind of record. Each has a kind from a fixed
list of twelve, optional make, model, serial number, caliber, cartridge,
value, acquisition details and notes, and the same disposition, insurance
scheduling, photos and documents a firearm has. Any active accessory or
firearm can be **mounted on** any active firearm or accessory, at most one
host at a time, nested to any depth and never in a loop. Mounts record only
the current configuration and are never judged. They show:
- on both records: a "Mounted on" chain, and a Mounted section listing
  everything below;
- on the collection page: "Mounted on …" and "N mounted";
- on the new Accessories page: grouping by "Mounted on".

Disposing of a host offers to dispose of what is mounted on it, each record
with its own price. Deleting either record removes the mount. The
spreadsheet gains an accessory table, and every row a stable record
identifier and a `mounted_on` column, so an export round-trips its mounts.
Re-importing matches by identifier, which also lets firearms with no serial
number match.

**Schema decision** (issue #50's 0.1.0 plan): the design **changes existing
tables**, so it is built in 0.1.0 (research.md §1). The changes:
- `firearms` gains `uid`;
- `photos`, `document_attachments` and `disposition_history` gain an
  `accessory_id` owner beside a now-nullable `firearm_id`;
- `pending_changes` allows `accessory` drafts.

It adds `accessory_kinds`, `accessories`, `mounts` and `accessories_fts`.
The free-text `firearms.accessories` column is untouched.

**Technical approach**:
- **Mounts** are an additive `mounts` table with an item pair and a host
  pair of foreign keys. `ON DELETE CASCADE` handles deletion, and
  `UNIQUE` item columns allow one host per item. Backstop triggers keep
  disposed records out of mounts (§4).
- **Loops, chains, subtrees and counts** come from an in-memory
  `MountGraph`, loaded once per call (§5).
- **The record identifier** is a lowercase v4 UUID in `uid`, chosen so that
  spreadsheets can't mangle it, and proposed as #53's format (§6).
- **One shape for "a record"**: `RecordRef { kind, id }`, with
  `RecordLabel` for naming over IPC (§7).
- **Mounting** is part of each form's input, plus one `mount_record`
  command for the host page's Mount menu (§8).
- **Disposal** with mounted records is one transaction (§9). It makes a
  disposed record's price optional, which the round trip needs.
- **Value, insurance and policy deletion** include accessories, keyed by
  `RecordRef` (§11).
- **The Accessories page** mirrors `list_firearms` with its own trigram
  FTS table (§12).
- **Nested mounts** are shown as a flat depth-first outline with "on …"
  lines (§14).
- **The spreadsheet** has two tables recognised by header, resolves mounts
  after every row has been saved (§17, §18).

## Technical Context

**Language/Version**: Rust 1.97+ (edition 2024, `src-tauri`), TypeScript 5.x /
React 18 (`src`), unchanged

**Primary Dependencies**: No new dependency. Reuses `rusqlite`
(`bundled-sqlcipher`, FTS5 trigram), `getrandom` (identifiers), 004's
entry-key, suggestion and caliber services, `csv` / `calamine` /
`rust_xlsxwriter` (two sheets; multi-sheet read), Radix through the shared
components (`Menu` with 005's radio items, `Combobox`, `Dialog`,
`SegmentedControl`)

**Storage**: The existing encrypted SQLCipher database. Migrations `0001`,
`0002` and `0003` are edited in place, so an existing development database
must be recreated. Changed tables:
- `firearms` (+`uid` and two triggers);
- `photos`, `document_attachments` and `disposition_history` (owner pair);
- `pending_changes` (kind `CHECK`).

New tables:
- `accessory_kinds`, seeded with 12 rows;
- `accessories`;
- `mounts`, with its backstop triggers (data-model.md);
- `accessories_fts`.

Each new table has its backup-due triggers.

**Testing**:
- `cargo test` integration tests against a real temporary SQLCipher database
  (no mocks), including:
  - raw-SQL tests of every backstop;
  - a seeded property-style sequence for SC-004;
  - a both-formats spreadsheet round trip (SC-002).
- Vitest + React Testing Library for the new page, form, record page,
  Mounted section, chooser, and the amended dispose, delete, export and
  import dialogs.
- One WebdriverIO spec with real keyboard input, and the screenshots walk.
- The human-testing seed must cover every new column and both import
  tables.

**Target Platform**: Desktop: Windows 10+, macOS 12+, Linux, unchanged

**Project Type**: Desktop application (Tauri: React frontend + Rust backend in
one repo)

**Performance Goals**: FR-026 and SC-007, at 10,000 firearms plus 10,000
accessories:
- within 1 s: opening a record with its mount detail, mounting, moving with
  a subtree, unmounting, disposing of a host and saving;
- within 500 ms: Accessories page search and grouping, and the collection
  page with its mount details;
- `suggest_entries` across both tables within 004's 50 ms.

The full table is in research.md §22.

**Constraints**:
- Fully offline, with no telemetry.
- Record only (FR-011): nothing reads a kind, type, caliber or cartridge to
  judge a mount.
- The identifier is never shown outside the spreadsheet (FR-019).
- A mount never outlives a disposed or deleted record (SC-004).
- Cipher settings are untouched.

**Scale/Scope**: Five user stories (P1–P5), 28 functional requirements and
7 success criteria.
- **Schema**: 4 new tables (1 seeded lookup of 12 rows), 5 changed tables
  and 8 new backstop triggers.
- **Interface**:
  - 12 new commands, 22 amended;
  - 2 shared IPC shapes (`RecordRef`, `RecordLabel`);
  - 1 new page, 1 new form, 1 new record page and 12 new drawings;
  - 1 new spreadsheet table, and 2 new firearm columns;
  - 0 new error codes.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Lint/static analysis, review, small single-purpose modules, no speculative abstraction | **One table and one code path per concern**: photos, documents and history get an owner pair instead of parallel accessory tables (research.md §3); the dispose, restore and coverage dialogs serve both record kinds (§20). **Rules live in single places**: `services::mounts` holds every mount rule as pure functions over one loaded graph (§5); `services::record_id` holds the identifier format. **Speculative alternatives rejected**: a `records` supertable (§2), and per-question recursive SQL (§5). `clippy`, `rustfmt`, `eslint` and `prettier` run locally. CI stays disabled by the owner's choice, an existing documented deviation |
| II. Testing (NON-NEGOTIABLE) | Tests first, real persistence, one test per acceptance scenario, test isolation | Every acceptance scenario and success criterion maps to a named test in quickstart.md. Persistence, mount, import and search tests run the real `ops` against a temporary SQLCipher database. Every backstop is tested by raw SQL. SC-004 gets a seeded random-operation test. Tests use throwaway databases only, and the seed tool keeps refusing the real data directory |
| III. UX Consistency | One component set and pattern, WCAG 2.1 AA | The Accessories page reuses the collection page's layout switch, search, grouping `Menu` and disposed toggle. The accessory form follows `FirearmForm`'s grid classes, `EntryField`s and unsaved-changes handling. The move uses the standard `ConfirmDialog`. The wording rule ("accessory", "firearm", "mount", never "item" or "host") is fixed in contracts/ui-accessories.md, as are the roles, keyboard behavior and an accessible description for each nested entry. The nested presentation is designed once (§14) and shared by the Mounted section and the dispose dialog |
| IV. Performance | 100 ms feedback / 1 s completion, 500 ms search, no UI-thread blocking | One narrow graph scan per call. Label lookups are batched by id. Accessory search uses trigram FTS5 with indexed suggestion columns, and grouping uses a `HashMap` (§12). `performance_test.rs` holds every FR-026 operation to its budget at 10,000 + 10,000 records. Import and export stay asynchronous with progress, as today |
| V. User Privacy | Local only, real deletion, no hidden copies, clear export disclosure | Accessories, their photos and documents, and mounts live only in the encrypted database. Deletion cascades and is reclaimed by `VACUUM` with `secure_delete`, and `deletion_wipe_test.rs` checks the raw file for an accessory's values, photo and document. Unmounting deletes the row, leaving no history. The export dialog names accessories, with their serial numbers, values and photos, whenever any leave (FR-021). No network access is added |
| Security & Data Handling | Encryption at rest, vetted dependencies, no cipher change | No dependency is added, and no key or cipher setting changes. The new commands go through the session's gate. Import parses two more columns with the same formula-safe writer and header rules. File drops still arrive as paths through the existing `add_*_from_path` commands, now with an owner |
| Licensing | GPLv3-compatible dependencies and bundled assets with recorded source | The 12 kind drawings are drawn for the project, traced for proportion from photographs, with no brand marks, and their source note is recorded in `typeDrawings.ts` (§15). No new package |

**Result**: PASS. No violations, so there are no Complexity Tracking entries.

## Project Structure

### Documentation (this feature)

```text
specs/006-accessory-links/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── tauri-commands.md      # RecordRef/RecordLabel; accessory, mount and export-scope
│   │                          #  commands; amended firearm, media, insurance, import/export
│   ├── spreadsheet-format.md  # the accessory table; record_id, mounted_on; recognition;
│   │                          #  matching; mount resolution; optional disposition price
│   └── ui-accessories.md      # Accessories page, accessory form and record page, Mount
│                              #  menu and chooser, Mounted section, dispose and delete,
│                              #  collection details, value summary, dialogs, screens walk
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

The contracts and data model are **deltas** against 001, as amended by 002
to 005. As in earlier features, the final task adds a one-line "amended by
006" pointer at each amended anchor:
- **001's documents**: FR-002, FR-004, FR-007 to FR-011, FR-014 to FR-020,
  FR-023 to FR-026, FR-030, FR-033 to FR-036, the Key Entities; the
  Firearm, Photo, DocumentAttachment and DispositionHistory tables; the
  spreadsheet columns and import matching; and the IPC anchors listed in
  contracts/tauri-commands.md.
- **003's**: FR-010, FR-039 and the pending-changes table.
- **004's**: the suggestion sources (FR-009 to FR-013).
- **005's**: the clarification and edge case on suppressor links, and the
  Assumption "No links between records".

### Source Code (repository root)

This is the existing Tauri layout. Only files that change or are added are
listed.

```text
src-tauri/
├── src/
│   ├── db/
│   │   ├── migrations/
│   │   │   ├── 0001_initial.sql      # firearms +uid (+2 triggers); accessory_kinds; accessories
│   │   │   │                         #  (+indexes, uid triggers); mounts (+4 backstop triggers);
│   │   │   │                         #  photos/document_attachments/disposition_history owner pair;
│   │   │   │                         #  pending_changes kind; backup-due triggers for new tables
│   │   │   ├── 0002_fts5.sql         # accessories_fts + its three triggers
│   │   │   └── 0003_seed_firearm_types.sql  # the 12 accessory kinds
│   │   └── mod.rs                    # reclaim_deleted_record (renamed; optimizes both FTS tables)
│   ├── models/
│   │   ├── record.rs                 # NEW: RecordRef, RecordKind, RecordLabel, MountedEntry, MountDetail
│   │   ├── accessory.rs              # NEW: Accessory, AccessoryInput, normalized, validate_accessory_input
│   │   ├── accessory_kind.rs         # NEW: AccessoryKind, AccessoryKindsOutput
│   │   ├── firearm.rs                # uid (serde skip), mountedOn; disposed price optional
│   │   ├── photo.rs, document_attachment.rs, disposition_history.rs   # owner: RecordRef
│   │   ├── database.rs               # DraftKind::Accessory
│   │   └── mod.rs
│   ├── services/
│   │   ├── mounts.rs                 # NEW: MountGraph (host_of, chain, below, count_below,
│   │   │                             #  would_loop), load, set_mount, labels
│   │   ├── record_id.rs              # NEW: generate (getrandom), parse
│   │   ├── insurance_status.rs       # blanket total over both tables; record_warning
│   │   ├── valuation.rs              # firearmsTotal, accessoriesTotal; RecordRef entries
│   │   ├── suggestions.rs            # vocabularies over firearms UNION ALL accessories
│   │   ├── spreadsheet.rs            # FIREARM_COLUMNS (+record_id, mounted_on), ACCESSORY_COLUMNS,
│   │   │                             #  table recognition, multi-sheet read, two-sheet/two-file write
│   │   ├── import_matching.rs        # match by uid first
│   │   └── mod.rs
│   ├── commands/
│   │   ├── accessories.rs            # NEW: the 9 accessory commands + ops
│   │   ├── mounts.rs                 # NEW: mount_record, list_mount_candidates + ops
│   │   ├── firearms.rs               # uid on create; mountedOn on save; MountDetail; summary
│   │   │                             #  mountedOn/mountedCount; dispose withMounted; HashMap groups
│   │   ├── photos.rs, documents.rs   # owner: RecordRef
│   │   ├── insurance.rs              # deletion impact and updates over both tables;
│   │   │                             #  assign_accessory_coverage
│   │   ├── import_export.rs          # get_export_scope; scope closure; accessory rows; two tables;
│   │   │                             #  uid matching; RowOutcome map; mount pass; conflict kind
│   │   ├── entries.rs                # list_accessory_kinds
│   │   └── mod.rs
│   ├── session/pending.rs            # accessory drafts
│   └── main.rs                       # register the 12 new commands
├── examples/human_seed.rs            # accessories, mounts, media, policy, import samples
└── tests/
    ├── accessory_test.rs             # NEW: US1, FR-001–FR-005, kinds, offered flag
    ├── mount_test.rs                 # NEW: US2, FR-010–FR-013, candidates, backstops, SC-004
    ├── mount_lifecycle_test.rs       # NEW: US3, FR-014, FR-015, stale dispose
    ├── list_accessories_test.rs      # NEW: US4, FR-016–FR-018
    ├── accessory_spreadsheet_test.rs # NEW: US5, FR-020–FR-024, SC-002, SC-003
    ├── record_identifier_test.rs     # NEW: FR-019, uid triggers
    ├── list_firearms_test.rs         # + mountedOn, mountedCount
    ├── fts_search_test.rs            # + not found through mounts
    ├── entry_suggestions_test.rs     # + both tables
    ├── photo_test.rs, document_test.rs  # + accessory owner, owner CHECK
    ├── valuation_test.rs, insurance_status_test.rs, policy_deletion_test.rs  # + accessories
    ├── disposition_reversal_test.rs  # + accessory; no mount restored
    ├── deletion_wipe_test.rs         # + accessory bytes, mounts
    ├── export_test.rs                # + both tables, headers, filtered closure
    ├── import_export_test.rs         # disposed row with blank price now imports
    ├── import_matching_test.rs       # + uid first
    ├── pending_changes_test.rs       # + accessory drafts
    ├── backup_test.rs                # + identifiers and mounts survive backup/restore
    ├── backup_due_tracking_test.rs   # passes with the new tables' triggers
    ├── performance_test.rs           # + research.md §22
    └── human_seed_coverage_test.rs   # is_user_table skips accessory_kinds

src/
├── components/                       # unchanged primitives (Menu radio items exist since 005)
├── features/
│   ├── app/
│   │   ├── navigation.ts             # routes accessories/accessory; ShellDialog addAccessory
│   │   ├── AppShell.tsx              # Accessories tab; accessory dialogs
│   │   ├── CollectionProvider.tsx    # loads accessory kinds and accessories
│   │   └── collectionStore.ts        # accessories, accessoriesById, useAccessoryKinds
│   ├── accessories/                  # NEW
│   │   ├── AccessoriesPage.tsx       # list/tiles, grouping menu, search, disposed toggle
│   │   ├── AccessoryForm.tsx         # add/edit; Mounted on; pending changes
│   │   ├── AccessoryRecordPage.tsx   # photos, details, chain, Mounted, documents, history, coverage
│   │   ├── accessoriesService.ts, types.ts, accessories.css
│   │   └── *.test.tsx
│   ├── mounts/                       # NEW: shared by both record pages and dialogs
│   │   ├── RecordName.tsx            # FirearmName or FR-005 naming
│   │   ├── MountChooser.tsx          # "Mounted on" Combobox (list_mount_candidates)
│   │   ├── MountedSection.tsx        # flat depth-first list, Mount menu, existing-record dialog,
│   │   │                             #  move confirmation, Unmount
│   │   ├── MountedOnChain.tsx
│   │   ├── mountsService.ts, types.ts, mounts.css
│   │   └── *.test.tsx
│   ├── firearms/
│   │   ├── FirearmForm.tsx           # Mounted on field; FORM_VERSION +1
│   │   ├── FirearmRecordPage.tsx     # chain; Mounted section; delete confirmation names mounted
│   │   ├── DisposeDialog.tsx         # either record kind; Mounted group with Keep | Dispose
│   │   │                             #  and per-record price; FORM_VERSION +1
│   │   ├── RestoreDialog.tsx         # either record kind
│   │   └── types.ts                  # mountedOn; RecordRef re-exported
│   ├── browse/
│   │   ├── BrowseList.tsx, BrowseTiles.tsx   # "Mounted on …", "N mounted"
│   │   ├── typeDrawings.ts           # + 12 kind drawings, with their source note
│   │   └── TypeDrawing.test.tsx      # every seeded kind key has a drawing
│   ├── media/
│   │   ├── PhotoGallery.tsx, DocumentList.tsx, mediaService.ts, types.ts   # owner: RecordRef
│   ├── insurance/
│   │   ├── CoverageDialog.tsx        # either record kind
│   │   ├── PolicyCard.tsx, InsurancePage.tsx, coverage.ts   # RecordRef entries; subtotal
│   │   └── *.test.tsx
│   ├── import-export/
│   │   ├── ExportDialog.tsx          # get_export_scope; accessories disclosure; two files
│   │   ├── ImportDialog.tsx          # one or two files; table names; mount warnings
│   │   └── *.test.tsx
│   └── session/SessionProvider.tsx   # accessory drafts resume into AccessoryForm/dialogs

e2e/
├── specs/us12-accessories.e2e.ts     # NEW: keyboard-only path through US1–US3, US4 grouping
└── screenshots/screens.e2e.ts        # + contracts/ui-accessories.md §13
```

**Structure Decision**: There is no new project, package or layer.
- **Rust** owns the accessory and mount rules, the identifier, validation,
  the backstops, value and insurance, grouping, search and spreadsheet I/O.
- **React** owns naming, presentation and the dialogs.
- **Two new frontend feature folders**: `accessories/` for the record kind,
  and `mounts/` for what both record pages and the dispose dialog share.
  Neither copy lives inside `firearms/`.
- **The E2E spec** is numbered `us12`, continuing the series.

## Complexity Tracking

*No entries. The Constitution Check above shows no violations requiring
justification.*

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (research.md, data-model.md, contracts/,
quickstart.md).*

- **Code Quality**: each rule more than one path needs is one function,
  called from the forms' commands, `mount_record`, the dispose commands and
  import:
  - the mount rules (`MountGraph`);
  - the identifier format (`record_id`);
  - the accessory validation;
  - the coverage warning;
  - the disposition rules (shared helpers, extracted from
    `validate_firearm_input` for both validators).

  The frontend reads the kinds from the backend. The only duplicated rule
  is `MountChooser` hiding the record's own subtree, which comes from the
  backend's candidate list, so it isn't copied either. Still PASS.
- **Testing**: each acceptance scenario and success criterion has a named
  test (quickstart.md). SC-004's "after any sequence" is a seeded random
  sequence checked after every step. The two manual checks are numbered
  procedures and not merge gates. Still PASS.
- **UX Consistency**: contracts/ui-accessories.md fixes the wording, roles,
  keys and copy. Every new piece reuses an existing pattern. The one new
  pattern, the nested outline, is defined once and used in both places.
  Still PASS.
- **Performance**: every FR-026 operation has an explicit test at the
  spec's volumes. The collection page's added cost is one narrow scan and
  two batched lookups. Still PASS.
- **User Privacy**: no storage outside the encrypted database. Deletion and
  its wipe test cover accessories, their media and mounts, and the export
  disclosure covers accessories. Still PASS.

**Result**: PASS. The design introduces no new violations.

## Findings confirmed with the user

Confirmed by the owner on 2026-10-01:

- **Build timing** (research.md §1): the design changes `firearms`,
  `photos`, `document_attachments`, `disposition_history` and
  `pending_changes`, so it is built in 0.1.0, per the owner's plan on
  issue #50.
- **A disposed record's price becomes optional** (research.md §9): a
  disposed row with a blank `disposition_price` is no longer an import
  error, in either table. The dispose dialog still requires the price for
  the record being disposed itself.
- **Identifier format for #53** (research.md §6): a lowercase v4 UUID, which
  #53 should adopt for its other tables.
- **Acquisition source stays plain text** (research.md §16): on both forms,
  with no suggestions or snapping, and FR-003 is amended to say so (the
  serial number too). The field itself is examined for both kinds of
  record in issue #55.
- **No special case for exports made before this feature** (research.md
  §18): until the first release there are none, so a missing `mounted_on`
  column reads as blank, like any missing column.

Design choices made at planning that the spec left open:

- The nested presentation is a flat depth-first outline with "on …" lines
  (§14).
- In a workbook, every non-blank sheet must be one of the two tables (§17).
- CSV accessory files are named `{base}-accessories.csv` (§17).
- The Accessories tab sits between Collection and Insurance (§20).
