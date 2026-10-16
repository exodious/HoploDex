//! The import's limits: file size, rows, columns, cell length and the like
//! (008 research.md §9, 008 data-model.md "Import limits").
//!
//! The helper applies them as it reads; the parent checks the file size from
//! the file's metadata first and the cell count across both files of an
//! import.

use crate::models::rules::thousands;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Bytes, per file. Checked from the metadata, and counted while streaming
/// (a file may grow).
pub const MAX_FILE_BYTES: u64 = 268_435_456;
/// Bytes, all parts of one workbook, measured by inflating.
pub const MAX_UNPACKED_BYTES: u64 = 1_073_741_824;
/// Entries per workbook.
pub const MAX_ZIP_ENTRIES: usize = 1_000;
/// Sheets per workbook.
pub const MAX_SHEETS: usize = 16;
/// Data rows per table.
pub const MAX_ROWS: usize = 100_000;
/// Cells per row.
pub const MAX_COLUMNS: usize = 256;
/// Cells in all the rows sent to the parent, per import, empty ones included
/// (a row is padded up to its last cell).
pub const MAX_CELLS: u64 = 5_000_000;
/// Characters per cell (Excel's own limit).
pub const MAX_CELL_CHARS: usize = 32_767;
/// UTF-8 bytes of all cells read, per import.
pub const MAX_TOTAL_TEXT_BYTES: u64 = 268_435_456;
/// Declared (`count`, `uniqueCount`) or present.
pub const MAX_SHARED_STRINGS: u64 = 2_000_000;
/// The import helper's data limit (Linux, Windows).
pub const HELPER_MEMORY_BYTES: usize = 536_870_912;
/// How long the parent waits with no frame from the helper.
pub const HELPER_IDLE_TIMEOUT: Duration = Duration::from_secs(120);

/// Which limit a file passed: `IMPORT_LIMIT_EXCEEDED`'s `details.limit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportLimit {
    FileSize,
    UnpackedSize,
    ZipEntries,
    Sheets,
    Rows,
    Columns,
    Cells,
    CellText,
    TotalText,
    SharedStrings,
}

impl ImportLimit {
    /// Every limit, in the order of 008 data-model.md's table.
    pub const ALL: [ImportLimit; 10] = [
        Self::FileSize,
        Self::UnpackedSize,
        Self::ZipEntries,
        Self::Sheets,
        Self::Rows,
        Self::Columns,
        Self::Cells,
        Self::CellText,
        Self::TotalText,
        Self::SharedStrings,
    ];

    /// What the file passed, and what the app imports instead, as the
    /// message's sentences (008 contracts/tauri-commands.md,
    /// `IMPORT_LIMIT_EXCEEDED`): `more than 100,000 rows. HoploDex imports at
    /// most 100,000 rows per table.`
    pub fn description(self) -> String {
        match self {
            Self::FileSize => format!(
                "larger than {}. HoploDex imports files of at most {}.",
                mib(MAX_FILE_BYTES),
                mib(MAX_FILE_BYTES)
            ),
            Self::UnpackedSize => format!(
                "unpacks to more than {}. HoploDex reads workbooks of at most {} unpacked.",
                gib(MAX_UNPACKED_BYTES),
                gib(MAX_UNPACKED_BYTES)
            ),
            Self::ZipEntries => format!(
                "more than {n} parts. HoploDex reads workbooks of at most {n} parts.",
                n = thousands(MAX_ZIP_ENTRIES as u64)
            ),
            Self::Sheets => format!(
                "more than {n} sheets. HoploDex imports at most {n} sheets per workbook.",
                n = thousands(MAX_SHEETS as u64)
            ),
            Self::Rows => format!(
                "more than {n} rows. HoploDex imports at most {n} rows per table.",
                n = thousands(MAX_ROWS as u64)
            ),
            Self::Columns => format!(
                "more than {n} columns. HoploDex imports at most {n} columns per row.",
                n = thousands(MAX_COLUMNS as u64)
            ),
            Self::Cells => format!(
                "more than {n} cells. HoploDex imports at most {n} cells per import, \
                 empty ones included.",
                n = thousands(MAX_CELLS)
            ),
            Self::CellText => format!(
                "a cell has more than {n} characters. HoploDex imports at most {n} \
                 characters per cell.",
                n = thousands(MAX_CELL_CHARS as u64)
            ),
            Self::TotalText => format!(
                "more than {n} of text. HoploDex imports at most {n} of text per import.",
                n = mib(MAX_TOTAL_TEXT_BYTES)
            ),
            Self::SharedStrings => format!(
                "more than {n} shared strings. HoploDex imports workbooks of at most {n} \
                 shared strings.",
                n = thousands(MAX_SHARED_STRINGS)
            ),
        }
    }

    /// The `IMPORT_LIMIT_EXCEEDED` message: the file, the sheet of a workbook
    /// if there is one, then [`description`](Self::description), as in
    /// `inventory.xlsx, sheet "Sheet3": more than 100,000 rows. HoploDex
    /// imports at most 100,000 rows per table.`
    pub fn message(self, file: &str, sheet: Option<&str>) -> String {
        match sheet {
            Some(sheet) => format!("{file}, sheet \"{sheet}\": {}", self.description()),
            None => format!("{file}: {}", self.description()),
        }
    }
}

fn mib(bytes: u64) -> String {
    format!("{} MiB", thousands(bytes / (1024 * 1024)))
}

fn gib(bytes: u64) -> String {
    format!("{} GiB", bytes / (1024 * 1024 * 1024))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_constants_are_data_models_values() {
        assert_eq!(MAX_FILE_BYTES, 256 * 1024 * 1024);
        assert_eq!(MAX_UNPACKED_BYTES, 1024 * 1024 * 1024);
        assert_eq!(MAX_TOTAL_TEXT_BYTES, 256 * 1024 * 1024);
        assert_eq!(HELPER_MEMORY_BYTES, 512 * 1024 * 1024);
        assert_eq!(HELPER_IDLE_TIMEOUT.as_secs(), 120);
    }

    #[test]
    fn the_rows_message_is_the_contracts_example() {
        assert_eq!(
            ImportLimit::Rows.message("inventory.xlsx", Some("Sheet3")),
            "inventory.xlsx, sheet \"Sheet3\": more than 100,000 rows. HoploDex imports at \
             most 100,000 rows per table."
        );
    }

    #[test]
    fn every_limit_has_a_message_naming_the_file_and_its_number() {
        assert_eq!(ImportLimit::ALL.len(), 10);
        let expected = [
            "256 MiB",
            "1 GiB",
            "1,000",
            "16",
            "100,000",
            "256",
            "5,000,000",
            "32,767",
            "256 MiB",
            "2,000,000",
        ];
        for (limit, number) in ImportLimit::ALL.into_iter().zip(expected) {
            let message = limit.message("a.csv", None);
            assert!(message.starts_with("a.csv: "), "{message}");
            assert!(message.contains(number), "{limit:?}: {message}");
            assert!(message.contains("HoploDex"), "{limit:?}: {message}");
            assert!(!limit.message("a.csv", Some("S")).is_empty());
        }
        assert_eq!(
            ImportLimit::Sheets.message("a.xlsx", None),
            "a.xlsx: more than 16 sheets. HoploDex imports at most 16 sheets per workbook."
        );
    }

    #[test]
    fn limits_serialize_camel_case() {
        let names: Vec<String> =
            ImportLimit::ALL.iter().map(|l| serde_json::to_string(l).unwrap()).collect();
        assert_eq!(
            names.join(","),
            "\"fileSize\",\"unpackedSize\",\"zipEntries\",\"sheets\",\"rows\",\"columns\",\
             \"cells\",\"cellText\",\"totalText\",\"sharedStrings\""
        );
    }
}
