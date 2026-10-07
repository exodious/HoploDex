use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, named_params};
use tauri::{AppHandle, State};

use crate::app_dirs;
use crate::commands::CommandError;
use crate::commands::firearms::DeleteResult;
use crate::models::document_attachment::{DocumentAttachment, DocumentSummary};
use crate::models::record::RecordRef;
use crate::services::attachments::read_attachment_file;
use crate::services::document_types::{self, DocumentType, classify};
use crate::services::preview::availability::PdfAvailabilityState;
use crate::services::secure_delete::secure_delete_dir;
use crate::session::Session;

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
        owner: RecordRef,
    ) -> Result<Vec<DocumentAttachment>, CommandError> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT * FROM document_attachments WHERE {} = :id ORDER BY created_at, id",
                owner.owner_column()
            ))
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map(named_params! { ":id": owner.id() }, DocumentAttachment::from_row)
            .map_err(CommandError::from_db)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CommandError::from_db)
    }

    /// The document types in table order (contracts/tauri-commands.md
    /// `list_document_types`): what the picker's `accept` and the drop
    /// router read, so the frontend keeps no list of its own.
    pub fn list_document_types() -> &'static [DocumentType] {
        document_types::all()
    }

    pub fn add_document(
        conn: &Connection,
        owner: RecordRef,
        file_bytes: &[u8],
        original_filename: &str,
    ) -> Result<DocumentAttachment, CommandError> {
        // The type recorded is the one the content check finds, whatever the
        // file chooser said (FR-016). Nothing is stored on a refusal.
        let document_type = classify(original_filename, file_bytes)?;
        let exists: bool = conn
            .query_row(
                &format!("SELECT EXISTS (SELECT 1 FROM {} WHERE id = :id)", owner.table()),
                named_params! { ":id": owner.id() },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;
        if !exists {
            return Err(CommandError::not_found(format!(
                "No {} was found with that id.",
                owner.noun()
            )));
        }
        let (firearm_id, accessory_id) = owner.owner_columns();
        conn.execute(
            "INSERT INTO document_attachments (
                firearm_id, accessory_id, file_bytes, original_filename, mime_type, created_at
            ) VALUES (
                :firearm_id, :accessory_id, :file_bytes, :original_filename, :mime_type,
                datetime('now')
            )",
            named_params! {
                ":firearm_id": firearm_id,
                ":accessory_id": accessory_id,
                ":file_bytes": file_bytes,
                ":original_filename": original_filename,
                ":mime_type": document_type.mime_type,
            },
        )
        .map_err(CommandError::from_db)?;

        get_document(conn, conn.last_insert_rowid())
    }

    /// `add_document` for a file already on disk — a document dropped onto
    /// the window arrives as a path, not as bytes. It is classified by its
    /// content like any other.
    pub fn add_document_from_path(
        conn: &Connection,
        owner: RecordRef,
        path: &Path,
    ) -> Result<DocumentAttachment, CommandError> {
        let file = read_attachment_file(path)?;
        add_document(conn, owner, &file.bytes, &file.filename)
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
        // The file name is in the document-names index (research.md §19).
        crate::db::reclaim_deleted_record(conn);
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
    let Ok(cache_dir) = app_dirs::cache_dir(app) else {
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
    session: State<'_, Session>,
) -> Result<(), CommandError> {
    let document = session.read(|conn| ops::get_document(conn, id))?;
    let dir = app_dirs::cache_dir(&app)
        .map_err(|e| CommandError::new("INTERNAL_ERROR", e.to_string()))?
        .join(OPENED_DOCUMENTS_DIR)
        .join(id.to_string());
    let path = ops::write_document_copy(&dir, &document)?;
    hand_to_os(&app, &path)
}

/// Opens `path` in the OS default app for its file type.
#[cfg(not(feature = "e2e"))]
fn hand_to_os(app: &AppHandle, path: &Path) -> Result<(), CommandError> {
    use tauri_plugin_opener::OpenerExt;

    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| {
        CommandError::new("INTERNAL_ERROR", format!("Could not open the document: {e}"))
    })
}

/// An E2E build never hands the copy on, which would start a real viewer on
/// the test machine, and no harness can stub the default app everywhere:
/// Windows and macOS look it up in their own registrations, not on `PATH`
/// (#27). It appends the copy's path to `HOPLODEX_E2E_OPENED_LOG` instead,
/// a file in the sandbox, for the specs to check.
#[cfg(feature = "e2e")]
fn hand_to_os(_app: &AppHandle, path: &Path) -> Result<(), CommandError> {
    use std::io::Write;

    let Some(log) = std::env::var_os("HOPLODEX_E2E_OPENED_LOG") else {
        return Ok(());
    };
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .and_then(|mut file| writeln!(file, "{}", path.display()))
        .map_err(|e| {
            CommandError::new("INTERNAL_ERROR", format!("Could not open the document: {e}"))
        })
}

#[tauri::command]
pub async fn list_documents(
    owner: RecordRef,
    session: State<'_, Session>,
    pdf: State<'_, PdfAvailabilityState>,
) -> Result<Vec<DocumentSummary>, CommandError> {
    let pdf = pdf.get();
    session.read(|conn| {
        Ok(ops::list_documents(conn, owner)?
            .into_iter()
            .map(|document| DocumentSummary::new(document, &pdf))
            .collect())
    })
}

/// The document types, for the picker and the drop router.
#[tauri::command]
pub async fn list_document_types() -> Result<&'static [DocumentType], CommandError> {
    Ok(ops::list_document_types())
}

#[tauri::command]
pub async fn add_document(
    owner: RecordRef,
    file_bytes: Vec<u8>,
    original_filename: String,
    session: State<'_, Session>,
    pdf: State<'_, PdfAvailabilityState>,
) -> Result<DocumentSummary, CommandError> {
    let pdf = pdf.get();
    session.write(|conn| {
        ops::add_document(conn, owner, &file_bytes, &original_filename)
            .map(|document| DocumentSummary::new(document, &pdf))
    })
}

/// Attaches a document from a file on disk: what a drop onto the window
/// delivers.
#[tauri::command]
pub async fn add_document_from_path(
    owner: RecordRef,
    path: String,
    session: State<'_, Session>,
    pdf: State<'_, PdfAvailabilityState>,
) -> Result<DocumentSummary, CommandError> {
    let pdf = pdf.get();
    session.write(|conn| {
        ops::add_document_from_path(conn, owner, Path::new(&path))
            .map(|document| DocumentSummary::new(document, &pdf))
    })
}

#[tauri::command]
pub async fn delete_document(
    id: i64,
    confirmed: bool,
    session: State<'_, Session>,
) -> Result<DeleteResult, CommandError> {
    // The preview of the document goes first: its bytes, helper and surface
    // must not outlive the document (contracts/tauri-commands.md
    // `delete_document`). Nothing is closed for a refused deletion.
    if confirmed {
        crate::commands::preview::ops::close_preview_of(&session, id)?;
    }
    session.write(|conn| ops::delete_document(conn, id, confirmed))
}
