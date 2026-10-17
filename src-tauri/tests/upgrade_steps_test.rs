//! The upgrade runner and the SQLite behaviours it depends on (009 research.md §4,
//! §7, §14). The two probes here run under SQLCipher with the app's connection
//! settings, on every OS the app ships on; they use only temporary folders and
//! SQLite's own file handle, so the same filter runs them everywhere:
//! `cargo test --features test-kdf --test integration upgrade_steps_test::`.

use crate::support::{self, TestDb};

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;

use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::db::upgrade::{self, UpgradeError, testing};
use hoplodex_lib::db::{self, OpenError, Opened, raw_file::RawFile};
use rusqlite::{Connection, ErrorCode};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

/// Set in the child process to the database it should try to read.
const CHILD_ENV: &str = "HOPLODEX_TEST_UPGRADE_PROBE_READ_FROM_CHILD";

/// A connection to `path` with the app's connection settings and the test
/// passphrase: the key, the pinned cipher settings, foreign keys, secure
/// deletion and exclusive locking, and a busy timeout of zero, as
/// `db::configure` sets them. Nothing is read yet, so a locked file is first
/// reported by the caller's first read.
fn app_connection(path: &Path) -> Connection {
    let conn = Connection::open(path).expect("could not open the file");
    conn.pragma_update(None, "key", support::TEST_PASSPHRASE).unwrap();
    db::cipher::apply_cipher_settings(&conn, "main").unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.pragma_update(None, "secure_delete", "ON").unwrap();
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE").unwrap();
    conn.busy_timeout(std::time::Duration::ZERO).unwrap();
    conn
}

/// The first read of an open: it needs the file's shared lock and the key.
fn first_read(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT count(*) FROM sqlite_schema", [], |row| row.get(0))
}

/// Calls `each` with every chunk of `conn`'s main database file, in order,
/// read through SQLite's own file handle (never `std::fs`, which would drop
/// the process's POSIX lock on the file when its descriptor closed).
fn for_each_chunk(conn: &Connection, mut each: impl FnMut(&[u8])) {
    const CHUNK: u64 = 64 * 1024;
    let file = RawFile::of(conn).expect("the database file is not open");
    let size = file.size().unwrap();
    let mut buf = vec![0u8; CHUNK as usize];
    let mut offset = 0;
    while offset < size {
        let len = CHUNK.min(size - offset) as usize;
        file.read_exact_at(&mut buf[..len], offset).unwrap();
        each(&buf[..len]);
        offset += len as u64;
    }
}

/// SHA-256 of the whole file, as lowercase hex.
fn file_hash(conn: &Connection) -> String {
    let mut hasher = Sha256::new();
    for_each_chunk(conn, |chunk| hasher.update(chunk));
    format!("{:x}", hasher.finalize())
}

fn is_locked(err: &rusqlite::Error) -> bool {
    matches!(err.sqlite_error_code(), Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked))
        && err.to_string().contains("database is locked")
}

/// Not a test of its own: in the child process it reads the database named by
/// [`CHILD_ENV`] as another process (another copy of HoploDex) would, and
/// prints what happened. SQLite shares lock state between the connections of
/// one process, so only another process can tell whether the file's lock is
/// really held at the operating system's level.
#[test]
fn child_reader() {
    let Some(path) = std::env::var_os(CHILD_ENV) else { return };
    let conn = app_connection(Path::new(&path));
    let outcome = match first_read(&conn) {
        Ok(_) => "readable".to_owned(),
        Err(err) if is_locked(&err) => "locked".to_owned(),
        Err(other) => format!("{other:?}"),
    };
    println!("OUTCOME={outcome}");
}

/// What another process gets when it reads `path` now.
fn read_from_another_process(path: &Path) -> String {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "upgrade_steps_test::child_reader", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, path)
        .output()
        .expect("could not start the child process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| line.split_once("OUTCOME=").map(|(_, outcome)| outcome))
        .unwrap_or_else(|| panic!("the child reported nothing: {stdout}"))
        .to_owned()
}

/// 009 research.md §4 step 2, probed (009 tasks.md T003): `BEGIN EXCLUSIVE;
/// COMMIT` with nothing between them changes no byte of the file, and under
/// `locking_mode = EXCLUSIVE` the connection keeps the file's lock afterwards,
/// so a second copy of HoploDex is told "in use" while the upgrade question
/// waits.
#[test]
fn begin_exclusive_commit_keeps_the_lock_and_changes_no_byte() {
    let (conn, dir) = TestDb::new().into_parts();
    drop(conn);
    let path = dir.path().join(TestDb::FILE_NAME);

    let probe = app_connection(&path);
    first_read(&probe).expect("the probe's first read");
    let before = file_hash(&probe);

    probe.execute_batch("BEGIN EXCLUSIVE; COMMIT").unwrap();

    // (a) Not a byte changed, read through the probe's own handle.
    assert_eq!(file_hash(&probe), before, "BEGIN EXCLUSIVE; COMMIT changed the file");

    // (b) Another connection's first keyed read is refused while it is held.
    let third = app_connection(&path);
    let refused = first_read(&third).expect_err("the lock was not kept after the COMMIT");
    assert!(is_locked(&refused), "expected \"database is locked\", got: {refused}");
    // Another process too: SQLite shares lock state within a process, so
    // this is what shows the operating system's lock is held.
    assert_eq!(read_from_another_process(&path), "locked");

    // Until the probing connection is dropped, and then it succeeds.
    drop(probe);
    first_read(&third).expect("the third connection could not read after the probe was dropped");
    drop(third);
    assert_eq!(read_from_another_process(&path), "readable");

    // And with nothing holding the file, its bytes on disk are the same.
    let on_disk = fs::read(&path).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&on_disk)), before);
}

/// One `sqlite_schema` row: type, name and SQL.
type SchemaRow = (String, String, Option<String>);

/// Everything a table rebuild changes, in a form to compare: the layout and
/// the rows of `x`.
#[derive(Debug, PartialEq)]
struct Snapshot {
    layout: Vec<SchemaRow>,
    rows: Vec<(i64, String, String)>,
}

fn snapshot(conn: &Connection) -> Snapshot {
    let layout = conn
        .prepare("SELECT type, name, sql FROM sqlite_schema ORDER BY type, name")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let rows = conn
        .prepare("SELECT id, label, payload FROM x ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    Snapshot { layout, rows }
}

/// 009 research.md §7, probed (009 tasks.md T004): a rebuild of a 5,000-row
/// table interrupted mid-transaction, with `cache_size = 1` so changed pages
/// have spilled into the file. A copy of the file (through `RawFile`) and its
/// `-journal` opens, with its passphrase, rolled back to the original rows and
/// layout, and `integrity_check` says `ok`. This is why an in-place upgrade is
/// atomic across power loss.
#[test]
fn an_interrupted_rebuild_rolls_back_from_its_hot_journal() {
    let (conn, dir) = TestDb::new().into_parts();
    conn.execute_batch(
        "CREATE TABLE x (id INTEGER PRIMARY KEY, label TEXT NOT NULL, payload TEXT NOT NULL);
         WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 5000)
         INSERT INTO x (id, label, payload)
             SELECT i, 'row ' || i, printf('%0200d', i) FROM n;
         CREATE INDEX x_label ON x (label);",
    )
    .unwrap();
    let original = snapshot(&conn);
    assert_eq!(original.rows.len(), 5000);
    let original_hash = file_hash(&conn);

    // The rebuild in 009 research.md §7's order, never committed.
    conn.pragma_update(None, "cache_size", 1).unwrap();
    conn.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE new_x (id INTEGER PRIMARY KEY, label TEXT NOT NULL, payload TEXT NOT NULL,
                             extra TEXT NOT NULL DEFAULT 'filled');
         INSERT INTO new_x (id, label, payload) SELECT id, label, payload FROM x;
         DROP TABLE x;
         ALTER TABLE new_x RENAME TO x;
         CREATE INDEX x_label ON x (label);",
    )
    .unwrap();

    // Copy the database through SQLite's own handle and its journal, while the
    // transaction is open and nothing else is dropped.
    let copy_dir = TempDir::new().unwrap();
    let copy = copy_dir.path().join(TestDb::FILE_NAME);
    let mut out = File::create(&copy).unwrap();
    for_each_chunk(&conn, |chunk| out.write_all(chunk).unwrap());
    out.sync_all().unwrap();
    drop(out);
    let journal = dir.path().join(format!("{}-journal", TestDb::FILE_NAME));
    let copied_journal = copy_dir.path().join(format!("{}-journal", TestDb::FILE_NAME));
    fs::copy(&journal, &copied_journal).expect("there is no journal beside the open transaction");

    // The probe is only meaningful if changed pages reached the file.
    let copied_hash = format!("{:x}", Sha256::digest(fs::read(&copy).unwrap()));
    assert_ne!(copied_hash, original_hash, "no changed page had spilled into the file");
    assert!(fs::metadata(&copied_journal).unwrap().len() > 0, "the journal is empty");

    // Open the copy with its passphrase: the first read rolls the hot journal back.
    let reopened = app_connection(&copy);
    first_read(&reopened).expect("the copy did not open");
    assert_eq!(snapshot(&reopened), original, "the copy was not rolled back");
    let integrity: String =
        reopened.query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
    assert_eq!(integrity, "ok");

    // Under `locking_mode = EXCLUSIVE` SQLite may leave the journal file in
    // place with its header zeroed rather than delete it, so the file's
    // presence says nothing; what matters is that it is no longer hot. Opening
    // the copy again finds the same, rolled-back, rows.
    drop(reopened);
    let again = app_connection(&copy);
    first_read(&again).unwrap();
    assert_eq!(snapshot(&again), original);
    drop(again);
    conn.execute_batch("ROLLBACK").unwrap();
}

// ---------------------------------------------------------------------------
// The runner (009 research.md §7, §14; 009 tasks.md T008). Every test uses
// test-only steps registered for its own temporary folder with
// `testing::scope`, on a database made before the scope was taken (so it is
// older than the build's steps) and then upgraded.
// ---------------------------------------------------------------------------

const STEP_A: (&str, &str) =
    ("9001_test_a", "CREATE TABLE t_a (id INTEGER PRIMARY KEY, note TEXT);");
const STEP_B: (&str, &str) =
    ("9002_test_b", "CREATE TABLE t_b (id INTEGER PRIMARY KEY); INSERT INTO t_b (id) VALUES (1);");
/// Valid SQL up to its last statement, which names a table that is not there.
const STEP_INVALID: (&str, &str) = (
    "9003_test_invalid",
    "CREATE TABLE t_invalid (id INTEGER PRIMARY KEY); INSERT INTO no_such_table VALUES (1);",
);
/// Leaves a child row whose parent is missing; the foreign keys are off while
/// a step runs, so only the runner's link check can see it.
const STEP_ORPHAN: (&str, &str) = (
    "9004_test_orphan",
    "CREATE TABLE t_parent (id INTEGER PRIMARY KEY);
     CREATE TABLE t_child (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES t_parent (id));
     INSERT INTO t_child (id, parent_id) VALUES (1, 99);",
);
/// Updates collection rows, which fires the change-tracking triggers.
const STEP_TOUCH_POLICIES: (&str, &str) =
    ("9005_test_touch_policies", "UPDATE insurance_policies SET notes = 'touched by a step';");

/// What an upgrade must leave exactly as it was when it fails: the layout and
/// the `schema_migrations` rows.
#[derive(Debug, PartialEq)]
struct Layout {
    schema: Vec<SchemaRow>,
    applied: Vec<String>,
}

fn layout(conn: &Connection) -> Layout {
    let schema = conn
        .prepare("SELECT type, name, sql FROM sqlite_schema ORDER BY type, name")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let applied = conn
        .prepare("SELECT name FROM schema_migrations ORDER BY rowid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    Layout { schema, applied }
}

fn foreign_keys_are_on(conn: &Connection) -> bool {
    conn.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0)).unwrap() == 1
}

fn changes_waiting(conn: &Connection) -> i64 {
    conn.query_row("SELECT changes_waiting FROM app_state", [], |row| row.get(0)).unwrap()
}

/// A closed database made by the build, with one policy in it, in a folder of
/// its own. Its `changes_waiting` is set as asked.
fn older_database(waiting: i64) -> (TempDir, std::path::PathBuf) {
    let (conn, dir) = TestDb::new().into_parts();
    insurance_ops::create_policy(
        &conn,
        &support::policy("Range rider", "2025-01-01", "2026-01-01", None),
    )
    .unwrap();
    conn.execute("UPDATE app_state SET changes_waiting = ?1", [waiting]).unwrap();
    drop(conn);
    let path = dir.path().join(TestDb::FILE_NAME);
    (dir, path)
}

/// Runs the runner over `path` with the build's steps (and any registered for
/// its folder), on a connection with the app's settings. The connection is
/// returned to look at, still holding the file.
fn run_upgrade(path: &Path) -> (Connection, Result<(), UpgradeError>) {
    let conn = app_connection(path);
    first_read(&conn).expect("the first read");
    let result = upgrade::upgrade(&conn, &upgrade::build_steps(path), None, &mut |_, _| {});
    (conn, result)
}

#[test]
fn the_steps_due_apply_in_one_transaction_with_a_row_each() {
    let (dir, path) = older_database(0);
    let scope = testing::scope(dir.path()).extra_steps(&[STEP_A, STEP_B]);
    let (conn, result) = run_upgrade(&path);
    result.expect("the upgrade");

    let applied = layout(&conn).applied;
    assert_eq!(applied.len(), 5, "{applied:?}");
    assert_eq!(&applied[3..], ["9001_test_a", "9002_test_b"]);
    let rows: i64 = conn.query_row("SELECT count(*) FROM t_b", [], |row| row.get(0)).unwrap();
    assert_eq!(rows, 1);
    assert!(foreign_keys_are_on(&conn), "foreign keys must be on again after success");
    // Nothing is due any more, and a second run changes nothing.
    assert!(upgrade::steps_due(&conn, &path).unwrap().is_empty());
    let before = layout(&conn);
    upgrade::upgrade(&conn, &upgrade::build_steps(&path), None, &mut |_, _| {}).unwrap();
    assert_eq!(layout(&conn), before);
    drop(scope);
}

#[test]
fn steps_due_names_the_build_steps_after_the_databases_own() {
    let (dir, path) = older_database(0);
    let _scope = testing::scope(dir.path()).extra_steps(&[STEP_A, STEP_B]);
    let conn = app_connection(&path);
    first_read(&conn).unwrap();
    assert_eq!(upgrade::steps_due(&conn, &path).unwrap(), ["9001_test_a", "9002_test_b"]);
}

/// 009 research.md §14: the `e2e` build's test-only step is behind its feature,
/// so a build without it (this one) gives a new database exactly `MIGRATIONS`'
/// steps, and no shipped build carries a step of its own.
#[test]
fn a_new_database_gets_exactly_the_migrations() {
    let names: Vec<&str> = db::MIGRATIONS.iter().map(|(name, _)| *name).collect();
    let new = TestDb::new();

    let built: Vec<&str> =
        upgrade::build_steps(&new.path()).into_iter().map(|(name, _)| name).collect();
    assert_eq!(built, names, "the steps this build applies");
    assert_eq!(layout(&new.conn).applied, names, "the steps a new database was given");
}

/// 009 research.md §7: the steps and their rows are one transaction, so a
/// failure in a later step takes the earlier ones with it (009 spec.md Edge Cases,
/// several releases skipped).
fn assert_nothing_applied(
    path: &Path,
    outcome: impl FnOnce(&Connection) -> Result<(), UpgradeError>,
) -> UpgradeError {
    let conn = app_connection(path);
    first_read(&conn).unwrap();
    let before = layout(&conn);
    let error = outcome(&conn).expect_err("the upgrade should have failed");
    assert_eq!(layout(&conn), before, "a failed upgrade left a step or a row behind");
    assert!(foreign_keys_are_on(&conn), "foreign keys must be on again after a failure");
    assert!(conn.is_autocommit(), "the failed upgrade left its transaction open");
    drop(conn);
    // And on disk, for the next open.
    let conn = app_connection(path);
    first_read(&conn).unwrap();
    assert_eq!(layout(&conn), before, "the file was changed by a failed upgrade");
    error
}

#[test]
fn a_failure_after_the_second_step_leaves_neither_step_nor_row() {
    let (dir, path) = older_database(0);
    let _scope = testing::scope(dir.path())
        .extra_steps(&[STEP_A, STEP_B])
        .fail_at(testing::Point::AfterStep(2));
    let error = assert_nothing_applied(&path, |conn| {
        upgrade::upgrade(conn, &upgrade::build_steps(&path), None, &mut |_, _| {})
    });
    assert!(matches!(error, UpgradeError::Injected(_)), "{error:?}");
}

#[test]
fn a_step_with_invalid_sql_leaves_neither_it_nor_the_steps_before_it() {
    let (dir, path) = older_database(0);
    let _scope = testing::scope(dir.path()).extra_steps(&[STEP_A, STEP_INVALID]);
    let error = assert_nothing_applied(&path, |conn| {
        upgrade::upgrade(conn, &upgrade::build_steps(&path), None, &mut |_, _| {})
    });
    // The error names the step, and carries no row value.
    assert!(
        matches!(&error, UpgradeError::Step { name, .. } if name == "9003_test_invalid"),
        "{error:?}"
    );
    assert!(error.to_string().contains("9003_test_invalid"), "{error}");
}

#[test]
fn an_orphan_row_fails_at_the_link_check_and_is_rolled_back() {
    let (dir, path) = older_database(0);
    let _scope = testing::scope(dir.path()).extra_steps(&[STEP_A, STEP_ORPHAN]);
    let error = assert_nothing_applied(&path, |conn| {
        upgrade::upgrade(conn, &upgrade::build_steps(&path), None, &mut |_, _| {})
    });
    assert!(matches!(error, UpgradeError::LinkCheck { .. }), "{error:?}");
    assert!(!error.to_string().contains("99"), "the error must not carry row values: {error}");
}

#[test]
fn a_failure_at_the_link_check_or_before_the_commit_rolls_back_too() {
    for point in [testing::Point::AtLinkCheck, testing::Point::BeforeCommit] {
        let (dir, path) = older_database(0);
        let _scope = testing::scope(dir.path()).extra_steps(&[STEP_A, STEP_B]).fail_at(point);
        let error = assert_nothing_applied(&path, |conn| {
            upgrade::upgrade(conn, &upgrade::build_steps(&path), None, &mut |_, _| {})
        });
        assert!(matches!(error, UpgradeError::Injected(_)), "{point:?}: {error:?}");
    }
}

#[test]
fn changes_waiting_is_the_same_after_a_step_that_updates_collection_rows() {
    for waiting in [0, 1] {
        let (dir, path) = older_database(waiting);
        let _scope = testing::scope(dir.path()).extra_steps(&[STEP_TOUCH_POLICIES]);
        let (conn, result) = run_upgrade(&path);
        result.unwrap();
        let notes: String =
            conn.query_row("SELECT notes FROM insurance_policies", [], |row| row.get(0)).unwrap();
        assert_eq!(notes, "touched by a step", "the step did not run");
        assert_eq!(changes_waiting(&conn), waiting);
    }
}

#[test]
fn the_upgrade_does_not_record_a_saved_version() {
    let (dir, path) = older_database(0);
    {
        let conn = app_connection(&path);
        first_read(&conn).unwrap();
        conn.execute("UPDATE app_state SET last_saved_version = '0.0.1'", []).unwrap();
    }
    let _scope = testing::scope(dir.path()).extra_steps(&[STEP_A]);
    let (conn, result) = run_upgrade(&path);
    result.unwrap();
    let saved: String =
        conn.query_row("SELECT last_saved_version FROM app_state", [], |row| row.get(0)).unwrap();
    assert_eq!(saved, "0.0.1");
}

#[test]
fn the_open_marker_is_written_in_the_same_transaction_when_one_is_given() {
    let (dir, path) = older_database(0);
    let _scope =
        testing::scope(dir.path()).extra_steps(&[STEP_A]).fail_at(testing::Point::BeforeCommit);
    let conn = app_connection(&path);
    first_read(&conn).unwrap();
    let marker = upgrade::OpenMarker::now(&support::other_machine());
    upgrade::upgrade(&conn, &upgrade::build_steps(&path), Some(&marker), &mut |_, _| {})
        .unwrap_err();
    let holder: Option<String> =
        conn.query_row("SELECT open_machine_id FROM app_state", [], |row| row.get(0)).unwrap();
    assert_eq!(holder.as_deref(), Some(support::test_machine().id.as_str()), "the marker leaked");
}

#[test]
fn a_hook_runs_at_its_point_with_the_connection_inside_the_transaction() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let (dir, path) = older_database(0);
    let seen = Arc::new(AtomicUsize::new(0));
    let in_transaction = Arc::new(AtomicUsize::new(0));
    let (seen_by_hook, in_transaction_by_hook) = (seen.clone(), in_transaction.clone());
    let _scope = testing::scope(dir.path()).extra_steps(&[STEP_A, STEP_B]).at_point(
        testing::Point::AfterStep(1),
        move |conn| {
            seen_by_hook.fetch_add(1, Ordering::SeqCst);
            // Step 1 is in, step 2 is not, and the transaction is still open.
            let t_a: i64 = conn
                .query_row("SELECT count(*) FROM sqlite_schema WHERE name = 't_a'", [], |r| {
                    r.get(0)
                })
                .unwrap();
            let t_b: i64 = conn
                .query_row("SELECT count(*) FROM sqlite_schema WHERE name = 't_b'", [], |r| {
                    r.get(0)
                })
                .unwrap();
            if t_a == 1 && t_b == 0 && !conn.is_autocommit() {
                in_transaction_by_hook.fetch_add(1, Ordering::SeqCst);
            }
        },
    );
    run_upgrade(&path).1.unwrap();
    assert_eq!(seen.load(Ordering::SeqCst), 1);
    assert_eq!(in_transaction.load(Ordering::SeqCst), 1);
}

/// The scope's isolation (009 research.md §14): a database in another folder
/// gets no extra step, whether it is made or opened while the scope is held,
/// and the steps are gone when the scope is dropped.
#[test]
fn a_scope_reaches_only_databases_in_its_folder() {
    let (dir, path) = older_database(0);
    let other = TempDir::new().unwrap();
    let other_path = other.path().join(TestDb::FILE_NAME);

    let scope = testing::scope(dir.path()).extra_steps(&[STEP_A, STEP_B]);
    let conn =
        db::create_database(&other_path, &support::passphrase(), &support::test_machine()).unwrap();
    let names = layout(&conn).applied;
    assert_eq!(names.len(), 3, "an extra step reached a database in another folder: {names:?}");
    assert!(upgrade::steps_due(&conn, &other_path).unwrap().is_empty());
    drop(conn);
    let reopened =
        db::open_database(&other_path, &support::passphrase(), &support::test_machine(), false)
            .unwrap()
            .expect_ready();
    assert_eq!(layout(&reopened).applied.len(), 3);
    drop(reopened);

    // The folder it names is asked about them on open (009 T046): the open
    // changes nothing, and the steps apply when the caller upgrades.
    let opened =
        db::open_database(&path, &support::passphrase(), &support::test_machine(), false).unwrap();
    let Opened::UpgradeDue { conn, steps_due, .. } = opened else {
        panic!("an open with steps due must not apply them");
    };
    assert_eq!(steps_due, ["9001_test_a", "9002_test_b"]);
    assert_eq!(layout(&conn).applied.len(), 3);
    upgrade::upgrade(&conn, &upgrade::build_steps(&path), None, &mut |_, _| {}).unwrap();
    assert_eq!(layout(&conn).applied.len(), 5);
    drop(conn);

    drop(scope);
    assert_eq!(upgrade::build_steps(&path).len(), 3, "the scope's steps outlived it");
}

#[test]
fn a_database_with_a_step_this_build_lacks_is_newer_and_refused() {
    let (dir, path) = older_database(0);
    {
        let _scope = testing::scope(dir.path()).extra_steps(&[STEP_A]);
        run_upgrade(&path).1.unwrap();
    }
    // The scope is gone, so the database holds a step the build lacks.
    let result = db::open_database(&path, &support::passphrase(), &support::test_machine(), false);
    assert!(matches!(result, Err(OpenError::NewerVersion)), "{result:?}");
}

/// 009 research.md §4 steps 1 and 2: an open with steps due writes nothing,
/// not even the open marker, and still holds the file's lock; nothing else
/// asks the question twice.
#[test]
fn an_open_with_steps_due_writes_nothing_and_holds_the_lock() {
    let (dir, path) = older_database(0);
    let hash_before = {
        let conn = app_connection(&path);
        first_read(&conn).unwrap();
        file_hash(&conn)
    };
    let _scope = testing::scope(dir.path()).extra_steps(&[STEP_A]);
    let opened =
        db::open_database(&path, &support::passphrase(), &support::test_machine(), false).unwrap();
    let Opened::UpgradeDue { conn, steps_due, marker_seen } = opened else {
        panic!("steps are due");
    };
    assert_eq!(steps_due, ["9001_test_a"]);
    // The test machine made it and left its marker.
    assert_eq!(marker_seen.machine_id.as_deref(), Some(support::test_machine().id.as_str()));
    assert_eq!(file_hash(&conn), hash_before, "the file changed");
    let second = app_connection(&path);
    let err = first_read(&second).expect_err("a second connection must be refused");
    assert!(is_locked(&err), "{err}");
    drop(second);
    // Dropped unanswered, the file is as it was and opens again.
    drop(conn);
    let again =
        db::open_database(&path, &support::passphrase(), &support::test_machine(), false).unwrap();
    assert!(matches!(again, Opened::UpgradeDue { .. }));
}

/// The marker another computer left is refused before the upgrade question,
/// and the take-over resend reaches it with the marker it found.
#[test]
fn the_open_marker_comes_before_the_question() {
    let (dir, path) = older_database(0);
    drop(
        db::open_database(&path, &support::passphrase(), &support::other_machine(), true)
            .unwrap()
            .expect_ready(),
    );
    let _scope = testing::scope(dir.path()).extra_steps(&[STEP_A]);
    let refused = db::open_database(&path, &support::passphrase(), &support::test_machine(), false);
    assert!(matches!(refused, Err(OpenError::OpenElsewhere { .. })), "{refused:?}");
    let taken =
        db::open_database(&path, &support::passphrase(), &support::test_machine(), true).unwrap();
    let Opened::UpgradeDue { marker_seen, .. } = taken else { panic!("steps are due") };
    assert_eq!(marker_seen.machine_id.as_deref(), Some(support::other_machine().id.as_str()));
}

/// A failed upgrade after the question leaves the file as it was.
#[test]
fn an_upgrade_that_fails_after_the_question_leaves_the_file_as_it_was() {
    let (dir, path) = older_database(0);
    let before = {
        let conn = app_connection(&path);
        first_read(&conn).unwrap();
        layout(&conn)
    };
    let _scope = testing::scope(dir.path()).extra_steps(&[STEP_A, STEP_INVALID]);
    let opened =
        db::open_database(&path, &support::passphrase(), &support::test_machine(), false).unwrap();
    let Opened::UpgradeDue { conn, .. } = opened else { panic!("steps are due") };
    let marker = upgrade::OpenMarker::now(&support::test_machine());
    let failed =
        upgrade::upgrade(&conn, &upgrade::build_steps(&path), Some(&marker), &mut |_, _| {});
    assert!(failed.is_err());
    drop(conn);
    let conn = app_connection(&path);
    first_read(&conn).unwrap();
    assert_eq!(layout(&conn), before);
}
