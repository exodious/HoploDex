# Contract: The HoploDex Database File (feature 009 delta)

A delta against 003's `contracts/database-file.md`. The cipher settings, the
open marker protocol and the file replacement are unchanged.

## The data layout version (amends "The file")

- **The version** is the set of step names in `schema_migrations`
  (research.md §3).
- **Older**: a build with steps the file lacks upgrades it, only after
  asking (FR-010).
- **Newer**: a file with a step the build doesn't know is refused unchanged
  (003 FR-014, with 009 FR-018's wording).
- **From the 0.1.0 tag on**, steps are only appended, and a released step's
  bytes never change. `src-tauri/src/db/migrations/released.txt` records each
  one's SHA-256, and `tests/released_migrations_test.rs` enforces it
  (FR-021).

## Upgrading the file in place (new)

- **One transaction.** All the steps due run in one SQLite transaction with
  foreign keys off, followed by `PRAGMA foreign_key_check`. The transaction
  also sets the open marker and the `schema_migrations` rows (research.md §7).
- **All or nothing.** An error at any point rolls all of it back. An
  interruption leaves a hot rollback journal, which the next keyed read rolls
  back, so the file is never seen half-upgraded.
- **Housekeeping kept.** `changes_waiting` is the same after the upgrade as
  before it. `last_saved_version` isn't written.
- **What a build may touch before upgrading**: an older file's
  `schema_migrations.name`, the named `app_state` columns,
  `collection_settings.backup_location` and the `pending_changes` table
  (research.md §13). Every layout from 0.1.0 on keeps them.

## Backup files (amends "Backup files")

| Kind | Name | Stamp |
|---|---|---|
| Regular | `<name> <YYYY-MM-DD HHMMSS> <id8>.hoplodex` | `backup_made_at`, `backup_of_name` |
| Before an upgrade | `<name> <YYYY-MM-DD HHMMSS> <id8> before <version>.hoplodex` | as above, plus `backup_before_upgrade_to` |

- **What it is.** A backup before an upgrade is a byte copy of the file in its
  older layout, stamped like any backup: the open marker is cleared and
  `pending_changes` emptied.
- **Where it goes.** It is written to the backup location, or to a folder the
  user chose for it (FR-012).
- **Rotation.** The newest one at the backup location isn't removed, and
  isn't counted against the keep count (FR-014).
- **The day's backup.** It never counts as one, and doesn't touch the backup
  record (`changes_waiting`, `last_backup_at`).
- **An earlier version** that knows this naming (0.1.0 on) lists it, keeps it
  in rotation, and names it when it is opened directly (FR-013).

## Restoring a backup in an older layout (amends "Replacing the file")

The restored copy at `.<file>.new` is upgraded in place, as above, after it
has been checked and stamped and before the swap. A failure removes the copy,
and the database is unchanged (FR-019).
