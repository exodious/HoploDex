//! The version that last saved a database (008 FR-015, data-model.md
//! `app_state`, 008 research.md §16). Real SQLCipher files in temp
//! directories.
//!
//! Set at creation; inside `write_pending`'s transaction whenever a draft is
//! written (a lock, or the immediate close at sleep or shutdown); and after
//! any `write`, `write_open` or `write_housekeeping` that succeeded and
//! changed rows. Never set by opening, the open marker, closing, a backup, a
//! restore's own bookkeeping, a passphrase change, or a write that changed
//! nothing. Each "set" case starts from a seeded older version, and each
//! "never set" case from a seeded `1.2.0`, so a case can't pass by finding the
//! current version already there.
//!
//! The `Session` calls are all in `World`'s helpers, so a change to the
//! `Session` accessors touches one place.

use crate::support;

use std::path::PathBuf;
use std::sync::Arc;

use hoplodex_lib::APP_VERSION;
use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::db;
use hoplodex_lib::models::database::{CloseReason, Draft, DraftKind, DraftMode, NoteKind};
use hoplodex_lib::services::backups;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::sandbox::Reach;
use hoplodex_lib::session::fingerprint::FileFingerprint;
use hoplodex_lib::session::scoped::Target;
use hoplodex_lib::session::{Session, lifecycle};
use serde_json::json;
use support::{ManualClock, passphrase, peek, test_session_at};
use tempfile::TempDir;

const START: &str = "2026-09-25T14:30:05+02:00";
/// The version a case starts from: not this build's.
const OLD: &str = "1.2.0";
const NEW_PASSPHRASE: &str = "a much longer passphrase of several words";

struct World {
    dir: TempDir,
    _config: TempDir,
    scratch: TempDir,
    machine: MachineSettings,
    session: Session,
    clock: Arc<ManualClock>,
}

impl World {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let clock = ManualClock::at(START);
        let (session, _events) =
            test_session_at(&config.path().join("opened-documents"), clock.clone());
        Self {
            dir: TempDir::new().unwrap(),
            scratch: TempDir::new().unwrap(),
            machine: MachineSettings::load(config.path()).unwrap(),
            _config: config,
            session,
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

    fn close(&self) {
        lifecycle::close_normal(
            &self.session,
            self.session.current_id(),
            &self.machine,
            CloseReason::Closed,
        )
        .unwrap();
    }

    /// Writes `version` as the recorded one, bypassing every rule, in the
    /// open database. The file's fingerprint is refreshed, as a write of the
    /// session's own would, so the seed doesn't look like another computer's
    /// take-over.
    fn seed(&self, version: &str) {
        self.session
            .inspect_mut(self.session.current_id(), |open| {
                open.conn
                    .execute("UPDATE app_state SET last_saved_version = ?1", [version])
                    .map_err(CommandError::from_db)?;
                open.fingerprint = FileFingerprint::capture(&open.path).unwrap();
                Ok(())
            })
            .unwrap();
    }

    fn seed_null(&self) {
        self.session
            .inspect_mut(self.session.current_id(), |open| {
                open.conn
                    .execute("UPDATE app_state SET last_saved_version = NULL", [])
                    .map_err(CommandError::from_db)?;
                open.fingerprint = FileFingerprint::capture(&open.path).unwrap();
                Ok(())
            })
            .unwrap();
    }

    /// The recorded version, in the open database.
    fn saved(&self) -> Option<String> {
        self.session
            .inspect(self.session.current_id(), |open| {
                open.conn
                    .query_row("SELECT last_saved_version FROM app_state", [], |row| row.get(0))
                    .map_err(CommandError::from_db)
            })
            .unwrap()
    }

    /// `changes_waiting`, in the open database.
    fn changes_waiting(&self) -> bool {
        self.session
            .inspect(self.session.current_id(), |open| {
                open.conn
                    .query_row("SELECT changes_waiting FROM app_state", [], |row| row.get(0))
                    .map_err(CommandError::from_db)
            })
            .unwrap()
    }

    fn database_id(&self) -> String {
        self.session
            .inspect(self.session.current_id(), |open| Ok(open.database_id.clone()))
            .unwrap()
    }

    fn add_firearm(&self, serial: &str) -> Result<i64, CommandError> {
        self.session.write(self.session.current_id(), |conn| {
            firearms::create_firearm(conn, &support::firearm("Colt", "Python", serial), false, None)
                .map(|created| created.id)
        })
    }

    fn stage(&self, draft: Option<Draft>) {
        databases::stage_pending_changes(&self.session, self.session.current_id(), draft).unwrap();
    }
}

/// The recorded version of a closed file (or a backup), looked at without
/// opening it.
fn saved_in(path: &std::path::Path) -> Option<String> {
    peek(path).query_row("SELECT last_saved_version FROM app_state", [], |row| row.get(0)).unwrap()
}

fn a_draft() -> Draft {
    Draft {
        form_version: 1,
        kind: DraftKind::Firearm,
        mode: DraftMode::Add,
        target_id: None,
        label: "Glock 19 (new)".into(),
        values: json!({ "make": "Glock", "model": "19" }),
    }
}

/// "Mine" with a firearm and `OLD` recorded, open.
fn seeded() -> World {
    let world = World::new();
    world.create();
    world.add_firearm("A1").unwrap();
    world.seed(OLD);
    world
}

// --- Set: creation -----------------------------------------------------------

#[test]
fn a_new_database_records_this_version() {
    let world = World::new();
    world.create();

    assert_eq!(world.saved().as_deref(), Some(APP_VERSION));
}

#[test]
fn the_column_is_null_until_a_version_is_recorded() {
    let world = World::new();
    world.create();
    world.seed_null();

    assert_eq!(world.saved(), None);
}

#[test]
fn creating_through_the_db_layer_records_it_too() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Direct.hoplodex");
    let conn = db::create_database(&path, &passphrase(), &support::test_machine()).unwrap();

    let saved: Option<String> =
        conn.query_row("SELECT last_saved_version FROM app_state", [], |row| row.get(0)).unwrap();

    assert_eq!(saved.as_deref(), Some(APP_VERSION));
}

// --- Set: the helper ---------------------------------------------------------

#[test]
fn recording_overwrites_an_older_version_and_changes_nothing_when_it_is_current() {
    let world = World::new();
    world.create();
    world.seed(OLD);

    let (first, second) = world
        .session
        .inspect(world.session.current_id(), |open| {
            let before = open.conn.total_changes();
            db::record_saved_version(&open.conn).unwrap();
            let first = open.conn.total_changes() - before;
            db::record_saved_version(&open.conn).unwrap();
            Ok((first, open.conn.total_changes() - before - first))
        })
        .unwrap();

    assert_eq!(world.saved().as_deref(), Some(APP_VERSION));
    assert_eq!(first, 1);
    assert_eq!(second, 0, "already current: no row changes");
}

#[test]
fn recording_it_does_not_make_a_backup_due() {
    let world = World::new();
    world.create();
    assert!(!world.changes_waiting());
    world.seed(OLD);

    world
        .session
        .inspect(world.session.current_id(), |open| {
            db::record_saved_version(&open.conn).unwrap();
            Ok(())
        })
        .unwrap();

    assert_eq!(world.saved().as_deref(), Some(APP_VERSION));
    assert!(!world.changes_waiting(), "app_state has no change-tracking triggers");
}

// --- Set: pending changes (waits for 008 T093) -------------------------------

#[test]
fn a_lock_that_keeps_a_draft_records_this_version() {
    let world = seeded();

    databases::lock_database(
        &world.session,
        world.session.current_id(),
        &world.machine,
        Some(a_draft()),
    )
    .unwrap();

    assert_eq!(saved_in(&world.path()).as_deref(), Some(APP_VERSION));
}

#[test]
fn the_idle_lock_that_keeps_a_staged_draft_records_this_version() {
    let world = seeded();
    world.stage(Some(a_draft()));

    lifecycle::lock_with_staged(&world.session, &world.machine, CloseReason::Idle).unwrap();

    assert_eq!(saved_in(&world.path()).as_deref(), Some(APP_VERSION));
}

#[test]
fn the_immediate_close_at_sleep_or_shutdown_that_keeps_a_draft_records_this_version() {
    for reason in [CloseReason::Sleep, CloseReason::Shutdown] {
        let world = seeded();
        world.stage(Some(a_draft()));

        lifecycle::close_immediate(&world.session, &world.machine, reason);

        assert_eq!(saved_in(&world.path()).as_deref(), Some(APP_VERSION), "{reason:?}");
    }
}

// --- Set: writes that changed rows (wait for 008 T093) -----------------------

#[test]
fn a_write_that_changed_rows_records_this_version() {
    let world = seeded();

    world.add_firearm("B2").unwrap();

    assert_eq!(world.saved().as_deref(), Some(APP_VERSION));
}

#[test]
fn a_write_open_that_changed_rows_records_this_version() {
    let world = seeded();

    world
        .session
        .write_open(world.session.current_id(), |open| {
            firearms::create_firearm(
                &open.conn,
                &support::firearm("Colt", "Python", "B2"),
                false,
                None,
            )
            .map(|_| ())
        })
        .unwrap();

    assert_eq!(world.saved().as_deref(), Some(APP_VERSION));
}

#[test]
fn a_housekeeping_write_that_changed_rows_records_this_version() {
    let world = seeded();

    databases::dismiss_note(&world.session, world.session.current_id(), NoteKind::DiskEncryption)
        .unwrap();

    assert_eq!(world.saved().as_deref(), Some(APP_VERSION));
}

// --- Never set ---------------------------------------------------------------

#[test]
fn opening_and_its_marker_leave_the_version_alone() {
    let world = seeded();
    world.close();

    world.open();

    assert_eq!(world.saved().as_deref(), Some(OLD));
}

#[test]
fn closing_leaves_the_version_alone() {
    let world = seeded();
    world.close();

    assert_eq!(saved_in(&world.path()).as_deref(), Some(OLD));
}

#[test]
fn a_backup_carries_the_version_and_the_original_keeps_it() {
    let world = seeded();
    let id = world.database_id();
    // The firearm is a change, so closing makes a backup.
    world.close();

    let made = backups::list(&world.default_folder(), &id).unwrap();

    assert_eq!(made.len(), 1, "closing with a change waiting makes a backup");
    assert_eq!(saved_in(std::path::Path::new(&made[0].path)).as_deref(), Some(OLD));
    assert_eq!(saved_in(&world.path()).as_deref(), Some(OLD));
}

#[test]
fn a_restore_names_the_version_that_saved_the_backup_not_this_one() {
    let world = seeded();
    let id = world.database_id();
    world.close();
    let backup = backups::list(&world.default_folder(), &id).unwrap().remove(0);
    // The restore backs the current file up first, which needs a later time.
    world.clock.advance(chrono::Duration::hours(1));
    world.open();
    // Saved by this version since the backup (once writes record it).
    world.add_firearm("B2").unwrap();

    backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &backup.path,
        &passphrase(),
        Target::Open(world.session.current_id()),
        false,
        &Reach::Unconfined,
    )
    .unwrap();

    assert_eq!(world.saved().as_deref(), Some(OLD));
}

#[test]
fn a_passphrase_change_leaves_the_version_alone() {
    let world = seeded();

    backups_ops::change_passphrase(
        &world.session,
        world.session.current_id(),
        &world.machine,
        world.scratch.path(),
        &passphrase(),
        &Passphrase::from_input(NEW_PASSPHRASE.to_owned()),
    )
    .unwrap();

    assert_eq!(world.saved().as_deref(), Some(OLD));
}

#[test]
fn a_write_that_changed_nothing_leaves_the_version_alone() {
    let world = seeded();

    // Does nothing at all.
    world.session.write(world.session.current_id(), |_| Ok(())).unwrap();
    world.session.write_open(world.session.current_id(), |_| Ok(())).unwrap();
    world.session.write_housekeeping(world.session.current_id(), |_| Ok(())).unwrap();
    // Matches no row.
    world
        .session
        .write(world.session.current_id(), |conn| {
            conn.execute("UPDATE firearms SET notes = 'x' WHERE id = -1", [])
                .map(|_| ())
                .map_err(CommandError::from_db)
        })
        .unwrap();
    // Refused before it wrote anything.
    let refused = world.session.write(world.session.current_id(), |conn| {
        firearms::create_firearm(conn, &support::firearm("", "Python", "C3"), false, None)
            .map(|_| ())
    });
    assert!(refused.is_err());

    assert_eq!(world.saved().as_deref(), Some(OLD));
}

#[test]
fn a_read_leaves_the_version_alone() {
    let world = seeded();

    world.session.read(world.session.current_id(), |_| Ok(())).unwrap();

    assert_eq!(world.saved().as_deref(), Some(OLD));
}
