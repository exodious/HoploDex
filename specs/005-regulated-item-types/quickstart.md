# Quickstart: Validate Regulated Item Types, Suppressors and NFA Registration

This is a runnable validation guide, not an implementation spec. It proves
the feature works end to end by mapping each user story's independent test
from [spec.md](./spec.md) to the automated tests that cover it. Field,
command and error names refer to [data-model.md](./data-model.md) and
[contracts/](./contracts). Setup is the same as
[001's quickstart](../001-firearms-inventory/quickstart.md).

## Prerequisites

- The development container (`scripts/dev-container.sh`; see DEVELOPMENT.md),
  or Rust, Node.js and the Tauri prerequisites on the host.
- A **fresh database**. The schema was edited in place, so an existing
  development database is incompatible and must be recreated. Nothing here
  opens the real databases: every test and E2E run uses throwaway locations
  (DEVELOPMENT.md, "Test isolation").

## Automated test commands

Run these through the container wrapper on a host with podman (CLAUDE.md):

```bash
scripts/dev-container.sh cargo test --manifest-path src-tauri/Cargo.toml
scripts/dev-container.sh npm test
scripts/dev-container.sh bash -c 'cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets \
  && cargo fmt --manifest-path src-tauri/Cargo.toml --check && npm run lint && npm run format:check'
scripts/dev-container.sh npm run audit
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us11-regulated-items.e2e.ts'
scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'
```

One file at a time while working:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test registration_test
npx vitest run src/components/Menu.test.tsx
```

All of these MUST pass before merge (constitution, Development Workflow).
Write each test to fail first against the unimplemented behavior (red-green,
constitution II).

## Scenario map: story → tests

| Story / criterion | Independent test (spec) | Where it is automated |
|---|---|---|
| US1: record a suppressor | Suppressor offered; saves and reopens; drawing; grouped by type; serial rules; cartridge derivation | `tests/suppressor_test.rs` (US1-1, 3, 6, 7 through `ops`; `list_firearm_types`' flags), `tests/list_firearms_test.rs` (US1-4: the Suppressor group, `genericThumbnailKey` "suppressor"), `tests/identity_uniqueness_test.rs` (US1-6 for a Suppressor), `FirearmForm.test.tsx` (US1-2: no Action, Barrel length or Capacity; "Caliber rating" and its hint), `FirearmRecordPage.test.tsx` (US1-3 label), `BrowseTiles.test.tsx` / `TypeDrawing.test.tsx` (US1-4 drawing) |
| US1-5, FR-004: type change | Rifle → Suppressor notice and clearing on save; back to Rifle offers them again | `FirearmForm.test.tsx` (notice text names only recorded fields; the input at save leaves them out; switching back restores them; announced once per change), `tests/suppressor_test.rs` (update that changes the type with the fields still set is refused, with field errors) |
| FR-003, SC-005: the fields rule | No path saves a Suppressor with an action, barrel length or capacity | `tests/suppressor_test.rs` (command layer, before `check_action_allowed`; the `firearms_fields_apply_*` triggers through raw `INSERT`/`UPDATE`), `tests/import_export_test.rs` (FR-022) |
| US2: record a registration | Each combination of details; reopen; no derived status; search; group; clear | `tests/registration_test.rs` (US2-1, 3, 4, 6, 12 through `ops`: none, some and all details; future approved date; details with no classification refused by command and by `CHECK`; unknown classification id; disposed and restored keep details; delete removes them), `tests/entry_suggestions_test.rs` (US2-5: built-in forms marked, forms and names on record, snapping, a name used only by a deleted firearm not offered), `tests/list_firearms_test.rs` (US2-9, 10: group order, "Unspecified" last and including unclassified firearms for Registered to), `tests/fts_search_test.rs` (US2-11: "smith family", "form 1", "short-barreled", and the one- and two-character `LIKE` path) |
| US2-2, 5, 7, 13: registration on the form | Details appear only with a classification; note and guide link; confirm on clear; keep on change | `FirearmForm.test.tsx` (section closed and open states, summary line, the standing note, `ConfirmDialog` text naming recorded parts, cancel keeps the classification, changing class keeps details, `FORM_VERSION` 3), `IdentificationGuide.test.tsx` (both parts; opening at "Registered items" focuses its heading) |
| US2-3, 4: record page | Registration panel values and date format; no status | `FirearmRecordPage.test.tsx` |
| US2-8, FR-014, SC-002: record only | No hint, warning or prompt anywhere | `tests/registration_test.rs` (every type × no classification and each classification, rifles and shotguns with barrels of 10.5 in and 14 in, all save), `FirearmForm.test.tsx` and `FirearmRecordPage.test.tsx` (contracts/ui-registration.md §3's text search) |
| US3: automatic or select-fire | Handgun, Rifle, Shotgun, Other with and without Machine gun; not offered for Suppressor | `tests/action_type_test.rs` (mapping for types 1 to 4, list order, the id and sort order, every combination with a classification saves), `tests/list_firearms_test.rs` (group order), `tests/fts_search_test.rs` ("select-fire"), `FirearmForm.test.tsx` (not rendered for a Suppressor) |
| US4: export and import | Columns; disclosure; round trip; error rows; pre-feature sheet | `tests/import_export_test.rs` (US4-1, 3 to 7: header order, blank cells, case-insensitive match including a non-offered class, every row error in spreadsheet-format.md, snapped forms and names reported, a sheet without the four columns), `tests/export_test.rs` (US4-1 values), `ExportDialog.test.tsx` (US4-2: note with and without registered firearms in each scope), `ImportDialog.test.tsx` (field names Form and Registered to in the report) |
| SC-001 | A registered suppressor, keyboard only, under 2 minutes, no inapplicable field offered | `e2e/specs/us11-regulated-items.e2e.ts` (create one from the keyboard with real input, pick "Form 4" from the list, save, reopen; the run's elapsed time is asserted well under 2 minutes as a guard, not as a measure of a person) |
| SC-003 | Export → import into an empty database reproduces everything | `tests/import_export_test.rs` (round trip exact without same-notation variants; merged and reported with them) |
| SC-004 | A classification no longer offered opens, shows, edits and saves | `tests/registration_test.rs` (raw `UPDATE registration_classes SET offered = 0`, then `get_firearm`, `update_firearm` with another field changed, and import by that name), `FirearmForm.test.tsx` (the select offers it for that record only) |
| SC-006 | 10,000 firearms: group by Registered as and Registered to within 1 s; search within 500 ms; suggestions within 50 ms | `tests/performance_test.rs` |
| SC-007 | Delete every firearm registered to a name: gone from suggestions and the file's bytes | `tests/deletion_wipe_test.rs`, `tests/entry_suggestions_test.rs` |
| Grouping control | Sections, roles, keys, one checked item | `Menu.test.tsx` (the new radio items), `CollectionPage.test.tsx` (trigger text, choosing regroups, Escape chooses nothing) |
| Backups and seed in step | New lookup tracked; every new column seeded | `tests/backup_due_tracking_test.rs` (the `registration_classes` triggers), `tests/human_seed_coverage_test.rs` (unchanged logic; `is_user_table` skips `registration_classes`) |

## Human-testing seed

`scripts/human-testing.sh` builds a scratch database from
`examples/human_seed.rs`, and refuses the real data directory. After this
feature the seed includes:

- a Suppressor registered as Suppressor on Form 4, approved, to "Smith
  Family Trust", with the approved form attached as a document;
- a Suppressor with no classification;
- a Rifle registered as Short-barreled rifle on Form 1 to the owner, with a
  10.5 in barrel;
- a Rifle with a 10.5 in barrel and no classification;
- a Rifle with action "Automatic or select-fire" registered as Machine gun;
- a Shotgun registered as Short-barreled shotgun with no approved date;
- a disposed firearm registered to a unique name;
- "Smith family trust" typed on one more firearm, to see snapping.

It also includes import samples: a round-trip sheet; a sheet with an unknown
`registered_as`, details with no classification, a future approved date and
a Suppressor with an action, a barrel length and a capacity; and a sheet
without the four columns.

## Manual checks (best effort before a release, not merge gates)

Everything above is automated. These need a person.

### M1. The grouping menu with a screen reader

Setup: Linux with Orca, and the human-testing seed open via
`scripts/dev-container.sh --gui scripts/human-testing.sh`.

1. Start Orca.
2. Press Tab until focus is on the grouping button. Expected: Orca reads
   "Group by, None, menu button" or similar.
3. Press Enter. Expected: Orca announces a menu and reads "None, checked".
4. Press Down twice. Expected: Orca reads "The firearm" (the group) and then
   "Action, not checked", or "Type, not checked" first, depending on how it
   reads groups. Either way, the section name is spoken once when it is
   entered.
5. Press R. Expected: focus moves to "Registered as".
6. Press Enter. Expected: the menu closes, focus is on the button, and Orca
   reads "Group by, Registered as". The list is grouped, with "Unspecified"
   last.
7. Press Enter, then Escape. Expected: the menu closes, the grouping is still
   Registered as, and focus is on the button.

### M2. The suppressor drawing at every size

Setup: as M1.

1. Open the collection in **Tiles** view. Expected: the suppressor with no
   photo shows the suppressor drawing, centred, not clipped, with its line
   weight matching the rifle's beside it.
2. Switch to **List**. Expected: the drawing in the thumbnail cell is legible
   as a suppressor at that size.
3. Open the suppressor's record page. Expected: the thumbnail shows the same
   drawing.
4. Switch the theme to dark with the theme toggle. Expected: the drawing's
   lines and fills follow the theme like the other drawings.
5. Open **Add firearm**. Expected: the Type cards show five drawings. The
   suppressor's is cropped to its own bounds like the rifle's, and fills its
   card without touching the edges.
6. Narrow the window to its minimum width. Expected: the cards wrap onto a
   second row, and no drawing is clipped.

## Checks that must hold before merge

- The migrations were edited in place and no new migration file exists. The
  user has been told their development database must be recreated.
- `human_seed_coverage_test` passes without adding to `NEVER_SEEDED`.
- The suppressor drawing's source note is in `typeDrawings.ts` (constitution,
  Licensing; research.md §4).
- No screen carries text about legal status (contracts/ui-registration.md §3).
- PRs touching the UI carry before/after screenshots, including the grouping
  control. The PR notes how the persistence changes meet the Security & Data
  Handling constraints (research.md §14) and their performance impact
  against Principle IV's budgets (research.md §7, §8).
