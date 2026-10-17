//! The upgrade flow end to end: the question, the backup, the transaction and the
//! result (009 research.md §4 to §7, §14). Real SQLCipher files in temp
//! directories; an older layout is a database made before a test-only step
//! (`db::upgrade::testing`) joined the build's list.
//!
//! This file holds the smoke tests of 009 T046 and T049 to T054; the full
//! scenarios of 009 T035 to T038 join it.

use crate::support;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::db::upgrade::testing;
use hoplodex_lib::models::database::CloseReason;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::session::scoped::Target;
use hoplodex_lib::session::{Session, lifecycle, upgrade as session_upgrade};
use support::{ManualClock, TestEvents, passphrase, test_session_at};
use tempfile::TempDir;

const START: &str = "2026-10-17T10:00:00+02:00";

/// A step this build does not have until a test registers it for the folder.
const STEP: (&str, &str) = ("9101_smoke", "CREATE TABLE smoke (id INTEGER PRIMARY KEY);");

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

    fn backups(&self) -> PathBuf {
        self.dir.path().join("HoploDex backups")
    }

    /// Creates the database in the current layout and closes it.
    fn create_and_close(&self) {
        lifecycle::create(&self.session, &self.machine, &self.path(), &passphrase()).unwrap();
        lifecycle::close_normal(
            &self.session,
            self.session.current_id(),
            &self.machine,
            CloseReason::Closed,
        )
        .unwrap();
        self.events.take();
    }

    /// An open that finds `STEP` due: the id the database waits under.
    fn open_asking(&self) -> u64 {
        let err = databases::open_database(
            &self.session,
            &self.machine,
            &self.path().to_string_lossy(),
            databases::Unlock::typed(&passphrase()),
            false,
        )
        .unwrap_err();
        assert_eq!(err.code, "UPGRADE_NEEDED", "{err}");
        let details = err.details.expect("UPGRADE_NEEDED names the waiting database");
        assert_eq!(details["version"], hoplodex_lib::APP_VERSION);
        details["awaitingId"].as_u64().expect("awaitingId")
    }

    fn upgrade(
        &self,
        awaiting_id: u64,
    ) -> Result<hoplodex_lib::models::database::DatabaseStatus, CommandError> {
        databases::upgrade_database(&self.session, &self.machine, awaiting_id, None)
    }
}

fn migration_names(path: &Path) -> Vec<String> {
    let conn = support::peek(path);
    conn.prepare("SELECT name FROM schema_migrations ORDER BY rowid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// 009 FR-010, FR-011, FR-017: the open asks, the answer backs up, upgrades and
/// opens, with the note.
#[test]
fn an_older_database_asks_then_backs_up_upgrades_and_opens() {
    let world = World::new();
    world.create_and_close();
    let _scope = testing::scope(world.dir.path()).extra_steps(&[STEP]);

    let id = world.open_asking();
    assert!(!world.session.is_open(), "nothing is open while the question is up");
    let status = world.upgrade(id).expect("the upgrade");

    let note = status.notes.upgraded.expect("the upgraded note");
    assert_eq!(note.version, hoplodex_lib::APP_VERSION);
    assert!(note.at_backup_location);
    let backup = PathBuf::from(&note.backup_path);
    assert!(backup.starts_with(world.backups()), "{backup:?}");
    assert!(
        backup
            .to_string_lossy()
            .ends_with(&format!(" before {}.hoplodex", hoplodex_lib::APP_VERSION)),
        "{backup:?}"
    );
    assert!(backup.is_file());
    assert!(world.session.is_open());

    let phases: Vec<_> = world
        .events
        .payloads("upgrade:progress")
        .iter()
        .map(|p| p["phase"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(phases.first().map(String::as_str), Some("backingUp"), "{phases:?}");
    assert_eq!(phases.last().map(String::as_str), Some("upgrading"), "{phases:?}");

    // The same id is not an answer twice.
    let again = world.upgrade(id).unwrap_err();
    assert_eq!(again.code, "DATABASE_CLOSED");
    // Close it and read what the upgrade wrote.
    lifecycle::close_normal(
        &world.session,
        world.session.current_id(),
        &world.machine,
        CloseReason::Closed,
    )
    .unwrap();
    assert_eq!(migration_names(&world.path()).last().map(String::as_str), Some(STEP.0));
}

/// 009 FR-010: Cancel while the question is up drops the database and writes
/// nothing; the id then answers nothing.
#[test]
fn cancel_at_the_question_leaves_the_file_untouched() {
    let world = World::new();
    world.create_and_close();
    let before = fs::read(world.path()).unwrap();
    let _scope = testing::scope(world.dir.path()).extra_steps(&[STEP]);

    let id = world.open_asking();
    databases::cancel_upgrade(&world.session, id).unwrap();
    assert_eq!(world.upgrade(id).unwrap_err().code, "DATABASE_CLOSED");
    assert_eq!(fs::read(world.path()).unwrap(), before);
    assert!(!world.backups().exists(), "no backup was made");
}

/// 009 FR-016: Cancel during the backup stops it between chunks, removes the
/// partial file and drops the database.
#[test]
fn cancel_during_the_backup_stops_it() {
    let world = World::new();
    world.create_and_close();
    let before = fs::read(world.path()).unwrap();
    let _scope = testing::scope(world.dir.path()).extra_steps(&[STEP]);
    let id = world.open_asking();

    let session = world.session.clone();
    world.events.on_event(move |event, payload| {
        if event == "upgrade:progress" && payload["phase"] == "backingUp" {
            databases::cancel_upgrade(&session, id).unwrap();
        }
    });
    let err = world.upgrade(id).unwrap_err();
    assert_eq!(err.code, "OPERATION_STOPPED", "{err}");
    assert_eq!(err.details.unwrap()["operation"], "upgrade");
    assert!(
        fs::read_dir(world.backups()).map_or(0, Iterator::count) == 0,
        "the partial backup was removed"
    );
    // Dropped: the id answers nothing, and the file is as it was.
    assert_eq!(world.upgrade(id).unwrap_err().code, "DATABASE_CLOSED");
    assert_eq!(fs::read(world.path()).unwrap(), before);
}

/// 009 FR-019: `check_backup` says whether restoring upgrades, refuses a wrong
/// passphrase, and `restore_backup` refuses an older backup without `upgrade`.
#[test]
fn check_backup_has_three_outcomes_and_restore_needs_the_answer() {
    let world = World::new();
    lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
    // A change, so closing makes a backup.
    world
        .session
        .write(world.session.current_id(), |conn| {
            conn.execute("UPDATE collection_settings SET backups_enabled = 1", [])
                .map_err(CommandError::from_db)?;
            conn.execute("UPDATE app_state SET changes_waiting = 1", [])
                .map_err(CommandError::from_db)
        })
        .unwrap();
    lifecycle::close_normal(
        &world.session,
        world.session.current_id(),
        &world.machine,
        CloseReason::Closed,
    )
    .unwrap();
    let backup = fs::read_dir(world.backups())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .next()
        .expect("a backup was made at close");
    let backup_text = backup.to_string_lossy().into_owned();
    databases::open_database(
        &world.session,
        &world.machine,
        &world.path().to_string_lossy(),
        databases::Unlock::typed(&passphrase()),
        false,
    )
    .unwrap();
    let id = world.session.current_id();

    // Current layout: nothing to upgrade.
    let check = |text: &str| {
        backups_ops::check_backup(
            &world.session,
            &world.machine,
            &backup_text,
            &Passphrase::from_input(text.to_owned()),
            Target::Open(id),
        )
    };
    assert_eq!(check(support::TEST_PASSPHRASE).unwrap().upgrade_to, None);
    // A wrong passphrase.
    assert_eq!(check("not the passphrase").unwrap_err().code, "PASSPHRASE_INCORRECT");
    // Older than the build.
    let _scope = testing::scope(world.dir.path()).extra_steps(&[STEP]);
    assert_eq!(
        check(support::TEST_PASSPHRASE).unwrap().upgrade_to.as_deref(),
        Some(hoplodex_lib::APP_VERSION)
    );

    // The backup before restoring is named by the time, so time moves on.
    world.clock.advance(chrono::Duration::hours(1));
    // Without the answer nothing is copied; with it the copy is upgraded.
    let refused = backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &backup_text,
        &passphrase(),
        Target::Open(id),
        false,
    )
    .unwrap_err();
    assert_eq!(refused.code, "UPGRADE_NEEDED");
    assert_eq!(refused.details.unwrap()["version"], hoplodex_lib::APP_VERSION);
    let status = backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &backup_text,
        &passphrase(),
        Target::Open(id),
        true,
    )
    .unwrap();
    let note = status.notes.upgraded.expect("the upgraded note");
    assert_eq!(note.backup_path, backup_text);
    let phases: Vec<_> = world
        .events
        .payloads("restore:progress")
        .iter()
        .map(|p| p["phase"].as_str().unwrap().to_owned())
        .collect();
    assert!(phases.iter().any(|p| p == "upgrading"), "{phases:?}");
}

/// 009 research.md §4: sleep, screen lock, quit and any other open drop the
/// waiting database, whose id then answers nothing. Another computer's marker
/// written during the question is refused at the answer.
#[test]
fn the_waiting_database_is_dropped_by_sleep_quit_and_another_open() {
    let world = World::new();
    world.create_and_close();
    let _scope = testing::scope(world.dir.path()).extra_steps(&[STEP]);

    let id = world.open_asking();
    lifecycle::will_sleep(&world.session, &world.machine);
    assert_eq!(world.upgrade(id).unwrap_err().code, "DATABASE_CLOSED");

    let id = world.open_asking();
    lifecycle::quit(&world.session, &world.machine);
    assert_eq!(world.upgrade(id).unwrap_err().code, "DATABASE_CLOSED");

    let first = world.open_asking();
    let second = world.open_asking();
    assert_ne!(first, second);
    assert_eq!(world.upgrade(first).unwrap_err().code, "DATABASE_CLOSED");

    // The marker another computer wrote while the question was up.
    let other = support::other_machine();
    session_upgrade::testing::with_waiting_connection(&world.session, second, |conn| {
        conn.execute(
            "UPDATE app_state SET open_machine_id = ?1, open_machine_name = ?2, open_since = ?3",
            [&other.id, &other.display_name, "2026-10-17T09:00:00Z"],
        )
        .unwrap();
    })
    .expect("the database waits");
    let refused = world.upgrade(second).unwrap_err();
    assert_eq!(refused.code, "DATABASE_OPEN_ELSEWHERE", "{refused}");
    assert_eq!(world.upgrade(second).unwrap_err().code, "DATABASE_CLOSED");
}
