# Contract: Accessories and Mounting UI

The application's external interface is its screens. This contract fixes
what the user sees and can do for this feature, so that the Accessories
page, the accessory form and record page, mounting, the Mounted section,
the dispose and delete dialogs, the collection page's mount details and the
export and import dialogs can be built and tested against one description.

**Rules throughout**:
- **Wording** (FR-012, constitution III): the application says
  "accessory", "firearm", "mount", "Mounted on" and "Mounted". It never says
  "item" or "host", which are the spec's terms only. A record of either kind
  is named by `RecordName`: a firearm as firearms are named, with its
  nickname (001 FR-031); an accessory by FR-005.
- **Record only** (FR-011): no screen hints whether a mount suits its
  firearm or accessory.
- **Shared components only**: `Dialog`, `ConfirmDialog`, `Combobox`,
  `EntryField`, `Select`, `MoneyField`, `DateField`, `TextArea`,
  `SegmentedControl`, the `Menu` family with 005's radio items,
  `PhotoGallery`, `DocumentList`, `TypeDrawing`, `InsuranceWarningBadge`.
  Form layouts use `forms.css`'s `hd-field--*` and `hd-form-grid--*`
  classes.

Requirement IDs refer to [spec.md](../spec.md), and commands to
[tauri-commands.md](./tauri-commands.md).

## 1. Navigation (FR-016)

- The tab row reads **Collection · Accessories · Insurance**. The
  Accessories tab opens the Accessories page.
- A link to an accessory or a firearm from any page follows
  `navigation.open`, so **Back** returns to the page it came from (001
  FR-040). That covers links from a Mounted section, a "Mounted on" chain,
  a collection tile, a group heading or a dialog.

## 2. The Accessories page (US1, US4, FR-016 to FR-018)

```text
┌ Accessories ───────────────────────────────────────────────────────────┐
│ [Search accessories…            ]  Group by: Kind ▾   ☐ Show disposed  │
│                                       [List | Tiles]  [Add accessory]  │
│ OPTIC                                                                  │
│  Accessory                      Mounted on               Value  Cover. │
│  Leupold VX-5HD 3-15x44 · Optic Winchester Model 70 “Deer  $1,000  ⚠   │
│                                 rifle”                                 │
│  Aimpoint T-2 · Optic           BCM upper · Upper receiver  $850       │
│ MAGAZINE                                                               │
│  Walther P38 magazines, pair ·  —                         $180         │
│  Magazine                                                              │
└────────────────────────────────────────────────────────────────────────┘
```

- **Layout and state**: the same `SegmentedControl` layout switch, search
  bar, `Menu` grouping control and "Show disposed" checkbox as the
  collection page (001 FR-011, FR-025; constitution III). The state is
  remembered for the session as the collection page's is.
- **Grouping menu**: "Group by" opens a radio menu with **No grouping**,
  **Kind**, **Make**, **Caliber**, **Cartridge** and **Mounted on** (FR-017).
  - Empty groups are "Unspecified", and the unmounted group is "Not
    mounted", always last.
  - Grouped by Mounted on, each heading is the host's `RecordName` as a
    link to its record. Two hosts with the same name are two groups, each
    heading followed by the host's serial number in muted text when it has
    one.
- **List columns**: Accessory (its `RecordName`, linking to its record),
  Mounted on (the direct host's `RecordName` as a link, or "—"), Value,
  and Coverage (the `InsuranceWarningBadge` the collection uses). A
  column whose value is the group heading is left out, as the collection
  page does for type.
- **Tiles**: the thumbnail, or the kind's `TypeDrawing` when the accessory
  has no photo (FR-007a), then its name and value. A mounted accessory
  shows "Mounted on {host}" as a link under its name.
- **Search**: every accessory text field, as the collection's search
  matches (FR-018). No result reads "No accessories match "{query}"."
- **Empty**: with no accessories at all, "No accessories recorded yet."
  and the Add accessory button.
- **Disposed accessories** appear only with "Show disposed", marked as on
  the collection page.

## 3. The accessory form (US1, FR-001 to FR-005, FR-012, FR-027)

A `Dialog` with the large form layout, as `FirearmForm`'s add and edit:

| Row | Fields | Notes |
|---|---|---|
| 1 | **Kind** (`Select`, required) | Offered kinds in list order. A saved record whose kind is no longer offered keeps it, shown as the selected option. The hint reads "A suppressor is recorded as a firearm." (FR-002) |
| 2 | **Make**, **Model** (`EntryField`s) | 004's suggestions, from firearms and accessories together (FR-003) |
| 3 | **Cartridge**, **Caliber** (`EntryField`s) | 004's row: a blank caliber is filled from the cartridge, with its hints |
| 4 | **Serial number** (`TextField`, `hd-field--third`) | No "no serial number" box (FR-004) |
| 5 | **Mounted on** (`MountChooser`, §4) | Optional. The hint reads "The firearm or accessory it is on now, if any." |
| 6 | **Estimated value** (`MoneyField`, `hd-field--quarter`) | The hint reads "The value of this record as a whole, everything it describes included. Value each record on its own." (FR-005) |
| 7 | **Acquired**: source (`TextField`), date (`DateField`), price (`MoneyField`) | As the firearm form's Acquisition group |
| 8 | **Notes** (`TextArea`) | |

- **Titles**: "Add accessory" and "Edit {name}". The buttons are **Save**
  and **Cancel**.
- **Opened from a record page's "Mount → New accessory…"** (§5), the title
  is still "Add accessory", and Mounted on is preset to that record and
  can be changed.
- **There is no quantity field** anywhere (US1-4).
- **Errors** appear on the fields, as on the firearm form. A save with no
  kind focuses Kind: "Choose a kind."
- **Unsaved changes**: closing with changes asks to save, discard or
  cancel, and a lock keeps them as pending changes labelled with the
  accessory's name, as the firearm form does (FR-027).

## 4. Choosing what something is mounted on (FR-010, FR-012)

`MountChooser` is a `Combobox` field labelled **Mounted on**. It is used on
the accessory form (§3) and on the firearm form, where it is placed after
Type and Action, with the hint "Only if this firearm is mounted on another
firearm or an accessory, such as a suppressor on a rifle."

- **Empty**, it reads "Not mounted". Typing searches
  `list_mount_candidates` (role `host`) by make, model, nickname or serial
  number, debounced as `EntryField` is.
- **Each option** shows the `RecordName`, then muted: the type or kind and
  the serial number. When the candidate is itself mounted, a second muted
  line reads "Mounted on {its host}".
- **Clearing** is the **×** button, or choosing "Not mounted" at the top of
  the list.
- **The record itself, and everything mounted on it, are never offered**
  (US2-6, US2-16). Choosing a record that is already mounted elsewhere
  needs no confirmation here (FR-012).
- **Keyboard**: the `Combobox`'s keys (↑ ↓ Enter Esc). The listbox has
  `aria-label="Mounted on"`.

## 5. The Mounted section (record pages; FR-012, FR-013)

The Mounted section appears on every **active** firearm's and accessory's
record page. On a firearm's page it sits in the main column after Notes and
before the free-text **Accessories** subsection (FR-007), and the
Accessories subsection is unchanged. A disposed record has no Mounted
section.

```text
┌ Mounted ─────────────────────────────────────────────── [Mount ▾] ┐
│ BCM upper · Upper receiver                              [Unmount] │
│    Leupold Mark 5HD 5-25x56 · Optic                               │
│    on BCM upper                                                   │
│    Trijicon RMR · Optic                                           │
│    on Leupold Mark 5HD 5-25x56                                    │
│    SureFire M600 · Light or laser                                 │
│    on BCM upper                                                   │
│ Gemtech GM-45 “Quiet one” · Suppressor                  [Unmount] │
└───────────────────────────────────────────────────────────────────┘
```

- **One flat list**, depth-first (research.md §14):
  - **Records mounted directly on this one** start at the list's edge, each
    with an **Unmount** button.
  - **Everything mounted further down** follows its direct ancestor, one
    fixed indent in, whatever its depth. Under its name, a muted line reads
    "on {the record it is mounted on}".
  - Every name is a link to its record.
  - A firearm's entry reads "{make} {model} “{nickname}” · {type}". An
    accessory's reads by FR-005.
- **Structure**: a `<ul>`, with each entry an `<li>`. A deeper entry's "on
  …" line is part of the link's accessible description
  (`aria-describedby`), so a screen reader hears "Trijicon RMR · Optic, on
  Leupold Mark 5HD 5-25x56".
- **Nothing mounted** reads "Nothing mounted." beside the Mount control
  (US2-10).
- **Mount ▾** opens a `Menu` with:
  - **New accessory…**: the accessory form, with Mounted on preset (§3).
  - **Existing accessory or firearm…**: a `Dialog` titled "Mount on
    {name}". It holds a `Combobox` that searches `list_mount_candidates`
    (role `item`), with options shown as in §4.
    - Choosing an unmounted record mounts it at once.
    - Choosing one listed with "Mounted on X" first asks a
      `ConfirmDialog`: "Move {name}?", with the body "It is mounted on {X}.
      Moving it takes everything mounted on it along." and the buttons
      **Move** and **Cancel**. Cancel leaves it where it is (US2-3a).
- **Unmount** asks nothing: the change is easily undone. It shows the
  standard toast "{name} unmounted."
- **Mounting or moving** shows the toast "{name} mounted on {this
  record}."

## 6. "Mounted on" on a record page (FR-013)

- **Where**: in the record page's header facts (the plate under the serial
  number), on a firearm's record page and an accessory's alike.
- **What it shows**: **Mounted on** followed by the chain, each record a
  link:
  - "Mounted on BCM upper · Upper receiver, on LaRue PredatAR · Rifle"
    (US2-12);
  - one host alone: "Mounted on {host}".
- **When it is absent**: when the record is not mounted.
- **Changing it**: the record's Edit form (§3, §4), or its host's Mounted
  section.

## 7. The dispose dialog with mounted records (FR-014, US3)

The existing `DisposeDialog` serves both kinds of record. Its shared fields
(type, recipient, date, the record's own price) are unchanged.

- **When the record is mounted**, a note sits under the title: "{name} will
  be unmounted from {host}." (US3-3).
- **When anything is mounted on it**, a **Mounted** group follows the
  record's own price, with the same flat list as §5. Each row has:
  - a `SegmentedControl` reading **Keep | Dispose with it**, with **Keep**
    selected;
  - when **Dispose with it** is chosen, a `MoneyField` labelled "Price for
    {name}" (`hd-field--third`, as the standard dialog's "Price
    received"), optional, with the hint "Leave blank if none was received
    separately."
- **What happens to kept records** is stated under the list, worked out
  from the choices:
  - "Kept records mounted on {name} will be unmounted."
  - When a kept record stays on a kept record: "Records kept with what
    they are mounted on stay mounted."
- **Confirm** submits one `dispose_*` call with `withMounted`. If the
  backend reports that what is mounted has changed, the dialog shows that
  error and reloads the list.
- **Unsaved changes**: the dialog's choices and prices are part of its
  values, so they are kept as pending changes and resumed (FR-027).
- **Keyboard**: each row's `SegmentedControl` is one tab stop, changed
  with arrow keys. Tab order runs down the list.

## 8. Delete confirmation (FR-015, US3-5, US3-9)

The standard delete `ConfirmDialog` gains, when anything is mounted
directly on the record: "{n} records mounted on it will stay in the
collection, unmounted:", followed by their names as a list. Records mounted
further down are not listed, because they stay where they are. The record's
own deletion wording is unchanged.

## 9. The collection page's mount details (FR-016a)

- **A mounted firearm**:
  - **List**: "Mounted on {host's RecordName}" as a muted line under the
    firearm's name, with the host as a link.
  - **Tiles**: the same line under the name.
- **A firearm with records mounted on it**: "{n} mounted" in the same
  place, counting everything below at any depth, not linked, not naming
  them (US2-11).
- **A firearm that is both mounted and carrying records** shows both
  lines: "Mounted on …" first.
- **A firearm that is neither** shows neither.
- **Search and grouping are unchanged** (FR-018).

## 10. Value summary and insurance

- **Value summary**: a third line under the collection total, "Accessories
  {accessoriesTotal}", beside the firearms subtotal (FR-008). The blanket
  policy's line counts "{n} firearms and {m} accessories".
- **Policy cards** list scheduled accessories with the scheduled firearms,
  each by `RecordName`. The coverage dialog serves accessories as it serves
  firearms (FR-009).
- **The accessory record page** has the same Coverage side panel as a
  firearm's.

## 11. Export and import dialogs (FR-020 to FR-022)

- **The export dialog's counts** read "{n} firearms and {m} accessories"
  from `get_export_scope`. With the filtered scope, a hint reads
  "Includes everything mounted on these firearms."
- **The disclosure** gains "accessories, with their serial numbers, values
  and photos" whenever `includesAccessories` is true (FR-021). It is worded
  in the same sentence as 005's registration disclosure.
- **CSV exports** with accessories report both file names in the result.
- **The import dialog's file picker** allows one or two files ("Choose one
  file, or the firearm and accessory files together"). The chosen files are
  listed. Picking a third replaces the selection.
- **The import report** names each row's table ("Accessories, row 4") and
  lists mount warnings with the other warnings. The conflict list shows an
  accessory conflict by its `RecordName`.
- **Replacing a record with a disposed row** (issue #56): the Replace
  confirmation adds, for each row being replaced whose conflict has a
  `mounted` list, a group titled "Mounted on {name} ({Table}, row {n})"
  with §7's list and choices (Keep | Dispose with it, Keep by default) and
  statements, but no price fields. A note above the groups says the
  records disposed with it take the row's type, recipient and date, with
  no price. Confirming sends each row's chosen records as `withMounted`.

## 12. Accessory record page (US1, FR-007a, FR-013)

`AccessoryRecordPage` has the same layout as `FirearmRecordPage`:
- **Bar**: Back, the accessory's name, and its actions (Edit, Mark disposed
  or Restore, Delete).
- **Main column**:
  - the photo gallery;
  - **Details**: kind, make, model, serial number, caliber and cartridge
    (the "Mounted on" chain is in the header facts, §6);
  - **Value and acquisition**;
  - **Notes**;
  - **Mounted** (§5);
  - **Documents**;
  - **Disposition history**.
- **Side column**: Coverage.

Drop targets for photos and documents work as on a firearm's record
(FR-007a).

## 13. Screens walk

Add to `e2e/screenshots/screens.e2e.ts`, continuing the numbering, in light
and dark:
- the Accessories page as a list, grouped by kind;
- the same as tiles, with a generic drawing and a photo thumbnail;
- grouped by Mounted on;
- the accessory form (add), and the same at minimum width;
- an accessory record page with its "Mounted on" chain;
- a firearm record page with a nested Mounted section;
- the Mount menu open;
- the "Existing accessory or firearm…" dialog with a mounted candidate;
- the move confirmation;
- the dispose dialog with nested mounted records, one set to "Dispose
  with it";
- the delete confirmation naming mounted records;
- the collection list and tiles with "Mounted on" and "3 mounted";
- the value summary with the accessories subtotal;
- the export dialog with the accessories disclosure;
- the import report with a mount warning;
- the import's Replace confirmation for a row that disposes of a record
  with records mounted on it, one set to "Dispose with it" (issue #56).
