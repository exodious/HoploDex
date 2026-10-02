use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, named_params};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::commands::CommandError;
use crate::commands::accessories::ops as accessory_ops;
use crate::commands::firearms::{DisposeWith, Disposition, ListFirearmsInput, ops as firearm_ops};
use crate::commands::mounts::ops as mount_ops;
use crate::models::accessory::{AccessoryInput, validate_accessory_input};
use crate::models::database::OperationKind;
use crate::models::firearm::{
    Condition, DispositionType, FirearmInput, FirearmStatus, Origin, validate_firearm_input,
};
use crate::models::record::{MountedEntry, RecordKind, RecordLabel, RecordRef};
use crate::services::cartridges::{CaliberSource, derive_caliber};
use crate::services::entry_text::{EntryField, check_entry_text};
use crate::services::mounts::{self, MountGraph};
use crate::services::record_id;
use crate::services::spreadsheet::{
    AccessoryExportRow, FirearmExportRow, RawAccessoryRow, RawImportRow, ReadTable,
    SpreadsheetFormat, TableKind, TableRows, dollars_to_string, parse_scaled_decimal,
    parse_whole_dollars, read_spreadsheet, scaled_to_string, write_accessory_csv,
    write_firearm_csv, write_workbook,
};
use crate::services::suggestions::{FieldVocabulary, SheetSpellings, snap_for_import};
use crate::session::Session;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub spreadsheet_path: PathBuf,
    /// specs/006-accessory-links FR-020: the second CSV file. `None` for a
    /// workbook (its second sheet is in `spreadsheet_path`) and when the
    /// export holds no accessory.
    pub accessory_spreadsheet_path: Option<PathBuf>,
    pub photos_folder_path: PathBuf,
    pub exported_firearm_count: usize,
    pub exported_accessory_count: usize,
    /// Firearm and accessory photos together.
    pub exported_photo_count: usize,
}

/// The records an export writes (FR-020): `ops::export_records`.
#[derive(Debug, Clone, Default)]
pub struct ExportRecords {
    pub firearm_ids: Vec<i64>,
    pub accessory_ids: Vec<i64>,
}

/// `get_export_scope`'s output: what the export dialog counts and discloses.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportScope {
    pub firearm_count: usize,
    pub accessory_count: usize,
    /// 005 FR-020's disclosure: some exported firearm has a classification.
    pub includes_registration: bool,
    /// FR-021's disclosure: the export holds an accessory.
    pub includes_accessories: bool,
}

/// Which table a row, and so a report entry, belongs to (FR-022).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportTable {
    Firearms,
    Accessories,
}

impl ImportTable {
    /// The table as a message names it ("also used by Accessories, row 3").
    fn heading(self) -> &'static str {
        match self {
            Self::Firearms => "Firearms",
            Self::Accessories => "Accessories",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowError {
    pub table: ImportTable,
    /// Numbered within its table, from 1.
    pub row: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConflict {
    pub conflict_id: String,
    pub table: ImportTable,
    pub row: usize,
    /// The record the row matched, by its identifier or, for a firearm row,
    /// by make, model and serial number.
    pub existing_record: RecordRef,
    /// Whether "create a duplicate" may be offered: false where FR-032 would
    /// block the resulting record (FR-026). Always true for an accessory.
    pub duplicate_allowed: bool,
    pub make: String,
    pub model: String,
    pub serial_number: Option<String>,
    /// An accessory row's kind name; `None` for a firearm.
    pub kind_name: Option<String>,
    /// Issue #56: when the row would dispose of an active record that has
    /// records mounted on it, everything below that record, as the dispose
    /// dialog lists it, so replacing it can ask which go with it. Empty
    /// otherwise.
    pub mounted: Vec<MountedEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub session_id: String,
    /// Both tables' rows.
    pub imported_count: usize,
    /// The accessory rows among `imported_count`.
    pub imported_accessory_count: usize,
    pub updated_count: usize,
    pub skipped_count: usize,
    pub row_errors: Vec<RowError>,
    pub conflicts: Vec<ImportConflict>,
    /// specs/002-firearm-identification FR-009: rows whose original marks
    /// match another active firearm's, imported anyway (US4-6); and
    /// specs/006-accessory-links FR-023: mounts that couldn't be made.
    /// Disjoint from `row_errors` — a row appears here only when it did not
    /// fail.
    pub warnings: Vec<RowError>,
    /// specs/004-cartridges-action-types FR-025: rows whose blank caliber was
    /// filled in from the cartridge. Imported and conflict rows only, never
    /// failed ones.
    pub derived_calibers: Vec<DerivedCaliberReport>,
    /// FR-026, SC-007: every value the import changed to a spelling already
    /// in use. Imported and conflict rows only.
    pub snapped_values: Vec<SnappedValue>,
}

/// A caliber worked out from the row's cartridge (FR-025).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedCaliberReport {
    pub table: ImportTable,
    pub row: usize,
    /// As recorded (after snapping).
    pub cartridge: String,
    /// As recorded.
    pub caliber: String,
    pub source: CaliberSource,
}

/// A value changed to the spelling in use (FR-026).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnappedValue {
    pub table: ImportTable,
    pub row: usize,
    pub field: EntryField,
    /// As in the sheet, trimmed.
    pub sheet_value: String,
    pub recorded_value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolution {
    pub conflict_id: String,
    pub action: String,
    /// Issue #56: for an `overwrite` that disposes of the record, the
    /// records of its `mounted` list to dispose of with it, taking the row's
    /// type, recipient and date and no price. The rest are kept (FR-014).
    #[serde(default)]
    pub with_mounted: Vec<RecordRef>,
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
    /// for rows saved by an `overwrite` or `duplicate` resolution, and the
    /// mounts those could not make (006 FR-023).
    pub warnings: Vec<RowError>,
}

/// One file of an import: a CSV file or a workbook (FR-022).
#[derive(Debug, Clone)]
pub struct ImportFile {
    pub file_path: PathBuf,
    pub format: SpreadsheetFormat,
}

/// The row as it would be saved, for whichever table it is in.
enum PendingInput {
    Firearm(Box<FirearmInput>),
    Accessory(Box<AccessoryInput>),
}

impl PendingInput {
    /// The row's disposition, when it marks the record disposed and has the
    /// type, recipient and date a disposed record needs. A row without them
    /// fails its own save.
    fn disposition(&self) -> Option<Disposition<'_>> {
        let (status, disposition_type, recipient, date) = match self {
            PendingInput::Firearm(input) => (
                input.status,
                input.disposition_type,
                &input.disposition_recipient,
                &input.disposition_date,
            ),
            PendingInput::Accessory(input) => (
                input.status,
                input.disposition_type,
                &input.disposition_recipient,
                &input.disposition_date,
            ),
        };
        if status != FirearmStatus::Disposed {
            return None;
        }
        Some(Disposition {
            disposition_type: disposition_type?,
            recipient: recipient.as_deref()?,
            date: date.as_deref()?,
        })
    }
}

struct PendingConflict {
    conflict_id: String,
    table: ImportTable,
    row: usize,
    existing: RecordRef,
    new_input: PendingInput,
    /// The row's `mounted_on` cell, applied when the row overwrites or is
    /// added as a duplicate (research.md §18).
    mounted_on: Option<String>,
}

/// One import's unresolved state between `import_collection` and
/// `resolve_import_conflicts`.
struct ImportSession {
    pending: Vec<PendingConflict>,
    /// Research.md §18's outcome map for the rows that conflicted: each
    /// one's identifier to the existing record it matched, so a mount can
    /// name such a row as its host.
    outcomes: HashMap<String, RecordRef>,
}

/// Holds each in-progress import's matched-but-unresolved rows between the
/// initial `import_collection` call and the follow-up
/// `resolve_import_conflicts` call — Tauri-managed state (`app.manage`),
/// analogous to `Session`.
#[derive(Default)]
pub struct ImportSessionStore {
    pending: Mutex<HashMap<String, ImportSession>>,
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

    fn all_ids(conn: &Connection, table: &str) -> Result<Vec<i64>, CommandError> {
        let mut stmt = conn
            .prepare(&format!("SELECT id FROM {table} ORDER BY id"))
            .map_err(CommandError::from_db)?;
        let rows = stmt.query_map([], |row| row.get(0)).map_err(CommandError::from_db)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CommandError::from_db)
    }

    /// Every firearm id, active or disposed.
    pub fn all_firearm_ids(conn: &Connection) -> Result<Vec<i64>, CommandError> {
        all_ids(conn, "firearms")
    }

    /// The records an export writes (FR-020). `"all"` is a full backup:
    /// every firearm and every accessory, active or disposed, unlike
    /// `list_firearms`'s default of active only. `"filtered"` is the
    /// firearms `filter` matches on the collection page and everything
    /// mounted on them at any depth, firearms and accessories alike, from
    /// one load of the mount graph; nothing else is exported, whatever it
    /// is mounted on.
    pub fn export_records(
        conn: &Connection,
        scope: &str,
        filter: Option<&ListFirearmsInput>,
    ) -> Result<ExportRecords, CommandError> {
        if scope != "filtered" {
            return Ok(ExportRecords {
                firearm_ids: all_ids(conn, "firearms")?,
                accessory_ids: all_ids(conn, "accessories")?,
            });
        }
        let default = ListFirearmsInput::default();
        let listing = firearm_ops::list_firearms(conn, filter.unwrap_or(&default))?;
        let matched: Vec<i64> =
            listing.groups.into_iter().flat_map(|g| g.firearms).map(|f| f.id).collect();

        let graph = MountGraph::load(conn)?;
        let mut known: HashSet<i64> = matched.iter().copied().collect();
        let mut mounted_firearms = Vec::new();
        let mut accessory_ids = Vec::new();
        let mut known_accessories = HashSet::new();
        for id in &matched {
            for (record, _, _) in graph.below(RecordRef::Firearm(*id)) {
                match record {
                    RecordRef::Firearm(id) if known.insert(id) => mounted_firearms.push(id),
                    RecordRef::Accessory(id) if known_accessories.insert(id) => {
                        accessory_ids.push(id)
                    }
                    _ => {}
                }
            }
        }
        mounted_firearms.sort_unstable();
        accessory_ids.sort_unstable();
        let mut firearm_ids = matched;
        firearm_ids.extend(mounted_firearms);
        Ok(ExportRecords { firearm_ids, accessory_ids })
    }

    /// What `export_collection` would write for this scope and filter, and
    /// what the export dialog must disclose about it (FR-020, FR-021).
    pub fn get_export_scope(
        conn: &Connection,
        scope: &str,
        filter: Option<&ListFirearmsInput>,
    ) -> Result<ExportScope, CommandError> {
        let records = export_records(conn, scope, filter)?;
        let includes_registration: bool = conn
            .query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM firearms
                    WHERE registration_class_id IS NOT NULL
                      AND id IN (SELECT value FROM json_each(:ids)))",
                named_params! { ":ids": json_ids(&records.firearm_ids) },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;
        Ok(ExportScope {
            firearm_count: records.firearm_ids.len(),
            accessory_count: records.accessory_ids.len(),
            includes_registration,
            includes_accessories: !records.accessory_ids.is_empty(),
        })
    }

    fn json_ids(ids: &[i64]) -> String {
        serde_json::to_string(ids).expect("integers serialize")
    }

    /// `id` to `name` for a lookup table, read once for the whole export.
    fn names(conn: &Connection, table: &str) -> Result<HashMap<i64, String>, CommandError> {
        let mut stmt = conn
            .prepare(&format!("SELECT id, name FROM {table}"))
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(CommandError::from_db)?;
        rows.collect::<Result<HashMap<_, _>, _>>().map_err(CommandError::from_db)
    }

    /// The identifiers of `records`, one query per table.
    fn uids_of(
        conn: &Connection,
        records: &[RecordRef],
    ) -> Result<HashMap<RecordRef, String>, CommandError> {
        let mut found = HashMap::new();
        for firearms in [true, false] {
            let ids: Vec<i64> = records
                .iter()
                .filter(|r| matches!(r, RecordRef::Firearm(_)) == firearms)
                .map(RecordRef::id)
                .collect();
            if ids.is_empty() {
                continue;
            }
            let table = if firearms { "firearms" } else { "accessories" };
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT id, uid FROM {table} WHERE id IN (SELECT value FROM json_each(:ids))"
                ))
                .map_err(CommandError::from_db)?;
            let rows = stmt
                .query_map(named_params! { ":ids": json_ids(&ids) }, |row| {
                    let id = row.get(0)?;
                    let record =
                        if firearms { RecordRef::Firearm(id) } else { RecordRef::Accessory(id) };
                    Ok((record, row.get::<_, String>(1)?))
                })
                .map_err(CommandError::from_db)?;
            for row in rows {
                let (record, uid) = row.map_err(CommandError::from_db)?;
                found.insert(record, uid);
            }
        }
        Ok(found)
    }

    /// The records with a photo, so only those are asked for theirs.
    fn records_with_photos(conn: &Connection) -> Result<HashSet<RecordRef>, CommandError> {
        let mut stmt = conn
            .prepare("SELECT DISTINCT firearm_id, accessory_id FROM photos")
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(match (row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?) {
                    (Some(id), _) => Some(RecordRef::Firearm(id)),
                    (None, Some(id)) => Some(RecordRef::Accessory(id)),
                    _ => None,
                })
            })
            .map_err(CommandError::from_db)?;
        let mut owners = HashSet::new();
        for row in rows {
            owners.extend(row.map_err(CommandError::from_db)?);
        }
        Ok(owners)
    }

    pub fn export_collection(
        conn: &Connection,
        destination_folder: &Path,
        base_name: &str,
        format: SpreadsheetFormat,
        records: &ExportRecords,
        on_progress: &mut dyn FnMut(usize, usize),
    ) -> Result<ExportResult, CommandError> {
        let never = || false;
        export_collection_stoppable(
            conn,
            destination_folder,
            base_name,
            format,
            records,
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
        records: &ExportRecords,
        on_progress: &mut dyn FnMut(usize, usize),
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<ExportResult, CommandError> {
        let photos_folder_path = destination_folder.join(format!("{base_name}_photos"));
        let spreadsheet_path =
            destination_folder.join(format!("{base_name}.{}", format.extension()));
        let accessory_csv_path = destination_folder.join(format!("{base_name}-accessories.csv"));
        // Only what this export made is removed if it stops.
        let made_folder = !photos_folder_path.exists();
        let files = ExportFiles {
            photos_folder: &photos_folder_path,
            spreadsheet: &spreadsheet_path,
            accessory_csv: &accessory_csv_path,
            format,
        };
        let exported = export_rows(conn, &files, records, on_progress, is_cancelled);
        if exported.as_ref().is_err_and(|err| err.code == "OPERATION_STOPPED") && made_folder {
            let _ = std::fs::remove_dir_all(&photos_folder_path);
        }
        exported
    }

    /// The records of `table` with these ids, by id.
    fn load_by_id<T>(
        conn: &Connection,
        table: &str,
        ids: &[i64],
        from_row: fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
        id_of: fn(&T) -> i64,
    ) -> Result<HashMap<i64, T>, CommandError> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT * FROM {table} WHERE id IN (SELECT value FROM json_each(:ids))"
            ))
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map(named_params! { ":ids": json_ids(ids) }, from_row)
            .map_err(CommandError::from_db)?;
        let mut found = HashMap::new();
        for row in rows {
            let record = row.map_err(CommandError::from_db)?;
            found.insert(id_of(&record), record);
        }
        Ok(found)
    }

    /// Writes the record's photos into the photos folder as
    /// `{prefix}{id}_{filename}` and returns the names, for the row's
    /// `photo_filenames` cell. `counted` gains one per photo.
    fn write_photos(
        conn: &Connection,
        photos_folder_path: &Path,
        owner: RecordRef,
        prefix: &str,
        counted: &mut usize,
    ) -> Result<String, CommandError> {
        let photos = crate::commands::photos::ops::list_photos(conn, owner)?;
        let mut names = Vec::with_capacity(photos.len());
        for photo in &photos {
            let dest_name = format!("{prefix}{}_{}", owner.id(), photo.original_filename);
            std::fs::write(photos_folder_path.join(&dest_name), &photo.original_bytes).map_err(
                |e| CommandError::new("INTERNAL_ERROR", format!("Could not write photo file: {e}")),
            )?;
            names.push(dest_name);
            *counted += 1;
        }
        Ok(names.join(";"))
    }

    /// Where an export writes, and in which format.
    struct ExportFiles<'a> {
        photos_folder: &'a Path,
        spreadsheet: &'a Path,
        /// The accessory table's file, used only by a CSV export.
        accessory_csv: &'a Path,
        format: SpreadsheetFormat,
    }

    fn export_rows(
        conn: &Connection,
        files: &ExportFiles<'_>,
        records: &ExportRecords,
        on_progress: &mut dyn FnMut(usize, usize),
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<ExportResult, CommandError> {
        let ExportFiles {
            photos_folder: photos_folder_path,
            spreadsheet: spreadsheet_path,
            accessory_csv: accessory_csv_path,
            format,
        } = *files;
        std::fs::create_dir_all(photos_folder_path).map_err(|e| {
            CommandError::new("INTERNAL_ERROR", format!("Could not create the photos folder: {e}"))
        })?;

        // Everything the rows name, read once rather than once per row.
        let firearm_types = names(conn, "firearm_types")?;
        let action_types = names(conn, "action_types")?;
        let registration_classes = names(conn, "registration_classes")?;
        let policies = names(conn, "insurance_policies")?;
        let kinds = names(conn, "accessory_kinds")?;
        let graph = MountGraph::load(conn)?;
        let exported: Vec<RecordRef> = records
            .firearm_ids
            .iter()
            .map(|id| RecordRef::Firearm(*id))
            .chain(records.accessory_ids.iter().map(|id| RecordRef::Accessory(*id)))
            .collect();
        let hosts: Vec<RecordRef> = exported.iter().filter_map(|r| graph.host_of(*r)).collect();
        let host_uids = uids_of(conn, &hosts)?;
        let mounted_on = |record: RecordRef| {
            graph.host_of(record).and_then(|host| host_uids.get(&host)).cloned().unwrap_or_default()
        };
        let with_photos = records_with_photos(conn)?;
        let firearms = load_by_id(
            conn,
            "firearms",
            &records.firearm_ids,
            crate::models::firearm::Firearm::from_row,
            |f| f.id,
        )?;
        let accessories = load_by_id(
            conn,
            "accessories",
            &records.accessory_ids,
            crate::models::accessory::Accessory::from_row,
            |a| a.id,
        )?;

        let total = exported.len();
        let mut done = 0usize;
        let mut firearm_rows = Vec::with_capacity(records.firearm_ids.len());
        let mut accessory_rows = Vec::with_capacity(records.accessory_ids.len());
        let mut exported_photo_count = 0usize;
        let name_of = |names: &HashMap<i64, String>, id: Option<i64>| {
            id.and_then(|id| names.get(&id)).cloned().unwrap_or_default()
        };

        for id in &records.firearm_ids {
            if is_cancelled() {
                return Err(CommandError::operation_stopped(OperationKind::Export, None, None));
            }
            let Some(firearm) = firearms.get(id) else {
                continue;
            };
            let owner = RecordRef::Firearm(*id);
            let photo_filenames = if with_photos.contains(&owner) {
                write_photos(conn, photos_folder_path, owner, "", &mut exported_photo_count)?
            } else {
                String::new()
            };
            firearm_rows.push(FirearmExportRow {
                record_id: firearm.uid.clone(),
                make: firearm.make.clone(),
                model: firearm.model.clone(),
                nickname: firearm.nickname.clone().unwrap_or_default(),
                serial_number: firearm.serial_number.clone().unwrap_or_default(),
                no_serial_attested: if firearm.no_serial_attested { "TRUE" } else { "FALSE" }
                    .to_string(),
                caliber: firearm.caliber.clone(),
                cartridge: firearm.cartridge.clone().unwrap_or_default(),
                action_type: name_of(&action_types, firearm.action_type_id),
                firearm_type: name_of(&firearm_types, Some(firearm.firearm_type_id)),
                notes: firearm.notes.clone().unwrap_or_default(),
                accessories: firearm.accessories.clone().unwrap_or_default(),
                status: firearm.status.as_str().to_string(),
                estimated_value: dollars_to_string(firearm.estimated_value),
                acquisition_source: firearm.acquisition_source.clone().unwrap_or_default(),
                acquisition_date: firearm.acquisition_date.clone().unwrap_or_default(),
                acquisition_price: dollars_to_string(firearm.acquisition_price),
                disposition_type: firearm
                    .disposition_type
                    .map(|d| d.as_str().to_string())
                    .unwrap_or_default(),
                disposition_recipient: firearm.disposition_recipient.clone().unwrap_or_default(),
                disposition_date: firearm.disposition_date.clone().unwrap_or_default(),
                disposition_price: dollars_to_string(firearm.disposition_price),
                insurance_policy_name: name_of(&policies, firearm.insurance_policy_id),
                scheduled_coverage_amount: dollars_to_string(firearm.scheduled_coverage_amount),
                barrel_length_in: scaled_to_string(firearm.barrel_length_hundredths, 2),
                overall_length_in: scaled_to_string(firearm.overall_length_hundredths, 2),
                weight_oz: scaled_to_string(firearm.weight_tenths_oz, 1),
                capacity: scaled_to_string(firearm.capacity, 0),
                finish: firearm.finish.clone().unwrap_or_default(),
                condition: firearm.condition.map(|c| c.label().to_string()).unwrap_or_default(),
                origin: firearm.origin.map(|o| o.label().to_string()).unwrap_or_default(),
                year_of_manufacture: scaled_to_string(firearm.year_of_manufacture, 0),
                country_of_manufacture: firearm.country_of_manufacture.clone().unwrap_or_default(),
                importer_name: firearm.importer_name.clone().unwrap_or_default(),
                original_make: firearm.original_make.clone().unwrap_or_default(),
                original_model: firearm.original_model.clone().unwrap_or_default(),
                original_serial_number: firearm.original_serial_number.clone().unwrap_or_default(),
                registered_as: name_of(&registration_classes, firearm.registration_class_id),
                registration_form: firearm.registration_form.clone().unwrap_or_default(),
                registration_approved: firearm.registration_approved.clone().unwrap_or_default(),
                registered_to: firearm.registered_to.clone().unwrap_or_default(),
                mounted_on: mounted_on(owner),
                photo_filenames,
            });
            done += 1;
            on_progress(done, total);
        }

        for id in &records.accessory_ids {
            if is_cancelled() {
                return Err(CommandError::operation_stopped(OperationKind::Export, None, None));
            }
            let Some(accessory) = accessories.get(id) else {
                continue;
            };
            let owner = RecordRef::Accessory(*id);
            // `a{id}_{filename}`, so an accessory's photo can't collide with
            // a firearm's `{id}_{filename}`.
            let photo_filenames = if with_photos.contains(&owner) {
                write_photos(conn, photos_folder_path, owner, "a", &mut exported_photo_count)?
            } else {
                String::new()
            };
            accessory_rows.push(AccessoryExportRow {
                record_id: accessory.uid.clone(),
                kind: name_of(&kinds, Some(accessory.accessory_kind_id)),
                make: accessory.make.clone(),
                model: accessory.model.clone(),
                serial_number: accessory.serial_number.clone().unwrap_or_default(),
                caliber: accessory.caliber.clone().unwrap_or_default(),
                cartridge: accessory.cartridge.clone().unwrap_or_default(),
                notes: accessory.notes.clone().unwrap_or_default(),
                status: accessory.status.as_str().to_string(),
                estimated_value: dollars_to_string(accessory.estimated_value),
                acquisition_source: accessory.acquisition_source.clone().unwrap_or_default(),
                acquisition_date: accessory.acquisition_date.clone().unwrap_or_default(),
                acquisition_price: dollars_to_string(accessory.acquisition_price),
                disposition_type: accessory
                    .disposition_type
                    .map(|d| d.as_str().to_string())
                    .unwrap_or_default(),
                disposition_recipient: accessory.disposition_recipient.clone().unwrap_or_default(),
                disposition_date: accessory.disposition_date.clone().unwrap_or_default(),
                disposition_price: dollars_to_string(accessory.disposition_price),
                insurance_policy_name: name_of(&policies, accessory.insurance_policy_id),
                scheduled_coverage_amount: dollars_to_string(accessory.scheduled_coverage_amount),
                mounted_on: mounted_on(owner),
                photo_filenames,
            });
            done += 1;
            on_progress(done, total);
        }

        if is_cancelled() {
            return Err(CommandError::operation_stopped(OperationKind::Export, None, None));
        }
        // The accessory table only when the export holds an accessory
        // (contracts/spreadsheet-format.md "Two tables").
        let has_accessories = !accessory_rows.is_empty();
        let mut accessory_spreadsheet_path = None;
        match format {
            SpreadsheetFormat::Csv => {
                write_firearm_csv(spreadsheet_path, &firearm_rows)?;
                if has_accessories {
                    write_accessory_csv(accessory_csv_path, &accessory_rows)?;
                    accessory_spreadsheet_path = Some(accessory_csv_path.to_owned());
                }
            }
            SpreadsheetFormat::Xlsx => write_workbook(
                spreadsheet_path,
                &firearm_rows,
                has_accessories.then_some(accessory_rows.as_slice()),
            )?,
        }

        Ok(ExportResult {
            spreadsheet_path: spreadsheet_path.to_owned(),
            accessory_spreadsheet_path,
            photos_folder_path: photos_folder_path.to_owned(),
            exported_firearm_count: firearm_rows.len(),
            exported_accessory_count: accessory_rows.len(),
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

    /// The spreadsheet column a validation field error is about, where the
    /// field is named differently over IPC (contracts/spreadsheet-format.md
    /// "Row errors"); any other field is shown as it is.
    fn sheet_column(field: &str) -> String {
        match field {
            "actionTypeId" => "action_type".to_owned(),
            "barrelLengthHundredths" => "barrel_length_in".to_owned(),
            "insurancePolicyId" => "insurance_policy_name".to_owned(),
            "accessoryKindId" => "kind".to_owned(),
            // The IPC names are the columns' in camel case.
            other => {
                let mut column = String::with_capacity(other.len() + 2);
                for c in other.chars() {
                    if c.is_ascii_uppercase() {
                        column.push('_');
                    }
                    column.push(c.to_ascii_lowercase());
                }
                column
            }
        }
    }

    /// The reason shown for a failing row: every per-field message (the
    /// summary line alone says nothing about which field is wrong), in a
    /// stable order. A record disposed of with the row's names itself
    /// (`withMounted`, issue #56), so it gets no column.
    fn row_message(error: &CommandError) -> String {
        let Some(fields) = &error.field_errors else {
            return error.message.clone();
        };
        let mut messages: Vec<_> = fields.iter().collect();
        messages.sort();
        messages
            .iter()
            .map(|(field, message)| match field.as_str() {
                "withMounted" => message.to_string(),
                _ => format!("{}: {message}", sheet_column(field)),
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// The entry cells of a raw row, for either table: the fields a row
    /// is settled on (FR-015, FR-025, FR-026). The registration fields are
    /// the firearm table's alone.
    trait EntryCells {
        fn cell(&self, field: EntryField) -> Option<&str>;
        fn set_cell(&mut self, field: EntryField, value: String);
    }

    impl EntryCells for RawImportRow {
        fn cell(&self, field: EntryField) -> Option<&str> {
            match field {
                EntryField::Make => self.make.as_deref(),
                EntryField::Model => self.model.as_deref(),
                EntryField::Cartridge => self.cartridge.as_deref(),
                EntryField::Caliber => self.caliber.as_deref(),
                EntryField::RegistrationForm => self.registration_form.as_deref(),
                EntryField::RegisteredTo => self.registered_to.as_deref(),
            }
        }

        fn set_cell(&mut self, field: EntryField, value: String) {
            let slot = match field {
                EntryField::Make => &mut self.make,
                EntryField::Model => &mut self.model,
                EntryField::Cartridge => &mut self.cartridge,
                EntryField::Caliber => &mut self.caliber,
                EntryField::RegistrationForm => &mut self.registration_form,
                EntryField::RegisteredTo => &mut self.registered_to,
            };
            *slot = Some(value);
        }
    }

    impl EntryCells for RawAccessoryRow {
        fn cell(&self, field: EntryField) -> Option<&str> {
            match field {
                EntryField::Make => self.make.as_deref(),
                EntryField::Model => self.model.as_deref(),
                EntryField::Cartridge => self.cartridge.as_deref(),
                EntryField::Caliber => self.caliber.as_deref(),
                EntryField::RegistrationForm | EntryField::RegisteredTo => None,
            }
        }

        fn set_cell(&mut self, field: EntryField, value: String) {
            let slot = match field {
                EntryField::Make => &mut self.make,
                EntryField::Model => &mut self.model,
                EntryField::Cartridge => &mut self.cartridge,
                EntryField::Caliber => &mut self.caliber,
                EntryField::RegistrationForm | EntryField::RegisteredTo => return,
            };
            *slot = Some(value);
        }
    }

    /// The snapping context of one import (research.md §12): the four fields'
    /// vocabularies as they were on record when the import started, in
    /// firearms and accessories together, and the sheets' own majority
    /// spellings across both tables. Rows added by the import never enter
    /// the vocabularies. Beside them, the names of the types whose caliber
    /// is never worked out from the cartridge (005 FR-022, research.md §15),
    /// since `settle_row` runs before `parse_row` and has no connection.
    struct Snapping {
        vocabularies: HashMap<EntryField, FieldVocabulary>,
        sheet: HashMap<EntryField, SheetSpellings>,
        underived_types: Vec<String>,
    }

    impl Snapping {
        fn new(
            conn: &Connection,
            firearms: &[RawImportRow],
            accessories: &[RawAccessoryRow],
        ) -> Result<Self, CommandError> {
            let mut vocabularies = HashMap::new();
            let mut sheet = HashMap::new();
            for field in EntryField::ALL {
                vocabularies.insert(
                    field,
                    FieldVocabulary::load(conn, field).map_err(CommandError::from_db)?,
                );
                sheet.insert(
                    field,
                    SheetSpellings::from_cells(
                        field,
                        firearms.iter().map(|raw| raw.cell(field).unwrap_or_default()).chain(
                            accessories.iter().map(|raw| raw.cell(field).unwrap_or_default()),
                        ),
                    ),
                );
            }
            let underived_types = conn
                .prepare("SELECT name FROM firearm_types WHERE caliber_from_cartridge = 0")
                .and_then(|mut stmt| {
                    stmt.query_map([], |row| row.get(0))?.collect::<Result<Vec<String>, _>>()
                })
                .map_err(CommandError::from_db)?;
            Ok(Self { vocabularies, sheet, underived_types })
        }

        /// The recorded name of the row's type when that type never derives
        /// its caliber, matched as `parse_row` matches it (`COLLATE NOCASE`).
        /// An unknown type derives as before, and `parse_row` refuses it.
        fn underived_type(&self, raw: &RawImportRow) -> Option<&str> {
            let name = raw.firearm_type.as_deref()?;
            self.underived_types.iter().find(|t| t.eq_ignore_ascii_case(name)).map(String::as_str)
        }

        fn snap(&self, field: EntryField, text: &str) -> String {
            snap_for_import(&self.vocabularies[&field], &self.sheet[&field], text)
        }
    }

    /// What settling a row changed, reported only if the row is imported or
    /// becomes a conflict.
    #[derive(Default)]
    struct Settled {
        snapped: Vec<(EntryField, String, String)>,
        derived: Option<(String, String, CaliberSource)>,
    }

    /// Checks and snaps the cells of `fields` that the row has.
    fn settle_entries<R: EntryCells>(
        snapping: &Snapping,
        raw: &R,
        row: &mut R,
        settled: &mut Settled,
        fields: &[EntryField],
    ) -> Result<(), String> {
        for field in fields {
            let Some(text) = raw.cell(*field) else {
                continue;
            };
            check_entry_text(*field, text).map_err(|m| format!("{}: {m}", field.column()))?;
            let snapped = snapping.snap(*field, text);
            if snapped != text {
                settled.snapped.push((*field, text.to_owned(), snapped.clone()));
            }
            row.set_cell(*field, snapped);
        }
        Ok(())
    }

    /// FR-015, FR-025, FR-026 and (005) FR-021 for one row: checks the entry cells,
    /// snaps them, and fills a blank caliber in from the cartridge, unless the
    /// row's type never derives one (005 FR-022). Returns the row to parse;
    /// the error is a row error's message.
    fn settle_row(
        snapping: &Snapping,
        raw: &RawImportRow,
    ) -> Result<(RawImportRow, Settled), String> {
        let mut row = raw.clone();
        let mut settled = Settled::default();
        settle_entries(
            snapping,
            raw,
            &mut row,
            &mut settled,
            &[
                EntryField::Make,
                EntryField::Model,
                EntryField::Cartridge,
                EntryField::RegistrationForm,
                EntryField::RegisteredTo,
            ],
        )?;

        match raw.cell(EntryField::Caliber) {
            Some(_) => {
                settle_entries(snapping, raw, &mut row, &mut settled, &[EntryField::Caliber])?
            }
            None if let Some(type_name) = snapping.underived_type(raw) => {
                // A Suppressor's caliber is its bore, never its rated
                // cartridge's (005 research.md §15).
                return Err(match &row.cartridge {
                    Some(_) => format!(
                        "caliber: Caliber is required; a {type_name}'s isn't worked out from its cartridge."
                    ),
                    None => "caliber: Caliber is required.".to_owned(),
                });
            }
            None => {
                let cartridge = row.cartridge.clone();
                let derived = cartridge.as_deref().and_then(derive_caliber);
                let (Some(cartridge), Some(derived)) = (cartridge, derived) else {
                    let reason = match &row.cartridge {
                        Some(cartridge) => format!(
                            "; it couldn't be worked out from the cartridge \"{cartridge}\""
                        ),
                        None => String::new(),
                    };
                    return Err(format!("caliber: Caliber is required{reason}."));
                };
                let caliber = snapping.snap(EntryField::Caliber, &derived.caliber);
                row.set_cell(EntryField::Caliber, caliber.clone());
                settled.derived = Some((cartridge, caliber, derived.source));
            }
        }
        Ok((row, settled))
    }

    /// [`settle_row`] for an accessory row (FR-024): make, model, caliber and
    /// cartridge are all optional, so a blank caliber is derived from the
    /// cartridge when it can be, and otherwise stays blank (research.md §16).
    fn settle_accessory_row(
        snapping: &Snapping,
        raw: &RawAccessoryRow,
    ) -> Result<(RawAccessoryRow, Settled), String> {
        let mut row = raw.clone();
        let mut settled = Settled::default();
        settle_entries(
            snapping,
            raw,
            &mut row,
            &mut settled,
            &[EntryField::Make, EntryField::Model, EntryField::Cartridge, EntryField::Caliber],
        )?;
        if raw.caliber.is_none()
            && let Some(cartridge) = row.cartridge.clone()
            && let Some(derived) = derive_caliber(&cartridge)
        {
            let caliber = snapping.snap(EntryField::Caliber, &derived.caliber);
            row.set_cell(EntryField::Caliber, caliber.clone());
            settled.derived = Some((cartridge, caliber, derived.source));
        }
        Ok((row, settled))
    }

    /// Adds `settled` to the report; called for imported and conflict rows
    /// only.
    fn report(
        settled: Settled,
        table: ImportTable,
        row: usize,
        snapped_values: &mut Vec<SnappedValue>,
        derived_calibers: &mut Vec<DerivedCaliberReport>,
    ) {
        for (field, sheet_value, recorded_value) in settled.snapped {
            snapped_values.push(SnappedValue { table, row, field, sheet_value, recorded_value });
        }
        if let Some((cartridge, caliber, source)) = settled.derived {
            derived_calibers.push(DerivedCaliberReport { table, row, cartridge, caliber, source });
        }
    }

    /// Parses and validates one raw spreadsheet row into a `FirearmInput`,
    /// resolving `firearm_type`/`insurance_policy_name` by lookup — a
    /// human-readable `Err` message per FR-020, never a panic/abort of the
    /// whole import.
    fn parse_row(conn: &Connection, raw: &RawImportRow) -> Result<FirearmInput, String> {
        let make = raw.make.clone().ok_or("Missing required field: make")?;
        let model = raw.model.clone().ok_or("Missing required field: model")?;
        // `settle_row` has already filled a blank caliber in or failed the row.
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

        // FR-024: by name, ignoring letter case and surrounding whitespace;
        // blank = none.
        let action_type_id = match &raw.action_type {
            None => None,
            Some(name) => {
                let id: i64 = conn
                    .query_row(
                        "SELECT id FROM action_types WHERE name = :name COLLATE NOCASE",
                        named_params! { ":name": name.trim() },
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|e| format!("Database error resolving action type: {e}"))?
                    .ok_or_else(|| format!("action_type: unknown action type {name:?}"))?;
                Some(id)
            }
        };

        // specs/005-regulated-item-types FR-021: by name among every
        // classification, offered or not, ignoring letter case and
        // surrounding whitespace; blank = none.
        let registration_class_id = match &raw.registered_as {
            None => {
                if raw.registration_form.is_some()
                    || raw.registration_approved.is_some()
                    || raw.registered_to.is_some()
                {
                    return Err(
                        "registered_as: Registration details need a classification.".to_string()
                    );
                }
                None
            }
            Some(name) => Some(
                conn.query_row(
                    "SELECT id FROM registration_classes WHERE name = :name COLLATE NOCASE",
                    named_params! { ":name": name.trim() },
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(|e| format!("Database error resolving classification: {e}"))?
                .ok_or_else(|| format!("registered_as: unknown classification {name:?}"))?,
            ),
        };

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
            cartridge: raw.cartridge.clone(),
            action_type_id,
            registration_class_id,
            registration_form: raw.registration_form.clone(),
            // Read as `acquisition_date` is: the cell as text, then
            // `validate_firearm_input`'s `checked_date`.
            registration_approved: raw.registration_approved.clone(),
            registered_to: raw.registered_to.clone(),
            // The row's mounted_on is resolved once every row is settled
            // (research.md §18), never while it is parsed.
            mounted_on: None,
        };

        // FR-022: fields the type doesn't have come first, so a Suppressor
        // row with an action is never reported as "allowed".
        firearm_ops::check_fields_apply(conn, &input).map_err(|e| row_message(&e))?;
        firearm_ops::check_action_allowed(conn, firearm_type_id, action_type_id)
            .map_err(|e| format!("action_type: {}", e.message))?;
        validate_firearm_input(&input, None).map_err(|e| row_message(&e))?;
        Ok(input)
    }

    /// Parses and validates one raw accessory row into an `AccessoryInput`
    /// and its kind's name, resolving `kind` and `insurance_policy_name` by
    /// lookup (FR-024). A human-readable `Err` message, as `parse_row`.
    fn parse_accessory_row(
        conn: &Connection,
        raw: &RawAccessoryRow,
    ) -> Result<(AccessoryInput, String), String> {
        let wanted = raw.kind.clone().ok_or("kind: Choose a kind.")?;
        let make = raw.make.clone().ok_or("Missing required field: make")?;
        let model = raw.model.clone().ok_or("Missing required field: model")?;
        // By name among every kind, offered or not, ignoring letter case and
        // surrounding whitespace.
        let (accessory_kind_id, kind_name): (i64, String) = conn
            .query_row(
                "SELECT id, name FROM accessory_kinds WHERE name = :name COLLATE NOCASE",
                named_params! { ":name": wanted.trim() },
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| format!("Database error resolving kind: {e}"))?
            .ok_or_else(|| format!("kind: unknown kind {wanted:?}"))?;

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

        let input = AccessoryInput {
            accessory_kind_id,
            make,
            model,
            serial_number: raw.serial_number.clone(),
            caliber: raw.caliber.clone(),
            cartridge: raw.cartridge.clone(),
            notes: raw.notes.clone(),
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
            mounted_on: None,
        };
        validate_accessory_input(&input, None).map_err(|e| row_message(&e))?;
        Ok((input, kind_name))
    }

    /// Why a row did not land: a message for the report, or an error that
    /// stops the whole import.
    enum Fail {
        Row(String),
        Fatal(CommandError),
    }

    impl From<String> for Fail {
        fn from(message: String) -> Self {
            Self::Row(message)
        }
    }

    impl From<&str> for Fail {
        fn from(message: &str) -> Self {
            Self::Row(message.to_owned())
        }
    }

    impl From<CommandError> for Fail {
        fn from(error: CommandError) -> Self {
            Self::Fatal(error)
        }
    }

    /// Where a row ended up.
    enum Landed {
        Created {
            record: RecordRef,
            /// The original-marks warning of a firearm (002 FR-009).
            warning: Option<String>,
        },
        Conflict {
            existing: RecordRef,
            input: PendingInput,
            duplicate_allowed: bool,
            make: String,
            model: String,
            serial_number: Option<String>,
            kind_name: Option<String>,
        },
    }

    /// A row that was created or matched, with what the report needs of it.
    struct Imported {
        uid: Option<String>,
        settled: Settled,
        mounted_on: Option<String>,
        landed: Landed,
    }

    /// FR-022: the row's identifier, trimmed and lowercased, or the row
    /// error it earns: malformed, or used by an earlier row of either table.
    fn row_record_id(
        cell: Option<&str>,
        seen: &HashMap<String, (ImportTable, usize)>,
    ) -> Result<Option<String>, String> {
        let Some(cell) = cell else {
            return Ok(None);
        };
        let uid = record_id::parse(cell)
            .ok_or_else(|| format!("record_id: \"{cell}\" is not a record ID"))?;
        match seen.get(&uid) {
            Some((table, row)) => {
                Err(format!("record_id: {uid} is also used by {}, row {row}", table.heading()))
            }
            None => Ok(Some(uid)),
        }
    }

    /// The record of `kind` that holds `uid`, if any; one of the other kind
    /// is a row error (contracts/spreadsheet-format.md "Record ID").
    fn matched_by_record_id(
        conn: &Connection,
        kind: RecordKind,
        uid: Option<&str>,
    ) -> Result<Option<i64>, Fail> {
        let Some(uid) = uid else {
            return Ok(None);
        };
        match crate::services::import_matching::find_by_record_id(conn, uid)? {
            Some(found) if found.kind() == kind => Ok(Some(found.id())),
            Some(RecordRef::Accessory(_)) => {
                Err(format!("record_id: {uid} belongs to an accessory").into())
            }
            Some(RecordRef::Firearm(_)) => {
                Err(format!("record_id: {uid} belongs to a firearm").into())
            }
            None => Ok(None),
        }
    }

    fn import_firearm_row(
        conn: &Connection,
        snapping: &Snapping,
        raw: &RawImportRow,
        seen: &HashMap<String, (ImportTable, usize)>,
    ) -> Result<Imported, Fail> {
        let uid = row_record_id(raw.record_id.as_deref(), seen)?;
        let by_id = matched_by_record_id(conn, RecordKind::Firearm, uid.as_deref())?;
        let (row, settled) = settle_row(snapping, raw)?;
        let input = parse_row(conn, &row)?;
        // The identifier first, then 001's make, model and serial number.
        let existing = match by_id {
            Some(id) => Some(id),
            None => crate::services::import_matching::find_match(
                conn,
                &input.make,
                &input.model,
                input.serial_number.as_deref(),
                input.no_serial_attested,
                input.year_of_manufacture,
            )?,
        };
        let mounted_on = raw.mounted_on.clone();
        if let Some(id) = existing {
            return Ok(Imported {
                uid,
                settled,
                mounted_on,
                landed: Landed::Conflict {
                    existing: RecordRef::Firearm(id),
                    duplicate_allowed: firearm_ops::check_uniqueness(conn, None, &input).is_ok(),
                    make: input.make.clone(),
                    model: input.model.clone(),
                    serial_number: input.serial_number.clone(),
                    kind_name: None,
                    input: PendingInput::Firearm(Box::new(input)),
                },
            });
        }
        match firearm_ops::create_firearm(conn, &input, true, uid.as_deref()) {
            Ok(created) => {
                let warning = original_marks_warning(conn, &created)?;
                Ok(Imported {
                    uid,
                    settled,
                    mounted_on,
                    landed: Landed::Created { record: RecordRef::Firearm(created.id), warning },
                })
            }
            // Rules that need the rest of the collection to judge
            // (nickname/identity uniqueness) fail one row, not the whole
            // import (FR-020).
            Err(e) if e.code == "VALIDATION_ERROR" => Err(Fail::Row(row_message(&e))),
            Err(e) => Err(Fail::Fatal(e)),
        }
    }

    fn import_accessory_row(
        conn: &Connection,
        snapping: &Snapping,
        raw: &RawAccessoryRow,
        seen: &HashMap<String, (ImportTable, usize)>,
    ) -> Result<Imported, Fail> {
        let uid = row_record_id(raw.record_id.as_deref(), seen)?;
        let by_id = matched_by_record_id(conn, RecordKind::Accessory, uid.as_deref())?;
        let (row, settled) = settle_accessory_row(snapping, raw)?;
        let (input, kind_name) = parse_accessory_row(conn, &row)?;
        let mounted_on = raw.mounted_on.clone();
        // An accessory has no key but its identifier: no match means new.
        if let Some(id) = by_id {
            return Ok(Imported {
                uid,
                settled,
                mounted_on,
                landed: Landed::Conflict {
                    existing: RecordRef::Accessory(id),
                    duplicate_allowed: true,
                    make: input.make.clone(),
                    model: input.model.clone(),
                    serial_number: input.serial_number.clone(),
                    kind_name: Some(kind_name),
                    input: PendingInput::Accessory(Box::new(input)),
                },
            });
        }
        match accessory_ops::create_accessory(conn, &input, uid.as_deref()) {
            Ok(created) => Ok(Imported {
                uid,
                settled,
                mounted_on,
                landed: Landed::Created { record: RecordRef::Accessory(created.id), warning: None },
            }),
            Err(e) if e.code == "VALIDATION_ERROR" => Err(Fail::Row(row_message(&e))),
            Err(e) => Err(Fail::Fatal(e)),
        }
    }

    /// A mount to make once every row is settled (research.md §18).
    struct MountRequest {
        table: ImportTable,
        row: usize,
        /// The record the row became (or, for a conflict, the one it
        /// changed).
        record: RecordRef,
        cell: String,
    }

    /// The name a warning gives a record: its make and model.
    fn record_name(label: &RecordLabel) -> String {
        format!("{} {}", label.make, label.model)
    }

    /// Every record's identifier, and which records are active, from two
    /// scans, so resolving a sheet's mounts asks nothing per row.
    fn record_index(
        conn: &Connection,
    ) -> Result<(HashMap<String, RecordRef>, HashSet<RecordRef>), CommandError> {
        let mut by_uid = HashMap::new();
        let mut active = HashSet::new();
        for firearms in [true, false] {
            let table = if firearms { "firearms" } else { "accessories" };
            let mut stmt = conn
                .prepare(&format!("SELECT uid, id, status = 'active' FROM {table}"))
                .map_err(CommandError::from_db)?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, bool>(2)?))
                })
                .map_err(CommandError::from_db)?;
            for row in rows {
                let (uid, id, is_active) = row.map_err(CommandError::from_db)?;
                let record =
                    if firearms { RecordRef::Firearm(id) } else { RecordRef::Accessory(id) };
                by_uid.insert(uid, record);
                if is_active {
                    active.insert(record);
                }
            }
        }
        Ok((by_uid, active))
    }

    /// Research.md §18: makes each request's mount, in the order given. A
    /// mount that can't be made never fails the row: the record stays
    /// unmounted and a warning names the row and the reason. The graph is
    /// loaded once and kept current, so each mount is checked against the
    /// ones before it.
    fn resolve_mounts(
        conn: &Connection,
        requests: &[MountRequest],
        outcomes: &HashMap<String, RecordRef>,
        warnings: &mut Vec<RowError>,
    ) -> Result<(), CommandError> {
        if requests.is_empty() {
            return Ok(());
        }
        let mut graph = MountGraph::load(conn)?;
        let (by_uid, active) = record_index(conn)?;
        for request in requests {
            let warn = |warnings: &mut Vec<RowError>, message: String| {
                warnings.push(RowError {
                    table: request.table,
                    row: request.row,
                    message: format!("{message} Imported unmounted."),
                });
            };
            let Some(host_id) = record_id::parse(&request.cell) else {
                warn(warnings, format!("Mounted on {}: not a record ID.", request.cell));
                continue;
            };
            // The rows of this import first: a host row that matched an
            // existing record counts as that record. Then the database.
            let Some(host) = outcomes.get(&host_id).or_else(|| by_uid.get(&host_id)).copied()
            else {
                warn(
                    warnings,
                    format!("Mounted on {host_id}: no firearm or accessory has this record ID."),
                );
                continue;
            };
            if !active.contains(&host) {
                warn(warnings, format!("Mounted on {host_id}: that {} is disposed.", host.noun()));
                continue;
            }
            if !active.contains(&request.record) {
                warn(
                    warnings,
                    format!(
                        "Mounted on {host_id}: a disposed {} is never mounted.",
                        request.record.noun()
                    ),
                );
                continue;
            }
            if graph.would_loop(request.record, host) {
                let name = mounts::label(conn, host)?
                    .map_or_else(|| host_id.clone(), |label| record_name(&label));
                warn(
                    warnings,
                    format!("Mounted on {host_id}: it would be mounted on itself through {name}."),
                );
                continue;
            }
            match mounts::set_mount(conn, &graph, request.record, Some(host), "mountedOn") {
                Ok(()) => graph.place(request.record, Some(host)),
                Err(e) if e.code == "VALIDATION_ERROR" => {
                    warn(warnings, format!("Mounted on {host_id}: {}", e.message));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Issue #56: fills in `mounted` for each conflict whose row would
    /// dispose of the record, once this import's own mounts are made, so
    /// the list is what replacing it would find.
    fn list_mounted(
        conn: &Connection,
        conflicts: &mut [ImportConflict],
        pending: &[PendingConflict],
    ) -> Result<(), CommandError> {
        if !pending.iter().any(|p| p.new_input.disposition().is_some()) {
            return Ok(());
        }
        let graph = MountGraph::load(conn)?;
        for (conflict, pending) in conflicts.iter_mut().zip(pending) {
            debug_assert_eq!(conflict.conflict_id, pending.conflict_id);
            if pending.new_input.disposition().is_some() {
                conflict.mounted = mounts::mounted_entries(conn, &graph, pending.existing)?;
            }
        }
        Ok(())
    }

    /// An `overwrite`: a row that disposes of the record takes the dispose
    /// dialog's steps, unmounting it and everything on it and disposing of
    /// the records chosen with it (FR-014, issue #56). `save` saves the
    /// row.
    fn replace<T>(
        conn: &Connection,
        existing: RecordRef,
        new_input: &PendingInput,
        with_mounted: &[RecordRef],
        save: impl FnOnce() -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let Some(disposition) = new_input.disposition() else {
            return save();
        };
        let with: Vec<DisposeWith> =
            with_mounted.iter().map(|&record| DisposeWith { record, price: None }).collect();
        mount_ops::dispose_chain(conn, existing, disposition, &with, save)
    }

    /// The report being built, and what a landed row adds to it.
    struct Run {
        session_id: String,
        imported_count: usize,
        imported_accessory_count: usize,
        row_errors: Vec<RowError>,
        conflicts: Vec<ImportConflict>,
        pending: Vec<PendingConflict>,
        warnings: Vec<RowError>,
        derived_calibers: Vec<DerivedCaliberReport>,
        snapped_values: Vec<SnappedValue>,
        /// Identifier to the table and row that settled with it.
        seen: HashMap<String, (ImportTable, usize)>,
        mount_requests: Vec<MountRequest>,
        outcomes: HashMap<String, RecordRef>,
    }

    impl Run {
        fn table_label(table: ImportTable) -> &'static str {
            match table {
                ImportTable::Firearms => "firearms",
                ImportTable::Accessories => "accessories",
            }
        }

        fn land(
            &mut self,
            table: ImportTable,
            row: usize,
            result: Result<Imported, Fail>,
            record_imported: &dyn Fn(u64),
        ) -> Result<(), CommandError> {
            let imported = match result {
                Err(Fail::Fatal(e)) => return Err(e),
                Err(Fail::Row(message)) => {
                    self.row_errors.push(RowError { table, row, message });
                    return Ok(());
                }
                Ok(imported) => imported,
            };
            if let Some(uid) = &imported.uid {
                self.seen.insert(uid.clone(), (table, row));
            }
            report(
                imported.settled,
                table,
                row,
                &mut self.snapped_values,
                &mut self.derived_calibers,
            );
            let mounted_on = imported.mounted_on;
            match imported.landed {
                Landed::Created { record, warning } => {
                    self.imported_count += 1;
                    if table == ImportTable::Accessories {
                        self.imported_accessory_count += 1;
                    }
                    record_imported(self.imported_count as u64);
                    if let Some(message) = warning {
                        self.warnings.push(RowError { table, row, message });
                    }
                    if let Some(cell) = mounted_on {
                        self.mount_requests.push(MountRequest { table, row, record, cell });
                    }
                }
                Landed::Conflict {
                    existing,
                    input,
                    duplicate_allowed,
                    make,
                    model,
                    serial_number,
                    kind_name,
                } => {
                    let conflict_id =
                        format!("{}-{}-row-{row}", self.session_id, Self::table_label(table));
                    if let Some(uid) = imported.uid {
                        self.outcomes.insert(uid, existing);
                    }
                    self.conflicts.push(ImportConflict {
                        conflict_id: conflict_id.clone(),
                        table,
                        row,
                        existing_record: existing,
                        duplicate_allowed,
                        make,
                        model,
                        serial_number,
                        kind_name,
                        mounted: Vec::new(),
                    });
                    self.pending.push(PendingConflict {
                        conflict_id,
                        table,
                        row,
                        existing,
                        new_input: input,
                        mounted_on,
                    });
                }
            }
            Ok(())
        }
    }

    /// FR-022: reads and recognises every file and sheet before anything is
    /// saved, and stops with `VALIDATION_ERROR` naming the file (and the
    /// sheet) when a sheet is no table, two sheets hold the same table, or
    /// more than two files are given.
    fn read_files(
        files: &[ImportFile],
    ) -> Result<(Vec<RawImportRow>, Vec<RawAccessoryRow>), CommandError> {
        if files.is_empty() {
            return Err(CommandError::new("VALIDATION_ERROR", "Choose a file to import."));
        }
        if files.len() > 2 {
            return Err(CommandError::new(
                "VALIDATION_ERROR",
                "Pick one firearm table and at most one accessory table: at most two files.",
            ));
        }
        let mut tables: Vec<ReadTable> = Vec::new();
        for file in files {
            tables.extend(read_spreadsheet(&file.file_path, file.format)?);
        }
        let mut firearms: Option<(String, Vec<RawImportRow>)> = None;
        let mut accessories: Option<(String, Vec<RawAccessoryRow>)> = None;
        let same_table = |first: &str, second: &str, noun: &str| {
            CommandError::new(
                "VALIDATION_ERROR",
                format!(
                    "{first} and {second} both hold {noun}. \
                     Pick one firearm table and at most one accessory table."
                ),
            )
        };
        for table in tables {
            debug_assert!(matches!(
                table.rows.kind(),
                TableKind::Firearms | TableKind::Accessories
            ));
            match table.rows {
                TableRows::Firearms(rows) => {
                    if let Some((first, _)) = &firearms {
                        return Err(same_table(first, &table.source, "firearms"));
                    }
                    firearms = Some((table.source, rows));
                }
                TableRows::Accessories(rows) => {
                    if let Some((first, _)) = &accessories {
                        return Err(same_table(first, &table.source, "accessories"));
                    }
                    accessories = Some((table.source, rows));
                }
            }
        }
        Ok((
            firearms.map(|(_, rows)| rows).unwrap_or_default(),
            accessories.map(|(_, rows)| rows).unwrap_or_default(),
        ))
    }

    /// Each row is validated independently; a failing row is reported in
    /// `rowErrors` and does not block other rows (FR-020). A row matching
    /// an existing record, by identifier or, for a firearm, by `(make,
    /// model, serial_number)`, becomes a pending conflict (stored in
    /// `store`) rather than being silently applied.
    pub fn import_collection(
        conn: &Connection,
        files: &[ImportFile],
        store: &ImportSessionStore,
        on_progress: &mut dyn FnMut(usize, usize),
    ) -> Result<ImportResult, CommandError> {
        let never = || false;
        import_collection_stoppable(conn, files, store, on_progress, &never, &|_| {})
    }

    /// [`import_collection`], checking `is_cancelled` between rows and
    /// reporting each imported row to `record_imported`. Each row is saved
    /// on its own, so a stop keeps every row imported before it, and fails
    /// with `OPERATION_STOPPED` and their number (FR-037). The firearm
    /// table's rows come first, then the accessory table's, then the mounts
    /// (research.md §17, §18).
    pub fn import_collection_stoppable(
        conn: &Connection,
        files: &[ImportFile],
        store: &ImportSessionStore,
        on_progress: &mut dyn FnMut(usize, usize),
        is_cancelled: &dyn Fn() -> bool,
        record_imported: &dyn Fn(u64),
    ) -> Result<ImportResult, CommandError> {
        let (firearm_rows, accessory_rows) = read_files(files)?;
        let snapping = Snapping::new(conn, &firearm_rows, &accessory_rows)?;
        let total = firearm_rows.len() + accessory_rows.len();
        let session_id = format!("import-{}", chrono::Utc::now().timestamp_micros());

        let mut run = Run {
            session_id: session_id.clone(),
            imported_count: 0,
            imported_accessory_count: 0,
            row_errors: Vec::new(),
            conflicts: Vec::new(),
            pending: Vec::new(),
            warnings: Vec::new(),
            derived_calibers: Vec::new(),
            snapped_values: Vec::new(),
            seen: HashMap::new(),
            mount_requests: Vec::new(),
            outcomes: HashMap::new(),
        };
        let mut done = 0;
        let stopped = |run: &Run| {
            CommandError::operation_stopped(
                OperationKind::Import,
                Some(run.imported_count as u64),
                None,
            )
        };

        for (index, raw) in firearm_rows.iter().enumerate() {
            if is_cancelled() {
                return Err(stopped(&run));
            }
            let result = import_firearm_row(conn, &snapping, raw, &run.seen);
            run.land(ImportTable::Firearms, index + 1, result, record_imported)?;
            done += 1;
            on_progress(done, total);
        }
        for (index, raw) in accessory_rows.iter().enumerate() {
            if is_cancelled() {
                return Err(stopped(&run));
            }
            let result = import_accessory_row(conn, &snapping, raw, &run.seen);
            run.land(ImportTable::Accessories, index + 1, result, record_imported)?;
            done += 1;
            on_progress(done, total);
        }

        resolve_mounts(conn, &run.mount_requests, &run.outcomes, &mut run.warnings)?;
        list_mounted(conn, &mut run.conflicts, &run.pending)?;

        if !run.pending.is_empty() {
            store.pending.lock().expect("import session mutex poisoned").insert(
                session_id.clone(),
                ImportSession {
                    pending: std::mem::take(&mut run.pending),
                    outcomes: std::mem::take(&mut run.outcomes),
                },
            );
        }

        Ok(ImportResult {
            session_id,
            imported_count: run.imported_count,
            imported_accessory_count: run.imported_accessory_count,
            updated_count: 0,
            skipped_count: 0,
            row_errors: run.row_errors,
            conflicts: run.conflicts,
            warnings: run.warnings,
            derived_calibers: run.derived_calibers,
            snapped_values: run.snapped_values,
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
    /// record (keeping its identifier), `duplicate` inserts the imported row
    /// as a new record with a new identifier alongside it (only where
    /// FR-032 allows), `skip` (the default, including any conflict not
    /// covered by `resolutions` or `apply_to_remaining`) leaves the existing
    /// record and its mount untouched. A conflict the action can't be
    /// applied to is reported in `unresolved` and stays open in the session.
    /// A saved row's `mounted_on` is applied by the rules of research.md §18,
    /// after all are saved: a blank cell leaves an overwritten record
    /// unmounted.
    pub fn resolve_import_conflicts(
        conn: &Connection,
        store: &ImportSessionStore,
        session_id: &str,
        resolutions: &[ConflictResolution],
        apply_to_remaining: Option<&str>,
    ) -> Result<ResolveResult, CommandError> {
        let ImportSession { pending, outcomes } = store
            .pending
            .lock()
            .expect("import session mutex poisoned")
            .remove(session_id)
            .unwrap_or_else(|| ImportSession { pending: Vec::new(), outcomes: HashMap::new() });

        let explicit: HashMap<&str, &ConflictResolution> =
            resolutions.iter().map(|r| (r.conflict_id.as_str(), r)).collect();

        let mut resolved_count = 0;
        let mut unresolved = Vec::new();
        let mut still_open = Vec::new();
        let mut warnings = Vec::new();
        let mut mount_requests = Vec::new();
        for conflict in pending {
            let chosen = explicit.get(conflict.conflict_id.as_str());
            let action = chosen.map(|r| r.action.as_str()).or(apply_to_remaining).unwrap_or("skip");
            let with_mounted = chosen.map_or(&[][..], |r| &r.with_mounted[..]);
            // "skip", or an unrecognized action, leaves the existing record
            // untouched — never saved, so it never carries a warning.
            if action != "overwrite" && action != "duplicate" {
                resolved_count += 1;
                continue;
            }
            let overwrite = action == "overwrite";
            let outcome: Result<(RecordRef, Option<String>), CommandError> =
                match (&conflict.new_input, conflict.existing) {
                    (PendingInput::Firearm(input), RecordRef::Firearm(id)) => {
                        let saved = if overwrite {
                            replace(
                                conn,
                                conflict.existing,
                                &conflict.new_input,
                                with_mounted,
                                || firearm_ops::update_firearm(conn, id, input, true),
                            )
                        } else {
                            firearm_ops::create_firearm(conn, input, true, None)
                        };
                        saved.and_then(|saved| {
                            let message = original_marks_warning(conn, &saved)?;
                            Ok((RecordRef::Firearm(saved.id), message))
                        })
                    }
                    (PendingInput::Accessory(input), RecordRef::Accessory(id)) => {
                        let saved = if overwrite {
                            replace(
                                conn,
                                conflict.existing,
                                &conflict.new_input,
                                with_mounted,
                                || accessory_ops::update_accessory(conn, id, input),
                            )
                        } else {
                            accessory_ops::create_accessory(conn, input, None)
                        };
                        saved.map(|saved| (RecordRef::Accessory(saved.id), None))
                    }
                    // A conflict's input and its record are of one kind.
                    _ => Err(CommandError::new("INTERNAL_ERROR", "An unexpected error occurred.")),
                };
            match outcome {
                Ok((saved, message)) => {
                    resolved_count += 1;
                    if let Some(message) = message {
                        warnings.push(RowError {
                            table: conflict.table,
                            row: conflict.row,
                            message,
                        });
                    }
                    if let Some(cell) = &conflict.mounted_on {
                        mount_requests.push(MountRequest {
                            table: conflict.table,
                            row: conflict.row,
                            record: saved,
                            cell: cell.clone(),
                        });
                    }
                }
                Err(e) if e.code == "VALIDATION_ERROR" => {
                    unresolved.push(RowError {
                        table: conflict.table,
                        row: conflict.row,
                        message: row_message(&e),
                    });
                    still_open.push(conflict);
                }
                Err(e) => return Err(e),
            }
        }

        resolve_mounts(conn, &mount_requests, &outcomes, &mut warnings)?;

        if !still_open.is_empty() {
            store
                .pending
                .lock()
                .expect("import session mutex poisoned")
                .insert(session_id.to_string(), ImportSession { pending: still_open, outcomes });
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

/// Input for `get_export_scope` (contracts/tauri-commands.md).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetExportScopeInput {
    #[serde(default = "default_scope")]
    pub scope: String,
    pub filter: Option<ListFirearmsInput>,
}

#[tauri::command]
pub async fn get_export_scope(
    input: GetExportScopeInput,
    session: State<'_, Session>,
) -> Result<ExportScope, CommandError> {
    session.read(|conn| ops::get_export_scope(conn, &input.scope, input.filter.as_ref()))
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
        let records = ops::export_records(conn, &input.scope, input.filter.as_ref())?;

        let base_name = format!("hoplodex-export-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
        ops::export_collection_stoppable(
            conn,
            Path::new(&input.destination_folder),
            &base_name,
            format,
            &records,
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
pub struct ImportFileInput {
    pub file_path: String,
    pub format: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCollectionInput {
    /// One file, or two: the firearm table and the accessory table
    /// (FR-022).
    pub files: Vec<ImportFileInput>,
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
        let files = input
            .files
            .iter()
            .map(|file| {
                Ok(ImportFile {
                    file_path: PathBuf::from(&file.file_path),
                    format: parse_format(&file.format)?,
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        ops::import_collection_stoppable(
            conn,
            &files,
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
