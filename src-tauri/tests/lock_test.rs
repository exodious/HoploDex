//! Locking (FR-033–FR-038, SC-010; research.md §14, §15): "lock now", the
//! idle lock on a manual clock, the lock at sleep with the operations it
//! stops, a sleep during a close, finishing on waking, the lock at screen
//! lock and the OS shutdown. Real SQLCipher files in temp directories; the
//! OS's notices are the lifecycle calls `main.rs` makes for them, some from
//! another thread while an operation runs, as they arrive in the app.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::import_export::{ImportSessionStore, ops as import_export};
use hoplodex_lib::db;
use hoplodex_lib::models::database::{
    BackupInfo, BackupLocationInput, BackupOutcome, BackupSettingsInput, BackupSettingsSaved,
    CloseReason, Draft, DraftKind, DraftMode, ExistingBackupsChoice, IdlePauseReason,
    LockSettingsInput, OperationKind, validate_lock_settings_input,
};
use hoplodex_lib::platform::WakeWatchdog;
use hoplodex_lib::services::backups;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use hoplodex_lib::session::{Session, lifecycle};
use serde_json::{Value, json};
use support::{ManualClock, TEST_PASSPHRASE, TestEvents, passphrase, peek, test_session_at};
use tempfile::TempDir;

/// 14:30:05 on 25 September 2026 in a UTC+2 time zone.
const START: &str = "2026-09-25T14:30:05+02:00";

const NEW_PASSPHRASE: &str = "a different and longer passphrase";

/// A throwaway world on a manual clock, shared with the threads that bring
/// the OS's notices.
struct World {
    dir: TempDir,
    config: TempDir,
    machine: Arc<MachineSettings>,
    session: Arc<Session>,
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
            machine: Arc::new(MachineSettings::load(config.path()).unwrap()),
            config,
            session: Arc::new(session),
            events,
            clock,
        }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("Mine.hoplodex")
    }

    fn path_text(&self) -> String {
        self.path().to_string_lossy().into_owned()
    }

    fn backup_folder(&self) -> PathBuf {
        self.dir.path().join("HoploDex backups")
    }

    /// Creates "Mine" and leaves it open.
    fn create(&self) {
        lifecycle::create(&self.session, &self.machine, &self.path(), &passphrase()).unwrap();
    }

    fn open_with(&self, typed: &str) -> Result<(), CommandError> {
        let typed = Passphrase::from_input(typed.to_owned());
        lifecycle::open(&self.session, &self.machine, &self.path(), &typed, false)
    }

    fn open(&self) {
        self.open_with(TEST_PASSPHRASE).unwrap();
    }

    /// A change to the collection: a new firearm.
    fn change(&self) -> i64 {
        let serial = db::random_hex(4).unwrap();
        self.session
            .write(|conn| {
                firearms::create_firearm(
                    conn,
                    &support::firearm("Glock", "19", &serial),
                    false,
                    None,
                )
                .map(|created| created.id)
            })
            .unwrap()
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

    fn firearm_count(&self) -> i64 {
        self.session
            .read(|conn| {
                conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0))
                    .map_err(CommandError::from_db)
            })
            .unwrap()
    }

    fn database_id(&self) -> String {
        self.session.inspect(|open| Ok(open.database_id.clone())).unwrap()
    }

    /// A decrypted document copy, as `open_document` leaves one.
    fn opened_document(&self) -> PathBuf {
        let folder = self.config.path().join("opened-documents");
        fs::create_dir_all(&folder).unwrap();
        let copy = folder.join("receipt.pdf");
        fs::write(&copy, b"%PDF-1.4 decrypted").unwrap();
        copy
    }

    fn set_lock(&self, idle_enabled: bool, idle_minutes: i64, on_screen_lock: bool) {
        databases::update_lock_settings(
            &self.session,
            &LockSettingsInput { idle_enabled, idle_minutes, on_screen_lock },
        )
        .unwrap();
    }

    /// The closed file's pending changes, backup record and open marker.
    fn closed_file(&self) -> (i64, bool, bool) {
        peek(&self.path())
            .query_row(
                "SELECT (SELECT count(*) FROM pending_changes), changes_waiting,
                        open_machine_id IS NOT NULL
                 FROM app_state",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
    }

    fn closed_reasons(&self) -> Vec<Value> {
        self.events.payloads("session:closed").into_iter().map(|p| p["reason"].clone()).collect()
    }

    fn notices(&self) -> Vec<Value> {
        self.events.payloads("notice")
    }

    fn minutes(&self, minutes: i64) {
        self.clock.advance(chrono::Duration::minutes(minutes));
    }

    fn seconds(&self, seconds: i64) {
        self.clock.advance(chrono::Duration::seconds(seconds));
    }

    fn tick(&self) -> bool {
        lifecycle::idle_tick(&self.session, &self.machine)
    }

    fn note_activity(&self) {
        databases::note_activity(&self.session);
    }

    /// The OS says the computer is going to sleep, from its own thread, as
    /// it does while an operation runs here. Returns once the lock's first
    /// step has asked the operation to stop.
    fn sleep_from_another_thread(&self) -> JoinHandle<()> {
        let (session, machine) = (Arc::clone(&self.session), Arc::clone(&self.machine));
        let sleeper = thread::spawn(move || lifecycle::will_sleep(&session, &machine));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.session.closing_immediately() {
            assert!(Instant::now() < deadline, "the sleep never began");
            thread::sleep(Duration::from_millis(1));
        }
        sleeper
    }

    /// Makes the sleep arrive at the first `event` that `when` picks, from
    /// the thread running the operation.
    fn sleep_at(
        self: &Arc<Self>,
        event: &'static str,
        when: impl Fn(&Value) -> bool + Send + Sync + 'static,
    ) -> Arc<Mutex<Option<JoinHandle<()>>>> {
        let sleeper = Arc::new(Mutex::new(None));
        let (world, slot, fired) = (Arc::clone(self), Arc::clone(&sleeper), AtomicBool::new(false));
        self.events.on_event(move |name, payload| {
            if name == event && when(payload) && !fired.swap(true, Ordering::SeqCst) {
                *slot.lock().unwrap() = Some(world.sleep_from_another_thread());
            }
        });
        sleeper
    }
}

fn join(sleeper: &Mutex<Option<JoinHandle<()>>>) {
    sleeper.lock().unwrap().take().expect("the sleep came").join().unwrap();
}

fn edit_draft(id: i64) -> Draft {
    Draft {
        form_version: 1,
        kind: DraftKind::Firearm,
        mode: DraftMode::Edit,
        target_id: Some(id),
        label: "Glock 19 (edit)".into(),
        values: json!({ "notes": "half typed" }),
    }
}

fn stopped_notice(world: &World, operation: &str, extra: Value) -> Value {
    let mut notice = json!({
        "kind": "operationStopped",
        "databasePath": world.path_text(),
        "operation": operation,
    });
    notice.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    notice
}

// --- Lock now (FR-033, FR-035) -----------------------------------------------

#[test]
fn lock_now_keeps_the_draft_then_closes_like_a_normal_close() {
    let world = World::new();
    world.create();
    let id = world.change();
    let copy = world.opened_document();

    let outcome =
        databases::lock_database(&world.session, &world.machine, Some(edit_draft(id))).unwrap();

    assert_eq!(outcome.backup, BackupOutcome::Made, "a due backup is made");
    assert!(!world.session.is_open());
    assert!(!copy.exists(), "decrypted document copies are deleted");
    let (pending, changes_waiting, marked) = world.closed_file();
    assert_eq!(pending, 1);
    assert!(!changes_waiting);
    assert!(!marked, "the open marker is cleared");
    assert_eq!(world.closed_reasons(), vec![json!("lockedByUser")]);
    assert!(world.notices().contains(&json!({
        "kind": "closed",
        "reason": "lockedByUser",
        "databasePath": world.path_text(),
    })));
    let chooser = databases::chooser_state(&world.session, &world.machine, None, None);
    assert_eq!(chooser.selected_path, Some(world.path_text()), "the locked database is selected");
    world.open();
    assert_eq!(world.firearm_count_after_discard(), 1);
}

impl World {
    fn firearm_count_after_discard(&self) -> i64 {
        databases::resolve_pending_changes(
            &self.session,
            hoplodex_lib::models::database::PendingAction::Discard,
        )
        .unwrap();
        self.firearm_count()
    }
}

#[test]
fn lock_now_with_nothing_open_is_refused() {
    let world = World::new();

    let refused = databases::lock_database(&world.session, &world.machine, None).unwrap_err();

    assert_eq!(refused.code, "DATABASE_CLOSED");
}

// --- The idle lock (FR-034, FR-035, SC-010) ----------------------------------

#[test]
fn the_idle_lock_locks_between_ten_and_eleven_minutes_after_the_last_input() {
    let world = World::new();
    world.create();
    world.note_activity();

    world.minutes(9);
    world.seconds(59);
    assert!(!world.tick());
    assert!(world.session.is_open());

    world.seconds(1);
    assert!(world.tick());
    assert!(!world.session.is_open());
    assert_eq!(world.closed_reasons(), vec![json!("idle")]);
    assert!(world.notices().contains(&json!({
        "kind": "closed",
        "reason": "idle",
        "databasePath": world.path_text(),
        "idleMinutes": 10,
    })));
}

#[test]
fn input_starts_the_idle_time_again() {
    let world = World::new();
    world.create();

    world.minutes(9);
    world.note_activity();
    world.minutes(9);
    assert!(!world.tick(), "only nine minutes since the last input");
    world.minutes(1);
    assert!(world.tick());
}

#[test]
fn a_running_operation_pauses_the_idle_clock_and_it_starts_again_after() {
    let world = World::new();
    world.create();

    {
        let _deleting =
            world.session.operations().begin(OperationKind::DeleteBackups, None).unwrap();
        world.minutes(30);
        assert!(!world.tick());
    }
    assert!(!world.tick(), "the idle time starts again once it has finished");
    world.minutes(9);
    assert!(!world.tick());
    world.minutes(1);
    assert!(world.tick());
}

#[test]
fn a_native_dialog_pauses_the_idle_clock_and_it_starts_again_after() {
    let world = World::new();
    world.create();

    databases::set_idle_paused(&world.session, IdlePauseReason::NativeDialog, true);
    world.minutes(30);
    assert!(!world.tick());
    databases::set_idle_paused(&world.session, IdlePauseReason::NativeDialog, false);
    world.minutes(9);
    assert!(!world.tick());
    world.minutes(1);
    assert!(world.tick());
}

#[test]
fn a_jump_in_wall_time_longer_than_the_duration_locks_at_the_next_tick() {
    let world = World::new();
    world.create();

    // Asleep for half an hour, with no notice of it.
    world.minutes(30);

    assert!(world.tick());
    assert!(!world.session.is_open());
}

#[test]
fn with_the_idle_lock_off_it_never_locks() {
    let world = World::new();
    world.create();
    world.set_lock(false, 10, false);

    world.minutes(24 * 60);

    assert!(!world.tick());
    assert!(world.session.is_open());
}

#[test]
fn saving_the_lock_settings_starts_the_idle_time_again() {
    let world = World::new();
    world.create();

    world.minutes(9);
    world.set_lock(true, 10, false);
    world.minutes(9);
    assert!(!world.tick());
    world.set_lock(true, 1, false);
    world.minutes(1);
    assert!(world.tick(), "the new duration applies at once");
}

#[test]
fn the_idle_lock_keeps_its_settings_in_the_database() {
    let world = World::new();
    world.create();
    world.set_lock(true, 2, true);
    lifecycle::close_normal(&world.session, &world.machine, CloseReason::Closed).unwrap();

    world.open();

    let status = databases::database_status(&world.session, &world.machine).unwrap();
    assert_eq!((status.settings.lock.idle_minutes, status.settings.lock.on_screen_lock), (2, true));
    world.minutes(2);
    assert!(world.tick());
}

#[test]
fn lock_settings_allow_one_to_two_hundred_and_forty_minutes() {
    let input = |idle_minutes| LockSettingsInput {
        idle_enabled: true,
        idle_minutes,
        on_screen_lock: false,
    };
    for minutes in [1, 10, 240] {
        validate_lock_settings_input(&input(minutes)).unwrap();
    }
    for minutes in [0, -5, 241] {
        let refused = validate_lock_settings_input(&input(minutes)).unwrap_err();
        assert_eq!(refused.code, "VALIDATION_ERROR");
        assert!(refused.field_errors.unwrap().contains_key("idleMinutes"));
    }
    let world = World::new();
    world.create();
    let refused = databases::update_lock_settings(&world.session, &input(0)).unwrap_err();
    assert_eq!(refused.code, "VALIDATION_ERROR");
}

// --- Sleep, with the idle lock on (FR-037) -----------------------------------

#[test]
fn a_sleep_announces_the_close_before_the_connection_closes() {
    let world = World::new();
    world.create();
    let open_when_announced = Arc::new(Mutex::new(Vec::new()));
    let (seen, session) = (Arc::clone(&open_when_announced), Arc::clone(&world.session));
    world.events.on_event(move |event, _| {
        if event == "session:closed" {
            seen.lock().unwrap().push(session.is_open());
        }
    });

    lifecycle::will_sleep(&world.session, &world.machine);

    assert_eq!(*open_when_announced.lock().unwrap(), vec![true]);
    assert!(!world.session.is_open());
}

#[test]
fn a_sleep_makes_no_backup_and_leaves_the_changes_waiting() {
    let world = World::new();
    world.create();
    let id = world.change();
    databases::stage_pending_changes(&world.session, Some(edit_draft(id))).unwrap();
    let copy = world.opened_document();

    lifecycle::will_sleep(&world.session, &world.machine);

    assert!(!world.session.is_open());
    assert!(world.events.payloads("backup:progress").is_empty());
    assert!(!world.backup_folder().exists(), "no backup is started");
    assert!(!copy.exists());
    assert_eq!(
        world.closed_file(),
        (1, true, false),
        "draft kept, changes waiting, marker cleared"
    );
    assert_eq!(world.closed_reasons(), vec![json!("sleep")]);
    assert!(world.notices().contains(&json!({
        "kind": "closed",
        "reason": "sleep",
        "databasePath": world.path_text(),
    })));
}

#[test]
fn a_sleep_with_the_idle_lock_off_leaves_an_open_database_open() {
    let world = World::new();
    world.create();
    world.set_lock(false, 10, false);

    lifecycle::will_sleep(&world.session, &world.machine);

    assert!(world.session.is_open());
    assert_eq!(world.events.payloads("system:clear-passphrase-fields").len(), 1);
}

#[test]
fn a_sleep_stops_an_import_after_the_row_in_progress() {
    let world = World::new();
    world.create();
    let file = world.dir.path().join("import.csv");
    let rows: Vec<String> =
        (1..=5).map(|n| support::csv_firearm("Colt", "Python", &format!("P{n}"), &[])).collect();
    fs::write(&file, support::csv_file(&rows)).unwrap();
    let store = ImportSessionStore::new();

    let operation = world.session.operations().begin(OperationKind::Import, None).unwrap();
    let mut sleeper = None;
    let imported = world.session.write(|conn| {
        import_export::import_collection_stoppable(
            conn,
            &support::import_files(&file, SpreadsheetFormat::Csv),
            &store,
            &mut |processed, _| {
                if processed == 2 && sleeper.is_none() {
                    sleeper = Some(world.sleep_from_another_thread());
                }
            },
            &|| operation.is_cancelled(),
            &|count| operation.record_done(count),
        )
    });
    drop(operation);
    sleeper.unwrap().join().unwrap();

    let stopped = imported.unwrap_err();
    assert_eq!(stopped.code, "OPERATION_STOPPED");
    assert_eq!(stopped.details.as_deref().unwrap()["importedCount"], json!(2));
    assert!(!world.session.is_open());
    assert!(world.notices().contains(&stopped_notice(
        &world,
        "import",
        json!({ "importedCount": 2 })
    )));
    world.open();
    assert_eq!(world.firearm_count(), 2, "the rows imported before the stop are kept");
}

#[test]
fn a_sleep_abandons_an_export_and_removes_what_it_wrote() {
    let world = World::new();
    world.create();
    let ids = [world.change(), world.change(), world.change()];
    let destination = TempDir::new().unwrap();

    let operation = world.session.operations().begin(OperationKind::Export, None).unwrap();
    let mut sleeper = None;
    let exported = world.session.read(|conn| {
        import_export::export_collection_stoppable(
            conn,
            destination.path(),
            "export",
            SpreadsheetFormat::Csv,
            &support::firearm_records(&ids),
            &mut |processed, _| {
                if processed == 1 && sleeper.is_none() {
                    sleeper = Some(world.sleep_from_another_thread());
                }
            },
            &|| operation.is_cancelled(),
        )
    });
    drop(operation);
    sleeper.unwrap().join().unwrap();

    assert_eq!(exported.unwrap_err().code, "OPERATION_STOPPED");
    assert_eq!(fs::read_dir(destination.path()).unwrap().count(), 0, "no partial export is left");
    assert!(world.notices().contains(&stopped_notice(&world, "export", json!({}))));
}

#[test]
fn a_sleep_interrupts_a_passphrase_change_and_the_old_passphrase_still_opens() {
    let world = Arc::new(World::new());
    world.create();
    world.grow(3 << 20);
    let count = world.firearm_count();
    let sleeper = world.sleep_at("passphrase_change:progress", |payload| {
        payload["phase"] == "copying" && payload["processed"].as_u64().unwrap() > 0
    });

    let changed = backups_ops::change_passphrase(
        &world.session,
        &world.machine,
        &world.config.path().join("probes"),
        &passphrase(),
        &Passphrase::from_input(NEW_PASSPHRASE.to_owned()),
    );
    join(&sleeper);
    world.events.on_event(|_, _| {});

    assert_eq!(changed.unwrap_err().code, "OPERATION_STOPPED");
    assert!(!world.session.is_open());
    assert!(world.notices().contains(&stopped_notice(&world, "passphraseChange", json!({}))));
    let leftovers: Vec<_> = fs::read_dir(world.dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != "Mine.hoplodex")
        .collect();
    assert!(leftovers.is_empty(), "no partial copy is left: {leftovers:?}");
    assert_eq!(world.open_with(NEW_PASSPHRASE).unwrap_err().code, "PASSPHRASE_INCORRECT");
    world.open();
    assert_eq!(world.firearm_count(), count);
}

#[test]
fn a_sleep_abandons_a_restore_and_leaves_the_database_as_it_was() {
    let world = Arc::new(World::new());
    world.create();
    world.change();
    lifecycle::close_normal(&world.session, &world.machine, CloseReason::Closed).unwrap();
    world.open();
    world.change();
    let count = world.firearm_count();
    let backup = backups::list(&world.backup_folder(), &world.database_id()).unwrap().remove(0);
    let sleeper = world.sleep_at("restore:progress", |_| true);

    let restored = backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &backup.path,
        &passphrase(),
        None,
    );
    join(&sleeper);
    world.events.on_event(|_, _| {});

    assert_eq!(restored.unwrap_err().code, "OPERATION_STOPPED");
    assert!(!world.session.is_open());
    assert!(world.notices().contains(&stopped_notice(&world, "restore", json!({}))));
    world.open();
    assert_eq!(world.firearm_count(), count, "the database is as it was");
}

#[test]
fn a_sleep_stops_deleting_the_backups_between_files() {
    let world = Arc::new(World::new());
    world.create();
    for _ in 0..3 {
        world.change();
        lifecycle::close_normal(&world.session, &world.machine, CloseReason::Closed).unwrap();
        world.open();
        world.clock.advance(chrono::Duration::days(1));
    }
    let id = world.database_id();
    assert_eq!(backups::list(&world.backup_folder(), &id).unwrap().len(), 3);
    let sleeper = world.sleep_at("backups_delete:progress", |payload| payload["processed"] == 1);

    let deleted = backups_ops::delete_all_backups(&world.session, true);
    join(&sleeper);
    world.events.on_event(|_, _| {});

    let stopped = deleted.unwrap_err();
    assert_eq!(stopped.code, "OPERATION_STOPPED");
    assert_eq!(stopped.details.as_deref().unwrap()["deletedCount"], json!(1));
    let left = backups::list(&world.backup_folder(), &id).unwrap();
    assert_eq!(left.len(), 2, "the rest are still there");
    for backup in &left {
        let firearms: i64 = peek(Path::new(&backup.path))
            .query_row("SELECT count(*) FROM firearms", [], |row| row.get(0))
            .unwrap();
        assert!(firearms > 0, "{} is whole", backup.file_name);
    }
    assert!(world.notices().contains(&stopped_notice(
        &world,
        "deleteBackups",
        json!({ "deletedCount": 1 })
    )));
}

/// "Mine" with three backups in its default folder, each made on its own
/// day, left open; each several chunks long when `large`. Oldest first.
fn with_three_backups(world: &World, large: bool) -> Vec<(BackupInfo, Vec<u8>)> {
    world.create();
    if large {
        world.grow(3 * 1024 * 1024);
    }
    for _ in 0..3 {
        world.change();
        lifecycle::close_normal(&world.session, &world.machine, CloseReason::Closed).unwrap();
        world.open();
        world.clock.advance(chrono::Duration::days(1));
    }
    let mut listed = backups::list(&world.backup_folder(), &world.database_id()).unwrap();
    listed.reverse();
    listed
        .into_iter()
        .map(|backup| {
            let bytes = fs::read(&backup.path).unwrap();
            (backup, bytes)
        })
        .collect()
}

/// Changes the backup location to `folder`, doing `choice` with the
/// backups at the old one.
fn change_location(
    world: &World,
    folder: &Path,
    choice: ExistingBackupsChoice,
) -> Result<BackupSettingsSaved, CommandError> {
    databases::update_backup_settings(
        &world.session,
        &world.machine,
        &BackupSettingsInput {
            enabled: true,
            keep_count: 5,
            location: BackupLocationInput::Custom { path: folder.to_string_lossy().into_owned() },
            existing_backups: Some(choice),
        },
    )
}

fn saved_backup_folder(world: &World) -> PathBuf {
    let status = databases::database_status(&world.session, &world.machine).unwrap();
    PathBuf::from(status.settings.backups.location.path)
}

#[test]
fn a_sleep_stops_moving_the_backups_between_files_and_keeps_the_new_location() {
    let world = Arc::new(World::new());
    let made = with_three_backups(&world, true);
    let new = world.dir.path().join("elsewhere");
    fs::create_dir(&new).unwrap();
    let old = world.backup_folder();
    // Copied, as between drives, and stopped midway through the second.
    let _copying = backups::testing::fail_hard_links();
    let first = made[0].0.size_bytes * 2;
    let sleeper = world.sleep_at("backups_move:progress", move |payload| {
        payload["processed"].as_u64().unwrap() > first
    });

    let moved = change_location(&world, &new, ExistingBackupsChoice::Move);
    join(&sleeper);
    world.events.on_event(|_, _| {});

    let stopped = moved.unwrap_err();
    assert_eq!(stopped.code, "OPERATION_STOPPED");
    let left = json!({ "leftBehindCount": 2, "folder": old.to_string_lossy() });
    assert_eq!(
        stopped.details.as_deref(),
        Some(&json!({ "operation": "moveBackups", "leftBehindCount": 2,
                      "folder": old.to_string_lossy() }))
    );
    assert_eq!(fs::read(new.join(&made[0].0.file_name)).unwrap(), made[0].1, "moved");
    for (backup, bytes) in &made[1..] {
        assert_eq!(&fs::read(&backup.path).unwrap(), bytes, "{} kept", backup.file_name);
        assert!(!new.join(&backup.file_name).exists());
    }
    let partials: Vec<_> = fs::read_dir(&new)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().ends_with(".partial"))
        .collect();
    assert!(partials.is_empty(), "the partial copy is removed");
    assert!(world.notices().contains(&stopped_notice(&world, "moveBackups", left)));
    assert_eq!(world.machine.unfinished_backup_move(), None);
    world.open();
    assert_eq!(saved_backup_folder(&world), new, "the new location is kept");
}

#[test]
fn a_sleep_stops_deleting_the_backups_at_a_location_change_and_keeps_the_old_location() {
    let world = Arc::new(World::new());
    let made = with_three_backups(&world, false);
    let new = world.dir.path().join("elsewhere");
    fs::create_dir(&new).unwrap();
    let sleeper = world.sleep_at("backups_delete:progress", |payload| payload["processed"] == 1);

    let deleted = change_location(&world, &new, ExistingBackupsChoice::Delete);
    join(&sleeper);
    world.events.on_event(|_, _| {});

    let stopped = deleted.unwrap_err();
    assert_eq!(stopped.code, "OPERATION_STOPPED");
    assert_eq!(
        stopped.details.as_deref(),
        Some(&json!({ "operation": "deleteBackups", "deletedCount": 1 }))
    );
    world.open();
    assert_eq!(saved_backup_folder(&world), world.backup_folder(), "the old location is kept");
    assert_eq!(backups::list(&world.backup_folder(), &world.database_id()).unwrap().len(), 2);
    assert_eq!(made.len(), 3);
}

#[test]
fn a_running_move_of_backups_pauses_the_idle_clock() {
    let world = Arc::new(World::new());
    with_three_backups(&world, false);
    let new = world.dir.path().join("elsewhere");
    fs::create_dir(&new).unwrap();
    let locked = Arc::new(AtomicBool::new(false));
    let (during, locked_during) = (Arc::clone(&world), Arc::clone(&locked));
    world.events.on_event(move |name, payload| {
        if name == "backups_move:progress" && payload["processed"] == 0 {
            assert_eq!(
                during.session.operations().running_kind(),
                Some(OperationKind::MoveBackups)
            );
            during.minutes(30);
            locked_during.store(during.tick(), Ordering::SeqCst);
        }
    });

    change_location(&world, &new, ExistingBackupsChoice::Move).unwrap();
    world.events.on_event(|_, _| {});

    assert!(!locked.load(Ordering::SeqCst), "no idle lock while the move runs");
    assert!(!world.tick(), "the idle time starts again once it has finished");
    world.minutes(9);
    assert!(!world.tick());
    world.minutes(1);
    assert!(world.tick());
}

// --- A sleep during a close already under way (FR-037, FR-038) -------------

/// A changed, multi-chunk database whose close is made to sleep midway
/// through its backup by `close`.
fn sleep_during_the_backup_of(close: impl FnOnce(&World)) {
    let world = Arc::new(World::new());
    world.create();
    // The idle lock off: a close under way turns immediate anyway.
    world.set_lock(false, 10, true);
    world.change();
    world.grow(3 << 20);
    let sleeper = world.sleep_at("backup:progress", |payload| {
        let (processed, total) =
            (payload["processed"].as_u64().unwrap(), payload["total"].as_u64().unwrap());
        processed > 0 && processed < total
    });

    close(&world);
    join(&sleeper);
    world.events.on_event(|_, _| {});

    assert!(!world.session.is_open());
    let leftovers: Vec<_> = fs::read_dir(world.backup_folder())
        .map(|entries| entries.map(|e| e.unwrap().file_name()).collect())
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "the partial backup is removed: {leftovers:?}");
    let (_, changes_waiting, marked) = world.closed_file();
    assert!(changes_waiting, "its changes stay waiting");
    assert!(!marked, "the close finishes as a lock at sleep");
    assert_eq!(world.closed_reasons(), vec![json!("sleep")], "announced once, as a sleep");
    assert!(world.notices().contains(&stopped_notice(&world, "backup", json!({}))));
}

#[test]
fn a_sleep_during_the_screen_lock_backup_finishes_the_close_at_once() {
    sleep_during_the_backup_of(|world| lifecycle::screen_locked(&world.session, &world.machine));
}

#[test]
fn a_sleep_during_a_user_closes_backup_finishes_the_close_at_once() {
    sleep_during_the_backup_of(|world| {
        let outcome = lifecycle::close_normal(&world.session, &world.machine, CloseReason::Closed);
        assert_eq!(outcome.unwrap().backup, BackupOutcome::Skipped);
    });
}

// --- Finishing on waking (FR-037, research.md §14) -------------------------

#[test]
fn a_sleep_lock_cut_short_after_its_first_step_is_finished_on_waking() {
    let world = World::new();
    world.create();
    let id = world.change();
    databases::stage_pending_changes(&world.session, Some(edit_draft(id))).unwrap();
    let copy = world.opened_document();

    assert!(lifecycle::begin_immediate(&world.session, CloseReason::Sleep));
    // No command reaches the database from the first step on.
    assert_eq!(world.session.read(|_| Ok(())).unwrap_err().code, "DATABASE_CLOSED");
    assert_eq!(
        databases::database_status(&world.session, &world.machine).unwrap_err().code,
        "DATABASE_CLOSED"
    );
    lifecycle::finish_on_wake(&world.session, &world.machine);

    assert!(!world.session.is_open());
    assert!(!copy.exists());
    assert_eq!(world.closed_file(), (1, true, false));
}

#[test]
fn a_wake_notices_an_unseen_sleep_and_locks_with_the_idle_lock_on() {
    let world = World::new();
    world.create();

    lifecycle::finish_on_wake(&world.session, &world.machine);

    assert!(!world.session.is_open());
    assert_eq!(world.closed_reasons(), vec![json!("sleep")]);
}

#[test]
fn a_wake_leaves_the_database_open_with_the_idle_lock_off() {
    let world = World::new();
    world.create();
    world.set_lock(false, 10, false);

    lifecycle::finish_on_wake(&world.session, &world.machine);

    assert!(world.session.is_open());
}

#[test]
fn the_wake_watchdog_reports_wall_time_running_ahead_of_monotonic_time() {
    let wall = SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_000_000);
    let monotonic = Instant::now();
    let mut watchdog = WakeWatchdog::new(wall, monotonic);

    let second = Duration::from_secs(1);
    assert!(!watchdog.observe(wall + second, monotonic + second), "an ordinary tick");
    assert!(
        !watchdog.observe(wall + second * 7, monotonic + second * 2),
        "exactly 5 s ahead is not yet a sleep"
    );
    assert!(watchdog.observe(wall + second * 14, monotonic + second * 3), "6 s ahead is");
    assert!(!watchdog.observe(wall + second * 15, monotonic + second * 4));
    assert!(
        !watchdog.observe(wall + second * 10, monotonic + second * 5),
        "a clock set back is not a sleep"
    );
}

// --- The screen lock (FR-038) ------------------------------------------------

#[test]
fn with_the_option_on_a_screen_lock_locks_as_a_normal_close() {
    let world = World::new();
    world.create();
    world.set_lock(true, 10, true);
    world.change();

    lifecycle::screen_locked(&world.session, &world.machine);

    assert!(!world.session.is_open());
    assert_eq!(world.closed_reasons(), vec![json!("screenLocked")]);
    let (_, changes_waiting, _) = world.closed_file();
    assert!(!changes_waiting, "a due backup was made");
}

#[test]
fn with_the_option_off_a_screen_lock_changes_nothing() {
    let world = World::new();
    world.create();

    lifecycle::screen_locked(&world.session, &world.machine);

    assert!(world.session.is_open());
    assert!(world.closed_reasons().is_empty());
}

#[test]
fn passphrase_fields_clear_at_sleep_and_screen_lock_whatever_is_open() {
    let world = World::new();

    lifecycle::will_sleep(&world.session, &world.machine);
    lifecycle::screen_locked(&world.session, &world.machine);

    assert_eq!(world.events.payloads("system:clear-passphrase-fields").len(), 2);
}

// --- Shutdown (FR-039) -------------------------------------------------------

#[test]
fn a_shutdown_keeps_the_draft_deletes_document_copies_and_makes_no_backup() {
    let world = World::new();
    world.create();
    let id = world.change();
    databases::stage_pending_changes(&world.session, Some(edit_draft(id))).unwrap();
    let copy = world.opened_document();

    lifecycle::will_shut_down(&world.session, &world.machine);

    assert!(!world.session.is_open());
    assert!(!copy.exists());
    assert!(!world.backup_folder().exists());
    assert_eq!(world.closed_file(), (1, true, false));
    assert_eq!(world.closed_reasons(), vec![json!("shutdown")]);
}
