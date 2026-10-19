//! Choosing, creating and opening databases, and the open database's own
//! status (003 contracts/tauri-commands.md). The commands are thin: each
//! passphrase argument becomes a [`Passphrase`] at once, and is dropped, and
//! wiped, when the command returns (003 FR-007). The logic is in [`ops`],
//! which takes its paths and `machine.json` from the caller so the tests use
//! throwaway ones.

use std::sync::Arc;

use tauri::ipc::Request;
use tauri::{AppHandle, Manager, State};

use crate::app_dirs;
use crate::commands::CommandError;
use crate::models::database::DocumentOpeningChanged;
use crate::models::database::{
    BackupLocationInput, BackupSettingsInput, BackupSettingsSaved, ChooserState, CloseOutcome,
    CloseReason, CollectionSettings, DatabaseStatus, Draft, ExistingBackupsChoice, IdlePauseReason,
    LockSettingsInput, NoteKind, PassphraseSaved, PendingAction, PendingResolved, RecentDatabase,
    RecentRemoved,
};
use crate::models::document_opening::DocumentOpening;
use crate::services::consent::Consent;
use crate::services::machine_settings::MachineSettings;
use crate::services::passphrase::Passphrase;
use crate::services::sandbox::Reach;
use crate::session::Session;
use crate::session::scoped::{ScopedSession, Target};
use ops::Unlock;

pub mod ops {
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};

    use rusqlite::Connection;
    use serde_json::json;

    use crate::commands::CommandError;
    use crate::commands::backups::ops::delete_backups_in;
    use crate::commands::documents::ops::IdlePause;
    use crate::db;
    use crate::models::database::DocumentOpeningChanged;
    use crate::models::database::{
        BackupLocation, BackupLocationKind, BackupSettings, BackupSettingsInput,
        BackupSettingsSaved, BackupSummary, ChooserState, CloseOutcome, CloseReason,
        CollectionSettings, DatabaseNotes, DatabaseStatus, Draft, ExistingBackupsChoice,
        ExistingBackupsOutcome, IdlePauseReason, LeftBehind, LockSettings, LockSettingsInput,
        NoteKind, OperationKind, PassphraseSaved, PendingAction, PendingResolved, RecentDatabase,
        RecentRemoved, SuggestedLocation, UnavailableReason, validate_backup_settings_input,
        validate_create_database_input, validate_lock_settings_input,
    };
    use crate::models::document_opening::DocumentOpening;
    use crate::services::backups::{self, LocationProblem, MoveJob};
    use crate::services::consent::{Consent, ConsentAnswer, ConsentRequest};
    use crate::services::disk_space;
    use crate::services::machine_settings::{self, MachineSettings, RecentEntry};
    use crate::services::passphrase::Passphrase;
    use crate::services::sandbox::{Need, Reach};
    use crate::session::operations::Operations;
    use crate::session::scoped::Target;
    use crate::session::{OpenDatabase, Session, SessionId};
    use crate::session::{lifecycle, pending};

    /// The name the create dialog suggests (003 research.md §19).
    pub const SUGGESTED_NAME: &str = "My collection";

    /// The folder the create dialog suggests: `HoploDex` in the documents
    /// folder, or in `~/Documents` when the OS names none. Never the app
    /// data directory, where the pre-feature database lives (003 research.md
    /// §19).
    pub fn suggested_folder(documents: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
        documents
            .or_else(|| home.map(|home| home.join("Documents")))
            .map(|documents| documents.join("HoploDex"))
            .unwrap_or_default()
    }

    /// A recent-list entry as the chooser shows it: unavailable when its
    /// file isn't at its path (003 FR-012), or, inside a Flatpak, when its
    /// folder is out of reach (011 FR-004: judged first and without touching
    /// the file, so the row says so instead of "not found"), and otherwise with
    /// its backups and whether it changed after this computer left it
    /// (003 FR-040). Only file names and times are read: nothing is decrypted.
    fn recent_database(entry: RecentEntry, reach: &Reach) -> RecentDatabase {
        let out_of_reach = reach.check(&entry.path, Need::DatabaseFolder).is_err();
        let available = !out_of_reach && entry.path.is_file();
        let backups = match (&entry.database_id, &entry.backup_folder) {
            (Some(id), Some(folder)) if available => backup_summary(&entry.path, folder, id),
            _ => None,
        };
        let changed_since_left_at = entry.left_modified_at.as_deref().and_then(|left| {
            let now = machine_settings::modified_at(&entry.path)?;
            (later(&now, left)?).then_some(now)
        });
        RecentDatabase {
            available,
            unavailable_reason: out_of_reach.then_some(UnavailableReason::OutOfReach),
            path: entry.path.to_string_lossy().into_owned(),
            name: entry.name,
            last_opened_at: entry.last_opened_at,
            passphrase_saved: entry.passphrase_saved,
            backups,
            changed_since_left_at,
        }
    }

    /// A missing folder means no backups only when it is the default one,
    /// which the first backup makes. A chosen folder that isn't there may be
    /// on a drive that isn't plugged in, so then nothing is said (003 FR-040).
    fn backup_summary(database: &Path, folder: &Path, database_id: &str) -> Option<BackupSummary> {
        if matches!(folder.try_exists(), Ok(false))
            && folder != backups::resolve_folder(database, "default")
        {
            return None;
        }
        match backups::list(folder, database_id) {
            Ok(found) => Some(BackupSummary {
                count: found.len(),
                latest_made_at: found.first().map(|backup| backup.made_at.clone()),
                oldest_made_at: found.last().map(|backup| backup.made_at.clone()),
            }),
            Err(err) => {
                log::warn!("could not list the backups in {}: {err}", folder.display());
                None
            }
        }
    }

    /// Whether the time `a` is after `b`; `None` when either doesn't parse.
    fn later(a: &str, b: &str) -> Option<bool> {
        let parse = |text: &str| chrono::DateTime::parse_from_rfc3339(text).ok();
        Some(parse(a)? > parse(b)?)
    }

    /// The chooser's state (003 FR-012, FR-020, FR-021). The selected row is the
    /// database just closed or locked (003 FR-033), or else the most recent.
    /// Whether passphrases can be saved here is probed the first time it is
    /// asked (003 FR-019); whether the screen lock is reported, when the
    /// system-events listener started (003 FR-038).
    pub fn chooser_state(
        session: &Session,
        machine: &MachineSettings,
        documents: Option<PathBuf>,
        home: Option<PathBuf>,
        reach: &Reach,
    ) -> ChooserState {
        let recent: Vec<RecentDatabase> =
            machine.recent().into_iter().map(|entry| recent_database(entry, reach)).collect();
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
    /// (003 FR-003, FR-004, FR-009). A missing folder is made first. Inside a
    /// Flatpak a folder out of reach is refused first, on the folder field,
    /// with nothing made (011 FR-004, 011 US1-8).
    pub fn create_database(
        session: &Session,
        machine: &MachineSettings,
        folder: &str,
        name: &str,
        passphrase: &Passphrase,
        acknowledged_unrecoverable: bool,
        reach: &Reach,
    ) -> Result<DatabaseStatus, CommandError> {
        let path =
            validate_create_database_input(folder, name, passphrase, acknowledged_unrecoverable)?;
        let path = reach.check(&path, Need::DatabaseFolder).map_err(|refusal| {
            let message = refusal.message.clone();
            refusal.on_field("folder", message)
        })?;
        let folder = path.parent().unwrap_or(Path::new(folder));
        fs::create_dir_all(folder).map_err(|err| folder_error(folder, &err))?;
        lifecycle::create(session, machine, &path, passphrase)?;
        database_status(session, machine)
    }

    /// The field error for a folder the new database can't be put in.
    pub(crate) fn folder_error(folder: &Path, err: &io::Error) -> CommandError {
        log::warn!("could not use {} for a new database: {err}", folder.display());
        if disk_space::is_disk_full(err) {
            return CommandError::disk_full(Some(folder));
        }
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
        /// it only after the 003 FR-017 confirmation.
        Typed { passphrase: &'a Passphrase, remember: bool },
        /// The passphrase saved in this computer's keyring (003 FR-017).
        Saved,
    }

    impl<'a> Unlock<'a> {
        /// A typed passphrase, not to be remembered.
        pub fn typed(passphrase: &'a Passphrase) -> Self {
            Self::Typed { passphrase, remember: false }
        }
    }

    /// Opens the database at `path` (003 FR-005, FR-006), closing any other open
    /// database first as a switch. A refused open changes neither the file
    /// nor the session. `take_over` opens a database marked open on another
    /// computer (003 FR-032); the UI sends it only after its confirmation. A
    /// database that is damaged, or that the passphrase didn't open (the two
    /// can't be told apart at page 1), says whether it has backups to
    /// restore from (003 FR-028, US3-6).
    ///
    /// The saved passphrase is found through the recent entry's cached
    /// database id, since the file can't be read without it. When it no
    /// longer opens the database the refusal says so (003 US5-5), and the next
    /// typed passphrase that does replaces it (003 FR-018).
    ///
    /// Inside a Flatpak the database's folder must be within reach first
    /// (011 FR-004): a Documents-portal path whose host path is, opens from
    /// the host path, which the recent list then names. A refused open touches
    /// nothing.
    pub fn open_database(
        session: &Session,
        machine: &MachineSettings,
        path: &str,
        unlock: Unlock<'_>,
        take_over: bool,
        reach: &Reach,
    ) -> Result<DatabaseStatus, CommandError> {
        let checked = reach.check(Path::new(path), Need::DatabaseFolder)?;
        let path = checked.as_path();
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
        // Whether the open saves or refreshes the passphrase in the keyring
        // once it succeeds (003 FR-017, FR-018), as the recent entry stood
        // before the open.
        let save_typed = match unlock {
            Unlock::Typed { remember, .. } => {
                remember || machine.recent_entry(path).is_some_and(|entry| entry.passphrase_saved)
            }
            Unlock::Saved => false,
        };
        match lifecycle::open(session, machine, path, passphrase, take_over) {
            Ok(()) => {
                if let (Unlock::Typed { passphrase, .. }, true) = (unlock, save_typed) {
                    save_if_listed(machine, path, passphrase);
                }
                database_status(session, machine)
            }
            Err(err) if err.code == "UPGRADE_NEEDED" => {
                // Saving waits for the upgrade to succeed, so the passphrase
                // stays with the database that waits, and nowhere else.
                if let (Unlock::Typed { passphrase, .. }, true) = (unlock, save_typed)
                    && let Some(id) = err.awaiting_id()
                {
                    session.keep_passphrase_for_upgrade(id, passphrase.clone());
                }
                Err(err)
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
            Err(err) if err.code == "DATABASE_NEWER_VERSION" => {
                // The backup folder this computer cached for it, for the
                // message that points at a backup made before the upgrade
                // (009 FR-018).
                let folder = machine.recent_entry(path).and_then(|entry| entry.backup_folder);
                Err(CommandError::database_newer_version(folder.as_deref()))
            }
            Err(err) => Err(err),
        }
    }

    /// Saves the typed `passphrase` in the keyring for the database just
    /// opened at `path`, whose recent entry carries its id.
    fn save_if_listed(machine: &MachineSettings, path: &Path, passphrase: &Passphrase) {
        if let Some(id) = machine.recent_entry(path).and_then(|entry| entry.database_id) {
            save_for(machine, path, &id, passphrase);
        }
    }

    /// Upgrades the database waiting under `awaiting_id` after backing it up,
    /// and opens it (009 FR-011 to FR-017). The database waits again for a
    /// retry when the backup can't be made at its location or in
    /// `backup_folder`, the folder the user chose for this one backup
    /// (009 FR-012). The passphrase kept for the keyring is saved now.
    pub fn upgrade_database(
        session: &Session,
        machine: &MachineSettings,
        awaiting_id: u64,
        backup_folder: Option<&str>,
    ) -> Result<DatabaseStatus, CommandError> {
        let upgraded = crate::session::upgrade::upgrade_database(
            session,
            machine,
            awaiting_id,
            backup_folder.map(Path::new),
        )?;
        if let Some(passphrase) = &upgraded.save_passphrase {
            let path = session.open_path();
            if let Some(path) = path {
                save_if_listed(machine, &path, passphrase);
            }
        }
        database_status(session, machine)
    }

    /// `cancel_upgrade` (009 FR-010, FR-016): drops the database waiting under
    /// `awaiting_id`, or stops its backup; nothing once the upgrade has
    /// started, or for an id that isn't current.
    pub fn cancel_upgrade(session: &Session, awaiting_id: u64) -> Result<(), CommandError> {
        crate::session::upgrade::cancel_upgrade(session, awaiting_id)
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
    /// change (003 FR-018) or a restore (003 research.md §8). Returns whether one is
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

    /// Closes the open database or switches away from it (003 FR-010). The
    /// frontend has already dealt with unsaved changes.
    pub fn close_database(
        session: &Session,
        id: SessionId,
        machine: &MachineSettings,
        reason: CloseReason,
    ) -> Result<CloseOutcome, CommandError> {
        if !matches!(reason, CloseReason::Closed | CloseReason::Switched) {
            return Err(CommandError::validation(
                "A database can only be closed or switched from here.",
                [("reason".to_owned(), "Use closed or switched.".to_owned())].into(),
            ));
        }
        lifecycle::close_normal(session, id, machine, reason)
    }

    /// Takes `path` off this computer's recent list, and forgets its saved
    /// passphrase (003 FR-018). The database file is never touched (003 FR-012,
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

    /// This computer's way of opening a document (007 FR-011); no database
    /// need be open.
    pub fn get_document_opening(machine: &MachineSettings) -> DocumentOpening {
        machine.document_opening()
    }

    /// Sets this computer's way of opening a document (007 FR-011, FR-012).
    /// Choosing `External` while it is `Preview` asks first, and Cancel
    /// leaves it unchanged; every other change is made without asking; the
    /// current value is a no-op. The answer is not the session's: it never
    /// sets `external_open_confirmed` (007 US3-2). A change to `External` clears
    /// an earlier yes in the open database (007 research.md §17, amended
    /// 2026-10-07), so the next open asks once with the "won't be asked
    /// again" line (007 US3-3). No database need be open: `session` is touched
    /// only if one is. While the dialog is up the idle clock waits for it
    /// (007 research.md §16).
    pub fn set_document_opening(
        session: &Session,
        machine: &MachineSettings,
        consent: &dyn Consent,
        value: DocumentOpening,
    ) -> DocumentOpeningChanged {
        if machine.document_opening() == value {
            return DocumentOpeningChanged { changed: false };
        }
        if value == DocumentOpening::External {
            let answer = {
                let _paused = session.is_open().then(|| IdlePause::new(session));
                consent.ask(ConsentRequest::Setting)
            };
            if answer == ConsentAnswer::Cancel {
                return DocumentOpeningChanged { changed: false };
            }
        }
        machine.set_document_opening(value);
        if value == DocumentOpening::External {
            // `DATABASE_CLOSED` only means there is no earlier yes to clear.
            let _ = session.inspect_mut(session.current_id(), |open| {
                open.external_open_confirmed = false;
                Ok(())
            });
        }
        DocumentOpeningChanged { changed: true }
    }

    /// Saves the open database's passphrase in this computer's keyring
    /// (003 FR-017), once it is proved to be the one the file opens with,
    /// against its first page (003 research.md §1a). The UI sends it only after
    /// the 003 FR-017 confirmation. `scratch_dir` takes the page-1 probe.
    pub fn save_passphrase(
        session: &Session,
        id: SessionId,
        machine: &MachineSettings,
        scratch_dir: &Path,
        passphrase: &Passphrase,
    ) -> Result<PassphraseSaved, CommandError> {
        if !machine.keyring().is_available() {
            return Err(CommandError::keyring_unavailable());
        }
        let (path, database_id) = session.inspect(id, |open| {
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
    /// open one (003 FR-018). `KEYRING_UNAVAILABLE`, with nothing changed, when
    /// the keyring can't be reached to delete it.
    pub fn forget_saved_passphrase(
        session: &Session,
        machine: &MachineSettings,
        target: Target<'_>,
    ) -> Result<PassphraseSaved, CommandError> {
        let (path, database_id) = match target {
            Target::At(path) => {
                let path = PathBuf::from(path);
                let id = machine.recent_entry(&path).and_then(|entry| entry.database_id);
                (path, id)
            }
            Target::Open(id) => session
                .inspect(id, |open| Ok((open.path.clone(), Some(open.database_id.clone()))))?,
        };
        if let Some(id) = database_id {
            machine.keyring().forget(&id)?;
            machine.clear_passphrase_saved(&id);
        }
        machine.set_passphrase_saved(&path, false);
        Ok(PassphraseSaved { passphrase_saved: false })
    }

    /// Points an unavailable recent entry at where the user found its file
    /// (003 FR-012), keeping everything else about it.
    pub fn locate_database(
        machine: &MachineSettings,
        path: &str,
        new_path: &str,
        reach: &Reach,
    ) -> Result<RecentDatabase, CommandError> {
        let new_path = reach.check(Path::new(new_path), Need::DatabaseFolder)?;
        machine
            .locate_recent(Path::new(path), &new_path)
            .map(|entry| recent_database(entry, reach))
            .ok_or_else(|| CommandError::not_found("That database isn't in the recent list."))
    }

    /// The open database as the frontend needs it.
    pub fn database_status(
        session: &Session,
        machine: &MachineSettings,
    ) -> Result<DatabaseStatus, CommandError> {
        // The frontend asks which session is open: whatever is.
        session.inspect(session.current_id(), |open| status(open, machine, session.reach()))
    }

    pub(crate) fn status(
        open: &OpenDatabase,
        machine: &MachineSettings,
        reach: &Reach,
    ) -> Result<DatabaseStatus, CommandError> {
        let note_dismissed: bool = open
            .conn
            .query_row("SELECT disk_encryption_note_dismissed FROM app_state", [], |row| row.get(0))
            .map_err(CommandError::from_db)?;
        let passphrase_saved =
            machine.recent().iter().any(|entry| entry.path == open.path && entry.passphrase_saved);
        Ok(DatabaseStatus {
            session_id: open.session_id,
            path: open.path.to_string_lossy().into_owned(),
            name: open.name.clone(),
            passphrase_saved,
            keyring_available: machine.keyring().is_available(),
            screen_lock_supported: crate::platform::screen_lock_supported(),
            settings: collection_settings(&open.conn, &open.path, reach)?,
            pending_changes: pending::summary(&open.conn)?,
            notes: DatabaseNotes { disk_encryption: !note_dismissed, ..open.notes.clone() },
        })
    }

    /// The backup and lock settings kept in the database, with the backup
    /// folder resolved on this computer. A backup folder out of reach is
    /// `available: false` with the reason (011 FR-004), judged without
    /// touching it.
    pub fn collection_settings(
        conn: &Connection,
        database_path: &Path,
        reach: &Reach,
    ) -> Result<CollectionSettings, CommandError> {
        conn.query_row(
            "SELECT backups_enabled, backup_keep_count, backup_location, idle_lock_enabled,
                    idle_lock_minutes, lock_on_screen_lock
             FROM collection_settings",
            [],
            |row| {
                let location: String = row.get(2)?;
                let folder = backups::resolve_folder(database_path, &location);
                let out_of_reach = reach.check(&folder, Need::WriteFolder).is_err();
                let (kind, exists) = if location == "default" {
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
                            available: exists && !out_of_reach,
                            unavailable_reason: out_of_reach
                                .then_some(UnavailableReason::OutOfReach),
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

    /// Dismisses a note for good (`diskEncryption`, 003 FR-008) or for this
    /// session. Dismissing is housekeeping, not a change to back up.
    pub fn dismiss_note(
        session: &Session,
        id: SessionId,
        note: NoteKind,
    ) -> Result<(), CommandError> {
        match note {
            NoteKind::DiskEncryption => session.write_housekeeping(id, |conn| {
                conn.execute("UPDATE app_state SET disk_encryption_note_dismissed = 1", [])
                    .map(|_| ())
                    .map_err(CommandError::from_db)
            }),
            // Session-only notes, shown until dismissed in this session.
            NoteKind::OpenedBackup => session.inspect_mut(id, |open| {
                open.notes.opened_backup = None;
                Ok(())
            }),
            NoteKind::Restored => session.inspect_mut(id, |open| {
                open.notes.restored_with_passphrase_of = None;
                open.notes.damaged_file_kept_at = None;
                Ok(())
            }),
            // 009 FR-017: once, after an open or a restore that upgraded it.
            NoteKind::Upgraded => session.inspect_mut(id, |open| {
                open.notes.upgraded = None;
                Ok(())
            }),
        }
    }

    /// Saves the backup settings (003 FR-024, FR-026), a change to the
    /// collection (003 FR-025). Lowering the number kept deletes nothing now:
    /// the next backup rotates.
    ///
    /// A changed location (its resolved folder, however spelled) with
    /// backups of the database at the old one saves nothing until
    /// `existing_backups` says what to do with them (003 research.md §22):
    /// `leave` saves; `delete` deletes them as "delete all backups" does and
    /// saves only when every one went; `move` checks the new folder and its
    /// space, saves, then moves them. An old folder that can't be read
    /// allows only `leave`. The recent entry's cached backup folder follows
    /// every save.
    ///
    /// Inside a Flatpak a changed custom location out of reach is refused
    /// before the question about existing backups, with nothing saved or made:
    /// as a `VALIDATION_ERROR` on the location field, or, with the backups to
    /// `move`, as `BACKUP_LOCATION_UNAVAILABLE` with the reason `outOfReach`
    /// (011 FR-004, 011 US1-8). The stored location is the host path of a
    /// Documents-portal one that is within reach. A location left as it was
    /// is not checked: saving the other settings writes nothing there.
    pub fn update_backup_settings(
        session: &Session,
        id: SessionId,
        machine: &MachineSettings,
        input: &BackupSettingsInput,
        reach: &Reach,
    ) -> Result<BackupSettingsSaved, CommandError> {
        let mut location = validate_backup_settings_input(input)?;
        let (database_path, database_id, old_location) = session.inspect(id, |open| {
            let old: String = open
                .conn
                .query_row("SELECT backup_location FROM collection_settings", [], |row| row.get(0))
                .map_err(CommandError::from_db)?;
            Ok((open.path.clone(), open.database_id.clone(), old))
        })?;
        let old_folder = backups::resolve_folder(&database_path, &old_location);
        let mut new_folder = backups::resolve_folder(&database_path, &location);
        if location != "default" && !backups::same_path(&old_folder, &new_folder) {
            match reach.check(&new_folder, Need::WriteFolder) {
                Ok(usable) => {
                    new_folder = usable;
                    location = new_folder.to_string_lossy().into_owned();
                }
                Err(_) if input.existing_backups == Some(ExistingBackupsChoice::Move) => {
                    return Err(CommandError::backup_location_unavailable(
                        &new_folder,
                        LocationProblem::OutOfReach.as_reason(),
                    ));
                }
                Err(refusal) => {
                    let message = refusal.message.clone();
                    return Err(CommandError::validation(
                        "Check the backup settings.",
                        [("location".to_owned(), message)].into(),
                    ));
                }
            }
        }
        let change =
            LocationChange { id, input, location: &location, database_path: &database_path, reach };
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
        id: SessionId,
        input: &'a BackupSettingsInput,
        /// As `backup_location` stores it.
        location: &'a str,
        database_path: &'a Path,
        reach: &'a Reach,
    }

    impl LocationChange<'_> {
        /// Saves the settings through `session.write`, so a take-over is
        /// found first, and points the recent entry at the new folder.
        fn save(
            &self,
            session: &Session,
            machine: &MachineSettings,
        ) -> Result<CollectionSettings, CommandError> {
            session.write(self.id, |conn| {
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
            session.inspect(self.id, |open| {
                collection_settings(&open.conn, &open.path, session.reach())
            })
        }
    }

    /// Moves the backups at the old location to the new one (003 FR-026,
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
        backups::check_location(new_folder, new_is_default, change.reach).map_err(|problem| {
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

    /// Stops the backup the current close is making (003 FR-027). Its partial
    /// file is removed and the changes stay waiting. Anything else running
    /// is left alone.
    pub fn skip_backup(operations: &Operations) {
        if operations.running_kind() == Some(OperationKind::Backup) {
            operations.stop_running();
        }
    }

    /// Saves the lock settings (003 FR-034, FR-038), a change to the collection
    /// (003 FR-025). They take effect at once: the idle time starts again.
    pub fn update_lock_settings(
        session: &Session,
        id: SessionId,
        input: &LockSettingsInput,
    ) -> Result<CollectionSettings, CommandError> {
        validate_lock_settings_input(input)?;
        session.write(id, |conn| {
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
        session.inspect_mut(id, |open| {
            open.lock_settings = settings.clone();
            Ok(())
        })?;
        session.idle().start(settings, session.clock().now());
        session.inspect(id, |open| collection_settings(&open.conn, &open.path, session.reach()))
    }

    /// "Lock now" (003 FR-033, FR-035): `draft`, the form's unsaved input, is
    /// kept as pending changes, then the database closes as a lock. No
    /// confirmation.
    pub fn lock_database(
        session: &Session,
        id: SessionId,
        machine: &MachineSettings,
        draft: Option<Draft>,
    ) -> Result<CloseOutcome, CommandError> {
        lifecycle::lock(session, id, machine, CloseReason::LockedByUser, draft)
    }

    /// Mirrors the open form's unsaved input into memory, for a lock the
    /// backend starts on its own (003 research.md §16). `None` clears it.
    pub fn stage_pending_changes(
        session: &Session,
        id: SessionId,
        draft: Option<Draft>,
    ) -> Result<(), CommandError> {
        pending::stage(session, id, draft)
    }

    /// Moves the pending changes along: resume, opened, notOpened or discard
    /// (003 FR-039; 008 FR-003, contracts/tauri-commands.md).
    pub fn resolve_pending_changes(
        session: &Session,
        id: SessionId,
        action: PendingAction,
    ) -> Result<PendingResolved, CommandError> {
        pending::resolve(session, id, action)
    }

    /// Input to the application's windows restarts the idle time (003 FR-035).
    pub fn note_activity(session: &Session) {
        session.idle().note_activity(session.clock().now());
    }

    /// A native file or folder dialog opened or closed: the idle clock
    /// pauses while it is open (003 research.md §15).
    pub fn set_idle_paused(session: &Session, reason: IdlePauseReason, paused: bool) {
        session.idle().set_paused(reason, paused, session.clock().now());
    }
}

#[tauri::command]
pub async fn get_chooser_state(
    app: AppHandle,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
    reach: State<'_, Arc<Reach>>,
) -> Result<ChooserState, CommandError> {
    // `document_dir` honours `user-dirs.dirs`, so a sandboxed run suggests a
    // sandboxed folder (003 research.md §19).
    Ok(ops::chooser_state(
        &session,
        &machine,
        app_dirs::document_dir(&app).ok(),
        app_dirs::home_dir(&app).ok(),
        &reach,
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
    reach: State<'_, Arc<Reach>>,
) -> Result<DatabaseStatus, CommandError> {
    let passphrase = Passphrase::from_input(passphrase);
    ops::create_database(
        &session,
        &machine,
        &folder,
        &name,
        &passphrase,
        acknowledged_unrecoverable,
        &reach,
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
    ops::open_database(
        &session,
        &machine,
        &path,
        unlock,
        take_over.unwrap_or(false),
        session.reach(),
    )
}

/// Upgrades the database `open_database` left waiting (`UPGRADE_NEEDED`):
/// no session exists yet, so none is named (009 contracts/tauri-commands.md).
#[tauri::command]
pub async fn upgrade_database(
    awaiting_id: u64,
    backup_folder: Option<String>,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<DatabaseStatus, CommandError> {
    ops::upgrade_database(&session, &machine, awaiting_id, backup_folder.as_deref())
}

#[tauri::command]
pub async fn cancel_upgrade(
    awaiting_id: u64,
    session: State<'_, Session>,
) -> Result<(), CommandError> {
    ops::cancel_upgrade(&session, awaiting_id)
}

#[tauri::command]
pub async fn close_database(
    reason: CloseReason,
    session: ScopedSession,
    machine: State<'_, MachineSettings>,
) -> Result<CloseOutcome, CommandError> {
    ops::close_database(&session.session, session.id, &machine, reason)
}

/// Closes the open database, if any, as a quit, then exits (003 research.md
/// §17). The frontend has already dealt with unsaved changes.
#[tauri::command]
pub async fn quit_application(
    app: AppHandle,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<(), CommandError> {
    crate::session::lifecycle::quit(&session, &machine);
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
    reach: State<'_, Arc<Reach>>,
) -> Result<RecentDatabase, CommandError> {
    ops::locate_database(&machine, &path, &new_path, &reach)
}

/// Where the page-1 probe of a passphrase check goes (003 research.md §1a).
pub(crate) const PROBE_DIR: &str = "passphrase-probes";

#[tauri::command]
pub async fn save_passphrase(
    passphrase: String,
    app: AppHandle,
    session: ScopedSession,
    machine: State<'_, MachineSettings>,
) -> Result<PassphraseSaved, CommandError> {
    let passphrase = Passphrase::from_input(passphrase);
    let scratch = app_dirs::cache_dir(&app).map_err(|err| {
        log::error!("no cache directory for the passphrase check: {err}");
        CommandError::new("INTERNAL_ERROR", "The passphrase couldn't be checked.")
    })?;
    ops::save_passphrase(
        &session.session,
        session.id,
        &machine,
        &scratch.join(PROBE_DIR),
        &passphrase,
    )
}

/// With a `path`, the passphrase saved for that database, which needs no
/// session; without one, the open database's, which needs the request's
/// (`UNSCOPED_COMMANDS`, 008 contracts/tauri-commands.md).
#[tauri::command]
pub async fn forget_saved_passphrase(
    path: Option<String>,
    request: Request<'_>,
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<PassphraseSaved, CommandError> {
    let target = Target::from_request(path.as_deref(), request.headers())?;
    ops::forget_saved_passphrase(&session, &machine, target)
}

#[tauri::command]
pub async fn get_database_status(
    session: State<'_, Session>,
    machine: State<'_, MachineSettings>,
) -> Result<DatabaseStatus, CommandError> {
    ops::database_status(&session, &machine)
}

#[tauri::command]
pub async fn dismiss_note(note: NoteKind, session: ScopedSession) -> Result<(), CommandError> {
    ops::dismiss_note(&session.session, session.id, note)
}

#[tauri::command]
pub async fn update_backup_settings(
    enabled: bool,
    keep_count: i64,
    location: BackupLocationInput,
    existing_backups: Option<ExistingBackupsChoice>,
    session: ScopedSession,
    machine: State<'_, MachineSettings>,
    reach: State<'_, Arc<Reach>>,
) -> Result<BackupSettingsSaved, CommandError> {
    ops::update_backup_settings(
        &session.session,
        session.id,
        &machine,
        &BackupSettingsInput { enabled, keep_count, location, existing_backups },
        &reach,
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
    session: ScopedSession,
) -> Result<CollectionSettings, CommandError> {
    ops::update_lock_settings(
        &session.session,
        session.id,
        &LockSettingsInput { idle_enabled, idle_minutes, on_screen_lock },
    )
}

#[tauri::command]
pub async fn lock_database(
    draft: Option<Draft>,
    session: ScopedSession,
    machine: State<'_, MachineSettings>,
) -> Result<CloseOutcome, CommandError> {
    ops::lock_database(&session.session, session.id, &machine, draft)
}

#[tauri::command]
pub async fn get_document_opening(
    machine: State<'_, MachineSettings>,
) -> Result<DocumentOpening, CommandError> {
    Ok(ops::get_document_opening(&machine))
}

/// The confirmation blocks until it is answered, so not on an async thread.
#[tauri::command]
pub async fn set_document_opening(
    value: DocumentOpening,
    app: AppHandle,
) -> Result<DocumentOpeningChanged, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = app.state::<Session>();
        let machine = app.state::<MachineSettings>();
        let consent = app.state::<Arc<dyn Consent>>();
        ops::set_document_opening(&session, &machine, &**consent, value)
    })
    .await
    .map_err(|e| CommandError::new("INTERNAL_ERROR", format!("Could not change the setting: {e}")))
}

#[tauri::command]
pub async fn stage_pending_changes(
    draft: Option<Draft>,
    session: ScopedSession,
) -> Result<(), CommandError> {
    ops::stage_pending_changes(&session.session, session.id, draft)
}

#[tauri::command]
pub async fn resolve_pending_changes(
    action: PendingAction,
    session: ScopedSession,
) -> Result<PendingResolved, CommandError> {
    ops::resolve_pending_changes(&session.session, session.id, action)
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

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::Path;

    use super::ops::folder_error;

    /// A folder that can't be made because the disk is full says so
    /// (issue #110), rather than "couldn't create this folder" on the field.
    #[test]
    fn a_folder_that_cannot_be_made_for_lack_of_room_says_the_disk_is_full() {
        let folder = Path::new("/somewhere/HoploDex");
        #[cfg(unix)]
        let code = 28;
        #[cfg(windows)]
        let code = 112;
        let full = folder_error(folder, &io::Error::from_raw_os_error(code));
        assert_eq!(full.code, "DISK_FULL");
        assert!(full.message.contains("Free some space"), "{}", full.message);
        let denied = folder_error(folder, &io::Error::from(io::ErrorKind::PermissionDenied));
        assert_eq!(denied.code, "VALIDATION_ERROR");
    }
}
