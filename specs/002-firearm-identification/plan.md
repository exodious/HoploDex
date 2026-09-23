# Implementation Plan: Firearm Identification & Markings

**Branch**: `002-firearm-identification` | **Date**: 2026-09-21 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/002-firearm-identification/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Extends the firearm record of feature 001 with how a firearm is identified
and marked: an optional origin (Domestic, Imported, Re-imported), an
optional year of manufacture on every firearm, and, for import-marked
origins, an optional country of manufacture, importer name, and the
original manufacturer's make, model and serial number. The main make,
model and serial number stay the identifying key (FR-005). Three rules
change around it: a duplicate main key is still blocked but is accepted
when both records carry a year of manufacture and the years differ
(FR-007/FR-008, amending 001's FR-032 and import matching); a
non-blocking, confirm-to-save warning appears when the original maker,
model and serial all match another active firearm (FR-009); and changing
origin away from an import-marked value asks before discarding what it
hides (FR-010).

Technically this is an additive change to the existing Tauri app: seven new
nullable columns and CHECK constraints (schema edited in place, as in
earlier unreleased-schema changes), a small extension of the FTS5 index, a
rewritten identity-clash query with a year predicate, one new query for the
original-marks warning, and seven new spreadsheet columns. The warning
travels over IPC as a domain error (`ORIGINAL_MARKS_MATCH`) that the caller
resolves by resending with `confirmedWarnings: true`, the same
confirm-and-resend shape `delete_firearm` already uses, so the check and
the save stay in one backend call. The frontend gains an origin control
with plain-language descriptions, conditional importer/original-marks
fields, a discard confirmation, a warning confirmation, a labeled
"Original maker's marks" block on the record page, an origin grouping
option, import-report warnings, and a short "how to record it" guide with
worked examples.

## Technical Context

**Language/Version**: Rust 1.75+ (backend, `src-tauri`), TypeScript 5.x /
React 18+ (frontend, `src`) — unchanged from feature 001

**Primary Dependencies**: No new dependencies. Reuses `rusqlite`
(`bundled-sqlcipher`, `fts5`), `chrono` (local-date year bound), `csv` /
`calamine` / `rust_xlsxwriter` (spreadsheet), and the existing Radix-based
component library (`Dialog`, `ConfirmDialog`, `ChoiceCards`, `Select`,
`TextField`, `Field`)

**Storage**: The existing single encrypted SQLCipher database. `firearms`
gains `origin`, `year_of_manufacture`, `country_of_manufacture`,
`importer_name`, `original_make`, `original_model`,
`original_serial_number`. `idx_firearms_active_identity` stops being a
unique index and a `BEFORE INSERT/UPDATE` trigger takes over as the exact
backstop for FR-007/FR-008. `firearms_fts` gains the seven searchable
values. Per the spec's Assumptions and `schema-edited-in-place`, migrations
`0001`/`0002` are edited directly; an existing development database is
recreated

**Testing**: `cargo test` integration tests against a real temporary
SQLCipher database (no mocks); Vitest + React Testing Library for form,
record page, restore dialog, guide, and import report; WebdriverIO E2E
through `tauri-driver`, run one spec at a time with scratch XDG dirs. The
human-testing seed (`examples/human_seed.rs`) must cover every new column
and spreadsheet column or `human_seed_coverage_test` fails

**Target Platform**: Desktop — Windows 10+, macOS 12+, Linux — unchanged

**Project Type**: Desktop application (Tauri: React frontend + Rust backend
in one repo)

**Performance Goals**: Constitution Principle IV budgets hold at 10,000
records. The FR-009 lookup runs on every create, edit, restore and import
row, so it is written to use a new partial index on
`original_serial_number` (case-insensitive, active rows only) instead of a
`lower(trim())` scan; search over the added FTS columns stays within the
500 ms budget; the trigger backstop is served by the existing identity
index, now non-unique

**Constraints**: Fully offline; no operation blocks the UI thread; no
telemetry; the app records what the owner states and never judges a
mark, year, import date or legality (FR-006); no importer city, state or
address is ever stored (spec Clarifications); the warning and the
block are enforced in the backend, not only in the form

**Scale/Scope**: Same single-user collection (up to ~10,000 records). Four
user stories (P1–P4), 15 functional requirements, 7 new columns, 7 new
spreadsheet columns, 1 new error code, 1 new group-by value, 1 in-app
guide

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Lint/static analysis, review, small single-purpose modules, no speculative abstraction | Changes stay inside the existing modules (`models::firearm`, `commands::firearms::ops`, `services::{import_matching, spreadsheet}`, migrations, the `firearms` and `browse` features); the only new units are a small `Origin` enum, one warning query, and a static guide component. No new dependency or layer. `clippy` / `rustfmt` / `eslint` / `prettier` run locally as in 001 (CI stays disabled by the owner's choice, an existing documented deviation) |
| II. Testing (NON-NEGOTIABLE) | Tests first, red-green, real persistence, one test per acceptance scenario | Every acceptance scenario in spec.md gets a `cargo test` integration test against the temp SQLCipher DB (create, edit, restore, import, search, export round-trip) written to fail before the implementation; form and record-page behavior gets Vitest tests; SC-007's first-use flow gets a WebdriverIO spec. Any bug found later gets a regression test |
| III. UX Consistency | One component set, one confirmation pattern, WCAG 2.1 AA | Both new prompts (discard, original-marks warning) use the shared `ConfirmDialog`; the guide uses the shared `Dialog`; the origin control uses the existing `ChoiceCards`/`Select` patterns with visible descriptions rather than tooltips; errors use the standard `fieldErrors` shape. Contract: [contracts/ui-identification.md](./contracts/ui-identification.md) |
| IV. Performance | 100 ms feedback / 1 s completion, 500 ms search, no UI-thread blocking, progress on bulk work | New checks are indexed lookups inside the existing async commands; import keeps its per-row progress events; a performance test extends `performance_test.rs` for the FR-009 lookup and the widened FTS index at 10,000 rows |
| V. User Privacy | Local storage only, no transmission, real deletion, clear export disclosure | Nothing leaves the device; the new fields are ordinary columns in the encrypted database and the FTS index, removed with the row by the existing `DELETE` + `secure_delete` + `VACUUM` path; export already discloses its destination and now carries seven more columns. The importer's location is deliberately never stored |
| Security & Data Handling | Encryption at rest, opt-in network features, vetted dependencies | No new dependency, no network feature, no change to key handling |

**Result**: PASS — no violations, so no Complexity Tracking entries. The
one piece of added machinery (the identity trigger) is justified in
[research.md](./research.md) §3 against the concrete requirement it backs
(SC-008: 100% of tested cases), not speculatively.

## Project Structure

### Documentation (this feature)

```text
specs/002-firearm-identification/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── tauri-commands.md         # deltas to 001's IPC contract
│   ├── spreadsheet-format.md     # deltas to 001's export/import contract
│   └── ui-identification.md      # form, record page, prompts, guide
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

The contracts and data model here are **deltas** against
`specs/001-firearms-inventory/`: each names the 001 anchor it amends.
Feature 001's own documents are not rewritten; the final task adds a
one-line "amended by 002" pointer at each amended anchor (FR-030, FR-032,
FR-026, the Firearm table, the spreadsheet columns) so a reader of 001 is
sent to the current rule.

### Source Code (repository root)

Existing Tauri layout from feature 001; only files that change or are added
are listed.

```text
src-tauri/
├── src/
│   ├── db/migrations/
│   │   ├── 0001_initial.sql          # +7 columns and CHECKs; identity index → non-unique;
│   │   │                             #  identity trigger; original-serial partial index
│   │   └── 0002_fts5.sql             # +7 FTS columns; origin label / "United States" in triggers
│   ├── models/firearm.rs             # Origin enum (+label), 7 fields on Firearm/FirearmInput,
│   │                                 #  normalized(), validate_firearm_input() origin/year rules
│   ├── commands/
│   │   ├── firearms.rs               # find_identity_clash(+year), check_uniqueness message,
│   │   │                             #  original_marks_clash(), confirmedWarnings on create/update,
│   │   │                             #  ORIGINAL_MARKS_MATCH, reverse_disposition warning,
│   │   │                             #  GroupBy::Origin
│   │   └── import_export.rs          # parse_row(+7 columns), find_match(+year),
│   │                                 #  warnings in ImportResult / ResolveResult
│   └── services/
│       ├── import_matching.rs        # key gains the year predicate (never original marks)
│       └── spreadsheet.rs            # COLUMNS / FirearmExportRow / RawImportRow +7 columns
├── examples/human_seed.rs            # seed re-imported, imported, adopted-marks, restarted-serial,
│                                     #  warning-pair records and import samples for every new column
└── tests/
    ├── identification_test.rs        # NEW: US1/US2 fields, origin rules, year bound, discard rules
    ├── identity_uniqueness_test.rs   # + FR-007/FR-008 year exception (create, edit, restore)
    ├── original_marks_warning_test.rs# NEW: FR-009 on create, edit, restore; partial set; disposed
    ├── import_matching_test.rs       # + year predicate, original marks never match
    ├── import_export_test.rs         # + round trip of the 7 columns, row errors, import warnings
    ├── fts_search_test.rs            # + origin labels ("imported" vs "re-imported"), year, importer
    ├── list_firearms_test.rs         # + group by origin, "Not specified" group
    ├── disposition_reversal_test.rs  # + reversal blocked / warned
    ├── performance_test.rs           # + FR-009 lookup and FTS at 10,000 records
    └── human_seed_coverage_test.rs   # unchanged logic; passes once the seed covers the new columns

src/
├── components/                       # no new shared component
├── features/
│   ├── firearms/
│   │   ├── types.ts                  # Origin, ORIGIN_OPTIONS, 7 fields on Firearm/FirearmInput
│   │   ├── FirearmForm.tsx           # origin control, conditional fields, discard confirm,
│   │   │                             #  ORIGINAL_MARKS_MATCH confirm, year validation
│   │   ├── OriginGuide.tsx           # NEW: "how to record it" dialog with worked examples
│   │   ├── FirearmRecordPage.tsx     # origin/year/importer/country rows; "Original maker's marks"
│   │   ├── RestoreDialog.tsx         # shows the warning, lets the user confirm
│   │   ├── firearmsService.ts        # confirmedWarnings argument
│   │   └── forms.css / record.css    # origin control and marks block styling
│   ├── browse/
│   │   └── types.ts                  # GroupBy gains "origin"; GROUP_BY_OPTIONS gains "Origin"
│   └── import-export/
│       ├── types.ts                  # warnings on ImportResult / ResolveResult
│       └── ImportDialog.tsx          # lists import warnings apart from row errors
└── **/*.test.tsx                     # FirearmForm, FirearmRecordPage, RestoreDialog, OriginGuide,
                                      #  ImportDialog

e2e/specs/
└── us6-identification.e2e.ts         # NEW: SC-007 re-imported M1 Carbine flow, warning confirm
```

**Structure Decision**: No new project, package, or layer. This is a
column-level extension of the existing `firearms` entity, so it follows
feature 001's split exactly: Rust owns validation, uniqueness, the
warning, search and spreadsheet I/O; React owns presentation and the two
confirmations; the IPC command surface gains one optional argument and one
error code rather than new commands. The E2E spec is numbered `us6` to
continue 001's `us1`–`us5` series.

## Complexity Tracking

*No entries — Constitution Check above shows no violations requiring justification.*

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (research.md, data-model.md, contracts/, quickstart.md).*

- **Code Quality**: the design adds one enum, one query and one trigger to
  modules that already own those concerns, and keeps the three copies of
  the origin label (SQL trigger, Rust `Origin::label`, TS `ORIGIN_OPTIONS`)
  in step with a single search test that walks all three (research.md §6),
  rather than a shared abstraction. Still PASS.
- **Testing**: data-model.md's rules and the contracts' error cases map one
  to one onto the acceptance scenarios; quickstart.md lists the test file
  for each story, and none needs a mock. Still PASS.
- **UX Consistency**: contracts/ui-identification.md reuses `ConfirmDialog`
  for both prompts and `Dialog` for the guide, and states each control's
  accessible name and description so the WCAG baseline is checkable. Still PASS.
- **Performance**: the FR-009 lookup and the identity trigger are index-backed
  (research.md §3, §4); import adds no extra passes over the file. Still PASS.
- **User Privacy**: no entity stores anything off-device, and dropping the
  importer's city/state keeps the record smaller than the source data it
  mirrors. Still PASS.

**Result**: PASS — the design introduces no new violations; no Complexity
Tracking entries needed.
