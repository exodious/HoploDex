//! Automatic backups at close, rotation, listing, deleting them all, and a
//! backup opened directly (FR-025–FR-029, SC-004, SC-005; research.md §7,
//! §9; data-model.md "Backup files"). Real SQLCipher files in temp
//! directories, a manual clock with its own time zone, and machine settings
//! in a temp config directory.

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::CommandError;
use hoplodex_lib::db;
use hoplodex_lib::models::database::{
    BackupFailureReason, BackupInfo, BackupLocationInput, BackupOutcome, BackupSettingsInput,
    CloseOutcome, CloseReason, NoteKind,
};
use hoplodex_lib::services::backups;
use hoplodex_lib::services::disk_space;
use hoplodex_lib::services::machine_settings::{MachineSettings, UnfinishedBackup};
use hoplodex_lib::session::{lifecycle, Session};
use serde_json::{json, Value};
use support::{passphrase, peek, test_session_at, ManualClock, TestEvents};
use tempfile::TempDir;

/// 14:30:05 on 25 September 2026 in a UTC+2 time zone, 12:30:05 UTC.
const START: &str = "2026-09-25T14:30:05+02:00";

/// A throwaway world on a manual clock: a folder for databases, a config
/// directory for `machine.json`, and a session reporting to a recorder.
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

    /// Creates "Mine" and leaves it open.
    fn create(&self) {
        lifecycle::create(&self.session, &self.machine, &self.path(), &passphrase()).unwrap();
    }

    fn open(&self) {
        lifecycle::open(&self.session, &self.machine, &self.path(), &passphrase(), false).unwrap();
    }

    fn close(&self) -> CloseOutcome {
        lifecycle::close_normal(&self.session, &self.machine, CloseReason::Closed).unwrap()
    }

    /// A change to the collection: a new firearm.
    fn change(&self) {
        let serial = db::random_hex(4).unwrap();
        self.session
            .write(|conn| {
                firearms::create_firearm(conn, &support::firearm("Colt", "Python", &serial), false)
            })
            .unwrap();
    }

    /// Makes the file about `bytes` bigger, so a copy takes several chunks.
    fn grow(&self, bytes: usize) {
        let filler = db::random_hex(bytes / 2).unwrap();
        self.session
            .write(|conn| {
                conn.execute_batch("CREATE TABLE IF NOT EXISTS filler (content TEXT)").unwrap();
                conn.execute("INSERT INTO filler (content) VALUES (?1)", [filler]).unwrap();
                Ok(())
            })
            .unwrap();
    }

    fn database_id(&self) -> String {
        self.session.inspect(|open| Ok(open.database_id.clone())).unwrap()
    }

    fn firearm_count(&self) -> i64 {
        self.session
            .read(|conn| {
                conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0))
                    .map_err(CommandError::from_db)
            })
            .unwrap()
    }

    /// `(changes_waiting, last_backup_at)` of the open database.
    fn backup_record(&self) -> (bool, Option<String>) {
        self.session
            .read(|conn| {
                conn.query_row("SELECT changes_waiting, last_backup_at FROM app_state", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .map_err(CommandError::from_db)
            })
            .unwrap()
    }

    fn set_backups(&self, enabled: bool, keep_count: i64, location: BackupLocationInput) {
        databases::update_backup_settings(
            &self.session,
            &BackupSettingsInput { enabled, keep_count, location },
        )
        .unwrap();
    }

    /// This database's backups in `folder`, newest first.
    fn backups_in(&self, folder: &Path, database_id: &str) -> Vec<BackupInfo> {
        backups::list(folder, database_id).unwrap()
    }

    fn file_names(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder)
            .map(|entries| {
                entries.map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    fn next_day(&self) {
        self.clock.advance(chrono::Duration::days(1));
    }
}

fn notices(events: &TestEvents) -> Vec<Value> {
    events.payloads("notice")
}

// --- Close triggers ---------------------------------------------------------

#[test]
fn a_close_after_changes_makes_one_backup_then_clears_the_changes() {
    let world = World::new();
    world.create();
    let id = world.database_id();
    world.change();

    let outcome = world.close();

    assert_eq!(outcome, CloseOutcome::backup(BackupOutcome::Made));
    assert_eq!(
        World::file_names(&world.default_folder()),
        vec![format!("Mine 2026-09-25 143005 {}.hoplodex", &id[..8])],
        "one backup, named for the database, the local time and its id"
    );
    world.open();
    assert_eq!(world.backup_record(), (false, Some("2026-09-25T12:30:05Z".to_owned())));
}

#[test]
fn a_second_close_the_same_day_makes_none_but_the_next_days_close_does() {
    let world = World::new();
    world.create();
    let id = world.database_id();
    world.change();
    assert_eq!(world.close().backup, BackupOutcome::Made);

    world.open();
    world.change();
    world.clock.set("2026-09-25T23:30:00+02:00");
    assert_eq!(world.close().backup, BackupOutcome::AlreadyToday);
    assert_eq!(world.backups_in(&world.default_folder(), &id).len(), 1);

    // Still the 25th in UTC, but the 26th where the computer is: a new day
    // (US3-1a), with the changes of the last session still waiting.
    world.clock.set("2026-09-26T00:30:00+02:00");
    world.open();
    assert!(world.backup_record().0, "the changes are still waiting");
    assert_eq!(world.close().backup, BackupOutcome::Made);
    assert_eq!(world.backups_in(&world.default_folder(), &id).len(), 2);
}

#[test]
fn an_unchanged_session_makes_no_backup() {
    let world = World::new();
    world.create();
    assert_eq!(world.close().backup, BackupOutcome::NotDue);
    world.open();
    assert_eq!(world.close().backup, BackupOutcome::NotDue);

    assert!(!world.default_folder().exists(), "no backup folder is made for nothing");
}

#[test]
fn with_backups_off_none_is_made_and_the_existing_ones_are_kept() {
    let world = World::new();
    world.create();
    let id = world.database_id();
    world.change();
    world.close();

    world.open();
    world.set_backups(false, 5, BackupLocationInput::Default);
    world.change();
    world.next_day();
    assert_eq!(world.close().backup, BackupOutcome::Off);

    assert_eq!(world.backups_in(&world.default_folder(), &id).len(), 1);
    world.open();
    assert!(world.backup_record().0, "the changes stay waiting (US3-8)");
}

#[test]
fn a_dropped_connection_leaves_the_changes_waiting_for_the_next_close() {
    let world = World::new();
    world.create();
    world.change();
    // A crash: the connection goes without a close.
    drop(world.session.take());

    world.open();
    assert_eq!(world.backup_record(), (true, None), "US3-1b");
    assert_eq!(world.close().backup, BackupOutcome::Made);
}

// --- Rotation and listing ---------------------------------------------------

#[test]
fn rotation_keeps_the_latest_and_deletes_only_once_the_new_backup_is_complete() {
    let world = World::new();
    world.create();
    let id = world.database_id();
    world.set_backups(true, 3, BackupLocationInput::Default);
    for _ in 0..3 {
        world.change();
        world.close();
        world.open();
        world.next_day();
    }
    let before = world.backups_in(&world.default_folder(), &id);
    assert_eq!(before.len(), 3);

    // While the fourth is being copied, the oldest is still there.
    let counted = Arc::new(Mutex::new(Vec::new()));
    let (folder, database_id, seen) = (world.default_folder(), id.clone(), counted.clone());
    world.events.on_event(move |event, _| {
        if event == "backup:progress" {
            seen.lock().unwrap().push(backups::list(&folder, &database_id).unwrap().len());
        }
    });
    world.change();
    assert_eq!(world.close().backup, BackupOutcome::Made);

    assert!(counted.lock().unwrap().iter().all(|count| *count == 3), "{counted:?}");
    let after = world.backups_in(&world.default_folder(), &id);
    assert_eq!(after.len(), 3);
    assert!(!after.iter().any(|b| b.path == before[2].path), "the oldest was deleted");
    assert!(!Path::new(&before[2].path).exists());
    assert_eq!(after[1].path, before[0].path);
}

#[test]
fn listing_takes_only_this_databases_backups_newest_first() {
    let world = World::new();
    world.create();
    let id = world.database_id();
    let shared = world.dir.path().join("Shared backups");
    fs::create_dir(&shared).unwrap();
    world.set_backups(true, 5, BackupLocationInput::Custom { path: path_text(&shared) });
    for _ in 0..2 {
        world.change();
        world.close();
        world.open();
        world.next_day();
    }
    let id8 = &id[..8];
    for stray in [
        format!("Mine 2026-09-28 101010 {id8}.hoplodex.partial"),
        "Mine 2026-09-28 101010 0badc0de.hoplodex".to_owned(),
        format!("Mine 2026-9-28 1010 {id8}.hoplodex"),
        format!("Mine {id8}.hoplodex"),
        "notes.txt".to_owned(),
    ] {
        fs::write(shared.join(stray), b"not a backup of Mine").unwrap();
    }

    let listed = world.backups_in(&shared, &id);

    let names: Vec<&str> = listed.iter().map(|b| b.file_name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            format!("Mine 2026-09-26 143005 {id8}.hoplodex"),
            format!("Mine 2026-09-25 143005 {id8}.hoplodex"),
        ]
    );
    assert_eq!(listed[0].made_at, "2026-09-26T14:30:05");
    assert_eq!(listed[0].size_bytes, fs::metadata(&listed[0].path).unwrap().len());
    // Renamed in the file manager, it is still this database's.
    let renamed = shared.join(format!("Old name 2026-09-20 080000 {id8}.hoplodex"));
    fs::rename(&listed[1].path, &renamed).unwrap();
    assert_eq!(world.backups_in(&shared, &id)[1].path, path_text(&renamed));
}

// --- Contents of a backup ---------------------------------------------------

#[test]
fn a_backup_opens_with_the_passphrase_without_the_marker_or_pending_changes() {
    let world = World::new();
    world.create();
    let id = world.database_id();
    world.change();
    world
        .session
        .write(|conn| {
            conn.execute(
                "INSERT INTO pending_changes (id, kind, mode, target_id, label, form_version,
                                              values_json, saved_at)
                 VALUES (1, 'firearm', 'add', NULL, 'New firearm', 1, '{}', ?1)",
                [db::now_utc()],
            )
            .map_err(CommandError::from_db)
        })
        .unwrap();
    world.close();

    let backup = &world.backups_in(&world.default_folder(), &id)[0];
    let conn = peek(Path::new(&backup.path));
    let (marker, made_at, of_name, backup_id): (Option<String>, String, String, String) = conn
        .query_row(
            "SELECT open_machine_id, backup_made_at, backup_of_name, database_id FROM app_state",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(marker, None);
    assert_eq!(made_at, "2026-09-25T12:30:05Z");
    assert_eq!(of_name, "Mine");
    assert_eq!(backup_id, id, "a backup keeps the identity until opened directly");
    let pending: i64 =
        conn.query_row("SELECT count(*) FROM pending_changes", [], |r| r.get(0)).unwrap();
    assert_eq!(pending, 0);
    let firearms: i64 = conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0)).unwrap();
    assert_eq!(firearms, 1);
    assert_eq!(
        World::file_names(&world.default_folder()).len(),
        1,
        "nothing but the backup is left in the folder"
    );
}

#[test]
fn a_backup_opened_directly_becomes_its_own_database() {
    let world = World::new();
    world.create();
    let id = world.database_id();
    world.change();
    world.close();
    let backup = PathBuf::from(&world.backups_in(&world.default_folder(), &id)[0].path);

    lifecycle::open(&world.session, &world.machine, &backup, &passphrase(), false).unwrap();

    let new_id = world.database_id();
    assert_ne!(new_id, id, "research.md §9: a new identity");
    let status = databases::database_status(&world.session, &world.machine).unwrap();
    let note = status.notes.opened_backup.expect("the opened-backup note");
    assert_eq!(note.backup_of_name, "Mine");
    assert_eq!(note.made_at, "2026-09-25T12:30:05Z");
    databases::dismiss_note(&world.session, NoteKind::OpenedBackup).unwrap();
    let status = databases::database_status(&world.session, &world.machine).unwrap();
    assert_eq!(status.notes.opened_backup, None, "shown once");
    world.close();

    let conn = peek(&backup);
    let (stamp, stored_id): (Option<String>, String) = conn
        .query_row("SELECT backup_made_at, database_id FROM app_state", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((stamp, stored_id), (None, new_id));
}

// --- Failures, skips and progress -------------------------------------------

fn assert_failed_and_waiting(world: &World, outcome: CloseOutcome, reason: BackupFailureReason) {
    assert_eq!(
        outcome,
        CloseOutcome { backup: BackupOutcome::Failed, failure_reason: Some(reason) }
    );
    assert_eq!(
        notices(&world.events).last(),
        Some(&json!({
            "kind": "backupFailed",
            "databasePath": path_text(&world.path()),
            "reason": reason,
        }))
    );
    world.open();
    assert!(world.backup_record().0, "the changes stay waiting");
}

#[test]
fn a_missing_backup_location_still_closes_and_leaves_the_changes_waiting() {
    let world = World::new();
    world.create();
    let gone = world.dir.path().join("Unplugged drive/Backups");
    world.set_backups(true, 5, BackupLocationInput::Custom { path: path_text(&gone) });
    world.change();

    let outcome = world.close();

    assert!(!world.session.is_open(), "the close went ahead");
    assert!(!gone.exists(), "a missing custom folder is not made");
    assert_failed_and_waiting(&world, outcome, BackupFailureReason::LocationUnavailable);
}

#[test]
fn an_unwritable_backup_location_fails_the_backup() {
    let world = World::new();
    world.create();
    let locked = world.dir.path().join("Read only");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    world.set_backups(true, 5, BackupLocationInput::Custom { path: path_text(&locked) });
    world.change();

    let outcome = world.close();

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(World::file_names(&locked).is_empty());
    assert_failed_and_waiting(&world, outcome, BackupFailureReason::LocationUnavailable);
}

#[test]
fn a_full_backup_location_fails_the_backup() {
    let world = World::new();
    world.create();
    world.change();
    let size = fs::metadata(world.path()).unwrap().len();
    // Just under the file size + 5%.
    let _space = disk_space::testing::fake_available_space(move |_| Some(size + size / 20 - 1));

    let outcome = world.close();

    assert!(World::file_names(&world.default_folder()).is_empty());
    assert_failed_and_waiting(&world, outcome, BackupFailureReason::InsufficientSpace);
}

#[test]
fn skipping_during_the_copy_removes_the_partial_file_and_leaves_the_changes_waiting() {
    let world = World::new();
    world.create();
    world.change();
    world.grow(3 << 20);
    let operations = world.session.operations_handle();
    world.events.on_event(move |event, payload| {
        if event == "backup:progress" && payload["processed"].as_u64().unwrap() > 0 {
            databases::skip_backup(&operations);
        }
    });

    let outcome = world.close();

    assert_eq!(outcome, CloseOutcome::backup(BackupOutcome::Skipped));
    assert!(World::file_names(&world.default_folder()).is_empty(), "no .partial is left");
    assert_eq!(world.machine.unfinished_backup(), None);
    world.open();
    assert!(world.backup_record().0);
}

#[test]
fn a_close_with_the_file_unreachable_makes_no_backup_and_writes_nothing() {
    let world = World::new();
    world.create();
    world.change();
    let aside = world.dir.path().join("aside.hoplodex");
    fs::rename(world.path(), &aside).unwrap();
    let refused = world.session.write(|_| Ok(()));
    assert_eq!(refused.unwrap_err().code, "DATABASE_UNAVAILABLE");
    fs::rename(&aside, world.path()).unwrap();
    let before = fs::read(world.path()).unwrap();

    let outcome = world.close();

    assert_eq!(fs::read(world.path()).unwrap(), before, "nothing is written");
    assert!(!world.default_folder().exists());
    assert_failed_and_waiting(&world, outcome, BackupFailureReason::DatabaseUnreachable);
}

#[test]
fn the_unfinished_backup_is_recorded_while_it_runs_and_cleared_after() {
    let world = World::new();
    world.create();
    world.change();
    let seen = Arc::new(Mutex::new(None));
    let (record, config) = (seen.clone(), world._config.path().to_owned());
    world.events.on_event(move |event, _| {
        if event == "backup:progress" && record.lock().unwrap().is_none() {
            // What a crash here would leave in machine.json.
            let machine = MachineSettings::load(&config).unwrap();
            *record.lock().unwrap() = machine.unfinished_backup();
        }
    });

    world.close();

    let recorded: Option<UnfinishedBackup> = seen.lock().unwrap().take();
    let recorded = recorded.expect("recorded before the copy");
    assert_eq!(recorded.database_path, world.path());
    assert!(recorded.partial_path.to_string_lossy().ends_with(".hoplodex.partial"));
    assert_eq!(world.machine.unfinished_backup(), None);
}

#[test]
fn a_backup_left_unfinished_is_swept_at_the_next_launch() {
    let world = World::new();
    world.create();
    let id = world.database_id();
    world.change();
    // A crash midway through a copy leaves this behind.
    fs::create_dir_all(world.default_folder()).unwrap();
    let partial = world
        .default_folder()
        .join(format!("Mine 2026-09-25 143005 {}.hoplodex.partial", &id[..8]));
    fs::write(&partial, b"half a backup").unwrap();
    world.machine.set_unfinished_backup(UnfinishedBackup {
        database_path: world.path(),
        partial_path: partial.clone(),
        started_at: "2026-09-25T12:30:05Z".into(),
    });
    drop(world.session.take());

    backups::sweep_unfinished(&world.machine);

    assert!(!partial.exists());
    assert_eq!(world.machine.unfinished_backup(), None);
    assert_eq!(
        world.machine.take_notices(),
        vec![hoplodex_lib::models::database::ChooserNotice::BackupFailed {
            database_path: path_text(&world.path()),
            reason: BackupFailureReason::Interrupted,
        }]
    );
    // SC-004: the database is whole, the changes wait, and the next close
    // backs them up.
    world.open();
    assert_eq!(world.firearm_count(), 1);
    assert!(world.backup_record().0);
    assert_eq!(world.close().backup, BackupOutcome::Made);
}

#[test]
fn the_first_progress_arrives_at_once_and_says_whether_to_show_the_bar() {
    let world = World::new();
    world.create();
    world.change();
    let closing_at = Arc::new(Mutex::new(None::<Instant>));
    let first_progress = Arc::new(Mutex::new(None::<std::time::Duration>));
    let (closing, progress) = (closing_at.clone(), first_progress.clone());
    world.events.on_event(move |event, _| match event {
        "session:closing" => *closing.lock().unwrap() = Some(Instant::now()),
        "backup:progress" if progress.lock().unwrap().is_none() => {
            let since = closing.lock().unwrap().unwrap().elapsed();
            *progress.lock().unwrap() = Some(since);
        }
        _ => {}
    });

    world.close();

    let waited = first_progress.lock().unwrap().expect("a backup:progress event");
    assert!(waited.as_millis() < 100, "SC-005: {waited:?}");
    let first = &world.events.payloads("backup:progress")[0];
    let total = fs::metadata(world.path()).unwrap().len();
    assert_eq!(first, &json!({ "processed": 0, "total": total, "showNow": false }));
    let last = world.events.payloads("backup:progress").pop().unwrap();
    assert_eq!(last["processed"], json!(total));
    // At 50 MiB/s, more than 50 MiB takes over a second: the bar shows at once.
    assert!(!backups::shows_progress_at_once(50 << 20));
    assert!(backups::shows_progress_at_once((50 << 20) + 1));
}

// --- Interruption points (SC-004) -------------------------------------------

/// Stops the backup of a changed, multi-chunk database when `stop_here`
/// says so, then checks nothing was lost or left behind, and that the next
/// allowed close makes the backup.
fn interrupted_backup(stop_here: fn(u64, u64) -> bool) {
    let world = World::new();
    world.create();
    let id = world.database_id();
    world.change();
    world.grow(3 << 20);
    let operations = world.session.operations_handle();
    world.events.on_event(move |event, payload| {
        if event == "backup:progress" {
            let processed = payload["processed"].as_u64().unwrap();
            let total = payload["total"].as_u64().unwrap();
            if stop_here(processed, total) {
                operations.stop_running();
            }
        }
    });

    let outcome = world.close();

    assert_eq!(outcome.backup, BackupOutcome::Skipped);
    assert!(backups::list(&world.default_folder(), &id).unwrap().is_empty());
    backups::sweep_unfinished(&world.machine);
    assert!(World::file_names(&world.default_folder()).is_empty(), "no .partial is left");
    world.events.on_event(|_, _| {});
    world.open();
    assert_eq!(world.firearm_count(), 1);
    assert!(world.backup_record().0, "the changes stay waiting");
    assert_eq!(world.close().backup, BackupOutcome::Made);
}

#[test]
fn a_backup_stopped_before_the_first_chunk_loses_nothing() {
    interrupted_backup(|processed, _| processed == 0);
}

#[test]
fn a_backup_stopped_midway_through_the_copy_loses_nothing() {
    interrupted_backup(|processed, total| processed > 0 && processed < total);
}

#[test]
fn a_backup_stopped_at_finalize_loses_nothing() {
    // The last progress event comes after the last chunk; the stop is then
    // noticed after the stamp, before the rename.
    interrupted_backup(|processed, total| processed == total);
}

// --- Deleting all backups (FR-029, US3-4) -----------------------------------

/// "Mine" with three backups made over three days, left open.
fn with_three_backups(world: &World) -> Vec<BackupInfo> {
    world.create();
    for _ in 0..3 {
        world.change();
        world.close();
        world.open();
        world.next_day();
    }
    let listed = world.backups_in(&world.default_folder(), &world.database_id());
    assert_eq!(listed.len(), 3);
    listed
}

#[test]
fn deleting_all_backups_needs_confirming() {
    let world = World::new();
    with_three_backups(&world);

    let refused = backups_ops::delete_all_backups(&world.session, false);

    assert_eq!(refused.unwrap_err().code, "CONFIRMATION_REQUIRED");
    assert_eq!(world.backups_in(&world.default_folder(), &world.database_id()).len(), 3);
}

#[test]
fn deleting_all_backups_deletes_every_one_and_nothing_else() {
    let world = World::new();
    let listed = with_three_backups(&world);
    world.events.take();
    let unrelated = world.default_folder().join("my notes.txt");
    fs::write(&unrelated, b"keep me").unwrap();
    let other = world.default_folder().join("Other 2026-09-20 101010 0badc0de.hoplodex");
    fs::write(&other, b"another database's backup").unwrap();
    let record = world.backup_record();
    let before = fs::read(world.path()).unwrap();

    let deleted = backups_ops::delete_all_backups(&world.session, true).unwrap();

    assert_eq!(deleted.deleted_count, 3);
    assert!(deleted.failed_paths.is_empty());
    for backup in &listed {
        assert!(!Path::new(&backup.path).exists());
    }
    assert_eq!(
        world.events.payloads("backups_delete:progress"),
        vec![
            json!({ "processed": 1, "total": 3 }),
            json!({ "processed": 2, "total": 3 }),
            json!({ "processed": 3, "total": 3 }),
        ]
    );
    assert_eq!(fs::read(&unrelated).unwrap(), b"keep me");
    assert!(other.exists());
    assert_eq!(fs::read(world.path()).unwrap(), before, "the database is untouched");
    assert_eq!(world.backup_record(), record, "deleting backups is not a change (FR-025)");

    // The next due close backs up as usual.
    world.change();
    assert_eq!(world.close().backup, BackupOutcome::Made);
}

#[test]
fn a_backup_that_cannot_be_deleted_is_reported_and_the_rest_still_go() {
    let world = World::new();
    let listed = with_three_backups(&world);
    let stuck = Path::new(&listed[1].path);
    fs::set_permissions(stuck, fs::Permissions::from_mode(0o444)).unwrap();

    let deleted = backups_ops::delete_all_backups(&world.session, true).unwrap();

    assert_eq!(deleted.deleted_count, 2);
    assert_eq!(deleted.failed_paths, vec![listed[1].path.clone()]);
    assert!(stuck.exists());
    assert!(!Path::new(&listed[0].path).exists());
    assert!(!Path::new(&listed[2].path).exists());
}

#[test]
fn deleting_all_backups_can_be_stopped_between_files() {
    let world = World::new();
    with_three_backups(&world);
    let operations = world.session.operations_handle();
    world.events.on_event(move |event, payload| {
        if event == "backups_delete:progress" && payload["processed"] == json!(1) {
            operations.stop_running();
        }
    });

    let stopped = backups_ops::delete_all_backups(&world.session, true).unwrap_err();

    assert_eq!(stopped.code, "OPERATION_STOPPED");
    assert_eq!(
        stopped.details.as_deref(),
        Some(&json!({ "operation": "deleteBackups", "deletedCount": 1 }))
    );
    assert_eq!(world.backups_in(&world.default_folder(), &world.database_id()).len(), 2);
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
