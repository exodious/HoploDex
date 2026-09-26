//! Choosing, creating and opening databases, and the open database's own
//! status (specs/003-database-protection-management
//! contracts/tauri-commands.md). The commands are thin: each passphrase
//! argument becomes a [`Passphrase`] at once, and is dropped, and wiped,
//! when the command returns (FR-007). The logic is in [`ops`], which takes
//! its paths and `machine.json` from the caller so the tests use throwaway
//! ones.

use tauri::{AppHandle, Manager, State};

use crate::commands::CommandError;
use crate::models::database::{
    ChooserState, CloseOutcome, CloseReason, DatabaseStatus, NoteKind, RecentDatabase,
    RecentRemoved,
};
use crate::services::machine_settings::MachineSettings;
use crate::services::passphrase::Passphrase;
use crate::session::Session;

pub mod ops {
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    use rusqlite::Connection;

    use crate::commands::CommandError;
    use crate::models::database::{
        validate_create_database_input, BackupLocation, BackupLocationKind, BackupSettings,
        ChooserState, CloseOutcome, CloseReason, CollectionSettings, DatabaseNotes, DatabaseStatus,
        LockSettings, NoteKind, RecentDatabase, RecentRemoved, SuggestedLocation,
    };
    use crate::services::machine_settings::{MachineSettings, RecentEntry};
    use crate::services::passphrase::Passphrase;
    use crate::session::lifecycle::{self, backup_folder};
    use crate::session::{OpenDatabase, Session};

    /// The name the create dialog suggests (research.md §19).
    pub const SUGGESTED_NAME: &str = "My collection";

    /// The folder the create dialog suggests: `HoploDex` in the documents
    /// folder, or in `~/Documents` when the OS names none. Never the app
    /// data directory, where the pre-feature database lives (research.md
    /// §19).
    pub fn suggested_folder(documents: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
        documents
            .or_else(|| home.map(|home| home.join("Documents")))
            .map(|documents| documents.join("HoploDex"))
            .unwrap_or_default()
    }

    /// A recent-list entry as the chooser shows it: unavailable when its
    /// file isn't at its path (FR-012).
    fn recent_database(entry: RecentEntry) -> RecentDatabase {
        RecentDatabase {
            available: entry.path.is_file(),
            path: entry.path.to_string_lossy().into_owned(),
            name: entry.name,
            last_opened_at: entry.last_opened_at,
            passphrase_saved: entry.passphrase_saved,
        }
    }

    /// The chooser's state (FR-012, FR-020, FR-021). The selected row is the
    /// database just closed or locked (FR-033), or else the most recent. The
    /// keyring and screen-lock flags stay off until those stories land.
    pub fn chooser_state(
        session: &Session,
        machine: &MachineSettings,
        documents: Option<PathBuf>,
        home: Option<PathBuf>,
    ) -> ChooserState {
        let recent: Vec<RecentDatabase> =
            machine.recent().into_iter().map(recent_database).collect();
        let last_closed = session.last_closed().map(|path| path.to_string_lossy().into_owned());
        let selected_path = last_closed
            .filter(|closed| recent.iter().any(|entry| &entry.path == closed))
            .or_else(|| recent.first().map(|entry| entry.path.clone()));
        ChooserState {
            selected_path,
            recent,
            keyring_available: false,
            screen_lock_supported: false,
            suggested: SuggestedLocation {
                folder: suggested_folder(documents, home).to_string_lossy().into_owned(),
                name: SUGGESTED_NAME.to_owned(),
            },
            notices: machine.take_notices(),
        }
    }

    /// Creates `<folder>/<name>.hoplodex` with `passphrase` and opens it
    /// (FR-003, FR-004, FR-009). A missing folder is made first.
    pub fn create_database(
        session: &Session,
        machine: &MachineSettings,
        folder: &str,
        name: &str,
        passphrase: &Passphrase,
        acknowledged_unrecoverable: bool,
    ) -> Result<DatabaseStatus, CommandError> {
        let path =
            validate_create_database_input(folder, name, passphrase, acknowledged_unrecoverable)?;
        let folder = Path::new(folder);
        fs::create_dir_all(folder).map_err(|err| folder_error(folder, &err))?;
        lifecycle::create(session, machine, &path, passphrase)?;
        database_status(session, machine)
    }

    /// The field error for a folder the new database can't be put in.
    pub(crate) fn folder_error(folder: &Path, err: &io::Error) -> CommandError {
        log::warn!("could not use {} for a new database: {err}", folder.display());
        let message = if err.kind() == io::ErrorKind::PermissionDenied {
            "HoploDex can't write to this folder."
        } else {
            "HoploDex couldn't create this folder."
        };
        CommandError::validation(
            "Check the new database's details.",
            [("folder".to_owned(), message.to_owned())].into(),
        )
    }

    /// Opens the database at `path` with a typed passphrase (FR-005, FR-006),
    /// closing any other open database first as a switch. A refused open
    /// changes neither the file nor the session. `take_over` opens a
    /// database marked open on another computer (FR-032); the UI sends it
    /// only after its confirmation.
    pub fn open_database(
        session: &Session,
        machine: &MachineSettings,
        path: &str,
        passphrase: &Passphrase,
        take_over: bool,
    ) -> Result<DatabaseStatus, CommandError> {
        lifecycle::open(session, machine, Path::new(path), passphrase, take_over)?;
        database_status(session, machine)
    }

    /// Closes the open database or switches away from it (FR-010). The
    /// frontend has already dealt with unsaved changes.
    pub fn close_database(
        session: &Session,
        reason: CloseReason,
    ) -> Result<CloseOutcome, CommandError> {
        if !matches!(reason, CloseReason::Closed | CloseReason::Switched) {
            return Err(CommandError::validation(
                "A database can only be closed or switched from here.",
                [("reason".to_owned(), "Use closed or switched.".to_owned())].into(),
            ));
        }
        lifecycle::close_normal(session, reason)
    }

    /// Takes `path` off this computer's recent list. The database file is
    /// never touched (FR-012, US2-5).
    pub fn remove_recent_database(machine: &MachineSettings, path: &str) -> RecentRemoved {
        machine.remove_recent(Path::new(path));
        RecentRemoved { removed: true }
    }

    /// Points an unavailable recent entry at where the user found its file
    /// (FR-012), keeping everything else about it.
    pub fn locate_database(
        machine: &MachineSettings,
        path: &str,
        new_path: &str,
    ) -> Result<RecentDatabase, CommandError> {
        machine
            .locate_recent(Path::new(path), Path::new(new_path))
            .map(recent_database)
            .ok_or_else(|| CommandError::not_found("That database isn't in the recent list."))
    }

    /// The open database as the frontend needs it.
    pub fn database_status(
        session: &Session,
        machine: &MachineSettings,
    ) -> Result<DatabaseStatus, CommandError> {
        session.inspect(|open| status(open, machine))
    }

    fn status(
        open: &OpenDatabase,
        machine: &MachineSettings,
    ) -> Result<DatabaseStatus, CommandError> {
        let note_dismissed: bool = open
            .conn
            .query_row("SELECT disk_encryption_note_dismissed FROM app_state", [], |row| row.get(0))
            .map_err(CommandError::from_db)?;
        let passphrase_saved =
            machine.recent().iter().any(|entry| entry.path == open.path && entry.passphrase_saved);
        Ok(DatabaseStatus {
            path: open.path.to_string_lossy().into_owned(),
            name: open.name.clone(),
            passphrase_saved,
            settings: collection_settings(&open.conn, &open.path)?,
            pending_changes: None,
            notes: DatabaseNotes {
                disk_encryption: !note_dismissed,
                opened_backup: None,
                restored_with_passphrase_of: None,
                damaged_file_kept_at: None,
            },
        })
    }

    /// The backup and lock settings kept in the database, with the backup
    /// folder resolved on this computer.
    pub fn collection_settings(
        conn: &Connection,
        database_path: &Path,
    ) -> Result<CollectionSettings, CommandError> {
        conn.query_row(
            "SELECT backups_enabled, backup_keep_count, backup_location, idle_lock_enabled,
                    idle_lock_minutes, lock_on_screen_lock
             FROM collection_settings",
            [],
            |row| {
                let location: String = row.get(2)?;
                let folder = backup_folder(database_path, &location);
                let (kind, available) = if location == "default" {
                    // Made at the first backup, beside the database.
                    (BackupLocationKind::Default, folder.is_dir() || database_path.exists())
                } else {
                    (BackupLocationKind::Custom, folder.is_dir())
                };
                Ok(CollectionSettings {
                    backups: BackupSettings {
                        enabled: row.get(0)?,
                        keep_count: row.get(1)?,
                        location: BackupLocation {
                            kind,
                            path: folder.to_string_lossy().into_owned(),
                            available,
                        },
                    },
                    lock: LockSettings {
                        idle_enabled: row.get(3)?,
                        idle_minutes: row.get(4)?,
                        on_screen_lock: row.get(5)?,
                    },
                })
            },
        )
        .map_err(CommandError::from_db)
    }

    /// Dismisses a note for good (`diskEncryption`, FR-008) or for this
    /// session. Dismissing is housekeeping, not a change to back up.
    pub fn dismiss_note(session: &Session, note: NoteKind) -> Result<(), CommandError> {
        match note {
            NoteKind::DiskEncryption => session.write(|conn| {
                conn.execute("UPDATE app_state SET disk_encryption_note_dismissed = 1", [])
                    .map(|_| ())
                    .map_err(CommandError::from_db)
            }),
            // Session-only notes, shown once from the open's own status.
            NoteKind::OpenedBackup | NoteKind::Restored => Ok(()),
        }
    }
}

#[tauri::command]
pub async fn get_chooser_state(
    app: AppHandle,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<ChooserState, CommandError> {
    // `document_dir` honours `user-dirs.dirs`, so a sandboxed run suggests a
    // sandboxed folder (research.md §19).
    Ok(ops::chooser_state(
        &session,
        &machine,
        app.path().document_dir().ok(),
        app.path().home_dir().ok(),
    ))
}

#[tauri::command]
pub async fn create_database(
    folder: String,
    name: String,
    passphrase: String,
    acknowledged_unrecoverable: bool,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<DatabaseStatus, CommandError> {
    let passphrase = Passphrase::from_input(passphrase);
    ops::create_database(
        &session,
        &machine,
        &folder,
        &name,
        &passphrase,
        acknowledged_unrecoverable,
    )
}

#[tauri::command]
pub async fn open_database(
    path: String,
    passphrase: String,
    take_over: Option<bool>,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<DatabaseStatus, CommandError> {
    let passphrase = Passphrase::from_input(passphrase);
    ops::open_database(&session, &machine, &path, &passphrase, take_over.unwrap_or(false))
}

#[tauri::command]
pub async fn close_database(
    reason: CloseReason,
    session: State<'_, Session>,
) -> Result<CloseOutcome, CommandError> {
    ops::close_database(&session, reason)
}

/// Closes the open database, if any, as a quit, then exits (research.md
/// §17). The frontend has already dealt with unsaved changes.
#[tauri::command]
pub async fn quit_application(
    app: AppHandle,
    session: State<'_, Session>,
) -> Result<(), CommandError> {
    if session.is_open() {
        // It can only fail when nothing is open any more.
        crate::session::lifecycle::close_normal(&session, CloseReason::Quit).ok();
    }
    // An exit with a code is ours, and `main` lets it through.
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn remove_recent_database(
    path: String,
    machine: State<'_, MachineSettings>,
) -> Result<RecentRemoved, CommandError> {
    Ok(ops::remove_recent_database(&machine, &path))
}

#[tauri::command]
pub async fn locate_database(
    path: String,
    new_path: String,
    machine: State<'_, MachineSettings>,
) -> Result<RecentDatabase, CommandError> {
    ops::locate_database(&machine, &path, &new_path)
}

#[tauri::command]
pub async fn get_database_status(
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<DatabaseStatus, CommandError> {
    ops::database_status(&session, &machine)
}

#[tauri::command]
pub async fn dismiss_note(note: NoteKind, session: State<'_, Session>) -> Result<(), CommandError> {
    ops::dismiss_note(&session, note)
}
