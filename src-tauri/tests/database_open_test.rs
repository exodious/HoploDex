//! Open outcomes and the normal close (research.md §2; data-model.md
//! "Session states and transitions"). Real SQLCipher files in temp
//! directories; machine settings in a temp config directory.

mod support;

use std::fs;
use std::path::PathBuf;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::db::{self, OpenError};
use hoplodex_lib::models::database::{BackupOutcome, CloseReason};
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::session::{lifecycle, Session};
use serde_json::json;
use support::{passphrase, TestDb, TestEvents};
use tempfile::TempDir;

#[test]
fn a_missing_file_is_not_found() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("gone.hoplodex");

    let result = db::open_database(&path, &passphrase());

    assert!(matches!(result, Err(OpenError::NotFound { path: ref p }) if p == &path), "{result:?}");
    let error = CommandError::from(result.unwrap_err());
    assert_eq!(error.code, "DATABASE_NOT_FOUND");
    assert_eq!(error.details.as_deref(), Some(&json!({ "path": path.to_string_lossy() })));
}

#[cfg(unix)]
#[test]
fn a_file_without_permission_is_unreadable() {
    use std::os::unix::fs::PermissionsExt;

    let db = TestDb::new();
    let path = db.path();
    let (conn, _dir) = db.into_parts();
    drop(conn);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();

    let result = db::open_database(&path, &passphrase());

    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(matches!(result, Err(OpenError::Unreadable { .. })), "{result:?}");
    assert_eq!(CommandError::from(result.unwrap_err()).code, "DATABASE_UNREADABLE");
}

#[test]
fn a_database_open_elsewhere_in_this_process_is_in_use_even_with_a_wrong_passphrase() {
    let db = TestDb::new();

    let right = db::open_database(&db.path(), &passphrase());
    let wrong = db::open_database(&db.path(), &Passphrase::from_input("wrong wrong wrong".into()));

    assert!(matches!(right, Err(OpenError::InUse)), "{right:?}");
    assert!(matches!(wrong, Err(OpenError::InUse)), "{wrong:?}");
    // The first connection is unaffected.
    db.conn.execute("UPDATE collection_settings SET idle_lock_minutes = 12", []).unwrap();
    let minutes: i64 = db
        .conn
        .query_row("SELECT idle_lock_minutes FROM collection_settings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(minutes, 12);
}

#[test]
fn a_reopened_database_holds_its_lock_too() {
    let mut db = TestDb::new();
    db.reopen();

    let second = db::open_database(&db.path(), &passphrase());

    assert!(matches!(second, Err(OpenError::InUse)), "{second:?}");
    let error = CommandError::from(second.unwrap_err());
    assert_eq!(error.code, "DATABASE_IN_USE");
}

#[test]
fn a_truncated_file_is_damaged() {
    let db = TestDb::new();
    let path = db.path();
    let (conn, _dir) = db.into_parts();
    drop(conn);
    // Keep the first pages (salt and page 1 intact, so the passphrase is
    // accepted) and cut off the rest of the schema.
    let bytes = fs::read(&path).unwrap();
    assert!(bytes.len() > 8 * 4096, "the schema should span several pages");
    fs::write(&path, &bytes[..2 * 4096]).unwrap();

    let result = db::open_database(&path, &passphrase());

    assert!(matches!(result, Err(OpenError::Damaged)), "{result:?}");
    assert_eq!(CommandError::from(result.unwrap_err()).code, "DATABASE_DAMAGED");
}

struct Setup {
    dir: TempDir,
    config: TempDir,
    session: Session,
    machine: MachineSettings,
}

impl Setup {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let machine = MachineSettings::load(config.path()).unwrap();
        Self { dir: TempDir::new().unwrap(), config, session: Session::default(), machine }
    }

    fn db_path(&self, name: &str) -> PathBuf {
        self.dir.path().join(format!("{name}.hoplodex"))
    }

    fn documents(&self) -> PathBuf {
        self.config.path().join("opened-documents")
    }

    fn create(&self, name: &str) {
        lifecycle::create(&self.session, &self.machine, &self.db_path(name), &passphrase())
            .unwrap();
    }

    fn close(&self, events: &TestEvents) {
        lifecycle::close_normal(&self.session, events, &self.documents(), CloseReason::Closed)
            .unwrap();
    }
}

#[test]
fn opening_installs_the_database_and_puts_it_first_in_the_recent_list() {
    let setup = Setup::new();
    let events = TestEvents::default();
    setup.create("First");
    setup.close(&events);
    setup.create("Second");
    setup.close(&events);
    assert_eq!(
        setup.machine.recent().iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["Second", "First"]
    );

    lifecycle::open(&setup.session, &setup.machine, &setup.db_path("First"), &passphrase())
        .unwrap();

    assert!(setup.session.is_open());
    let recent = setup.machine.recent();
    assert_eq!(recent.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["First", "Second"]);
    assert_eq!(recent[0].path, setup.db_path("First"));
    let database_id: String = setup
        .session
        .read(|conn| {
            conn.query_row("SELECT database_id FROM app_state", [], |r| r.get(0))
                .map_err(CommandError::from_db)
        })
        .unwrap();
    assert_eq!(recent[0].database_id.as_deref(), Some(database_id.as_str()));
    assert_eq!(
        recent[0].backup_folder.as_deref(),
        Some(setup.dir.path().join("HoploDex backups").as_path())
    );
}

#[test]
fn a_wrong_passphrase_through_the_lifecycle_installs_nothing() {
    let setup = Setup::new();
    let events = TestEvents::default();
    setup.create("Mine");
    setup.close(&events);

    let result = lifecycle::open(
        &setup.session,
        &setup.machine,
        &setup.db_path("Mine"),
        &Passphrase::from_input("wrong wrong wrong".into()),
    );

    assert_eq!(result.unwrap_err().code, "PASSPHRASE_INCORRECT");
    assert!(!setup.session.is_open());
}

#[test]
fn the_normal_close_clears_the_marker_drops_the_connection_and_the_document_copies() {
    let setup = Setup::new();
    let events = TestEvents::default();
    setup.create("Mine");
    let marker_set: bool = setup
        .session
        .read(|conn| {
            conn.query_row("SELECT open_machine_id IS NOT NULL FROM app_state", [], |r| r.get(0))
                .map_err(CommandError::from_db)
        })
        .unwrap();
    assert!(marker_set, "create sets the open marker");
    let copy = setup.documents().join("7").join("receipt.pdf");
    fs::create_dir_all(copy.parent().unwrap()).unwrap();
    fs::write(&copy, b"decrypted").unwrap();

    let outcome =
        lifecycle::close_normal(&setup.session, &events, &setup.documents(), CloseReason::Closed)
            .unwrap();

    assert_eq!(outcome.backup, BackupOutcome::NotAttempted);
    assert!(!setup.session.is_open());
    assert!(!copy.exists(), "decrypted document copies are deleted at close (FR-022)");
    let path = setup.db_path("Mine");
    assert_eq!(
        events.recorded(),
        vec![
            ("session:closing".to_owned(), json!({ "reason": "closed" })),
            (
                "session:closed".to_owned(),
                json!({
                    "reason": "closed",
                    "databasePath": path.to_string_lossy(),
                    "outcome": { "backup": "notAttempted" }
                })
            ),
        ]
    );
    // The connection is gone: the file opens again, and the marker is clear.
    let conn = db::open_database(&path, &passphrase()).unwrap();
    let marker: Option<String> =
        conn.query_row("SELECT open_machine_id FROM app_state", [], |r| r.get(0)).unwrap();
    assert_eq!(marker, None);
}

#[test]
fn closing_with_nothing_open_is_refused() {
    let setup = Setup::new();
    let events = TestEvents::default();

    let result =
        lifecycle::close_normal(&setup.session, &events, &setup.documents(), CloseReason::Closed);

    assert_eq!(result.unwrap_err().code, "DATABASE_CLOSED");
    assert!(events.recorded().is_empty());
}
