//! A full disk (issue #110): opening, creating, saving and closing each say
//! the disk is full and what to do, never "An unexpected error occurred.",
//! and leave no partial or damaged file behind.
//!
//! Two levels. `max_page_count` makes SQLite itself return `SQLITE_FULL`,
//! which needs no special system, so it runs everywhere. The paths that also
//! meet the real file system (open, create, the close's backup) run on a
//! small tmpfs that the test mounts for itself in a private user and mount
//! namespace (`unshare --map-root-user --mount`; Linux only), where the disk
//! really fills: the test starts its own binary again inside the namespace,
//! and that run mounts the tmpfs and does the work. A computer that doesn't
//! allow unprivileged namespaces says so and skips those; the dev container
//! does allow them.

use crate::support;

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::db::{self, DbError};
use hoplodex_lib::models::database::{BackupFailureReason, BackupOutcome, CloseReason};
use hoplodex_lib::services::backups::{BackupFailure, LocationProblem};
use hoplodex_lib::services::disk_space;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::session::{Session, lifecycle};
use serde_json::Value;
use support::{TestEvents, passphrase, test_session};
use tempfile::TempDir;

/// The OS's disk-full codes: `ENOSPC` on Linux and macOS,
/// `ERROR_HANDLE_DISK_FULL` and `ERROR_DISK_FULL` on Windows.
#[cfg(unix)]
const DISK_FULL_CODES: [i32; 1] = [28];
#[cfg(windows)]
const DISK_FULL_CODES: [i32; 2] = [39, 112];

const DISK_FULL: &str = "DISK_FULL";

fn assert_disk_full(err: &CommandError) {
    assert_eq!(err.code, DISK_FULL, "{err}");
    assert!(err.message.contains("The disk is full."), "{err}");
    assert!(err.message.contains("Free some space"), "{err}");
    assert!(err.message.contains("try again"), "{err}");
}

// ----------------------------------------------------------- the error maps

#[test]
fn an_io_error_for_a_full_disk_is_disk_full() {
    for code in DISK_FULL_CODES {
        let err = io::Error::from_raw_os_error(code);
        assert!(disk_space::is_disk_full(&err), "{code}");
        let mapped = CommandError::from_io(&err, || panic!("a full disk is not another error"));
        assert_disk_full(&mapped);
        assert_disk_full(&CommandError::from(DbError::Io(io::Error::from_raw_os_error(code))));
    }
    let other = io::Error::from(io::ErrorKind::PermissionDenied);
    assert!(!disk_space::is_disk_full(&other));
    let mapped = CommandError::from_io(&other, || CommandError::new("OTHER", "other"));
    assert_eq!(mapped.code, "OTHER");
    assert_eq!(CommandError::from(DbError::Io(other)).code, "INTERNAL_ERROR");
}

#[test]
fn a_disk_full_names_the_file_it_was_about() {
    let path = Path::new("/somewhere/Mine.hoplodex");
    let located = CommandError::disk_full(None).located_at(path);
    assert_disk_full(&located);
    assert!(located.message.contains("Mine.hoplodex"), "{located}");
    assert_eq!(
        located.details.as_deref().unwrap()["path"],
        Value::from(path.to_string_lossy().into_owned())
    );
    // Any other error is left as it is.
    let other = CommandError::new("OTHER", "other").located_at(path);
    assert_eq!((other.code.as_str(), other.message.as_str()), ("OTHER", "other"));
}

#[test]
fn a_backup_that_ran_out_of_room_is_reported_as_insufficient_space() {
    for code in DISK_FULL_CODES {
        let full = BackupFailure::Io(io::Error::from_raw_os_error(code));
        assert_eq!(full.reason(), Some(BackupFailureReason::InsufficientSpace));
    }
    let other = BackupFailure::Io(io::Error::from(io::ErrorKind::PermissionDenied));
    assert_eq!(other.reason(), Some(BackupFailureReason::Io));
    let location = BackupFailure::Location(LocationProblem::InsufficientSpace);
    assert_eq!(location.reason(), Some(BackupFailureReason::InsufficientSpace));
    assert_eq!(LocationProblem::InsufficientSpace.as_reason(), "insufficientSpace");
}

// ------------------------------------------------- SQLite's own "full"

/// A database held open by a session, whose file may grow by no more pages
/// than SQLite is told it may (`PRAGMA max_page_count`).
struct Limited {
    session: Session,
    _events: Arc<TestEvents>,
    _machine: MachineSettings,
    _dir: TempDir,
    _config: TempDir,
}

impl Limited {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let (session, events) = test_session_at_config(config.path());
        let machine = MachineSettings::load(config.path()).unwrap();
        let dir = TempDir::new().unwrap();
        lifecycle::create(&session, &machine, &dir.path().join("Mine.hoplodex"), &passphrase())
            .unwrap();
        Self { session, _events: events, _machine: machine, _dir: dir, _config: config }
    }

    fn id(&self) -> hoplodex_lib::session::SessionId {
        self.session.current_id()
    }

    /// From now on the file may not grow past the pages it has.
    fn stop_growing(&self) {
        self.session
            .write(self.id(), |conn| {
                let pages: i64 =
                    conn.query_row("PRAGMA page_count", [], |r| r.get(0)).map_err(from_db)?;
                conn.query_row(&format!("PRAGMA max_page_count = {pages}"), [], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(from_db)?;
                Ok(())
            })
            .unwrap();
    }

    fn allow_growing(&self) {
        self.session
            .write(self.id(), |conn| {
                conn.query_row("PRAGMA max_page_count = 1000000", [], |r| r.get::<_, i64>(0))
                    .map_err(from_db)?;
                Ok(())
            })
            .unwrap();
    }

    fn add_firearm(&self, serial: &str) -> Result<i64, CommandError> {
        self.session.write(self.id(), |conn| {
            firearms::create_firearm(conn, &support::firearm("Colt", "Python", serial), false, None)
                .map(|created| created.id)
        })
    }

    fn firearm_count(&self) -> i64 {
        self.session
            .read(self.id(), |conn| {
                conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0)).map_err(from_db)
            })
            .unwrap()
    }
}

fn from_db(err: rusqlite::Error) -> CommandError {
    CommandError::from_db(err)
}

fn test_session_at_config(config: &Path) -> (Session, Arc<TestEvents>) {
    test_session(&config.join("opened-documents"))
}

/// A save that the disk has no room for says so, keeps nothing of itself,
/// and leaves the session working for when there is room again.
#[test]
fn a_save_on_a_full_disk_says_the_disk_is_full_and_changes_nothing() {
    let world = Limited::new();
    world.add_firearm("A1").unwrap();
    world.stop_growing();

    let mut saved = 1;
    let mut refused = None;
    for n in 0..500 {
        match world.add_firearm(&format!("S{n}")) {
            Ok(_) => saved += 1,
            Err(err) => {
                refused = Some(err);
                break;
            }
        }
    }
    let refused = refused.expect("the file can't grow, so a save must fail in the end");
    assert_disk_full(&refused);
    assert!(refused.message.contains("Mine.hoplodex"), "{refused}");
    assert_eq!(world.firearm_count(), saved, "the failed save left a partial record");

    // The session was not closed or marked unreachable by it.
    world.allow_growing();
    world.add_firearm("AFTER").unwrap();
    assert_eq!(world.firearm_count(), saved + 1);
}

/// `SQLITE_FULL` from any statement is `DISK_FULL`; another SQL error is
/// still an unexpected one.
#[test]
fn sqlite_full_is_disk_full() {
    let db = support::TestDb::new();
    db.conn.execute_batch("CREATE TABLE filler (content TEXT)").unwrap();
    let pages: i64 = db.conn.query_row("PRAGMA page_count", [], |r| r.get(0)).unwrap();
    db.conn
        .query_row(&format!("PRAGMA max_page_count = {pages}"), [], |r| r.get::<_, i64>(0))
        .unwrap();
    let full = (0..500)
        .find_map(|_| {
            db.conn
                .execute(
                    "INSERT INTO filler (content) VALUES (?1)",
                    [db::random_hex(2000).unwrap()],
                )
                .err()
        })
        .expect("a file that can't grow fills up");
    assert_disk_full(&CommandError::from_db(full));

    let unrelated = db.conn.execute("INSERT INTO no_such_table VALUES (1)", []).unwrap_err();
    assert_eq!(CommandError::from_db(unrelated).code, "INTERNAL_ERROR");
}

// ------------------------------------------------- a really full file system

/// The tmpfs's mount point, which the re-run inside the namespace gets.
#[cfg(target_os = "linux")]
const MOUNT_VARIABLE: &str = "HOPLODEX_TEST_FULL_DISK_MOUNT";

/// Runs `body` with a folder on a tmpfs of `size_kib` KiB. Outside the
/// namespace this starts the test again inside one (the test harness has
/// threads, and a thread can't make a user namespace); inside, it mounts
/// the tmpfs and runs the body.
#[cfg(target_os = "linux")]
fn on_small_disk(test: &str, size_kib: u64, body: impl FnOnce(&Path)) {
    use std::process::Command;

    if let Ok(mount) = std::env::var(MOUNT_VARIABLE) {
        mount_tmpfs(Path::new(&mount), size_kib);
        body(Path::new(&mount));
        return;
    }
    let namespaces_work = Command::new("unshare")
        .args(["--map-root-user", "--mount", "true"])
        .status()
        .is_ok_and(|status| status.success());
    if !namespaces_work {
        eprintln!("SKIPPED {test}: this computer doesn't allow `unshare --map-root-user --mount`");
        return;
    }
    let mount = TempDir::new().unwrap();
    let output = Command::new("unshare")
        .args(["--map-root-user", "--mount", "--"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(MOUNT_VARIABLE, mount.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{test} failed on the small disk:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(not(target_os = "linux"))]
fn on_small_disk(test: &str, _size_kib: u64, _body: impl FnOnce(&Path)) {
    eprintln!("SKIPPED {test}: a tmpfs can only be made on Linux");
}

#[cfg(target_os = "linux")]
fn mount_tmpfs(at: &Path, size_kib: u64) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let target = CString::new(at.as_os_str().as_bytes()).unwrap();
    let options = CString::new(format!("size={size_kib}k,mode=0755")).unwrap();
    let rc = unsafe {
        libc::mount(
            c"tmpfs".as_ptr(),
            target.as_ptr(),
            c"tmpfs".as_ptr(),
            0,
            options.as_ptr().cast(),
        )
    };
    assert_eq!(rc, 0, "could not mount a tmpfs: {}", io::Error::last_os_error());
}

/// Writes until the disk has `leave` bytes free, in a file that `fill`
/// returns, so a test can free the space again.
struct Filler(PathBuf);

impl Filler {
    fn new(folder: &Path, leave: u64) -> Self {
        let path = folder.join("filler");
        let mut file = File::create(&path).unwrap();
        let chunk = vec![0u8; 4096];
        let mut written = 0u64;
        loop {
            match file.write_all(&chunk) {
                Ok(()) => written += chunk.len() as u64,
                Err(err) if disk_space::is_disk_full(&err) => break,
                Err(err) => panic!("{err}"),
            }
        }
        file.set_len(written.saturating_sub(leave)).unwrap();
        Self(path)
    }

    /// The disk has room again.
    fn free(self) {
        fs::remove_file(self.0).unwrap();
    }
}

/// What a folder holds, by name, for "nothing was left behind".
fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

struct Disk {
    folder: PathBuf,
    _config: TempDir,
    machine: MachineSettings,
    session: Session,
    events: Arc<TestEvents>,
}

impl Disk {
    fn new(folder: &Path) -> Self {
        let config = TempDir::new().unwrap();
        let (session, events) = test_session_at_config(config.path());
        Self {
            folder: folder.to_owned(),
            machine: MachineSettings::load(config.path()).unwrap(),
            _config: config,
            session,
            events,
        }
    }

    fn path(&self) -> PathBuf {
        self.folder.join("Mine.hoplodex")
    }

    fn create(&self) -> Result<(), CommandError> {
        lifecycle::create(&self.session, &self.machine, &self.path(), &passphrase())
    }

    fn open(&self) -> Result<(), CommandError> {
        lifecycle::open(&self.session, &self.machine, &self.path(), &passphrase(), false)
    }

    fn close(&self) -> hoplodex_lib::models::database::CloseOutcome {
        lifecycle::close_normal(
            &self.session,
            self.session.current_id(),
            &self.machine,
            CloseReason::Closed,
        )
        .unwrap()
    }

    fn add_firearm(&self, serial: &str) -> Result<i64, CommandError> {
        self.session.write(self.session.current_id(), |conn| {
            firearms::create_firearm(conn, &support::firearm("Colt", "Python", serial), false, None)
                .map(|created| created.id)
        })
    }

    fn notices(&self) -> Vec<Value> {
        self.events.payloads("notice")
    }
}

const SMALL_DISK_KIB: u64 = 6 * 1024;

/// Creating a database where the disk fills part way removes what it made.
#[test]
fn creating_on_a_full_disk_says_so_and_leaves_no_file() {
    on_small_disk(
        "disk_full_test::creating_on_a_full_disk_says_so_and_leaves_no_file",
        SMALL_DISK_KIB,
        |folder| {
            let disk = Disk::new(folder);
            // Room for a part of the new file, not all of it.
            let filler = Filler::new(folder, 64 * 1024);

            let err = disk.create().expect_err("there is no room for a database");

            assert_disk_full(&err);
            assert!(err.message.contains("Mine.hoplodex"), "{err}");
            assert_eq!(names(folder), ["filler"], "the failed create left something behind");
            assert!(!disk.session.is_open());
            filler.free();
            // And it can be made once there is room.
            disk.create().unwrap();
            assert!(disk.path().is_file());
        },
    );
}

/// Opening where the open's own writes (its journal, the open marker) have
/// no room leaves the file exactly as it was.
#[test]
fn opening_on_a_full_disk_says_so_and_leaves_the_file_as_it_was() {
    on_small_disk(
        "disk_full_test::opening_on_a_full_disk_says_so_and_leaves_the_file_as_it_was",
        SMALL_DISK_KIB,
        |folder| {
            let disk = Disk::new(folder);
            disk.create().unwrap();
            disk.close();
            let before = fs::read(disk.path()).unwrap();
            let held = names(folder);
            let filler = Filler::new(folder, 0);

            let err = disk.open().expect_err("there is no room to open");

            assert_disk_full(&err);
            assert!(!disk.session.is_open());
            assert_eq!(fs::read(disk.path()).unwrap(), before, "the failed open changed the file");
            let mut expected = held.clone();
            expected.push("filler".into());
            expected.sort();
            assert_eq!(names(folder), expected, "the failed open left a file beside the database");
            filler.free();
            disk.open().unwrap();
            assert!(disk.session.is_open());
        },
    );
}

/// A save where the file system fills says so, and the session carries on.
#[test]
fn saving_on_a_full_disk_says_so_and_the_session_carries_on() {
    on_small_disk(
        "disk_full_test::saving_on_a_full_disk_says_so_and_the_session_carries_on",
        SMALL_DISK_KIB,
        |folder| {
            let disk = Disk::new(folder);
            disk.create().unwrap();
            disk.add_firearm("FIRST").unwrap();
            let filler = Filler::new(folder, 0);

            // The first saves may fit in pages the file already has.
            let mut saved = 1;
            let err = (0..500)
                .find_map(|n| match disk.add_firearm(&format!("S{n}")) {
                    Ok(_) => {
                        saved += 1;
                        None
                    }
                    Err(err) => Some(err),
                })
                .expect("a full disk refuses a save in the end");

            assert_disk_full(&err);
            filler.free();
            disk.add_firearm("AFTER").unwrap();
            saved += 1;
            let count: i64 = disk
                .session
                .read(disk.session.current_id(), |conn| {
                    conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0))
                        .map_err(from_db)
                })
                .unwrap();
            assert_eq!(count, saved);
        },
    );
}

/// The close's backup, which finds room when it measures and then runs out
/// of it (other files fill the disk meanwhile), is reported as a lack of
/// room, with nothing partial left in the backup folder.
#[test]
fn closing_on_a_full_disk_says_the_backup_has_no_room() {
    on_small_disk(
        "disk_full_test::closing_on_a_full_disk_says_the_backup_has_no_room",
        SMALL_DISK_KIB,
        |folder| {
            let disk = Disk::new(folder);
            disk.create().unwrap();
            disk.add_firearm("FIRST").unwrap();
            // Room for a part of the copy, though the check was told there was
            // plenty.
            let filler = Filler::new(folder, 64 * 1024);
            let _measured = disk_space::testing::fake_available_space(|_| Some(u64::MAX));

            let outcome = disk.close();

            assert_eq!(outcome.backup, BackupOutcome::Failed);
            assert_eq!(outcome.failure_reason, Some(BackupFailureReason::InsufficientSpace));
            let notices = disk.notices();
            assert!(
                notices
                    .iter()
                    .any(|n| n["kind"] == "backupFailed" && n["reason"] == "insufficientSpace"),
                "{notices:?}"
            );
            // No partial backup is left in the backup folder.
            let backup_folder = folder.join("HoploDex backups");
            let left: Vec<String> =
                if backup_folder.is_dir() { names(&backup_folder) } else { vec![] };
            assert!(left.is_empty(), "the failed backup left {left:?}");
            // The database itself is intact: it opens with the room back.
            filler.free();
            disk.open().unwrap();
        },
    );
}

// ------------------------------------------------- documents and exports

/// The native dialog, always answered "open".
struct AlwaysOpen;

impl hoplodex_lib::services::consent::Consent for AlwaysOpen {
    fn ask(
        &self,
        _request: hoplodex_lib::services::consent::ConsentRequest,
    ) -> hoplodex_lib::services::consent::ConsentAnswer {
        hoplodex_lib::services::consent::ConsentAnswer::Open
    }
}

/// An opener that must not be reached: the copy it would open isn't there.
struct NeverReached;

impl hoplodex_lib::commands::documents::Opener for NeverReached {
    fn open(&self, _path: &Path) -> Result<(), hoplodex_lib::commands::documents::OpenFailure> {
        panic!("the copy could not be written, so nothing is opened");
    }
}

/// The decrypted copy that opening a document in another app writes, on a
/// disk that fills while it is written: `DISK_FULL` naming the copy, and no
/// part of the copy left.
#[test]
fn opening_a_document_on_a_full_disk_says_so_and_leaves_no_partial_copy() {
    on_small_disk(
        "disk_full_test::opening_a_document_on_a_full_disk_says_so_and_leaves_no_partial_copy",
        SMALL_DISK_KIB,
        |folder| {
            let disk = Disk::new(folder);
            disk.create().unwrap();
            let firearm = crate::preview_support::new_firearm(&disk.session);
            let document = crate::preview_support::add_document(
                &disk.session,
                firearm,
                "receipt.pdf",
                &support::document_fixture("three-pages.pdf"),
            );
            let copies = folder.join("opened-documents");
            let filler = Filler::new(folder, 0);

            let err = hoplodex_lib::commands::documents::ops::open_document(
                &disk.session,
                disk.session.current_id(),
                &disk.machine,
                &AlwaysOpen,
                &NeverReached,
                &copies,
                document,
            )
            .expect_err("there is no room for the copy");

            assert_disk_full(&err);
            assert!(err.message.contains("opened-documents"), "{err}");
            let left: Vec<PathBuf> = walk(&copies).into_iter().filter(|p| p.is_file()).collect();
            assert!(left.is_empty(), "a partial copy was left: {left:?}");
            filler.free();
        },
    );
}

/// Every path under `folder`.
fn walk(folder: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Ok(entries) = fs::read_dir(folder) {
        for entry in entries {
            let path = entry.unwrap().path();
            found.push(path.clone());
            if path.is_dir() {
                found.extend(walk(&path));
            }
        }
    }
    found
}

/// An export whose files fill the disk (the spreadsheet, either format)
/// says so, naming the folder, and removes what it made.
#[test]
fn exporting_on_a_full_disk_says_so_and_removes_what_it_made() {
    on_small_disk(
        "disk_full_test::exporting_on_a_full_disk_says_so_and_removes_what_it_made",
        SMALL_DISK_KIB,
        |folder| {
            use hoplodex_lib::commands::import_export::{ExportRecords, ops};
            use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;

            let disk = Disk::new(folder);
            disk.create().unwrap();
            let ids: Vec<i64> =
                (0..40).map(|n| disk.add_firearm(&format!("E{n}")).unwrap()).collect();
            let destination = folder.join("exports");
            fs::create_dir(&destination).unwrap();
            let filler = Filler::new(folder, 0);

            for format in [SpreadsheetFormat::Csv, SpreadsheetFormat::Xlsx] {
                let records = ExportRecords { firearm_ids: ids.clone(), accessory_ids: vec![] };
                let err = disk
                    .session
                    .read(disk.session.current_id(), |conn| {
                        ops::export_collection(
                            conn,
                            &destination,
                            "collection",
                            format,
                            &records,
                            &mut |_, _| {},
                        )
                        .map(|_| ())
                    })
                    .expect_err("there is no room for the export");

                assert_disk_full(&err);
                assert!(err.message.contains("exports"), "{err}");
                assert_eq!(names(&destination), Vec::<String>::new(), "{format:?} left files");
            }
            filler.free();
        },
    );
}
