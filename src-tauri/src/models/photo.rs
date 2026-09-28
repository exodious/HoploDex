use std::io::Cursor;

use image::ImageReader;
use image::codecs::jpeg::JpegEncoder;
use rusqlite::Row;
use serde::Serialize;

use crate::commands::CommandError;

/// Small cached thumbnail dimension (research.md §4: "~200px JPEG") — list/
/// tile browse views read only this, never the full-resolution original,
/// so browse performance is independent of photo file size (Principle IV).
const THUMBNAIL_MAX_DIMENSION: u32 = 200;

const SUPPORTED_MIME_TYPES: &[&str] = &["image/jpeg", "image/png"];

pub fn validate_photo_mime_type(mime_type: &str) -> Result<(), CommandError> {
    if SUPPORTED_MIME_TYPES.contains(&mime_type) {
        Ok(())
    } else {
        Err(CommandError::new(
            "VALIDATION_ERROR",
            format!("Unsupported photo type: {mime_type}. Use JPEG or PNG."),
        ))
    }
}

/// Decodes `original_bytes` and re-encodes a small JPEG thumbnail capped at
/// `THUMBNAIL_MAX_DIMENSION` on its longest side, generated once at insert
/// time (research.md §4) rather than on every browse render.
pub fn generate_thumbnail(original_bytes: &[u8]) -> Result<Vec<u8>, CommandError> {
    let image = ImageReader::new(Cursor::new(original_bytes))
        .with_guessed_format()
        .map_err(|_| CommandError::new("VALIDATION_ERROR", "Could not read the image file."))?
        .decode()
        .map_err(|_| CommandError::new("VALIDATION_ERROR", "Could not decode the image file."))?;

    let thumbnail = image.thumbnail(THUMBNAIL_MAX_DIMENSION, THUMBNAIL_MAX_DIMENSION);

    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 80)
        .encode_image(&thumbnail)
        .map_err(|_| CommandError::new("INTERNAL_ERROR", "Failed to generate a thumbnail."))?;
    Ok(bytes)
}

/// Full `Photo` row, including `original_bytes` — used internally for
/// export (FR-018) and thumbnail regeneration, never serialized directly to
/// the frontend (see `PhotoSummary`, which omits the potentially large
/// original bytes).
#[derive(Debug, Clone)]
pub struct Photo {
    pub id: i64,
    pub firearm_id: i64,
    pub original_bytes: Vec<u8>,
    pub original_filename: String,
    pub mime_type: String,
    pub thumbnail_bytes: Vec<u8>,
    pub sort_order: i64,
    pub created_at: String,
}

impl Photo {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            firearm_id: row.get("firearm_id")?,
            original_bytes: row.get("original_bytes")?,
            original_filename: row.get("original_filename")?,
            mime_type: row.get("mime_type")?,
            thumbnail_bytes: row.get("thumbnail_bytes")?,
            sort_order: row.get("sort_order")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// The frontend-facing DTO for a photo — omits `original_bytes` (only ever
/// needed server-side for export) to keep `list_photos`/`add_photo`
/// payloads small.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoSummary {
    pub id: i64,
    pub firearm_id: i64,
    pub original_filename: String,
    pub mime_type: String,
    pub thumbnail_bytes: Vec<u8>,
    pub sort_order: i64,
    pub created_at: String,
}

impl From<Photo> for PhotoSummary {
    fn from(photo: Photo) -> Self {
        Self {
            id: photo.id,
            firearm_id: photo.firearm_id,
            original_filename: photo.original_filename,
            mime_type: photo.mime_type,
            thumbnail_bytes: photo.thumbnail_bytes,
            sort_order: photo.sort_order,
            created_at: photo.created_at,
        }
    }
}
