# Database migrations

Each `NNNN_name.sql` here is one upgrade **step**. `MIGRATIONS` in `../mod.rs`
lists them in order, and `../upgrade.rs` applies the ones a database lacks, all
in one transaction, with the foreign keys off and a link check at the end. This
file is the author's guide; the reasons are in 009 research.md §7, §11 and §13
(`specs/009-upgrade-readiness/research.md`), and the workflow around it (testing
a step, the stored release databases) is in `DEVELOPMENT.md`, "Changing the
data layout".

Until 0.1.0 is tagged these files are edited in place. **From the tag on, a
released step never changes** (`released.txt` holds its hash and
`tests/released_migrations_test.rs` fails when the bytes differ): a change to the
layout is a new step.

## The rules for a step

- **Pure SQL.** No `BEGIN`, `COMMIT`, `ROLLBACK`, `SAVEPOINT`, `RELEASE`,
  `PRAGMA foreign_keys`, `VACUUM`, `ATTACH` or `DETACH`: the runner owns the
  transaction, and those break it or do nothing inside it.
  `tests/upgrade_steps_test.rs` scans every step for them. No app-defined
  functions or collations.
- **It depends only on the layout it starts from.** It reads no app setting and
  no value only the app can interpret, because a database may skip several
  releases and get all their steps at once.
- **It never drops or renames what the open reads before any step runs**:
  `schema_migrations (name)`; `app_state`'s `id`, `database_id`,
  `open_machine_id`, `open_machine_name`, `open_since`, `changes_waiting`,
  `backup_made_at`, `backup_of_name` and `backup_before_upgrade_to`;
  `collection_settings.backup_location`; and the table `pending_changes`
  (`../upgrade.rs`, "What an older layout must keep").
- **It leaves no row pointing at a missing record.** The runner's link check
  (`PRAGMA foreign_key_check`) abandons the whole upgrade otherwise, and it runs
  with the foreign keys off, so nothing else catches it.
- A step that updates rows fires the change-tracking triggers; the runner puts
  `app_state.changes_waiting` back afterwards, since an upgrade is not a change
  to the collection.

## Changing a table: the rebuild

SQLite's `ALTER TABLE` cannot change a column or a `CHECK`, so the table is
rebuilt, in the order SQLite documents:

1. `CREATE TABLE new_x (…)` with the new definition;
2. `INSERT INTO new_x (…) SELECT … FROM x`;
3. `DROP TABLE x`;
4. `ALTER TABLE new_x RENAME TO x`;
5. create every index, trigger and full-text trigger of `x` again, as the
   earlier steps wrote them (`DROP TABLE` took them with it).

**Never rename `x` away first** (`ALTER TABLE x RENAME TO old_x`): since SQLite
3.26 that rewrites the other tables' foreign keys to follow the old name, and
they would end up pointing at a table that is then dropped.

Worked example: adding `location` to the kinds a draft in `pending_changes` may
have (#16). The `CHECK` on `kind` changes, so the table is rebuilt (the
real table's columns are in `0001_initial.sql`; this is the pattern):

```sql
-- 0004_pending_changes_location.sql
CREATE TABLE new_pending_changes (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    kind TEXT NOT NULL CHECK (kind IN ('firearm', 'policy', 'accessory', 'location')),
    mode TEXT NOT NULL CHECK (mode IN ('add', 'edit', 'dispose', 'restore', 'coverage')),
    target_id INTEGER,
    label TEXT NOT NULL,
    form_version INTEGER NOT NULL,
    values_json TEXT NOT NULL CHECK (length(values_json) <= 1048576),
    saved_at TEXT NOT NULL,
    CHECK (mode <> 'coverage' OR kind IN ('firearm', 'accessory')),
    CHECK (target_id IS NOT NULL OR mode = 'add')
);

INSERT INTO new_pending_changes
    (id, kind, mode, target_id, label, form_version, values_json, saved_at)
SELECT id, kind, mode, target_id, label, form_version, values_json, saved_at
FROM pending_changes;

DROP TABLE pending_changes;
ALTER TABLE new_pending_changes RENAME TO pending_changes;
-- pending_changes has no index or trigger; a table that has them recreates
-- each here, word for word.
```

`pending_changes` is one of the tables the open reads before any step runs, so
this rebuild keeps its name and every column. A table other tables link to
(`photos`, linked from `firearms.thumbnail_photo_id`) is rebuilt the same way;
`db::upgrade::testing::PHOTOS_REBUILD` is the worked case, with its indexes and
triggers.

## Filling a new required unique value

A new `NOT NULL UNIQUE` column, such as the `uid` a later table gets, is filled
in SQL, row by row, in the `INSERT … SELECT` of the rebuild, with the version 4
UUID expression (the `CHECK` on 006's `uid` columns and `record_id::parse`
accept it, and `tests/upgrade_steps_test.rs` checks 10,000 values of it):

```sql
lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4'
  || substr(lower(hex(randomblob(2))), 2) || '-'
  || substr('89ab', 1 + (abs(random()) % 4), 1)
  || substr(lower(hex(randomblob(2))), 2) || '-' || lower(hex(randomblob(6)))
```

`db::upgrade::testing::HISTORY_UID_FILL` is a worked case.

## At a release

`released.txt` lists, one per line, `<step name or fixture path> <sha256 of its
bytes> <release>`. When a release is tagged (#19), add a line for every step it
shipped (`sha256sum` of the step's file, LF line endings) and one for its stored
database `tests/fixtures/releases/<release>.hoplodex`, in `MIGRATIONS`' order,
and never edit a line after that. The release's steps must be the first entries
of `MIGRATIONS`, in order; a later release only appends.
