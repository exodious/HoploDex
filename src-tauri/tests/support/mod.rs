use hoplodex_lib::db;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::services::spreadsheet::COLUMNS;
use rusqlite::Connection;
use tempfile::TempDir;

/// A real, migrated, encrypted SQLCipher database in a temp directory —
/// never a mock connection, per the constitution's Testing Standards
/// principle. `_dir` is held only to keep the temp directory alive for the
/// lifetime of the returned handle.
pub struct TestDb {
    pub conn: Connection,
    _dir: TempDir,
}

impl TestDb {
    pub fn new() -> Self {
        let dir = TempDir::new().expect("failed to create temp dir for test DB");
        let db_path = dir.path().join("test.db");
        let key_hex = db::generate_key_hex().expect("failed to generate test DB key");
        let conn =
            db::open_encrypted(&db_path, &key_hex).expect("failed to open encrypted test DB");
        Self { conn, _dir: dir }
    }
}

impl Default for TestDb {
    fn default() -> Self {
        Self::new()
    }
}

/// A tiny (20x20, solid red) but genuinely valid PNG, so
/// `services::photos`'s real image-decoding thumbnail generator has real
/// bytes to decode — no mocks, per the constitution.
// Shared by every integration-test crate, but only some of them use each helper.
#[allow(dead_code)]
pub fn sample_png_bytes() -> Vec<u8> {
    vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 20, 0, 0, 0, 20, 8,
        2, 0, 0, 0, 2, 235, 138, 90, 0, 0, 0, 26, 73, 68, 65, 84, 120, 218, 99, 248, 207, 192, 64,
        54, 98, 24, 213, 60, 170, 121, 84, 243, 168, 230, 129, 213, 12, 0, 49, 205, 142, 128, 132,
        11, 139, 140, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]
}

/// One spreadsheet data row built from named cells, in the real export
/// column order (so adding or dropping a column never means hand-editing
/// positional literals). Columns not named are blank; a later entry for the
/// same column wins.
#[allow(dead_code)]
pub fn csv_row(cells: &[(&str, &str)]) -> String {
    for (name, _) in cells {
        assert!(COLUMNS.contains(name), "unknown spreadsheet column {name}");
    }
    COLUMNS
        .iter()
        .map(|column| {
            cells.iter().rev().find(|(name, _)| name == column).map_or("", |(_, value)| value)
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// A complete, valid Handgun row for `make`/`model`/`serial` (a blank
/// serial means "no serial number", attested), with `extra` cells layered
/// on top.
#[allow(dead_code)]
pub fn csv_firearm(make: &str, model: &str, serial: &str, extra: &[(&str, &str)]) -> String {
    let mut cells = vec![
        ("make", make),
        ("model", model),
        ("serial_number", serial),
        ("no_serial_attested", if serial.is_empty() { "TRUE" } else { "FALSE" }),
        ("caliber", "9mm"),
        ("firearm_type", "Handgun"),
        ("estimated_value", "500.00"),
    ];
    cells.extend_from_slice(extra);
    csv_row(&cells)
}

/// The export header plus `rows`, newline-terminated — a whole import file.
#[allow(dead_code)]
pub fn csv_file(rows: &[String]) -> String {
    let mut lines = vec![COLUMNS.join(",")];
    lines.extend_from_slice(rows);
    lines.join("\n") + "\n"
}

/// A valid, active Handgun record for `make`/`model`/`serial`, every other
/// optional field empty — tests set only what they exercise.
#[allow(dead_code)]
pub fn firearm(make: &str, model: &str, serial: &str) -> FirearmInput {
    FirearmInput {
        make: make.into(),
        model: model.into(),
        serial_number: Some(serial.into()),
        no_serial_attested: false,
        caliber: "9mm".into(),
        firearm_type_id: 1,
        nickname: None,
        notes: None,
        accessories: None,
        status: FirearmStatus::Active,
        estimated_value: None,
        acquisition_source: None,
        acquisition_date: None,
        acquisition_price: None,
        disposition_type: None,
        disposition_recipient: None,
        disposition_date: None,
        disposition_price: None,
        insurance_policy_id: None,
        coverage_kind: None,
        scheduled_coverage_amount: None,
    }
}
