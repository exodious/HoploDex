# Feature Specification: Firearms Collection Inventory

**Feature Branch**: `001-firearms-inventory`

**Created**: 2026-07-20

**Status**: Draft

**Input**: User description: "Build a cross-platform desktop application that can help me keep detailed records of my firearms collection. These records include identifying information such as make/model/serial number, caliber (e.g. 9mm), type of firearm (e.g. handgun), accessories, and other such information that the user may want to store (e.g. there's a scratch on one side, the handle is cracked, etc). Records also include acquisition and disposition information (e.g. from whom was it purchased and at what price, to whom was it sold and at what price). Records may also include estimated replacement value (for insurance or other purposes). Records may also include photographs. Records may also include insurance policy information, including whether the firearm is individually scheduled or is covered by a blanket policy. Records may also include arbitrary document attachements such as PDF files. The firearms should be groupable by type, caliber, make, etc. - any relevant non-free-form field. The firearms should be searchable by any recorded information. A value summary should be computed and displayed to the user for various purposes, e.g. so that they can decide what limits to place on their insurance policies, if any. A warning shall be displayed to the user whenever the firearms are un- or under-insured. The firearms should be browsable in either list or tiled form, with an automatically generated thumbnail image for each firearm if the firearm has photos stored with it. Either the first photo added to a firearm or one the user explicitly selects is used for a thumbnail. If a photo is not stored, then a generic thumbnail image per firearm type shall be used instead. The firearms records are to be exportable to a well known data format such as a CSV, Excel, or LibreOffice spreadsheet, as well as any photographs in their original format. Firearms records (without photos) can be imported from the same well known data formats. The application shall store all data locally, not in a cloud."

## Clarifications

### Session 2026-07-20

- Q: How should insurance policies be modeled so multiple firearms can share one coverage limit for the under-insured calculation (FR-017/FR-024)? → A: Insurance policies are a distinct entity the user creates/names, capturing the insurance company, policy ID/number, insurance agent, contact information for both the company and the agent, a blanket coverage limit, and the policy's effective dates. Firearms are assigned to a policy either as individually-scheduled (their own scheduled coverage amount under that policy) or as blanket-covered (drawing from the policy's shared limit) — a single policy may have both individually-scheduled and blanket-covered firearms assigned to it at once, since both are riders/coverage on the same real-world policy. All policies, not just blanket ones, expire: the system MUST warn when a policy is within 30 days of its expiration date, and MUST treat any firearm assigned to an expired policy as uninsured regardless of whether it was individually scheduled or blanket-covered.
- Q: What role does serial number play in firearm identity and import deduplication (FR-026)? → A: Serial number alone cannot be used for deduplication because it is only unique per make/model, not across the whole collection. Serial number is also not always present: homemade firearms and firearms manufactured before the U.S. Gun Control Act of 1968 (effective October 22, 1968) are not legally required to have one. Serial number is therefore optional on a firearm record; when present, the make + model + serial-number combination is the closest available identifying key for matching, and records with no serial number cannot be automatically matched during import — they are always treated as new. An empty serial number field alone is not sufficient to mark a firearm as serial-less: the user must explicitly attest (e.g., via a dedicated checkbox) that no serial number is present or required, so blank entries are not silently mistaken for a legal exemption. Federal law (the Gun Control Act of 1968) is the general norm applied; state/local serialization requirements, and non-U.S. laws, are out of scope.
- Q: Should the auto-recalculated value summary (FR-015) break down by insurance policy, or show only a single collection-wide total? → A: The value summary breaks down by insurance policy — showing each policy's blanket-covered total against its coverage limit, plus each individually-scheduled firearm's value against its own scheduled amount — in addition to the overall collection-wide grand total, since that breakdown is what lets the user decide what limits to set on each policy.

### Session 2026-09-19

- Q: How do users tell apart multiple firearms with identical make and model (and possibly caliber)? → A: Add an optional, user-assigned nickname (free-form text) to each firearm, purely to help the user visually tell records apart. It is not part of the make + model + serial key (FR-030) and plays no role in import matching. Unlike make/model, it MUST be unique among active firearms (compared ignoring letter case and surrounding whitespace), because users do not reuse a nickname for different items; a disposed firearm releases its nickname, so a reacquired firearm can take its old nickname back. It is displayed alongside make and model wherever a firearm is shown or named (list, tile, detail, value summary, insurance warnings) and is searchable like any other recorded field. It is free-form, so it is not a grouping field (FR-012).
- Q: Must make + model + serial number be unique across firearm records, and is a duplicate blocked or only warned about? → A: It depends on the serial-number attestation (FR-029). A record that has a serial number and is NOT attested as exempt (i.e., subject to GCA 1968 serialization) MUST NOT duplicate the make + model + serial of another active firearm: the save is blocked. A record marked exempt (homemade or pre-GCA 1968) that nevertheless records a serial number gets only a warning on a match, because serials were not required to be unique before the Act; the save is allowed. A match against a disposed record never blocks or warns, since a disposed firearm may be reacquired as a new record. Records with no serial number are never compared. Comparison ignores letter case and surrounding whitespace. The rule applies on create, edit, import, and when reversing a disposition.
- Q: Can a firearm mistakenly marked as disposed be restored to active, and what happens to its disposition details? → A: Yes. The user can reverse a disposition on a disposed firearm; this is a targeted correction, not a general undo system (a broader change-history/undo feature is out of scope here). Reversing MUST ask the user what to do with the recorded disposition details: keep them as retained history on the record, or discard them permanently. Either way the firearm returns to active status with no current disposition. Reversal is blocked if the restored record would violate nickname or make + model + serial uniqueness (FR-031, FR-032) against a firearm that is active at that time; the user resolves the clash first (rename, or change/dispose/delete the other record).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Record a Firearm (Priority: P1)

A collector adds a new firearm to their inventory, capturing its identifying details (make, model, serial number, caliber, type), condition notes, accessories, how it was acquired (source, date, price), and its estimated replacement value. They can later edit that record or mark the firearm as disposed of (sold, traded, gifted, destroyed, or lost) with recipient, date, and price.

**Why this priority**: Without the ability to create and maintain a single accurate record, no other capability (browsing, valuation, insurance warnings, export) has any data to operate on. This is the minimum viable product.

**Independent Test**: Can be fully tested by adding a new firearm record with all core fields, saving it, reopening it to confirm the data persisted, editing a field, and marking it disposed — independent of search, photos, or export.

**Acceptance Scenarios**:

1. **Given** an empty collection, **When** the user creates a new firearm record with make, model, serial number, caliber, and type, **Then** the record is saved and appears in the collection with those values intact.
2. **Given** an existing firearm record, **When** the user edits a field (e.g., adds a condition note about a scratch), **Then** the updated value is saved and displayed on the record.
3. **Given** an existing firearm record, **When** the user records acquisition details (source, date, price), **Then** those details are saved and shown on the record.
4. **Given** an existing firearm record, **When** the user marks it as disposed (recipient, date, price, disposition type), **Then** the record reflects its disposed status while retaining its full history.
5. **Given** an existing firearm record, **When** the user deletes it, **Then** the record and any data uniquely associated with it are removed from the collection after confirmation.
6. **Given** a homemade or pre-1968 firearm with no serial number, **When** the user explicitly attests it has no serial number and saves the record, **Then** the record is saved without a serial number and without any validation error.
7. **Given** a new firearm record with an empty serial number field, **When** the user attempts to save it without explicitly attesting that no serial number is present, **Then** the system blocks the save and prompts the user to either enter a serial number or confirm the attestation.
8. **Given** two firearms with identical make, model, and caliber, **When** the user gives each a different nickname, **Then** both nicknames are shown alongside make and model in list, tile, and detail views, so the two records are visually distinguishable; and **When** the user leaves the nickname blank on a record, **Then** it saves normally and displays make and model only.
9. **Given** an active firearm with a nickname, **When** the user saves another active firearm with the same nickname (ignoring letter case), **Then** the save is blocked and the conflicting firearm is named; and **When** the first firearm is disposed, **Then** its nickname can be used on another firearm.
10. **Given** an active firearm that is not attested as serial-exempt, **When** the user saves another firearm with the same make, model, and serial number, **Then** the save is blocked and the existing record is named; and **When** the existing record is disposed, **Then** the same make, model, and serial number can be saved as a new record (reacquisition).
11. **Given** an active firearm, **When** the user saves a firearm marked serial-exempt that records the same make, model, and serial number, **Then** a warning naming the matching record is shown and the save succeeds.
12. **Given** a disposed firearm, **When** the user reverses the disposition, **Then** the system asks whether to keep the disposition details as history or discard them, restores the firearm to active status with no current disposition, and (if kept) the prior disposition remains viewable on the record; and **When** restoring would duplicate the nickname or make/model/serial of a currently active firearm (per FR-031/FR-032), **Then** the reversal is blocked with a message naming the conflicting record.

---

### User Story 2 - Browse, Search, and Group the Collection (Priority: P2)

A collector views their entire collection as a list or as tiles, groups firearms by type, caliber, or make, and searches across all recorded information (including free-form notes) to quickly locate a specific firearm.

**Why this priority**: Once records exist, finding and organizing them is the second most valuable capability — collections grow to dozens or hundreds of items, and manual scanning does not scale.

**Independent Test**: Can be fully tested by populating a handful of firearm records, switching between list and tile views, grouping by a structured field, and running a search that matches a term found only in a free-form note — independent of photos, valuation, or export.

**Acceptance Scenarios**:

1. **Given** a collection with multiple firearms, **When** the user switches between list view and tiled view, **Then** the same set of firearms is displayed in the selected layout.
2. **Given** a collection with firearms of different types, calibers, and makes, **When** the user groups by one of those fields, **Then** firearms are organized into groups matching their values for that field.
3. **Given** a collection with firearms containing varied data, **When** the user searches for a term that only appears in one firearm's free-form notes, **Then** that firearm appears in the search results.
4. **Given** a collection with firearms containing varied data, **When** the user searches for a term matching a structured field (e.g., a caliber or serial number), **Then** all matching firearms appear in the results.
5. **Given** no search or group is applied, **When** the user opens the browse view, **Then** the full collection is shown.

---

### User Story 3 - Track Value and Insurance Coverage (Priority: P3)

A collector records insurance information for each firearm (whether it is individually scheduled with a coverage amount, or covered under a blanket policy), then views a summary of the collection's total estimated value — automatically kept current as the collection changes — and sees a clear warning for any firearm that is uninsured or under-insured relative to its estimated replacement value.

**Why this priority**: This is the feature's key differentiating value — informing real insurance decisions — but it depends on firearm records (US1) already existing and benefits from being able to browse/group them (US2).

**Independent Test**: Can be fully tested by setting an estimated replacement value and insurance coverage on a small set of firearms (some fully covered, some not), viewing the value summary, then adding, editing, dispositioning, and deleting firearms and confirming the summary and under/uninsured warnings update accordingly after each change — independent of photos or export.

**Acceptance Scenarios**:

1. **Given** a firearm with an estimated replacement value and no recorded insurance coverage, **When** the user views that firearm or the collection summary, **Then** an "uninsured" warning is displayed for it.
2. **Given** an individually-scheduled firearm whose recorded coverage amount is less than its estimated replacement value, **When** the user views that firearm or the collection summary, **Then** an "under-insured" warning is displayed for it.
3. **Given** an individually-scheduled firearm whose recorded coverage amount meets or exceeds its estimated replacement value, **When** the user views that firearm, **Then** no under/uninsured warning is displayed for it.
4. **Given** a set of firearms blanket-covered under the same insurance policy, **When** the combined estimated replacement value of those firearms exceeds that policy's blanket coverage limit, **Then** an under-insured warning is displayed for the affected group.
5. **Given** a populated collection, **When** the user adds, edits, dispositions, or deletes a firearm, **Then** the displayed total estimated replacement value of the current (non-disposed) collection is recalculated to reflect that change without requiring a separate manual request.
6. **Given** a populated collection with firearms spread across multiple insurance policies and some with no policy assigned, **When** the user views the value summary, **Then** it shows the overall collection-wide total plus a breakdown per insurance policy (blanket-covered total vs. that policy's limit, and each individually-scheduled firearm vs. its own scheduled amount), with a distinct grouping for unassigned firearms.
7. **Given** an insurance policy whose effective end date is within 30 days of the current date, **When** the user views the collection or that policy, **Then** a policy-expiration warning is displayed.
8. **Given** an insurance policy whose effective end date has already passed, **When** the user views a firearm assigned to that policy (individually scheduled or blanket-covered) or the collection summary, **Then** that firearm is treated and flagged as uninsured, in addition to the expired-policy warning.

---

### User Story 4 - Attach Photos and Documents (Priority: P4)

A collector adds one or more photographs to a firearm record, designates which photo is used as its thumbnail (defaulting to the first photo added), and attaches arbitrary documents (such as a PDF receipt or appraisal) to the record.

**Why this priority**: Photos and documents enrich records and drive the thumbnail display in US2's browse views, but the collection remains fully usable without them, so this follows the core data, browsing, and valuation capabilities.

**Independent Test**: Can be fully tested by adding two photos to a firearm, confirming the first becomes the thumbnail, explicitly selecting the second as the thumbnail instead, attaching a PDF document, and confirming a firearm with no photos shows the generic thumbnail for its type — independent of export.

**Acceptance Scenarios**:

1. **Given** a firearm record with no photos, **When** the user adds a photo, **Then** that photo becomes the record's thumbnail and appears in list/tile browsing.
2. **Given** a firearm record with multiple photos, **When** the user explicitly selects a different photo as the thumbnail, **Then** browse views update to show the newly selected thumbnail.
3. **Given** a firearm record with no photos, **When** the user views it in list or tile form, **Then** a generic thumbnail matching the firearm's type is shown.
4. **Given** a firearm record, **When** the user attaches a document (e.g., a PDF), **Then** the document is saved with the record and can be reopened from it.

---

### User Story 5 - Export and Import Records (Priority: P5)

A collector exports their full collection (including photographs) to a standard spreadsheet format for backup or sharing with an insurer, and imports firearm records (without photos) from a spreadsheet in the same format to bulk-populate or update their collection.

**Why this priority**: Export/import is valuable for backup, insurance documentation, and bulk data entry, but is not required for day-to-day use of the application and depends on the record structure established by the earlier stories.

**Independent Test**: Can be fully tested by exporting a populated collection to a spreadsheet file and a photo folder, inspecting the output for completeness, and importing a separately prepared spreadsheet to confirm new/updated records appear correctly — independent of the UI browsing or valuation features.

**Acceptance Scenarios**:

1. **Given** a populated collection, **When** the user exports it, **Then** a spreadsheet file (CSV, Excel, or LibreOffice format) containing all recorded fields is produced, along with copies of all stored photographs in their original file format.
2. **Given** a spreadsheet file matching the supported import format, **When** the user imports it, **Then** firearm records are created or updated from its rows without requiring photo data.
3. **Given** an import file with a row missing a required field, **When** the user imports it, **Then** the system reports which row(s) failed and why, without discarding the successfully-imported rows.

---

### Edge Cases

- How does the system behave when a firearm's type has no generic thumbnail defined yet?
- What happens when the user chooses "apply to all" partway through resolving import conflicts, and a later conflicting row would warrant different handling (e.g., it also fails validation)?
- How does the system treat a firearm with an estimated replacement value of zero or unset when computing under/uninsured warnings?
- What happens when the user exports while a search or group filter is active — the filtered subset or the entire collection?
- How does the system handle very large photo or document files in terms of storage and import/export time?
- What happens when the user attempts to delete a firearm that still has attached photos or documents?
- What happens when the user attempts to delete an insurance policy that still has firearms assigned to it?
- What happens when the user updates an expired policy's effective end date to a future date (renewal) — do previously-flagged firearms immediately stop being treated as uninsured?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST allow the user to create a firearm record with make, model, caliber, and firearm type; a serial number MUST be recorded when the firearm has one.
- **FR-002**: System MUST allow the user to record free-form notes on a firearm (e.g., condition details such as a scratch or a cracked handle) and a list of accessories.
- **FR-003**: System MUST allow the user to record acquisition information for a firearm: source (from whom acquired), date, and price paid.
- **FR-004**: System MUST allow the user to record disposition information for a firearm: recipient (to whom transferred), date, price received, and disposition type (e.g., sold, traded, gifted, destroyed, lost/stolen).
- **FR-005**: System MUST allow the user to record an estimated replacement value for a firearm.
- **FR-006**: System MUST allow the user to edit and delete any firearm record, with confirmation required before deletion.
- **FR-007**: System MUST allow the user to attach one or more photographs to a firearm record.
- **FR-008**: System MUST designate a thumbnail image for each firearm record: the first photo added by default, or a photo the user explicitly selects instead.
- **FR-009**: System MUST display a generic, type-specific thumbnail image for any firearm record that has no photographs.
- **FR-010**: System MUST allow the user to attach arbitrary documents (e.g., PDF files) to a firearm record and reopen them from the record.
- **FR-011**: System MUST allow the user to browse the collection in both a list layout and a tiled (thumbnail-based) layout.
- **FR-012**: System MUST allow the user to group the collection by any structured (non-free-form) field, including at minimum type, caliber, and make.
- **FR-013**: System MUST allow the user to search the collection by any recorded information, including free-form fields such as notes.
- **FR-014**: System MUST allow the user to assign a firearm to an insurance policy record (see FR-027), specifying whether that firearm's coverage under the policy is individually scheduled (with its own scheduled coverage amount) or blanket (drawing from the policy's shared coverage limit).
- **FR-015**: System MUST compute and display a value summary of the current (non-disposed) firearms in the collection, automatically recalculated whenever a firearm is added, edited, dispositioned, or deleted — without requiring the user to manually trigger the calculation. The summary MUST show the overall collection-wide total estimated replacement value, and MUST break that total down per insurance policy (each policy's blanket-covered total against its coverage limit, and each individually-scheduled firearm's value against its own scheduled amount), including a distinct grouping for firearms with no policy assigned.
- **FR-016**: System MUST display a visible warning on any individually-scheduled firearm whose recorded coverage amount is less than its estimated replacement value, and on any firearm with no recorded coverage at all.
- **FR-017**: System MUST display a visible warning when the combined estimated replacement value of firearms blanket-covered under a given insurance policy exceeds that policy's recorded blanket coverage limit.
- **FR-018**: System MUST allow the user to export the full collection's recorded data to a widely-supported spreadsheet format (CSV, Excel, or LibreOffice Calc) and export all stored photographs in their original file format alongside it.
- **FR-019**: System MUST allow the user to import firearm records, without photographs, from a spreadsheet file in the same supported format(s) used for export.
- **FR-020**: System MUST report, per row, any import records that fail to import and the reason for the failure, without discarding successfully-imported rows.
- **FR-021**: System MUST store all collection data (records, photographs, and attached documents) locally on the user's device; no data may be transmitted to or stored in a cloud service.
- **FR-022**: System MUST run on multiple desktop operating systems (the collector's own machines) rather than being tied to a single platform.
- **FR-023**: System MUST retain a disposed firearm's full record (including acquisition and disposition history) after it is marked disposed, rather than deleting it.
- **FR-024**: System MUST determine "under-insured" or "uninsured" status as follows: for a firearm individually scheduled under a policy, compare its own recorded scheduled coverage amount against its estimated replacement value; for a firearm blanket-covered under a policy, compare the sum of the estimated replacement values of every firearm blanket-covered under that same policy against the policy's recorded blanket coverage limit. A firearm with no insurance policy assigned at all, or whose assigned policy has expired (its effective end date has passed), MUST be treated as uninsured regardless of whether it was individually scheduled or blanket-covered.
- **FR-025**: System MUST exclude disposed firearms from the default browse, search, group, and value-summary views, and MUST provide a toggle that reveals disposed firearms alongside active ones when the user chooses to see them.
- **FR-026**: When an imported row's make, model, and serial number (see FR-030) match an existing record, System MUST prompt the user to resolve the conflict (skip, overwrite, or create a duplicate) for that row, and MUST offer the user an option to apply the same resolution automatically to all subsequent conflicting rows in that import, in addition to resolving each row individually. The "create a duplicate" resolution MUST be offered only where FR-032 would allow the resulting record to be saved; where FR-032 would block it, only skip and overwrite are offered.
- **FR-027**: System MUST allow the user to create and maintain insurance policy records, each capturing: a user-assigned name, policy identifier/number, insurance company name, a blanket coverage limit, insurance agent name, contact information for both the company and the agent, and the policy's effective start and end dates. A single policy MAY have both individually-scheduled firearms and blanket-covered firearms assigned to it at the same time. Each firearm MUST be assignable to at most one insurance policy.
- **FR-028**: System MUST display a visible warning for any insurance policy whose effective end date is within 30 days of the current date, and for any insurance policy whose effective end date has already passed.
- **FR-029**: System MUST require the user to explicitly attest (e.g., via a dedicated "no serial number" indicator) that a firearm has no serial number and is not legally required to have one, before a record can be saved without one; an empty serial number field alone MUST NOT be accepted as that attestation.
- **FR-030**: System MUST treat the combination of make, model, and serial number — not serial number alone — as the closest available identifying key for a firearm, since serial numbers are unique only within a given make/model, not across the collection. Firearm records with no serial number (per their FR-029 attestation) MUST always be treated as new, unmatched records during import (see FR-026).
- **FR-031**: System MUST allow the user to record an optional nickname for a firearm. The nickname is free-form, MUST be unique among active firearms (compared ignoring letter case and surrounding whitespace; a blank nickname is not a value, so any number of firearms may have none), and is released when its firearm is disposed. It MUST NOT be treated as part of the firearm's identifying key (FR-030) or used in import matching (FR-026); it MUST be displayed alongside make and model wherever a firearm is shown or named (including list, tile, detail, value-summary, and insurance-warning views) when present; and it MUST be included in search (FR-013) and in export/import. A duplicate nickname is rejected on create, edit, import (as a row error, FR-020), and when reversing a disposition.
- **FR-032**: System MUST enforce make + model + serial number uniqueness (FR-030) as follows, comparing ignoring letter case and surrounding whitespace, and only against ACTIVE firearms: (a) a record with a serial number that is NOT attested serial-exempt (FR-029) MUST be blocked from saving when it matches another active firearm, with a message naming the conflicting record; (b) a record attested serial-exempt that records a serial number MUST save but MUST show a warning naming the matching active firearm; (c) a match against a disposed firearm MUST neither block nor warn (reacquisition of a previously disposed firearm is a new record); (d) a record with no serial number is never compared. The rule MUST be applied on create, edit, import, and when reversing a disposition.
- **FR-033**: System MUST allow the user to reverse a firearm's disposition, restoring it to active status. Before completing the reversal the system MUST ask the user whether to keep the recorded disposition details (type, recipient, date, price) as retained history on the record or to discard them permanently; a firearm MAY accumulate more than one retained disposition if it is disposed, restored, and disposed again. The reversal MUST re-apply FR-031 and FR-032 and be blocked, naming the conflicting record, when the restored firearm would conflict with a currently active one. This does not weaken FR-023: a disposed firearm's record is never deleted by disposal.

### Key Entities

- **Firearm**: A single item in the collection. Represents identifying information (make, model, an optional serial number, caliber, type), an optional user-assigned nickname (unique among active firearms) used only to tell similar records apart, an explicit "no serial number" attestation when no serial number is recorded, free-form notes, accessories, current status (active or disposed), acquisition details, disposition details (if disposed), estimated replacement value, and links to its photographs, documents, and insurance coverage.
- **Disposition History**: A retained, read-only record of a past disposition (type, recipient, date, price) of a firearm that was later restored to active status, kept only when the user chose to keep it (FR-033). Belongs to one firearm; removed when that firearm is deleted.
- **Photo**: An image associated with a firearm. Tracks its original file, the order in which it was added, and whether it is the designated thumbnail for that firearm.
- **Document Attachment**: An arbitrary file (e.g., a PDF) associated with a firearm, such as a receipt or appraisal.
- **Insurance Policy**: A user-created record representing a real-world insurance policy. Captures a user-assigned name, policy identifier/number, insurance company, insurance agent, contact information for both the company and the agent, a blanket coverage limit, and effective start and end dates. A policy may have firearms individually scheduled under it, blanket-covered under it, or both at once, and expires on its recorded end date.
- **Insurance Coverage**: How a specific firearm is insured under its assigned Insurance Policy — either an individually-scheduled amount specific to that firearm, or blanket coverage drawn from the policy's shared limit alongside other firearms on the same policy. A firearm may have no policy assigned (uninsured).
- **Generic Thumbnail**: A default image associated with each firearm type, shown for any firearm record lacking its own photographs.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can add a complete firearm record (all identifying fields, acquisition info, and estimated value) in under 5 minutes.
- **SC-002**: A user can locate any single firearm in a collection of at least 500 records by searching any one piece of recorded information (including a free-form note) in a single search action.
- **SC-003**: The value summary — both the collection-wide total and its breakdown per insurance policy — always reflects the current state of the collection, never stale after an add, edit, disposition, or delete, without the user needing to take a separate action to refresh it.
- **SC-004**: 100% of firearms that are uninsured or under-insured relative to their recorded coverage are visibly flagged whenever the collection or an individual record is viewed.
- **SC-005**: A user can export their entire collection, including all stored photographs, to a spreadsheet-based backup in a single action.
- **SC-006**: A user can import a correctly-formatted spreadsheet of firearm records without needing to supply photographs, and receives a clear report of any rows that failed to import.
- **SC-007**: Every firearm record shown in list or tile browsing displays a thumbnail — either derived from a stored photograph or a generic type-based image — with no record ever shown without one.
- **SC-008**: No collection data is ever transmitted off the user's device; all functionality (browsing, search, valuation, insurance warnings) works with no network connection present.
- **SC-009**: Users are warned at least 30 days before any tracked insurance policy's coverage lapses, and every firearm on an expired policy is flagged as uninsured immediately upon expiration.

## Assumptions

- This is a single-user, personal application with no accounts, login, or multi-user access control — the entire local data store belongs to one collector.
- "Type of firearm" is a structured field with an initial common set of values (e.g., Handgun, Rifle, Shotgun, Other), each with its own generic thumbnail image; the set of types can grow as needed.
- Make, model, and caliber are treated as structured (non-free-form) fields for grouping and search purposes, even though their specific values are user-entered rather than a fixed list.
- "Cross-platform desktop" means the application is usable on the major desktop operating systems (Windows, macOS, Linux) without requiring a server or internet connection.
- The value summary is recalculated automatically whenever the underlying data changes (a firearm is added, edited, dispositioned, or deleted), rather than requiring the user to manually trigger the calculation.
- Deleting a firearm record also removes its uniquely-associated photographs and document attachments from local storage.
- Serial number requirements and exemptions (e.g., the U.S. Gun Control Act of 1968's October 22, 1968 cutoff for homemade and antique firearms) are evaluated against U.S. federal law as the general norm; state, local, and non-U.S. serialization or registration requirements are out of scope.
