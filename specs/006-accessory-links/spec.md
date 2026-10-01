# Feature Specification: Accessory Records and Mounting

**Feature Branch**: `006-accessory-links`

**Created**: 2026-10-01

**Status**: Draft

**Input**: User description: GitHub issue #50, "Link accessories such as suppressors to the firearms they fit or are mounted on". It was raised while clarifying spec 005, when the owner kept linking a suppressor to its host firearms out of that feature because linking is not specific to suppressors or NFA items. The issue asks what can be linked, what a link means, where links show (records, search, grouping, the spreadsheet), what happens to links when a record is disposed or deleted, whether linking covers a registered receiver used with several uppers, and how it performs at 10,000 items. The request, its questions and the owner's 0.1.0 plan are reproduced under [Source Request](#source-request) so this spec stands on its own.

## Clarifications

### Session 2026-10-01

- Q: What can be linked: firearm records only, or accessories as a new kind of record, and what happens to the free-text Accessories field? → A: Accessories become a new kind of record, separate from firearms: an optic, a magazine, a stock, an upper and so on, each with its kind, make, model, optional serial number, value and notes. The free-text Accessories field on a firearm is kept as it is, alongside the new records, and nothing converts its text. A suppressor stays a firearm record (005), so items that can be mounted are accessory records and firearm records (FR-001, FR-007, FR-010).
- Q: What does a link mean: "fits" (compatible), "mounted on" (current configuration), or both? → A: "Mounted on" only. A link records where an item is mounted now: each item is mounted on at most one firearm at a time, and a firearm can have any number of items mounted on it. Which firearms an item fits is not recorded; it goes in notes. No history of past mounts is kept (FR-010, FR-013).
- Q: How do links appear in the spreadsheet, given one row per firearm and no record identifier that holds across files yet (#53)? → A: Export and import them by identifier. Every firearm and accessory row carries its stable record identifier (issue #53), and a row's "mounted on" column holds the identifier of its host firearm, so exporting and importing reproduces the mounts. This makes #53's identifier, and its use in the spreadsheet, a dependency of this feature (FR-019 to FR-024).
- Q: How does one export or import carry both firearms and accessories, given that a CSV file holds one table? → A: As two tables. Export writes a workbook with a firearm sheet and an accessory sheet, or two CSV files. Import takes a workbook with both sheets, or two single-table files picked together (two workbooks or two CSV files), each recognized by its header. The accessory table is optional: without it, no accessories are imported (FR-020, FR-022).
- Q: When items are disposed of with their host firearm, what disposition price do they get? → A: Each its own. The dispose dialog asks for an optional price for each item going with the firearm, beside the firearm's own price; the type, recipient and date are shared (FR-014).
- Q: Can the user add a new accessory from a firearm's record page, already mounted on it, or only mount accessories that already exist? → A: Both, through one "Mount" control on the host's record page with two choices, "New accessory…" (the accessory form, with "Mounted on" set to this firearm) and "Existing accessory or firearm…". The application's wording is consistent: it says "accessory", "firearm" and "mount", never "item" or "host", which are this spec's terms only (FR-012).
- Q: Where does a firearm's record page list the accessories and firearms mounted on it? → A: In a new "Mounted" section, which also holds the Mount control. The free-text "Accessories" section stays separate and unchanged, so a mounted firearm is never listed under "Accessories" (FR-007, FR-013).
- Q: When the user exports only the firearms matching the collection page's filter, which accessories go with them? → A: The accessories mounted on the exported firearms. Unmounted accessories, and accessories mounted on firearms outside the filter, are left out; an export of everything writes every accessory (FR-020).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Record an Accessory (Priority: P1)

A collector records the scope they bought for their deer rifle: kind "Optic", make "Leupold", model "VX-5HD 3-15x44", its serial number, the $1,100 they paid, its estimated value of $1,000 and a note about the reticle. They record their six spare Glock 17 magazines as one record of kind "Magazine" with quantity 6. Each accessory's value now counts in the collection's value summary and in the blanket policy's total, so the insurance figures reflect what they own.

**Why this priority**: Accessories are the records this feature introduces, and their value matters for insurance on its own, even before any of them is mounted on a firearm. An expensive optic is often worth more than the rifle it sits on, and today its value can only be folded into the firearm's or left out.

**Independent Test**: Create, edit, dispose, restore and delete accessories of several kinds, with and without each optional field; confirm they save, reopen intact, appear on the Accessories page, and are counted in the value summary, the blanket total and the coverage warnings exactly when they are active.

**Acceptance Scenarios**:

1. **Given** the Accessories page, **When** the user adds an accessory, **Then** the form asks for a kind (required) and offers make, model, serial number, quantity (default 1), caliber, cartridge, estimated value, acquisition source, date and price, and notes, all optional.
2. **Given** the kind choice, **When** the user opens it, **Then** it offers Optic, Light or laser, Magazine, Stock or brace, Upper receiver, Barrel, Muzzle device, Conversion kit, Mount or rail, Sling, Case and Other, and does not offer Suppressor; the form's hint says a suppressor is recorded as a firearm.
3. **Given** an Optic "Leupold VX-5HD 3-15x44" with serial number, value $1,000, price $1,100, acquired 2025-11-02 and a note, **When** the user saves and reopens it, **Then** every value is intact and amounts are shown as elsewhere ("$1,000").
4. **Given** a Magazine with make "Glock", model "G17 17-round", quantity 6 and value $180, **When** the user views it, **Then** it is named "Glock G17 17-round" with "× 6" and its value is the value of all six.
5. **Given** an accessory with only a kind ("Sling"), **When** the user saves it, **Then** it saves and is named by its kind.
6. **Given** a new accessory, **When** the user picks the cartridge "5.56x45mm NATO" with an empty caliber, **Then** the caliber is filled in as it is for a firearm (004); **and When** they type a make, **Then** the suggestions include the makes already on record for firearms and accessories.
7. **Given** active accessories with values $1,000 and $180, a blanket policy in force and no accessory scheduled, **When** the user views the value summary, **Then** the collection total and the blanket total each include $1,180, and accessories are shown as their own subtotal.
8. **Given** an accessory with a value, **When** the user schedules it under a policy with a coverage amount below its value, **Then** it shows the under-insured warning a firearm would; **and When** no policy covers it, **Then** it shows the uninsured warning.
9. **Given** an active accessory, **When** the user marks it disposed (sold, with recipient, date and price), **Then** it leaves the default Accessories list and the value summary, appears when disposed records are shown, and can be restored as a firearm can.
10. **Given** an accessory, **When** the user deletes it after confirming, **Then** it and everything recorded on it are removed and its values are no longer suggested if nothing else uses them.
11. **Given** a firearm with free text in its Accessories field, **When** this feature is in place, **Then** the text is unchanged, still editable, searchable and exported, and no accessory record is created from it.

---

### User Story 2 - Mount an Item on a Firearm (Priority: P2)

The collector mounts the Leupold on their deer rifle, and their suppressor (a firearm record since 005) on the same rifle. The rifle's record lists both as mounted; the scope's record and the suppressor's record each say "Mounted on" the rifle. Before a range trip they move the suppressor to their AR in one step. Their short-barreled rifle is registered as one receiver used with three uppers, so they record each upper as an accessory of kind "Upper receiver" and keep the one currently in use mounted on the receiver.

**Why this priority**: Mounting is what the issue asks for, and it is what tells the owner, or an insurer or executor reading the records, which items go with which firearm. It depends on accessory records (User Story 1) for most items, but a suppressor can be mounted using firearm records alone.

**Independent Test**: Mount accessories and firearms on a host from either record, move them between hosts, unmount them, and confirm both records show the current mount, that no item is ever on two hosts, and that every rule in FR-010 to FR-015 blocks or allows as stated.

**Acceptance Scenarios**:

1. **Given** an active Optic with quantity 1 and an active Rifle, **When** the user sets the Optic's "Mounted on" to the Rifle and saves, **Then** the Optic's record shows "Mounted on" with the Rifle's name as a link, and the Rifle's record lists the Optic under Mounted as a link to its record.
2. **Given** an active Suppressor firearm and an active Rifle, **When** the user mounts the Suppressor on the Rifle from the Rifle's record, **Then** both records show the mount, and the Suppressor is listed on the Rifle with its type.
3. **Given** a Suppressor mounted on a Rifle, **When** the user mounts it on an AR instead, **Then** it is mounted on the AR and no longer on the Rifle, with no separate unmount step.
4. **Given** a Rifle's record page, **When** the user chooses Mount, then "New accessory…", and saves an Optic, **Then** the Optic is recorded and mounted on the Rifle in one step; **and When** they choose "Existing accessory or firearm…", **Then** they choose among the active accessories with quantity 1 and the active firearms that can be mounted on the Rifle (FR-010), found by make, model, nickname or serial number.
5. **Given** a mounted item, **When** the user unmounts it from either record, **Then** neither record shows the mount, and nothing records that it was ever mounted.
6. **Given** the "Mounted on" choice, **When** the user opens it, **Then** it lists only active firearms other than the item itself (and, for a firearm, none that is mounted on it directly or through other firearms), and finds them by make, model, nickname or serial number.
7. **Given** a Magazine record with quantity 6, **When** the user tries to mount it, **Then** mounting is not offered and the form says an accessory with a quantity of more than 1 can't be mounted; **and Given** a mounted Optic, **When** the user changes its quantity to 2, **Then** the save is blocked with a message on the quantity field.
8. **Given** a Suppressor rated for .22 WMR, **When** the user mounts it on a .308 rifle, **Then** it is mounted with no warning: the application does not judge compatibility.
9. **Given** a receiver registered as a short-barreled rifle and three Upper receiver accessories, **When** the user mounts one upper on it and then mounts another, **Then** only the second is mounted, and the first remains a record that is not mounted.
10. **Given** a Rifle with a mounted Optic and a mounted Suppressor, **When** the user views the Rifle's record, **Then** its Mounted section lists the Optic and the Suppressor, and its Accessories section shows its free-text accessories as today; **and Given** an active Rifle with nothing mounted, **Then** its Mounted section says "Nothing mounted." beside the Mount control, and its Accessories section, if empty, shows "No accessories recorded." with its "Add" link, as today.

---

### User Story 3 - Dispose and Delete With Items Mounted (Priority: P3)

The collector sells the deer rifle with its scope but keeps the suppressor. Disposing the rifle lists what is mounted on it; they choose to dispose the scope with it, and the suppressor is unmounted and stays active. Later they delete a record entered by mistake, and anything mounted on it is unmounted, not deleted.

**Why this priority**: The lifecycle rules keep mounts truthful (nothing appears mounted on a firearm that is gone), and constitution V requires deletion to remove the data. It builds on User Story 2.

**Independent Test**: With items mounted, dispose, restore and delete hosts and items, choosing each option, and confirm the resulting records, mounts, value summary and the database contents.

**Acceptance Scenarios**:

1. **Given** a Rifle with an Optic and a Suppressor mounted, **When** the user starts to dispose of the Rifle, **Then** the dispose dialog lists both items and lets the user choose for each whether it is disposed with the Rifle or kept, with "kept" chosen by default; an item chosen to go with the Rifle gets its own optional price field.
2. **Given** that dialog with the Optic chosen to go with the Rifle, the Rifle's price $1,200 and the Optic's price $300, **When** the user confirms, **Then** the Rifle and the Optic are disposed with the same type, recipient and date, the Rifle with price $1,200 and the Optic with $300, the Suppressor is active and not mounted, and no mount remains on any of them; **and When** the Optic's price is left blank, **Then** it is disposed with no price.
3. **Given** a mounted Optic, **When** the user disposes of the Optic, **Then** the dialog says it will be unmounted from its host, and it is.
4. **Given** a disposed Rifle and an Optic disposed with it, **When** the user restores either, **Then** it is restored unmounted and the other is unchanged.
5. **Given** a Rifle with items mounted, **When** the user deletes the Rifle, **Then** the confirmation names the items that will be left unmounted, the items stay in the collection unmounted, and no trace of the mount remains in the database.
6. **Given** a mounted accessory, **When** the user deletes it, **Then** it and its mount are removed completely and the host's record no longer lists it.

---

### User Story 4 - Browse, Group and Search Accessories (Priority: P4)

The collector opens the Accessories page, groups it by kind to see all their optics, then by "Mounted on" to see what is on each firearm and what is in the safe unmounted, and searches for a serial number to find a light.

**Why this priority**: Browsing makes a growing set of accessories manageable, but recording and mounting deliver value without it.

**Independent Test**: With accessories of every kind, mounted and unmounted, active and disposed, group by each grouping field and search by each recorded field, and check membership and the disposed toggle.

**Acceptance Scenarios**:

1. **Given** accessories of several kinds, **When** the user groups the Accessories page by kind, **Then** each kind is a group.
2. **Given** accessories mounted on two firearms and some unmounted, **When** the user groups by "Mounted on", **Then** each host is a group named as the firearm is named elsewhere, and the rest are under "Not mounted".
3. **Given** accessories with and without a make or caliber, **When** the user groups by make, caliber or cartridge, **Then** the blank ones are under "Unspecified".
4. **Given** an accessory whose serial number, model or note contains a search term, **When** the user searches the Accessories page for it, **Then** the accessory is found; a search matches inside a value as the collection's search does.
5. **Given** a disposed accessory, **When** the user browses or searches, **Then** it appears only when disposed records are shown, as on the collection page.
6. **Given** a Rifle with an Optic mounted, **When** the user searches the collection page for the Optic's model, **Then** the Rifle is not found through the mount (it is found only through its own fields, including its free-text accessories).

---

### User Story 5 - Accessories and Mounts Through Export and Import (Priority: P5)

The collector exports the collection and gets the firearms and, in their own table, the accessories, each row with its record identifier and the identifier of the firearm it is mounted on. Importing that export into an empty database reproduces every firearm, accessory and mount. Importing an edited copy back into the original database matches each row to its record by identifier, including firearms with no serial number, so nothing is duplicated.

**Why this priority**: It completes the feature for users who keep a spreadsheet copy or move records between databases, but the feature is usable without it. It depends on issue #53's record identifier.

**Independent Test**: Export a collection with accessories of every kind and mounts of both kinds of item, import it into an empty database and compare; re-import it into the source database with each conflict choice; import sheets with each error and warning case below and check the report.

**Acceptance Scenarios**:

1. **Given** a collection with firearms and accessories, **When** the user exports, **Then** firearms and accessories are written as separate tables (a firearm sheet and an accessory sheet in one workbook, or two CSV files), every row has a record identifier column, and every row has a "mounted on" column holding its host's identifier or blank.
2. **Given** a collection with accessories, **When** the export dialog states what leaves the encrypted database, **Then** it names accessories with their serial numbers and values.
3. **Given** that export, **When** it is imported into an empty database that has the same insurance policies (by name), **Then** every firearm, accessory, field and mount is reproduced, and each record keeps the identifier from its row; this holds for a workbook with both sheets, for the two sheets saved as two single-sheet workbooks, and for the two CSV files, each picked together in one import.
4. **Given** that export edited and imported back into the source database, **When** a row's identifier matches an existing record, **Then** the row is a conflict resolved as today (skip, overwrite or create a duplicate), including a firearm with no serial number and any accessory; **and When** the user chooses to create a duplicate, **Then** the new record gets a new identifier.
5. **Given** an accessory row whose "mounted on" holds an identifier that matches no firearm in the file or the database, or a disposed firearm, **When** it is imported, **Then** the accessory is imported unmounted and the report lists a warning naming the row and the reason.
6. **Given** an accessory row with a blank or unknown kind, a quantity that is not a whole number of at least 1, or an invalid amount or date, **When** it is imported, **Then** the row is an error naming the problem; a kind is matched ignoring letter case and surrounding whitespace.
7. **Given** a spreadsheet exported before this feature, with no identifier or "mounted on" column and no accessories table, **When** it is imported, **Then** the firearms import as they do today and no accessories or mounts are created.
8. **Given** an export's firearm table alone (a workbook without its accessory sheet, or only the firearm CSV), **When** it is imported, **Then** the firearms and their mounts on each other import, and no accessories are imported or changed; **and Given** the accessory table alone, **When** it is imported, **Then** its mounts are resolved against the firearms already in the database (FR-023).
9. **Given** a collection page filtered to one Rifle with an Optic and a Suppressor mounted, plus an unmounted Light, **When** the user exports the filtered firearms, **Then** the export holds the Rifle and the Optic, not the Light, and holds the Suppressor only if it matches the filter itself.

---

### Edge Cases

- **A suppressor that fits several firearms**: only where it is mounted now is recorded; compatibility goes in notes (clarification of 2026-10-01).
- **A firearm mounted on a firearm** (a suppressor, an under-barrel shotgun or launcher): allowed, since any active firearm can be an item. A firearm can both host items and be mounted (a launcher with a light, mounted on a rifle), but never on itself, directly or through other firearms (FR-010).
- **An optic on an upper**: the host is always a firearm, so an optic on an upper that is on a receiver is recorded as mounted on the receiver's firearm record. An optic on an upper that is off any receiver is recorded as not mounted, with a note if wanted (Assumptions).
- **A short-barreled rifle registered as one receiver with several uppers**: the receiver is the firearm record and holds the registration (005); each upper is an Upper receiver accessory, at most one mounted at a time (User Story 2, scenario 9).
- **A record of several identical items** (six magazines): one record with a quantity; it can't be mounted. To mount one of them, the owner records it separately with quantity 1.
- **Value counted twice**: if a firearm's estimated value already includes its scope and the scope is also recorded with a value, the total counts the scope twice. The application does not detect this; the accessory's value hint says to value each record on its own.
- **The free-text Accessories field names an item that is also recorded as an accessory**: nothing compares them; the owner may edit the text.
- **An item mounted on a host that is then disposed of or deleted**: see User Story 3; the mount always ends.
- **Restoring a disposed host**: its former items are not mounted again; the owner mounts them again if they are.
- **An accessory kind or firearm type that a later version no longer offers**: records holding it keep it and show it; a kind is never removed or renamed (FR-002).
- **Changing an item's host while it is disposed**: not possible; only active records are mounted.
- **Importing with an identifier that matches one record and a make, model and serial number that match another**: the identifier wins; the make, model and serial number are compared only when the row has no identifier or its identifier matches nothing (FR-022).
- **Two rows in one import claiming the same identifier**: the second is a row error.
- **Two files picked for one import that hold the same table** (two firearm files, or two accessory files), or a file whose header is neither table's: the import does not start, and the dialog says which file is the problem.
- **Overwriting a record on import whose row has a blank "mounted on"**: the record ends up unmounted, since overwrite replaces the record's values.
- **A mount in an import that would form a loop** (two firearm rows each mounted on the other): the rows import, the mount that would close the loop is dropped, and the report lists a warning.
- **Policy deletion with scheduled accessories**: they are resolved as scheduled firearms are (001 FR-034), counted together with them.

## Requirements *(mandatory)*

### Functional Requirements

**Accessory records**

- **FR-001**: The system MUST allow the user to create, view, edit and delete **accessory** records, a kind of record separate from firearms. An accessory has a **kind** (required) and these optional fields: make, model, serial number, quantity (a whole number of at least 1, 1 when left blank), caliber, cartridge, estimated value, acquisition source, acquisition date, acquisition price and notes.
- **FR-002**: The kind MUST be chosen from a list that ships with the application: Optic, Light or laser, Magazine, Stock or brace, Upper receiver, Barrel, Muzzle device, Conversion kit, Mount or rail, Sling, Case, Other. The user cannot add, rename or remove kinds. A later version MAY add kinds or stop offering one for new choices, but MUST NOT remove or rename a kind that a saved record can hold (as 005 FR-007). Suppressor is not a kind: a suppressor is a firearm record (005 FR-001), and the kind choice's hint says so.
- **FR-003**: Make, model, serial number, caliber, cartridge and acquisition source MUST follow 004's entry rules (004 FR-015). Make, model, caliber, cartridge and acquisition source MUST offer suggestions and snap to spellings on record (004 FR-009 to FR-013), drawn from firearms and accessories together, and a blank caliber MUST be derived from the cartridge as for a firearm (004 FR-003, FR-005, FR-006). Amounts are whole dollars (001 FR-037), and the acquisition date MUST NOT be in the future (as 001 for firearms).
- **FR-004**: An accessory's serial number MUST NOT take part in any uniqueness rule or identifying key, and an accessory needs no "no serial number" attestation (001 FR-029 to FR-032 do not apply). An accessory carries no identification and markings (002), registration (005) or physical details (001 FR-039).
- **FR-005**: An accessory MUST be named, wherever it is shown or listed, by its make and model with its kind (e.g. "Leupold VX-5HD 3-15x44 · Optic"), by its kind alone when it has neither, and with its quantity when that is more than 1 ("× 6"). The estimated value is the value of the record as a whole, all items included, and the value field's hint MUST say so and that each record is valued on its own.
- **FR-006**: An accessory MUST be able to be disposed of, its disposition reversed, and its disposition history kept or discarded exactly as a firearm's (001 FR-004, FR-023, FR-033), using the same dialogs. The rules that re-check a firearm's nickname, key and marks on reversal do not apply.
- **FR-007**: A firearm's free-text Accessories field (001 FR-002) MUST be kept unchanged: its contents, editing, search, export and import are as today, and nothing converts it into records or records into it. On the firearm's record page it stays in its own Accessories section, as today, separate from the Mounted section (FR-013).

**Value and insurance**

- **FR-008**: Active accessories MUST count in the value summary (001 FR-015): in the collection total, in the blanket total of the blanket policy in force when not individually scheduled, and in the group of items no policy covers when uninsured, with accessories shown as their own subtotal. Disposed accessories do not count (001 FR-025).
- **FR-009**: An accessory MUST be able to be individually scheduled under an insurance policy with its own coverage amount, through the same coverage dialog as a firearm, and is otherwise covered by the blanket policy in force (001 FR-014, FR-036). The under-insured and uninsured warnings (001 FR-016, FR-017, FR-024) and the resolution of scheduled items when a policy is deleted (001 FR-034) MUST apply to accessories as to firearms, counted together with them.

**Mounting**

- **FR-010**: The system MUST allow the user to record that an **item** is currently **mounted on** a **host**. An item is an active accessory whose quantity is 1, or an active firearm of any type. A host is an active firearm other than the item. An item MUST be mounted on at most one host at a time; a host may have any number of items. A firearm MUST NOT be mounted on itself or on a firearm that is mounted on it, directly or through other firearms.
- **FR-011**: The system MUST NOT judge whether an item suits its host: it MUST NOT check, warn about, suggest or restrict a mount from a kind, type, caliber, cartridge, rated cartridge or any other recorded value (the record-only stance of 005 FR-014).
- **FR-012**: The user MUST be able to mount, move and unmount an item from the item's record and edit form (a "Mounted on" choice) and from the host's record page. The host's record page MUST offer one "Mount" control with two choices: "New accessory…", which opens the accessory form with "Mounted on" set to this firearm, and "Existing accessory or firearm…", which chooses among the records FR-010 allows. The application's own wording MUST say "accessory", "firearm", "mount" and "Mounted on", and MUST NOT call them "items" or "hosts", which are this spec's terms only (constitution III). The choice MUST list only the firearms FR-010 allows and find them by make, model, nickname or serial number. Mounting an item on another host MUST move it in one step. An accessory whose quantity is more than 1 MUST NOT be offered for mounting, and changing a mounted accessory's quantity to more than 1 MUST block the save with a field-level message.
- **FR-013**: A mount records only the current configuration: no history of past mounts is kept, and unmounting removes the mount completely. The item's record page MUST show "Mounted on" with the host's name linking to its record. The host's record page MUST list its items in a **Mounted** section, separate from the free-text Accessories section and holding the Mount control (FR-012), saying "Nothing mounted." when empty and not shown on a disposed firearm, which has no mounts. Each item there is named (FR-005; a firearm item as firearms are named, with its type) and linking to its record (001 FR-040 applies to the return).
- **FR-014**: Disposing of an item MUST unmount it, and the dispose dialog MUST say so. Disposing of a host MUST list its items in the dispose dialog and let the user choose, for each item, to dispose of it with the host, or to keep it active and unmounted, with "keep" chosen by default. An item disposed of with the host takes the host's disposition type, recipient and date, and its own optional price entered in the same dialog (blank means no price; the host's price is never copied to it); either way, no mount remains on any of them. Reversing a disposition MUST NOT restore a mount.
- **FR-015**: Deleting an item or a host MUST remove its mounts completely (constitution V). Deleting a host MUST NOT delete its items, and the delete confirmation MUST name the items that will be left unmounted.

**Browse and search**

- **FR-016**: The application MUST have an **Accessories** page reached from the main navigation, listing accessories with the same disposed-records toggle as the collection (001 FR-025). The list shows each accessory's name (FR-005), its host when mounted, and its estimated value.
- **FR-017**: Accessories MUST be groupable by kind, make, caliber, cartridge and "Mounted on". An accessory with no value for the grouping field is under "Unspecified", and for "Mounted on" under "Not mounted". The grouping control MUST be the same one the collection page uses (constitution III).
- **FR-018**: The Accessories page MUST search every recorded accessory field, notes and serial number included, matching as the collection's search does (001 FR-013). The collection page's search is unchanged: a firearm is not found through the items mounted on it.

**Export and import**

- **FR-019**: Every firearm and accessory MUST have the stable **record identifier** of issue #53, set when the record is created and never changed by editing, disposing, restoring, backing up or restoring a backup. The identifier is shown nowhere in the application except the spreadsheet.
- **FR-020**: The spreadsheet export MUST write firearms and accessories as separate tables: a firearm sheet and an accessory sheet in one workbook, or two CSV files. The accessory table is written only when the export includes an accessory. An export of the whole collection includes every accessory; an export of the firearms matching the collection page's filter (001's export scope) includes only the accessories mounted on those firearms. Firearms are included by the filter alone, never because they are mounted on an included firearm, and a row's "mounted on" names its host even when the host is not in the export. The firearm table MUST gain a record identifier column and a "mounted on" column (the host's identifier, blank when not mounted). The accessory table MUST hold every accessory field (FR-001), its status and disposition, its insurance policy and scheduled amount (as the firearm table does), its record identifier and its "mounted on". Column names and positions are set in the spreadsheet contract.
- **FR-021**: When the export includes any accessory, the export dialog's statement of what leaves the encrypted database MUST name accessories, with their serial numbers and values (constitution V; as 005 FR-020).
- **FR-022**: Import MUST accept a workbook holding a firearm sheet, an accessory sheet or both, or one or two single-table files picked together in one import (single-sheet workbooks or CSV files), recognizing each table by its header and producing one import report. Either table MAY be imported alone; without an accessory table, no accessories are imported or changed. Two files holding the same table, or a file whose header matches neither table, MUST stop the import before any row is imported, naming the file. On import, the record identifier and "mounted on" columns MUST be optional: a sheet without them imports as before. A row whose identifier matches an existing record of the same kind of record MUST be treated as a match for the conflict choice (skip, overwrite or create a duplicate, 001 FR-026), ahead of the make, model and serial number key (001 FR-030), which is compared only when the row has no identifier or its identifier matches nothing. For accessories the identifier is the only match. A record created from a row keeps the row's identifier, except a duplicate created by the conflict choice, which gets a new one. A row with no identifier gets a new one. A malformed identifier, one used by an earlier row of the same import, or one that belongs to a record of the other kind (an accessory row naming a firearm's identifier, or the reverse), is a row error (001 FR-020).
- **FR-023**: On import, "mounted on" MUST be resolved against the firearm rows of the same import and the firearms already in the database, by identifier. A mount that cannot be made (no such firearm, an item or host that is disposed, an accessory whose quantity is more than 1, a firearm that would end up mounted on itself through others) MUST NOT fail the row: the record is imported unmounted and the import report lists a warning naming the row and the reason. An overwritten record takes its row's mount, or none if the column is blank.
- **FR-024**: On import, an accessory row MUST be a row error when its kind is blank or not a kind the application knows (matched against every kind, including any no longer offered, ignoring letter case and surrounding whitespace), its quantity is not a whole number of at least 1, or an amount, date or text field breaks the rules of FR-003. Accessory text fields MUST be snapped to spellings on record as firearm fields are (004 FR-026), and the snapped values listed in the report.

**Data handling and performance**

- **FR-025**: Accessories and mounts MUST be stored only in the open encrypted database, like firearm records. They leave it only in the user's spreadsheet export (FR-021) and the encrypted backups of feature 003. This feature adds no network access.
- **FR-026**: With 10,000 firearms and 10,000 accessories, opening a record, mounting, moving and unmounting an item, disposing of a host and saving MUST complete within 1 second, and grouping and searching the Accessories page within 500 ms (constitution IV).

### Key Entities

- **Accessory**: a record of something the owner keeps with or on their firearms that is not itself a firearm. It has a kind, optional make, model, serial number, quantity, caliber, cartridge, estimated value, acquisition details and notes; a status with disposition details and retained disposition history as a firearm has; optional individual insurance scheduling; and a record identifier. Deleted completely with everything recorded on it.
- **Accessory Kind**: a fixed list shipped with the application (Optic, Light or laser, Magazine, Stock or brace, Upper receiver, Barrel, Muzzle device, Conversion kit, Mount or rail, Sling, Case, Other). Entries may stop being offered for new choices but are never removed or renamed.
- **Mount**: the current fact that one item (an accessory with quantity 1, or a firearm) is mounted on one host firearm. At most one per item; a host may have many. Exists only while both records are active; no history.
- **Record Identifier** *(from issue #53)*: a stable identifier on every firearm and accessory that holds across database files, used by the spreadsheet to match rows to records and to name a mount's host.
- **Firearm** *(extended)*: can be a host for any number of items, can itself be an item mounted on another firearm, and keeps its free-text Accessories field unchanged.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can record an accessory with kind, make, model, serial number and value, and mount it on a firearm, in under 1 minute.
- **SC-002**: Exporting a collection with accessories of every kind, scheduled and unscheduled, active and disposed, and mounts of both accessories and firearms, and importing it into an empty database that has the same insurance policies (by name: the spreadsheet names a record's policy, and policies themselves are not exported), reproduces every record, field, identifier and mount exactly, in both the workbook and the CSV form.
- **SC-003**: Importing a collection's own export back into it with "skip" chosen for every conflict creates no records, including for firearms with no serial number and for accessories.
- **SC-004**: After any sequence of mounting, moving, disposing, restoring and deleting, no item is mounted on more than one firearm, no mount involves a disposed or deleted record, and no firearm is mounted on itself through others.
- **SC-005**: The value summary's collection total always equals the sum of the estimated values of active firearms and active accessories.
- **SC-006**: After an accessory is deleted, none of its values can be found anywhere in the database, and a deleted host's former items are all still present and unmounted.
- **SC-007**: With 10,000 firearms and 10,000 accessories, the operations in FR-026 meet their budgets (actions within 1 s, search and grouping within 500 ms).

## Assumptions

- **Release timing (issue #50's 0.1.0 plan)**: the owner's plan builds this feature in 0.1.0 only if it changes existing tables or columns, and otherwise merges the spec on its own and builds it after 0.1.0. The free-text Accessories column is kept, so whether anything existing changes depends on the plan's design for mounts, insurance scheduling and dispositions of accessories, and is decided and written down in `data-model.md`.
- **Depends on issue #53**: the record identifier is #53's. This feature settles #53's spreadsheet question for firearms and accessories (FR-019, FR-022); #53 itself remains where the identifier's format and its other tables are decided.
- **No photos or documents on accessories in this feature**: an accessory has no photos, documents or thumbnail, and the Accessories page is a list with no tile view. A receipt or photo can be attached to the host firearm. Attachments on accessories are a possible later feature.
- **The host is always a firearm**: accessories are not mounted on accessories, so an optic on an upper is recorded on the firearm that the upper is on. This keeps every mount one step from a firearm.
- **Fits is not recorded**: compatibility, including a suppressor's list of hosts it fits or a modular item's configurations, goes in notes (clarification of 2026-10-01).
- **Record only**: the application does not judge whether a mount is safe, compatible or lawful (FR-011). A suppressor mounted on a firearm whose cartridge exceeds its rating is accepted without comment, like every other recorded value since 005.
- **Kinds**: the kind list covers the accessories owners commonly value or insure. Anything else is "Other", with its description in the model or notes. A kind has no rules attached (no fields that don't apply), so every kind has the same fields.
- **A suppressor stays a firearm** (005), so it is mounted as a firearm item, and its registration (005) stays on its firearm record.
- **Collection search is unchanged**: the Accessories page finds accessories, and each accessory shows its host, so a firearm need not be found through its items (FR-018).
- **The free-text field keeps its label**: "Accessories" names the field on the form and its section on the record page, unchanged; mounted accessories and firearms are listed in the separate Mounted section (FR-007, FR-013).
- **Insurance**: accessories are covered as firearms are, including individual scheduling, because owners commonly schedule expensive optics. All items are kept at one location (001 Assumptions), so the blanket policy in force covers accessories as it covers firearms.
- **Unreleased application**: schema changes are made in place while the application is unreleased (as 005's Assumptions), and databases from earlier development builds need not be migrated.

## Relationship to Other Features

This feature extends `specs/001-firearms-inventory/`. It **amends**:

- **FR-002** (notes and accessories): the free-text accessories list is kept unchanged, in its own section beside the new Mounted section (FR-007, FR-013).
- **FR-004, FR-023, FR-033** (disposition and its reversal): apply to accessories too (FR-006); disposing of a host can dispose of its items with it (FR-014).
- **FR-014 to FR-017, FR-024, FR-034, FR-036** (insurance, value summary, warnings, policy deletion): accessories count and are covered, scheduled and warned about as firearms are (FR-008, FR-009).
- **FR-018 to FR-020, FR-026, FR-030** (export, import, row errors, import matching): the accessory table, record identifier and "mounted on" columns; matching by identifier ahead of make, model and serial number, so firearms with no serial number can be matched (FR-020 to FR-024).
- **FR-025** (disposed records hidden by default): applies to the Accessories page (FR-016).
- **Key Entities**: Accessory, Accessory Kind and Mount are added; Firearm can host and be mounted.

It also amends:

- `specs/004-cartridges-action-types/`: the entry rules, suggestions, snapping and caliber derivation (FR-003, FR-005, FR-006, FR-009 to FR-015, FR-026) apply to accessory fields, and make suggestions are drawn from firearms and accessories together (FR-003).
- `specs/005-regulated-item-types/`: the clarification and edge case that a suppressor is not linked to the firearms it fits or is mounted on, and the Assumption "No links between records", are superseded for "mounted on" (FR-010); "fits" remains in notes. A receiver used with several uppers can now record each upper as an accessory (Edge Cases).
- Issue #53 (record identifiers): this feature uses its identifier in the spreadsheet and as the first import matching key (FR-019, FR-022).

## Source Request

This feature was filed as GitHub issue #50, "Link accessories such as suppressors to the firearms they fit or are mounted on" (labels: new feature, needs spec; milestone 0.1.0).

**The issue, verbatim:**

> ## Origin (2026-09-30)
>
> Raised while clarifying spec 005 (regulated item types, #12). Asked whether a suppressor record should be linked to the host firearms it fits or is mounted on, the owner chose to keep linking out of 005 and file it as its own feature, because it is not specific to suppressors or NFA items.
>
> Until then, 005 records no links: the owner uses notes, and grouping by caliber puts a suppressor next to the firearms of that bore.
>
> ## Questions for the spec
>
> - **What can be linked?** Suppressors to host firearms, and possibly other accessories (optics, magazines, stocks, uppers) if they become records. Are accessories firearm records, or a new kind of record?
> - **What does a link mean?** "Fits" (compatible), "mounted on" (current configuration), or both? Can one item be linked to many firearms, and is the link directional?
> - **Where does it show?** On both records, in search and grouping, and in the spreadsheet export and import (how would a link be written in one row per firearm?).
> - **Lifecycle:** what happens to links when either record is disposed, its disposition reversed, or deleted (constitution V: deleting removes the data).
> - **Registered configurations:** a short-barreled rifle registered as one receiver with several uppers overlaps with this; decide whether linking covers it.
> - **Performance:** grouping and search at 10,000 items within the existing budgets.

**The owner's 0.1.0 plan, from the issue's comment of 2026-10-01, verbatim:**

> In the 0.1.0 milestone because the answer to "are accessories firearm records, or a new kind of record?" may change the schema, including what happens to the existing free-text `firearms.accessories` column.
>
> Plan:
> 1. Run `/speckit-specify`, `/speckit-clarify` and `/speckit-plan` on the feature's own branch, as usual. The schema decision is written down in `data-model.md`.
> 2. **If the plan changes existing tables or columns** (e.g. accessories become records, or `firearms.accessories` is replaced), build it in 0.1.0, before migrations start to count (#24).
> 3. **If it only adds tables** (e.g. a link table, with `firearms.accessories` left alone), merge the spec on its own and build it after 0.1.0. An added table is the cheapest kind of migration. Earlier specs went from spec to implementation on one branch; a spec-only merge is new here, but nothing prevents it.

**Recorded decisions** (the decision recorded on the issue, from 005's clarification of 2026-09-30, and this spec's session of 2026-10-01), with where this spec answers each question:

- *Origin:* linking is not specific to suppressors or NFA items, so it is its own feature. → Items are accessories and firearms of any type (FR-010).
- *What can be linked?* → Accessories become a new kind of record; the free-text field is kept; a suppressor stays a firearm and can be mounted as one (FR-001, FR-007, FR-010; clarification of 2026-10-01).
- *What does a link mean?* → "Mounted on" only, directional from item to host firearm, at most one host per item, any number of items per host, current configuration only (FR-010, FR-013; clarification of 2026-10-01).
- *Where does it show?* → On both records (FR-013); the Accessories page groups by "Mounted on" and searches accessories (FR-017, FR-018); the collection's search and grouping are unchanged (Assumptions); the spreadsheet carries record identifiers and a "mounted on" column (FR-019 to FR-024; clarification of 2026-10-01).
- *Lifecycle:* → Disposing of either record ends the mount, and a host's items can be disposed of with it; reversal restores no mount; deletion removes mounts and never deletes the other record (FR-014, FR-015).
- *Registered configurations:* → Covered: the receiver is the firearm and keeps its registration, each upper is an Upper receiver accessory, one mounted at a time (Edge Cases).
- *Performance:* → FR-026, SC-007.
