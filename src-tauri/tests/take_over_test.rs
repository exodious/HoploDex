//! The open marker, taking a database over, and noticing a take-over or
//! lost storage while open (FR-032, SC-009; research.md §6; data-model.md
//! "TakenOver"). Real SQLCipher files in temp directories; machine settings
//! in a temp config directory.

mod support;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::CommandError;
use hoplodex_lib::db::{self, OpenError};
use hoplodex_lib::models::database::{BackupFailureReason, BackupOutcome, CloseReason};
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::session::{lifecycle, Session};
use rusqlite::Connection;
use serde_json::json;
use support::{other_machine, passphrase, test_machine, test_session, TestDb, TestEvents};
use tempfile::TempDir;

/// Marks the database behind `conn` as open on `other_machine()` since
/// `since`, as that computer would have left it.
fn mark_open_elsewhere(conn: &Connection, since: &str) {
    let other = other_machine();
    conn.execute(
        "UPDATE app_state SET open_machine_id = ?1, open_machine_name = ?2, open_since = ?3",
        rusqlite::params![other.id, other.display_name, since],
    )
    .unwrap();
}

fn marker_id(conn: &Connection) -> Option<String> {
    conn.query_row("SELECT open_machine_id FROM app_state", [], |r| r.get(0)).unwrap()
}

#[test]
fn a_database_marked_open_on_another_computer_is_refused_and_left_untouched() {
    let db = TestDb::new();
    mark_open_elsewhere(&db.conn, "2026-09-25T14:30:05Z");
    let path = db.path();
    let (conn, _dir) = db.into_parts();
    drop(conn);
    let before = fs::read(&path).unwrap();

    let result = db::open_database(&path, &passphrase(), &test_machine(), false);

    match &result {
        Err(OpenError::OpenElsewhere { machine_name, since }) => {
            assert_eq!(machine_name, "Workshop PC");
            assert_eq!(since, "2026-09-25T14:30:05Z");
        }
        other => panic!("expected OpenElsewhere, got {other:?}"),
    }
    let error = CommandError::from(result.unwrap_err());
    assert_eq!(error.code, "DATABASE_OPEN_ELSEWHERE");
    assert_eq!(
        error.details.as_deref(),
        Some(&json!({ "machineName": "Workshop PC", "since": "2026-09-25T14:30:05Z" }))
    );
    assert_eq!(fs::read(&path).unwrap(), before, "a refused open never writes the file");
}

#[test]
fn this_computers_own_stale_marker_opens_without_a_warning() {
    // Created here, so marked open here, then dropped without a close, as a
    // crash leaves it.
    let db = TestDb::new();
    let path = db.path();
    let (conn, _dir) = db.into_parts();
    assert_eq!(marker_id(&conn), Some(test_machine().id));
    drop(conn);

    let conn = db::open_database(&path, &passphrase(), &test_machine(), false).unwrap();

    assert_eq!(marker_id(&conn), Some(test_machine().id));
}

/// A throwaway world: a folder for databases, a config directory for
/// `machine.json`, and a session reporting to a recorder.
struct World {
    dir: TempDir,
    _config: TempDir,
    machine: MachineSettings,
    session: Session,
    events: Arc<TestEvents>,
}

impl World {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let (session, events) = test_session(&config.path().join("opened-documents"));
        Self {
            dir: TempDir::new().unwrap(),
            machine: MachineSettings::load(config.path()).unwrap(),
            _config: config,
            session,
            events,
        }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("Mine.hoplodex")
    }

    fn path_text(&self) -> String {
        self.path().to_string_lossy().into_owned()
    }

    /// Creates "Mine" and leaves it open.
    fn create(&self) {
        lifecycle::create(&self.session, &self.machine, &self.path(), &passphrase()).unwrap();
    }

    fn add_firearm(&self, serial: &str) -> Result<(), CommandError> {
        self.session
            .write(|conn| {
                firearms::create_firearm(conn, &support::firearm("Colt", "Python", serial), false)
            })
            .map(|_| ())
    }

    fn count_firearms(&self) -> Result<i64, CommandError> {
        self.session.read(|conn| {
            conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0))
                .map_err(CommandError::from_db)
        })
    }

    /// Another computer's copy of the file, as a sync client delivers it:
    /// renamed over the path. Returns a second name for the file this
    /// session had open, to see that nothing more is written to it.
    fn replace_from_another_computer(&self) -> PathBuf {
        let ours = self.dir.path().join("ours-link.hoplodex");
        fs::hard_link(self.path(), &ours).unwrap();
        let theirs = self.dir.path().join(".sync-incoming");
        fs::copy(self.path(), &theirs).unwrap();
        fs::rename(&theirs, self.path()).unwrap();
        ours
    }
}

#[test]
fn taking_over_opens_and_marks_the_database_as_open_here() {
    let world = World::new();
    {
        let conn = db::create_database(&world.path(), &passphrase(), &other_machine()).unwrap();
        mark_open_elsewhere(&conn, "2026-09-25T14:30:05Z");
    }

    let refused =
        lifecycle::open(&world.session, &world.machine, &world.path(), &passphrase(), false);
    assert_eq!(refused.unwrap_err().code, "DATABASE_OPEN_ELSEWHERE");
    assert!(!world.session.is_open());

    lifecycle::open(&world.session, &world.machine, &world.path(), &passphrase(), true).unwrap();

    let marker = world
        .session
        .read(|conn| {
            conn.query_row("SELECT open_machine_id, open_machine_name FROM app_state", [], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(CommandError::from_db)
        })
        .unwrap();
    assert_eq!(marker, (world.machine.machine_id(), world.machine.identity().display_name));
}

#[test]
fn our_own_writes_never_look_like_a_take_over() {
    let world = World::new();
    world.create();

    for serial in ["A1", "A2", "A3"] {
        world.add_firearm(serial).unwrap();
    }
    world
        .session
        .write(|conn| {
            conn.execute("UPDATE collection_settings SET idle_lock_minutes = 30", [])
                .map_err(CommandError::from_db)
        })
        .unwrap();

    assert_eq!(world.count_firearms().unwrap(), 3);
    assert!(world.session.is_open());
    assert!(world.events.recorded().is_empty());
}

#[test]
fn a_file_replaced_by_another_computer_refuses_the_next_write_and_closes() {
    let world = World::new();
    world.create();
    world.add_firearm("A1").unwrap();
    let ours = world.replace_from_another_computer();
    let ours_before = fs::read(&ours).unwrap();
    let theirs_before = fs::read(world.path()).unwrap();

    // Reads still work until a write is tried.
    assert_eq!(world.count_firearms().unwrap(), 1);
    assert!(world.events.recorded().is_empty());

    let ran = Cell::new(false);
    let refused = world.session.write(|_| {
        ran.set(true);
        Ok(())
    });

    assert_eq!(refused.unwrap_err().code, "DATABASE_TAKEN_OVER");
    assert!(!ran.get(), "nothing is written");
    assert!(!world.session.is_open());
    assert_eq!(
        world.events.recorded(),
        vec![
            (
                "notice".to_owned(),
                json!({ "kind": "takenOver", "databasePath": world.path_text() })
            ),
            (
                "session:closed".to_owned(),
                json!({ "reason": "takenOver", "databasePath": world.path_text() })
            ),
        ]
    );
    assert_eq!(fs::read(&ours).unwrap(), ours_before, "our file gets nothing more");
    assert_eq!(fs::read(world.path()).unwrap(), theirs_before, "theirs is never written");
    assert_eq!(world.add_firearm("A2").unwrap_err().code, "DATABASE_CLOSED");
}

#[test]
fn a_close_after_a_take_over_writes_nothing() {
    let world = World::new();
    world.create();
    world.add_firearm("A1").unwrap();
    let ours = world.replace_from_another_computer();
    let ours_before = fs::read(&ours).unwrap();
    let theirs_before = fs::read(world.path()).unwrap();

    let outcome = lifecycle::close_normal(&world.session, CloseReason::Closed).unwrap();

    assert_eq!(outcome.backup, BackupOutcome::NotAttempted);
    assert!(!world.session.is_open());
    let events = world.events.recorded();
    assert_eq!(events.last().unwrap().1["reason"], "takenOver");
    assert!(events
        .iter()
        .any(|(event, payload)| event == "notice" && payload["kind"] == "takenOver"));
    assert_eq!(fs::read(&ours).unwrap(), ours_before, "no marker clear, no backup");
    assert_eq!(fs::read(world.path()).unwrap(), theirs_before);
}

fn assert_unavailable(result: Result<(), CommandError>, path: &Path) {
    let error = result.unwrap_err();
    assert_eq!(error.code, "DATABASE_UNAVAILABLE");
    assert_eq!(error.details.as_deref(), Some(&json!({ "path": path.to_string_lossy() })));
}

#[test]
fn storage_that_goes_away_is_not_a_take_over() {
    let world = World::new();
    world.create();
    world.add_firearm("A1").unwrap();
    let away = world.dir.path().join("unplugged.hoplodex");
    fs::rename(world.path(), &away).unwrap();

    let ran = Cell::new(false);
    let refused = world.session.write(|_| {
        ran.set(true);
        Ok(())
    });

    assert_unavailable(refused, &world.path());
    assert!(!ran.get(), "nothing is written");
    assert!(world.session.is_open(), "the session stays open, and the form keeps its input");
    assert!(world.events.recorded().is_empty(), "no session:closed, no notice");

    // The drive comes back, but every write is still refused.
    fs::rename(&away, world.path()).unwrap();
    assert_unavailable(world.add_firearm("A2"), &world.path());
    assert_unavailable(world.add_firearm("A3"), &world.path());

    let before = fs::read(world.path()).unwrap();
    let outcome = lifecycle::close_normal(&world.session, CloseReason::Closed).unwrap();

    assert_eq!(outcome.backup, BackupOutcome::Failed);
    assert_eq!(outcome.failure_reason, Some(BackupFailureReason::DatabaseUnreachable));
    assert_eq!(fs::read(world.path()).unwrap(), before, "the close writes nothing");
    let events = world.events.recorded();
    assert!(events.contains(&(
        "notice".to_owned(),
        json!({
            "kind": "backupFailed",
            "databasePath": world.path_text(),
            "reason": "databaseUnreachable"
        })
    )));
    assert_eq!(events.last().unwrap().1["reason"], "closed");

    // Its own marker stays, and opens here without a warning.
    lifecycle::open(&world.session, &world.machine, &world.path(), &passphrase(), false).unwrap();
    assert_eq!(world.count_firearms().unwrap(), 1);
}

#[cfg(unix)]
#[test]
fn a_folder_that_cant_be_read_any_more_is_unreachable() {
    use std::os::unix::fs::PermissionsExt;

    let world = World::new();
    let share = world.dir.path().join("share");
    fs::create_dir(&share).unwrap();
    let path = share.join("Mine.hoplodex");
    lifecycle::create(&world.session, &world.machine, &path, &passphrase()).unwrap();

    fs::set_permissions(&share, fs::Permissions::from_mode(0o000)).unwrap();
    let refused = world.session.write(|_| Ok(()));
    fs::set_permissions(&share, fs::Permissions::from_mode(0o700)).unwrap();

    assert_unavailable(refused, &path);
    assert!(world.session.is_open());
    assert!(world.events.recorded().is_empty());
}

#[test]
fn an_io_error_from_sqlite_during_a_write_counts_as_lost_storage() {
    let world = World::new();
    world.create();

    let failed = world.session.write(|_| -> Result<(), CommandError> {
        Err(CommandError::from_db(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
            None,
        )))
    });

    assert_unavailable(failed, &world.path());
    assert_unavailable(world.add_firearm("A1"), &world.path());
    assert!(world.session.is_open());
}
