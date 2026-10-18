//! The one runner for creating a database and for upgrading one: a single
//! transaction for every step due, the foreign-key link check, and the test
//! hooks that exercise it (009 research.md §7, §14). The steps are the files
//! of `db/migrations/` (`MIGRATIONS`); `db/migrations/README.md` shows how to
//! write one.
//!
//! # What an older layout must keep
//!
//! What a build may touch in an older layout before it upgrades it must mean
//! the same thing in every release from 0.1.0 on (009 research.md §13,
//! data-model.md "What an older layout must keep"). The open reads and writes
//! some of an older database before any step runs (the open marker, the
//! changes-waiting flag, the backup location), and the backup before an
//! upgrade is stamped in the older layout, so a step never drops or renames
//! any of this, and `tests/upgrade_steps_test.rs` checks a new database still
//! has it all:
//! - `schema_migrations (name)`: what is due is read from it;
//! - `app_state`'s `id`, `database_id`, `open_machine_id`,
//!   `open_machine_name`, `open_since`, `changes_waiting`, `backup_made_at`,
//!   `backup_of_name` and `backup_before_upgrade_to`: the open marker, the
//!   flag the upgrade keeps, and the backup's stamp;
//! - `collection_settings.backup_location`: where the backup before the
//!   upgrade goes;
//! - the table `pending_changes`, which the stamp empties.
//!
//! If one ever must change, the open has to read the old and new layouts
//! both, and that is a decision for that release.
//!
//! # The rules for a step
//!
//! A step is a file in `db/migrations/` (009 FR-020, FR-022, research.md §7;
//! `tests/upgrade_steps_test.rs` scans for the first rule):
//! - **Pure SQL.** No `BEGIN`, `COMMIT`, `ROLLBACK`, `SAVEPOINT`, `RELEASE`,
//!   `PRAGMA foreign_keys`, `VACUUM`, `ATTACH` or `DETACH`: [`upgrade`] owns
//!   the transaction and turns the foreign keys off around it, and those break
//!   it or do nothing inside it. No app-defined functions or collations.
//! - **A rebuild follows SQLite's documented order**: `CREATE TABLE new_x`;
//!   `INSERT INTO new_x … SELECT … FROM x`; `DROP TABLE x`; `ALTER TABLE new_x
//!   RENAME TO x`; then every index, trigger and full-text trigger of `x`
//!   again. Never rename `x` away first: since SQLite 3.26 that rewrites the
//!   other tables' foreign keys to follow the old name.
//! - **A new required unique value**, such as a later table's `uid`, is filled
//!   with SQL alone, with the version 4 expression of
//!   [`testing::UID_EXPRESSION`] (which 006's `CHECK` and `record_id::parse`
//!   accept).
//! - **It depends only on the layout it starts from**: it reads no app setting
//!   and no value only the app can interpret.
//!
use std::path::Path;

use rusqlite::{Connection, OptionalExtension};

use super::MIGRATIONS;
use crate::services::machine_settings::MachineIdentity;

// `upgrade-test-steps` compiles in forced failures and test-only steps, so it
// must never reach a build that opens a user's files (009 research.md §14).
#[cfg(all(feature = "upgrade-test-steps", any(feature = "custom-protocol", feature = "e2e")))]
compile_error!(
    "the `upgrade-test-steps` feature is for `cargo test` builds only: never with `custom-protocol` \
     or `e2e`"
);

/// Calls the test hooks registered for a `Point` on the database in a
/// file, and returns the failure one forces (with `?`, so the function's error
/// type needs a `From<testing::Injected>`). Expands to nothing without
/// `upgrade-test-steps`, so no call site needs a `cfg` of its own.
///
/// - `reach!(conn, Point::AfterStep(n))`: the runner's points, with the
///   connection that holds the database.
/// - `reach!(Point::AtStamp, path, Some(conn), if cond)`: `services::backups`'
///   points, which may have no connection (a hook needs one), only when
///   `cond` holds (only a backup before an upgrade has them).
#[cfg(feature = "upgrade-test-steps")]
macro_rules! reach {
    (Point::$point:ident, $path:expr, $conn:expr, if $cond:expr) => {
        if $cond {
            $crate::db::upgrade::testing::reach(
                $crate::db::upgrade::testing::Point::$point,
                $path,
                $conn,
            )?;
        }
    };
    ($conn:expr, Point::$point:ident $(($n:expr))?) => {
        $crate::db::upgrade::testing::reach(
            $crate::db::upgrade::testing::Point::$point $(($n))?,
            std::path::Path::new($conn.path().unwrap_or("")),
            Some($conn),
        )?
    };
}

#[cfg(not(feature = "upgrade-test-steps"))]
macro_rules! reach {
    ($($tokens:tt)*) => {};
}
pub(crate) use reach;

/// Why a create or an upgrade did not complete. Every variant means the
/// transaction was rolled back and the file is as it was. None carries a row's
/// data: a step is named, a link failure is counted, and the table and row id
/// of each are logged (009 research.md §7).
#[derive(Debug, thiserror::Error)]
pub enum UpgradeError {
    /// A step's SQL (or its `schema_migrations` row) failed.
    #[error("upgrade step {name} failed: {source}")]
    Step {
        name: String,
        #[source]
        source: rusqlite::Error,
    },
    /// After the last step, `PRAGMA foreign_key_check` found rows pointing at
    /// records that are not there.
    #[error("the upgrade left {violations} record(s) pointing at a record that is not there")]
    LinkCheck { violations: usize },
    /// The transaction itself, the open marker or the changes-waiting flag.
    #[error("upgrade failed: {0}")]
    Database(#[from] rusqlite::Error),
    /// A failure forced by a test (`testing::scope(..).fail_at(..)`).
    #[cfg(feature = "upgrade-test-steps")]
    #[error("{0}")]
    Injected(#[from] testing::Injected),
}

impl UpgradeError {
    /// The SQLite error underneath, when there is one, so a caller can map it
    /// as it maps any other (`CommandError::from_db`: a full disk, a damaged
    /// file). A link failure or a forced one has none.
    pub fn into_sqlite(self) -> Result<rusqlite::Error, Self> {
        match self {
            Self::Step { source, .. } | Self::Database(source) => Ok(source),
            other => Err(other),
        }
    }
}

/// What an upgrade at open writes besides the steps, in its transaction: the
/// open marker, so "this computer holds the database" and the upgrade are one
/// atomic change (009 research.md §4 step 5c), and, for a backup opened
/// directly, taking its stamp (009 research.md §8).
#[derive(Debug, Clone)]
pub struct OpenMarker {
    pub machine_id: String,
    pub machine_name: String,
    /// UTC ISO-8601, as `open_since` stores it.
    pub since: String,
    /// The file is a backup opened directly: it becomes a database of its
    /// own, with this new `database_id`, and its stamp is cleared.
    pub take_backup_stamp_as: Option<String>,
}

impl OpenMarker {
    /// The marker for `machine` opening the database now.
    pub fn now(machine: &MachineIdentity) -> Self {
        Self {
            machine_id: machine.id.clone(),
            machine_name: machine.display_name.clone(),
            since: super::now_utc(),
            take_backup_stamp_as: None,
        }
    }

    /// The same marker, taking the backup stamp with `database_id` as the new
    /// identity (009 research.md §8).
    pub fn taking_backup_stamp_as(mut self, database_id: String) -> Self {
        self.take_backup_stamp_as = Some(database_id);
        self
    }
}

/// The `e2e` build's one test-only step (009 research.md §14): an empty table,
/// so a database an E2E run creates is newer than the stored 0.1.0 fixture,
/// which lacks it, and opening the fixture asks the upgrade question. It is
/// appended after [`MIGRATIONS`] by [`build_steps`] in an `e2e` build only, so
/// no shipped build has it.
#[cfg(feature = "e2e")]
const E2E_STEPS: &[(&str, &str)] =
    &[("9000_e2e_upgrade_check", "CREATE TABLE e2e_upgrade_check (id INTEGER PRIMARY KEY);")];

/// Every step a build applies to the database at `database_path`, in order:
/// [`MIGRATIONS`], plus, with `upgrade-test-steps`, the test-only steps
/// registered for the database's folder, or, in an `e2e` build, [`E2E_STEPS`].
/// Every caller that creates, opens or
/// checks a database uses this list, never `MIGRATIONS` itself.
pub fn build_steps(database_path: &Path) -> Vec<(&'static str, &'static str)> {
    // `mut` is used only by the builds that add steps: the test hooks, and the `e2e` build's.
    #[cfg_attr(not(any(feature = "upgrade-test-steps", feature = "e2e")), allow(unused_mut))]
    let mut steps: Vec<(&'static str, &'static str)> = MIGRATIONS.to_vec();
    #[cfg(feature = "upgrade-test-steps")]
    steps.extend(testing::extra_steps(database_path));
    #[cfg(not(feature = "upgrade-test-steps"))]
    let _ = database_path;
    #[cfg(feature = "e2e")]
    steps.extend_from_slice(E2E_STEPS);
    steps
}

/// The names of the steps of [`build_steps`] the database on `conn` lacks, in
/// order: the build's names after the database's own (009 research.md §3). A
/// database with no `schema_migrations` yet lacks all of them.
pub fn steps_due(conn: &Connection, database_path: &Path) -> rusqlite::Result<Vec<&'static str>> {
    let applied = applied_steps(conn)?;
    Ok(build_steps(database_path)
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| !applied.iter().any(|done| done == name))
        .collect())
}

fn applied_steps(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    if !table_exists(conn, "schema_migrations")? {
        return Ok(Vec::new());
    }
    conn.prepare("SELECT name FROM schema_migrations")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect()
}

fn table_exists(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
        [name],
        |row| row.get(0),
    )
}

/// Applies the steps in `steps` that the database lacks, in one transaction
/// (009 research.md §7), and records a `schema_migrations` row for each:
///
/// 1. `PRAGMA foreign_keys = OFF`, outside any transaction;
/// 2. `BEGIN IMMEDIATE`;
/// 3. remember `app_state.changes_waiting` (a new database has no `app_state`);
/// 4. each step's SQL, then its row;
/// 5. the open marker, when `marker` is given (an open, not a create);
/// 6. `PRAGMA foreign_key_check`: any row abandons the upgrade;
/// 7. put `changes_waiting` back, since an upgrade is not a collection change;
/// 8. `COMMIT`, then `PRAGMA foreign_keys = ON` on every path.
///
/// Any error before the commit rolls everything back. It never records the
/// saved version: an upgrade is not a save (008 FR-015). Pass the list
/// [`build_steps`] returns; a step the database already has is skipped.
///
/// `progress(done, due)` is called after each step is applied (not yet
/// committed), `due` being how many this run applies.
pub fn upgrade(
    conn: &Connection,
    steps: &[(&str, &str)],
    marker: Option<&OpenMarker>,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<(), UpgradeError> {
    conn.execute_batch("PRAGMA foreign_keys = OFF")?;
    let result = run(conn, steps, marker, progress);
    if result.is_err()
        && !conn.is_autocommit()
        && let Err(err) = conn.execute_batch("ROLLBACK")
    {
        log::error!("could not roll back a failed upgrade: {err}");
    }
    // On every path, so a failure never leaves the link checks off.
    let restored = conn.execute_batch("PRAGMA foreign_keys = ON");
    result?;
    restored?;
    Ok(())
}

fn run(
    conn: &Connection,
    steps: &[(&str, &str)],
    marker: Option<&OpenMarker>,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<(), UpgradeError> {
    conn.execute_batch("BEGIN IMMEDIATE")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            name TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL
        )",
    )?;
    let applied = applied_steps(conn)?;
    let changes_waiting = if table_exists(conn, "app_state")? {
        conn.query_row("SELECT changes_waiting FROM app_state WHERE id = 1", [], |row| {
            row.get::<_, i64>(0)
        })
        .optional()?
    } else {
        None
    };

    let due = steps.iter().filter(|(name, _)| !applied.iter().any(|done| done == name)).count();
    let mut _count = 0usize;
    for (name, sql) in steps {
        if applied.iter().any(|done| done == name) {
            continue;
        }
        let step = |source| UpgradeError::Step { name: (*name).to_owned(), source };
        conn.execute_batch(sql).map_err(step)?;
        conn.execute(
            "INSERT INTO schema_migrations (name, applied_at) VALUES (?1, datetime('now'))",
            [name],
        )
        .map_err(step)?;
        _count += 1;
        progress(_count, due);
        reach!(conn, Point::AfterStep(_count));
    }

    if let Some(marker) = marker {
        conn.execute(
            "UPDATE app_state SET open_machine_id = ?1, open_machine_name = ?2, open_since = ?3",
            rusqlite::params![marker.machine_id, marker.machine_name, marker.since],
        )?;
        if let Some(database_id) = &marker.take_backup_stamp_as {
            conn.execute(
                "UPDATE app_state
                 SET database_id = ?1, backup_made_at = NULL, backup_of_name = NULL,
                     backup_before_upgrade_to = NULL",
                [database_id],
            )?;
        }
    }

    reach!(conn, Point::AtLinkCheck);
    check_links(conn)?;

    if let Some(waiting) = changes_waiting {
        conn.execute("UPDATE app_state SET changes_waiting = ?1 WHERE id = 1", [waiting])?;
    }

    reach!(conn, Point::BeforeCommit);
    conn.execute_batch("COMMIT")?;
    Ok(())
}

/// `PRAGMA foreign_key_check`: a row for every record pointing at one that is
/// not there. The table and row id are logged, never the data.
fn check_links(conn: &Connection) -> Result<(), UpgradeError> {
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let broken = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if broken.is_empty() {
        return Ok(());
    }
    for (table, row_id) in &broken {
        match row_id {
            Some(id) => log::error!("the upgrade left a broken link in {table}, row {id}"),
            None => log::error!("the upgrade left a broken link in {table}"),
        }
    }
    Err(UpgradeError::LinkCheck { violations: broken.len() })
}

/// Test hooks (009 research.md §14): test-only steps, forced failures and
/// interruptions, registered per folder with [`scope`]. They apply to any
/// database whose file is in the folder, on whichever thread works on it, and
/// are cleared when the [`Scope`] is dropped. [`reach`] is called by the
/// runner and by `services::backups` at each [`Point`].
#[cfg(feature = "upgrade-test-steps")]
pub mod testing {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, MutexGuard};

    use rusqlite::Connection;

    /// 009 research.md §7's version 4 expression: a random UUID in SQL alone,
    /// which 006's `CHECK` on a `uid` column and `record_id::parse` accept.
    /// A macro, so [`concat!`] can build a step's text from it.
    macro_rules! uid_expression {
        () => {
            "lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' \
             || substr(lower(hex(randomblob(2))), 2) || '-' \
             || substr('89ab', 1 + (abs(random()) % 4), 1) \
             || substr(lower(hex(randomblob(2))), 2) || '-' || lower(hex(randomblob(6)))"
        };
    }

    /// The expression of `uid_expression!`, for a test to check against the
    /// `CHECK` and `record_id::parse`.
    pub const UID_EXPRESSION: &str = uid_expression!();

    /// The two test-only steps every test program shares (009 research.md §14),
    /// so `upgrade_steps_test`, `release_upgrade_test` and
    /// `performance_test` run one text. Register them with
    /// `testing::scope(dir).extra_steps(&[PHOTOS_REBUILD, HISTORY_UID_FILL])`.
    ///
    /// A rebuild of `photos`, which links to firearms and accessories and is
    /// linked from their `thumbnail_photo_id`, in 009 research.md §7's order: it
    /// changes the table's `CHECK` (a sort order is never negative), and
    /// creates the table's indexes and triggers again, as `0001_initial.sql`
    /// writes them. Every row, `uid`, owner and thumbnail link is kept.
    pub const PHOTOS_REBUILD: (&str, &str) = (
        "9101_test_photos_rebuild",
        "CREATE TABLE new_photos (
    id INTEGER PRIMARY KEY,
    uid TEXT NOT NULL UNIQUE CHECK (
        length(uid) = 36
        AND uid GLOB '????????-????-4???-????-????????????'
        AND substr(uid, 20, 1) IN ('8', '9', 'a', 'b')
        AND NOT (replace(uid, '-', '') GLOB '*[^0-9a-f]*')
    ),
    firearm_id INTEGER REFERENCES firearms (id) ON DELETE CASCADE,
    accessory_id INTEGER REFERENCES accessories (id) ON DELETE CASCADE,
    original_bytes BLOB NOT NULL,
    original_filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    thumbnail_bytes BLOB NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    CHECK ((firearm_id IS NULL) <> (accessory_id IS NULL)),
    CHECK (sort_order >= 0)
);
INSERT INTO new_photos (id, uid, firearm_id, accessory_id, original_bytes, original_filename,
                        mime_type, thumbnail_bytes, sort_order, created_at)
    SELECT id, uid, firearm_id, accessory_id, original_bytes, original_filename,
           mime_type, thumbnail_bytes, sort_order, created_at
    FROM photos;
DROP TABLE photos;
ALTER TABLE new_photos RENAME TO photos;
CREATE INDEX idx_photos_firearm ON photos (firearm_id);
CREATE INDEX idx_photos_accessory ON photos (accessory_id) WHERE accessory_id IS NOT NULL;
CREATE TRIGGER photos_uid_fixed BEFORE UPDATE OF uid ON photos
WHEN NEW.uid IS NOT OLD.uid
BEGIN SELECT RAISE(ABORT, 'a record identifier never changes'); END;
CREATE TRIGGER photos_marks_backup_due_after_insert AFTER INSERT ON photos
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;
CREATE TRIGGER photos_marks_backup_due_after_update AFTER UPDATE ON photos
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;
CREATE TRIGGER photos_marks_backup_due_after_delete AFTER DELETE ON photos
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;",
    );

    /// A rebuild of `disposition_history` that adds a required unique
    /// identifier, `uid` (the `CHECK` of every other table's), filled for each
    /// existing row with [`UID_EXPRESSION`]. The table's indexes and triggers
    /// are created again.
    pub const HISTORY_UID_FILL: (&str, &str) = (
        "9102_test_history_uid_fill",
        concat!(
            "CREATE TABLE new_disposition_history (
    id INTEGER PRIMARY KEY,
    uid TEXT NOT NULL UNIQUE CHECK (
        length(uid) = 36
        AND uid GLOB '????????-????-4???-????-????????????'
        AND substr(uid, 20, 1) IN ('8', '9', 'a', 'b')
        AND NOT (replace(uid, '-', '') GLOB '*[^0-9a-f]*')
    ),
    firearm_id INTEGER REFERENCES firearms (id) ON DELETE CASCADE,
    accessory_id INTEGER REFERENCES accessories (id) ON DELETE CASCADE,
    disposition_type TEXT NOT NULL
        CHECK (disposition_type IN ('sold', 'traded', 'gifted', 'destroyed', 'lost_stolen')),
    disposition_recipient TEXT NOT NULL,
    disposition_date TEXT NOT NULL,
    disposition_price INTEGER CHECK (disposition_price IS NULL OR disposition_price >= 0),
    reversed_at TEXT NOT NULL,
    CHECK ((firearm_id IS NULL) <> (accessory_id IS NULL))
);
INSERT INTO new_disposition_history (id, uid, firearm_id, accessory_id, disposition_type,
                                     disposition_recipient, disposition_date, disposition_price,
                                     reversed_at)
    SELECT id, ",
            uid_expression!(),
            ", firearm_id, accessory_id, disposition_type, disposition_recipient,
           disposition_date, disposition_price, reversed_at
    FROM disposition_history;
DROP TABLE disposition_history;
ALTER TABLE new_disposition_history RENAME TO disposition_history;
CREATE INDEX idx_disposition_history_firearm ON disposition_history (firearm_id);
CREATE INDEX idx_disposition_history_accessory ON disposition_history (accessory_id) WHERE accessory_id IS NOT NULL;
CREATE TRIGGER disposition_history_marks_backup_due_after_insert AFTER INSERT ON disposition_history
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;
CREATE TRIGGER disposition_history_marks_backup_due_after_update AFTER UPDATE ON disposition_history
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;
CREATE TRIGGER disposition_history_marks_backup_due_after_delete AFTER DELETE ON disposition_history
BEGIN UPDATE app_state SET changes_waiting = 1 WHERE changes_waiting = 0; END;"
        ),
    );

    /// Where in an upgrade (or in the backup before it) a hook runs or a
    /// failure is forced.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Point {
        /// After an open's key has been derived and what the question needs has
        /// been read, before the database is put in the waiting slot (009
        /// research.md §4).
        BeforeWaiting,
        /// Before the backup before an upgrade begins.
        BeforeBackup,
        /// While the backup is being copied (an I/O error).
        DuringCopy,
        /// At the backup's stamp.
        AtStamp,
        /// Before the finished backup is renamed into place.
        BeforeRename,
        /// After the n-th step this run applied (1-based) and its row.
        AfterStep(usize),
        /// Before the foreign-key link check.
        AtLinkCheck,
        /// Before the commit.
        BeforeCommit,
    }

    /// A failure forced by [`Scope::fail_at`].
    #[derive(Debug, thiserror::Error)]
    #[error("a test forced a failure at {0:?}")]
    pub struct Injected(pub Point);

    type Hook = Arc<dyn Fn(&Connection) + Send + Sync>;

    #[derive(Default)]
    struct Entry {
        extra_steps: Vec<(&'static str, &'static str)>,
        failures: Vec<Point>,
        hooks: Vec<(Point, Hook)>,
    }

    static REGISTRY: Mutex<Option<HashMap<PathBuf, Entry>>> = Mutex::new(None);

    fn registry() -> MutexGuard<'static, Option<HashMap<PathBuf, Entry>>> {
        REGISTRY.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The registry's key for a folder: the same folder under any spelling
    /// (macOS' `/var` and `/private/var`, Windows' `\\?\` prefix).
    fn key(folder: &Path) -> PathBuf {
        folder.canonicalize().unwrap_or_else(|_| folder.to_owned())
    }

    fn key_of_file(database_path: &Path) -> PathBuf {
        key(database_path.parent().unwrap_or_else(|| Path::new("")))
    }

    /// The hooks registered for the folder `dir`, until the returned
    /// [`Scope`] is dropped. One scope per folder at a time.
    pub fn scope(dir: &Path) -> Scope {
        let key = key(dir);
        let mut registry = registry();
        let previous =
            registry.get_or_insert_with(HashMap::new).insert(key.clone(), Entry::default());
        assert!(previous.is_none(), "{} already has a scope", dir.display());
        Scope { key }
    }

    /// The registration of one folder. Dropping it clears every hook.
    #[must_use = "the hooks are cleared when the scope is dropped"]
    pub struct Scope {
        key: PathBuf,
    }

    impl Scope {
        fn with(self, change: impl FnOnce(&mut Entry)) -> Self {
            change(
                registry()
                    .as_mut()
                    .and_then(|r| r.get_mut(&self.key))
                    .expect("scope is registered"),
            );
            self
        }

        /// Appends test-only steps to [`build_steps`](super::build_steps) for
        /// databases in the folder.
        pub fn extra_steps(self, steps: &[(&'static str, &'static str)]) -> Self {
            self.with(|entry| entry.extra_steps.extend_from_slice(steps))
        }

        /// Makes [`reach`] fail at `point`, every time it is reached, until
        /// [`clear_failures`](Self::clear_failures).
        pub fn fail_at(self, point: Point) -> Self {
            self.with(|entry| entry.failures.push(point))
        }

        /// Runs `hook` with the connection each time `point` is reached with
        /// one, before any failure forced there.
        pub fn at_point(
            self,
            point: Point,
            hook: impl Fn(&Connection) + Send + Sync + 'static,
        ) -> Self {
            self.with(|entry| entry.hooks.push((point, Arc::new(hook))))
        }

        /// Stops forcing failures, so a retry can succeed.
        pub fn clear_failures(&self) {
            if let Some(entry) = registry().as_mut().and_then(|r| r.get_mut(&self.key)) {
                entry.failures.clear();
            }
        }
    }

    impl Drop for Scope {
        fn drop(&mut self) {
            if let Some(registry) = registry().as_mut() {
                registry.remove(&self.key);
            }
        }
    }

    /// The steps registered for the folder of `database_path`.
    pub(super) fn extra_steps(database_path: &Path) -> Vec<(&'static str, &'static str)> {
        let key = key_of_file(database_path);
        registry()
            .as_ref()
            .and_then(|r| r.get(&key))
            .map(|entry| entry.extra_steps.clone())
            .unwrap_or_default()
    }

    /// Called by the runner and `services::backups` at each [`Point`] with the
    /// database's path and the connection they hold, if any (a hook needs
    /// one). Runs the hooks registered for the database's folder, outside the
    /// registry's lock so a hook may use other scopes' databases, then fails
    /// if a failure is forced here.
    pub fn reach(
        point: Point,
        database_path: &Path,
        conn: Option<&Connection>,
    ) -> Result<(), Injected> {
        let key = key_of_file(database_path);
        let (hooks, forced) = {
            let registry = registry();
            let Some(entry) = registry.as_ref().and_then(|r| r.get(&key)) else {
                return Ok(());
            };
            let hooks: Vec<Hook> =
                entry.hooks.iter().filter(|(at, _)| *at == point).map(|(_, h)| h.clone()).collect();
            (hooks, entry.failures.contains(&point))
        };
        if let Some(conn) = conn {
            for hook in hooks {
                hook(conn);
            }
        }
        if forced { Err(Injected(point)) } else { Ok(()) }
    }
}
