//! The shared types of specs/003-database-protection-management's
//! contracts/tauri-commands.md, camelCase over IPC and mirrored by
//! `src/features/databases/types.ts`. Each user story adds its own
//! `validate_*_input` here.

use serde::{Deserialize, Serialize};

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
});

text_enum!(DraftKind {
    Firearm => "firearm",
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

text_enum!(BackupLocationKind {
    Default => "default",
    Custom => "custom",
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
}

/// Something to tell the user in the chooser, once (data-model.md
/// "Machine-local"). Kept in `machine.json` until shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ChooserNotice {
    Closed {
        reason: CloseReason,
        database_path: String,
    },
    OperationStopped {
        database_path: String,
        operation: OperationKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        imported_count: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deleted_count: Option<u64>,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
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
    pub passphrase_saved: bool,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub file_name: String,
    pub made_at: String,
    pub size_bytes: u64,
}
