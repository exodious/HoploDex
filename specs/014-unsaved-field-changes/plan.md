# Implementation Plan: Unsaved Changes per Field, with Revert

**Branch**: `fm/hoplodex-017-unsaved-field-changes` | **Date**: 2026-11-27 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/014-unsaved-field-changes/spec.md`

## Summary

One shared mechanism makes every form that edits something saved say which fields differ from what is saved, revert each one, and enable Save only while something differs: `changedFields` and `formChanges` (the comparison, research.md §1), a `change` prop on the shared field frame (marker and revert, §2), `Button`'s `unavailable` state (a disabled Save that stays focusable, §3) and a close guard in `Dialog` ("Discard changes?", §4). Its `isDirty` replaces each form's own `JSON.stringify` comparison as the input to `useDirtyForm`, so the quit question and pending changes agree with Save. Mount changes move from the record page into the firearm and accessory edit forms as a Mounted list saved with the record in one transaction (§8). The Settings dialog joins the same pieces (§9).

## Technical Context

**Language/Version**: Rust 1.97, TypeScript with React 18 (Tauri 2)

**Primary Dependencies**: Existing only (Radix Dialog through `Dialog`, `ConfirmDialog`, `MountChooser`). No new package or crate.

**Storage**: No schema change. Kept drafts of the firearm and accessory forms gain the Mounted list's changes (form versions 5 and 2).

**Testing**: Vitest (components and forms), `cargo nextest` with real databases (the mount changes), WebdriverIO E2E (journeys, close guard, lock and resume).

**Target Platform**: Linux, Linux Flatpak, macOS, Windows.

**Project Type**: Desktop app.

**Performance Goals**: Marker within 100 ms of an edit at 10,000 items (SC-001): the comparison is per form field, not per record (research.md §10).

**Constraints**: Hidden or unreadable values follow FR-003 and FR-005; nothing is saved by a revert.

**Scale/Scope**: 6 forms in scope (firearm edit, accessory edit, policy edit, coverage, Settings, plus the Mounted list), 3 add forms (Save unchanged, `isDirty` from the same hook), about 12 shared components touched, 2 commands' inputs and 1 command's options extended.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality**: one shared hook and component changes instead of per-form copies; no new layer. PASS.
- **II. Testing**: every scenario has a test below; mount saving is tested on real databases; tests stay in their sandboxes. PASS.
- **III. UX Consistency**: the marker, revert, disabled Save and discard question live in shared components and are carried to every form in scope; the discard question is `ConfirmDialog`, destructive, so it starts on Keep editing (#44); the new screens (changed form, Mounted list, discard question) join the screenshot walk. Accessibility: accessible names and descriptions for the marker, revert and Save (no conformance target, constitution 1.6.0). PASS.
- **IV. Performance**: research.md §10. PASS.
- **Security gates**: persistence (mount changes in the record's transaction, bounded, each reference checked) below; the per-PR AI review runs before the PR.

**Attack surface** (`docs/attack-surface.md`): unchanged: no new command, capability, file or helper; the changed inputs are in `ipc-commands`' existing paths (research.md §11). Re-checked by the per-PR review.

## Test Levels

| Story | E2E journey | Other E2E (error mechanism, real app, security) | Scenarios tested in Rust or Vitest |
|-------|-------------|-------------------------------------------------|------------------------------------|
| US1 | `record-firearm.e2e.ts`: edit a firearm, change Make, see the marker, type it back, marker gone | | US1-1 to US1-9 → Vitest `FirearmForm.changes.test.tsx`, `InsurancePolicyForm.test.tsx`, `Field`/`fieldChanges` unit tests |
| US2 | `record-firearm.e2e.ts`: revert a field by keyboard, focus lands in it | | US2-1 to US2-9 → Vitest `FirearmForm.changes.test.tsx` (Type, Origin, Registered as, entry fields) |
| US3 | `value-insurance.e2e.ts`: coverage dialog Save disabled, change amount, enabled, back, disabled; Enter does nothing | | US3-1 to US3-8 → Vitest per form (`CoverageDialog`, `InsurancePolicyForm`, `AccessoryForm`, `DatabaseSettingsDialog`, add forms) |
| US4 | `locking.e2e.ts`: change, lock, reopen, resume: same fields marked; revert all, lock: no pending changes; Escape asks "Discard changes?" | the lock and resume need the real app | US4-1, US4-2, US4-6, US4-7 → Vitest `FirearmForm.changes.test.tsx`; US4-8, US4-9 → Vitest `fieldChanges.test.tsx` (the close guard), `RestoreDialog.test.tsx`, `DisposeDialog.test.tsx` |
| US5 | `accessories-mounting.e2e.ts`: record page → Change what's mounted… → add one, then on another record add it again, its row saying where it moves from; Save; the record pages show the result. `accessories-dispose.e2e.ts`'s setup mounts the same way | None: the firearm form's field-error mechanism already has its E2E test (`record-firearm.e2e.ts`, the nickname), so the list's refusal on `mounted` is shown in Rust and Vitest | US5-1 to US5-10 → Rust `tests/mounted_list_test.rs` (all-or-none, stale removal, loop, bound, disposed, the candidates); Vitest `MountedEditor.test.tsx`, `MountedSection.test.tsx` |

## Project Structure

### Documentation (this feature)

```text
specs/014-unsaved-field-changes/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/tauri-commands.md
├── contracts/ui.md
└── tasks.md
```

### Source Code

```text
src/components/fieldChanges.ts, ChangedMark.tsx, Field.tsx, fieldIds.ts, Button.tsx, Dialog.tsx, Disclosure.tsx, components.css
src/components/TextField.tsx, TextArea.tsx, Select.tsx, DateField.tsx, MoneyField.tsx, NumberField.tsx, DecimalField.tsx, Checkbox.tsx, ChoiceCards.tsx, SegmentedControl.tsx
src/features/firearms/FirearmForm.tsx, EntryField.tsx, FirearmRecordPage.tsx
src/features/accessories/AccessoryForm.tsx, AccessoryRecordPage.tsx, types.ts
src/features/insurance/InsurancePolicyForm.tsx, CoverageDialog.tsx
src/features/databases/DatabaseSettingsDialog.tsx
src/features/mounts/MountedEditor.tsx, MountedSection.tsx, MountChooser.tsx, mountsService.ts
src/features/firearms/DisposeDialog.tsx, RestoreDialog.tsx   # close guard only
src-tauri/src/commands/firearms.rs, accessories.rs, mounts.rs; src-tauri/src/services/mounts.rs; src-tauri/src/models/
src-tauri/tests/ (mounts), src-tauri/examples/human_seed.rs if a sample needs it
e2e/specs/record-firearm.e2e.ts, value-insurance.e2e.ts, locking.e2e.ts, accessories-mounting.e2e.ts, e2e/screenshots/screens.e2e.ts
docs/architecture/frontend.md; specs/006-accessory-links/ and specs/003-database-protection-management/ amendment notes
```

**Structure Decision**: The existing Tauri layout; the shared pieces go in `src/components/` beside the field components they extend.

## Close-out (at pull request time)

- Issue #17 is closed by the pull request's `Closes #17`; check `closingIssuesReferences` lists it.
- No other open issue is settled by this work (#30 and #31 stand apart); check each named issue's state before commenting.

## Complexity Tracking

None.
