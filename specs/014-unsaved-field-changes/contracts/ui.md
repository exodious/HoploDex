# UI contract: feature 014

## §1 The changed marker and revert (FR-006, FR-010, FR-013)

```
Make *   [Changed] [↺]
[ Glock Ges.m.b.H.          ]
Hint text
```

- The tag reads "Changed" in the field label row, after the label (and "*"). Style: small caps-free text tag using the warning-subtle tokens, legible without colour.
- The revert button is icon-only (`rotate-ccw` icon), `Button size="sm" variant="ghost"`. Tooltip: `Revert to “{saved}”`; accessible name: `Revert {label} to “{saved}”`. Saved text: "Not recorded" when empty; longer than 40 characters, cut at 40 with "…".
- The control's `aria-describedby` lists the tag first, then hint, then error.
- Activating revert restores the value and focuses the field's input (or, for choice cards and checkboxes, the selected or first option).
- Fieldsets (Type, Origin, weight, Mounted on, a Mounted list row) put the tag and revert after the legend or row name.

## §2 Save (FR-015, FR-016)

- Forms in scope: `Save` with no changes has `aria-disabled="true"`, the disabled look, stays in the tab order, tooltip and accessible description "No changes to save"; activating it or Enter in a field does nothing.
- Add forms: unchanged (FR-015a).

## §3 Closing with changes (FR-021a)

`ConfirmDialog`, destructive: title "Discard changes?", body "Your changes to {form label} haven't been saved.", buttons **Keep editing** (focused first) and **Discard**. Asked on Cancel, Escape and the close button of a form with unsaved changes, and of the Settings dialog when a setting differs.

## §4 Folded sections (FR-007)

A `Disclosure` with a changed field inside: its summary ends "· Changed".

## §5 Mounted list (FR-024 to FR-028)

In the firearm and accessory edit forms of an active record, after "Mounted on":

```
Mounted                                  [Add…]
 Leupold VX-3HD (scope)                  [Remove]
 Trijicon RMR (red dot)  [Changed][↺]  Will be mounted here, moving from Glock 19 with 1 item.
 Aimpoint PRO (red dot)  [Changed][↺]  Will be unmounted.
```

- Add… opens the existing "Mount on {name}" chooser (`MountChooser`, `role: "item"`), with `hostMountedOn` the form's Mounted on, and leaves out rows already listed.
- A removed row stays listed, its note saying "Will be unmounted."; it has no Remove, only revert.
- Nothing asks; Save stores all.

Record page Mounted section: the list (links), **Change what's mounted…** (opens the edit form at its Mounted list, focus on Add…), and the Mount menu with only **New accessory…**. No Unmount, no "Move {name}?" or "Unmount {name}?" questions.
