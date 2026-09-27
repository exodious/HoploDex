use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use serde_json::{json, Value};

use crate::db::{DbError, OpenError};
use crate::models::database::OperationKind;

/// The single error shape returned by every Tauri IPC command, per
/// contracts/tauri-commands.md. `code` is a stable, machine-matchable
/// identifier (e.g. `"VALIDATION_ERROR"`); `message` is safe to show
/// directly to the user and never contains a passphrase; `field_errors`
/// carries per-field validation failures for form rendering; `details`
/// carries structured data for specific codes (specs/003
/// contracts/tauri-commands.md "Error codes added by this feature").
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_errors: Option<HashMap<String, String>>,
    /// Boxed so every command's `Result` stays small.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Box<Value>>,
}

fn path_details(path: &Path) -> Option<Value> {
    Some(json!({ "path": path.to_string_lossy() }))
}

impl CommandError {
    /// The code of [`CommandError::database_unavailable`], which the session
    /// also looks for in what a command's closure returns.
    pub const DATABASE_UNAVAILABLE: &'static str = "DATABASE_UNAVAILABLE";

    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into(), field_errors: None, details: None }
    }

    fn with_details(mut self, details: Option<Value>) -> Self {
        self.details = details.map(Box::new);
        self
    }

    pub fn validation(message: impl Into<String>, field_errors: HashMap<String, String>) -> Self {
        Self { field_errors: Some(field_errors), ..Self::new("VALIDATION_ERROR", message) }
    }

    /// Puts the error on a form field as well, e.g. a wrong passphrase on
    /// the field it was typed in.
    pub fn on_field(mut self, field: &str, message: impl Into<String>) -> Self {
        self.field_errors.get_or_insert_with(HashMap::new).insert(field.to_owned(), message.into());
        self
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("NOT_FOUND", message)
    }

    /// No database is open, usually a race with a lock or close.
    pub fn database_closed() -> Self {
        Self::new("DATABASE_CLOSED", "No database is open.")
    }

    /// FR-039: pending changes must be resumed or discarded first.
    pub fn pending_changes_unresolved() -> Self {
        Self::new(
            "PENDING_CHANGES_UNRESOLVED",
            "This database has unsaved changes from last time. Resume or discard them first.",
        )
    }

    /// FR-032: another computer replaced or changed the file.
    pub fn database_taken_over() -> Self {
        Self::new(
            "DATABASE_TAKEN_OVER",
            "Another computer has taken over this database. Nothing more was saved here.",
        )
    }

    /// FR-028.
    pub fn database_damaged(backups_available: bool) -> Self {
        Self::new("DATABASE_DAMAGED", "This database is damaged and can't be opened.")
            .with_details(Some(json!({ "backupsAvailable": backups_available })))
    }

    /// FR-006: a wrong passphrase, a file that is not a HoploDex database
    /// and a damaged first page are not told apart.
    pub fn passphrase_incorrect(
        saved_passphrase_failed: Option<bool>,
        backups_available: Option<bool>,
    ) -> Self {
        let mut details = serde_json::Map::new();
        if let Some(failed) = saved_passphrase_failed {
            details.insert("savedPassphraseFailed".into(), failed.into());
        }
        if let Some(available) = backups_available {
            details.insert("backupsAvailable".into(), available.into());
        }
        Self::new(
            "PASSPHRASE_INCORRECT",
            "The passphrase is incorrect, or this file is not a HoploDex database or is damaged.",
        )
        .with_details((!details.is_empty()).then_some(Value::Object(details)))
    }

    pub fn database_not_found(path: &Path) -> Self {
        Self::new("DATABASE_NOT_FOUND", "The database file is not at this location.")
            .with_details(path_details(path))
    }

    pub fn database_unreadable(path: &Path) -> Self {
        Self::new(
            "DATABASE_UNREADABLE",
            "HoploDex can't open this file: there is no permission to, or its storage is \
             unavailable.",
        )
        .with_details(path_details(path))
    }

    /// FR-014.
    pub fn database_in_use() -> Self {
        Self::new(
            "DATABASE_IN_USE",
            "This database is open in another copy of HoploDex on this or another computer.",
        )
    }

    /// FR-014.
    pub fn database_newer_version() -> Self {
        Self::new(
            "DATABASE_NEWER_VERSION",
            "This database was last used by a newer version of HoploDex. Update HoploDex to \
             open it.",
        )
    }

    /// FR-032. `since` is ISO-8601 UTC.
    pub fn database_open_elsewhere(machine_name: &str, since: &str) -> Self {
        Self::new(
            "DATABASE_OPEN_ELSEWHERE",
            format!("This database is open on {machine_name}, or was when it last closed there."),
        )
        .with_details(Some(json!({ "machineName": machine_name, "since": since })))
    }

    pub fn database_exists(path: &Path) -> Self {
        Self::new("DATABASE_EXISTS", "A file with this name already exists in that folder.")
            .with_details(path_details(path))
    }

    /// FR-016.
    pub fn insufficient_space(bytes_needed: u64, bytes_available: u64, path: &Path) -> Self {
        Self::new("INSUFFICIENT_SPACE", "There isn't enough free space.").with_details(Some(
            json!({
                "bytesNeeded": bytes_needed,
                "bytesAvailable": bytes_available,
                "path": path.to_string_lossy(),
            }),
        ))
    }

    /// FR-027. `reason` is `missing`, `notWritable` or `insufficientSpace`.
    pub fn backup_location_unavailable(path: &Path, reason: &str) -> Self {
        Self::new("BACKUP_LOCATION_UNAVAILABLE", "The backup folder can't be used.")
            .with_details(Some(json!({ "path": path.to_string_lossy(), "reason": reason })))
    }

    /// FR-019.
    pub fn keyring_unavailable() -> Self {
        Self::new("KEYRING_UNAVAILABLE", "This computer's keyring isn't available.")
    }

    /// FR-037: a sleep stopped a long operation.
    pub fn operation_stopped(
        operation: OperationKind,
        imported_count: Option<u64>,
        deleted_count: Option<u64>,
    ) -> Self {
        let mut details = json!({ "operation": operation });
        if let Some(count) = imported_count {
            details["importedCount"] = count.into();
        }
        if let Some(count) = deleted_count {
            details["deletedCount"] = count.into();
        }
        Self::new("OPERATION_STOPPED", "The operation was stopped before it finished.")
            .with_details(Some(details))
    }

    /// FR-037: a sleep stopped a move of backups (FR-026), with
    /// `left_behind_count` of them still in `folder`. The new location is
    /// kept.
    pub fn move_stopped(left_behind_count: u64, folder: &Path) -> Self {
        Self::new("OPERATION_STOPPED", "The operation was stopped before it finished.")
            .with_details(Some(json!({
                "operation": OperationKind::MoveBackups,
                "leftBehindCount": left_behind_count,
                "folder": folder.to_string_lossy(),
            })))
    }

    /// FR-026: the backup location changed and `count` backups of the
    /// database, `total_bytes` in all, are at the old one. Nothing was
    /// saved; the UI asks what to do with them and sends the settings again.
    pub fn backups_at_old_location(folder: &Path, count: u64, total_bytes: u64) -> Self {
        Self::new(
            "BACKUPS_AT_OLD_LOCATION",
            "There are backups at the old location. Choose what to do with them.",
        )
        .with_details(Some(json!({
            "folder": folder.to_string_lossy(),
            "count": count,
            "totalBytes": total_bytes,
        })))
    }

    /// FR-026: some backups at the old location couldn't be deleted, so the
    /// old location was kept, and they are still the database's.
    pub fn backups_not_all_deleted(deleted_count: u64, failed_paths: &[String]) -> Self {
        Self::new(
            "BACKUPS_NOT_ALL_DELETED",
            "Some backups couldn't be deleted, so the backup location wasn't changed.",
        )
        .with_details(Some(json!({ "deletedCount": deleted_count, "failedPaths": failed_paths })))
    }

    /// FR-026: the old backup location can't be read, so its backups can't
    /// be moved or deleted from here. Nothing was saved; leaving them is
    /// the only choice.
    pub fn old_backup_location_unavailable(folder: &Path) -> Self {
        Self::new(
            "OLD_BACKUP_LOCATION_UNAVAILABLE",
            "The old backup location can't be reached, so its backups can't be moved or deleted \
             from here.",
        )
        .with_details(Some(json!({ "folder": folder.to_string_lossy() })))
    }

    pub fn operation_in_progress(operation: OperationKind) -> Self {
        Self::new(
            "OPERATION_IN_PROGRESS",
            "Another operation is still running. Try again when it has finished.",
        )
        .with_details(Some(json!({ "operation": operation })))
    }

    /// FR-028: the "before restoring" backup of the current database could
    /// not be made, so the restore went no further.
    pub fn restore_cancelled() -> Self {
        Self::new(
            "RESTORE_CANCELLED",
            "The current database couldn't be backed up, so the restore was cancelled. Nothing \
             has been changed.",
        )
    }

    pub fn confirmation_required(message: impl Into<String>) -> Self {
        Self::new("CONFIRMATION_REQUIRED", message)
    }

    pub fn replace_failed(path: &Path) -> Self {
        Self::new(
            "REPLACE_FAILED",
            "The database file couldn't be replaced. The original is unchanged.",
        )
        .with_details(path_details(path))
    }

    /// FR-032: the file's storage can't be reached.
    pub fn database_unavailable(path: &Path) -> Self {
        Self::new(
            Self::DATABASE_UNAVAILABLE,
            format!(
                "HoploDex can't reach {}. Nothing already saved was lost. Close the database and \
                 open it again once the drive or network is back.",
                path.display()
            ),
        )
        .with_details(path_details(path))
    }

    /// Maps a raw `rusqlite::Error` to a safe, generic `CommandError` for
    /// use with `.map_err(CommandError::from_db)` at call sites — internal
    /// error detail is logged, never forwarded to the frontend. Corruption
    /// found by any command is reported as a damaged database (research.md
    /// §2), and an I/O error as a file that can't be reached, whose path the
    /// session adds (research.md §6).
    pub fn from_db(err: rusqlite::Error) -> Self {
        log::error!("db error: {err}");
        match err {
            rusqlite::Error::QueryReturnedNoRows => {
                Self::not_found("The requested record was not found.")
            }
            _ => match err.sqlite_error_code() {
                Some(rusqlite::ErrorCode::DatabaseCorrupt) => Self::database_damaged(false),
                Some(rusqlite::ErrorCode::SystemIoFailure | rusqlite::ErrorCode::CannotOpen) => {
                    Self::new(Self::DATABASE_UNAVAILABLE, "HoploDex can't reach the database file.")
                }
                _ => Self::new("INTERNAL_ERROR", "An unexpected error occurred."),
            },
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

/// Maps infrastructure failures (DB, filesystem) to a safe, generic
/// `CommandError` — internal error detail is logged, never forwarded to the
/// frontend, since it may include file paths or SQL text.
impl From<DbError> for CommandError {
    fn from(err: DbError) -> Self {
        match err {
            DbError::Sqlite(err) => Self::from_db(err),
            DbError::Exists(path) => Self::database_exists(&path),
            other => {
                log::error!("db error: {other}");
                Self::new("INTERNAL_ERROR", "An unexpected error occurred.")
            }
        }
    }
}

/// One code per open outcome (research.md §2).
impl From<OpenError> for CommandError {
    fn from(err: OpenError) -> Self {
        match err {
            OpenError::NotFound { path } => Self::database_not_found(&path),
            OpenError::Unreadable { path } => Self::database_unreadable(&path),
            OpenError::InUse => Self::database_in_use(),
            OpenError::PassphraseIncorrect => Self::passphrase_incorrect(None, None),
            OpenError::NewerVersion => Self::database_newer_version(),
            OpenError::OpenElsewhere { machine_name, since } => {
                Self::database_open_elsewhere(&machine_name, &since)
            }
            OpenError::Damaged => Self::database_damaged(false),
            OpenError::Internal(err) => Self::from_db(err),
        }
    }
}
