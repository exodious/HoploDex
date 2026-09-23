# Quickstart: Validate Firearm Identification & Markings

This is a runnable validation guide, not an implementation spec. It proves
the feature works end-to-end by walking through each user story's
independent test from [spec.md](./spec.md). Field, command and error names
refer to [data-model.md](./data-model.md) and [contracts/](./contracts).
Setup and prerequisites are the same as
[001's quickstart](../001-firearms-inventory/quickstart.md).

## Prerequisites

- Rust, Node.js and the Tauri prerequisites for your OS, as in 001.
  On Linux, the development container has them all
  (`scripts/dev-container.sh`; see README.md).
- Node via nvm on `PATH`; use `npm@11` if a lockfile changes (this feature
  changes none).
- A **fresh database**: the schema was edited in place, so an existing
  development database is incompatible and must be recreated. Never point a
  test at the real data directory.

## Automated test commands

```bash
# Rust: every acceptance scenario against a real temp SQLCipher DB
cargo test --manifest-path src-tauri/Cargo.toml

# Frontend unit tests
npm run test

# Lint and format (run locally; CI is intentionally disabled)
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
cargo fmt --manifest-path src-tauri/Cargo.toml --check
npm run lint && npm run format:check
```

E2E specs run **one at a time**. `npm run test:e2e` builds the release binary
and runs under `xvfb` on Linux, and `e2e/wdio.conf.ts` points every launch at
scratch `XDG_*` directories so a spec can never touch the real database.
Select the new spec with wdio's `--spec` argument (extra arguments pass
through `e2e/run-e2e.mjs`):

```bash
npm run test:e2e -- --spec e2e/specs/us6-identification.e2e.ts
```

All of these MUST pass before merge (Constitution, Development Workflow).

## Scenario map: story → tests

| Story | Independent test (spec) | Where it is automated |
|---|---|---|
| US1 — where it came from | Create an imported and a re-imported firearm with country, importer and year; reopen; search | `tests/identification_test.rs`, `tests/fts_search_test.rs`, `FirearmForm.test.tsx`, `FirearmRecordPage.test.tsx` |
| US2 — original marks | Imported firearm with importer serial + original maker, model, serial; both shown labeled; search the original serial | `tests/identification_test.rs`, `FirearmRecordPage.test.tsx` |
| US3 — duplicate checks | Records colliding on main marks, on original marks only, and on neither | `tests/identity_uniqueness_test.rs`, `tests/original_marks_warning_test.rs`, `tests/disposition_reversal_test.rs`, `RestoreDialog.test.tsx` |
| US4 — browse, export, import | Group by origin; export → import round trip; invalid rows | `tests/list_firearms_test.rs`, `tests/import_export_test.rs`, `tests/import_matching_test.rs`, `ImportDialog.test.tsx` |
| Backstop / performance | Raw `INSERT`s cannot create a forbidden pair; 10,000-record budgets | `tests/identity_uniqueness_test.rs` (bypasses the command layer), `tests/performance_test.rs` |
| First-use flow (SC-007) | Record a re-imported M1 Carbine from the guide | `e2e/specs/us6-identification.e2e.ts` |

Write each test to fail first against the unimplemented behavior, then pass
(red-green, Constitution II).

## Manual validation with the human-testing seed

`scripts/human-testing.sh` builds a scratch database populated by
`examples/human_seed.rs` and launches the app against it (it refuses the
real data directory). After this feature the seed includes:

- a **re-imported M1 Carbine** (importer name, U.S. maker's serial as the main
  serial, no country);
- an **imported pistol** with the maker's model and serial as the main marks
  and an importer name, no original marks;
- an **imported firearm** with an importer-assigned main serial and full
  original maker / model / serial;
- two firearms sharing **original** maker, model and serial (to see the
  warning);
- two pre-1968 **revolvers** with identical main marks and different years;
- import samples carrying all seven new columns, including a bad origin, a
  future year and a re-imported row with a country.

Walk the stories:

1. **US1**. Add a firearm, choose **Imported**, enter a country, importer and
   year 1943, save, reopen: all three shown with labels. Repeat with
   **Re-imported**: no country field, "United States" shown. Search
   `imported` (both appear), `re-imported` (only the re-imported one),
   `domestic`, `1943`, and the importer's name. Choose **Domestic** on a firearm
   with an importer recorded: the discard dialog appears; **Cancel** keeps
   everything. Enter year `1300` or next year: the save is blocked with the
   field message. Open the guide from the origin control.
2. **US2**. On an imported firearm enter original maker, model and serial in
   addition to the main marks: the record page shows the two sets under
   separate headings; search the original serial. Leave them all blank: no
   original-marks block. Confirm a domestic firearm offers no such fields.
3. **US3**. Save a second firearm with the same main make, model and serial
   and no years: blocked, the other record named, the message points to a
   year. Give both a year and make them differ (1943 / 1944): both kept. Edit
   the second to 1943: blocked. Dispose of the first, restore it, and watch the
   same rule apply. Save a firearm whose original maker, model and serial
   match another active one: the warning names it and **Save anyway** saves;
   **Cancel** does not. Confirm a partial original set never warns.
4. **US4**. Group by **Origin**: Domestic, Imported, Re-imported, Not
   specified. Export, import into a fresh collection, compare every new
   field. Import the sample with invalid rows: row-level errors, other rows
   still import, and the original-marks match appears under **Warnings**.

## Checks that must hold before merge

- `human_seed_coverage_test` passes with `NEVER_SEEDED` still empty (every
  new column and spreadsheet column is covered by the seed).
- The identity trigger backstop test passes for null/null, equal-year and
  year/no-year pairs, and allows a pair with two different years.
- The origin label is identical in the SQL trigger, `Origin::label()` and
  `ORIGIN_OPTIONS` (the search test over all three origins and the Vitest
  label test).
- PRs touching the UI carry before/after screenshots (Constitution,
  Development Workflow) and note how persistence changes satisfy the
  Security & Data Handling constraints and the performance budgets
  (research.md §3, §5).
