use std::path::{Path, PathBuf};

use rusqlite::{named_params, Connection, OptionalExtension};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::commands::firearms::DeleteResult;
use crate::commands::CommandError;
use crate::db::DbHandle;
use crate::models::document_attachment::{DocumentAttachment, DocumentDetail, DocumentSummary};
use crate::services::attachments::read_attachment_file;
use crate::services::secure_delete::secure_delete_dir;

/// Pure, `Connection`-based business logic — mirrors `commands::firearms::ops`
/// (constitution: no mocks, integration tests call these directly against a
/// real temporary SQLCipher database).
pub mod ops {
    use super::*;

    pub fn get_document(conn: &Connection, id: i64) -> Result<DocumentAttachment, CommandError> {
        conn.query_row(
            "SELECT * FROM document_attachments WHERE id = :id",
            named_params! { ":id": id },
            DocumentAttachment::from_row,
        )
        .optional()
        .map_err(CommandError::from_db)?
        .ok_or_else(|| CommandError::not_found("No document was found with that id."))
    }

    pub fn list_documents(
        conn: &Connection,
        firearm_id: i64,
    ) -> Result<Vec<DocumentAttachment>, CommandError> {
        let mut stmt = conn
            .prepare(
                "SELECT * FROM document_attachments WHERE firearm_id = :firearm_id ORDER BY created_at",
            )
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map(named_params! { ":firearm_id": firearm_id }, DocumentAttachment::from_row)
            .map_err(CommandError::from_db)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CommandError::from_db)
    }

    pub fn add_document(
        conn: &Connection,
        firearm_id: i64,
        file_bytes: &[u8],
        original_filename: &str,
        mime_type: &str,
    ) -> Result<DocumentAttachment, CommandError> {
        conn.execute(
            "INSERT INTO document_attachments (
                firearm_id, file_bytes, original_filename, mime_type, created_at
            ) VALUES (:firearm_id, :file_bytes, :original_filename, :mime_type, datetime('now'))",
            named_params! {
                ":firearm_id": firearm_id,
                ":file_bytes": file_bytes,
                ":original_filename": original_filename,
                ":mime_type": mime_type,
            },
        )
        .map_err(CommandError::from_db)?;

        get_document(conn, conn.last_insert_rowid())
    }

    /// `add_document` for a file already on disk — a document dropped onto
    /// the window arrives as a path, not as bytes.
    pub fn add_document_from_path(
        conn: &Connection,
        firearm_id: i64,
        path: &Path,
    ) -> Result<DocumentAttachment, CommandError> {
        let file = read_attachment_file(path)?;
        add_document(conn, firearm_id, &file.bytes, &file.filename, file.mime_type)
    }

    pub fn delete_document(
        conn: &Connection,
        id: i64,
        confirmed: bool,
    ) -> Result<DeleteResult, CommandError> {
        if !confirmed {
            return Err(CommandError::new(
                "CONFIRMATION_REQUIRED",
                "Deletion must be explicitly confirmed.",
            ));
        }
        let deleted = conn
            .execute("DELETE FROM document_attachments WHERE id = :id", named_params! { ":id": id })
            .map_err(CommandError::from_db)?;
        if deleted == 0 {
            return Err(CommandError::not_found("No document was found with that id."));
        }
        crate::db::reclaim_freed_space(conn);
        Ok(DeleteResult { deleted: true })
    }

    /// Writes `document` into `dir` (created if absent) under its original
    /// filename, reduced to a single path component with characters that
    /// are invalid on any supported OS replaced — the copy `open_document`
    /// hands to the OS default app.
    pub fn write_document_copy(
        dir: &Path,
        document: &DocumentAttachment,
    ) -> Result<PathBuf, CommandError> {
        let io_error = |e: std::io::Error| {
            CommandError::new("INTERNAL_ERROR", format!("Could not prepare the document: {e}"))
        };
        std::fs::create_dir_all(dir).map_err(io_error)?;
        let path = dir.join(safe_file_name(&document.original_filename));
        std::fs::write(&path, &document.file_bytes).map_err(io_error)?;
        Ok(path)
    }

    /// Securely deletes every temporary copy `open_document` left under
    /// `dir` (FR-035), returning the files that could not be deleted. Run on
    /// normal exit, and again at every startup as the backstop for crashes,
    /// forced kills, and anything a previous run failed to delete.
    pub fn clear_opened_documents(dir: &Path) -> Vec<PathBuf> {
        secure_delete_dir(dir)
    }

    fn safe_file_name(original: &str) -> String {
        let last_component = original.rsplit(['/', '\\']).next().unwrap_or_default();
        let cleaned: String = last_component
            .chars()
            .map(|c| if c.is_control() || r#"<>:"|?*"#.contains(c) { '_' } else { c })
            .collect();
        let trimmed = cleaned.trim();
        if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
            "document".into()
        } else {
            trimmed.into()
        }
    }
}

/// Folder under the app cache directory that holds the temporary copies
/// `open_document` hands to the OS. Cleared on exit and again at startup
/// (FR-035) so decrypted copies never outlive the session that made them.
pub const OPENED_DOCUMENTS_DIR: &str = "opened-documents";

/// Clears [`OPENED_DOCUMENTS_DIR`] under the app cache directory, logging
/// (never failing on) anything that couldn't be deleted — the next startup
/// sweep retries it.
pub fn clear_opened_documents_cache(app: &AppHandle) {
    let Ok(cache_dir) = app.path().app_cache_dir() else {
        return;
    };
    for path in ops::clear_opened_documents(&cache_dir.join(OPENED_DOCUMENTS_DIR)) {
        log::warn!("could not delete opened-document copy {}", path.display());
    }
}

/// Reopens a document from its record (FR-010) in the OS default app for
/// its file type, via a temporary copy under [`OPENED_DOCUMENTS_DIR`].
#[tauri::command]
pub async fn open_document(
    id: i64,
    app: AppHandle,
    state: State<'_, DbHandle>,
) -> Result<(), CommandError> {
    let document = {
        let conn = state.0.lock().expect("db mutex poisoned");
        ops::get_document(&conn, id)?
    };
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| CommandError::new("INTERNAL_ERROR", e.to_string()))?
        .join(OPENED_DOCUMENTS_DIR)
        .join(id.to_string());
    let path = ops::write_document_copy(&dir, &document)?;
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| {
        CommandError::new("INTERNAL_ERROR", format!("Could not open the document: {e}"))
    })
}

#[tauri::command]
pub async fn list_documents(
    firearm_id: i64,
    state: State<'_, DbHandle>,
) -> Result<Vec<DocumentSummary>, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    Ok(ops::list_documents(&conn, firearm_id)?.into_iter().map(DocumentSummary::from).collect())
}

#[tauri::command]
pub async fn add_document(
    firearm_id: i64,
    file_bytes: Vec<u8>,
    original_filename: String,
    mime_type: String,
    state: State<'_, DbHandle>,
) -> Result<DocumentSummary, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::add_document(&conn, firearm_id, &file_bytes, &original_filename, &mime_type)
        .map(Into::into)
}

/// Attaches a document from a file on disk: what a drop onto the window
/// delivers.
#[tauri::command]
pub async fn add_document_from_path(
    firearm_id: i64,
    path: String,
    state: State<'_, DbHandle>,
) -> Result<DocumentSummary, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::add_document_from_path(&conn, firearm_id, Path::new(&path)).map(Into::into)
}

#[tauri::command]
pub async fn get_document(
    id: i64,
    state: State<'_, DbHandle>,
) -> Result<DocumentDetail, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::get_document(&conn, id).map(Into::into)
}

#[tauri::command]
pub async fn delete_document(
    id: i64,
    confirmed: bool,
    state: State<'_, DbHandle>,
) -> Result<DeleteResult, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::delete_document(&conn, id, confirmed)
}
