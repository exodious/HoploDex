# Quickstart: Validate Accessory Records and Mounting

This is a runnable validation guide, not an implementation spec. It proves
the feature works end to end by mapping each user story's independent test
from [spec.md](./spec.md) to the automated tests that cover it. Field,
command and error names refer to [data-model.md](./data-model.md) and
[contracts/](./contracts). Setup is the same as
[001's quickstart](../001-firearms-inventory/quickstart.md).

## Prerequisites

- The development container (`scripts/dev-container.sh`; see
  DEVELOPMENT.md), or Rust, Node.js and the Tauri prerequisites on the host.
- A **fresh database**. The schema was edited in place, so an existing
  development database is incompatible and must be recreated. Nothing here
  opens the real databases: every test, E2E run and screenshot uses
  throwaway locations (DEVELOPMENT.md, "Test isolation").

## Automated test commands

Run these through the container wrapper on a host with podman (CLAUDE.md):

```bash
scripts/dev-container.sh cargo test --manifest-path src-tauri/Cargo.toml
scripts/dev-container.sh npm test
scripts/dev-container.sh bash -c 'cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets \
  && cargo fmt --manifest-path src-tauri/Cargo.toml --check && npm run lint && npm run format:check'
scripts/dev-container.sh npm run audit
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us12-accessories.e2e.ts'
scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'
```

One file at a time while working:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test mount_test
npx vitest run src/features/mounts/MountedSection.test.tsx
```

All of these MUST pass before merge (constitution, Development Workflow).
Write each test to fail first against the unimplemented behavior (red-green,
constitution II).

## Scenario map: story → tests

New backend test files are marked *(new)*. Every backend test runs the real
`ops` against a temporary SQLCipher database.

| Story / criterion | What is checked | Where it is automated |
|---|---|---|
| US1-1, 3, 4, 5: record an accessory | Kind required, all else optional; saves and reopens intact; a pair-of-magazines record; kind alone | `tests/accessory_test.rs` *(new)*; `AccessoryForm.test.tsx` (fields, no quantity field, "Choose a kind.") |
| US1-2, FR-002: kinds | The 12 kinds in order, no Suppressor, the hint; a kind no longer offered stays on its record and is accepted on save and import | `tests/accessory_test.rs` (`list_accessory_kinds`; raw `UPDATE accessory_kinds SET offered = 0`); `AccessoryForm.test.tsx` |
| US1-6, FR-003: entry rules | Caliber derived from cartridge; make suggestions from both tables; snapping | `tests/entry_suggestions_test.rs` (+ suggestions and `settle_entry` over both tables); `tests/accessory_test.rs` (entry rules on save); `AccessoryForm.test.tsx` (derivation row) |
| FR-004, FR-005: no identity, naming | Two accessories with the same make, model and serial number both save; naming with and without make and model | `tests/accessory_test.rs`; `RecordName.test.tsx` *(new)* |
| US1-7, US1-8, FR-008, FR-009, SC-005: value and insurance | Totals and subtotals; blanket total and counts; scheduling; under-insured and uninsured warnings; policy deletion moves or unschedules accessories | `tests/valuation_test.rs`, `tests/insurance_status_test.rs`, `tests/policy_deletion_test.rs` (+ accessories); `CoverageDialog.test.tsx`, `PolicyCard.test.tsx` |
| US1-9, FR-006: dispose and restore | Leaves the list and summary; restored with history kept or discarded; no nickname or identity re-check | `tests/disposition_reversal_test.rs` (+ accessory); `DisposeDialog.test.tsx`, `RestoreDialog.test.tsx` (accessory variant) |
| US1-10, SC-006: delete | Row, photos, documents, history and mounts gone; no bytes left; values no longer suggested | `tests/deletion_wipe_test.rs` (+ a unique accessory serial number, note, photo and document); `tests/entry_suggestions_test.rs` |
| US1-11, FR-007: free-text field | The firearm's Accessories text is unchanged, searchable, exported and never converted | `tests/fts_search_test.rs`, `tests/export_test.rs` (existing assertions still hold); `FirearmRecordPage.test.tsx` (Accessories subsection beside Mounted) |
| US1-12, FR-007a: photos and documents | First photo is the thumbnail, switched; document opens; tile drawing per kind; drop on the record | `tests/photo_test.rs`, `tests/document_test.rs` (+ `owner` accessory, owner `CHECK` by raw SQL); `AccessoriesPage.test.tsx` (tiles); `TypeDrawing.test.tsx` (every seeded kind key has a drawing) |
| US2-1–3, 5, 7, 8, 15: mount, move, unmount | Both records show it; move in one step; unmount leaves no row; nothing judged | `tests/mount_test.rs` *(new)* |
| US2-3a, US2-4: from the host's page | Candidates exclude the host, its chain and its direct items; a mounted candidate carries its host; the move asks first; "New accessory…" mounts on save | `tests/mount_test.rs` (`list_mount_candidates` both roles); `MountedSection.test.tsx` *(new)* (menu, dialog, confirmation, cancel); `AccessoryForm.test.tsx` (preset) |
| US2-6, US2-16, FR-010: no loops | The record and its subtree are never offered as hosts; a direct `mount_record` that would loop is refused with the field error | `tests/mount_test.rs` |
| US2-9, 12–14, FR-010: nesting | Upper swaps keep their optics; unmounting or moving an upper moves its subtree; moving an optic between uppers | `tests/mount_test.rs` |
| US2-10, FR-013: record pages | Mounted section depth-first with "on …" lines and Unmount only on direct entries; "Nothing mounted."; the "Mounted on" chain with links | `tests/mount_test.rs` (`MountDetail` order and depths); `MountedSection.test.tsx`, `FirearmRecordPage.test.tsx`, `AccessoryRecordPage.test.tsx` *(new)* |
| US2-11, FR-016a: collection page | "Mounted on" with a linked host name; "{counts} mounted" by kind, counting all depths; neither when neither | `tests/list_firearms_test.rs` (`mountedOn`, `mountedCounts`); `BrowseList.test.tsx`, `BrowseTiles.test.tsx` |
| SC-004: invariants | After a seeded random sequence of mount, move, dispose, restore and delete operations: one host at most, no disposed record in a mount, no loop, subtrees intact | `tests/mount_test.rs` (property-style test with a fixed seed, checked after every step) |
| Backstops | Mount to a disposed record, disposing while mounted, self-mount, changing a `uid`, a cross-table `uid` | `tests/mount_test.rs`, `tests/record_identifier_test.rs` *(new)* (raw SQL; each is refused) |
| US3-1, 2, 7, 8: dispose with mounted records | The list with choices and per-record prices; shared type, recipient and date; blank price stored as none; kept records' mounts | `tests/mount_lifecycle_test.rs` *(new)*; `DisposeDialog.test.tsx` (list, defaults, price field appears, the stated outcomes) |
| US3-3, 4: dispose a mounted record; restore | The note; unmounted; restored records are not mounted | `tests/mount_lifecycle_test.rs`; `DisposeDialog.test.tsx` |
| US3-5, 6, 9, FR-015: delete | The confirmation names direct records; they stay unmounted; deeper mounts stay; no trace remains | `tests/mount_lifecycle_test.rs`; `tests/deletion_wipe_test.rs`; `FirearmRecordPage.test.tsx`, `AccessoryRecordPage.test.tsx` (confirmation text) |
| Stale dispose | A `withMounted` record no longer below the host fails with nothing changed | `tests/mount_lifecycle_test.rs` |
| US4-1–5, FR-016–FR-018: browse | Group by each field, with "Unspecified" and "Not mounted"; two hosts with the same name are two groups; search every text field, including 1–2 characters; disposed toggle | `tests/list_accessories_test.rs` *(new)*; `AccessoriesPage.test.tsx` *(new)* |
| US4-6: collection search | A firearm is not found by a model mounted on it | `tests/fts_search_test.rs` |
| US5-1, 2: export | Two sheets or two CSV files; exact headers; identifiers and `mounted_on`; accessory photos `a{id}_…`; disclosure | `tests/export_test.rs` (+ both tables, both formats); `ExportDialog.test.tsx` |
| US5-3, SC-002: round trip | Export, then import into an empty database with the same policies: every record, field, identifier and mount, in the workbook, two single-sheet workbooks and two CSV files | `tests/accessory_spreadsheet_test.rs` *(new)* |
| US5-4, SC-003: re-import | Identifier matches are conflicts; "skip" for all creates nothing, including firearms with no serial number; a duplicate gets a new identifier; overwrite keeps it | `tests/accessory_spreadsheet_test.rs`, `tests/import_matching_test.rs` (identifier ahead of the make, model and serial number key) |
| Disposed row on overwrite, issue #56 | A row marking an active host disposed lists everything below it; the records chosen go with it at the row's disposition and no price, the rest are kept and unmounted; a chosen record that fails its own checks, or is no longer below, leaves the conflict open with nothing changed; the Replace confirmation asks, Keep by default | `tests/accessory_spreadsheet_test.rs`; `tests/human_seed_coverage_test.rs` (`import-dispose-receiver.csv`); `ImportDialog.test.tsx` |
| US5-5, FR-023: mount warnings | Unknown, disposed, malformed and looping `mounted_on`; imported unmounted with the warning; overwrite with a blank cell unmounts | `tests/accessory_spreadsheet_test.rs` |
| US5-6, FR-024: row errors | Blank or unknown kind; bad amount or date; malformed, repeated or other-kind `record_id` | `tests/accessory_spreadsheet_test.rs` |
| US5-7, US5-8, FR-022: partial imports | A pre-feature sheet; firearm table alone; accessory table alone; a header that is neither; two of the same table | `tests/accessory_spreadsheet_test.rs`; `ImportDialog.test.tsx` (one or two files) |
| US5-9, FR-020: filtered export | Descendants of every depth included, firearms too; unmounted and outside records excluded; the host named but absent | `tests/export_test.rs`; `tests/accessory_spreadsheet_test.rs` (`get_export_scope` counts) |
| Disposed with no price | A disposed row with a blank price imports, in both tables | `tests/import_export_test.rs` (the changed 001 assertion) |
| FR-019: identifier | Distinct on create; kept by edit, dispose, restore, backup and restore; a new one after the highest id is deleted and reused; not in any IPC output | `tests/record_identifier_test.rs`; `tests/backup_test.rs` (+ identifiers equal across a backup) |
| FR-025: data handling | No network; deletion as above; backups carry accessories | `tests/backup_test.rs` (an accessory and a mount survive backup and restore); `tests/csp_test.rs` (unchanged) |
| FR-026, SC-007: performance | Each row of research.md §22 within its budget at 10,000 firearms and 10,000 accessories | `tests/performance_test.rs` (+ the new operations) |
| FR-027: unsaved changes | An accessory add, edit, dispose (with choices and prices), restore and coverage are kept at a lock and asked about at a close; a firearm draft keeps `mountedOn` | `tests/pending_changes_test.rs` (+ `kind: "accessory"`; `coverage` allowed); `SessionProvider.test.tsx`, `AccessoryForm.test.tsx`, `DisposeDialog.test.tsx` |
| Backup due | The four new tables mark a backup due | `tests/backup_due_tracking_test.rs` |
| Human seed | Every new column and both import tables seeded | `tests/human_seed_coverage_test.rs` |
| End to end | Keyboard only: add an optic from a rifle's Mount menu; mount a suppressor; move it with confirmation; dispose the rifle with the optic; the Accessories page grouped by Mounted on | `e2e/specs/us12-accessories.e2e.ts` *(new)* |
| UI evidence | contracts/ui-accessories.md §13 | `e2e/screenshots/screens.e2e.ts` |

## Manual checks (best effort before a release, not merge gates)

### M1. Screen reader on the Mounted section and the dispose dialog

**Setup**: `scripts/human-testing.sh` (the seeded sandbox), Orca on Linux or
VoiceOver on macOS, and the seeded receiver with its upper, scope, red dot
and light.

1. Open the receiver's record page.
2. Move focus into the Mounted section with Tab.
   **Expected**: the list is announced as a list of 4 items.
3. Tab to the scope's link.
   **Expected**: its name, then "on BCM upper".
4. Tab to the red dot's link.
   **Expected**: its name, then "on" and the scope's name.
5. Tab past the upper's Unmount button.
   **Expected**: "Unmount BCM upper, button".
6. Open Mark disposed. Tab to the upper's Keep | Dispose with it control.
   **Expected**: it is announced as a group of two, with Keep selected.
7. Press → .
   **Expected**: "Dispose with it" is selected, and a "Price for BCM upper"
   field follows in the tab order.

### M2. Accessory drawings at every size

**Setup**: the seeded sandbox, with one accessory of each kind and no
photos.

1. Open Accessories and choose Tiles.
   **Expected**: each of the 12 tiles shows its kind's drawing, centred and
   uncropped, in light theme.
2. Switch to dark theme.
   **Expected**: every drawing stays legible against the tile.
3. Narrow the window to its minimum width.
   **Expected**: the drawings scale down with their tiles and no line
   disappears.
