//! The shared types of specs/003-database-protection-management's
//! contracts/tauri-commands.md, camelCase over IPC and mirrored by
//! `src/features/databases/types.ts`. Each user story adds its own
//! `validate_*_input` here.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::commands::CommandError;
use crate::services::passphrase::{Passphrase, new_passphrase_problem};

/// A database file's extension, for databases and backups alike (research.md
/// §19).
pub const DATABASE_EXTENSION: &str = "hoplodex";

text_enum!(CloseReason {
    Closed => "closed",
    Switched => "switched",
    Quit => "quit",
    LockedByUser => "lockedByUser",
    Idle => "idle",
    ScreenLocked => "screenLocked",
    Sleep => "sleep",
    Shutdown => "shutdown",
    TakenOver => "takenOver",
});

text_enum!(OperationKind {
    Backup => "backup",
    PassphraseChange => "passphraseChange",
    Restore => "restore",
    Import => "import",
    Export => "export",
    DeleteBackups => "deleteBackups",
    MoveBackups => "moveBackups",
});

text_enum!(DraftKind {
    Firearm => "firearm",
    Accessory => "accessory",
    Policy => "policy",
});

text_enum!(DraftMode {
    Add => "add",
    Edit => "edit",
    Dispose => "dispose",
    Restore => "restore",
    Coverage => "coverage",
});

text_enum!(BackupOutcome {
    Made => "made",
    NotDue => "notDue",
    AlreadyToday => "alreadyToday",
    Off => "off",
    Skipped => "skipped",
    Failed => "failed",
    NotAttempted => "notAttempted",
});

// Why a backup was not made. `Interrupted` only appears in a chooser notice
// (an unfinished backup found at startup), never in a `CloseOutcome`.
text_enum!(BackupFailureReason {
    LocationUnavailable => "locationUnavailable",
    InsufficientSpace => "insufficientSpace",
    Interrupted => "interrupted",
    Io => "io",
    DatabaseUnreachable => "databaseUnreachable",
});

// A note shown once in the collection (contracts/ui-databases.md §10).
text_enum!(NoteKind {
    DiskEncryption => "diskEncryption",
    OpenedBackup => "openedBackup",
    Restored => "restored",
});

text_enum!(BackupLocationKind {
    Default => "default",
    Custom => "custom",
});

// What the pending-changes prompt chose (FR-039).
text_enum!(PendingAction {
    Resume => "resume",
    Discard => "discard",
});

// What to do with the backups at the old location when it changes (FR-026,
// research.md §22).
text_enum!(ExistingBackupsChoice {
    Move => "move",
    Leave => "leave",
    Delete => "delete",
});

// Why a move of backups left some at the old location (FR-026). `NameTaken`
// only when taken names were the only reason.
text_enum!(LeftBehindReason {
    NameTaken => "nameTaken",
    LocationUnavailable => "locationUnavailable",
    InsufficientSpace => "insufficientSpace",
    Io => "io",
});

// Why the idle clock is paused from the frontend (research.md §15). Running
// operations pause it from the backend's own registry.
text_enum!(IdlePauseReason {
    NativeDialog => "nativeDialog",
});

/// A recent-list entry as the chooser shows it (FR-012).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentDatabase {
    pub path: String,
    /// The file name without its extension.
    pub name: String,
    /// ISO-8601 UTC.
    pub last_opened_at: String,
    /// The file exists at `path`.
    pub available: bool,
    /// FR-017, on this computer.
    pub passphrase_saved: bool,
    /// The backups at its backup location (FR-040), or `None` when this
    /// computer doesn't know where they are, can't read the folder, or a
    /// chosen folder isn't there. A missing default folder means none yet.
    pub backups: Option<BackupSummary>,
    /// The file's modification time, ISO-8601 UTC, when it has changed since
    /// this computer last closed it (FR-040): another computer, or something
    /// outside HoploDex, has written to it.
    pub changed_since_left_at: Option<String>,
}

/// The backups of a database the chooser lists (FR-040), found by their
/// file names alone, so nothing is decrypted.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub count: usize,
    /// Local date and time, as `BackupInfo::made_at`. `None` when there are
    /// none.
    pub latest_made_at: Option<String>,
    pub oldest_made_at: Option<String>,
}

/// `remove_recent_database`'s answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentRemoved {
    pub removed: bool,
}

/// `save_passphrase`'s and `forget_saved_passphrase`'s answer (FR-017,
/// FR-018).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PassphraseSaved {
    pub passphrase_saved: bool,
}

/// Something to tell the user in the chooser, once (data-model.md
/// "Machine-local"). Kept in `machine.json` until shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ChooserNotice {
    /// A lock (FR-033); the chooser shows nothing for an ordinary close.
    Closed {
        reason: CloseReason,
        database_path: String,
        /// The idle duration, for an idle lock's "after <n> minutes without
        /// use".
        #[serde(default, skip_serializing_if = "Option::is_none")]
        idle_minutes: Option<i64>,
    },
    OperationStopped {
        database_path: String,
        operation: OperationKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        imported_count: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deleted_count: Option<u64>,
        /// A move of backups (FR-026): how many are still in `folder`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        left_behind_count: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
    /// A move of backups cut short by a crash or a forced quit (research.md
    /// §22): `count` of them are still in `folder`.
    BackupsLeftBehind {
        database_path: String,
        folder: String,
        count: u64,
    },
    PendingChangesLost {
        database_path: String,
    },
    BackupFailed {
        database_path: String,
        reason: BackupFailureReason,
    },
    TakenOver {
        database_path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupLocation {
    pub kind: BackupLocationKind,
    /// Resolved on this computer.
    pub path: String,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettings {
    pub enabled: bool,
    /// 1–100.
    pub keep_count: i64,
    pub location: BackupLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockSettings {
    pub idle_enabled: bool,
    /// 1–240.
    pub idle_minutes: i64,
    pub on_screen_lock: bool,
}

/// The backup and lock settings kept inside the database
/// (`collection_settings`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSettings {
    pub backups: BackupSettings,
    pub lock: LockSettings,
}

/// The pending changes a database holds, as the prompt describes them
/// (FR-039).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingSummary {
    /// The form's draft version, which the frontend checks it still knows.
    pub form_version: i64,
    pub kind: DraftKind,
    pub mode: DraftMode,
    pub target_id: Option<i64>,
    pub label: String,
    pub saved_at: String,
    /// False when the target no longer exists or the form version is unknown.
    pub resumable: bool,
}

/// A form's unsaved input, mirrored from the frontend (research.md §16).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub form_version: i64,
    pub kind: DraftKind,
    pub mode: DraftMode,
    pub target_id: Option<i64>,
    pub label: String,
    /// The form's own state, opaque to the backend; at most 1 MiB serialized.
    pub values: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedBackupNote {
    pub backup_of_name: String,
    pub made_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseNotes {
    /// FR-008, until dismissed.
    pub disk_encryption: bool,
    /// research.md §9, once.
    pub opened_backup: Option<OpenedBackupNote>,
    /// ISO time of the backup, once after a restore.
    pub restored_with_passphrase_of: Option<String>,
    /// Where a damaged database was set aside, once after a restore.
    pub damaged_file_kept_at: Option<String>,
}

/// The open database, as the frontend needs it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    pub path: String,
    pub name: String,
    /// FR-017, on this computer.
    pub passphrase_saved: bool,
    /// Passphrases can be saved on this computer (FR-019), for the
    /// settings' "This computer" section.
    pub keyring_available: bool,
    /// This desktop reports a screen lock (FR-038), for the settings'
    /// "Locking" section.
    pub screen_lock_supported: bool,
    pub settings: CollectionSettings,
    pub pending_changes: Option<PendingSummary>,
    pub notes: DatabaseNotes,
}

/// What a normal close did about the backup.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseOutcome {
    pub backup: BackupOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<BackupFailureReason>,
}

impl CloseOutcome {
    pub fn backup(backup: BackupOutcome) -> Self {
        Self { backup, failure_reason: None }
    }
}

/// `list_backups`'s answer: a database's backup folder and what is in it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupList {
    pub folder: String,
    /// The folder exists on this computer.
    pub available: bool,
    /// Newest first.
    pub backups: Vec<BackupInfo>,
}

/// `delete_all_backups`'s answer (FR-029).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupsDeleted {
    pub deleted_count: u64,
    /// The backups that could not be deleted, left in place.
    pub failed_paths: Vec<String>,
}

/// `change_passphrase`'s answer (FR-016, FR-018, US4-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PassphraseChanged {
    /// The previous file was securely deleted.
    pub old_file_removed: bool,
    /// Where the previous file still is, opening with the old passphrase,
    /// when it couldn't be deleted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_file_path: Option<String>,
    /// The passphrase saved in this computer's keyring was updated to the
    /// new one.
    pub passphrase_saved: bool,
}

/// Where backups go, as the settings dialog sends it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum BackupLocationInput {
    /// A "HoploDex backups" folder next to the database.
    Default,
    Custom {
        path: String,
    },
}

/// `update_lock_settings`'s input (FR-034, FR-038).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockSettingsInput {
    pub idle_enabled: bool,
    pub idle_minutes: i64,
    pub on_screen_lock: bool,
}

/// `resolve_pending_changes`'s answer: the draft to resume, or none after a
/// discard.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingResolved {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<Draft>,
}

/// `update_backup_settings`'s input (FR-024, FR-026).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettingsInput {
    pub enabled: bool,
    pub keep_count: i64,
    pub location: BackupLocationInput,
    /// For a changed location with backups at the old one: what to do with
    /// them. Absent until the user has been asked (research.md §22).
    #[serde(default)]
    pub existing_backups: Option<ExistingBackupsChoice>,
}

/// Backups a move left at the old location, and why (FR-026).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftBehind {
    pub count: u64,
    pub folder: String,
    pub reason: LeftBehindReason,
}

/// What was done with the backups at the old location (FR-026).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ExistingBackupsOutcome {
    Leave,
    Delete { deleted_count: u64 },
    Move { moved_count: u64, left_behind: Option<LeftBehind> },
}

/// `update_backup_settings`'s answer: the saved settings, and what was done
/// with the backups at the old location, `None` when the location did not
/// change or no backups were there.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettingsSaved {
    pub settings: CollectionSettings,
    pub existing_backups: Option<ExistingBackupsOutcome>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub file_name: String,
    pub made_at: String,
    pub size_bytes: u64,
}

/// Where the create dialog suggests putting a new database (FR-009,
/// research.md §19).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedLocation {
    pub folder: String,
    pub name: String,
}

/// Everything the chooser shows before a database is open.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChooserState {
    /// Most recent first.
    pub recent: Vec<RecentDatabase>,
    /// The row selected when the chooser appears (FR-021, FR-033).
    pub selected_path: Option<String>,
    /// FR-019.
    pub keyring_available: bool,
    /// FR-038: whether this desktop reports a screen lock.
    pub screen_lock_supported: bool,
    pub suggested: SuggestedLocation,
    /// Shown once, then gone.
    pub notices: Vec<ChooserNotice>,
}

/// The most characters a database name may have.
const MAX_NAME_CHARS: usize = 120;
/// The most bytes a database name may take. File names are limited to 255
/// bytes, and the longest one made from a database name, a backup being
/// written (`<name> YYYY-MM-DD HHMMSS <id8>.hoplodex.partial`, research.md
/// §7), adds 44, so a name of 120 accented or non-Latin characters could
/// otherwise make files that can't be created.
const MAX_NAME_BYTES: usize = 200;
/// Characters a file name can't hold on at least one supported OS.
const FORBIDDEN_NAME_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

fn name_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        Some("Enter a name.")
    } else if name.chars().count() > MAX_NAME_CHARS {
        Some("Use at most 120 characters.")
    } else if name.len() > MAX_NAME_BYTES {
        Some("Use a shorter name.")
    } else if name.chars().any(|c| FORBIDDEN_NAME_CHARS.contains(&c) || c.is_control()) {
        Some("A name can't contain < > : \" / \\ | ? * or control characters.")
    } else if name == "." || name == ".." {
        Some("Choose a different name.")
    } else if name.ends_with(' ') || name.ends_with('.') {
        Some("A name can't end with a space or a dot.")
    } else {
        None
    }
}

/// A folder may not exist yet: the suggested `<Documents>/HoploDex` usually
/// doesn't on first run, and creating the database makes it. One that exists
/// must be a folder HoploDex can write to.
fn folder_problem(folder: &Path) -> Option<&'static str> {
    if folder.as_os_str().is_empty() {
        return Some("Enter a folder.");
    }
    if !folder.is_absolute() {
        return Some("Enter the full path of a folder.");
    }
    match fs::metadata(folder) {
        Ok(meta) if !meta.is_dir() => Some("This isn't a folder."),
        Ok(meta) if meta.permissions().readonly() => Some("HoploDex can't write to this folder."),
        _ => None,
    }
}

/// Checks the create dialog's input (data-model.md "Validation rules") and
/// returns the file to create, `<folder>/<name>.hoplodex`. Every field's
/// problem is reported at once; `DATABASE_EXISTS` only once the fields are
/// right.
pub fn validate_create_database_input(
    folder: &str,
    name: &str,
    passphrase: &Passphrase,
    acknowledged_unrecoverable: bool,
) -> Result<PathBuf, CommandError> {
    let folder = Path::new(folder);
    let mut errors = HashMap::new();
    if let Some(problem) = name_problem(name) {
        errors.insert("name".to_owned(), problem.to_owned());
    }
    if let Some(problem) = folder_problem(folder) {
        errors.insert("folder".to_owned(), problem.to_owned());
    }
    if let Some(problem) = new_passphrase_problem(passphrase) {
        errors.insert("passphrase".to_owned(), problem.to_owned());
    }
    // FR-004: there is no recovery, and the user has said they know it.
    if !acknowledged_unrecoverable {
        errors.insert(
            "acknowledgedUnrecoverable".to_owned(),
            "Confirm that you have stored the passphrase somewhere safe.".to_owned(),
        );
    }
    if !errors.is_empty() {
        return Err(CommandError::validation("Check the new database's details.", errors));
    }
    let target = folder.join(format!("{name}.{DATABASE_EXTENSION}"));
    // `symlink_metadata`, so even a dangling link at the path counts as taken.
    if fs::symlink_metadata(&target).is_ok() {
        return Err(CommandError::database_exists(&target));
    }
    Ok(target)
}

/// The most backups a database may keep (data-model.md).
const MAX_KEEP_COUNT: i64 = 100;

/// Checks the backup settings (data-model.md "Validation rules") and
/// returns the location as `collection_settings.backup_location` stores
/// it: `default`, or the absolute path. A path that does not exist is
/// accepted, so a location on another computer's drive can be kept; the
/// settings then report it unavailable here.
pub fn validate_backup_settings_input(input: &BackupSettingsInput) -> Result<String, CommandError> {
    let mut errors = HashMap::new();
    if !(1..=MAX_KEEP_COUNT).contains(&input.keep_count) {
        errors.insert("keepCount".to_owned(), "Keep between 1 and 100 backups.".to_owned());
    }
    let location = match &input.location {
        BackupLocationInput::Default => "default".to_owned(),
        BackupLocationInput::Custom { path } => {
            if path.trim().is_empty() {
                errors.insert("location".to_owned(), "Choose a folder.".to_owned());
            } else if !Path::new(path).is_absolute() {
                errors.insert("location".to_owned(), "Enter the full path of a folder.".to_owned());
            } else if path.chars().any(char::is_control) {
                errors.insert(
                    "location".to_owned(),
                    "A folder can't contain control characters.".to_owned(),
                );
            }
            path.clone()
        }
    };
    if !errors.is_empty() {
        return Err(CommandError::validation("Check the backup settings.", errors));
    }
    Ok(location)
}

/// The longest idle duration, in minutes (data-model.md).
const MAX_IDLE_MINUTES: i64 = 240;

/// Checks the lock settings (data-model.md "Validation rules"):
/// `idle_lock_minutes` 1–240, whether or not the idle lock is on.
pub fn validate_lock_settings_input(input: &LockSettingsInput) -> Result<(), CommandError> {
    if (1..=MAX_IDLE_MINUTES).contains(&input.idle_minutes) {
        return Ok(());
    }
    Err(CommandError::validation(
        "Check the lock settings.",
        [("idleMinutes".to_owned(), "Choose between 1 and 240 minutes.".to_owned())].into(),
    ))
}
