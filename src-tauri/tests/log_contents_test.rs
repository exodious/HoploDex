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
use hoplodex_lib::db::upgrade::{self, UpgradeError};
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
    let (dir, folder, _) = in_a_child_with_stderr(name, expect_success);
    (dir, folder)
}

/// [`in_a_child`], and what the child wrote to standard error.
fn in_a_child_with_stderr(name: &str, expect_success: bool) -> (TempDir, PathBuf, String) {
    in_a_child_with_env(name, expect_success, &[])
}

/// [`in_a_child_with_stderr`], with more of the child's environment.
fn in_a_child_with_env(
    name: &str,
    expect_success: bool,
    env: &[(&str, &str)],
) -> (TempDir, PathBuf, String) {
    let dir = TempDir::new().unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env(CHILD, dir.path())
        .envs(env.iter().copied())
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.success(), expect_success, "{text}");
    let folder = dir.path().to_owned();
    (dir, folder, text)
}

fn log_text(folder: &Path) -> String {
    fs::read_to_string(folder.join(file::CURRENT)).unwrap()
}

/// 013 FR-019, FR-026, US4-1. The binary's own modules count as ours: their target
/// is `hoplodex::app_menu`, say.
#[test]
fn only_our_own_lines_reach_the_file_and_debug_lines_only_there() {
    let Some(root) = std::env::var_os(CHILD) else {
        let (_dir, folder, stderr) = in_a_child_with_stderr(
            "only_our_own_lines_reach_the_file_and_debug_lines_only_there",
            true,
        );
        let text = log_text(&folder.join(file::FOLDER));
        assert!(text.contains("[DEBUG] said before the folder was known"), "{text}");
        assert!(text.contains("[DEBUG] a debug line of ours"), "{text}");
        assert!(text.contains("[INFO] an info line of ours"), "{text}");
        assert!(text.contains("[WARN] a warning from the binary"), "{text}");
        assert!(!text.contains("library"), "{text}");
        assert!(!stderr.contains("[DEBUG]"), "{stderr}");
        assert!(stderr.contains("[WARN] a warning from the binary"), "{stderr}");
        assert!(stderr.contains("a library warning"), "{stderr}");
        return;
    };
    logging::init_with(None, Some("debug".into()));
    log::debug!(target: OURS, "said before the folder was known");
    logging::start(Some(PathBuf::from(root).join(file::FOLDER)), None);
    log::debug!(target: OURS, "a debug line of ours");
    log::warn!(target: "tauri::app", "a library warning");
    log::error!(target: "zbus", "a library error");
    log::info!(target: OURS, "an info line of ours");
    log::warn!(target: "hoplodex::app_menu", "a warning from the binary");
}

/// 013 US1-1, FR-004: debug lines begin at the raise, which then lasts until quit.
#[test]
fn debug_lines_begin_at_the_raise_and_it_lasts_until_quit() {
    let Some(root) = std::env::var_os(CHILD) else {
        let (_dir, folder) =
            in_a_child("debug_lines_begin_at_the_raise_and_it_lasts_until_quit", true);
        let text = log_text(&folder.join(file::FOLDER));
        assert!(!text.contains("before the raise"), "{text}");
        assert!(text.contains("[INFO] an info line"), "{text}");
        assert!(text.contains("[DEBUG] after the raise"), "{text}");
        assert!(text.contains("[DEBUG] after asking for less"), "{text}");
        return;
    };
    logging::init_with(None, Some("on".into()));
    logging::start(Some(PathBuf::from(root).join(file::FOLDER)), None);
    log::debug!(target: OURS, "before the raise");
    log::info!(target: OURS, "an info line");
    assert_eq!(logging::set_state(LogState::Detailed), None);
    log::debug!(target: OURS, "after the raise");
    assert_eq!(logging::set_state(LogState::On), None);
    log::debug!(target: OURS, "after asking for less");
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

/// 013 FR-027: what a release build's `main()` installs first. Standard error, the
/// journal on a Linux desktop, gets the location and never the message.
#[test]
fn a_release_builds_panic_prints_only_its_location() {
    let Some(root) = std::env::var_os(CHILD) else {
        let (_dir, folder, stderr) =
            in_a_child_with_stderr("a_release_builds_panic_prints_only_its_location", false);
        assert!(stderr.contains("[PANIC] at "), "{stderr}");
        assert!(stderr.contains("log_contents_test.rs"), "{stderr}");
        assert!(!stderr.contains("PLANTED-PANIC-TEXT"), "{stderr}");
        let text = log_text(&folder.join(file::FOLDER));
        assert!(text.contains("[PANIC] at "), "{text}");
        assert!(!text.contains("PLANTED-PANIC-TEXT"), "{text}");
        return;
    };
    logging::panic_location_only();
    logging::init_with(None, Some("on".into()));
    logging::start(Some(PathBuf::from(root).join(file::FOLDER)), None);
    panic!("the serial is PLANTED-PANIC-TEXT");
}

/// 013 US4-1: a run the variable decides has its file before anything in `setup`
/// can fail, so what came before and why it failed are both there, and the OS's
/// text names no folder (013 FR-016).
#[test]
fn a_failed_start_under_the_variable_keeps_its_lines_and_why() {
    let Some(root) = std::env::var_os(CHILD) else {
        let (_dir, folder, stderr) = in_a_child_with_stderr(
            "a_failed_start_under_the_variable_keeps_its_lines_and_why",
            true,
        );
        let text = log_text(&folder.join(file::FOLDER));
        assert!(text.contains("[DEBUG] said before anything failed"), "{text}");
        assert!(text.contains("[ERROR] the app could not start: "), "{text}");
        assert_eq!(text.matches("--- HoploDex ").count(), 1, "{text}");
        assert!(!text.contains("PLANTED"), "{text}");
        assert!(stderr.contains("[ERROR] the app could not start: "), "{stderr}");
        assert!(!stderr.contains("PLANTED"), "{stderr}");
        return;
    };
    let root = PathBuf::from(root);
    logging::init_with(None, Some("debug".into()));
    log::debug!(target: OURS, "said before anything failed");
    let decided = logging::start_for_variable(Some(root.join(file::FOLDER))).unwrap();
    assert_eq!(decided.state, LogState::Detailed);
    let blocked = root.join("FOLDER-PLANTED");
    fs::write(&blocked, "").unwrap();
    let err = MachineSettings::load(&blocked.join("config")).err().expect("the load fails");
    logging::startup_failed(&err);
}

/// 013 FR-002, FR-012: with no variable the saved choice decides, and a start that
/// failed before reading it says why on standard error and leaves the files alone.
/// Starting on a guess would add to them, and at 1.0.0 delete a saved On's.
#[test]
fn a_failed_start_without_the_variable_leaves_the_files_alone() {
    const EARLIER: &str = "from an earlier run\n";
    let Some(root) = std::env::var_os(CHILD) else {
        let (_dir, folder, stderr) = in_a_child_with_stderr(
            "a_failed_start_without_the_variable_leaves_the_files_alone",
            true,
        );
        assert_eq!(log_text(&folder.join(file::FOLDER)), EARLIER);
        assert!(stderr.contains("[ERROR] the app could not start: "), "{stderr}");
        return;
    };
    let folder = PathBuf::from(root).join(file::FOLDER);
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join(file::CURRENT), EARLIER).unwrap();
    logging::init_with(None, None);
    assert_eq!(logging::start_for_variable(Some(folder)), None);
    logging::startup_failed(&std::io::Error::other("the OS's text"));
}

/// 013 FR-016: a broken link is told by its table, once, however many rows have
/// one, so the log doesn't say how many of a user's records are affected.
#[test]
fn an_upgrades_broken_links_are_logged_once_for_each_table() {
    let Some(root) = std::env::var_os(CHILD) else {
        let (_dir, folder) =
            in_a_child("an_upgrades_broken_links_are_logged_once_for_each_table", true);
        let text = log_text(&folder.join(file::FOLDER));
        assert_eq!(text.matches("a broken link in photo_like").count(), 1, "{text}");
        assert_eq!(text.matches("a broken link in document_like").count(), 1, "{text}");
        return;
    };
    let root = PathBuf::from(root);
    logging::init_with(None, Some("on".into()));
    logging::start(Some(root.join(file::FOLDER)), None);
    let conn = rusqlite::Connection::open(root.join("links.db")).unwrap();
    let step = (
        "breaks_links",
        "CREATE TABLE owner (id INTEGER PRIMARY KEY);
         CREATE TABLE photo_like (id INTEGER PRIMARY KEY, owner INTEGER REFERENCES owner (id));
         CREATE TABLE document_like (id INTEGER PRIMARY KEY, owner INTEGER REFERENCES owner (id));
         INSERT INTO photo_like (owner) VALUES (101), (102), (103);
         INSERT INTO document_like (owner) VALUES (104), (105);",
    );
    let result = upgrade::upgrade(&conn, &[step], None, &mut |_, _| {});
    assert!(matches!(result, Err(UpgradeError::LinkCheck { violations: 5 })), "{result:?}");
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

/// 013 FR-025: each portal call and its result, on a private bus that the child
/// takes as its session bus. A refusal gives the D-Bus error's name, never its
/// message, and no answer's path reaches the file (013 FR-016).
#[cfg(target_os = "linux")]
mod portals {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};

    use hoplodex_lib::commands::documents::portal_open_directory;
    use hoplodex_lib::services::keyring::portal::{
        DesktopPortal, SecretPortal, secret_portal_available,
    };
    use hoplodex_lib::services::sandbox::{HostPaths, PortalHostPaths};
    use zbus::fdo;
    use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue};

    use super::*;
    use crate::support::private_bus::Bus;

    const DESKTOP: &str = "org.freedesktop.portal.Desktop";
    const DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
    const REFUSAL: &str = "the file is PLANTED";
    const FAILED: &str = "org.freedesktop.DBus.Error.Failed";
    /// A name the portal's own documentation gives, which ashpd turns into a
    /// variant of its `PortalError`.
    const NOT_ALLOWED: &str = "org.freedesktop.portal.Error.NotAllowed";
    /// A name it doesn't, which ashpd leaves as a D-Bus error.
    const UNEXPECTED: &str = "org.freedesktop.portal.Error.Unexpected";

    struct Documents;

    #[zbus::interface(name = "org.freedesktop.portal.Documents")]
    impl Documents {
        async fn get_host_paths(&self, ids: Vec<String>) -> fdo::Result<HashMap<String, Vec<u8>>> {
            if ids.iter().any(|id| id == "refused-PLANTED") {
                return Err(fdo::Error::Failed(REFUSAL.to_owned()));
            }
            Ok(ids
                .into_iter()
                .filter(|id| id == "known-PLANTED")
                .map(|id| (id, b"/HOST-PLANTED\0".to_vec()))
                .collect())
        }
    }

    #[derive(zbus::DBusError, Debug)]
    #[zbus(prefix = "org.freedesktop.portal.Error")]
    enum Refusal {
        #[zbus(error)]
        ZBus(zbus::Error),
        NotAllowed(String),
        Unexpected(String),
    }

    /// Refuses the first `RetrieveSecret` with a name ashpd doesn't know and the
    /// rest with one it does.
    struct Secret {
        refused: AtomicBool,
    }

    #[zbus::interface(name = "org.freedesktop.portal.Secret")]
    impl Secret {
        #[zbus(property, name = "version")]
        fn version(&self) -> u32 {
            1
        }

        async fn retrieve_secret(
            &self,
            _fd: OwnedFd,
            _options: HashMap<String, OwnedValue>,
        ) -> Result<OwnedObjectPath, Refusal> {
            Err(if self.refused.swap(true, Ordering::SeqCst) {
                Refusal::NotAllowed(REFUSAL.to_owned())
            } else {
                Refusal::Unexpected(REFUSAL.to_owned())
            })
        }
    }

    /// A Secret portal that refuses to say its version.
    struct NoVersion;

    #[zbus::interface(name = "org.freedesktop.portal.Secret")]
    impl NoVersion {
        #[zbus(property, name = "version")]
        fn version(&self) -> fdo::Result<u32> {
            Err(fdo::Error::Failed(REFUSAL.to_owned()))
        }
    }

    /// Accepts the first `OpenDirectory` and refuses the rest.
    struct OpenUri {
        accepted: AtomicBool,
    }

    #[zbus::interface(name = "org.freedesktop.portal.OpenURI")]
    impl OpenUri {
        async fn open_directory(
            &self,
            _parent_window: String,
            _fd: OwnedFd,
            _options: HashMap<String, OwnedValue>,
        ) -> fdo::Result<OwnedObjectPath> {
            if self.accepted.swap(true, Ordering::SeqCst) {
                return Err(fdo::Error::Failed(REFUSAL.to_owned()));
            }
            OwnedObjectPath::try_from("/request").map_err(|err| fdo::Error::Failed(err.to_string()))
        }
    }

    #[test]
    fn calls_and_results_reach_the_file_by_name_only() {
        let Some(root) = std::env::var_os(CHILD) else {
            let bus = Bus::start();
            let _documents = bus.serve(
                "org.freedesktop.portal.Documents",
                "/org/freedesktop/portal/documents",
                Documents,
            );
            let desktop =
                bus.serve(DESKTOP, DESKTOP_PATH, Secret { refused: AtomicBool::new(false) });
            zbus::block_on(
                desktop
                    .object_server()
                    .at(DESKTOP_PATH, OpenUri { accepted: AtomicBool::new(false) }),
            )
            .unwrap();
            let (_dir, folder, _) = in_a_child_with_env(
                "portals::calls_and_results_reach_the_file_by_name_only",
                true,
                &[("DBUS_SESSION_BUS_ADDRESS", bus.address())],
            );
            let text = log_text(&folder.join(file::FOLDER));
            let refused = |call: &str| format!("[DEBUG] portal: {call} refused: {FAILED}\n");
            for line in [
                "[DEBUG] portal: GetHostPaths\n".to_owned(),
                "[DEBUG] portal: GetHostPaths answered with the document's path\n".to_owned(),
                "[DEBUG] portal: GetHostPaths answered without this document\n".to_owned(),
                refused("GetHostPaths"),
                "[DEBUG] portal: RetrieveSecret\n".to_owned(),
                format!("[DEBUG] portal: RetrieveSecret refused: {UNEXPECTED}\n"),
                format!("[DEBUG] portal: RetrieveSecret refused: {NOT_ALLOWED}\n"),
                "[DEBUG] portal: OpenDirectory\n".to_owned(),
                "[DEBUG] portal: OpenDirectory accepted\n".to_owned(),
                refused("OpenDirectory"),
            ] {
                assert!(text.contains(&line), "no {line:?} in:\n{text}");
            }
            assert!(!text.contains("PLANTED"), "{text}");
            return;
        };
        let root = PathBuf::from(root);
        logging::init_with(None, Some("debug".into()));
        logging::start(Some(root.join(file::FOLDER)), None);

        let document = |id: &str| PathBuf::from(format!("/run/user/1000/doc/{id}/FILE-PLANTED"));
        let paths = PortalHostPaths::new();
        assert_eq!(
            paths.host_path(&document("known-PLANTED")),
            Some(PathBuf::from("/HOST-PLANTED"))
        );
        assert_eq!(paths.host_path(&document("absent-PLANTED")), None);
        assert_eq!(paths.host_path(&document("refused-PLANTED")), None);

        let reason = |name: &str| format!("the Secret portal gave no secret: {name}");
        assert_eq!(DesktopPortal.retrieve_secret().unwrap_err(), reason(UNEXPECTED));
        assert_eq!(DesktopPortal.retrieve_secret().unwrap_err(), reason(NOT_ALLOWED));

        let shown = root.join("FILE-PLANTED");
        fs::write(&shown, "").unwrap();
        let connection = zbus::block_on(zbus::Connection::session()).unwrap();
        portal_open_directory(&connection, &shown).unwrap();
        assert!(portal_open_directory(&connection, &shown).is_err());
    }

    /// 013 FR-016: what the portal says when it won't give its version stays out.
    #[test]
    fn a_refused_version_is_logged_by_name_only() {
        let Some(root) = std::env::var_os(CHILD) else {
            let bus = Bus::start();
            let _desktop = bus.serve(DESKTOP, DESKTOP_PATH, NoVersion);
            let (_dir, folder, _) = in_a_child_with_env(
                "portals::a_refused_version_is_logged_by_name_only",
                true,
                &[("DBUS_SESSION_BUS_ADDRESS", bus.address())],
            );
            let text = log_text(&folder.join(file::FOLDER));
            let line = format!("[WARN] no Secret portal: the Secret portal's version: {FAILED}\n");
            assert!(text.contains(&line), "no {line:?} in:\n{text}");
            assert!(!text.contains("PLANTED"), "{text}");
            return;
        };
        logging::init_with(None, Some("on".into()));
        logging::start(Some(PathBuf::from(root).join(file::FOLDER)), None);
        let connection = zbus::block_on(zbus::Connection::session()).unwrap();
        assert!(!secret_portal_available(&connection));
    }
}
