//! Opening and closing, as ordered steps (data-model.md "Session states and
//! transitions"). The commands in `commands::databases` are thin wrappers
//! over these, and the tests call them directly.

use std::io;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::json;

use crate::commands::databases::ops::folder_error;
use crate::commands::CommandError;
use crate::db;
use crate::models::database::{
    BackupFailureReason, BackupOutcome, ChooserNotice, CloseOutcome, CloseReason,
};
use crate::services::machine_settings::MachineSettings;
use crate::services::passphrase::Passphrase;
use crate::session::fingerprint::FingerprintCheck;
use crate::session::{OpenDatabase, Session};

/// The folder a database's backups go to on this computer (research.md
/// §7): `default` is a "HoploDex backups" folder next to the database.
pub fn backup_folder(database_path: &Path, location: &str) -> PathBuf {
    if location == "default" {
        database_path.parent().unwrap_or(Path::new("")).join("HoploDex backups")
    } else {
        PathBuf::from(location)
    }
}

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
    install(session, machine, conn, path)
}

/// Opens the database at `path` with `passphrase` and makes it the open
/// one, closing any other open database first as a switch. A refused open
/// installs nothing and changes nothing. A database marked open on another
/// computer opens only with `take_over` (FR-032).
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
        close_normal(session, CloseReason::Switched).ok();
    }
    let conn = db::open_database(path, passphrase, &machine.identity(), take_over)?;
    install(session, machine, conn, path)
}

/// Installs a freshly opened connection and records the open in the recent
/// list, with what this computer should remember about the database.
fn install(
    session: &Session,
    machine: &MachineSettings,
    conn: Connection,
    path: &Path,
) -> Result<(), CommandError> {
    let location: String = conn
        .query_row("SELECT backup_location FROM collection_settings", [], |row| row.get(0))
        .map_err(CommandError::from_db)?;
    let open = OpenDatabase::new(conn, path)?;
    machine.touch_recent(path, &open.name, &open.database_id, &backup_folder(path, &location));
    session.install(open);
    Ok(())
}

/// The normal close (FR-010, FR-022, FR-025, FR-032): announce it, clear the
/// open marker, close the connection (which clears the key from memory),
/// and delete this session's decrypted document copies. Refused with
/// `DATABASE_CLOSED` when nothing is open.
///
/// The file is checked first, as before any write (research.md §6). If
/// another computer has taken it over, this is a take-over close. If it
/// can't be reached, nothing is written: the open marker stays, which is
/// this computer's own and so is cleared at the next open here, and the
/// backup fails.
pub fn close_normal(session: &Session, reason: CloseReason) -> Result<CloseOutcome, CommandError> {
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
        events.notice(ChooserNotice::BackupFailed {
            database_path: open.path.to_string_lossy().into_owned(),
            reason: BackupFailureReason::DatabaseUnreachable,
        });
        CloseOutcome {
            backup: BackupOutcome::Failed,
            failure_reason: Some(BackupFailureReason::DatabaseUnreachable),
        }
    } else {
        // Housekeeping: this computer no longer has the database open.
        if let Err(err) = open.conn.execute(
            "UPDATE app_state
             SET open_machine_id = NULL, open_machine_name = NULL, open_since = NULL",
            [],
        ) {
            log::error!("could not clear the open marker of {}: {err}", open.path.display());
        }
        CloseOutcome::backup(BackupOutcome::NotAttempted)
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
