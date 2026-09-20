//! Integration test: `export_collection` produces a spreadsheet + photos
//! folder matching contracts/spreadsheet-format.md (spec.md US5 Acceptance
//! Scenario 1), run against a real temporary SQLCipher database and a real
//! temporary destination folder — no mocks.

mod support;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use support::{sample_png_bytes, TestDb};
use tempfile::TempDir;

fn firearm_with_photo(make: &str) -> FirearmInput {
    FirearmInput {
        make: make.into(),
        model: "M".into(),
        serial_number: Some(format!("{make}-SN")),
        no_serial_attested: false,
        caliber: "9mm".into(),
        firearm_type_id: 1,
        notes: Some("some notes".into()),
        accessories: None,
        status: FirearmStatus::Active,
        estimated_value: Some(50000),
        acquisition_source: None,
        acquisition_date: None,
        acquisition_price: None,
        disposition_type: None,
        disposition_recipient: None,
        disposition_date: None,
        disposition_price: None,
        insurance_policy_id: None,
        nickname: None,
        scheduled_coverage_amount: None,
    }
}

#[test]
fn scenario_1_exports_a_spreadsheet_and_photos_folder() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();

    let f1 = firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Glock")).unwrap();
    let f2 = firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Sig")).unwrap();
    hoplodex_lib::commands::photos::ops::add_photo(
        &db.conn,
        f1.id,
        &sample_png_bytes(),
        "range-day.png",
        "image/png",
    )
    .unwrap();

    let mut progress_calls = Vec::new();
    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "test-export",
        SpreadsheetFormat::Csv,
        &[f1.id, f2.id],
        &mut |done, total| progress_calls.push((done, total)),
    )
    .unwrap();

    assert_eq!(result.exported_firearm_count, 2);
    assert_eq!(result.exported_photo_count, 1);
    assert!(result.spreadsheet_path.exists());
    assert!(result.photos_folder_path.exists());
    assert!(!progress_calls.is_empty(), "progress must be reported for bulk export");

    let contents = std::fs::read_to_string(&result.spreadsheet_path).unwrap();
    assert!(contents.contains("Glock"));
    assert!(contents.contains("Sig"));
    assert!(contents.contains("make"), "header row must be present");

    let photo_files: Vec<_> = std::fs::read_dir(&result.photos_folder_path)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(photo_files.len(), 1);
    assert!(photo_files[0].ends_with("range-day.png"));
}

#[test]
fn exports_as_xlsx_when_requested() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let f1 = firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Glock")).unwrap();

    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "test-export",
        SpreadsheetFormat::Xlsx,
        &[f1.id],
        &mut |_, _| {},
    )
    .unwrap();

    assert!(result.spreadsheet_path.extension().unwrap() == "xlsx");
    assert!(result.spreadsheet_path.exists());
}
