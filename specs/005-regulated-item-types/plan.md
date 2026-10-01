# Implementation Plan: Regulated Item Types: Suppressors and NFA Registration

**Branch**: `005-regulated-item-types` | **Date**: 2026-09-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/005-regulated-item-types/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

**Suppressor** becomes a fifth firearm type, with its own drawing and no
action type, barrel length or capacity. Its caliber is its bore, and its
cartridge, labelled "Rated cartridge", is the most powerful one it is rated
for, so its caliber is never derived from its cartridge. Separately, any firearm can carry an optional **Registered as**
classification from a fixed list: Suppressor, Short-barreled rifle,
Short-barreled shotgun, Any other weapon, Machine gun, Destructive device.
Once a classification is chosen, three optional details can be recorded: the
**form**, the **approved** date and **Registered to**. The application
records what the owner states and never judges legal status. The action list
gains "Automatic or select-fire". Both registration fields can be grouped by,
all three text values are searched, and all four go through export and
import.

Technically, `firearm_types` gains three "applies" flags. A command-layer
check, run before 004's action mapping, enforces them, with a trigger pair as
the backstop (research.md §2). A fourth flag, `caliber_from_cartridge`,
says whether a type's caliber is derived from its cartridge. The form and
import read it, and Suppressor's is 0 (§15). A new read command, `list_firearm_types`,
replaces the frontend's hard-coded type list (§3). Classifications are a
seeded lookup table with fixed ids and an `offered` flag (§5).
`list_registration_classes` exposes them, and any known classification is
accepted on save and import. Registration is four columns on `firearms`, with
a `CHECK` that the details need a classification (§6). Form and "Registered
to" become two more `EntryField`s, so 004's suggestion, snapping and import
machinery covers them unchanged. Three built-in form names act as the form's
catalog (§7). `list_firearms` gains two groupings, the FTS table three columns
and the summary one field (§8).

The collection page's "Group by" segmented row, with nine choices, gives way
to a button that opens a sectioned radio menu: "The firearm", "Its maker",
"Registration". It was designed with the frontend design skill, as the spec
requires, and is built from new radio items on the shared `Menu` (§9). The
form gains a folded **Registration** section with a standing "records only"
note, and a confirmation before clearing a classification discards its
details (§11). 002's guide becomes `IdentificationGuide` and gains two
registration examples (§12). The legal background was checked at planning:
an August 2026 ruling has since ended registration for some owners of these
items. That confirms the record-only design and changes nothing in it (§1).

## Technical Context

**Language/Version**: Rust 1.97+ (edition 2024, `src-tauri`), TypeScript 5.x /
React 18 (`src`), unchanged

**Primary Dependencies**: No new dependency. Reuses `rusqlite`
(`bundled-sqlcipher`, FTS5), 004's entry-key and suggestion services,
`csv` / `calamine` / `rust_xlsxwriter`, and `@radix-ui/react-dropdown-menu`
(already used by `Menu`) for the grouping menu's radio items

**Storage**: The existing encrypted SQLCipher database:
- `firearm_types` gains four flags (three "applies" flags and
  `caliber_from_cartridge`) and is seeded with fixed ids, adding Suppressor
  (id 5).
- `action_types` gains id 13.
- A new `registration_classes` table is seeded with six rows.
- `firearms` gains four columns, one `CHECK`, two partial indexes and a
  trigger pair.
- `firearms_fts` gains three columns.
- Migrations `0001`, `0002` and `0003` are edited in place, so an existing
  development database must be recreated.
- The built-in form names are **not** stored: they are a constant in the
  backend.

**Testing**: `cargo test` integration tests against a real temporary
SQLCipher database (no mocks), including raw-SQL tests of both backstops and
of a no-longer-offered classification. Vitest + React Testing Library for the
form, record page, guide, export and import dialogs, the new `Menu` radio
items and the collection page. One WebdriverIO spec with real keyboard input,
and the screenshots walk. The human-testing seed must cover every new column
and spreadsheet column

**Target Platform**: Desktop: Windows 10+, macOS 12+, Linux, unchanged

**Project Type**: Desktop application (Tauri: React frontend + Rust backend in
one repo)

**Performance Goals**: SC-006: grouping by Registered as or Registered to
within 1 s and searching registration details within 500 ms at 10,000
firearms. `suggest_entries` for the two new fields within 004's 50 ms

**Constraints**:
- Fully offline, with no telemetry.
- Record only (FR-014): no rule about what is regulated exists anywhere in
  code or data, and nothing reads barrel or overall length.
- Saved classifications and details never change without an edit (FR-015).
- A Suppressor's caliber is never derived from its cartridge, on the form
  or on import (FR-002, FR-022).
- Nothing is cleared silently: the form announces what a type change will
  clear, and asks before a cleared classification discards details.
- Cipher settings are untouched.

**Scale/Scope**: Four user stories (P1–P4) and 23 functional requirements.
The schema gains 1 seeded type, 1 action, 1 new lookup table (6 rows),
4 type flags, 4 firearm columns and 3 FTS columns. The interface gains
2 new read commands, 2 new entry fields, 4 spreadsheet columns, 2 group-by
values, 1 new drawing, 3 new shared menu primitives and 0 new error codes

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Lint/static analysis, review, small single-purpose modules, no speculative abstraction | No new layer or dependency. The fields rule is one function (`check_fields_apply`) plus a backstop, like 004's action rule. Registration reuses 004's `EntryField` machinery rather than a parallel suggestion path. The new `services::registration` module holds only the form names. The three "applies" flags are the minimum the spec names: a general "omitted fields" table was rejected as speculative (research.md §2). The fourth, `caliber_from_cartridge`, is the type property FR-002 asks for. It is read only where a caliber is derived, with no save check or trigger, because a derived caliber is stored like a typed one (§15). The `offered` flag is required by FR-007 and tested by SC-004. `clippy`/`rustfmt`/`eslint`/`prettier` run locally (CI stays disabled by the owner's choice, an existing documented deviation) |
| II. Testing (NON-NEGOTIABLE) | Tests first, real persistence, one test per acceptance scenario, test isolation | Every acceptance scenario and success criterion maps to a named test in quickstart.md. Persistence, import and search tests run the real `ops` against a temporary SQLCipher database. Both backstops (the fields trigger and the details `CHECK`) are tested by raw SQL. Tests use throwaway databases only, and the seed tool keeps refusing the real data directory |
| III. UX Consistency | One component set and pattern, WCAG 2.1 AA | The Registration section is a folded `Disclosure` like Origin and Physical details. Clearing uses the shared `ConfirmDialog`, worded like origin's discard. Form and Registered to use 004's `EntryField`. An unrecorded value is "Unspecified". The grouping control adds radio items to the shared `Menu` rather than a one-off widget, with `menuitemradio` roles and full keyboard use. The guide stays one dialog. Contract: [contracts/ui-registration.md](./contracts/ui-registration.md) |
| IV. Performance | 100 ms feedback / 1 s completion, 500 ms search, no UI-thread blocking | Grouping adds one `LEFT JOIN` on a six-row table to the existing list query. Search adds three trigram columns. The two new suggestion fields have partial indexes. `performance_test.rs` holds all three to the budgets at 10,000 firearms (research.md §7, §8) |
| V. User Privacy | Local only, real deletion, no hidden copies, clear export disclosure | Registration lives only in `firearms` and its FTS entry. Deletion wipes it through the existing path, and the wipe test gains a unique "Registered to" value (SC-007). The export note names registration details whenever the scope holds any (FR-020, research.md §10). No network access is added |
| Security & Data Handling | Encryption at rest, vetted dependencies, no cipher change | New columns live in the encrypted database. No dependency is added, and no key or cipher setting changes. The two new commands are read-only and go through the session's gate |
| Licensing | GPLv3-compatible dependencies and bundled assets with recorded source | The suppressor drawing is drawn for the project, and its source note is recorded in `typeDrawings.ts` (research.md §4). No new package |

**Result**: PASS. No violations, so there are no Complexity Tracking entries.

## Project Structure

### Documentation (this feature)

```text
specs/005-regulated-item-types/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── tauri-commands.md      # deltas to the IPC contract; 2 new commands
│   ├── spreadsheet-format.md  # 4 new columns, row errors, snapping
│   └── ui-registration.md     # type fields, Registration section, record page,
│                              #  guide, export note, grouping menu
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

The contracts and data model are **deltas** against 001, as amended by 002
and 004. As in 002 and 004, the final task adds a one-line "amended by 005"
pointer at each amended anchor:
- In 001's documents: the Assumptions on types, FR-001, FR-009, FR-012,
  FR-013, FR-018 to FR-020, FR-039, the FirearmType and Firearm tables, the
  FTS table, the spreadsheet columns and `FirearmSummary`.
- In 002's: FR-006, FR-015, the Assumption on registration detail, and
  contracts/ui-identification.md §8, the guide.
- In 004's: FR-003, FR-005, FR-006 and FR-025 (no derivation for a
  Suppressor), FR-017, FR-018, FR-024, the clarification on automatic fire,
  the action seed, the `EntryField` list, and contracts/ui-entry.md §3 (the
  caliber's derivation states).

### Source Code (repository root)

This is the existing Tauri layout. Only files that change or are added are
listed.

```text
src-tauri/
├── src/
│   ├── db/migrations/
│   │   ├── 0001_initial.sql          # firearm_types flags; registration_classes (+ backup-due
│   │   │                             #  triggers); firearms +4 columns, CHECK, 2 partial indexes;
│   │   │                             #  firearms_fields_apply_* trigger pair
│   │   ├── 0002_fts5.sql             # +registered_as, +registration_form, +registered_to
│   │   └── 0003_seed_firearm_types.sql  # fixed type ids + Suppressor; action 13 and its
│   │                                 #  mappings, sort orders; the six classifications
│   ├── models/
│   │   ├── firearm.rs                # 4 fields on Firearm/FirearmInput; normalized(); validation
│   │   │                             #  (details need a class, approved date, entry rules via ipc_name)
│   │   ├── firearm_type.rs           # NEW: FirearmTypeInfo (+ caliber_from_cartridge),
│   │   │                             #  FirearmTypesOutput (+ mod.rs)
│   │   └── registration.rs           # NEW: RegistrationClass, RegistrationClassesOutput
│   ├── services/
│   │   ├── entry_text.rs             # EntryField +RegistrationForm, +RegisteredTo; ipc_name();
│   │   │                             #  serde camelCase
│   │   ├── registration.rs           # NEW: BUILT_IN_FORMS
│   │   ├── suggestions.rs            # form names as the form field's catalog
│   │   └── spreadsheet.rs            # COLUMNS +4; export row and raw row fields
│   ├── commands/
│   │   ├── firearms.rs               # check_fields_apply; GroupBy +2; summary registeredAs;
│   │   │                             #  LIKE branch +3; class-id check
│   │   ├── entries.rs                # list_firearm_types, list_registration_classes (+ ops)
│   │   └── import_export.rs          # parse registered_as and details; FR-022 mapping to
│   │                                 #  columns; settle_row covers the 2 new fields; export +4;
│   │                                 #  settle_row derives no caliber for a Suppressor (§15)
│   └── main.rs                       # register the two new commands
├── examples/human_seed.rs            # the records and import samples in quickstart.md
└── tests/
    ├── suppressor_test.rs            # NEW: US1, the fields rule and its trigger (SC-005),
    │                                 #  caliber and rated cartridge, caliberFromCartridge
    ├── registration_test.rs          # NEW: US2, FR-008/FR-014 matrix (SC-002), SC-004, CHECK
    ├── action_type_test.rs           # + Automatic or select-fire, sort order, mapping
    ├── entry_suggestions_test.rs     # + form and Registered to suggestions, snapping, deletion
    ├── entry_text_test.rs            # + ipc_name, serde names of the new fields
    ├── list_firearms_test.rs         # + Suppressor group, group by registered_as / registered_to
    ├── fts_search_test.rs            # + registration search, short-query LIKE
    ├── identity_uniqueness_test.rs   # + a Suppressor, with and without a classification
    ├── import_export_test.rs         # + US4, row errors (a Suppressor's blank caliber among
    │                                 #  them), round trip, pre-feature sheet
    ├── export_test.rs                # + column order and values
    ├── deletion_wipe_test.rs         # + a unique Registered to leaves no bytes behind
    ├── disposition_reversal_test.rs  # + details kept through dispose and restore
    ├── performance_test.rs           # + grouping, search, suggestions at 10,000
    ├── backup_due_tracking_test.rs   # passes with the new lookup's triggers
    └── human_seed_coverage_test.rs   # is_user_table skips registration_classes

src/
├── components/
│   ├── Menu.tsx                      # + MenuRadioGroup, MenuRadioItem, MenuLabel
│   ├── Menu.test.tsx
│   ├── components.css                # radio indicator, section label
│   └── index.ts
├── features/
│   ├── app/
│   │   ├── CollectionProvider.tsx    # loads list_firearm_types and list_registration_classes
│   │   └── collectionStore.ts        # useFirearmTypes, useRegistrationClasses
│   ├── firearms/
│   │   ├── types.ts                  # registration fields; FirearmTypeInfo; RegistrationClass;
│   │   │                             #  FIREARM_TYPE_OPTIONS removed; firearmTypeOption from store
│   │   ├── firearmsService.ts        # listFirearmTypes, listRegistrationClasses
│   │   ├── FirearmForm.tsx           # type-driven fields and clearing note; Rated cartridge
│   │   │                             #  and the two hints; no derivation for a Suppressor;
│   │   │                             #  Registration section and confirm; FORM_VERSION 3
│   │   ├── caliberDerivation.ts      # derives only while the type's caliberFromCartridge
│   │   ├── FirearmRecordPage.tsx     # Rated cartridge; hidden fields; Registration panel
│   │   ├── IdentificationGuide.tsx   # renamed from OriginGuide.tsx; + Registered items part
│   │   └── *.test.ts(x)              # FirearmForm, FirearmRecordPage, IdentificationGuide,
│   │                                 #  caliberDerivation
│   ├── browse/
│   │   ├── typeDrawings.ts           # + suppressor drawing, with its source note
│   │   ├── TypeDrawing.test.tsx      # NEW: every seeded key has a drawing
│   │   ├── types.ts                  # GroupBy +2; GROUP_BY_OPTIONS sections; summary registeredAs
│   │   ├── CollectionPage.tsx        # the grouping menu
│   │   ├── CollectionPage.test.tsx   # NEW
│   │   └── collection.css
│   └── import-export/
│       ├── ExportDialog.tsx          # registration disclosure
│       ├── ImportDialog.tsx          # field names Form, Registered to
│       └── *.test.tsx

e2e/
├── specs/us11-regulated-items.e2e.ts # NEW: keyboard-only registered suppressor; group by
└── screenshots/screens.e2e.ts        # + contracts/ui-registration.md §10
```

**Structure Decision**: There is no new project, package or layer. The
feature follows the existing split. Rust owns the type flags, the
classification list, validation, the two backstops, suggestions, grouping,
search and spreadsheet I/O. React owns presentation, the form's
hidden-until-save state and the grouping menu. The two lookup lists reach the
frontend through new read commands, following 004's `list_action_types`,
because the frontend must never copy a rule the database holds. The E2E spec
is numbered `us11` to continue the existing series.

## Complexity Tracking

*No entries. The Constitution Check above shows no violations requiring
justification.*

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (research.md, data-model.md, contracts/,
quickstart.md).*

- **Code Quality**: each rule that more than one path needs is one function,
  called from the form's commands and from import:
  - the fields rule;
  - details needing a classification;
  - the approved date (`checked_date`, already shared);
  - the entry rules and snapping (004's).
  The frontend reads the flags and classifications from the backend instead
  of copying them. Whether a caliber is derived is one column that the form
  and import both read, and neither names Suppressor. The only duplicated check is the `DateField`'s `max` for
  an immediate refusal, with the backend as the authority, as for the other
  dates. Still PASS.
- **Testing**: each acceptance scenario and success criterion has a named
  test (quickstart.md). SC-002's "no screen shows a hint" is automated as a
  text search over rendered screens plus a save matrix. The two manual checks
  (screen reader on the grouping menu, the drawing at every size) are
  numbered procedures and not merge gates. Still PASS.
- **UX Consistency**: contracts/ui-registration.md fixes the roles, keys and
  copy. Every new piece reuses an existing pattern: a folded section, a
  confirm-to-discard, a suggestion field, a menu, the guide. The one pattern
  change, the grouping control, goes into the shared `Menu` rather than a
  one-off. Still PASS.
- **Performance**: grouping, search and suggestions have explicit
  performance tests at 10,000 firearms, and the query plan changes by one
  join on a tiny table. Still PASS.
- **User Privacy**: the design adds no storage outside `firearms` and its FTS
  entry, and extends the deletion wipe test and the export disclosure to the
  new values. Still PASS.

**Result**: PASS. The design introduces no new violations.

## Findings to confirm with the user

- **The law moved after the spec was written** (research.md §1). A court
  ruling in effect since 13 August 2026 ends NFA registration for
  suppressors, short-barreled rifles and short-barreled shotguns, but only
  for the parties it protects. The spec's design already handles it. A
  suppressor with no classification is valid, records made before the
  ruling keep their registration, and the standing note says "the law
  changes" without naming a law. No spec change is proposed.
- **Design choices made at planning** that the spec left open:
  - The grouping control becomes a sectioned radio menu (research.md §9).
  - `OriginGuide` is renamed `IdentificationGuide` (§12).
  - The export note counts a classification alone as registration details
    (§10).
  - "Automatic or select-fire" sorts seventh, after the six common actions (§13).
- **The clarification of 2026-09-30 came after implementation** (research.md
  §15). A suppressor's caliber is its bore and its cartridge is the most
  powerful one it is rated for. The built feature still labels the caliber
  "Caliber rating" and derives it from the cartridge as for any type, so the
  schema seed, `list_firearm_types`, the form, the record page, import, the
  guide's example, the human seed, the E2E spec, their tests and the
  screenshots all change. tasks.md doesn't list that work yet;
  `/speckit-tasks` (or `/speckit-converge`) adds it.
