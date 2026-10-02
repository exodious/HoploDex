# Feature Specification: Regulated Item Types: Suppressors and NFA Registration

**Feature Branch**: `005-regulated-item-types`

**Created**: 2026-09-30

**Status**: Draft

**Input**: User description: GitHub issue #12, "Regulated item types: suppressors and other NFA items". It asks for the collection to cover suppressors and the other items regulated under the U.S. National Firearms Act (NFA), while keeping what an item *is* separate from what the rules currently say about it, because whether each category still needs registration is in flux. The request, the issue's analysis and its open questions are reproduced under [Source Request](#source-request) so this spec stands on its own.

## Clarifications

### Session 2026-09-30

- Q: How far does the application go in stating legal status: record only, hint, or warn? → A: Record only. The application records what the owner states about a registration (what the item is registered as, the form, the approval date, who it is registered to) and never decides whether an item is regulated, needs registering or is lawful. It shows no hints and no warnings, and nothing acts on barrel or overall length. A short standing note in the registration section says so. This extends feature 002's stance (002 FR-006) from marks to registration, so a change in the law needs no change to the application (FR-014).
- Q: Is each NFA category a firearm type, or a classification separate from the type? → A: Separate. Only **Suppressor** becomes a new firearm type, since it is the one item that is physically unlike a handgun, rifle or shotgun; it gets its own drawing and no action type. Short-barreled rifle, short-barreled shotgun, any other weapon, machine gun and destructive device are values of an optional **Registered as** classification, which any firearm of any type can carry; a short-barreled rifle stays a Rifle. Suppressor is also a classification value, so type and classification never depend on each other (FR-001, FR-007, FR-008).
- Q: The application calls every record a "firearm". Should the wording change now that suppressors are included? → A: No. Under U.S. federal law a silencer and every NFA item is a "firearm" (18 U.S.C. 921(a)(3); 26 U.S.C. 5845(a)), so the term is accurate for everything this feature adds, and nothing is renamed.
- Q: How should a denied or withdrawn registration application be handled? → A: Pending applications are not tracked at all. There is no submitted date and no derived status (pending, approved or otherwise); a registration holds only the classification, the form, the approved date and "Registered to". An owner can record an item with no approved date, and an application's progress or outcome goes in notes (FR-009, FR-011).
- Q: Should a suppressor record be linked to the host firearms it fits or is mounted on? → A: No, out of scope for this feature. The owner uses notes, and grouping by caliber gathers a suppressor with the firearms of that bore. Linking accessories to firearms in general is filed as its own feature, issue #50.
  _Amended by [spec 006](../006-accessory-links/spec.md): superseded for "mounted on" by 006 FR-010: an accessory or firearm can be recorded as mounted on a firearm or an accessory. What a suppressor fits remains in notes._
- Q: Should "Registered to" also be a field the collection can be grouped by, alongside "Registered as"? → A: Yes. "Registered to" is a grouping field, with firearms that have no value (including those with no classification) under "Unspecified"; the form is not a grouping field. The collection page's grouping choice is getting crowded, so its UI is to be designed with the frontend design skill (FR-016, Assumptions).
- Q: Which ATF form names does the built-in suggestion list offer? → A: Only Form 1, Form 4 and Form 5 (Form 5 covers, for example, an inheritance). The application is for personal collections, so Forms 2, 3 and 10, which are for licensed dealers and manufacturers, are not offered. The field stays free text within 004's entry rules, so an owner who holds one may still type it, and forms already on record are still suggested.
- Q: On a suppressor, what do the caliber and cartridge fields record, and what are they called? → A: Caliber is the suppressor's bore diameter, labeled "Caliber" with the hint "The bore diameter."; the cartridge is the most powerful (generally highest-pressure) cartridge the suppressor is rated for, labeled "Rated cartridge" with the hint "The most powerful cartridge the suppressor is rated for." A suppressor of .22 bore may be rated only up to .22 WMR, and firing a .223 or 5.56x45mm through it would destroy it, so the two are recorded separately. This replaces the earlier "Caliber rating" label (FR-002).
- Q: When the owner picks a rated cartridge for a suppressor, is the caliber derived from it? → A: No. For a Suppressor the cartridge never fills in, guesses or suggests the caliber, on the form or on import; the owner enters the bore separately (a .46 suppressor rated for .300 Winchester Magnum would otherwise get ".30"). On import, a Suppressor row with a blank caliber is a row error even when a cartridge is given (FR-002, FR-022).
- Q: Is a suppressor's rated cartridge required? → A: No. It stays optional, as cartridge is for every type, and nothing prompts for it when it is blank; an owner who doesn't know an item's rating leaves it empty rather than guessing (FR-002).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Record a Suppressor (Priority: P1)

A collector adds a suppressor to the collection. They choose the type "Suppressor", enter its make, model and serial number, its bore (".30") as the caliber, and the most powerful cartridge it is rated for (".300 Winchester Magnum") as the rated cartridge. The form does not ask for an action type, barrel length or capacity, which a suppressor doesn't have. In the collection the suppressor shows its own drawing when it has no photo, and it groups under its own type.

**Why this priority**: Suppressors are the most common item in this feature's scope and the one the original request names. Today they can only be recorded as "Other" with a meaningless action choice and barrel length, so this story is the smallest change that makes them first-class records. It does not depend on registration details.

**Independent Test**: Create a Suppressor with make, model, serial number, caliber and rated cartridge; confirm the form offers no action, barrel length or capacity, the record saves, reopens intact, shows the suppressor drawing, groups under Suppressor, and is subject to the same serial number rules as any firearm.

**Acceptance Scenarios**:

1. **Given** the new-firearm form, **When** the user opens the type choice, **Then** "Suppressor" is offered alongside Handgun, Rifle, Shotgun and Other.
2. **Given** the type is Suppressor, **When** the user looks at the form, **Then** no action type, barrel length or capacity is offered; the caliber field keeps its "Caliber" label with the hint "The bore diameter.", and the cartridge field is labeled "Rated cartridge" with the hint "The most powerful cartridge the suppressor is rated for."; and overall length, weight, finish and condition are still offered.
3. **Given** a Suppressor with make, model, serial number, caliber ".22" and rated cartridge ".22 WMR", **When** the user saves and reopens it, **Then** all values are intact and the record page labels them "Caliber" and "Rated cartridge".
4. **Given** a Suppressor with no photo, **When** the user browses the list or tiles, **Then** it shows the suppressor drawing; **and When** they group by type, **Then** it is in a "Suppressor" group.
5. **Given** a Rifle with action "Bolt action", barrel length 20 in and capacity 5, **When** the user changes its type to Suppressor, **Then** the form says that the action, barrel length and capacity will be cleared, and they are cleared on save; **and When** a Suppressor's type is changed to Rifle, **Then** those fields are offered again, empty.
6. **Given** an active Suppressor with make "SilencerCo", model "Omega 300" and serial "ABC123", **When** the user saves another active firearm with the same make, model and serial, **Then** the save is blocked as for any firearm (001 FR-032).
7. **Given** a new Suppressor with an empty caliber, **When** the user picks the rated cartridge ".300 Winchester Magnum", **Then** the caliber stays empty and must be entered separately; **and Given** a saved Suppressor with caliber ".46", **When** its rated cartridge is changed, **Then** no caliber is suggested from it. A rated cartridge stays optional.

---

### User Story 2 - Record an Item's Registration (Priority: P2)

A collector owns a suppressor bought on an approved Form 4 and registered to their trust, and a rifle they made into a short-barreled rifle on a Form 1. On each record they choose what the item is registered as, then record the form, the date it was approved and who it is registered to. They attach the approved form as a document, as they would any other. The application never tells them whether an item needs registering.

**Why this priority**: Registration is the information that makes these items different to keep records of, and owners track it closely (approval dates, which trust holds what). It builds on User Story 1 for suppressors, but also works on its own for a Rifle registered as a short-barreled rifle.

**Independent Test**: On a Suppressor and on a Rifle, record a classification with each combination of registration details (none, some, all), save, reopen, confirm the values and that no status is derived from them, search and group by them, and clear a classification. Separately, confirm that no screen shows a hint or warning about legal status for items with and without a classification, including rifles and shotguns with short barrels.

**Acceptance Scenarios**:

1. **Given** any firearm, **When** the user opens its "Registered as" choice, **Then** it offers Suppressor, Short-barreled rifle, Short-barreled shotgun, Any other weapon, Machine gun and Destructive device, and leaving it blank is always allowed.
2. **Given** a firearm with no classification, **When** the user looks at the form, **Then** no registration details are offered; **and When** they choose a classification, **Then** the form offers the form, the approved date and "Registered to", all optional, with a standing note that HoploDex records what the user enters and does not decide what is regulated, and that laws change.
3. **Given** a Suppressor registered as "Suppressor" with form "Form 4", approved 2026-02-10 and registered to "Smith Family Trust", **When** the user saves and reopens it, **Then** the record page shows a Registration section with each value, the approved date formatted as elsewhere in the application ("Feb 10, 2026").
4. **Given** a Suppressor registered as "Suppressor" with form "Form 4" and no approved date, **When** the user saves and views it, **Then** the Registration section shows the classification and form only, with no status such as "pending" or "approved".
5. **Given** a classification chosen, **When** the user types in the form field, **Then** the suggestion list (004 FR-009) offers the built-in form names (Form 1, Form 4, Form 5) and forms already on record; **and When** they type in "Registered to", **Then** it offers the names already on record; any value within 004's entry rules may be kept as typed.
6. **Given** an approved date in the future, **When** the user saves, **Then** the save is blocked with a message on that field.
7. **Given** a firearm with a classification and registration details, **When** the user clears the classification, **Then** the user is told the registration details will be discarded and must confirm before the change is saved; **and When** they change it to another classification instead, **Then** the details are kept.
8. **Given** a Rifle with a 10.5 in barrel and no classification, and a Suppressor with no classification, **When** the user saves and browses them, **Then** both save normally and no screen shows any hint, warning or statement that either may be regulated or needs registering.
9. **Given** firearms registered as Suppressor, as Short-barreled rifle, and with no classification, **When** the user groups by "Registered as", **Then** each classification is a group and the firearms with none are in "Unspecified".
10. **Given** firearms registered to "Smith Family Trust", to "Jane Smith", and with no "Registered to" or no classification, **When** the user groups by "Registered to", **Then** each name is a group and the rest are in "Unspecified".
11. **Given** a firearm registered to "Smith Family Trust" on a "Form 1", **When** the user searches for "smith family", "form 1" or "short-barreled", **Then** the matching firearm is found.
12. **Given** a registered firearm, **When** the user marks it disposed, **Then** its registration details stay on the record; **and When** the firearm is deleted, **Then** its registration details are removed with it and its "Registered to" value is no longer suggested if no other firearm uses it.
13. **Given** the registration section, **When** the user opens the guide from it, **Then** the guide has worked examples for a suppressor on a Form 4 registered to a trust, and a rifle made into a short-barreled rifle on a Form 1.

---

### User Story 3 - Record a Firearm That Fires Automatically (Priority: P3)

A collector with a registered select-fire rifle records its action as "Automatic or select-fire" instead of semi-automatic, and records it as registered as a Machine gun. The two are separate: the action says what the firearm does, the classification says what the owner's paperwork says.

**Why this priority**: Feature 004 deferred automatic and select-fire firearms to this feature. It is a small change to the fixed action list, independent of the other stories.

**Independent Test**: Create a Rifle, a Handgun and a Shotgun with action "Automatic or select-fire", with and without a Machine gun classification; confirm each saves, groups and searches by the action, and that the action is not offered for a Suppressor.

**Acceptance Scenarios**:

1. **Given** a firearm of type Handgun, Rifle, Shotgun or Other, **When** the user opens the action choice, **Then** "Automatic or select-fire" is offered.
2. **Given** a Rifle with action "Automatic or select-fire" and no classification, **When** the user saves, **Then** it saves normally, with no prompt to add a classification.
3. **Given** a Rifle registered as Machine gun, **When** the user sets its action to "Semi-automatic", **Then** it saves normally: the application does not relate the action to the classification.

---

### User Story 4 - Suppressors and Registrations Through Export and Import (Priority: P4)

The collector exports the collection and sees the Suppressor type and the registration columns in the spreadsheet. The export warning names registration details among what leaves the database unencrypted. Importing that spreadsheet into an empty database recreates every record, and importing a sheet from another source reports each row whose registration or type-specific fields don't make sense.

**Why this priority**: Export and import complete the feature for users who keep a spreadsheet copy or move records in, but the collection is fully usable without them.

**Independent Test**: Export a collection with suppressors, classifications and every registration field, import it into an empty database and compare; import a sheet with the error cases below and check the report.

**Acceptance Scenarios**:

1. **Given** a collection with registered items, **When** the user exports, **Then** the spreadsheet has `registered_as`, `registration_form`, `registration_approved` and `registered_to` columns, blank where there is none, and a Suppressor's `firearm_type` is "Suppressor".
2. **Given** a collection where at least one exported record has registration details, **When** the export dialog states what leaves the encrypted database, **Then** it names registration details alongside serial numbers and values.
3. **Given** that export, **When** it is imported into an empty database, **Then** every type, classification and registration detail is reproduced.
4. **Given** an import row with `registered_as` "SHORT-BARRELED RIFLE", **When** it is imported, **Then** it matches "Short-barreled rifle"; **and When** the value is not on the list, **Then** the row is an error naming the problem.
5. **Given** an import row with registration details but a blank `registered_as`, **When** it is imported, **Then** the row is an error.
6. **Given** an import row of type Suppressor with an action type, barrel length or capacity, **When** it is imported, **Then** the row is an error naming the field.
7. **Given** an import row of type Suppressor with `cartridge` ".300 Winchester Magnum" and a blank `caliber`, **When** it is imported, **Then** the row is an error asking for the caliber, and no caliber is derived from the cartridge.
8. **Given** a spreadsheet exported before this feature, without the registration columns, **When** it is imported, **Then** the rows import with no classification.

---

### Edge Cases

- **An integrally suppressed firearm**: recorded as the firearm it is (e.g. a Rifle), with whatever classification the owner's paperwork gives it; the application does not decide it.
- **A type and a classification that seem not to match** (a Suppressor-type item registered as a Short-barreled rifle, or a Rifle registered as a Suppressor): accepted; type and classification are independent and the application does not judge them (FR-008).
- **A suppressor whose rating is unknown** (an older or inherited item): recorded with its caliber and no rated cartridge; nothing prompts for one.
- **Type changed to Suppressor after a cartridge filled in the caliber**: the caliber already on the form is kept as it stands; the owner checks that it is the bore (FR-002).
- **A suppressor with no classification**: valid. If suppressors are removed from registration, new suppressors are simply recorded without one.
- **A classification that a later version no longer offers** (for example after a change in the law): records holding it keep it and show it, and editing such a record still offers it so saving never forces a change; other records are not offered it (FR-007, FR-015).
- **A registered item that is transferred away**: the owner marks it disposed (001 FR-004); the registration details stay on the record as history. The new owner's registration is not tracked.
- **An item made by the owner on a Form 1**: the make is whatever the owner records (their name, trust or the maker of the host firearm); nothing about make or serial changes for registered items (FR-005).
- **A registered item recorded with the "no serial number" attestation**: accepted; the application does not judge whether it needs a serial (001 FR-029, 002 FR-006).
- **An application not yet approved, or denied or withdrawn**: not tracked. The owner may record the item with a classification and no approved date; nothing shows it as pending, and any progress or outcome goes in notes. An item never received is not added, or is disposed or deleted.
- **Changing type to Suppressor several times before saving**: fields to be cleared are checked against the type in effect at save, and each change that clears something is announced once.
- **Destructive devices with a large bore** (a 37mm or 40mm launcher, a 20mm rifle): recorded as Other or Rifle as appropriate, with the bore as the caliber ("37mm"); 004's caliber guess reads metric bores.
- **"Registered to" spellings**: same-notation variants (e.g. "smith family trust") snap to the spelling on record, as for make and model (004 FR-013).
- **Form 5320.20 (approval to move an item across state lines), local permits, or state registrations**: not tracked as fields; they can be attached as documents or put in notes.
- **Which firearms a suppressor fits or is mounted on**: not recorded as a link (out of scope, issue #50); the owner uses notes, and grouping by caliber gathers a suppressor with the firearms of that bore. A short-barreled rifle registered as one receiver used with several uppers is likewise one record, with the uppers in notes.
  _Amended by [spec 006](../006-accessory-links/spec.md): superseded for "mounted on" by 006 FR-010: an accessory or firearm can be recorded as mounted on a firearm or an accessory. What a suppressor fits remains in notes._
- **Grouping by type while suppressors exist**: Suppressor is its own group; a firearm's type and classification can be grouped by independently.

## Requirements *(mandatory)*

### Functional Requirements

**Suppressor type**

- **FR-001**: The seeded firearm types MUST include **Suppressor**, alongside Handgun, Rifle, Shotgun and Other (001 Assumptions). It MUST have its own generic drawing for records with no photos (001 FR-009), drawn for the project or under a recorded GPLv3-compatible license (constitution, Licensing).
- **FR-002**: Caliber MUST stay required for every type, including Suppressor (001 FR-001). For a Suppressor, the caliber is the suppressor's bore diameter and the cartridge is the most powerful (generally highest-pressure) cartridge it is rated for. The form and the record page MUST label them "Caliber" and "Rated cartridge", and the form MUST explain them with the hints "The bore diameter." and "The most powerful cartridge the suppressor is rated for." The cartridge (a Suppressor's rated cartridge) stays optional for every type, and nothing prompts for it when it is blank. For a Suppressor the caliber MUST NOT be derived from the cartridge: picking or typing a rated cartridge never fills in, guesses or suggests the caliber (004 FR-003, FR-005, FR-006 do not apply), and on import it is never derived (004 FR-025 does not apply; FR-022). Whether a type's caliber is derived from its cartridge is a property of the type, like the fields that don't apply to it (FR-003). A caliber already on the form when the type changes to or from Suppressor is kept as entered.
- **FR-003**: A firearm type MUST be able to mark the fields that don't apply to it. For Suppressor these are action type, barrel length and capacity: the form MUST NOT offer them, and a Suppressor MUST NOT be saved with any of them. This is distinct from a type with no action mapping, which offers every action (004 FR-017). Overall length, weight, finish and condition apply to every type.
- **FR-004**: When the user changes a firearm's type to one for which its recorded action type, barrel length or capacity doesn't apply, the system MUST show a notice on the form naming the values that will be cleared, and clear them on save (as 004 FR-019). Changing to a type where they apply clears nothing and offers them again.
- **FR-005**: Serial number, the "no serial number" attestation, the identifying key and uniqueness rules (001 FR-029 to FR-032; 002 FR-005, FR-008) MUST apply unchanged to every type, including Suppressor, and whether or not a classification is recorded.
- **FR-006**: The fixed action list (004 FR-018) MUST gain **Automatic or select-fire**, for a firearm that can fire more than one shot per pull of the trigger. It is allowed for Handgun, Rifle and Shotgun, and, as every action is, for Other and for types with no mapping. It describes what the firearm does, not its legal status, and is not linked to any classification (FR-008).

**Registration**

- **FR-007**: The system MUST allow the user to record an optional **Registered as** classification on a firearm of any type, chosen from a list that ships with the application: Suppressor, Short-barreled rifle, Short-barreled shotgun, Any other weapon, Machine gun, Destructive device. The user cannot add, rename or remove classifications. A later version MAY add entries or stop offering an entry for new choices, but MUST NOT remove or rename an entry that a saved record can hold; a record holding an entry no longer offered keeps it, shows it, and is still offered it when edited.
- **FR-008**: The firearm type and the classification MUST be independent. The system MUST NOT set, require, suggest or restrict the classification from the type, the action, the barrel or overall length or any other recorded value, nor any of those from the classification.
- **FR-009**: When a classification is recorded, the system MUST allow the user to record these **registration details**, all optional:
  - **Form**: free text within 004's entry rules (004 FR-015), with suggestions (004 FR-009 to FR-013) drawn from a built-in list of form names (Form 1, Form 4, Form 5) and the forms on record, and snapped to their spellings in the same way.
  - **Approved**: the date the application was approved, explained on the form as the date on the approved form (the tax stamp date).
  - **Registered to**: free text within 004's entry rules, with suggestions and snapping from the values on record only, e.g. the owner's name, a trust or a company.
  Without a classification the details MUST NOT be offered, and a record MUST NOT hold any.
- **FR-010**: The approved date MUST NOT be in the future (judged against the user's local date). A violation MUST block the save with a field-level message, and on import is a row error (001 FR-020).
- **FR-011**: The system MUST NOT track registration applications: it records no submitted date and derives or shows no registration status (pending, approved, denied or otherwise). The approved date is shown as a recorded detail, formatted as dates are elsewhere in the application.
- **FR-012**: Clearing the classification MUST discard the registration details, and the system MUST ask the user to confirm first when any detail is recorded (as 002 does when changing origin discards importer details). Changing to another classification MUST keep the details.
- **FR-013**: Registration details MUST be part of the firearm record: kept when the firearm is disposed or its disposition reversed, and removed completely when the firearm is deleted (constitution V). A "Registered to" or form value used only by deleted firearms MUST NOT be suggested (as 004 FR-011).
- **FR-014**: The system MUST NOT display, imply or act on any conclusion about whether an item is regulated, needs registering, is lawfully held or is correctly registered. It MUST show no hints or warnings about legal status, and it MUST NOT prompt for a classification or registration details. Nothing acts on barrel or overall length (001 FR-039). The registration section MUST show a standing note that HoploDex records what the user enters, does not decide what is regulated, and that laws change. The guide shipped with feature 002 (002 FR-015) MUST be reachable from the registration section and gain worked examples: a suppressor on a Form 4 registered to a trust, and a rifle made into a short-barreled rifle on a Form 1.
- **FR-015**: Saved classifications and registration details MUST NOT change unless the user edits them. Changes in the law, and changes to the shipped classification or form lists in later versions, never alter existing records.

**Browse and search**

- **FR-016**: "Registered as" and "Registered to" MUST be grouping fields (001 FR-012), with firearms that have no value grouped as "Unspecified" (for "Registered to", this includes firearms with no classification). The form is not a grouping field. Suppressor is a type like any other for grouping by type. The grouping choice MUST stay easy to scan with these added (see Assumptions, "Grouping choice design").
- **FR-017**: The classification name, form and "Registered to" MUST be included in search (001 FR-013).
- **FR-018**: The firearm record page MUST show a Registration section when a classification is recorded, with the classification and each recorded detail (FR-011). The browse list and tiles do not show registration details in this feature.

**Export and import**

- **FR-019**: The spreadsheet export MUST add `registered_as` (the classification's name), `registration_form`, `registration_approved` and `registered_to` columns, blank where there is no value. Their position is set in the spreadsheet contract.
- **FR-020**: When any exported record has registration details, the export dialog's statement of what leaves the encrypted database MUST name registration details alongside serial numbers and values (constitution V).
- **FR-021**: On import, the new columns MUST be optional; a sheet without them imports with no classification. `registered_as` MUST be matched against every classification the application knows, including entries no longer offered, ignoring letter case and surrounding whitespace; an unknown value is a row error. Registration details with a blank `registered_as` are a row error. `registration_approved` is checked against FR-010 and read as feature 001 reads dates. `registration_form` and `registered_to` are checked against 004's entry rules and snapped as make and model are (004 FR-026), with snapped values listed in the import report.
- **FR-022**: On import, a row whose type is Suppressor (or another type for which a field doesn't apply, FR-003) and which has a value in `action_type`, `barrel_length_in` or `capacity` MUST be a row error naming the field. A Suppressor row (or a row of another type whose caliber isn't derived, FR-002) with a blank `caliber` MUST be a row error, even when `cartridge` is given.

**Data handling**

- **FR-023**: Registration details MUST be stored only in the open encrypted database like every other firearm field. They leave it only in the user's spreadsheet export (FR-020) and the encrypted backups of feature 003. This feature adds no network access.

### Key Entities

- **Firearm Type** *(extended)*: gains the seeded **Suppressor** type with its own generic drawing, a record of which fields don't apply to a type (for Suppressor: action type, barrel length, capacity), and whether the type's caliber is derived from its cartridge (not for Suppressor, whose caliber is its bore and whose cartridge is the most powerful one it is rated for).
- **Action Type** *(extended)*: the fixed list gains "Automatic or select-fire", mapped to Handgun, Rifle and Shotgun.
- **Registration Classification**: a fixed list shipped with the application naming what an item is registered as (Suppressor, Short-barreled rifle, Short-barreled shotgun, Any other weapon, Machine gun, Destructive device). Each entry can be marked as no longer offered for new choices, but is never removed or renamed. It carries no rule about what is regulated.
- **Registration** *(on the Firearm)*: at most one per firearm, present only with a classification: the classification, and optionally the form, the approved date and who the item is registered to. Stored on the firearm record, owner-stated, never derived.
- **Built-in Form Name**: a short read-only list of form names shipped with the application, used only as suggestions for the form field, like 004's cartridge catalog. It is not stored in the database.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can record a registered suppressor (type, make, model, serial number, caliber, rated cartridge, classification, form, approved date and registered to) in under 2 minutes, and no field that doesn't apply to a suppressor is offered.
- **SC-002**: In a test collection covering every type with and without each classification, including rifles and shotguns with barrels under 16 and 18 inches, every record saves and no screen shows a hint, warning or statement about legal status.
- **SC-003**: Exporting a collection with suppressors, every classification and every registration field, and importing the file into an empty database, reproduces every type, classification and registration detail exactly.
- **SC-004**: A record holding a classification that the application no longer offers opens, displays, can be edited and saved, and still has its classification afterwards.
- **SC-005**: No firearm can be saved or imported as a Suppressor with an action type, barrel length or capacity, or with registration details and no classification.
- **SC-006**: With 10,000 firearms, grouping by classification or by "Registered to" and searching registration details meet the existing budgets (search within 500 ms, actions within 1 s).
- **SC-007**: After every firearm registered to a given name is deleted, that name appears in no suggestion list and cannot be found anywhere in the database.

## Assumptions

- **U.S. federal scope**: feature 001 scopes itself to U.S. federal norms, and the classifications are the categories of firearm defined by the National Firearms Act (26 U.S.C. 5845); "a weapon made from a rifle or shotgun" is recorded under the short-barreled rifle or shotgun. State and local registration, permits and bans are out of scope; documents and notes cover them.
- **Record only**: the application records what the owner states and makes no statement about legal status (clarification of 2026-09-30, FR-014). That is why the classification carries no "regulated" flag and nothing reads the barrel or overall length: the only regulatory knowledge in the application is the names of the classifications and the forms.
- **No first-use disclosure**: with nothing judged, a standing note in the registration section and the guide are enough; there is no first-use dialog.
- **"Firearm" stays the term** throughout, because every item this feature adds is a firearm in U.S. federal law (clarification of 2026-09-30).
- **Only Suppressor is a new type.** A 37/40mm launcher, a grenade or another item that is not a handgun, rifle, shotgun or suppressor is recorded as Other. A stripped frame or receiver, which is also a firearm in law, is out of scope as a type and is recorded as Other. The data model already allows more types; there is still no screen for adding them.
- **A suppressor's caliber is its bore, and its cartridge is its rating**: caliber stays required for every type, so grouping by caliber gathers suppressors with the firearms of that bore, and grouping by cartridge gathers a suppressor with the firearms chambered in its rated cartridge. Only the single most powerful rated cartridge is recorded; a manufacturer's list of other approved cartridges, or a modular suppressor's configurations, can go in notes.
- **"Automatic or select-fire" is an action, not a legal status**: it replaces 004's decision that automatic fire is not recorded. It is chosen instead of "Semi-automatic" for a firearm that can fire automatically.
- **No links between records**: a suppressor is not linked to the firearms it fits or is mounted on. Linking accessories to firearms is a separate, general feature (issue #50), not specific to NFA items.
  _Amended by [spec 006](../006-accessory-links/spec.md): superseded for "mounted on" by 006 FR-010: an accessory or firearm can be recorded as mounted on a firearm or an accessory. What a suppressor fits remains in notes._
- **One registration per record**: a firearm record belongs to one owner's collection, so it holds one registration. Earlier owners' registrations are not tracked; a transfer away is a disposition.
- **Which registration details are structured**: those that are worth grouping, searching or dating by. The approval document is attached like any document (001 FR-010), and anything else (a control number, a Form 5320.20, correspondence) goes in notes or attachments.
- **Insurance is unchanged**: a registered item is valued, scheduled or blanket-covered like any firearm (001 FR-014, FR-036). Some insurers ask for such items to be scheduled; the user does that through the existing coverage dialog.
- **Research for the plan (not legal advice, to be checked when planning)**: under the 2025 budget reconciliation act (Public Law 119-21), from 1 January 2026 the $200 tax on making or transferring a suppressor, short-barreled rifle, short-barreled shotgun or any other weapon is $0, while registration and approval continue and machine guns and destructive devices keep the $200 tax. Lawsuits challenge the registration requirement for the untaxed categories, and bills propose removing suppressors and short-barreled long guns from the Act. So the approved date is not called the "tax stamp date" (a $0 approval still carries a stamp), and nothing in the application depends on which categories are currently registered.
- **Grouping choice design**: with these two fields the collection page offers many things to group by, and its grouping choice is getting cluttered. The plan MUST design that control with the frontend design skill (`frontend-design`) rather than appending two more entries to the existing list, and carry the result to any other screen that offers the same choice (constitution III).
- **Unreleased application**: schema changes are made in place, and databases from earlier development builds need not be migrated.

## Relationship to Feature 001

This feature extends `specs/001-firearms-inventory/`. It **amends**:

- **Assumptions** ("Type of firearm is a structured field with an initial common set of values (e.g., Handgun, Rifle, Shotgun, Other)"): the seeded set gains Suppressor (FR-001).
- **FR-001** (record contents): caliber stays required for every type; for a Suppressor it is the bore, and the cartridge is labeled "Rated cartridge" (FR-002); a firearm may carry a classification and registration details (FR-007, FR-009).
- **FR-009** (generic thumbnails): Suppressor has its own drawing (FR-001).
- **FR-012** (grouping): "Registered as" and "Registered to" are grouping fields (FR-016).
- **FR-013** (search): classification, form and "Registered to" are searchable (FR-017).
- **FR-018 / FR-019 / FR-020** (export, import, row errors): the registration columns and their row errors (FR-019 to FR-022).
- **FR-039** (physical details "on any firearm"): barrel length and capacity don't apply to a Suppressor (FR-003); nothing acts on the lengths, which the session of 2026-09-20 had left to this feature (FR-014).
- **Key Entities / data model**: `FirearmType` gains Suppressor and the fields that don't apply to a type; a classification lookup and registration fields on `Firearm` are added; the full-text index covers classification, form and "Registered to".

It also amends:

- `specs/002-firearm-identification/`: FR-006's "records what the owner states" stance extends to registration (FR-014); the Assumption that "registration or tax-stamp detail belong to a planned feature for NFA and other regulated item types" is resolved here; the guide (002 FR-015) gains registration examples.
- `specs/004-cartridges-action-types/`: FR-003, FR-005, FR-006 and FR-025 (a Suppressor's caliber is never derived from its cartridge, FR-002); FR-017 (a type may have no action at all, distinct from no mapping; FR-003); FR-018 and the clarification that "automatic and select-fire are not recorded in this feature" ("Automatic or select-fire" is added, FR-006); FR-024 (a Suppressor row with an action is a row error, FR-022).

## Source Request

This feature was filed as GitHub issue #12, "Regulated item types: suppressors and other NFA items" (labels: new feature, needs spec). The issue suggested following the cartridge and action-type feature (#11, spec 004), since it extends that feature's type→action mapping.

**Amends or supersedes in 001 (as listed on the issue):** FR-001, FR-012, FirearmType (seeded Handgun/Rifle/Shotgun/Other, extensible), the generic thumbnails (FR-009), the spec's Assumptions on firearm types, and the type→action mapping from #11.

**The request, verbatim (2026-09-21):**

> Expand the types to cover items such as suppressors and other NFA items. Whether each currently NFA-registered category continues to require registration is in flux because of current events

**Guidance and questions recorded on the issue, with where this spec answers them:**

- *Settle first: keep regulatory status out of the code.* "Model the item's *type* (what it is) separately from any registration or legal status (what the rules currently say)… Decide how much the app should say about legality at all: recording what the *owner* says (e.g. 'registered', 'tax stamp date') is a much smaller commitment than the app deciding what is regulated. The scope should say so plainly, as feature 002 did for regulatory history." → Type and classification are independent (FR-008), the classification carries no rule (Key Entities), and the application records only (FR-014, clarification of 2026-09-30).
- *Candidate types (need research, not decided):* suppressors, short-barreled rifles and shotguns, any other weapons, machine guns, destructive devices, each a `FirearmType` "or a new classification" with its own thumbnail. → Suppressor is a type with its own drawing (FR-001); all six are classifications (FR-007).
- *Caliber (FR-001, required):* "a suppressor is rated for a range of calibers rather than chambered in one, and a destructive device has a bore diameter. This may need 'caliber rating' semantics, or an exemption from FR-001's required caliber for some types." → Caliber stays required and is a suppressor's bore; the cartridge records the most powerful cartridge it is rated for (FR-002, clarification of 2026-09-30); a destructive device records its bore (Edge Cases).
- *Serial number (FR-029/030/032):* "suppressors are serialized, so the uniqueness rule applies. But the rules for registered items and their markings differ by type and era, which overlaps feature 002." → The rules apply unchanged, and the application does not judge markings by era (FR-005, 002 FR-006).
- *"Firearm" wording:* "decide on neutral wording ('item' or 'record') and whether it's worth changing." → Unchanged (clarification of 2026-09-30).
- *Type→action mapping (#11):* "action type doesn't apply to a suppressor, so the mapping needs a 'no action type' case." → FR-003; also "Automatic or select-fire", which 004 deferred here (FR-006).
- *Candidate registration data (not decided):* "Approval or tax-stamp date, form number, trust or entity name, and the approval document. The document is already covered by the existing attachments (FR-010); the question is which fields deserve structure. These are sensitive, so any new field is subject to constitution V (what leaves the device, including the spreadsheet export, which is unencrypted)." → Form, approved date and "Registered to" (FR-009); applications and their status are not tracked (FR-011); the document stays an attachment; export disclosure and storage (FR-020, FR-023).
- *Dependencies:* the barrel and overall length fields (FR-039) "would feed any future 'this configuration may be regulated' hint, if one is wanted" → no hint is wanted (FR-014); the type→action mapping (FR-003, FR-006); feature 002's identity and marking rules (FR-005); insurance (FR-014, FR-036), "check whether such items are scheduled differently" → no change (Assumptions).
- *Open questions:*
  - "How far does the app go in stating legal status: record only, hint, or warn?" → Record only (FR-014).
  - "What happens to existing records when a type's status changes?" → Nothing: saved records keep what the owner recorded, and a classification is never removed from the list (FR-007, FR-015).
  - "Can users define their own classification, the way `FirearmType` already lets them add types?" → No. The classification is a fixed list updated with the application, like action type; since it carries no rule, an unexpected case goes under the nearest classification or in notes. (`FirearmType` is extensible in the data model, but no screen adds types.)
  - "Does any of this warrant a warning or disclosure at first use?" → No first-use dialog; a standing note in the registration section and the guide (FR-014, Assumptions).
