//! Integration test: `export_collection` produces a spreadsheet + photos
//! folder matching contracts/spreadsheet-format.md (spec.md US5 Acceptance
//! Scenario 1), run against a real temporary SQLCipher database and a real
//! temporary destination folder — no mocks.

mod support;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use support::{TestDb, sample_png_bytes};
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
        barrel_length_hundredths: None,
        overall_length_hundredths: None,
        weight_tenths_oz: None,
        capacity: None,
        finish: None,
        condition: None,
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
        origin: None,
        year_of_manufacture: None,
        country_of_manufacture: None,
        importer_name: None,
        original_make: None,
        original_model: None,
        original_serial_number: None,
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        cartridge: None,
        action_type_id: None,
        mounted_on: None,
    }
}

#[test]
fn scenario_1_exports_a_spreadsheet_and_photos_folder() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();

    let f1 = firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Glock"), false).unwrap();
    let f2 = firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Sig"), false).unwrap();
    hoplodex_lib::commands::photos::ops::add_photo(
        &db.conn,
        RecordRef::Firearm(f1.id),
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
    let f1 = firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Glock"), false).unwrap();

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

/// FR-039 / spreadsheet-format.md "Physical details": plain numbers with no
/// unit text and no trailing zeros, and the condition's display name,
/// between `scheduled_coverage_amount` and `photo_filenames`.
#[test]
fn physical_details_are_exported_as_plain_numbers_between_coverage_and_photos() {
    use hoplodex_lib::models::firearm::Condition;

    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let created = firearm_ops::create_firearm(
        &db.conn,
        &FirearmInput {
            barrel_length_hundredths: Some(1625),
            overall_length_hundredths: Some(1800),
            weight_tenths_oz: Some(405),
            capacity: Some(15),
            finish: Some("Cerakote".into()),
            condition: Some(Condition::LikeNew),
            ..firearm_with_photo("Glock")
        },
        false,
    )
    .unwrap();

    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "physical",
        SpreadsheetFormat::Csv,
        &[created.id],
        &mut |_, _| {},
    )
    .unwrap();

    let mut reader = csv::Reader::from_path(&result.spreadsheet_path).unwrap();
    let headers = reader.headers().unwrap().clone();
    let record = reader.records().next().unwrap().unwrap();
    let column = |name: &str| headers.iter().position(|h| h == name).unwrap();
    let cell = |name: &str| record.get(column(name)).unwrap();

    assert_eq!(cell("barrel_length_in"), "16.25");
    assert_eq!(cell("overall_length_in"), "18");
    assert_eq!(cell("weight_oz"), "40.5");
    assert_eq!(cell("capacity"), "15");
    assert_eq!(cell("finish"), "Cerakote");
    assert_eq!(cell("condition"), "Like new");

    let order: Vec<_> = headers.iter().collect();
    let after = column("scheduled_coverage_amount");
    assert_eq!(
        &order[after + 1..],
        [
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
            "photo_filenames"
        ]
    );
}

#[test]
fn blank_physical_details_are_exported_as_blank_cells() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let created = firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Sig"), false).unwrap();
    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "blank",
        SpreadsheetFormat::Csv,
        &[created.id],
        &mut |_, _| {},
    )
    .unwrap();

    let mut reader = csv::Reader::from_path(&result.spreadsheet_path).unwrap();
    let headers = reader.headers().unwrap().clone();
    let record = reader.records().next().unwrap().unwrap();
    for name in
        ["barrel_length_in", "overall_length_in", "weight_oz", "capacity", "finish", "condition"]
    {
        let cell = record.get(headers.iter().position(|h| h == name).unwrap()).unwrap();
        assert_eq!(cell, "", "{name}");
    }
}

#[test]
fn the_export_header_is_the_40_columns_with_cartridge_and_action_type_after_caliber() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Glock"), false).unwrap();
    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "header",
        SpreadsheetFormat::Csv,
        &[created.id],
        &mut |_, _| {},
    )
    .unwrap();

    let mut reader = csv::Reader::from_path(&result.spreadsheet_path).unwrap();
    let headers: Vec<_> = reader.headers().unwrap().iter().map(str::to_owned).collect();
    assert_eq!(headers, hoplodex_lib::services::spreadsheet::COLUMNS);
    assert_eq!(headers.len(), 40);
    let caliber_at = headers.iter().position(|h| h == "caliber").unwrap();
    assert_eq!(headers[caliber_at + 1], "cartridge");
    assert_eq!(headers[caliber_at + 2], "action_type");
    assert_eq!(headers[caliber_at + 3], "firearm_type");
}

/// specs/005-regulated-item-types US4-1: the four registration columns hold
/// the classification's name, the form, the approved date and "Registered
/// to"; a Suppressor exports its type with blank action, barrel length and
/// capacity; a firearm with no classification has all four blank.
#[test]
fn export_writes_the_registration_columns_and_blanks_what_a_suppressor_lacks() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let suppressor = firearm_ops::create_firearm(
        &db.conn,
        &FirearmInput {
            firearm_type_id: 5,
            registration_class_id: Some(1),
            registration_form: Some("Form 4".into()),
            registration_approved: Some("2024-03-05".into()),
            registered_to: Some("Jane Doe".into()),
            ..firearm_with_photo("Omega")
        },
        false,
    )
    .unwrap();
    let plain = firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Glock"), false).unwrap();

    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "registration",
        SpreadsheetFormat::Csv,
        &[suppressor.id, plain.id],
        &mut |_, _| {},
    )
    .unwrap();
    let mut reader = csv::Reader::from_path(&result.spreadsheet_path).unwrap();
    let headers = reader.headers().unwrap().clone();
    let at = |name: &str| headers.iter().position(|h| h == name).unwrap();
    let rows: Vec<_> = reader.records().map(Result::unwrap).collect();

    assert_eq!(&rows[0][at("firearm_type")], "Suppressor");
    assert_eq!(&rows[0][at("registered_as")], "Suppressor");
    assert_eq!(&rows[0][at("registration_form")], "Form 4");
    assert_eq!(&rows[0][at("registration_approved")], "2024-03-05");
    assert_eq!(&rows[0][at("registered_to")], "Jane Doe");
    for blank in ["action_type", "barrel_length_in", "capacity"] {
        assert_eq!(&rows[0][at(blank)], "", "{blank}");
    }
    for blank in ["registered_as", "registration_form", "registration_approved", "registered_to"] {
        assert_eq!(&rows[1][at(blank)], "", "{blank}");
    }
}
