//! Integration test: `import_collection` creates new records and reports
//! failing rows without discarding successful ones (spec.md US5 Acceptance
//! Scenarios 2-3; FR-020), run against a real temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::ImportSessionStore;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use support::{TestDb, csv_file, csv_firearm};
use tempfile::TempDir;

fn write_csv(dir: &TempDir, contents: &str) -> std::path::PathBuf {
    let path = dir.path().join("import.csv");
    std::fs::write(&path, contents).unwrap();
    path
}

#[test]
fn scenario_2_imports_new_records_from_a_spreadsheet() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();

    let csv = csv_file(&[
        csv_firearm("Glock", "19", "ABC123", &[]),
        csv_firearm("Sig", "P226", "DEF456", &[("estimated_value", "600.00")]),
    ]);
    let path = write_csv(&dir, &csv);

    let mut progress_calls = Vec::new();
    let result = import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |done, total| progress_calls.push((done, total)),
    )
    .unwrap();

    assert_eq!(result.imported_count, 2);
    assert_eq!(result.row_errors.len(), 0);
    assert!(!progress_calls.is_empty());

    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    let all: Vec<_> = listing.groups.iter().flat_map(|g| &g.firearms).collect();
    assert!(all.iter().any(|f| f.make == "Glock" && f.model == "19"));
    assert!(all.iter().any(|f| f.make == "Sig" && f.model == "P226"));
}

#[test]
fn scenario_3_reports_a_failing_row_without_discarding_successful_ones() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();

    // Row 1 is missing `make` (required); row 2 is valid.
    let csv = csv_file(&[
        csv_firearm("", "19", "ABC123", &[("no_serial_attested", "FALSE")]),
        csv_firearm("Sig", "P226", "DEF456", &[("estimated_value", "600.00")]),
    ]);
    let path = write_csv(&dir, &csv);

    let result = import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |_, _| {},
    )
    .unwrap();

    assert_eq!(result.imported_count, 1, "the valid row must still import");
    assert_eq!(result.row_errors.len(), 1);
    assert_eq!(result.row_errors[0].row, 1);

    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    let all: Vec<_> = listing.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].make, "Sig");
}

#[test]
fn rejects_an_unknown_firearm_type_with_a_row_error() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();

    let csv =
        csv_file(&[csv_firearm("Glock", "19", "ABC123", &[("firearm_type", "NotARealType")])]);
    let path = write_csv(&dir, &csv);

    let result = import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |_, _| {},
    )
    .unwrap();

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.row_errors.len(), 1);
}

// --- Date rules (FR-003 / FR-004): a violation is a row error ---

fn import_one_row(row: String) -> hoplodex_lib::commands::import_export::ImportResult {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let path = write_csv(&dir, &csv_file(&[row]));
    import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |_, _| {},
    )
    .unwrap()
}

/// A disposed Glock 19 row acquired on `acquired` (if any) and disposed of
/// on `disposed_on`.
fn disposed_row(acquired: Option<&str>, disposed_on: &str) -> String {
    let mut cells = vec![
        ("status", "disposed"),
        ("disposition_type", "sold"),
        ("disposition_recipient", "Jane"),
        ("disposition_date", disposed_on),
        ("disposition_price", "400.00"),
    ];
    if let Some(acquired) = acquired {
        cells.push(("acquisition_date", acquired));
    }
    csv_firearm("Glock", "19", "ABC123", &cells)
}

#[test]
fn a_future_acquisition_date_is_a_row_error() {
    let tomorrow = (chrono::Local::now().date_naive() + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let result =
        import_one_row(csv_firearm("Glock", "19", "ABC123", &[("acquisition_date", &tomorrow)]));

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.row_errors.len(), 1);
    assert_eq!(result.row_errors[0].row, 1);
    assert!(result.row_errors[0].message.to_lowercase().contains("acquisition"));
}

#[test]
fn a_future_disposition_date_is_a_row_error() {
    let tomorrow = (chrono::Local::now().date_naive() + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let result = import_one_row(disposed_row(None, &tomorrow));

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.row_errors.len(), 1);
    assert!(result.row_errors[0].message.to_lowercase().contains("disposition"));
}

#[test]
fn a_disposition_before_the_acquisition_date_is_a_row_error() {
    let result = import_one_row(disposed_row(Some("2025-03-01"), "2025-02-28"));

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.row_errors.len(), 1);
    assert!(result.row_errors[0].message.to_lowercase().contains("acquisition"));
}

#[test]
fn a_valid_disposition_on_or_after_the_acquisition_date_imports() {
    let result = import_one_row(disposed_row(Some("2025-03-01"), "2025-03-01"));

    assert!(result.row_errors.is_empty(), "{:?}", result.row_errors);
    assert_eq!(result.imported_count, 1);
}

// --- Coverage columns (FR-014/FR-036, contracts/spreadsheet-format.md) ---

fn import_into(
    db: &TestDb,
    rows: &[String],
) -> hoplodex_lib::commands::import_export::ImportResult {
    let dir = TempDir::new().unwrap();
    let path = write_csv(&dir, &csv_file(rows));
    import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &ImportSessionStore::new(),
        &mut |_, _| {},
    )
    .unwrap()
}

fn rider(db: &TestDb) -> i64 {
    hoplodex_lib::commands::insurance::ops::create_policy(
        &db.conn,
        &support::policy("Collectibles rider", "2020-01-01", "2099-01-01", None),
    )
    .unwrap()
    .id
}

#[test]
fn there_is_no_coverage_kind_column() {
    assert!(!hoplodex_lib::services::spreadsheet::COLUMNS.contains(&"coverage_kind"));
}

#[test]
fn a_row_naming_a_policy_and_an_amount_is_scheduled_under_it() {
    let db = TestDb::new();
    let policy_id = rider(&db);

    let result = import_into(
        &db,
        &[csv_firearm(
            "Colt",
            "Python",
            "V1",
            &[
                ("insurance_policy_name", "collectibles RIDER"),
                ("scheduled_coverage_amount", "3500.00"),
            ],
        )],
    );

    assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    let imported = &listing.groups[0].firearms[0];
    assert_eq!(imported.insurance_policy_id, Some(policy_id));
    assert_eq!(imported.scheduled_coverage_amount, Some(3500));
}

#[test]
fn a_row_with_no_policy_is_unscheduled_meaning_covered_by_the_blanket_policy() {
    let db = TestDb::new();
    let result = import_into(&db, &[csv_firearm("Glock", "19", "A1", &[])]);

    assert_eq!(result.imported_count, 1);
    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    let imported = &listing.groups[0].firearms[0];
    assert_eq!(imported.insurance_policy_id, None);
    assert_eq!(imported.scheduled_coverage_amount, None);
}

#[test]
fn a_policy_without_an_amount_is_a_row_error() {
    let db = TestDb::new();
    rider(&db);

    let result = import_into(
        &db,
        &[csv_firearm("Colt", "Python", "V1", &[("insurance_policy_name", "Collectibles rider")])],
    );

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.row_errors.len(), 1);
    assert!(result.row_errors[0].message.to_lowercase().contains("amount"));
}

#[test]
fn an_amount_without_a_policy_is_a_row_error() {
    let db = TestDb::new();

    let result = import_into(
        &db,
        &[csv_firearm("Colt", "Python", "V1", &[("scheduled_coverage_amount", "3500.00")])],
    );

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.row_errors.len(), 1);
    assert!(result.row_errors[0].message.to_lowercase().contains("policy"));
}

#[test]
fn an_unknown_policy_name_is_a_row_error() {
    let db = TestDb::new();

    let result = import_into(
        &db,
        &[csv_firearm(
            "Colt",
            "Python",
            "V1",
            &[("insurance_policy_name", "No such policy"), ("scheduled_coverage_amount", "1.00")],
        )],
    );

    assert_eq!(result.imported_count, 0);
    assert!(result.row_errors[0].message.contains("No such policy"));
}

#[test]
fn a_scheduled_firearm_survives_an_export_and_re_import() {
    let db = TestDb::new();
    let policy_id = rider(&db);
    let created =
        firearm_ops::create_firearm(&db.conn, &support::firearm("Colt", "Python", "V1"), false)
            .unwrap();
    hoplodex_lib::commands::insurance::ops::assign_firearm_coverage(
        &db.conn,
        created.id,
        Some(policy_id),
        Some(3500),
    )
    .unwrap();
    let plain =
        firearm_ops::create_firearm(&db.conn, &support::firearm("Glock", "19", "A1"), false)
            .unwrap();

    let dest = TempDir::new().unwrap();
    let exported = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "backup",
        SpreadsheetFormat::Csv,
        &[created.id, plain.id],
        &mut |_, _| {},
    )
    .unwrap();
    firearm_ops::delete_firearm(&db.conn, created.id, true).unwrap();
    firearm_ops::delete_firearm(&db.conn, plain.id, true).unwrap();

    let result = import_export_ops::import_collection(
        &db.conn,
        &exported.spreadsheet_path,
        SpreadsheetFormat::Csv,
        &ImportSessionStore::new(),
        &mut |_, _| {},
    )
    .unwrap();

    assert_eq!(result.imported_count, 2, "{:?}", result.row_errors);
    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    let all: Vec<_> = listing.groups.iter().flat_map(|g| &g.firearms).collect();
    let colt = all.iter().find(|f| f.make == "Colt").unwrap();
    assert_eq!(colt.insurance_policy_id, Some(policy_id));
    assert_eq!(colt.scheduled_coverage_amount, Some(3500));
    let glock = all.iter().find(|f| f.make == "Glock").unwrap();
    assert_eq!(glock.insurance_policy_id, None);
}

// --- Whole-dollar amounts (FR-037, spreadsheet-format.md "Amounts") ---

fn imported_value(cell: &str) -> Option<i64> {
    let db = TestDb::new();
    let result =
        import_into(&db, &[csv_firearm("Glock", "19", "A1", &[("estimated_value", cell)])]);
    assert_eq!(result.imported_count, 1, "{cell:?}: {:?}", result.row_errors);
    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    listing.groups[0].firearms[0].estimated_value
}

#[test]
fn import_accepts_whole_dollar_amounts_however_they_are_written() {
    assert_eq!(imported_value("450"), Some(450));
    assert_eq!(imported_value("450.00"), Some(450));
    assert_eq!(imported_value("\"$1,250\""), Some(1250));
    assert_eq!(imported_value("\" $ 1,250.00 \""), Some(1250));
    assert_eq!(imported_value("0"), Some(0));
}

#[test]
fn import_rejects_cents_negatives_and_text_as_row_errors_naming_the_column() {
    for column in
        ["estimated_value", "acquisition_price", "disposition_price", "scheduled_coverage_amount"]
    {
        for bad in ["450.50", "-5", "abc", "12.345", "1e3"] {
            let db = TestDb::new();
            let result = import_into(&db, &[csv_firearm("Glock", "19", "A1", &[(column, bad)])]);
            assert_eq!(result.imported_count, 0, "{column}={bad} must not import");
            assert_eq!(result.row_errors.len(), 1, "{column}={bad}");
            assert!(
                result.row_errors[0].message.contains(column),
                "{column}={bad}: message {:?} should name the column",
                result.row_errors[0].message
            );
        }
    }
}

#[test]
fn a_bad_amount_fails_only_its_own_row() {
    let db = TestDb::new();
    let result = import_into(
        &db,
        &[
            csv_firearm("Glock", "19", "A1", &[("estimated_value", "450.50")]),
            csv_firearm("Sig", "P226", "B2", &[("estimated_value", "450")]),
        ],
    );
    assert_eq!(result.imported_count, 1);
    assert_eq!(result.row_errors.len(), 1);
    assert_eq!(result.row_errors[0].row, 1);
}

#[test]
fn import_accepts_a_numeric_xlsx_cell() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("import.xlsx");

    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sheet = workbook.add_worksheet();
    let values = [
        ("make", "Glock"),
        ("model", "19"),
        ("serial_number", "X1"),
        ("no_serial_attested", "FALSE"),
        ("caliber", "9mm"),
        ("firearm_type", "Handgun"),
    ];
    for (col, name) in hoplodex_lib::services::spreadsheet::COLUMNS.iter().enumerate() {
        sheet.write_string(0, col as u16, *name).unwrap();
        if let Some((_, value)) = values.iter().find(|(n, _)| n == name) {
            sheet.write_string(1, col as u16, *value).unwrap();
        }
        if *name == "estimated_value" {
            sheet.write_number(1, col as u16, 450.0).unwrap();
        }
    }
    workbook.save(&path).unwrap();

    let result = import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Xlsx,
        &ImportSessionStore::new(),
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    assert_eq!(listing.groups[0].firearms[0].estimated_value, Some(450));
}

#[test]
fn export_writes_whole_dollars_with_no_separators_and_they_import_back_unchanged() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let mut input = support::firearm("Glock", "19", "A1");
    input.estimated_value = Some(1250);
    input.acquisition_price = Some(1000000);
    firearm_ops::create_firearm(&db.conn, &input, false).unwrap();
    let id =
        firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap().groups[0].firearms[0].id;

    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "amounts",
        SpreadsheetFormat::Csv,
        &[id],
        &mut |_, _| {},
    )
    .unwrap();

    let mut reader = csv::Reader::from_path(&result.spreadsheet_path).unwrap();
    let headers = reader.headers().unwrap().clone();
    let record = reader.records().next().unwrap().unwrap();
    let cell = |name: &str| record.get(headers.iter().position(|h| h == name).unwrap()).unwrap();
    assert_eq!(cell("estimated_value"), "1250");
    assert_eq!(cell("acquisition_price"), "1000000");

    let db2 = TestDb::new();
    let reimported = import_export_ops::import_collection(
        &db2.conn,
        &result.spreadsheet_path,
        SpreadsheetFormat::Csv,
        &ImportSessionStore::new(),
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(reimported.imported_count, 1, "{:?}", reimported.row_errors);
    let listing = firearm_ops::list_firearms(&db2.conn, &Default::default()).unwrap();
    assert_eq!(listing.groups[0].firearms[0].estimated_value, Some(1250));
}

// --- Physical details (FR-039, spreadsheet-format.md "Physical details") ---

fn imported_detail(cells: &[(&str, &str)]) -> hoplodex_lib::models::firearm::Firearm {
    let db = TestDb::new();
    let result = import_into(&db, &[csv_firearm("Glock", "19", "A1", cells)]);
    assert_eq!(result.imported_count, 1, "{cells:?}: {:?}", result.row_errors);
    let id =
        firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap().groups[0].firearms[0].id;
    firearm_ops::get_firearm(&db.conn, id).unwrap()
}

#[test]
fn import_accepts_blank_physical_details() {
    let firearm = imported_detail(&[]);
    assert_eq!(firearm.barrel_length_hundredths, None);
    assert_eq!(firearm.overall_length_hundredths, None);
    assert_eq!(firearm.weight_tenths_oz, None);
    assert_eq!(firearm.capacity, None);
    assert_eq!(firearm.finish, None);
    assert_eq!(firearm.condition, None);
}

#[test]
fn import_reads_lengths_weight_capacity_finish_and_condition() {
    use hoplodex_lib::models::firearm::Condition;

    let firearm = imported_detail(&[
        ("barrel_length_in", "16.25"),
        ("overall_length_in", "18"),
        ("weight_oz", "40.5"),
        ("capacity", "15"),
        ("finish", "Parkerized"),
        ("condition", "Excellent"),
    ]);
    assert_eq!(firearm.barrel_length_hundredths, Some(1625));
    assert_eq!(firearm.overall_length_hundredths, Some(1800));
    assert_eq!(firearm.weight_tenths_oz, Some(405));
    assert_eq!(firearm.capacity, Some(15));
    assert_eq!(firearm.finish.as_deref(), Some("Parkerized"));
    assert_eq!(firearm.condition, Some(Condition::Excellent));
}

#[test]
fn import_accepts_a_zero_fraction_beyond_the_precision() {
    let firearm = imported_detail(&[("barrel_length_in", "16.250"), ("weight_oz", "40.50")]);
    assert_eq!(firearm.barrel_length_hundredths, Some(1625));
    assert_eq!(firearm.weight_tenths_oz, Some(405));
}

#[test]
fn import_rounds_extra_precision_to_the_nearest_storable_unit() {
    for (barrel, weight, hundredths, tenths) in [
        ("16.255", "40.55", 1626, 406),
        ("16.254", "40.54", 1625, 405),
        ("18.005", "0.05", 1801, 1),
        ("16.2549999", "40.5999", 1625, 406),
    ] {
        let firearm = imported_detail(&[("barrel_length_in", barrel), ("weight_oz", weight)]);
        assert_eq!(firearm.barrel_length_hundredths, Some(hundredths), "{barrel}");
        assert_eq!(firearm.weight_tenths_oz, Some(tenths), "{weight}");
    }
}

#[test]
fn import_reads_a_condition_in_any_letter_case_or_as_the_stored_form() {
    use hoplodex_lib::models::firearm::Condition;

    for (cell, expected) in [
        ("like new", Condition::LikeNew),
        ("LIKE NEW", Condition::LikeNew),
        ("Like new", Condition::LikeNew),
        ("new_in_box", Condition::NewInBox),
        ("New in box", Condition::NewInBox),
        ("POOR", Condition::Poor),
    ] {
        assert_eq!(imported_detail(&[("condition", cell)]).condition, Some(expected), "{cell}");
    }
}

#[test]
fn import_rejects_bad_physical_details_as_row_errors_naming_the_column() {
    for (column, bad) in [
        ("barrel_length_in", "0"),
        ("barrel_length_in", "-1"),
        ("barrel_length_in", "abc"),
        ("overall_length_in", "0.00"),
        ("overall_length_in", "0.004"),
        ("weight_oz", "0"),
        ("weight_oz", "0.04"),
        ("weight_oz", "-2"),
        ("weight_oz", "heavy"),
        ("capacity", "0"),
        ("capacity", "12.5"),
        ("capacity", "-3"),
        ("capacity", "many"),
        ("condition", "mint"),
    ] {
        let db = TestDb::new();
        let result = import_into(&db, &[csv_firearm("Glock", "19", "A1", &[(column, bad)])]);
        assert_eq!(result.imported_count, 0, "{column}={bad} must not import");
        assert_eq!(result.row_errors.len(), 1, "{column}={bad}");
        assert!(
            result.row_errors[0].message.contains(column),
            "{column}={bad}: message {:?} should name the column",
            result.row_errors[0].message
        );
    }
}

#[test]
fn an_export_re_imports_with_all_six_intact() {
    use hoplodex_lib::models::firearm::Condition;

    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let mut input = support::firearm("Glock", "19", "A1");
    input.barrel_length_hundredths = Some(1625);
    input.overall_length_hundredths = Some(1800);
    input.weight_tenths_oz = Some(405);
    input.capacity = Some(15);
    input.finish = Some("Cerakote".into());
    input.condition = Some(Condition::NewInBox);
    let created = firearm_ops::create_firearm(&db.conn, &input, false).unwrap();

    for format in [SpreadsheetFormat::Csv, SpreadsheetFormat::Xlsx] {
        let result = import_export_ops::export_collection(
            &db.conn,
            dest.path(),
            "detail",
            format,
            &[created.id],
            &mut |_, _| {},
        )
        .unwrap();

        let db2 = TestDb::new();
        let reimported = import_export_ops::import_collection(
            &db2.conn,
            &result.spreadsheet_path,
            format,
            &ImportSessionStore::new(),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(reimported.imported_count, 1, "{:?}", reimported.row_errors);
        let id = firearm_ops::list_firearms(&db2.conn, &Default::default()).unwrap().groups[0]
            .firearms[0]
            .id;
        let back = firearm_ops::get_firearm(&db2.conn, id).unwrap();
        assert_eq!(back.barrel_length_hundredths, Some(1625));
        assert_eq!(back.overall_length_hundredths, Some(1800));
        assert_eq!(back.weight_tenths_oz, Some(405));
        assert_eq!(back.capacity, Some(15));
        assert_eq!(back.finish.as_deref(), Some("Cerakote"));
        assert_eq!(back.condition, Some(Condition::NewInBox));
    }
}

// --- specs/002-firearm-identification User Story 4: browse, export, import ---

mod identification_spreadsheet {
    use super::*;
    use hoplodex_lib::models::firearm::Origin;

    #[test]
    fn export_writes_the_seven_new_columns_after_condition_and_before_photo_filenames() {
        let db = TestDb::new();
        let dest = TempDir::new().unwrap();
        let created = firearm_ops::create_firearm(
            &db.conn,
            &hoplodex_lib::models::firearm::FirearmInput {
                origin: Some(Origin::Imported),
                year_of_manufacture: Some(1943),
                country_of_manufacture: Some("Belgium".into()),
                importer_name: Some("Global Arms Import Co.".into()),
                original_make: Some("FN".into()),
                original_model: Some("High Power".into()),
                original_serial_number: Some("FN-1".into()),
                ..support::firearm("Ridgeline Arms", "Hi-Power", "RA-1")
            },
            false,
        )
        .unwrap();

        let result = import_export_ops::export_collection(
            &db.conn,
            dest.path(),
            "identification",
            SpreadsheetFormat::Csv,
            &[created.id],
            &mut |_, _| {},
        )
        .unwrap();

        let mut reader = csv::Reader::from_path(&result.spreadsheet_path).unwrap();
        let headers = reader.headers().unwrap().clone();
        let record = reader.records().next().unwrap().unwrap();
        let cell =
            |name: &str| record.get(headers.iter().position(|h| h == name).unwrap()).unwrap();

        assert_eq!(cell("origin"), "Imported");
        assert_eq!(cell("year_of_manufacture"), "1943");
        assert_eq!(cell("country_of_manufacture"), "Belgium");
        assert_eq!(cell("importer_name"), "Global Arms Import Co.");
        assert_eq!(cell("original_make"), "FN");
        assert_eq!(cell("original_model"), "High Power");
        assert_eq!(cell("original_serial_number"), "FN-1");

        let order: Vec<_> = headers.iter().collect();
        let condition_at = headers.iter().position(|h| h == "condition").unwrap();
        let photos_at = headers.iter().position(|h| h == "photo_filenames").unwrap();
        assert_eq!(
            &order[condition_at + 1..photos_at],
            [
                "origin",
                "year_of_manufacture",
                "country_of_manufacture",
                "importer_name",
                "original_make",
                "original_model",
                "original_serial_number",
            ]
        );
    }

    #[test]
    fn a_reimported_row_always_exports_a_blank_country() {
        let db = TestDb::new();
        let dest = TempDir::new().unwrap();
        let created = firearm_ops::create_firearm(
            &db.conn,
            &hoplodex_lib::models::firearm::FirearmInput {
                origin: Some(Origin::Reimported),
                ..support::firearm("Inland", "M1 Carbine", "IN-1")
            },
            false,
        )
        .unwrap();

        let result = import_export_ops::export_collection(
            &db.conn,
            dest.path(),
            "reimported",
            SpreadsheetFormat::Csv,
            &[created.id],
            &mut |_, _| {},
        )
        .unwrap();

        let mut reader = csv::Reader::from_path(&result.spreadsheet_path).unwrap();
        let headers = reader.headers().unwrap().clone();
        let record = reader.records().next().unwrap().unwrap();
        assert_eq!(
            record.get(headers.iter().position(|h| h == "origin").unwrap()).unwrap(),
            "Re-imported"
        );
        assert_eq!(
            record
                .get(headers.iter().position(|h| h == "country_of_manufacture").unwrap())
                .unwrap(),
            ""
        );
    }

    #[test]
    fn an_export_followed_by_import_into_an_empty_collection_reproduces_every_new_field() {
        // US4-2, SC-005
        let db = TestDb::new();
        let dest = TempDir::new().unwrap();
        let domestic = firearm_ops::create_firearm(
            &db.conn,
            &hoplodex_lib::models::firearm::FirearmInput {
                origin: Some(Origin::Domestic),
                year_of_manufacture: Some(1955),
                ..support::firearm("Colt", "1911", "C-1")
            },
            false,
        )
        .unwrap();
        let imported = firearm_ops::create_firearm(
            &db.conn,
            &hoplodex_lib::models::firearm::FirearmInput {
                origin: Some(Origin::Imported),
                year_of_manufacture: Some(1943),
                country_of_manufacture: Some("Belgium".into()),
                importer_name: Some("Global Arms Import Co.".into()),
                original_make: Some("FN".into()),
                original_model: Some("High Power".into()),
                original_serial_number: Some("FN-2".into()),
                ..support::firearm("Ridgeline Arms", "Hi-Power", "RA-2")
            },
            false,
        )
        .unwrap();
        let unspecified = firearm_ops::create_firearm(
            &db.conn,
            &support::firearm("Ruger", "10/22", "RU-1"),
            false,
        )
        .unwrap();

        let exported = import_export_ops::export_collection(
            &db.conn,
            dest.path(),
            "roundtrip",
            SpreadsheetFormat::Csv,
            &[domestic.id, imported.id, unspecified.id],
            &mut |_, _| {},
        )
        .unwrap();

        let db2 = TestDb::new();
        let result = import_export_ops::import_collection(
            &db2.conn,
            &exported.spreadsheet_path,
            SpreadsheetFormat::Csv,
            &ImportSessionStore::new(),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(result.imported_count, 3, "{:?}", result.row_errors);

        let listing = firearm_ops::list_firearms(&db2.conn, &Default::default()).unwrap();
        let all: Vec<_> = listing.groups.iter().flat_map(|g| &g.firearms).collect();

        let back_domestic =
            firearm_ops::get_firearm(&db2.conn, all.iter().find(|f| f.make == "Colt").unwrap().id)
                .unwrap();
        assert_eq!(back_domestic.origin, Some(Origin::Domestic));
        assert_eq!(back_domestic.year_of_manufacture, Some(1955));

        let back_imported = firearm_ops::get_firearm(
            &db2.conn,
            all.iter().find(|f| f.make == "Ridgeline Arms").unwrap().id,
        )
        .unwrap();
        assert_eq!(back_imported.origin, Some(Origin::Imported));
        assert_eq!(back_imported.year_of_manufacture, Some(1943));
        assert_eq!(back_imported.country_of_manufacture.as_deref(), Some("Belgium"));
        assert_eq!(back_imported.importer_name.as_deref(), Some("Global Arms Import Co."));
        assert_eq!(back_imported.original_make.as_deref(), Some("FN"));
        assert_eq!(back_imported.original_model.as_deref(), Some("High Power"));
        assert_eq!(back_imported.original_serial_number.as_deref(), Some("FN-2"));

        let back_unspecified =
            firearm_ops::get_firearm(&db2.conn, all.iter().find(|f| f.make == "Ruger").unwrap().id)
                .unwrap();
        assert_eq!(back_unspecified.origin, None);
    }

    #[test]
    fn an_origin_cell_outside_the_three_labels_is_a_row_error_naming_the_value() {
        let db = TestDb::new();
        let result =
            import_into(&db, &[csv_firearm("Glock", "19", "A1", &[("origin", "Reimported")])]);
        assert_eq!(result.imported_count, 0);
        assert_eq!(result.row_errors.len(), 1);
        assert!(result.row_errors[0].message.contains("Reimported"), "{:?}", result.row_errors);
    }

    #[test]
    fn origin_is_matched_ignoring_letter_case() {
        let db = TestDb::new();
        let result =
            import_into(&db, &[csv_firearm("Glock", "19", "A1", &[("origin", "DOMESTIC")])]);
        assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
    }

    #[test]
    fn importer_and_original_marks_on_a_domestic_or_blank_origin_row_are_row_errors() {
        for column in ["importer_name", "original_make", "original_model", "original_serial_number"]
        {
            let db = TestDb::new();
            let result =
                import_into(&db, &[csv_firearm("Glock", "19", "A1", &[(column, "Something")])]);
            assert_eq!(result.imported_count, 0, "{column}");
            assert_eq!(result.row_errors.len(), 1, "{column}");
        }
    }

    #[test]
    fn country_of_manufacture_on_a_reimported_row_is_a_row_error() {
        let db = TestDb::new();
        let result = import_into(
            &db,
            &[csv_firearm(
                "Inland",
                "M1 Carbine",
                "IN-1",
                &[("origin", "Re-imported"), ("country_of_manufacture", "Germany")],
            )],
        );
        assert_eq!(result.imported_count, 0);
        assert_eq!(result.row_errors.len(), 1);
    }

    #[test]
    fn a_malformed_year_is_a_row_error_naming_the_column() {
        for bad in ["circa 1943", "43", "19430", "1943.5"] {
            let db = TestDb::new();
            let result = import_into(
                &db,
                &[csv_firearm("Glock", "19", "A1", &[("year_of_manufacture", bad)])],
            );
            assert_eq!(result.imported_count, 0, "{bad}");
            assert_eq!(result.row_errors.len(), 1, "{bad}");
            assert!(
                result.row_errors[0].message.contains("year_of_manufacture")
                    || result.row_errors[0].message.to_lowercase().contains("year of manufacture"),
                "{bad}: {:?}",
                result.row_errors[0].message
            );
        }
    }

    #[test]
    fn an_imported_row_with_every_new_column_blank_imports_normally() {
        let db = TestDb::new();
        let result =
            import_into(&db, &[csv_firearm("FN", "1922", "FN-1", &[("origin", "Imported")])]);
        assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
    }

    #[test]
    fn a_row_matching_main_marks_but_distinguished_by_year_is_a_new_record_with_no_conflict() {
        // US4-5a
        let db = TestDb::new();
        firearm_ops::create_firearm(
            &db.conn,
            &hoplodex_lib::models::firearm::FirearmInput {
                year_of_manufacture: Some(1943),
                ..support::firearm("Colt", "1873", "SAA-1")
            },
            false,
        )
        .unwrap();

        let result = import_into(
            &db,
            &[csv_firearm("Colt", "1873", "SAA-1", &[("year_of_manufacture", "1944")])],
        );

        assert_eq!(result.conflicts.len(), 0, "distinguished by year: no conflict prompt at all");
        assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
    }

    #[test]
    fn a_row_whose_original_marks_match_an_existing_firearm_still_imports_and_warns() {
        // US4-6, FR-009
        let db = TestDb::new();
        firearm_ops::create_firearm(
            &db.conn,
            &hoplodex_lib::models::firearm::FirearmInput {
                origin: Some(Origin::Imported),
                original_make: Some("FN".into()),
                original_model: Some("High Power".into()),
                original_serial_number: Some("FN-3".into()),
                ..support::firearm("Ridgeline Arms", "Hi-Power", "RA-3")
            },
            false,
        )
        .unwrap();

        let result = import_into(
            &db,
            &[csv_firearm(
                "Century Arms",
                "Clone",
                "CA-3",
                &[
                    ("origin", "Imported"),
                    ("original_make", "FN"),
                    ("original_model", "High Power"),
                    ("original_serial_number", "FN-3"),
                ],
            )],
        );

        assert_eq!(
            result.imported_count, 1,
            "the row still imports (US4-6): {:?}",
            result.row_errors
        );
        assert_eq!(result.row_errors.len(), 0, "a warning is never a row error");
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0].row, 1);
        assert!(result.warnings[0].message.contains("Ridgeline Arms"), "{:?}", result.warnings);
    }
}

// --- Stopped by a sleep (FR-037, research.md §13) ----------------------------

#[test]
fn a_stopped_import_keeps_the_rows_imported_before_it() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let rows: Vec<String> =
        (1..=5).map(|n| csv_firearm("Colt", "Python", &format!("P{n}"), &[])).collect();
    let path = write_csv(&dir, &csv_file(&rows));
    let cancelled = std::cell::Cell::new(false);
    let recorded = std::cell::Cell::new(0);

    let stopped = import_export_ops::import_collection_stoppable(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        // Asked to stop once three rows are done.
        &mut |done, _| cancelled.set(done == 3),
        &|| cancelled.get(),
        &|imported| recorded.set(imported),
    )
    .unwrap_err();

    assert_eq!(stopped.code, "OPERATION_STOPPED");
    let details = stopped.details.as_deref().unwrap();
    assert_eq!(details["operation"], "import");
    assert_eq!(details["importedCount"], 3);
    assert_eq!(recorded.get(), 3);
    let serials: Vec<String> = db
        .conn
        .prepare("SELECT serial_number FROM firearms ORDER BY serial_number")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(serials, ["P1", "P2", "P3"], "each imported row is kept whole");
}

#[test]
fn a_stopped_export_removes_its_partial_output() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let ids: Vec<i64> = ["A1", "A2", "A3"]
        .iter()
        .map(|serial| {
            firearm_ops::create_firearm(&db.conn, &support::firearm("Glock", "19", serial), false)
                .unwrap()
                .id
        })
        .collect();
    let cancelled = std::cell::Cell::new(false);

    let stopped = import_export_ops::export_collection_stoppable(
        &db.conn,
        dest.path(),
        "stopped",
        SpreadsheetFormat::Csv,
        &ids,
        &mut |done, _| cancelled.set(done == 2),
        &|| cancelled.get(),
    )
    .unwrap_err();

    assert_eq!(stopped.code, "OPERATION_STOPPED");
    assert_eq!(stopped.details.as_deref().unwrap()["operation"], "export");
    assert_eq!(std::fs::read_dir(dest.path()).unwrap().count(), 0, "nothing is left behind");
}

#[test]
fn a_stopped_export_leaves_a_photos_folder_it_did_not_make() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let existing = dest.path().join("stopped_photos");
    std::fs::create_dir(&existing).unwrap();
    std::fs::write(existing.join("mine.jpg"), b"not HoploDex's").unwrap();
    let id = firearm_ops::create_firearm(&db.conn, &support::firearm("Glock", "19", "A1"), false)
        .unwrap()
        .id;

    let stopped = import_export_ops::export_collection_stoppable(
        &db.conn,
        dest.path(),
        "stopped",
        SpreadsheetFormat::Csv,
        &[id],
        &mut |_, _| {},
        &|| true,
    )
    .unwrap_err();

    assert_eq!(stopped.code, "OPERATION_STOPPED");
    assert!(existing.join("mine.jpg").exists());
}
