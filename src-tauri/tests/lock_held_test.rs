//! An open database stays locked against other processes while HoploDex
//! reads its file (research.md §2, §3). On Linux and macOS, closing any file
//! descriptor on a file drops every POSIX lock the process holds on it,
//! SQLite's exclusive lock included, so nothing may read an open database
//! through a descriptor of its own. SQLite shares lock state between the
//! connections of one process, so only another process can tell: each test
//! re-runs this test binary as that process, through [`child_opener`].

mod support;

use std::path::Path;
use std::process::Command;

use hoplodex_lib::db::{self, OpenError};
use hoplodex_lib::services::backups::{self, BackupJob};
use hoplodex_lib::services::machine_settings::MachineSettings;
use support::{TestDb, other_machine, passphrase};
use tempfile::TempDir;

/// Set in the child process to the database it should try to open.
const CHILD_ENV: &str = "HOPLODEX_TEST_OPEN_FROM_CHILD";

/// Not a test of its own: in the child process it tries to open the
/// database named by [`CHILD_ENV`], as another copy of HoploDex on another
/// computer taking it over would, and prints what happened.
#[test]
fn child_opener() {
    let Some(path) = std::env::var_os(CHILD_ENV) else { return };
    let outcome = match db::open_database(Path::new(&path), &passphrase(), &other_machine(), true) {
        Ok(_) => "opened".to_owned(),
        Err(OpenError::InUse) => "in-use".to_owned(),
        Err(other) => format!("{other:?}"),
    };
    println!("OUTCOME={outcome}");
}

/// What another process gets when it opens `path` now.
fn open_from_another_process(path: &Path) -> String {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_opener", "--nocapture", "--test-threads=1"])
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

#[test]
fn another_process_finds_an_open_database_in_use() {
    let db = TestDb::new();

    assert_eq!(open_from_another_process(&db.path()), "in-use");
}

#[test]
fn checking_a_passphrase_keeps_the_lock() {
    let db = TestDb::new();
    let scratch = TempDir::new().unwrap();

    assert!(db::verify_passphrase(&db.conn, &passphrase(), scratch.path()).unwrap());

    assert_eq!(open_from_another_process(&db.path()), "in-use");
    // And the connection still writes.
    db.conn.execute("UPDATE collection_settings SET idle_lock_minutes = 11", []).unwrap();
}

#[test]
fn making_a_backup_keeps_the_lock() {
    let db = TestDb::new();
    let config = TempDir::new().unwrap();
    let machine = MachineSettings::load(config.path()).unwrap();
    let folder = db.dir().join("HoploDex backups");

    backups::make_backup(
        &machine,
        BackupJob {
            conn: &db.conn,
            database_path: &db.path(),
            name: "test",
            database_id: &"a".repeat(32),
            folder: &folder,
            make_folder: true,
            now: chrono::Local::now().fixed_offset(),
            cancel: &|| false,
            progress: &mut |_, _| {},
        },
    )
    .unwrap();

    assert_eq!(open_from_another_process(&db.path()), "in-use");
}
