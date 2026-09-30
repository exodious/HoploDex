# Implementation Plan: Cartridges, Action Types & Entry Suggestions

**Branch**: `004-cartridges-action-types` | **Date**: 2026-09-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/004-cartridges-action-types/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

A firearm gains an optional **cartridge** (free text, e.g. "9x19mm
Parabellum") beside its required caliber, and an optional **action type**
chosen from a fixed list filtered by firearm type. Make, model, cartridge and
caliber get a narrowing suggestion list drawn from the user's own records and,
for cartridge and caliber, a built-in catalog of about 250 common cartridges.
Same-notation variants ("smith and wesson", "9 x 19mm parabellum") snap to
the spelling already on record or in the catalog when a field is left, and on
import. Picking a catalog cartridge fills the caliber with its **bore class**
(".30", "9mm", "12 gauge"); a custom cartridge gets a visible, editable guess
read from the start of its name, or none.

Technically, the catalog is a TSV compiled into the backend and never written
to a database (research.md §1–§2). All matching, ranking, snapping and caliber
derivation live in the backend in three small services built around one
comparison function, `entry_key` (research.md §3), and are reached through
two new read commands, `suggest_entries` (per keystroke, nothing cached) and
`settle_entry` (on leaving a field). Import calls the same functions. Action
types are a seeded lookup table with a type↔action join table, a nullable
foreign key on `firearms`, a command-layer check and a trigger backstop, and
a third read command, `list_action_types`, so the frontend never copies the
mapping (research.md §10). Import switches from positional to **header-based**
column reading, because the new columns go after `caliber` and would shift
every older sheet (research.md §12). The frontend gains a shared, hand-built
WAI-ARIA `Combobox`, a pure caliber-derivation reducer, an Action select, two
browse groupings, a combined "cartridge (caliber)" display and two import
report sections.

## Technical Context

**Language/Version**: Rust 1.97+ (edition 2024, `src-tauri`), TypeScript 5.x /
React 18 (`src`), unchanged

**Primary Dependencies**: No new dependency. Reuses `rusqlite`
(`bundled-sqlcipher`, FTS5), `unicode-normalization` (already a dependency
for passphrases; NFKC for the entry key), `csv` / `calamine` /
`rust_xlsxwriter`, and `@radix-ui/react-popover` (already used by `DateField`)
for the suggestion list's layer

**Storage**: The existing encrypted SQLCipher database. `firearms` gains
`cartridge` and `action_type_id`; new `action_types` and
`firearm_type_actions` tables, seeded; `firearms_fts` gains `cartridge` and
`action_type_name`; one new index and one trigger pair. Migrations `0001`,
`0002` and `0003` are edited in place; an existing development database must
be recreated. The cartridge catalog is **not** stored: it is embedded in the
binary

**Testing**: `cargo test` integration tests against a real temporary SQLCipher
database (no mocks), including a table-driven guess corpus and catalog
invariants; Vitest + React Testing Library for the `Combobox`, the caliber
reducer, the form, browse views, record page and import report; one
WebdriverIO spec with real keyboard input; the screenshots walk. The
human-testing seed must cover both new columns and both spreadsheet columns

**Target Platform**: Desktop: Windows 10+, macOS 12+, Linux, unchanged

**Project Type**: Desktop application (Tauri: React frontend + Rust backend in
one repo)

**Performance Goals**: SC-004: the suggestion list updates within 100 ms of a
keystroke at 10,000 firearms; the backend command is held to 50 ms in the
performance test (worst case 10,000 distinct models). Grouping and search by
cartridge and action meet the existing 1 s and 500 ms budgets

**Constraints**: Fully offline; no telemetry. Suggestions are computed per
request and nothing is cached, so a deleted firearm's values vanish at once
(FR-011). Nothing is applied without the user seeing it: snapping and derived
calibers happen in the form before save, or on import with a report. Records
never depend on the catalog (FR-004). Cipher settings are untouched

**Scale/Scope**: Four user stories (P1–P4), 28 functional requirements, 2 new
columns, 2 new tables, 3 new read commands, 2 new spreadsheet columns, 2 new
group-by values, 1 new shared component, about 250 catalog entries, 0 new
error codes

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Lint/static analysis, review, small single-purpose modules, no speculative abstraction | Backend logic is split by concern: `entry_text` (the key and entry rules), `cartridges` (catalog and derivation), `suggestions` (ranking and snapping), each small and pure over its inputs; `commands/entries.rs` is thin. The one new shared component (`Combobox`) has four users on day one. No dependency or layer is added. `clippy`/`rustfmt`/`eslint`/`prettier` run locally (CI stays disabled by the owner's choice, an existing documented deviation) |
| II. Testing (NON-NEGOTIABLE) | Tests first, real persistence, one test per acceptance scenario, test isolation | Every acceptance scenario maps to a test in quickstart.md; suggestion, snapping and import tests run the real `ops` against a temporary SQLCipher database. The guess is tested against a fixture of expected results, not against itself. Tests use throwaway databases only; the seed tool keeps refusing the real data directory |
| III. UX Consistency | One component set and pattern, WCAG 2.1 AA | One `Combobox` for all four fields, following the WAI-ARIA combobox pattern (keyboard-only operation, announced list, markers in accessible names). Snap notes, the Guess tag and the cleared-action note are hints under the field, like the rest of the form. The empty group is "Not specified", the term origin grouping already uses (research.md §11). Contract: [contracts/ui-entry.md](./contracts/ui-entry.md) |
| IV. Performance | 100 ms feedback / 1 s completion, 500 ms search, no UI-thread blocking | `suggest_entries` is a covered `GROUP BY` plus in-memory ranking, held to 50 ms at 10,000 rows by a performance test; stale responses are dropped rather than queued; grouping by the new fields reuses the list query; import adds one pre-pass over rows already in memory and keeps its progress events |
| V. User Privacy | Local only, real deletion, no hidden copies | Nothing leaves the device. The catalog is static application data holding no user data. Suggestions read `firearms` at request time and keep nothing (FR-011); deleting a firearm removes its values through the existing delete + `secure_delete` + reclaim path, now tested for a cartridge (research.md §14) |
| Security & Data Handling | Encryption at rest, vetted dependencies, no cipher change | New columns live in the encrypted database; no dependency is added; no key or cipher setting changes; three new commands are read-only and go through the session's gate |
| Licensing | GPLv3-compatible dependencies and bundled data with recorded source | The catalog is written for the project and carries a GPL-3.0-only header stating its source (research.md §1); no new package |

**Result**: PASS. No violations, so no Complexity Tracking entries. The added
machinery is justified in research.md against concrete requirements: the
action-rule trigger by SC-008 ("never", §10), header-based import by FR-023
and US4-8 (§12), the hand-built combobox by FR-016 without a new dependency
(§13).

## Project Structure

### Documentation (this feature)

```text
specs/004-cartridges-action-types/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── tauri-commands.md      # deltas to 001/002/003's IPC contract; 3 new commands
│   ├── spreadsheet-format.md  # deltas to the export/import contract; header-based reading
│   └── ui-entry.md            # suggestion list, cartridge/caliber/action fields, browse, report
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

The contracts and data model are **deltas** against 001 (and 002, 003 where
they amended the same anchors). As in 002, the final task adds a one-line
"amended by 004" pointer at each amended anchor in 001's documents (FR-001,
FR-012, FR-013, FR-018 to FR-020, FR-026, the Firearm table, the spreadsheet
columns, `FirearmSummary`).

### Source Code (repository root)

The existing Tauri layout; only files that change or are added are listed.

```text
src-tauri/
├── src/
│   ├── db/migrations/
│   │   ├── 0001_initial.sql          # firearms.cartridge, .action_type_id; action_types,
│   │   │                             #  firearm_type_actions; idx_firearms_cartridge;
│   │   │                             #  action-allowed trigger pair; backup-due triggers
│   │   ├── 0002_fts5.sql             # +cartridge, +action_type_name in firearms_fts and triggers
│   │   └── 0003_seed_firearm_types.sql  # + action type and mapping seeds (name kept, research §10)
│   ├── models/
│   │   ├── firearm.rs                # cartridge, action_type_id on Firearm/FirearmInput;
│   │   │                             #  normalized() trims make/model/caliber/cartridge
│   │   └── action_type.rs            # NEW: ActionType, ActionTypesOutput
│   ├── services/
│   │   ├── entry_text.rs             # NEW: entry_key, words, initialism, check_entry_text (FR-015)
│   │   ├── cartridges/
│   │   │   ├── mod.rs                # NEW: Catalog (OnceLock), classes, derive_caliber (guess)
│   │   │   └── catalog.tsv           # NEW: ~250 entries, rank/name/caliber/aliases, GPL header
│   │   ├── suggestions.rs            # NEW: vocabulary queries, ranking, snap (shared with import)
│   │   └── spreadsheet.rs            # COLUMNS +2; read by header; duplicate-header error
│   ├── commands/
│   │   ├── entries.rs                # NEW: suggest_entries, settle_entry, list_action_types (+ ops)
│   │   ├── firearms.rs               # entry-rule and action checks on create/update;
│   │   │                             #  GroupBy::Cartridge/ActionType; FirearmSummary +2
│   │   ├── import_export.rs          # sheet pass, snapping, derivation, action parsing,
│   │   │                             #  derivedCalibers/snappedValues; export +2 columns
│   │   └── mod.rs                    # pub mod entries
│   └── main.rs                       # register the three new commands
├── examples/human_seed.rs            # cartridges, actions, variants; import samples for both columns
└── tests/
    ├── fixtures/caliber_guess_corpus.tsv   # NEW: FR-007 corpus with expected results
    ├── caliber_guess_test.rs         # NEW: SC-002
    ├── cartridge_catalog_test.rs     # NEW: catalog invariants (research §2)
    ├── entry_text_test.rs            # NEW: variant / non-variant pairs, FR-015 rules
    ├── entry_suggestions_test.rs     # NEW: US2, SC-001, SC-003, SC-005 via ops
    ├── cartridge_test.rs             # NEW: US1 persistence, self-contained records
    ├── action_type_test.rs           # NEW: US3, mapping, trigger backstop (SC-008)
    ├── list_firearms_test.rs         # + group by cartridge / action, summary fields
    ├── fts_search_test.rs            # + cartridge and action search
    ├── import_export_test.rs         # + US4, header-based reading, report lists
    ├── deletion_wipe_test.rs         # + a unique cartridge leaves no bytes behind
    ├── performance_test.rs           # + suggest_entries, grouping and search at 10,000
    ├── backup_due_tracking_test.rs   # unchanged logic; passes with the new triggers
    └── human_seed_coverage_test.rs   # is_user_table skips the two seeded lookups

src/
├── components/
│   ├── Combobox.tsx                  # NEW: shared WAI-ARIA combobox (list autocomplete)
│   ├── Combobox.test.tsx
│   └── index.ts                      # export Combobox
├── features/
│   ├── app/CollectionProvider.tsx    # loads list_action_types once per open database
│   ├── firearms/
│   │   ├── types.ts                  # cartridge, actionTypeId; ActionType types
│   │   ├── firearmsService.ts        # suggestEntries, settleEntry, listActionTypes
│   │   ├── EntryField.tsx            # NEW: Combobox + suggest/settle + snap note
│   │   ├── caliberDerivation.ts      # NEW: derived/edited reducer (research §8)
│   │   ├── caliberDerivation.test.ts
│   │   ├── FirearmForm.tsx           # four EntryFields, Cartridge|Caliber row, Action select,
│   │   │                             #  type-change clearing, save-time settle, FORM_VERSION 2
│   │   ├── FirearmRecordPage.tsx     # Cartridge and Action title cells
│   │   └── forms.css                 # Guess tag, suggestion line
│   ├── browse/
│   │   ├── types.ts                  # GroupBy +cartridge, +action_type; summary +2 fields
│   │   ├── BrowseList.tsx            # cartridge (caliber) cell; Action column; hide rules
│   │   ├── BrowseTiles.tsx           # cartridge (caliber) line
│   │   └── BrowseList.test.tsx, BrowseTiles.test.tsx   # NEW
│   └── import-export/
│       ├── types.ts                  # derivedCalibers, snappedValues
│       └── ImportDialog.tsx          # two report sections
└── **/*.test.tsx                     # FirearmForm, FirearmRecordPage, ImportDialog

e2e/
├── specs/us10-cartridges-actions.e2e.ts   # NEW: keyboard-only pick, caliber fill, group
└── screenshots/screens.e2e.ts             # + the screens in ui-entry.md §9
```

**Structure Decision**: No new project, package or layer. The feature follows
the existing split: Rust owns the catalog, matching, snapping, derivation,
validation, the action rule, search and spreadsheet I/O; React owns
presentation and the form's derived/edited state. Suggestions and settling are
new read commands rather than extra arguments on existing ones, because they
run while the user types, long before any save. The E2E spec is numbered
`us10` to continue the existing `us1`–`us9` series.

## Complexity Tracking

*No entries. The Constitution Check above shows no violations requiring
justification.*

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (research.md, data-model.md, contracts/,
quickstart.md).*

- **Code Quality**: every rule that more than one path needs (the entry key,
  the snap target, the caliber derivation, the action rule, the entry length
  cap) is one function called from the form's commands and from import, so
  the paths cannot drift. The only duplicated rule is the frontend's echo of
  FR-015 for an immediate message, with the backend as the authority, as for
  the year of manufacture in 002. Still PASS.
- **Testing**: each acceptance scenario and success criterion has a named test
  file (quickstart.md). The two manual checks (screen reader, popover
  placement at small sizes) are written as numbered procedures and are not
  merge gates. Still PASS.
- **UX Consistency**: contracts/ui-entry.md specifies the combobox's roles,
  keys and announcements, so the WCAG baseline is checkable; all automatic
  changes appear as hints under their field; the empty-group label matches
  origin's. Still PASS.
- **Performance**: the per-keystroke command has an explicit 50 ms test budget
  at the worst case; grouping and search reuse indexed paths (research.md §5,
  §11). Still PASS.
- **User Privacy**: the design adds no storage outside `firearms` and its FTS
  entry, keeps no suggestion cache, and extends the deletion wipe test to the
  new column (research.md §14). Still PASS.

**Result**: PASS. The design introduces no new violations.

## Open points for the user

- **Group label**: the spec's scenarios say "Unspecified"; the plan uses "Not
  specified" to match origin grouping (research.md §11). The spec's wording
  can be aligned when tasks are generated, or the plan changed if
  "Unspecified" is preferred for both.
- **SC-006 and snapping**: a collection that already holds two same-notation
  spellings of one value will have them joined when exported and re-imported,
  with each change reported (contracts/spreadsheet-format.md). SC-006's
  exactness holds for collections without such pairs.
