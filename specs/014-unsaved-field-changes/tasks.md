---
description: "Task list for unsaved changes per field, with revert"
---

# Tasks: Unsaved Changes per Field, with Revert

**Input**: Design documents from `/specs/014-unsaved-field-changes/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Every story's tests are written first and expected to fail, at the level plan.md's Test Levels gives them.

## Format: `[ID] [P?] [Story] Description`

Bare IDs here mean this feature's. In code, cite them as `014 FR-001`, `014 US1-2`, `014 T012`, `014 research.md §3`.

---

## Phase 1: Setup

- [X] T001 Confirm the baseline passes: `scripts/dev-container.sh bash -c 'npm run lint && npm test'` and the Rust tests, in the background with a log.

---

## Phase 2: Foundational

**Purpose**: the shared pieces every form uses (research.md §1 to §4, §7; contracts/ui.md §1 to §4).

- [X] T002 [P] Write failing Vitest tests in src/components/fieldChanges.test.tsx for `changedFields`: a field changed and typed back, an `UNREADABLE` value (FR-003: changed unless empty against "not recorded"), `isDirty` from the set.
- [X] T003 Implement `changedFields`, `formChanges` and `UNREADABLE` in src/components/fieldChanges.ts (research.md §1), and export them from src/components/index.ts.
- [X] T004 [P] Write failing Vitest tests in src/components/fieldChanges.test.tsx with TextField, Checkbox and ChoiceCards: with `change` set, the "Changed" tag shows after the label, its id comes first in `aria-describedby`, the revert's accessible name is "Revert {label} to “{saved}”", tooltip "Revert to “{saved}”", "Not recorded" for empty, 40-character cut with "…"; without `change`, nothing.
- [X] T005 Implement `ChangedMark` (src/components/ChangedMark.tsx, styles in src/components/components.css) and the `change` prop on `FieldFrame` and `useFieldIds` (src/components/Field.tsx, src/components/fieldIds.ts), passed through by TextField, TextArea, Select, DateField, MoneyField, NumberField, DecimalField, Checkbox, ChoiceCards and SegmentedControl in src/components/.
- [X] T006 [P] Write failing Vitest tests in src/components/fieldChanges.test.tsx for `Button`'s `unavailable`: `aria-disabled`, stays focusable, the reason as tooltip and accessible description, a click does nothing.
- [X] T007 Implement `Button`'s `unavailable` prop in src/components/Button.tsx (research.md §3).
- [X] T008 [P] Write failing Vitest tests in src/components/fieldChanges.test.tsx for the close guard: Escape, the close button and `requestClose()` on a dirty form ask "Discard changes?" (destructive, focus on Keep editing); Keep editing returns with focus where it was; Discard closes; a clean form closes at once; Escape in an open listbox closes only that.
- [X] T009 Implement the close guard context and `useCloseGuard(isDirty)` in src/components/Dialog.tsx, using `ConfirmDialog` (research.md §4; contracts/ui.md §3).
- [X] T010 [P] Add `changed` to `Disclosure` in src/components/Disclosure.tsx, with a test in src/components/fieldChanges.test.tsx (FR-007).

---

## Phase 3: User Story 1 - See Which Fields I Have Changed (Priority: P1) 🎯 MVP

**Goal**: Markers on the firearm, policy, accessory, coverage and Settings forms.

**Independent test**: spec US1's Independent Test.

- [X] T011 [P] [US1] Write failing Vitest tests in src/features/firearms/FirearmForm.changes.test.tsx for US1-1 to US1-8 (trailing space, "01250", 0 lb 40.5 oz, one weight marker, folded section, description), plus FR-022 (a photo or document added or removed on the record page marks nothing and keeps no draft) and SC-001 (a change on a form whose collection store holds 10,000 records marks within 100 ms).
- [X] T012 [US1] In src/features/firearms/FirearmForm.tsx, write `stored(field, state)` from the existing parsers (FR-002, FR-005: hidden fields judged by what a save stores, serial under "no serial number"), feed `changedFields` with the saved state built from `initialValues` (never the resumed draft), pass `change` to every field (one for the weight pair), `changed` to the folded sections, and its `isDirty` to `useDirtyForm`; add forms use the opening state and show no markers.
- [X] T013 [P] [US1] Same in src/features/insurance/InsurancePolicyForm.tsx, with tests in InsurancePolicyForm.test.tsx (US1-9).
- [X] T014 [P] [US1] Same in src/features/accessories/AccessoryForm.tsx, with tests in AccessoryForm.test.tsx.
- [X] T015 [P] [US1] Same in src/features/insurance/CoverageDialog.tsx (policy and amount, compared in whole dollars), with tests in CoverageDialog.test.tsx.
- [X] T016 [P] [US1] Same in src/features/databases/DatabaseSettingsDialog.tsx (research.md §9: keep count as a number, location, the three locking settings, Remember the passphrase; not Open documents or the backup actions), with tests in DatabaseSettingsDialog.test.tsx.
- [X] T017 [US1] E2E in e2e/specs/record-firearm.e2e.ts: `describe("014 US1 - See Which Fields I Have Changed")`: edit a firearm, change Make, marker shows; type it back, marker gone.

---

## Phase 4: User Story 2 - Put One Field Back to Its Saved Value (Priority: P2)

**Goal**: Per-field revert, with dependents.

**Independent test**: spec US2's Independent Test.

- [X] T018 [P] [US2] Write failing Vitest tests in src/features/firearms/FirearmForm.changes.test.tsx for US2-1 to US2-9 (Type, Origin, Registered as, entry fields' re-spelling, errors cleared, focus), and a saved value no longer offered (unchanged until another is picked; revert brings it back).
- [X] T019 [US2] Implement `revert(field)` in FirearmForm.tsx (research.md §5: dependents restored when still as the change left them, no Origin or classification question again) and in src/features/firearms/EntryField.tsx (research.md §6: clear the note and suggestion, no re-spelling until the next edit); focus the field after a revert (FR-013).
- [X] T020 [P] [US2] Implement revert in InsurancePolicyForm.tsx, AccessoryForm.tsx, CoverageDialog.tsx and DatabaseSettingsDialog.tsx (Remember: drop the staged choice, ask nothing), with tests in each form's test file.
- [X] T021 [US2] E2E in e2e/specs/record-firearm.e2e.ts: `describe("014 US2 - Put One Field Back to Its Saved Value")`: revert by keyboard; focus lands in the field; value is the saved one.

---

## Phase 5: User Story 3 - Save Only When There Is Something to Save (Priority: P3)

**Goal**: Save enabled exactly while something differs; add forms unchanged.

**Independent test**: spec US3's Independent Test.

- [X] T022 [P] [US3] Write failing Vitest tests for US3-1 to US3-8 in each form's test file (Save `aria-disabled` with "No changes to save"; Enter does nothing; errors still shown when changed; a failed save leaves markers; add forms keep Save enabled and show required-field errors).
- [X] T023 [US3] Pass `unavailable={isDirty ? undefined : "No changes to save"}` to Save and return early from submit when `!isDirty` in FirearmForm.tsx, AccessoryForm.tsx, InsurancePolicyForm.tsx, CoverageDialog.tsx and DatabaseSettingsDialog.tsx (replacing its `disabled={!dirty}`); add forms keep their Save.
- [X] T024 [US3] E2E in e2e/specs/value-insurance.e2e.ts: `describe("014 US3 - Save Only When There Is Something to Save")`: coverage dialog Save disabled, change amount, enabled, back, disabled; Enter saves nothing.

---

## Phase 6: User Story 4 - Unsaved Means the Same Thing Everywhere (Priority: P4)

**Goal**: The quit question, pending changes and the close question agree with Save.

**Independent test**: spec US4's Independent Test.

- [X] T025 [P] [US4] Write failing Vitest tests in src/features/firearms/FirearmForm.changes.test.tsx, RestoreDialog.test.tsx and DisposeDialog.test.tsx for US4-1 to US4-9 (typed back: no dirty form, no draft; resumed draft: markers against the saved record; add form with only a make: dirty; Cancel asks).
- [X] T026 [US4] Use `useCloseGuard` for Cancel, Escape and the close button in FirearmForm.tsx, AccessoryForm.tsx, InsurancePolicyForm.tsx, CoverageDialog.tsx, DatabaseSettingsDialog.tsx, src/features/firearms/DisposeDialog.tsx and src/features/firearms/RestoreDialog.tsx (FR-021a; Dispose and Restore with their existing `isDirty`; Restore through `ConfirmDialog`'s `unsavedChanges`).
- [X] T027 [US4] E2E in e2e/specs/locking.e2e.ts: `describe("014 US4 - Unsaved Means the Same Thing Everywhere")`: change two fields, lock, reopen, resume: same fields marked, Save enabled; revert both, lock: no pending changes; Escape with a change asks "Discard changes?".

---

## Phase 7: User Story 5 - Change What Is Mounted, and Save It with the Record (Priority: P5)

**Goal**: The Mounted list in the edit form, saved with the record.

**Independent test**: spec US5's Independent Test.

- [X] T028 [P] [US5] Write failing Rust tests in src-tauri/tests/mounted_list_test.rs for `mountChanges` (contracts/tauri-commands.md): add one mounted elsewhere (moves with what is on it), remove one, all or none when one fails, a stale removal → `VALIDATION` on `mounted`, a loop → `VALIDATION` on `mounted`, "at most 1,000" entries and an item listed twice → `VALIDATION`, ignored when saved as disposed; `list_mount_candidates` with `hostMountedOn`.
- [X] T029 [US5] Add a `mount_changes` argument to `update_firearm` and `update_accessory` (src-tauri/src/commands/firearms.rs, accessories.rs; `ops::update_*_with_mounted`), applied in the record's savepoint after its own mount, through `mounts::save_mounted_changes` in src-tauri/src/services/mounts.rs; add `host_mounted_on` to `list_mount_candidates` in src-tauri/src/commands/mounts.rs.
- [X] T030 [P] [US5] Mirror the shapes in src/features/firearms/types.ts, src/features/accessories/types.ts, src/features/mounts/types.ts and src/features/mounts/mountsService.ts.
- [X] T031 [P] [US5] Write failing Vitest tests in src/features/mounts/MountedEditor.test.tsx (US5-2 to US5-6, US5-8) and src/features/mounts/MountedSection.test.tsx (US5-1, US5-10).
- [X] T032 [US5] Implement src/features/mounts/MountedEditor.tsx (research.md §8, contracts/ui.md §5) and add it to the active firearm and accessory edit forms with `mounted` (`mountedChanges.ts`) in their state, field changes, reverts and save input; raise `FORM_VERSION` (firearm 5, accessory 2).
- [X] T033 [US5] Make src/features/mounts/MountedSection.tsx read-only with "Change what's mounted…" (opens the edit form with `focusField: "mounted"` from FirearmRecordPage.tsx and AccessoryRecordPage.tsx) and the Mount menu's New accessory… only; remove Unmount, Mount → Existing… and their questions.
- [X] T034 [US5] E2E in e2e/specs/accessories-mounting.e2e.ts: replace the record-page Mount → Existing journey with the edit form's Mounted list, under `describe("… 014 US5 - Change What Is Mounted, and Save It with the Record")`; mount the same way in accessories-dispose.e2e.ts's setup.
- [ ] T035 [US5] Seed: confirm src-tauri/examples/human_seed.rs already has a record mounted on another (no new column, so `human_seed_coverage_test.rs` needs nothing); add one only if quickstart M3 lacks it.

---

## Phase 8: Docs and surface

- [X] T036 Add the changed form, the discard question, the Mounted list and the read-only Mounted section to e2e/screenshots/screens.e2e.ts.
- [ ] T037 Update docs/architecture/frontend.md (the shared change mechanism, close guard) and the contracts' notes: `_Amended by_` lines in specs/006-accessory-links/ (FR-012, contracts/ui-accessories.md §5, contracts/tauri-commands.md) and specs/003-database-protection-management/ (FR-010, FR-039, contracts/ui-databases.md §7).
- [ ] T038 Check docs/attack-surface.md against research.md §11 (`npm test` runs `scripts/attack-surface.mjs`); update it only if a path or command changed.

---

## Phase 9: Validation on every environment

- [ ] T039 Linux dev container: format, lint, `npm test`, Rust tests, the dependency and license audit, in the background with logs.
- [ ] T040 Linux dev container: `npm run build`, then E2E (all specs).
- [ ] T041 Linux dev container: window-only screenshots of each changed form, before (main) and after.
- [ ] T042 On macOS (VM, take the lock): Rust tests and E2E, in the task's folder on the external volume.
- [ ] T043 On macOS: window-only screenshots, before and after (`screencapture -l <window id>`).
- [ ] T044 On Windows (VM, take the lock): Rust tests and E2E, in the task's folder.
- [ ] T045 On Windows: window-only screenshots, before and after.
- [ ] T046 Report every test that failed then passed on rerun, and every skip, as they happen.

---

## Phase 10: Close-out

- [ ] T047 Run `/ai-security-review pr` before the PR; keep its three lines in `## Security review`.
- [ ] T048 PR description: `Closes #17` on its own line, screenshots, the persistence note (mount changes in the record's transaction, bounded), the performance note (research.md §10). Check `closingIssuesReferences`.
- [ ] T049 Close-out per plan.md: check each issue's state before commenting; comment on none that is closed.

---

## Dependencies

- Phase 2 blocks every story. US1 → US2 → US3 (each builds on the field changes); US4 needs US3's `isDirty`; US5 needs Phase 2 and US1's form wiring, and its backend tasks (T028 to T030) can run beside US1 to US4.
- Within a story, tests first, then the code that makes them pass.

## Parallel examples

- Phase 2: T002, T004, T006, T008, T010 together.
- US1: T013, T014, T015, T016 together after T012's pattern is set.
- US5: T028 and T031 together; T030 beside T029.

## Implementation strategy

MVP is Phase 2 plus US1 to US3 on the firearm form; then the other forms, US4, and US5 last. Each story is committed when its tests pass.
