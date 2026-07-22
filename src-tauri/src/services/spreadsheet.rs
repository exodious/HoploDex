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
/// export-only (ignored on import, FR-019).
pub const COLUMNS: &[&str] = &[
    "make",
    "model",
    "serial_number",
    "no_serial_attested",
    "caliber",
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
    "coverage_kind",
    "scheduled_coverage_amount",
    "photo_filenames",
];

/// One exported row — every cell already formatted as display text (blank
/// string for an absent value), per contracts/spreadsheet-format.md.
#[derive(Debug, Clone, Default)]
pub struct FirearmExportRow {
    pub make: String,
    pub model: String,
    pub serial_number: String,
    pub no_serial_attested: String,
    pub caliber: String,
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
    pub coverage_kind: String,
    pub scheduled_coverage_amount: String,
    pub photo_filenames: String,
}

impl FirearmExportRow {
    fn as_fields(&self) -> [&str; 21] {
        [
            &self.make,
            &self.model,
            &self.serial_number,
            &self.no_serial_attested,
            &self.caliber,
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
            &self.coverage_kind,
            &self.scheduled_coverage_amount,
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
    pub serial_number: Option<String>,
    pub no_serial_attested: Option<String>,
    pub caliber: Option<String>,
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
    pub coverage_kind: Option<String>,
    pub scheduled_coverage_amount: Option<String>,
}

fn non_blank(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn row_from_cells(cells: &[String]) -> RawImportRow {
    let cell = |index: usize| cells.get(index).map(String::as_str).unwrap_or("");
    RawImportRow {
        make: non_blank(cell(0)),
        model: non_blank(cell(1)),
        serial_number: non_blank(cell(2)),
        no_serial_attested: non_blank(cell(3)),
        caliber: non_blank(cell(4)),
        firearm_type: non_blank(cell(5)),
        notes: non_blank(cell(6)),
        accessories: non_blank(cell(7)),
        status: non_blank(cell(8)),
        estimated_value: non_blank(cell(9)),
        acquisition_source: non_blank(cell(10)),
        acquisition_date: non_blank(cell(11)),
        acquisition_price: non_blank(cell(12)),
        disposition_type: non_blank(cell(13)),
        disposition_recipient: non_blank(cell(14)),
        disposition_date: non_blank(cell(15)),
        disposition_price: non_blank(cell(16)),
        insurance_policy_name: non_blank(cell(17)),
        coverage_kind: non_blank(cell(18)),
        scheduled_coverage_amount: non_blank(cell(19)),
    }
}

/// Formats an integer-cents amount as a decimal currency string (e.g.
/// `450.00`), or `""` when absent — the spreadsheet-format.md convention.
pub fn cents_to_decimal_string(cents: Option<i64>) -> String {
    match cents {
        Some(c) => format!("{:.2}", c as f64 / 100.0),
        None => String::new(),
    }
}

/// Parses a decimal currency string (e.g. `450.00`) into integer cents;
/// blank/unparsable input yields `None`.
pub fn parse_decimal_to_cents(value: &Option<String>) -> Option<i64> {
    let raw = value.as_deref()?.trim();
    if raw.is_empty() {
        return None;
    }
    raw.parse::<f64>().ok().map(|parsed| (parsed * 100.0).round() as i64)
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

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| {
            CommandError::new("VALIDATION_ERROR", format!("Could not parse a row: {e}"))
        })?;
        let cells: Vec<String> = record.iter().map(str::to_string).collect();
        rows.push(row_from_cells(&cells));
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

    let mut rows = Vec::new();
    for (row_index, row) in range.rows().enumerate() {
        if row_index == 0 {
            continue; // header row
        }
        let cells: Vec<String> = row.iter().map(|cell| cell.to_string()).collect();
        rows.push(row_from_cells(&cells));
    }
    Ok(rows)
}
