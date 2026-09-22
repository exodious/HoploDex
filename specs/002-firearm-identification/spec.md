# Feature Specification: Firearm Identification & Markings

**Feature Branch**: `002-firearm-identification`

**Created**: 2026-09-21

**Status**: Draft

**Input**: User description: "@spec_TODO.md let's start track B" — resolved to the first Track B item, **B1** (`002-firearm-identification`): how a firearm is identified and marked. Covers the two B1 items in `spec_TODO.md`: (1) the deduplication/validation rules for imported firearms, whose identifying marks depend on when and by whom they were imported (the importer may have marked its own name and assigned its own serial number, and possibly a model), and (2) recording the original manufacturer's marks (make, model, serial, sometimes year) in addition to the importer's.

## Clarifications

### Session 2026-09-21

- Q: When an importer adopts the original manufacturer's model and serial number and adds only its own name and location as a mark, what does the record need? → A: Nothing beyond the existing optional fields. The adopted model and serial are recorded as the main marks, the importer's name goes in the importer field, and the original-marks section stays empty. The application records the importer's name only, never its city, state, or other location (the user does not want addresses tracked), so importer location is removed from this feature.

- Q: When an imported firearm's main make, model and serial number match another active firearm, should the app block the save or warn and let the user confirm? → A: Block, with one exception: two imported firearms whose main marks are identical are accepted when the year of manufacture tells them apart, and in that case the year is required. Both records must have a year recorded and the years must differ; a missing year on either record, or the same year on both, is still blocked, and the message says that a year on each record is what would distinguish them. The year is not required otherwise. Warn-and-confirm was rejected, so a true duplicate can never be saved by a single confirmation. (Scope widened from imported firearms to every firearm in the next entry.)
- Q: Does the year-of-manufacture exception also cover domestic firearms made before the 1968 Gun Control Act? → A: Yes, and it applies to every firearm regardless of origin. Research found that before 1968 serial numbers had no legal meaning and makers commonly restarted numbering or ran overlapping series, and that wartime military contractors sometimes overran their assigned blocks, so the same serial appears twice; the year of manufacture is what tells such records apart. The rule is therefore: a duplicate main make + model + serial among active firearms is blocked unless both records have a year of manufacture and the years differ. There is no cut-off year, since the application does not judge legality by era (FR-006); the trade-off is that a post-1968 duplicate with a differing year is accepted, on the owner's word.
- Q: How should a firearm that was made in the U.S., exported, and later brought back be recorded, when the user may think of it as domestic? → A: Origin has a third choice, "Re-imported", described in the form as "Made in the U.S., exported, then brought back in", alongside "Domestic" and "Imported". It offers the same importer and original-marks fields as "Imported" (the original marks are the U.S. maker's), its country of manufacture is the United States and is not asked for, and the form gives a plain cue for choosing it: an importer's name stamped on a U.S.-made firearm means it was re-imported. The system does not decide which of the maker's or the importer's marks are the main ones, or whether any import was lawful or properly marked; the user records what is stamped on the firearm and what their paperwork says. The application also ships a short "how to record it" guide with worked examples, including a re-imported M1 Carbine.
- Q: Which of the new fields are searchable? → A: All of them. Origin, year of manufacture, country of manufacture, importer name, original maker, original model, and original serial number are all found by the same search as any other recorded information (feature 001 FR-013), matched against the value as displayed. That includes the United States shown for a re-imported firearm and the origin's name itself, so a search for "imported" also finds re-imported firearms (its label contains that word), while "re-imported" finds only those, and "domestic" finds domestic ones. A firearm with no origin has nothing to match on origin.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Record Where a Firearm Came From (Priority: P1)

A collector records that a firearm is imported (made abroad) or re-imported (made in the U.S., exported, then brought back), and captures the country it was made in and who imported it (the importer's name). They can also record the year the firearm was manufactured, for any firearm. Firearms recorded before this feature, and firearms where the collector does not care to say, simply show no origin.

**Why this priority**: The importer's identification is, for most imported firearms, part of how the firearm is legally recorded. Without a place for it, the record is incomplete for every imported firearm, and the later stories (original marks, duplicate rules) have nothing to hang on.

**Independent Test**: Create a firearm, mark it imported (and separately another re-imported), enter a country, an importer name, and a year of manufacture, save, reopen, and confirm all three persisted, are shown on the detail view, and are found by search — independent of original-maker marks, duplicate rules, or import/export.

**Acceptance Scenarios**:

1. **Given** a new firearm record, **When** the user leaves origin unset, **Then** the record saves exactly as before, with no origin shown and no importer or country fields offered.
2. **Given** a new firearm record, **When** the user sets its origin to "Imported", **Then** optional fields for country of manufacture and importer name are offered, and any or all may be left blank; **and When** the origin is instead "Re-imported", **Then** the importer name is offered but the country is not asked for and is shown as the United States.
3. **Given** an imported firearm with an importer name and country recorded, **When** the user reopens the record, **Then** those values are shown, labeled, on the detail view.
4. **Given** an imported firearm with importer "Example Arms Co." recorded, **When** the user searches for a word in the importer name or country, **Then** that firearm appears in the results; **and When** the user searches for "imported", **Then** both imported and re-imported firearms appear, **and When** they search "re-imported", **Then** only re-imported ones do, **and When** they search "domestic", **Then** only domestic ones do.
5. **Given** any firearm regardless of origin, **When** the user enters a year of manufacture of 1943, **Then** it is saved and displayed, and a search for "1943" finds it; **and When** the year is later than the current year, is not a four-digit year, or is earlier than 1400, **Then** the save is blocked with a message on that field.
6. **Given** a firearm with an import-marked origin and an importer recorded, **When** the user changes its origin to "Domestic" or clears it, **Then** the user is told the importer, country, and any original-maker marks will be discarded and must confirm before the change is saved.
7. **Given** a user with a U.S.-made M1 Carbine that was exported to another country, later brought back, and bears an importer's stamp, **When** they open the origin control, **Then** it lists "Re-imported: made in the U.S., exported, then brought back in", the guide's worked example describes this exact case, and choosing Domestic first shows a cue pointing to Re-imported; **and When** they choose Re-imported, enter the U.S. maker and its serial number as the main marks, and enter the importer's name, **Then** the record saves with no further prompt.
8. **Given** firearm records created before this feature existed, **When** the user opens them, **Then** all their existing data is intact, origin is shown as not specified, and nothing needs to be done to keep using them.

---

### User Story 2 - Record the Original Manufacturer's Marks (Priority: P2)

A collector who owns an imported or surplus firearm records the original manufacturer's marks — maker, model, and serial number — in addition to the identifying marks of the firearm's main record. For a firearm that carries an importer-assigned serial number, the main make/model/serial are what the paperwork identifies, and the original marks are kept alongside as supplementary information. For a firearm that carries only its original maker's serial number (as with some older surplus imports, or where the importer adopted the maker's model and serial and added only its own name), the main record simply holds those marks and nothing more needs to be entered.

**Why this priority**: It lets the record match both the importer's marking and what is stamped by the original maker, which is what a collector needs when researching provenance or dating a firearm. It builds on User Story 1 because original marks belong to imported firearms.

**Independent Test**: Create an imported firearm whose main serial number is the importer's, enter an original maker, model, and serial number, save, reopen, and confirm both sets of marks are shown and clearly labeled, and that a search for the original serial finds it — independent of duplicate rules and spreadsheet import/export.

**Acceptance Scenarios**:

1. **Given** an imported or re-imported firearm, **When** the user enters an original maker name, original model, and original serial number in addition to the main make, model, and serial number, **Then** all are saved and the detail view shows the two sets separately, labeled so it is unmistakable which set is the firearm's main identification and which are the original maker's marks.
2. **Given** an imported firearm, **When** the user leaves every original-maker mark blank, **Then** the record saves normally and no original-marks section is shown.
3. **Given** an imported firearm on which the importer adopted the original maker's model and serial number and added only its own name and location as a mark (e.g., a pistol made in Austria and stamped by its U.S. importer), **When** the user records the maker's model and serial as the main marks and the importer's name in the importer field, **Then** the record is complete and saves without error or prompt, with no original-marks entry, and no importer location is recorded.
4. **Given** a domestic or origin-unspecified firearm, **When** the user edits it, **Then** no original-maker mark fields are offered.
5. **Given** an imported firearm with an original serial number recorded, **When** the user searches for that serial number or for the original maker's name, **Then** the firearm appears in the results.
6. **Given** an imported firearm with only a partial set of original marks (e.g., a maker name but no model or serial), **When** the user saves, **Then** the partial set is accepted as entered.

---

### User Story 3 - Duplicate Checks That Respect How Imports Are Marked (Priority: P3)

A collector whose firearms include imports, re-imports, or old firearms with restarted serial series is not wrongly blocked from saving a legitimate record, and is not silently allowed to record the same firearm twice. Duplicate detection continues to treat a firearm's main make + model + serial number as its identifying key, but takes into account that imported firearms can carry two serial numbers, and that an original maker's serial number is not guaranteed unique across firearms.

**Why this priority**: The duplicate rule from feature 001 already works for simple cases and can keep shipping without this. This story refines it (year-of-manufacture exception, original-marks warning) so it neither over-blocks nor under-warns; it depends on the fields introduced in User Stories 1 and 2.

**Independent Test**: With a small set of imported and domestic firearms, save records that collide on main marks, on original marks only, and on neither, and confirm each gets the blocking, warning, or no reaction the rules call for — independent of spreadsheet import/export.

**Acceptance Scenarios**:

1. **Given** an active firearm with a serial number and no year of manufacture, **When** the user saves another firearm (of any origin) with the same make, model, and serial number, **Then** the save is blocked, the existing record is named, and the message says a year of manufacture on each record is what would distinguish them.
2. **Given** two firearms with identical main make, model, and serial number, **When** the first has a year of manufacture of 1943 and the user saves the second with 1944, **Then** it is accepted; **and When** the second is instead saved with 1943, or with no year, **Then** the save is blocked as in the previous scenario.
3. **Given** an active pre-1968 domestic revolver whose maker restarted its serial numbering, **When** the user saves a second of the same make, model, and serial number, with a different year of manufacture on each record, **Then** both are kept.
4. **Given** two firearms with identical main marks and differing years, **When** the user edits the second to the same year as the first, **Then** the save is blocked; **and When** a disposed firearm is later restored and would then collide with an active one that does not differ by year, **Then** the reversal is blocked with the same message.
5. **Given** an active imported or re-imported firearm with original marks (maker, model, and serial all recorded), **When** the user saves another firearm whose original maker, model, and serial number match (ignoring letter case and surrounding whitespace), **Then** a warning names the existing record and asks the user to confirm before saving; and **When** the user confirms, **Then** the record saves.
6. **Given** two imported or re-imported firearms with the same original maker, model, and serial number but different main serial numbers assigned by the importer, **When** the user saves the second, **Then** it is not blocked by the main-marks rule, and only the FR-009 warning applies.
7. **Given** a disposed firearm whose original marks match those of a firearm being saved, **When** the user saves, **Then** no warning is shown.
8. **Given** an imported firearm with only a partial set of original marks (e.g., maker and model but no serial), **When** the user saves another firearm with the same partial set, **Then** no original-marks warning is shown, since a serial number is what makes a match meaningful.
9. **Given** a disposed firearm whose original marks match an active firearm's, **When** the user reverses the disposition, **Then** the same warning is shown and the user may confirm to proceed.

---

### User Story 4 - Carry Identification Through Browse, Export, and Import (Priority: P4)

A collector can group the collection by origin, and can export imported firearms with all their identification details and re-import them without loss. Rows in a spreadsheet prepared elsewhere that carry importer and original-marks details are read the same way.

**Why this priority**: Keeps the new details usable everywhere a firearm's data already appears, but the details are useful on their own before this is in place.

**Independent Test**: Export a collection containing domestic, imported, and origin-unspecified firearms, re-import the file into an empty collection, and confirm every new field is identical; then import a hand-edited file with deliberately invalid values and confirm the row-level errors.

**Acceptance Scenarios**:

1. **Given** a collection with domestic, imported, and origin-unspecified firearms, **When** the user groups by origin, **Then** firearms are organized into groups for each origin, with a distinct group for those with none.
2. **Given** a collection containing imported firearms with importer, country, year, and original marks, **When** the user exports it and later imports the file into an empty collection, **Then** every one of those values is present and identical on the re-imported records.
3. **Given** an import file where a row gives an origin other than "Domestic" or "Imported", **When** the user imports it, **Then** that row fails with a message identifying the value, and the other rows still import.
4. **Given** an import file where a row that is not import-marked carries importer, country, or original-marks values, or a re-imported row carries a country, **When** the user imports it, **Then** that row fails with a message saying which value the origin does not allow.
5. **Given** an import row whose main make, model, and serial number match an existing active record (of any origin) and are not distinguished by year of manufacture (FR-008), **When** the user imports it, **Then** the conflict prompt of feature 001 appears with only skip and overwrite offered, and matching never uses the original-maker marks.
5a. **Given** an import row and an existing active firearm with identical main marks, **When** both have a year of manufacture and the years differ, **Then** the row is treated as a new record with no conflict prompt.
6. **Given** an import row whose original maker, model, and serial match those of an existing active firearm, **When** the user imports it, **Then** the row still imports, and the import report lists it as a warning naming the existing record.
7. **Given** an import row with a year of manufacture in the future, or not a four-digit year, **When** the user imports it, **Then** that row fails with a message identifying the field.

---

### Edge Cases

- What if the importer recorded on a firearm is not a person or company the user can name (e.g., they only know it was imported in the 1960s)? All importer details are optional, so the user may record origin "Imported" and nothing else.
- What if a firearm is imported *and* its main serial number is the original maker's (no importer serial)? The user records the main marks as they appear on their paperwork; the original-marks section is for the case where the two differ or both are worth keeping.
- What if the same importer name is spelled two ways across records ("Example Arms" vs. "Example Arms Co.")? Names are stored as entered; consolidating spellings is a suggestion-list matter for a later feature (003), not this one.
- What if the year of manufacture is uncertain (e.g., "circa 1943")? Only a single four-digit year is accepted; uncertainty belongs in the free-form notes (feature 001 FR-002).
- What happens to an imported firearm's importer and original-marks data when the firearm is disposed of and later restored (feature 001 FR-033)? It is unaffected; disposal and restoration never change identification data.
- What happens when a hand-edited import row sets origin to imported but leaves every importer, country, and original-marks column blank? It imports normally; all of those are optional.
- What if two firearms share a main serial number and make/model, and the collector knows they are distinct firearms (e.g., surplus rifles from different arsenals whose serial numbers restart each year)? Handled by FR-008: if each has a year of manufacture and the years differ, both may be recorded (this covers pre-1968 domestic firearms too); otherwise the year is what the user must add.
- What if a user with a re-imported firearm calls it domestic and enters no importer? The record is valid; origin is optional and the app never overrides it. The cue and the guide (FR-015) are what steer them, and changing the origin later carries no penalty.
- What if a re-imported firearm has a new serial number the importer assigned, with the U.S. maker's original serial still stamped on it? The user records the serial their paperwork uses as the main serial and the other as the original serial (FR-005).
- What if the importer's stamp is unreadable, or the user cannot tell which mark is the importer's? All importer details are optional; the user may choose Re-imported or Imported and leave the name blank, or leave origin unset.
- What if wartime military duplicates exist (two makers' firearms sharing a serial number, sometimes with an "X" added to one)? The make differs, so no collision arises; the guide tells users to record the actual manufacturer as the make (e.g., "Inland", not the generic government nomenclature) and any suffix as part of the serial exactly as stamped.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST allow the user to record an optional origin for a firearm, chosen from a fixed list of "Domestic", "Imported" (made abroad and brought in), and "Re-imported" (made in the U.S., exported, then brought back in); a blank origin means not specified and MUST always be allowed. "Imported" and "Re-imported" are together called import-marked origins in this spec.
- **FR-002**: When a firearm's origin is "Imported", System MUST allow the user to record, all optionally, a country of manufacture and the importer's name. When it is "Re-imported", System MUST allow the importer's name only, and treat the country of manufacture as the United States (shown as such, not asked for). The system MUST NOT record the importer's city, state, or other location. These fields MUST NOT be offered for a domestic or origin-unspecified firearm.
- **FR-003**: System MUST allow the user to record an optional year of manufacture on any firearm, regardless of origin; it is required only in the case FR-008 describes, and the system does not otherwise judge a firearm by its year (FR-006). It MUST be a whole four-digit year, not earlier than 1400 and not later than the current year (judged against the user's local date); anything else MUST be rejected with a field-level message.
- **FR-004**: When a firearm's origin is an import-marked origin, System MUST allow the user to record, all optionally, the original manufacturer's name, model, and serial number as marked by the original maker, in addition to the firearm's main make, model, and serial number. For a re-imported firearm these are the U.S. maker's marks. These MUST NOT be offered for a domestic or origin-unspecified firearm. A partial set MUST be accepted as entered.
- **FR-005**: The firearm's main make, model, and serial number (feature 001 FR-001, FR-029, FR-030) MUST remain the marks by which the firearm is identified, and the "no serial number" attestation (feature 001 FR-029) applies to the main serial number only. The user decides which marks (the importer's or the original maker's) to record as the main marks, according to their paperwork; the system MUST NOT decide, infer, or require which set is "the" legal identifier. Original-maker marks are supplementary and MUST NOT participate in the identifying key or in import matching (feature 001 FR-026).
- **FR-006**: System MUST NOT validate, require, or reject any identifying mark on the basis of a firearm's manufacture year, import date, or regulatory history, and MUST NOT display any conclusion about the firearm's legal status. The system records what the owner states; regulatory rules that vary by era are out of scope.
- **FR-007**: System MUST block saving a firearm whose main make + model + serial number matches another active firearm (ignoring letter case and surrounding whitespace), naming the conflicting record, for a firearm of any origin, unless FR-008 applies. This amends feature 001 FR-032 by adding the year-of-manufacture exception; everything else in FR-032 (active firearms only, no-serial records never compared) stands.
- **FR-008**: A match under FR-007 MUST be accepted when both records have a year of manufacture recorded (FR-003) and the years differ. When a year is missing on either record or the years are equal, the save MUST stay blocked and the message MUST say that a year of manufacture on each record is what would distinguish the two firearms. A year of manufacture is otherwise never required. There is no cut-off year and the origin of the two firearms is irrelevant. The rule MUST be applied on create, edit, import (feature 001 FR-020, as a row error), and when reversing a disposition (feature 001 FR-033); matches against disposed firearms are ignored (feature 001 FR-032).
- **FR-009**: When a firearm has an original maker, original model, and original serial number all recorded and another active firearm has the same three values (ignoring letter case and surrounding whitespace), System MUST show a non-blocking warning naming the other record and MUST let the user confirm and save. A partial set of original marks MUST NOT trigger the warning, and disposed firearms MUST NOT be considered. The check MUST run on create, edit, import (reported as a warning in the import report, without failing the row, feature 001 FR-020), and when reversing a disposition (feature 001 FR-033).
- **FR-010**: When the user changes a firearm's origin so that recorded importer, country, or original-marks values would no longer be offered (from an import-marked origin to Domestic or none, or the country when changing from Imported to Re-imported), System MUST tell the user those values will be discarded and require confirmation before saving; importer and original-marks values carry over unchanged between Imported and Re-imported without a prompt, using the same confirmation pattern as other destructive actions.
- **FR-011**: Firearm records that exist without origin information MUST remain fully valid and usable, showing origin as not specified, with no user action required.
- **FR-012**: Every field this feature adds (origin, year of manufacture, country of manufacture, importer name, original maker, original model, and original serial number) MUST be included in search (feature 001 FR-013), matched against the value as it is displayed: this includes the United States shown for a re-imported firearm and the origin's name, so "imported" also finds re-imported firearms, "re-imported" finds only those, and a firearm with no origin has nothing to match on origin. Origin MUST be available as a grouping field (feature 001 FR-012), with one group per origin and a distinct group for firearms with no origin. Year of manufacture is searchable but not a grouping field.
- **FR-013**: The firearm detail view MUST show each recorded identification detail with a label, and MUST present the original-maker marks separately and clearly labeled from the main make, model, and serial number. List and tile views (feature 001 FR-011) MUST continue to show only the main make, model, and nickname.
- **FR-014**: Export and import (feature 001 FR-018, FR-019) MUST carry origin, country of manufacture, importer name, original maker, original model, original serial number, and year of manufacture, blank allowed for each. On import, an origin other than "Domestic", "Imported", or "Re-imported" (matched ignoring letter case), a year of manufacture that fails FR-003, or importer, country, or original-marks values on a row whose origin does not offer them (country of manufacture is also an error on a "Re-imported" row) MUST be a row error (feature 001 FR-020). Import matching MUST continue to key only on the main make, model, and serial number, plus, per FR-008, the year of manufacture where both records have one (feature 001 FR-026, FR-030); original-maker marks are never used to match. The import conflict prompt's "create a duplicate" option MUST be offered only where the FR-007 or FR-008 outcome would allow the resulting record to be saved, which for two identical-mark records means it is never offered, since FR-008 accepts a record whose year differs without any prompt.

- **FR-015**: The origin control MUST describe each choice in plain words beside it ("Domestic: made in the U.S."; "Imported: made abroad and brought in"; "Re-imported: made in the U.S., exported, then brought back in"; leaving it blank is always fine if the user is unsure). When the user has chosen Domestic, the form MUST show a one-line cue that a U.S.-made firearm carrying an importer's name was re-imported. The application MUST ship a short user-facing guide, reachable from the origin control, with worked examples of which fields to fill in (see the research note in Assumptions for the cases it covers). This is guidance only: the system MUST NOT block, reject, or second-guess the user's choice of origin.

### Key Entities

- **Firearm** *(extended from feature 001)*: gains an optional origin (Domestic, Imported, or Re-imported), an optional year of manufacture, and, when import-marked, optional country of manufacture, optional importer identification, and optional original-maker marks. The main make, model, and serial number remain the firearm's identifying key.
- **Importer identification**: the name of the party that imported the firearm (no address or location). Belongs to one imported or re-imported firearm; it is not shared or looked up across firearms.
- **Original-maker marks**: the original manufacturer's name, model, and serial number as stamped by the maker, kept in addition to the main marks. Belongs to one imported or re-imported firearm and is supplementary: it takes no part in the identifying key or in import matching, only in the FR-009 warning.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can record an imported firearm's origin, country, importer, and original-maker marks in under 3 minutes on top of the core record.
- **SC-002**: 100% of firearm records that carry both importer-assigned and original-maker marks display the two sets separately and labeled, so a reader can never mistake original-maker marks for the firearm's main identification.
- **SC-003**: 100% of attempts to save two imported firearms that share original-maker marks but have different main serial numbers succeed without being blocked, and 100% of them show the FR-009 warning when original make, model, and serial number are all recorded.
- **SC-004**: 100% of firearm records created before this feature remain fully usable with all their data intact and no user action.
- **SC-005**: An export followed by an import into an empty collection reproduces every new field on every firearm with no loss.
- **SC-006**: A user can find an imported firearm in a collection of at least 500 records by any one of its new details (importer name, country, origin, year of manufacture, original maker, original model, or original serial number) in a single search action.
- **SC-007**: In a first-use check, a user given the worked example of a re-imported M1 Carbine records it as Re-imported with the importer's name and the maker's serial number in under 2 minutes, without help beyond what the form and the linked guide show.
- **SC-008**: Two firearms of any origin with identical main make, model, and serial number and differing years of manufacture can both be saved, and an identical pair without differing years can never be saved, in 100% of tested create, edit, import, and restore cases.

## Assumptions

- **Regulatory scope (states the answer 001 and `spec_TODO.md` ask for):** feature 001 scopes itself to U.S. federal norms and, in this feature, that scope does not deepen. The system records identification details the owner supplies and makes no claim about what any era's rules required of a manufacturer or importer (FR-006). Importer marking requirements have changed over time; pre-1968 surplus imports often carry only the original maker's marks, later imports carry importer marks and an importer-assigned serial number, and whether an importer must also assign a model needs to be checked against the regulations. None of this changes the data captured here, because both the main model and the original model are recordable, so it does not block the spec.
- Origin has three values because they change what marks exist: Domestic (one set of maker marks), Imported (a foreign maker's marks, plus possibly an importer's), and Re-imported (a U.S. maker's marks, plus an importer's). Homemade or unserialized firearms are already covered by the "no serial number" attestation (feature 001 FR-029) and need no origin of their own.
- Country of manufacture is free text and the importer is identified by name only; the importer's city and state are deliberately not recorded. A picklist of countries, suggested importer names, and consolidation of variant spellings belong to the suggestion mechanism in feature 003 (classification vocabularies), not here.
- Year of manufacture is a single year on any firearm, not restricted to imports, because collectors record it for domestic firearms too. The lower bound of 1400 is a sanity check, not a claim about firearm history.
- **Research on duplicates and re-imports** (for the plan; not legal advice): before the 1968 Gun Control Act serial numbers had no legal meaning, and makers commonly restarted numbering or ran separate series, so identical make + model + serial pairs exist among pre-1968 U.S. firearms; wartime contractors also sometimes overran their assigned blocks and stamped another contractor's numbers (M1 Carbines by different makers, one with an "X" added). ATF Ruling 2013-3 lets a licensed importer adopt the serial number, caliber and model already on a firearm, provided it adds its own name and city and state and does not remove or alter the original serial number; where an importer receives two firearms with the same serial number it adds letters, numbers or a hyphen to it. The ruling does not distinguish foreign-made from U.S.-made firearms brought back, and many re-imported carbines and rifles bear an importer's stamp. The application records what is stamped, never whether it was permitted.
- Original-maker marks are offered only for imported and re-imported firearms, since a domestic firearm has only one set of maker marks and those are its main marks.
- The system does not track the date of import, the importer's federal licence number, or any registration or tax-stamp detail. Those belong to the regulated-item-types feature (B8 in `spec_TODO.md`), not this one.
- Nothing needs to be migrated for existing records: all new fields are optional and the application is unreleased (0.1.0), so an existing development database can be recreated as with earlier schema changes.
- Physical details of a firearm (barrel length, weight, capacity, finish, condition) and cartridge/action type are covered elsewhere (feature 001 FR-039 and feature 003) and are not part of this feature.

## Relationship to Feature 001

This feature is an extension of `specs/001-firearms-inventory/`. It **amends** the following, and supersedes none:

- **FR-001** and Key Entities (Firearm): adds the optional origin, year of manufacture, importer, country, and original-maker fields.
- **FR-030** (identifying key): restates that the main make, model, and serial number are the key and that original-maker marks are supplementary (FR-005 here).
- **FR-032** (uniqueness): a duplicate is still blocked but is accepted when both records have differing years of manufacture, for any origin (FR-007 and FR-008 here); adds the non-blocking original-marks warning (FR-009 here).
- **FR-026** (import matching) and **FR-019/FR-020**: matching stays on the main marks (original-maker marks are never used), with the year joining where both records have one; new columns and row errors for the new fields; the "create a duplicate" gate follows FR-007/FR-008 here.
- **FR-033** (reverse disposition): the reversal also runs the FR-009 warning.
- **FR-012** and **FR-013**: origin becomes a grouping field; every new field becomes searchable.
- **FR-018** and the spreadsheet contract, plus the data model's Firearm identity columns and search index: gain the new fields.
