use rusqlite::{named_params, Connection, OptionalExtension};
use tauri::State;

use crate::commands::firearms::DeleteResult;
use crate::commands::CommandError;
use crate::db::DbHandle;
use crate::models::document_attachment::{DocumentAttachment, DocumentDetail, DocumentSummary};

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
        Ok(DeleteResult { deleted: true })
    }
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
