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

use hoplodex_lib::db::{self, raw_file::RawFile};
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

    // The rebuild in research.md §7's order, never committed.
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
