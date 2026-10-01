use rusqlite::Row;
use serde::Serialize;

use crate::models::record::RecordRef;

#[derive(Debug, Clone)]
pub struct DocumentAttachment {
    pub id: i64,
    pub owner: RecordRef,
    pub file_bytes: Vec<u8>,
    pub original_filename: String,
    pub mime_type: String,
    pub created_at: String,
}

impl DocumentAttachment {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            owner: RecordRef::from_owner_row(row)?,
            file_bytes: row.get("file_bytes")?,
            original_filename: row.get("original_filename")?,
            mime_type: row.get("mime_type")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// The frontend-facing DTO for listing a record's documents — omits
/// `file_bytes` (read only by `open_document`, to reopen it, FR-010) to keep
/// `list_documents` payloads small.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    pub id: i64,
    pub owner: RecordRef,
    pub original_filename: String,
    pub mime_type: String,
    pub created_at: String,
}

impl From<DocumentAttachment> for DocumentSummary {
    fn from(doc: DocumentAttachment) -> Self {
        Self {
            id: doc.id,
            owner: doc.owner,
            original_filename: doc.original_filename,
            mime_type: doc.mime_type,
            created_at: doc.created_at,
        }
    }
}
