//! A database that needs an upgrade waits here, keyed and unchanged, for the
//! user's answer (009 research.md §4, §8; 009 data-model.md "AwaitingUpgrade").
//!
//! `lifecycle::open` calls [`wait_for_answer`] when `db::open_database` returns
//! `Opened::UpgradeDue`; the unscoped commands `upgrade_database` and
//! `cancel_upgrade` call [`upgrade_database`] and [`cancel_upgrade`].

use std::path::Path;

use rusqlite::Connection;

use crate::APP_VERSION;
use crate::commands::CommandError;
use crate::db::upgrade::reach;
use crate::db::{self, SeenMarker, upgrade as runner};
use crate::models::database::{
    OpenedBackupNote, OperationKind, UpgradePhase, UpgradeProgress, UpgradedNote,
};
use crate::services::backups::{
    self, BackupFailure, BackupJob, BackupKind, ChosenFolder, LocationProblem,
};
use crate::services::machine_settings::MachineSettings;
use crate::services::passphrase::Passphrase;
use crate::session::{
    AwaitingBackupStamp, AwaitingGeneration, AwaitingUpgrade, OpenDatabase, Session, SessionId,
    database_name, lifecycle,
};

/// What a successful [`upgrade_database`] leaves for its caller: the database
/// is open in the session.
pub struct Upgraded {
    /// A typed passphrase the open was to save or refresh in the keyring
    /// (003 FR-017, FR-018). It is dropped, and wiped, with this value.
    pub save_passphrase: Option<Passphrase>,
}

/// Puts the database `db::open_database` found in an older layout in the
/// waiting slot (009 research.md §4 step 3), and returns the
/// `UPGRADE_NEEDED` the open fails with. Nothing has been written to the
/// file. If what the question needs can't be read, that failure is returned
/// instead and the connection is dropped.
///
/// `generation` is the slot's generation `lifecycle::open` read when it began,
/// before the key was derived. If the slot was dropped since (a sleep, screen
/// lock or shutdown came meanwhile), the database is not put in it: it is
/// dropped, and the open fails with `DATABASE_CLOSED` (009 FR-010).
pub fn wait_for_answer(
    session: &Session,
    conn: Connection,
    path: &Path,
    steps_due: Vec<&'static str>,
    marker_seen: SeenMarker,
    generation: AwaitingGeneration,
) -> CommandError {
    let waiting = match describe(conn, path, steps_due, marker_seen) {
        Ok(waiting) => waiting,
        Err(err) => return err,
    };
    let error =
        CommandError::upgrade_needed_to_open(waiting.id, APP_VERSION, &waiting.backup_folder);
    if session.wait_for_upgrade(waiting, generation) {
        error
    } else {
        CommandError::database_closed()
    }
}

/// Reads what the question and the upgrade need from the older layout, which
/// keeps those columns (009 data-model.md "What an older layout must keep").
fn describe(
    conn: Connection,
    path: &Path,
    steps_due: Vec<&'static str>,
    marker_seen: SeenMarker,
) -> Result<AwaitingUpgrade, CommandError> {
    let (database_id, made_at, of_name, before_upgrade_to): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT database_id, backup_made_at, backup_of_name, backup_before_upgrade_to
             FROM app_state",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|err| CommandError::from_db(err).located_at(path))?;
    let location: String = conn
        .query_row("SELECT backup_location FROM collection_settings", [], |row| row.get(0))
        .map_err(|err| CommandError::from_db(err).located_at(path))?;
    // A backup opened directly becomes a database of its own when it is
    // upgraded; the identity is chosen now, for the backup before the upgrade
    // to be named with it (009 research.md §8).
    let backup_stamp = match made_at {
        Some(made_at) => {
            let new_database_id = db::random_hex(16).map_err(|err| {
                log::error!("could not make a database id: {err}");
                CommandError::new("INTERNAL_ERROR", "An unexpected error occurred.")
            })?;
            Some(AwaitingBackupStamp {
                new_database_id,
                note: OpenedBackupNote {
                    backup_of_name: of_name.unwrap_or_default(),
                    made_at,
                    before_upgrade_to,
                },
            })
        }
        None => None,
    };
    // The window between the key's derivation and the slot (009 T098).
    reach!(&conn, Point::BeforeWaiting);
    Ok(AwaitingUpgrade {
        id: SessionId::next(),
        conn,
        path: path.to_owned(),
        name: database_name(path),
        database_id,
        steps_due,
        marker_seen,
        backup_stamp,
        save_passphrase: None,
        backup_folder: backups::resolve_folder(path, &location),
        backup_folder_is_default: location == "default",
    })
}

/// `cancel_upgrade` (009 FR-010, FR-016): while the question is up, drops the
/// waiting database, so the connection closes and nothing is written; while
/// its backup is being made, stops the `upgrade` operation, and
/// [`upgrade_database`] then ends with `OPERATION_STOPPED`; once the upgrade has
/// started, and for an `awaiting_id` that isn't current, does nothing.
pub fn cancel_upgrade(session: &Session, awaiting_id: u64) -> Result<(), CommandError> {
    let Some(id) = SessionId::from_request(awaiting_id) else { return Ok(()) };
    let dropped = {
        let mut waiting = session.awaiting();
        if waiting.as_ref().is_some_and(|waiting| waiting.id == id) { waiting.take() } else { None }
    };
    if dropped.is_none() && session.upgrading_id() == id {
        session.operations().stop_by_user(OperationKind::Upgrade);
    }
    Ok(())
}

/// What taking the waiting database out of the slot leaves behind while its
/// upgrade works: the id `cancel_upgrade` finds the operation by, cleared when
/// this is dropped.
struct Working<'a>(&'a Session);

impl Drop for Working<'_> {
    fn drop(&mut self) {
        self.0.set_upgrading_id(SessionId::NONE);
    }
}

/// `upgrade_database` (009 FR-011 to FR-017; 009 contracts/tauri-commands.md
/// "Order"): backs the waiting database up, upgrades it and opens it. The
/// database waits again, for a retry under the same id, when the backup
/// can't be made at its location or folder; every other failure drops it.
///
/// `backup_folder` is a folder the user chose for this one backup
/// (009 FR-012); without it the backup goes to the database's backup location.
pub fn upgrade_database(
    session: &Session,
    machine: &MachineSettings,
    awaiting_id: u64,
    backup_folder: Option<&Path>,
) -> Result<Upgraded, CommandError> {
    let id = SessionId::from_request(awaiting_id).ok_or_else(CommandError::database_closed)?;
    // 1. Take the waiting database, and begin the operation, under the slot's
    // lock so `cancel_upgrade` finds one or the other.
    // The generation is read in the same lock: a sleep, screen lock or
    // shutdown from here on finds the slot empty, and raises it (009 T098).
    let (waiting, generation, operation, _working) = {
        let mut slot = session.awaiting();
        if slot.as_ref().is_none_or(|waiting| waiting.id != id) {
            return Err(CommandError::database_closed());
        }
        let operation = session.operations().begin(OperationKind::Upgrade, None)?;
        operation.set_user_stoppable(true);
        session.set_upgrading_id(id);
        let generation = session.generation(&slot);
        (slot.take().expect("checked above"), generation, operation, Working(session))
    };

    // 2. Another computer may have marked the database open since the
    // question (a shared folder whose locks don't reach this computer).
    let seen_now = SeenMarker::read(&waiting.conn)
        .map_err(|err| CommandError::from_db(err).located_at(&waiting.path))?;
    if seen_now != waiting.marker_seen && seen_now.is_another_computers(&machine.identity()) {
        return Err(CommandError::database_open_elsewhere(
            seen_now.machine_name.as_deref().unwrap_or_default(),
            seen_now.since.as_deref().unwrap_or_default(),
        ));
    }

    // 3 and 4. The folder, its room, and the backup.
    let (folder, make_folder, at_backup_location) = match backup_folder {
        Some(chosen) => match backups::check_chosen_folder(chosen, &waiting.backup_folder) {
            Ok(ChosenFolder { folder, is_backup_location }) => (folder, false, is_backup_location),
            Err(problem) => {
                let error = location_problem(chosen, problem);
                return Err(wait_again(session, waiting, generation, error));
            }
        },
        None => (waiting.backup_folder.clone(), waiting.backup_folder_is_default, true),
    };
    let named_for = waiting
        .backup_stamp
        .as_ref()
        .map_or(waiting.database_id.as_str(), |stamp| stamp.new_database_id.as_str())
        .to_owned();
    let made = {
        let mut progress = |processed: u64, total: u64| {
            emit(
                session,
                UpgradePhase::BackingUp,
                processed,
                total,
                backups::shows_progress_at_once(total),
            );
            if processed == 0 {
                phase_wait(&|| operation.is_cancelled());
            }
        };
        backups::make_backup(
            machine,
            BackupJob {
                conn: &waiting.conn,
                database_path: &waiting.path,
                name: &waiting.name,
                database_id: &named_for,
                folder: &folder,
                make_folder,
                now: session.clock().now(),
                kind: BackupKind::BeforeUpgrade { to_version: APP_VERSION },
                cancel: &|| operation.is_cancelled(),
                progress: &mut progress,
            },
        )
    };
    let backup_path = match made {
        Ok(path) => path,
        Err(BackupFailure::Stopped) => {
            return Err(CommandError::operation_stopped(OperationKind::Upgrade, None, None));
        }
        Err(BackupFailure::Location(problem)) => {
            let error = location_problem(&folder, problem);
            return Err(wait_again(session, waiting, generation, error));
        }
        Err(BackupFailure::InsufficientSpace(short)) => {
            return Err(wait_again(session, waiting, generation, short.into()));
        }
        Err(BackupFailure::Io(err)) => {
            log::error!("the backup before upgrading {} failed: {err}", waiting.path.display());
            return Err(wait_again(
                session,
                waiting,
                generation,
                CommandError::upgrade_backup_failed(&folder),
            ));
        }
    };

    // 5. The upgrade. From here it can't be stopped by the user.
    operation.set_user_stoppable(false);
    emit(session, UpgradePhase::Upgrading, 0, 0, false);
    let mut marker = runner::OpenMarker::now(&machine.identity());
    if let Some(stamp) = &waiting.backup_stamp {
        marker = marker.taking_backup_stamp_as(stamp.new_database_id.clone());
    }
    let upgraded = runner::upgrade(
        &waiting.conn,
        &runner::build_steps(&waiting.path),
        Some(&marker),
        &mut |done, due| {
            emit(session, UpgradePhase::Upgrading, done as u64, due as u64, false);
            if done == due {
                phase_wait(&|| false);
            }
        },
    );
    if let Err(err) = upgraded {
        return Err(CommandError::upgrade_error(
            err,
            &backup_path,
            at_backup_location,
            &waiting.path,
        ));
    }
    // A sleep or shutdown stopped the operation, which it can't do mid-upgrade:
    // the database is upgraded, and stays closed, so this computer no longer
    // has it open (003 FR-032, FR-040).
    if operation.is_cancelled() {
        if let Err(err) = db::clear_open_marker(&waiting.conn) {
            log::error!("could not clear the open marker of {}: {err}", waiting.path.display());
        }
        return Err(CommandError::operation_stopped(OperationKind::Upgrade, None, None));
    }

    // 6. Continue as every open does.
    let AwaitingUpgrade { conn, path, backup_stamp, save_passphrase, .. } = waiting;
    if session.is_open() {
        // Another database was opened meanwhile (the chooser does not allow
        // it): the upgraded one is not put in its place.
        return Err(CommandError::database_closed());
    }
    let mut open: OpenDatabase = lifecycle::prepare(machine, conn, &path)?;
    open.notes.opened_backup = backup_stamp.map(|stamp| stamp.note);
    open.notes.upgraded = Some(UpgradedNote {
        version: APP_VERSION.to_owned(),
        backup_path: backup_path.to_string_lossy().into_owned(),
        at_backup_location,
    });
    session.install(open);
    Ok(Upgraded { save_passphrase })
}

/// The database waits again, as `error` says why it couldn't go on (the
/// location, the room or the copy: a retry may do better). If the slot was
/// dropped meanwhile, by a sleep, screen lock or shutdown that found it empty
/// (which also stopped the operation), the database is dropped instead and the
/// upgrade ends as a stopped one (009 T098).
fn wait_again(
    session: &Session,
    waiting: AwaitingUpgrade,
    generation: AwaitingGeneration,
    error: CommandError,
) -> CommandError {
    if session.put_back_for_upgrade(waiting, generation) {
        error
    } else {
        CommandError::operation_stopped(OperationKind::Upgrade, None, None)
    }
}

fn location_problem(folder: &Path, problem: LocationProblem) -> CommandError {
    CommandError::backup_location_unavailable(folder, problem.as_reason())
}

fn emit(session: &Session, phase: UpgradePhase, processed: u64, total: u64, show_now: bool) {
    let progress = UpgradeProgress { phase, processed, total, show_now };
    match serde_json::to_value(progress) {
        Ok(payload) => session.events().emit(UpgradeProgress::EVENT, payload),
        Err(err) => log::warn!("could not report the upgrade's progress: {err}"),
    }
}

/// An `e2e` build holds each upgrade phase on screen for
/// `HOPLODEX_E2E_UPGRADE_PHASE_SECONDS`, so the screenshot walk and the
/// screen-reader check can see it (009 research.md §14): before the backup's
/// first chunk, stoppable as a chunk is, and before the upgrade's commit. A
/// missing, unparsable or non-positive value waits for nothing. No other build
/// reads it.
#[cfg(feature = "e2e")]
fn phase_wait(cancelled: &dyn Fn() -> bool) {
    let seconds = std::env::var("HOPLODEX_E2E_UPGRADE_PHASE_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0);
    let Some(seconds) = seconds else { return };
    let until = std::time::Instant::now() + std::time::Duration::from_secs_f64(seconds);
    while std::time::Instant::now() < until && !cancelled() {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[cfg(not(feature = "e2e"))]
fn phase_wait(_cancelled: &dyn Fn() -> bool) {}

#[cfg(feature = "upgrade-test-steps")]
pub mod testing {
    use super::*;

    /// Runs `with` on the connection of the database waiting under
    /// `awaiting_id`, for a test that must act while the question is up (write
    /// another computer's marker; set `cache_size`). `None` when no such
    /// database waits.
    pub fn with_waiting_connection<T>(
        session: &Session,
        awaiting_id: u64,
        with: impl FnOnce(&Connection) -> T,
    ) -> Option<T> {
        let id = SessionId::from_request(awaiting_id)?;
        let waiting = session.awaiting();
        waiting.as_ref().filter(|waiting| waiting.id == id).map(|waiting| with(&waiting.conn))
    }
}

/// A test that forces a failure at a point (`Scope::fail_at`) reaches the
/// open as an unexpected error.
#[cfg(feature = "upgrade-test-steps")]
impl From<runner::testing::Injected> for CommandError {
    fn from(err: runner::testing::Injected) -> Self {
        log::error!("{err}");
        Self::new("INTERNAL_ERROR", "An unexpected error occurred.")
    }
}
