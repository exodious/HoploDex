# Contract: Entry Suggestions, Cartridge and Action UI

The application's external interface is its screens. This contract fixes
what the user sees and can do for this feature, so the suggestion list, the
form's cartridge, caliber and action fields, the browse views, the record
page and the import report can be built and tested against one description.
Its rule throughout is the recorded decision's "guide, never restrict": a
suggestion is only applied when the user picks it, every value within the
entry rules can be kept as typed, and every automatic change (a snap, a
derived caliber, a cleared action) is shown where the user is looking before
anything is saved. Shared components only (constitution III): the new
`Combobox`, plus `TextField`, `Select`, `Field`, `Button`, `Toast`.
Requirement IDs refer to [spec.md](../spec.md); commands to
[tauri-commands.md](./tauri-commands.md).

## 1. The suggestion list (`Combobox`, US2, FR-009, FR-012, FR-016)

Used, identically, by **Make**, **Model**, **Cartridge** and **Caliber** on the
firearm form. Nowhere else in this feature (original marks are left alone,
spec Assumptions).

**Structure (WAI-ARIA "combobox with listbox popup", list autocomplete)**:

- The field is a text input with `role="combobox"`, `aria-autocomplete="list"`,
  `aria-expanded`, `aria-controls` (the list) and `aria-activedescendant` (the
  highlighted option). It keeps the field's visible label, hint and error, as
  `TextField` does, and focus never leaves it.
- The list is a `role="listbox"` directly below the field, as wide as the
  field, showing up to **8 rows** and scrolling for the rest (at most 20 come
  back). It is rendered in a popover layer so the form dialog's scrolling never
  clips it.
- Each row (`role="option"`) shows the value, and at the row's end, in the
  muted secondary text style:
  - **Built-in** for a catalog entry, and for a catalog cartridge its caliber:
    "Built-in · 9mm";
  - **"N in collection"** for a value on record (`useCount`);
  - both, "Built-in · 9mm · 2 in collection", for a value that is both.
  The marker is part of the option's accessible name, so it is announced.
- The part of the value that matched the typed text is not highlighted (a
  prefix, word or initialism match would highlight inconsistently).

**When it opens and what it shows**:

- It opens when the field gains focus by a click or a Tab from another control
  (showing the ranked list for the current text, even when empty) and on every
  keystroke. Focus a dialog places in the field as it opens does not open it,
  so Escape still closes the dialog; the first key or Down brings the list up. Each keystroke asks
  `suggest_entries`; a response for anything but the field's current text is
  dropped.
- For **Model**, the request carries the make currently on the form.
- With no matches it closes. There is never an "Add …" row: typing on is how
  a new value is entered.

**Keyboard** (FR-016, US2-10):

| Key | Effect |
|---|---|
| Down / Up | Open the list if closed; move the highlight (wraps at the ends). The field's text does not change while moving |
| Enter | With a row highlighted: pick it (the field takes the value, the list closes, focus stays in the field). With none: the list closes and the form's normal Enter behavior applies (§2, submit) |
| Escape | With the list open: close it and keep the typed text. With it closed: the dialog's own Escape behavior |
| Tab / Shift+Tab | Close the list, keep the typed text, move focus as usual (this is "leaving the field", §2) |
| Home / End | Move the text caret, as in any text field |

**Pointer**: clicking a row picks it. Clicking outside closes the list and
keeps the text.

**Announcements**: the number of suggestions is announced politely after
typing pauses ("6 suggestions"), through the list's live status region, not
on every keystroke.

## 2. Snapping on the form (US2-5, US2-6, FR-013, FR-014)

- When the user **leaves** one of the four fields, or **picks** a suggestion,
  the form calls `settle_entry` if the field's text has changed since it was
  last settled, and, when editing a saved firearm, differs from the saved
  value. An untouched field, or one typed back to its saved value, is never
  settled (FR-014).
- If `changedBy` is set, the field takes `value`, and a note appears under it
  (as a hint, not an error), replacing any hint there:
  - `catalog`: "Changed to the built-in spelling “9x19mm Parabellum”."
  - `record`: "Changed to “Smith & Wesson”, as already in your collection."
  The note is announced politely and stays until the field is edited again.
- A response is applied only if the field still holds the text that was sent
  (the user may already be typing something else).
- **Saving**: Save first waits for any settle in flight, then settles the
  field that has focus. If that changes its value, the change is shown (the
  note above) and the form **does not save**; the user reviews and saves
  again. This happens only when Enter is pressed inside a field.

## 3. Cartridge and caliber (US1, FR-003 to FR-006)

**Layout**: in the **Identification** section, after **Type** and **Action**
(§4), one `hd-form-grid--2` row: **Cartridge** (left, optional) then
**Caliber** (right, required), so a derived caliber fills in to the right of
what was typed. **Serial number** and its checkbox follow on their own row at
half width (`hd-field--half`, as Nickname).

- **Cartridge**: label "Cartridge", optional; hint "Optional. The exact round
  it's chambered for, e.g. 9x19mm Parabellum." Placeholder "e.g. 9x19mm
  Parabellum".
- **Caliber**: label "Caliber", required; placeholder "e.g. 9mm"; its hint
  changes with its state (below).

**Caliber states** (research.md §8). On a **new firearm** the caliber starts
*derived*; editing a **saved firearm** it starts *edited*.

| Event | Caliber *derived* | Caliber *edited* |
|---|---|---|
| Cartridge settles to a catalog cartridge or alias | Takes the catalog caliber; hint "From the cartridge." | Unchanged. If the derived caliber differs: the suggestion line below |
| Cartridge settles to a name with a readable bore | Takes the guess; the field shows a **Guess** tag at its end and the hint "Guessed from the cartridge. Check it before saving." | As above, with the guess |
| Cartridge settles to a name with no readable bore (US1-4) | Emptied; hint "We couldn't work out a caliber from “Wildcat Special”. Enter it." | Unchanged; no suggestion |
| Cartridge cleared | Emptied, no hint | Unchanged; no suggestion |
| User types in Caliber | Becomes *edited*; tag and hint removed | — |
| User leaves Caliber empty | Stays *derived*; if a cartridge is entered, re-derived at once | Becomes *derived*, then as the left column |

- **Suggestion line** (*edited* mode, US1-7): under Caliber, "The cartridge
  suggests “.45”." followed by a secondary button **Use .45**. Pressing it sets
  the caliber and removes the line; the caliber stays *edited*. The line is
  removed when the cartridge changes again or the caliber is edited to the
  suggested value.
- The **Guess** tag is text, with the hint as its accessible description; it
  is not conveyed by color alone.
- An empty caliber on save is the existing required-field error, "Enter the
  caliber."

## 4. Action (US3, FR-017 to FR-019)

- The field is the spec's *action type* (FR-017); the form, the browse
  column, the Group by control and the record page label it **Action**, and
  the spreadsheet column is `action_type` (FR-022).
- A `Select` labeled **Action**, optional, directly after **Type**, at a third
  of the form's width (`hd-field--third`, as other short choices). Its first
  option is **Unspecified**; then the actions allowed for the selected type,
  in list order (the whole list when the type maps none, or no type is chosen
  yet). The choices come from `list_action_types`. It cannot take typed text
  (US3-7).
- **Changing Type** when the chosen action is not allowed for the new type:
  the action returns to **Unspecified**, and a note appears under Action:
  "Pump action doesn't apply to a Handgun, so the action was cleared." It is
  announced politely once per type change, and removed when an action is
  chosen or the type changes again. An allowed action is kept, with no note.
- The backend's `actionTypeId` field error, if it ever arrives, shows under
  Action like any field error.

## 5. The rest of the form

- **Entry rules** (FR-015), checked when the field is left and on save, with
  the backend as the authority: "Make can be at most 100 characters." / "Make
  can't contain control characters." (and likewise Model, Cartridge,
  Caliber). The fields do not truncate what is typed or pasted.
- **Carry to the other forms** (CLAUDE.md, UI consistency): no other form has
  make, model, cartridge, caliber or action fields; the `Combobox` is the
  shared primitive for any later free-text field with suggestions.

## 6. Browse (FR-027, US1-6, US1-10, US3-4, US3-8)

- **Group by** gains **Cartridge** and **Action**, after **Caliber** and **Type**
  in the control's order: Type, Action, Caliber, Cartridge, Make, Origin.
  Groups of firearms with none are headed **Unspecified**, last.
- **One term** (research.md §11): origin's group, its choice card on the form
  (002 ui-identification §1, "Unspecified — Leave this if you're not sure.")
  and its record-page value change from "Not specified" to **Unspecified**, so
  the application names an unrecorded value the same way everywhere.
- **List view**: the **Caliber** column shows "9x19mm Parabellum (9mm)" when a
  cartridge is recorded, else "9mm". A new **Action** column follows **Type**,
  blank when none. The Caliber column is hidden when grouped by Caliber or
  Cartridge; the Action column when grouped by Action; Type as today.
- **Tile view**: the caliber line shows the same text as the list's Caliber
  cell. Tiles do not show the action.
- A long cartridge is truncated with an ellipsis in the cell, the full text in
  the row's accessible name.

## 7. Record page (FR-027)

In the title block beside **Caliber** (`TitleCell`s): **Cartridge** before
Caliber, and **Action** after them. A cartridge or action that is not
recorded is shown as the page shows other unrecorded optional values.

## 8. Import report (US4, FR-025, FR-026, SC-007)

The import dialog's result gains two sections after the counts and before row
errors, each shown only when non-empty and each a disclosure listing rows in
row order:

- **Calibers filled in from the cartridge (N)**: "Row 4: 9x19mm Parabellum →
  9mm (built-in)" / "Row 7: .30 Custom Improved → .30 (guessed)". A guessed row
  is marked by the word "guessed", not only by style.
- **Spellings matched to existing values (N)**: "Row 2, make: “springfield
  armory” → “Springfield Armory”".

Row errors, conflicts and 002's warnings are unchanged.

## 9. Screens walk

`e2e/screenshots/screens.e2e.ts` adds: the firearm form with the Make list
open showing record and built-in markers; the Cartridge list open for "9mm";
Caliber with the **Guess** tag; the edited-mode suggestion line; the cleared
action note; the list grouped by Cartridge; and the import report with both
new sections.
