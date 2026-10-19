//! Every command that names a location refuses one outside what a Flatpak can
//! reach, before it writes anything (011 US1-8, 011 US1-5, 011 FR-004,
//! 011 FR-007, 011 SC-003; 011 research.md §5; 011 contracts/tauri-commands.md
//! "Commands that check reach"; 011 data-model.md "Backup location
//! availability"). Real files and real SQLCipher databases in temp folders, each
//! test under a `Reach::Flatpak` built for its own temporary folder: a writable
//! grant `<tmp>/home`, a read-only grant `<tmp>/ro`, and `<tmp>/outside`, which is
//! granted nothing. Each refusal also shows that `<tmp>`'s whole recursive listing
//! (names, sizes, modification times) is the same afterwards: no scratch file,
//! probe, partial backup, partial export or `.<file>.new`.
//!
//! Written ahead of 011 T042 and T044 to T048; it fails to compile until they
//! exist. Every name this file assumes of code that was not there yet is gathered
//! here, so one place says what the implementation was built to. Each ops
//! function that is given `&Reach` takes it as its LAST parameter:
//!
//! - `databases::create_database(&Session, &MachineSettings, folder: &str,
//!   name: &str, &Passphrase, acknowledged_unrecoverable: bool, &Reach)`;
//! - `databases::open_database(&Session, &MachineSettings, path: &str, Unlock,
//!   take_over: bool, &Reach)`;
//! - `databases::locate_database(&MachineSettings, path: &str, new_path: &str,
//!   &Reach)`;
//! - `databases::chooser_state(&Session, &MachineSettings, documents:
//!   Option<PathBuf>, home: Option<PathBuf>, &Reach)`;
//! - `databases::update_backup_settings(&Session, SessionId, &MachineSettings,
//!   &BackupSettingsInput, &Reach)`;
//! - `backups_ops::restore_backup(&Session, &MachineSettings, backup_path: &str,
//!   &Passphrase, Target, upgrade: bool, &Reach)`;
//! - `import_export_ops::export_collection(&Connection, &Path, base_name: &str,
//!   SpreadsheetFormat, &ExportRecords, on_progress, &Reach)`;
//! - `import_export_ops::read_import_files(helper_exe: &Path, &[ImportFile],
//!   &OperationGuard, session_is_current, progress, &Reach)` (the only op that
//!   takes an import file's path, so it is where "`import_collection`" checks
//!   each file: `import_collection` itself takes tables already read);
//! - `photo_ops::add_photo_from_path(&Connection, RecordRef, &Path, &Reach)` and
//!   `document_ops::add_document_from_path(&Connection, RecordRef, &Path,
//!   &Reach)`.
//!
//! Unchanged signatures, which take the `Reach` from the `Session` they are given
//! (`Session::reach()`, built here with `support::test_session_reaching`):
//! `lifecycle::close_normal`, `backups_ops::list_backups`,
//! `backups_ops::change_passphrase`, `databases::upgrade_database`,
//! `databases::database_status`, `databases::cancel_upgrade`.
//!
//! The new IPC fields are read from the serialized answers (`unavailableReason`,
//! `failureReason`) rather than through their Rust types, so only their JSON
//! spelling (011 contracts/tauri-commands.md "Shared types") is assumed.

use crate::support;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::databases::ops::Unlock;
use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::commands::import_export::{ExportRecords, ImportFile, ImportSessionStore};
use hoplodex_lib::commands::photos::ops as photo_ops;
use hoplodex_lib::db::upgrade::testing;
use hoplodex_lib::models::database::{
    BackupLocationInput, BackupSettingsInput, CloseReason, DatabaseStatus, ExistingBackupsChoice,
    OperationKind,
};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
#[cfg(unix)]
use hoplodex_lib::services::sandbox::Need;
use hoplodex_lib::services::sandbox::{Grant, HostPaths, Reach};
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use hoplodex_lib::session::scoped::Target;
use hoplodex_lib::session::{Session, lifecycle};
use serde_json::{Value, json};
use support::{ManualClock, TestDb, document_fixture, passphrase, sample_png_bytes};
use tempfile::TempDir;

const START: &str = "2026-10-19T09:00:00+02:00";
const OTHER_PASSPHRASE: &str = "a much longer passphrase of several words";
/// A test step the upgrade tests register, so a database made by this build is
/// older than the build (as `upgrade_test` does).
const STEP_A: (&str, &str) =
    ("9001_test_a", "CREATE TABLE t_a (id INTEGER PRIMARY KEY, note TEXT);");

// ------------------------------------------------------------------ listings

/// What a listing records of one path: whether it is a folder, its size (files
/// only) and its modification time.
type Entry = (bool, u64, Option<SystemTime>);

/// `root`'s whole recursive listing, `root` excluded, by path below it. Reads
/// metadata only, never a file's bytes, so it is safe on an open database.
fn snapshot(root: &Path) -> BTreeMap<String, Entry> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<String, Entry>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).unwrap();
            let name = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            let is_dir = meta.is_dir();
            into.insert(name, (is_dir, if is_dir { 0 } else { meta.len() }, meta.modified().ok()));
            if is_dir {
                walk(root, &path, into);
            }
        }
    }
    let mut listing = BTreeMap::new();
    walk(root, root, &mut listing);
    listing
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
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

fn details(err: &CommandError) -> Value {
    err.details.as_deref().cloned().unwrap_or(Value::Null)
}

/// `path` as the message writes it in the `flatpak override` command: as it is
/// when every character is plain, else in single quotes (a Windows path, which
/// a Flatpak never has, holds backslashes, so it is quoted there).
fn shell_word(path: &Path) -> String {
    let text = path.display().to_string();
    let plain = text.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(c, '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-')
    });
    if plain { text } else { format!("'{}'", text.replace('\'', r"'\''")) }
}

/// The refusal of 011 FR-004: its code, the two fixes in its message, and the
/// folder it says needs access.
#[track_caller]
fn assert_out_of_reach(err: &CommandError, folder: &Path) {
    assert_eq!(err.code, "LOCATION_OUT_OF_REACH", "{err:?}");
    assert!(
        err.message
            .contains(&format!("flatpak override --user --filesystem={}", shell_word(folder))),
        "the message names the override for {}: {}",
        folder.display(),
        err.message
    );
    assert_eq!(details(err)["folder"], json!(text(folder)), "{err:?}");
}

/// `BACKUP_LOCATION_UNAVAILABLE` with the reason 011 adds.
#[track_caller]
fn assert_backup_out_of_reach(err: &CommandError, folder: &Path) {
    assert_eq!(err.code, "BACKUP_LOCATION_UNAVAILABLE", "{err:?}");
    assert_eq!(details(err), json!({ "path": text(folder), "reason": "outOfReach" }), "{err:?}");
}

// ------------------------------------------------------------------ the portal

/// A fixed-map stand-in for the Documents portal's host-path lookup.
struct FixedHostPaths(HashMap<PathBuf, PathBuf>);

impl HostPaths for FixedHostPaths {
    fn host_path(&self, portal_path: &Path) -> Option<PathBuf> {
        self.0.get(portal_path).cloned()
    }
}

/// A path the Documents portal would hand out (`/run/user/<uid>/doc/<id>/<name>`).
/// It never exists on this computer, and nothing is made at it.
#[cfg(unix)]
fn portal(id: &str, name: &str) -> PathBuf {
    PathBuf::from(format!("/run/user/4242/doc/{id}/{name}"))
}

// ------------------------------------------------------------------ the world

struct World {
    tmp: TempDir,
    config: TempDir,
    machine: MachineSettings,
    clock: Arc<ManualClock>,
    reach: Arc<Reach>,
    /// A session under `Reach::Unconfined`, which makes the files a test starts
    /// from (a database in `outside`, a backup location set there).
    plain: Session,
    /// A session under the Flatpak reach, which the test acts through.
    boxed: Session,
}

impl World {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        for folder in ["home", "ro", "outside", "scratch"] {
            fs::create_dir(tmp.path().join(folder)).unwrap();
        }
        let config = TempDir::new().unwrap();
        let clock = ManualClock::at(START);
        let home = tmp.path().join("home");
        let reach = Arc::new(Reach::flatpak(
            vec![
                Grant { root: home.clone(), writable: true },
                Grant { root: tmp.path().join("ro"), writable: false },
            ],
            Vec::new(),
            home,
            Arc::new(FixedHostPaths(HashMap::new())),
        ));
        let opened = config.path().join("opened-documents");
        let (plain, _) =
            support::test_session_reaching(&opened, clock.clone(), Arc::new(Reach::Unconfined));
        let (boxed, _) = support::test_session_reaching(&opened, clock.clone(), reach.clone());
        Self {
            machine: MachineSettings::load(config.path()).unwrap(),
            tmp,
            config,
            clock,
            reach,
            plain,
            boxed,
        }
    }

    fn home(&self) -> PathBuf {
        self.tmp.path().join("home")
    }

    fn ro(&self) -> PathBuf {
        self.tmp.path().join("ro")
    }

    fn outside(&self) -> PathBuf {
        self.tmp.path().join("outside")
    }

    fn scratch(&self) -> PathBuf {
        self.tmp.path().join("scratch")
    }

    fn listing(&self) -> BTreeMap<String, Entry> {
        snapshot(self.tmp.path())
    }

    fn next_day(&self) {
        self.clock.advance(chrono::Duration::days(1));
    }

    /// A closed database `<folder>/<name>.hoplodex` with one firearm, made as
    /// the `.deb` would (`Unconfined`), with a regular backup made by its close
    /// when `backups` is true and no backup at all when it is false.
    fn make_db(&self, folder: &Path, name: &str, backups: bool) -> PathBuf {
        databases::create_database(
            &self.plain,
            &self.machine,
            &text(folder),
            name,
            &passphrase(),
            true,
            &Reach::Unconfined,
        )
        .unwrap();
        if !backups {
            self.set_plain_backups(false, BackupLocationInput::Default);
        }
        self.change(&self.plain);
        let outcome = self.close(&self.plain);
        assert_eq!(outcome["backup"], json!(if backups { "made" } else { "off" }), "{outcome}");
        self.next_day();
        folder.join(format!("{name}.hoplodex"))
    }

    /// The database `make_db` makes in `home`, its backups going to a custom
    /// folder in `outside` that exists and holds the backup its close made.
    /// Returns the database and that folder.
    fn make_db_backing_up_outside(&self) -> (PathBuf, PathBuf) {
        let backups = self.outside().join("Backups");
        fs::create_dir(&backups).unwrap();
        databases::create_database(
            &self.plain,
            &self.machine,
            &text(&self.home()),
            "Mine",
            &passphrase(),
            true,
            &Reach::Unconfined,
        )
        .unwrap();
        self.set_plain_backups(true, BackupLocationInput::Custom { path: text(&backups) });
        self.change(&self.plain);
        assert_eq!(self.close(&self.plain)["backup"], json!("made"));
        assert_eq!(names_in(&backups).len(), 1, "the backup the close made is in `outside`");
        self.next_day();
        (self.home().join("Mine.hoplodex"), backups)
    }

    fn set_plain_backups(&self, enabled: bool, location: BackupLocationInput) {
        databases::update_backup_settings(
            &self.plain,
            self.plain.current_id(),
            &self.machine,
            &BackupSettingsInput { enabled, keep_count: 5, location, existing_backups: None },
            &Reach::Unconfined,
        )
        .unwrap();
    }

    fn change(&self, session: &Session) {
        let serial = hoplodex_lib::db::random_hex(4).unwrap();
        session
            .write(session.current_id(), |conn| {
                firearms::create_firearm(
                    conn,
                    &support::firearm("Colt", "Python", &serial),
                    false,
                    None,
                )
            })
            .unwrap();
    }

    /// A normal close, and the outcome as the frontend receives it.
    fn close(&self, session: &Session) -> Value {
        let outcome = lifecycle::close_normal(
            session,
            session.current_id(),
            &self.machine,
            CloseReason::Closed,
        )
        .unwrap();
        serde_json::to_value(outcome).unwrap()
    }

    fn open_boxed(&self, path: &str) -> Result<DatabaseStatus, CommandError> {
        databases::open_database(
            &self.boxed,
            &self.machine,
            path,
            Unlock::typed(&passphrase()),
            false,
            &self.reach,
        )
    }

    fn open_boxed_ok(&self, path: &Path) -> DatabaseStatus {
        self.open_boxed(&text(path)).unwrap_or_else(|e| panic!("opening {}: {e:?}", path.display()))
    }

    fn update_backups(
        &self,
        location: BackupLocationInput,
        existing_backups: Option<ExistingBackupsChoice>,
    ) -> Result<(), CommandError> {
        databases::update_backup_settings(
            &self.boxed,
            self.boxed.current_id(),
            &self.machine,
            &BackupSettingsInput { enabled: true, keep_count: 5, location, existing_backups },
            &self.reach,
        )
        .map(|_| ())
    }
}

// ============================================================== create_database

/// 011 US1-8, 011 FR-004, 011 SC-003: a new database in a folder the Flatpak
/// can't reach, or can only read, is refused on the folder field, and nothing
/// is made, not even the folder.
#[test]
fn create_database_outside_reach_and_in_a_read_only_grant_is_refused_and_nothing_is_made() {
    let world = World::new();
    for granted_nothing_or_read_only in [world.outside(), world.ro()] {
        let folder = granted_nothing_or_read_only.join("Guns");
        let before = world.listing();
        let recent_before = world.machine.recent().len();

        let refused = databases::create_database(
            &world.boxed,
            &world.machine,
            &text(&folder),
            "Mine",
            &passphrase(),
            true,
            &world.reach,
        )
        .unwrap_err();

        assert_eq!(refused.code, "LOCATION_OUT_OF_REACH", "{refused:?}");
        assert_eq!(
            refused.field_errors.as_ref().and_then(|fields| fields.get("folder")),
            Some(&refused.message),
            "the message is on the folder field: {refused:?}"
        );
        assert!(refused.message.contains("flatpak override --user --filesystem="));
        assert_eq!(world.listing(), before, "{}: nothing made", folder.display());
        assert!(!folder.exists());
        assert!(!world.boxed.is_open());
        assert_eq!(world.machine.recent().len(), recent_before);
    }
}

/// 011 US1-4: the same call in the writable grant, made as a missing folder
/// first, goes through.
#[test]
fn create_database_in_the_writable_grant_is_made() {
    let world = World::new();
    let folder = world.home().join("Guns");

    let status = databases::create_database(
        &world.boxed,
        &world.machine,
        &text(&folder),
        "Mine",
        &passphrase(),
        true,
        &world.reach,
    )
    .unwrap();

    assert_eq!(status.path, text(&folder.join("Mine.hoplodex")));
    assert!(folder.join("Mine.hoplodex").is_file());
}

// ============================================================== open_database

/// 011 US1-8, 011 FR-004: a database in a folder the Flatpak can't reach, or
/// can only read, is refused at open, with the file's bytes and modification
/// time unchanged and no open marker or journal beside it.
#[test]
fn open_database_outside_reach_and_in_a_read_only_grant_is_refused_without_touching_the_file() {
    let world = World::new();
    for folder in [world.outside(), world.ro()] {
        let path = world.make_db(&folder, "Theirs", true);
        let bytes = fs::read(&path).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let before = world.listing();

        let refused = world.open_boxed(&text(&path)).unwrap_err();

        assert_out_of_reach(&refused, &folder);
        assert_eq!(fs::read(&path).unwrap(), bytes, "{}: the same bytes", path.display());
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        assert_eq!(world.listing(), before, "{}: no marker, journal or copy", path.display());
        assert!(!world.boxed.is_open());
    }
}

/// 011 US1-4: a database within the writable grant opens as before, and the
/// recent list names it.
#[test]
fn open_database_in_the_writable_grant_opens() {
    let world = World::new();
    let path = world.make_db(&world.home(), "Mine", true);

    let status = world.open_boxed_ok(&path);

    assert_eq!(status.path, text(&path));
    assert!(world.boxed.is_open());
}

/// 011 US1-8: an entry in the recent list whose folder is out of reach (an
/// override reset between runs) says so, judged without touching the file; a
/// database in reach is unchanged.
#[test]
fn the_chooser_marks_a_recent_database_out_of_reach_instead_of_not_found() {
    let world = World::new();
    let outside = world.make_db(&world.outside(), "Theirs", true);
    let read_only = world.make_db(&world.ro(), "Read only", true);
    let home = world.make_db(&world.home(), "Mine", true);
    let before = world.listing();

    let state = databases::chooser_state(&world.boxed, &world.machine, None, None, &world.reach);

    let state = serde_json::to_value(state).unwrap();
    let recent = state["recent"].as_array().unwrap();
    assert_eq!(recent.len(), 3);
    let entry = |path: &Path| {
        recent.iter().find(|e| e["path"] == json!(text(path))).unwrap_or_else(|| panic!("{path:?}"))
    };
    for out in [&outside, &read_only] {
        assert_eq!(entry(out)["available"], json!(false), "{out:?}");
        assert_eq!(entry(out)["unavailableReason"], json!("outOfReach"), "{out:?}");
    }
    assert_eq!(entry(&home)["available"], json!(true));
    assert!(entry(&home).get("unavailableReason").is_none(), "set only when out of reach");
    assert_eq!(world.listing(), before, "judged without touching any file");
}

/// 011 US1-8: pointing a recent entry at a file out of reach is refused, and
/// the entry stays where it was.
#[test]
fn locate_database_to_a_path_out_of_reach_is_refused() {
    let world = World::new();
    let home = world.make_db(&world.home(), "Mine", true);
    for new_path in [world.outside().join("Moved.hoplodex"), world.ro().join("Moved.hoplodex")] {
        let refused = databases::locate_database(
            &world.machine,
            &text(&home),
            &text(&new_path),
            &world.reach,
        )
        .unwrap_err();

        assert_out_of_reach(&refused, new_path.parent().unwrap());
        assert!(world.machine.recent_entry(&home).is_some(), "the entry is where it was");
        assert!(world.machine.recent_entry(&new_path).is_none());
    }
}

// ============================================================== backup location

/// 011 US1-8, 011 FR-004: a custom backup location out of reach is refused on
/// the location field when it is saved, and nothing is saved or made.
#[test]
fn update_backup_settings_to_a_location_out_of_reach_is_a_validation_error_on_the_location() {
    let world = World::new();
    let path = world.make_db(&world.home(), "Mine", false);
    world.open_boxed_ok(&path);
    let before = world.listing();
    for location in [world.outside().join("Backups"), world.ro().join("Backups")] {
        let refused =
            world.update_backups(BackupLocationInput::Custom { path: text(&location) }, None);

        let refused = refused.unwrap_err();
        assert_eq!(refused.code, "VALIDATION_ERROR", "{refused:?}");
        let on_field = refused.field_errors.as_ref().and_then(|fields| fields.get("location"));
        let on_field = on_field.unwrap_or_else(|| panic!("no fieldErrors.location: {refused:?}"));
        assert!(
            on_field.starts_with("This copy of HoploDex is a Flatpak"),
            "the out-of-reach message: {on_field}"
        );
        assert!(
            on_field.contains(&format!(
                "flatpak override --user --filesystem={}",
                shell_word(&location)
            )),
            "{on_field}"
        );
        assert_eq!(world.listing(), before, "nothing saved or made");
    }
}

/// 011 US1-8: with the existing backups to move, the same refusal is
/// `BACKUP_LOCATION_UNAVAILABLE` with the reason `outOfReach`, and the backups
/// stay where they are.
#[test]
fn update_backup_settings_move_to_a_location_out_of_reach_leaves_the_backups_where_they_are() {
    let world = World::new();
    let path = world.make_db(&world.home(), "Mine", true);
    world.open_boxed_ok(&path);
    let old = world.home().join("HoploDex backups");
    assert_eq!(names_in(&old).len(), 1, "a backup to move");
    let before = world.listing();
    let location = world.outside().join("Backups");

    let refused = world
        .update_backups(
            BackupLocationInput::Custom { path: text(&location) },
            Some(ExistingBackupsChoice::Move),
        )
        .unwrap_err();

    assert_backup_out_of_reach(&refused, &location);
    assert_eq!(world.listing(), before, "the backups were not moved, nothing was made");
    let settings = databases::database_status(&world.boxed, &world.machine).unwrap();
    let settings = serde_json::to_value(settings).unwrap();
    assert_eq!(settings["settings"]["backups"]["location"]["kind"], json!("default"));
}

/// 011 US1-8, 011 research.md §5: a custom backup location that is out of
/// reach (set under `Unconfined`, then reopened under the Flatpak) is reported
/// as not available, with the reason, in the settings and the backup list; it
/// is not an error.
#[test]
fn the_settings_and_the_backup_list_say_the_location_is_out_of_reach() {
    let world = World::new();
    let (path, backups) = world.make_db_backing_up_outside();
    world.open_boxed_ok(&path);
    let before = world.listing();

    let status = databases::database_status(&world.boxed, &world.machine).unwrap();
    let list = backups_ops::list_backups(
        &world.boxed,
        &world.machine,
        Target::Open(world.boxed.current_id()),
    )
    .unwrap();

    let location =
        serde_json::to_value(&status).unwrap()["settings"]["backups"]["location"].clone();
    assert_eq!(location["kind"], json!("custom"));
    assert_eq!(location["path"], json!(text(&backups)));
    assert_eq!(location["available"], json!(false));
    assert_eq!(location["unavailableReason"], json!("outOfReach"));
    let list = serde_json::to_value(&list).unwrap();
    assert_eq!(list["folder"], json!(text(&backups)));
    assert_eq!(list["available"], json!(false));
    assert_eq!(list["unavailableReason"], json!("outOfReach"));
    assert_eq!(world.listing(), before);
}

/// 011 US1-8, 011 FR-004: the close's backup, with the custom location out of
/// reach, fails with the reason `outOfReach` and leaves no partial backup; the
/// close itself goes ahead.
#[test]
fn closing_with_a_backup_location_out_of_reach_fails_the_backup_and_leaves_no_partial() {
    let world = World::new();
    let (path, backups) = world.make_db_backing_up_outside();
    world.open_boxed_ok(&path);
    world.change(&world.boxed);
    let before = snapshot(&world.outside());

    let outcome = world.close(&world.boxed);

    assert_eq!(outcome, json!({ "backup": "failed", "failureReason": "outOfReach" }));
    assert!(!world.boxed.is_open(), "the close went ahead");
    assert_eq!(snapshot(&world.outside()), before, "no partial backup in {}", backups.display());
}

/// 011 US1-8: the backup before a restore stops the restore, with the database
/// file unchanged.
#[test]
fn restore_backup_stops_when_the_before_backup_location_is_out_of_reach() {
    let world = World::new();
    let (path, backups) = world.make_db_backing_up_outside();
    let backup = backups.join(&names_in(&backups)[0]);
    world.open_boxed_ok(&path);
    let before = world.listing();

    let refused = backups_ops::restore_backup(
        &world.boxed,
        &world.machine,
        &text(&backup),
        &passphrase(),
        Target::Open(world.boxed.current_id()),
        false,
        &world.reach,
    )
    .unwrap_err();

    assert_backup_out_of_reach(&refused, &backups);
    assert_eq!(world.listing(), before, "the database and the folders are as they were");
}

/// 011 US1-8 (011 contracts/tauri-commands.md, last paragraph of "Commands that
/// check reach"): `change_passphrase` acts on the open database's own path, which
/// `open_database` already found within reach, and makes no backup in this
/// build, so a backup location out of reach does not stop it and nothing is
/// written there. (The contract's table lists it among the "before" backups; the
/// code has none, so this test asserts the paragraph, not the table.)
#[test]
fn change_passphrase_is_not_stopped_by_a_backup_location_out_of_reach() {
    let world = World::new();
    let (path, _backups) = world.make_db_backing_up_outside();
    world.open_boxed_ok(&path);
    let outside = snapshot(&world.outside());

    backups_ops::change_passphrase(
        &world.boxed,
        world.boxed.current_id(),
        &world.machine,
        &world.scratch(),
        &passphrase(),
        &Passphrase::from_input(OTHER_PASSPHRASE.to_owned()),
    )
    .unwrap();

    assert_eq!(snapshot(&world.outside()), outside, "nothing written out of reach");
    assert!(names_in(&world.scratch()).is_empty(), "no scratch file left");
}

/// 011 US1-8: the backup before an upgrade stops it with the question still
/// waiting, with the database file unchanged. Both when the folder is the
/// database's own custom one and when the user chose a folder out of reach for
/// this one backup (assumed: 011 contracts/tauri-commands.md says "the
/// database's backup folder when custom", and the chosen folder is the backup
/// folder for that upgrade).
#[test]
fn upgrade_database_stops_when_the_before_backup_location_is_out_of_reach() {
    let world = World::new();
    let (path, backups) = world.make_db_backing_up_outside();
    let _scope = testing::scope(&world.home()).extra_steps(&[STEP_A]);
    let asked = world.open_boxed(&text(&path)).unwrap_err();
    assert_eq!(asked.code, "UPGRADE_NEEDED", "{asked:?}");
    let awaiting_id = details(&asked)["awaitingId"].as_u64().expect("an awaitingId");
    let before = world.listing();

    let refused =
        databases::upgrade_database(&world.boxed, &world.machine, awaiting_id, None).unwrap_err();
    assert_backup_out_of_reach(&refused, &backups);
    assert_eq!(world.listing(), before, "the database as it was, no partial backup");

    let chosen = world.outside().join("Elsewhere");
    let refused = databases::upgrade_database(
        &world.boxed,
        &world.machine,
        awaiting_id,
        Some(&text(&chosen)),
    )
    .unwrap_err();
    assert_backup_out_of_reach(&refused, &chosen);
    assert_eq!(world.listing(), before);
    databases::cancel_upgrade(&world.boxed, awaiting_id).unwrap();
}

// ============================================================== restore_backup

/// 011 US1-8: a backup file that does not exist in the sandbox and lies outside
/// reach (a path that would be on the host) is out of reach, not "not found";
/// a missing one inside the writable grant keeps the existing "not found".
#[test]
fn restore_backup_of_a_file_that_would_be_on_the_host_is_out_of_reach_not_missing() {
    let world = World::new();
    let path = world.make_db(&world.home(), "Mine", true);
    world.open_boxed_ok(&path);
    let before = world.listing();
    let restore = |backup: &Path| {
        backups_ops::restore_backup(
            &world.boxed,
            &world.machine,
            &text(backup),
            &passphrase(),
            Target::Open(world.boxed.current_id()),
            false,
            &world.reach,
        )
        .unwrap_err()
    };

    let outside = world.outside().join("Mine 2026-10-18 090000 abcd1234.hoplodex");
    assert_out_of_reach(&restore(&outside), &world.outside());
    assert_eq!(world.listing(), before, "nothing made before the refusal");

    let gone = world.home().join("gone.hoplodex");
    // (Not compared with a listing: the existing check of the backup folder
    // makes and removes a probe file in it, as it always has.)
    assert_eq!(restore(&gone).code, "NOT_FOUND");
}

/// 011 US1-8 (011 data-model.md "Need": `check_backup` reads the backup file): the
/// look at a backup before a restore refuses one that would be on the host the
/// same way, and reads nothing. (Added with T044-T048: the contract's table lists
/// `check_backup` for `available`, but `BackupCheck` has no such field, so only the
/// file check applies.)
#[test]
fn check_backup_of_a_file_that_would_be_on_the_host_is_out_of_reach() {
    let world = World::new();
    let path = world.make_db(&world.home(), "Mine", true);
    world.open_boxed_ok(&path);
    let before = world.listing();

    let refused = backups_ops::check_backup(
        &world.boxed,
        &world.machine,
        &text(&world.outside().join("Mine 2026-10-18 090000 abcd1234.hoplodex")),
        &passphrase(),
        Target::Open(world.boxed.current_id()),
    )
    .unwrap_err();

    assert_out_of_reach(&refused, &world.outside());
    assert_eq!(world.listing(), before);
}

/// 011 US1-8: restoring a database that doesn't open, with nothing open, whose
/// folder is now out of reach (an override reset between runs), is refused
/// naming the database's folder, before the "before" backup or any scratch
/// file, with the database file unchanged. The backup file itself is within
/// reach.
#[test]
fn restore_backup_with_nothing_open_checks_the_database_path_before_anything() {
    let world = World::new();
    let database = world.make_db(&world.outside(), "Theirs", true);
    let backups = world.outside().join("HoploDex backups");
    let in_reach = world.home().join("the backup.hoplodex");
    fs::copy(backups.join(&names_in(&backups)[0]), &in_reach).unwrap();
    assert!(!world.boxed.is_open());
    let bytes = fs::read(&database).unwrap();
    let before = world.listing();

    let refused = backups_ops::restore_backup(
        &world.boxed,
        &world.machine,
        &text(&in_reach),
        &passphrase(),
        Target::At(&text(&database)),
        false,
        &world.reach,
    )
    .unwrap_err();

    assert_out_of_reach(&refused, &world.outside());
    assert_eq!(fs::read(&database).unwrap(), bytes, "the database file is unchanged");
    assert_eq!(world.listing(), before, "no \"before\" backup, no scratch file");
    assert!(!world.boxed.is_open());
}

// ============================================================== export and import

fn db_with_a_firearm() -> (TestDb, ExportRecords) {
    let db = TestDb::new();
    let firearm =
        firearms::create_firearm(&db.conn, &support::firearm("Glock", "19", "EX-1"), false, None)
            .unwrap();
    (db, support::firearm_records(&[firearm.id]))
}

fn export_to(
    db: &TestDb,
    records: &ExportRecords,
    folder: &Path,
    reach: &Reach,
) -> Result<hoplodex_lib::commands::import_export::ExportResult, CommandError> {
    import_export_ops::export_collection(
        &db.conn,
        folder,
        "export",
        SpreadsheetFormat::Csv,
        records,
        &mut |_, _| {},
        reach,
    )
}

/// 011 US1-8, 011 SC-003: an export to a folder out of reach, or one that is
/// read only, is refused before any file or scratch file is made.
#[test]
fn export_collection_to_a_folder_out_of_reach_is_refused_before_any_file_is_made() {
    let world = World::new();
    let (db, records) = db_with_a_firearm();
    for folder in [world.outside(), world.outside().join("Exports"), world.ro()] {
        let before = world.listing();

        let refused = export_to(&db, &records, &folder, &world.reach).unwrap_err();

        assert_out_of_reach(&refused, &folder);
        assert_eq!(
            world.listing(),
            before,
            "{}: no file, photos folder or scratch",
            folder.display()
        );
    }
}

/// 011 US1-8: the same export into the writable grant is made.
#[test]
fn export_collection_into_the_writable_grant_is_made() {
    let world = World::new();
    let (db, records) = db_with_a_firearm();

    let result = export_to(&db, &records, &world.home(), &world.reach).unwrap();

    assert!(result.spreadsheet_path.starts_with(world.home()));
    assert!(result.spreadsheet_path.is_file());
}

fn import_file(path: &Path) -> ImportFile {
    ImportFile { file_path: path.to_path_buf(), format: SpreadsheetFormat::Csv }
}

impl World {
    /// `read_import_files` as `import_collection` calls it, with `helper` as the
    /// import helper's program.
    fn read_import(
        &self,
        helper: &Path,
        files: &[ImportFile],
    ) -> Result<Vec<hoplodex_lib::services::spreadsheet::ReadTable>, CommandError> {
        let operation = self.boxed.operations().begin(OperationKind::Import, None).unwrap();
        import_export_ops::read_import_files(
            helper,
            files,
            &operation,
            &|| true,
            &mut |_, _, _| {},
            &self.reach,
        )
    }
}

/// 011 US1-8, 011 FR-004: an import file that does not exist in the sandbox and
/// lies outside reach is out of reach, not "could not read the file", and the
/// check comes before the import helper starts: the helper's program here does
/// not exist, so starting it would fail differently, and neither file is read.
#[test]
fn an_import_file_that_would_be_on_the_host_is_out_of_reach_before_the_helper_starts() {
    let world = World::new();
    let no_helper = world.tmp.path().join("no such helper");
    let before = world.listing();
    let gone_outside = world.outside().join("collection.csv");
    let gone_in_ro = world.ro().join("collection.csv");

    let refused = world.read_import(&no_helper, &[import_file(&gone_outside)]).unwrap_err();
    assert_out_of_reach(&refused, &world.outside());

    // One of two files out of reach refuses the whole import, naming it.
    let refused = world
        .read_import(&no_helper, &[import_file(&gone_in_ro), import_file(&gone_outside)])
        .unwrap_err();
    assert_out_of_reach(&refused, &world.outside());

    assert_eq!(world.listing(), before);
}

/// 011 US1-8: a readable file in the read-only grant is read and imported. A
/// file only needs to be readable, so read-only is enough.
#[test]
fn an_existing_import_file_in_the_read_only_grant_is_imported() {
    let world = World::new();
    let file = world.ro().join("collection.csv");
    fs::write(&file, support::csv_file(&[support::csv_firearm("Colt", "Python", "RO-1", &[])]))
        .unwrap();
    let helper = Path::new(env!("CARGO_BIN_EXE_hoplodex"));

    let tables = world.read_import(helper, &[import_file(&file)]).unwrap();

    let db = TestDb::new();
    let result = import_export_ops::import_collection(
        &db.conn,
        tables,
        &ImportSessionStore::new(),
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(result.imported_count, 1);
}

// ============================================================== photos and documents

fn db_with_a_record() -> (TestDb, RecordRef) {
    let db = TestDb::new();
    let firearm = firearms::create_firearm(
        &db.conn,
        &support::firearm("Colt", "Python", "AT-1"),
        false,
        None,
    )
    .unwrap();
    (db, RecordRef::Firearm(firearm.id))
}

/// 011 US1-5, 011 FR-007: a dropped photo arrives as a host path; one that
/// doesn't exist in the sandbox and lies outside reach is refused, and nothing
/// is attached.
#[test]
fn a_dropped_photo_that_would_be_on_the_host_is_refused_and_nothing_is_attached() {
    let world = World::new();
    let (db, owner) = db_with_a_record();
    let before = world.listing();

    let refused = photo_ops::add_photo_from_path(
        &db.conn,
        owner,
        &world.outside().join("dropped.png"),
        &world.reach,
    )
    .unwrap_err();

    assert_out_of_reach(&refused, &world.outside());
    assert!(photo_ops::list_photos(&db.conn, owner).unwrap().is_empty(), "no row added");
    assert_eq!(world.listing(), before);
}

/// 011 US1-5, 011 FR-007: and a photo that exists in the read-only grant is
/// attached (a file only needs to be readable).
#[test]
fn a_photo_in_the_read_only_grant_is_attached() {
    let world = World::new();
    let (db, owner) = db_with_a_record();
    let file = world.ro().join("range day.png");
    fs::write(&file, sample_png_bytes()).unwrap();

    let photo = photo_ops::add_photo_from_path(&db.conn, owner, &file, &world.reach).unwrap();

    assert_eq!(photo.original_filename, "range day.png");
    assert_eq!(photo_ops::list_photos(&db.conn, owner).unwrap().len(), 1);
}

/// 011 US1-5, 011 FR-007: documents, the same two ways.
#[test]
fn a_document_that_would_be_on_the_host_is_refused_and_one_in_the_read_only_grant_is_attached() {
    let world = World::new();
    let (db, owner) = db_with_a_record();
    let before = world.listing();

    let refused = document_ops::add_document_from_path(
        &db.conn,
        owner,
        &world.outside().join("bill of sale.pdf"),
        &world.reach,
    )
    .unwrap_err();

    assert_out_of_reach(&refused, &world.outside());
    assert!(document_ops::list_documents(&db.conn, owner).unwrap().is_empty());
    assert_eq!(world.listing(), before);

    let file = world.ro().join("receipt.pdf");
    fs::write(&file, document_fixture("three-pages.pdf")).unwrap();
    let document =
        document_ops::add_document_from_path(&db.conn, owner, &file, &world.reach).unwrap();
    assert_eq!(document.original_filename, "receipt.pdf");
    assert_eq!(document_ops::list_documents(&db.conn, owner).unwrap().len(), 1);
}

/// 011 US1-5: a file the sandbox can see is readable wherever it is, so one in
/// `outside` that exists (a mounted drive the grants don't name, say) is not
/// refused. Only a path the sandbox can't see is out of reach.
#[test]
fn a_photo_the_sandbox_can_see_is_attached_even_outside_the_grants() {
    let world = World::new();
    let (db, owner) = db_with_a_record();
    let file = world.outside().join("visible.png");
    fs::write(&file, sample_png_bytes()).unwrap();

    photo_ops::add_photo_from_path(&db.conn, owner, &file, &world.reach).unwrap();

    assert_eq!(photo_ops::list_photos(&db.conn, owner).unwrap().len(), 1);
}

// ============================================================== Documents-portal paths

/// 011 US1-4, 011 research.md §5: a database picked through the file chooser
/// arrives as a Documents-portal path whose host path is in the writable grant.
/// It opens from the host path, and the recent list, the status and the
/// backup's folder name the host path.
#[cfg(unix)]
#[test]
fn a_portal_path_whose_host_path_is_within_reach_opens_from_the_host_path() {
    let mut world = World::new();
    let host = world.make_db(&world.home(), "Mine", true);
    let picked = portal("t038a", "Mine.hoplodex");
    world.reach_for_portal(&picked, &host);

    let status = world.open_boxed_ok(&picked);

    assert_eq!(status.path, text(&host), "the host path, not the portal's");
    assert_eq!(world.machine.recent()[0].path, host);
    assert!(world.machine.recent_entry(&picked).is_none());
    world.change(&world.boxed);
    assert_eq!(world.close(&world.boxed)["backup"], json!("made"));
    assert_eq!(names_in(&world.home().join("HoploDex backups")).len(), 2, "beside the host file");
}

// Used by the portal tests, which are Unix-only.
#[allow(dead_code)]
impl World {
    /// Rebuilds the Flatpak side of the world with a Documents-portal lookup
    /// that maps `picked` to `host`, keeping the same grants, folders and
    /// clock.
    fn reach_for_portal(&mut self, picked: &Path, host: &Path) {
        let home = self.home();
        self.reach = Arc::new(Reach::flatpak(
            self.reach.grants().to_vec(),
            Vec::new(),
            home,
            Arc::new(FixedHostPaths(HashMap::from([(picked.to_owned(), host.to_owned())]))),
        ));
        let (boxed, _) = support::test_session_reaching(
            &self.config.path().join("opened-documents"),
            self.clock.clone(),
            self.reach.clone(),
        );
        self.boxed = boxed;
    }
}

/// 011 US1-8, 011 research.md §5: a portal path whose host path is outside every
/// grant is refused at open, as is one the portal can't resolve; the file is
/// untouched.
#[cfg(unix)]
#[test]
fn a_portal_path_whose_host_path_is_out_of_reach_is_refused_at_open() {
    let mut world = World::new();
    let host = world.make_db(&world.outside(), "Theirs", true);
    let picked = portal("t038b", "Theirs.hoplodex");
    world.reach_for_portal(&picked, &host);
    let bytes = fs::read(&host).unwrap();
    let before = world.listing();

    let refused = world.open_boxed(&text(&picked)).unwrap_err();
    assert_eq!(refused.code, "LOCATION_OUT_OF_REACH", "{refused:?}");

    let unresolved = portal("t038c", "Nobody knows.hoplodex");
    let refused = world.open_boxed(&text(&unresolved)).unwrap_err();
    assert_eq!(refused.code, "LOCATION_OUT_OF_REACH", "{refused:?}");

    assert_eq!(fs::read(&host).unwrap(), bytes);
    assert_eq!(world.listing(), before);
    assert!(!world.boxed.is_open());
}

/// 011 US1-8, 011 research.md §5: an export folder and an import file given as
/// portal paths outside reach are used as portal paths (writing and reading
/// inside the portal's own folder works), whatever the lookup says.
#[cfg(unix)]
#[test]
fn portal_paths_for_an_export_folder_and_an_import_file_are_used_as_they_are() {
    let mut world = World::new();
    let folder = portal("t038d", "Exports");
    let file = portal("t038e", "collection.csv");
    // One resolves to a host path outside every grant, the other is unknown.
    world.reach_for_portal(&folder, &world.outside().join("Exports"));

    assert_eq!(world.reach.check(&folder, Need::WriteFolder).unwrap(), folder);
    assert_eq!(world.reach.check(&file, Need::ReadFile).unwrap(), file);

    // Through the ops: the file isn't there, which is not a reach problem, and
    // nothing is made anywhere.
    let before = world.listing();
    let no_helper = world.tmp.path().join("no such helper");
    let err = world.read_import(&no_helper, &[import_file(&file)]).unwrap_err();
    assert_ne!(err.code, "LOCATION_OUT_OF_REACH", "{err:?}");
    assert_eq!(world.listing(), before);
}

// ============================================================== Unconfined

/// 011 FR-004 (011 data-model.md "Reach"): outside a Flatpak nothing is checked,
/// and the same paths behave exactly as they did before. One representative call
/// of each command above (the existing tests cover the rest).
#[test]
fn unconfined_every_location_is_within_reach_and_nothing_changes() {
    let world = World::new();
    let reach = Reach::Unconfined;
    let fresh = world.outside().join("Guns");

    // create_database and open_database in `outside` and `ro`.
    let status = databases::create_database(
        &world.plain,
        &world.machine,
        &text(&fresh),
        "Mine",
        &passphrase(),
        true,
        &reach,
    )
    .unwrap();
    assert_eq!(status.path, text(&fresh.join("Mine.hoplodex")));
    world.change(&world.plain);
    world.close(&world.plain);
    let read_only = world.make_db(&world.ro(), "Read only", true);
    databases::open_database(
        &world.plain,
        &world.machine,
        &text(&read_only),
        Unlock::typed(&passphrase()),
        false,
        &reach,
    )
    .unwrap();

    // The chooser marks nothing unavailable for reach.
    let state = databases::chooser_state(&world.plain, &world.machine, None, None, &reach);
    let state = serde_json::to_value(state).unwrap();
    for entry in state["recent"].as_array().unwrap() {
        assert_eq!(entry["available"], json!(true), "{entry}");
        assert!(entry.get("unavailableReason").is_none(), "{entry}");
    }

    // A custom backup location in `outside`, and restore of a backup there.
    let backups = world.outside().join("Backups");
    fs::create_dir(&backups).unwrap();
    databases::update_backup_settings(
        &world.plain,
        world.plain.current_id(),
        &world.machine,
        &BackupSettingsInput {
            enabled: true,
            keep_count: 5,
            location: BackupLocationInput::Custom { path: text(&backups) },
            existing_backups: Some(ExistingBackupsChoice::Leave),
        },
        &reach,
    )
    .unwrap();
    let gone = backups_ops::restore_backup(
        &world.plain,
        &world.machine,
        &text(&world.outside().join("gone.hoplodex")),
        &passphrase(),
        Target::Open(world.plain.current_id()),
        false,
        &reach,
    )
    .unwrap_err();
    assert_eq!(gone.code, "NOT_FOUND", "{gone:?}");

    // locate_database to `outside`.
    databases::locate_database(
        &world.machine,
        &text(&read_only),
        &text(&world.outside().join("Located.hoplodex")),
        &reach,
    )
    .unwrap();

    // export, import, photo and document at paths that don't exist: the existing
    // errors, never the reach refusal.
    let (db, records) = db_with_a_firearm();
    let exported = export_to(&db, &records, &world.outside(), &reach).unwrap();
    assert!(exported.spreadsheet_path.is_file());
    let missing = world.outside().join("missing.csv");
    let no_helper = world.tmp.path().join("no such helper");
    let operation = world.plain.operations().begin(OperationKind::Import, None).unwrap();
    let err = import_export_ops::read_import_files(
        &no_helper,
        &[import_file(&missing)],
        &operation,
        &|| true,
        &mut |_, _, _| {},
        &reach,
    )
    .unwrap_err();
    assert_ne!(err.code, "LOCATION_OUT_OF_REACH", "{err:?}");
    let (db, owner) = db_with_a_record();
    let err =
        photo_ops::add_photo_from_path(&db.conn, owner, &world.outside().join("gone.png"), &reach)
            .unwrap_err();
    assert_eq!(err.code, "NOT_FOUND", "{err:?}");
    let err = document_ops::add_document_from_path(
        &db.conn,
        owner,
        &world.outside().join("gone.pdf"),
        &reach,
    )
    .unwrap_err();
    assert_eq!(err.code, "NOT_FOUND", "{err:?}");
}
