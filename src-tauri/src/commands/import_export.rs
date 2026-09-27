use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, named_params};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::commands::CommandError;
use crate::commands::firearms::{ListFirearmsInput, ops as firearm_ops};
use crate::models::database::OperationKind;
use crate::models::firearm::{
    Condition, DispositionType, FirearmInput, FirearmStatus, Origin, validate_firearm_input,
};
use crate::services::spreadsheet::{
    FirearmExportRow, RawImportRow, SpreadsheetFormat, dollars_to_string, parse_scaled_decimal,
    parse_whole_dollars, read_spreadsheet, scaled_to_string, write_spreadsheet,
};
use crate::session::Session;

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
    /// Whether "create a duplicate" may be offered: false where FR-032 would
    /// block the resulting record (FR-026).
    pub duplicate_allowed: bool,
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
    /// specs/002-firearm-identification FR-009: rows whose original marks
    /// match another active firearm's, imported anyway (US4-6). Disjoint
    /// from `row_errors` — a row appears here only when it did not fail.
    pub warnings: Vec<RowError>,
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
    /// Conflicts the chosen action could not be applied to (a duplicate
    /// FR-032 forbids, an overwrite that fails validation). They stay open
    /// in the import session, so a different action can still be chosen.
    pub unresolved: Vec<RowError>,
    /// specs/002-firearm-identification FR-009: as `ImportResult::warnings`,
    /// for rows saved by an `overwrite` or `duplicate` resolution.
    pub warnings: Vec<RowError>,
}

struct PendingConflict {
    conflict_id: String,
    row: usize,
    existing_firearm_id: i64,
    new_input: FirearmInput,
}

/// Holds each in-progress import's matched-but-unresolved rows between the
/// initial `import_collection` call and the follow-up
/// `resolve_import_conflicts` call — Tauri-managed state (`app.manage`),
/// analogous to `Session`.
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
        let never = || false;
        export_collection_stoppable(
            conn,
            destination_folder,
            base_name,
            format,
            firearm_ids,
            on_progress,
            &never,
        )
    }

    /// [`export_collection`], checking `is_cancelled` between rows. A
    /// stopped export removes the photos folder it made, and never gets to
    /// the spreadsheet, which is written last; it fails with
    /// `OPERATION_STOPPED` (FR-037).
    pub fn export_collection_stoppable(
        conn: &Connection,
        destination_folder: &Path,
        base_name: &str,
        format: SpreadsheetFormat,
        firearm_ids: &[i64],
        on_progress: &mut dyn FnMut(usize, usize),
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<ExportResult, CommandError> {
        let photos_folder_path = destination_folder.join(format!("{base_name}_photos"));
        let spreadsheet_path =
            destination_folder.join(format!("{base_name}.{}", format.extension()));
        // Only what this export made is removed if it stops.
        let made_folder = !photos_folder_path.exists();
        let exported = export_rows(
            conn,
            &photos_folder_path,
            &spreadsheet_path,
            format,
            firearm_ids,
            on_progress,
            is_cancelled,
        );
        if exported.as_ref().is_err_and(|err| err.code == "OPERATION_STOPPED") && made_folder {
            let _ = std::fs::remove_dir_all(&photos_folder_path);
        }
        exported
    }

    fn export_rows(
        conn: &Connection,
        photos_folder_path: &Path,
        spreadsheet_path: &Path,
        format: SpreadsheetFormat,
        firearm_ids: &[i64],
        on_progress: &mut dyn FnMut(usize, usize),
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<ExportResult, CommandError> {
        std::fs::create_dir_all(photos_folder_path).map_err(|e| {
            CommandError::new("INTERNAL_ERROR", format!("Could not create the photos folder: {e}"))
        })?;

        let total = firearm_ids.len();
        let mut rows = Vec::with_capacity(total);
        let mut exported_photo_count = 0usize;

        for (index, &firearm_id) in firearm_ids.iter().enumerate() {
            if is_cancelled() {
                return Err(CommandError::operation_stopped(OperationKind::Export, None, None));
            }
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
                nickname: firearm.nickname.unwrap_or_default(),
                serial_number: firearm.serial_number.unwrap_or_default(),
                no_serial_attested: if firearm.no_serial_attested { "TRUE" } else { "FALSE" }
                    .to_string(),
                caliber: firearm.caliber,
                firearm_type: firearm_type_name,
                notes: firearm.notes.unwrap_or_default(),
                accessories: firearm.accessories.unwrap_or_default(),
                status: firearm.status.as_str().to_string(),
                estimated_value: dollars_to_string(firearm.estimated_value),
                acquisition_source: firearm.acquisition_source.unwrap_or_default(),
                acquisition_date: firearm.acquisition_date.unwrap_or_default(),
                acquisition_price: dollars_to_string(firearm.acquisition_price),
                disposition_type: firearm
                    .disposition_type
                    .map(|d| d.as_str().to_string())
                    .unwrap_or_default(),
                disposition_recipient: firearm.disposition_recipient.unwrap_or_default(),
                disposition_date: firearm.disposition_date.unwrap_or_default(),
                disposition_price: dollars_to_string(firearm.disposition_price),
                insurance_policy_name: insurance_policy_name.unwrap_or_default(),
                scheduled_coverage_amount: dollars_to_string(firearm.scheduled_coverage_amount),
                barrel_length_in: scaled_to_string(firearm.barrel_length_hundredths, 2),
                overall_length_in: scaled_to_string(firearm.overall_length_hundredths, 2),
                weight_oz: scaled_to_string(firearm.weight_tenths_oz, 1),
                capacity: scaled_to_string(firearm.capacity, 0),
                finish: firearm.finish.unwrap_or_default(),
                condition: firearm.condition.map(|c| c.label().to_string()).unwrap_or_default(),
                origin: firearm.origin.map(|o| o.label().to_string()).unwrap_or_default(),
                year_of_manufacture: scaled_to_string(firearm.year_of_manufacture, 0),
                country_of_manufacture: firearm.country_of_manufacture.unwrap_or_default(),
                importer_name: firearm.importer_name.unwrap_or_default(),
                original_make: firearm.original_make.unwrap_or_default(),
                original_model: firearm.original_model.unwrap_or_default(),
                original_serial_number: firearm.original_serial_number.unwrap_or_default(),
                photo_filenames: photo_filenames.join(";"),
            });

            on_progress(index + 1, total);
        }

        if is_cancelled() {
            return Err(CommandError::operation_stopped(OperationKind::Export, None, None));
        }
        write_spreadsheet(spreadsheet_path, format, &rows)?;

        Ok(ExportResult {
            spreadsheet_path: spreadsheet_path.to_owned(),
            photos_folder_path: photos_folder_path.to_owned(),
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

    /// A condition cell: the display name (`Like new`) or the stored form
    /// (`like_new`), in any letter case (FR-039).
    fn parse_condition(value: &str) -> Result<Condition, String> {
        let wanted = value.trim().to_lowercase();
        Condition::ALL
            .into_iter()
            .find(|c| c.label().to_lowercase() == wanted || c.as_str() == wanted)
            .ok_or_else(|| format!("condition: unknown condition {value:?}"))
    }

    /// An `origin` cell: exactly `Domestic`/`Imported`/`Re-imported`, matched
    /// ignoring letter case. Unlike `condition`, no alias is accepted —
    /// `reimported`/`re_imported` are row errors — since the label and the
    /// intent are the same word here (spreadsheet-format.md "Origin values",
    /// research.md §9, FR-014/US4-3).
    fn parse_origin(value: &str) -> Result<Origin, String> {
        match value.trim().to_lowercase().as_str() {
            "domestic" => Ok(Origin::Domestic),
            "imported" => Ok(Origin::Imported),
            "re-imported" => Ok(Origin::Reimported),
            _ => Err(format!(
                "origin: unknown origin {value:?}; must be Domestic, Imported or Re-imported"
            )),
        }
    }

    /// The reason shown for a failing row: every per-field message (the
    /// summary line alone says nothing about which field is wrong), in a
    /// stable order.
    fn row_message(error: &CommandError) -> String {
        let Some(fields) = &error.field_errors else {
            return error.message.clone();
        };
        let mut messages: Vec<_> = fields.iter().collect();
        messages.sort();
        messages
            .iter()
            .map(|(field, message)| format!("{field}: {message}"))
            .collect::<Vec<_>>()
            .join("; ")
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
            nickname: raw.nickname.clone(),
            notes: raw.notes.clone(),
            accessories: raw.accessories.clone(),
            barrel_length_hundredths: parse_scaled_decimal(
                "barrel_length_in",
                &raw.barrel_length_in,
                2,
            )?,
            overall_length_hundredths: parse_scaled_decimal(
                "overall_length_in",
                &raw.overall_length_in,
                2,
            )?,
            weight_tenths_oz: parse_scaled_decimal("weight_oz", &raw.weight_oz, 1)?,
            capacity: parse_scaled_decimal("capacity", &raw.capacity, 0)?,
            finish: raw.finish.clone(),
            condition: raw.condition.as_deref().map(parse_condition).transpose()?,
            status,
            estimated_value: parse_whole_dollars("estimated_value", &raw.estimated_value)?,
            acquisition_source: raw.acquisition_source.clone(),
            acquisition_date: raw.acquisition_date.clone(),
            acquisition_price: parse_whole_dollars("acquisition_price", &raw.acquisition_price)?,
            disposition_type,
            disposition_recipient: raw.disposition_recipient.clone(),
            disposition_date: raw.disposition_date.clone(),
            disposition_price: parse_whole_dollars("disposition_price", &raw.disposition_price)?,
            insurance_policy_id,
            scheduled_coverage_amount: parse_whole_dollars(
                "scheduled_coverage_amount",
                &raw.scheduled_coverage_amount,
            )?,
            // specs/002-firearm-identification: origin gating (importer,
            // country, original marks disallowed by the row's origin) is
            // enforced below by the same `validate_firearm_input` call
            // `condition`/`status` already go through, so no separate check
            // is needed here (FR-014).
            origin: raw.origin.as_deref().map(parse_origin).transpose()?,
            year_of_manufacture: parse_scaled_decimal(
                "year_of_manufacture",
                &raw.year_of_manufacture,
                0,
            )?,
            country_of_manufacture: raw.country_of_manufacture.clone(),
            importer_name: raw.importer_name.clone(),
            original_make: raw.original_make.clone(),
            original_model: raw.original_model.clone(),
            original_serial_number: raw.original_serial_number.clone(),
        };

        validate_firearm_input(&input).map_err(|e| row_message(&e))?;
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
        let never = || false;
        import_collection_stoppable(conn, file_path, format, store, on_progress, &never, &|_| {})
    }

    /// [`import_collection`], checking `is_cancelled` between rows and
    /// reporting each imported row to `record_imported`. Each row is saved
    /// on its own, so a stop keeps every row imported before it, and fails
    /// with `OPERATION_STOPPED` and their number (FR-037).
    pub fn import_collection_stoppable(
        conn: &Connection,
        file_path: &Path,
        format: SpreadsheetFormat,
        store: &ImportSessionStore,
        on_progress: &mut dyn FnMut(usize, usize),
        is_cancelled: &dyn Fn() -> bool,
        record_imported: &dyn Fn(u64),
    ) -> Result<ImportResult, CommandError> {
        let raw_rows = read_spreadsheet(file_path, format)?;
        let total = raw_rows.len();
        let session_id = format!("import-{}", chrono::Utc::now().timestamp_micros());

        let mut imported_count = 0;
        let mut row_errors = Vec::new();
        let mut conflicts = Vec::new();
        let mut pending = Vec::new();
        let mut warnings = Vec::new();

        for (index, raw) in raw_rows.iter().enumerate() {
            if is_cancelled() {
                return Err(CommandError::operation_stopped(
                    OperationKind::Import,
                    Some(imported_count as u64),
                    None,
                ));
            }
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
                        input.year_of_manufacture,
                    )?;
                    match existing {
                        Some(existing_firearm_id) => {
                            let conflict_id = format!("{session_id}-row-{row_number}");
                            conflicts.push(ImportConflict {
                                conflict_id: conflict_id.clone(),
                                row: row_number,
                                existing_firearm_id,
                                duplicate_allowed: firearm_ops::check_uniqueness(
                                    conn, None, &input,
                                )
                                .is_ok(),
                                make: input.make.clone(),
                                model: input.model.clone(),
                                serial_number: input.serial_number.clone(),
                            });
                            pending.push(PendingConflict {
                                conflict_id,
                                row: row_number,
                                existing_firearm_id,
                                new_input: input,
                            });
                        }
                        None => match firearm_ops::create_firearm(conn, &input, true) {
                            Ok(created) => {
                                imported_count += 1;
                                record_imported(imported_count as u64);
                                if let Some(message) = original_marks_warning(conn, &created)? {
                                    warnings.push(RowError { row: row_number, message });
                                }
                            }
                            // Rules that need the rest of the collection to
                            // judge (nickname/identity uniqueness) fail one
                            // row, not the whole import (FR-020).
                            Err(e) if e.code == "VALIDATION_ERROR" => row_errors
                                .push(RowError { row: row_number, message: row_message(&e) }),
                            Err(e) => return Err(e),
                        },
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
            warnings,
        })
    }

    /// specs/002-firearm-identification FR-009/T048: `saved`'s original
    /// marks against the firearms active now, excluding itself. Import and
    /// conflict resolution always save with `confirmed_warnings: true`
    /// (never prompting), so this is how a match is still surfaced — as a
    /// warning, never failing the row (US4-6).
    fn original_marks_warning(
        conn: &Connection,
        saved: &crate::models::firearm::Firearm,
    ) -> Result<Option<String>, CommandError> {
        let Some(other_id) = firearm_ops::original_marks_clash(
            conn,
            Some(saved.id),
            saved.original_make.as_deref(),
            saved.original_model.as_deref(),
            saved.original_serial_number.as_deref(),
        )?
        else {
            return Ok(None);
        };
        Ok(Some(firearm_ops::original_marks_warning_message(conn, other_id)?))
    }

    /// Applies each conflict's resolution: `overwrite` updates the existing
    /// record, `duplicate` inserts the imported row as a new record
    /// alongside it (only where FR-032 allows), `skip` (the default,
    /// including any conflict not covered by `resolutions` or
    /// `apply_to_remaining`) leaves the existing record untouched. A conflict
    /// the action can't be applied to is reported in `unresolved` and stays
    /// open in the session.
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
        let mut unresolved = Vec::new();
        let mut still_open = Vec::new();
        let mut warnings = Vec::new();
        for conflict in pending {
            let action = explicit
                .get(conflict.conflict_id.as_str())
                .copied()
                .or(apply_to_remaining)
                .unwrap_or("skip");
            // "skip", or an unrecognized action, leaves the existing record
            // untouched — never saved, so it never carries a warning.
            if action != "overwrite" && action != "duplicate" {
                resolved_count += 1;
                continue;
            }
            let outcome = if action == "overwrite" {
                firearm_ops::update_firearm(
                    conn,
                    conflict.existing_firearm_id,
                    &conflict.new_input,
                    true,
                )
            } else {
                firearm_ops::create_firearm(conn, &conflict.new_input, true)
            };
            match outcome {
                Ok(saved) => {
                    resolved_count += 1;
                    if let Some(message) = original_marks_warning(conn, &saved)? {
                        warnings.push(RowError { row: conflict.row, message });
                    }
                }
                Err(e) if e.code == "VALIDATION_ERROR" => {
                    unresolved.push(RowError { row: conflict.row, message: row_message(&e) });
                    still_open.push(conflict);
                }
                Err(e) => return Err(e),
            }
        }

        if !still_open.is_empty() {
            store
                .pending
                .lock()
                .expect("import session mutex poisoned")
                .insert(session_id.to_string(), still_open);
        }

        Ok(ResolveResult { resolved_count, unresolved, warnings })
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
    session: State<'_, Session>,
) -> Result<ExportResult, CommandError> {
    // Registered, so a sleep stops it and the idle clock pauses meanwhile.
    let operation = session.operations().begin(OperationKind::Export, None)?;
    session.read(|conn| {
        let format = parse_format(&input.format)?;
        let firearm_ids = if input.scope == "filtered" {
            let filter = input.filter.unwrap_or_default();
            let listing = firearm_ops::list_firearms(conn, &filter)?;
            listing.groups.into_iter().flat_map(|g| g.firearms).map(|f| f.id).collect()
        } else {
            ops::all_firearm_ids(conn)?
        };

        let base_name = format!("hoplodex-export-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
        ops::export_collection_stoppable(
            conn,
            Path::new(&input.destination_folder),
            &base_name,
            format,
            &firearm_ids,
            &mut |processed, total| {
                let _ =
                    app.emit("export_collection:progress", ProgressPayload { processed, total });
            },
            &|| operation.is_cancelled(),
        )
    })
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
    session: State<'_, Session>,
    session_store: State<'_, ImportSessionStore>,
) -> Result<ImportResult, CommandError> {
    // Registered, so a sleep stops it and the idle clock pauses meanwhile.
    let operation = session.operations().begin(OperationKind::Import, None)?;
    session.write(|conn| {
        let format = parse_format(&input.format)?;
        ops::import_collection_stoppable(
            conn,
            Path::new(&input.file_path),
            format,
            &session_store,
            &mut |processed, total| {
                let _ =
                    app.emit("import_collection:progress", ProgressPayload { processed, total });
            },
            &|| operation.is_cancelled(),
            &|imported| operation.record_done(imported),
        )
    })
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
    session: State<'_, Session>,
    session_store: State<'_, ImportSessionStore>,
) -> Result<ResolveResult, CommandError> {
    session.write(|conn| {
        ops::resolve_import_conflicts(
            conn,
            &session_store,
            &input.import_session_id,
            &input.resolutions,
            input.apply_to_remaining.as_deref(),
        )
    })
}
