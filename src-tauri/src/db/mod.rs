//! Database files: creating, opening and checking a passphrase against the
//! passphrase-keyed SQLCipher format (003 research.md §1, §1a, §2;
//! 003 contracts/database-file.md). Every function takes
//! its path from the caller: nothing here knows where a user keeps their
//! databases.

pub mod cipher;
pub mod raw_file;
pub mod upgrade;

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, ErrorCode, OpenFlags};

use crate::services::machine_settings::MachineIdentity;
use crate::services::passphrase::Passphrase;

/// Versioned SQL migrations, applied in order. Names are stored in
/// `schema_migrations`, which is also the database's data-layout version
/// (003 contracts/database-file.md). Every caller that creates, opens or checks
/// a database uses [`upgrade::build_steps`], which adds a build's test-only
/// steps, never this list itself; it is public for the tests that check the two
/// agree.
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_initial", include_str!("migrations/0001_initial.sql")),
    ("0002_fts5", include_str!("migrations/0002_fts5.sql")),
    ("0003_seed_firearm_types", include_str!("migrations/0003_seed_firearm_types.sql")),
];

/// The cipher page size, which is also the size of the page-1 probe
/// (003 research.md §1a).
const PAGE_SIZE: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("filesystem error: {0}")]
    Io(#[from] io::Error),
    #[error("could not generate a random value: {0}")]
    Random(#[from] getrandom::Error),
    #[error("a file already exists at {}", .0.display())]
    Exists(PathBuf),
    #[error("{0}")]
    Upgrade(#[from] upgrade::UpgradeError),
}

/// Why a database did not open (003 research.md §2). Each maps to one command
/// error code. None of them is reported after anything was written.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("no file at {}", path.display())]
    NotFound { path: PathBuf },
    #[error("cannot read or write {}", path.display())]
    Unreadable { path: PathBuf },
    #[error("the database is in use by another connection")]
    InUse,
    /// Also a file that is not a HoploDex database: 003 FR-006 does not tell the
    /// two apart.
    #[error("wrong passphrase, or not a HoploDex database")]
    PassphraseIncorrect,
    #[error("the database was last used by a newer version")]
    NewerVersion,
    #[error("the database is open on {machine_name} since {since}")]
    OpenElsewhere { machine_name: String, since: String },
    #[error("the database is damaged")]
    Damaged,
    #[error("database error: {0}")]
    Internal(rusqlite::Error),
}

impl OpenError {
    /// Sorts an SQLite error met while opening into the outcomes above.
    pub(crate) fn classify(err: rusqlite::Error) -> Self {
        match err.sqlite_error_code() {
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => Self::InUse,
            Some(ErrorCode::NotADatabase) => Self::PassphraseIncorrect,
            Some(ErrorCode::DatabaseCorrupt) => Self::Damaged,
            _ => Self::Internal(err),
        }
    }
}

/// `2 * bytes` random lowercase hex digits.
pub fn random_hex(bytes: usize) -> Result<String, getrandom::Error> {
    let mut buffer = vec![0u8; bytes];
    getrandom::fill(&mut buffer)?;
    Ok(buffer.iter().map(|b| format!("{b:02x}")).collect())
}

/// The current time as the database and `machine.json` store it: UTC
/// ISO-8601 to the second.
pub fn now_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Keys a fresh connection and sets everything each connection needs,
/// before its first read: the pinned cipher settings (003 FR-011), foreign keys,
/// secure deletion (constitution V), and exclusive locking, so a second
/// copy of HoploDex is told "in use" rather than "wrong passphrase"
/// (003 research.md §2). A busy file is reported at once rather than waited on.
fn configure(conn: &Connection, passphrase: &Passphrase) -> rusqlite::Result<()> {
    conn.pragma_update(None, "key", passphrase.as_str())?;
    cipher::apply_cipher_settings(conn, "main")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "secure_delete", "ON")?;
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
    conn.busy_timeout(Duration::ZERO)
}

/// Creates a new database at `path`, which must not exist yet, and returns
/// its open connection holding the file lock. The database has the default
/// settings, a new random identity, nothing waiting to be backed up, and
/// the open marker set to `machine` (003 FR-032).
pub fn create_database(
    path: &Path,
    passphrase: &Passphrase,
    machine: &MachineIdentity,
) -> Result<Connection, DbError> {
    // Claiming the path first means an existing file is never opened, let
    // alone written.
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            return Err(DbError::Exists(path.to_owned()));
        }
        Err(err) => return Err(err.into()),
    }
    let created = initialize(path, passphrase, machine);
    if created.is_err() {
        // The file is ours and half made: remove it rather than leave a
        // database that can never open.
        let _ = fs::remove_file(path);
    }
    created
}

fn initialize(
    path: &Path,
    passphrase: &Passphrase,
    machine: &MachineIdentity,
) -> Result<Connection, DbError> {
    let conn = Connection::open(path)?;
    configure(&conn, passphrase)?;
    upgrade::upgrade(&conn, &upgrade::build_steps(path), None, &mut |_, _| {})?;
    // The settings row goes in before `app_state` exists, so its
    // change-tracking trigger finds no backup record to mark: creating a
    // database is not a change to back up (003 FR-025).
    conn.execute("INSERT INTO collection_settings (id) VALUES (1)", [])?;
    conn.execute(
        "INSERT INTO app_state (id, database_id, created_at, open_machine_id, open_machine_name,
                                open_since, changes_waiting)
         VALUES (1, ?1, ?2, ?3, ?4, ?2, 0)",
        rusqlite::params![random_hex(16)?, now_utc(), machine.id, machine.display_name],
    )?;
    // A new database was written by this version (008 FR-015).
    record_saved_version(&conn)?;
    Ok(conn)
}

/// Records this build's [`APP_VERSION`](crate::APP_VERSION) as the version
/// that last saved the database (`app_state.last_saved_version`, 008 FR-015,
/// 008 research.md §16). It changes no row when that
/// is already the recorded value, so a caller can count rows changed. Not a
/// change to back up: `app_state` has no change-tracking triggers.
pub fn record_saved_version(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE app_state SET last_saved_version = ?1 WHERE last_saved_version IS NOT ?1",
        [crate::APP_VERSION],
    )
    .map(|_| ())
}

/// What [`open_database`] found (009 research.md §4).
#[derive(Debug)]
pub enum Opened {
    /// Opened: this computer's open marker is set, and the connection holds
    /// the file's exclusive lock.
    Ready(Connection),
    /// The database is in an older layout. It is keyed and holds the file's
    /// write lock, but nothing has been written to it, not even the open
    /// marker: the caller keeps it until the user answers the upgrade
    /// question, and then runs [`upgrade::upgrade`] on it.
    UpgradeDue {
        conn: Connection,
        /// The names of the steps to apply, in order.
        steps_due: Vec<&'static str>,
        /// The open marker found at the question: another computer's when
        /// `take_over` opened a database it had open.
        marker_seen: SeenMarker,
    },
}

impl Opened {
    /// The connection of an open that had nothing to upgrade.
    ///
    /// # Panics
    /// When steps are due: for a caller that knows there are none (a test's
    /// database in the current layout).
    pub fn expect_ready(self) -> Connection {
        match self {
            Self::Ready(conn) => conn,
            Self::UpgradeDue { steps_due, .. } => {
                panic!("expected a database in the current layout, steps are due: {steps_due:?}")
            }
        }
    }

    /// The connection of an open that must have had nothing to upgrade
    /// (a restored copy that was just upgraded; a re-keyed copy), as an
    /// error when steps are due after all.
    pub fn into_ready(self) -> Result<Connection, OpenError> {
        match self {
            Self::Ready(conn) => Ok(conn),
            Self::UpgradeDue { steps_due, .. } => {
                log::error!("steps are due on a database that should be current: {steps_due:?}");
                Err(OpenError::Internal(rusqlite::Error::InvalidQuery))
            }
        }
    }
}

/// The open marker as `app_state` holds it (003 FR-032).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeenMarker {
    pub machine_id: Option<String>,
    pub machine_name: Option<String>,
    pub since: Option<String>,
}

impl SeenMarker {
    /// Reads the marker of the database on `conn`.
    pub fn read(conn: &Connection) -> rusqlite::Result<Self> {
        conn.query_row(
            "SELECT open_machine_id, open_machine_name, open_since FROM app_state",
            [],
            |row| {
                Ok(Self { machine_id: row.get(0)?, machine_name: row.get(1)?, since: row.get(2)? })
            },
        )
    }

    /// Another computer than `machine` has marked the database open.
    pub fn is_another_computers(&self, machine: &MachineIdentity) -> bool {
        self.machine_id.as_ref().is_some_and(|id| *id != machine.id)
    }
}

/// Opens the database at `path` with `passphrase` on `machine`, in
/// 003 research.md §2's order. Nothing is written until every check has passed,
/// so a refused open never changes the file (003 FR-006, FR-014). A database
/// marked open on another computer is refused unless `take_over` is set
/// (003 FR-032). The returned connection holds the file's exclusive lock until
/// it is dropped.
///
/// A database in an older layout is not changed (009 research.md §4): it is
/// returned as [`Opened::UpgradeDue`], holding the file's write lock, for the
/// caller to ask the user before it upgrades it.
pub fn open_database(
    path: &Path,
    passphrase: &Passphrase,
    machine: &MachineIdentity,
    take_over: bool,
) -> Result<Opened, OpenError> {
    // Before SQLite: a missing or inaccessible file.
    match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) if file.metadata().is_ok_and(|m| m.is_file()) => {}
        Ok(_) => return Err(OpenError::Unreadable { path: path.to_owned() }),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Err(OpenError::NotFound { path: path.to_owned() });
        }
        Err(_) => return Err(OpenError::Unreadable { path: path.to_owned() }),
    }
    // Without CREATE, so a file removed in the meantime is not recreated.
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(OpenError::classify)?;

    // Step 1: key, cipher settings and locking mode.
    configure(&conn, passphrase).map_err(OpenError::classify)?;
    // Step 2: the first read. Another copy's lock gives BUSY before the
    // passphrase is even checked; a wrong passphrase gives NOTADB.
    conn.query_row("SELECT count(*) FROM sqlite_schema", [], |row| row.get::<_, i64>(0))
        .map_err(OpenError::classify)?;
    if !is_hoplodex_database(&conn).map_err(OpenError::classify)? {
        return Err(OpenError::PassphraseIncorrect);
    }
    // Step 3, read-only: a migration this build doesn't know means a newer
    // version of HoploDex has changed the data layout.
    if has_unknown_migration(&conn, path).map_err(OpenError::classify)? {
        return Err(OpenError::NewerVersion);
    }
    // Step 4, read-only: open on another computer. This computer's own
    // marker is one a crash left behind, since the exclusive lock has
    // already shown no other copy here has the file open.
    let marker_seen = SeenMarker::read(&conn).map_err(OpenError::classify)?;
    if marker_seen.is_another_computers(machine) && !take_over {
        return Err(OpenError::OpenElsewhere {
            machine_name: marker_seen.machine_name.unwrap_or_default(),
            since: marker_seen.since.unwrap_or_default(),
        });
    }

    // Step 4b: steps due. Taking the write lock changes nothing, and
    // exclusive locking mode keeps it, so any other connection to the file is
    // refused as "in use" while the user decides (009 research.md §4).
    let steps_due = upgrade::steps_due(&conn, path).map_err(OpenError::classify)?;
    if !steps_due.is_empty() {
        conn.execute_batch("BEGIN EXCLUSIVE; COMMIT").map_err(OpenError::classify)?;
        return Ok(Opened::UpgradeDue { conn, steps_due, marker_seen });
    }

    // Step 5: the housekeeping write. Setting the marker also takes the
    // write lock, which exclusive locking mode then keeps, so any other
    // connection to the file is refused as "in use" from here on.
    set_open_marker(&conn, &upgrade::OpenMarker::now(machine)).map_err(OpenError::classify)?;
    Ok(Opened::Ready(conn))
}

/// Marks the database open on this computer.
fn set_open_marker(conn: &Connection, marker: &upgrade::OpenMarker) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE app_state SET open_machine_id = ?1, open_machine_name = ?2, open_since = ?3",
        rusqlite::params![marker.machine_id, marker.machine_name, marker.since],
    )
    .map(|_| ())
}

/// Opens a copy that is to replace a database (a restore's, a passphrase
/// change's) with the passphrase it should have, and proves it sound
/// before anything is replaced (003 research.md §4): every page's HMAC
/// (`cipher_integrity_check`), SQLite's own `integrity_check`, a HoploDex
/// database, and no migration this build doesn't know. Returns the
/// connection so the caller can stamp the copy. Nothing else knows about
/// the copy, so it is not locked exclusively.
pub fn check_copy(path: &Path, passphrase: &Passphrase) -> Result<Connection, OpenError> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(OpenError::classify)?;
    (|| {
        conn.pragma_update(None, "key", passphrase.as_str())?;
        cipher::apply_cipher_settings(&conn, "main")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "secure_delete", "ON")
    })()
    .map_err(OpenError::classify)?;
    conn.query_row("SELECT count(*) FROM sqlite_schema", [], |row| row.get::<_, i64>(0))
        .map_err(OpenError::classify)?;

    // It returns a row for each page that fails, and none when all pass.
    let cipher_failed = conn
        .prepare("PRAGMA cipher_integrity_check")
        .and_then(|mut stmt| stmt.query([])?.next().map(|row| row.is_some()))
        .map_err(OpenError::classify)?;
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(OpenError::classify)?;
    if cipher_failed || integrity != "ok" {
        log::error!("the copy {} failed its checks: {integrity}", path.display());
        return Err(OpenError::Damaged);
    }
    if !is_hoplodex_database(&conn).map_err(OpenError::classify)? {
        return Err(OpenError::PassphraseIncorrect);
    }
    if has_unknown_migration(&conn, path).map_err(OpenError::classify)? {
        return Err(OpenError::NewerVersion);
    }
    Ok(conn)
}

/// Whether `schema_migrations` names a migration this build doesn't have
/// (for the database at `path`: [`upgrade::build_steps`]).
pub(crate) fn has_unknown_migration(conn: &Connection, path: &Path) -> rusqlite::Result<bool> {
    let steps = upgrade::build_steps(path);
    let mut stmt = conn.prepare("SELECT name FROM schema_migrations")?;
    let names = stmt.query_map([], |row| row.get::<_, String>(0))?;
    for name in names {
        let name = name?;
        if !steps.iter().any(|(known, _)| *known == name) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A HoploDex database has its migrations table and its `app_state` row.
/// Any other SQLCipher file that the passphrase happens to open is not one.
fn is_hoplodex_database(conn: &Connection) -> rusqlite::Result<bool> {
    let tables: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_schema
         WHERE type = 'table' AND name IN ('schema_migrations', 'app_state')",
        [],
        |row| row.get(0),
    )?;
    if tables < 2 {
        return Ok(false);
    }
    conn.query_row("SELECT EXISTS (SELECT 1 FROM app_state WHERE id = 1)", [], |row| row.get(0))
}

/// Checks `candidate` against the database open on `conn`, whose file is
/// locked against any second connection (003 research.md §1a): the file's first
/// page, ciphertext only, is copied into `scratch_dir` and opened with the
/// candidate, and page 1's HMAC check decides. The probe is deleted whatever
/// the result. The page is read through the connection's own file handle
/// ([`raw_file`]), so the database keeps its lock, and is never written.
pub fn verify_passphrase(
    conn: &Connection,
    candidate: &Passphrase,
    scratch_dir: &Path,
) -> Result<bool, DbError> {
    fs::create_dir_all(scratch_dir)?;
    let probe = scratch_dir.join(format!("probe-{}.hoplodex", random_hex(8)?));
    let result = probe_first_page(conn, &probe, candidate);
    let _ = fs::remove_file(&probe);
    result
}

fn probe_first_page(
    conn: &Connection,
    probe: &Path,
    candidate: &Passphrase,
) -> Result<bool, DbError> {
    let file = raw_file::RawFile::of(conn)?;
    let mut first_page = vec![0u8; PAGE_SIZE.min(file.size()? as usize)];
    file.read_exact_at(&mut first_page, 0)?;
    OpenOptions::new().write(true).create_new(true).open(probe)?.write_all(&first_page)?;

    let conn = Connection::open(probe)?;
    conn.pragma_update(None, "key", candidate.as_str())?;
    cipher::apply_cipher_settings(&conn, "main")?;
    match conn.query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0)) {
        Ok(_) => Ok(true),
        Err(err) => match err.sqlite_error_code() {
            Some(ErrorCode::NotADatabase) => Ok(false),
            // Page 1 decrypted and authenticated; SQLite then notices the
            // rest of the file is missing. That is the probe working.
            Some(ErrorCode::DatabaseCorrupt) => Ok(true),
            _ => Err(err.into()),
        },
    }
}

/// Gives the space a delete freed back to the filesystem by rebuilding the
/// file (`VACUUM`), so a deleted photo or document leaves neither its bytes
/// nor an oversized file behind (Constitution V). Call after the `DELETE`
/// has committed: `VACUUM` cannot run inside a transaction.
///
/// The delete itself has already succeeded and `secure_delete` has already
/// zeroed the content, so a failure here is logged and not reported as a
/// failed delete.
pub fn reclaim_freed_space(conn: &Connection) {
    reclaim(conn);
}

/// [`reclaim_freed_space`] after deleting a firearm or an accessory, which
/// also had an entry in its full-text index. FTS5 records a delete as a marker beside the old
/// entry, both holding the deleted words, until its segments are merged; the
/// merge (`optimize`) drops both, so no word of a deleted firearm stays in
/// the file (004 SC-005, research.md §14).
/// Both indexes are merged, since a delete cascades through a mount and
/// either record's words may be in either one (006 SC-006). A deleted
/// document's file name is in a third one,
/// `document_names_fts`, which a document delete and a cascade from either
/// record both touch (007 research.md §19), so it is
/// merged too. Failures are logged, as in [`reclaim_freed_space`].
pub fn reclaim_deleted_record(conn: &Connection) {
    for index in ["firearms_fts", "accessories_fts", "document_names_fts"] {
        if let Err(err) =
            conn.execute(&format!("INSERT INTO {index} ({index}) VALUES ('optimize')"), [])
        {
            log::warn!("could not merge the search index {index} after a delete: {err}");
        }
    }
    reclaim(conn);
}

fn reclaim(conn: &Connection) {
    if let Err(err) = conn.execute_batch("VACUUM") {
        log::warn!("could not vacuum the database after a delete: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSPHRASE: &str = "unit test passphrase";

    fn machine() -> MachineIdentity {
        MachineIdentity { id: "0".repeat(32), display_name: "Unit test".into() }
    }

    fn passphrase(text: &str) -> Passphrase {
        Passphrase::from_input(text.into())
    }

    #[test]
    fn creates_migrates_and_reopens_a_real_temp_database() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.hoplodex");

        let conn = create_database(&db_path, &passphrase(PASSPHRASE), &machine()).unwrap();
        let type_count: i64 =
            conn.query_row("SELECT count(*) FROM firearm_types", [], |r| r.get(0)).unwrap();
        assert_eq!(type_count, 5, "seed migration should insert 5 firearm types");
        drop(conn);

        // Reopening must succeed and must not re-apply (and thus fail to
        // re-insert unique-constrained) migrations.
        let conn2 = open_database(&db_path, &passphrase(PASSPHRASE), &machine(), false)
            .unwrap()
            .expect_ready();
        let type_count2: i64 =
            conn2.query_row("SELECT count(*) FROM firearm_types", [], |r| r.get(0)).unwrap();
        assert_eq!(type_count2, 5);
    }

    #[test]
    fn wrong_passphrase_on_reopen_fails() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.hoplodex");
        drop(create_database(&db_path, &passphrase(PASSPHRASE), &machine()).unwrap());

        let result = open_database(&db_path, &passphrase("another passphrase"), &machine(), false);
        assert!(matches!(result, Err(OpenError::PassphraseIncorrect)), "{result:?}");
    }
}
