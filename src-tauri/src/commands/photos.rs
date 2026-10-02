use rusqlite::{Connection, OptionalExtension, named_params};
use serde::Serialize;
use tauri::State;

use crate::commands::CommandError;
use crate::commands::firearms::DeleteResult;
use crate::models::photo::{Photo, PhotoSummary, generate_thumbnail, validate_photo_mime_type};
use crate::models::record::RecordRef;
use crate::services::attachments::read_attachment_file;
use crate::session::Session;

/// What `set_thumbnail_photo` returns (contracts/tauri-commands.md "Photos
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
        .map_err(CommandError::from_db)?
        .ok_or_else(|| CommandError::not_found("No photo was found with that id."))
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

    pub fn list_photos(conn: &Connection, owner: RecordRef) -> Result<Vec<Photo>, CommandError> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT * FROM photos WHERE {} = :id ORDER BY sort_order",
                owner.owner_column()
            ))
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map(named_params! { ":id": owner.id() }, Photo::from_row)
            .map_err(CommandError::from_db)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CommandError::from_db)
    }

    /// The first photo added to a record automatically becomes its
    /// thumbnail (FR-008, US4 Acceptance Scenario 1; 006 FR-007a).
    pub fn add_photo(
        conn: &Connection,
        owner: RecordRef,
        file_bytes: &[u8],
        original_filename: &str,
        mime_type: &str,
    ) -> Result<Photo, CommandError> {
        validate_photo_mime_type(mime_type)?;
        let thumbnail_bytes = generate_thumbnail(file_bytes)?;
        require_owner(conn, owner)?;

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
                firearm_id, accessory_id, original_bytes, original_filename, mime_type,
                thumbnail_bytes, sort_order, created_at
            ) VALUES (
                :firearm_id, :accessory_id, :original_bytes, :original_filename, :mime_type,
                :thumbnail_bytes, :sort_order, datetime('now')
            )",
            named_params! {
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
    pub fn add_photo_from_path(
        conn: &Connection,
        owner: RecordRef,
        path: &std::path::Path,
    ) -> Result<Photo, CommandError> {
        let file = read_attachment_file(path)?;
        add_photo(conn, owner, &file.bytes, &file.filename, file.mime_type)
    }

    /// Makes `photo_id` the owner's thumbnail; a photo of a different owner
    /// is `NOT_FOUND`.
    pub fn set_thumbnail_photo(
        conn: &Connection,
        owner: RecordRef,
        photo_id: i64,
    ) -> Result<ThumbnailChoice, CommandError> {
        let photo = get_photo(conn, photo_id)?;
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

        let owner = get_photo(conn, photo_id)?.owner;
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
    session: State<'_, Session>,
) -> Result<Vec<PhotoSummary>, CommandError> {
    session.read(|conn| {
        Ok(ops::list_photos(conn, owner)?.into_iter().map(PhotoSummary::from).collect())
    })
}

#[tauri::command]
pub async fn add_photo(
    owner: RecordRef,
    file_bytes: Vec<u8>,
    original_filename: String,
    mime_type: String,
    session: State<'_, Session>,
) -> Result<PhotoSummary, CommandError> {
    session.write(|conn| {
        ops::add_photo(conn, owner, &file_bytes, &original_filename, &mime_type).map(Into::into)
    })
}

/// Adds a photo from a file on disk: what a drop onto the window delivers.
#[tauri::command]
pub async fn add_photo_from_path(
    owner: RecordRef,
    path: String,
    session: State<'_, Session>,
) -> Result<PhotoSummary, CommandError> {
    session.write(|conn| {
        ops::add_photo_from_path(conn, owner, std::path::Path::new(&path)).map(Into::into)
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
    owner: RecordRef,
    photo_id: i64,
    session: State<'_, Session>,
) -> Result<ThumbnailChoice, CommandError> {
    session.write(|conn| ops::set_thumbnail_photo(conn, owner, photo_id))
}

#[tauri::command]
pub async fn delete_photo(
    photo_id: i64,
    confirmed: bool,
    session: State<'_, Session>,
) -> Result<DeleteResult, CommandError> {
    session.write(|conn| ops::delete_photo(conn, photo_id, confirmed))
}
