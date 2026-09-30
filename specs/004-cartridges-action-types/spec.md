# Feature Specification: Cartridges, Action Types & Entry Suggestions

**Feature Branch**: `004-cartridges-action-types`

**Created**: 2026-09-29

**Status**: Draft

**Input**: User description: GitHub issue #11, "Cartridges, action types and suggestion picklists for make, model and caliber". It gathers four requests: record the specific cartridge a firearm is chambered in and derive its caliber from it; make caliber and cartridge entry a narrowing picklist backed by a built-in list of common cartridges; record a firearm's action (semi-automatic, bolt action, and so on); and suggest previously entered values while typing make, model, caliber and cartridge. The requests and the decisions already recorded on the issue are reproduced under [Source Request](#source-request) so this spec stands on its own.

## Clarifications

### Session 2026-09-29

- Q: Should full-auto or select-fire be recorded in this feature, and how? → A: Not in this feature. The action list describes how a firearm cycles, not its fire-control capability; machine guns are a candidate type for the regulated item types feature (#12), which extends the type→action mapping (FR-018).
- Q: Which caliber does a catalog cartridge get when its name gives a finer designation than its bore class (e.g. `.308 Winchester`, `.300 AAC Blackout`, `.380 ACP`)? → A: The coarser bore class, for broader groups: every .30-class inch cartridge (.308, .300 BLK, .30-06, .30-30) is ".30", and `.380 ACP` is "9mm". A custom cartridge's guess maps its designation to that same class (FR-005).
- Q: When editing a saved firearm and changing its cartridge, is the saved caliber re-derived? → A: No. A saved caliber counts as already edited and is left alone; the form offers the newly derived caliber as a suggestion the user can accept with one action (FR-006).
- Q: On import, are same-notation variants that exist only within the sheet snapped to each other? → A: Yes, by majority: the spelling used by the most rows wins, and on a tie the one in the earliest row; catalog spellings and values on record still take precedence (FR-026).
- Q: Where do cartridge and action type appear when browsing the collection? → A: The cartridge joins the caliber in the list's caliber cell and on tiles ("9x19mm Parabellum (9mm)", or the caliber alone when there is no cartridge); action type gets its own list column, not on tiles; a column is hidden while the list is grouped by it (FR-027).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Record the Exact Cartridge, With the Caliber Filled In (Priority: P1)

A collector adds a pistol chambered in 9x19mm Parabellum. Instead of typing "9mm" into a caliber box, which would say nothing about whether it is 9x19, 9x18 Makarov or .380 ACP, they start typing the cartridge, pick "9x19mm Parabellum" from the list, and see the caliber field filled in as "9mm". For a cartridge the app does not know, such as a wildcat, they type its name and the app makes a visible guess at the caliber that they can correct before saving.

**Why this priority**: The ambiguity of "caliber" is the core problem the requests describe. Recording the cartridge, while keeping a caliber on every record for grouping, is the part of the feature that changes what a record can say; the suggestion mechanics in the other stories make it faster and more consistent but depend on the cartridge existing as a field.

**Independent Test**: Create firearms with a built-in cartridge, with a custom cartridge whose caliber can be guessed, with a custom cartridge whose caliber cannot be guessed, and with a caliber only. Confirm each saves with the expected cartridge and caliber, reopens intact, and groups correctly by caliber.

**Acceptance Scenarios**:

1. **Given** the new-firearm form, **When** the user picks the built-in cartridge "9x19mm Parabellum", **Then** the cartridge field shows "9x19mm Parabellum" and the caliber field is filled with that cartridge's caliber ("9mm"); saving and reopening shows both.
2. **Given** the user types a cartridge that is not built in, e.g. "6.5x47 Wildcat" or ".30 Custom Improved", **When** they leave the cartridge field, **Then** the caliber field is pre-filled with a guess ("6.5mm", ".30") that is visibly marked as a guess and can be edited before saving.
3. **Given** the user has typed over the pre-filled caliber, **When** they then change the cartridge again, **Then** the caliber they typed is left unchanged.
4. **Given** the user types a cartridge whose name gives no recognizable bore size, e.g. "Wildcat Special", **When** they leave the cartridge field, **Then** the caliber field stays empty and the form asks them to enter a caliber; the record cannot be saved until a caliber is given.
5. **Given** the user leaves the cartridge blank, **When** they enter a caliber and save, **Then** the record saves with a caliber and no cartridge, as records did before this feature.
6. **Given** firearms chambered in 9x19mm Parabellum, 9x18mm Makarov and a firearm whose cartridge was recorded as just "9mm", **When** the user groups the collection by caliber, **Then** all three fall in the "9mm" group; **and When** they group by cartridge, **Then** each cartridge is its own group, and firearms with no cartridge are grouped as "Unspecified".
7. **Given** a saved firearm with cartridge "9x19mm Parabellum" and caliber "9mm", **When** the user edits it and changes the cartridge to ".45 ACP", **Then** the caliber stays "9mm" and the form offers ".45" as a suggested caliber; accepting it sets the caliber to ".45".
8. **Given** a firearm recorded with a built-in cartridge, **When** a later version of the application changes that cartridge's built-in name or caliber, **Then** the saved record keeps the cartridge and caliber it was saved with.
9. **Given** a firearm whose cartridge is "7.62x39mm", **When** the user searches for "7.62x39", **Then** that firearm is found.
10. **Given** firearms with and without a cartridge, **When** the user browses the list or tiles, **Then** one with a cartridge shows e.g. "9x19mm Parabellum (9mm)" where the caliber is shown, and one without shows only its caliber (FR-027).

---

### User Story 2 - Pick From a Narrowing List While Typing (Priority: P2)

While entering make, model, cartridge or caliber, the collector sees a list below the field of values they have already recorded, plus (for cartridge and caliber) the built-in common cartridges and their calibers. The list shrinks as they type, finds "Smith & Wesson" from "sw" or "smith w", finds ".30-30 Winchester" and ".30 Carbine" from "30", and lets them pick a value or keep typing their own. Small spelling variants such as "smith and wesson" or "9 x 19" snap to the spelling already on record, so the collection doesn't end up split across several spellings of the same thing.

**Why this priority**: This keeps the data consistent, which is what makes grouping by make, caliber and cartridge useful, and it makes the cartridge field of User Story 1 practical to fill in. It is a separate slice because every field still works as plain typed entry without it.

**Independent Test**: With a small seeded collection, type partial values into each of the four fields and confirm the list contents, order and source markers; pick a suggestion; type a same-notation variant and confirm it snaps; type a different notation and confirm it is kept as typed; delete the only firearm using a value and confirm the value no longer appears.

**Acceptance Scenarios**:

1. **Given** firearms recorded with makes "Smith & Wesson", "Heckler & Koch" and "Ruger", **When** the user types "sw", "s&w" or "smith w" in the make field, **Then** "Smith & Wesson" is suggested; **and When** they type "hk", **Then** "Heckler & Koch" is suggested; **and When** they type "r", **Then** "Ruger" is suggested and the other two are not.
2. **Given** the cartridge field is empty or has focus, **When** the user types "30" or ".30", **Then** the list includes ".30-30 Winchester" and ".30 Carbine" (and other cartridges whose name starts with .30), and does not include, e.g., "9x19mm Parabellum".
3. **Given** the user types "9mm" in the cartridge field, **When** the list appears, **Then** it offers the specific cartridges of 9mm caliber, most common first (9x19mm Parabellum, then .380 ACP, 9x18mm Makarov, 9x21mm…); **and When** the user does not pick one, **Then** "9mm" is kept as typed, not blocked, not replaced, and the caliber guess is "9mm".
4. **Given** built-in cartridges and cartridges on record in the list, **When** the list is shown, **Then** each entry shows whether it is built in or from the user's own records, and a value present in both appears once.
5. **Given** "Smith & Wesson" is on record, **When** the user types "smith and wesson", "Smith&Wesson" or "SMITH & WESSON" and leaves the field, **Then** the field changes to "Smith & Wesson" before saving, visibly.
6. **Given** "9x19mm Parabellum" is a built-in cartridge, **When** the user types "9 x 19mm parabellum" or "9×19mm Parabellum", **Then** the field snaps to "9x19mm Parabellum"; **but When** the user types "9x19" or "9mm Luger", **Then** the value is kept as typed (a different notation, not a variant), though the list offers "9x19mm Parabellum".
7. **Given** a custom cartridge "Wildcat Special" used by one firearm only, **When** that firearm is deleted, **Then** "Wildcat Special" no longer appears in any suggestion list; **but When** the firearm is instead marked disposed, **Then** it still appears.
8. **Given** the user typed a make on a new-firearm form and then cancelled without saving, **When** they open a new form and type in the make field, **Then** the unsaved value is not suggested.
9. **Given** firearms of make "Ruger" with models "10/22" and "Mini-14", and of make "Marlin" with model "336", **When** the user has entered make "Ruger" and types in the model field, **Then** "10/22" and "Mini-14" are listed before "336".
10. **Given** the user is moving through the form with the keyboard, **When** the list is open, **Then** they can move through suggestions, pick one, or dismiss the list and keep their own text, without using the mouse.

---

### User Story 3 - Record the Action Type (Priority: P3)

The collector records how a firearm operates by choosing its action (semi-automatic, bolt action, lever action, pump action, break action, revolver, and so on) from a fixed list. The list shows only the actions that make sense for the chosen firearm type. The action is optional, can be grouped by and searched, and is carried through export and import.

**Why this priority**: It is a useful new structured field, but independent of the cartridge and suggestion work, and the collection is fully usable without it.

**Independent Test**: Create firearms of each seeded type with and without an action, change a firearm's type to one that doesn't allow its action, group by action, search by action name, and round-trip through export and import.

**Acceptance Scenarios**:

1. **Given** a new firearm of type Shotgun, **When** the user opens the action choice, **Then** only the actions mapped to Shotgun are offered (e.g. pump action, semi-automatic, break action, lever action, bolt action), and "revolver" is offered only if mapped to Shotgun.
2. **Given** a firearm of type "Other" or of a type the user added, **When** the user opens the action choice, **Then** the whole action list is offered.
3. **Given** a firearm of type Rifle with action "lever action", **When** the user changes its type to Handgun and lever action is not mapped to Handgun, **Then** the action is cleared and a notice says so before saving; **and When** the new type does allow the action, **Then** it is kept.
4. **Given** firearms with and without actions, **When** the user groups by action type, **Then** each action is a group and firearms with none are grouped as "Unspecified".
5. **Given** a firearm with action "bolt action", **When** the user searches for "bolt", **Then** that firearm is found.
6. **Given** the user leaves the action unset, **When** they save, **Then** the record saves normally with no action.
7. **Given** the action choice, **When** the user tries to type a value that is not on the list, **Then** it cannot be entered: the action is chosen only from the fixed list.
8. **Given** firearms with and without an action, **When** the user browses the list, **Then** the action type column shows each action's name or is blank, and the column is hidden while grouped by action type (FR-027).

---

### User Story 4 - Cartridge and Action Through Export and Import (Priority: P4)

The collector exports the collection and sees cartridge and action type columns beside caliber. They import a spreadsheet from another source that has cartridges but no calibers, or spelling variants of makes already on record, and the import fills in and tidies those values, telling them exactly which rows it guessed or changed.

**Why this priority**: It extends the existing export and import to the new fields and applies the same consistency rules to bulk entry; it depends on the fields from Stories 1 and 3.

**Independent Test**: Export a collection with cartridges and actions and re-import it into an empty database; import a sheet with blank calibers, snapping variants, unknown actions and disallowed actions; check the records and the import report.

**Acceptance Scenarios**:

1. **Given** firearms with cartridges and actions, **When** the user exports, **Then** the spreadsheet has `cartridge` and `action_type` columns with the recorded cartridge and the action's name, and re-importing it into an empty database recreates both.
2. **Given** an import row with cartridge "9x19mm Parabellum" and a blank caliber, **When** it is imported, **Then** the caliber is filled from the built-in list ("9mm"), and the import report lists the row as one whose caliber was derived.
3. **Given** an import row with a custom cartridge ".30 Custom Improved" and a blank caliber, **When** it is imported, **Then** the caliber is filled by the same guess used in the form, and the report lists the row as guessed, showing the value used.
4. **Given** an import row with a blank caliber and a cartridge whose caliber cannot be guessed, or with both blank, **When** it is imported, **Then** it is a row error saying a caliber is required.
5. **Given** "Smith & Wesson" is on record, **When** an import row has make "smith and wesson", **Then** the imported record has "Smith & Wesson", and the import report counts the values changed this way and lists them.
6. **Given** no make like "Springfield Armory" is on record, **When** a sheet has "springfield armory" in row 2 and "Springfield Armory" in rows 5 and 8, **Then** all three records get "Springfield Armory" and the report lists the change to row 2.
7. **Given** an import row with action type "BOLT ACTION", **When** it is imported, **Then** it matches "bolt action"; **and When** the action is "flintlockish" (not on the list) or is not allowed for the row's firearm type, **Then** the row is an error naming the problem.
8. **Given** a spreadsheet exported before this feature, without `cartridge` or `action_type` columns, **When** it is imported, **Then** the rows import with no cartridge and no action.

---

### Edge Cases

- **A value in the built-in list and on record with different capitalization** (e.g. "9X19mm Parabellum" on record): the built-in spelling is shown once; newly entered variants snap to the built-in spelling; the existing record keeps its spelling until edited (no retroactive rewrite).
- **Two spellings of the same value on record** with no built-in spelling (e.g. "Springfield Armory" on 3 firearms and "Springfield armory" on 1): suggestions show the most-used spelling, and new variants snap to it; on a tie, the spelling first recorded wins.
- **Variants only within an import sheet** (nothing on record or in the catalog): they snap to the spelling most rows use, the earliest row on a tie (FR-026).
- **Existing records that differ by notation** ("S&W" on some, "Smith & Wesson" on others) stay as they are; both appear in suggestions and as separate groups. Merging them is not part of this feature.
- **Editing an existing record without touching a field** does not snap that field; only a field the user enters or changes is snapped.
- **A snap the user doesn't want**: the user can see the change before saving. Because snapping only joins same-notation variants, the only way to keep a variant is a different notation; this is accepted.
- **Leading dot and separators**: "22" finds ".22 Long Rifle", ".22 Short", ".22 WMR"; "3006" and "30-06" find ".30-06 Springfield".
- **Aliases**: ".22 LR" finds ".22 Long Rifle"; "9mm Luger", "9mm Para" and "9x19" find "9x19mm Parabellum"; ".223" finds ".223 Remington" and "5.56" finds "5.56x45mm NATO", which are distinct entries.
- **Abbreviations that are not initialisms** (e.g. "CZ" for Česká zbrojovka) find a value only if it is on record under that spelling. No built-in list of make or model aliases exists.
- **Shotgun gauges and bores**: "12 gauge", "20 gauge", ".410 bore" are built-in cartridges; the caliber is the gauge or bore itself (e.g. "12 gauge").
- **Muzzleloaders and other firearms with no cartridge**: the cartridge is left blank and only the caliber is recorded (e.g. ".50").
- **A firearm chambered for more than one cartridge** (e.g. a .357 Magnum revolver that also fires .38 Special, or a convertible): one cartridge is recorded, the one the user chooses; others can go in notes. Multiple cartridges per firearm are out of scope.
- **A cap-and-ball revolver** fits two actions (revolver, percussion): the user chooses one; the action is single-valued.
- **Input over the length cap, with control characters, or only spaces** in make, model, cartridge or caliber: rejected with a field message on entry, and as a row error on import; surrounding whitespace is trimmed.
- **The suggestion list with a very large collection**: it must stay responsive at 10,000 firearms (see SC-004); it shows a limited number of entries at a time, best matches first.
- **The user changes firearm type while the action choice is open** or changes it several times: the action is checked against the type in effect at save; a cleared action is announced once per change.
- **Firearm type renamed or deleted**: the action mapping follows the type it belongs to; a firearm whose type has no mapping shows the whole list (FR-017).
- **Database open on a later version with more built-in cartridges**: records are unaffected (FR-004); only suggestions change.

## Requirements *(mandatory)*

### Functional Requirements

**Cartridge and caliber**

- **FR-001**: System MUST allow the user to record an optional **cartridge** on a firearm (free text, e.g. "9x19mm Parabellum", ".22 Long Rifle", "12 gauge"), alongside the **caliber**, which stays required (001 FR-001). A blank cartridge is stored as no value.
- **FR-002**: System MUST ship a built-in, read-only catalog of common cartridges. Each entry has a name, a caliber, zero or more aliases, and a commonness rank. The catalog is part of the application, not written to the user's database; a catalog entry that no firearm uses leaves nothing in the database.
- **FR-003**: When the user picks a catalog cartridge, the system MUST copy the entry's name to the cartridge field and its caliber to the caliber field, unless the user has already edited the caliber field on this form (FR-006).
- **FR-004**: A saved firearm MUST store its own cartridge and caliber text. Grouping, search, display, export and value summaries MUST use the stored values and MUST NOT depend on the catalog, so catalog changes in later versions never alter existing records.
- **FR-004a**: A catalog cartridge's caliber MUST be its bore class, not the finer designation in its name: cartridges of the same nominal bore share one caliber, so that grouping by caliber gathers them. For example `.308 Winchester`, `.300 AAC Blackout`, `.30-06 Springfield` and `.30-30 Winchester` are all ".30"; `9x19mm Parabellum`, `9x18mm Makarov` and `.380 ACP` are all "9mm". The class for each catalog entry is catalog data, settled with the catalog in planning.
- **FR-005**: For a cartridge not in the catalog, the system MUST make a best-effort guess at its caliber and pre-fill the caliber field with it, visibly marked as a guess until the user accepts or edits it. The guess reads the bore designation at the start of the name: an inch-decimal (".30 Custom Improved" → ".30"), a metric diameter ("6.5x47 Wildcat" → "6.5mm"; "9mm" → "9mm"), or a gauge or bore ("16 gauge" → "16 gauge"); the designation is then mapped to its bore class (FR-004a): when it matches the leading designation, an alias or the caliber of a catalog cartridge, that cartridge's caliber is used (".308 Improved" → ".30", ".380 Custom" → "9mm"). When no designation can be read, the caliber field is left empty and the user is asked for it; the guess MUST NOT invent a value.
- **FR-006**: Once the user has edited the caliber field on a form, later changes to the cartridge on that form MUST NOT overwrite the caliber. Before that, each cartridge change re-derives the caliber (FR-003, FR-005). Clearing the caliber field returns it to being derived. When editing a saved firearm, its saved caliber counts as already edited: a cartridge change MUST NOT overwrite it, and the form MUST instead offer the caliber the new cartridge derives as a suggestion the user can accept with one action (none is offered when it equals the saved caliber or none can be derived).
- **FR-007**: The caliber guess MUST be verified against a test corpus of real cartridge names covering at least rimfire, centerfire pistol and rifle, inch and metric notation, shotgun gauges and bores, and names with no readable bore size (e.g. .22 LR, .300 BLK, 7.62x39, 9x19, .45 ACP, 12 gauge, .410 bore), with the expected caliber, or no guess, for each.
- **FR-008**: Cartridge MUST be included in search (001 FR-013) and MUST be a grouping field (001 FR-012), with firearms that have no cartridge grouped as "Unspecified".

**Suggestions while typing**

- **FR-009**: The make, model, cartridge and caliber fields MUST offer a suggestion list below the field while the user types, which narrows as they type. Suggestions are only offered: nothing is applied unless the user picks it, and any value within the entry rules (FR-015) may be kept as typed.
- **FR-010**: Suggestions MUST be drawn, at the time they are shown, from (a) the distinct values of that field across all firearms in the open database, active and disposed, and (b) for cartridge, the catalog's cartridge names, and for caliber, the catalog's calibers. Values are deduplicated ignoring case and the variants of FR-013, so each appears once; when both sources have it, the catalog's spelling is shown. Make and model draw on the user's records only.
- **FR-011**: A value typed but never saved on a firearm MUST NOT be suggested. A value used only by deleted firearms MUST NOT be suggested or otherwise remain usable by this feature (constitution V); the suggestion sources keep no copy of their own.
- **FR-012**: A value MUST match what the user has typed if any of these do, ignoring case, a leading dot, spaces and separators ("-", "/", "x", "×", ".", "&" and "and" where they separate words or numbers): (a) the value starts with the typed text; (b) a word of the value starts with each typed word in order ("smith w"); (c) the typed text is the value's initialism, ignoring "&" and spaces ("sw", "s&w", "hk"); (d) for a catalog cartridge, one of its aliases matches by (a) to (c), or its caliber matches by (a). Suggestions are ordered by match (a), then (b), then (c) and (d); within each, values on record before catalog-only values; values on record by how many firearms use them, catalog-only values by commonness rank. For model, values recorded with the make currently entered on the form come first.
- **FR-013**: When the user finishes entering a make, model, cartridge or caliber (leaves the field or picks a suggestion), and the text is a same-notation variant of a catalog spelling or a value on record, the system MUST replace it with that spelling, visibly, before saving. Same-notation variants differ only in letter case, spacing, the separators of FR-012, or "&" versus "and" between words. A catalog spelling wins over values on record; among values on record, the one used by the most firearms wins, and on a tie the one recorded first. Different notations (e.g. "9mm", "9x19", "9x19mm Parabellum"; "S&W" and "Smith & Wesson") MUST NOT be snapped to each other; aliases and initialisms only narrow the list (FR-012).
- **FR-014**: Snapping MUST apply only to values being entered or changed (on a form or on import). Values already saved on existing records MUST NOT be rewritten, including when a record is edited without changing that field.
- **FR-015**: Make, model, cartridge and caliber MUST accept any text that, once surrounding whitespace is trimmed, is at most 100 characters long and contains no control characters; make, model and caliber MUST also be non-empty. Nothing requires a value to be in the catalog or on record.
- **FR-016**: Each suggestion MUST show whether it comes from the built-in catalog or from the user's own records, so built-in entries also serve as a guide to how a cartridge can be written. The list MUST be operable with the keyboard alone (move, pick, dismiss) and announced to assistive technology (constitution III, WCAG 2.1 AA).

**Action type**

- **FR-017**: System MUST allow the user to record an optional **action type** on a firearm, chosen from a fixed list that ships with the application; the user cannot add, rename or remove actions, and new actions arrive only with application updates. The choices offered MUST be only those mapped to the firearm's type (FR-018); a type with no mapping (the "Other" type and any type the user adds) MUST offer the whole list.
- **FR-018**: The shipped action list and its mapping to the seeded firearm types MUST be:

  | Action type | Handgun | Rifle | Shotgun |
  |-------------|:-------:|:-----:|:-------:|
  | Semi-automatic | ✓ | ✓ | ✓ |
  | Revolver | ✓ | ✓ | ✓ |
  | Bolt action | ✓ | ✓ | ✓ |
  | Lever action | ✓ | ✓ | ✓ |
  | Pump action | | ✓ | ✓ |
  | Break action | ✓ | ✓ | ✓ |
  | Falling block | | ✓ | |
  | Rolling block | ✓ | ✓ | |
  | Single shot (other) | ✓ | ✓ | ✓ |
  | Flintlock | ✓ | ✓ | ✓ |
  | Percussion | ✓ | ✓ | ✓ |
  | Inline muzzleloader | | ✓ | ✓ |

  Automatic and select-fire are not recorded in this feature: the list describes how a firearm cycles, not its fire-control capability, and machine guns are a candidate type for the regulated item types feature (#12), which extends this mapping. Until then, a select-fire firearm records the action it cycles with (e.g. semi-automatic) and the capability goes in notes.
- **FR-019**: When the user changes a firearm's type and its current action is not allowed for the new type, the system MUST clear the action and show a notice saying so on the form before saving. An action allowed for the new type MUST be kept.
- **FR-020**: Action type MUST be a grouping field (001 FR-012), with firearms that have none grouped as "Unspecified", and MUST be included in search (001 FR-013) by its name.
- **FR-021**: Firearms recorded before this feature have no action type and no cartridge; they MUST need no change to remain valid and editable.

**Export and import**

- **FR-022**: The spreadsheet export MUST add a `cartridge` column (the recorded text, blank when none) and an `action_type` column (the action's name, blank when none), placed after `caliber`.
- **FR-023**: On import, `cartridge` and `action_type` MUST be optional columns; a sheet without them imports with no cartridge and no action.
- **FR-024**: On import, `action_type` MUST be matched against the fixed list ignoring letter case and surrounding whitespace. An unknown value, or one not allowed for the row's firearm type (FR-017), MUST be a row error (001 FR-020), as with `firearm_type`.
- **FR-025**: On import, when `caliber` is blank and `cartridge` is given, the system MUST derive the caliber from the catalog, or failing that by the guess of FR-005, and MUST list every such row in the import report with the caliber used and whether it came from the catalog or was guessed. When no caliber can be derived, or both are blank, the row MUST be a row error. A caliber given in the sheet is always used as given (after FR-013).
- **FR-026**: On import, make, model, cartridge and caliber MUST be checked against FR-015 (a violation is a row error) and snapped per FR-013 against the catalog and the values on record at the start of the import. Variants of a value that is neither in the catalog nor on record MUST also be snapped to each other within the sheet: the spelling used by the most rows wins, and on a tie the one in the earliest row. The import report MUST count the values changed by snapping and list each change (row, field, value in the sheet, value recorded). Import matching of existing records (001 FR-026) MUST compare the snapped values.

**Display**

- **FR-027**: The browse list's caliber cell and the browse tiles MUST show the cartridge with its caliber as "<cartridge> (<caliber>)", e.g. "9x19mm Parabellum (9mm)", or the caliber alone when there is no cartridge. The browse list MUST show action type in its own column (blank when none); tiles do not show it. As today with caliber, a column is hidden while the list is grouped by it (grouping by caliber or cartridge hides the caliber cell; grouping by action type hides the action column). The firearm record page MUST show cartridge, caliber and action type.

### Key Entities

- **Firearm** *(extended)*: gains an optional cartridge (free text) and an optional action type (a reference to one Action Type). Caliber stays required free text. Both cartridge and caliber are stored on the record itself.
- **Catalog Cartridge**: a built-in, read-only entry shipped with the application, not stored in any database: name, caliber, aliases, and commonness rank. Used only for suggestions, caliber derivation and snapping.
- **Action Type**: a fixed, application-supplied list of how a firearm operates (e.g. semi-automatic, bolt action). Not user-editable.
- **Firearm Type ↔ Action Type mapping**: which actions apply to which seeded firearm types; many-to-many, since one action applies to several types. A type with no mapping offers every action.
- **Suggestion**: a value offered while typing, from the user's records (active and disposed firearms) or the catalog, with its source shown. Computed when shown, never stored.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can record a firearm's cartridge and caliber for any of the 25 most common cartridges in the built-in catalog by typing at most 4 characters and picking one suggestion, without typing the caliber.
- **SC-002**: For every name in the caliber-guess test corpus (FR-007), the guessed caliber is the expected one, or no guess is made where none is expected; the guess never produces a wrong value for a name listed as unguessable.
- **SC-003**: In a test collection seeded with case, spacing, separator and "&"/"and" variants of 20 makes and cartridges already on record, 100% of the variants entered through the form or import are recorded under the existing spelling, and 0 different-notation values (e.g. "9mm" vs "9x19") are changed.
- **SC-004**: With 10,000 firearms, the suggestion list updates within 100 ms of each keystroke, and grouping and search by cartridge and action type meet the existing budgets (search within 500 ms, actions within 1 s).
- **SC-005**: After deleting every firearm that uses a value, that value appears in no suggestion list and cannot be found anywhere in the database.
- **SC-006**: Exporting a collection with cartridges and action types and importing the file into an empty database reproduces every cartridge, caliber and action exactly.
- **SC-007**: Every import row whose caliber was derived, and every value changed by snapping, appears in the import report; no derived or snapped value is recorded silently.
- **SC-008**: The action choice for a Handgun, Rifle or Shotgun offers only its mapped actions, and a firearm can never be saved with an action its type does not allow.

## Assumptions

- **Cartridge is optional.** Caliber alone remains a complete record (muzzleloaders, unknown chambering, records from older spreadsheets), and a required cartridge would make every existing sheet fail to import.
- **Caliber spelling convention**: a caliber is a bore class (FR-004a), written as collectors commonly name it: inch classes with a leading dot (".22", ".30", ".357", ".45"), metric classes in millimetres ("9mm", "7.62mm", "6.5mm"), shotguns by gauge or bore ("12 gauge", ".410 bore"). A cartridge named in one notation may belong to a class named in the other where that is the common usage (".380 ACP" → "9mm"). The class for each catalog entry, e.g. whether "7.62x39mm" is "7.62mm" or ".30" and ".38 Special" is ".38" or ".357", is catalog data settled with the catalog in planning.
- **Catalog size and content**: a few hundred of the cartridges most commonly found in U.S. private collections, including rimfire, handgun, rifle, shotgun gauges and common military surplus cartridges. Its data source must be recorded with a GPLv3-compatible license, or written for the project (constitution, Licensing).
- **Snapping happens when a field is left or a suggestion picked**, not on every keystroke, so it never fights the user while typing.
- **The length cap of 100 characters** is new: make, model and caliber had no cap before. No existing value is expected to exceed it; an existing record over the cap keeps its value but must be shortened if that field is edited.
- **Model suggestions come from all records**, with models of the entered make ranked first (FR-012), since a model name alone is often shared or reused.
- **Original marks are left alone.** Feature 002's original make, model and serial number record the marks as stamped on the firearm, so they are not snapped and get no suggestions in this feature.
- **The action list is U.S.-centric and deliberately small.** Detail such as double- versus single-action, straight-pull or gas versus recoil operation is left to notes. "Single shot (other)" covers single-shot actions not otherwise listed. The user-added firearm types and "Other" get the whole list, per the recorded decision.
- **Unreleased application**: schema changes are made in place, and databases from earlier development builds need not be migrated (decision of 2026-09-20; see #24 for the policy after 1.0).
- **Normalization of existing values** (merging "S&W" and "Smith & Wesson" on existing records) is a possible later feature, not part of this one.

## Relationship to Feature 001

This feature extends `specs/001-firearms-inventory/`. It **amends**:

- **FR-001** (record contents): a firearm also has an optional cartridge and an optional action type; caliber stays required and may be derived from the cartridge (FR-001, FR-003, FR-005, FR-017).
- **FR-012** (grouping): cartridge and action type become grouping fields, beside type, caliber, make and origin (FR-008, FR-020).
- **FR-013** (search): cartridge and action type name are searchable (FR-008, FR-020).
- **FR-018 / FR-019 / FR-020** (export, import, row errors): `cartridge` and `action_type` columns, derived calibers, snapping and their report (FR-022 to FR-026).
- **FR-026** (import matching): compares make and model after snapping (FR-026).
- **Assumptions** ("Make, model, and caliber are treated as structured (non-free-form) fields … even though their specific values are user-entered"): still true, now with suggestions, snapping and an entry length cap (FR-009 to FR-015).
- **Data model**: `firearms.caliber` stays; a cartridge column, an `ActionType` lookup with a type↔action mapping (seeded, like `FirearmType`) and a nullable action reference on `firearms` are added; the full-text index covers cartridge and action name; `FirearmSummary` carries cartridge and action type.
- **Spreadsheet format**: the two new columns.

It follows the precedent of `FirearmType`, which is seeded into the database because firearms refer to it; the cartridge catalog is deliberately different (not in the database, no reference from a firearm), per the recorded decision below.

## Source Request

This feature was filed as GitHub issue #11, "Cartridges, action types and suggestion picklists for make, model and caliber" (labels: new feature, needs spec). The issue asked for this section to quote its requests verbatim and carry its recorded decisions, which follow. The issue suggested specifying this before the regulated item types feature (#12), which extends the type→action mapping.

**Amends or supersedes in 001 (as listed on the issue):** the spec's Assumptions ("caliber is a structured field… user-entered"), FR-001 (adds cartridge and an optional action type), FR-012 (grouping by caliber; adds action type), data-model `caliber TEXT` (plus a new cartridge column, `ActionType` and type→action tables), the spreadsheet `caliber` column (plus `cartridge`, `action_type`), `FirearmSummary`, the FTS index.

**The requests, verbatim:**

> caliber is somewhat ambiguous, e.g. 9x18 and 9x19 are both "9mm" but are different cartridges, .223 and 5.56x45 are also nominally the same caliber but are different cartridges, .22 Short, .22 Long, and .22 Long Rifle (LR) are all .22 caliber but are different cartridges. Need to provide a way to enter the specific cartridge and derive the caliber based on the input.

> caliber/cartridge input is just a free form field, and users may end up with multiple different ways of specifying the same information. The input should be a combo box that lets users select from existing values or input new values. the available selections should shrink based on what the user types in, e.g. if they start typing ".3" then the list might show ".30-30" and ".30 Carbine". The database should get pre-populated with the most common calibers/cartridges.

> need to be able to specify the action or sub-type of a firearm, e.g. semi-automatic, break action, revolver, lever action, pump action, bolt action, etc.

> when typing in makes, models, calibers, sub/action types, a picklist of vales should appear below the input field (e.g. as with a combo box) to suggest values already entered previously. The suggestions should narrow as the user types. The suggestions should only be based on values presently in the database, including from disposed of firearms. Deleted firearms should not leave behind any values usable by this feature.

**Why these are one feature (from the issue):** the second caliber request and the picklist request are the same combobox mechanism, and the cartridge catalog, action type and make/model suggestions all share it. They also all add or reshape lookup data.

**Decisions recorded on the issue, carried into the requirements above:**

- *Seed the suggestions, not the database (2026-09-19).* "Pre-populated" is read as: the app ships a built-in catalog of common cartridges that feeds the combobox, alongside values taken from the user's own firearm records. Nothing is written to the user's database, so a built-in cartridge that no firearm uses isn't residue, and the "deleted firearms leave nothing behind" rule (constitution V) holds by construction. The catalog is there for convenience (common cartridges can be picked instead of typed) and as formatting hints showing how the user might name their own cartridges. (FR-002)
- *Suggestion sources are merged at query time:* (1) `SELECT DISTINCT` of the field over all firearm rows, active and disposed; (2) the bundled read-only catalog, shipped as an app resource like the generic thumbnails (001 research §10), not as a DB table or migration. Deduplicate case-insensitively, so "9x19mm" appears once if both sources have it. A value the user typed but never saved on a firearm is never remembered. Once the last firearm using a custom cartridge is deleted, that cartridge is gone from the suggestions. (FR-010, FR-011)
- *The firearm record must be self-contained.* The catalog isn't in the DB, so the record stores its own cartridge string and its derived caliber, which lets grouping by caliber (FR-012) work without the catalog. Picking a catalog entry copies its cartridge name and caliber onto the record. Catalog changes in later app versions never alter existing records. (FR-003, FR-004)
- *Caliber for a custom cartridge: guess it, pre-fill it, let the user override it.* A catalog cartridge maps to its caliber exactly. For a cartridge not in the catalog, the app makes a best-effort guess and pre-fills the caliber field; the guess is visible and editable before saving; once the user has edited the caliber field, later cartridge edits must not overwrite it. Caliber stays required (FR-001), so grouping never has a gap; if a name doesn't parse, leave the field empty and ask rather than guess wrong. On import, if `caliber` is blank but a cartridge is given, derive the caliber (catalog first, then the guess) and list those rows in the import report, so a guess is never silent. The heuristic needs a test corpus of real names (.22 LR, .300 BLK, 7.62x39, 9x19, .45 ACP, 12 gauge…). (FR-005 to FR-007, FR-025)
- *Normalization: yes, and visible, for make, model, cartridge and caliber* (action type is a closed list). Equivalence ignores case, spacing and separator variants: "9mm" = "9 mm", "9x19" = "9 x 19" = "9×19", "Smith & Wesson" = "Smith and Wesson" = "Smith&Wesson" (`&` ≡ `and` is a token rule). The field snaps on entry, so the user sees the change before saving. On import it snaps too, and the report counts the changes. A built-in spelling wins over a user's variant; otherwise the most-used existing spelling wins. (FR-013, FR-026)
- *Guide, never restrict.* The catalog and suggestions steer users toward specific cartridges, but free entry is always accepted within reason: non-empty, trimmed, a sensible length cap, no control characters. Nothing requires a value to be in the catalog. Many users mean 9x19 when they say "9mm" and may not know other 9mm cartridges exist, and recording "9mm" is acceptable. So a bare "9mm" is kept as typed (not blocked, not snapped). The list narrows to the specific 9mm cartridges, most common first (9x19mm Parabellum, then .380 ACP, 9x18 Makarov, 9x21…), and the user may pick one or decline. A suggestion is never applied unless the user chooses it. The same principle applies to make, model and caliber. Action type is the one closed list. (FR-009, FR-015)
- *Snapping is for same-notation variants only.* Case, spacing, separator and `&`/`and` variants of the same text snap to each other ("9 mm" → "9mm", "9 x 19" → "9x19", "Smith and Wesson" → "Smith & Wesson") when the target is on record or is a catalog spelling. Different notations are not snapped: "9mm", "9x19" and "9x19mm Parabellum" stay as entered. Catalog aliases ("9mm Luger", "9mm Para", "9x19") only narrow and rank the suggestion list. So catalog entries need a commonness rank to order suggestions, and the caliber guess for "9mm" is simply "9mm". (FR-012, FR-013)
- *Abbreviations and acronyms narrow the suggestion list only; they never snap.* The narrowing modes are prefix, word prefix ("smith w") and initialism ignoring `&` and spaces ("sw" or "s&w" finds Smith & Wesson; "hk" finds Heckler & Koch), run against the merged suggestion pool. Catalog aliases add ".22 LR" ↔ ".22 Long Rifle" and similar. Abbreviations that aren't initialisms (e.g. "CZ" for Česká zbrojovka) match only if the value is on record under that spelling. No built-in list of make aliases is planned; makes and models come only from the user's records. (FR-012)
- *Only new entries and imports are normalized.* Existing records that already differ ("S&W" vs "Smith & Wesson") stay split. A later tool to merge or rename values is a possible follow-up, not part of this feature unless wanted. (FR-014)
- *Action type is a closed choice from a fixed, pre-populated list*, not a combobox and not part of the suggestion mechanism. It's modeled like `FirearmType`: a seeded `ActionType` lookup table (migration) and a nullable FK on Firearm. New values arrive only with app updates ("generalize, or ask for an update"). The choices are filtered by firearm type through a seeded type→action mapping (a join table, since e.g. semi-automatic applies to several types). The spec had to define: the seed list and the mapping (starting set: semi-automatic, break action, revolver, lever action, pump action, bolt action; likely additions: single-shot, falling/rolling block, muzzleloader ignition types; decide how to treat full-auto/select-fire); the fallback (`FirearmType` is user-extensible, and a user-added type or "Other" has no mapping, so show the whole list); that changing the firearm type after choosing an action the new type doesn't allow clears the action, with a notice; that the field is optional and existing records get none and group as "Unspecified"; import/export (`action_type` matched case-insensitively against the list; an unknown value, or one not allowed for the type, is a row error, as with `firearm_type`; export writes the name); and that it is a structured field, so it becomes groupable (FR-012) and searchable through a join, like the type name. This takes action type out of the picklist request above, which keeps make, model, cartridge and caliber. (FR-017 to FR-024)
- *Migration: not applicable (2026-09-20).* The app is unreleased, so data from earlier development builds doesn't have to be migrated or preserved. Per CLAUDE.md, schema changes edit the existing migrations in place. See #24 for the policy after 1.0.
- *UI:* show catalog entries and the user's own entries in one narrowing list, but mark which source each comes from (e.g. a "built-in" marker or grouping) so the catalog works as a formatting hint. Matching should forgive leading dots and separators ("30" or ".30" finds ".30-30" and ".30 Carbine") and use the abbreviation and initialism modes above. (FR-012, FR-016)
- *Precedent, not a conflict:* `FirearmType` is seeded into the DB (migration 0003) because it's a real lookup table with an FK from `firearms`. Cartridge is deliberately different: no FK, free text on the record.
