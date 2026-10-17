//! The one runner for creating a database and for upgrading one: a single
//! transaction for every step due, the foreign-key link check, and the test
//! hooks that exercise it (009 research.md §7, §14).
//!
//! What a build may touch in an older layout before it upgrades it must mean
//! the same thing in every release from 0.1.0 on (009 research.md §13,
//! data-model.md "What an older layout must keep"); a step never drops or
//! renames any of it, and `tests/upgrade_steps_test.rs` checks a new database
//! still has it all:
//! - `schema_migrations (name)`;
//! - `app_state`'s `id`, `database_id`, `open_machine_id`,
//!   `open_machine_name`, `open_since`, `changes_waiting`, `backup_made_at`,
//!   `backup_of_name` and `backup_before_upgrade_to`;
//! - `collection_settings.backup_location`;
//! - the table `pending_changes`.

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

/// Calls the test hooks registered for `Point` on the database in `conn`'s
/// file, and returns the failure one forces. Expands to nothing without
/// `upgrade-test-steps`.
#[cfg(feature = "upgrade-test-steps")]
macro_rules! reach {
    ($conn:expr, Point::$point:ident $(($n:expr))?) => {
        testing::reach(
            testing::Point::$point $(($n))?,
            Path::new($conn.path().unwrap_or("")),
            Some($conn),
        )?
    };
}

#[cfg(not(feature = "upgrade-test-steps"))]
macro_rules! reach {
    ($($tokens:tt)*) => {};
}
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

/// The open marker an upgrade at open writes in its transaction, so "this
/// computer holds the database" and the upgrade are one atomic change
/// (009 research.md §4 step 5c).
#[derive(Debug, Clone)]
pub struct OpenMarker {
    pub machine_id: String,
    pub machine_name: String,
    /// UTC ISO-8601, as `open_since` stores it.
    pub since: String,
}

impl OpenMarker {
    /// The marker for `machine` opening the database now.
    pub fn now(machine: &MachineIdentity) -> Self {
        Self {
            machine_id: machine.id.clone(),
            machine_name: machine.display_name.clone(),
            since: super::now_utc(),
        }
    }
}

/// Every step a build applies to the database at `database_path`, in order:
/// [`MIGRATIONS`], plus, with `upgrade-test-steps`, the test-only steps
/// registered for the database's folder. Every caller that creates, opens or
/// checks a database uses this list, never `MIGRATIONS` itself.
pub fn build_steps(database_path: &Path) -> Vec<(&'static str, &'static str)> {
    // `mut` is used only by the builds that add steps (test hooks; 009 T034's E2E step).
    #[cfg_attr(not(feature = "upgrade-test-steps"), allow(unused_mut))]
    let mut steps: Vec<(&'static str, &'static str)> = MIGRATIONS.to_vec();
    #[cfg(feature = "upgrade-test-steps")]
    steps.extend(testing::extra_steps(database_path));
    #[cfg(not(feature = "upgrade-test-steps"))]
    let _ = database_path;
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
pub fn upgrade(
    conn: &Connection,
    steps: &[(&str, &str)],
    marker: Option<&OpenMarker>,
) -> Result<(), UpgradeError> {
    conn.execute_batch("PRAGMA foreign_keys = OFF")?;
    let result = run(conn, steps, marker);
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
        reach!(conn, Point::AfterStep(_count));
    }

    if let Some(marker) = marker {
        conn.execute(
            "UPDATE app_state SET open_machine_id = ?1, open_machine_name = ?2, open_since = ?3",
            rusqlite::params![marker.machine_id, marker.machine_name, marker.since],
        )?;
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

    /// Where in an upgrade (or in the backup before it) a hook runs or a
    /// failure is forced.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Point {
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
