# Contract: The HoploDex Database File

This is the format promise behind FR-002, FR-011 and SC-001: a database file
plus its passphrase opens on every supported operating system, with nothing
else needed. Any change to a rule marked **(format)** makes files unreadable
by earlier builds, or makes earlier files unreadable. Once the application is
released, such a change needs a new data-layout version and a migration.
Until then it follows CLAUDE.md's "schema edited in place" rule.

## The file

- One file, `<name>.hoplodex`. No sidecar files. SQLite's rollback journal
  `<name>.hoplodex-journal` exists only during a write, and is replayed by
  the next open if a crash leaves it behind.
- **(format)** An SQLCipher 4 database. The whole file is encrypted except
  the 16-byte salt at offset 0.
- **(format)** The key is the passphrase, NFC-normalized, applied with
  `PRAGMA key`. Every connection and every attached schema sets, before its
  first read:

  ```sql
  PRAGMA cipher_page_size = 4096;
  PRAGMA kdf_iter = 1000000;
  PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;
  PRAGMA cipher_hmac_algorithm = HMAC_SHA512;
  PRAGMA cipher_plaintext_header_size = 0;
  ```

  These are defined once (`db::cipher::CIPHER_SETTINGS`).
  `tests/portability_test.rs` opens a committed fixture made with them, and
  fails if any of them changes.
- Every connection also sets `PRAGMA foreign_keys = ON`,
  `PRAGMA secure_delete = ON` (001, constitution V) and
  `PRAGMA locking_mode = EXCLUSIVE` (research §2). These are behaviour
  settings, not format.
- **(format)** The data-layout version is the set of names in
  `schema_migrations`. A file naming a migration the running build does not
  know is refused, and not modified (FR-014). `PRAGMA user_version` is not
  used, because `sqlcipher_export` does not copy it (research §2).

## What is inside, and what travels

| Travels in the file | Table | Counts as a change (FR-025) |
|---|---|---|
| Collection: firearms, photos, documents, disposition history, policies, types | 001/002 tables | yes |
| Backup settings, lock settings | `collection_settings` | yes |
| Identity, open marker, backup record, dismissed note, backup stamp | `app_state` | no |
| Pending changes | `pending_changes` | no |
| Data-layout version | `schema_migrations` | no |

Columns and constraints: [data-model.md](../data-model.md).

Nothing about a database is kept on a computer except what FR-013 lists
there: the recent entry and a saved passphrase. Neither is needed to open the
file (SC-002).

## Open marker protocol (FR-032)

1. At open, after the passphrase is accepted and before anything is written:
   `open_machine_id` is empty, or equals this machine's id → go on;
   otherwise refuse with the machine name and time, unless the user takes over.
2. The open sets the marker to this machine's id, its display name and the
   current UTC time.
3. At every normal close, the marker is cleared in the same transaction as
   the final backup-record write. At sleep and OS shutdown it is cleared only
   as part of the pending-changes write, if there is one (research §14, §16).
4. A copy made as a backup has the marker cleared.

## Backup files

- Same format as the database, protected by the passphrase current when the
  backup was made (FR-023).
- Name: `<database name> <YYYY-MM-DD HHMMSS> <first 8 hex digits of database_id>.hoplodex`
- `app_state.backup_made_at` and `backup_of_name` set, marker cleared,
  `pending_changes` empty.
- Default folder: `HoploDex backups`, next to the database. The folder is stored
  as `default` in `collection_settings.backup_location`, so it resolves on
  every computer.
- While being written: `<final name>.partial`.

## Replacing the file (passphrase change, restore)

Temporary names in the database's folder: `.<file>.new` (the verified copy)
and `.<file>.old` (the previous content, until securely deleted). A reader
that finds the database path missing and `.<file>.new` present completes the
replacement. One that finds only `.<file>.old` renames it back. Both cases
are handled at the next open of that path (research §4).
