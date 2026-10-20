use std::collections::HashMap;
use std::io;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};

use crate::db::{DbError, OpenError};
use crate::models::database::OperationKind;
use crate::models::record::RecordRef;
use crate::services::disk_space;
use crate::services::import_reader::limits::ImportLimit;
use crate::services::sandbox::Grant;
use crate::session::SessionId;

/// What a `LOCATION_OUT_OF_REACH` message tells the user to do besides the two
/// fixes every one gives (move the file into reach, or run the override
/// command). The attach commands name the button that attaches a file from
/// anywhere: a host path outside reach there can only be a drop, and the file
/// chooser reaches every folder (011 FR-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutOfReachAdvice {
    Move,
    AttachPhoto,
    AttachDocument,
}

/// The single error shape returned by every Tauri IPC command, per
/// 001 contracts/tauri-commands.md. `code` is a stable, machine-matchable
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

    /// 003 FR-039: pending changes must be resumed or discarded first.
    pub fn pending_changes_unresolved() -> Self {
        Self::new(
            "PENDING_CHANGES_UNRESOLVED",
            "This database has unsaved changes from last time. Resume or discard them first.",
        )
    }

    /// 003 FR-032: another computer replaced or changed the file.
    pub fn database_taken_over() -> Self {
        Self::new(
            "DATABASE_TAKEN_OVER",
            "Another computer has taken over this database. Nothing more was saved here.",
        )
    }

    /// 003 FR-028.
    pub fn database_damaged(backups_available: bool) -> Self {
        Self::new("DATABASE_DAMAGED", "This database is damaged and can't be opened.")
            .with_details(Some(json!({ "backupsAvailable": backups_available })))
    }

    /// 003 FR-006: a wrong passphrase, a file that is not a HoploDex database
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

    /// 003 FR-014.
    pub fn database_in_use() -> Self {
        Self::new(
            "DATABASE_IN_USE",
            "This database is open in another copy of HoploDex on this or another computer.",
        )
    }

    /// 003 FR-014.
    ///
    /// 009 FR-018: `backup_folder` is the backup folder this computer cached
    /// for the database, when it has one, so the message can point at a backup
    /// made before the newer version's upgrade.
    pub fn database_newer_version(backup_folder: Option<&Path>) -> Self {
        Self::new(
            "DATABASE_NEWER_VERSION",
            "This database was last used by a newer version of HoploDex. Update HoploDex to \
             open it.",
        )
        .with_details(
            backup_folder.map(|folder| json!({ "backupFolder": folder.to_string_lossy() })),
        )
    }

    /// 009 FR-010: `open_database` found steps due. The database waits,
    /// keyed and locked with nothing written, under `awaiting_id`; `version`
    /// is the build's `APP_VERSION` and `backup_folder` the backup location
    /// resolved on this computer.
    pub fn upgrade_needed_to_open(
        awaiting_id: SessionId,
        version: &str,
        backup_folder: &Path,
    ) -> Self {
        Self::new(
            "UPGRADE_NEEDED",
            "This database was made by an earlier version of HoploDex. It needs to be upgraded \
             before it can be opened.",
        )
        .with_details(Some(json!({
            "awaitingId": awaiting_id,
            "version": version,
            "backupFolder": backup_folder.to_string_lossy(),
        })))
    }

    /// The `awaitingId` of an `UPGRADE_NEEDED` that [`upgrade_needed_to_open`]
    /// made.
    ///
    /// [`upgrade_needed_to_open`]: Self::upgrade_needed_to_open
    pub fn awaiting_id(&self) -> Option<SessionId> {
        let raw = self.details.as_ref()?.get("awaitingId")?.as_u64()?;
        SessionId::from_request(raw)
    }

    /// 009 FR-019: `restore_backup` without `upgrade: true` for a backup that
    /// needs one; nothing was copied or written.
    pub fn upgrade_needed_to_restore(version: &str) -> Self {
        Self::new(
            "UPGRADE_NEEDED",
            "This backup was made by an earlier version of HoploDex. It needs to be upgraded \
             before it can be restored.",
        )
        .with_details(Some(json!({ "version": version })))
    }

    /// 009 FR-015: the upgrade failed (a step, the link check, the commit) and
    /// was rolled back, so the database is as it was. `backup_path` is the
    /// backup made before it; for a restore, the backup restored from.
    pub fn upgrade_failed(backup_path: &Path, at_backup_location: bool) -> Self {
        Self::new(
            "UPGRADE_FAILED",
            "The database couldn't be upgraded. It is as it was before, and the backup made \
             before the upgrade is kept.",
        )
        .with_details(Some(json!({
            "backupPath": backup_path.to_string_lossy(),
            "atBackupLocation": at_backup_location,
        })))
    }

    /// A rolled-back upgrade (a step, the link check, the commit) as the command
    /// reports it: `UPGRADE_FAILED` naming `backup_path` (009 FR-015), or
    /// `DISK_FULL` naming the file `database_path` when its disk filled, and
    /// also `backup_path` and `at_backup_location`, since the backup before the
    /// upgrade was made by then.
    pub fn upgrade_error(
        err: crate::db::upgrade::UpgradeError,
        backup_path: &Path,
        at_backup_location: bool,
        database_path: &Path,
    ) -> Self {
        log::error!("upgrading {} failed and was rolled back: {err}", database_path.display());
        let error = Self::from_upgrade(err);
        if error.code == Self::DISK_FULL {
            let mut error = error.located_at(database_path);
            let mut details = match error.details.take().map(|details| *details) {
                Some(Value::Object(details)) => details,
                _ => serde_json::Map::new(),
            };
            details.insert("backupPath".into(), backup_path.to_string_lossy().into());
            details.insert("atBackupLocation".into(), at_backup_location.into());
            error.details = Some(Box::new(Value::Object(details)));
            error
        } else {
            Self::upgrade_failed(backup_path, at_backup_location)
        }
    }

    /// 009 FR-012: the backup before the upgrade failed while it was being
    /// written (I/O) in `folder`. Nothing was upgraded, and the database still
    /// waits.
    pub fn upgrade_backup_failed(folder: &Path) -> Self {
        Self::new(
            "UPGRADE_BACKUP_FAILED",
            "The backup before the upgrade couldn't be saved, so the database wasn't upgraded. \
             Nothing has been changed.",
        )
        .with_details(Some(json!({ "folder": folder.to_string_lossy() })))
    }

    /// 003 FR-032. `since` is ISO-8601 UTC.
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

    /// 003 FR-016.
    pub fn insufficient_space(bytes_needed: u64, bytes_available: u64, path: &Path) -> Self {
        Self::new("INSUFFICIENT_SPACE", "There isn't enough free space.").with_details(Some(
            json!({
                "bytesNeeded": bytes_needed,
                "bytesAvailable": bytes_available,
                "path": path.to_string_lossy(),
            }),
        ))
    }

    /// The code of [`CommandError::disk_full`].
    pub const DISK_FULL: &'static str = "DISK_FULL";

    /// A write ran out of room while it was being made (issue #110): unlike
    /// `INSUFFICIENT_SPACE`, which refuses before anything is written and
    /// knows how much is needed, this is the disk itself saying it is full,
    /// from SQLite (`SQLITE_FULL`) or the OS (`ENOSPC`, `ERROR_DISK_FULL`).
    /// `path` is the file that was being changed, when the caller knows it
    /// ([`located_at`](Self::located_at) adds it later). Nothing a failed
    /// write began is kept.
    pub fn disk_full(path: Option<&Path>) -> Self {
        let message = match path {
            Some(path) => format!(
                "The disk is full. Free some space on the drive that holds {}, then try again.",
                path.display()
            ),
            None => "The disk is full. Free some space on the drive that holds this database, \
                     then try again."
                .to_owned(),
        };
        Self::new(Self::DISK_FULL, message).with_details(path.and_then(path_details))
    }

    /// Names the file a `DISK_FULL` that a lower layer raised without one
    /// was about; any other error, and a `DISK_FULL` that already names its
    /// file, is returned as it is.
    pub fn located_at(self, path: &Path) -> Self {
        if self.code == Self::DISK_FULL && self.details.is_none() {
            Self::disk_full(Some(path))
        } else {
            self
        }
    }

    /// Sorts a filesystem error: a full disk is `DISK_FULL`, anything else
    /// is what `otherwise` makes of it. The one place an I/O error becomes a
    /// `CommandError`, so a full disk never reads as an unexpected error.
    pub fn from_io(err: &io::Error, otherwise: impl FnOnce() -> Self) -> Self {
        if disk_space::is_disk_full(err) { Self::disk_full(None) } else { otherwise() }
    }

    /// 003 FR-027. `reason` is `missing`, `notWritable` or `insufficientSpace`.
    pub fn backup_location_unavailable(path: &Path, reason: &str) -> Self {
        Self::new("BACKUP_LOCATION_UNAVAILABLE", "The backup folder can't be used.")
            .with_details(Some(json!({ "path": path.to_string_lossy(), "reason": reason })))
    }

    /// 011 FR-004 (011 research.md §5): inside a Flatpak, a location the
    /// operation needs is outside what this copy may reach. `path` is the
    /// location as the user gave it, `folder` the folder that would need
    /// access, `documents` the user's Documents folder when there is one a
    /// Flatpak can be granted, `grants` what this copy can reach now, and
    /// `advice` what to tell the user to do besides the two fixes. Exactly the
    /// shipped default (`<documents>/HoploDex`, `/media` and `/run/media`, each
    /// writable, in any order) is worded as such; any other set is the user's
    /// own and is listed. Never overridable: `confirmedWarnings` has no effect
    /// on it.
    pub fn out_of_reach(
        path: &Path,
        folder: &Path,
        documents: Option<&Path>,
        grants: &[Grant],
        advice: OutOfReachAdvice,
    ) -> Self {
        let roots: Vec<String> =
            grants.iter().map(|grant| grant.root.to_string_lossy().into_owned()).collect();
        let default_folder = documents.map(|documents| documents.join("HoploDex"));
        let shipped = default_folder.as_ref().is_some_and(|default_folder| {
            let default_folder = default_folder.to_string_lossy();
            grants.len() == 3
                && grants.iter().all(|grant| grant.writable)
                && [default_folder.as_ref(), "/media", "/run/media"]
                    .iter()
                    .all(|shipped| roots.iter().any(|root| root == shipped))
        });
        let (reach, moving) = if let (true, Some(default_folder)) = (shipped, &default_folder) {
            let default_folder = default_folder.display();
            (
                format!(
                    "the HoploDex folder in your Documents folder (`{default_folder}`) and \
                     removable drives"
                ),
                format!("Move the file into `{default_folder}` or onto a removable drive"),
            )
        } else if grants.is_empty() {
            ("none of your folders".to_owned(), "Move the file into a folder it can reach".into())
        } else {
            let named: Vec<String> = grants
                .iter()
                .map(|grant| {
                    let read_only = if grant.writable { "" } else { " (read only)" };
                    format!("`{}`{read_only}", grant.root.display())
                })
                .collect();
            let listed = match named.split_last() {
                Some((last, first)) if !first.is_empty() => {
                    format!("{} and {last}", first.join(", "))
                }
                _ => named.join(""),
            };
            (listed, "Move the file into a folder it can reach".into())
        };
        let attach = match advice {
            OutOfReachAdvice::Move => "",
            OutOfReachAdvice::AttachPhoto => {
                " To attach it from where it is, use Add photos and choose it there."
            }
            OutOfReachAdvice::AttachDocument => {
                " To attach it from where it is, use Attach and choose it there."
            }
        };
        Self::new(
            "LOCATION_OUT_OF_REACH",
            format!(
                "This copy of HoploDex is a Flatpak, and it can only reach {reach}, so it can't \
                 use `{path}`.{attach} {moving}, or give HoploDex access to `{folder}` by \
                 running: `flatpak override --user --filesystem={quoted} \
                 io.github.exodious.HoploDex` and then restarting HoploDex.",
                path = path.display(),
                folder = folder.display(),
                quoted = shell_quote(&folder.to_string_lossy()),
            ),
        )
        .with_details(Some(json!({
            "path": path.to_string_lossy(),
            "folder": folder.to_string_lossy(),
            "grants": roots,
        })))
    }

    /// 003 FR-019.
    pub fn keyring_unavailable() -> Self {
        Self::new("KEYRING_UNAVAILABLE", "This computer's keyring isn't available.")
    }

    /// 003 FR-037: a sleep stopped a long operation.
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

    /// 008 FR-006: a file passed one of the import's limits
    /// (`IMPORT_LIMIT_EXCEEDED`, 008 contracts/tauri-commands.md). The message
    /// names the file, the sheet of a workbook, and the limit.
    pub fn import_limit_exceeded(file: &str, sheet: Option<&str>, limit: ImportLimit) -> Self {
        let mut details = json!({ "file": file, "limit": limit });
        if let Some(sheet) = sheet {
            details["sheet"] = sheet.into();
        }
        Self::new("IMPORT_LIMIT_EXCEEDED", limit.message(file, sheet)).with_details(Some(details))
    }

    /// 001 FR-007 and FR-010 (amended 2026-10-11): `owner` already has
    /// `limit` photos or documents (`what`, plural), the most a record may
    /// have (`ATTACHMENT_LIMIT_REACHED`, 001 contracts/tauri-commands.md).
    pub fn attachment_limit_reached(owner: RecordRef, what: &str, limit: u32) -> Self {
        Self::new(
            "ATTACHMENT_LIMIT_REACHED",
            format!("This {} has {limit} {what}, the most it can have.", owner.noun()),
        )
        .with_details(Some(json!({ "limit": limit })))
    }

    /// 008 FR-007: an import file couldn't be read: not a table, damaged,
    /// or the reader's child died, timed out or sent a malformed frame. No
    /// parser detail is shown.
    pub fn import_unreadable(file: &str) -> Self {
        Self::new("VALIDATION_ERROR", format!("{file}: could not read the file."))
    }

    /// 003 FR-037: a sleep stopped a move of backups (003 FR-026), with
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

    /// 003 FR-026: the backup location changed and `count` backups of the
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

    /// 003 FR-026: some backups at the old location couldn't be deleted, so the
    /// old location was kept, and they are still the database's.
    pub fn backups_not_all_deleted(deleted_count: u64, failed_paths: &[String]) -> Self {
        Self::new(
            "BACKUPS_NOT_ALL_DELETED",
            "Some backups couldn't be deleted, so the backup location wasn't changed.",
        )
        .with_details(Some(json!({ "deletedCount": deleted_count, "failedPaths": failed_paths })))
    }

    /// 003 FR-026: the old backup location can't be read, so its backups can't
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

    /// 003 FR-028: the "before restoring" backup of the current database could
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

    /// 003 FR-032: the file's storage can't be reached.
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
    /// found by any command is reported as a damaged database (003 research.md
    /// §2), and an I/O error as a file that can't be reached, whose path the
    /// session adds (003 research.md §6).
    pub fn from_db(err: rusqlite::Error) -> Self {
        log::error!("db error: {err}");
        match err {
            rusqlite::Error::QueryReturnedNoRows => {
                Self::not_found("The requested record was not found.")
            }
            _ => match err.sqlite_error_code() {
                Some(rusqlite::ErrorCode::DatabaseCorrupt) => Self::database_damaged(false),
                Some(rusqlite::ErrorCode::DiskFull) => Self::disk_full(None),
                Some(rusqlite::ErrorCode::SystemIoFailure | rusqlite::ErrorCode::CannotOpen) => {
                    Self::new(Self::DATABASE_UNAVAILABLE, "HoploDex can't reach the database file.")
                }
                _ => Self::new("INTERNAL_ERROR", "An unexpected error occurred."),
            },
        }
    }
}

impl CommandError {
    /// A failed create or upgrade (009 research.md §7), rolled back: the SQLite
    /// error underneath maps as any other (a full disk, a damaged file), and
    /// anything else is an internal error, logged with the step's name or the
    /// link check, never row values.
    pub fn from_upgrade(err: crate::db::upgrade::UpgradeError) -> Self {
        match err.into_sqlite() {
            Ok(sqlite) => Self::from_db(sqlite),
            Err(other) => {
                log::error!("upgrade error: {other}");
                Self::new("INTERNAL_ERROR", "An unexpected error occurred.")
            }
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
            DbError::Upgrade(err) => Self::from_upgrade(err),
            DbError::Io(err) => Self::from_io(&err, || {
                log::error!("db error: {err}");
                Self::new("INTERNAL_ERROR", "An unexpected error occurred.")
            }),
            other => {
                log::error!("db error: {other}");
                Self::new("INTERNAL_ERROR", "An unexpected error occurred.")
            }
        }
    }
}

/// One code per open outcome (003 research.md §2).
impl From<OpenError> for CommandError {
    fn from(err: OpenError) -> Self {
        match err {
            OpenError::NotFound { path } => Self::database_not_found(&path),
            OpenError::Unreadable { path } => Self::database_unreadable(&path),
            OpenError::InUse => Self::database_in_use(),
            OpenError::PassphraseIncorrect => Self::passphrase_incorrect(None, None),
            OpenError::NewerVersion => Self::database_newer_version(None),
            OpenError::OpenElsewhere { machine_name, since } => {
                Self::database_open_elsewhere(&machine_name, &since)
            }
            OpenError::Damaged => Self::database_damaged(false),
            OpenError::Internal(err) => Self::from_db(err),
        }
    }
}

/// `text` as one POSIX shell word, so the `flatpak override` command works if
/// pasted: as it is when every character is plain, else in single quotes with
/// each embedded single quote written `'\''` (the same rule as
/// `src/features/databases/outOfReach.ts`'s `overrideCommand`).
fn shell_quote(text: &str) -> String {
    let plain = !text.is_empty()
        && text.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-')
        });
    if plain { text.to_owned() } else { format!("'{}'", text.replace('\'', r"'\''")) }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{CommandError, OutOfReachAdvice};
    use crate::services::sandbox::Grant;

    const DOCUMENTS: &str = "/home/someone/Documents";

    fn grant(root: &str, writable: bool) -> Grant {
        Grant { root: PathBuf::from(root), writable }
    }

    /// The three grants the manifest ships (011 research.md §4, amended
    /// 2026-10-19).
    fn shipped() -> Vec<Grant> {
        vec![
            grant("/home/someone/Documents/HoploDex", true),
            grant("/media", true),
            grant("/run/media", true),
        ]
    }

    /// A refusal for `/x/y`, folder `/x`, with Documents at [`DOCUMENTS`].
    fn refusal(grants: &[Grant], advice: OutOfReachAdvice) -> CommandError {
        CommandError::out_of_reach(
            Path::new("/x/y"),
            Path::new("/x"),
            Some(Path::new(DOCUMENTS)),
            grants,
            advice,
        )
    }

    const DEFAULT_REACH: &str = "it can only reach the HoploDex folder in your Documents folder \
        (`/home/someone/Documents/HoploDex`) and removable drives";

    #[test]
    fn out_of_reach_has_its_code_and_camel_case_details() {
        let err = CommandError::out_of_reach(
            Path::new("/srv/guns/a.hoplodex"),
            Path::new("/srv/guns"),
            Some(Path::new(DOCUMENTS)),
            &shipped(),
            OutOfReachAdvice::Move,
        );
        assert_eq!(err.code, "LOCATION_OUT_OF_REACH");
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(
            json["details"],
            serde_json::json!({
                "path": "/srv/guns/a.hoplodex",
                "folder": "/srv/guns",
                "grants": ["/home/someone/Documents/HoploDex", "/media", "/run/media"],
            })
        );
        assert!(json.get("fieldErrors").is_none());
    }

    /// 011 FR-004: the default's wording, word for word as 011 data-model.md gives
    /// it, which the frontend's tests copy.
    #[test]
    fn out_of_reach_message_is_the_one_data_model_gives_word_for_word() {
        let err = CommandError::out_of_reach(
            Path::new("/srv/guns/a.hoplodex"),
            Path::new("/srv/guns"),
            Some(Path::new(DOCUMENTS)),
            &shipped(),
            OutOfReachAdvice::Move,
        );
        assert_eq!(
            err.message,
            "This copy of HoploDex is a Flatpak, and it can only reach the HoploDex folder in \
             your Documents folder (`/home/someone/Documents/HoploDex`) and removable drives, so \
             it can't use `/srv/guns/a.hoplodex`. Move the file into \
             `/home/someone/Documents/HoploDex` or onto a removable drive, or give HoploDex \
             access to `/srv/guns` by running: `flatpak override --user --filesystem=/srv/guns \
             io.github.exodious.HoploDex` and then restarting HoploDex."
        );
    }

    /// 011 FR-007: an attach command's refusal puts the sentence naming the
    /// button before the two fixes; nothing else changes.
    #[test]
    fn out_of_reach_for_an_attach_command_says_to_use_its_button_first() {
        let moved = refusal(&shipped(), OutOfReachAdvice::Move).message;
        let photo = refusal(&shipped(), OutOfReachAdvice::AttachPhoto).message;
        let document = refusal(&shipped(), OutOfReachAdvice::AttachDocument).message;
        assert_eq!(
            photo,
            moved.replace(
                "can't use `/x/y`. Move",
                "can't use `/x/y`. To attach it from where it is, use Add photos and choose it \
                 there. Move"
            )
        );
        assert_ne!(photo, moved);
        assert_eq!(
            document,
            moved.replace(
                "can't use `/x/y`. Move",
                "can't use `/x/y`. To attach it from where it is, use Attach and choose it \
                 there. Move"
            )
        );
        assert_ne!(document, moved);
    }

    #[test]
    fn out_of_reach_lists_the_grants_when_the_user_has_added_more_than_the_shipped_three() {
        let mut grants = shipped();
        grants.push(grant("/srv/archive", true));
        grants.push(grant("/data", false));
        let err = refusal(&grants, OutOfReachAdvice::Move);
        assert!(
            err.message.starts_with(
                "This copy of HoploDex is a Flatpak, and it can only reach \
                 `/home/someone/Documents/HoploDex`, `/media`, `/run/media`, `/srv/archive` and \
                 `/data` (read only), so it can't use `/x/y`."
            ),
            "{}",
            err.message
        );
        assert!(!err.message.contains("removable drives, so"));
        // The advice after the first sentence does not name a folder it
        // can't know.
        assert!(err.message.ends_with(
            "Move the file into a folder it can reach, or give HoploDex access to `/x` by \
             running: `flatpak override --user --filesystem=/x io.github.exodious.HoploDex` and \
             then restarting HoploDex."
        ));
        assert_eq!(
            serde_json::to_value(&err).unwrap()["details"]["grants"].as_array().unwrap().len(),
            5
        );
    }

    /// 011 FR-004: only the shipped three, exactly, get the default wording.
    #[test]
    fn out_of_reach_lists_the_grants_for_any_set_but_the_shipped_three() {
        let say = |grants: &[Grant]| refusal(grants, OutOfReachAdvice::Move).message;
        assert!(say(&shipped()).contains(DEFAULT_REACH));
        // In any order.
        let mut shuffled = shipped();
        shuffled.reverse();
        assert!(say(&shuffled).contains(DEFAULT_REACH));
        // The HoploDex folder removed.
        assert!(!say(&shipped()[1..]).contains(DEFAULT_REACH));
        // One added, among them the old home folder and `/mnt`.
        for extra in ["/srv/archive", "/home/someone", "/mnt"] {
            let mut more = shipped();
            more.push(grant(extra, true));
            let message = say(&more);
            assert!(!message.contains(DEFAULT_REACH), "{extra}: {message}");
            assert!(message.contains(&format!("`{extra}`")), "{extra}: {message}");
        }
        // One made read-only.
        let mut read_only = shipped();
        read_only[2].writable = false;
        let message = say(&read_only);
        assert!(!message.contains(DEFAULT_REACH), "{message}");
        assert!(message.contains("`/run/media` (read only)"), "{message}");
        // A removable folder missing and another put in its place.
        let mut swapped = shipped();
        swapped[2] = grant("/srv/archive", true);
        assert!(!say(&swapped).contains(DEFAULT_REACH));
    }

    /// 011 FR-004: the default is named by the Documents folder the caller
    /// gives, not by whichever grant is not a drive folder. A folder named
    /// `HoploDex` somewhere else is not the default, and with no Documents
    /// folder nothing is.
    #[test]
    fn out_of_reach_names_the_default_by_the_documents_folder_it_is_given() {
        let elsewhere = vec![
            grant("/srv/Documents/HoploDex", true),
            grant("/media", true),
            grant("/run/media", true),
        ];
        let message = refusal(&elsewhere, OutOfReachAdvice::Move).message;
        assert!(!message.contains(DEFAULT_REACH), "{message}");
        assert!(message.contains("it can only reach `/srv/Documents/HoploDex`, `/media` and"));
        // The same grants are the default when that is the Documents folder.
        let message = CommandError::out_of_reach(
            Path::new("/x/y"),
            Path::new("/x"),
            Some(Path::new("/srv/Documents")),
            &elsewhere,
            OutOfReachAdvice::Move,
        )
        .message;
        assert!(message.contains("(`/srv/Documents/HoploDex`) and removable drives"), "{message}");
        assert!(message.contains("Move the file into `/srv/Documents/HoploDex` or onto"));
        // No Documents folder the Flatpak can be granted: the drives only.
        let drives = vec![grant("/media", true), grant("/run/media", true)];
        let message = CommandError::out_of_reach(
            Path::new("/x/y"),
            Path::new("/x"),
            None,
            &drives,
            OutOfReachAdvice::Move,
        )
        .message;
        assert!(message.contains("it can only reach `/media` and `/run/media`, so"), "{message}");
        assert!(!message.contains("Documents"), "{message}");
    }

    #[test]
    fn out_of_reach_with_no_grants_says_none_of_your_folders() {
        let message = refusal(&[], OutOfReachAdvice::Move).message;
        assert!(message.contains("it can only reach none of your folders, so"), "{message}");
    }

    /// 011 FR-004: the folder in the `flatpak override` command is one shell
    /// word; the other mentions of it are not quoted.
    #[test]
    fn out_of_reach_quotes_the_override_folder_only_when_a_shell_would_split_it() {
        let say = |folder: &str| {
            CommandError::out_of_reach(
                Path::new("/x/y"),
                Path::new(folder),
                Some(Path::new(DOCUMENTS)),
                &shipped(),
                OutOfReachAdvice::Move,
            )
            .message
        };
        let message = say("/srv/my guns");
        assert!(message.contains("access to `/srv/my guns` by running"), "{message}");
        assert!(message.contains("--filesystem='/srv/my guns' io.github"), "{message}");
        let message = say("/srv/it's $HOME");
        assert!(message.contains(r"--filesystem='/srv/it'\''s $HOME' io.github"), "{message}");
        assert!(say("/srv/plain-1_a.b").contains("--filesystem=/srv/plain-1_a.b io.github"));
    }

    #[test]
    fn out_of_reach_carries_a_field_error_only_when_the_caller_adds_one() {
        let err = refusal(&shipped(), OutOfReachAdvice::Move);
        assert!(err.field_errors.is_none());
        let message = err.message.clone();
        let err = err.on_field("folder", message.clone());
        assert_eq!(err.field_errors.as_ref().unwrap()["folder"], message);
        assert_eq!(err.code, "LOCATION_OUT_OF_REACH");
    }

    /// 011 contracts/tauri-commands.md "Errors": `confirmedWarnings` has no
    /// effect on it. Nothing in the error offers an override: it is a plain
    /// refusal with no warning code for a resend to confirm.
    #[test]
    fn out_of_reach_is_a_refusal_no_confirmation_overrides() {
        let err = refusal(&shipped(), OutOfReachAdvice::AttachPhoto);
        assert!(!err.code.ends_with("_WARNING"), "{}", err.code);
        let json = serde_json::to_value(&err).unwrap();
        assert!(json["details"].get("confirmedWarnings").is_none());
        assert!(json["details"].get("overridable").is_none());
    }
}
