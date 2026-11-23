use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, named_params};
use serde::Serialize;
use tauri::State;

use crate::commands::CommandError;
use crate::commands::firearms::DeleteResult;
use crate::models::photo::{
    MAX_THUMBNAIL_BYTES, Photo, PhotoSummary, generate_thumbnail, photo_type,
};
use crate::models::record::RecordRef;
use crate::models::rules::{MAX_PHOTO_BYTES, MAX_PHOTOS_PER_RECORD, size_label};
use crate::services::attachments::{
    read_attachment_file, require_room, require_size, require_total_room, safe_basename,
};
use crate::services::sandbox::{Need, Reach};
use crate::session::scoped::ScopedSession;
use crate::session::{Session, SessionId};

/// What `set_thumbnail_photo` returns (006 contracts/tauri-commands.md "Photos
/// and documents (amended)"): the frontend already holds the record.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailChoice {
    pub thumbnail_photo_id: i64,
}

/// Pure, `Connection`-based business logic — mirrors `commands::firearms::ops`
/// (constitution: no mocks, integration tests call these directly against a
/// real temporary SQLCipher database).
pub mod ops {
    use super::*;

    pub fn get_photo(conn: &Connection, id: i64) -> Result<Photo, CommandError> {
        conn.query_row(
            "SELECT * FROM photos WHERE id = :id",
            named_params! { ":id": id },
            Photo::from_row,
        )
        .optional()
        .map_err(|e| CommandError::from_attachment_read(e, "photo"))?
        .ok_or_else(|| CommandError::not_found("No photo was found with that id."))
    }

    /// A photo without its original, which only export and the photo viewer
    /// read.
    pub fn get_photo_summary(conn: &Connection, id: i64) -> Result<PhotoSummary, CommandError> {
        conn.query_row(
            &format!("SELECT {} FROM photos WHERE id = :id", PhotoSummary::COLUMNS),
            named_params! { ":id": id },
            PhotoSummary::from_row,
        )
        .optional()
        .map_err(CommandError::from_db)?
        .ok_or_else(|| CommandError::not_found("No photo was found with that id."))
    }

    /// A photo's thumbnail; `ATTACHMENT_TOO_LARGE` for one over
    /// [`MAX_THUMBNAIL_BYTES`] or not a BLOB, which [`PhotoSummary::COLUMNS`]
    /// leaves unread.
    pub fn get_photo_thumbnail(conn: &Connection, id: i64) -> Result<Vec<u8>, CommandError> {
        let bytes = get_photo_summary(conn, id)?.thumbnail_bytes;
        let stored_empty = || {
            conn.query_row(
                "SELECT CASE WHEN typeof(thumbnail_bytes) = 'blob' \
                 THEN length(thumbnail_bytes) = 0 ELSE 0 END FROM photos WHERE id = :id",
                named_params! { ":id": id },
                |row| row.get::<_, bool>(0),
            )
            .map_err(CommandError::from_db)
        };
        if bytes.is_empty() && !stored_empty()? {
            return Err(CommandError::attachment_too_large(
                format!(
                    "This photo's thumbnail is too large to show. HoploDex shows one of at most {}.",
                    size_label(MAX_THUMBNAIL_BYTES)
                ),
                MAX_THUMBNAIL_BYTES,
            ));
        }
        Ok(bytes)
    }

    /// Fails with `NOT_FOUND` when the owner's row doesn't exist.
    fn require_owner(conn: &Connection, owner: RecordRef) -> Result<(), CommandError> {
        let exists: bool = conn
            .query_row(
                &format!("SELECT EXISTS (SELECT 1 FROM {} WHERE id = :id)", owner.table()),
                named_params! { ":id": owner.id() },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;
        if exists {
            Ok(())
        } else {
            Err(CommandError::not_found(format!("No {} was found with that id.", owner.noun())))
        }
    }

    /// The owner's photos without their originals.
    pub fn list_photos(
        conn: &Connection,
        owner: RecordRef,
    ) -> Result<Vec<PhotoSummary>, CommandError> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM photos WHERE {} = :id ORDER BY sort_order",
                PhotoSummary::COLUMNS,
                owner.owner_column()
            ))
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map(named_params! { ":id": owner.id() }, PhotoSummary::from_row)
            .map_err(CommandError::from_db)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CommandError::from_db)
    }

    /// The first photo added to a record automatically becomes its
    /// thumbnail (001 FR-008, US4-1; 006 FR-007a). A record
    /// holds at most `MAX_PHOTOS_PER_RECORD` (001 FR-007, amended 2026-10-11).
    pub fn add_photo(
        conn: &Connection,
        owner: RecordRef,
        file_bytes: &[u8],
        original_filename: &str,
    ) -> Result<Photo, CommandError> {
        admit_photo(conn, owner, file_bytes.len() as u64)?;
        // The name comes over IPC. A name is only ever a name: no path in it
        // (issue #68; the export checks again, for rows made before this).
        let original_filename = safe_basename(original_filename, "photo");
        let mime_type = photo_type(&original_filename, file_bytes)?;
        let thumbnail_bytes = generate_thumbnail(&original_filename, file_bytes)?;

        let next_sort_order: i64 = conn
            .query_row(
                &format!(
                    "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM photos WHERE {} = :id",
                    owner.owner_column()
                ),
                named_params! { ":id": owner.id() },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;

        let (firearm_id, accessory_id) = owner.owner_columns();
        conn.execute(
            "INSERT INTO photos (
                uid, firearm_id, accessory_id, original_bytes, original_filename, mime_type,
                thumbnail_bytes, sort_order, created_at
            ) VALUES (
                :uid, :firearm_id, :accessory_id, :original_bytes, :original_filename, :mime_type,
                :thumbnail_bytes, :sort_order, datetime('now')
            )",
            named_params! {
                // 009 FR-001: the record's identifier, never sent over IPC.
                ":uid": crate::services::record_id::generate(),
                ":firearm_id": firearm_id,
                ":accessory_id": accessory_id,
                ":original_bytes": file_bytes,
                ":original_filename": original_filename,
                ":mime_type": mime_type,
                ":thumbnail_bytes": thumbnail_bytes,
                ":sort_order": next_sort_order,
            },
        )
        .map_err(CommandError::from_db)?;
        let photo_id = conn.last_insert_rowid();

        let has_thumbnail: bool = conn
            .query_row(
                &format!(
                    "SELECT thumbnail_photo_id IS NOT NULL FROM {} WHERE id = :id",
                    owner.table()
                ),
                named_params! { ":id": owner.id() },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;
        if !has_thumbnail {
            set_owner_thumbnail(conn, owner, Some(photo_id))?;
        }

        get_photo(conn, photo_id)
    }

    /// Whether `owner` takes one more photo of `size` bytes: it exists, it has
    /// room for another, and the photo is small enough on its own and with the
    /// rest. Checked before any of the photo is read or decoded.
    fn admit_photo(conn: &Connection, owner: RecordRef, size: u64) -> Result<(), CommandError> {
        require_owner(conn, owner)?;
        require_room(conn, owner, "photos", "photos", MAX_PHOTOS_PER_RECORD)?;
        require_size("photo", size, MAX_PHOTO_BYTES)?;
        require_total_room(conn, owner, size)
    }

    fn set_owner_thumbnail(
        conn: &Connection,
        owner: RecordRef,
        photo_id: Option<i64>,
    ) -> Result<(), CommandError> {
        conn.execute(
            &format!("UPDATE {} SET thumbnail_photo_id = :photo_id WHERE id = :id", owner.table()),
            named_params! { ":photo_id": photo_id, ":id": owner.id() },
        )
        .map_err(CommandError::from_db)?;
        Ok(())
    }

    /// `add_photo` for a file already on disk — a photo dropped onto the
    /// window arrives as a path, not as bytes.
    ///
    /// Inside a Flatpak a file that can't be seen and lies outside reach is
    /// refused with `LOCATION_OUT_OF_REACH` before it is read, for the file
    /// chooser and a drop alike (011 FR-007, 011 US1-5).
    pub fn add_photo_from_path(
        conn: &Connection,
        owner: RecordRef,
        path: &std::path::Path,
        reach: &Reach,
    ) -> Result<Photo, CommandError> {
        let path = reach.check(path, Need::AttachPhoto)?;
        let file = read_attachment_file(
            &path,
            |size| admit_photo(conn, owner, size),
            |name, head| photo_type(&safe_basename(name, "photo"), head).map(drop),
        )?;
        add_photo(conn, owner, &file.bytes, &file.filename)
    }

    /// [`add_photo_from_path`] for a file dropped onto the main window in
    /// session `id`: a path the user didn't drop there moments ago is refused
    /// with a fixed `VALIDATION_ERROR` before reach or the file is looked at,
    /// and a drop is added once (code scanning alert `paths/cwe-73` on
    /// `add_document_from_path`).
    pub fn add_dropped_photo(
        session: &Session,
        id: SessionId,
        owner: RecordRef,
        path: &std::path::Path,
        reach: &Reach,
    ) -> Result<Photo, CommandError> {
        let path = session.take_drop(id, path)?;
        session.write(id, |conn| add_photo_from_path(conn, owner, &path, reach))
    }

    /// Makes `photo_id` the owner's thumbnail; a photo of a different owner
    /// is `NOT_FOUND`.
    pub fn set_thumbnail_photo(
        conn: &Connection,
        owner: RecordRef,
        photo_id: i64,
    ) -> Result<ThumbnailChoice, CommandError> {
        let photo = get_photo_summary(conn, photo_id)?;
        if photo.owner != owner {
            return Err(CommandError::not_found(format!(
                "That photo does not belong to this {}.",
                owner.noun()
            )));
        }
        set_owner_thumbnail(conn, owner, Some(photo_id))?;
        Ok(ThumbnailChoice { thumbnail_photo_id: photo_id })
    }

    /// If the deleted photo was the thumbnail, falls back to the
    /// next-oldest remaining photo, or `null` (generic thumbnail) if none
    /// remain (001 contracts/tauri-commands.md's `delete_photo`).
    pub fn delete_photo(
        conn: &Connection,
        photo_id: i64,
        confirmed: bool,
    ) -> Result<DeleteResult, CommandError> {
        if !confirmed {
            return Err(CommandError::new(
                "CONFIRMATION_REQUIRED",
                "Deletion must be explicitly confirmed.",
            ));
        }

        let owner = get_photo_summary(conn, photo_id)?.owner;
        let was_thumbnail: bool = conn
            .query_row(
                &format!(
                    "SELECT thumbnail_photo_id IS :photo_id FROM {} WHERE id = :id",
                    owner.table()
                ),
                named_params! { ":photo_id": photo_id, ":id": owner.id() },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;

        conn.execute(
            "DELETE FROM photos WHERE id = :photo_id",
            named_params! { ":photo_id": photo_id },
        )
        .map_err(CommandError::from_db)?;

        if was_thumbnail {
            let next_oldest: Option<i64> = conn
                .query_row(
                    &format!(
                        "SELECT id FROM photos WHERE {} = :id ORDER BY sort_order LIMIT 1",
                        owner.owner_column()
                    ),
                    named_params! { ":id": owner.id() },
                    |row| row.get(0),
                )
                .optional()
                .map_err(CommandError::from_db)?;
            set_owner_thumbnail(conn, owner, next_oldest)?;
        }

        crate::db::reclaim_freed_space(conn);
        Ok(DeleteResult { deleted: true })
    }
}

#[tauri::command]
pub async fn list_photos(
    owner: RecordRef,
    session: ScopedSession,
) -> Result<Vec<PhotoSummary>, CommandError> {
    session.read(|conn| ops::list_photos(conn, owner))
}

#[tauri::command]
pub async fn add_photo(
    owner: RecordRef,
    file_bytes: Vec<u8>,
    original_filename: String,
    session: ScopedSession,
) -> Result<PhotoSummary, CommandError> {
    session
        .write(|conn| ops::add_photo(conn, owner, &file_bytes, &original_filename).map(Into::into))
}

/// Adds a photo dropped onto the window, by the path the drop delivered; only
/// a file the user dropped (`ops::add_dropped_photo`).
#[tauri::command]
pub async fn add_photo_from_path(
    owner: RecordRef,
    path: String,
    session: ScopedSession,
    reach: State<'_, Arc<Reach>>,
) -> Result<PhotoSummary, CommandError> {
    ops::add_dropped_photo(&session.session, session.id, owner, std::path::Path::new(&path), &reach)
        .map(Into::into)
}

/// Just the small cached thumbnail bytes for one photo — lets browse views
/// (BrowseList/BrowseTiles) render a firearm's designated thumbnail
/// without fetching every photo attached to it. Raw binary IPC, as
/// `get_photo_original`, not a JSON number array of about four bytes a byte.
#[tauri::command]
pub async fn get_photo_thumbnail(
    photo_id: i64,
    session: ScopedSession,
) -> Result<tauri::ipc::Response, CommandError> {
    session.read(|conn| Ok(tauri::ipc::Response::new(ops::get_photo_thumbnail(conn, photo_id)?)))
}

/// The full-resolution original bytes for one photo, returned as a raw
/// binary IPC response (an `ArrayBuffer` in the frontend, not a JSON number
/// array) so multi-megabyte photos transfer quickly — for the record view's
/// photo viewer, where condition details need to be legible.
#[tauri::command]
pub async fn get_photo_original(
    photo_id: i64,
    session: ScopedSession,
) -> Result<tauri::ipc::Response, CommandError> {
    session
        .read(|conn| Ok(tauri::ipc::Response::new(ops::get_photo(conn, photo_id)?.original_bytes)))
}

#[tauri::command]
pub async fn set_thumbnail_photo(
    owner: RecordRef,
    photo_id: i64,
    session: ScopedSession,
) -> Result<ThumbnailChoice, CommandError> {
    session.write(|conn| ops::set_thumbnail_photo(conn, owner, photo_id))
}

#[tauri::command]
pub async fn delete_photo(
    photo_id: i64,
    confirmed: bool,
    session: ScopedSession,
) -> Result<DeleteResult, CommandError> {
    session.write(|conn| ops::delete_photo(conn, photo_id, confirmed))
}
