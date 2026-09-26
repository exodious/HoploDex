//! Listing, restoring and deleting a database's backups
//! (specs/003-database-protection-management contracts/tauri-commands.md
//! "long-running operations"; FR-028, FR-029; research.md §8). The commands
//! are thin; the logic is in [`ops`], which the tests call directly. The
//! backup passphrase becomes a [`Passphrase`] at once and is wiped when the
//! command returns (FR-007).

use tauri::State;

use crate::commands::CommandError;
use crate::models::database::{BackupList, BackupsDeleted, DatabaseStatus};
use crate::services::machine_settings::MachineSettings;
use crate::services::passphrase::Passphrase;
use crate::session::Session;

pub mod ops {
    use std::fs::{self, File};
    use std::io;
    use std::path::{Path, PathBuf};

    use rusqlite::{Connection, OpenFlags};
    use serde_json::json;

    use crate::commands::databases::ops::status;
    use crate::commands::CommandError;
    use crate::db::{self, cipher, raw_file::RawFile, OpenError};
    use crate::models::database::{
        BackupList, BackupsDeleted, CloseReason, DatabaseStatus, OperationKind,
    };
    use crate::services::backups::{self, BackupFailure, BackupJob, CopyError};
    use crate::services::disk_space;
    use crate::services::file_swap;
    use crate::services::machine_settings::MachineSettings;
    use crate::services::passphrase::Passphrase;
    use crate::services::secure_delete::{self, WipeControl, Wiped};
    use crate::session::fingerprint::FingerprintCheck;
    use crate::session::lifecycle;
    use crate::session::operations::OperationGuard;
    use crate::session::{database_name, Session, SessionEvents};

    /// Where a database's backups are and what is there (FR-028): the open
    /// database's, or, for one that doesn't open, the folder and id this
    /// computer cached when it last opened it (research.md §8).
    pub fn list_backups(
        session: &Session,
        machine: &MachineSettings,
        database_path: Option<&str>,
    ) -> Result<BackupList, CommandError> {
        let (folder, database_id) = match database_path {
            None => session.inspect(|open| {
                Ok((backup_settings(&open.conn, &open.path)?.folder, open.database_id.clone()))
            })?,
            Some(path) => cached_backups_of(machine, Path::new(path))?,
        };
        let backups = backups::list(&folder, &database_id).unwrap_or_else(|err| {
            log::warn!("could not list the backups in {}: {err}", folder.display());
            Vec::new()
        });
        Ok(BackupList {
            available: folder.is_dir(),
            folder: folder.to_string_lossy().into_owned(),
            backups,
        })
    }

    /// A database's backup settings as a backup needs them here.
    struct BackupSettings {
        folder: PathBuf,
        /// The default folder, made when missing.
        is_default: bool,
        keep: usize,
    }

    fn backup_settings(conn: &Connection, path: &Path) -> Result<BackupSettings, CommandError> {
        let (location, keep): (String, i64) = conn
            .query_row(
                "SELECT backup_location, backup_keep_count FROM collection_settings",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(CommandError::from_db)?;
        Ok(BackupSettings {
            folder: backups::resolve_folder(path, &location),
            is_default: location == "default",
            keep: usize::try_from(keep).unwrap_or(1),
        })
    }

    fn cached_backups_of(
        machine: &MachineSettings,
        path: &Path,
    ) -> Result<(PathBuf, String), CommandError> {
        machine
            .recent()
            .into_iter()
            .find(|entry| entry.path == path)
            .and_then(|entry| Some((entry.backup_folder?, entry.database_id?)))
            .ok_or_else(|| {
                CommandError::not_found("HoploDex doesn't know where this database's backups are.")
            })
    }

    /// Reports a restore's progress (contracts/tauri-commands.md).
    fn progress(events: &dyn SessionEvents, phase: &str, processed: u64, total: u64) {
        events.emit(
            "restore:progress",
            json!({ "phase": phase, "processed": processed, "total": total }),
        );
    }

    fn stopped() -> CommandError {
        CommandError::operation_stopped(OperationKind::Restore, None, None)
    }

    /// Replaces a database with one of its backups (FR-028, research.md §8),
    /// opened with that backup's passphrase afterwards. With `database_path`
    /// it restores a database that doesn't open, with nothing open; without
    /// it, the open database.
    pub fn restore_backup(
        session: &Session,
        machine: &MachineSettings,
        backup_path: &str,
        backup_passphrase: &Passphrase,
        database_path: Option<&str>,
    ) -> Result<DatabaseStatus, CommandError> {
        let backup = Path::new(backup_path);
        match database_path {
            None => restore_open(session, machine, backup, backup_passphrase),
            Some(path) => {
                restore_damaged(session, machine, backup, backup_passphrase, Path::new(path))
            }
        }
    }

    /// The backup's length, which must be a file that is there.
    fn backup_len(backup: &Path) -> Result<u64, CommandError> {
        match fs::metadata(backup) {
            Ok(meta) if meta.is_file() => Ok(meta.len()),
            _ => Err(CommandError::not_found("That backup is no longer there.")),
        }
    }

    /// Checks the backup's passphrase against its first page before
    /// anything is copied, so a wrong one is refused at once.
    fn check_backup_passphrase(backup: &Path, passphrase: &Passphrase) -> Result<(), CommandError> {
        let checked = Connection::open_with_flags(backup, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .and_then(|conn| {
                conn.pragma_update(None, "key", passphrase.as_str())?;
                cipher::apply_cipher_settings(&conn, "main")?;
                conn.query_row("SELECT count(*) FROM sqlite_schema", [], |row| row.get::<_, i64>(0))
            });
        match checked {
            Ok(_) => Ok(()),
            Err(err) => Err(match OpenError::classify(err) {
                OpenError::Damaged => CommandError::database_damaged(false),
                other => other.into(),
            }),
        }
    }

    /// Copies the backup to `.<file>.new` beside the database, opens it with
    /// its passphrase, proves it sound and stamps it as the restored
    /// database: nothing waiting to be backed up (its content is a backup),
    /// no backup stamp, no open marker, and its identity kept. Returns when
    /// the backup was made. On any failure the copy is removed.
    fn prepare_restored_copy(
        session: &Session,
        operation: &OperationGuard,
        backup: &Path,
        new: &Path,
        passphrase: &Passphrase,
    ) -> Result<Option<String>, CommandError> {
        let prepared = copy_check_and_stamp(session, operation, backup, new, passphrase);
        if prepared.is_err() {
            remove_copy(new);
        }
        prepared
    }

    fn copy_check_and_stamp(
        session: &Session,
        operation: &OperationGuard,
        backup: &Path,
        new: &Path,
        passphrase: &Passphrase,
    ) -> Result<Option<String>, CommandError> {
        let events = session.events();
        let source = File::open(backup).map_err(|err| io_failure(backup, &err))?;
        let copied = backups::copy_chunked(
            &source,
            new,
            &|| operation.is_cancelled(),
            &mut |done, total| progress(events, "copying", done, total),
        );
        match copied {
            Ok(()) => {}
            Err(CopyError::Stopped) => return Err(stopped()),
            Err(CopyError::Io(err)) => return Err(io_failure(new, &err)),
        }

        progress(events, "checking", 0, 0);
        let conn = db::check_copy(new, passphrase).map_err(|err| match err {
            OpenError::Damaged => CommandError::database_damaged(false),
            other => other.into(),
        })?;
        let made_at: Option<String> = conn
            .query_row("SELECT backup_made_at FROM app_state", [], |row| row.get(0))
            .map_err(CommandError::from_db)?;
        conn.execute_batch(
            "BEGIN;
             UPDATE app_state
             SET changes_waiting = 0, backup_made_at = NULL, backup_of_name = NULL,
                 open_machine_id = NULL, open_machine_name = NULL, open_since = NULL;
             DELETE FROM pending_changes;
             COMMIT;",
        )
        .map_err(CommandError::from_db)?;
        drop(conn);
        if operation.is_cancelled() {
            return Err(stopped());
        }
        Ok(made_at)
    }

    fn io_failure(path: &Path, err: &io::Error) -> CommandError {
        log::error!("restore: {}: {err}", path.display());
        CommandError::new("INTERNAL_ERROR", "The backup couldn't be copied.")
    }

    /// Removes a restore's copy, and the journal checking it may leave.
    fn remove_copy(new: &Path) {
        if let Err(err) = secure_delete::secure_delete_file(new) {
            if err.kind() != io::ErrorKind::NotFound {
                log::warn!("could not remove {}: {err}", new.display());
            }
        }
        let mut journal = new.as_os_str().to_owned();
        journal.push("-journal");
        let _ = fs::remove_file(journal);
    }

    fn restore_open(
        session: &Session,
        machine: &MachineSettings,
        backup: &Path,
        passphrase: &Passphrase,
    ) -> Result<DatabaseStatus, CommandError> {
        let operation = session.operations().begin(OperationKind::Restore, None)?;
        let events = session.events();
        let mut slot = session.hold();
        let open = slot.as_mut().ok_or_else(CommandError::database_closed)?;
        // The file must still be this session's (research.md §6).
        if open.storage_lost {
            return Err(CommandError::database_unavailable(&open.path));
        }
        match open.fingerprint.check(&open.path) {
            FingerprintCheck::Same => {}
            FingerprintCheck::Replaced => {
                let open = slot.take().expect("checked above");
                drop(slot);
                lifecycle::close_taken_over(session, open);
                return Err(CommandError::database_taken_over());
            }
            FingerprintCheck::Unreachable => {
                open.storage_lost = true;
                return Err(CommandError::database_unavailable(&open.path));
            }
        }
        let settings = backup_settings(&open.conn, &open.path)?;
        let path = open.path.clone();
        let database_folder = path.parent().unwrap_or(Path::new("")).to_owned();

        // 1. Before anything is written: the backup location, and room for
        // the restored copy and the "before restoring" backup.
        backups::check_location(&settings.folder, settings.is_default).map_err(|problem| {
            CommandError::backup_location_unavailable(&settings.folder, problem.as_reason())
        })?;
        let restored_len = backup_len(backup)?;
        let current_len = RawFile::of(&open.conn)
            .and_then(|file| file.size())
            .map_err(|err| io_failure(&path, &err))?;
        disk_space::check_room(&[
            (&database_folder, restored_len),
            (&settings.folder, current_len),
        ])?;
        check_backup_passphrase(backup, passphrase)?;

        // 2. The restored copy, known good before the current database is
        // touched.
        let new = file_swap::new_path(&path);
        let made_at = prepare_restored_copy(session, &operation, backup, &new, passphrase)?;

        // 3. The "before restoring" backup, whatever the once-a-day limit
        // and even with automatic backups off: it makes the restore
        // undoable (FR-028). Without it there is no restore.
        progress(events, "savingCurrent", 0, current_len);
        let saved = backups::make_backup(
            machine,
            BackupJob {
                conn: &open.conn,
                database_path: &path,
                name: &open.name,
                database_id: &open.database_id,
                folder: &settings.folder,
                make_folder: settings.is_default,
                now: session.clock().now(),
                cancel: &|| operation.is_cancelled(),
                progress: &mut |done, total| progress(events, "savingCurrent", done, total),
            },
        );
        match saved {
            Ok(_) => {}
            Err(BackupFailure::Stopped) => {
                remove_copy(&new);
                return Err(stopped());
            }
            Err(failure) => {
                log::error!("the backup before restoring {} failed: {failure:?}", path.display());
                remove_copy(&new);
                return Err(CommandError::restore_cancelled());
            }
        }

        // 4. Close, replace and reopen with the backup's passphrase.
        progress(events, "replacing", 0, 0);
        if operation.is_cancelled() {
            remove_copy(&new);
            return Err(stopped());
        }
        let database_id = open.database_id.clone();
        // Its file is about to be replaced, so nothing more is written to it.
        drop(slot.take());
        session.clear_opened_documents();
        if let Err(err) = file_swap::replace(&path) {
            // The original is unchanged, but closed, and the backend never
            // holds its passphrase (FR-007): the user opens it again.
            session.set_last_closed(&path);
            events.emit(
                "session:closed",
                json!({ "reason": CloseReason::Closed, "databasePath": path.to_string_lossy() }),
            );
            return Err(err);
        }
        backups::rotate(&settings.folder, &database_id, settings.keep, Some(backup));
        let conn = db::open_database(&path, passphrase, &machine.identity(), false)?;
        let mut reopened = lifecycle::prepare(machine, conn, &path)?;
        reopened.notes.restored_with_passphrase_of = made_at;
        let restored = status(&reopened, machine);
        *slot = Some(reopened);
        restored
    }

    /// Restores a database that no longer opens (US3-6). No "before
    /// restoring" backup can be made of it, so it is kept, renamed
    /// `<name> damaged <YYYY-MM-DD HHMMSS>.hoplodex` beside the restored
    /// database, and only the room for the restored copy is needed.
    fn restore_damaged(
        session: &Session,
        machine: &MachineSettings,
        backup: &Path,
        passphrase: &Passphrase,
        path: &Path,
    ) -> Result<DatabaseStatus, CommandError> {
        if session.is_open() {
            lifecycle::close_normal(session, machine, CloseReason::Switched).ok();
        }
        let operation = session.operations().begin(OperationKind::Restore, None)?;
        let events = session.events();
        let database_folder = path.parent().unwrap_or(Path::new("")).to_owned();
        disk_space::check_room_for_copy(backup_len(backup)?, &database_folder)?;
        check_backup_passphrase(backup, passphrase)?;
        let new = file_swap::new_path(path);
        let made_at = prepare_restored_copy(session, &operation, backup, &new, passphrase)?;

        progress(events, "replacing", 0, 0);
        if operation.is_cancelled() {
            remove_copy(&new);
            return Err(stopped());
        }
        let now = session.clock().now();
        let kept = database_folder.join(format!(
            "{} damaged {}.{}",
            database_name(path),
            now.format("%Y-%m-%d %H%M%S"),
            crate::models::database::DATABASE_EXTENSION,
        ));
        let set_aside = path.exists();
        if set_aside {
            if let Err(err) = fs::rename(path, &kept) {
                log::error!("could not set {} aside: {err}", path.display());
                remove_copy(&new);
                return Err(CommandError::replace_failed(path));
            }
        }
        if let Err(err) = fs::rename(&new, path) {
            log::error!("could not put the restored copy at {}: {err}", path.display());
            if set_aside {
                let _ = fs::rename(&kept, path);
            }
            remove_copy(&new);
            return Err(CommandError::replace_failed(path));
        }
        let conn = db::open_database(path, passphrase, &machine.identity(), false)?;
        let mut restored = lifecycle::prepare(machine, conn, path)?;
        restored.notes.restored_with_passphrase_of = made_at;
        restored.notes.damaged_file_kept_at =
            set_aside.then(|| kept.to_string_lossy().into_owned());
        session.install(restored);
        crate::commands::databases::ops::database_status(session, machine)
    }

    /// Deletes every backup of the open database (FR-029, US3-4): only the
    /// files its listing names, never another database's in a shared
    /// folder, each by secure deletion. One that can't be deleted is left
    /// and reported. A sleep stops it between files, and a file whose
    /// overwrite was under way is removed without finishing (FR-037).
    /// Deleting backups is not a change to the collection.
    pub fn delete_all_backups(
        session: &Session,
        confirmed: bool,
    ) -> Result<BackupsDeleted, CommandError> {
        if !confirmed {
            return Err(CommandError::confirmation_required(
                "Confirm deleting all backups of this database.",
            ));
        }
        let (folder, database_id) = session.inspect(|open| {
            Ok((backup_settings(&open.conn, &open.path)?.folder, open.database_id.clone()))
        })?;
        let operation = session.operations().begin(OperationKind::DeleteBackups, None)?;
        let listed = backups::list(&folder, &database_id).unwrap_or_default();
        let total = listed.len() as u64;
        let mut deleted = 0;
        let mut failed_paths = Vec::new();
        for (index, backup) in listed.iter().enumerate() {
            if operation.is_cancelled() {
                return Err(CommandError::operation_stopped(
                    OperationKind::DeleteBackups,
                    None,
                    Some(deleted),
                ));
            }
            let wiped = secure_delete::secure_delete_whole_file(
                Path::new(&backup.path),
                WipeControl { progress: None, cancel: Some(&|| operation.is_cancelled()) },
            );
            match wiped {
                Ok(Wiped::Deleted | Wiped::Stopped) => deleted += 1,
                Err(err) => {
                    log::warn!("could not delete the backup {}: {err}", backup.path);
                    failed_paths.push(backup.path.clone());
                }
            }
            session.events().emit(
                "backups_delete:progress",
                json!({ "processed": index as u64 + 1, "total": total }),
            );
        }
        if operation.is_cancelled() && deleted < total {
            return Err(CommandError::operation_stopped(
                OperationKind::DeleteBackups,
                None,
                Some(deleted),
            ));
        }
        Ok(BackupsDeleted { deleted_count: deleted, failed_paths })
    }
}

#[tauri::command]
pub async fn list_backups(
    database_path: Option<String>,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<BackupList, CommandError> {
    ops::list_backups(&session, &machine, database_path.as_deref())
}

#[tauri::command]
pub async fn restore_backup(
    backup_path: String,
    backup_passphrase: String,
    database_path: Option<String>,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<DatabaseStatus, CommandError> {
    let backup_passphrase = Passphrase::from_input(backup_passphrase);
    ops::restore_backup(
        &session,
        &machine,
        &backup_path,
        &backup_passphrase,
        database_path.as_deref(),
    )
}

#[tauri::command]
pub async fn delete_all_backups(
    confirmed: bool,
    session: State<'_, Session>,
) -> Result<BackupsDeleted, CommandError> {
    ops::delete_all_backups(&session, confirmed)
}
