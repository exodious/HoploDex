# Research: Stable Record Identifiers, Unique Policy Names and Upgradable Databases

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Date**: 2026-10-17

Each section gives the decision, why, and what else was weighed. "Today"
describes the code at `2a7d110`. The SQLite behaviours marked _probed_ were
checked with SQLite 3.54 from Python at planning (lock kept after an empty
exclusive transaction, rollback from a hot journal after a simulated crash, the
case-insensitive unique index and its use by the lookup). SQLCipher is the same
engine with page encryption added. The first tasks repeat each probe as a Rust
test against SQLCipher, because the design depends on them (§7, §14).

## §1 Record identifiers on policies, photos and documents (FR-001 to FR-005)

**Decision**: `insurance_policies`, `photos` and `document_attachments` each gain
`uid TEXT NOT NULL UNIQUE` with 006's `CHECK`: 36 characters, the v4 pattern,
the variant digit, lowercase hex only. Each also gets a `<table>_uid_fixed`
trigger (`BEFORE UPDATE OF uid … RAISE(ABORT)`), as `firearms` and
`accessories` have. They go in `0001_initial.sql`, edited in place (the last
time, §11). The value is set in `ops`, at the single `INSERT` of each table
(`commands/insurance.rs` `create_policy`, `commands/photos.rs` and
`commands/documents.rs`'s inserts), from `services::record_id::generate()`.
Nothing reads it yet: it isn't in any IPC type, export or screen (FR-003).

- **No cross-kind trigger.** `*_uid_distinct` stays between firearms and
  accessories only (FR-005). A spreadsheet's `mounted_on` names either of those
  two, and nothing names a policy, photo or document by identifier yet.
- **Disposition history and the built-in lists get none** (FR-004). A history
  entry is only ever restored or imported with its record, and a list entry is
  identified by its unique name.
- **Why the database refuses rather than defaults.** FR-001 asks the database
  to refuse a missing identifier, so there is no `DEFAULT`. 006 made the same
  choice for firearms.
- **Kept everywhere by construction.**
  - Edits, reorders (`photos.sort_order`), dispositions and reversals don't
    write `uid`, and the trigger refuses anything that does.
  - Backups and restores are byte copies (003 research.md §3).
  - A passphrase change is `sqlcipher_export`, which copies rows.
  - An upgrade copies rows (§7).
- **Tests and fixtures that insert directly** must now give a `uid`:
  `firearm_lifecycle_test`, `backup_due_tracking_test`, `photo_test`,
  `document_test`, `amount_cap_test`, `export_test`, `list_accessories_test`,
  `passphrase_change_test`, `performance_test`, `support/preview_support.rs`
  and `examples/human_seed.rs` (its document insert). A shared test helper
  gives them one, rather than each spelling out a UUID.

**Alternatives considered**:
- A `DEFAULT` expression that makes a v4 UUID in SQL. Rejected for new rows:
  the database could then never refuse a missing one (FR-001), and it would
  depart from 006. It is still the tool for a future upgrade step that must
  fill identifiers for existing rows (§7).
- One identifier table shared by every kind. Rejected: it adds a join to
  every insert for no current need.
- `AUTOINCREMENT` on the integer keys, to stop reuse. Rejected: it would stop
  reuse within one file, but #53 needs an identity that holds across files.

## §2 Unique policy names, and the comparison (FR-006, FR-007, FR-008; the Assumption on letter case)

**Decision**: a unique expression index on the stored name,
`CREATE UNIQUE INDEX idx_insurance_policies_name ON insurance_policies (trim(name) COLLATE NOCASE)`,
and the same comparison everywhere a name is matched. That is the policy
form's check and import's lookup, both
`WHERE trim(name) = trim(?) COLLATE NOCASE`. _Probed_: the index refuses
`"homeowners rider "` and `" HOMEOWNERS RIDER"` beside `"Homeowners rider"`,
and the lookup's query plan searches the index rather than scanning.

- **Letter case is folded for A–Z only.** SQLite's `NOCASE` folds only ASCII
  letters, so "Église" and "église" are two different names. That is what
  import's lookup (`name = :name COLLATE NOCASE`) does today, so the rule and
  the lookup agree, as the Assumption asks. Folding all of Unicode would need
  an app-defined collation registered on every connection. An index that
  depends on one can't be written, or even checked, by anything that hasn't
  registered it: an upgrade step (FR-022), `sqlcipher_export`, an inspection
  tool. Rust's own Unicode folding also changes with the Unicode version.
- **"Surrounding spaces" means spaces.** SQLite's one-argument `trim` removes
  U+0020 only, so a tab is part of the name. The form's check and the index
  use the same expression, so they can't disagree. Names are still stored as
  typed (today's behaviour); only comparisons trim them.
- **The form's error** (FR-008). `create_policy` and `update_policy` look the
  name up first, excluding the policy being edited (`AND id IS NOT ?2`). A
  match is a `VALIDATION_ERROR` with
  `fieldErrors.name = "“Homeowners rider” is already the name of another policy. Give this one a different name, such as “Homeowners rider 2026”."`.
  It quotes the other policy's name as it is stored, and the policy form
  shows it at its name field, as it does every field error. The index is the
  backstop. One connection behind the session's mutex means the check and the
  write can't race, so the backstop is expected never to fire. If it does,
  `from_db` maps it to `INTERNAL_ERROR`, as for 006's triggers.
- **Import** (FR-006). Both lookups in `commands/import_export.rs` (the
  firearm row's and the accessory row's `insurance_policy_name`) use the new
  comparison. An unknown name stays the row error
  `Unknown insurance policy: <name>`. With unique names a match is one policy
  or none, so the "first one found" bug in #54 can't happen.
- **Pending changes** (a policy draft kept at a lock) are checked when saved,
  like any other input.
- **Existing tests and seeds** that make two policies with the same name are
  changed to distinct names.

**Alternatives considered**:
- `UNIQUE COLLATE NOCASE` on the column itself. Rejected: it wouldn't ignore
  surrounding spaces, so `"Rider "` and `"Rider"` could both exist and an
  import of `"rider"` would be ambiguous again.
- A stored `name_key` column that the app computes. Rejected: the database
  couldn't check that the key matches the name, and a later upgrade step that
  renamed anything would have to call app code (FR-022).
- Trimming names when they are stored. Not needed for the rule, and it would
  change what an existing record shows. Left as it is.

## §3 What makes a database older or newer (FR-009)

**Decision**: unchanged in kind. The data layout version is the set of step
names in `schema_migrations`. A database is **older** when the build's
`MIGRATIONS` has a name the database lacks, and **newer** when the database
has a name the build lacks (003 FR-014, `has_unknown_migration`). Newer wins:
a database with an unknown name is refused even if it also lacks one.

Steps are applied in list order, and released steps are a prefix of the list
that never changes (§11). So a database's names are always a prefix of some
build's list. The steps due are the build's names after that prefix, applied
in order.

## §4 Opening a database that needs an upgrade (FR-009, FR-010, FR-011; Edge Cases)

**Decision**: the open stops at the question with the database keyed but
unchanged, and the backend holds it there until the user answers.

1. `db::open_database` runs today's steps 1 to 4, which are read-only: key,
   first read, not a newer version, the open marker. A database open on
   another computer is refused there as today, so the take-over question
   comes before the upgrade question, as the Edge Cases ask.
2. New step 4b: if steps are due, it takes the file's write lock without
   changing anything, `BEGIN EXCLUSIVE; COMMIT` under
   `locking_mode = EXCLUSIVE`. That mode keeps the lock afterwards (_probed_:
   a second connection's read then fails with "database is locked"), so
   another copy of HoploDex is told "in use" from here on. It returns the
   connection as `Opened::UpgradeDue`, with the steps due and the open marker
   it found. Nothing has been written to the file.
3. `lifecycle::open` keeps it in the session's new `awaiting_upgrade` slot, an
   `AwaitingUpgrade` (data-model.md). The slot is separate from `OpenDatabase`,
   so no collection command, scoped or not, can reach it. The command fails
   with `UPGRADE_NEEDED`, whose details give an `awaitingId` (from the same
   counter as session ids), the version and the backup folder. The chooser
   asks FR-010's question.
4. **Cancel** calls `cancel_upgrade { awaitingId }`. That drops the
   connection, which releases the lock and clears the key, and the database
   is closed untouched.
5. **Upgrade** calls `upgrade_database { awaitingId }`, which takes the held
   connection out of the slot and then:
   - **5a.** It reads the open marker again under the lock. If another
     computer has marked the database open since the question (a shared
     folder whose file locks don't reach this computer), it refuses with
     `DATABASE_OPEN_ELSEWHERE` and drops the connection, nothing written.
   - **5b.** It makes the backup before the upgrade (§6). It can be cancelled
     here (§5).
   - **5c.** It runs the upgrade transaction (§7). This also sets the open
     marker, so "this computer holds it" (FR-009) and the upgrade are one
     atomic change.
   - **5d.** It continues as every open does: `prepare`, `install`, the recent
     list, the saved passphrase. The status carries `notes.upgraded` (FR-017).

**The slot's life**: at most one database waits, and it waits only for its
answer.
- **Dropped** by `cancel_upgrade`, by the start of any other open, create or
  restore, by `quit_application`, by sleep, screen lock and shutdown (with
  `system:clear-passphrase-fields`, which also closes the question in the
  chooser as Cancel), and by the app ending.
- **Kept** when the backup can't be made at the backup location (FR-012), so
  choosing a folder retries without the passphrase being typed again. It is
  put back in the slot, and the command's error names the problem.
- **Not kept** after a cancel during the backup, or a failed upgrade (FR-015,
  FR-016).

**The passphrase**: the backend still never keeps one beyond the open it is
for (003 FR-007).
- The held connection holds SQLCipher's derived key, as an open database
  does, and not the passphrase.
- The one exception is a typed passphrase the open will save in the keyring
  (`rememberPassphrase`, or a saved one being refreshed, 003 FR-018). Saving
  happens once the open succeeds, so that `Passphrase` stays in the
  `AwaitingUpgrade` until the upgrade ends. It is zeroized when the slot is
  dropped, which a sleep or screen lock does.
- The chooser keeps no passphrase while the question is up: it was read and
  cleared when the open was sent, as for take-over.

**Alternatives considered**:
- Refuse without holding anything, and resend the open with consent. It needs
  the passphrase again: either the chooser keeps it while the user decides,
  which 003's take-over confirmation deliberately avoids (003
  contracts/ui-databases.md §1), or the question asks for it again. A
  passphrase field in the question would take the focus FR-010 gives to
  **Upgrade**. It would also derive the key a second time.
- Open the database first (writing the open marker), then ask. Rejected: Cancel
  could no longer leave the file untouched (FR-010), and an `OpenDatabase` that
  serves nothing until answered would be one more state for every session
  path.

## §5 The question, the progress and the result, in the interface (FR-010, FR-016, FR-017)

**Decision** (wording and layout in contracts/ui.md §1 to §5):

- **The question** is a `ConfirmDialog` from the chooser. It is non-destructive,
  so it opens with focus on **Upgrade** (the One Enter rule). It names the
  version (`APP_VERSION`), says a backup is made first and where, and says
  earlier versions will open only that backup.
- **Progress** is a dialog over the chooser, with a `ProgressBar` and the
  phase.
  - It shows at once when the backup is expected to take more than a second
    (`backups::shows_progress_at_once`). Otherwise it shows only if the open
    is still running a second later, as 003 SC-005 does at close.
  - **Cancel** is offered only while the backup is being made (FR-016). It calls
    the new unscoped `cancel_upgrade`, which stops the `upgrade` operation.
    The backup then stops between chunks, its partial file is removed,
    `upgrade_database` returns `OPERATION_STOPPED` (`operation: "upgrade"`),
    the held connection is dropped, and nothing has been written to the
    database.
  - Once the phase is `upgrading`, Cancel goes and the operation ignores a
    stop.
- **The operation** is a new `OperationKind::Upgrade` in the session's
  registry, begun when `upgrade_database` starts. Progress goes out as an
  `upgrade:progress` event, `{ phase: "backingUp" | "upgrading", processed,
  total, showNow }`.
- **Sleep and shutdown.** While the question is up, a sleep, screen lock or
  shutdown drops the waiting database (§4). Once **Upgrade** is chosen:
  - A shutdown or signal stops the operation, as `close_immediate` stops any.
    During the backup that is a Cancel. During the upgrade the stop is
    ignored and the process ends when the OS ends it, which is an
    interruption (§7).
  - A sleep doesn't stop it. No database is open yet, so there is no idle-lock
    setting to apply. The OS suspends the copy, and on waking
    `finish_on_wake` locks the database if it opened meanwhile and its idle
    lock is on.
  - Neither leaves a chooser notice (FR-015: after an interruption, nothing
    more is reported).
- **The result** is `DatabaseNotes.upgraded = { version, backupPath, atBackupLocation }`.
  It is shown once in the collection's notes area, like the "backup opened
  directly" note (FR-017).
- **A failure while running** is one of two kinds:
  - In the backup phase (the location check, the copy, the stamp, the
    rename), it is `UPGRADE_BACKUP_FAILED` (or the location refusals), the
    partial file is removed, and the database stays waiting so a folder can
    be chosen (§6, FR-012).
  - From the first step on, it is `UPGRADE_FAILED`, which always carries
    `backupPath`, since the backup was finished before any step ran. It is
    shown in the chooser's row, as other open failures are, in
    contracts/ui.md §4's words.

## §6 The backup before an upgrade (FR-011 to FR-014)

**Decision**: `services::backups::make_backup` gains a `kind`:
`BackupKind::Regular` or `BackupKind::BeforeUpgrade { to_version }`. The
pre-upgrade backup is made exactly like a close's, through the database's own
file handle, with the location check, the room check, the `.partial` file, the
unfinished-backup record, the stamp and the rename. The kind adds four things:

- **Its name ends in ` before <version>`**:
  `<name> <YYYY-MM-DD HHMMSS> <id8> before <version>.hoplodex`, for example
  `Main collection 2027-01-15 093012 3f9c0a12 before 0.2.0.hoplodex`.
  - `made_at_by_name` first strips a trailing ` before <version>`, where the
    version is `[0-9]+.[0-9]+.[0-9]+` with an optional `-` or `+` suffix of
    `[0-9A-Za-z.-]`.
  - It then parses the rest as today. An ordinary backup's stem ends in its
    `id8`, so no ordinary name is ever read as having the suffix, whatever the
    database is called.
  - `BackupInfo` gains `beforeUpgradeTo: string | null` from it. That makes it
    identifiable in every list (FR-013).
- **Its stamp also sets** `app_state.backup_before_upgrade_to` (a new column,
  §11) to the version. So the "backup opened directly" note can say "made
  before it was upgraded to HoploDex 0.2.0" (FR-013).
- **Settings don't apply to it.** It is made whether automatic backups are on
  or off and whatever the once-a-day limit says (FR-011, as 003 FR-028's backup
  before restoring). It doesn't touch `changes_waiting` or `last_backup_at`,
  and `has_backup_made_on` ignores it, so it never counts as the day's backup
  (Assumptions).
- **Its unfinished record says so.** `UnfinishedBackup` gains
  `beforeUpgrade: bool` (`serde(default)`), and the launch sweep removes such
  a partial file without a chooser notice (FR-015: an interruption is not
  reported). An ordinary unfinished backup still gets its "will be backed up
  at the next close" notice.

**Rotation** (FR-014): `rotate` sets aside the newest backup whose name has a
` before ` version, then keeps the newest `keep` of the others. Older
pre-upgrade backups are among "the others" and rotate normally. The kept one
is replaced as "most recent" by the next upgrade's backup. `delete_all_backups`
deletes it like any other (003 FR-029), and a move of the backup location
moves it.

**Location problems** (FR-012): `upgrade_database` checks the backup location
and its room before writing anything, as `make_backup` already does.
- A problem is refused with today's `BACKUP_LOCATION_UNAVAILABLE` (`missing`,
  `notWritable`, `insufficientSpace`) or `INSUFFICIENT_SPACE`, or with
  `UPGRADE_BACKUP_FAILED` for an I/O failure during the copy. The details name
  the folder.
- The chooser then offers **Choose a folder…** or **Cancel**. A chosen folder
  goes back as `upgrade_database { awaitingId, backupFolder }`, the database
  still waiting in its slot (§4).
- The backend checks that folder the same way. It must be absolute, an
  existing directory, writable and roomy, and it is never created. A failure
  offers the choice again.
- The backup is written into it under the same name, created exclusively, and
  no file is ever overwritten (003 FR-026).
- A chosen folder that resolves to the database's own backup location
  (`same_path`) is simply the backup location. Any other folder is outside
  the listing and rotation, and FR-017's message names the path.

**Pending changes and the open marker in the backup**: stamped as every backup
is (003 research.md §9). The marker is cleared and `pending_changes` emptied.
Those are housekeeping, which is never in a backup. The collection, its
settings and its attachments are byte-identical. That is what SC-004's
"content identical" checks (§12). The pending changes themselves survive in
the upgraded database (FR-015).

**What 0.1.0 must already know** (Assumptions): 0.1.0 is the version that will
be handed this backup. It needs four things, and all of them ship in this
feature, so they are in 0.1.0:
- the ` before <version>` name, so it can list the backup and protect it in
  rotation;
- the `backup_before_upgrade_to` column, so its stamp can be written into a
  0.1.0-layout copy and read back when the copy is opened;
- the stamp's note;
- FR-014's rotation rule.

**Alternatives considered**:
- Mark the backup only inside the file. Rejected: rotation and listings read
  names, never contents (003 research.md §7). They would have to decrypt
  every backup to find the one to keep.
- A separate "before upgrade" folder. Rejected: the backup should be found
  where the user's backups are, and listed and restorable like the others.
- Put the marker before the date (`<name> before 0.2.0 <date> …`). Rejected:
  a database really named "Guns before 1.0.0" would be ambiguous.

## §7 The upgrade itself: one transaction, rebuilds, and the link check (FR-015, FR-020, FR-022)

**Decision**: `db::apply_migrations` becomes `db::upgrade` (a new
`db/upgrade.rs`), the one runner for creating a database and for upgrading
one:

1. `PRAGMA foreign_keys = OFF`, outside any transaction (inside one it is a
   no-op).
2. `BEGIN IMMEDIATE`.
3. Read `app_state.changes_waiting` (on a new database `app_state` doesn't
   exist yet, so there is nothing to keep).
4. For each due step, in order: `execute_batch(sql)`, then insert its
   `schema_migrations` row.
5. The open marker (an open, not a create).
6. `PRAGMA foreign_key_check`. Any row abandons the upgrade, logged with the
   table and row id, never the data.
7. Put `changes_waiting` back as it was. A step that updates rows would
   otherwise fire the change-tracking triggers, and the upgrade isn't a
   collection change (Assumptions).
8. `COMMIT`, then `PRAGMA foreign_keys = ON`.

Any error before the commit rolls back everything: every step, the marker and
the identifiers. `last_saved_version` is not recorded, since an upgrade isn't
a save (008 FR-015, unchanged).

**Why one transaction for all steps**: FR-015's all-or-nothing covers "every
step it applies", and several releases skipped means several steps (Edge
Cases). Per-step transactions would leave a database half-way between two
releases after a failure.

**Atomic across power loss**: SQLite's rollback journal (`journal_mode` stays
at its default, `DELETE`) makes the commit atomic.
- An interrupted upgrade leaves a hot journal beside the file. The next
  connection to read it with the right key rolls it back. That is the first
  read of the next open, before `has_unknown_migration` looks.
- So the next open finds the old layout and asks again, as if for the first
  time (FR-015).
- _Probed_: a rebuild of a 5,000-row table was interrupted mid-transaction,
  with `cache_size = 1` so that changed pages had spilled into the file.
  Copying the file and its journal and opening the copy rolled back to the
  original rows and layout, and `integrity_check` said `ok`.
- `secure_delete` (on for every connection) zeroes the pages of a table a
  rebuild drops.

**Rules for an upgrade step** (FR-020, FR-022), recorded in
`db/migrations/README.md` and checked by `tests/upgrade_steps_test.rs`:

- **Pure SQL.** No `BEGIN`, `COMMIT`, `ROLLBACK`, `SAVEPOINT`, `RELEASE`,
  `PRAGMA foreign_keys`, `VACUUM`, `ATTACH`, `DETACH`. The runner owns the
  transaction, and those break it or are no-ops inside it. No app-defined
  functions or collations. The test scans every step's text for the
  forbidden statements.
- **A rebuild follows SQLite's documented order**:
  1. `CREATE TABLE new_x (…)`;
  2. `INSERT INTO new_x (…) SELECT … FROM x`;
  3. `DROP TABLE x`;
  4. `ALTER TABLE new_x RENAME TO x`;
  5. then every index, trigger and full-text trigger of `x` is created again.

  Never rename `x` away first: since SQLite 3.26 that rewrites the other
  tables' foreign keys to follow the old name.
- **Filling a new required unique value**, such as a later table's `uid`, uses
  SQL alone. The v4 expression is

  ```sql
  lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4'
    || substr(lower(hex(randomblob(2))), 2) || '-'
    || substr('89ab', 1 + (abs(random()) % 4), 1)
    || substr(lower(hex(randomblob(2))), 2) || '-' || lower(hex(randomblob(6)))
  ```

  which 006's `CHECK` accepts. `tests/upgrade_steps_test.rs` checks it against
  the `CHECK` and `record_id::parse`.
- **Depends only on the layout it starts from**: no reading of app settings or
  of values only the app can interpret.

**What can still fail after the commit**: `prepare` reading the upgraded
database. The upgrade has then succeeded. The open reports the failure as any
open failure, and the next open opens it without asking (nothing is due).
That is "fully upgraded", FR-015's first outcome.

**Alternatives considered**:
- Upgrade a copy and swap it in with `file_swap`, as a restore does. Rejected
  as the default: it needs room for a third copy (database, backup, upgraded
  copy), and the journal already gives atomicity. It stays the method for the
  restore (§9), where the copy exists anyway.
- One transaction per step. Rejected above.
- `PRAGMA legacy_alter_table = ON` around rebuilds. Not needed with the
  documented order. It also changes trigger and view rewriting in ways a step
  author wouldn't expect.

## §8 A backup opened directly that needs an upgrade (Edge Cases, 003 research.md §9)

Today `prepare` takes a backup's stamp when one is opened directly. It gives
the file a new `database_id` and clears the stamp, so the copy becomes a
database of its own.

**Decision**: for a stamped file the stamp is taken inside the upgrade
transaction, not before the question. Cancel then leaves the file untouched
and still a backup (FR-010).
- The new id is chosen before the backup before the upgrade is made.
- That backup is named with the new id's `id8` and stamped with the new id, so
  it belongs to the database the file becomes.
- It goes to the backup folder the file's own settings name, resolved from
  the file's path (Edge Cases).
- The transaction's stamp-taking returns the `opened_backup` note, which
  `prepare` then shows as it does today.

## §9 Restoring a backup in an older layout (FR-019)

**Decision**:
- **A pre-check.** A new unscoped command, `check_backup(backupPath,
  passphrase, databasePath?)`, opens the backup read-only with its passphrase.
  It returns `{ upgradeTo: string | null }`, or refuses as `restore_backup`
  would: `PASSPHRASE_INCORRECT`, `DATABASE_DAMAGED`, `DATABASE_NEWER_VERSION`,
  or the damaged restore's authorization refusals (#62) when `databasePath`
  is given.
- **One confirmation.** `RestoreBackupDialog` calls it when Restore is pressed,
  before its confirmation. A wrong passphrase is then reported at the field
  without a confirmation in between. The confirmation itself adds FR-019's
  sentence when `upgradeTo` is set (contracts/ui.md §6), so there is one
  confirmation, not two.
- **Consent on the restore.** `restore_backup` gains `upgrade: bool`. A backup
  that needs one, restored without it, is refused with `UPGRADE_NEEDED` before
  anything is copied. The backend never upgrades without the confirmation
  having said so.
- **Upgrading the copy.** The restore already prepares the backup's copy at
  `.<file>.new` and proves it sound before replacing anything. The copy is now
  also upgraded there, after `check_copy` and the restore's stamp, by the same
  runner (§7), with no open marker. That adds an `upgrading` phase to
  `restore:progress`. A failure removes the copy, and the database is as it
  was, which is FR-015 for free.
- **What stays.** The "before restoring" backup of the current database is
  made as today, in the current layout (003 FR-028). The backup restored from
  stays where it is and is the copy in the earlier layout, so no other backup
  is made (FR-019).
- **Rotation.** The backup restored from is not renamed or marked: it is an
  ordinary backup made when it was made, and it rotates as one. FR-014
  protects backups made before an upgrade, and this one wasn't. A user who
  wants the earlier layout kept has it until rotation reaches it.
- **The result.** `notes.upgraded` is set with the restored-from backup's path
  (FR-017's wording applies).
- **The damaged-database restore** from the chooser (003 US3-6) gets the same
  upgrade of the copy.

**Alternatives considered**:
- Ask after the restore's confirmation with a second dialog. Rejected:
  FR-019 says the restore's confirmation says so, in place of the separate
  question.
- Use the overridable-warning resend (`confirmedWarnings`) on `restore_backup`.
  Rejected: the restore's confirmation is the frontend's own dialog today, and
  a pre-check also moves the wrong-passphrase error before it, which is
  better.
- Protect the restored-from backup in rotation by renaming it. Rejected:
  renaming a backup the user chose is surprising, and its stamp wouldn't
  match its name.

## §10 Refusing a newer database (FR-018)

**Decision**: `DATABASE_NEWER_VERSION` gains `details.backupFolder`, the
folder this computer cached for the database (its recent entry) when there is
one. The chooser's text becomes:

> “X” was last used by a newer version of HoploDex, which upgraded it.
> Update HoploDex to open it. The file has not been changed. A backup saved
> before that upgrade opens in this version: look for one whose name ends in
> “before” and a version number, in <folder>.

Without a cached folder, <folder> is "the “HoploDex backups” folder next to
the database, or the backup location you chose for it" (contracts/ui.md §8). This
wording ships in 0.1.0 (FR-018). The same text is used when a restore or
`check_backup` meets a newer backup.

## §11 Keeping released steps unchanged (FR-021)

**Decision**:
- **The manifest.** A manifest `src-tauri/src/db/migrations/released.txt`
  holds one line per released step, `<name> <sha256 of the file's bytes>
  <release>`, plus one line for each stored release database (§12). It is
  empty, apart from a comment, until 0.1.0 is tagged. The release process
  (#19) adds the 0.1.0 lines at the tag.
- **The test.** `tests/released_migrations_test.rs` fails:
  - when a listed step's bytes no longer hash to its line;
  - when a listed step is missing from `MIGRATIONS`, renamed or out of order
    (the released steps must be the list's first entries, in order);
  - when a listed fixture's bytes have changed.

  The failure message names CLAUDE.md's rule.
- **Hashing** uses `sha2`, added as a dev-dependency. It is already in the
  build through `secret-service` and `tauri-codegen`, so no new code arrives
  (§18). `.gitattributes` gives every text file LF line endings in every
  checkout, so the bytes hash the same on Windows.
- **CLAUDE.md**'s "Schema changes edit the existing migrations in place"
  becomes the new rule: "until 0.1.0 is tagged; from then on every change is a
  new step in `db/migrations/`, the released ones never change
  (`released.txt`, `tests/released_migrations_test.rs`), and each new step is
  covered by `tests/release_upgrade_test.rs` (§12)". DEVELOPMENT.md gains a
  short "Changing the data layout" section with §7's step rules.
- **This feature's own layout changes** (§1, §2, `app_state.backup_before_upgrade_to`)
  are the last made in place: `0001_initial.sql` is edited, and existing
  development databases must be recreated (Edge Cases). The developer's
  databases are not touched.
- **Test-only steps** (§14) live outside `MIGRATIONS` and outside the manifest.

**Alternatives considered**:
- Frozen copies of the released `.sql` files, compared byte for byte.
  Rejected: a thousand duplicated lines, and editing both is as easy as
  editing a hash.
- Comparing against the `v0.1.0` git tag. Rejected: the tests would need git
  history and the tag in every checkout, which a shallow clone or a source
  archive lacks.

## §12 Stored databases made by each release (FR-023, SC-004)

**Decision**:
- **The fixture.** `src-tauri/tests/fixtures/releases/0.1.0.hoplodex`, made
  by `examples/release_fixture.rs`. It seeds a database with
  `human_seed.rs`'s "Main collection" seeding, so every kind of record,
  link, setting and attachment is covered as `human_seed_coverage_test`
  requires. Then it adds pending changes and a backup record, and takes the
  pinned KDF and a fixed passphrase. It refuses a `test-kdf` build, as
  `portable_fixture.rs` does.
- **Size.** The aim is under 2 MB (small images, no `--extra` firearms),
  measured when it is made. It is committed with the rest of the repository.
  It is 0.1.0's sample, not the developer's data.
- **Upgrade test.** `tests/release_upgrade_test.rs`, for each stored release
  database:
  - It reads every table's rows from a copy without upgrading it (a raw
    keyed connection: no steps run). Blobs are compared by SHA-256, and
    `app_state`'s marker fields and `pending_changes` are kept apart.
  - It opens another copy through `lifecycle::open` and then
    `upgrade_database`'s ops, as the app does.
  - It checks that the backup before the upgrade was made at the backup
    location, with its ` before <version>` name and stamp, and that the
    backup's collection rows equal the copy's (SC-004).
  - It checks that the upgraded database's rows equal the copy's after each
    step's declared change, a per-step function in the test, empty until a
    step exists. The identifiers, pending changes and settings are included
    (FR-015).
  - It checks that every table has a row and every column a non-null value
    somewhere in the fixture, so "holds every kind" can't silently erode.
- **Until 0.1.0 is released**, `release_fixture_matches_layout` fails when
  the fixture's `sqlite_schema` differs from a new database's. The failure
  says to rerun the example. Today there is no step after 0.1.0, so the
  upgrade test exercises the backup and the "nothing to change" path. The
  test-only steps (§14) exercise the rest.
- **From the release on**, the fixture's hash goes in `released.txt` (§11)
  and the layout check stops applying to it. Each later release adds its own
  fixture the same way.

**Alternatives considered**:
- A committed dump of expected values. Rejected: the dump-before /
  dump-after comparison checks every value without a second copy to keep in
  step. The coverage check stops the fixture from thinning out.
- Making the fixture at test time from the current code. Rejected: from 0.1.0
  on, it must be what 0.1.0 made, not what today's code makes.

## §13 What a build may touch in an older layout before upgrading it

The open reads and writes some of an older database before any step runs, and
the backup is stamped in the older layout. Those parts of the layout must mean
the same thing in every release from 0.1.0 on:

- `schema_migrations (name)`;
- `app_state`'s `id`, `database_id`, `open_machine_id`, `open_machine_name`,
  `open_since`, `changes_waiting`, `backup_made_at`, `backup_of_name` and
  `backup_before_upgrade_to`;
- `collection_settings.backup_location`;
- the table `pending_changes`, which the stamp empties.

**Decision**: `db/upgrade.rs` documents this list. `tests/upgrade_steps_test.rs`
checks that a new database still has every one of these columns. A later step
that drops or renames one fails the test. If one ever must change, the open
has to read the old and new layouts both, and that is a decision for that
release.

## §14 Testing the upgrade: test-only steps, forced failures and interruptions (SC-005, US4)

**Decision**: the hooks are compiled only with a Cargo feature of their own,
`upgrade-test-steps`.
- `test-kdf` turns it on, so the usual test command is unchanged.
- The release `performance_test` is built with it alone. `test-kdf` can't be
  used there, because `db::cipher` refuses it without debug assertions, and
  SC-007 must be timed in a release build.
- `db/upgrade.rs` refuses to compile it with `custom-protocol` (every shipped
  build) or `e2e`, as `db::cipher` refuses `test-kdf`. The app (`main.rs`) and
  the examples refuse to run with it. So no shipped or E2E build has them.

**Scope.** Every hook is registered for a folder, `testing::scope(dir)`, and
applies to any database whose file is in that folder, on whichever thread
works on it. `upgrade_database` is `async` and may run on another thread
than the test's, and a cancel test needs two threads, so a thread-local guard
wouldn't reach it. A folder keeps concurrent tests apart, since each test has
its own temporary folder, and it covers the restore's `.<file>.new` copy
beside the database. The registry is process-wide behind a mutex; a scope
clears its entries when dropped.

`db::upgrade::testing` provides:

- **Extra steps.** `scope(dir).extra_steps(&[(name, sql)])` appends test-only
  steps to `build_steps()` for databases in that folder. The step SQL is
  kept in the module as constants, so every test program (the integration
  tests and `performance_test`) uses the same text:
  - `PHOTOS_REBUILD`: a rebuild of `photos`, which links to firearms and
    accessories and is linked from their `thumbnail_photo_id`, changing a
    `CHECK` and recreating its indexes and triggers;
  - `HISTORY_UID_FILL`: a rebuild of `disposition_history` that adds a
    required unique identifier filled with §7's expression.

  They are US4's independent test, and `release_upgrade_test` runs them over
  the 0.1.0 fixture.
- **Forced failures.** `scope(dir).fail_at(point)`. The points are: before the backup;
  during the copy (I/O error); at the stamp; before the backup's rename; after
  each step; at the link check (an orphan row is planted in a test step); and
  before the commit. Each run then checks that the database's rows and
  layout are exactly the fixture's and that no `.partial` file is left
  (SC-005). A point in the backup phase (before the backup, during the copy,
  the stamp, the rename) gives `UPGRADE_BACKUP_FAILED` with the database
  still waiting, so the same `awaitingId` can retry. A point from the first
  step on gives `UPGRADE_FAILED` with `backupPath`, the database is dropped,
  and the next open asks again.
- **Interruptions.** `scope(dir).at_point(point, hook)` runs `hook(&Connection)` at a
  point. The interruption test sets `cache_size = 1` so that changes spill
  into the file. At each point inside the transaction it copies the database,
  through `RawFile` (never `std::fs`, which would drop the lock), and its
  journal to another folder. It then opens the copy and checks: the copy
  rolls back, holds the fixture's rows, and asks again; a backup finished
  before the point is still there under its name; and a `.partial` left at
  the point is removed by the launch sweep without a notice (FR-015).
- **Cancel.** A test cancels between backup chunks through the operations
  registry. The partial file is removed, the error is `OPERATION_STOPPED`, and
  the file's bytes are unchanged (a SHA-256 of the whole file before and
  after).
- **The probes as tests.** The lock kept after `BEGIN EXCLUSIVE; COMMIT` under
  SQLCipher, and the hot-journal rollback, each get a test in
  `upgrade_steps_test.rs`, among the first tasks.

_Probed on Linux (T005, `tests/upgrade_steps_test.rs`, SQLCipher with the
`test-kdf` settings): both probes hold, so §4 step 2 and §7 stand as written._
- _`BEGIN EXCLUSIVE; COMMIT` leaves the file's SHA-256 unchanged, a second
  connection's first keyed read fails with "database is locked" until the
  probing connection is dropped, and another process is refused the same way
  (SQLite shares lock state inside a process, so only a child process shows
  the operating system's lock is held)._
- _The copy of a mid-rebuild file and its `-journal` opens rolled back to the
  original rows and `sqlite_schema`, and `integrity_check` is `ok`. The test
  first asserts that changed pages had reached the file (the copy differs from
  the original), so the probe isn't vacuous._
- _Found while probing: under `locking_mode = EXCLUSIVE` a rolled-back journal
  is not deleted, only its header zeroed, so a `-journal` file may stay beside
  an open database and a rolled-back copy until the connection closes. Its
  presence therefore says nothing about whether it is hot._
- _macOS 26 (T006, tart VM, standard user, same filter): both probes hold and
  the child-process helper runs, 3 tests passed, so §4 step 2 and §7 stand as
  written there too. Nothing the probes check differed from Linux._
- _Windows Server 2025 (T007, same filter): both probes hold, 3 tests passed
  including the child-process check, so §4 step 2 and §7 stand as written.
  Windows' mandatory `LockFileEx` locks made no difference: after `BEGIN
  EXCLUSIVE; COMMIT` the lock refused the second connection and the child
  process with "database is locked", and reading the file and its `-journal`
  through `RawFile` while the lock was held raised no sharing error._

**E2E** needs a database older than the app.
- An `e2e` build appends one test-only step, `9000_e2e_upgrade_check`, which
  creates an empty table and changes nothing else. It sits behind
  `cfg(feature = "e2e")`, in a list of its own beside `MIGRATIONS`, so it is
  never in a shipped build; a Rust test (built without `e2e`) checks that the
  steps an open applies are exactly `MIGRATIONS`.
- The upgrade journey copies the stored 0.1.0 fixture, which lacks that step,
  into the sandbox and opens it. Databases the E2E suite creates itself have
  the step, so no other spec sees the question.
- `scripts/human-testing.sh --release --older` puts the same copy in the
  human-testing sandbox, as "Older collection", for the manual checks.
- **Holding the phases on screen.** The fixture upgrades in well under a
  second, so the screenshot walk and the screen-reader check couldn't see
  each phase. An `e2e` build reads `HOPLODEX_E2E_UPGRADE_PHASE_SECONDS`, as
  it reads `HOPLODEX_E2E_IDLE_MINUTE_SECONDS`. When it is set, the backup
  phase waits that long before its first chunk (stoppable, like a chunk),
  and the upgrade phase waits that long before the commit. No other build
  reads it.

## §15 The recent list read by an earlier version (found at planning)

The downgrade this feature supports (0.2.0's backup opened in 0.1.0) runs
0.1.0 against the `machine.json` that 0.2.0 last wrote. Today
`MachineSettings::load` sets an unparsable file aside and starts afresh, and a
notice kind or enum value that 0.1.0 doesn't know makes the file unparsable.
The user would lose the recent list on going back.

**Decision**, shipped in 0.1.0 with this feature:
- `machine.json`'s notices are read one by one, and a notice that doesn't
  parse is dropped with a log line.
- An unknown `documentOpening` value reads as the default.
- Unknown fields are already ignored (no `deny_unknown_fields`).

A test writes a `machine.json` with an unknown notice kind and an unknown
field and checks that the recent list survives. This follows the owner's rule
that related problems found mid-task are fixed in the same branch; it is
listed in plan.md for a look.

## §16 Performance (constitution IV, SC-007)

- **An open with nothing due** adds one read of `schema_migrations`, which it
  already does for `has_unknown_migration`. The 003 SC-003 budget (1 s) is
  unaffected, and `performance_test.rs` keeps timing it.
- **The backup** takes as long as a close's: proportional to the file, with
  the same progress and the same estimate.
- **The upgrade** takes as long as its steps. The test-only `photos` rebuild
  over 10,000 items is timed in `performance_test.rs` and reported in the pull
  request. There is no budget for it: FR-016 asks only that progress shows and
  the interface stays responsive.
- **Progress within a second** (SC-007): the event comes before the first
  chunk, with `showNow`, and the chooser's dialog shows on it or after a
  second. `performance_test.rs` checks that the first `upgrade:progress`
  arrives within 1 s of `upgrade_database` being called on a 10,000-item
  database that `testing::PHOTOS_REBUILD` makes older. It runs in a release
  build with `upgrade-test-steps` (§14).
- **The interface stays responsive**: `upgrade_database` is `async` and runs off
  the UI thread, as today.

## §17 Security and data handling (for the pull request)

- **The backup** is a byte copy of the encrypted file, so it opens only with
  the passphrase current when it was made. It is written only to local
  storage: the backup location the user set, or a folder they choose for this
  one backup. The question names where. Nothing goes to a network service
  (Security & Data Handling Constraints).
- **A chosen folder** comes from the web view, as the backup location setting
  already does. The backend accepts only an existing, absolute directory. It
  writes one file there with a fixed name pattern, created exclusively, never
  following or replacing what is there (#63's `scratch::create`). A partial
  file is securely deleted.
- **The passphrase** is not kept while the user decides (003 FR-007): the
  waiting database holds SQLCipher's derived key, as an open one does, and
  sleep, screen lock and shutdown drop it. Only a passphrase the open will save
  in the keyring is kept until the upgrade ends, zeroized when it is dropped
  (§4). The chooser keeps none.
- **Upgrade steps** are pure SQL compiled into the binary. They can't be
  supplied at run time. Test-only steps exist only in Rust test builds with
  `upgrade-test-steps`, which a `custom-protocol` (shipped) or `e2e` build
  refuses to compile, and in `e2e` builds (one empty table). A dropped table's pages are zeroed (`secure_delete`).
- **Identifiers** are random (OS RNG), never shown, and not exported.
- **`check_backup`** reveals only whether a passphrase opens a backup and its
  layout. With `databasePath`, it is authorized as the damaged restore is
  (#62).
- **New commands** (`upgrade_database`, `cancel_upgrade`, `check_backup`) go
  through `commands_list.rs`, the ACL manifest and `capabilities/default.json`
  for the main web view only. All three are unscoped (`UNSCOPED_COMMANDS`),
  because no session exists when they run. The waiting database is reached
  only by its `awaitingId`, never by a path from the web view.

## §18 Dependencies and licensing

No new runtime dependency. One new Cargo feature, `upgrade-test-steps`
(§14), which pulls in no crate. `sha2` 0.10 becomes a dev-dependency of
`src-tauri`. It is already in `Cargo.lock` through `secret-service` and
`tauri-codegen`, is MIT/Apache-2.0, and has no advisory. The audit runs
unchanged.

## §19 Residual risks

- **Filesystems that ignore `fsync`** (some network shares and USB bridges)
  can tear any SQLite commit after a power loss, an upgrade's included. The
  backup before the upgrade is the remedy. The final message names it.
- **A waiting database holds its file's lock** until the user answers (§4).
  A question left up keeps another copy of HoploDex out, as an open database
  does.
- **The restored-from backup rotates normally** (§9). After enough closes, the
  earlier-layout copy of a restored database may be gone, unless the user
  keeps it elsewhere.
- **Letters outside A–Z aren't folded** (§2): "Église" and "église" can be two
  policies, and import keeps them apart, as it does today.
- **A backup opened directly in an earlier version becomes a database of its
  own**, with a new database id (003 research.md §9). Its records keep their
  identifiers. Going back to 0.1.0 therefore means a second database in the
  recent list, not the original one in its old place.
