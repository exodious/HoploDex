# Research: Unsaved Changes per Field, with Revert

Feature 014, spec [spec.md](spec.md). Each section is a decision, why, and what else was weighed.

## §1 One definition of "changed", shared by every form

**Decision**: A new hook, `useFieldChanges` (`src/components/fieldChanges.ts`), takes the form's saved state, its current state and one function per form, `stored(field, state)`, that returns what a save would store for that field as a comparable string (or a marker for "can't be read yet", FR-003). It returns the set of changed fields and `isDirty` (at least one). Every form in scope passes that `isDirty` to `useDirtyForm` in place of today's `JSON.stringify(form) !== JSON.stringify(pristine)`, so the markers, Save, the quit question and pending changes all read the one definition (spec FR-019). `useDirtyForm`'s contract doesn't change: it already takes `isDirty`.

Add forms use the same hook against the form as it opened (FR-019a), and show nothing from it but `isDirty`.

**Why**: The rules for "what a save would store" (trimming, leading zeros, whole dollars, pounds and ounces as one weight, dates once readable, hidden fields that won't be saved) already live in each form's input builder (`toInput`-style functions). `stored` reuses those parsers per field rather than building a second normaliser, and keeps the shared part to the comparison and bookkeeping.

**Alternatives**: Comparing the whole save input against the saved record's (one call): fails for an unreadable value, which has no save input, and can't say which field changed. A per-component `changed` computed inside each field: the weight's two boxes, hidden fields and the type's knock-on clears can't be judged by one field alone.

## §2 The marker and revert live in the shared field frame

**Decision**: `FieldFrame` and `useFieldIds` (`src/components/Field.tsx`, `fieldIds.ts`) take an optional `change?: { savedText: string; onRevert: () => void }`. When set, the label row shows a `ChangedMark`: a "Changed" tag (with an id joined into the control's `aria-describedby`, before the hint and error, FR-006) and an icon-only `Button` (↺) whose `aria-label` is "Revert {label} to “{saved}”" and whose tooltip is "Revert to “{saved}”" (FR-010). `TextField`, `TextArea`, `Select`, `DateField`, `MoneyField`, `NumberField`, `DecimalField`, `Checkbox`, `ChoiceCards`, `SegmentedControl` and the entry fields pass it through; fieldsets (choice cards, the weight's pair of boxes, the Mounted on chooser, the Mounted list's rows) render `ChangedMark` beside their legend. "Not recorded" names an empty saved value, and a saved text longer than 40 characters is shortened with "…".

**Why**: FR-029 asks for one look in every form; the frame is already the one place every field's label, hint and error is laid out.

**Alternatives**: A wrapper component around each field in the forms: hundreds of call-site edits and easy to miss one.

## §3 Save that is disabled but focusable

**Decision**: `Button` gains `unavailable?: string`. When set, it renders `aria-disabled="true"`, the disabled look, a `title` and a visually hidden description with that text, and swallows clicks. Each form in scope passes `unavailable={isDirty ? undefined : "No changes to save"}` to its Save, and its submit handler returns at once when `!isDirty`, which covers Enter in a field (FR-016, the Enter rule #44). The Settings dialog's `disabled={!dirty}` (#100) moves to the same prop.

**Why**: A real `disabled` drops out of the tab order and its tooltip can't be reached by keyboard (clarification of 2026-10-02).

## §4 The "Discard changes?" question at close

**Decision**: `Dialog` provides a small context, and a hook `useCloseGuard(isDirty)` lets the form inside register whether a close needs asking. Escape, the close button and a click outside go through `Dialog`'s `onOpenChange(false)`; when the registered guard says the form is dirty, the dialog asks instead, with `ConfirmDialog` ("Discard changes?", destructive, Discard / Keep editing, focus starting on Keep editing per #44). The form's Cancel button calls the hook's `requestClose()`, which does the same. Saving, the quit question and a lock close the form without going through `onOpenChange`, so they don't ask (FR-021a). Radix already routes Escape to the topmost layer, so an open suggestion list, menu or picker closes first.

**Alternatives**: Asking in each page that hosts a form: one copy per page, against constitution III.

## §5 Reverting a field that cleared others (FR-012)

**Decision**: Each form's `revert(field)` sets the field to its saved value and, for the fields that field's change cleared or hid (Type: action, barrel length, capacity; Origin: country, importer, original marks; "Registered as": form, approved date, Registered to), restores each one whose current value is still what the change left (empty, or hidden) to its saved value. A dependent the user has typed into since keeps the user's value. Origin's and "Registered as"'s discard confirmations aren't asked again (spec US2-6, US2-7).

## §6 Entry fields' re-spelling (004)

**Decision**: Reverting an entry field (make, model, cartridge, caliber) clears its re-spelling note and caliber suggestion and sets a per-field "settled" flag, so the leave-field re-spelling doesn't run until the next edit clears it (FR-011).

## §7 Folded sections

**Decision**: `Disclosure` gains `changed?: boolean`; its summary then ends "· Changed" (text, so it is perceivable without colour) (FR-007).

## §8 The Mounted list in the edit form (User Story 5)

**Decision**:
- **Frontend**: a `MountedEditor` (`src/features/mounts/MountedEditor.tsx`) in the firearm and accessory edit forms of an active record lists what is directly mounted (from `getMountDetail`'s depth-1 entries), with Add (the existing `MountChooser` in `role: "item"`) and a Remove per row. The form state holds `mountAdds: RecordLabel[]` (with where each is mounted now) and `mountRemoves: RecordRef[]`; adding a removed entry cancels the removal and removing an added one cancels the add, so a round trip is no change (FR-025). Each changed row shows `ChangedMark` with a revert that undoes that one change.
- **Candidates**: `list_mount_candidates` (`role: "item"`) gains an optional `hostMountedOn: RecordRef | null | undefined`: the host's "Mounted on" as the form holds it, so the host's chain is judged as the form would save it (FR-026). The chooser also leaves out what the list already holds or adds.
- **Saving**: `FirearmInput` and `AccessoryInput` gain `mountChanges: { item: RecordRef; mounted: boolean }[]` (default empty, at most 1,000 entries). `update_firearm` and `update_accessory` apply them inside the transaction that saves the record and its own "Mounted on" (`mounts::atomically`), after the record's own mount: a removal requires the item to be directly on this record now (else `CONFLICT`, as a stale form), an addition goes through `mounts::set_mount` with a fresh graph, so a loop is refused with a field error on `mounted`. All or none (FR-027).
- **Drafts**: the forms' kept input gains the two lists, so `FORM_VERSION` goes up (firearm 4 → 5, accessory 1 → 2); pending changes kept by an earlier version are offered as kept by another version (008 FR-016).
- **Record page**: `MountedSection` loses Unmount, Mount → Existing… and their two questions; it keeps the read-only list, Mount → New accessory…, and gains "Change what's mounted…", which opens the edit form with `focusField: "mounted"` (FR-028).

**Alternatives**: A save/discard bar on the record page (rejected by the clarification of 2026-10-02); a separate command for the list's changes after the record's save (not all-or-none).

## §9 Settings dialog

**Decision**: `SettingsForm` keeps its own state and comparison (#100) but feeds it through `useFieldChanges` with a `stored` for each setting (the keep count compared as a number, so "05" equals 5), shows markers and revert on Automatic backups, Keep, Location, the three locking settings and Remember the passphrase, and uses §3's Save and §4's question. Open documents and the backup actions stay outside (FR-001).

## §10 Performance

The comparison is a pass over a form's fields (at most about 40) on each render, independent of the collection's size; the candidates query is the existing one with one more excluded chain. SC-001's 100 ms holds at 10,000 items. Saving a Mounted list of n changes costs n `set_mount` calls in one transaction (each O(chain depth) since #233).

## §11 Attack surface

No new command, capability or file access. `update_firearm` and `update_accessory` take a bounded list more, and `list_mount_candidates` one optional reference more; their code paths are already in `ipc-commands` and `src-tauri/src/services/mounts.rs`. Each reference the page sends is checked as any other (it must exist and be active), and the list's length is bounded. `docs/attack-surface.md` is unchanged unless the per-PR review finds otherwise.
