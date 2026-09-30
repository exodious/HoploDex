# Contract: Identification UI

The application's external interface is its screens. This contract fixes
what the user sees and can do for this feature, so the form, the record
page, the two prompts, the browse control and the import report can be built
and tested against one description. It is guidance-only about origin: the UI
never blocks, rejects or second-guesses a choice of origin (FR-015), and
never displays a conclusion about legal status (FR-006). Shared components
only (Constitution III): `ChoiceCards`, `TextField`, `ConfirmDialog`,
`Dialog`, `Select`, `Field`, `Disclosure`. Requirement IDs refer to [spec.md](./../spec.md).

## 1. Firearm form — origin group and origin control (US1, FR-001, FR-015)

Every field this feature adds to the form sits in the **Identification**
section, after Caliber / Serial number, inside one `Disclosure` titled
**Origin and year of manufacture**. The required identifying marks (make,
model, type, caliber, serial) come first and stay together; these optional
fields, which most records never use, fold away below them.

- The group starts **closed** on a new record and on a record with none of
  its fields recorded, and **open** on a record with any of them recorded.
  The user can open or close it at any time; closing it keeps every value,
  and they are saved as usual.
- Closed, the group's button shows one line under its title. With nothing
  recorded: "Optional: where and when it was made, and who imported it."
  Otherwise it reads the recorded values back, for example "Imported from
  Austria by Glock Inc. Made in 1998. Original maker's marks recorded." So
  closing the group never hides a value.
- Closed, its fields are not rendered, so they are never focusable. The
  group opens itself when a save is stopped by an invalid field inside it
  (the field is then focused, as for any other field) and when the backend
  rejects a field inside it (§4).
- Inside the group, in order: **Year of manufacture** (§2), the origin
  control below, its conditional fields (§2), and the **Original maker's
  marks** group (§2).

The origin control is a `ChoiceCards` radio group labeled **Origin**
(optional, not marked required) with four cards, each with a visible
one-line description (not a tooltip):

| Card | Description shown | Stored value |
|---|---|---|
| Domestic | Made in the U.S. | `domestic` |
| Imported | Made abroad and brought in | `imported` |
| Re-imported | Made in the U.S., exported, then brought back in | `reimported` |
| Not specified | Leave this if you're not sure. | `null` |

- A new record starts on **Not specified**; existing records show their
  stored value, and one with none shows **Not specified** with nothing else
  offered (US1-1, US1-8).

_Amended by [spec 004](../../004-cartridges-action-types/contracts/ui-entry.md): the card, the display and the
group labelled **Not specified** here read **Unspecified**._
- Below the cards, aligned with their left edge, is a button **How do I
  record this?** that opens the guide (§8). It is in the tab order right
  after the origin cards.
- When **Domestic** is selected, a one-line cue appears under the control:
  "Made in the U.S. but stamped with an importer's name? Choose
  Re-imported." (FR-015, US1-7). It is text, not an error, and does not
  disable Save.
- The radio group has a programmatic name ("Origin") and each card's
  description is associated with it, so a screen reader announces the
  description with the option (WCAG 2.1 AA).

## 2. Firearm form — conditional fields (US1, US2, FR-002, FR-004)

Below the origin control, inside the origin group (§1):

| Origin selected | Fields shown | Notes |
|---|---|---|
| Not specified, Domestic | none | Year of manufacture is still shown (below) |
| Imported | **Country of manufacture**, **Importer** (name) | all optional, free text |
| Re-imported | **Importer** (name); read-only line "Country of manufacture: United States" | the country is not an input |

- **Importer** is labeled as a name only. There is no city, state or address
  field anywhere (spec Clarifications).
- For an import-marked origin, a second group titled **Original maker's
  marks** shows **Original maker**, **Original model**, **Original serial
  number**, all optional, with a hint: "Only if the original maker's marks
  differ from the make, model and serial number above, or you want both."
  A partial set is accepted with no message (US2-6). The group is a
  `fieldset` with that legend, set off from the fields above it by a rule
  and titled like a field label rather than a form section.
- **Year of manufacture** is shown for every origin, first in the origin
  group, as a numeric text input (digits only, four characters) whose
  control is a quarter of the form's width, like the form's other short
  values, rather than the column's full width. Optional;
  hint: "A single year, e.g. 1943. Put anything uncertain in Notes." When two
  records with the same make, model and serial need telling apart, the
  identity error (§4) points here.
- Client-side validation mirrors the backend so the message appears on blur
  (same touched-field pattern as the other fields): year outside 1400 to the
  current local year, or not four digits, shows "Year of manufacture must be
  a four-digit year from 1400 to {year}." on that field (US1-5). The backend
  is the authority and its `fieldErrors` are mapped onto the same fields.
- Fields the origin does not offer are removed from the page (not merely
  disabled), so they are never focusable and never submitted.

## 3. Changing origin — discard confirmation (FR-010, US1-6)

When the user selects a new origin and at least one value the new origin
would not offer is non-blank, a `ConfirmDialog` (destructive) opens before
the selection takes effect:

- **Title**: "Discard importer and original marks?" (or "Discard the country
  of manufacture?" for Imported → Re-imported, where only that is lost).
- **Body**: lists what will be discarded, by name and recorded value
  (importer, country, original maker / model / serial), and says they cannot
  be recovered once the record is saved.
- **Actions**: **Discard and change** (confirms: the origin changes and those
  form fields are cleared) and **Cancel** (the origin stays as it was).
- No dialog appears when nothing would be lost, when moving between Imported
  and Re-imported with only importer or original marks recorded (they carry
  over), or when moving from none to another origin.

## 4. Save-time messages

- **Identity clash (FR-007, FR-008, US3-1)**: a `VALIDATION_ERROR` on
  `serialNumber` shown under Serial number, as in 001. The message names the
  existing record and ends with "Or record a year of manufacture on each
  firearm: two firearms with the same marks are accepted when both have a
  year and the years differ." The form marks Year of manufacture as the
  place to act: if the year is empty, it opens the origin group (§1) and
  scrolls the year into view.
- Any other backend `fieldErrors` on a field inside the origin group opens
  the group, so the message is never hidden.
- **Original-marks match (FR-009, US3-5)**: a save that returns
  `ORIGINAL_MARKS_MATCH` opens a `ConfirmDialog` (non-destructive):
  - **Title**: "Another firearm has the same original marks"
  - **Body**: the command's `message` (it names the other record).
  - **Actions**: **Save anyway** (resends with `confirmedWarnings: true`)
    and **Cancel** (returns to the form; nothing is saved).
- Neither message states or implies that a mark, year or import is legal or
  illegal (FR-006).

## 5. Firearm record page (FR-013, US1-3, US2-1, US2-2)

- Under **Identification**, the main marks (make, model, serial number) keep
  their present presentation. Below them, labeled rows appear only for
  values that are recorded: **Origin**, **Year of manufacture**, **Country
  of manufacture** (the United States for Re-imported), **Importer**. A
  record with no origin shows **Origin: Not specified** and no importer or
  country row.
- A separate block titled **Original maker's marks**, visually distinct
  from the main marks and with its own heading, lists **Maker**, **Model**
  and **Serial number** for the values recorded. It renders only when at
  least one of the three is recorded (US2-2). Its heading, not just its
  position, marks it as supplementary, so the two sets cannot be confused
  (SC-002).
- The **List and tile views** (collection page) show only make, model and
  nickname, and are unchanged (FR-013).
- Reversing a disposition (Restore dialog): an identity clash shows the
  same message as §4 in the dialog (as a nickname or make/model/serial
  clash does today); an `ORIGINAL_MARKS_MATCH` shows its message with a
  **Restore anyway** action that resends with `confirmedWarnings: true`
  (US3-9). Nothing changes until the user confirms.

## 6. Browse — group by origin (FR-012, US4-1)

The collection page's **Group by** control gains **Origin** after Make.
Groups appear in the order Domestic, Imported, Re-imported, Not specified
(firearms with no origin). Empty groups are not shown. Search is the
existing search box; it now also finds the new values (§ contracts/
tauri-commands.md, `list_firearms`).

## 7. Import report (FR-009, US4-6)

The import result screen shows, in addition to imported, failed and
conflicting rows, a **Warnings** section when `warnings` is non-empty: one
line per row, "Row {n}: {message}", with the count in the summary tally. A
warning never changes a row's outcome and is styled as information, not as
an error. Nothing is shown when there are none.

## 8. The "How do I record this?" guide (FR-015, SC-007)

A shared `Dialog` titled **How to record where a firearm came from**,
opened from the origin control (§1), closable with Escape and a Close
button, returning focus to the button. It is static text with worked
examples; nothing in it depends on the collection. It states, once, at the
top: "Record what is stamped on the firearm and what your paperwork says.
The app does not check it against any rules." (FR-006).

So the worked examples are not read as rules (an Austrian pistol, the
make "Inland"), the dialog's description introduces them as examples: "Six
examples, each showing what you might see on a firearm and how to record
it. Yours may not match any of them exactly: use the closest as a guide."
The list sits under an **Examples** heading, with each example titled one
level below it.

Each example has a title, "What you see", and "How to record it" (which
origin, which fields go in the main make/model/serial and which in
Original maker's marks). Required examples:

1. **Re-imported M1 Carbine**: a U.S.-made carbine that was exported, later
   brought back, and now bears an importer's stamp. Origin **Re-imported**;
   main make, model and serial are the U.S. maker's; importer name is the
   name in the stamp; country is not asked for (SC-007, US1-7).
2. **Importer adopted the maker's marks**: a pistol made in Austria and
   stamped with its U.S. importer's name. Origin **Imported**, country
   Austria; main model and serial are the maker's; importer name is the
   name in the stamp; no original marks and no importer location (US2-3).
3. **Importer assigned its own serial number**: main make, model and serial
   are the importer's (per the paperwork); the maker's are entered as
   **Original maker's marks** (US2-1).
4. **Older surplus import with only the maker's marks**: main marks are the
   maker's; origin **Imported**; importer optional.
5. **Same serial from two wartime makers**: record the **actual
   manufacturer** as the make (for example "Inland", not the government
   nomenclature) and any suffix such as an added "X" as part of the serial
   exactly as stamped; the make then tells the records apart.
6. **Pre-1968 domestic firearms whose maker restarted numbering**: two
   firearms can share make, model and serial if each has a **year of
   manufacture** and the years differ (FR-008).

Each example names the fields by the labels in §2 so a first-time user can
follow it against the form. Adding or rewording an example is a static-text
change with no data or schema impact.

## Accessibility and consistency checks

- Every new input has a visible label; hints and errors are associated
  with their field; the origin group and the guide dialog are keyboard
  operable and announced correctly.
- Both prompts use `ConfirmDialog` with its existing pending/failed
  behavior; neither invents a new pattern.
- New styles use the design tokens in `src/styles/tokens.css` and follow the
  light and dark themes, with contrast meeting WCAG 2.1 AA.
