//! What reaches the troubleshooting log, through the process-wide logger and real
//! persistence (013 FR-019, FR-026, FR-027, FR-028). Each test runs in a child
//! process, so it installs a logger of its own under `cargo test` as well as
//! nextest. Every path is under a throwaway folder; the keyring stays off.
//!
//! The journey covers what it walks. Failure paths it doesn't reach are the review
//! question's in `docs/attack-surface.md`, which is why what the app tells users
//! says "designed to leave out" (013 FR-021).

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::commands::import_export::{ExportRecords, ImportSessionStore};
use hoplodex_lib::commands::photos::ops as photo_ops;
use hoplodex_lib::logging::{self, file};
use hoplodex_lib::models::database::{CloseReason, Draft, DraftKind, DraftMode};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::models::troubleshooting_log::LogState;
use hoplodex_lib::services::backups;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::sandbox::Reach;
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use hoplodex_lib::session::lifecycle;
use hoplodex_lib::session::scoped::Target;
use tempfile::TempDir;

const CHILD: &str = "HOPLODEX_TEST_LOG_FOLDER";

/// A line this test file writes as if from the app: its own target isn't ours.
const OURS: &str = "hoplodex_lib::session";

/// Runs the test `name` in a process of its own and returns the folder its file
/// is in, or what the child's exit status was if `expect_success` isn't met.
fn in_a_child(name: &str, expect_success: bool) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env(CHILD, dir.path())
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.success(), expect_success, "{text}");
    let folder = dir.path().to_owned();
    (dir, folder)
}

fn log_text(folder: &Path) -> String {
    fs::read_to_string(folder.join(file::CURRENT)).unwrap()
}

/// 013 FR-019, FR-026, US4-1, US1-1, US1-2.
#[test]
fn only_our_own_lines_reach_the_file_and_debug_only_while_raised() {
    let Some(root) = std::env::var_os(CHILD) else {
        let (_dir, folder) =
            in_a_child("only_our_own_lines_reach_the_file_and_debug_only_while_raised", true);
        let text = log_text(&folder.join(file::FOLDER));
        assert!(text.contains("[DEBUG] said before the folder was known"), "{text}");
        assert!(text.contains("[DEBUG] a debug line of ours"), "{text}");
        assert!(text.contains("[INFO] an info line of ours"), "{text}");
        assert!(!text.contains("library"), "{text}");
        assert!(!text.contains("not while normal"), "{text}");
        assert!(text.contains("still written"), "{text}");
        return;
    };
    logging::init_with(None, Some("debug".into()));
    log::debug!(target: OURS, "said before the folder was known");
    logging::start(Some(PathBuf::from(root).join(file::FOLDER)), None);
    log::debug!(target: OURS, "a debug line of ours");
    log::warn!(target: "tauri::app", "a library warning");
    log::error!(target: "zbus", "a library error");
    log::info!(target: OURS, "an info line of ours");
    assert_eq!(logging::set_state(LogState::On), None);
    log::debug!(target: OURS, "not while normal");
    log::info!(target: OURS, "still written");
}

/// 013 FR-027: a panic's location is written, and its message never is.
#[test]
fn a_panic_writes_its_location_and_not_its_message() {
    let Some(root) = std::env::var_os(CHILD) else {
        let (_dir, folder) = in_a_child("a_panic_writes_its_location_and_not_its_message", false);
        let text = log_text(&folder.join(file::FOLDER));
        assert!(text.contains("[PANIC] at "), "{text}");
        assert!(text.contains("log_contents_test.rs"), "{text}");
        assert!(!text.contains("PLANTED-PANIC-TEXT"), "{text}");
        return;
    };
    logging::init_with(None, Some("on".into()));
    logging::start(Some(PathBuf::from(root).join(file::FOLDER)), None);
    panic!("the serial is PLANTED-PANIC-TEXT");
}

/// What the journey plants: a user name in the home folder, and text in the
/// database's folder and name, a record, a photo's and a document's names, a
/// draft and two passphrases. 013 US2-1 to US2-5.
const PLANTED: [&str; 12] = [
    "USERNAME-PLANTED",
    "FOLDER-PLANTED",
    "DBNAME-PLANTED",
    "MAKE-PLANTED",
    "MODEL-PLANTED",
    "SERIAL-PLANTED",
    "NOTES-PLANTED",
    "PHOTO-PLANTED",
    "DOCUMENT-PLANTED",
    "DRAFT-PLANTED",
    "PASSPHRASE-PLANTED",
    "EXPORT-PLANTED",
];

fn all_text_below(folder: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            all_text_below(&path, found);
        } else {
            found.push(path);
        }
    }
}

/// 013 FR-028, SC-002, US5-4. The journey, at the raised level, with failures
/// where a test can provoke them.
#[test]
fn the_planted_text_of_a_journey_never_reaches_the_log() {
    let Some(root) = std::env::var_os(CHILD) else {
        let (_dir, root) = in_a_child("the_planted_text_of_a_journey_never_reaches_the_log", true);
        let logs = root.join("cache").join(file::FOLDER);
        let text = log_text(&logs);
        for planted in PLANTED {
            assert!(!text.contains(planted), "{planted} reached the log:\n{text}");
            assert!(!text.contains(&planted.to_lowercase()), "{planted}:\n{text}");
        }
        // Not vacuous: the file holds what the journey did, and shapes.
        for said in [
            "[DEBUG] creating a database at ~/*/*.hoplodex",
            "[DEBUG] opening ~/*/*.hoplodex",
            "[DEBUG] backing up ~/*/*.hoplodex",
            "[DEBUG] exporting as Csv to ~/*",
            "[DEBUG] import: matching and saving what was read",
            "[DEBUG] changing the passphrase",
            "[DEBUG] restoring a backup",
            "[DEBUG] locking the database",
            "[DEBUG] closing the database",
        ] {
            let said = said.replace('/', std::path::MAIN_SEPARATOR_STR);
            assert!(text.contains(&said), "the log doesn't say {said:?}:\n{text}");
        }
        // 013 US2-4: nothing says how many records there are.
        assert!(!text.contains("rows"), "{text}");
        // 013 US5-4: the log travels with neither a backup nor an export.
        let mut made = Vec::new();
        all_text_below(&root.join("home"), &mut made);
        assert!(made.iter().any(|path| path.to_string_lossy().contains("backups")), "{made:?}");
        for path in &made {
            assert!(!path.to_string_lossy().contains(file::CURRENT), "{path:?}");
        }
        return;
    };
    journey(&PathBuf::from(root));
}

fn journey(root: &Path) {
    let home = root.join("home").join("USERNAME-PLANTED");
    let folder = home.join("FOLDER-PLANTED");
    fs::create_dir_all(&folder).unwrap();
    logging::init_with(Some(home.clone()), Some("debug".into()));
    let config = root.join("config");
    fs::create_dir_all(&config).unwrap();
    let machine = MachineSettings::load(&config).unwrap();
    logging::start(Some(root.join("cache").join(file::FOLDER)), machine.troubleshooting_log());

    let clock = support::ManualClock::at("2026-10-25T12:00:00Z");
    let (session, _events) =
        support::test_session_at(&root.join("cache/opened-documents"), clock.clone());
    let path = folder.join("DBNAME-PLANTED.hoplodex");
    let passphrase = Passphrase::from_input("PASSPHRASE-PLANTED one".to_owned());
    let new_passphrase = Passphrase::from_input("PASSPHRASE-PLANTED two".to_owned());
    let wrong = Passphrase::from_input("PASSPHRASE-PLANTED wrong".to_owned());

    lifecycle::create(&session, &machine, &path, &passphrase).unwrap();
    let mut record = support::firearm("MAKE-PLANTED", "MODEL-PLANTED", "SERIAL-PLANTED");
    record.notes = Some("NOTES-PLANTED".into());
    let firearm = session
        .write(session.current_id(), |conn| {
            let firearm = firearm_ops::create_firearm(conn, &record, false, None)?;
            let on = RecordRef::Firearm(firearm.id);
            photo_ops::add_photo(conn, on, &support::sample_png_bytes(), "PHOTO-PLANTED.png")?;
            document_ops::add_document(conn, on, b"%PDF-1.4 sample", "DOCUMENT-PLANTED.pdf")?;
            // Failures: a document whose name and contents don't match, and a
            // second record with the same serial.
            let _ = document_ops::add_document(conn, on, b"not a pdf", "DOCUMENT-PLANTED.pdf");
            let _ = firearm_ops::create_firearm(conn, &record, false, None);
            Ok(firearm)
        })
        .unwrap();

    // An export, and an import of it, which meets a conflict.
    let export = home.join("EXPORT-PLANTED");
    fs::create_dir_all(&export).unwrap();
    let records = ExportRecords { firearm_ids: vec![firearm.id], accessory_ids: vec![] };
    let exported = session
        .read(session.current_id(), |conn| {
            import_export_ops::export_collection_stoppable(
                conn,
                &export,
                "EXPORT-PLANTED",
                SpreadsheetFormat::Csv,
                &records,
                &mut |_, _| {},
                &|| false,
                &Reach::Unconfined,
            )
        })
        .unwrap();
    let store = ImportSessionStore::new();
    let tables = support::read_import(&exported.spreadsheet_path, SpreadsheetFormat::Csv);
    session
        .write(session.current_id(), |conn| {
            import_export_ops::import_collection_stoppable(
                conn,
                tables,
                &store,
                &mut |_, _| {},
                &|| false,
                &|_| {},
            )
        })
        .unwrap();

    // A close, which backs up; a wrong passphrase; opening what isn't there.
    lifecycle::close_normal(&session, session.current_id(), &machine, CloseReason::Closed).unwrap();
    assert!(lifecycle::open(&session, &machine, &path, &wrong, false).is_err());
    let missing = folder.join("DBNAME-PLANTED missing.hoplodex");
    assert!(lifecycle::open(&session, &machine, &missing, &passphrase, false).is_err());
    lifecycle::open(&session, &machine, &path, &passphrase, false).unwrap();

    // A change of passphrase, a wrong one first.
    let scratch = root.join("cache/scratch");
    let id = session.current_id();
    assert!(
        backups_ops::change_passphrase(&session, id, &machine, &scratch, &wrong, &new_passphrase)
            .is_err()
    );
    backups_ops::change_passphrase(&session, id, &machine, &scratch, &passphrase, &new_passphrase)
        .unwrap();

    // A backup to a location that can't be used, and a restore, a day on so its
    // own backup of what is there gets a name of its own.
    clock.advance(chrono::Duration::days(1));
    let blocked = home.join("FOLDER-PLANTED").join("a file");
    fs::write(&blocked, "").unwrap();
    let _ = backups::check_chosen_folder(&blocked.join("below"), &folder, &Reach::Unconfined);
    let backup = backups_ops::list_backups(&session, &machine, Target::Open(session.current_id()))
        .unwrap()
        .backups
        .into_iter()
        .next()
        .expect("the close made a backup");
    backups_ops::restore_backup(
        &session,
        &machine,
        &backup.path,
        &passphrase,
        Target::Open(session.current_id()),
        false,
        &Reach::Unconfined,
    )
    .unwrap();

    // A lock with a draft.
    let draft = Draft {
        form_version: 1,
        kind: DraftKind::Firearm,
        mode: DraftMode::Add,
        target_id: None,
        label: "DRAFT-PLANTED".into(),
        values: serde_json::json!({ "notes": "DRAFT-PLANTED" }),
    };
    lifecycle::lock(
        &session,
        session.current_id(),
        &machine,
        CloseReason::LockedByUser,
        Some(draft),
    )
    .unwrap();
    log::logger().flush();
}
