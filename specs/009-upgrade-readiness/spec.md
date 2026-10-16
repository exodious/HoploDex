# Feature Specification: Stable Record Identifiers, Unique Policy Names and Upgradable Databases

**Feature Branch**: `009-upgrade-readiness`

**Created**: 2026-10-16

**Status**: Draft

**Input**: User description: "issues 53 and 115, and let's settle 54's questions about unique policy names." Two GitHub issues, plus one question from a third, taken together because all three are about the 0.1.0 data layout: #53, stable record identifiers that hold across database files; #115, beta testers' databases upgraded from 0.1.0 on rather than recreated; and from #54 (import and export of insurance policies, 0.2.0), only the question of whether policy names are unique and how a spreadsheet row names its policy. The rest of #54 stays in #54.

## Clarifications

### Session 2026-10-16

- Q: Should policy names be unique? → A: Yes, among all of a database's policies (current, expired and future), ignoring letter case and surrounding spaces the way import matches them. A spreadsheet row keeps naming its policy by its name (FR-007).
- Q: When a database needs an upgrade, should the user be asked first, or told afterwards? → A: Asked first, with Upgrade and Cancel. The question names the version, says a backup is made first, and says earlier versions will then open only the backup; Cancel closes the database untouched (FR-010).
- Q: When the backup before an upgrade can't be made at the backup location, what happens? → A: The user is offered to save this one backup in a folder they choose, or to cancel. The database is never upgraded without a backup (FR-012).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Records Keep One Identity in Every Copy of a Database (Priority: P1)

A collector's insurance policies, photos and documents each carry an identity that never changes and is never reused, as firearms and accessories already do (feature 006). A backup, a restored database or a copy of the file holds the same records under the same identities. Later features (restoring single records from a backup, #30; importing from another HoploDex database, #31; importing policies, #54) can then match a record in one file with the same record in another, rather than guessing from its internal number, its name or its contents.

**Why this priority**: It is the one part of this feature that becomes expensive after 0.1.0. Today it is a few lines added to the data layout. Once beta testers have databases, adding a required, unique value to existing records means an upgrade that rebuilds tables and fills in every row (#53, "Why now"). The collector sees nothing; the value is in what it makes safe later.

**Independent Test**: In a test database, create policies, photos and documents; edit them, back the database up, restore it, change its passphrase; delete the newest of each and create another. Confirm each record's identifier is unique, unchanged through all of it, the same in the backup and the restored database, and new for the newly created record even where its internal number was reused.

**Acceptance Scenarios**:

1. **Given** two policies with the same insurer and dates, **When** both are saved, **Then** each has its own identifier, and the database refuses a second record carrying an identifier already in use.
2. **Given** a policy, photo or document, **When** it is edited, reordered, or its owning record is edited, disposed of or restored to active, **Then** its identifier is unchanged.
3. **Given** a database with policies, photos and documents, **When** a backup is made and later restored, or the passphrase is changed, **Then** every record has the same identifier as before.
4. **Given** the policy, photo or document with the highest internal number, **When** it is deleted and a new one created, **Then** the new record gets a new identifier even if it reuses the internal number.

---

### User Story 2 - A Policy Name Means One Policy (Priority: P2)

A collector names each insurance policy, and a spreadsheet row names the policy its item is scheduled under. Today two policies may share a name (for example one "Homeowners rider" per year), and an import then schedules the item under whichever of them it finds first, without a word (#54, "Today"). After this feature, a name in a spreadsheet row either picks out exactly one policy or the row is reported, and the collector is never left with an item silently scheduled under the wrong policy.

**Why this priority**: It is a data-integrity bug today, and whether names become unique is a rule on the data layout, which is cheapest to set before 0.1.0 (#54, "Schema note"). It does not depend on Story 1, though it decides how much of #54 will lean on Story 1's identifiers.

**Independent Test**: Try to save two policies whose names are the same, or differ only in letter case or surrounding spaces, and confirm the second is refused. Then import firearm and accessory rows naming existing policies with different letter case, and a policy that doesn't exist. Confirm each row is scheduled under the one policy its name identifies, or reported.

**Acceptance Scenarios**:

1. **Given** a policy named "Homeowners rider", current or expired, **When** the collector saves another policy named "homeowners rider " (or renames one to it), **Then** the save is refused with a message at the name field saying which existing policy conflicts, and nothing is saved.
2. **Given** a spreadsheet row whose policy name matches exactly one policy (ignoring letter case, as import does today), **When** it is imported, **Then** the item is scheduled under that policy.
3. **Given** a spreadsheet row whose policy name matches no policy, **When** it is imported, **Then** the row is reported as an error naming the unknown policy, and the item is not scheduled under any policy by a guess.

---

### User Story 3 - A Beta Tester's Database Survives an Upgrade (Priority: P3)

A beta tester has used HoploDex 0.1.0 for a month and installs 0.2.0, whose data layout has changed. When they open their database, a backup of it as it was is made first, then it is upgraded, and everything in it is still there. If they don't like 0.2.0, the backup opens in 0.1.0. If the upgrade fails, or the computer loses power in the middle of it, the database is exactly as it was and they are told.

**Why this priority**: It only comes into play with the first release after 0.1.0, but two of its parts must ship in 0.1.0 itself: 0.1.0 is the version that will be asked to open a backup after an unwanted upgrade, and 0.1.0's refusal of a newer database is what will point the tester to that backup (FR-018).

**Independent Test**: Open a database made by an earlier layout (for the first release, 0.1.0's) with a build whose layout is newer. Confirm a backup of the unchanged database is made at the backup location before anything is written, the database opens upgraded with every value intact, the user is told where the backup is, and the backup opens in a build with the earlier layout. Then make the upgrade fail at each step and confirm the database is left as it was.

**Acceptance Scenarios**:

1. **Given** a database whose layout is older than the application's, **When** the user opens it, **Then** the user is asked whether to upgrade it (FR-010); on Upgrade a backup of the database as it was is made at its backup location before anything in it is changed, and on Cancel the database closes untouched.
2. **Given** the backup has been made, **When** the upgrade finishes, **Then** every record, photo, document, policy, link between records, setting and pending change is in the upgraded database, and the user is told the database was upgraded, where its backup is, and that earlier versions open the backup but not the upgraded database.
3. **Given** an upgrade in progress, **When** it fails or is interrupted at any point (an error, the application ended, power lost), **Then** the next open finds the database exactly as it was before the upgrade began, and an interrupted backup is never mistaken for a complete one.
4. **Given** a database already upgraded by a newer version, **When** the user opens it in the earlier version, **Then** it is refused, unchanged, with a message that a newer version is needed and that a backup made before an upgrade opens in this version.
5. **Given** backups are turned off for the database, **When** it is upgraded, **Then** the backup before the upgrade is still made.

---

### User Story 4 - The Data Layout Can Still Change After Release (Priority: P4)

The maintainer changes the data layout after 0.1.0, including changes that can only be made by rebuilding a table (a new allowed value, as #16's storage locations will need for kept unsaved changes; a new required, unique value). Each change ships as a new upgrade step; the steps already released never change; and the project's automated checks prove that a database made by every supported release opens, upgraded, with all its data.

**Why this priority**: The collector never sees it directly, but without it Story 3 can't be kept: an edited released step would upgrade some databases differently from others, and a change that needs a rebuilt table couldn't be written at all today (#115).

**Independent Test**: Add, in a test build, an upgrade step that rebuilds a table with links to and from other tables; open a database full of data with it and confirm every row and link survives and the links are checked. Alter a released step and confirm the project's checks fail. Open the stored database made by 0.1.0 with the current build and confirm every value.

**Acceptance Scenarios**:

1. **Given** an upgrade step that rebuilds a table other tables refer to, **When** a database is upgraded, **Then** every row, identifier and link between records is kept, the links are verified before the upgrade finishes, and an inconsistency abandons the whole upgrade (Story 3, scenario 3).
2. **Given** 0.1.0 has been released, **When** anyone edits one of the upgrade steps it shipped, **Then** the project's automated checks fail.
3. **Given** a stored database made by each supported release, **When** the automated tests open it with the current build, **Then** every value it holds is checked after the upgrade.

---

### Edge Cases

- **Several releases skipped** (0.1.0 straight to 0.4.0): one backup is made, and every step between applies as one upgrade, all or nothing.
- **Not enough space, or the backup location unavailable** (a removable drive not attached, a full disk): the user is offered to save this one backup in a folder they choose, or to cancel (FR-012). The database is never upgraded without a backup.
- **Keep count of 1, or many closes after the upgrade**: the most recent backup before an upgrade isn't removed by rotation (FR-014), so it is still there after the next close's backup.
- **Restoring an older backup inside the newer version**: the restored database is upgraded as part of the restore. The backup restored from is itself the copy in the earlier layout and stays at the location, so no second backup before the upgrade is made (FR-019).
- **Opening an older backup directly** (from its backup folder): it is a database like any other and is upgraded on the same terms. Its own backup goes to the backup location its settings name.
- **Pending changes kept by the earlier version** (a lock mid-edit, then an upgrade): they survive the upgrade unchanged; whether they can be resumed stays decided by the form's version (008 FR-016).
- **The database open on another computer**: the open-marker rules (003 FR-032) apply before anything else, and nothing is backed up or upgraded until this computer has the database.
- **A newer version's database** (downgrade): refused unchanged, as 003 FR-014, with FR-018's wording.
- **Databases made before this feature** (the developer's own, and early-access ones): this is the last change to the layout made in place (CLAUDE.md). They must be created again; nothing is migrated, and the developer's databases are not touched.
- **Two policies named alike that existed before**: none can exist in a database made under this feature's layout, since earlier databases are recreated.
- **Renewing a policy each year under the same name**: the new year's policy needs a different name (for example "Homeowners rider 2026"); the old one can be renamed instead.
- **A record created as a duplicate on import** (001 FR-026 "create a duplicate"): gets a new identifier, as 006 does for firearms and accessories.

## Requirements *(mandatory)*

### Functional Requirements

**Record identifiers (#53)**

- **FR-001**: Every insurance policy, photo and document attachment MUST carry a record identifier on the same terms as a firearm's and an accessory's (006): a random version-4 UUID in lowercase hyphenated form, set when the record is created, never changed afterwards, and unique among the records of its kind. The database itself MUST refuse a record without one, with a malformed one, or with one already used by another record of its kind, and MUST refuse a change to it.
- **FR-002**: A record's identifier MUST be kept by every copy of the whole database: a backup, a restore, a passphrase change, a copy of the file, and an upgrade (FR-015). Editing, reordering, disposing of or reversing the disposition of a record, or of the record that owns it, MUST NOT change it.
- **FR-003**: The new identifiers MUST NOT be shown anywhere in the application, and this feature adds them to no export. The features that need them (#30, #31, #54) decide how they appear.
- **FR-004**: Disposition history entries and the built-in lists (firearm types, action types, registration classes, accessory kinds) MUST NOT get an identifier: a history entry is only ever restored or imported with the record it belongs to, and a built-in list entry is identified by its unique name.
- **FR-005**: An identifier MUST be unique within its own kind of record. Firearms and accessories keep refusing each other's identifiers (006), because a spreadsheet's mounted-on value can name either; no other kinds need to be unique across each other.

**Policy names (#54's question)**

- **FR-006**: Importing a firearm or accessory row that names an insurance policy MUST look the name up with the same comparison as FR-007's rule, so a name identifies at most one policy. A name matching none MUST be a row error naming it, and the item MUST NOT be scheduled under any policy by a guess.
- **FR-007**: A policy's name MUST be unique among all of the database's policies, current, expired and future, compared ignoring letter case and surrounding spaces, the way import matches names. The database itself MUST enforce this as well as the policy form.
- **FR-008**: A policy save that breaks FR-007's rule MUST be refused with a field error at the policy name, in the form's usual error pattern, naming the policy it conflicts with.

**Upgrading a database (#115)**

- **FR-009**: The system MUST recognise, at open, a database whose data layout is older than the application's, and MUST upgrade it only after the database's passphrase has opened it and this computer holds it (003 FR-032).
- **FR-010**: Before upgrading, the system MUST ask the user, naming the version the database will be upgraded for, saying that a backup of it is made first and that earlier versions will then open only that backup, not the database. **Upgrade** goes ahead; **Cancel** closes the database untouched and returns to where it was opened from. The question MUST follow the application's standard confirmation pattern (constitution III) with focus on Upgrade, as for a non-destructive question (CLAUDE.md, "One Enter rule").
- **FR-011**: Before changing anything in the database, the system MUST make a backup of it as it is, at the database's backup location (003 FR-026). This backup MUST be made whether or not automatic backups are on and regardless of the once-a-day limit, as 003 FR-028 does before a restore, and only after the user has chosen Upgrade (FR-010). Before writing, the system MUST check that the backup location is available and has room for the backup.
- **FR-012**: When the backup cannot be made at the backup location (the location unavailable, out of space, or the backup failing), the system MUST say what is missing and offer to save this one backup in a folder the user chooses, or to cancel, which closes the database untouched. The chosen folder MUST be checked the same way (FR-011), and a failure there MUST offer the choice again. The database MUST NOT be upgraded without a backup. A backup saved in a chosen folder is not at the backup location, so it is neither listed nor rotated (003 FR-026); FR-017's message names where it is.
- **FR-013**: The backup made before an upgrade MUST be an unchanged copy in the earlier layout, opening in the earlier version with the passphrase current when it was made, and MUST be identifiable, in the backup list and when opened directly, as made before upgrading to a named version.
- **FR-014**: The most recent backup made before an upgrade MUST NOT be removed by rotation (003 FR-025) until the next upgrade's backup replaces it or the user deletes all backups (003 FR-029). Earlier ones rotate like any other backup.
- **FR-015**: An upgrade MUST be all or nothing, across every step it applies: either the database opens fully upgraded, with every record, identifier, photo, document, policy, link between records, setting, backup record and pending change intact, or, after a failure or interruption at any point (including the application being ended or the computer losing power), the database MUST be exactly as it was before the upgrade began, and the user MUST be told the upgrade failed and where the backup is.
- **FR-016**: When the backup and upgrade together are expected to take longer than one second, the system MUST show a progress indicator (constitution IV), as 003 FR-027 does for a backup at close.
- **FR-017**: After an upgrade, the system MUST tell the user that the database was upgraded for this version, where the backup made before the upgrade is, and that earlier versions can open that backup but not the upgraded database.
- **FR-018**: The refusal of a database whose layout is newer than the application's (003 FR-014) MUST also say that a backup made before an upgrade by a newer version opens in this version, and where backups are usually kept. This wording MUST ship in 0.1.0, which is the first version to show it.
- **FR-019**: Restoring a backup whose layout is older than the application's (003 FR-028) MUST upgrade the restored database as part of the restore, on FR-015's terms; the restore's confirmation MUST say so, in place of FR-010's separate question. The backup restored from stays at the backup location and serves as the copy before the upgrade, so no further backup is made for it.
- **FR-020**: An upgrade MUST be able to make changes that rebuild a table (for example a new allowed value, or a new required unique value), keeping every row, identifier and link between records. It MUST verify that every link between records is consistent before it finishes, and MUST abandon the whole upgrade under FR-015 if any is not.

**Keeping upgrades trustworthy (#115)**

- **FR-021**: From the 0.1.0 release on, every change to the data layout MUST be a new upgrade step, and an upgrade step that has shipped in a release MUST NOT change. The project's automated checks MUST fail when a released step is altered, and the project's development guidance (CLAUDE.md, "Schema changes edit the existing migrations in place") MUST say so.
- **FR-022**: Each upgrade step MUST depend only on the layout it starts from, and not on application behaviour that may change in later versions, so it gives the same result in every later version.
- **FR-023**: The project's automated tests MUST keep a database made by each supported release, starting with 0.1.0, holding every kind of record, link, setting and attachment, and MUST open each with the current build and check every value after the upgrade and that the backup was made. Until 0.1.0 is released the stored 0.1.0 database MUST be made again whenever the layout changes; from the release on it MUST NOT change.

### Key Entities

- **Record identifier**: the permanent, random identity of a firearm, accessory (both from 006), insurance policy, photo or document attachment. Unique within its kind; the same in every copy of the database; never shown.
- **Insurance policy**: gains a record identifier, and its name is unique among the database's policies, ignoring letter case and surrounding spaces (FR-007).
- **Data layout version**: what upgrade steps a database has had. Older than the application's means an upgrade is due; newer means refusal (003 FR-014).
- **Upgrade step**: one change to the data layout, shipped in a release and never edited afterwards.
- **Backup before an upgrade**: a backup (003) in the earlier layout, made before an upgrade regardless of the backup settings, marked with the version it preceded, and kept out of rotation while it is the most recent such backup.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In automated tests on real databases, 100% of policies, photos and documents have distinct identifiers, and 0 change through editing, backup, restore, passphrase change and upgrade.
- **SC-002**: After the highest-numbered policy, photo or document is deleted and a new one created, the new record's identifier matches no earlier record's in 100% of runs.
- **SC-003**: In automated tests on real databases, 100% of attempts to save a policy whose name matches another's (ignoring letter case and surrounding spaces) are refused, and 0 imported rows are scheduled under a policy other than the one their name identifies; every unknown name is reported.
- **SC-004**: The stored database made by 0.1.0 opens with the current build with 100% of its values unchanged, and the backup made before its upgrade holds content identical to the database before the upgrade.
- **SC-005**: With a failure forced at each step of an upgrade in turn, 100% of runs leave a database that opens in the earlier version with all its content, and 0 leave a partly upgraded database.
- **SC-006**: Altering any released upgrade step fails the project's automated checks in 100% of cases.
- **SC-007**: Opening a 10,000-item database that needs an upgrade shows progress within 1 second whenever the backup and upgrade take longer, and the interface stays responsive throughout (constitution IV).

## Assumptions

- **One feature for two issues and a question**: specified together at the owner's request. Stories 1 and 2 change the layout and must land before 0.1.0 is tagged; Stories 3 and 4 build the upgrade machinery and its tests. The plan may order them, but Story 4's stored 0.1.0 database is made after Stories 1 and 2 are in.
- **The last change made in place**: Stories 1 and 2 edit the existing layout in place, as CLAUDE.md allows while the application is unreleased. FR-021's rule takes effect at the 0.1.0 tag, which the release process (#19) records.
- **What the earlier version must already know**: FR-013's marking of the backup before an upgrade is written into a copy in the earlier layout. Whatever holds it (a value in the database's own housekeeping, or the backup's file name) must therefore already exist in 0.1.0's layout or naming; the plan settles which, before 0.1.0 is tagged.
- **The backup before an upgrade and the backup record**: it does not clear or set the database's "changes waiting" or count as the day's backup at close (003 FR-025). The upgrade itself isn't a collection change.
- **No upgrade of pre-0.1.0 databases**: databases made before this feature are recreated, as before every unreleased layout change.
- **Disposition history and built-in lists** need no identifier (FR-004), as #53 asked to confirm; the plan can revisit this if #30's spec needs history entries matched on their own, at the cost of an upgrade step.
- **Letter case**: "ignoring letter case" means what import's lookup does today; whether letters outside the basic Latin alphabet are folded too is settled at planning, the same way for the rule and the lookup.
- **The rest of #54** (the policy sheet's format, matching imported policies, conflicts, blanket coverage, disclosure) stays in #54 for 0.2.0, as does whether policies get documents (#118).
- **The post-1.0 policy** (what counts as a breaking change, a fuller upgrade dialog, squashing the 0.x steps at 1.0, showing the last-saved version) stays in #24.
- **Testing**: everything is tested against real persistence (constitution II): identifiers and names in the operations tests, upgrades against stored databases and test-only upgrade steps, failure at each step by forced errors. The human-testing seed and import samples gain records exercising the new identifiers and the naming rule (CLAUDE.md).
- **Security review**: the feature changes persistence and the open path. The plan records how it meets the Security & Data Handling Constraints (the backup is local, at the location the user chose, and protected by the passphrase current when made), and the pull request says so.

## Relationship to Earlier Features

- **001** (`specs/001-firearms-inventory/`): insurance policies (FR-027) gain an identifier and unique names (FR-007); import's policy lookup (FR-019, FR-020) follows the same comparison (FR-006).
- **003** (`specs/003-database-protection-management/`): FR-014's refusal gains FR-018's wording; FR-025's rotation keeps the latest backup before an upgrade (FR-014); FR-028's restore upgrades an older backup (FR-019). Opening a database gains the backup and upgrade (FR-009 to FR-017).
- **006** (`specs/006-accessory-links/`): the identifier settled there for firearms and accessories (FR-019) is extended to policies, photos and documents on the same terms.
- **008** (`specs/008-hardening-batch/`): the last-saved version (FR-015) and the form-version rule for pending changes (FR-016) are unchanged by an upgrade.

## Source Request
This feature was filed as two GitHub issues and one question from a third, specified together at the owner's request (`/speckit-specify issues 53 and 115, and let's settle 54's questions about unique policy names.`). Each issue is reproduced verbatim, with the comments recorded on it and where this spec answers it.

### #53: "Stable record identifiers that hold across database files (groundwork for #30 and #31)" (label: infrastructure)
> ## Problem
>
> Records have no identity that holds across database files. `firearms`, `insurance_policies`, `photos`, `document_attachments` and `disposition_history` all use `id INTEGER PRIMARY KEY`, without `AUTOINCREMENT`, and there is no other identifying column. SQLite gives a new row the highest rowid plus one, so after the highest-numbered record is deleted, the next one created gets the same id.
>
> This doesn't matter within one database today. It does matter as soon as a record in one file has to be matched with a record in another:
>
> - **#30, restore individual records from backups.** A backup's firearm 57 may be a firearm that was since deleted, and today's firearm 57 a different one created after it. Matching by `id` would show the wrong differences and could restore one firearm's fields onto another.
> - **#31, import records from other HoploDex databases.** Two databases' ids are unrelated, so the only match is the spreadsheet import's `(make, model, serial_number)` key. Firearms with no serial number can never match (FR-030), so importing twice duplicates them.
> - **Copies of one database** (a file copied to a second computer, then both edited) have the same problem as #31.
>
> ## Why now
>
> 0.1.0 is the first release, and beta testers' databases will be upgraded rather than recreated from then on. Adding the column now, while schema changes still edit `0001_initial.sql` in place, costs a few lines. Adding it after release means a migration that rebuilds tables to add a `NOT NULL UNIQUE` column and fills it for every existing row. #30 and #31 themselves can wait until after 0.1.0. Only the identifier has to be in place first.
>
> ## Proposal
>
> - Add a stable, random identifier column (e.g. `uid TEXT NOT NULL UNIQUE`, a UUIDv4 or 32 lowercase hex digits like `app_state.database_id`) to each table whose rows can be restored or imported on their own: `firearms`, `insurance_policies`, `photos`, `document_attachments`, and probably `disposition_history`.
> - **`insurance_policies` is required**, not one of the tables still to decide: its `uid` is `NOT NULL UNIQUE` like `firearms`'. #54 (import and export of insurance policies) has to match a policy across database files, and policy names can't do it: they aren't unique, and today's import picks whichever same-named policy it finds first.
> - **Accessories get it too**, once spec 006 (#50) adds an accessories table: 006 FR-019 requires every accessory to carry this identifier, and its spreadsheet matches accessory rows only by it. Whichever of #53 and 006 is built second adds the `uid` column to that table, `NOT NULL UNIQUE` like the others.
> - Set it when the row is created, in `ops`, and never change it. Editing, disposing and reversing a disposition keep it. Copying a whole database (backups, restore, a file copy) keeps it, because the rows are the same records.
> - Keep `INTEGER PRIMARY KEY` as the internal key for foreign keys and the IPC `id`. The new column only identifies a record across files.
> - Seed it in `examples/human_seed.rs` (`tests/human_seed_coverage_test.rs`).
>
> ## To decide
>
> Spec 006 (#61) settled the format, the spreadsheet column and the firearm and accessory tables; see the comment below. Still open:
>
> 1. **The remaining tables.** `insurance_policies` gets one (required, for #54, see Proposal); `photos` and `document_attachments` (for #30). Is `disposition_history` restored or imported on its own, or always with its record? Confirm that the built-in lookups (`firearm_types`, `action_types`, `registration_classes`, `accessory_kinds`) need none, their unique name being enough.
> 2. **Cross-table uniqueness.** `firearms` and `accessories` refuse each other's identifiers (`*_uid_distinct` triggers), since a spreadsheet's `mounted_on` can name either. Does that matter for the other tables, or is uniqueness within each table enough there?
> 3. **Import from other databases (#31).** That's #31's decision. 006's spreadsheet import gives a data point: a created record keeps its row's identifier, and one created as a duplicate of a match gets a new one.
> 4. **Spec.** Adding the column to the remaining tables, set at creation and never shown, likely needs no spec; #54's and #30's specs say how they use it.
>
> Settled by 006, for reference: the format is a lowercase hyphenated v4 UUID (`services::record_id`, with a `CHECK` on the column), set in `ops` at creation and fixed by trigger; export writes it as `record_id` and import matches by it first, ahead of `(make, model, serial_number)`.
>
> ## Acceptance
>
> - Every row in the chosen tables has a unique, non-null identifier, set at creation and unchanged by any edit.
> - `insurance_policies` is one of them: two policies with the same name get distinct identifiers, and a second row with an existing policy's identifier is refused by the database.
> - If the accessories table exists when this is built, it has the identifier on the same terms as `firearms`.
> - `ops` tests against real persistence: created records get distinct identifiers; editing, disposing and reversing a disposition keep it; a backup and a restored database have the same identifiers; deleting the highest-id firearm and creating a new one gives a new identifier even when the integer `id` is reused.
> - Human-testing seed updated.
>
> Related: #30, #31, #50 (accessories, spec 006), #54 (policy import and export), #24 (migration policy).

**Comment recorded on the issue (owner, at feature 006's pull request #61):**
> Spec 006 (accessories and mounting, #61) settles part of this.
>
> **Settled**
> - `uid` on `firearms` and `accessories`: `TEXT NOT NULL UNIQUE`, a lowercase hyphenated v4 UUID from `services::record_id`, checked by a `CHECK` on the column. It is set in `ops` when the record is created and fixed by trigger (`*_uid_fixed`), so editing, disposing, reversing a disposition, backups and restores keep it. The two tables also refuse each other's identifiers (`*_uid_distinct`), since a spreadsheet's `mounted_on` can name either. The identifier is shown nowhere in the app except the spreadsheet.
> - The spreadsheet: export writes it as `record_id` in both tables, and `mounted_on` holds the host's identifier. Import matches by it first, among records of the row's kind, ahead of `(make, model, serial_number)`, so an export edited and imported back no longer duplicates firearms without a serial number. A sheet without the column still imports.
> - A data point for #31: a record created from a row keeps the row's identifier, and one created as a duplicate of a match gets a new one.
> - Tests against real persistence (`record_identifier_test`, `accessory_spreadsheet_test` and `import_matching`'s unit tests), and the human-testing seed and import samples for both tables.
>
> **Left**
> - `insurance_policies.uid`, required, for #54.
> - `photos.uid` and `document_attachments.uid`, for #30.
> - Whether `disposition_history` needs one.
> - Confirming the lookup tables need none.
> - Whether cross-table uniqueness matters beyond firearms and accessories.
> - The acceptance tests and seed for those tables.
>
> I've updated the "To decide" list above to match.

→ Story 1; FR-001 to FR-005; SC-001, SC-002. The issue's "To decide" list: (1) the remaining tables are `insurance_policies`, `photos` and `document_attachments`; `disposition_history` and the built-in lookups get none (FR-004); (2) uniqueness across kinds stays limited to firearms and accessories (FR-005); (3) import from other databases stays #31's decision; (4) it is specified here after all, because it is grouped with #115.

### #115: "Upgradable databases from 0.1.0: frozen migrations, backup before migrating, table rebuilds, upgrade tests" (label: infrastructure)
> ## Request
>
> Split from #24. Beta testers' databases are to be upgraded, not recreated, so from 0.1.0 on a schema change needs a migration path for existing databases. The full versioning policy (what counts as breaking, the UI flow for an older schema, the constitution) stays post-1.0 in #24. This issue is the part 0.1.0 needs.
>
> ## Groundwork that already exists
>
> The `schema_migrations` table tracks applied migrations and doubles as the data-layout version, and feature 003's forward-compatibility guard (FR-014, the schema version check at open) refuses a database from a newer build.
>
> ## Needed for 0.1.0
>
> - [ ] **Stop editing migrations in place once 0.1.0 is tagged.** From then on, every schema change is a new migration. Update the convention in `CLAUDE.md` ("Schema changes edit the existing migrations in place … while the app is unreleased").
> - [ ] **Back up before migrating.** `apply_migrations` runs during `open_database` with no backup first. Before applying a migration a database hasn't had yet, make a backup (feature 003's mechanism) so a failed or unwanted upgrade can be undone.
> - [ ] **Let the runner rebuild tables.** SQLite can't change a CHECK constraint or add a `NOT NULL UNIQUE` column with `ALTER TABLE`, so such a change rebuilds the table. That needs `PRAGMA foreign_keys = OFF` around the rebuild and a `PRAGMA foreign_key_check` after, and SQLite can't change `foreign_keys` inside a transaction. `apply_migrations` wraps each migration in one, with foreign keys already on from `configure`, so a rebuild migration can't be written today. Example: adding a `location` kind to `pending_changes` for #16.
> - [ ] **Test upgrades.** Add a fixture database made by 0.1.0, and a test that opens it with the current build and checks the data after migrating (real persistence, constitution II).
>
> ## Not in scope
>
> Squashing the 0.x migrations is decided at 1.0 (#24). Beta databases stay upgradable through 0.x either way. Keep migrations pure SQL that never calls app logic, so they keep working with every later SQLite/SQLCipher.
>
>
> ---
> _Generated by [Claude Code](https://claude.ai/code)_

→ Stories 3 and 4. Item 1 is FR-021; item 2 is FR-009 to FR-017; item 3 is FR-020; item 4 is FR-023. The backup's user-facing questions (backups off, rotation, failure, telling the user, the earlier version's message) are FR-011 to FR-014, FR-017 and FR-018.

### #54: "Import and export insurance policies through the spreadsheet" (label: new feature, needs spec)
> ## Origin (2026-10-01)
>
> Raised while clarifying spec 006 (accessory records and mounting, #50). Its round-trip criterion said that importing an export into an empty database reproduces every record. That isn't true today for anything scheduled under an insurance policy, because policies aren't exported. 006's SC-002 now says the target database must already have the same policies, by name.
>
> ## Today
>
> - Export writes one row per firearm. A scheduled firearm's row names its policy in `insurance_policy_name`, with its `scheduled_coverage_amount`. The policy itself (insurer, policy number, contacts, notes, blanket limit, effective dates) never leaves the database.
> - Import looks the name up among the policies already in the database, ignoring letter case (`src-tauri/src/commands/import_export.rs`, the `insurance_policy_name` lookup). An unknown name is a row error ("Unknown insurance policy: …"). Moving a collection into a new database therefore means re-entering every policy by hand first, with exactly the same names.
> - **Policy names aren't unique.** Nothing in the schema or in validation stops two policies having the same name (e.g. one per year, both "Homeowners rider"). Import's lookup takes whichever row comes first, so a firearm can be scheduled under the wrong policy without any warning. This is a bug today, separate from this feature, but import/export of policies has to settle how a policy is identified anyway.
>
> ## Questions for the spec
>
> - **Format:** a policy table alongside the firearm table (and 006's accessory table): another sheet in a workbook, another CSV file. 006 settled that tables are separate and optional, and that import tells them apart by header. Policies should follow the same pattern.
> - **Identity and matching:** match an imported policy to an existing one by #53's record identifier, by name, or by insurer and policy number? What should firearm and accessory rows use to name their policy: the name (readable, but not unique) or the identifier (unique, but opaque)? Should policy names become unique among policies?
> - **Conflicts:** when a policy row matches an existing policy, offer skip, overwrite or create a duplicate, as firearm rows do (001 FR-026)?
> - **Order and validation:** firearm and accessory rows that name a policy imported in the same run must resolve to it. Apply policy rules (end date after start date, blanket limit ≥ 0, whole dollars) as row errors. Decide what happens to a firearm row whose policy row failed.
> - **Blanket coverage:** a blanket policy's limit is shared by every unscheduled item while the policy is in force (001 FR-036). Make sure importing overlapping blanket policies can't leave the collection in a state the app wouldn't allow.
> - **Documents:** policies can have attached documents (`document_attachments.kind = 'policy'`). Export photos already go in a sibling folder. Do policy documents get exported too, or stay out, as firearm documents do today (export writes no documents)?
> - **Export dialog disclosure:** policy numbers and agent contacts leave the encrypted database, so the dialog's statement of what's exported has to name them (constitution V; as 005 FR-020, 006 FR-021).
> - **Scope:** a filtered export (006 decided it includes the accessories mounted on the filtered firearms). Does it include only the policies its rows name, or every policy?
>
> ## Schema note
>
> Matching by identifier needs #53's `uid` on `insurance_policies` (#53 already lists that table). Making policy names unique would be a schema change, which is cheaper before 0.1.0 (#24). Everything else here is spreadsheet and import logic, with no schema change.
>
> Related: #50 / spec 006 (separate tables, optional accessory table), #53 (record identifiers), #31 (import from other HoploDex databases), #24 (migration policy).

**Comment recorded on the issue (owner):**
> Correction to the **Documents** question above: policies can't have attached documents today. `document_attachments` has only `firearm_id` and `accessory_id`, with a check that exactly one is set. The `kind IN ('firearm', 'policy', 'accessory')` column is on `pending_changes` (a form's unsaved input kept at a lock), not on documents.
>
> Attaching documents to a policy is now #118. Whether policy documents are exported depends on it, and should be settled together with the existing rule that export writes no documents for any record.

→ Story 2; FR-006 to FR-008; SC-003. Settled here (Clarifications, 2026-10-16): policy names become unique among policies, ignoring letter case and surrounding spaces, and a firearm or accessory row keeps naming its policy by name. That also ends the import bug described under "Today" (a same-named policy picked silently). How an imported policy row is matched to an existing policy, and everything else in this issue, stays in #54.
