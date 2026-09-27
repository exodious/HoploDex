//! Restoring a database from a backup, including one that no longer opens
//! (FR-028, SC-004, SC-007; research.md §8). Real SQLCipher files in temp
//! directories, a manual clock, and machine settings in a temp config
//! directory.

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::photos::ops as photos;
use hoplodex_lib::commands::CommandError;
use hoplodex_lib::db::cipher;
use hoplodex_lib::models::database::{
    BackupInfo, BackupLocationInput, BackupSettingsInput, CloseReason, DatabaseStatus,
    ExistingBackupsChoice,
};
use hoplodex_lib::services::backups;
use hoplodex_lib::services::disk_space;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::session::{lifecycle, Session};
use rusqlite::Connection;
use serde_json::json;
use support::{passphrase, peek, sample_png_bytes, test_session_at, ManualClock, TestEvents};
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
        lifecycle::close_normal(&self.session, &self.machine, CloseReason::Closed).unwrap();
    }

    fn next_day(&self) {
        self.clock.advance(chrono::Duration::days(1));
    }

    /// Adds a firearm with a photo, a change with a blob in it.
    fn add_firearm(&self, serial: &str) {
        self.session
            .write(|conn| {
                let firearm = firearms::create_firearm(
                    conn,
                    &support::firearm("Colt", "Python", serial),
                    false,
                )?;
                photos::add_photo(conn, firearm.id, &sample_png_bytes(), "front.png", "image/png")
                    .map(|_| ())
            })
            .unwrap();
    }

    fn database_id(&self) -> String {
        self.session.inspect(|open| Ok(open.database_id.clone())).unwrap()
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
            None,
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
        // Backups already at an old location stay there, as before FR-026
        // asked about them; backup_location_change_test covers the rest.
        let existing_backups = Some(ExistingBackupsChoice::Leave);
        databases::update_backup_settings(
            &self.session,
            &self.machine,
            &BackupSettingsInput { enabled, keep_count, location, existing_backups },
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
    world.session.read(|conn| Ok(collection(conn))).unwrap()
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
    assert_eq!(open_collection(&world), expected, "SC-007");
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
        .read(|conn| {
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
        .read(|conn| {
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
    assert_eq!(open_collection(&world), current, "SC-004");
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

// --- A damaged database (US3-6) ---------------------------------------------

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
        Some(&world.path().to_string_lossy()),
    )
    .unwrap();
    assert!(found.available);
    assert_eq!(found.backups, listed, "found from the recent entry's cached folder and id");

    let status = backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &listed[0].path,
        &passphrase(),
        Some(&world.path().to_string_lossy()),
    )
    .unwrap();

    let kept = world.dir.path().join("Mine damaged 2026-09-26 153005.hoplodex");
    assert_eq!(status.notes.damaged_file_kept_at.as_deref(), Some(kept.to_str().unwrap()));
    assert_eq!(fs::read(&kept).unwrap(), damaged_bytes, "the damaged file is kept as it was");
    assert_eq!(open_collection(&world), expected);
    assert_eq!(world.listed().len(), 2, "no before-restoring backup of a file that won't open");
}

// --- Preconditions and failures (FR-028, research.md §8) --------------------

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
        Some(&world.path().to_string_lossy()),
    )
    .unwrap();

    assert!(world.session.is_open());
}
