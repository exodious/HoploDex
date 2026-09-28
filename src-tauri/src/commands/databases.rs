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
    BackupLocationInput, BackupSettingsInput, BackupSettingsSaved, ChooserState, CloseOutcome,
    CloseReason, CollectionSettings, DatabaseStatus, Draft, ExistingBackupsChoice, IdlePauseReason,
    LockSettingsInput, NoteKind, PassphraseSaved, PendingAction, PendingResolved, RecentDatabase,
    RecentRemoved,
};
use crate::services::machine_settings::MachineSettings;
use crate::services::passphrase::Passphrase;
use crate::session::Session;
use ops::Unlock;

pub mod ops {
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    use rusqlite::Connection;
    use serde_json::json;

    use crate::commands::CommandError;
    use crate::commands::backups::ops::delete_backups_in;
    use crate::db;
    use crate::models::database::{
        BackupLocation, BackupLocationKind, BackupSettings, BackupSettingsInput,
        BackupSettingsSaved, ChooserState, CloseOutcome, CloseReason, CollectionSettings,
        DatabaseNotes, DatabaseStatus, Draft, ExistingBackupsChoice, ExistingBackupsOutcome,
        IdlePauseReason, LeftBehind, LockSettings, LockSettingsInput, NoteKind, OperationKind,
        PassphraseSaved, PendingAction, PendingResolved, RecentDatabase, RecentRemoved,
        SuggestedLocation, validate_backup_settings_input, validate_create_database_input,
        validate_lock_settings_input,
    };
    use crate::services::backups::{self, MoveJob};
    use crate::services::disk_space;
    use crate::services::machine_settings::{MachineSettings, RecentEntry};
    use crate::services::passphrase::Passphrase;
    use crate::session::operations::Operations;
    use crate::session::{OpenDatabase, Session};
    use crate::session::{lifecycle, pending};

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
    /// database just closed or locked (FR-033), or else the most recent.
    /// Whether passphrases can be saved here is probed the first time it is
    /// asked (FR-019); whether the screen lock is reported, when the
    /// system-events listener started (FR-038).
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
            keyring_available: machine.keyring().is_available(),
            screen_lock_supported: crate::platform::screen_lock_supported(),
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

    /// What opens a database.
    pub enum Unlock<'a> {
        /// A passphrase the user typed. `remember` saves it in this
        /// computer's keyring once it has opened the database; the UI sends
        /// it only after the FR-017 confirmation.
        Typed { passphrase: &'a Passphrase, remember: bool },
        /// The passphrase saved in this computer's keyring (FR-017).
        Saved,
    }

    impl<'a> Unlock<'a> {
        /// A typed passphrase, not to be remembered.
        pub fn typed(passphrase: &'a Passphrase) -> Self {
            Self::Typed { passphrase, remember: false }
        }
    }

    /// Opens the database at `path` (FR-005, FR-006), closing any other open
    /// database first as a switch. A refused open changes neither the file
    /// nor the session. `take_over` opens a database marked open on another
    /// computer (FR-032); the UI sends it only after its confirmation. A
    /// database that is damaged, or that the passphrase didn't open (the two
    /// can't be told apart at page 1), says whether it has backups to
    /// restore from (FR-028, US3-6).
    ///
    /// The saved passphrase is found through the recent entry's cached
    /// database id, since the file can't be read without it. When it no
    /// longer opens the database the refusal says so (US5-5), and the next
    /// typed passphrase that does replaces it (FR-018).
    pub fn open_database(
        session: &Session,
        machine: &MachineSettings,
        path: &str,
        unlock: Unlock<'_>,
        take_over: bool,
    ) -> Result<DatabaseStatus, CommandError> {
        let path = Path::new(path);
        let loaded;
        let (passphrase, saved) = match unlock {
            Unlock::Typed { passphrase, .. } => (passphrase, false),
            Unlock::Saved => {
                loaded = machine
                    .recent_entry(path)
                    .and_then(|entry| entry.database_id)
                    .and_then(|id| machine.keyring().load(&id));
                match &loaded {
                    Some(passphrase) => (passphrase, true),
                    None => return Err(CommandError::passphrase_incorrect(Some(true), None)),
                }
            }
        };
        match lifecycle::open(session, machine, path, passphrase, take_over) {
            Ok(()) => {
                if let Unlock::Typed { passphrase, remember } = unlock {
                    let entry = machine.recent_entry(path);
                    let was_saved = entry.as_ref().is_some_and(|entry| entry.passphrase_saved);
                    if let Some(id) = entry.and_then(|entry| entry.database_id)
                        && (remember || was_saved)
                    {
                        save_for(machine, path, &id, passphrase);
                    }
                }
                database_status(session, machine)
            }
            Err(err) if err.code == "DATABASE_DAMAGED" => {
                Err(CommandError::database_damaged(has_backups(machine, path)))
            }
            Err(err) if err.code == "PASSPHRASE_INCORRECT" => {
                Err(CommandError::passphrase_incorrect(
                    saved.then_some(true),
                    has_backups(machine, path).then_some(true),
                ))
            }
            Err(err) => Err(err),
        }
    }

    /// Saves `passphrase` in the keyring for the database `database_id`,
    /// listed at `path`, and records whether that worked. A failure is
    /// logged, not reported: the database is open either way.
    pub(crate) fn save_for(
        machine: &MachineSettings,
        path: &Path,
        database_id: &str,
        passphrase: &Passphrase,
    ) -> bool {
        let saved = machine.keyring().save(database_id, passphrase).is_ok();
        machine.set_passphrase_saved(path, saved);
        saved
    }

    /// Replaces the passphrase saved for the database at `path`, if one is,
    /// with `passphrase`, the one it opens with now: after a passphrase
    /// change (FR-018) or a restore (research.md §8). Returns whether one is
    /// saved afterwards.
    pub(crate) fn refresh_saved(
        machine: &MachineSettings,
        path: &Path,
        database_id: &str,
        passphrase: &Passphrase,
    ) -> bool {
        let was_saved = machine.recent_entry(path).is_some_and(|entry| entry.passphrase_saved);
        was_saved && save_for(machine, path, database_id, passphrase)
    }

    /// Whether this computer knows of backups of the database at `path`,
    /// from its recent entry's cached folder and id, since the file itself
    /// may not open.
    fn has_backups(machine: &MachineSettings, path: &Path) -> bool {
        machine
            .recent()
            .into_iter()
            .find(|entry| entry.path == path)
            .and_then(|entry| Some((entry.backup_folder?, entry.database_id?)))
            .and_then(|(folder, id)| backups::list(&folder, &id).ok())
            .is_some_and(|listed| !listed.is_empty())
    }

    /// Closes the open database or switches away from it (FR-010). The
    /// frontend has already dealt with unsaved changes.
    pub fn close_database(
        session: &Session,
        machine: &MachineSettings,
        reason: CloseReason,
    ) -> Result<CloseOutcome, CommandError> {
        if !matches!(reason, CloseReason::Closed | CloseReason::Switched) {
            return Err(CommandError::validation(
                "A database can only be closed or switched from here.",
                [("reason".to_owned(), "Use closed or switched.".to_owned())].into(),
            ));
        }
        lifecycle::close_normal(session, machine, reason)
    }

    /// Takes `path` off this computer's recent list, and forgets its saved
    /// passphrase (FR-018). The database file is never touched (FR-012,
    /// US2-5).
    pub fn remove_recent_database(machine: &MachineSettings, path: &str) -> RecentRemoved {
        let removed = machine.remove_recent(Path::new(path));
        if let Some(entry) = removed.filter(|entry| entry.passphrase_saved)
            && let Some(id) = entry.database_id
        {
            // Logged by the keyring; the entry is off the list either way.
            if machine.keyring().forget(&id).is_ok() {
                machine.clear_passphrase_saved(&id);
            }
        }
        RecentRemoved { removed: true }
    }

    /// Saves the open database's passphrase in this computer's keyring
    /// (FR-017), once it is proved to be the one the file opens with,
    /// against its first page (research.md §1a). The UI sends it only after
    /// the FR-017 confirmation. `scratch_dir` takes the page-1 probe.
    pub fn save_passphrase(
        session: &Session,
        machine: &MachineSettings,
        scratch_dir: &Path,
        passphrase: &Passphrase,
    ) -> Result<PassphraseSaved, CommandError> {
        if !machine.keyring().is_available() {
            return Err(CommandError::keyring_unavailable());
        }
        let (path, database_id) = session.inspect(|open| {
            match db::verify_passphrase(&open.conn, passphrase, scratch_dir) {
                Ok(true) => Ok((open.path.clone(), open.database_id.clone())),
                Ok(false) => Err(CommandError::passphrase_incorrect(None, None)
                    .on_field("passphrase", "That isn't this database's passphrase.".to_owned())),
                Err(err) => {
                    log::error!("could not check the passphrase to save: {err}");
                    Err(CommandError::new("INTERNAL_ERROR", "The passphrase couldn't be checked."))
                }
            }
        })?;
        machine.keyring().save(&database_id, passphrase)?;
        machine.set_passphrase_saved(&path, true);
        Ok(PassphraseSaved { passphrase_saved: true })
    }

    /// Deletes the passphrase saved for the database at `path`, or for the
    /// open one (FR-018). `KEYRING_UNAVAILABLE`, with nothing changed, when
    /// the keyring can't be reached to delete it.
    pub fn forget_saved_passphrase(
        session: &Session,
        machine: &MachineSettings,
        path: Option<&str>,
    ) -> Result<PassphraseSaved, CommandError> {
        let (path, database_id) = match path {
            Some(path) => {
                let path = PathBuf::from(path);
                let id = machine.recent_entry(&path).and_then(|entry| entry.database_id);
                (path, id)
            }
            None => {
                session.inspect(|open| Ok((open.path.clone(), Some(open.database_id.clone()))))?
            }
        };
        if let Some(id) = database_id {
            machine.keyring().forget(&id)?;
            machine.clear_passphrase_saved(&id);
        }
        machine.set_passphrase_saved(&path, false);
        Ok(PassphraseSaved { passphrase_saved: false })
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

    pub(crate) fn status(
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
            keyring_available: machine.keyring().is_available(),
            screen_lock_supported: crate::platform::screen_lock_supported(),
            settings: collection_settings(&open.conn, &open.path)?,
            pending_changes: pending::summary(&open.conn)?,
            notes: DatabaseNotes { disk_encryption: !note_dismissed, ..open.notes.clone() },
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
                let folder = backups::resolve_folder(database_path, &location);
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
            NoteKind::DiskEncryption => session.write_housekeeping(|conn| {
                conn.execute("UPDATE app_state SET disk_encryption_note_dismissed = 1", [])
                    .map(|_| ())
                    .map_err(CommandError::from_db)
            }),
            // Session-only notes, shown until dismissed in this session.
            NoteKind::OpenedBackup => session.inspect_mut(|open| {
                open.notes.opened_backup = None;
                Ok(())
            }),
            NoteKind::Restored => session.inspect_mut(|open| {
                open.notes.restored_with_passphrase_of = None;
                open.notes.damaged_file_kept_at = None;
                Ok(())
            }),
        }
    }

    /// Saves the backup settings (FR-024, FR-026), a change to the
    /// collection (FR-025). Lowering the number kept deletes nothing now:
    /// the next backup rotates.
    ///
    /// A changed location (its resolved folder, however spelled) with
    /// backups of the database at the old one saves nothing until
    /// `existing_backups` says what to do with them (research.md §22):
    /// `leave` saves; `delete` deletes them as "delete all backups" does and
    /// saves only when every one went; `move` checks the new folder and its
    /// space, saves, then moves them. An old folder that can't be read
    /// allows only `leave`. The recent entry's cached backup folder follows
    /// every save.
    pub fn update_backup_settings(
        session: &Session,
        machine: &MachineSettings,
        input: &BackupSettingsInput,
    ) -> Result<BackupSettingsSaved, CommandError> {
        let location = validate_backup_settings_input(input)?;
        let (database_path, database_id, old_location) = session.inspect(|open| {
            let old: String = open
                .conn
                .query_row("SELECT backup_location FROM collection_settings", [], |row| row.get(0))
                .map_err(CommandError::from_db)?;
            Ok((open.path.clone(), open.database_id.clone(), old))
        })?;
        let old_folder = backups::resolve_folder(&database_path, &old_location);
        let new_folder = backups::resolve_folder(&database_path, &location);
        let change = LocationChange { input, location: &location, database_path: &database_path };
        let saved = |settings, existing_backups| BackupSettingsSaved { settings, existing_backups };
        if backups::same_path(&old_folder, &new_folder) {
            return Ok(saved(change.save(session, machine)?, None));
        }

        // The default folder sits beside the open database, so it is only
        // missing when no backup has been made yet; a custom one that is
        // missing is on a drive that isn't there.
        let listed = if old_location != "default" && !old_folder.is_dir() {
            None
        } else {
            backups::list(&old_folder, &database_id).ok()
        };
        let Some(listed) = listed else {
            return match input.existing_backups {
                Some(ExistingBackupsChoice::Leave) => {
                    Ok(saved(change.save(session, machine)?, Some(ExistingBackupsOutcome::Leave)))
                }
                _ => Err(CommandError::old_backup_location_unavailable(&old_folder)),
            };
        };
        if listed.is_empty() {
            return Ok(saved(change.save(session, machine)?, None));
        }
        match input.existing_backups {
            None => Err(CommandError::backups_at_old_location(
                &old_folder,
                listed.len() as u64,
                listed.iter().map(|backup| backup.size_bytes).sum(),
            )),
            Some(ExistingBackupsChoice::Leave) => {
                Ok(saved(change.save(session, machine)?, Some(ExistingBackupsOutcome::Leave)))
            }
            Some(ExistingBackupsChoice::Delete) => {
                let deleted = delete_backups_in(session, &old_folder, &database_id)?;
                if !deleted.failed_paths.is_empty() {
                    return Err(CommandError::backups_not_all_deleted(
                        deleted.deleted_count,
                        &deleted.failed_paths,
                    ));
                }
                let outcome =
                    ExistingBackupsOutcome::Delete { deleted_count: deleted.deleted_count };
                Ok(saved(change.save(session, machine)?, Some(outcome)))
            }
            Some(ExistingBackupsChoice::Move) => {
                let new_is_default = location == "default";
                move_existing(
                    session,
                    machine,
                    &change,
                    &database_id,
                    &old_folder,
                    &new_folder,
                    new_is_default,
                )
            }
        }
    }

    /// The backup settings a save writes.
    struct LocationChange<'a> {
        input: &'a BackupSettingsInput,
        /// As `backup_location` stores it.
        location: &'a str,
        database_path: &'a Path,
    }

    impl LocationChange<'_> {
        /// Saves the settings through `session.write`, so a take-over is
        /// found first, and points the recent entry at the new folder.
        fn save(
            &self,
            session: &Session,
            machine: &MachineSettings,
        ) -> Result<CollectionSettings, CommandError> {
            session.write(|conn| {
                conn.execute(
                    "UPDATE collection_settings
                     SET backups_enabled = ?1, backup_keep_count = ?2, backup_location = ?3",
                    rusqlite::params![self.input.enabled, self.input.keep_count, self.location],
                )
                .map(|_| ())
                .map_err(CommandError::from_db)
            })?;
            machine.set_backup_folder(
                self.database_path,
                &backups::resolve_folder(self.database_path, self.location),
            );
            session.inspect(|open| collection_settings(&open.conn, &open.path))
        }
    }

    /// Moves the backups at the old location to the new one (FR-026,
    /// research.md §22): the new folder is checked for availability and
    /// room first, with nothing saved if it fails; then the settings are
    /// saved, and kept whatever happens to the move, which runs as the
    /// `moveBackups` operation.
    fn move_existing(
        session: &Session,
        machine: &MachineSettings,
        change: &LocationChange,
        database_id: &str,
        old_folder: &Path,
        new_folder: &Path,
        new_is_default: bool,
    ) -> Result<BackupSettingsSaved, CommandError> {
        backups::check_location(new_folder, new_is_default).map_err(|problem| {
            CommandError::backup_location_unavailable(new_folder, problem.as_reason())
        })?;
        let plan = backups::plan_move(old_folder, new_folder, database_id)
            .map_err(|_| CommandError::old_backup_location_unavailable(old_folder))?;
        disk_space::check_room_for_copy(plan.bytes(), new_folder)?;
        let operation = session.operations().begin(OperationKind::MoveBackups, None)?;
        operation.record_folder(old_folder);
        let settings = change.save(session, machine)?;

        let events = session.events();
        let moved = backups::move_backups(MoveJob {
            machine,
            database_path: change.database_path,
            database_id,
            from: old_folder,
            to: new_folder,
            plan: &plan,
            cancel: &|| operation.is_cancelled(),
            progress: &mut |processed, total| {
                events.emit(
                    "backups_move:progress",
                    json!({
                        "processed": processed,
                        "total": total,
                        "showNow": backups::shows_progress_at_once(total),
                    }),
                );
            },
            not_yet_moved: &|count| operation.record_done(count),
        });
        if moved.stopped {
            return Err(CommandError::move_stopped(moved.left_behind, old_folder));
        }
        let left_behind = moved.reason.map(|reason| LeftBehind {
            count: moved.left_behind,
            folder: old_folder.to_string_lossy().into_owned(),
            reason,
        });
        Ok(BackupSettingsSaved {
            settings,
            existing_backups: Some(ExistingBackupsOutcome::Move {
                moved_count: moved.moved,
                left_behind,
            }),
        })
    }

    /// Stops the backup the current close is making (FR-027). Its partial
    /// file is removed and the changes stay waiting. Anything else running
    /// is left alone.
    pub fn skip_backup(operations: &Operations) {
        if operations.running_kind() == Some(OperationKind::Backup) {
            operations.stop_running();
        }
    }

    /// Saves the lock settings (FR-034, FR-038), a change to the collection
    /// (FR-025). They take effect at once: the idle time starts again.
    pub fn update_lock_settings(
        session: &Session,
        input: &LockSettingsInput,
    ) -> Result<CollectionSettings, CommandError> {
        validate_lock_settings_input(input)?;
        session.write(|conn| {
            conn.execute(
                "UPDATE collection_settings
                 SET idle_lock_enabled = ?1, idle_lock_minutes = ?2, lock_on_screen_lock = ?3",
                rusqlite::params![input.idle_enabled, input.idle_minutes, input.on_screen_lock],
            )
            .map(|_| ())
            .map_err(CommandError::from_db)
        })?;
        let settings = LockSettings {
            idle_enabled: input.idle_enabled,
            idle_minutes: input.idle_minutes,
            on_screen_lock: input.on_screen_lock,
        };
        session.inspect_mut(|open| {
            open.lock_settings = settings.clone();
            Ok(())
        })?;
        session.idle().start(settings, session.clock().now());
        session.inspect(|open| collection_settings(&open.conn, &open.path))
    }

    /// "Lock now" (FR-033, FR-035): `draft`, the form's unsaved input, is
    /// kept as pending changes, then the database closes as a lock. No
    /// confirmation.
    pub fn lock_database(
        session: &Session,
        machine: &MachineSettings,
        draft: Option<Draft>,
    ) -> Result<CloseOutcome, CommandError> {
        lifecycle::lock(session, machine, CloseReason::LockedByUser, draft)
    }

    /// Mirrors the open form's unsaved input into memory, for a lock the
    /// backend starts on its own (research.md §16). `None` clears it.
    pub fn stage_pending_changes(
        session: &Session,
        draft: Option<Draft>,
    ) -> Result<(), CommandError> {
        pending::stage(session, draft)
    }

    /// Resumes or discards the pending changes (FR-039).
    pub fn resolve_pending_changes(
        session: &Session,
        action: PendingAction,
    ) -> Result<PendingResolved, CommandError> {
        Ok(PendingResolved { draft: pending::resolve(session, action)? })
    }

    /// Input to the application's windows restarts the idle time (FR-035).
    pub fn note_activity(session: &Session) {
        session.idle().note_activity(session.clock().now());
    }

    /// A native file or folder dialog opened or closed: the idle clock
    /// pauses while it is open (research.md §15).
    pub fn set_idle_paused(session: &Session, reason: IdlePauseReason, paused: bool) {
        session.idle().set_paused(reason, paused, session.clock().now());
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

/// Exactly one of `passphrase` and `use_saved_passphrase: true` is given.
#[tauri::command]
pub async fn open_database(
    path: String,
    passphrase: Option<String>,
    use_saved_passphrase: Option<bool>,
    remember_passphrase: Option<bool>,
    take_over: Option<bool>,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<DatabaseStatus, CommandError> {
    let passphrase = passphrase.map(Passphrase::from_input);
    let unlock = match (&passphrase, use_saved_passphrase.unwrap_or(false)) {
        (Some(passphrase), false) => {
            Unlock::Typed { passphrase, remember: remember_passphrase.unwrap_or(false) }
        }
        (None, true) => Unlock::Saved,
        _ => {
            return Err(CommandError::validation(
                "Give a passphrase, or use the saved one.",
                [("passphrase".to_owned(), "Enter the passphrase.".to_owned())].into(),
            ));
        }
    };
    ops::open_database(&session, &machine, &path, unlock, take_over.unwrap_or(false))
}

#[tauri::command]
pub async fn close_database(
    reason: CloseReason,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<CloseOutcome, CommandError> {
    ops::close_database(&session, &machine, reason)
}

/// Closes the open database, if any, as a quit, then exits (research.md
/// §17). The frontend has already dealt with unsaved changes.
#[tauri::command]
pub async fn quit_application(
    app: AppHandle,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<(), CommandError> {
    if session.is_open() {
        // It can only fail when nothing is open any more.
        crate::session::lifecycle::close_normal(&session, &machine, CloseReason::Quit).ok();
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

/// Where the page-1 probe of a passphrase check goes (research.md §1a).
pub(crate) const PROBE_DIR: &str = "passphrase-probes";

#[tauri::command]
pub async fn save_passphrase(
    passphrase: String,
    app: AppHandle,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<PassphraseSaved, CommandError> {
    let passphrase = Passphrase::from_input(passphrase);
    let scratch = app.path().app_cache_dir().map_err(|err| {
        log::error!("no cache directory for the passphrase check: {err}");
        CommandError::new("INTERNAL_ERROR", "The passphrase couldn't be checked.")
    })?;
    ops::save_passphrase(&session, &machine, &scratch.join(PROBE_DIR), &passphrase)
}

#[tauri::command]
pub async fn forget_saved_passphrase(
    path: Option<String>,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<PassphraseSaved, CommandError> {
    ops::forget_saved_passphrase(&session, &machine, path.as_deref())
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

#[tauri::command]
pub async fn update_backup_settings(
    enabled: bool,
    keep_count: i64,
    location: BackupLocationInput,
    existing_backups: Option<ExistingBackupsChoice>,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<BackupSettingsSaved, CommandError> {
    ops::update_backup_settings(
        &session,
        &machine,
        &BackupSettingsInput { enabled, keep_count, location, existing_backups },
    )
}

#[tauri::command]
pub async fn skip_backup(session: State<'_, Session>) -> Result<(), CommandError> {
    ops::skip_backup(session.operations());
    Ok(())
}

#[tauri::command]
pub async fn update_lock_settings(
    idle_enabled: bool,
    idle_minutes: i64,
    on_screen_lock: bool,
    session: State<'_, Session>,
) -> Result<CollectionSettings, CommandError> {
    ops::update_lock_settings(
        &session,
        &LockSettingsInput { idle_enabled, idle_minutes, on_screen_lock },
    )
}

#[tauri::command]
pub async fn lock_database(
    draft: Option<Draft>,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<CloseOutcome, CommandError> {
    ops::lock_database(&session, &machine, draft)
}

#[tauri::command]
pub async fn stage_pending_changes(
    draft: Option<Draft>,
    session: State<'_, Session>,
) -> Result<(), CommandError> {
    ops::stage_pending_changes(&session, draft)
}

#[tauri::command]
pub async fn resolve_pending_changes(
    action: PendingAction,
    session: State<'_, Session>,
) -> Result<PendingResolved, CommandError> {
    ops::resolve_pending_changes(&session, action)
}

#[tauri::command]
pub async fn note_activity(session: State<'_, Session>) -> Result<(), CommandError> {
    ops::note_activity(&session);
    Ok(())
}

#[tauri::command]
pub async fn set_idle_paused(
    reason: IdlePauseReason,
    paused: bool,
    session: State<'_, Session>,
) -> Result<(), CommandError> {
    ops::set_idle_paused(&session, reason, paused);
    Ok(())
}
