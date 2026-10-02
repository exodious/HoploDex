//! Spreadsheet fixtures for the two-table tests (specs/006-accessory-links
//! US5, contracts/spreadsheet-format.md): rows built from named cells in the
//! real column order of each table, CSV files and workbooks written and read
//! back, and the two-file `import_collection` call.
//!
//! Shared by `export_test.rs`, `accessory_spreadsheet_test.rs` and
//! `import_matching_test.rs` (declare it with `mod sheets;`). Nothing here
//! mocks anything: the files are real and the import goes through the real
//! `ops` onto a real temporary database.

// Shared by several test crates, each using a subset.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use calamine::{Reader, Xlsx, open_workbook};
use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::import_export::{ImportFile, ImportResult, ImportSessionStore, ops};
use hoplodex_lib::services::spreadsheet::{ACCESSORY_COLUMNS, FIREARM_COLUMNS, SpreadsheetFormat};
use rusqlite::Connection;
use serde::Serialize;

/// A table as the file holds it: the header row, then the data rows.
pub type Table = Vec<Vec<String>>;

// --- Rows ----------------------------------------------------------------------------------------

/// A row of `columns` from named cells; a column not named is blank and a
/// later entry for the same column wins. A name that is no column fails the
/// test, so a typo can't pass unnoticed.
pub fn row_of(columns: &[&str], cells: &[(&str, &str)]) -> Vec<String> {
    for (name, _) in cells {
        assert!(columns.contains(name), "unknown spreadsheet column {name}");
    }
    columns
        .iter()
        .map(|column| {
            cells
                .iter()
                .rev()
                .find(|(name, _)| name == column)
                .map_or("", |(_, value)| value)
                .to_string()
        })
        .collect()
}

pub fn firearm_row(cells: &[(&str, &str)]) -> Vec<String> {
    row_of(FIREARM_COLUMNS, cells)
}

pub fn accessory_row(cells: &[(&str, &str)]) -> Vec<String> {
    row_of(ACCESSORY_COLUMNS, cells)
}

/// A complete, valid Handgun row (a blank serial number means "no serial
/// number", attested), with `extra` cells layered on top.
pub fn firearm_cells(make: &str, model: &str, serial: &str, extra: &[(&str, &str)]) -> Vec<String> {
    let mut cells = vec![
        ("make", make),
        ("model", model),
        ("serial_number", serial),
        ("no_serial_attested", if serial.is_empty() { "TRUE" } else { "FALSE" }),
        ("caliber", "9mm"),
        ("firearm_type", "Handgun"),
        ("status", "active"),
        ("estimated_value", "500"),
    ];
    cells.extend_from_slice(extra);
    firearm_row(&cells)
}

/// A complete, valid accessory row of `kind` (a kind's name), its required
/// make and model "Make" and "Model", with `extra` cells layered on top.
pub fn accessory_cells(kind: &str, extra: &[(&str, &str)]) -> Vec<String> {
    let mut cells =
        vec![("kind", kind), ("make", "Make"), ("model", "Model"), ("status", "active")];
    cells.extend_from_slice(extra);
    accessory_row(&cells)
}

pub fn firearm_table(rows: &[Vec<String>]) -> Table {
    table_of(FIREARM_COLUMNS, rows)
}

pub fn accessory_table(rows: &[Vec<String>]) -> Table {
    table_of(ACCESSORY_COLUMNS, rows)
}

fn table_of(columns: &[&str], rows: &[Vec<String>]) -> Table {
    let mut table = vec![columns.iter().map(|c| c.to_string()).collect()];
    table.extend_from_slice(rows);
    table
}

/// A table with a header of the test's own choosing, for sheets that are
/// not a HoploDex table, or a table without some column.
pub fn table_with(header: &[&str], rows: &[&[&str]]) -> Table {
    let mut table = vec![header.iter().map(|c| c.to_string()).collect()];
    table.extend(rows.iter().map(|row| row.iter().map(|c| c.to_string()).collect()));
    table
}

/// `table` without the named columns, as an older export would be.
pub fn without_columns(table: &Table, dropped: &[&str]) -> Table {
    let keep: Vec<usize> =
        (0..table[0].len()).filter(|i| !dropped.contains(&table[0][*i].as_str())).collect();
    table.iter().map(|row| keep.iter().map(|i| row[*i].clone()).collect()).collect()
}

/// The index of the column `name` in the table's header.
pub fn column(table: &Table, name: &str) -> usize {
    table[0].iter().position(|h| h == name).unwrap_or_else(|| panic!("no {name} column"))
}

/// The cell of data row `row` (0-based, below the header) in column `name`.
pub fn cell<'a>(table: &'a Table, row: usize, name: &str) -> &'a str {
    &table[row + 1][column(table, name)]
}

// --- Files ---------------------------------------------------------------------------------------

pub fn write_csv(path: &Path, table: &Table) {
    let mut writer = csv::Writer::from_path(path).unwrap();
    for row in table {
        writer.write_record(row).unwrap();
    }
    writer.flush().unwrap();
}

pub fn read_csv(path: &Path) -> Table {
    let mut reader = csv::ReaderBuilder::new().has_headers(false).from_path(path).unwrap();
    reader.records().map(|record| record.unwrap().iter().map(str::to_owned).collect()).collect()
}

/// A workbook with one sheet per `(name, table)`, every cell text, as the
/// export writes it. An empty table is a blank sheet.
pub fn write_workbook(path: &Path, sheets: &[(&str, &Table)]) {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    for (name, table) in sheets {
        let sheet = workbook.add_worksheet();
        sheet.set_name(*name).unwrap();
        for (r, row) in table.iter().enumerate() {
            for (c, value) in row.iter().enumerate() {
                sheet.write_string(r as u32, c as u16, value).unwrap();
            }
        }
    }
    workbook.save(path).unwrap();
}

/// Every sheet of a workbook, in order, with its name.
pub fn read_workbook(path: &Path) -> Vec<(String, Table)> {
    let mut workbook: Xlsx<_> = open_workbook(path).unwrap();
    workbook
        .sheet_names()
        .into_iter()
        .map(|name| {
            let range = workbook.worksheet_range(&name).unwrap();
            let table =
                range.rows().map(|row| row.iter().map(|cell| cell.to_string()).collect()).collect();
            (name, table)
        })
        .collect()
}

// --- Importing -----------------------------------------------------------------------------------

/// One file for `import_collection`, its format from the extension.
pub fn import_file(path: &Path) -> ImportFile {
    let format = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xlsx")) {
        SpreadsheetFormat::Xlsx
    } else {
        SpreadsheetFormat::Csv
    };
    ImportFile { file_path: path.to_path_buf(), format }
}

/// `import_collection` on one or two files (contracts/tauri-commands.md:
/// `files: { filePath, format }[]`).
pub fn import(
    conn: &Connection,
    store: &ImportSessionStore,
    paths: &[&Path],
) -> Result<ImportResult, CommandError> {
    let files: Vec<ImportFile> = paths.iter().map(|p| import_file(p)).collect();
    ops::import_collection(conn, &files, store, &mut |_, _| {})
}

/// Writes `table` as `name` in `dir` and returns its path.
pub fn csv_in(dir: &Path, name: &str, table: &Table) -> PathBuf {
    let path = dir.join(name);
    write_csv(&path, table);
    path
}

/// `(table, row, message)` of each entry of a report list (`rowErrors`,
/// `warnings`, `unresolved`), read as the frontend receives it.
pub fn entries<T: Serialize>(list: &[T]) -> Vec<(String, u64, String)> {
    list.iter()
        .map(|entry| {
            let value = serde_json::to_value(entry).unwrap();
            (
                value["table"].as_str().expect("an entry names its table").to_owned(),
                value["row"].as_u64().unwrap(),
                value["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// The messages of the entries that belong to `table` ("firearms" or
/// "accessories") and `row`.
pub fn messages_for<T: Serialize>(list: &[T], table: &str, row: u64) -> Vec<String> {
    entries(list)
        .into_iter()
        .filter(|(t, r, _)| t == table && *r == row)
        .map(|(_, _, message)| message)
        .collect()
}
