# Data Model: Stable Record Identifiers, Unique Policy Names and Upgradable Databases

**Feature**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

What this feature adds or changes. Everything not named here is unchanged.
The stored changes are made in `0001_initial.sql` in place, the last time
before 0.1.0 (research.md §11), so existing development databases must be
created again.

## Stored: the database

### `insurance_policies` (amended)

| Column / rule | Change | Why |
|---|---|---|
| `uid TEXT NOT NULL UNIQUE CHECK (…)` | added, after `id`; the same `CHECK` as `firearms.uid` (006) | FR-001, research.md §1 |
| `insurance_policies_uid_fixed` | trigger added: `BEFORE UPDATE OF uid … WHEN NEW.uid IS NOT OLD.uid → RAISE(ABORT, 'a record identifier never changes')` | FR-001, FR-002 |
| `idx_insurance_policies_name` | `CREATE UNIQUE INDEX … ON insurance_policies (trim(name) COLLATE NOCASE)` | FR-007, research.md §2 |

`name` is still stored as typed. Two names conflict when they are equal after
removing surrounding U+0020 spaces and folding A–Z to a–z.

### `photos` and `document_attachments` (amended)

| Column / rule | Change |
|---|---|
| `uid TEXT NOT NULL UNIQUE CHECK (…)` | added, after `id`; 006's `CHECK` |
| `photos_uid_fixed`, `document_attachments_uid_fixed` | triggers added, as above |

There is no `*_uid_distinct` trigger between these tables, or between them
and firearms or accessories (FR-005).

### `disposition_history`, `firearm_types`, `action_types`, `registration_classes`, `accessory_kinds`

Unchanged: they get no identifier (FR-004).

### `app_state` (amended)

| Column | Change | Meaning |
|---|---|---|
| `backup_before_upgrade_to TEXT` | added, after `backup_of_name` | Set only in a backup made before an upgrade: the version it preceded, e.g. `0.2.0` (FR-013). `NULL` everywhere else. It is part of the backup stamp, so it is cleared with the stamp when a backup is opened directly or restored. |

`app_state` stays housekeeping, with no change-tracking triggers.

### `schema_migrations` (unchanged in shape)

`name TEXT PRIMARY KEY, applied_at TEXT NOT NULL`. It is still the data-layout
version (research.md §3). An upgrade inserts one row per step, all inside the
upgrade's one transaction (research.md §7).

### What an older layout must keep (research.md §13)

From 0.1.0 on, these keep their names and meanings in every layout:
- `schema_migrations.name`;
- `app_state`'s `id`, `database_id`, `open_machine_id`, `open_machine_name`,
  `open_since`, `changes_waiting`, `backup_made_at`, `backup_of_name` and
  `backup_before_upgrade_to`;
- `collection_settings.backup_location`;
- the table `pending_changes`.

`tests/upgrade_steps_test.rs` enforces it.

## Stored: upgrade steps (`src-tauri/src/db/`)

### `MIGRATIONS` (amended in meaning)

The ordered list of `(name, sql)` steps. Its rules:
- From the 0.1.0 tag on, entries are only ever appended (FR-021).
- A released entry's bytes never change.
- Every step obeys research.md §7's rules (pure SQL, the transaction
  belonging to the runner, the rebuild order, and depending only on the layout
  it starts from).

### `migrations/released.txt` (new)

```text
# <step name or fixture path> <sha256 of its bytes> <release>
# Lines are added at each release's tag (#19) and never edited.
```

Empty apart from comments until 0.1.0 is tagged. Then it gets one line for
each of `0001_initial`, `0002_fts5` and `0003_seed_firearm_types`, and one for
`tests/fixtures/releases/0.1.0.hoplodex`. `tests/released_migrations_test.rs`
checks every line (research.md §11).

### Test-only steps (not stored in any shipped build)

- `db::upgrade::testing`'s `PHOTOS_REBUILD` and `HISTORY_UID_FILL` (the
  `photos` and `disposition_history` rebuilds of research.md §14), applied
  through `testing::scope(dir).extra_steps(…)` to databases in that folder.
  Compiled only with the `upgrade-test-steps` feature, which `test-kdf` turns
  on and `custom-protocol` and `e2e` builds refuse.
- `9000_e2e_upgrade_check`, in `e2e` builds only: one empty table.

## Stored: backup files (amends 003 data-model.md "Backup files")

| Kind | File name | Stamp inside |
|---|---|---|
| Regular (unchanged) | `<name> <YYYY-MM-DD HHMMSS> <id8>.hoplodex` | `backup_made_at`, `backup_of_name` |
| Before an upgrade (new) | `<name> <YYYY-MM-DD HHMMSS> <id8> before <version>.hoplodex` | as above, plus `backup_before_upgrade_to = <version>` |

- `<version>` matches `[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?`.
- A name parses as a backup of the database `id8` only when its stem, after
  any ` before <version>` suffix is removed, ends in ` <date> <time> <id8>`.
- A `.partial` suffix still means "being written"; such files are never
  listed.

**Rotation** (amends 003 FR-025; FR-014): among the database's backups at its
location, the newest one with a `before` suffix is kept and not counted. The
newest `backup_keep_count` of the rest are kept, and the others are securely
deleted.

**Once a day** (003 FR-025): `has_backup_made_on` counts regular backups only.

## Stored: `machine.json` (amended)

### `UnfinishedBackup`

| Field | Change |
|---|---|
| `beforeUpgrade: bool` | added, `serde(default)` = `false`. When `true`, the launch sweep removes the partial file with no chooser notice (FR-015). |

### Reading (research.md §15)

- `notices` is read entry by entry, and an entry that doesn't parse is dropped
  and logged.
- An unknown `documentOpening` reads as the default.
- Unknown fields are ignored, as today.

An earlier or later version's file therefore never costs the recent list.

_Amended 2026-10-17 (the owner's decision D2, tasks.md T104): the file's own
`version` was the one thing still set aside, so a build now reads a file whose
`version` is higher than its own, by the fields it knows, with the same checks
on `machineId`; a lower (0), non-numeric or missing `version`, or a bad
`machineId`, is set aside as before. Loading writes nothing. The first change
written afterwards carries this build's `version` and only the fields it knows,
so a field only a later version knows is gone from the file then, and the later
version reads that file as it does any file of this layout._

## In memory: models and IPC types (`src-tauri/src/models/database.rs`, `src/features/databases/types.ts`)

### `OperationKind` (amended)

Adds `Upgrade` (`"upgrade"`): the backup before an upgrade and the upgrade
itself, begun by `upgrade_database`. Only the backup phase can be stopped.

### `BackupInfo` (amended)

| Field | Change |
|---|---|
| `beforeUpgradeTo: string \| null` | added, from the file name |

### `OpenedBackupNote` (amended)

| Field | Change |
|---|---|
| `beforeUpgradeTo: string \| null` | added, from the stamp: the opened file is a backup made before upgrading to that version |

### `DatabaseNotes` (amended)

| Field | Change |
|---|---|
| `upgraded: UpgradedNote \| null` | added; shown once after an open or a restore that upgraded the database (FR-017) |

### `UpgradedNote` (new)

| Field | Type | Meaning |
|---|---|---|
| `version` | string | `APP_VERSION`, the version it was upgraded for |
| `backupPath` | string | the backup made before it: for a restore, the backup restored from |
| `atBackupLocation` | bool | `false` when it was saved in a folder the user chose (FR-012), so it isn't in the backup list |

### `AwaitingUpgrade` (new, `session`)

The database waiting for the answer to FR-010's question (research.md §4), in
`SessionInner.awaiting_upgrade: Mutex<Option<AwaitingUpgrade>>`. It is not an
`OpenDatabase`, and no collection command can reach it.

| Field | Meaning |
|---|---|
| `id` | the `awaitingId` the chooser answers with, from the session-id counter, never reused in a run |
| `conn` | the keyed connection, holding the file's exclusive lock; nothing written |
| `path`, `name`, `database_id` | as an `OpenDatabase` has them; `database_id` read from `app_state` |
| `steps_due` | the step names to apply, in order |
| `marker_seen` | the open marker found at the question (another computer's when the user took it over), compared again under the lock at Upgrade |
| `backup_stamp` | the backup stamp, when the file is a backup opened directly (research.md §8), with the new id chosen for it |
| `save_passphrase` | `Option<Passphrase>`: only a typed passphrase the open will save or refresh in the keyring (003 FR-017, FR-018); zeroized when dropped |
| `backup_folder` | the backup location resolved for this computer |

Dropped by `cancel_upgrade`, by any other open, create or restore beginning,
by quit, sleep, screen lock and shutdown, and after a cancelled backup or a
failed upgrade. It is put back when the backup can't be made at its location
(FR-012).

### `BackupKind` (new, `services::backups`)

`Regular` or `BeforeUpgrade { to_version }`, passed in `BackupJob`. It decides
the name, the extra stamp field, the unfinished record's `beforeUpgrade`, and
that the backup record (`changes_waiting`, `last_backup_at`) isn't touched.

### `db::open_database`'s result (amended)

`open_database` returns `Opened::Ready(conn)` as today, or
`Opened::UpgradeDue { conn, steps_due, marker_seen }` with the write lock
taken and nothing written (research.md §4).

## State: an open that needs an upgrade

```text
chooser ──open_database──▶ checks 1–4 ─▶ steps due ─▶ write lock taken (nothing written)
                                                   │
                         UPGRADE_NEEDED { awaitingId } ─▶ waiting (AwaitingUpgrade)
                                                   │
       Cancel / sleep / screen lock / quit / another open ──▶ dropped: chooser, file untouched
                                                   │ Upgrade: upgrade_database
                                                   ▼
                           marker re-read ──another computer's──▶ DATABASE_OPEN_ELSEWHERE, dropped
                                                   ▼
                                               backingUp ──Cancel / shutdown──▶ dropped, partial removed,
                                                   │                           OPERATION_STOPPED
                                                   │ location, space or copy problem
                                                   ├──▶ waiting again: chooser offers a folder / Cancel
                                                   │      └─ folder ─▶ upgrade_database { backupFolder }
                                                   ▼
                                               upgrading (no Cancel)
                                      failure ─┤          ├─ success
                    UPGRADE_FAILED, rolled back,          ▼
                    dropped (file as it was)        open: notes.upgraded
   interruption anywhere ─▶ next launch: partial swept silently;
                            next open: the journal rolls back, the question again
```

_Amended 2026-10-17: in `backingUp` a sleep stops the backup as a shutdown does
(`OPERATION_STOPPED`, dropped, partial removed). In `upgrading` a sleep or
shutdown doesn't stop it: the upgrade finishes, this computer's open marker is
cleared, and the command ends with `OPERATION_STOPPED`, the database upgraded
and closed. A sleep, screen lock or shutdown that comes while nothing is in the
slot (during the open's key derivation, or while the backup's refusal is on its
way back) still drops the database (research.md §4)._

_Amended 2026-10-17: a screen lock during `backingUp` or `upgrading` stops
nothing, but with the database's "lock when the screen locks" on it ends like a
sleep in `upgrading`: upgraded, closed, `OPERATION_STOPPED` (research.md §5)._

## Validation rules added

| Rule | Where | Error |
|---|---|---|
| A policy name is unique among all policies, as compared above | `insurance::ops::create_policy`, `update_policy`; index backstop | `VALIDATION_ERROR`, `fieldErrors.name` naming the other policy (FR-008) |
| An imported row's policy name matches exactly one policy, as compared above, or none | `import_export` row parsing | row error `Unknown insurance policy: <name>` (FR-006) |
| A chosen backup folder is absolute, exists, is a directory, is writable and has room | `upgrade_database` | `BACKUP_LOCATION_UNAVAILABLE` or `INSUFFICIENT_SPACE`, with the folder (FR-012) |
| `uid` is present, a lowercase v4 UUID, unique in its table and never changed | schema `CHECK`, `UNIQUE`, trigger | `INTERNAL_ERROR` (never expected: ops always sets one) |
