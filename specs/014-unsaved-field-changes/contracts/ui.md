# UI contract: feature 014

## §1 The changed marker and revert (FR-006, FR-010, FR-013)

```
Make *   [Changed] [↺]
[ Glock Ges.m.b.H.          ]
Hint text
```

- The tag reads "Changed" in the field label row, after the label (and "*"). Style: the shared `hd-badge--info` tag, its word legible without colour.
- The revert button is icon-only (the `revert` icon), `Button size="sm" variant="ghost"`. Tooltip: `Revert to “{saved}”`; accessible name: `Revert {label} to “{saved}”`. Saved text: "Not recorded" when empty; longer than 40 characters, cut at 40 with "…".
- The control's `aria-describedby` lists the tag first, then hint, then error.
- Activating revert restores the value and focuses the field's input (or, for choice cards and checkboxes, the selected or first option).
- Fieldsets (Type, Origin, weight, Mounted on, a Mounted list row) put the tag and revert after the legend or row name.
- No tag or revert on a field that is hidden or disabled by another (the serial number under "This firearm has no serial number"), or whose saved value the form can't show under the current state (an action the current type doesn't allow): the change shows on the field that hides or limits it (014 FR-005, FR-014).
- Reverting a field that hides, shows or disables others also puts back what it hides and what it emptied (014 FR-012): Type, Origin, Registered as, No serial number, the coverage dialog's Policy, Make automatic backups, Lock after a period without use.

## §2 Save (FR-015, FR-016)

- Forms in scope: `Save` with no changes has `aria-disabled="true"`, the disabled look, stays in the tab order, tooltip and accessible description "No changes to save"; activating it or Enter in a field does nothing.
- Add forms: unchanged (FR-015a).

## §3 Closing with changes (FR-021a)

`ConfirmDialog`, destructive: title "Discard changes?", body "Your unsaved changes will be lost.", buttons **Keep editing** (focused first) and **Discard**. Asked on Cancel, Escape, a click outside and the close button of a form with unsaved changes, of the Settings dialog when a setting differs, and of Mark disposed and Restore (Restore is itself a `ConfirmDialog`, which asks through its `unsavedChanges` prop).

## §4 Folded sections (FR-007)

A `Disclosure` with a changed field inside: its summary ends "· Changed".

## §5 Mounted list (FR-024 to FR-028)

In the firearm and accessory edit forms of an active record, after "Mounted on":

```
Mounted
 Leupold VX-3HD (scope)                  [Remove]
 Trijicon RMR (red dot)  [Changed][↺]  Will be mounted here, moving from Glock 19 with 1 item.
 Aimpoint PRO (red dot)  [Changed][↺]  Will be unmounted.
Add to Mounted
[ Search for an accessory or firearm ]
Nothing changes until you save.
```

- **Add to Mounted** is a search field under the list (the shared `MountChooser`, `role: "item"`), with `hostMountedOn` the form's Mounted on; it leaves out what the list already holds. Its hint: "Nothing changes until you save."
- A saved row has **Remove**; a changed row's revert is named "Revert {name} to “Mounted here”" (a removal) or "to “Not mounted here”" (an addition), and focus then goes to the add field.
- A removed row stays listed, its note saying "Will be unmounted."; it has no Remove, only revert.
- The form's **Mounted on** chooser leaves out what the list adds. A record mounted below one of those additions is still offered; saving that choice is refused as a loop, with the error on `mounted` (FR-026, FR-027).
- Nothing asks; Save stores all.
- Resumed pending changes (FR-020): each added row is named, and its "moving from" host given, as the collection has them now, not as the draft kept them; the Mounted on field names its record as it is now. A row that no longer changes anything (unmounting what is no longer mounted here, mounting what already is) is dropped when the form opens, and one adding a record that is gone or no longer active once the collection has loaded; a Mounted on naming such a record goes back to the saved one. The coverage dialog does the same with a policy that no longer exists. A collection that failed to load drops nothing: until it loads without an error, every record counts as not yet known, not as gone.

Record page Mounted section: the list (links), **Change what's mounted…** (opens the edit form at its Mounted list, focus in Add to Mounted), and the Mount menu with only **New accessory…**. No Unmount, no "Move {name}?" or "Unmount {name}?" questions.
