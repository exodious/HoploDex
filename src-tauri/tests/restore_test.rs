//! Restoring a database from a backup, including one that no longer opens
//! (003 FR-028, SC-004, SC-007; 003 research.md §8). Real SQLCipher files in temp
//! directories, a manual clock, and machine settings in a temp config
//! directory.

use crate::support;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::photos::ops as photos;
use hoplodex_lib::db::cipher;
use hoplodex_lib::db::upgrade::testing::{self, Point};
use hoplodex_lib::models::database::{
    BackupInfo, BackupLocationInput, BackupSettingsInput, CloseReason, DatabaseStatus,
    ExistingBackupsChoice,
};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::backups;
use hoplodex_lib::services::disk_space;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::sandbox::Reach;
use hoplodex_lib::session::scoped::Target;
use hoplodex_lib::session::{Session, SessionId, lifecycle};
use rusqlite::Connection;
use serde_json::json;
use support::{ManualClock, TestEvents, passphrase, peek, sample_png_bytes, test_session_at};
use tempfile::TempDir;

const START: &str = "2026-09-25T14:30:05+02:00";
/// The passphrase of a backup made before a (simulated) passphrase change.
const OLD_PASSPHRASE: &str = "the passphrase it used to have";

struct World {
    dir: TempDir,
    _config: TempDir,
    machine: MachineSettings,
    session: Session,
    events: Arc<TestEvents>,
    clock: Arc<ManualClock>,
}

impl World {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let clock = ManualClock::at(START);
        let (session, events) =
            test_session_at(&config.path().join("opened-documents"), clock.clone());
        Self {
            dir: TempDir::new().unwrap(),
            machine: MachineSettings::load(config.path()).unwrap(),
            _config: config,
            session,
            events,
            clock,
        }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("Mine.hoplodex")
    }

    fn default_folder(&self) -> PathBuf {
        self.dir.path().join("HoploDex backups")
    }

    fn open_with(&self, passphrase: &Passphrase) -> Result<(), CommandError> {
        lifecycle::open(&self.session, &self.machine, &self.path(), passphrase, false)
    }

    fn open(&self) {
        self.open_with(&passphrase()).unwrap();
    }

    fn close(&self) {
        lifecycle::close_normal(
            &self.session,
            self.session.current_id(),
            &self.machine,
            CloseReason::Closed,
        )
        .unwrap();
    }

    fn next_day(&self) {
        self.clock.advance(chrono::Duration::days(1));
    }

    /// Adds a firearm with a photo, a change with a blob in it.
    fn add_firearm(&self, serial: &str) {
        self.session
            .write(self.session.current_id(), |conn| {
                let firearm = firearms::create_firearm(
                    conn,
                    &support::firearm("Colt", "Python", serial),
                    false,
                    None,
                )?;
                photos::add_photo(
                    conn,
                    RecordRef::Firearm(firearm.id),
                    &sample_png_bytes(),
                    "front.png",
                    "image/png",
                )
                .map(|_| ())
            })
            .unwrap();
    }

    fn database_id(&self) -> String {
        self.session
            .inspect(self.session.current_id(), |open| Ok(open.database_id.clone()))
            .unwrap()
    }

    /// Creates "Mine" with one firearm, backs it up at close on the 25th,
    /// then adds a second firearm on the 26th, backed up at that close too,
    /// and leaves it open. Returns the backups, newest first.
    fn with_two_backups(&self) -> Vec<BackupInfo> {
        lifecycle::create(&self.session, &self.machine, &self.path(), &passphrase()).unwrap();
        self.add_firearm("A1");
        self.close();
        self.next_day();
        self.open();
        self.add_firearm("B2");
        self.close();
        self.clock.advance(chrono::Duration::hours(1));
        self.open();
        self.listed()
    }

    fn listed(&self) -> Vec<BackupInfo> {
        backups::list(&self.default_folder(), &self.database_id()).unwrap()
    }

    fn restore(&self, backup: &str, with: &str) -> Result<DatabaseStatus, CommandError> {
        backups_ops::restore_backup(
            &self.session,
            &self.machine,
            backup,
            &Passphrase::from_input(with.to_owned()),
            Target::Open(self.session.current_id()),
            false,
            &Reach::Unconfined,
        )
    }

    fn names(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Every file beside the database whose name marks it as temporary.
    fn leftovers(&self) -> Vec<String> {
        let mut found = Vec::new();
        for folder in [self.dir.path().to_owned(), self.default_folder()] {
            if folder.exists() {
                found.extend(
                    Self::names(&folder)
                        .into_iter()
                        .filter(|n| n.ends_with(".new") || n.ends_with(".partial")),
                );
            }
        }
        found
    }

    fn set_backups(&self, enabled: bool, keep_count: i64, location: BackupLocationInput) {
        // Backups already at an old location stay there, as before 003 FR-026
        // asked about them; backup_location_change_test covers the rest.
        let existing_backups = Some(ExistingBackupsChoice::Leave);
        databases::update_backup_settings(
            &self.session,
            self.session.current_id(),
            &self.machine,
            &BackupSettingsInput { enabled, keep_count, location, existing_backups },
            &Reach::Unconfined,
        )
        .unwrap();
    }
}

/// The collection as rows: every collection table, every column, in order.
/// Housekeeping (`app_state`, `pending_changes`) is left out.
fn collection(conn: &Connection) -> Vec<String> {
    let mut rows = Vec::new();
    for table in [
        "firearms",
        "photos",
        "document_attachments",
        "disposition_history",
        "insurance_policies",
        "firearm_types",
        "collection_settings",
    ] {
        let mut stmt = conn.prepare(&format!("SELECT * FROM {table} ORDER BY rowid")).unwrap();
        let columns = stmt.column_count();
        let found = stmt
            .query_map([], |row| {
                let values: Vec<String> =
                    (0..columns).map(|i| format!("{:?}", row.get_ref(i).unwrap())).collect();
                Ok(format!("{table}: {}", values.join(" | ")))
            })
            .unwrap();
        rows.extend(found.map(Result::unwrap));
    }
    rows
}

fn open_collection(world: &World) -> Vec<String> {
    world.session.read(world.session.current_id(), |conn| Ok(collection(conn))).unwrap()
}

// --- Restoring ---------------------------------------------------------------

#[test]
fn restoring_an_earlier_backup_reproduces_exactly_its_content() {
    let world = World::new();
    let listed = world.with_two_backups();
    let older = &listed[1];
    let expected = collection(&peek(Path::new(&older.path)));
    assert_ne!(open_collection(&world), expected);

    let status = world.restore(&older.path, support::TEST_PASSPHRASE).unwrap();

    assert_eq!(status.path, world.path().to_string_lossy());
    assert_eq!(open_collection(&world), expected, "003 SC-007");
    assert!(expected.iter().any(|row| row.starts_with("photos: ")), "blobs included");
}

#[test]
fn the_current_database_is_backed_up_first_even_after_todays_backup() {
    let world = World::new();
    let listed = world.with_two_backups();
    let current = open_collection(&world);

    world.restore(&listed[1].path, support::TEST_PASSPHRASE).unwrap();

    let after = world.listed();
    assert_eq!(after.len(), 3, "a before-restoring backup, whatever the once-a-day limit");
    assert_eq!(after[0].made_at, "2026-09-26T15:30:05");
    assert_eq!(collection(&peek(Path::new(&after[0].path))), current, "it can be undone");
}

#[test]
fn the_backup_restored_from_survives_the_rotation_that_follows() {
    let world = World::new();
    let listed = world.with_two_backups();
    world.set_backups(true, 1, BackupLocationInput::Default);

    world.restore(&listed[1].path, support::TEST_PASSPHRASE).unwrap();

    assert!(Path::new(&listed[1].path).exists(), "the backup restored from is kept");
    assert!(!Path::new(&listed[0].path).exists(), "rotation still ran");
    assert_eq!(world.listed().len(), 2);
}

#[test]
fn the_restored_database_has_nothing_waiting_no_stamp_and_its_identity() {
    let world = World::new();
    let listed = world.with_two_backups();
    let id = world.database_id();

    world.restore(&listed[1].path, support::TEST_PASSPHRASE).unwrap();

    assert_eq!(world.database_id(), id);
    let state: (bool, Option<String>, Option<String>, Option<String>) = world
        .session
        .read(world.session.current_id(), |conn| {
            conn.query_row(
                "SELECT changes_waiting, backup_made_at, backup_of_name, open_machine_id
                 FROM app_state",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(CommandError::from_db)
        })
        .unwrap();
    assert_eq!(state, (false, None, None, Some(world.machine.machine_id())));
    world.close();
    assert!(world.leftovers().is_empty());
}

/// Puts a backup of the open database made with `OLD_PASSPHRASE` into the
/// default folder, as if it had been made before a passphrase change.
fn backup_with_an_older_passphrase(world: &World) -> PathBuf {
    let id = world.database_id();
    fs::create_dir_all(world.default_folder()).unwrap();
    let path = world.default_folder().join(format!("Mine 2026-09-20 090000 {}.hoplodex", &id[..8]));
    // The app's connections can't create files, only attach existing ones.
    fs::write(&path, b"").unwrap();
    world
        .session
        .read(world.session.current_id(), |conn| {
            conn.execute(
                "ATTACH DATABASE ?1 AS rekey KEY ?2",
                [path.to_str().unwrap(), OLD_PASSPHRASE],
            )
            .unwrap();
            cipher::apply_cipher_settings(conn, "rekey").unwrap();
            conn.query_row("SELECT sqlcipher_export('rekey')", [], |_| Ok(())).unwrap();
            conn.execute_batch(
                "UPDATE rekey.app_state SET open_machine_id = NULL, open_machine_name = NULL,
                        open_since = NULL, backup_made_at = '2026-09-20T07:00:00Z',
                        backup_of_name = 'Mine';
                 DETACH DATABASE rekey;",
            )
            .unwrap();
            Ok(())
        })
        .unwrap();
    path
}

#[test]
fn the_restored_database_opens_with_the_passphrase_of_the_backup() {
    let world = World::new();
    world.with_two_backups();
    let old = backup_with_an_older_passphrase(&world);

    let status = world.restore(&old.to_string_lossy(), OLD_PASSPHRASE).unwrap();

    assert_eq!(status.notes.restored_with_passphrase_of.as_deref(), Some("2026-09-20T07:00:00Z"));
    world.close();
    let refused = world.open_with(&passphrase()).unwrap_err();
    assert_eq!(refused.code, "PASSPHRASE_INCORRECT");
    world.open_with(&Passphrase::from_input(OLD_PASSPHRASE.to_owned())).unwrap();
}

#[test]
fn a_wrong_backup_passphrase_changes_nothing() {
    let world = World::new();
    let listed = world.with_two_backups();
    let before = fs::read(world.path()).unwrap();
    let names = World::names(&world.default_folder());

    let refused = world.restore(&listed[1].path, "not the passphrase at all").unwrap_err();

    assert_eq!(refused.code, "PASSPHRASE_INCORRECT");
    assert_eq!(fs::read(world.path()).unwrap(), before);
    assert_eq!(World::names(&world.default_folder()), names);
    assert!(world.leftovers().is_empty());
    assert!(world.session.is_open(), "the current database stays open");
}

#[test]
fn a_file_planted_at_the_copys_name_fails_the_restore_and_is_left_alone() {
    // #63: the failure path removed whatever was at `.new`.
    let world = World::new();
    let listed = world.with_two_backups();
    let before = fs::read(world.path()).unwrap();
    let planted = world.dir.path().join(".Mine.hoplodex.new");
    fs::write(&planted, b"somebody else's file").unwrap();

    let refused = world.restore(&listed[1].path, support::TEST_PASSPHRASE).unwrap_err();

    assert_eq!(refused.code, "INTERNAL_ERROR");
    assert_eq!(fs::read(&planted).unwrap(), b"somebody else's file", "not truncated or removed");
    assert_eq!(fs::read(world.path()).unwrap(), before);
    assert!(world.session.is_open());
}

#[test]
fn a_file_planted_at_the_copys_name_fails_the_restore_of_a_damaged_database() {
    let world = World::new();
    let listed = damaged(&world);
    let planted = world.dir.path().join(".Mine.hoplodex.new");
    fs::write(&planted, b"somebody else's file").unwrap();
    let damaged_bytes = fs::read(world.path()).unwrap();

    let refused = backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &listed[0].path,
        &passphrase(),
        Target::At(&world.path().to_string_lossy()),
        false,
        &Reach::Unconfined,
    )
    .unwrap_err();

    assert_eq!(refused.code, "INTERNAL_ERROR");
    assert_eq!(fs::read(&planted).unwrap(), b"somebody else's file");
    assert_eq!(fs::read(world.path()).unwrap(), damaged_bytes, "the damaged file stays put");
}

/// Stops the restore at the first `restore:progress` of `phase` (after
/// some bytes, for `copying`), then checks the database is as it was, open,
/// and reopens with its old passphrase and content.
fn restore_stopped_in(phase: &'static str) {
    let world = World::new();
    let listed = world.with_two_backups();
    let current = open_collection(&world);
    let operations = world.session.operations_handle();
    world.events.on_event(move |event, payload| {
        if event == "restore:progress"
            && payload["phase"] == phase
            && (phase != "copying" || payload["processed"].as_u64().unwrap() > 0)
        {
            operations.stop_running();
        }
    });

    let stopped = world.restore(&listed[1].path, support::TEST_PASSPHRASE).unwrap_err();

    assert_eq!(stopped.code, "OPERATION_STOPPED");
    assert_eq!(stopped.details.as_deref().unwrap()["operation"], json!("restore"));
    assert!(world.leftovers().is_empty(), "no .new or .partial is left");
    assert_eq!(open_collection(&world), current);
    world.close();
    world.open();
    assert_eq!(open_collection(&world), current, "003 SC-004");
}

#[test]
fn a_restore_stopped_during_the_copy_changes_nothing() {
    restore_stopped_in("copying");
}

#[test]
fn a_restore_stopped_during_the_check_changes_nothing() {
    restore_stopped_in("checking");
}

#[test]
fn a_restore_stopped_while_backing_up_the_current_database_changes_nothing() {
    restore_stopped_in("savingCurrent");
}

#[test]
fn a_restore_stopped_just_before_the_replacement_changes_nothing() {
    restore_stopped_in("replacing");
}

// --- A damaged database (003 US3-6) -----------------------------------------

/// "Mine" with two backups, closed, then cut short so it no longer opens.
fn damaged(world: &World) -> Vec<BackupInfo> {
    let listed = world.with_two_backups();
    world.close();
    let bytes = fs::read(world.path()).unwrap();
    fs::write(world.path(), &bytes[..2 * 4096]).unwrap();
    listed
}

#[test]
fn a_damaged_database_says_whether_backups_are_available() {
    let world = World::new();
    damaged(&world);

    let refused = databases::open_database(
        &world.session,
        &world.machine,
        &world.path().to_string_lossy(),
        databases::Unlock::typed(&passphrase()),
        false,
        &Reach::Unconfined,
    )
    .unwrap_err();

    assert_eq!(refused.code, "DATABASE_DAMAGED");
    assert_eq!(refused.details.as_deref(), Some(&json!({ "backupsAvailable": true })));
    let wrong = databases::open_database(
        &world.session,
        &world.machine,
        &world.path().to_string_lossy(),
        databases::Unlock::typed(&Passphrase::from_input("not the passphrase at all".into())),
        false,
        &Reach::Unconfined,
    )
    .unwrap_err();
    assert_eq!(wrong.code, "PASSPHRASE_INCORRECT");
    assert_eq!(wrong.details.as_deref(), Some(&json!({ "backupsAvailable": true })));
}

#[test]
fn a_damaged_database_is_restored_and_kept_aside() {
    let world = World::new();
    let listed = damaged(&world);
    let damaged_bytes = fs::read(world.path()).unwrap();
    let expected = collection(&peek(Path::new(&listed[0].path)));
    let found = backups_ops::list_backups(
        &world.session,
        &world.machine,
        Target::At(&world.path().to_string_lossy()),
    )
    .unwrap();
    assert!(found.available);
    assert_eq!(found.backups, listed, "found from the recent entry's cached folder and id");

    let status = backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &listed[0].path,
        &passphrase(),
        Target::At(&world.path().to_string_lossy()),
        false,
        &Reach::Unconfined,
    )
    .unwrap();

    let kept = world.dir.path().join("Mine damaged 2026-09-26 153005.hoplodex");
    assert_eq!(status.notes.damaged_file_kept_at.as_deref(), Some(kept.to_str().unwrap()));
    assert_eq!(fs::read(&kept).unwrap(), damaged_bytes, "the damaged file is kept as it was");
    assert_eq!(open_collection(&world), expected);
    assert_eq!(world.listed().len(), 2, "no before-restoring backup of a file that won't open");
}

// --- Who and what a damaged restore may replace (issue #62) ------------------

impl World {
    /// Restores into `target`, which is whatever the web view sent.
    fn restore_into(&self, backup: &str, target: &Path) -> Result<DatabaseStatus, CommandError> {
        backups_ops::restore_backup(
            &self.session,
            &self.machine,
            backup,
            &passphrase(),
            Target::At(&target.to_string_lossy()),
            false,
            &Reach::Unconfined,
        )
    }
}

/// `damaged`, then the damaged file's bytes and what is beside it.
fn damaged_with_listing(world: &World) -> (Vec<BackupInfo>, Vec<u8>, Vec<String>) {
    let listed = damaged(world);
    (listed, fs::read(world.path()).unwrap(), World::names(world.dir.path()))
}

#[test]
fn issue_62_a_path_that_is_not_a_recent_database_is_never_replaced() {
    let world = World::new();
    let (listed, _, _) = damaged_with_listing(&world);
    let stranger = world.dir.path().join("Not a database.txt");
    fs::write(&stranger, b"precious").unwrap();
    let names = World::names(world.dir.path());

    let refused = world.restore_into(&listed[0].path, &stranger).unwrap_err();

    assert_eq!(refused.code, "NOT_FOUND");
    assert_eq!(fs::read(&stranger).unwrap(), b"precious");
    assert_eq!(World::names(world.dir.path()), names, "nothing was set aside or added");
}

#[test]
fn issue_62_a_recent_entry_located_onto_another_file_does_not_authorize_it() {
    let world = World::new();
    let (listed, _, _) = damaged_with_listing(&world);
    let stranger = world.dir.path().join("Not a database.txt");
    fs::write(&stranger, b"precious").unwrap();
    // The web view re-points Mine's entry, cached id and folder included,
    // at a file of its choosing.
    databases::locate_database(
        &world.machine,
        &world.path().to_string_lossy(),
        &stranger.to_string_lossy(),
        &Reach::Unconfined,
    )
    .unwrap();
    let names = World::names(world.dir.path());

    let refused = world.restore_into(&listed[0].path, &stranger).unwrap_err();

    assert_eq!(refused.code, "VALIDATION_ERROR");
    assert!(refused.field_errors.unwrap().contains_key("databasePath"));
    assert_eq!(fs::read(&stranger).unwrap(), b"precious");
    assert_eq!(World::names(world.dir.path()), names, "nothing was set aside or added");
}

#[test]
fn issue_62_a_located_database_is_restorable_once_it_has_opened_there() {
    let world = World::new();
    world.with_two_backups();
    world.close();
    let moved = world.dir.path().join("Moved.hoplodex");
    fs::rename(world.path(), &moved).unwrap();
    databases::locate_database(
        &world.machine,
        &world.path().to_string_lossy(),
        &moved.to_string_lossy(),
        &Reach::Unconfined,
    )
    .unwrap();
    databases::open_database(
        &world.session,
        &world.machine,
        &moved.to_string_lossy(),
        databases::Unlock::typed(&passphrase()),
        false,
        &Reach::Unconfined,
    )
    .unwrap();
    world.close();
    let listed = backups_ops::list_backups(
        &world.session,
        &world.machine,
        Target::At(&moved.to_string_lossy()),
    )
    .unwrap()
    .backups;
    let expected = collection(&peek(Path::new(&listed[0].path)));
    let bytes = fs::read(&moved).unwrap();
    fs::write(&moved, &bytes[..2 * 4096]).unwrap();

    let status = world.restore_into(&listed[0].path, &moved).unwrap();

    assert!(status.notes.damaged_file_kept_at.is_some());
    assert_eq!(open_collection(&world), expected);
}

#[test]
fn issue_62_a_backup_outside_the_cached_backup_folder_is_refused() {
    let world = World::new();
    let (listed, bytes, names) = damaged_with_listing(&world);
    let elsewhere = TempDir::new().unwrap();
    let copy = elsewhere.path().join(&listed[0].file_name);
    fs::copy(&listed[0].path, &copy).unwrap();

    let refused = world.restore_into(copy.to_str().unwrap(), &world.path()).unwrap_err();

    assert_eq!(refused.code, "VALIDATION_ERROR");
    assert!(refused.field_errors.unwrap().contains_key("backupPath"));
    assert_eq!(fs::read(world.path()).unwrap(), bytes);
    assert_eq!(World::names(world.dir.path()), names);
}

#[test]
fn issue_62_a_backup_of_another_database_is_refused_whatever_its_name_and_place() {
    let world = World::new();
    let listed = world.with_two_backups();
    world.close();
    // Another database whose backups share the folder, with the same
    // passphrase: it is only the id inside the backup that tells them apart.
    let other = world.dir.path().join("Other.hoplodex");
    lifecycle::create(&world.session, &world.machine, &other, &passphrase()).unwrap();
    let other_id = world.database_id();
    world.add_firearm("Z9");
    world.close();
    let theirs = backups::list(&world.default_folder(), &other_id).unwrap().remove(0);
    // Named as Mine's backup, in Mine's folder.
    let disguised = Path::new(&listed[0].path).with_file_name(&listed[1].file_name);
    fs::copy(&theirs.path, world.dir.path().join("scratch")).unwrap();
    fs::remove_file(&listed[1].path).unwrap();
    fs::rename(world.dir.path().join("scratch"), &disguised).unwrap();
    let bytes = fs::read(world.path()).unwrap();
    let names = World::names(world.dir.path());

    for backup in [theirs.path.as_str(), disguised.to_str().unwrap()] {
        let refused = world.restore_into(backup, &world.path()).unwrap_err();
        assert_eq!(refused.code, "VALIDATION_ERROR", "{backup}");
    }

    assert_eq!(fs::read(world.path()).unwrap(), bytes, "Mine is as it was");
    assert_eq!(World::names(world.dir.path()), names);
    assert!(world.leftovers().is_empty());
}

#[test]
fn issue_62_a_directory_at_the_target_is_refused_and_left_alone() {
    let world = World::new();
    let (listed, _, _) = damaged_with_listing(&world);
    fs::remove_file(world.path()).unwrap();
    fs::create_dir(world.path()).unwrap();
    fs::write(world.path().join("inside.txt"), b"mine").unwrap();
    let names = World::names(world.dir.path());

    let refused = world.restore_into(&listed[0].path, &world.path()).unwrap_err();

    assert_eq!(refused.code, "VALIDATION_ERROR");
    assert!(refused.field_errors.unwrap().contains_key("databasePath"));
    assert!(world.path().is_dir());
    assert_eq!(fs::read(world.path().join("inside.txt")).unwrap(), b"mine");
    assert_eq!(World::names(world.dir.path()), names);
}

#[cfg(unix)]
#[test]
fn issue_62_a_symlink_at_the_target_is_refused_and_not_followed() {
    let world = World::new();
    let (listed, _, _) = damaged_with_listing(&world);
    let outside = TempDir::new().unwrap();
    let victim = outside.path().join("victim.txt");
    fs::write(&victim, b"precious").unwrap();
    fs::remove_file(world.path()).unwrap();
    std::os::unix::fs::symlink(&victim, world.path()).unwrap();
    let names = World::names(world.dir.path());

    let refused = world.restore_into(&listed[0].path, &world.path()).unwrap_err();

    assert_eq!(refused.code, "VALIDATION_ERROR");
    assert_eq!(fs::read_link(world.path()).unwrap(), victim, "the link is as it was");
    assert_eq!(fs::read(&victim).unwrap(), b"precious");
    assert_eq!(World::names(world.dir.path()), names);
}

#[test]
fn issue_62_a_database_file_that_is_gone_is_put_back() {
    let world = World::new();
    let (listed, _, _) = damaged_with_listing(&world);
    let expected = collection(&peek(Path::new(&listed[0].path)));
    fs::remove_file(world.path()).unwrap();

    let status = world.restore_into(&listed[0].path, &world.path()).unwrap();

    assert_eq!(status.notes.damaged_file_kept_at, None, "there was nothing to keep");
    assert_eq!(open_collection(&world), expected);
}

// --- Preconditions and failures (003 FR-028, research.md §8) ----------------

fn assert_nothing_changed(world: &World, before: &[u8], names: &[String]) {
    assert_eq!(fs::read(world.path()).unwrap(), before, "the database is byte-identical");
    assert_eq!(World::names(&world.default_folder()), names);
    assert!(world.leftovers().is_empty());
    assert!(world.session.is_open());
}

#[test]
fn too_little_space_for_the_restored_copy_refuses_before_anything_is_written() {
    let world = World::new();
    let listed = world.with_two_backups();
    let before = fs::read(world.path()).unwrap();
    let names = World::names(&world.default_folder());
    let _space = disk_space::testing::fake_available_space(|_| Some(4096));

    let refused = world.restore(&listed[1].path, support::TEST_PASSPHRASE).unwrap_err();

    assert_eq!(refused.code, "INSUFFICIENT_SPACE");
    assert_nothing_changed(&world, &before, &names);
}

#[test]
fn the_space_for_both_copies_is_summed_on_one_volume() {
    let world = World::new();
    let listed = world.with_two_backups();
    let before = fs::read(world.path()).unwrap();
    let names = World::names(&world.default_folder());
    let backup_len = listed[1].size_bytes;
    let current_len = before.len() as u64;
    // Enough for either copy alone, not for both.
    let enough_for_one =
        disk_space::room_for_copy(backup_len).max(disk_space::room_for_copy(current_len));
    let _space = disk_space::testing::fake_available_space(move |_| Some(enough_for_one));

    let refused = world.restore(&listed[1].path, support::TEST_PASSPHRASE).unwrap_err();

    assert_eq!(refused.code, "INSUFFICIENT_SPACE");
    let details = refused.details.as_deref().unwrap();
    assert_eq!(
        details["bytesNeeded"],
        json!(disk_space::room_for_copy(backup_len) + disk_space::room_for_copy(current_len))
    );
    assert_nothing_changed(&world, &before, &names);
}

#[test]
fn a_missing_backup_location_refuses_before_anything_is_written() {
    let world = World::new();
    let listed = world.with_two_backups();
    let gone = world.dir.path().join("Unplugged/Backups");
    world.set_backups(true, 5, BackupLocationInput::Custom { path: gone.to_string_lossy().into() });
    let before = fs::read(world.path()).unwrap();
    let names = World::names(&world.default_folder());

    let refused = world.restore(&listed[1].path, support::TEST_PASSPHRASE).unwrap_err();

    assert_eq!(refused.code, "BACKUP_LOCATION_UNAVAILABLE");
    assert_eq!(
        refused.details.as_deref(),
        Some(&json!({ "path": gone.to_string_lossy(), "reason": "missing" }))
    );
    assert_nothing_changed(&world, &before, &names);
}

#[cfg(unix)]
#[test]
fn an_unwritable_backup_location_refuses_before_anything_is_written() {
    let world = World::new();
    let listed = world.with_two_backups();
    let locked = world.dir.path().join("Read only");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    world.set_backups(
        true,
        5,
        BackupLocationInput::Custom { path: locked.to_string_lossy().into() },
    );
    let before = fs::read(world.path()).unwrap();
    let names = World::names(&world.default_folder());

    let refused = world.restore(&listed[1].path, support::TEST_PASSPHRASE).unwrap_err();

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(refused.code, "BACKUP_LOCATION_UNAVAILABLE");
    assert_eq!(refused.details.as_deref().unwrap()["reason"], json!("notWritable"));
    assert_nothing_changed(&world, &before, &names);
}

#[test]
fn with_backups_off_the_current_database_is_still_backed_up_first() {
    let world = World::new();
    let listed = world.with_two_backups();
    world.set_backups(false, 5, BackupLocationInput::Default);

    world.restore(&listed[1].path, support::TEST_PASSPHRASE).unwrap();

    assert_eq!(world.listed().len(), 3);
}

#[cfg(unix)]
#[test]
fn a_failed_backup_of_the_current_database_cancels_the_restore() {
    let world = World::new();
    let listed = world.with_two_backups();
    let before = fs::read(world.path()).unwrap();
    let names = World::names(&world.default_folder());
    let folder = world.default_folder();
    world.events.on_event(move |event, payload| {
        // The location passed its check, then became unwritable.
        if event == "restore:progress" && payload["phase"] == "savingCurrent" {
            fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
        }
    });

    let refused = world.restore(&listed[1].path, support::TEST_PASSPHRASE).unwrap_err();

    fs::set_permissions(world.default_folder(), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(refused.code, "RESTORE_CANCELLED");
    assert_nothing_changed(&world, &before, &names);
}

#[test]
fn restoring_a_damaged_database_checks_only_the_space_for_the_restored_copy() {
    let world = World::new();
    let listed = damaged(&world);
    let needed = disk_space::room_for_copy(listed[0].size_bytes);
    let _space = disk_space::testing::fake_available_space(move |_| Some(needed));

    backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &listed[0].path,
        &passphrase(),
        Target::At(&world.path().to_string_lossy()),
        false,
        &Reach::Unconfined,
    )
    .unwrap();

    assert!(world.session.is_open());
}

// --- A backup in an earlier layout (009 FR-019; 009 research.md §9) ----------
//
// Assumed API (commands::backups::ops, 009 T051): `restore_backup` takes
// `upgrade: bool` as its last argument, and `check_backup(session, machine,
// backup_path, passphrase, target)` takes the same `Target` as `restore_backup`
// and returns a value whose `upgrade_to` is `Option<String>`.

/// A step the build gains after the backups below are made: a database or a
/// backup made without it is in an earlier layout, and one that has it is
/// current (009 research.md §14).
const NEW_STEP: (&str, &str) =
    ("9001_test_restore", "CREATE TABLE restore_upgrade_check (id INTEGER PRIMARY KEY);");

/// The registrations that make [`NEW_STEP`] part of the build for the folders
/// the restore reads from and writes to, until it is dropped. The backup's
/// folder is registered too, so it does not matter whether the backend works
/// out the steps due from the database's path or the backup's.
struct BuildGainsAStep {
    database_folder: testing::Scope,
    _backup_folder: testing::Scope,
}

impl BuildGainsAStep {
    fn new(world: &World) -> Self {
        Self {
            database_folder: testing::scope(world.dir.path()).extra_steps(&[NEW_STEP]),
            _backup_folder: testing::scope(&world.default_folder()).extra_steps(&[NEW_STEP]),
        }
    }

    /// As [`BuildGainsAStep::new`], with the upgrade of a database failing
    /// after its first step.
    fn failing(world: &World) -> Self {
        let step = Self::new(world);
        Self { database_folder: step.database_folder.fail_at(Point::AfterStep(1)), ..step }
    }
}

impl World {
    fn restore_with(
        &self,
        backup: &str,
        with: &str,
        upgrade: bool,
    ) -> Result<DatabaseStatus, CommandError> {
        backups_ops::restore_backup(
            &self.session,
            &self.machine,
            backup,
            &Passphrase::from_input(with.to_owned()),
            Target::Open(self.session.current_id()),
            upgrade,
            &Reach::Unconfined,
        )
    }

    /// `check_backup` for the open database's backup.
    fn check(&self, backup: &str, with: &str) -> Result<Option<String>, CommandError> {
        self.check_for(backup, with, Target::Open(self.session.current_id()))
    }

    fn check_for(
        &self,
        backup: &str,
        with: &str,
        target: Target<'_>,
    ) -> Result<Option<String>, CommandError> {
        backups_ops::check_backup(
            &self.session,
            &self.machine,
            backup,
            &Passphrase::from_input(with.to_owned()),
            target,
        )
        .map(|checked| checked.upgrade_to)
    }

    /// The phases of the restore's `restore:progress`, in order, each once.
    fn restore_phases(&self) -> Vec<String> {
        let mut phases: Vec<String> = self
            .events
            .payloads("restore:progress")
            .iter()
            .map(|payload| payload["phase"].as_str().unwrap().to_owned())
            .collect();
        phases.dedup();
        phases
    }

    /// The names of the steps the open database has applied.
    fn applied_steps(&self) -> Vec<String> {
        self.session
            .read(self.session.current_id(), |conn| {
                let mut stmt = conn
                    .prepare("SELECT name FROM schema_migrations ORDER BY name")
                    .map_err(CommandError::from_db)?;
                let names = stmt
                    .query_map([], |row| row.get(0))
                    .map_err(CommandError::from_db)?
                    .collect::<Result<_, _>>()
                    .map_err(CommandError::from_db)?;
                Ok(names)
            })
            .unwrap()
    }
}

/// Records a step this build does not know in a backup, as a newer version's
/// upgrade would have.
fn make_newer(backup: &str) {
    let conn = Connection::open(backup).expect("could not open the backup");
    conn.pragma_update(None, "key", support::TEST_PASSPHRASE).unwrap();
    cipher::apply_cipher_settings(&conn, "main").unwrap();
    conn.execute(
        "INSERT INTO schema_migrations (name, applied_at) VALUES ('0099_from_the_future', 'x')",
        [],
    )
    .unwrap();
}

/// What a closed world holds: the database's bytes and the names beside it and
/// in its backup folder.
fn snapshot(world: &World) -> (Vec<u8>, Vec<String>, Vec<String>) {
    let backup_names = if world.default_folder().exists() {
        World::names(&world.default_folder())
    } else {
        vec![]
    };
    (fs::read(world.path()).unwrap_or_default(), World::names(world.dir.path()), backup_names)
}

// 009 FR-019
#[test]
fn check_backup_says_what_an_older_backup_will_be_upgraded_for_and_changes_nothing() {
    let world = World::new();
    let listed = world.with_two_backups();
    let _gains = BuildGainsAStep::new(&world);
    let before = snapshot(&world);
    let backup_bytes = fs::read(&listed[1].path).unwrap();

    let upgrade_to = world.check(&listed[1].path, support::TEST_PASSPHRASE).unwrap();

    assert_eq!(upgrade_to.as_deref(), Some(hoplodex_lib::APP_VERSION));
    assert_eq!(fs::read(&listed[1].path).unwrap(), backup_bytes, "the backup is read, not written");
    assert_eq!(snapshot(&world), before, "no copy is made and nothing else is written");
    assert!(world.leftovers().is_empty());
    assert!(world.session.is_open());
}

// 009 FR-019
#[test]
fn check_backup_gives_null_for_a_backup_in_the_current_layout() {
    let world = World::new();
    let listed = world.with_two_backups();
    let backup_bytes = fs::read(&listed[0].path).unwrap();

    let upgrade_to = world.check(&listed[0].path, support::TEST_PASSPHRASE).unwrap();

    assert_eq!(upgrade_to, None);
    assert_eq!(fs::read(&listed[0].path).unwrap(), backup_bytes);
}

// 009 FR-019
#[test]
fn check_backup_refuses_a_wrong_passphrase_a_newer_backup_and_a_missing_one() {
    let world = World::new();
    let listed = world.with_two_backups();
    let before = snapshot(&world);

    let wrong = world.check(&listed[1].path, "not the passphrase at all").unwrap_err();
    assert_eq!(wrong.code, "PASSPHRASE_INCORRECT");

    let missing = world.dir.path().join("HoploDex backups").join("gone.hoplodex");
    let gone = world.check(&missing.to_string_lossy(), support::TEST_PASSPHRASE).unwrap_err();
    assert_eq!(gone.code, "NOT_FOUND");

    make_newer(&listed[1].path);
    let newer = world.check(&listed[1].path, support::TEST_PASSPHRASE).unwrap_err();
    assert_eq!(newer.code, "DATABASE_NEWER_VERSION");
    assert_eq!(snapshot(&world).0, before.0, "the database is as it was");
}

// 009 FR-019
#[test]
fn check_backup_of_a_damaged_backup_is_database_damaged() {
    let world = World::new();
    let listed = world.with_two_backups();
    let bytes = fs::read(&listed[1].path).unwrap();
    fs::write(&listed[1].path, &bytes[..2 * 4096]).unwrap();

    let refused = world.check(&listed[1].path, support::TEST_PASSPHRASE).unwrap_err();

    assert_eq!(refused.code, "DATABASE_DAMAGED");
}

// 009 FR-019
#[test]
fn check_backup_without_a_database_path_needs_the_session_it_names() {
    let world = World::new();
    let listed = world.with_two_backups();
    let ended = world.session.current_id();
    world.close();
    world.open();

    for id in [ended, SessionId::NONE] {
        let refused = world.check_for(&listed[0].path, support::TEST_PASSPHRASE, Target::Open(id));
        assert_eq!(refused.unwrap_err().code, "DATABASE_CLOSED", "id {id}");
    }
    assert_eq!(world.check(&listed[0].path, support::TEST_PASSPHRASE).unwrap(), None);
}

// 009 FR-019, 009 research.md §9; issue #62
#[test]
fn check_backup_with_a_database_path_applies_the_damaged_restores_authorization() {
    let world = World::new();
    let listed = damaged(&world);
    let _gains = BuildGainsAStep::new(&world);
    let at = |path: &Path, backup: &str| {
        world.check_for(backup, support::TEST_PASSPHRASE, Target::At(&path.to_string_lossy()))
    };
    let stranger = world.dir.path().join("Not a database.txt");
    fs::write(&stranger, b"precious").unwrap();
    let before = snapshot(&world);

    // The database the backup is of, nothing open: the dialog's own call.
    assert_eq!(
        at(&world.path(), &listed[0].path).unwrap().as_deref(),
        Some(hoplodex_lib::APP_VERSION)
    );

    // Not on the recent list.
    assert_eq!(at(&stranger, &listed[0].path).unwrap_err().code, "NOT_FOUND");

    // A backup outside the cached backup folder.
    let elsewhere = TempDir::new().unwrap();
    let copy = elsewhere.path().join(&listed[0].file_name);
    fs::copy(&listed[0].path, &copy).unwrap();
    let outside = at(&world.path(), &copy.to_string_lossy()).unwrap_err();
    assert_eq!(outside.code, "VALIDATION_ERROR");
    assert!(outside.field_errors.unwrap().contains_key("backupPath"));

    // An entry the web view re-located is refused until it has opened there.
    databases::locate_database(
        &world.machine,
        &world.path().to_string_lossy(),
        &stranger.to_string_lossy(),
        &Reach::Unconfined,
    )
    .unwrap();
    let located = at(&stranger, &listed[0].path).unwrap_err();
    assert_eq!(located.code, "VALIDATION_ERROR");
    assert!(located.field_errors.unwrap().contains_key("databasePath"));

    fs::remove_file(&copy).unwrap();
    assert_eq!(fs::read(&stranger).unwrap(), b"precious");
    assert_eq!(snapshot(&world).1, before.1, "nothing is written beside the database");
}

// 009 FR-019, 009 research.md §9; issue #62
#[test]
fn check_backup_with_a_database_path_refuses_another_databases_backup() {
    let world = World::new();
    let listed = world.with_two_backups();
    world.close();
    let other = world.dir.path().join("Other.hoplodex");
    lifecycle::create(&world.session, &world.machine, &other, &passphrase()).unwrap();
    let other_id = world.database_id();
    world.add_firearm("Z9");
    world.close();
    let theirs = backups::list(&world.default_folder(), &other_id).unwrap().remove(0);
    // Named as Mine's backup, in Mine's folder.
    let disguised = Path::new(&listed[0].path).with_file_name(&listed[1].file_name);
    fs::copy(&theirs.path, world.dir.path().join("scratch")).unwrap();
    fs::remove_file(&listed[1].path).unwrap();
    fs::rename(world.dir.path().join("scratch"), &disguised).unwrap();

    for backup in [theirs.path.as_str(), disguised.to_str().unwrap()] {
        let refused = world.check_for(
            backup,
            support::TEST_PASSPHRASE,
            Target::At(&world.path().to_string_lossy()),
        );
        assert_eq!(refused.unwrap_err().code, "VALIDATION_ERROR", "{backup}");
    }
}

// 009 FR-019
#[test]
fn restoring_an_older_backup_without_upgrade_is_refused_before_anything_is_touched() {
    let world = World::new();
    let listed = world.with_two_backups();
    let _gains = BuildGainsAStep::new(&world);
    let before = snapshot(&world);
    let current = open_collection(&world);
    let session = world.session.current_id();
    world.events.take();

    let refused = world.restore_with(&listed[1].path, support::TEST_PASSPHRASE, false).unwrap_err();

    assert_eq!(refused.code, "UPGRADE_NEEDED");
    assert_eq!(
        refused.details.as_deref(),
        Some(&json!({ "version": hoplodex_lib::APP_VERSION })),
        "no awaiting database: the restore's form names only the version"
    );
    assert_eq!(snapshot(&world), before, "nothing copied, backed up or replaced");
    assert!(world.leftovers().is_empty());
    assert!(world.session.is_open(), "nothing closed");
    assert_eq!(world.session.current_id(), session);
    assert_eq!(open_collection(&world), current);
    let phases = world.restore_phases();
    assert!(
        !phases.iter().any(|phase| phase == "savingCurrent" || phase == "replacing"),
        "{phases:?}"
    );
}

// 009 FR-019
#[test]
fn restoring_an_older_backup_with_upgrade_restores_and_upgrades_it() {
    let world = World::new();
    let listed = world.with_two_backups();
    let _gains = BuildGainsAStep::new(&world);
    let expected = collection(&peek(Path::new(&listed[1].path)));
    let backup_bytes = fs::read(&listed[1].path).unwrap();
    assert_ne!(open_collection(&world), expected);
    // Later than the backup restored from: the one before restoring is named
    // for the second.
    world.clock.advance(chrono::Duration::hours(1));
    world.events.take();

    let status = world.restore_with(&listed[1].path, support::TEST_PASSPHRASE, true).unwrap();

    assert_eq!(open_collection(&world), expected, "the backup's content, 003 SC-007");
    let upgraded = status.notes.upgraded.expect("notes.upgraded");
    assert_eq!(upgraded.version, hoplodex_lib::APP_VERSION);
    assert_eq!(upgraded.backup_path, listed[1].path, "the backup restored from");
    assert!(upgraded.at_backup_location, "it is in the backup location");
    assert!(world.applied_steps().iter().any(|name| name == NEW_STEP.0), "the step was applied");
    assert_eq!(fs::read(&listed[1].path).unwrap(), backup_bytes, "the backup is unchanged");
    assert_eq!(world.listed().len(), 3, "and the current database was backed up first");
    assert!(
        world.listed().iter().all(|backup| backup.before_upgrade_to.is_none()),
        "no backup before an upgrade is made: the one restored from is the earlier layout"
    );
    assert!(world.leftovers().is_empty());
    assert_eq!(
        world.restore_phases(),
        ["copying", "checking", "upgrading", "savingCurrent", "replacing"]
    );
    let first_upgrading = world
        .events
        .payloads("restore:progress")
        .into_iter()
        .find(|payload| payload["phase"] == "upgrading")
        .unwrap();
    assert_eq!(first_upgrading["total"], json!(0), "indeterminate");
}

// 009 FR-019, FR-014; 009 research.md §9
#[test]
fn the_backup_restored_from_still_rotates_as_a_regular_one() {
    let world = World::new();
    let listed = world.with_two_backups();
    let id = world.database_id();
    let _gains = BuildGainsAStep::new(&world);
    world.clock.advance(chrono::Duration::hours(1));
    world.restore_with(&listed[1].path, support::TEST_PASSPHRASE, true).unwrap();
    assert!(Path::new(&listed[1].path).exists());
    world.set_backups(true, 1, BackupLocationInput::Default);
    world.add_firearm("C3");
    world.next_day();

    world.close();

    assert!(!Path::new(&listed[1].path).exists(), "kept for no reason but its age");
    assert_eq!(backups::list(&world.default_folder(), &id).unwrap().len(), 1);
}

// 009 FR-019
#[test]
fn upgrade_true_for_a_backup_in_the_current_layout_changes_nothing() {
    let world = World::new();
    let listed = world.with_two_backups();
    let expected = collection(&peek(Path::new(&listed[1].path)));
    world.clock.advance(chrono::Duration::hours(1));
    world.events.take();

    let status = world.restore_with(&listed[1].path, support::TEST_PASSPHRASE, true).unwrap();

    assert_eq!(open_collection(&world), expected);
    assert!(status.notes.upgraded.is_none(), "nothing was upgraded");
    assert_eq!(world.restore_phases(), ["copying", "checking", "savingCurrent", "replacing"]);
}

// 009 FR-019, FR-015
#[test]
fn a_failure_in_the_copys_upgrade_leaves_the_database_as_it_was() {
    let world = World::new();
    let listed = world.with_two_backups();
    let gains = BuildGainsAStep::failing(&world);
    let bytes = fs::read(world.path()).unwrap();
    let current = open_collection(&world);
    let backup_bytes = fs::read(&listed[1].path).unwrap();

    let failed = world.restore_with(&listed[1].path, support::TEST_PASSPHRASE, true).unwrap_err();

    assert_eq!(failed.code, "UPGRADE_FAILED");
    assert_eq!(
        failed.details.as_deref().unwrap()["backupPath"],
        json!(listed[1].path),
        "the backup restored from"
    );
    assert!(world.leftovers().is_empty(), "the copy at .<file>.new is removed");
    assert_eq!(fs::read(world.path()).unwrap(), bytes, "the database is byte-identical");
    assert_eq!(fs::read(&listed[1].path).unwrap(), backup_bytes);
    assert!(world.session.is_open());
    assert_eq!(open_collection(&world), current);

    // Nothing is left over that stops the same restore from working once the
    // fault is gone.
    gains.database_folder.clear_failures();
    world.clock.advance(chrono::Duration::hours(1));
    let status = world.restore_with(&listed[1].path, support::TEST_PASSPHRASE, true).unwrap();
    assert!(status.notes.upgraded.is_some());
    assert!(world.leftovers().is_empty());
}

// 009 FR-019, 009 research.md §9: the chooser's restore of a damaged database
#[test]
fn restoring_a_damaged_database_upgrades_the_copy_the_same_way() {
    let world = World::new();
    let listed = damaged(&world);
    let _gains = BuildGainsAStep::new(&world);
    let expected = collection(&peek(Path::new(&listed[0].path)));
    let before = snapshot(&world);
    let at = world.path();
    let restore = |upgrade: bool| {
        backups_ops::restore_backup(
            &world.session,
            &world.machine,
            &listed[0].path,
            &passphrase(),
            Target::At(&at.to_string_lossy()),
            upgrade,
            &Reach::Unconfined,
        )
    };

    let refused = restore(false).unwrap_err();

    assert_eq!(refused.code, "UPGRADE_NEEDED");
    assert_eq!(refused.details.as_deref(), Some(&json!({ "version": hoplodex_lib::APP_VERSION })));
    assert_eq!(snapshot(&world), before, "the damaged file is not set aside, nothing is copied");

    let status = restore(true).unwrap();

    let upgraded = status.notes.upgraded.expect("notes.upgraded");
    assert_eq!(upgraded.backup_path, listed[0].path);
    assert!(status.notes.damaged_file_kept_at.is_some());
    assert_eq!(open_collection(&world), expected);
    assert!(world.applied_steps().iter().any(|name| name == NEW_STEP.0));
    assert!(world.leftovers().is_empty());
}

// 009 FR-018, FR-019
#[test]
fn restoring_a_newer_backup_is_refused_whether_or_not_it_asks_to_upgrade() {
    let world = World::new();
    let listed = world.with_two_backups();
    let before = snapshot(&world);
    make_newer(&listed[1].path);

    for upgrade in [false, true] {
        let refused =
            world.restore_with(&listed[1].path, support::TEST_PASSPHRASE, upgrade).unwrap_err();
        assert_eq!(refused.code, "DATABASE_NEWER_VERSION", "upgrade {upgrade}");
    }

    assert_eq!(snapshot(&world).0, before.0);
    assert!(world.leftovers().is_empty());
    assert!(world.session.is_open());
}
