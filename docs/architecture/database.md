# The database

_Part of the [architecture docs](../architecture.md)._

`db/mod.rs` creates and opens passphrase-keyed databases (`create_database`,
`open_database`), always from a path the caller passes, and `db/upgrade.rs` is
the one runner for both: it applies `db/migrations/*.sql`, tracked in
`schema_migrations`, which is also the data-layout version (a migration this
build doesn't know means `NewerVersion`), in one transaction with the link check
and foreign keys off around it, and never records the saved version.

Callers take the steps from `build_steps(path)`, never `MIGRATIONS` itself: it
adds the `e2e` build's empty-table step `9000_e2e_upgrade_check` (so the stored
0.1.0 fixture asks the question; `HOPLODEX_E2E_UPGRADE_PHASE_SECONDS` holds each
upgrade phase on screen) and, with `upgrade-test-steps`, a test's registered
steps.

`open_database` returns `Opened`: `Ready(conn)`, or `UpgradeDue { conn,
steps_due, marker_seen }` with the file locked and nothing written, not even the
open marker.

`db::upgrade::testing` (forced failures, test-only steps) exists only with
`upgrade-test-steps`, which `test-kdf` turns on and the release
`performance_test` needs alone, and which Tauri's `custom-protocol` and `e2e`
refuse to compile with. `tauri build` turns on Tauri's `custom-protocol` without
the app's own feature, so both guards test tauri-build's `dev` cfg, which is
set only while it is off.

A policy's name is unique ignoring A-Z case and surrounding spaces
(`insurance::ops` refuses with `fieldErrors.name`; `idx_insurance_policies_name`
backs it).

Every connection to a database, a backup or a copy is opened by
`db::connect`, since any of them can be a database from elsewhere:
`trusted_schema` off, `SQLITE_DBCONFIG_DEFENSIVE` on, and `SQLITE_LIMIT_LENGTH`
at `db::MAX_VALUE_BYTES`, 31 MiB, just above the largest attachment (a 30 MiB
document), so no value read or made is longer. A list of photos or documents
reads no attachment's content (`PhotoSummary::COLUMNS`, `DocumentInfo::COLUMNS`),
so a longer one stored before the limits still lists, and only opening it, or
an export holding it, fails (`ATTACHMENT_TOO_LARGE`,
`CommandError::from_attachment_read`). `sqlcipher_export` copies it unchanged,
so a passphrase change still works.

Each connection gets `db/cipher.rs`'s pinned `CIPHER_SETTINGS`, then foreign
keys, `secure_delete` and `locking_mode = EXCLUSIVE`.

`db::reclaim_deleted_record` merges both search indexes after any delete (a
delete cascades through a mount, so either record's words may be in either).

`app_state.last_saved_version` records the build (`APP_VERSION` in `lib.rs`,
kept equal to `tauri.conf.json` and `package.json` by
`tests/app_version_test.rs`) that last saved the file: `record_saved_version()`
runs at creation, after a write that changed rows and in `write_pending`, never
on open, close, backup or passphrase change; pending changes name it so the
dialog can say another version kept them.

Never change a cipher setting: every existing file becomes unreadable
(`tests/portability_test.rs`).

The Rust tests run with `--features test-kdf`, which lowers only `kdf_iter` and
only in a debug, non-shipped build (`cipher::test_kdf`); never enable it
anywhere a real file is made.

Read an open database's bytes only through `db/raw_file.rs` (SQLite's own file
handle), never `std::fs`: closing any other descriptor on the file drops the
process's POSIX lock on it.
