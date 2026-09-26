//! Opening and closing, as ordered steps (data-model.md "Session states and
//! transitions"). The commands in `commands::databases` are thin wrappers
//! over these, and the tests call them directly.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::json;

use crate::commands::documents::ops::clear_opened_documents;
use crate::commands::CommandError;
use crate::db;
use crate::models::database::{BackupOutcome, CloseOutcome, CloseReason};
use crate::services::machine_settings::MachineSettings;
use crate::services::passphrase::Passphrase;
use crate::session::{OpenDatabase, Session, SessionEvents};

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
    let conn = db::create_database(path, passphrase, &machine.identity())?;
    install(session, machine, conn, path)
}

/// Opens the database at `path` with `passphrase` and makes it the open
/// one. A refused open installs nothing and changes nothing.
pub fn open(
    session: &Session,
    machine: &MachineSettings,
    path: &Path,
    passphrase: &Passphrase,
) -> Result<(), CommandError> {
    let conn = db::open_database(path, passphrase)?;
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
    let open = OpenDatabase::new(conn, path).map_err(CommandError::from_db)?;
    machine.touch_recent(path, &open.name, &open.database_id, &backup_folder(path, &location));
    session.install(open);
    Ok(())
}

/// The normal close (FR-010, FR-022, FR-025, FR-032): announce it, clear the
/// open marker, close the connection (which clears the key from memory),
/// and delete this session's decrypted document copies. Refused with
/// `DATABASE_CLOSED` when nothing is open.
pub fn close_normal(
    session: &Session,
    events: &dyn SessionEvents,
    opened_documents_dir: &Path,
    reason: CloseReason,
) -> Result<CloseOutcome, CommandError> {
    let open = session.take().ok_or_else(CommandError::database_closed)?;
    events.emit("session:closing", json!({ "reason": reason }));

    let outcome = CloseOutcome::backup(BackupOutcome::NotAttempted);

    // Housekeeping: this computer no longer has the database open.
    if let Err(err) = open.conn.execute(
        "UPDATE app_state SET open_machine_id = NULL, open_machine_name = NULL, open_since = NULL",
        [],
    ) {
        log::error!("could not clear the open marker of {}: {err}", open.path.display());
    }
    let path = open.path.clone();
    drop(open);

    for leftover in clear_opened_documents(opened_documents_dir) {
        log::warn!("could not delete opened-document copy {}", leftover.display());
    }

    events.emit(
        "session:closed",
        json!({ "reason": reason, "databasePath": path.to_string_lossy(), "outcome": outcome }),
    );
    Ok(outcome)
}
