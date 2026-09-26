//! Listing, restoring and deleting a database's backups, and changing its
//! passphrase (specs/003-database-protection-management
//! contracts/tauri-commands.md "long-running operations"; FR-015, FR-016,
//! FR-028, FR-029; research.md §3, §4, §8). The commands are thin; the logic
//! is in [`ops`], which the tests call directly. Each passphrase becomes a
//! [`Passphrase`] at once and is wiped when the command returns (FR-007).

use tauri::{AppHandle, Manager, State};

use crate::commands::databases::PROBE_DIR;
use crate::commands::CommandError;
use crate::models::database::{BackupList, BackupsDeleted, DatabaseStatus, PassphraseChanged};
use crate::services::machine_settings::MachineSettings;
use crate::services::passphrase::Passphrase;
use crate::session::Session;

pub mod ops {
    use std::collections::{BTreeMap, HashMap};
    use std::fs::{self, File};
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::sync::MutexGuard;
    use std::thread;
    use std::time::Duration;

    use rusqlite::{Connection, OpenFlags};
    use serde_json::json;

    use crate::commands::databases::ops::{refresh_saved, status};
    use crate::commands::CommandError;
    use crate::db::{self, cipher, raw_file::RawFile, OpenError};
    use crate::models::database::{
        BackupList, BackupsDeleted, CloseReason, DatabaseStatus, OperationKind, PassphraseChanged,
    };
    use crate::services::backups::{self, BackupFailure, BackupJob, CopyError};
    use crate::services::disk_space;
    use crate::services::file_swap;
    use crate::services::machine_settings::MachineSettings;
    use crate::services::passphrase::{new_passphrase_problem, Passphrase};
    use crate::services::secure_delete::{self, WipeControl, Wiped};
    use crate::session::fingerprint::FingerprintCheck;
    use crate::session::lifecycle;
    use crate::session::operations::OperationGuard;
    use crate::session::{database_name, OpenDatabase, Session, SessionEvents};

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

    /// Holds the session for an operation that replaces the open database's
    /// file, once that file is known to still be this session's (research.md
    /// §6), as before any write. A file another computer has taken over is
    /// closed as a take-over.
    fn hold_still_ours(
        session: &Session,
    ) -> Result<MutexGuard<'_, Option<OpenDatabase>>, CommandError> {
        let mut slot = session.hold();
        let open = slot.as_mut().ok_or_else(CommandError::database_closed)?;
        if open.storage_lost {
            return Err(CommandError::database_unavailable(&open.path));
        }
        match open.fingerprint.check(&open.path) {
            FingerprintCheck::Same => Ok(slot),
            FingerprintCheck::Replaced => {
                let open = slot.take().expect("checked above");
                drop(slot);
                lifecycle::close_taken_over(session, open);
                Err(CommandError::database_taken_over())
            }
            FingerprintCheck::Unreachable => {
                open.storage_lost = true;
                Err(CommandError::database_unavailable(&open.path))
            }
        }
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
        let mut slot = hold_still_ours(session)?;
        let open = slot.as_mut().expect("held open");
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
        // A saved passphrase now opens it only if it is the backup's.
        refresh_saved(machine, &path, &reopened.database_id, passphrase);
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
        refresh_saved(machine, path, &restored.database_id, passphrase);
        session.install(restored);
        crate::commands::databases::ops::database_status(session, machine)
    }

    /// Reports a passphrase change's progress (contracts/tauri-commands.md).
    fn change_progress(events: &dyn SessionEvents, phase: &str, processed: u64, total: u64) {
        events.emit(
            "passphrase_change:progress",
            json!({ "phase": phase, "processed": processed, "total": total }),
        );
    }

    fn change_stopped() -> CommandError {
        CommandError::operation_stopped(OperationKind::PassphraseChange, None, None)
    }

    fn change_io_failure(path: &Path, err: &io::Error) -> CommandError {
        log::error!("passphrase change: {}: {err}", path.display());
        CommandError::new("INTERNAL_ERROR", "The copy with the new passphrase couldn't be made.")
    }

    /// How often the copy's size is reported while `sqlcipher_export` runs,
    /// which has no progress callback of its own (research.md §3).
    const EXPORT_POLL_EVERY: Duration = Duration::from_millis(100);

    /// Changes the open database's passphrase (FR-015, FR-016; research.md
    /// §3, §4): a copy keyed with `new` is made beside it, proved sound and
    /// complete, and only then put in its place in one step, and the
    /// previous file is securely deleted. Until that step the database is
    /// untouched, so a failure or a stop leaves it opening with `current`
    /// and all its content, and the copy is removed. Either passphrase is
    /// held for this operation only (FR-007). `scratch_dir` takes the
    /// page-1 probe that checks `current` (research.md §1a).
    pub fn change_passphrase(
        session: &Session,
        machine: &MachineSettings,
        scratch_dir: &Path,
        current: &Passphrase,
        new: &Passphrase,
    ) -> Result<PassphraseChanged, CommandError> {
        // 1. The new passphrase follows the rules for setting one (FR-003).
        if let Some(problem) = new_passphrase_problem(new) {
            return Err(CommandError::validation(
                "Check the new passphrase.",
                HashMap::from([("newPassphrase".to_owned(), problem.to_owned())]),
            ));
        }
        let mut slot = hold_still_ours(session)?;
        let open = slot.as_mut().expect("held open");
        // The copy would leave pending changes behind (FR-039).
        if open.pending_unresolved {
            return Err(CommandError::pending_changes_unresolved());
        }

        // 2. The current passphrase, against the file's first page.
        match db::verify_passphrase(&open.conn, current, scratch_dir) {
            Ok(true) => {}
            Ok(false) => {
                return Err(CommandError::passphrase_incorrect(None, None)
                    .on_field("currentPassphrase", "That isn't the current passphrase."))
            }
            Err(err) => {
                log::error!("could not check the current passphrase: {err}");
                return Err(CommandError::new(
                    "INTERNAL_ERROR",
                    "The current passphrase couldn't be checked.",
                ));
            }
        }

        // 3. Room for a second copy beside the database (FR-016).
        let path = open.path.clone();
        let len = RawFile::of(&open.conn)
            .and_then(|file| file.size())
            .map_err(|err| change_io_failure(&path, &err))?;
        disk_space::check_room_for_copy(len, path.parent().unwrap_or(Path::new("")))?;

        // 4. From here a sleep stops it, interrupting the export.
        let operation = session
            .operations()
            .begin(OperationKind::PassphraseChange, Some(open.conn.get_interrupt_handle()))?;
        let events = session.events();

        // 5–7. The copy, stamped and proved sound.
        let new_copy = file_swap::new_path(&path);
        let prepared = rekeyed_copy(&open.conn, &operation, events, &new_copy, new)
            .and_then(|()| check_rekeyed_copy(&open.conn, &operation, events, &new_copy, new));
        if let Err(err) = prepared {
            remove_copy(&new_copy);
            return Err(err);
        }

        // 8. Close and replace. What the session knows about the database
        // outlives its connection.
        change_progress(events, "replacing", 0, 0);
        if operation.is_cancelled() {
            remove_copy(&new_copy);
            return Err(change_stopped());
        }
        let notes = std::mem::take(&mut open.notes);
        let staged_draft = open.staged_draft.take();
        drop(slot.take());
        let replaced = file_swap::replace(&path);

        // 9. Reopen with the passphrase the file now has: the new one, or,
        // when the replacement was refused, the current one.
        let reopen_with = if replaced.is_ok() { new } else { current };
        let reopened = db::open_database(&path, reopen_with, &machine.identity(), false)
            .map_err(CommandError::from)
            .and_then(|conn| lifecycle::prepare(machine, conn, &path));
        let database_id = match reopened {
            Ok(mut reopened) => {
                reopened.notes = notes;
                reopened.staged_draft = staged_draft;
                let database_id = reopened.database_id.clone();
                *slot = Some(reopened);
                database_id
            }
            Err(err) => {
                // Closed, and the backend keeps no passphrase (FR-007): the
                // user opens it again.
                log::error!("could not reopen {} after changing its passphrase", path.display());
                session.set_last_closed(&path);
                events.emit(
                    "session:closed",
                    json!({ "reason": CloseReason::Closed, "databasePath": path.to_string_lossy() }),
                );
                return Err(replaced.err().unwrap_or(err));
            }
        };
        let replaced = replaced?;
        Ok(PassphraseChanged {
            old_file_removed: replaced.old_removed,
            old_file_path: (!replaced.old_removed)
                .then(|| replaced.old_path.to_string_lossy().into_owned()),
            // A saved passphrase follows the change (FR-018, US4-7).
            passphrase_saved: refresh_saved(machine, &path, &database_id, new),
        })
    }

    /// Makes `.<file>.new`, keyed with `new`, by `sqlcipher_export` into an
    /// attachment (research.md §3), reporting its size as it grows, and
    /// stamps it: no open marker, no pending changes, and a change waiting
    /// to be backed up, since backups made before now keep the old
    /// passphrase (FR-025, research.md §5).
    fn rekeyed_copy(
        conn: &Connection,
        operation: &OperationGuard,
        events: &dyn SessionEvents,
        new_copy: &Path,
        new: &Passphrase,
    ) -> Result<(), CommandError> {
        let total: i64 = conn
            .query_row(
                "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
                [],
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;
        let total = u64::try_from(total).unwrap_or(0);
        // The session's connection can't create files, only attach one that
        // exists.
        File::create(new_copy).map_err(|err| change_io_failure(new_copy, &err))?;
        let Some(copy_path) = new_copy.to_str() else {
            return Err(change_io_failure(
                new_copy,
                &io::Error::new(io::ErrorKind::InvalidInput, "the path is not UTF-8"),
            ));
        };
        conn.execute("ATTACH DATABASE ?1 AS rekey KEY ?2", [copy_path, new.as_str()])
            .map_err(CommandError::from_db)?;

        change_progress(events, "copying", 0, total);
        let copied = cipher::apply_cipher_settings(conn, "rekey")
            .and_then(|()| export_reporting_size(conn, events, new_copy, total))
            .and_then(|()| {
                conn.execute_batch(
                    "BEGIN;
                     UPDATE rekey.app_state
                     SET open_machine_id = NULL, open_machine_name = NULL, open_since = NULL,
                         changes_waiting = 1;
                     DELETE FROM rekey.pending_changes;
                     COMMIT;",
                )
            });
        if !conn.is_autocommit() {
            let _ = conn.execute_batch("ROLLBACK");
        }
        let detached = conn.execute("DETACH DATABASE rekey", []).map(|_| ());
        match copied.and(detached) {
            Ok(()) if !operation.is_cancelled() => {
                change_progress(events, "copying", total, total);
                Ok(())
            }
            Ok(()) => Err(change_stopped()),
            Err(_) if operation.is_cancelled() => Err(change_stopped()),
            Err(err) => Err(CommandError::from_db(err)),
        }
    }

    /// `SELECT sqlcipher_export('rekey')`, with a thread reporting the size
    /// of the copy every [`EXPORT_POLL_EVERY`] against `total`: the copy
    /// only grows, so the bar is honest, if uneven.
    fn export_reporting_size(
        conn: &Connection,
        events: &dyn SessionEvents,
        new_copy: &Path,
        total: u64,
    ) -> rusqlite::Result<()> {
        let (finished, wait) = mpsc::channel::<()>();
        thread::scope(|scope| {
            scope.spawn(move || {
                while let Err(RecvTimeoutError::Timeout) = wait.recv_timeout(EXPORT_POLL_EVERY) {
                    let written = fs::metadata(new_copy).map_or(0, |meta| meta.len());
                    change_progress(events, "copying", written.min(total), total);
                }
            });
            let exported = conn.query_row("SELECT sqlcipher_export('rekey')", [], |_| Ok(()));
            drop(finished);
            exported
        })
    }

    /// Opens the copy on its own with the new passphrase and proves it sound
    /// (research.md §4) and complete: every table has as many rows as the
    /// database it was copied from, which is idle meanwhile.
    fn check_rekeyed_copy(
        conn: &Connection,
        operation: &OperationGuard,
        events: &dyn SessionEvents,
        new_copy: &Path,
        new: &Passphrase,
    ) -> Result<(), CommandError> {
        change_progress(events, "checking", 0, 0);
        let not_sound = || {
            CommandError::new(
                "INTERNAL_ERROR",
                "The copy with the new passphrase didn't pass its checks, so the passphrase was \
                 not changed.",
            )
        };
        let copy = db::check_copy(new_copy, new).map_err(|err| {
            log::error!("the copy with the new passphrase failed its checks: {err}");
            not_sound()
        })?;
        let expected = table_row_counts(conn).map_err(CommandError::from_db)?;
        let found = table_row_counts(&copy).map_err(CommandError::from_db)?;
        drop(copy);
        if found != expected {
            log::error!("the copy with the new passphrase has other row counts: {found:?}");
            return Err(not_sound());
        }
        if operation.is_cancelled() {
            return Err(change_stopped());
        }
        Ok(())
    }

    /// The row count of every table in the main database, the search
    /// index's shadow tables included. The index itself is left out:
    /// counting an external-content FTS5 table reads its content table.
    fn table_row_counts(conn: &Connection) -> rusqlite::Result<BTreeMap<String, i64>> {
        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM pragma_table_list
                 WHERE schema = 'main' AND type IN ('table', 'shadow')",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        tables
            .into_iter()
            .map(|table| {
                let quoted = table.replace('"', "\"\"");
                let count = conn.query_row(
                    &format!("SELECT count(*) FROM main.\"{quoted}\""),
                    [],
                    |row| row.get(0),
                )?;
                Ok((table, count))
            })
            .collect()
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

#[tauri::command]
pub async fn change_passphrase(
    current_passphrase: String,
    new_passphrase: String,
    app: AppHandle,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<PassphraseChanged, CommandError> {
    let current_passphrase = Passphrase::from_input(current_passphrase);
    let new_passphrase = Passphrase::from_input(new_passphrase);
    let scratch = app.path().app_cache_dir().map_err(|err| {
        log::error!("no cache directory for the passphrase check: {err}");
        CommandError::new("INTERNAL_ERROR", "The current passphrase couldn't be checked.")
    })?;
    ops::change_passphrase(
        &session,
        &machine,
        &scratch.join(PROBE_DIR),
        &current_passphrase,
        &new_passphrase,
    )
}
