use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{named_params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::commands::firearms::{ops as firearm_ops, ListFirearmsInput};
use crate::commands::CommandError;
use crate::db::DbHandle;
use crate::models::firearm::{
    validate_firearm_input, CoverageKind, DispositionType, FirearmInput, FirearmStatus,
};
use crate::services::spreadsheet::{
    cents_to_decimal_string, parse_decimal_to_cents, read_spreadsheet, write_spreadsheet,
    FirearmExportRow, RawImportRow, SpreadsheetFormat,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub spreadsheet_path: PathBuf,
    pub photos_folder_path: PathBuf,
    pub exported_firearm_count: usize,
    pub exported_photo_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowError {
    pub row: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConflict {
    pub conflict_id: String,
    pub row: usize,
    pub existing_firearm_id: i64,
    pub make: String,
    pub model: String,
    pub serial_number: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub session_id: String,
    pub imported_count: usize,
    pub updated_count: usize,
    pub skipped_count: usize,
    pub row_errors: Vec<RowError>,
    pub conflicts: Vec<ImportConflict>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolution {
    pub conflict_id: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveResult {
    pub resolved_count: usize,
}

struct PendingConflict {
    conflict_id: String,
    existing_firearm_id: i64,
    new_input: FirearmInput,
}

/// Holds each in-progress import's matched-but-unresolved rows between the
/// initial `import_collection` call and the follow-up
/// `resolve_import_conflicts` call — Tauri-managed state (`app.manage`),
/// analogous to `DbHandle`.
#[derive(Default)]
pub struct ImportSessionStore {
    pending: Mutex<HashMap<String, Vec<PendingConflict>>>,
}

impl ImportSessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Pure, `Connection`-based business logic — mirrors `commands::firearms::ops`
/// (constitution: no mocks, integration tests call these directly against a
/// real temporary SQLCipher database).
pub mod ops {
    use super::*;

    /// Every firearm id, active or disposed — export's `scope: "all"` is a
    /// full backup, unlike `list_firearms`'s default of active-only.
    pub fn all_firearm_ids(conn: &Connection) -> Result<Vec<i64>, CommandError> {
        let mut stmt = conn.prepare("SELECT id FROM firearms").map_err(CommandError::from_db)?;
        let rows = stmt.query_map([], |row| row.get(0)).map_err(CommandError::from_db)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CommandError::from_db)
    }

    pub fn export_collection(
        conn: &Connection,
        destination_folder: &Path,
        base_name: &str,
        format: SpreadsheetFormat,
        firearm_ids: &[i64],
        on_progress: &mut dyn FnMut(usize, usize),
    ) -> Result<ExportResult, CommandError> {
        let photos_folder_path = destination_folder.join(format!("{base_name}_photos"));
        std::fs::create_dir_all(&photos_folder_path).map_err(|e| {
            CommandError::new("INTERNAL_ERROR", format!("Could not create the photos folder: {e}"))
        })?;
        let spreadsheet_path =
            destination_folder.join(format!("{base_name}.{}", format.extension()));

        let total = firearm_ids.len();
        let mut rows = Vec::with_capacity(total);
        let mut exported_photo_count = 0usize;

        for (index, &firearm_id) in firearm_ids.iter().enumerate() {
            let firearm = firearm_ops::get_firearm(conn, firearm_id)?;
            let firearm_type_name: String = conn
                .query_row(
                    "SELECT name FROM firearm_types WHERE id = :id",
                    named_params! { ":id": firearm.firearm_type_id },
                    |row| row.get(0),
                )
                .map_err(CommandError::from_db)?;
            let insurance_policy_name: Option<String> = match firearm.insurance_policy_id {
                Some(policy_id) => conn
                    .query_row(
                        "SELECT name FROM insurance_policies WHERE id = :id",
                        named_params! { ":id": policy_id },
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(CommandError::from_db)?,
                None => None,
            };

            let photos = crate::commands::photos::ops::list_photos(conn, firearm_id)?;
            let mut photo_filenames = Vec::with_capacity(photos.len());
            for photo in &photos {
                let dest_name = format!("{firearm_id}_{}", photo.original_filename);
                std::fs::write(photos_folder_path.join(&dest_name), &photo.original_bytes)
                    .map_err(|e| {
                        CommandError::new(
                            "INTERNAL_ERROR",
                            format!("Could not write photo file: {e}"),
                        )
                    })?;
                photo_filenames.push(dest_name);
                exported_photo_count += 1;
            }

            rows.push(FirearmExportRow {
                make: firearm.make,
                model: firearm.model,
                serial_number: firearm.serial_number.unwrap_or_default(),
                no_serial_attested: if firearm.no_serial_attested { "TRUE" } else { "FALSE" }
                    .to_string(),
                caliber: firearm.caliber,
                firearm_type: firearm_type_name,
                notes: firearm.notes.unwrap_or_default(),
                accessories: firearm.accessories.unwrap_or_default(),
                status: firearm.status.as_str().to_string(),
                estimated_value: cents_to_decimal_string(firearm.estimated_value),
                acquisition_source: firearm.acquisition_source.unwrap_or_default(),
                acquisition_date: firearm.acquisition_date.unwrap_or_default(),
                acquisition_price: cents_to_decimal_string(firearm.acquisition_price),
                disposition_type: firearm
                    .disposition_type
                    .map(|d| d.as_str().to_string())
                    .unwrap_or_default(),
                disposition_recipient: firearm.disposition_recipient.unwrap_or_default(),
                disposition_date: firearm.disposition_date.unwrap_or_default(),
                disposition_price: cents_to_decimal_string(firearm.disposition_price),
                insurance_policy_name: insurance_policy_name.unwrap_or_default(),
                coverage_kind: firearm
                    .coverage_kind
                    .map(|c| c.as_str().to_string())
                    .unwrap_or_default(),
                scheduled_coverage_amount: cents_to_decimal_string(
                    firearm.scheduled_coverage_amount,
                ),
                photo_filenames: photo_filenames.join(";"),
            });

            on_progress(index + 1, total);
        }

        write_spreadsheet(&spreadsheet_path, format, &rows)?;

        Ok(ExportResult {
            spreadsheet_path,
            photos_folder_path,
            exported_firearm_count: firearm_ids.len(),
            exported_photo_count,
        })
    }

    fn parse_bool(value: &Option<String>) -> bool {
        matches!(value.as_deref().map(str::to_uppercase).as_deref(), Some("TRUE"))
    }

    fn parse_disposition_type(value: &str) -> Result<DispositionType, String> {
        match value.to_lowercase().as_str() {
            "sold" => Ok(DispositionType::Sold),
            "traded" => Ok(DispositionType::Traded),
            "gifted" => Ok(DispositionType::Gifted),
            "destroyed" => Ok(DispositionType::Destroyed),
            "lost_stolen" => Ok(DispositionType::LostStolen),
            other => Err(format!("Unknown disposition type: {other}")),
        }
    }

    fn parse_coverage_kind(value: &str) -> Result<CoverageKind, String> {
        match value.to_lowercase().as_str() {
            "individually_scheduled" => Ok(CoverageKind::IndividuallyScheduled),
            "blanket" => Ok(CoverageKind::Blanket),
            other => Err(format!("Unknown coverage kind: {other}")),
        }
    }

    /// Parses and validates one raw spreadsheet row into a `FirearmInput`,
    /// resolving `firearm_type`/`insurance_policy_name` by lookup — a
    /// human-readable `Err` message per FR-020, never a panic/abort of the
    /// whole import.
    fn parse_row(conn: &Connection, raw: &RawImportRow) -> Result<FirearmInput, String> {
        let make = raw.make.clone().ok_or("Missing required field: make")?;
        let model = raw.model.clone().ok_or("Missing required field: model")?;
        let caliber = raw.caliber.clone().ok_or("Missing required field: caliber")?;
        let type_name = raw.firearm_type.clone().ok_or("Missing required field: firearm_type")?;

        let firearm_type_id: i64 = conn
            .query_row(
                "SELECT id FROM firearm_types WHERE name = :name COLLATE NOCASE",
                named_params! { ":name": type_name },
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("Database error resolving firearm type: {e}"))?
            .ok_or_else(|| format!("Unknown firearm type: {type_name}"))?;

        let status = match raw.status.as_deref().map(str::to_lowercase).as_deref() {
            None | Some("") | Some("active") => FirearmStatus::Active,
            Some("disposed") => FirearmStatus::Disposed,
            Some(other) => return Err(format!("Unknown status: {other}")),
        };

        let disposition_type =
            raw.disposition_type.as_deref().map(parse_disposition_type).transpose()?;
        let coverage_kind = raw.coverage_kind.as_deref().map(parse_coverage_kind).transpose()?;

        let insurance_policy_id = match &raw.insurance_policy_name {
            None => None,
            Some(name) => Some(
                conn.query_row(
                    "SELECT id FROM insurance_policies WHERE name = :name COLLATE NOCASE",
                    named_params! { ":name": name },
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| format!("Database error resolving insurance policy: {e}"))?
                .ok_or_else(|| format!("Unknown insurance policy: {name}"))?,
            ),
        };

        let input = FirearmInput {
            make,
            model,
            serial_number: raw.serial_number.clone(),
            no_serial_attested: parse_bool(&raw.no_serial_attested),
            caliber,
            firearm_type_id,
            notes: raw.notes.clone(),
            accessories: raw.accessories.clone(),
            status,
            estimated_value: parse_decimal_to_cents(&raw.estimated_value),
            acquisition_source: raw.acquisition_source.clone(),
            acquisition_date: raw.acquisition_date.clone(),
            acquisition_price: parse_decimal_to_cents(&raw.acquisition_price),
            disposition_type,
            disposition_recipient: raw.disposition_recipient.clone(),
            disposition_date: raw.disposition_date.clone(),
            disposition_price: parse_decimal_to_cents(&raw.disposition_price),
            insurance_policy_id,
            coverage_kind,
            scheduled_coverage_amount: parse_decimal_to_cents(&raw.scheduled_coverage_amount),
        };

        validate_firearm_input(&input).map_err(|e| e.message)?;
        Ok(input)
    }

    /// Each row is validated independently; a failing row is reported in
    /// `rowErrors` and does not block other rows (FR-020). A row matching
    /// an existing `(make, model, serial_number)` key becomes a pending
    /// conflict (stored in `store`) rather than being silently applied.
    pub fn import_collection(
        conn: &Connection,
        file_path: &Path,
        format: SpreadsheetFormat,
        store: &ImportSessionStore,
        on_progress: &mut dyn FnMut(usize, usize),
    ) -> Result<ImportResult, CommandError> {
        let raw_rows = read_spreadsheet(file_path, format)?;
        let total = raw_rows.len();
        let session_id = format!("import-{}", chrono::Utc::now().timestamp_micros());

        let mut imported_count = 0;
        let mut row_errors = Vec::new();
        let mut conflicts = Vec::new();
        let mut pending = Vec::new();

        for (index, raw) in raw_rows.iter().enumerate() {
            let row_number = index + 1;
            match parse_row(conn, raw) {
                Err(message) => row_errors.push(RowError { row: row_number, message }),
                Ok(input) => {
                    let existing = crate::services::import_matching::find_match(
                        conn,
                        &input.make,
                        &input.model,
                        input.serial_number.as_deref(),
                        input.no_serial_attested,
                    )?;
                    match existing {
                        Some(existing_firearm_id) => {
                            let conflict_id = format!("{session_id}-row-{row_number}");
                            conflicts.push(ImportConflict {
                                conflict_id: conflict_id.clone(),
                                row: row_number,
                                existing_firearm_id,
                                make: input.make.clone(),
                                model: input.model.clone(),
                                serial_number: input.serial_number.clone(),
                            });
                            pending.push(PendingConflict {
                                conflict_id,
                                existing_firearm_id,
                                new_input: input,
                            });
                        }
                        None => {
                            firearm_ops::create_firearm(conn, &input)?;
                            imported_count += 1;
                        }
                    }
                }
            }
            on_progress(row_number, total);
        }

        if !pending.is_empty() {
            store
                .pending
                .lock()
                .expect("import session mutex poisoned")
                .insert(session_id.clone(), pending);
        }

        Ok(ImportResult {
            session_id,
            imported_count,
            updated_count: 0,
            skipped_count: 0,
            row_errors,
            conflicts,
        })
    }

    /// Applies each conflict's resolution: `overwrite` updates the existing
    /// record, `duplicate` inserts the imported row as a new record
    /// alongside it, `skip` (the default, including any conflict not
    /// covered by `resolutions` or `apply_to_remaining`) leaves the
    /// existing record untouched.
    pub fn resolve_import_conflicts(
        conn: &Connection,
        store: &ImportSessionStore,
        session_id: &str,
        resolutions: &[ConflictResolution],
        apply_to_remaining: Option<&str>,
    ) -> Result<ResolveResult, CommandError> {
        let pending = store
            .pending
            .lock()
            .expect("import session mutex poisoned")
            .remove(session_id)
            .unwrap_or_default();

        let explicit: HashMap<&str, &str> =
            resolutions.iter().map(|r| (r.conflict_id.as_str(), r.action.as_str())).collect();

        let mut resolved_count = 0;
        for conflict in pending {
            let action = explicit
                .get(conflict.conflict_id.as_str())
                .copied()
                .or(apply_to_remaining)
                .unwrap_or("skip");
            match action {
                "overwrite" => {
                    firearm_ops::update_firearm(
                        conn,
                        conflict.existing_firearm_id,
                        &conflict.new_input,
                    )?;
                }
                "duplicate" => {
                    firearm_ops::create_firearm(conn, &conflict.new_input)?;
                }
                _ => {} // "skip", or an unrecognized action, leaves the existing record untouched
            }
            resolved_count += 1;
        }

        Ok(ResolveResult { resolved_count })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCollectionInput {
    pub format: String,
    pub destination_folder: String,
    #[serde(default = "default_scope")]
    pub scope: String,
    pub filter: Option<ListFirearmsInput>,
}

fn default_scope() -> String {
    "all".to_string()
}

fn parse_format(format: &str) -> Result<SpreadsheetFormat, CommandError> {
    match format {
        "csv" => Ok(SpreadsheetFormat::Csv),
        "xlsx" => Ok(SpreadsheetFormat::Xlsx),
        other => Err(CommandError::new("VALIDATION_ERROR", format!("Unsupported format: {other}"))),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct ProgressPayload {
    processed: usize,
    total: usize,
}

#[tauri::command]
pub async fn export_collection(
    input: ExportCollectionInput,
    app: tauri::AppHandle,
    state: State<'_, DbHandle>,
) -> Result<ExportResult, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    let format = parse_format(&input.format)?;
    let firearm_ids = if input.scope == "filtered" {
        let filter = input.filter.unwrap_or_default();
        let listing = firearm_ops::list_firearms(&conn, &filter)?;
        listing.groups.into_iter().flat_map(|g| g.firearms).map(|f| f.id).collect()
    } else {
        ops::all_firearm_ids(&conn)?
    };

    let base_name = format!("hoplodex-export-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
    ops::export_collection(
        &conn,
        Path::new(&input.destination_folder),
        &base_name,
        format,
        &firearm_ids,
        &mut |processed, total| {
            let _ = app.emit("export_collection:progress", ProgressPayload { processed, total });
        },
    )
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCollectionInput {
    pub file_path: String,
    pub format: String,
}

#[tauri::command]
pub async fn import_collection(
    input: ImportCollectionInput,
    app: tauri::AppHandle,
    state: State<'_, DbHandle>,
    session_store: State<'_, ImportSessionStore>,
) -> Result<ImportResult, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    let format = parse_format(&input.format)?;
    ops::import_collection(
        &conn,
        Path::new(&input.file_path),
        format,
        &session_store,
        &mut |processed, total| {
            let _ = app.emit("import_collection:progress", ProgressPayload { processed, total });
        },
    )
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveImportConflictsInput {
    pub import_session_id: String,
    pub resolutions: Vec<ConflictResolution>,
    pub apply_to_remaining: Option<String>,
}

#[tauri::command]
pub async fn resolve_import_conflicts(
    input: ResolveImportConflictsInput,
    state: State<'_, DbHandle>,
    session_store: State<'_, ImportSessionStore>,
) -> Result<ResolveResult, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::resolve_import_conflicts(
        &conn,
        &session_store,
        &input.import_session_id,
        &input.resolutions,
        input.apply_to_remaining.as_deref(),
    )
}
