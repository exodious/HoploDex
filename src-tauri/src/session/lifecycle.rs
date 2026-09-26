//! Opening and closing, as ordered steps (data-model.md "Session states and
//! transitions"). The commands in `commands::databases` are thin wrappers
//! over these, and the tests call them directly.

use std::io;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension};
use serde_json::json;

use crate::commands::databases::ops::folder_error;
use crate::commands::CommandError;
use crate::db;
use crate::models::database::{
    BackupFailureReason, BackupOutcome, ChooserNotice, CloseOutcome, CloseReason, OpenedBackupNote,
    OperationKind,
};
use crate::services::backups::{self, BackupFailure, BackupJob, Due};
use crate::services::file_swap;
use crate::services::machine_settings::MachineSettings;
use crate::services::passphrase::Passphrase;
use crate::session::fingerprint::FingerprintCheck;
use crate::session::{OpenDatabase, Session};

/// Creates a database at `path` and makes it the open one.
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

/// Wraps a freshly opened connection for the session and records the open
/// in the recent list, with what this computer should remember about the
/// database. A backup opened directly becomes a database of its own here
/// (research.md §9).
pub(crate) fn prepare(
    machine: &MachineSettings,
    conn: Connection,
    path: &Path,
) -> Result<OpenDatabase, CommandError> {
    let opened_backup = take_backup_stamp(&conn)?;
    let location: String = conn
        .query_row("SELECT backup_location FROM collection_settings", [], |row| row.get(0))
        .map_err(CommandError::from_db)?;
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
/// copies. Refused with `DATABASE_CLOSED` when nothing is open.
///
/// The file is checked first, as before any write (research.md §6). If
/// another computer has taken it over, this is a take-over close. If it
/// can't be reached, nothing is written and no backup is made: the open
/// marker stays, which is this computer's own and so is cleared at the next
/// open here.
pub fn close_normal(
    session: &Session,
    machine: &MachineSettings,
    reason: CloseReason,
) -> Result<CloseOutcome, CommandError> {
    let open = session.take().ok_or_else(CommandError::database_closed)?;
    let check = if open.storage_lost {
        FingerprintCheck::Unreachable
    } else {
        open.fingerprint.check(&open.path)
    };
    if check == FingerprintCheck::Replaced {
        close_taken_over(session, open);
        return Ok(CloseOutcome::backup(BackupOutcome::NotAttempted));
    }
    let events = session.events();
    events.emit("session:closing", json!({ "reason": reason }));

    let outcome = if check == FingerprintCheck::Unreachable {
        backup_failed(session, &open, BackupFailureReason::DatabaseUnreachable)
    } else {
        let now = session.clock().now();
        let outcome = backup_at_close(session, machine, &open, now);
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

    events.emit(
        "session:closed",
        json!({ "reason": reason, "databasePath": path.to_string_lossy(), "outcome": outcome }),
    );
    Ok(outcome)
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
    match backups::is_due(enabled, changes_waiting, last_backup_at.as_deref(), &now) {
        Due::Off => return CloseOutcome::backup(BackupOutcome::Off),
        Due::NothingChanged => return CloseOutcome::backup(BackupOutcome::NotDue),
        Due::AlreadyToday => return CloseOutcome::backup(BackupOutcome::AlreadyToday),
        Due::Yes => {}
    }
    let Ok(operation) = session.operations().begin(OperationKind::Backup, None) else {
        log::error!("another operation was running at the close of {}", open.path.display());
        return backup_failed(session, open, BackupFailureReason::Io);
    };
    let folder = backups::resolve_folder(&open.path, &location);
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
pub(crate) fn close_taken_over(session: &Session, open: OpenDatabase) {
    log::warn!("{} was taken over by another computer", open.path.display());
    let path = open.path.to_string_lossy().into_owned();
    let events = session.events();
    events.notice(ChooserNotice::TakenOver { database_path: path.clone() });
    events
        .emit("session:closed", json!({ "reason": CloseReason::TakenOver, "databasePath": path }));
    session.set_last_closed(&open.path);
    drop(open);
    session.clear_opened_documents();
}
