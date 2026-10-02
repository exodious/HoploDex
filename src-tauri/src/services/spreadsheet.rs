//! CSV/XLSX export (`rust_xlsxwriter`/`csv`) and import (`calamine`/`csv`)
//! per contracts/spreadsheet-format.md. Kept in the Rust backend
//! (research.md §6) so the business logic that must be tested against a
//! real SQLCipher DB — matching, validation, conflict resolution — never
//! crosses the IPC boundary mid-computation.

use std::borrow::Cow;
use std::path::Path;

use calamine::Reader;
use rust_xlsxwriter::Workbook;

use crate::commands::CommandError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadsheetFormat {
    Csv,
    Xlsx,
}

impl SpreadsheetFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
        }
    }
}

/// Column order per contracts/spreadsheet-format.md. `photo_filenames` is
/// export-only (ignored on import, FR-019). specs/002-firearm-identification
/// adds seven columns after `condition` and before `photo_filenames`
/// (contracts/spreadsheet-format.md's "New columns");
/// specs/004-cartridges-action-types adds `cartridge` and `action_type`
/// directly after `caliber` (FR-022); specs/005-regulated-item-types adds
/// four registration columns after `original_serial_number` (FR-019);
/// specs/006-accessory-links puts `record_id` first and `mounted_on` before
/// `photo_filenames`. Import finds columns by header, not by position, so
/// this order is the export's only.
pub const FIREARM_COLUMNS: &[&str] = &[
    "record_id",
    "make",
    "model",
    "nickname",
    "serial_number",
    "no_serial_attested",
    "caliber",
    "cartridge",
    "action_type",
    "firearm_type",
    "notes",
    "accessories",
    "status",
    "estimated_value",
    "acquisition_source",
    "acquisition_date",
    "acquisition_price",
    "disposition_type",
    "disposition_recipient",
    "disposition_date",
    "disposition_price",
    "insurance_policy_name",
    "scheduled_coverage_amount",
    "barrel_length_in",
    "overall_length_in",
    "weight_oz",
    "capacity",
    "finish",
    "condition",
    "origin",
    "year_of_manufacture",
    "country_of_manufacture",
    "importer_name",
    "original_make",
    "original_model",
    "original_serial_number",
    "registered_as",
    "registration_form",
    "registration_approved",
    "registered_to",
    "mounted_on",
    "photo_filenames",
];

/// The accessory table's columns, in export order
/// (specs/006-accessory-links contracts/spreadsheet-format.md "Accessory
/// table: columns"). Read by header on import, like the firearm table's.
pub const ACCESSORY_COLUMNS: &[&str] = &[
    "record_id",
    "kind",
    "make",
    "model",
    "serial_number",
    "caliber",
    "cartridge",
    "notes",
    "status",
    "estimated_value",
    "acquisition_source",
    "acquisition_date",
    "acquisition_price",
    "disposition_type",
    "disposition_recipient",
    "disposition_date",
    "disposition_price",
    "insurance_policy_name",
    "scheduled_coverage_amount",
    "mounted_on",
    "photo_filenames",
];

/// One exported row — every cell already formatted as display text (blank
/// string for an absent value), per contracts/spreadsheet-format.md.
#[derive(Debug, Clone, Default)]
pub struct FirearmExportRow {
    /// specs/006-accessory-links FR-019: the record's identifier, never
    /// blank.
    pub record_id: String,
    pub make: String,
    pub model: String,
    pub nickname: String,
    pub serial_number: String,
    pub no_serial_attested: String,
    pub caliber: String,
    /// specs/004-cartridges-action-types FR-022: the recorded cartridge, or
    /// blank.
    pub cartridge: String,
    /// The action's name (`Bolt action`), or blank.
    pub action_type: String,
    pub firearm_type: String,
    pub notes: String,
    pub accessories: String,
    pub status: String,
    pub estimated_value: String,
    pub acquisition_source: String,
    pub acquisition_date: String,
    pub acquisition_price: String,
    pub disposition_type: String,
    pub disposition_recipient: String,
    pub disposition_date: String,
    pub disposition_price: String,
    pub insurance_policy_name: String,
    pub scheduled_coverage_amount: String,
    pub barrel_length_in: String,
    pub overall_length_in: String,
    pub weight_oz: String,
    pub capacity: String,
    pub finish: String,
    pub condition: String,
    /// specs/002-firearm-identification: the origin's display label
    /// (`Domestic`/`Imported`/`Re-imported`), or blank for not specified.
    pub origin: String,
    pub year_of_manufacture: String,
    /// Always blank for a Re-imported firearm: the United States is
    /// displayed, never stored (data-model.md).
    pub country_of_manufacture: String,
    pub importer_name: String,
    pub original_make: String,
    pub original_model: String,
    pub original_serial_number: String,
    /// specs/005-regulated-item-types FR-019: the classification's name, the
    /// form, the approved date (`YYYY-MM-DD`) and "Registered to", each
    /// blank when there is none.
    pub registered_as: String,
    pub registration_form: String,
    pub registration_approved: String,
    pub registered_to: String,
    /// The direct host's identifier, a firearm's or an accessory's, or blank
    /// (FR-023).
    pub mounted_on: String,
    pub photo_filenames: String,
}

impl FirearmExportRow {
    fn as_fields(&self) -> [&str; 42] {
        [
            &self.record_id,
            &self.make,
            &self.model,
            &self.nickname,
            &self.serial_number,
            &self.no_serial_attested,
            &self.caliber,
            &self.cartridge,
            &self.action_type,
            &self.firearm_type,
            &self.notes,
            &self.accessories,
            &self.status,
            &self.estimated_value,
            &self.acquisition_source,
            &self.acquisition_date,
            &self.acquisition_price,
            &self.disposition_type,
            &self.disposition_recipient,
            &self.disposition_date,
            &self.disposition_price,
            &self.insurance_policy_name,
            &self.scheduled_coverage_amount,
            &self.barrel_length_in,
            &self.overall_length_in,
            &self.weight_oz,
            &self.capacity,
            &self.finish,
            &self.condition,
            &self.origin,
            &self.year_of_manufacture,
            &self.country_of_manufacture,
            &self.importer_name,
            &self.original_make,
            &self.original_model,
            &self.original_serial_number,
            &self.registered_as,
            &self.registration_form,
            &self.registration_approved,
            &self.registered_to,
            &self.mounted_on,
            &self.photo_filenames,
        ]
    }
}

/// One exported accessory row, every cell display text
/// (contracts/spreadsheet-format.md "Accessory table: columns").
#[derive(Debug, Clone, Default)]
pub struct AccessoryExportRow {
    pub record_id: String,
    /// The kind's name, e.g. `Optic`.
    pub kind: String,
    pub make: String,
    pub model: String,
    pub serial_number: String,
    pub caliber: String,
    pub cartridge: String,
    pub notes: String,
    pub status: String,
    pub estimated_value: String,
    pub acquisition_source: String,
    pub acquisition_date: String,
    pub acquisition_price: String,
    pub disposition_type: String,
    pub disposition_recipient: String,
    pub disposition_date: String,
    pub disposition_price: String,
    pub insurance_policy_name: String,
    pub scheduled_coverage_amount: String,
    pub mounted_on: String,
    pub photo_filenames: String,
}

impl AccessoryExportRow {
    fn as_fields(&self) -> [&str; 21] {
        [
            &self.record_id,
            &self.kind,
            &self.make,
            &self.model,
            &self.serial_number,
            &self.caliber,
            &self.cartridge,
            &self.notes,
            &self.status,
            &self.estimated_value,
            &self.acquisition_source,
            &self.acquisition_date,
            &self.acquisition_price,
            &self.disposition_type,
            &self.disposition_recipient,
            &self.disposition_date,
            &self.disposition_price,
            &self.insurance_policy_name,
            &self.scheduled_coverage_amount,
            &self.mounted_on,
            &self.photo_filenames,
        ]
    }
}

/// One raw imported row — every cell as the string found in the source
/// file (or `None` if the column was blank/absent), before any validation
/// or type coercion. `photo_filenames` is deliberately omitted: ignored on
/// import per FR-019.
#[derive(Debug, Clone, Default)]
pub struct RawImportRow {
    /// specs/006-accessory-links FR-022: the identifier cell, as found
    /// (trimmed, `None` when blank or the column is absent).
    pub record_id: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub nickname: Option<String>,
    pub serial_number: Option<String>,
    pub no_serial_attested: Option<String>,
    pub caliber: Option<String>,
    pub cartridge: Option<String>,
    pub action_type: Option<String>,
    pub firearm_type: Option<String>,
    pub notes: Option<String>,
    pub accessories: Option<String>,
    pub status: Option<String>,
    pub estimated_value: Option<String>,
    pub acquisition_source: Option<String>,
    pub acquisition_date: Option<String>,
    pub acquisition_price: Option<String>,
    pub disposition_type: Option<String>,
    pub disposition_recipient: Option<String>,
    pub disposition_date: Option<String>,
    pub disposition_price: Option<String>,
    pub insurance_policy_name: Option<String>,
    pub scheduled_coverage_amount: Option<String>,
    pub barrel_length_in: Option<String>,
    pub overall_length_in: Option<String>,
    pub weight_oz: Option<String>,
    pub capacity: Option<String>,
    pub finish: Option<String>,
    pub condition: Option<String>,
    pub origin: Option<String>,
    pub year_of_manufacture: Option<String>,
    pub country_of_manufacture: Option<String>,
    pub importer_name: Option<String>,
    pub original_make: Option<String>,
    pub original_model: Option<String>,
    pub original_serial_number: Option<String>,
    pub registered_as: Option<String>,
    pub registration_form: Option<String>,
    pub registration_approved: Option<String>,
    pub registered_to: Option<String>,
    /// The host's identifier cell (FR-023), as found.
    pub mounted_on: Option<String>,
}

/// One raw accessory row, as [`RawImportRow`] is one raw firearm row.
#[derive(Debug, Clone, Default)]
pub struct RawAccessoryRow {
    pub record_id: Option<String>,
    pub kind: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub caliber: Option<String>,
    pub cartridge: Option<String>,
    pub notes: Option<String>,
    pub status: Option<String>,
    pub estimated_value: Option<String>,
    pub acquisition_source: Option<String>,
    pub acquisition_date: Option<String>,
    pub acquisition_price: Option<String>,
    pub disposition_type: Option<String>,
    pub disposition_recipient: Option<String>,
    pub disposition_date: Option<String>,
    pub disposition_price: Option<String>,
    pub insurance_policy_name: Option<String>,
    pub scheduled_coverage_amount: Option<String>,
    pub mounted_on: Option<String>,
}

fn non_blank(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

/// Which column of a table each column of the file is, by header
/// (research.md §12): trimmed and in any letter case. A header that is not a
/// known column maps to `None` and its cells are ignored; a known column the
/// file does not have reads as blank on every row.
struct HeaderMap {
    columns: &'static [&'static str],
    /// For each column of the file, its index in `columns`.
    map: Vec<Option<usize>>,
}

/// A header cell as compared: a byte-order mark starts some spreadsheets'
/// first header.
fn header_name(header: &str) -> String {
    header.trim_start_matches('\u{feff}').trim().to_lowercase()
}

impl HeaderMap {
    /// Fails, naming the column, when two headers are the same known column
    /// (contracts/spreadsheet-format.md "Columns are read by header"), and
    /// naming `source` (the file, and the sheet of a workbook that holds
    /// several), since one import can take two files (specs/006 FR-022).
    fn new<'a>(
        columns: &'static [&'static str],
        headers: impl Iterator<Item = &'a str>,
        source: &str,
    ) -> Result<Self, CommandError> {
        let mut seen = vec![false; columns.len()];
        let mut map = Vec::new();
        for header in headers {
            let header = header_name(header);
            let column = columns.iter().position(|known| *known == header);
            if let Some(column) = column
                && std::mem::replace(&mut seen[column], true)
            {
                return Err(CommandError::new(
                    "VALIDATION_ERROR",
                    format!("{source}: the header has two \"{}\" columns.", columns[column]),
                ));
            }
            map.push(column);
        }
        Ok(Self { columns, map })
    }

    /// The row's cells by position in `columns`, blank cells as `None`.
    fn cells(&self, cells: &[String]) -> Cells {
        let mut by_column: Vec<Option<String>> = vec![None; self.columns.len()];
        for (cell, column) in cells.iter().zip(&self.map) {
            if let Some(column) = column {
                by_column[*column] = non_blank(cell);
            }
        }
        Cells { columns: self.columns, by_column }
    }
}

/// One row's cells, taken out by column name.
struct Cells {
    columns: &'static [&'static str],
    by_column: Vec<Option<String>>,
}

impl Cells {
    fn take(&mut self, name: &str) -> Option<String> {
        let column = self.columns.iter().position(|known| *known == name).expect("a known column");
        self.by_column[column].take()
    }
}

impl RawImportRow {
    fn from_cells(mut cells: Cells) -> Self {
        let mut take = |name: &str| cells.take(name);
        RawImportRow {
            record_id: take("record_id"),
            make: take("make"),
            model: take("model"),
            nickname: take("nickname"),
            serial_number: take("serial_number"),
            no_serial_attested: take("no_serial_attested"),
            caliber: take("caliber"),
            cartridge: take("cartridge"),
            action_type: take("action_type"),
            firearm_type: take("firearm_type"),
            notes: take("notes"),
            accessories: take("accessories"),
            status: take("status"),
            estimated_value: take("estimated_value"),
            acquisition_source: take("acquisition_source"),
            acquisition_date: take("acquisition_date"),
            acquisition_price: take("acquisition_price"),
            disposition_type: take("disposition_type"),
            disposition_recipient: take("disposition_recipient"),
            disposition_date: take("disposition_date"),
            disposition_price: take("disposition_price"),
            insurance_policy_name: take("insurance_policy_name"),
            scheduled_coverage_amount: take("scheduled_coverage_amount"),
            barrel_length_in: take("barrel_length_in"),
            overall_length_in: take("overall_length_in"),
            weight_oz: take("weight_oz"),
            capacity: take("capacity"),
            finish: take("finish"),
            condition: take("condition"),
            origin: take("origin"),
            year_of_manufacture: take("year_of_manufacture"),
            country_of_manufacture: take("country_of_manufacture"),
            importer_name: take("importer_name"),
            original_make: take("original_make"),
            original_model: take("original_model"),
            original_serial_number: take("original_serial_number"),
            registered_as: take("registered_as"),
            registration_form: take("registration_form"),
            registration_approved: take("registration_approved"),
            registered_to: take("registered_to"),
            mounted_on: take("mounted_on"),
        }
    }
}

impl RawAccessoryRow {
    fn from_cells(mut cells: Cells) -> Self {
        let mut take = |name: &str| cells.take(name);
        RawAccessoryRow {
            record_id: take("record_id"),
            kind: take("kind"),
            make: take("make"),
            model: take("model"),
            serial_number: take("serial_number"),
            caliber: take("caliber"),
            cartridge: take("cartridge"),
            notes: take("notes"),
            status: take("status"),
            estimated_value: take("estimated_value"),
            acquisition_source: take("acquisition_source"),
            acquisition_date: take("acquisition_date"),
            acquisition_price: take("acquisition_price"),
            disposition_type: take("disposition_type"),
            disposition_recipient: take("disposition_recipient"),
            disposition_date: take("disposition_date"),
            disposition_price: take("disposition_price"),
            insurance_policy_name: take("insurance_policy_name"),
            scheduled_coverage_amount: take("scheduled_coverage_amount"),
            mounted_on: take("mounted_on"),
        }
    }
}

/// Formats a whole-dollar amount as plain digits (`1250`: no `$`, no
/// thousands separator, no decimals), or `""` when absent — the
/// spreadsheet-format.md "Amounts" convention (FR-037).
pub fn dollars_to_string(dollars: Option<i64>) -> String {
    dollars.map(|d| d.to_string()).unwrap_or_default()
}

/// Parses an amount cell into whole dollars (FR-037). A leading `$`,
/// thousands commas and whitespace are dropped, and a zero fraction
/// (`450.00`) is accepted; a value with non-zero cents, a sign, or any other
/// text is an `Err` naming `column`, never rounded. A blank cell is `None`.
pub fn parse_whole_dollars(column: &str, value: &Option<String>) -> Result<Option<i64>, String> {
    let Some(raw) = value.as_deref() else {
        return Ok(None);
    };
    let cleaned: String =
        raw.chars().filter(|c| !c.is_whitespace() && *c != '$' && *c != ',').collect();
    if cleaned.is_empty() && raw.trim().is_empty() {
        return Ok(None);
    }

    let (whole, fraction) = match cleaned.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (cleaned.as_str(), None),
    };
    let digits_only = |text: &str| !text.is_empty() && text.chars().all(|c| c.is_ascii_digit());
    if !digits_only(whole) || fraction.is_some_and(|f| !digits_only(f)) {
        return Err(format!("{column}: {raw:?} is not a whole-dollar amount"));
    }
    if fraction.is_some_and(|f| f.chars().any(|c| c != '0')) {
        return Err(format!("{column}: whole dollars only, but {raw:?} has cents"));
    }
    whole
        .parse::<i64>()
        .map(Some)
        .map_err(|_| format!("{column}: {raw:?} is too large to be an amount"))
}

/// Formats a stored scaled integer (`1625` hundredths, `405` tenths) as a
/// plain decimal with no trailing zeros (`16.25`, `18`, `40.5`), or `""` when
/// absent — the spreadsheet-format.md "Physical details" convention (FR-039).
pub fn scaled_to_string(value: Option<i64>, places: u32) -> String {
    let Some(value) = value else {
        return String::new();
    };
    let scale = 10_i64.pow(places);
    let (whole, fraction) = (value / scale, value % scale);
    if fraction == 0 {
        return whole.to_string();
    }
    let fraction = format!("{fraction:0width$}", width = places as usize);
    format!("{whole}.{}", fraction.trim_end_matches('0'))
}

/// Parses a physical-detail cell into a positive integer scaled by
/// `10^places` (FR-039): `16.25` with 2 places is 1625. More precision than
/// `places` is rounded half up to the nearest storable unit (`16.255` is
/// 1626, `40.54` with 1 place is 405); a sign, zero (including a value that
/// rounds to zero) or any other text is an `Err` naming `column`. A blank
/// cell is `None`. With 0 places it parses a whole number (a capacity),
/// where a fraction is an `Err`, since half a round means nothing.
pub fn parse_scaled_decimal(
    column: &str,
    value: &Option<String>,
    places: u32,
) -> Result<Option<i64>, String> {
    let Some(raw) = value.as_deref().map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Ok(None);
    };
    let (whole, fraction) = match raw.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (raw, ""),
    };
    let digits = |text: &str| text.chars().all(|c| c.is_ascii_digit());
    let well_formed = !whole.is_empty()
        && digits(whole)
        && digits(fraction)
        && (!raw.contains('.') || !fraction.is_empty());
    if !well_formed {
        return Err(format!("{column}: {raw:?} is not a number"));
    }
    let (kept, beyond) = fraction.split_at(fraction.len().min(places as usize));
    if places == 0 && beyond.chars().any(|c| c != '0') {
        return Err(format!("{column}: {raw:?} must be a whole number"));
    }
    let padded = format!("{kept:0<width$}", width = places as usize);
    let too_large = || format!("{column}: {raw:?} is too large");
    let truncated = format!("{whole}{padded}").parse::<i64>().map_err(|_| too_large())?;
    let round_up = beyond.starts_with(|c: char| c >= '5');
    let scaled = if round_up { truncated.checked_add(1).ok_or_else(too_large)? } else { truncated };
    if scaled == 0 {
        return Err(format!("{column}: {raw:?} must be greater than 0"));
    }
    Ok(Some(scaled))
}

fn export_error(what: &str, e: impl std::fmt::Display) -> CommandError {
    CommandError::new("INTERNAL_ERROR", format!("{what}: {e}"))
}

/// The characters that make a spreadsheet program read a cell as a formula.
const FORMULA_TRIGGERS: [char; 6] = ['=', '+', '-', '@', '\t', '\r'];

/// Protects one CSV text cell against formula injection (Constitution V,
/// contracts/spreadsheet-format.md): a cell starting with a formula trigger
/// gets a leading `'`. `unprotect_csv_cell` is its exact inverse for every
/// cell this writes.
fn protect_csv_cell(cell: &str) -> Cow<'_, str> {
    if cell.starts_with(FORMULA_TRIGGERS) {
        Cow::Owned(format!("'{cell}"))
    } else {
        Cow::Borrowed(cell)
    }
}

/// Removes the one leading `'` that `protect_csv_cell` adds: a `'` followed by
/// a formula trigger. A cell that genuinely starts with `'` and a trigger
/// character loses its `'` too, the one accepted ambiguity.
fn unprotect_csv_cell(cell: &str) -> &str {
    match cell.strip_prefix('\'') {
        Some(rest) if rest.starts_with(FORMULA_TRIGGERS) => rest,
        _ => cell,
    }
}

fn write_csv_table<const N: usize>(
    path: &Path,
    columns: &[&str],
    rows: impl Iterator<Item = [String; N]>,
) -> Result<(), CommandError> {
    let mut writer = csv::Writer::from_path(path)
        .map_err(|e| export_error("Could not create the export file", e))?;
    writer.write_record(columns).map_err(|e| export_error("Failed writing export header", e))?;
    for row in rows {
        writer
            .write_record(row.iter().map(|cell| protect_csv_cell(cell).into_owned()))
            .map_err(|e| export_error("Failed writing export row", e))?;
    }
    writer.flush().map_err(|e| export_error("Failed saving the export file", e))
}

/// Writes the firearm table as a CSV file.
pub fn write_firearm_csv(path: &Path, rows: &[FirearmExportRow]) -> Result<(), CommandError> {
    write_csv_table(
        path,
        FIREARM_COLUMNS,
        rows.iter().map(|row| row.as_fields().map(str::to_owned)),
    )
}

/// Writes the accessory table as a CSV file.
pub fn write_accessory_csv(path: &Path, rows: &[AccessoryExportRow]) -> Result<(), CommandError> {
    write_csv_table(
        path,
        ACCESSORY_COLUMNS,
        rows.iter().map(|row| row.as_fields().map(str::to_owned)),
    )
}

fn write_sheet<'a>(
    workbook: &mut Workbook,
    name: &str,
    columns: &[&str],
    rows: impl Iterator<Item = Vec<&'a str>>,
) -> Result<(), CommandError> {
    let sheet = workbook.add_worksheet();
    sheet.set_name(name).map_err(|e| export_error("Failed naming a sheet", e))?;
    for (col, header) in columns.iter().enumerate() {
        sheet
            .write_string(0, col as u16, *header)
            .map_err(|e| export_error("Failed writing export header", e))?;
    }
    for (row_index, row) in rows.enumerate() {
        for (col, value) in row.iter().enumerate() {
            sheet
                .write_string((row_index + 1) as u32, col as u16, *value)
                .map_err(|e| export_error("Failed writing export row", e))?;
        }
    }
    Ok(())
}

/// Writes one workbook: the "Firearms" sheet, and the "Accessories" sheet
/// when `accessories` is given (contracts/spreadsheet-format.md "Two
/// tables").
pub fn write_workbook(
    path: &Path,
    firearms: &[FirearmExportRow],
    accessories: Option<&[AccessoryExportRow]>,
) -> Result<(), CommandError> {
    let mut workbook = Workbook::new();
    write_sheet(
        &mut workbook,
        "Firearms",
        FIREARM_COLUMNS,
        firearms.iter().map(|row| row.as_fields().to_vec()),
    )?;
    if let Some(accessories) = accessories {
        write_sheet(
            &mut workbook,
            "Accessories",
            ACCESSORY_COLUMNS,
            accessories.iter().map(|row| row.as_fields().to_vec()),
        )?;
    }
    workbook.save(path).map_err(|e| export_error("Failed saving the export file", e))
}

/// The rows of one recognised table.
#[derive(Debug)]
pub enum TableRows {
    Firearms(Vec<RawImportRow>),
    Accessories(Vec<RawAccessoryRow>),
}

impl TableRows {
    pub fn kind(&self) -> TableKind {
        match self {
            Self::Firearms(_) => TableKind::Firearms,
            Self::Accessories(_) => TableKind::Accessories,
        }
    }
}

/// Which table a sheet is (contracts/spreadsheet-format.md "Recognising a
/// table on import").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableKind {
    Firearms,
    Accessories,
}

/// A recognised table and where it was read from, as the stop messages name
/// it: the file, and the sheet of a workbook that holds several.
#[derive(Debug)]
pub struct ReadTable {
    pub source: String,
    pub rows: TableRows,
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// FR-022: the header row decides. `firearm_type` without `kind` is the
/// firearm table, `kind` without `firearm_type` the accessory table; both or
/// neither is not a table, and stops the import naming `source`.
fn recognise(headers: &[String], source: &str) -> Result<TableKind, CommandError> {
    let names: Vec<String> = headers.iter().map(|h| header_name(h)).collect();
    let has = |wanted: &str| names.iter().any(|name| name == wanted);
    match (has("firearm_type"), has("kind")) {
        (true, false) => Ok(TableKind::Firearms),
        (false, true) => Ok(TableKind::Accessories),
        _ => Err(CommandError::new(
            "VALIDATION_ERROR",
            format!(
                "{source}: this isn't a HoploDex firearm or accessory table. \
                 Its header needs a firearm_type or a kind column."
            ),
        )),
    }
}

/// Reads and recognises every non-blank sheet of the file, without saving
/// anything. A sheet with no header row is ignored.
pub fn read_spreadsheet(
    path: &Path,
    format: SpreadsheetFormat,
) -> Result<Vec<ReadTable>, CommandError> {
    match format {
        SpreadsheetFormat::Csv => read_csv(path),
        SpreadsheetFormat::Xlsx => read_xlsx(path),
    }
}

/// One sheet's header and data rows, as text.
fn read_table(
    headers: &[String],
    data: impl Iterator<Item = Vec<String>>,
    source: String,
) -> Result<ReadTable, CommandError> {
    let header_refs = || headers.iter().map(String::as_str);
    let rows = match recognise(headers, &source)? {
        TableKind::Firearms => {
            let map = HeaderMap::new(FIREARM_COLUMNS, header_refs(), &source)?;
            TableRows::Firearms(
                data.map(|cells| RawImportRow::from_cells(map.cells(&cells))).collect(),
            )
        }
        TableKind::Accessories => {
            let map = HeaderMap::new(ACCESSORY_COLUMNS, header_refs(), &source)?;
            TableRows::Accessories(
                data.map(|cells| RawAccessoryRow::from_cells(map.cells(&cells))).collect(),
            )
        }
    };
    Ok(ReadTable { source, rows })
}

fn is_blank_header(headers: &[String]) -> bool {
    headers.iter().all(|h| h.trim().is_empty())
}

fn read_csv(path: &Path) -> Result<Vec<ReadTable>, CommandError> {
    let source = file_name(path);
    let unreadable = |e: csv::Error| {
        CommandError::new("VALIDATION_ERROR", format!("{source}: could not read the file: {e}"))
    };
    let mut reader =
        csv::ReaderBuilder::new().has_headers(true).from_path(path).map_err(unreadable)?;
    let headers: Vec<String> =
        reader.headers().map_err(unreadable)?.iter().map(str::to_string).collect();
    if is_blank_header(&headers) {
        return Ok(Vec::new());
    }
    let mut data = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| {
            CommandError::new("VALIDATION_ERROR", format!("{source}: could not parse a row: {e}"))
        })?;
        data.push(record.iter().map(|cell| unprotect_csv_cell(cell).to_string()).collect());
    }
    Ok(vec![read_table(&headers, data.into_iter(), source)?])
}

fn read_xlsx(path: &Path) -> Result<Vec<ReadTable>, CommandError> {
    let file = file_name(path);
    let unreadable = |e: &dyn std::fmt::Display| {
        CommandError::new("VALIDATION_ERROR", format!("{file}: could not read the file: {e}"))
    };
    let mut workbook: calamine::Xlsx<_> =
        calamine::open_workbook(path).map_err(|e| unreadable(&e))?;
    // Every sheet's text, blank sheets dropped, before any is recognised.
    let mut sheets: Vec<(String, Vec<Vec<String>>)> = Vec::new();
    for name in workbook.sheet_names() {
        let range = workbook.worksheet_range(&name).map_err(|e| unreadable(&e))?;
        let mut rows = range.rows().map(|row| row.iter().map(|c| c.to_string()).collect());
        let Some(headers) = rows.next().filter(|h: &Vec<String>| !is_blank_header(h)) else {
            continue;
        };
        let mut all = vec![headers];
        all.extend(rows);
        sheets.push((name, all));
    }
    let several = sheets.len() > 1;
    sheets
        .into_iter()
        .map(|(name, mut rows)| {
            let headers = rows.remove(0);
            let source = if several { format!("{file}, sheet \"{name}\"") } else { file.clone() };
            read_table(&headers, rows.into_iter(), source)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIGGERS: [&str; 6] = ["=", "+", "-", "@", "\t", "\r"];

    #[test]
    fn a_cell_starting_with_each_trigger_character_gets_a_leading_apostrophe() {
        for trigger in TRIGGERS {
            let cell = format!("{trigger}SUM(A1)");
            assert_eq!(protect_csv_cell(&cell), format!("'{cell}"), "{trigger:?}");
        }
    }

    #[test]
    fn other_cells_are_written_as_they_are() {
        for cell in ["", "plain", "a=b", "x-1", "'quoted", "'", "1-2", " =lead", "2024-01-02"] {
            assert_eq!(protect_csv_cell(cell), cell, "{cell:?}");
        }
    }

    #[test]
    fn reading_removes_one_apostrophe_only_before_a_trigger_character() {
        for trigger in TRIGGERS {
            assert_eq!(unprotect_csv_cell(&format!("'{trigger}x")), format!("{trigger}x"));
            // Only one apostrophe is removed.
            assert_eq!(unprotect_csv_cell(&format!("''{trigger}x")), format!("''{trigger}x"));
        }
        for cell in ["'quoted", "'", "''", "plain", "", "'a=b"] {
            assert_eq!(unprotect_csv_cell(cell), cell, "{cell:?}");
        }
    }

    #[test]
    fn every_cell_survives_protect_then_unprotect_unless_it_starts_with_an_apostrophe_and_a_trigger()
     {
        for cell in ["=1", "+1", "-1", "@a", "\tx", "\rx", "'x", "x", "", "'", "a'=", "-"] {
            assert_eq!(unprotect_csv_cell(&protect_csv_cell(cell)), cell, "{cell:?}");
        }
        // The one accepted ambiguity: the apostrophe is read as protection.
        assert_eq!(unprotect_csv_cell(&protect_csv_cell("'=1")), "=1");
    }

    #[test]
    fn the_csv_file_holds_the_protected_text_and_reading_returns_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.csv");
        let rows =
            vec![["=1+1".to_owned(), "\tTab".to_owned(), "\rCR".to_owned(), "ok".to_owned()]];
        write_csv_table(&path, &["a", "b", "c", "d"], rows.into_iter()).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert_eq!(raw, "a,b,c,d\n'=1+1,'\tTab,\"'\rCR\",ok\n");

        let mut reader = csv::Reader::from_path(&path).unwrap();
        let record = reader.records().next().unwrap().unwrap();
        let read: Vec<&str> = record.iter().map(unprotect_csv_cell).collect();
        assert_eq!(read, ["=1+1", "\tTab", "\rCR", "ok"]);
    }
}
