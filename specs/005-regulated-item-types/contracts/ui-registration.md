# Contract: Suppressor, Registration and Grouping UI

The application's external interface is its screens. This contract fixes
what the user sees and can do for this feature, so the form's type-dependent
fields, the Registration section, the record page, the guide, the export
note and the collection page's grouping control can be built and tested
against one description. The rule throughout is the spec's **record only**
(FR-014): no screen says, hints or implies whether an item is regulated,
needs registering or is lawful, and nothing prompts for a classification.
Only shared components are used (constitution III): `Select`, `EntryField`
(004's `Combobox`), `DateField`, `Disclosure`, `ConfirmDialog`, `Dialog`,
and the `Menu` family, which gains radio items (§6). Requirement IDs refer
to [spec.md](../spec.md), and commands to
[tauri-commands.md](./tauri-commands.md).

## 1. Type-dependent fields on the firearm form (US1, FR-002 to FR-004)

- **Type** stays the existing `ChoiceCards`, built from `list_firearm_types`
  in list order: Handgun, Rifle, Shotgun, **Suppressor**, Other. Each card shows
  its cropped `TypeDrawing`, the suppressor's included. With five cards at
  `minCardWidth` 140, the row wraps at narrow dialog widths, as
  `ChoiceCards` already allows. The screens walk checks both the wide and
  the minimum width.
- **When the type is Suppressor**:
  - **Action** is not rendered.
  - In the folded **Physical details** section, **Barrel length** and
    **Capacity** are not rendered. Overall length, weight, finish and
    condition stay. The closed section's summary omits the hidden fields.
  - **Caliber** is labelled **Caliber rating**, with the hint "The largest
    bore the suppressor is rated for." The Cartridge|Caliber row and 004's
    derivation are otherwise unchanged (US1-7).
- **Changing type to one that omits a recorded field** (US1-5, FR-004): a
  note appears under the Type select, in the same hint style as 004's
  cleared-action note: "A Suppressor has no action, barrel length or
  capacity, so Bolt action, 20 in and 5 rounds will be cleared when you
  save." It names only the fields that hold a value, and it disappears if
  the type changes back. The values stay in the form until save. The input
  sent at save leaves out whatever the type in effect omits. A pending
  draft keeps them.
- **Changing to a type where they apply**: the fields are offered again,
  holding whatever the form still has (empty for a saved Suppressor).
- The note is announced through the form's existing polite live region, once
  per type change that clears something.

## 2. The Registration section (US2, FR-007, FR-009, FR-012, FR-014)

A folded section titled **Registration**, placed after "Origin and year of
manufacture" and before "Physical details". It follows the same rules:
closed unless something is recorded, opened by a save error inside it.

**Closed summary** (one line, as the other folded sections):

- Nothing recorded: "Optional: what the firearm is registered as, and the
  approval."
- Otherwise, the recorded parts in this order: "Registered as Suppressor.
  Form 4, approved Feb 10, 2026. Registered to Smith Family Trust." Missing
  parts are left out, and the sentences are joined as in the origin
  summary.

**Open**, top to bottom:

1. **The standing note** (`role="note"`, the muted note style used by the
   origin section): "HoploDex records what you enter here. It doesn't decide
   what is regulated or needs registering, and the law changes." It is
   followed by a link-style button, **How to record registrations**, which
   opens the guide at its "Registered items" part (§7).
2. **Registered as**: a `Select`. The first option is **Unspecified**, the
   application's one term for an unrecorded value (004 research §11). Then
   come the offered classifications in list order, plus the record's own
   classification if it is no longer offered, in its list position. Any
   choice is always allowed.
3. Shown only once a classification is chosen, in one
   `hd-form-grid hd-form-grid--registration` row at the standard dialog
   widths:
   - **Form**: `EntryField` (`registrationForm`), `hd-field--third`, with
     suggestions marked "Built-in" for the three built-in form names
     (Form 1, Form 4 and Form 5).
   - **Approved**: `DateField`, `hd-field--third`, with the hint "The date on
     the approved form (the tax stamp date)." Its `max` is today's local
     date, so the calendar can't pick a future date. A typed one gets the
     backend's message on save (FR-010).
   - **Registered to**: `EntryField` (`registeredTo`), full row below, with
     the hint "A person, trust or company, as named on the form."
   All three are optional, and none is marked required.

**Clearing the classification** (FR-012, US2-7): choosing **Unspecified**
while Form, Approved or Registered to has a value opens `ConfirmDialog`:

- Title: "Discard the registration details?"
- Body: "Clearing what the firearm is registered as will discard the form
  "Form 4", the approved date and Registered to "Smith Family Trust". They
  can't be recovered once saved." It names only the parts that have a value.
- Buttons: **Discard details** (destructive style) and **Keep them**.
  Cancelling leaves the classification as it was.

Choosing another classification keeps the details, with no question.
Choosing Unspecified with no details recorded clears it at once.

**Snapping**: Form and Registered to settle on leaving the field, and show
004's snap note under the field ("Changed to the spelling on record "Smith
Family Trust"").

## 3. What the form never shows (FR-014, US2-8, SC-002)

No message, badge, colour, icon or empty-state text anywhere on the form, the
record page, the browse views, the import report or the export dialog may:

- refer to barrel or overall length in relation to registration;
- suggest, preselect or prompt for a classification from the type, action or
  any other value, or the reverse;
- describe a record as pending, approved, unregistered, compliant or needing
  anything.

`FirearmForm.test.tsx` and `FirearmRecordPage.test.tsx` check this for a Rifle
with a 10.5 in barrel and no classification, a Suppressor with none, and a
Rifle registered as Machine gun with a Semi-automatic action. They search the
rendered text for "regulat", "register" (outside the Registration section's
own labels), "NFA", "pending" and "required".

## 4. The record page (FR-002, FR-018)

- **Caliber rating** replaces the Caliber label in the title block for a
  Suppressor.
- **Action**, **Barrel length** and **Capacity** are not listed for a
  Suppressor.
- A **Registration** panel (`hd-panel`, after "Original maker's marks" and
  before "Physical details") appears only when a classification is recorded.
  It is a definition list:
  - **Registered as**: Suppressor
  - **Form**: Form 4
  - **Approved**: Feb 10, 2026 (`formatDate`)
  - **Registered to**: Smith Family Trust

  Rows with no value are omitted. There is no status row (FR-011) and no
  derived text. The panel's Edit link opens the form on the Registration
  section, as the other panels' links open theirs.

## 5. Export note (FR-020, US4-2)

The privacy note's last sentence becomes, when any firearm in the chosen
scope has a classification:

> Anyone who can open that folder can read these records, including serial
> numbers, values and registration details.

Otherwise it is unchanged ("…including serial numbers and values."). The
dialog recomputes it when the scope changes between "all" and "filtered"
(research.md §10).

## 6. Grouping control on the collection page (FR-016)

Designed with the frontend design skill (research.md §9). It replaces the
"Group by" `SegmentedControl`. The List/Tiles control stays a segmented
control.

**Trigger** (in the toolbar, where the segmented control was):

```text
┌──────────────────────────────┐
│ Group by  Caliber          ▾ │   ← Button, size sm, height --control-height
└──────────────────────────────┘
   └ --ink-2    └ --ink, 650
```

- The accessible name is "Group by, Caliber". `aria-haspopup="menu"`.
- It reads "Group by  None" when the list is ungrouped.
- Its width follows its text, so the toolbar reflows as it does today.

**Menu** (the shared `Menu`, aligned to the trigger's start, `hd-menu`
styles):

```text
┌────────────────────────────┐
│ ●  None                    │
├────────────────────────────┤
│ The firearm                │  ← MenuLabel: --text-sm, --ink-2, 650, sentence case
│ ○  Type                    │
│ ○  Action                  │
│ ◉  Caliber                 │  ← chosen: Niter dot in the leading gutter
│ ○  Cartridge               │
│ Its maker                  │
│ ○  Make                    │
│ ○  Origin                  │
│ Registration               │
│ ○  Registered as           │
│ ○  Registered to           │
└────────────────────────────┘
```

- **Roles**: the menu is `role="menu"`. Each section is a `MenuRadioGroup`
  (`role="group"`, labelled by its `MenuLabel`). None sits in a group of its
  own with no label. Items are `role="menuitemradio"` with `aria-checked`.
  One value is shared across the groups, so exactly one item is checked.
- **The indicator** is drawn in the leading gutter for every item: an empty
  ring (`--rule-strong`) for unchecked items and a Niter-filled dot for the
  checked one. That keeps the labels aligned whichever is chosen. Items use
  the existing `hd-menu__item` height and highlight (`--niter-wash`).
- **Keys**: Enter, Space or Down on the trigger opens the menu with focus on
  the checked item. Up and Down move through all items across sections.
  Home and End go to the first and last items. Typing a letter jumps to the
  next item starting with it (Radix typeahead). Enter or Space chooses an
  item and closes the menu. Escape closes it and returns focus to the
  trigger, choosing nothing.
- **Choosing** updates the browse state at once, and the trigger's value
  changes to the new label. The list regroups under the existing busy
  indicator. There is no other animation beyond the menu's standard open and
  close. `prefers-reduced-motion` removes that too, as `hd-menu` already
  does.
- **Section copy** is fixed: "The firearm", "Its maker", "Registration".
  The item labels are `GROUP_BY_OPTIONS`' existing labels plus "Registered
  as" and "Registered to". `GROUP_BY_OPTIONS` gains a `section` field so the
  menu is built from it.
- **Narrow windows**: the menu is a popover, so it never wraps. At the
  window's minimum width it opens below the trigger and flips above when
  there is no room, as other `Menu`s do.

**Shared components** (`src/components/Menu.tsx`, exported from `index.ts`):
`MenuRadioGroup`, `MenuRadioItem` and `MenuLabel`, over Radix's
`DropdownMenu.RadioGroup`, `RadioItem` + `ItemIndicator` and `Label`.
`Menu.test.tsx` covers roles, the checked state and keys.

## 7. The guide (FR-014, US2-13)

`IdentificationGuide` (formerly `OriginGuide`, research.md §12) is one
`Dialog`:

- **Title**: "How to record where a firearm came from and how it's
  registered".
- **Disclaimer**: "Record what is stamped on the firearm and what your
  paperwork says. HoploDex doesn't check it against any rules or decide what
  is regulated."
- **Part 1, "Where it came from"**: 002's six examples, unchanged.
- **Part 2, "Registered items"**: the two examples of research.md §12, in the
  same "What you have" / "How to record it" pairs. The first label is "What
  you have" rather than "What you see", because paperwork is what the owner
  has.
- **Opening**: the origin section's trigger opens it at the top. The
  Registration section's link opens it with Part 2's heading scrolled into
  view and focused (`placeFocus`).

## 8. Browse (FR-001, FR-016, FR-018)

- A Suppressor with no photo shows the `suppressor` drawing in the list's
  thumbnail cell and on its tile, and in the record page's thumbnail.
- The list and tiles show no registration detail (FR-018). The Action column
  shows nothing for a Suppressor, as for any firearm with no action.
- Grouped by **Registered as** or **Registered to**, group headers read as the
  other groups' do, with "Unspecified" last.

## 9. Import report (FR-021)

- 004's "Values changed to a spelling already in use" section lists snapped
  forms and "Registered to" values with the field names **Form** and
  **Registered to**.
- Row errors appear in the existing row-error list with the messages in
  [spreadsheet-format.md](./spreadsheet-format.md).

## 10. Screens walk

`e2e/screenshots/screens.e2e.ts` adds: the form with type Suppressor
(Caliber rating, no Action, Physical details open without barrel length or
capacity); the type-change clearing note; the Registration section closed
with a summary, open empty with only "Registered as", and open with all
details and the Form list showing built-in names; the clear-classification
confirmation; the record page's Registration panel; the guide at "Registered
items"; the grouping menu open; the list grouped by Registered as; the
suppressor drawing in tiles; and the export dialog with the registration
note.
