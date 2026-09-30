//! CSV/XLSX export (`rust_xlsxwriter`/`csv`) and import (`calamine`/`csv`)
//! per contracts/spreadsheet-format.md. Kept in the Rust backend
//! (research.md §6) so the business logic that must be tested against a
//! real SQLCipher DB — matching, validation, conflict resolution — never
//! crosses the IPC boundary mid-computation.

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
/// directly after `caliber` (FR-022). Import finds columns by header, not by
/// position, so this order is the export's only.
pub const COLUMNS: &[&str] = &[
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
    "photo_filenames",
];

/// One exported row — every cell already formatted as display text (blank
/// string for an absent value), per contracts/spreadsheet-format.md.
#[derive(Debug, Clone, Default)]
pub struct FirearmExportRow {
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
    pub photo_filenames: String,
}

impl FirearmExportRow {
    fn as_fields(&self) -> [&str; 36] {
        [
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
}

fn non_blank(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

/// Which `COLUMNS` entry each column of the file is, by header (research.md
/// §12): trimmed and in any letter case. A header that is not in `COLUMNS`
/// maps to `None` and its cells are ignored; a `COLUMNS` entry the file does
/// not have reads as blank on every row.
struct HeaderMap(Vec<Option<usize>>);

impl HeaderMap {
    /// Fails, naming the column, when two headers are the same known column
    /// (contracts/spreadsheet-format.md "Columns are read by header").
    fn from_headers<'a>(headers: impl Iterator<Item = &'a str>) -> Result<Self, CommandError> {
        let mut seen = [false; COLUMNS.len()];
        let mut map = Vec::new();
        for (index, header) in headers.enumerate() {
            // A byte-order mark starts some spreadsheets' first header.
            let header = header.trim_start_matches('\u{feff}').trim().to_lowercase();
            let column = COLUMNS.iter().position(|known| *known == header);
            if let Some(column) = column
                && std::mem::replace(&mut seen[column], true)
            {
                return Err(CommandError::new(
                    "VALIDATION_ERROR",
                    format!("The import file has two \"{}\" columns.", COLUMNS[column]),
                ));
            }
            debug_assert_eq!(map.len(), index);
            map.push(column);
        }
        Ok(Self(map))
    }

    fn row(&self, cells: &[String]) -> RawImportRow {
        let mut by_column: [Option<String>; COLUMNS.len()] = std::array::from_fn(|_| None);
        for (cell, column) in cells.iter().zip(&self.0) {
            if let Some(column) = column {
                by_column[*column] = non_blank(cell);
            }
        }
        let mut take = |name: &str| {
            let column = COLUMNS.iter().position(|known| *known == name).expect("a known column");
            by_column[column].take()
        };
        RawImportRow {
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

pub fn write_spreadsheet(
    path: &Path,
    format: SpreadsheetFormat,
    rows: &[FirearmExportRow],
) -> Result<(), CommandError> {
    match format {
        SpreadsheetFormat::Csv => write_csv(path, rows),
        SpreadsheetFormat::Xlsx => write_xlsx(path, rows),
    }
}

fn write_csv(path: &Path, rows: &[FirearmExportRow]) -> Result<(), CommandError> {
    let mut writer = csv::Writer::from_path(path).map_err(|e| {
        CommandError::new("INTERNAL_ERROR", format!("Could not create the export file: {e}"))
    })?;
    writer.write_record(COLUMNS).map_err(|e| {
        CommandError::new("INTERNAL_ERROR", format!("Failed writing export header: {e}"))
    })?;
    for row in rows {
        writer.write_record(row.as_fields()).map_err(|e| {
            CommandError::new("INTERNAL_ERROR", format!("Failed writing export row: {e}"))
        })?;
    }
    writer.flush().map_err(|e| {
        CommandError::new("INTERNAL_ERROR", format!("Failed saving the export file: {e}"))
    })
}

fn write_xlsx(path: &Path, rows: &[FirearmExportRow]) -> Result<(), CommandError> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    for (col, header) in COLUMNS.iter().enumerate() {
        sheet.write_string(0, col as u16, *header).map_err(|e| {
            CommandError::new("INTERNAL_ERROR", format!("Failed writing export header: {e}"))
        })?;
    }
    for (row_index, row) in rows.iter().enumerate() {
        for (col, value) in row.as_fields().iter().enumerate() {
            sheet.write_string((row_index + 1) as u32, col as u16, *value).map_err(|e| {
                CommandError::new("INTERNAL_ERROR", format!("Failed writing export row: {e}"))
            })?;
        }
    }
    workbook.save(path).map_err(|e| {
        CommandError::new("INTERNAL_ERROR", format!("Failed saving the export file: {e}"))
    })
}

pub fn read_spreadsheet(
    path: &Path,
    format: SpreadsheetFormat,
) -> Result<Vec<RawImportRow>, CommandError> {
    match format {
        SpreadsheetFormat::Csv => read_csv(path),
        SpreadsheetFormat::Xlsx => read_xlsx(path),
    }
}

fn read_csv(path: &Path) -> Result<Vec<RawImportRow>, CommandError> {
    let mut reader = csv::ReaderBuilder::new().has_headers(true).from_path(path).map_err(|e| {
        CommandError::new("VALIDATION_ERROR", format!("Could not read the import file: {e}"))
    })?;

    let headers = reader.headers().map_err(|e| {
        CommandError::new("VALIDATION_ERROR", format!("Could not read the import file: {e}"))
    })?;
    let map = HeaderMap::from_headers(headers.iter())?;

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| {
            CommandError::new("VALIDATION_ERROR", format!("Could not parse a row: {e}"))
        })?;
        let cells: Vec<String> = record.iter().map(str::to_string).collect();
        rows.push(map.row(&cells));
    }
    Ok(rows)
}

fn read_xlsx(path: &Path) -> Result<Vec<RawImportRow>, CommandError> {
    let mut workbook: calamine::Xlsx<_> = calamine::open_workbook(path).map_err(|e| {
        CommandError::new("VALIDATION_ERROR", format!("Could not read the import file: {e}"))
    })?;
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| CommandError::new("VALIDATION_ERROR", "The workbook has no sheets."))?;
    let range = workbook.worksheet_range(&sheet_name).map_err(|e| {
        CommandError::new("VALIDATION_ERROR", format!("Could not read the sheet: {e}"))
    })?;

    let mut sheet_rows = range.rows();
    let Some(header_row) = sheet_rows.next() else {
        return Ok(Vec::new());
    };
    let headers: Vec<String> = header_row.iter().map(|cell| cell.to_string()).collect();
    let map = HeaderMap::from_headers(headers.iter().map(String::as_str))?;

    let mut rows = Vec::new();
    for row in sheet_rows {
        let cells: Vec<String> = row.iter().map(|cell| cell.to_string()).collect();
        rows.push(map.row(&cells));
    }
    Ok(rows)
}
