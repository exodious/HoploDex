//! The upgrade flow end to end: the question, the backup, the transaction and the
//! result (009 research.md §4 to §7, §14). Real SQLCipher files in temp
//! directories, a manual clock, machine settings in a temp config directory, and
//! test-only upgrade steps registered for each test's own folder
//! (`db::upgrade::testing`), so a database made by this build is older than the
//! build once a step is registered.
//!
//! Written before the code (009 T035 to T038), against 009
//! contracts/tauri-commands.md, 009 data-model.md and 009 research.md §4, §6,
//! §8, §14.
//! Every name this file assumes of code that was not there yet is gathered in
//! the "API the upgrade flow is assumed to have" section below, so one place
//! says what the implementation was built to.

use crate::support;

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use hoplodex_lib::APP_VERSION;
use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::accessories::ops as accessories;
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::insurance::ops as insurance;
use hoplodex_lib::commands::mounts::ops as mounts;
use hoplodex_lib::commands::photos::ops as photos;
use hoplodex_lib::db::raw_file::RawFile;
use hoplodex_lib::db::upgrade::testing::{self, Point};
use hoplodex_lib::db::{self, cipher};
use hoplodex_lib::models::database::{
    BackupLocationInput, BackupOutcome, BackupSettingsInput, CloseOutcome, CloseReason,
    DatabaseStatus, Draft, DraftKind, DraftMode, NoteKind,
};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::backups;
use hoplodex_lib::services::disk_space;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::sandbox::Reach;
use hoplodex_lib::session::clock::Clock;
use hoplodex_lib::session::scoped::Target;
use hoplodex_lib::session::upgrade::testing as waiting;
use hoplodex_lib::session::{Session, SessionId, lifecycle};
use rusqlite::{Connection, ErrorCode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::{ManualClock, TEST_PASSPHRASE, TestEvents, other_machine, passphrase, peek};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// API the upgrade flow is assumed to have (009 T045 to T053), the
// single list to check this file against. None of it existed when the tests
// were written; the contract names the behaviour, these are the spellings.
//
// - `databases::upgrade_database(&Session, &MachineSettings, awaiting_id: u64,
//   backup_folder: Option<&str>) -> Result<DatabaseStatus, CommandError>`
//   (`commands::databases::ops`, thin over `session::upgrade`).
// - `databases::cancel_upgrade(&Session, awaiting_id: u64) -> Result<(),
//   CommandError>` (never fails).
// - `NoteKind::Upgraded` (`"upgraded"`) for `databases::dismiss_note`.
// - `lifecycle::open`'s signature is unchanged: a database with steps due
//   comes back as `Err(CommandError)` with code `UPGRADE_NEEDED` and details
//   `{ awaitingId, version, backupFolder }`; `databases::open_database` too.
// - `lifecycle::quit(&Session, &MachineSettings)`: what `quit_application`
//   does before it exits (close the open database as a quit and drop the
//   waiting one), taken out of the command so it can be called.
// - `lifecycle::will_sleep`, `screen_locked`, `will_shut_down` drop a waiting
//   database whatever the lock settings.
// - `session::upgrade::testing::with_waiting_connection(&Session, awaiting_id:
//   u64, impl FnOnce(&Connection) -> T) -> Option<T>` (`upgrade-test-steps`
//   only): runs a closure on the connection held in the awaiting-upgrade slot,
//   `None` when `awaiting_id` is not the one waiting. The slot is otherwise
//   unreachable, and the cases that need the held connection (a marker written
//   by "another computer" during the question, `cache_size = 1`) have no other
//   way in (009 data-model.md `AwaitingUpgrade`, 009 research.md §14).
// - `db::upgrade::testing::reach` is called with `Some(conn)` at the backup's
//   points too (`BeforeBackup`, `DuringCopy`, `AtStamp`, `BeforeRename`), so a
//   hook registered there runs (009 research.md §14 gives them hooks and
//   forced failures alike).
// - `UnfinishedBackup` serializes `beforeUpgrade: true` for the backup before
//   an upgrade (009 data-model.md "machine.json").
// - Events: `upgrade:progress` `{ phase, processed, total, showNow }`.
// - `restore_backup` keeps the arguments it has here; 009 T053 adds an
//   `upgrade` one, which this file's one call (in
//   `a_restore_drops_the_waiting_database`) must then pass as `false`.
// ---------------------------------------------------------------------------

/// 14:30:05 on 25 September 2026 in a UTC+2 time zone.
const START: &str = "2026-09-25T14:30:05+02:00";

// --- Test-only upgrade steps ---------------------------------------------------

type Step = (&'static str, &'static str);

const STEP_A: Step = ("9001_test_a", "CREATE TABLE t_a (id INTEGER PRIMARY KEY, note TEXT);");
const STEP_B: Step =
    ("9002_test_b", "CREATE TABLE t_b (id INTEGER PRIMARY KEY); INSERT INTO t_b (id) VALUES (1);");
/// Enough rows that, with `cache_size = 1`, changed pages spill into the file
/// before the transaction ends.
const STEP_BIG: Step = (
    "9003_test_big",
    "CREATE TABLE t_big (id INTEGER PRIMARY KEY, payload TEXT NOT NULL);
     WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 3000)
     INSERT INTO t_big (id, payload) SELECT i, printf('%0200d', i) FROM n;",
);
/// Valid SQL up to its last statement, which names a table that is not there.
const STEP_INVALID: Step = (
    "9004_test_invalid",
    "CREATE TABLE t_invalid (id INTEGER PRIMARY KEY); INSERT INTO no_such_table VALUES (1);",
);
/// Leaves a child row whose parent is missing; foreign keys are off while a
/// step runs, so only the runner's link check sees it.
const STEP_ORPHAN: Step = (
    "9005_test_orphan",
    "CREATE TABLE t_parent (id INTEGER PRIMARY KEY);
     CREATE TABLE t_child (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES t_parent (id));
     INSERT INTO t_child (id, parent_id) VALUES (1, 99);",
);

/// The tables the steps above make, which a comparison of the collection skips.
const EXTRA_TABLES: &[&str] = &["t_a", "t_b", "t_big", "t_invalid", "t_parent", "t_child"];
/// What a backup and an upgrade treat as housekeeping, not collection.
const HOUSEKEEPING: &[&str] = &["app_state", "schema_migrations", "pending_changes"];

// --- Looking at files ----------------------------------------------------------

/// A connection to `path` as the app makes one, with a zero busy timeout, to
/// tell whether another connection holds the file's lock (009 research.md §4,
/// as `upgrade_steps_test` probes it).
fn app_connection(path: &Path) -> Connection {
    let conn = Connection::open(path).expect("could not open the file");
    conn.pragma_update(None, "key", TEST_PASSPHRASE).unwrap();
    cipher::apply_cipher_settings(&conn, "main").unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.pragma_update(None, "secure_delete", "ON").unwrap();
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE").unwrap();
    conn.busy_timeout(std::time::Duration::ZERO).unwrap();
    conn
}

/// Whether another connection in this process holds `path`: its first keyed
/// read fails with "database is locked".
fn is_held(path: &Path) -> bool {
    let conn = app_connection(path);
    match conn.query_row("SELECT count(*) FROM sqlite_schema", [], |row| row.get::<_, i64>(0)) {
        Ok(_) => false,
        Err(err)
            if matches!(
                err.sqlite_error_code(),
                Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
            ) && err.to_string().contains("database is locked") =>
        {
            true
        }
        Err(other) => panic!("the probe failed for another reason: {other}"),
    }
}

/// SHA-256 of a closed file's bytes. Never call it on a file a connection
/// holds: closing a descriptor drops the process's POSIX lock on it.
fn sha(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

/// SHA-256 of the file behind `conn`, read through SQLite's own handle
/// (`RawFile`), so the lock stays held.
fn raw_sha(conn: &Connection) -> String {
    let file = RawFile::of(conn).expect("the database file is not open");
    let size = file.size().unwrap();
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut offset = 0;
    while offset < size {
        let len = (buf.len() as u64).min(size - offset) as usize;
        file.read_exact_at(&mut buf[..len], offset).unwrap();
        hasher.update(&buf[..len]);
        offset += len as u64;
    }
    format!("{:x}", hasher.finalize())
}

/// Runs `sql` against the closed file `path` with the test passphrase, as
/// another computer or an earlier build would have left it.
fn raw_exec(path: &Path, sql: &str, params: impl rusqlite::Params) {
    let conn = Connection::open(path).expect("could not open the file");
    conn.pragma_update(None, "key", TEST_PASSPHRASE).unwrap();
    cipher::apply_cipher_settings(&conn, "main").unwrap();
    conn.execute(sql, params).expect("the statement failed");
}

fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .map(|entries| {
            entries.map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// The backups made before an upgrade in `folder`, by their ` before <version>` name.
fn before_backups(folder: &Path) -> Vec<String> {
    names_in(folder)
        .into_iter()
        .filter(|name| name.contains(" before ") && name.ends_with(".hoplodex"))
        .collect()
}

fn no_partials(folder: &Path) -> bool {
    names_in(folder).iter().all(|name| !name.contains(".partial"))
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

// --- A comparable picture of a database ---------------------------------------

/// Every table's rows, as text, by table name.
type Dump = BTreeMap<String, Vec<String>>;

/// The rows of every table of `conn` but those in `skip` and the search
/// indexes (`*_fts*`: virtual tables and their shadow tables, derived from the
/// rows of the tables they index, which are compared).
fn dump(conn: &Connection, skip: &[&str]) -> Dump {
    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
               AND name NOT LIKE '%\\_fts%' ESCAPE '\\' ORDER BY name",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut out = Dump::new();
    for table in tables.iter().filter(|table| !skip.contains(&table.as_str())) {
        let mut stmt = conn.prepare(&format!("SELECT * FROM \"{table}\" ORDER BY 1")).unwrap();
        let columns = stmt.column_count();
        let rows = stmt
            .query_map([], |row| {
                let mut cells = Vec::new();
                for i in 0..columns {
                    cells.push(format!("{:?}", row.get::<_, rusqlite::types::Value>(i)?));
                }
                Ok(cells.join(" | "))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        out.insert(table.clone(), rows);
    }
    out
}

/// The collection alone: not housekeeping, not the tables the test steps make.
fn collection(conn: &Connection) -> Dump {
    dump(conn, &[HOUSEKEEPING, EXTRA_TABLES].concat())
}

type SchemaRow = (String, String, Option<String>);

/// The layout and every row, for "the file as it was".
#[derive(Debug, PartialEq)]
struct Snapshot {
    layout: Vec<SchemaRow>,
    rows: Dump,
}

fn snapshot(conn: &Connection) -> Snapshot {
    let layout = conn
        .prepare("SELECT type, name, sql FROM sqlite_schema ORDER BY type, name")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    Snapshot { layout, rows: dump(conn, &[]) }
}

/// The backup record and stamps of `app_state`, none of which an upgrade changes.
fn app_record(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT database_id, created_at, changes_waiting, last_backup_at, last_saved_version,
                    backup_made_at, backup_of_name, backup_before_upgrade_to FROM app_state",
        )
        .unwrap();
    stmt.query_row([], |row| {
        (0..8).map(|i| row.get::<_, rusqlite::types::Value>(i).map(|v| format!("{v:?}"))).collect()
    })
    .unwrap()
}

fn applied_steps(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT name FROM schema_migrations ORDER BY rowid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn database_id_of(path: &Path) -> String {
    peek(path).query_row("SELECT database_id FROM app_state", [], |r| r.get(0)).unwrap()
}

fn marker_of(conn: &Connection) -> Option<String> {
    conn.query_row("SELECT open_machine_id FROM app_state", [], |r| r.get(0)).unwrap()
}

// --- The world a test runs in --------------------------------------------------

/// How an older database is made (see [`World::older_database`]).
#[derive(Clone, Copy, Default)]
struct Older {
    /// Automatic backups stay on. Needs `regular_today`: a lock with changes
    /// waiting and no backup today would make one.
    backups_on: bool,
    /// A regular backup was made today, then more changes.
    regular_today: bool,
    /// Grow the file by about this many bytes, so a copy takes several chunks.
    extra_bytes: usize,
}

struct World {
    dir: TempDir,
    config: TempDir,
    machine: Arc<MachineSettings>,
    session: Session,
    events: Arc<TestEvents>,
    clock: Arc<ManualClock>,
}

impl World {
    fn new() -> Self {
        Self::with_machine(|machine| machine)
    }

    fn with_machine(configure: impl FnOnce(MachineSettings) -> MachineSettings) -> Self {
        let config = TempDir::new().unwrap();
        let clock = ManualClock::at(START);
        let (session, events) =
            support::test_session_at(&config.path().join("opened-documents"), clock.clone());
        Self {
            dir: TempDir::new().unwrap(),
            machine: Arc::new(configure(MachineSettings::load(config.path()).unwrap())),
            config,
            session,
            events,
            clock,
        }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("Mine.hoplodex")
    }

    fn default_folder(&self) -> PathBuf {
        self.dir.path().join("HoploDex backups")
    }

    fn today(&self) -> NaiveDate {
        self.clock.now().date_naive()
    }

    /// The test-only steps, for databases in this world's folder, until the
    /// scope is dropped.
    fn steps(&self, steps: &[Step]) -> testing::Scope {
        testing::scope(self.dir.path()).extra_steps(steps)
    }

    fn create(&self) {
        lifecycle::create(&self.session, &self.machine, &self.path(), &passphrase()).unwrap();
    }

    fn open(&self) {
        lifecycle::open(&self.session, &self.machine, &self.path(), &passphrase(), false).unwrap();
    }

    fn close(&self) -> CloseOutcome {
        lifecycle::close_normal(
            &self.session,
            self.session.current_id(),
            &self.machine,
            CloseReason::Closed,
        )
        .unwrap()
    }

    fn open_id(&self) -> SessionId {
        self.session.current_id()
    }

    /// A change to the collection: a new firearm.
    fn change(&self) {
        let serial = db::random_hex(4).unwrap();
        self.session
            .write(self.open_id(), |conn| {
                firearms::create_firearm(
                    conn,
                    &support::firearm("Colt", "Python", &serial),
                    false,
                    None,
                )
            })
            .unwrap();
    }

    /// A little of everything the upgrade must carry: a policy, two firearms
    /// (one insured by it), an accessory with a photo mounted on one of them.
    /// Returns the insured firearm's id.
    fn fill(&self) -> i64 {
        let accessory = json!({
            "accessoryKindId": 1,
            "make": "Leupold",
            "model": "VX-5HD",
            "serialNumber": "L1",
            "status": "active",
        });
        self.session
            .write(self.open_id(), |conn| {
                let policy = insurance::create_policy(
                    conn,
                    &support::policy("Range rider", "2026-01-01", "2027-01-01", Some(10_000)),
                )?;
                let mut insured = support::firearm("Colt", "Python", "A1");
                insured.insurance_policy_id = Some(policy.id);
                insured.scheduled_coverage_amount = Some(1_000);
                let colt = firearms::create_firearm(conn, &insured, false, None)?;
                firearms::create_firearm(
                    conn,
                    &support::firearm("Glock", "19", "B2"),
                    false,
                    None,
                )?;
                let scope = accessories::create_accessory(
                    conn,
                    &serde_json::from_value(accessory.clone()).unwrap(),
                    None,
                )?;
                photos::add_photo(
                    conn,
                    RecordRef::Accessory(scope.id),
                    &support::sample_png_bytes(),
                    "scope.png",
                    "image/png",
                )?;
                mounts::mount_record(
                    conn,
                    &serde_json::from_value(json!({
                        "item": RecordRef::Accessory(scope.id),
                        "host": RecordRef::Firearm(colt.id),
                    }))
                    .unwrap(),
                )?;
                Ok(colt.id)
            })
            .unwrap()
    }

    fn grow(&self, bytes: usize) {
        let filler = db::random_hex(bytes / 2).unwrap();
        self.session
            .write(self.open_id(), |conn| {
                conn.execute_batch("CREATE TABLE IF NOT EXISTS filler (content TEXT)").unwrap();
                conn.execute("INSERT INTO filler (content) VALUES (?1)", [filler]).unwrap();
                Ok(())
            })
            .unwrap();
    }

    fn set_backups(&self, enabled: bool) {
        databases::update_backup_settings(
            &self.session,
            self.open_id(),
            &self.machine,
            &BackupSettingsInput {
                enabled,
                keep_count: 5,
                location: BackupLocationInput::Default,
                existing_backups: None,
            },
            &Reach::Unconfined,
        )
        .unwrap();
    }

    /// Makes "Mine", fills it, and leaves it closed and, once a step is
    /// registered for the folder, older than the build: changes waiting, a
    /// pending change kept by a lock, a saved version of its own, and the
    /// clock an hour on so a later backup is named differently. The step is
    /// registered afterwards by the caller, since a database made under a scope
    /// would already have it.
    fn older_database(&self, older: Older) {
        assert!(!older.backups_on || older.regular_today, "a lock would make today's backup");
        self.create();
        let insured = self.fill();
        if older.extra_bytes > 0 {
            self.grow(older.extra_bytes);
        }
        if older.regular_today {
            assert_eq!(self.close().backup, BackupOutcome::Made);
            self.open();
            self.change();
        }
        if !older.backups_on {
            self.set_backups(false);
        }
        databases::stage_pending_changes(
            &self.session,
            self.open_id(),
            Some(Draft {
                form_version: 1,
                kind: DraftKind::Firearm,
                mode: DraftMode::Edit,
                target_id: Some(insured),
                label: "Colt Python (edit)".into(),
                values: json!({ "notes": "half typed, with “quotes” é" }),
            }),
        )
        .unwrap();
        lifecycle::lock_with_staged(&self.session, &self.machine, CloseReason::LockedByUser)
            .unwrap();
        self.clock.advance(chrono::Duration::hours(1));
        raw_exec(
            &self.path(),
            "UPDATE app_state SET last_saved_version = '0.0.1'",
            rusqlite::params![],
        );
        assert!(!self.session.is_open());
        // The lock's own events are not the question's.
        self.events.take();
    }

    /// The closed database file, looked at read-only.
    fn closed(&self) -> Connection {
        peek(&self.path())
    }

    /// `open` of an older database, which must ask the question. Returns the
    /// `awaitingId` after checking the refusal's shape.
    fn waiting(&self) -> u64 {
        self.waiting_at(&self.path(), false)
    }

    fn waiting_at(&self, path: &Path, take_over: bool) -> u64 {
        let refused = lifecycle::open(&self.session, &self.machine, path, &passphrase(), take_over)
            .expect_err("an older database must ask first");
        awaiting_id_of(&refused)
    }

    fn upgrade(&self, id: u64, folder: Option<&str>) -> Result<DatabaseStatus, CommandError> {
        databases::upgrade_database(&self.session, &self.machine, id, folder)
    }

    fn cancel(&self, id: u64) {
        databases::cancel_upgrade(&self.session, id).unwrap();
    }

    /// The connection `awaiting_id` holds, for a hook on it.
    fn with_held<T>(&self, id: u64, f: impl FnOnce(&Connection) -> T) -> T {
        waiting::with_waiting_connection(&self.session, id, f)
            .expect("the database is not waiting for an answer")
    }

    /// The open database's rows and stamps, once an upgrade has opened it.
    fn open_state(&self) -> (Dump, Vec<String>, Vec<String>) {
        self.session
            .inspect(self.open_id(), |open| {
                Ok((collection(&open.conn), app_record(&open.conn), applied_steps(&open.conn)))
            })
            .unwrap()
    }

    fn progress(&self) -> Vec<Value> {
        self.events.payloads("upgrade:progress")
    }

    /// Calls `act` once, at the first `upgrade:progress` event `when` picks,
    /// on the thread that is making the backup or running the upgrade.
    fn at_progress(
        &self,
        when: impl Fn(&Value) -> bool + Send + Sync + 'static,
        act: impl Fn() + Send + Sync + 'static,
    ) {
        let fired = AtomicBool::new(false);
        self.events.on_event(move |event, payload| {
            if event == "upgrade:progress" && when(payload) && !fired.swap(true, Ordering::SeqCst) {
                act();
            }
        });
    }

    /// What a database that was dropped without an answer looks like: its
    /// lock released, its id refused, no backup made, nothing open.
    fn assert_dropped(&self, id: u64) {
        assert!(!is_held(&self.path()), "the dropped database still holds its lock");
        assert_eq!(self.upgrade(id, None).unwrap_err().code, "DATABASE_CLOSED");
        self.cancel(id);
        assert!(
            before_backups(&self.default_folder()).is_empty(),
            "a dropped question made a backup"
        );
    }

    /// The chooser has nothing to say about a question that was answered by
    /// Cancel, or dropped.
    fn assert_no_notice(&self) {
        assert_eq!(self.events.payloads("notice"), Vec::<Value>::new());
    }
}

fn awaiting_id_of(err: &CommandError) -> u64 {
    assert_eq!(err.code, "UPGRADE_NEEDED", "{err:?}");
    err.details
        .as_deref()
        .and_then(|details| details["awaitingId"].as_u64())
        .expect("UPGRADE_NEEDED carries an awaitingId")
}

fn details(err: &CommandError) -> Value {
    err.details.as_deref().cloned().unwrap_or(Value::Null)
}

fn as_json(status: &DatabaseStatus) -> Value {
    serde_json::to_value(status).unwrap()
}

/// The path the backup before upgrading to this build is expected at.
fn expected_before_name(world: &World, database_id: &str) -> String {
    format!(
        "Mine {} {} before {APP_VERSION}.hoplodex",
        world.clock.now().format("%Y-%m-%d %H%M%S"),
        &database_id[..8],
    )
}

// ===========================================================================
// 009 T035: the question and the answer
// ===========================================================================

/// 009 US3-1, 009 FR-009, 009 FR-010: an older database stops at the question,
/// keyed and locked and unwritten, and nothing can reach it.
#[test]
fn an_older_database_waits_keyed_locked_and_unwritten_for_the_answer() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let before = sha(&world.path());
    let stat = fs::metadata(world.path()).unwrap();

    let refused =
        lifecycle::open(&world.session, &world.machine, &world.path(), &passphrase(), false)
            .expect_err("an older database must ask first");

    let id = awaiting_id_of(&refused);
    assert_eq!(
        details(&refused),
        json!({
            "awaitingId": id,
            "version": APP_VERSION,
            "backupFolder": text(&world.default_folder()),
        })
    );
    // Held, but nothing is open: no collection command, scoped or not, and
    // not the status, sees it, whatever id they send, the awaiting id included.
    assert!(is_held(&world.path()), "009 research.md §4: the file's lock is kept");
    assert!(!world.session.is_open());
    assert_eq!(world.session.current_id(), SessionId::NONE);
    assert_eq!(
        databases::database_status(&world.session, &world.machine).unwrap_err().code,
        "DATABASE_CLOSED"
    );
    for tried in [SessionId::NONE, SessionId::from_request(id).unwrap()] {
        assert_eq!(world.session.read(tried, |_| Ok(())).unwrap_err().code, "DATABASE_CLOSED");
        assert_eq!(world.session.write(tried, |_| Ok(())).unwrap_err().code, "DATABASE_CLOSED");
        assert_eq!(
            world.session.write_housekeeping(tried, |_| Ok(())).unwrap_err().code,
            "DATABASE_CLOSED"
        );
        assert_eq!(world.session.inspect(tried, |_| Ok(())).unwrap_err().code, "DATABASE_CLOSED");
    }
    // Nothing was written: the file's size and time stand, and (below) so do
    // its bytes. No backup either.
    let now = fs::metadata(world.path()).unwrap();
    assert_eq!((now.len(), now.modified().unwrap()), (stat.len(), stat.modified().unwrap()));
    assert!(!world.default_folder().exists());
    world.cancel(id);
    assert_eq!(sha(&world.path()), before, "009 FR-010: nothing was written");
}

/// 009 US3-2, 009 FR-010: Cancel closes it untouched.
#[test]
fn cancel_drops_the_waiting_database_with_nothing_written_and_no_backup() {
    let world = World::new();
    world.older_database(Older { backups_on: true, regular_today: true, ..Older::default() });
    let _scope = world.steps(&[STEP_A]);
    let before = sha(&world.path());
    let snapshot_before = snapshot(&world.closed());
    let regular_backups = names_in(&world.default_folder());
    let id = world.waiting();
    assert!(is_held(&world.path()));

    world.cancel(id);

    assert!(!is_held(&world.path()), "the lock is released");
    assert_eq!(sha(&world.path()), before);
    assert_eq!(snapshot(&world.closed()), snapshot_before);
    assert_eq!(names_in(&world.default_folder()), regular_backups, "no backup was made");
    world.assert_dropped(id);
    world.assert_no_notice();
    // Cancel is not a refusal of the database: it asks again at the next open.
    let again = world.waiting();
    assert_ne!(again, id, "an awaiting id is never reused");
    world.cancel(again);
}

/// 009 US3-1, 009 FR-011, 009 FR-015, 009 FR-017, 009 research.md §4 step 5
/// and 009 research.md §6: the backup is finished before the transaction begins, the
/// transaction changes the file only at its commit, and the database opens
/// upgraded with everything in it as it was.
#[test]
fn upgrade_makes_the_backup_first_then_upgrades_in_one_transaction_and_opens() {
    let world = World::new();
    world.older_database(Older { backups_on: true, regular_today: true, ..Older::default() });
    let before_dump = collection(&world.closed());
    let before_pending = dump(&world.closed(), &[]).remove("pending_changes").unwrap();
    assert_eq!(before_pending.len(), 1, "the fixture has a pending change");
    let before_record = app_record(&world.closed());
    let old_hash = sha(&world.path());
    let size = fs::metadata(world.path()).unwrap().len();
    let database_id = database_id_of(&world.path());
    let before_steps = applied_steps(&world.closed());

    // At the first step and before the commit: the backup is there, finished,
    // and the database file has not changed by a byte yet.
    /// Where, backup finished, file unchanged, transaction open.
    type Seen = (&'static str, bool, bool, bool);
    let seen: Arc<Mutex<Vec<Seen>>> = Arc::default();
    let observe = |label: &'static str| {
        let (seen, folder, old_hash) = (seen.clone(), world.default_folder(), old_hash.clone());
        move |conn: &Connection| {
            let backed_up = before_backups(&folder).len() == 1 && no_partials(&folder);
            seen.lock().unwrap().push((
                label,
                backed_up,
                raw_sha(conn) == old_hash,
                !conn.is_autocommit(),
            ));
        }
    };
    let _scope = world
        .steps(&[STEP_A])
        .at_point(Point::AfterStep(1), observe("after step 1"))
        .at_point(Point::BeforeCommit, observe("before commit"));
    let id = world.waiting();

    let status = world.upgrade(id, None).expect("the upgrade");

    assert_eq!(
        *seen.lock().unwrap(),
        [("after step 1", true, true, true), ("before commit", true, true, true)],
        "009 research.md §4 step 5b: the backup is finished before the transaction begins, and \
         the file is unchanged until the commit"
    );
    // The backup: named for the database, the minute and this build; an
    // ordinary backup of the older layout, stamped, with no marker or pending
    // changes, and the collection identical.
    let backups = before_backups(&world.default_folder());
    assert_eq!(backups, [expected_before_name(&world, &database_id)]);
    let backup = peek(&world.default_folder().join(&backups[0]));
    let stamp: (Option<String>, Option<String>, Option<String>, Option<String>) = backup
        .query_row(
            "SELECT backup_made_at, backup_of_name, backup_before_upgrade_to, open_machine_id
             FROM app_state",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert!(stamp.0.is_some(), "stamped with the time it was made");
    assert_eq!(stamp.1.as_deref(), Some("Mine"));
    assert_eq!(stamp.2.as_deref(), Some(APP_VERSION), "009 FR-013");
    assert_eq!(stamp.3, None, "no open marker");
    let pending_in_backup: i64 =
        backup.query_row("SELECT count(*) FROM pending_changes", [], |r| r.get(0)).unwrap();
    assert_eq!(pending_in_backup, 0, "no pending changes");
    assert_eq!(applied_steps(&backup), before_steps, "the backup is of the older layout");
    assert_eq!(collection(&backup), before_dump, "and holds the collection as it was");
    // The upgraded database: the step's row, the collection, every uid, link,
    // setting and pending change, the backup record and the saved version as
    // they were.
    let status_json = as_json(&status);
    assert_eq!(
        status_json["notes"]["upgraded"],
        json!({
            "version": APP_VERSION,
            "backupPath": text(&world.default_folder().join(&backups[0])),
            "atBackupLocation": true,
        })
    );
    assert!(
        status_json["pendingChanges"].is_object(),
        "the pending change is offered: {status_json}"
    );
    let (after_dump, after_record, applied) = world.open_state();
    assert_eq!(after_dump, before_dump);
    assert_eq!(after_record, before_record, "changes_waiting, last_backup_at, last_saved_version…");
    assert_eq!(applied.last().map(String::as_str), Some("9001_test_a"));
    assert_eq!(applied.len(), before_steps.len() + 1);
    let pending_after = world
        .session
        .inspect(world.open_id(), |open| Ok(dump(&open.conn, &[]).remove("pending_changes")))
        .unwrap();
    assert_eq!(pending_after, Some(before_pending));
    // This computer holds it now, and the old waiting id is spent.
    let marker = world.session.inspect(world.open_id(), |open| Ok(marker_of(&open.conn))).unwrap();
    assert_eq!(marker, Some(world.machine.identity().id));
    assert_eq!(world.upgrade(id, None).unwrap_err().code, "DATABASE_CLOSED");
    // The progress the chooser draws: the backup from nothing to all of it,
    // then the upgrade (not stoppable) from nothing, every event of one phase
    // before the next.
    let progress = world.progress();
    assert_eq!(
        progress[0],
        json!({ "phase": "backingUp", "processed": 0, "total": size, "showNow": false })
    );
    let phases: Vec<&str> = progress.iter().map(|p| p["phase"].as_str().unwrap()).collect();
    let first_upgrading =
        phases.iter().position(|p| *p == "upgrading").expect("an upgrading event");
    assert!(phases[..first_upgrading].iter().all(|p| *p == "backingUp"));
    assert!(phases[first_upgrading..].iter().all(|p| *p == "upgrading"));
    let last_backing_up = &progress[first_upgrading - 1];
    assert_eq!(last_backing_up["processed"], json!(size));
    assert_eq!(
        (
            progress[first_upgrading]["processed"].clone(),
            progress[first_upgrading]["total"].clone()
        ),
        (json!(0), json!(0))
    );
    world.assert_no_notice();
}

/// 009 FR-017: the note is session-only and goes with `dismiss_note`.
#[test]
fn the_upgraded_note_is_shown_once_and_dismissed_for_the_session() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    let status = world.upgrade(id, None).unwrap();
    assert!(as_json(&status)["notes"]["upgraded"].is_object());
    assert!(
        as_json(&databases::database_status(&world.session, &world.machine).unwrap())["notes"]
            ["upgraded"]
            .is_object(),
        "still there until dismissed"
    );

    databases::dismiss_note(&world.session, world.open_id(), NoteKind::Upgraded).unwrap();

    let after = as_json(&databases::database_status(&world.session, &world.machine).unwrap());
    assert_eq!(after["notes"]["upgraded"], Value::Null);
}

/// 009 US3-5, 009 FR-011, 009 research.md §6: with automatic backups off and a
/// regular backup already made today, the backup before the upgrade is still
/// made, and it touches neither the changes waiting nor the last backup.
#[test]
fn with_automatic_backups_off_the_backup_before_the_upgrade_is_still_made_and_the_record_untouched()
{
    let world = World::new();
    world.older_database(Older { backups_on: false, regular_today: true, ..Older::default() });
    let database_id = database_id_of(&world.path());
    let record_before = app_record(&world.closed());
    let (waiting_before, last_before): (bool, Option<String>) = world
        .closed()
        .query_row("SELECT changes_waiting, last_backup_at FROM app_state", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert!(waiting_before && last_before.is_some(), "the fixture has changes waiting");
    let regular = names_in(&world.default_folder());
    assert_eq!(regular.len(), 1);
    assert!(backups::has_backup_made_on(&world.default_folder(), &database_id, world.today()));
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();

    world.upgrade(id, None).unwrap();

    let made = before_backups(&world.default_folder());
    assert_eq!(made, [expected_before_name(&world, &database_id)], "made though backups are off");
    assert_eq!(world.open_state().1, record_before, "changes_waiting and last_backup_at stand");
    assert!(backups::has_backup_made_on(&world.default_folder(), &database_id, world.today()));
    assert_eq!(names_in(&world.default_folder()).len(), 2, "the regular backup and this one");
    assert!(names_in(&world.default_folder()).contains(&regular[0]));
}

/// 009 US3-5, 009 research.md §6: the backup before an upgrade is never the
/// day's backup, so with no regular backup today the once-a-day rule still
/// calls for one.
#[test]
fn the_backup_before_an_upgrade_does_not_count_as_the_days_backup() {
    let world = World::new();
    world.older_database(Older::default());
    let database_id = database_id_of(&world.path());
    assert!(!backups::has_backup_made_on(&world.default_folder(), &database_id, world.today()));
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();

    world.upgrade(id, None).unwrap();

    assert_eq!(before_backups(&world.default_folder()).len(), 1);
    assert!(
        !backups::has_backup_made_on(&world.default_folder(), &database_id, world.today()),
        "009 research.md §6"
    );
}

/// 009 FR-009: with no step due, an open asks nothing and makes no backup.
#[test]
fn a_database_with_no_step_due_opens_with_no_question_and_no_backup() {
    let world = World::new();
    world.older_database(Older::default());

    world.open();

    assert!(world.session.is_open());
    assert!(!world.default_folder().exists());
    assert!(world.progress().is_empty());
}

// The passphrase waits in the slot only to be saved once the upgrade succeeds
// (009 research.md §4 "The passphrase"; 003 FR-017, FR-018).
#[cfg(feature = "mock-keyring")]
mod remembering {
    use super::*;
    use hoplodex_lib::commands::databases::ops::Unlock;
    use hoplodex_lib::services::keyring::Keyring;
    use hoplodex_lib::services::passphrase::Passphrase;

    /// What the keyring holds for `database_id`, read straight from the store.
    fn stored(database_id: &str) -> Option<String> {
        let entry = keyring_core::Entry::new(
            "io.github.exodious.HoploDex",
            &format!("passphrase:{database_id}"),
        )
        .unwrap();
        match entry.get_password() {
            Ok(saved) => Some(saved),
            Err(keyring_core::Error::NoEntry) => None,
            Err(err) => panic!("the keyring failed: {err}"),
        }
    }

    fn world_with_keyring() -> World {
        World::with_machine(|machine| machine.with_keyring(Keyring::mock(None)))
    }

    /// `open_database` with the "remember" choice of the chooser's question.
    fn open_remembering(world: &World) -> CommandError {
        databases::open_database(
            &world.session,
            &world.machine,
            &text(&world.path()),
            Unlock::Typed { passphrase: &passphrase(), remember: true },
            false,
            &Reach::Unconfined,
        )
        .expect_err("an older database must ask first")
    }

    fn remembered(world: &World) -> bool {
        world.machine.recent_entry(&world.path()).is_some_and(|entry| entry.passphrase_saved)
    }

    fn older_remembering() -> (World, String) {
        let world = world_with_keyring();
        world.older_database(Older::default());
        let id = database_id_of(&world.path());
        assert_eq!(stored(&id), None);
        assert!(!remembered(&world));
        (world, id)
    }

    /// 009 FR-009, 009 contracts/tauri-commands.md `open_database`: the passphrase
    /// is saved when the upgrade succeeds, not at the question.
    #[test]
    fn a_remembered_passphrase_is_saved_only_when_the_upgrade_succeeds() {
        let (world, database_id) = older_remembering();
        let _scope = world.steps(&[STEP_A]);

        let id = awaiting_id_of(&open_remembering(&world));

        assert_eq!(stored(&database_id), None, "not at the question");
        assert!(!remembered(&world));
        let status = world.upgrade(id, None).unwrap();
        assert_eq!(stored(&database_id).as_deref(), Some(TEST_PASSPHRASE));
        assert!(status.passphrase_saved);
        assert!(remembered(&world));
    }

    #[test]
    fn a_remembered_passphrase_is_never_saved_after_a_cancel() {
        let (world, database_id) = older_remembering();
        let _scope = world.steps(&[STEP_A]);
        let id = awaiting_id_of(&open_remembering(&world));

        world.cancel(id);

        assert_eq!(stored(&database_id), None);
        assert!(!remembered(&world));
        // And the dropped slot keeps no passphrase to save late.
        assert_eq!(world.upgrade(id, None).unwrap_err().code, "DATABASE_CLOSED");
        assert_eq!(stored(&database_id), None);
    }

    #[test]
    fn a_remembered_passphrase_is_never_saved_after_a_failed_upgrade() {
        let (world, database_id) = older_remembering();
        let _scope = world.steps(&[STEP_A]).fail_at(Point::BeforeCommit);
        let id = awaiting_id_of(&open_remembering(&world));

        let failed = world.upgrade(id, None).unwrap_err();

        assert_eq!(failed.code, "UPGRADE_FAILED");
        assert_eq!(stored(&database_id), None);
        assert!(!remembered(&world));
    }

    /// 003 FR-018: a saved passphrase that failed is refreshed by the next typed
    /// one that opens the database, and for an older database that means when
    /// the upgrade succeeds.
    #[test]
    fn a_saved_passphrase_is_refreshed_when_the_upgrade_succeeds() {
        let (world, database_id) = older_remembering();
        // Saved before, as an earlier session did.
        world
            .machine
            .keyring()
            .save(&database_id, &Passphrase::from_input("stale".into()))
            .unwrap();
        world.machine.set_passphrase_saved(&world.path(), true);
        let _scope = world.steps(&[STEP_A]);
        let refused = databases::open_database(
            &world.session,
            &world.machine,
            &text(&world.path()),
            Unlock::typed(&passphrase()),
            false,
            &Reach::Unconfined,
        )
        .unwrap_err();
        let id = awaiting_id_of(&refused);
        assert_eq!(stored(&database_id).as_deref(), Some("stale"), "not at the question");

        world.upgrade(id, None).unwrap();

        assert_eq!(stored(&database_id).as_deref(), Some(TEST_PASSPHRASE));
    }
}

// ===========================================================================
// 009 T036: the location problems (009 FR-012)
// ===========================================================================

/// The refusal for a backup folder that can't be used, and that the database
/// still waits for the same `awaitingId`.
fn assert_unusable(err: &CommandError, folder: &Path, reason: &str) {
    assert_eq!(err.code, "BACKUP_LOCATION_UNAVAILABLE", "{err:?}");
    // 003's field names: `path` and `reason` (009 contracts/tauri-commands.md
    // amended 2026-10-17; 009 T036 calls the path `folder`).
    assert_eq!(details(err), json!({ "path": text(folder), "reason": reason }));
}

/// 009 FR-012: a file where the backup folder would be.
#[test]
fn a_file_where_the_backup_folder_would_be_is_refused_and_the_database_keeps_waiting() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    fs::write(world.default_folder(), b"not a folder").unwrap();

    let refused = world.upgrade(id, None).unwrap_err();

    assert_unusable(&refused, &world.default_folder(), "missing");
    assert_eq!(fs::read(world.default_folder()).unwrap(), b"not a folder", "left alone");
    assert!(is_held(&world.path()), "still waiting");
    // The same id upgrades once the problem is gone.
    fs::remove_file(world.default_folder()).unwrap();
    let status = world.upgrade(id, None).expect("the same awaiting id");
    assert_eq!(as_json(&status)["notes"]["upgraded"]["atBackupLocation"], json!(true));
    assert_eq!(before_backups(&world.default_folder()).len(), 1);
}

/// 009 FR-012: and the user may answer the refusal with a folder instead.
#[test]
fn after_a_refusal_a_chosen_folder_takes_the_backup_for_the_same_awaiting_id() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    fs::write(world.default_folder(), b"not a folder").unwrap();
    assert_eq!(world.upgrade(id, None).unwrap_err().code, "BACKUP_LOCATION_UNAVAILABLE");
    let chosen = TempDir::new().unwrap();

    let status = world.upgrade(id, Some(&text(chosen.path()))).expect("the chosen folder");

    let made = before_backups(chosen.path());
    assert_eq!(made.len(), 1);
    assert_eq!(
        as_json(&status)["notes"]["upgraded"],
        json!({
            "version": APP_VERSION,
            "backupPath": text(&chosen.path().join(&made[0])),
            "atBackupLocation": false,
        })
    );
}

/// 009 FR-012 (`notWritable`).
#[cfg(unix)]
#[test]
fn a_read_only_backup_folder_is_refused_and_the_database_keeps_waiting() {
    use std::os::unix::fs::PermissionsExt;

    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    fs::create_dir(world.default_folder()).unwrap();
    fs::set_permissions(world.default_folder(), fs::Permissions::from_mode(0o555)).unwrap();

    let refused = world.upgrade(id, None).unwrap_err();

    fs::set_permissions(world.default_folder(), fs::Permissions::from_mode(0o755)).unwrap();
    assert_unusable(&refused, &world.default_folder(), "notWritable");
    assert!(names_in(&world.default_folder()).is_empty());
    assert!(is_held(&world.path()), "still waiting");
    world.upgrade(id, None).expect("the same awaiting id, once the folder can be written");
}

/// 009 FR-012: too little room, by the free-space seam `backup_test` uses and
/// never by filling a volume (CLAUDE.md memory "No filling real volumes").
#[test]
fn too_little_room_for_the_backup_is_refused_and_the_database_keeps_waiting() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    let size = fs::metadata(world.path()).unwrap().len();
    // Just under the file size + 5%.
    let short = disk_space::testing::fake_available_space(move |_| Some(size + size / 20 - 1));

    let refused = world.upgrade(id, None).unwrap_err();

    assert_eq!(refused.code, "INSUFFICIENT_SPACE", "{refused:?}");
    assert_eq!(details(&refused)["path"], json!(text(&world.default_folder())));
    assert!(before_backups(&world.default_folder()).is_empty());
    assert!(no_partials(&world.default_folder()));
    assert!(is_held(&world.path()), "still waiting");
    drop(short);
    world.upgrade(id, None).expect("the same awaiting id, once there is room");
}

/// 009 FR-012: the same check for a chosen folder.
#[test]
fn too_little_room_in_a_chosen_folder_is_refused_naming_it() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    let chosen = TempDir::new().unwrap();
    let chosen_text = text(chosen.path());
    let size = fs::metadata(world.path()).unwrap().len();
    let wanted = chosen.path().to_owned();
    let _short = disk_space::testing::fake_available_space(move |dir| {
        (dir == wanted).then_some(size + size / 20 - 1)
    });

    let refused = world.upgrade(id, Some(&chosen_text)).unwrap_err();

    assert_eq!(refused.code, "INSUFFICIENT_SPACE", "{refused:?}");
    assert_eq!(details(&refused)["path"], json!(chosen_text));
    assert!(names_in(chosen.path()).is_empty());
    assert!(is_held(&world.path()), "still waiting");
}

/// 009 FR-012: a chosen folder gets the backup, outside the listing.
#[test]
fn a_chosen_folder_gets_the_backup_and_is_not_the_backup_location() {
    let world = World::new();
    world.older_database(Older::default());
    let database_id = database_id_of(&world.path());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    let chosen = TempDir::new().unwrap();

    let status = world.upgrade(id, Some(&text(chosen.path()))).unwrap();

    let name = expected_before_name(&world, &database_id);
    assert_eq!(names_in(chosen.path()), std::slice::from_ref(&name));
    assert_eq!(
        as_json(&status)["notes"]["upgraded"],
        json!({
            "version": APP_VERSION,
            "backupPath": text(&chosen.path().join(&name)),
            "atBackupLocation": false,
        })
    );
    assert!(!world.default_folder().exists(), "the backup location got nothing");
    let backup = peek(&chosen.path().join(&name));
    let stamped: Option<String> = backup
        .query_row("SELECT backup_before_upgrade_to FROM app_state", [], |r| r.get(0))
        .unwrap();
    assert_eq!(stamped.as_deref(), Some(APP_VERSION));
}

/// 009 FR-012, 009 research.md §6: a folder that resolves to the backup
/// location is the backup location.
#[test]
fn a_chosen_folder_that_resolves_to_the_backup_location_is_the_backup_location() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    fs::create_dir(world.default_folder()).unwrap();
    // The same folder, spelled another way.
    let spelled = world.default_folder().join("..").join("HoploDex backups");
    assert_ne!(spelled, world.default_folder());

    let status = world.upgrade(id, Some(&text(&spelled))).unwrap();

    assert_eq!(as_json(&status)["notes"]["upgraded"]["atBackupLocation"], json!(true));
    assert_eq!(before_backups(&world.default_folder()).len(), 1);
}

/// 009 FR-012: a relative path, a missing folder, a file and (Unix) an
/// unwritable folder are each refused, never created, and the database keeps
/// waiting for the same id.
#[test]
fn a_chosen_folder_that_cannot_be_used_is_refused_and_never_created() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    let scratch = TempDir::new().unwrap();
    let relative = format!("relative-{}/never-created", db::random_hex(4).unwrap());
    let missing = scratch.path().join("Unplugged drive").join("Backups");
    let a_file = scratch.path().join("a file");
    fs::write(&a_file, b"precious").unwrap();

    // Only the unix block below pushes more.
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut refusals = vec![
        (relative.clone(), world.upgrade(id, Some(&relative)).unwrap_err()),
        (text(&missing), world.upgrade(id, Some(&text(&missing))).unwrap_err()),
        (text(&a_file), world.upgrade(id, Some(&text(&a_file))).unwrap_err()),
    ];
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let locked = scratch.path().join("Read only");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        refusals.push((text(&locked), world.upgrade(id, Some(&text(&locked))).unwrap_err()));
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(names_in(&locked).is_empty());
    }

    for (folder, refused) in &refusals {
        // `path` names the folder as it was sent; the reason for each is the
        // contract's own (`missing` or `notWritable`).
        assert_eq!(refused.code, "BACKUP_LOCATION_UNAVAILABLE", "{folder}: {refused:?}");
        assert_eq!(details(refused)["path"], json!(folder), "{refused:?}");
        let reason = details(refused)["reason"].as_str().unwrap().to_owned();
        assert!(["missing", "notWritable"].contains(&reason.as_str()), "{folder}: {reason}");
    }
    assert!(!Path::new(&relative).exists(), "a relative folder is never made");
    assert!(!Path::new(&relative).parent().unwrap().exists());
    assert!(
        !missing.exists() && !missing.parent().unwrap().exists(),
        "and neither is a missing one"
    );
    assert_eq!(fs::read(&a_file).unwrap(), b"precious");
    assert!(!world.default_folder().exists(), "the backup location was not used either");
    assert!(is_held(&world.path()), "still waiting through all of them");
    let good = TempDir::new().unwrap();
    world
        .upgrade(id, Some(&text(good.path())))
        .expect("then a usable folder, the same awaiting id");
    assert_eq!(before_backups(good.path()).len(), 1);
}

/// 009 FR-012, 009 research.md §6, 003 FR-026: an existing file of the backup's
/// name, or of its partial's, is never overwritten, in the backup location or in
/// a chosen folder.
#[test]
fn an_existing_file_of_the_backups_name_is_never_overwritten() {
    for chosen in [false, true] {
        for suffix in ["", ".partial"] {
            let world = World::new();
            world.older_database(Older::default());
            let database_id = database_id_of(&world.path());
            let _scope = world.steps(&[STEP_A]);
            let id = world.waiting();
            let picked = TempDir::new().unwrap();
            let folder = if chosen { picked.path().to_owned() } else { world.default_folder() };
            fs::create_dir_all(&folder).unwrap();
            let planted =
                folder.join(format!("{}{suffix}", expected_before_name(&world, &database_id)));
            fs::write(&planted, b"somebody else's file").unwrap();

            let refused = world.upgrade(id, chosen.then(|| text(&folder)).as_deref()).unwrap_err();

            assert_eq!(refused.code, "UPGRADE_BACKUP_FAILED", "{chosen} {suffix}: {refused:?}");
            assert_eq!(details(&refused), json!({ "folder": text(&folder) }));
            assert_eq!(fs::read(&planted).unwrap(), b"somebody else's file", "not touched");
            assert_eq!(names_in(&folder).len(), 1, "and nothing else was left");
            assert_eq!(world.machine.unfinished_backup(), None, "nothing is recorded to sweep");
            assert!(is_held(&world.path()), "still waiting");
            // Once the name is free, the same id goes through.
            fs::remove_file(&planted).unwrap();
            world.upgrade(id, chosen.then(|| text(&folder)).as_deref()).unwrap();
        }
    }
}

/// An I/O failure at `point` while the backup is made: `UPGRADE_BACKUP_FAILED`
/// with the folder, no `.partial` and nothing recorded, the database still
/// waiting, and the same `awaitingId` upgrading once the failure is gone.
fn assert_backup_failure_keeps_waiting(point: Point, chosen: bool) {
    let world = World::new();
    world.older_database(Older::default());
    let scope = world.steps(&[STEP_A]).fail_at(point);
    let id = world.waiting();
    let picked = TempDir::new().unwrap();
    let folder = if chosen { picked.path().to_owned() } else { world.default_folder() };
    let folder_text = chosen.then(|| text(&folder));

    let failed = world.upgrade(id, folder_text.as_deref()).unwrap_err();

    assert_eq!(failed.code, "UPGRADE_BACKUP_FAILED", "{point:?}: {failed:?}");
    assert_eq!(details(&failed), json!({ "folder": text(&folder) }), "{point:?}");
    assert!(no_partials(&folder), "{point:?}: a .partial was left: {:?}", names_in(&folder));
    assert!(before_backups(&folder).is_empty(), "{point:?}");
    assert_eq!(world.machine.unfinished_backup(), None, "{point:?}");
    assert!(is_held(&world.path()), "{point:?}: the database is still waiting");
    scope.clear_failures();
    let status = world
        .upgrade(id, folder_text.as_deref())
        .unwrap_or_else(|err| panic!("{point:?}: the same awaiting id then upgrades: {err:?}"));
    assert_eq!(as_json(&status)["notes"]["upgraded"]["atBackupLocation"], json!(!chosen));
    assert_eq!(before_backups(&folder).len(), 1, "{point:?}");
}

/// 009 FR-012: an I/O failure during the copy, in the backup location and in a
/// chosen folder.
#[test]
fn an_io_failure_during_the_copy_is_upgrade_backup_failed_and_the_database_keeps_waiting() {
    assert_backup_failure_keeps_waiting(Point::DuringCopy, false);
    assert_backup_failure_keeps_waiting(Point::DuringCopy, true);
}

// ===========================================================================
// 009 T037: failure and interruption at each point
// ===========================================================================

/// 009 US3-3, 009 FR-015, 009 SC-005: a failure before the backup, at its stamp
/// and before its rename keeps the database waiting.
#[test]
fn a_failure_in_the_backup_phase_leaves_no_partial_and_the_database_waiting() {
    for point in [Point::BeforeBackup, Point::AtStamp, Point::BeforeRename] {
        assert_backup_failure_keeps_waiting(point, false);
    }
    assert_backup_failure_keeps_waiting(Point::AtStamp, true);
}

/// A forced or real failure from the first step on: `UPGRADE_FAILED` with the
/// backup's path, the file as it was, the finished backup still there, the slot
/// empty, and the next open asking again.
fn assert_rolled_back(steps: &[Step], failure: Option<Point>) {
    let names: Vec<&str> = steps.iter().map(|step| step.0).collect();
    let label = format!("{names:?} {failure:?}");
    let world = World::new();
    world.older_database(Older { backups_on: true, regular_today: true, ..Older::default() });
    let before = snapshot(&world.closed());
    let mut scope = world.steps(steps);
    if let Some(point) = failure {
        scope = scope.fail_at(point);
    }
    let id = world.waiting();

    let failed = world.upgrade(id, None).unwrap_err();

    assert_eq!(failed.code, "UPGRADE_FAILED", "{label}: {failed:?}");
    let backups = before_backups(&world.default_folder());
    assert_eq!(backups.len(), 1, "{label}: the finished backup is still there");
    assert_eq!(
        details(&failed),
        json!({
            "backupPath": text(&world.default_folder().join(&backups[0])),
            "atBackupLocation": true,
        }),
        "{label}"
    );
    assert!(no_partials(&world.default_folder()));
    assert!(!world.session.is_open(), "{label}");
    assert!(!is_held(&world.path()), "{label}: the slot is empty and the lock released");
    assert_eq!(snapshot(&world.closed()), before, "{label}: rows and sqlite_schema as they were");
    assert_eq!(world.upgrade(id, None).unwrap_err().code, "DATABASE_CLOSED", "{label}");
    // The next open is the question again.
    let again = world.waiting();
    assert_ne!(again, id);
    world.cancel(again);
    drop(scope);
}

/// 009 US3-3, 009 FR-015, 009 SC-005: after each of two steps, at the link
/// check and before the commit.
#[test]
fn a_failure_from_the_first_step_on_rolls_back_and_drops_the_database() {
    for point in [Point::AfterStep(1), Point::AfterStep(2), Point::AtLinkCheck, Point::BeforeCommit]
    {
        assert_rolled_back(&[STEP_A, STEP_B], Some(point));
    }
}

/// 009 FR-015, 009 research.md §7: an orphan planted by a step is caught by the
/// link check, and a step with invalid SQL fails with the steps before it.
#[test]
fn an_orphan_or_a_broken_step_fails_the_upgrade_and_leaves_the_file_as_it_was() {
    assert_rolled_back(&[STEP_A, STEP_ORPHAN], None);
    assert_rolled_back(&[STEP_A, STEP_INVALID], None);
}

/// 009 FR-015, 009 research.md §7, §14: the interruption. With `cache_size = 1`
/// changed pages spill into the file, so at each point inside the transaction a
/// copy of the file (through `RawFile`) and its journal is what a crash there
/// leaves. Opening the copy rolls back to the earlier rows and layout, and asks
/// the question again; the backup finished before the point is still there.
#[test]
fn an_upgrade_interrupted_inside_the_transaction_rolls_back_and_asks_again() {
    let points =
        [Point::AfterStep(1), Point::AfterStep(2), Point::AtLinkCheck, Point::BeforeCommit];
    let world = World::new();
    world.older_database(Older::default());
    let before = snapshot(&world.closed());
    let old_hash = sha(&world.path());
    let journal = world.dir.path().join("Mine.hoplodex-journal");
    let steps = [STEP_BIG, STEP_B];
    // One folder per crash, each with the steps registered, to open its copy in.
    let crashes: Vec<World> = points.iter().map(|_| World::new()).collect();
    // (the copy differs from the original, its journal's length, backups then)
    let seen: Arc<Mutex<Vec<(bool, u64, usize)>>> = Arc::default();
    let mut scope = world.steps(&steps);
    for (point, crash) in points.iter().zip(&crashes) {
        let (copy, journal, old_hash, seen, backups) =
            (crash.path(), journal.clone(), old_hash.clone(), seen.clone(), world.default_folder());
        scope = scope.at_point(*point, move |conn: &Connection| {
            let mut out = File::create(&copy).unwrap();
            let file = RawFile::of(conn).unwrap();
            let size = file.size().unwrap();
            let mut buf = vec![0u8; 64 * 1024];
            let mut offset = 0;
            while offset < size {
                let len = (buf.len() as u64).min(size - offset) as usize;
                file.read_exact_at(&mut buf[..len], offset).unwrap();
                out.write_all(&buf[..len]).unwrap();
                offset += len as u64;
            }
            out.sync_all().unwrap();
            drop(out);
            let copied_journal = PathBuf::from(format!("{}-journal", copy.display()));
            fs::copy(&journal, &copied_journal)
                .expect("there is no journal beside the transaction");
            seen.lock().unwrap().push((
                sha(&copy) != old_hash,
                fs::metadata(&copied_journal).unwrap().len(),
                before_backups(&backups).len(),
            ));
        });
    }
    let id = world.waiting();
    world.with_held(id, |conn| conn.pragma_update(None, "cache_size", 1).unwrap());

    world.upgrade(id, None).expect("the upgrade itself goes through");

    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.len(), points.len(), "a hook did not run: {seen:?}");
    for (_, journal_len, backups) in &seen {
        assert!(*journal_len > 0, "the copy's journal is empty, so it would not be hot");
        assert_eq!(*backups, 1, "009 FR-015: the finished backup is there at every point");
    }
    assert!(
        seen.iter().any(|(differs, ..)| *differs),
        "no changed page ever reached the file, so nothing was rolled back"
    );
    for (point, crash) in points.iter().zip(&crashes) {
        let _scope = crash.steps(&steps);
        let asked = crash.waiting();
        let rolled_back = crash.with_held(asked, snapshot);
        assert_eq!(rolled_back, before, "{point:?}: the copy was not rolled back");
        let integrity: String = crash
            .with_held(asked, |conn| conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)))
            .unwrap();
        assert_eq!(integrity, "ok", "{point:?}");
        crash.cancel(asked);
    }
}

/// 009 FR-015, 009 research.md §6: a `.partial` left at a point in the copy (what
/// a crash there leaves, with its record in `machine.json`) is removed by the
/// launch sweep, with no chooser notice.
#[test]
fn a_partial_left_by_a_crash_in_the_backup_is_swept_with_no_notice() {
    for point in [Point::DuringCopy, Point::AtStamp, Point::BeforeRename] {
        let world = World::new();
        world.older_database(Older::default());
        let record: Arc<Mutex<Option<hoplodex_lib::services::machine_settings::UnfinishedBackup>>> =
            Arc::default();
        let (taken, config) = (record.clone(), world.config.path().to_owned());
        let _scope = world.steps(&[STEP_A]).at_point(point, move |_| {
            let mut taken = taken.lock().unwrap();
            if taken.is_none() {
                // What a crash here would leave in machine.json.
                *taken = MachineSettings::load(&config).unwrap().unfinished_backup();
            }
        });
        let id = world.waiting();
        world.upgrade(id, None).unwrap();

        let record = record.lock().unwrap().take().unwrap_or_else(|| {
            panic!("{point:?}: no unfinished backup was recorded when the point was reached")
        });
        assert_eq!(
            serde_json::to_value(&record).unwrap()["beforeUpgrade"],
            json!(true),
            "{point:?}: the sweep must know this one is not to be reported"
        );
        // The crash: the partial is still there and still recorded.
        fs::write(&record.partial_path, b"half a backup").unwrap();
        world.machine.set_unfinished_backup(record.clone());
        backups::sweep_unfinished(&world.machine);

        assert!(!record.partial_path.exists(), "{point:?}");
        assert_eq!(world.machine.unfinished_backup(), None, "{point:?}");
        assert_eq!(
            world.machine.take_notices(),
            vec![],
            "{point:?}: an interruption is not reported"
        );
    }
}

/// A multi-chunk database whose upgrade is waiting, for the cancel tests.
fn waiting_big() -> (World, u64, String, testing::Scope) {
    let world = World::new();
    world.older_database(Older { extra_bytes: 3 << 20, ..Older::default() });
    let before = sha(&world.path());
    let scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    (world, id, before, scope)
}

/// The backup was stopped: `OPERATION_STOPPED` for the upgrade, the partial
/// removed, the file untouched and the slot empty.
fn assert_stopped(world: &World, id: u64, before: &str, stopped: CommandError) {
    assert_eq!(stopped.code, "OPERATION_STOPPED", "{stopped:?}");
    assert_eq!(details(&stopped)["operation"], json!("upgrade"));
    let folder = world.default_folder();
    assert!(names_in(&folder).is_empty(), "the partial is removed: {:?}", names_in(&folder));
    assert_eq!(world.machine.unfinished_backup(), None);
    assert!(!is_held(&world.path()), "the slot is empty");
    assert_eq!(sha(&world.path()), before, "009 FR-016: not a byte changed");
    assert_eq!(world.upgrade(id, None).unwrap_err().code, "DATABASE_CLOSED");
    assert!(!world.session.is_open());
}

/// 009 US3-3, 009 FR-016: Cancel while the backup is made.
#[test]
fn cancel_between_backup_chunks_removes_the_partial_and_stops_the_upgrade() {
    let (world, id, before, _scope) = waiting_big();
    let (session, awaiting) = (world.session.clone(), id);
    world.at_progress(
        |p| p["phase"] == "backingUp" && p["processed"].as_u64().unwrap() > 0,
        move || databases::cancel_upgrade(&session, awaiting).unwrap(),
    );

    let stopped = world.upgrade(id, None).unwrap_err();

    assert_stopped(&world, id, &before, stopped);
    assert!(
        world.progress().iter().all(|p| p["phase"] == "backingUp"),
        "the upgrade itself never began"
    );
}

/// 009 FR-016, 003 FR-037: a shutdown during the backup does the same.
#[test]
fn a_shutdown_during_the_backup_stops_the_upgrade_the_same_way() {
    let (world, id, before, _scope) = waiting_big();
    let (session, machine) = (world.session.clone(), world.machine.clone());
    world.at_progress(
        |p| p["phase"] == "backingUp" && p["processed"].as_u64().unwrap() > 0,
        move || lifecycle::close_immediate(&session, &machine, CloseReason::Shutdown),
    );

    let stopped = world.upgrade(id, None).unwrap_err();

    assert_stopped(&world, id, &before, stopped);
}

/// 009 FR-016: a stop before the first chunk and at the last one are stops too.
#[test]
fn cancel_before_the_first_chunk_and_after_the_last_also_stop_the_upgrade() {
    for first in [true, false] {
        let (world, id, before, _scope) = waiting_big();
        let (session, awaiting) = (world.session.clone(), id);
        world.at_progress(
            move |p| {
                let (done, total) =
                    (p["processed"].as_u64().unwrap(), p["total"].as_u64().unwrap());
                p["phase"] == "backingUp" && if first { done == 0 } else { done == total }
            },
            move || databases::cancel_upgrade(&session, awaiting).unwrap(),
        );

        let stopped = world.upgrade(id, None).unwrap_err();

        assert_stopped(&world, id, &before, stopped);
    }
}

/// 009 FR-016: once the `upgrading` phase has begun, Cancel is ignored and the
/// upgrade completes.
#[test]
fn cancel_once_the_upgrade_has_begun_is_ignored() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    let (session, awaiting) = (world.session.clone(), id);
    world.at_progress(
        |p| p["phase"] == "upgrading",
        move || databases::cancel_upgrade(&session, awaiting).unwrap(),
    );

    let status = world.upgrade(id, None).expect("the upgrade completes");

    assert!(as_json(&status)["notes"]["upgraded"].is_object());
    assert_eq!(world.open_state().2.last().map(String::as_str), Some("9001_test_a"));
    assert_eq!(before_backups(&world.default_folder()).len(), 1);
}

// ===========================================================================
// 009 T038: the slot's life and the edge cases
// ===========================================================================

/// A healthy database elsewhere, with a backup, closed: to open, create over or
/// restore while another database waits. Returns its folder, file and backup.
fn another_database(world: &World) -> (TempDir, PathBuf, String) {
    let folder = TempDir::new().unwrap();
    let path = folder.path().join("Other.hoplodex");
    lifecycle::create(&world.session, &world.machine, &path, &passphrase()).unwrap();
    world.change();
    assert_eq!(world.close().backup, BackupOutcome::Made);
    let id = database_id_of(&path);
    let backup = backups::list(&folder.path().join("HoploDex backups"), &id).unwrap().remove(0);
    world.clock.advance(chrono::Duration::hours(1));
    (folder, path, backup.path)
}

/// 009 research.md §4 "The slot's life": any other open drops it.
#[test]
fn another_open_drops_the_waiting_database() {
    let world = World::new();
    let (_other, other_path, _) = another_database(&world);
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();

    lifecycle::open(&world.session, &world.machine, &other_path, &passphrase(), false).unwrap();

    assert!(world.session.is_open(), "the other database is the open one");
    world.assert_dropped(id);
    assert!(world.session.is_open(), "and the stale answers did not close it");
}

/// 009 research.md §4: the same database opened again is a new question, and the
/// first is dropped before the second needs the file's lock.
#[test]
fn opening_the_waiting_database_again_drops_the_first_question() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let first = world.waiting();

    let second = world.waiting();

    assert!(second > first, "an awaiting id never goes back");
    assert_eq!(world.upgrade(first, None).unwrap_err().code, "DATABASE_CLOSED");
    // A stale cancel does nothing to the one that waits now.
    world.cancel(first);
    assert!(is_held(&world.path()), "the second question is still waiting");
    world.upgrade(second, None).expect("the second question is the live one");
}

/// 009 research.md §4: any create drops it.
#[test]
fn a_create_drops_the_waiting_database() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();
    let elsewhere = TempDir::new().unwrap();

    lifecycle::create(
        &world.session,
        &world.machine,
        &elsewhere.path().join("New.hoplodex"),
        &passphrase(),
    )
    .unwrap();

    assert!(world.session.is_open());
    world.assert_dropped(id);
    assert!(world.session.is_open());
}

/// 009 research.md §4: any restore beginning drops it.
#[test]
fn a_restore_drops_the_waiting_database() {
    let world = World::new();
    let (_other, other_path, backup) = another_database(&world);
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();

    backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &backup,
        &passphrase(),
        Target::At(&text(&other_path)),
        false,
        &Reach::Unconfined,
    )
    .unwrap();

    world.assert_dropped(id);
}

/// 009 research.md §4: quit drops it.
#[test]
fn quitting_drops_the_waiting_database() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let id = world.waiting();

    lifecycle::quit(&world.session, &world.machine);

    world.assert_dropped(id);
}

/// 009 research.md §4: sleep, screen lock and shutdown drop it (the first two
/// with `system:clear-passphrase-fields`, which closes the question in the
/// chooser as Cancel), whatever the lock settings.
#[test]
fn sleep_screen_lock_and_shutdown_drop_the_waiting_database() {
    type Event = fn(&Session, &MachineSettings);
    let events: [(&str, Event, bool); 3] = [
        ("sleep", lifecycle::will_sleep, true),
        ("screen lock", lifecycle::screen_locked, true),
        ("shutdown", lifecycle::will_shut_down, false),
    ];
    for (name, event, clears_fields) in events {
        let world = World::new();
        world.older_database(Older::default());
        let _scope = world.steps(&[STEP_A]);
        let id = world.waiting();

        event(&world.session, &world.machine);

        world.assert_dropped(id);
        if clears_fields {
            assert_eq!(
                world.events.payloads("system:clear-passphrase-fields").len(),
                1,
                "{name}: the chooser's fields clear"
            );
        }
        // Cancel after the fact is still nothing, and the next open asks again.
        let again = world.waiting();
        assert_ne!(again, id, "{name}");
        world.cancel(again);
    }
}

/// 009 Edge Cases, 003 FR-032: a database marked open by another computer is
/// refused first, and the take-over resend reaches the upgrade question.
#[test]
fn open_elsewhere_comes_before_the_upgrade_question_and_the_take_over_resend_reaches_it() {
    let world = World::new();
    world.older_database(Older::default());
    let other = other_machine();
    raw_exec(
        &world.path(),
        "UPDATE app_state SET open_machine_id = ?1, open_machine_name = ?2, open_since = ?3",
        rusqlite::params![other.id, other.display_name, "2026-09-25T10:00:00Z"],
    );
    let _scope = world.steps(&[STEP_A]);
    let before = sha(&world.path());

    let refused =
        lifecycle::open(&world.session, &world.machine, &world.path(), &passphrase(), false)
            .unwrap_err();

    assert_eq!(refused.code, "DATABASE_OPEN_ELSEWHERE", "{refused:?}");
    assert!(!is_held(&world.path()), "no question is waiting");
    assert_eq!(sha(&world.path()), before);
    let id = world.waiting_at(&world.path(), true);
    assert!(is_held(&world.path()));
    let status = world.upgrade(id, None).unwrap();
    assert!(as_json(&status)["notes"]["upgraded"].is_object());
    let marker = world.session.inspect(world.open_id(), |open| Ok(marker_of(&open.conn))).unwrap();
    assert_eq!(marker, Some(world.machine.identity().id), "this computer holds it now");
}

/// 009 research.md §4 step 5a, 009 contracts/tauri-commands.md `upgrade_database`
/// order 2: a marker written by another computer during the question refuses the
/// upgrade, with nothing written and the slot empty.
#[test]
fn a_marker_written_by_another_computer_during_the_question_refuses_the_upgrade() {
    let world = World::new();
    world.older_database(Older::default());
    let _scope = world.steps(&[STEP_A]);
    let before_steps = applied_steps(&world.closed());
    let id = world.waiting();
    let other = other_machine();
    world.with_held(id, |conn| {
        conn.execute(
            "UPDATE app_state SET open_machine_id = ?1, open_machine_name = ?2, open_since = ?3",
            rusqlite::params![other.id, other.display_name, "2026-09-25T15:00:00Z"],
        )
        .unwrap();
    });

    let refused = world.upgrade(id, None).unwrap_err();

    assert_eq!(refused.code, "DATABASE_OPEN_ELSEWHERE", "{refused:?}");
    assert_eq!(details(&refused)["machineName"], json!("Workshop PC"));
    assert!(!world.default_folder().exists(), "refused before any backup");
    assert!(!is_held(&world.path()), "the slot is empty");
    let file = world.closed();
    assert_eq!(applied_steps(&file), before_steps, "no step was applied");
    assert_eq!(marker_of(&file), Some(other.id), "and the other computer's marker stands");
    assert_eq!(world.upgrade(id, None).unwrap_err().code, "DATABASE_CLOSED");
    assert!(!world.session.is_open());
}

/// A backup of "Mine" made today, in the world's backup folder, and the world's
/// folder for databases that are older than the build when opened directly.
fn a_backup_that_needs_an_upgrade(world: &World) -> (PathBuf, String) {
    world.create();
    world.fill();
    world.change();
    let id = database_id_of_open(world);
    assert_eq!(world.close().backup, BackupOutcome::Made);
    world.clock.advance(chrono::Duration::hours(1));
    let backup = backups::list(&world.default_folder(), &id).unwrap().remove(0);
    (PathBuf::from(backup.path), id)
}

fn database_id_of_open(world: &World) -> String {
    world.session.inspect(world.open_id(), |open| Ok(open.database_id.clone())).unwrap()
}

/// 009 Edge Cases, 009 research.md §8: Cancel leaves a backup opened directly
/// untouched, stamp and all.
#[test]
fn cancelling_the_upgrade_of_a_backup_opened_directly_leaves_it_a_backup() {
    let world = World::new();
    let (backup, _) = a_backup_that_needs_an_upgrade(&world);
    let _scope = testing::scope(backup.parent().unwrap()).extra_steps(&[STEP_A]);
    let before = sha(&backup);
    let stamp_before = app_record(&peek(&backup));
    assert_ne!(stamp_before[5], "Null", "it carries a backup stamp");

    let id = world.waiting_at(&backup, false);
    world.cancel(id);

    assert!(!is_held(&backup));
    assert_eq!(sha(&backup), before, "not a byte changed");
    assert_eq!(app_record(&peek(&backup)), stamp_before, "still a backup, with its own id");
    world.assert_no_notice();
}

/// 009 Edge Cases, 009 research.md §8: Upgrade gives it a new id first; its
/// backup before the upgrade is named with that id, stamped with it, and goes to
/// the backup folder its own settings name; the opened-backup note is shown.
#[test]
fn upgrading_a_backup_opened_directly_gives_it_a_new_id_and_backs_up_under_that_id() {
    let world = World::new();
    let (backup, original_id) = a_backup_that_needs_an_upgrade(&world);
    let _scope = testing::scope(backup.parent().unwrap()).extra_steps(&[STEP_A]);
    let backup_stem = backup.file_stem().unwrap().to_string_lossy().into_owned();
    let id = world.waiting_at(&backup, false);

    let status = world.upgrade(id, None).unwrap();

    let new_id = database_id_of_open(&world);
    assert_ne!(new_id, original_id, "it became a database of its own");
    // Resolved from the file's own path and settings: a "HoploDex backups"
    // folder beside the file.
    let folder = backup.parent().unwrap().join("HoploDex backups");
    let made = before_backups(&folder);
    assert_eq!(
        made,
        [format!(
            "{backup_stem} {} {} before {APP_VERSION}.hoplodex",
            world.clock.now().format("%Y-%m-%d %H%M%S"),
            &new_id[..8]
        )]
    );
    let stamped = peek(&folder.join(&made[0]));
    let (stored_id, made_at, of_name, to): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = stamped
        .query_row(
            "SELECT database_id, backup_made_at, backup_of_name, backup_before_upgrade_to
                 FROM app_state",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(stored_id, new_id, "stamped with the new id");
    assert!(made_at.is_some() && of_name.is_some());
    assert_eq!(to.as_deref(), Some(APP_VERSION));
    assert_eq!(backups::list(&folder, &new_id).unwrap().len(), 1, "listed under the new id");
    let json = as_json(&status);
    assert_eq!(json["notes"]["openedBackup"]["backupOfName"], json!("Mine"));
    assert_eq!(json["notes"]["upgraded"]["atBackupLocation"], json!(true));
    // The stamp is taken in the upgrade's transaction.
    let (_, record, applied) = world.open_state();
    assert_eq!(record[5], "Null", "backup_made_at is cleared");
    assert_eq!(record[7], "Null", "and so is backup_before_upgrade_to");
    assert_eq!(applied.last().map(String::as_str), Some("9001_test_a"));
}

/// 009 Edge Cases (several releases skipped), 009 research.md §7: two steps
/// upgrade in one go.
#[test]
fn two_steps_upgrade_in_one_go() {
    let world = World::new();
    world.older_database(Older::default());
    let before = collection(&world.closed());
    let before_steps = applied_steps(&world.closed());
    let _scope = world.steps(&[STEP_A, STEP_B]);
    let id = world.waiting();

    world.upgrade(id, None).unwrap();

    let (after, _, applied) = world.open_state();
    assert_eq!(after, before);
    assert_eq!(applied[before_steps.len()..], ["9001_test_a", "9002_test_b"]);
    assert_eq!(before_backups(&world.default_folder()).len(), 1, "one backup for both");
}

/// 009 Edge Cases (several releases skipped), 009 FR-015: a failure in the second
/// of two steps leaves neither.
#[test]
fn a_failure_in_the_second_of_two_steps_leaves_neither() {
    let world = World::new();
    world.older_database(Older::default());
    let before = snapshot(&world.closed());
    let _scope = world.steps(&[STEP_A, STEP_INVALID]);
    let id = world.waiting();

    let failed = world.upgrade(id, None).unwrap_err();

    assert_eq!(failed.code, "UPGRADE_FAILED", "{failed:?}");
    assert!(!is_held(&world.path()));
    let after = snapshot(&world.closed());
    assert_eq!(after, before, "neither the first step nor the second");
    assert!(!after.rows.contains_key("t_a") && !after.rows.contains_key("t_invalid"));
}

// ===========================================================================
// 009 T098, T099, T101, T103: the slot while no database is in it, a sleep or
// shutdown during the backup and the upgrade, and a full disk
// ===========================================================================

type SystemEvent = fn(&Session, &MachineSettings);

/// The events that drop a waiting database (009 research.md §4).
const DROPPING_EVENTS: [(&str, SystemEvent); 3] = [
    ("sleep", lifecycle::will_sleep),
    ("screen lock", lifecycle::screen_locked),
    ("shutdown", lifecycle::will_shut_down),
];

/// What an open or an upgrade that lost its database to an event leaves: no
/// lock held, nothing open, no backup, the file as it was.
fn assert_nothing_waits(world: &World, before: &str, label: &str) {
    assert!(!is_held(&world.path()), "{label}: the database still holds its lock");
    assert!(!world.session.is_open(), "{label}");
    assert!(before_backups(&world.default_folder()).is_empty(), "{label}: a backup was made");
    assert_eq!(sha(&world.path()), before, "{label}: not a byte changed");
}

/// 009 FR-010, 009 research.md §4 "The slot's life": a sleep, screen lock or
/// shutdown that comes while an open derives its key finds the slot empty, and
/// still wins: nothing is put in it, and the open fails with `DATABASE_CLOSED`.
#[test]
fn an_event_during_the_opens_key_derivation_leaves_no_waiting_database() {
    for (name, event) in DROPPING_EVENTS {
        let world = World::new();
        world.older_database(Older::default());
        let before = sha(&world.path());
        let (session, machine) = (world.session.clone(), world.machine.clone());
        let scope = world
            .steps(&[STEP_A])
            .at_point(Point::BeforeWaiting, move |_| event(&session, &machine));

        let refused =
            lifecycle::open(&world.session, &world.machine, &world.path(), &passphrase(), false)
                .unwrap_err();

        assert_eq!(refused.code, "DATABASE_CLOSED", "{name}: {refused:?}");
        assert_nothing_waits(&world, &before, name);
        // The next open asks again.
        drop(scope);
        let _scope = world.steps(&[STEP_A]);
        let id = world.waiting();
        world.cancel(id);
    }
}

/// 009 FR-010, 009 FR-012: the same while a refusal to make the backup is on
/// its way back to the slot: the database is dropped, not put back, and the
/// upgrade ends as a stopped one.
#[test]
fn an_event_while_a_location_refusal_returns_leaves_no_waiting_database() {
    for (name, event) in DROPPING_EVENTS {
        let world = World::new();
        world.older_database(Older::default());
        let before = sha(&world.path());
        let (session, machine) = (world.session.clone(), world.machine.clone());
        let _scope = world
            .steps(&[STEP_A])
            .at_point(Point::BeforeBackup, move |_| event(&session, &machine));
        let id = world.waiting();
        fs::write(world.default_folder(), b"not a folder").unwrap();

        let stopped = world.upgrade(id, None).unwrap_err();

        assert_eq!(stopped.code, "OPERATION_STOPPED", "{name}: {stopped:?}");
        assert_eq!(details(&stopped)["operation"], json!("upgrade"), "{name}");
        assert_eq!(fs::read(world.default_folder()).unwrap(), b"not a folder", "{name}");
        assert!(!is_held(&world.path()), "{name}: the database still holds its lock");
        assert!(!world.session.is_open(), "{name}");
        assert_eq!(sha(&world.path()), before, "{name}: not a byte changed");
        assert_eq!(world.upgrade(id, None).unwrap_err().code, "DATABASE_CLOSED", "{name}");
    }
}

/// 009 FR-016, 003 FR-037: a sleep during the backup stops the upgrade as a
/// shutdown does (009 research.md §5).
#[test]
fn a_sleep_during_the_backup_stops_the_upgrade() {
    let (world, id, before, _scope) = waiting_big();
    let (session, machine) = (world.session.clone(), world.machine.clone());
    world.at_progress(
        |p| p["phase"] == "backingUp" && p["processed"].as_u64().unwrap() > 0,
        move || lifecycle::will_sleep(&session, &machine),
    );

    let stopped = world.upgrade(id, None).unwrap_err();

    assert_stopped(&world, id, &before, stopped);
}

/// 009 FR-016, 003 FR-037, 003 FR-032, 003 FR-040: a sleep or shutdown that
/// comes in the `upgrading` phase can't stop it: the upgrade finishes and
/// commits, the command ends as a stopped one, the database stays closed, and
/// this computer's open marker is gone, so another computer doesn't see the
/// database as open here.
#[test]
fn a_sleep_or_shutdown_during_the_upgrade_lets_it_finish_and_leaves_it_closed() {
    type Event = fn(&Session, &MachineSettings);
    let events: [(&str, Event); 2] =
        [("sleep", lifecycle::will_sleep), ("shutdown", lifecycle::will_shut_down)];
    for (name, event) in events {
        let world = World::new();
        world.older_database(Older::default());
        let _scope = world.steps(&[STEP_A]);
        let id = world.waiting();
        let (session, machine) = (world.session.clone(), world.machine.clone());
        world.at_progress(
            |p| p["phase"] == "upgrading",
            move || {
                event(&session, &machine);
            },
        );

        let stopped = world.upgrade(id, None).unwrap_err();

        assert_eq!(stopped.code, "OPERATION_STOPPED", "{name}: {stopped:?}");
        assert_eq!(details(&stopped)["operation"], json!("upgrade"), "{name}");
        assert!(!world.session.is_open(), "{name}: the database stays closed");
        assert!(!is_held(&world.path()), "{name}: the connection is dropped");
        let closed = world.closed();
        assert_eq!(
            applied_steps(&closed).last().map(String::as_str),
            Some("9001_test_a"),
            "{name}: the upgrade finished"
        );
        assert_eq!(marker_of(&closed), None, "{name}: no computer has it open");
        assert_eq!(before_backups(&world.default_folder()).len(), 1, "{name}");
        assert!(no_partials(&world.default_folder()), "{name}");
        assert_eq!(world.upgrade(id, None).unwrap_err().code, "DATABASE_CLOSED", "{name}");
    }
}

/// 009 FR-016, 003 FR-038: a screen lock that comes once Upgrade is chosen, in
/// the backup or in the upgrade, stops nothing. With "lock when the screen
/// locks" on, the database is upgraded and left closed, as a sleep leaves it
/// in the `upgrading` phase: this computer's open marker cleared, the
/// connection dropped, the command ended with `OPERATION_STOPPED`. With it off
/// the database opens with the upgraded note.
#[test]
fn a_screen_lock_during_the_upgrade_leaves_it_closed_only_when_the_setting_is_on() {
    for phase in ["backingUp", "upgrading"] {
        for lock_on in [true, false] {
            let label = format!("{phase}, setting {}", if lock_on { "on" } else { "off" });
            let world = World::new();
            // A multi-chunk file, for the lock to come between chunks.
            world.older_database(Older { extra_bytes: 3 << 20, ..Older::default() });
            raw_exec(
                &world.path(),
                "UPDATE collection_settings SET lock_on_screen_lock = ?1",
                rusqlite::params![lock_on],
            );
            let _scope = world.steps(&[STEP_A]);
            let id = world.waiting();
            let (session, machine) = (world.session.clone(), world.machine.clone());
            world.at_progress(
                move |p| {
                    p["phase"] == phase
                        && (phase == "upgrading" || p["processed"].as_u64().unwrap() > 0)
                },
                move || lifecycle::screen_locked(&session, &machine),
            );

            let result = world.upgrade(id, None);

            if lock_on {
                let stopped = result.expect_err(&label);
                assert_eq!(stopped.code, "OPERATION_STOPPED", "{label}: {stopped:?}");
                assert_eq!(details(&stopped)["operation"], json!("upgrade"), "{label}");
                assert!(!world.session.is_open(), "{label}: the database stays closed");
                assert!(!is_held(&world.path()), "{label}: the connection is dropped");
                let closed = world.closed();
                assert_eq!(
                    applied_steps(&closed).last().map(String::as_str),
                    Some("9001_test_a"),
                    "{label}: the upgrade finished"
                );
                assert_eq!(marker_of(&closed), None, "{label}: no computer has it open");
                assert_eq!(before_backups(&world.default_folder()).len(), 1, "{label}");
                assert!(no_partials(&world.default_folder()), "{label}");
                assert_eq!(world.machine.unfinished_backup(), None, "{label}");
                assert_eq!(world.upgrade(id, None).unwrap_err().code, "DATABASE_CLOSED", "{label}");
            } else {
                let status = result.unwrap_or_else(|err| panic!("{label}: {err:?}"));
                assert!(as_json(&status)["notes"]["upgraded"].is_object(), "{label}");
                assert!(world.session.is_open(), "{label}");
                assert_eq!(before_backups(&world.default_folder()).len(), 1, "{label}");
                assert!(no_partials(&world.default_folder()), "{label}");
            }
            // The screen lock found the slot empty, and said so to the chooser as any does.
            assert_eq!(world.events.payloads("system:clear-passphrase-fields").len(), 1, "{label}");
        }
    }
}

/// 009 FR-016, 003 FR-038: a screen lock whose note comes after the upgrade's
/// check and whose read of the settings comes before the database is
/// installed finds nothing to lock. The upgrade reads the note again once the
/// database is installed and locks it, so it ends upgraded and closed, whatever
/// the interleaving (009 research.md §5 "Sleep and shutdown").
#[test]
fn a_screen_lock_between_the_check_and_the_install_still_locks_the_database() {
    for lock_on in [true, false] {
        let label = format!("setting {}", if lock_on { "on" } else { "off" });
        let world = World::new();
        world.older_database(Older::default());
        raw_exec(
            &world.path(),
            "UPDATE collection_settings SET lock_on_screen_lock = ?1",
            rusqlite::params![lock_on],
        );
        let (session, machine) = (world.session.clone(), world.machine.clone());
        let _scope = world
            .steps(&[STEP_A])
            .at_point(Point::BeforeInstall, move |_| lifecycle::screen_locked(&session, &machine));
        let id = world.waiting();

        let result = world.upgrade(id, None);

        if lock_on {
            let stopped = result.expect_err(&label);
            assert_eq!(stopped.code, "OPERATION_STOPPED", "{label}: {stopped:?}");
            assert_eq!(details(&stopped)["operation"], json!("upgrade"), "{label}");
            assert!(!world.session.is_open(), "{label}: the database ends locked");
            assert!(!is_held(&world.path()), "{label}");
            let closed = world.closed();
            assert_eq!(
                applied_steps(&closed).last().map(String::as_str),
                Some("9001_test_a"),
                "{label}: the upgrade finished"
            );
            assert_eq!(marker_of(&closed), None, "{label}: no computer has it open");
            assert!(no_partials(&world.default_folder()), "{label}");
        } else {
            let status = result.unwrap_or_else(|err| panic!("{label}: {err:?}"));
            assert!(as_json(&status)["notes"]["upgraded"].is_object(), "{label}");
            assert!(world.session.is_open(), "{label}");
        }
    }
}

/// Caps the database at the pages it has, so a step that adds rows runs out of
/// room as SQLite says it (`SQLITE_FULL`), without a full disk.
fn no_room_to_grow(conn: &Connection) {
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0)).unwrap();
    let _: i64 =
        conn.query_row(&format!("PRAGMA max_page_count = {pages}"), [], |r| r.get(0)).unwrap();
}

/// 009 FR-015, 009 contracts/tauri-commands.md "upgrade_database": a full disk
/// during the upgrade rolls it back and says where the backup made before it
/// is, in the backup location and in a chosen folder.
#[test]
fn a_full_disk_during_the_upgrade_is_disk_full_naming_the_backup_made_before_it() {
    for chosen in [false, true] {
        let world = World::new();
        world.older_database(Older::default());
        let before_steps = applied_steps(&world.closed());
        let _scope = world.steps(&[STEP_BIG]).at_point(Point::BeforeBackup, no_room_to_grow);
        let id = world.waiting();
        let picked = TempDir::new().unwrap();
        let folder = if chosen { picked.path().to_owned() } else { world.default_folder() };
        let folder_text = chosen.then(|| text(&folder));

        let failed = world.upgrade(id, folder_text.as_deref()).unwrap_err();

        assert_eq!(failed.code, "DISK_FULL", "chosen {chosen}: {failed:?}");
        let backups = before_backups(&folder);
        assert_eq!(backups.len(), 1, "chosen {chosen}: the backup was made first");
        assert_eq!(
            details(&failed),
            json!({
                "path": text(&world.path()),
                "backupPath": text(&folder.join(&backups[0])),
                "atBackupLocation": !chosen,
            }),
            "chosen {chosen}"
        );
        assert!(!is_held(&world.path()), "chosen {chosen}: dropped");
        assert_eq!(applied_steps(&world.closed()), before_steps, "chosen {chosen}: rolled back");
    }
}
