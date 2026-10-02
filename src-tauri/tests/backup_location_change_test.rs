//! Changing a database's backup location with backups already at the old
//! one: asking first, then leaving, deleting or moving them (FR-026,
//! US3-4a, US3-4b, SC-007; research.md §22). Real backups made by earlier
//! closes, in temp directories, on a manual clock, with machine settings in
//! a temp config directory.

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::db;
use hoplodex_lib::models::database::{
    BackupInfo, BackupLocationInput, BackupOutcome, BackupSettingsInput, BackupSettingsSaved,
    CloseReason, ExistingBackupsChoice,
};
use hoplodex_lib::services::backups;
use hoplodex_lib::services::disk_space;
use hoplodex_lib::services::machine_settings::{MachineSettings, UnfinishedBackupMove};
use hoplodex_lib::session::{Session, lifecycle};
use serde_json::{Value, json};
use support::{ManualClock, TestEvents, passphrase, test_session_at};
use tempfile::TempDir;

const START: &str = "2026-09-25T14:30:05+02:00";

struct World {
    dir: TempDir,
    config: TempDir,
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
            config,
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

    /// A custom folder beside the database, made.
    fn folder(&self, name: &str) -> PathBuf {
        let folder = self.dir.path().join(name);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn open(&self) {
        lifecycle::open(&self.session, &self.machine, &self.path(), &passphrase(), false).unwrap();
    }

    fn close(&self) -> BackupOutcome {
        lifecycle::close_normal(&self.session, &self.machine, CloseReason::Closed).unwrap().backup
    }

    fn change(&self) {
        let serial = db::random_hex(4).unwrap();
        self.session
            .write(|conn| {
                firearms::create_firearm(
                    conn,
                    &support::firearm("Colt", "Python", &serial),
                    false,
                    None,
                )
            })
            .unwrap();
    }

    /// A backup at the next close, a day later than the last.
    fn backup_now(&self) {
        self.change();
        assert_eq!(self.close(), BackupOutcome::Made);
        self.open();
        self.clock.advance(chrono::Duration::days(1));
    }

    fn database_id(&self) -> String {
        self.session.inspect(|open| Ok(open.database_id.clone())).unwrap()
    }

    fn update(
        &self,
        location: BackupLocationInput,
        existing_backups: Option<ExistingBackupsChoice>,
    ) -> Result<BackupSettingsSaved, CommandError> {
        self.update_keeping(5, location, existing_backups)
    }

    fn update_keeping(
        &self,
        keep_count: i64,
        location: BackupLocationInput,
        existing_backups: Option<ExistingBackupsChoice>,
    ) -> Result<BackupSettingsSaved, CommandError> {
        databases::update_backup_settings(
            &self.session,
            &self.machine,
            &BackupSettingsInput { enabled: true, keep_count, location, existing_backups },
        )
    }

    /// The saved location, resolved.
    fn saved_folder(&self) -> PathBuf {
        let status = databases::database_status(&self.session, &self.machine).unwrap();
        PathBuf::from(status.settings.backups.location.path)
    }

    /// The folder this computer's recent entry caches for the database.
    fn cached_folder(&self) -> Option<PathBuf> {
        self.machine.recent_entry(&self.path()).and_then(|entry| entry.backup_folder)
    }

    /// The backups `list_backups` offers for restore.
    fn listed(&self) -> Vec<BackupInfo> {
        backups_ops::list_backups(&self.session, &self.machine, None).unwrap().backups
    }

    fn backups_in(&self, folder: &Path) -> Vec<BackupInfo> {
        backups::list(folder, &self.database_id()).unwrap()
    }
}

fn custom(folder: &Path) -> BackupLocationInput {
    BackupLocationInput::Custom { path: folder.to_string_lossy().into_owned() }
}

fn details(err: &CommandError) -> Value {
    err.details.as_deref().cloned().unwrap_or(Value::Null)
}

fn outcome(saved: &BackupSettingsSaved) -> Value {
    serde_json::to_value(&saved.existing_backups).unwrap()
}

/// "Mine", created and left open with three backups in the default folder,
/// each made on its own day. Returns them, newest first, with their bytes.
fn with_three_backups(world: &World) -> Vec<(BackupInfo, Vec<u8>)> {
    lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
    for _ in 0..3 {
        world.backup_now();
    }
    let listed = world.backups_in(&world.default_folder());
    assert_eq!(listed.len(), 3);
    listed
        .into_iter()
        .map(|backup| {
            let bytes = fs::read(&backup.path).unwrap();
            (backup, bytes)
        })
        .collect()
}

fn total_bytes(backups: &[(BackupInfo, Vec<u8>)]) -> u64 {
    backups.iter().map(|(backup, _)| backup.size_bytes).sum()
}

/// Each backup is at `folder`, byte for byte as it was.
fn assert_all_at(world: &World, backups: &[(BackupInfo, Vec<u8>)], folder: &Path) {
    for (backup, bytes) in backups {
        let moved = folder.join(&backup.file_name);
        assert_eq!(&fs::read(&moved).unwrap(), bytes, "{} is identical", backup.file_name);
    }
    assert_eq!(world.backups_in(folder).len(), backups.len());
}

fn partial_files(folder: &Path) -> Vec<String> {
    fs::read_dir(folder)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(".partial"))
                .collect()
        })
        .unwrap_or_default()
}

// --- Asking (FR-026, US3-4a) -------------------------------------------------

#[test]
fn a_changed_location_with_backups_at_the_old_one_asks_first_and_saves_nothing() {
    let world = World::new();
    let made = with_three_backups(&world);
    let new = world.folder("elsewhere");

    let refused = world.update(custom(&new), None).unwrap_err();

    assert_eq!(refused.code, "BACKUPS_AT_OLD_LOCATION");
    assert_eq!(
        details(&refused),
        json!({
            "folder": world.default_folder().to_string_lossy(),
            "count": 3,
            "totalBytes": total_bytes(&made),
        })
    );
    assert_eq!(world.saved_folder(), world.default_folder(), "nothing was saved");
    assert_eq!(world.cached_folder(), Some(world.default_folder()));
    assert_eq!(world.listed().len(), 3, "the backups are untouched");
}

#[test]
fn with_no_backups_at_the_old_location_it_saves_at_once() {
    let world = World::new();
    lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
    let new = world.folder("elsewhere");

    let saved = world.update(custom(&new), None).unwrap();

    assert_eq!(saved.existing_backups, None);
    assert_eq!(world.saved_folder(), new);
    assert_eq!(world.cached_folder(), Some(new));
}

#[test]
fn the_same_folder_named_another_way_is_not_a_change() {
    let world = World::new();
    with_three_backups(&world);
    let spelled_otherwise = world.dir.path().join(".").join("HoploDex backups");

    let saved = world.update(custom(&spelled_otherwise), None).unwrap();

    assert_eq!(saved.existing_backups, None);
    assert_eq!(world.listed().len(), 3);
}

// --- Leave (US3-4a) ---------------------------------------------------------

#[test]
fn leaving_them_saves_and_they_are_no_longer_the_databases_backups() {
    let world = World::new();
    let made = with_three_backups(&world);
    let new = world.folder("elsewhere");

    let saved = world.update_keeping(1, custom(&new), Some(ExistingBackupsChoice::Leave)).unwrap();

    assert_eq!(outcome(&saved), json!({ "action": "leave" }));
    assert_eq!(world.saved_folder(), new);
    assert_eq!(world.cached_folder(), Some(new.clone()));
    assert!(world.listed().is_empty(), "not listed for restore");

    // Rotation after the next backup, keeping 1, touches none of them.
    world.backup_now();
    assert_eq!(world.backups_in(&new).len(), 1);
    assert_all_at(&world, &made, &world.default_folder());

    // Nor does deleting all backups.
    let deleted = backups_ops::delete_all_backups(&world.session, true).unwrap();
    assert_eq!(deleted.deleted_count, 1);
    assert_all_at(&world, &made, &world.default_folder());

    // Choosing their folder again makes them the database's again.
    let saved = world.update(BackupLocationInput::Default, None).unwrap();
    assert_eq!(saved.existing_backups, None, "nothing is left at the new folder");
    assert_eq!(world.listed().len(), 3);
}

// --- Delete (FR-029) ---------------------------------------------------------

#[test]
fn deleting_them_removes_only_this_databases_backups_at_the_old_folder() {
    let world = World::new();
    let made = with_three_backups(&world);
    let unrelated = world.default_folder().join("my notes.txt");
    fs::write(&unrelated, b"keep me").unwrap();
    let other = world.default_folder().join("Other 2026-09-20 101010 0badc0de.hoplodex");
    fs::write(&other, b"another database's backup").unwrap();
    let new = world.folder("elsewhere");
    world.events.take();

    let saved = world.update(custom(&new), Some(ExistingBackupsChoice::Delete)).unwrap();

    assert_eq!(outcome(&saved), json!({ "action": "delete", "deletedCount": 3 }));
    for (backup, _) in &made {
        assert!(!Path::new(&backup.path).exists());
    }
    assert_eq!(fs::read(&unrelated).unwrap(), b"keep me");
    assert!(other.exists());
    assert_eq!(
        world.events.payloads("backups_delete:progress"),
        vec![
            json!({ "processed": 1, "total": 3 }),
            json!({ "processed": 2, "total": 3 }),
            json!({ "processed": 3, "total": 3 }),
        ]
    );
    assert_eq!(world.saved_folder(), new);
    assert_eq!(world.cached_folder(), Some(new));
}

#[test]
fn a_backup_that_cannot_be_deleted_keeps_the_old_location() {
    let world = World::new();
    let made = with_three_backups(&world);
    let stuck = &made[1].0.path;
    fs::set_permissions(stuck, fs::Permissions::from_mode(0o444)).unwrap();
    let new = world.folder("elsewhere");

    let refused = world.update(custom(&new), Some(ExistingBackupsChoice::Delete)).unwrap_err();

    assert_eq!(refused.code, "BACKUPS_NOT_ALL_DELETED");
    assert_eq!(details(&refused), json!({ "deletedCount": 2, "failedPaths": [stuck] }));
    assert_eq!(world.saved_folder(), world.default_folder());
    assert_eq!(world.cached_folder(), Some(world.default_folder()));
    assert_eq!(world.listed().len(), 1, "the one left is still the database's");
}

// --- Move (FR-026, research.md §22) -----------------------------------------

#[test]
fn moving_them_on_one_drive_takes_every_backup_byte_for_byte() {
    let world = World::new();
    let made = with_three_backups(&world);
    let new = world.folder("elsewhere");
    world.events.take();

    let saved = world.update(custom(&new), Some(ExistingBackupsChoice::Move)).unwrap();

    assert_eq!(outcome(&saved), json!({ "action": "move", "movedCount": 3, "leftBehind": null }));
    assert_all_at(&world, &made, &new);
    assert!(world.backups_in(&world.default_folder()).is_empty());
    assert!(world.default_folder().is_dir(), "the old folder is never removed");
    assert_eq!(world.listed().len(), 3);
    assert_eq!(world.saved_folder(), new);
    assert_eq!(world.cached_folder(), Some(new));

    let total = total_bytes(&made) * 2;
    let progress = world.events.payloads("backups_move:progress");
    assert_eq!(
        progress.first(),
        Some(&json!({
            "processed": 0,
            "total": total,
            "showNow": backups::shows_progress_at_once(total),
        })),
        "the first event carries the total"
    );
    assert_eq!(progress.last().unwrap()["processed"], json!(total));
}

#[test]
fn moving_them_to_another_drive_copies_verifies_then_deletes_each_original() {
    let world = World::new();
    let made = with_three_backups(&world);
    let new = world.folder("elsewhere");
    let _copying = backups::testing::fail_hard_links();
    let config = world.config.path().to_owned();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_by_hook = Arc::clone(&seen);
    let _watch = backups::testing::after_each_copy(move |partial| {
        // While a copy is being made, machine.json names it for the sweep,
        // and its original is still there.
        let machine: Value =
            serde_json::from_slice(&fs::read(config.join("machine.json")).unwrap()).unwrap();
        let record = &machine["unfinishedBackupMove"];
        assert_eq!(record["partialPath"], json!(partial.to_string_lossy()));
        let name = partial.file_name().unwrap().to_string_lossy();
        let original = Path::new(record["fromFolder"].as_str().unwrap())
            .join(name.strip_suffix(".partial").unwrap());
        assert!(original.exists(), "the original stays until its copy is verified");
        seen_by_hook.lock().unwrap().push(partial.to_owned());
    });
    world.events.take();

    let saved = world.update(custom(&new), Some(ExistingBackupsChoice::Move)).unwrap();

    assert_eq!(outcome(&saved), json!({ "action": "move", "movedCount": 3, "leftBehind": null }));
    assert_eq!(seen.lock().unwrap().len(), 3, "each backup was copied");
    assert_all_at(&world, &made, &new);
    assert!(world.backups_in(&world.default_folder()).is_empty());
    assert!(partial_files(&new).is_empty());
    assert_eq!(world.machine.unfinished_backup_move(), None, "cleared when the move ends");
    let total = total_bytes(&made) * 2;
    let progress = world.events.payloads("backups_move:progress");
    assert_eq!(progress.first().unwrap()["total"], json!(total));
    assert_eq!(progress.last().unwrap()["processed"], json!(total), "copy and read-back");
}

#[test]
fn a_copy_that_does_not_match_its_original_is_removed_and_the_original_kept() {
    let world = World::new();
    let made = with_three_backups(&world);
    let new = world.folder("elsewhere");
    let _copying = backups::testing::fail_hard_links();
    let _corrupt = backups::testing::after_each_copy(|partial| {
        let mut bytes = fs::read(partial).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        fs::write(partial, bytes).unwrap();
    });

    let saved = world.update(custom(&new), Some(ExistingBackupsChoice::Move)).unwrap();

    assert_eq!(
        outcome(&saved),
        json!({
            "action": "move",
            "movedCount": 0,
            "leftBehind": {
                "count": 3, "folder": world.default_folder().to_string_lossy(), "reason": "io",
            },
        })
    );
    assert_all_at(&world, &made, &world.default_folder());
    assert!(partial_files(&new).is_empty(), "the partial copy is removed");
    assert_eq!(world.saved_folder(), new, "the new location is kept");
}

#[test]
fn a_missing_custom_new_folder_is_refused_with_nothing_moved_or_saved() {
    let world = World::new();
    let made = with_three_backups(&world);
    let gone = world.dir.path().join("unplugged drive");

    let refused = world.update(custom(&gone), Some(ExistingBackupsChoice::Move)).unwrap_err();

    assert_eq!(refused.code, "BACKUP_LOCATION_UNAVAILABLE");
    assert_eq!(details(&refused)["path"], json!(gone.to_string_lossy()));
    assert!(!gone.exists(), "a custom folder is never made");
    assert_eq!(world.saved_folder(), world.default_folder());
    assert_all_at(&world, &made, &world.default_folder());
}

#[test]
fn too_little_space_at_the_new_folder_is_refused_with_nothing_moved_or_saved() {
    let world = World::new();
    let made = with_three_backups(&world);
    let new = world.folder("elsewhere");
    let needed = disk_space::room_for_copy(total_bytes(&made));
    let short = new.clone();
    let _full =
        disk_space::testing::fake_available_space(move |dir| (dir == short).then_some(needed - 1));

    let refused = world.update(custom(&new), Some(ExistingBackupsChoice::Move)).unwrap_err();

    assert_eq!(refused.code, "INSUFFICIENT_SPACE");
    assert_eq!(details(&refused)["bytesNeeded"], json!(needed), "all of them, plus 5%");
    assert_eq!(details(&refused)["bytesAvailable"], json!(needed - 1));
    assert_eq!(world.saved_folder(), world.default_folder());
    assert_all_at(&world, &made, &world.default_folder());
    assert!(world.backups_in(&new).is_empty());
}

#[test]
fn a_name_already_taken_at_the_new_folder_is_left_behind_and_never_overwritten() {
    let world = World::new();
    let made = with_three_backups(&world);
    let new = world.folder("elsewhere");
    let taken = new.join(&made[1].0.file_name);
    fs::write(&taken, b"already here").unwrap();
    // Its room isn't needed: only the two that move count.
    let needed = disk_space::room_for_copy(made[0].0.size_bytes + made[2].0.size_bytes);
    let exact = new.clone();
    let _space =
        disk_space::testing::fake_available_space(move |dir| (dir == exact).then_some(needed));

    let saved = world.update(custom(&new), Some(ExistingBackupsChoice::Move)).unwrap();

    assert_eq!(
        outcome(&saved),
        json!({
            "action": "move",
            "movedCount": 2,
            "leftBehind": {
                "count": 1, "folder": world.default_folder().to_string_lossy(), "reason": "nameTaken",
            },
        })
    );
    assert_eq!(fs::read(&taken).unwrap(), b"already here", "the file there is untouched");
    assert_eq!(fs::read(&made[1].0.path).unwrap(), made[1].1, "that backup stays behind");
    assert_eq!(fs::read(new.join(&made[0].0.file_name)).unwrap(), made[0].1);
    assert_eq!(fs::read(new.join(&made[2].0.file_name)).unwrap(), made[2].1);
}

#[test]
fn a_failure_part_way_keeps_the_new_location_and_leaves_the_rest_behind() {
    let world = World::new();
    let made = with_three_backups(&world);
    let new = world.folder("elsewhere");
    // Oldest first: the oldest moves, the middle one can't be deleted once
    // copied, so it is never copied, and the newest is never tried.
    let (oldest, middle, newest) = (&made[2], &made[1], &made[0]);
    fs::set_permissions(&middle.0.path, fs::Permissions::from_mode(0o444)).unwrap();
    let _copying = backups::testing::fail_hard_links();

    let saved = world.update(custom(&new), Some(ExistingBackupsChoice::Move)).unwrap();

    assert_eq!(
        outcome(&saved),
        json!({
            "action": "move",
            "movedCount": 1,
            "leftBehind": {
                "count": 2, "folder": world.default_folder().to_string_lossy(), "reason": "io",
            },
        })
    );
    assert_eq!(fs::read(new.join(&oldest.0.file_name)).unwrap(), oldest.1);
    assert_eq!(fs::read(&middle.0.path).unwrap(), middle.1);
    assert_eq!(fs::read(&newest.0.path).unwrap(), newest.1);
    assert!(partial_files(&new).is_empty());
    assert_eq!(world.saved_folder(), new);
    assert_eq!(world.listed().len(), 1);
}

#[test]
fn the_new_folder_disappearing_part_way_is_reported_as_unavailable() {
    let world = World::new();
    let made = with_three_backups(&world);
    let new = world.folder("elsewhere");
    let _copying = backups::testing::fail_hard_links();
    let _unplug = backups::testing::after_each_copy(|partial| {
        fs::remove_dir_all(partial.parent().unwrap()).unwrap();
    });

    let saved = world.update(custom(&new), Some(ExistingBackupsChoice::Move)).unwrap();

    assert_eq!(saved.existing_backups.as_ref().map(|_| ()), Some(()));
    let moved = outcome(&saved);
    assert_eq!(moved["movedCount"], json!(0));
    assert_eq!(moved["leftBehind"]["reason"], json!("locationUnavailable"));
    assert_eq!(moved["leftBehind"]["count"], json!(3));
    assert_all_at(&world, &made, &world.default_folder());
}

#[test]
fn a_move_into_a_folder_already_holding_backups_rotates_nothing_until_the_next_backup() {
    let world = World::new();
    with_three_backups(&world);
    let new = world.folder("elsewhere");
    // Two backups made at the new folder earlier, then left there.
    world.update(custom(&new), Some(ExistingBackupsChoice::Leave)).unwrap();
    world.backup_now();
    world.backup_now();
    world.update(BackupLocationInput::Default, Some(ExistingBackupsChoice::Leave)).unwrap();
    assert_eq!(world.backups_in(&new).len(), 2);

    let saved = world.update_keeping(3, custom(&new), Some(ExistingBackupsChoice::Move)).unwrap();

    assert_eq!(outcome(&saved)["movedCount"], json!(3));
    assert_eq!(world.backups_in(&new).len(), 5, "more than kept, until the next backup (SC-007)");
    world.backup_now();
    assert_eq!(world.backups_in(&new).len(), 3);
}

// --- An unreadable old location (US3-4b) -------------------------------------

#[test]
fn an_unreachable_old_custom_folder_allows_only_leaving_its_backups() {
    let world = World::new();
    lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
    let usb = world.folder("usb stick");
    world.update(custom(&usb), None).unwrap();
    world.backup_now();
    let unplugged = world.dir.path().join("usb stick (unplugged)");
    fs::rename(&usb, &unplugged).unwrap();

    for choice in [None, Some(ExistingBackupsChoice::Move), Some(ExistingBackupsChoice::Delete)] {
        let refused = world.update(BackupLocationInput::Default, choice).unwrap_err();
        assert_eq!(refused.code, "OLD_BACKUP_LOCATION_UNAVAILABLE", "{choice:?}");
        assert_eq!(details(&refused), json!({ "folder": usb.to_string_lossy() }));
        assert_eq!(world.saved_folder(), usb, "nothing saved for {choice:?}");
    }

    let saved =
        world.update(BackupLocationInput::Default, Some(ExistingBackupsChoice::Leave)).unwrap();
    assert_eq!(outcome(&saved), json!({ "action": "leave" }));
    assert_eq!(world.saved_folder(), world.default_folder());
    assert_eq!(world.cached_folder(), Some(world.default_folder()));
    assert_eq!(world.backups_in(&unplugged).len(), 1, "left where it was");
}

// --- Other checks -----------------------------------------------------------

#[test]
fn another_running_operation_refuses_a_move_before_anything_is_saved() {
    let world = World::new();
    let made = with_three_backups(&world);
    let new = world.folder("elsewhere");
    let _running = world
        .session
        .operations()
        .begin(hoplodex_lib::models::database::OperationKind::Export, None)
        .unwrap();

    let refused = world.update(custom(&new), Some(ExistingBackupsChoice::Move)).unwrap_err();

    assert_eq!(refused.code, "OPERATION_IN_PROGRESS");
    assert_eq!(world.saved_folder(), world.default_folder());
    assert_all_at(&world, &made, &world.default_folder());
}

#[test]
fn the_cached_folder_follows_a_save_that_does_not_change_the_location() {
    let world = World::new();
    lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
    world.machine.set_backup_folder(&world.path(), Path::new("/stale"));

    world.update(BackupLocationInput::Default, None).unwrap();

    assert_eq!(world.cached_folder(), Some(world.default_folder()));
}

// --- A move cut short by a crash (research.md §22) -------------------------

#[test]
fn a_move_cut_short_by_a_crash_is_swept_at_the_next_launch() {
    let world = World::new();
    let made = with_three_backups(&world);
    let id = world.database_id();
    let new = world.folder("elsewhere");
    let partial = new.join(format!("{}.partial", made[2].0.file_name));
    fs::write(&partial, &made[2].1[..100]).unwrap();
    world.machine.set_unfinished_backup_move(UnfinishedBackupMove {
        database_path: world.path(),
        database_id: id,
        from_folder: world.default_folder(),
        partial_path: Some(partial.clone()),
    });

    // The next launch.
    let machine = MachineSettings::load(world.config.path()).unwrap();
    backups::sweep_unfinished(&machine);

    assert!(!partial.exists(), "the partial copy is removed");
    assert_eq!(machine.unfinished_backup_move(), None);
    assert_eq!(
        serde_json::to_value(machine.take_notices()).unwrap(),
        json!([{
            "kind": "backupsLeftBehind",
            "databasePath": world.path().to_string_lossy(),
            "folder": world.default_folder().to_string_lossy(),
            "count": 3,
        }])
    );
    assert_all_at(&world, &made, &world.default_folder());
}

#[test]
fn a_move_record_with_nothing_left_behind_leaves_no_notice() {
    let world = World::new();
    lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
    world.machine.set_unfinished_backup_move(UnfinishedBackupMove {
        database_path: world.path(),
        database_id: world.database_id(),
        from_folder: world.default_folder(),
        partial_path: None,
    });

    backups::sweep_unfinished(&world.machine);

    assert_eq!(world.machine.unfinished_backup_move(), None);
    assert!(world.machine.take_notices().is_empty());
}
