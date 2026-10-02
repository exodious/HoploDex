# Feature Specification: Unsaved Changes per Field, with Revert

**Feature Branch**: `008-unsaved-field-changes`

**Created**: 2026-10-02

**Status**: Draft

**Input**: User description: GitHub issue #17, "Show unsaved changes per field, with per-field revert and a save button enabled only when something changed". While a saved record is being edited, each input that differs from what is saved shows that it has changed and offers a way to put back the saved value, and the form's Save button is enabled only while the form differs from what is saved. The request, the notes recorded on the issue and the follow-up about the Mounted section from feature 006 are reproduced under [Source Request](#source-request) so this spec stands on its own.

## Clarifications

### Session 2026-10-02

- Q: Is the database's Settings dialog in scope? It edits saved settings and applies them with Save, but holds no record and keeps nothing at a lock. → A: Yes. It gets the changed markers, per-field revert and a Save button disabled until something differs from the saved settings. Its unsaved settings are still not kept at a lock and the quit question still doesn't ask about them (003 FR-039) (FR-001).
- Q: Do forms that create something new (add firearm, add policy, add accessory) keep Save disabled until something is entered? → A: Save is disabled until every required field is filled in; on a form with no required fields, until at least one field is set. They have no markers or revert, since nothing on them is saved yet (FR-001, FR-015a). (This replaces an earlier answer of the same session that kept Save enabled from the start.)
- Q: When does an add form count as having unsaved changes for the quit question and for pending changes kept at a lock, given that its Save can be disabled while it holds input? → A: As soon as anything is entered, judged by FR-002's comparison against the form as it opened, whether or not Save is enabled. FR-019's "exactly when Save is enabled" applies to the forms that edit something saved (FR-019a).
- Q: Should the record page's mount changes (Unmount, Mount → Existing accessory or firearm…) be held until the record is saved, and where? → A: Yes, in the edit form. A firearm's or accessory's edit form gains a **Mounted** list of what is mounted directly on it, where existing records are added and removed; each change is held as an unsaved change with its own marker and revert and is saved with the form's Save. The record page's Mounted section becomes a read-only list, with a way to open the edit form at that list, and keeps Mount → New accessory…, which creates a record. This keeps 003's rule that every form holding unsaved changes is a dialog (FR-024 to FR-028, User Story 5).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See Which Fields I Have Changed (Priority: P1)

A collector opens a firearm's edit form to correct its serial number and update its estimated value. As soon as they type in a field, a "changed" marker appears beside that field; the fields they haven't touched show nothing. Before saving, a glance down the form shows exactly the two fields that will change. If they type a value back to what it was, the marker goes away, because the field no longer differs from what is saved. A section that is folded away shows that something inside it has changed.

**Why this priority**: Knowing what a save will change is the core of the request and the base the other stories build on. Without it, revert and the save button's state have nothing to refer to. It delivers value on its own: the user can check their edits before saving.

**Independent Test**: Open the edit form of a saved firearm and of a saved insurance policy; change one field, several fields, and a field inside a folded section; type a field back to its saved value; confirm the markers appear and disappear as described and that nothing is saved.

**Acceptance Scenarios**:

1. **Given** the edit form of a saved firearm, just opened, **When** the user looks at it, **Then** no field shows a changed marker.
2. **Given** that form, **When** the user changes the make from "Glock" to "Glock Ges.m.b.H.", **Then** a changed marker appears beside Make, and no other field shows one.
3. **Given** the make changed, **When** the user types it back to "Glock", **Then** the marker beside Make disappears.
4. **Given** a saved make "Glock", **When** the user adds a trailing space ("Glock "), **Then** Make shows no change, because the saved value would be the same; **and When** they type "glock", **Then** Make shows a change.
5. **Given** a saved estimated value of $1,250, **When** the user types "01250", **Then** Estimated value shows no change.
6. **Given** a saved weight of 2 lb 8.5 oz, **When** the user enters 0 lb 40.5 oz, **Then** Weight shows no change; **and When** they enter 2 lb 9 oz, **Then** Weight shows one change, not one per box.
7. **Given** the Physical details section folded away, **When** a field inside it differs from the saved value, **Then** the section's folded summary says that it has changes.
8. **Given** a changed field, **When** a screen-reader user moves to it, **Then** they hear that it has been changed, along with its label, hint and any error.
9. **Given** the edit form of a saved insurance policy, **When** the user changes its end date, **Then** the same marker appears beside End date as on a firearm's form.

---

### User Story 2 - Put One Field Back to Its Saved Value (Priority: P2)

Halfway through a long edit, the collector realizes they overwrote the notes by mistake. Beside the Notes field's changed marker is a revert control that names the saved value it will restore. They use it, the notes return to what is saved, and every other edit they made is untouched.

**Why this priority**: It is the second half of the request. It depends on User Story 1 to know which fields differ, and it saves the user from cancelling a whole form to undo one mistake.

**Independent Test**: Change several fields, including one whose change made the form clear or hide other fields (type, origin, registration classification); revert each in turn with mouse and keyboard; confirm each returns to its saved value, the fields it cleared come back, the other edits stay, and nothing is saved.

**Acceptance Scenarios**:

1. **Given** Make and Notes changed, **When** the user reverts Notes, **Then** Notes shows its saved text, its marker and revert control disappear, Make keeps its edit, and nothing is saved.
2. **Given** a changed field, **When** the user inspects its revert control, **Then** it names the field and the saved value it restores (for example "Revert Make to “Glock”"), with an empty saved value described as "Not recorded" and a long value shortened.
3. **Given** a changed field, **When** a keyboard user tabs to its revert control and activates it, **Then** the field is reverted and focus moves to the field's own input.
4. **Given** a Rifle with action "Bolt action", barrel length 20 in and capacity 5, **When** the user changes the type to Suppressor (which clears the action at once and will clear barrel length and capacity at save) and then reverts Type, **Then** Type is Rifle again and the action, barrel length and capacity are all back to their saved values, none marked as changed.
5. **Given** the type changed to Suppressor and then back to Rifle by choosing it, with the action cleared by the change, **When** the user looks at Action, **Then** Action shows a change (it is empty, the saved value is "Bolt action") with its own revert control.
6. **Given** an imported firearm with an importer and original marks, **When** the user confirms changing the origin to one without import marks and then reverts Origin, **Then** the origin, importer, country and original marks return to their saved values, without a confirmation.
7. **Given** a registered firearm whose classification the user confirmed clearing, **When** they revert "Registered as", **Then** the classification and its registration details return to their saved values.
8. **Given** a cartridge or make that the form re-spelled on leaving the field (004's suggestions), **When** the user reverts that field, **Then** the saved text returns, the note about the re-spelling disappears, and the field is not re-spelled again until the user next edits it.
9. **Given** a field with a validation error from the user's edit, **When** they revert it, **Then** the error disappears with the edit.

---

### User Story 3 - Save Only When There Is Something to Save (Priority: P3)

The collector opens a record's edit form just to look something up. Save is disabled, and the form says there are no changes to save. When they change a field, Save becomes available; when they revert it, or type it back, Save is disabled again. The same holds in every form that edits something already saved.

**Why this priority**: It is the third part of the request and the simplest. It is most useful once the markers show what Save would do, but it can be tested on its own.

**Independent Test**: In each form in scope, open it, check Save is disabled, change a field, check Save is enabled, revert it or type it back, check Save is disabled; press Enter in a field while Save is disabled and confirm nothing is saved and no "saved" message appears.

**Acceptance Scenarios**:

1. **Given** any form in scope, just opened, **When** the user looks at its Save button, **Then** it is disabled, and it is perceivable (including to assistive technology) that there are no changes to save.
2. **Given** that form, **When** the user changes any field, **Then** Save is enabled at once.
3. **Given** a single changed field, **When** the user reverts it or types it back to the saved value, **Then** Save is disabled again.
4. **Given** Save disabled, **When** the user presses Enter in a field, **Then** nothing is saved and no confirmation or "saved" message appears.
5. **Given** a changed field with a value that fails validation (for example an approved date in the future), **When** the user looks at Save, **Then** it is enabled, and pressing it shows the errors as today without saving.
6. **Given** a save that fails (validation from the system, or a warning the user cancels), **When** the form returns, **Then** the markers and Save's state are as they were before the attempt.
7. **Given** a firearm's coverage dialog showing "Scheduled on Policy A, $2,000", **When** the user changes the amount and back again, **Then** Save is enabled and then disabled again, and the amount shows a marker in between.
8. **Given** the add firearm form, just opened, **When** the user looks at Save, **Then** it is disabled and it is perceivable that required fields are still empty; **When** they fill in every required field, **Then** Save is enabled; **and When** they empty one of them again, **Then** Save is disabled; no field shows a changed marker or revert at any point.
9. **Given** an add form with no required fields, just opened, **When** the user sets any one field, **Then** Save is enabled; **and When** they clear it again, **Then** Save is disabled.

---

### User Story 4 - Unsaved Means the Same Thing Everywhere (Priority: P4)

The collector edits a record, changes a field, then types it back. Nothing differs from what is saved, so when they quit, the application doesn't ask whether to save, and when the database locks, no pending changes are kept for the next open. When the form does differ, quitting asks, locking keeps the changes, and on the next open "Resume editing" brings back the form with the same fields marked as changed and Save available.

**Why this priority**: Feature 003 already asks at quit and keeps pending changes at a lock; this story makes those flows agree with the markers. It builds on User Stories 1 and 3.

**Independent Test**: With a form typed back to its saved values, quit and lock; confirm no question and no pending changes. With a changed form, lock, reopen, resume; confirm the same fields are marked, Save is enabled, reverting each restores the saved value, and reverting the last one leaves no pending changes behind at the next lock.

**Acceptance Scenarios**:

1. **Given** a firearm's edit form whose only edit has been typed back to the saved value, **When** the user quits the application, **Then** it does not ask whether to save changes (003 FR-010).
2. **Given** that same form, **When** the database locks, **Then** no pending changes are kept, and the next open goes straight to the collection (003 FR-039).
3. **Given** a form with Make and Notes changed, **When** the database locks and is reopened and the user chooses "Resume editing", **Then** the form reopens with Make and Notes marked as changed, no other field marked, and Save enabled.
4. **Given** that resumed form, **When** the user reverts both fields and the database locks again, **Then** no pending changes are kept.
5. **Given** a form with changes, **When** the user quits, **Then** the question asks to save, discard or cancel exactly as today.
6. **Given** the add firearm form with only a make entered, so Save is disabled, **When** the user quits, **Then** they are asked to save, discard or cancel, and choosing to save shows the required fields' errors without saving; **and When** the database locks instead, **Then** the make is kept as pending changes and resuming brings it back with Save still disabled (FR-019a).
7. **Given** the add firearm form with a make typed and then deleted, **When** the user quits or the database locks, **Then** nothing is asked and no pending changes are kept.

---

### User Story 5 - Change What Is Mounted, and Save It with the Record (Priority: P5)

*Depends on feature 006 (accessories and mounts).*

A collector reworks a rifle: the old scope comes off and a red dot they already own goes on. On the rifle's record page the Mounted section lists what is mounted on it, read-only, with a way to change it. That opens the rifle's edit form at its Mounted list. They remove the scope and add the red dot, which is currently mounted on a pistol. Both rows are marked as changed: the scope "will be unmounted", the red dot "will be mounted here, moving from" the pistol. Nothing has changed in the collection yet. They change their mind about the scope and revert its row; then they save, and only the red dot moves.

**Why this priority**: It answers the follow-up comment on the issue, and it depends on feature 006, which is not yet merged. It reuses the markers, revert and Save of User Stories 1 to 4.

**Independent Test**: On a firearm and on an accessory, add an unmounted record, add one mounted elsewhere, and remove one in the edit form's Mounted list; confirm nothing changes until Save, each row is marked and revertible, Save's state follows the list, a lock keeps the list's changes as pending changes and resuming restores them, and Save stores all of them at once with the form's other fields.

**Acceptance Scenarios**:

1. **Given** a firearm's record page with a scope mounted on it, **When** the user looks at the Mounted section, **Then** it lists the scope (each entry a link) with no Unmount control, offers a way to change what is mounted that opens the edit form at its Mounted list, and still offers Mount → New accessory….
2. **Given** that firearm's edit form, **When** the user looks at its Mounted list, **Then** it lists what is mounted directly on the firearm, with a way to add an existing accessory or firearm and a way to remove each entry, and Save is disabled.
3. **Given** the Mounted list, **When** the user removes the scope, **Then** the scope's row stays in the list marked as changed and saying it will be unmounted, with a revert; nothing is saved and nothing is asked; and Save is enabled.
4. **Given** the Mounted list, **When** the user adds a red dot that is mounted on a pistol, **Then** a row for it appears, marked as changed and saying it will move here from the pistol, along with anything mounted on it; nothing is asked or saved.
5. **Given** the added red dot, **When** the user reverts its row, **Then** the row disappears and the red dot stays mounted on the pistol; **and Given** the removed scope, **When** the user reverts its row, **Then** it is listed as mounted again with no marker.
6. **Given** the scope removed and added back, or the red dot added and then removed, **When** the user looks at the form, **Then** the list shows no change for it, and Save is disabled if nothing else changed.
7. **Given** the scope removed, the red dot added and the firearm's notes changed, **When** the user saves, **Then** the notes, the unmount and the mount are all saved together; the scope is no longer mounted, the red dot and everything on it are mounted on the firearm, and the pistol no longer lists the red dot; **and When** the save does not go through, **Then** none of them is saved.
8. **Given** the add chooser, **When** the user looks for something to add, **Then** it offers only active records that could be mounted here without forming a loop, judged against what the form currently holds (including its own changed "Mounted on").
9. **Given** changes in the Mounted list, **When** the database locks and is reopened and the user resumes editing, **Then** the form opens with the same rows marked; **and When** the user quits instead, **Then** they are asked to save, discard or cancel.
10. **Given** the record page's Mount → New accessory…, **When** the user saves the new accessory, **Then** it is created mounted on this record, as in 006.

---

### Edge Cases

- **A field typed back to its saved value**: unchanged; no marker, and it doesn't count towards Save, the quit question or pending changes (FR-002, FR-019).
- **Whitespace, leading zeros, equivalent units**: compared as the save would store them, so " Glock", "01250" and 0 lb 40.5 oz against a saved 2 lb 8.5 oz are unchanged (FR-002). A change of letter case is a change.
- **A value that can't be read yet** (a half-typed date such as "3/1", or "13/45/2024"): it differs from the saved value, so it is marked as changed and Save is enabled; pressing Save shows the field's error (FR-003).
- **A date typed differently** ("3/14/2024" against a saved 2024-03-14): unchanged once the date can be read.
- **The serial number while "no serial number" is checked**: the serial field is empty and disabled; the change is shown on the checkbox, and the serial number field shows a change only when the value that would be saved differs (FR-005).
- **Fields hidden by another field's change** (barrel length after a type change, registration details after clearing the classification): they keep no marker of their own while hidden; the change is shown on the field that hid them, and the existing note still names what will be cleared at save. Reverting that field brings them back as saved (FR-005, FR-012).
- **A field the form filled in automatically** (a caliber suggestion accepted with "Use .45", an action cleared by a type change): it is an ordinary change with its own marker and revert.
- **A user reverts a field that was cleared automatically, then reverts the field that cleared it**: each revert restores its own field; the outcome is the saved record either way.
- **A field whose saved value is no longer offered** (a registration classification or action removed from a list in a later version, which the form keeps offering for that record): it is unchanged until the user picks something else, and reverting brings it back.
- **A folded section with changes**: its summary says it has changes, and opening it shows which (FR-007).
- **Reverting the last change**: Save is disabled and the kept pending changes are removed at once, not at the next lock (FR-015, FR-021).
- **A resumed draft from an older or different version of the form**: the form takes only the fields it knows, as 003 already allows; markers are worked out against the saved record as usual, so a draft whose values all match the record opens with no changes and Save disabled.
- **The record was changed after the form opened**: not possible within one computer, since only one form is open and the database is held by one computer at a time; a take-over closes the session (003).
- **An add form with input but a required field still empty**: Save is disabled (FR-015a), but the input is unsaved: quitting asks about it and a lock keeps it as pending changes (FR-019a).
- **An add form whose only required field becomes optional** (checking "no serial number"): Save follows what is required now, so it may become enabled without anything else being typed.
- **Cancel, Escape or the dialog's close button with changes**: behaves as today (the form closes and the edits are dropped); this feature adds no question there.
- **Photos and documents added or removed while a record is open**: not form changes; they are saved at once on the record page, as today, and never mark a field or enable Save (FR-022).
- **A Mounted list entry disposed of or deleted while the form is open**: not possible, since only one form is open at a time and those actions are taken from outside it.
- **Adding to the Mounted list the record this one is mounted on**: not offered, since it would form a loop (FR-026); the same holds if the form's own "Mounted on" was changed to one of the list's entries.
- **Removing an entry and changing "Mounted on" in the same form**: both are ordinary changes with their own markers and reverts, saved together (FR-027).
- **A change to a setting that the dialog applies only on Save** (backups location chosen through Change…, then "Use the default"): judged against the saved setting like any field (FR-001).

## Requirements *(mandatory)*

### Functional Requirements

**Which forms**

- **FR-001**: The changed markers, per-field revert and the Save button's state (FR-006 to FR-018) MUST apply to every form that edits something already saved:
  - a firearm's edit form;
  - an insurance policy's edit form;
  - a firearm's coverage dialog;
  - once feature 006 is merged, an accessory's edit form and the "Mounted on" field of every edit form (FR-023);
  - the database's Settings dialog, for the settings it applies with Save (not its immediate actions: restoring from or deleting backups, remembering or forgetting the passphrase).
  Forms that create something new (add firearm, add policy, add accessory) show no markers or revert; their Save follows FR-015a and their unsaved changes FR-019a. Forms that perform an action rather than edit saved values are also out of scope and keep their current buttons: Mark disposed, Restore, Delete policy, Import, Export, Change passphrase, Create database, Restore backup and the pending-changes prompt.

**What counts as changed**

- **FR-002**: A field MUST count as changed exactly when the value a save would store differs from the saved value. The comparison MUST treat as equal what a save would store the same way: leading and trailing spaces, an empty entry and "not recorded", numbers that differ only in leading zeros or trailing decimal zeros, money in whole dollars, lengths and weights at the precision they are stored (a weight entered in pounds and ounces compared as one total), and dates once they can be read. Any other difference, including a change of letter case or of spacing inside a text, MUST count as a change.
- **FR-003**: A value the form can't read yet (an incomplete or impossible date or number) MUST count as changed, unless the field is empty and the saved value is "not recorded".
- **FR-004**: The changed state MUST follow the user's input as they type, choose or pick, and MUST become unchanged as soon as the field is typed or chosen back to the saved value.
- **FR-005**: Where the form keeps a value that a save will not store (fields the type doesn't have, the serial number while "no serial number" is checked, registration details while no classification is chosen, the origin's discarded details), whether the record changes MUST be judged by what a save would store, and the change MUST be shown on the visible field that causes it. Hidden fields MUST NOT show markers; the form's existing notes that name what will be cleared at save MUST stay.

**The changed marker**

- **FR-006**: Every changed field MUST show a "changed" marker next to its input. The marker MUST be perceivable without color, MUST be announced to assistive technology as part of the field's description together with its label, hint and any error, and MUST NOT replace the field's hint, note or error.
- **FR-007**: A folded section of a form MUST say in its summary when any field inside it is changed.
- **FR-008**: A field entered in more than one box (weight in pounds and ounces) MUST show one marker and offer one revert for the whole value.
- **FR-009**: Fields set by choosing rather than typing (choice cards such as Type and Origin, selects, checkboxes, a date picker, a folder chosen through a "Change…" button, the "Mounted on" chooser) MUST show the same marker and revert as text fields.

**Per-field revert**

- **FR-010**: Each changed field MUST offer a revert control next to its marker that the user can reach and use with the keyboard. Its accessible name and visible tooltip or label MUST name the field and the saved value it restores; an empty saved value is described as "Not recorded", and a value too long to show is shortened.
- **FR-011**: Reverting a field MUST set it back to its saved value without asking for confirmation and without saving anything. It MUST also remove what the edit brought with it for that field: its validation error, the note left by re-spelling it to a suggestion (004), and a caliber suggestion offered because of it. A reverted field MUST NOT be re-spelled or given a suggestion until the user next edits it.
- **FR-012**: Reverting a field whose change made the form clear or hide other fields MUST also restore those fields to their saved values, unless the user has edited them since. This covers at least: Type (action, barrel length, capacity, and the cartridge's and caliber's labels and hints), Origin (country, importer and original marks), "Registered as" (form, approved date and "Registered to") and, once 006 is merged, any field whose change cleared another. Reverting MUST NOT change any other field the user edited.
- **FR-013**: After a revert, focus MUST move to the reverted field's input.
- **FR-014**: Revert MUST be available only for fields that are changed; an unchanged field MUST show neither marker nor revert.

**The Save button**

- **FR-015**: A form's Save button MUST be disabled while no field is changed and enabled while at least one is, updating as the user edits and reverts.
- **FR-015a**: On a form that creates something new (add firearm, add policy, add accessory), Save MUST be disabled until every field that is required, given what the form currently holds (for example the serial number unless "no serial number" is checked), is filled in, and MUST be disabled again when one is emptied. On such a form with no required fields, Save MUST be disabled until at least one field differs from how the form opened (FR-002). A filled-in value that fails validation still counts as filled in; pressing Save shows the errors and saves nothing (FR-017). While Save is disabled, pressing Enter MUST NOT save, and the form MUST make it perceivable, including to assistive technology, that required fields are still empty.
- **FR-016**: While Save is disabled, pressing Enter in the form MUST NOT save, and the form MUST make it perceivable, including to assistive technology, that there are no changes to save.
- **FR-017**: While at least one field is changed, Save MUST stay enabled even if the form has errors; pressing it MUST show the errors and save nothing, as today.
- **FR-018**: Saving MUST behave as today in every other respect: the confirmations and overridable warnings (002's original-marks match, 005's classification clearing, the serial number attestation) still apply, and if a save does not go through the form's markers and Save state MUST be as they were before the attempt.

**Agreement with feature 003's unsaved changes**

- **FR-019**: A form in scope (FR-001) MUST count as having unsaved changes for the question asked at quitting (003 FR-010) and for pending changes kept at a lock (003 FR-039) exactly when its Save button is enabled. A form whose edits have all been reverted or typed back MUST NOT cause the quit question and MUST NOT leave pending changes.
- **FR-019a**: A form that creates something new MUST count as having unsaved changes for the quit question and for pending changes kept at a lock as soon as any field differs from how the form opened, compared as FR-002 compares, whether or not its Save is enabled (FR-015a). An add form left as it opened, or typed back to it, MUST NOT cause the quit question and MUST NOT leave pending changes. Resuming its pending changes reopens it with Save enabled or disabled by FR-015a.
- **FR-020**: When the user resumes pending changes (003 FR-039), the form MUST open with exactly the fields that differ from the saved record marked as changed, Save enabled if any do, and each revert restoring the saved record's value.
- **FR-021**: Reverting MUST update the kept pending changes as any other edit does; reverting the last change MUST remove them.

**Photos and documents**

- **FR-022**: Adding, removing or choosing the thumbnail of a photo or document MUST NOT count as a form change. They stay saved at once from the record page as today, never show a changed marker, never enable Save, and are never part of pending changes.

**Mounts (feature 006)**

- **FR-023**: Once feature 006 is merged, the "Mounted on" field of a firearm's or accessory's edit form MUST be a field like any other: marked when changed, revertible to the saved mount, and counted for Save, the quit question and pending changes.
- **FR-024**: Once feature 006 is merged, a firearm's or accessory's edit form MUST have a **Mounted** list showing what is mounted directly on that record, from which the user can add an existing active accessory or firearm and remove any entry. Adding and removing MUST change only the form: nothing is saved, and no confirmation is asked, until the form is saved.
- **FR-025**: Each added or removed entry MUST be marked as changed and say what saving will do: a removed entry stays listed and says it will be unmounted; an added entry says it will be mounted here and, if it is mounted elsewhere now, that it will move from there with everything mounted on it. Each MUST have a revert that undoes that one change. An entry removed and added back, or added and removed, MUST show no change. The list's changes MUST count for Save, the quit question and pending changes like any field (FR-015, FR-019 to FR-021).
- **FR-026**: The add chooser MUST offer only active records that can be mounted on this one without forming a loop, judged against what the form currently holds, including a changed "Mounted on" and the list's own changes.
- **FR-027**: Saving the form MUST save the Mounted list's changes together with its other fields, all or none.
- **FR-028**: The record page's Mounted section MUST become read-only: it lists what is mounted on the record, each a link as in 006, and offers a way to change what is mounted that opens the edit form at its Mounted list. Unmount and Mount → Existing accessory or firearm… MUST no longer act from the page, and their confirmations ("Unmount {name}?", "Move {name}?") are no longer needed. Mount → New accessory… MUST stay, since it creates a record and its mount is saved when that accessory is saved, as in 006.

**Consistency**

- **FR-029**: The marker, the revert control, their wording and the disabled Save MUST look and behave the same in every form in scope, as part of the shared form components (constitution III), and MUST meet WCAG 2.1 AA, including contrast and visible focus for the revert control.

### Key Entities

- **Saved value**: what is stored for a field of the record (or setting) being edited, as it was when the form opened. It is the reference for every marker and every revert. A form being resumed from pending changes uses the saved record, not the draft, as its saved values.
- **Edited value**: what the form currently holds for a field, which may not be readable yet.
- **Field change**: a field whose edited value, stored as a save would store it, differs from its saved value. A form that edits something saved has unsaved changes when it has at least one field change; that one definition drives the markers, Save, the quit question and pending changes. An add form has unsaved changes when any field differs from how it opened, but its Save depends on its required fields (FR-015a, FR-019a).
- **Pending changes** (feature 003): the form's unsaved input kept in the database at a lock. Unchanged in shape; this feature only settles when a form has any.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In every form in scope, after any single edit, the changed marker appears or disappears within 100 ms, with 10,000 items in the collection.
- **SC-002**: In every form in scope, opening the form and making no change, or making changes and then reverting or typing all of them back, leaves Save disabled in 100% of cases; any remaining change leaves it enabled in 100% of cases. On every add form, Save is enabled in exactly the cases where every currently required field is filled in (or, with none required, any field is set).
- **SC-003**: Any single changed field can be returned to its saved value in one action (one click, or one key press on its revert control), without affecting any other edit.
- **SC-004**: After reverting any field, including Type, Origin and "Registered as", saving the form stores exactly the saved value for that field and for every field it had cleared.
- **SC-005**: The quit question is asked, and pending changes are kept at a lock, in exactly the cases where the form's Save is enabled: never for a form with no field changes, always for one with at least one. For add forms, they happen whenever any field differs from how the form opened, including when Save is disabled.
- **SC-006**: A screen-reader user can find every changed field and revert it using the keyboard alone, and hears for each one that it is changed and what value revert restores.
- **SC-007**: In a test with a collector editing a record, they can say which fields a save will change by looking at the form, without opening the record page to compare.
- **SC-008**: Once feature 006 is merged, no mount or unmount of an existing record takes effect before the user saves the edit form that holds it, and after a save every change in its Mounted list has taken effect, or none has.

## Assumptions

- **Comparison is against the value as saved, not as first shown**: the saved record is the reference even when a form opens with resumed pending changes, so the markers say what a save would change in the database.
- **No "revert all" and no undo of a revert**: the request asks for a revert per change. Cancel already drops every edit at once, and a reverted value is the user's own unsaved typing, small enough to retype, so reverting asks no confirmation.
- **Cancel stays as it is**: closing an edit form with changes drops them without a question today, and the request doesn't ask to change that. The markers make it visible what would be lost.
- **New-record forms**: everything on an add form is new, so there is nothing to mark or revert there. Its Save waits for the required fields instead, which the forms already mark as "Required" (clarification of 2026-10-02).
- **Settings and passphrases**: the Settings dialog's unsaved settings are still not kept at a lock and the quit question still doesn't ask about them (003 FR-039 keeps only record and policy forms); only its markers, revert and Save change. Passphrase fields are never compared or marked.
- **Photos and documents are not form input**: they are added and deleted from the record page and saved at once, and a file dropped on the window is ignored while a form is open, so nothing can change under an open form. This feature doesn't change that.
- **Disposition fields**: the edit form of a disposed firearm shows its disposition details, which are ordinary fields of that form and are covered like the others.
- **Feature 006 is merged first or alongside**: requirements naming accessories, the "Mounted on" field and the Mounted list (FR-023 to FR-028, User Story 5) apply once 006 is on the main branch; nothing else in this feature depends on it.
- **The Mounted list holds only what is mounted directly on the record**: what is mounted on those entries moves with them, as in 006, and is changed from their own forms. Moving an entry no longer asks first, because nothing happens until Save and the row says what Save will do.
- **No data changes for mounts**: a mount is stored as in 006; only when it is saved changes.
- **No data changes**: no stored record, setting or pending-changes layout changes; this feature is the forms' behavior only.

## Relationship to Other Features

This feature **amends**:

- `specs/003-database-protection-management/`: FR-010 and FR-039 ask about and keep "unsaved changes" without saying when a form has any; FR-019 here defines it, so a form typed back to its saved values no longer asks at quit or leaves pending changes. US6-6 and SC-010 (resuming reproduces the input) are unchanged; FR-020 adds the markers on resume.
- `specs/001-firearms-inventory/` FR-006 (edit a firearm record) and the insurance policy forms (FR-014, FR-036): their Save buttons are disabled until something changes, and on the add forms until the required fields are filled in (FR-015a).
- `specs/002-firearm-identification/` FR-010 and `specs/005-regulated-item-types/` FR-012: their confirmations before discarding details are unchanged, and reverting Origin or "Registered as" restores what they discarded (FR-012).
- `specs/004-cartridges-action-types/`: a reverted entry field drops its re-spelling note and isn't re-spelled until edited again (FR-011); FR-014's "text equal to the saved value is not settled" is unchanged.
- `specs/006-accessory-links/` (not yet merged): the AccessoryForm and the "Mounted on" field are covered (FR-001, FR-023). FR-012 and contracts/ui-accessories.md §5, which left "staging such changes until the record is saved" to this feature, are amended: the record page's Unmount and Mount → Existing accessory or firearm… move into the edit form's Mounted list, held until Save (FR-024 to FR-028); Mount → New accessory… is unchanged. The draft kept as pending changes for these forms gains the list's changes.
- `specs/003-database-protection-management/` ui-databases.md §7 (Settings): its Save is disabled until a setting differs from what is saved (FR-001).

## Source Request

This feature was filed as GitHub issue #17, "Show unsaved changes per field, with per-field revert and a save button enabled only when something changed" (labels: new feature, needs spec).

**The request, verbatim:**

> while editing an existing record (firearm or insurance), a visual indication that a change has been made needs to be provided next to each input, along with a way to revert that specific change to the saved value. If the form has no remaining changes, its save button should be disabled. The save button should only be enabled when the form has changes vs what is saved in all cases.

**Notes recorded on the issue, with where this spec answers them:**

- "It's a UI pattern, so under the UI-consistency principle it applies to every form and dialog that edits a saved record: FirearmForm, InsurancePolicyForm, DisposeDialog, RestoreDialog, CoverageDialog and the rest. Decide which of them count as 'editing an existing record'." → The firearm and policy edit forms, the coverage dialog and (with 006) the accessory edit form; Dispose, Restore and the other action dialogs are not edits of saved values (FR-001). The Settings dialog is also in scope; add forms get no markers or revert, and their Save waits for the required fields (FR-015a, clarifications of 2026-10-02).
- "'Changed' compares against the saved value, so a field edited and then typed back to its saved value shows no change, and the save button is disabled again." → FR-002, FR-004, FR-015.
- "Photos and documents: decide whether adding or removing an attachment counts as a form change, given that attachments are saved as they're added today." → It doesn't; they stay saved at once (FR-022).
- "Interaction with feature 003's unsaved-changes handling: closing the window, a lock, and the pending changes kept after a lock. The per-field 'changed' state should agree with what those flows treat as unsaved." → One definition for all of them (FR-019 to FR-021).
- *Related*: #30 (restore individual records from backups) and #31 (import records from other HoploDex databases) were once planned with this request as one spec. "This one stands alone: it undoes unsaved edits in the open form and needs no backup."

**Follow-up comment on the issue (2026-10-02), from feature 006's manual testing, verbatim request:**

> Unmounting happens without any confirmation whatsoever. Maybe this needs to go into the #17 issue for unsaved changes? This strongly feels like something that should be queued and require the user to select save on the record.

The comment records that 006 now asks before Unmount (and before a Move), still saving as soon as it is confirmed, and leaves to this issue "whether a record page's mount changes (Unmount, and mounting an existing record from the Mount menu) should be **staged**, shown as pending and saved only with the record's Save". Its questions, and where this spec answers them:

- "A record page isn't a form today: edits go through the edit dialog, and the Mounted section acts on the page itself. Staging would need a save/discard bar (or similar) on the page, or would have to move mount changes into the edit form, where 'Mounted on' already saves with the form." → Move them into the edit form, as a Mounted list held until Save; the record page's section becomes read-only (FR-024, FR-028, clarification of 2026-10-02).
- "Mount → New accessory… creates a record, so it probably stays immediate." → It does (FR-028).
- "Staged mount changes would need to fit feature 003's unsaved-changes flows: the close/quit question, and pending changes kept at a lock." → They are held by the edit form, so FR-019 to FR-021 apply (FR-025).
- "Per-item revert: a staged unmount or mount would need its own 'changed' marker and a way to revert it, as each field has." → Each added or removed entry has its own marker and revert (FR-025).
