use rusqlite::Row;
use serde::Serialize;

use crate::models::record::RecordRef;
use crate::services::document_types::from_recorded;
use crate::services::preview::availability::PdfAvailability;

/// How a document can be previewed in HoploDex (FR-001), from its recorded
/// type alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewKind {
    Pdf,
    Tiff,
    Text,
}

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
/// `file_bytes` (read only to preview or reopen a document, FR-010) to keep
/// `list_documents` payloads small. The last three fields are derived on
/// read (data-model.md "Derived on read"), never stored.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    pub id: i64,
    pub owner: RecordRef,
    pub original_filename: String,
    pub mime_type: String,
    pub created_at: String,
    /// From the recorded type alone; the content is confirmed again when the
    /// preview opens.
    pub preview_kind: Option<PreviewKind>,
    /// `preview_kind` is set, except a PDF while PDF preview is off on this
    /// computer (FR-003a).
    pub preview_available: bool,
    /// The recorded type is a document type. `false` only for a row stored
    /// before 007 with another type, which stays listed and deletable
    /// (FR-017).
    pub openable: bool,
}

impl DocumentSummary {
    pub fn new(doc: DocumentAttachment, pdf: &PdfAvailability) -> Self {
        let recorded = from_recorded(&doc.mime_type);
        let preview_kind = recorded.and_then(|t| t.preview_kind);
        let preview_available = match preview_kind {
            Some(PreviewKind::Pdf) => pdf.is_available(),
            Some(_) => true,
            None => false,
        };
        Self {
            id: doc.id,
            owner: doc.owner,
            original_filename: doc.original_filename,
            mime_type: doc.mime_type,
            created_at: doc.created_at,
            preview_kind,
            preview_available,
            openable: recorded.is_some(),
        }
    }
}
