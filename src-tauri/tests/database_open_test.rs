//! Open outcomes, switching and the normal close (research.md §2;
//! data-model.md "Session states and transitions"). Real SQLCipher files in
//! temp directories; machine settings in a temp config directory.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoplodex_lib::commands::databases::ops;
use hoplodex_lib::commands::CommandError;
use hoplodex_lib::db::{self, OpenError};
use hoplodex_lib::models::database::{BackupOutcome, ChooserState, CloseReason};
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::session::{lifecycle, Session};
use rusqlite::Connection;
use serde_json::json;
use support::{passphrase, test_machine, test_session, TestDb, TestEvents};
use tempfile::TempDir;

/// Opens `path` as this test's computer would.
fn open(path: &Path, passphrase: &Passphrase) -> Result<Connection, OpenError> {
    db::open_database(path, passphrase, &test_machine(), false)
}

#[test]
fn a_missing_file_is_not_found() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("gone.hoplodex");

    let result = open(&path, &passphrase());

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

    let result = open(&path, &passphrase());

    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(matches!(result, Err(OpenError::Unreadable { .. })), "{result:?}");
    assert_eq!(CommandError::from(result.unwrap_err()).code, "DATABASE_UNREADABLE");
}

#[test]
fn a_database_open_elsewhere_in_this_process_is_in_use_even_with_a_wrong_passphrase() {
    let db = TestDb::new();

    let right = open(&db.path(), &passphrase());
    let wrong = open(&db.path(), &Passphrase::from_input("wrong wrong wrong".into()));

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

    let second = open(&db.path(), &passphrase());

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

    let result = open(&path, &passphrase());

    assert!(matches!(result, Err(OpenError::Damaged)), "{result:?}");
    assert_eq!(CommandError::from(result.unwrap_err()).code, "DATABASE_DAMAGED");
}

struct Setup {
    dir: TempDir,
    config: TempDir,
    session: Session,
    events: Arc<TestEvents>,
    machine: MachineSettings,
}

impl Setup {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let machine = MachineSettings::load(config.path()).unwrap();
        let (session, events) = test_session(&config.path().join("opened-documents"));
        Self { dir: TempDir::new().unwrap(), config, session, events, machine }
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

    fn open(&self, name: &str) -> Result<(), CommandError> {
        lifecycle::open(&self.session, &self.machine, &self.db_path(name), &passphrase(), false)
    }

    fn close(&self) {
        lifecycle::close_normal(&self.session, CloseReason::Closed).unwrap();
    }

    fn chooser(&self) -> ChooserState {
        ops::chooser_state(&self.session, &self.machine, None, None)
    }

    fn path_text(&self, name: &str) -> String {
        self.db_path(name).to_string_lossy().into_owned()
    }
}

#[test]
fn opening_installs_the_database_and_puts_it_first_in_the_recent_list() {
    let setup = Setup::new();
    setup.create("First");
    setup.close();
    setup.create("Second");
    setup.close();
    assert_eq!(
        setup.machine.recent().iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["Second", "First"]
    );

    setup.open("First").unwrap();

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
    setup.create("Mine");
    setup.close();

    let result = lifecycle::open(
        &setup.session,
        &setup.machine,
        &setup.db_path("Mine"),
        &Passphrase::from_input("wrong wrong wrong".into()),
        false,
    );

    assert_eq!(result.unwrap_err().code, "PASSPHRASE_INCORRECT");
    assert!(!setup.session.is_open());
}

#[test]
fn the_normal_close_clears_the_marker_drops_the_connection_and_the_document_copies() {
    let setup = Setup::new();
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

    let outcome = lifecycle::close_normal(&setup.session, CloseReason::Closed).unwrap();

    assert_eq!(outcome.backup, BackupOutcome::NotAttempted);
    assert!(!setup.session.is_open());
    assert!(!copy.exists(), "decrypted document copies are deleted at close (FR-022)");
    let path = setup.db_path("Mine");
    assert_eq!(
        setup.events.recorded(),
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
    let conn = db::open_database(&path, &passphrase(), &support::other_machine(), false).unwrap();
    let marker: String =
        conn.query_row("SELECT open_machine_id FROM app_state", [], |r| r.get(0)).unwrap();
    assert_eq!(marker, support::other_machine().id, "another computer opens it without a warning");
}

#[test]
fn closing_with_nothing_open_is_refused() {
    let setup = Setup::new();

    let result = lifecycle::close_normal(&setup.session, CloseReason::Closed);

    assert_eq!(result.unwrap_err().code, "DATABASE_CLOSED");
    assert!(setup.events.recorded().is_empty());
}

// --- User Story 2 -----------------------------------------------------------

#[test]
fn a_database_naming_an_unknown_migration_is_newer_and_left_untouched() {
    let db = TestDb::new();
    db.conn
        .execute(
            "INSERT INTO schema_migrations (name, applied_at) VALUES ('0099_from_the_future', 'x')",
            [],
        )
        .unwrap();
    let path = db.path();
    let (conn, _dir) = db.into_parts();
    drop(conn);
    let before = fs::read(&path).unwrap();

    let result = open(&path, &passphrase());

    assert!(matches!(result, Err(OpenError::NewerVersion)), "{result:?}");
    assert_eq!(CommandError::from(result.unwrap_err()).code, "DATABASE_NEWER_VERSION");
    assert_eq!(fs::read(&path).unwrap(), before, "FR-014: a refused open never writes the file");
}

#[test]
fn opening_sets_the_open_marker_to_this_machine() {
    let db = TestDb::new();
    let path = db.path();
    let (conn, _dir) = db.into_parts();
    conn.execute(
        "UPDATE app_state SET open_machine_id = NULL, open_machine_name = NULL, open_since = NULL",
        [],
    )
    .unwrap();
    drop(conn);

    let conn = open(&path, &passphrase()).unwrap();

    let (id, name, since): (String, String, String) = conn
        .query_row(
            "SELECT open_machine_id, open_machine_name, open_since FROM app_state",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(id, test_machine().id);
    assert_eq!(name, test_machine().display_name);
    assert!(chrono::DateTime::parse_from_rfc3339(&since).is_ok(), "{since}");
}

#[test]
fn opening_a_second_database_closes_the_first_as_a_switch() {
    let setup = Setup::new();
    setup.create("First");
    setup.close();
    setup.create("Second");
    setup.events.take();

    setup.open("First").unwrap();

    assert_eq!(
        setup.events.recorded(),
        vec![
            ("session:closing".to_owned(), json!({ "reason": "switched" })),
            (
                "session:closed".to_owned(),
                json!({
                    "reason": "switched",
                    "databasePath": setup.path_text("Second"),
                    "outcome": { "backup": "notAttempted" }
                })
            ),
        ]
    );
    let open_path = setup.session.inspect(|open| Ok(open.path.clone())).unwrap();
    assert_eq!(open_path, setup.db_path("First"));
    // "Second" was closed normally: another computer opens it without a
    // warning.
    db::open_database(&setup.db_path("Second"), &passphrase(), &support::other_machine(), false)
        .unwrap();
}

#[test]
fn close_database_accepts_only_closed_and_switched() {
    let setup = Setup::new();
    setup.create("Mine");

    let refused = ops::close_database(&setup.session, CloseReason::Idle).unwrap_err();
    assert_eq!(refused.code, "VALIDATION_ERROR");
    assert!(setup.session.is_open());

    let outcome = ops::close_database(&setup.session, CloseReason::Switched).unwrap();
    assert_eq!(outcome.backup, BackupOutcome::NotAttempted);
    assert!(!setup.session.is_open());
    let (event, payload) = setup.events.recorded().pop().unwrap();
    assert_eq!(event, "session:closed");
    assert_eq!(payload["reason"], "switched");
}

#[test]
fn the_chooser_marks_a_missing_file_unavailable() {
    let setup = Setup::new();
    setup.create("Kept");
    setup.close();
    setup.create("Moved");
    setup.close();
    fs::rename(setup.db_path("Moved"), setup.dir.path().join("elsewhere.hoplodex")).unwrap();

    let state = setup.chooser();

    let rows: Vec<(&str, bool)> =
        state.recent.iter().map(|r| (r.name.as_str(), r.available)).collect();
    assert_eq!(rows, [("Moved", false), ("Kept", true)]);
}

#[test]
fn the_chooser_selects_the_database_just_closed() {
    let setup = Setup::new();
    setup.create("First");
    setup.close();
    setup.create("Second");
    setup.close();
    assert_eq!(setup.chooser().selected_path, Some(setup.path_text("Second")));

    setup.open("First").unwrap();
    setup.close();
    // "Second" is the most recent again, but "First" was closed last.
    let second_id = setup.machine.recent()[1].database_id.clone().unwrap();
    setup.machine.touch_recent(
        &setup.db_path("Second"),
        "Second",
        &second_id,
        &setup.dir.path().join("HoploDex backups"),
    );

    let state = setup.chooser();
    assert_eq!(state.recent[0].name, "Second");
    assert_eq!(state.selected_path, Some(setup.path_text("First")));
}

#[test]
fn a_new_run_selects_the_most_recent() {
    let setup = Setup::new();
    setup.create("First");
    setup.close();
    setup.create("Second");
    setup.close();
    setup.open("First").unwrap();
    setup.close();
    // A new run: a new session, the same machine.json.
    let (session, _events) = test_session(&setup.documents());

    let state = ops::chooser_state(&session, &setup.machine, None, None);

    assert_eq!(state.selected_path, Some(setup.path_text("First")));
    assert_eq!(state.recent[0].name, "First");
}

#[test]
fn locate_and_remove_through_the_commands() {
    let setup = Setup::new();
    setup.create("Kept");
    setup.close();
    setup.create("Moved");
    setup.close();
    let moved = setup.db_path("Moved");
    let found = setup.dir.path().join("found").join("Moved.hoplodex");
    fs::create_dir_all(found.parent().unwrap()).unwrap();
    fs::rename(&moved, &found).unwrap();

    let located =
        ops::locate_database(&setup.machine, &moved.to_string_lossy(), &found.to_string_lossy())
            .unwrap();

    assert_eq!(located.path, found.to_string_lossy());
    assert_eq!(located.name, "Moved");
    assert!(located.available);
    let missing = ops::locate_database(&setup.machine, "/no/such.hoplodex", "/x.hoplodex");
    assert_eq!(missing.unwrap_err().code, "NOT_FOUND");

    let kept = setup.db_path("Kept");
    let before = fs::read(&kept).unwrap();
    let removed = ops::remove_recent_database(&setup.machine, &kept.to_string_lossy());
    assert!(removed.removed);
    let names: Vec<String> = setup.machine.recent().into_iter().map(|r| r.name).collect();
    assert_eq!(names, ["Moved"]);
    assert_eq!(fs::read(&kept).unwrap(), before, "the database file is never touched");
}
