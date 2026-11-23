use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::{ImageError, ImageReader, Limits};
use rusqlite::Row;
use serde::Serialize;

use crate::commands::CommandError;
use crate::models::record::RecordRef;

/// Small cached thumbnail dimension (001 research.md §4: "~200px JPEG") — list/
/// tile browse views read only this, never the full-resolution original,
/// so browse performance is independent of photo file size (Principle IV).
const THUMBNAIL_MAX_DIMENSION: u32 = 200;

/// The largest stored thumbnail HoploDex reads. Its own are 200 px JPEGs of a
/// few KiB; one in a database from elsewhere could be as long as a value may
/// be, and every browse tile that shows it asks for it (code scanning alert
/// untrusted-images/cwe-770/src-tauri/src/commands/photos.rs#get_photo_thumbnail).
/// [`PhotoSummary::COLUMNS`] repeats it.
pub const MAX_THUMBNAIL_BYTES: u64 = 1024 * 1024;

/// The type a photo's first bytes show, JPEG or PNG; its name and the type
/// it was sent with don't decide it, and anything else is refused before it is
/// read on or decoded (001 FR-007, amended 2026-11-04).
pub fn photo_type(name: &str, head: &[u8]) -> Result<&'static str, CommandError> {
    if head.starts_with(b"\xFF\xD8\xFF") {
        Ok("image/jpeg")
    } else if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        Ok("image/png")
    } else {
        Err(CommandError::new("VALIDATION_ERROR", format!("{name} isn't a JPEG or PNG photo.")))
    }
}

/// The most pixels a photo may have on a side: zune-jpeg's own default, which
/// `image` would otherwise lift (code scanning alert
/// untrusted-images/cwe-789/src-tauri/src/models/photo.rs#generate_thumbnail).
const MAX_PHOTO_SIDE: u32 = 16_384;

/// The most pixels a JPEG zune-jpeg might decode through coefficient planes
/// may have. It keeps one `i16` per pixel and component for a progressive or
/// multi-scan JPEG before it decodes, outside `image`'s 512 MiB budget, so
/// four components at this size fill the same budget again (the same alert).
/// A larger JPEG is decoded from [`baseline_copy`], or refused.
const MAX_PLANAR_JPEG_PIXELS: u64 = 64 * 1024 * 1024;

/// Decodes `original_bytes` and re-encodes a small JPEG thumbnail capped at
/// `THUMBNAIL_MAX_DIMENSION` on its longest side, generated once at insert
/// time (001 research.md §4) rather than on every browse render. `name`
/// names the photo in a refusal, as `photo_type`'s does.
pub fn generate_thumbnail(name: &str, original_bytes: &[u8]) -> Result<Vec<u8>, CommandError> {
    let reader = |bytes| {
        ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|_| CommandError::new("VALIDATION_ERROR", "Could not read the image file."))
    };
    let too_large = || {
        CommandError::new(
            "VALIDATION_ERROR",
            format!("{name} has more pixels than HoploDex reads."),
        )
    };
    let decode_failed = |err: ImageError| match err {
        ImageError::Limits(_) => too_large(),
        _ => CommandError::new("VALIDATION_ERROR", "Could not decode the image file."),
    };
    let mut bytes = original_bytes;
    let copy;
    if original_bytes.starts_with(b"\xFF\xD8\xFF") {
        // The header alone, as zune-jpeg itself reads it: its decode reads
        // the same frame.
        let (width, height) = reader(bytes)?.into_dimensions().map_err(decode_failed)?;
        if u64::from(width) * u64::from(height) > MAX_PLANAR_JPEG_PIXELS {
            copy = baseline_copy(original_bytes).ok_or_else(too_large)?;
            bytes = &copy;
        }
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_PHOTO_SIDE);
    limits.max_image_height = Some(MAX_PHOTO_SIDE);
    let mut reader = reader(bytes)?;
    reader.limits(limits);
    let image = reader.decode().map_err(decode_failed)?;

    let thumbnail = image.thumbnail(THUMBNAIL_MAX_DIMENSION, THUMBNAIL_MAX_DIMENSION);

    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 80)
        .encode_image(&thumbnail)
        .map_err(|_| CommandError::new("INTERNAL_ERROR", "Failed to generate a thumbnail."))?;
    Ok(bytes)
}

/// A copy of `jpeg` that zune-jpeg 0.4 decodes without coefficient planes,
/// or `None`. It holds only what decoding needs, each segment exactly as
/// long as it says: the tables (DQT, DHT, DRI), Adobe's 14-byte APP14, one
/// baseline or extended sequential frame (SOF0, SOF1), and a first scan
/// holding every component (its `mcu.rs`, `all_components_in_first_scan`),
/// then the rest of the file as it is. The other APPn and COM segments are
/// left out: zune-jpeg reads some of them its own way (an APP0 of length 6
/// runs into the next segment), and a marker hidden where it lands would be
/// one this walk never saw. Anything else, fill bytes included, is `None`.
fn baseline_copy(jpeg: &[u8]) -> Option<Vec<u8>> {
    let mut copy = jpeg.get(..2)?.to_vec();
    let mut at = 2;
    let mut components = None;
    loop {
        let &[0xFF, marker, high, low] = jpeg.get(at..at + 4)? else { return None };
        let end = at + 2 + usize::from(u16::from_be_bytes([high, low]));
        let body = jpeg.get(at + 4..end)?;
        let keep = match marker {
            0xDB => tables_fill(body, |head, _| match head >> 4 {
                0 => Some(64),
                1 => Some(128),
                _ => None,
            }),
            0xC4 => tables_fill(body, |_, rest| {
                let counts = rest.get(..16)?;
                Some(16 + counts.iter().map(|&n| usize::from(n)).sum::<usize>())
            }),
            0xDD => body.len() == 2,
            0xEE => body.len() == 12 && body.starts_with(b"Adobe"),
            0xC0 | 0xC1 if components.is_none() => {
                components = body.get(5).copied();
                components.is_some_and(|n| n > 0 && body.len() == 6 + 3 * usize::from(n))
            }
            0xDA => {
                let scan = body.first().copied();
                let whole = body.len() == 4 + 2 * usize::from(scan?);
                return (whole && components.is_some() && scan == components).then(|| {
                    copy.extend_from_slice(&jpeg[at..]);
                    copy
                });
            }
            0xE0..=0xEF | 0xFE => {
                at = end;
                continue;
            }
            _ => return None,
        };
        if !keep {
            return None;
        }
        copy.extend_from_slice(&jpeg[at..end]);
        at = end;
    }
}

/// Whether `body` is a run of tables, each one byte of `head` and then the
/// `size(head, rest)` bytes that follow it, with nothing left over.
fn tables_fill(mut body: &[u8], size: impl Fn(u8, &[u8]) -> Option<usize>) -> bool {
    while let Some((&head, rest)) = body.split_first() {
        match size(head, rest).and_then(|n| rest.get(n..)) {
            Some(after) => body = after,
            None => return false,
        }
    }
    true
}

/// Full `Photo` row, including `original_bytes` — used internally for
/// export (001 FR-018) and thumbnail regeneration, never serialized directly to
/// the frontend (see `PhotoSummary`, which omits the potentially large
/// original bytes).
#[derive(Debug, Clone)]
pub struct Photo {
    pub id: i64,
    pub owner: RecordRef,
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
            owner: RecordRef::from_owner_row(row)?,
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
    pub owner: RecordRef,
    pub original_filename: String,
    pub mime_type: String,
    pub thumbnail_bytes: Vec<u8>,
    pub sort_order: i64,
    pub created_at: String,
}

impl PhotoSummary {
    /// The columns [`PhotoSummary::from_row`] reads: every one but the
    /// original, so a list or a thumbnail never reads it, and a photo larger
    /// than any connection reads (one stored before 001 FR-007's limit,
    /// amended 2026-11-21) still lists. A thumbnail over
    /// [`MAX_THUMBNAIL_BYTES`] reads as empty, unread: SQLite has its length
    /// without its bytes.
    pub const COLUMNS: &str = "id, firearm_id, accessory_id, original_filename, mime_type, \
        CASE WHEN length(thumbnail_bytes) <= 1048576 THEN thumbnail_bytes ELSE X'' END AS thumbnail_bytes, \
        sort_order, created_at";

    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            owner: RecordRef::from_owner_row(row)?,
            original_filename: row.get("original_filename")?,
            mime_type: row.get("mime_type")?,
            thumbnail_bytes: row.get("thumbnail_bytes")?,
            sort_order: row.get("sort_order")?,
            created_at: row.get("created_at")?,
        })
    }
}

impl From<Photo> for PhotoSummary {
    fn from(photo: Photo) -> Self {
        Self {
            id: photo.id,
            owner: photo.owner,
            original_filename: photo.original_filename,
            mime_type: photo.mime_type,
            thumbnail_bytes: photo.thumbnail_bytes,
            sort_order: photo.sort_order,
            created_at: photo.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::baseline_copy;

    const SOI: [u8; 2] = [0xFF, 0xD8];
    /// An APP0 of length 6, which zune-jpeg 0.4 reads one byte past.
    const APP0: [u8; 8] = [0xFF, 0xE0, 0x00, 0x06, b'J', b'F', b'I', b'F'];
    /// A COM whose body is a scan of one component, as one hides a marker.
    const HIDDEN_SCAN: [u8; 14] =
        [0xFF, 0xFE, 0x00, 0x0C, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00];
    /// A DHT of one table with one 1-bit code.
    const DHT: [u8; 22] =
        [0xFF, 0xC4, 0x00, 0x14, 0x00, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05];

    fn frame(marker: u8) -> Vec<u8> {
        vec![0xFF, marker, 0x00, 0x11, 8, 0, 16, 0, 16, 3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0]
    }

    fn scan(components: u8) -> Vec<u8> {
        let mut scan = vec![0xFF, 0xDA, 0x00, 6 + 2 * components, components];
        for component in 1..=components {
            scan.extend([component, 0]);
        }
        scan.extend([0, 0x3F, 0, 0x12, 0x34, 0xFF, 0xD9]);
        scan
    }

    fn jpeg(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    #[test]
    fn a_single_scan_baseline_jpeg_is_copied_with_only_what_decoding_needs() {
        let (sof, sos) = (frame(0xC0), scan(3));
        let copy = baseline_copy(&jpeg(&[&SOI, &APP0, &HIDDEN_SCAN, &DHT, &sof, &sos]));
        assert_eq!(copy, Some(jpeg(&[&SOI, &DHT, &sof, &sos])), "the APP0 and COM are left out");
        assert!(baseline_copy(&jpeg(&[&SOI, &frame(0xC1), &sos])).is_some());
    }

    #[test]
    fn anything_else_has_no_copy() {
        let (sof, sos) = (frame(0xC0), scan(3));
        let none =
            |parts: &[&[u8]], why: &str| assert_eq!(baseline_copy(&jpeg(parts)), None, "{why}");
        none(&[&SOI, &frame(0xC2), &sos], "progressive");
        none(&[&SOI, &frame(0xCA), &sos], "arithmetic progressive");
        none(&[&SOI, &sof, &scan(1)], "a first scan of one component");
        none(&[&SOI, &sof, &sof, &sos], "a second frame");
        none(&[&SOI, &[0xFF, 0x00], &sof, &sos], "a stuffed FF 00 before a marker");
        none(&[&SOI, &[0xFF], &sof, &sos], "a fill byte");
        none(&[&SOI, &[0xFF, 0xD0], &sof, &sos], "a marker without a length");
        none(&[&SOI, &[0x00], &sof, &sos], "a byte between segments");
        none(&[&SOI, &sof], "no scan");
        let mut long = DHT.to_vec();
        long[3] += 1;
        long.push(0);
        none(&[&SOI, &long, &sof, &sos], "a DHT longer than its table");
        none(&[&SOI, &[0xFF, 0xDB, 0x00, 0x03, 0x20], &sof, &sos], "a DQT of an unknown precision");
        none(
            &[&SOI, &[0xFF, 0xEE, 0x00, 0x06, b'A', b'd', b'o', b'b'], &sof, &sos],
            "a short APP14",
        );
    }
}
