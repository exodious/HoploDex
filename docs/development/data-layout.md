# Changing the data layout

_Part of the [development docs](../../DEVELOPMENT.md)._

Until 0.1.0 is tagged, a schema change edits the existing migrations in place
(`0001_initial.sql` and the rest) and regenerates the stored 0.1.0 database
(below). From the tag on, **every change is a new step in
`src-tauri/src/db/migrations/`, and a released step never changes**
(009 FR-021, `released.txt`, `tests/released_migrations_test.rs`). A database
made by an older release is upgraded when it is opened, after the user agrees
and a backup is made (009 FR-010 to FR-017); the steps run in one transaction
(`db::upgrade`), so a failure leaves the file as it was.

**Writing a step.** Add `NNNN_name.sql` to `db/migrations/` and its
`("NNNN_name", include_str!(...))` to `MIGRATIONS` in `db/mod.rs`, after the
last one. A step is pure SQL, and the runner owns the transaction, so it has no
`BEGIN`, `COMMIT`, `ROLLBACK`, `SAVEPOINT`, `RELEASE`, `PRAGMA foreign_keys`,
`VACUUM`, `ATTACH` or `DETACH` (`upgrade_steps_test` scans every step), no
app-defined function, and it reads only the layout it starts from. Changing a
column or a `CHECK` is a rebuild of the table, in SQLite's order: `CREATE TABLE
new_x`, `INSERT INTO new_x … SELECT … FROM x`, `DROP TABLE x`, `ALTER TABLE new_x
RENAME TO x`, then every index, trigger and full-text trigger of `x` again.
Never rename `x` away first. A new required unique value (a `uid`) is filled
with the version 4 expression in `db/migrations/README.md`. That file has the
rules and a worked example. What an older layout must keep, because the open
reads it before any step runs (`schema_migrations.name`, `app_state`'s marker
and backup-stamp columns, `collection_settings.backup_location`, the table
`pending_changes`), is listed in `db/upgrade.rs` and checked by
`upgrade_steps_test`.

**Testing a step.** Add what it does to the rows to `declared_change` in
`tests/release_upgrade_test.rs`, which opens a copy of each stored release
database through `lifecycle::open` and `upgrade_database` and checks every row
afterwards (an empty entry says the step changes no row; a step without an entry
fails the test). Add a test of the step to `tests/upgrade_steps_test.rs` if it
has a failure mode of its own. A new table or column also needs the
human-testing seed to use it (see [Human testing](human-testing.md)).

**Test-only steps and the `upgrade-test-steps` feature.** The upgrade's test
hooks are compiled only with the Cargo feature `upgrade-test-steps`, which
`test-kdf` turns on (the release `performance_test` uses it alone). In a test,
`db::upgrade::testing::scope(dir)` registers, for every database whose file is in
`dir` and until the scope is dropped, extra steps (`extra_steps`), failures
forced at a named point (`fail_at`: before the backup, during the copy, at the
stamp, before the rename, after step *n*, at the link check, before the commit)
and hooks that run at one (`at_point`). Two shared test steps exist so that
every test program runs the same text: `testing::PHOTOS_REBUILD` (a rebuild of
a table that others link to, changing a `CHECK` and recreating its indexes and
triggers) and `testing::HISTORY_UID_FILL` (a rebuild that adds a required
unique `uid`, filled in SQL). The feature does not compile with
`custom-protocol` or `e2e`, and the app and the examples refuse to run with it,
so no shipped or E2E build has the hooks. An `e2e` build instead appends one
step of its own, `9000_e2e_upgrade_check`, which makes a database it creates
newer than the stored 0.1.0 one, so the E2E suite can open that and meet the
upgrade question.

**`released.txt`.** `src/db/migrations/released.txt` lists, one line each,
`<step name or fixture path> <sha256 of its bytes> <release>`: every step a
release shipped and every stored release database. The release process (#19)
adds a release's lines at its tag, with `sha256sum` on the step files (LF line
endings in every checkout, `.gitattributes`) and on the fixture, and lines are
never edited afterwards. Until then it holds only a comment and the test passes.
`released_migrations_test` fails when a listed step's bytes change, when it is
missing, renamed or not among the build's first steps in order, when a listed
fixture's bytes change, or when a line is malformed, and its message names
CLAUDE.md's rule.

**The stored release databases.** `src-tauri/tests/fixtures/releases/<release>.hoplodex`
is what that release made: the human-testing seed's main collection, pending
changes and a backup record, keyed with the pinned KDF and the passphrase
`release fixture <release>`. Until 0.1.0 is tagged, regenerate it whenever the
layout changes (`release_fixture_matches_layout` fails when it is out of step
with a new database), without `test-kdf` and `upgrade-test-steps`, which
`examples/release_fixture.rs` refuses:

```bash
scripts/dev-container.sh bash -c 'cd src-tauri && cargo run --example release_fixture'
```

From the tag on, never regenerate it: its hash goes in `released.txt`, and each
later release adds its own file beside it and an entry to `RELEASES` in
`release_upgrade_test.rs`. That test also fails when a table has no row or a
column no value in a stored database, so the fixture cannot thin out.

**Seeing it by hand and in E2E.** `scripts/human-testing.sh --older` puts a
copy of the stored 0.1.0 database in the sandbox as "Older collection". In an
`e2e` build, `HOPLODEX_E2E_UPGRADE_PHASE_SECONDS` (the harness's
`upgradePhaseSeconds` setting, `e2e/support/sandbox.ts`) holds each phase of the
upgrade on screen that many seconds, for the screenshot walk and for watching
it by hand; no other build reads it.
