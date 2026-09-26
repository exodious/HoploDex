use rusqlite::{named_params, Connection, OptionalExtension};
use tauri::State;

use crate::commands::firearms::DeleteResult;
use crate::commands::CommandError;
use crate::models::photo::{generate_thumbnail, validate_photo_mime_type, Photo, PhotoSummary};
use crate::services::attachments::read_attachment_file;
use crate::session::Session;

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
        .map_err(CommandError::from_db)?
        .ok_or_else(|| CommandError::not_found("No photo was found with that id."))
    }

    pub fn list_photos(conn: &Connection, firearm_id: i64) -> Result<Vec<Photo>, CommandError> {
        let mut stmt = conn
            .prepare("SELECT * FROM photos WHERE firearm_id = :firearm_id ORDER BY sort_order")
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map(named_params! { ":firearm_id": firearm_id }, Photo::from_row)
            .map_err(CommandError::from_db)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CommandError::from_db)
    }

    /// The first photo added to a firearm automatically becomes its
    /// thumbnail (FR-008, US4 Acceptance Scenario 1).
    pub fn add_photo(
        conn: &Connection,
        firearm_id: i64,
        file_bytes: &[u8],
        original_filename: &str,
        mime_type: &str,
    ) -> Result<Photo, CommandError> {
        validate_photo_mime_type(mime_type)?;
        let thumbnail_bytes = generate_thumbnail(file_bytes)?;

        let next_sort_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM photos WHERE firearm_id = :firearm_id",
                named_params! { ":firearm_id": firearm_id },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;

        conn.execute(
            "INSERT INTO photos (
                firearm_id, original_bytes, original_filename, mime_type,
                thumbnail_bytes, sort_order, created_at
            ) VALUES (
                :firearm_id, :original_bytes, :original_filename, :mime_type,
                :thumbnail_bytes, :sort_order, datetime('now')
            )",
            named_params! {
                ":firearm_id": firearm_id,
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
                "SELECT thumbnail_photo_id IS NOT NULL FROM firearms WHERE id = :firearm_id",
                named_params! { ":firearm_id": firearm_id },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;
        if !has_thumbnail {
            conn.execute(
                "UPDATE firearms SET thumbnail_photo_id = :photo_id WHERE id = :firearm_id",
                named_params! { ":photo_id": photo_id, ":firearm_id": firearm_id },
            )
            .map_err(CommandError::from_db)?;
        }

        get_photo(conn, photo_id)
    }

    /// `add_photo` for a file already on disk — a photo dropped onto the
    /// window arrives as a path, not as bytes.
    pub fn add_photo_from_path(
        conn: &Connection,
        firearm_id: i64,
        path: &std::path::Path,
    ) -> Result<Photo, CommandError> {
        let file = read_attachment_file(path)?;
        add_photo(conn, firearm_id, &file.bytes, &file.filename, file.mime_type)
    }

    pub fn set_thumbnail_photo(
        conn: &Connection,
        firearm_id: i64,
        photo_id: i64,
    ) -> Result<crate::models::firearm::Firearm, CommandError> {
        let owner: i64 = conn
            .query_row(
                "SELECT firearm_id FROM photos WHERE id = :photo_id",
                named_params! { ":photo_id": photo_id },
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::from_db)?
            .ok_or_else(|| CommandError::not_found("No photo was found with that id."))?;
        if owner != firearm_id {
            return Err(CommandError::not_found("That photo does not belong to this firearm."));
        }

        conn.execute(
            "UPDATE firearms SET thumbnail_photo_id = :photo_id WHERE id = :firearm_id",
            named_params! { ":photo_id": photo_id, ":firearm_id": firearm_id },
        )
        .map_err(CommandError::from_db)?;

        crate::commands::firearms::ops::get_firearm(conn, firearm_id)
    }

    /// If the deleted photo was the thumbnail, falls back to the
    /// next-oldest remaining photo, or `null` (generic thumbnail) if none
    /// remain (contracts/tauri-commands.md's `delete_photo`).
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

        let (firearm_id, was_thumbnail): (i64, bool) = conn
            .query_row(
                "SELECT p.firearm_id, f.thumbnail_photo_id = p.id
                 FROM photos p JOIN firearms f ON f.id = p.firearm_id
                 WHERE p.id = :photo_id",
                named_params! { ":photo_id": photo_id },
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::from_db)?
            .ok_or_else(|| CommandError::not_found("No photo was found with that id."))?;

        conn.execute(
            "DELETE FROM photos WHERE id = :photo_id",
            named_params! { ":photo_id": photo_id },
        )
        .map_err(CommandError::from_db)?;

        if was_thumbnail {
            let next_oldest: Option<i64> = conn
                .query_row(
                    "SELECT id FROM photos WHERE firearm_id = :firearm_id ORDER BY sort_order LIMIT 1",
                    named_params! { ":firearm_id": firearm_id },
                    |row| row.get(0),
                )
                .optional()
                .map_err(CommandError::from_db)?;
            conn.execute(
                "UPDATE firearms SET thumbnail_photo_id = :photo_id WHERE id = :firearm_id",
                named_params! { ":photo_id": next_oldest, ":firearm_id": firearm_id },
            )
            .map_err(CommandError::from_db)?;
        }

        crate::db::reclaim_freed_space(conn);
        Ok(DeleteResult { deleted: true })
    }
}

#[tauri::command]
pub async fn list_photos(
    firearm_id: i64,
    session: State<'_, Session>,
) -> Result<Vec<PhotoSummary>, CommandError> {
    session.read(|conn| {
        Ok(ops::list_photos(conn, firearm_id)?.into_iter().map(PhotoSummary::from).collect())
    })
}

#[tauri::command]
pub async fn add_photo(
    firearm_id: i64,
    file_bytes: Vec<u8>,
    original_filename: String,
    mime_type: String,
    session: State<'_, Session>,
) -> Result<PhotoSummary, CommandError> {
    session.write(|conn| {
        ops::add_photo(conn, firearm_id, &file_bytes, &original_filename, &mime_type)
            .map(Into::into)
    })
}

/// Adds a photo from a file on disk: what a drop onto the window delivers.
#[tauri::command]
pub async fn add_photo_from_path(
    firearm_id: i64,
    path: String,
    session: State<'_, Session>,
) -> Result<PhotoSummary, CommandError> {
    session.write(|conn| {
        ops::add_photo_from_path(conn, firearm_id, std::path::Path::new(&path)).map(Into::into)
    })
}

/// Just the small cached thumbnail bytes for one photo — lets browse views
/// (BrowseList/BrowseTiles) render a firearm's designated thumbnail
/// without fetching every photo attached to it.
#[tauri::command]
pub async fn get_photo_thumbnail(
    photo_id: i64,
    session: State<'_, Session>,
) -> Result<Vec<u8>, CommandError> {
    session.read(|conn| Ok(ops::get_photo(conn, photo_id)?.thumbnail_bytes))
}

/// The full-resolution original bytes for one photo, returned as a raw
/// binary IPC response (an `ArrayBuffer` in the frontend, not a JSON number
/// array) so multi-megabyte photos transfer quickly — for the record view's
/// photo viewer, where condition details need to be legible.
#[tauri::command]
pub async fn get_photo_original(
    photo_id: i64,
    session: State<'_, Session>,
) -> Result<tauri::ipc::Response, CommandError> {
    session
        .read(|conn| Ok(tauri::ipc::Response::new(ops::get_photo(conn, photo_id)?.original_bytes)))
}

#[tauri::command]
pub async fn set_thumbnail_photo(
    firearm_id: i64,
    photo_id: i64,
    session: State<'_, Session>,
) -> Result<crate::models::firearm::Firearm, CommandError> {
    session.write(|conn| ops::set_thumbnail_photo(conn, firearm_id, photo_id))
}

#[tauri::command]
pub async fn delete_photo(
    photo_id: i64,
    confirmed: bool,
    session: State<'_, Session>,
) -> Result<DeleteResult, CommandError> {
    session.write(|conn| ops::delete_photo(conn, photo_id, confirmed))
}
