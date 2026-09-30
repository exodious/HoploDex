# Quickstart: Validate Cartridges, Action Types & Entry Suggestions

This is a runnable validation guide, not an implementation spec. It proves
the feature works end to end by mapping each user story's independent test
from [spec.md](./spec.md) to the automated tests that cover it. Field,
command and error names refer to [data-model.md](./data-model.md) and
[contracts/](./contracts). Setup is the same as
[001's quickstart](../001-firearms-inventory/quickstart.md).

## Prerequisites

- The development container (`scripts/dev-container.sh`; see DEVELOPMENT.md),
  or Rust, Node.js and the Tauri prerequisites on the host.
- A **fresh database**: the schema was edited in place, so an existing
  development database is incompatible and must be recreated. Nothing here
  opens the real databases: every test and E2E run uses throwaway locations
  (DEVELOPMENT.md, "Test isolation").

## Automated test commands

Run through the container wrapper on a host with podman (CLAUDE.md):

```bash
scripts/dev-container.sh cargo test --manifest-path src-tauri/Cargo.toml
scripts/dev-container.sh npm test
scripts/dev-container.sh bash -c 'cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets \
  && cargo fmt --manifest-path src-tauri/Cargo.toml --check && npm run lint && npm run format:check'
scripts/dev-container.sh npm run audit
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us10-cartridges-actions.e2e.ts'
scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'
```

One file at a time while working:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test caliber_guess_test
npx vitest run src/components/Combobox.test.tsx
```

All of these MUST pass before merge (constitution, Development Workflow).
Write each test to fail first against the unimplemented behavior (red-green,
constitution II).

## Scenario map: story → tests

| Story / criterion | Independent test (spec) | Where it is automated |
|---|---|---|
| US1 — cartridge with the caliber filled in | Built-in, guessable custom, unguessable custom and caliber-only records save, reopen and group; edit keeps the saved caliber and offers the new one | `tests/cartridge_test.rs` (US1-1, 5, 6, 8, 9), `caliberDerivation.test.ts` (US1-2, 3, 4, 7: every derived/edited transition), `FirearmForm.test.tsx` (the form wiring, Guess tag, suggestion line), `tests/fts_search_test.rs` (US1-9) |
| US1-10, US3-8 — browse display | Caliber cell, tiles, action column, hidden while grouped | `BrowseList.test.tsx`, `BrowseTiles.test.tsx`, `tests/list_firearms_test.rs` (summary fields) |
| US2 — narrowing list | Partial values in the four fields; order and markers; snap a variant; keep a different notation; deleted vs disposed | `tests/entry_suggestions_test.rs` (US2-1 to 9 through `suggest_entries`/`settle_entry` ops on a real database), `Combobox.test.tsx` (US2-10 keyboard, ARIA attributes, stale-response drop), `FirearmForm.test.tsx` (settle on leave, the note, save-time settle) |
| US3 — action type | Each seeded type's choices; Other gets all; type change clears; group, search, round trip | `tests/action_type_test.rs` (mapping, `list_action_types`, `actionTypeId` errors, the trigger backstop via raw SQL), `tests/list_firearms_test.rs` (group order, "Unspecified" last, origin's renamed group), `tests/fts_search_test.rs` ("bolt"), `FirearmForm.test.tsx` (filtered options, clearing note) |
| US4 — export and import | Round trip into an empty database; blank calibers; snapping; bad actions; a pre-feature sheet | `tests/import_export_test.rs` (US4-1 to 8, header-based reading, duplicate header, the report lists), `ImportDialog.test.tsx` (the two report sections) |
| FR-007 / SC-002 — the guess corpus | Every corpus name gives its expected caliber or none | `tests/caliber_guess_test.rs` over `tests/fixtures/caliber_guess_corpus.tsv` |
| FR-002, FR-004a — catalog invariants | Ranks, unique keys, class grammar, shared designations, guess agrees with catalog | `tests/cartridge_catalog_test.rs` |
| FR-012, FR-013 — the entry key | Variant and non-variant pairs from the spec | `tests/entry_text_test.rs` (also FR-015's length and control-character rules) |
| SC-001 | Each of the 25 top-ranked cartridges appears in the first 8 suggestions for a prefix of at most 4 characters of its name or an alias, in an empty collection, and picking it fills the caliber | `tests/entry_suggestions_test.rs` |
| SC-003 | 20 makes and cartridges on record, their case/spacing/separator/`&` variants through `settle_entry` and import: 100% snapped; different notations: none changed | `tests/entry_suggestions_test.rs`, `tests/import_export_test.rs` |
| SC-004 | 10,000 firearms: `suggest_entries` ≤ 50 ms (worst case 10,000 distinct models); grouping and search by cartridge and action within 1 s / 500 ms | `tests/performance_test.rs` |
| SC-005 | Delete the only firearm with a unique cartridge: gone from `suggest_entries` and from the database file's bytes | `tests/deletion_wipe_test.rs`, `tests/entry_suggestions_test.rs` |
| SC-006, SC-007 | Round trip exact without same-notation variants; with them, merged and each merge reported; every derived and snapped value in the report | `tests/import_export_test.rs` |
| SC-008 | No path saves a disallowed action | `tests/action_type_test.rs` (command layer and raw `INSERT`/`UPDATE`) |
| End to end, real input | Pick "9x19mm Parabellum" from a typed "9x1" with the keyboard only; caliber fills; save; group by Cartridge | `e2e/specs/us10-cartridges-actions.e2e.ts`, driving keys through the real-input harness (DEVELOPMENT.md, "Real keyboard and mouse input") |
| Seed in step | Every new column and spreadsheet column seeded | `tests/human_seed_coverage_test.rs` (unchanged logic) |

## Human-testing seed

`scripts/human-testing.sh` builds a scratch database from
`examples/human_seed.rs` (it refuses the real data directory). After this
feature the seed includes firearms with a built-in cartridge, a custom
cartridge with a guessed caliber, a caliber-only record, a muzzleloader with
no cartridge, "Smith & Wesson" on several firearms and "S&W" on one, a
disposed firearm with a unique custom cartridge, one firearm of each seeded
type with an action, and one with none; and import samples with a blank
caliber beside a catalog cartridge, a guessable and an unguessable custom
cartridge, make variants, an unknown action, a disallowed action, and a
sheet without the two new columns.

## Manual checks (best effort before a release, not merge gates)

Everything above is automated. These two need a person.

### M1. Screen reader announces the suggestion list

Setup: Linux with Orca, the human-testing seed open via
`scripts/dev-container.sh --gui scripts/human-testing.sh`.

1. Start Orca.
2. Open **Add firearm**.
3. Press Tab until focus is in **Make**. Expected: Orca reads "Make, combo box,
   required" or similar, and that a list is available.
4. Type `sw`. Expected: after a pause, Orca announces the number of
   suggestions.
5. Press Down. Expected: Orca reads "Smith & Wesson, 4 in collection".
6. Press Enter. Expected: the field reads "Smith & Wesson" and focus is still
   in Make.
7. Press Tab twice to reach **Cartridge**, type `9 x 19mm parabellum`, press
   Tab. Expected: Orca announces "Changed to the built-in spelling “9x19mm
   Parabellum”", and Caliber reads "9mm".

### M2. The list is never clipped and follows the field

Setup: as M1, with the window at its minimum size.

1. Open **Add firearm**.
2. Scroll the form so **Cartridge** is at the bottom edge of the dialog.
3. Click in **Cartridge**. Expected: the list opens fully visible (flipping
   above the field if there is no room below), not cut off by the dialog.
4. Scroll the form with the mouse wheel. Expected: the list moves with the
   field or closes; it never floats detached from it.

## Checks that must hold before merge

- The migrations were edited in place and no new migration file exists; the
  user has been told their development database must be recreated.
- `human_seed_coverage_test` passes without adding to `NEVER_SEEDED`.
- `catalog.tsv` carries its origin and license header (constitution,
  Licensing; research.md §1).
- PRs touching the UI carry before/after screenshots; the PR notes how the
  persistence changes meet the Security & Data Handling constraints
  (research.md §14) and the performance impact against Principle IV's budgets
  (research.md §5, §11).
