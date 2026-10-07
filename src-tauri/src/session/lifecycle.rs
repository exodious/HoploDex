//! Opening and closing, as ordered steps (data-model.md "Session states and
//! transitions"). The commands in `commands::databases` are thin wrappers
//! over these, and the tests call them directly.

use std::io;
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension};
use serde_json::json;

use crate::commands::CommandError;
use crate::commands::databases::ops::folder_error;
use crate::db;
use crate::models::database::{
    BackupFailureReason, BackupOutcome, ChooserNotice, CloseOutcome, CloseReason, Draft,
    OpenedBackupNote, OperationKind,
};
use crate::services::backups::{self, BackupFailure, BackupJob, Due};
use crate::services::file_swap;
use crate::services::machine_settings::{self, MachineSettings};
use crate::services::passphrase::Passphrase;
use crate::services::suggestions;
use crate::session::fingerprint::FingerprintCheck;
use crate::session::operations::StoppedOperation;
use crate::session::{ImmediateClose, OpenDatabase, Session, SessionInner, pending};

/// How long a sleep or shutdown waits for a close already under way, or
/// for a stopped operation to unwind. The OS allows a few seconds at most
/// (research.md §14); what is not done by then is finished on waking.
const IMMEDIATE_WAIT: Duration = Duration::from_secs(10);

/// Creates a database at `path` and makes it the open one, closing any other
/// open database first as a switch.
pub fn create(
    session: &Session,
    machine: &MachineSettings,
    path: &Path,
    passphrase: &Passphrase,
) -> Result<(), CommandError> {
    let conn =
        db::create_database(path, passphrase, &machine.identity()).map_err(|err| match err {
            // The folder passed its checks but refused the file itself.
            db::DbError::Io(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                folder_error(path.parent().unwrap_or(path), &err)
            }
            other => other.into(),
        })?;
    if session.is_open() {
        // A switch, as in `open`: the other database is closed normally
        // (its preview ends, its backup is made) rather than replaced.
        close_normal(session, machine, CloseReason::Switched).ok();
    }
    session.install(prepare(machine, conn, path)?);
    Ok(())
}

/// Opens the database at `path` with `passphrase` and makes it the open
/// one, closing any other open database first as a switch. A replacement of
/// the file that a crash interrupted is finished or undone first
/// (research.md §4). A refused open installs nothing and changes nothing.
/// A database marked open on another computer opens only with `take_over`
/// (FR-032).
pub fn open(
    session: &Session,
    machine: &MachineSettings,
    path: &Path,
    passphrase: &Passphrase,
    take_over: bool,
) -> Result<(), CommandError> {
    if session.is_open() {
        // The frontend closes first itself, to ask about unsaved changes;
        // this covers anything else. It can only fail when nothing is open.
        close_normal(session, machine, CloseReason::Switched).ok();
    }
    file_swap::recover(path);
    let conn = db::open_database(path, passphrase, &machine.identity(), take_over)?;
    session.install(prepare(machine, conn, path)?);
    Ok(())
}

/// Wraps a freshly opened connection for the session, warms what the first
/// suggestion reads, and records the open in the recent list, with what this
/// computer should remember about the database. A backup opened directly
/// becomes a database of its own here (research.md §9).
pub(crate) fn prepare(
    machine: &MachineSettings,
    conn: Connection,
    path: &Path,
) -> Result<OpenDatabase, CommandError> {
    let opened_backup = take_backup_stamp(&conn)?;
    let location: String = conn
        .query_row("SELECT backup_location FROM collection_settings", [], |row| row.get(0))
        .map_err(CommandError::from_db)?;
    // Before the connection goes into the session, so no command waits on
    // it. It is part of the open: the 1s of SC-003 includes it, timed in
    // `performance_test.rs`.
    if let Err(err) = suggestions::warm(&conn) {
        log::warn!("warming the suggestions failed: {err}");
    }
    let mut open = OpenDatabase::new(conn, path)?;
    open.notes.opened_backup = opened_backup;
    machine.touch_recent(
        path,
        &open.name,
        &open.database_id,
        &backups::resolve_folder(path, &location),
    );
    Ok(open)
}

/// A file carrying a backup stamp was opened other than by a restore. It
/// gets a new identity, so its own backups, rotation and saved passphrase
/// are kept apart from the original's, and the stamp is cleared: both are
/// housekeeping, not a change to back up. Returns the note to show once.
fn take_backup_stamp(conn: &Connection) -> Result<Option<OpenedBackupNote>, CommandError> {
    let stamp: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT backup_made_at, backup_of_name FROM app_state WHERE backup_made_at IS NOT NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::from_db)?;
    let Some((made_at, of_name)) = stamp else { return Ok(None) };
    let new_id = db::random_hex(16).map_err(|err| {
        log::error!("could not make a database id: {err}");
        CommandError::new("INTERNAL_ERROR", "An unexpected error occurred.")
    })?;
    conn.execute(
        "UPDATE app_state SET database_id = ?1, backup_made_at = NULL, backup_of_name = NULL",
        [new_id],
    )
    .map_err(CommandError::from_db)?;
    Ok(Some(OpenedBackupNote { backup_of_name: of_name.unwrap_or_default(), made_at }))
}

/// The normal close (FR-010, FR-022, FR-025, FR-027, FR-032): announce it,
/// make a backup if one is due, clear the open marker (in the same
/// transaction as the backup record), close the connection (which clears
/// the key from memory), and delete this session's decrypted document
/// copies. Refused with `DATABASE_CLOSED` when nothing is open. A lock
/// (FR-033) is this close, after its unsaved input is kept (see [`lock`]).
///
/// The file is checked first, as before any write (research.md §6). If
/// another computer has taken it over, this is a take-over close. If it
/// can't be reached, nothing is written and no backup is made: the open
/// marker stays, which is this computer's own and so is cleared at the next
/// open here.
///
/// A sleep or shutdown during the backup turns it into an immediate close
/// from there (FR-037, FR-038): the backup is stopped, its changes stay
/// waiting, and this thread finishes the close as [`close_immediate`] would.
pub fn close_normal(
    session: &Session,
    machine: &MachineSettings,
    reason: CloseReason,
) -> Result<CloseOutcome, CommandError> {
    {
        let mut closing = session.closing();
        if closing.immediate.is_some() {
            return Err(CommandError::database_closed());
        }
        closing.normal = session.open_path();
    }
    let Some(mut open) = session.take() else {
        finish_closing(session);
        return Err(CommandError::database_closed());
    };
    // Unsaved input was kept by a lock, or dealt with by the user.
    open.staged_draft = None;
    // The preview is off the screen and out of memory before the close is
    // announced, however long the backup takes (FR-014, research.md §20).
    drop(open.end_preview());
    let check = if open.storage_lost {
        FingerprintCheck::Unreachable
    } else {
        open.fingerprint.check(&open.path)
    };
    if check == FingerprintCheck::Replaced {
        close_taken_over(session, open);
        finish_closing(session);
        return Ok(CloseOutcome::backup(BackupOutcome::NotAttempted));
    }
    let events = session.events();
    if is_lock(reason) {
        events.notice(ChooserNotice::Closed {
            reason,
            database_path: open.path.to_string_lossy().into_owned(),
            idle_minutes: (reason == CloseReason::Idle).then_some(open.lock_settings.idle_minutes),
        });
    }
    events.emit("session:closing", json!({ "reason": reason }));

    let outcome = if check == FingerprintCheck::Unreachable {
        backup_failed(session, &open, BackupFailureReason::DatabaseUnreachable)
    } else {
        let now = session.clock().now();
        let outcome = backup_at_close(session, machine, &open, now);
        if session.closing().immediate.is_some() {
            // A sleep or shutdown stopped the backup: the rest of the close
            // is immediate, and was already announced.
            finish_immediately(session, machine, open);
            finish_closing(session);
            return Ok(outcome);
        }
        // Housekeeping, in one transaction: the backup record when a backup
        // was made, and this computer no longer having the database open.
        let made = outcome.backup == BackupOutcome::Made;
        let finished = open.conn.unchecked_transaction().and_then(|tx| {
            if made {
                tx.execute(
                    "UPDATE app_state SET changes_waiting = 0, last_backup_at = ?1",
                    [backups::utc_text(&now)],
                )?;
            }
            tx.execute(
                "UPDATE app_state
                 SET open_machine_id = NULL, open_machine_name = NULL, open_since = NULL",
                [],
            )?;
            tx.commit()
        });
        if let Err(err) = finished {
            log::error!("could not finish closing {}: {err}", open.path.display());
        }
        outcome
    };
    let path = open.path.clone();
    drop(open);
    session.clear_opened_documents();
    session.set_last_closed(&path);
    leave(machine, &path, check == FingerprintCheck::Same);

    // A sleep that came after the backup has already announced the close.
    if !finish_closing(session) {
        events.emit(
            "session:closed",
            json!({ "reason": reason, "databasePath": path.to_string_lossy(), "outcome": outcome }),
        );
    }
    Ok(outcome)
}

/// The locks (FR-033).
fn is_lock(reason: CloseReason) -> bool {
    matches!(
        reason,
        CloseReason::LockedByUser
            | CloseReason::Idle
            | CloseReason::ScreenLocked
            | CloseReason::Sleep
    )
}

/// A normal close has finished, one way or another. An immediate close
/// that began meanwhile has nothing left to do but report the operation it
/// stopped. Returns whether one had begun.
fn finish_closing(session: &Session) -> bool {
    let began = report_stopped(session);
    {
        let mut closing = session.closing();
        closing.normal = None;
        closing.immediate = None;
    }
    session.closing_changed();
    began
}

/// Locks the open database (FR-033, FR-035, FR-039): `draft`, the form's
/// unsaved input at this moment, is kept as pending changes inside the
/// database, then the normal close runs with `reason`, making a backup if
/// one is due. No confirmation is asked. If the draft can't be kept, the
/// lock still goes ahead and the chooser says the changes were lost.
pub fn lock(
    session: &Session,
    machine: &MachineSettings,
    reason: CloseReason,
    draft: Option<Draft>,
) -> Result<CloseOutcome, CommandError> {
    if let Some(draft) = draft {
        let saved_at = backups::utc_text(&session.clock().now());
        let kept = session.write_housekeeping(|conn| {
            pending::write_pending(conn, Some(&draft), false, &saved_at)
        });
        match kept {
            Ok(()) => {}
            // Closed already, as a take-over or by another close.
            Err(err) if err.code == "DATABASE_TAKEN_OVER" || err.code == "DATABASE_CLOSED" => {
                return Ok(CloseOutcome::backup(BackupOutcome::NotAttempted));
            }
            Err(err) => {
                log::error!("could not keep the unsaved changes at a lock: {}", err.message);
                if let Some(path) = session.open_path() {
                    session.events().notice(ChooserNotice::PendingChangesLost {
                        database_path: path.to_string_lossy().into_owned(),
                    });
                }
            }
        }
    }
    close_normal(session, machine, reason)
}

/// Locks with the draft the frontend last staged: the idle lock and the
/// lock at screen lock, which the backend starts on its own (research.md
/// §16).
pub fn lock_with_staged(
    session: &Session,
    machine: &MachineSettings,
    reason: CloseReason,
) -> Result<CloseOutcome, CommandError> {
    let draft = session.inspect_mut(|open| Ok(open.staged_draft.take()))?;
    lock(session, machine, reason, draft)
}

/// The immediate close, at sleep or shutdown (FR-037, FR-039; data-model.md
/// "Closing(immediate)"), strictly in this order:
/// 1. stop any running operation, and tell the frontend, which drops the
///    collection from the screen;
/// 2. keep the staged draft as pending changes, clearing the open marker in
///    the same write (the marker alone with no draft);
/// 3. close the connection, clearing the key and collection data from
///    memory;
/// 4. delete decrypted document copies;
/// 5. remove the stopped operation's partial files. Each operation removes
///    its own as it unwinds, and this waits for it to have done so, to say
///    in the chooser how far it got.
///
/// It never makes a backup. A close already under way, whatever its reason
/// and the idle-lock setting, is taken over: its backup is stopped and it
/// finishes from step 2. Returns once the close has finished, or after a
/// few seconds, whatever is left then being finished on waking.
pub fn close_immediate(session: &Session, machine: &MachineSettings, reason: CloseReason) {
    if begin_immediate(session, reason) {
        complete_immediate(session, machine);
    }
    session.wait_closed(IMMEDIATE_WAIT);
}

/// Step 1 of [`close_immediate`]. The session is taken only after stopping
/// the operation, so a running copy lets go of it. From here on no command
/// reaches the database. Returns whether there was anything to close.
pub fn begin_immediate(session: &Session, reason: CloseReason) -> bool {
    let stopped = session.operations().stop();
    let (path, lock_announced) = {
        let mut closing = session.closing();
        if closing.immediate.is_some() {
            return true;
        }
        // A close under way has said so already if it is a lock.
        let lock_announced = closing.normal.is_some();
        let Some(path) = closing.normal.clone().or_else(|| session.open_path()) else {
            return false;
        };
        closing.immediate = Some(ImmediateClose {
            path: path.clone(),
            stopped: stopped.clone(),
            completing: false,
        });
        (path, lock_announced)
    };
    session.set_last_closed(&path);
    // Before `session:closed`, which is when the frontend drops the
    // collection: the preview goes first, without waiting for the rest of
    // the close (FR-014).
    session.end_preview();
    let events = session.events();
    if is_lock(reason) && !lock_announced {
        events.notice(ChooserNotice::Closed {
            reason,
            database_path: path.to_string_lossy().into_owned(),
            idle_minutes: None,
        });
    }
    let mut closed = json!({ "reason": reason, "databasePath": path.to_string_lossy() });
    if let Some(stopped) = &stopped {
        closed["stoppedOperation"] = json!(stopped.kind());
    }
    events.emit("session:closed", closed);
    true
}

/// Steps 2 to 5 of an immediate close that has begun, unless a normal close
/// under way will finish them, or another thread is already doing so.
fn complete_immediate(session: &Session, machine: &MachineSettings) {
    {
        let mut closing = session.closing();
        let normal_under_way = closing.normal.is_some();
        let Some(immediate) = closing.immediate.as_mut() else { return };
        if immediate.completing || normal_under_way {
            return;
        }
        immediate.completing = true;
    }
    let open = session.hold_for_close().take();
    session.forget_open();
    if let Some(open) = open {
        finish_immediately(session, machine, open);
    }
    report_stopped(session);
    session.closing().immediate = None;
    session.closing_changed();
}

/// Steps 2 to 4 of an immediate close, for the database it took. Nothing is
/// written to a file that can't be reached or that another computer has
/// taken over.
fn finish_immediately(session: &Session, machine: &MachineSettings, mut open: OpenDatabase) {
    let draft = open.staged_draft.take();
    let writable =
        !open.storage_lost && open.fingerprint.check(&open.path) == FingerprintCheck::Same;
    let kept = if writable {
        let saved_at = backups::utc_text(&session.clock().now());
        pending::write_pending(&open.conn, draft.as_ref(), true, &saved_at).map_err(|err| {
            log::error!(
                "could not keep the unsaved changes of {}: {}",
                open.path.display(),
                err.message
            )
        })
    } else {
        Err(())
    };
    if kept.is_err() && draft.is_some() {
        session.events().notice(ChooserNotice::PendingChangesLost {
            database_path: open.path.to_string_lossy().into_owned(),
        });
    }
    let path = open.path.clone();
    drop(open);
    session.clear_opened_documents();
    session.set_last_closed(&path);
    // Last, after everything that clears the collection away (FR-037).
    leave(machine, &path, writable);
}

/// Records how this computer left the file at `path` (FR-040): its
/// modification time once the connection has closed, or unknown when the
/// file was out of reach or another computer had replaced it, so that a
/// change made there is never taken for this computer's own.
fn leave(machine: &MachineSettings, path: &Path, reachable_and_ours: bool) {
    let modified = if reachable_and_ours { machine_settings::modified_at(path) } else { None };
    machine.set_left_modified(path, modified);
}

/// Step 5 of an immediate close, if one has begun: waits for the operation
/// it stopped to unwind, removing its partial files, then tells the chooser
/// which it was and how far it got (FR-037). The close counts as under way
/// until then. Returns whether an immediate close had begun.
fn report_stopped(session: &Session) -> bool {
    let (path, stopped) = match &session.closing().immediate {
        Some(immediate) => (immediate.path.clone(), immediate.stopped.clone()),
        None => return false,
    };
    let Some(stopped) = stopped else { return true };
    if !session.operations().wait_ended(&stopped, IMMEDIATE_WAIT) {
        log::warn!("the stopped {} is still unwinding", stopped.kind().as_str());
    }
    session.events().notice(stopped_notice(&path, &stopped));
    true
}

fn stopped_notice(path: &Path, stopped: &StoppedOperation) -> ChooserNotice {
    let kind = stopped.kind();
    ChooserNotice::OperationStopped {
        database_path: path.to_string_lossy().into_owned(),
        operation: kind,
        imported_count: (kind == OperationKind::Import).then(|| stopped.done()),
        deleted_count: (kind == OperationKind::DeleteBackups).then(|| stopped.done()),
        left_behind_count: (kind == OperationKind::MoveBackups).then(|| stopped.done()),
        folder: (kind == OperationKind::MoveBackups).then(|| stopped.folder()).flatten(),
    }
}

/// Finishes, on waking, whatever an immediate close did not get to before
/// the computer slept: steps 1 to 3 before any command is accepted, then
/// the rest (FR-037). A sleep the OS gave no usable notice of is only
/// noticed now, and locks now if the idle lock is on (research.md §14).
pub fn finish_on_wake(session: &Session, machine: &MachineSettings) {
    complete_immediate(session, machine);
    session.wait_closed(IMMEDIATE_WAIT);
    let idle_lock_on = session.idle().settings().is_some_and(|settings| settings.idle_enabled);
    if idle_lock_on && session.is_open() {
        close_immediate(session, machine, CloseReason::Sleep);
    }
}

/// The computer is going to sleep (FR-007, FR-037): passphrase fields
/// clear whatever else happens; a close under way becomes immediate; and
/// with the idle lock on, an open database locks at once.
pub fn will_sleep(session: &Session, machine: &MachineSettings) {
    session.events().emit("system:clear-passphrase-fields", json!({}));
    let closing = {
        let closing = session.closing();
        closing.normal.is_some() || closing.immediate.is_some()
    };
    let idle_lock_on = session.idle().settings().is_some_and(|settings| settings.idle_enabled);
    if closing || idle_lock_on {
        close_immediate(session, machine, CloseReason::Sleep);
    }
}

/// The screen locked (FR-007, FR-038): passphrase fields clear, and with
/// the option on, the open database locks as a normal close.
pub fn screen_locked(session: &Session, machine: &MachineSettings) {
    session.events().emit("system:clear-passphrase-fields", json!({}));
    let lock_on_screen_lock =
        session.idle().settings().is_some_and(|settings| settings.on_screen_lock);
    if lock_on_screen_lock {
        // Refused only when nothing is open any more.
        lock_with_staged(session, machine, CloseReason::ScreenLocked).ok();
    }
}

/// The OS is shutting down or ending the application (FR-039): pending
/// changes are kept before the key is cleared and document copies are
/// deleted, and no backup is made.
pub fn will_shut_down(session: &Session, machine: &MachineSettings) {
    close_immediate(session, machine, CloseReason::Shutdown);
}

/// One tick of the idle lock (FR-034, research.md §15): locks once the idle
/// duration has passed since the last input. Returns whether it locked.
pub fn idle_tick(session: &Session, machine: &MachineSettings) -> bool {
    let running = session.operations().is_running();
    if !session.idle().tick(session.clock().now(), running) {
        return false;
    }
    lock_with_staged(session, machine, CloseReason::Idle).is_ok()
}

/// The backup step of a normal close (FR-025, FR-027): made only when due,
/// reporting `backup:progress`, and skippable through the operations
/// registry. A failure is reported in the chooser and leaves the changes
/// waiting for the next close.
fn backup_at_close(
    session: &Session,
    machine: &MachineSettings,
    open: &OpenDatabase,
    now: chrono::DateTime<chrono::FixedOffset>,
) -> CloseOutcome {
    let state = open.conn.query_row(
        "SELECT a.changes_waiting, a.last_backup_at, s.backups_enabled, s.backup_keep_count,
                s.backup_location
         FROM app_state a, collection_settings s",
        [],
        |row| {
            Ok((
                row.get::<_, bool>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        },
    );
    let (changes_waiting, last_backup_at, enabled, keep, location) = match state {
        Ok(state) => state,
        Err(err) => {
            log::error!("could not read the backup record of {}: {err}", open.path.display());
            return backup_failed(session, open, BackupFailureReason::Io);
        }
    };
    // A sleep or shutdown came first: it starts no backup (FR-037).
    if session.closing_immediately() {
        return CloseOutcome::backup(BackupOutcome::Skipped);
    }
    let folder = backups::resolve_folder(&open.path, &location);
    let kept_from = |today| backups::has_backup_made_on(&folder, &open.database_id, today);
    match backups::is_due(enabled, changes_waiting, last_backup_at.as_deref(), &now, kept_from) {
        Due::Off => return CloseOutcome::backup(BackupOutcome::Off),
        Due::NothingChanged => return CloseOutcome::backup(BackupOutcome::NotDue),
        Due::AlreadyToday => return CloseOutcome::backup(BackupOutcome::AlreadyToday),
        Due::Yes => {}
    }
    let Ok(operation) = session.operations().begin(OperationKind::Backup, None) else {
        log::error!("another operation was running at the close of {}", open.path.display());
        return backup_failed(session, open, BackupFailureReason::Io);
    };
    let events = session.events();
    let mut progress = |processed: u64, total: u64| {
        events.emit(
            "backup:progress",
            json!({
                "processed": processed,
                "total": total,
                "showNow": backups::shows_progress_at_once(total),
            }),
        );
    };
    let made = backups::make_backup(
        machine,
        BackupJob {
            conn: &open.conn,
            database_path: &open.path,
            name: &open.name,
            database_id: &open.database_id,
            folder: &folder,
            make_folder: location == "default",
            now,
            cancel: &|| operation.is_cancelled(),
            progress: &mut progress,
        },
    );
    match made {
        Ok(_) => {
            let keep = usize::try_from(keep).unwrap_or(1);
            backups::rotate(&folder, &open.database_id, keep, None);
            CloseOutcome::backup(BackupOutcome::Made)
        }
        Err(BackupFailure::Stopped) => CloseOutcome::backup(BackupOutcome::Skipped),
        Err(failure) => {
            let reason = failure.reason().unwrap_or(BackupFailureReason::Io);
            backup_failed(session, open, reason)
        }
    }
}

/// A backup that could not be made: the chooser says so, with the reason
/// (FR-027), and the changes stay waiting.
fn backup_failed(
    session: &Session,
    open: &OpenDatabase,
    reason: BackupFailureReason,
) -> CloseOutcome {
    session.events().notice(ChooserNotice::BackupFailed {
        database_path: open.path.to_string_lossy().into_owned(),
        reason,
    });
    CloseOutcome { backup: BackupOutcome::Failed, failure_reason: Some(reason) }
}

/// Another computer has taken the database over (FR-032): tell the user,
/// then let go of it without writing anything more, so no backup and no
/// marker clear. The frontend is told first, so it drops the collection
/// view before anything else happens.
pub(crate) fn close_taken_over(session: &SessionInner, mut open: OpenDatabase) {
    drop(open.end_preview());
    log::warn!("{} was taken over by another computer", open.path.display());
    let path = open.path.to_string_lossy().into_owned();
    let events = session.events();
    events.notice(ChooserNotice::TakenOver { database_path: path.clone() });
    events
        .emit("session:closed", json!({ "reason": CloseReason::TakenOver, "databasePath": path }));
    session.set_last_closed(&open.path);
    session.forget_open();
    drop(open);
    session.clear_opened_documents();
}
