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
        &support::import_files(&path, SpreadsheetFormat::Csv),
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
        &support::import_files(&path, SpreadsheetFormat::Csv),
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
        &support::import_files(&path, SpreadsheetFormat::Csv),
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
        &support::import_files(&path, SpreadsheetFormat::Csv),
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

// --- A disposed row's price is optional (research.md §9, 006 FR-014) ---

/// A disposed Glock 19 row with the given cells replacing the usual ones.
fn disposed_cells(overrides: &[(&str, &str)]) -> String {
    let mut cells = vec![
        ("status", "disposed"),
        ("disposition_type", "sold"),
        ("disposition_recipient", "Jane"),
        ("disposition_date", "2025-03-01"),
        ("disposition_price", "400.00"),
    ];
    for (column, value) in overrides {
        cells.retain(|(name, _)| name != column);
        cells.push((column, value));
    }
    csv_firearm("Glock", "19", "ABC123", &cells)
}

#[test]
fn a_disposed_row_with_a_blank_price_imports_with_no_price() {
    // 001 made this a row error; a disposed accessory's price is optional
    // and its export must re-import (SC-002), so a firearm's is too.
    let db = TestDb::new();

    let result = import_into(&db, &[disposed_cells(&[("disposition_price", "")])]);

    assert!(result.row_errors.is_empty(), "{:?}", result.row_errors);
    assert_eq!(result.imported_count, 1);
    let listing = firearm_ops::list_firearms(
        &db.conn,
        &serde_json::from_value(serde_json::json!({ "includeDisposed": true })).unwrap(),
    )
    .unwrap();
    let all: Vec<_> = listing.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(all.len(), 1);
    let stored = firearm_ops::get_firearm(&db.conn, all[0].id).unwrap();
    assert_eq!(stored.disposition_price, None);
    assert_eq!(stored.disposition_recipient.as_deref(), Some("Jane"));
}

#[test]
fn a_disposed_row_missing_its_type_recipient_or_date_is_still_a_row_error() {
    for (column, field) in [
        ("disposition_type", "type"),
        ("disposition_recipient", "recipient"),
        ("disposition_date", "date"),
    ] {
        let result = import_one_row(disposed_cells(&[(column, ""), ("disposition_price", "")]));

        assert_eq!(result.imported_count, 0, "{column}");
        assert_eq!(result.row_errors.len(), 1, "{column}: {:?}", result.row_errors);
        assert!(
            result.row_errors[0].message.to_lowercase().contains("disposition")
                || result.row_errors[0].message.to_lowercase().contains(field),
            "{column}: {}",
            result.row_errors[0].message
        );
    }
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
        &support::import_files(&path, SpreadsheetFormat::Csv),
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
    assert!(!hoplodex_lib::services::spreadsheet::FIREARM_COLUMNS.contains(&"coverage_kind"));
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
        &support::firearm_records(&[created.id, plain.id]),
        &mut |_, _| {},
    )
    .unwrap();
    firearm_ops::delete_firearm(&db.conn, created.id, true).unwrap();
    firearm_ops::delete_firearm(&db.conn, plain.id, true).unwrap();

    let result = import_export_ops::import_collection(
        &db.conn,
        &support::import_files(&exported.spreadsheet_path, SpreadsheetFormat::Csv),
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
    for (col, name) in hoplodex_lib::services::spreadsheet::FIREARM_COLUMNS.iter().enumerate() {
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
        &support::import_files(&path, SpreadsheetFormat::Xlsx),
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
        &support::firearm_records(&[id]),
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
        &support::import_files(&result.spreadsheet_path, SpreadsheetFormat::Csv),
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
            &support::firearm_records(&[created.id]),
            &mut |_, _| {},
        )
        .unwrap();

        let db2 = TestDb::new();
        let reimported = import_export_ops::import_collection(
            &db2.conn,
            &support::import_files(&result.spreadsheet_path, format),
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
                registration_class_id: None,
                registration_form: None,
                registration_approved: None,
                registered_to: None,
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
            &support::firearm_records(&[created.id]),
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
        // The four registration columns of specs/005 follow original_serial_number.
        let photos_at = headers.iter().position(|h| h == "registered_as").unwrap();
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
            &support::firearm_records(&[created.id]),
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
                registration_class_id: None,
                registration_form: None,
                registration_approved: None,
                registered_to: None,
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
            &support::firearm_records(&[domestic.id, imported.id, unspecified.id]),
            &mut |_, _| {},
        )
        .unwrap();

        let db2 = TestDb::new();
        let result = import_export_ops::import_collection(
            &db2.conn,
            &support::import_files(&exported.spreadsheet_path, SpreadsheetFormat::Csv),
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
                registration_class_id: None,
                registration_form: None,
                registration_approved: None,
                registered_to: None,
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
        &support::import_files(&path, SpreadsheetFormat::Csv),
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
        &support::firearm_records(&ids),
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
        &support::firearm_records(&[id]),
        &mut |_, _| {},
        &|| true,
    )
    .unwrap_err();

    assert_eq!(stopped.code, "OPERATION_STOPPED");
    assert!(existing.join("mine.jpg").exists());
}

// --- specs/004-cartridges-action-types User Story 4: cartridge and action
// through export and import (contracts/spreadsheet-format.md) ---

mod cartridge_spreadsheet {
    use super::*;
    use hoplodex_lib::commands::import_export::ImportResult;
    use hoplodex_lib::models::firearm::{Firearm, FirearmInput};
    use hoplodex_lib::services::cartridges::CaliberSource;
    use hoplodex_lib::services::entry_text::EntryField;
    use hoplodex_lib::services::spreadsheet::FIREARM_COLUMNS;

    const HANDGUN: i64 = 1;
    const RIFLE: i64 = 2;
    const SEMI_AUTOMATIC: i64 = 1;
    const BOLT_ACTION: i64 = 3;

    fn import_text(db: &TestDb, text: &str) -> ImportResult {
        let dir = TempDir::new().unwrap();
        let path = write_csv(&dir, text);
        import_export_ops::import_collection(
            &db.conn,
            &support::import_files(&path, SpreadsheetFormat::Csv),
            &ImportSessionStore::new(),
            &mut |_, _| {},
        )
        .unwrap()
    }

    fn import_rows(db: &TestDb, rows: &[String]) -> ImportResult {
        import_text(db, &csv_file(rows))
    }

    /// Every firearm, oldest first.
    fn all_firearms(db: &TestDb) -> Vec<Firearm> {
        let mut ids = import_export_ops::all_firearm_ids(&db.conn).unwrap();
        ids.sort();
        ids.into_iter().map(|id| firearm_ops::get_firearm(&db.conn, id).unwrap()).collect()
    }

    fn record(db: &TestDb, input: FirearmInput) {
        firearm_ops::create_firearm(&db.conn, &input, false).unwrap();
    }

    fn on_record(db: &TestDb, make: &str, serial: &str, cartridge: Option<&str>) {
        record(
            db,
            FirearmInput {
                cartridge: cartridge.map(str::to_owned),
                ..support::firearm(make, "Model", serial)
            },
        );
    }

    fn export_all(db: &TestDb, dest: &TempDir) -> std::path::PathBuf {
        let ids = import_export_ops::all_firearm_ids(&db.conn).unwrap();
        import_export_ops::export_collection(
            &db.conn,
            dest.path(),
            "backup",
            SpreadsheetFormat::Csv,
            &support::firearm_records(&ids),
            &mut |_, _| {},
        )
        .unwrap()
        .spreadsheet_path
    }

    fn reimport(path: &std::path::Path, into: &TestDb) -> ImportResult {
        import_export_ops::import_collection(
            &into.conn,
            &support::import_files(path, SpreadsheetFormat::Csv),
            &ImportSessionStore::new(),
            &mut |_, _| {},
        )
        .unwrap()
    }

    fn only_error(result: &ImportResult) -> &str {
        assert_eq!(result.row_errors.len(), 1, "{:?}", result.row_errors);
        &result.row_errors[0].message
    }

    #[test]
    fn the_header_puts_cartridge_and_action_type_directly_after_caliber() {
        assert_eq!(FIREARM_COLUMNS.len(), 42);
        let caliber_at = FIREARM_COLUMNS.iter().position(|c| *c == "caliber").unwrap();
        assert_eq!(
            &FIREARM_COLUMNS[caliber_at + 1..caliber_at + 4],
            ["cartridge", "action_type", "firearm_type"]
        );
    }

    #[test]
    fn export_writes_the_cartridge_and_the_actions_name_and_blank_when_none() {
        let db = TestDb::new();
        record(
            &db,
            FirearmInput {
                cartridge: Some("9x19mm Parabellum".into()),
                action_type_id: Some(SEMI_AUTOMATIC),
                ..support::firearm("Glock", "19", "A1")
            },
        );
        record(&db, support::firearm("Ruger", "Mk IV", "B2"));

        let dest = TempDir::new().unwrap();
        let path = export_all(&db, &dest);
        let mut reader = csv::Reader::from_path(path).unwrap();
        let headers = reader.headers().unwrap().clone();
        let at = |name: &str| headers.iter().position(|h| h == name).unwrap();
        assert_eq!(headers.iter().count(), 42);
        assert_eq!(at("cartridge"), at("caliber") + 1);
        assert_eq!(at("action_type"), at("caliber") + 2);
        let rows: Vec<_> = reader.records().map(Result::unwrap).collect();
        assert_eq!(&rows[0][at("cartridge")], "9x19mm Parabellum");
        assert_eq!(&rows[0][at("action_type")], "Semi-automatic");
        assert_eq!(&rows[1][at("cartridge")], "");
        assert_eq!(&rows[1][at("action_type")], "");
    }

    #[test]
    fn a_collection_without_same_notation_variants_round_trips_exactly() {
        let db = TestDb::new();
        record(
            &db,
            FirearmInput {
                cartridge: Some("9x19mm Parabellum".into()),
                action_type_id: Some(SEMI_AUTOMATIC),
                ..support::firearm("Glock", "19", "A1")
            },
        );
        record(
            &db,
            FirearmInput {
                caliber: ".30".into(),
                cartridge: Some(".308 Winchester".into()),
                action_type_id: Some(BOLT_ACTION),
                firearm_type_id: RIFLE,
                ..support::firearm("Remington", "700", "B2")
            },
        );
        record(&db, support::firearm("Ruger", "Mk IV", "C3"));

        let dest = TempDir::new().unwrap();
        let path = export_all(&db, &dest);
        let fresh = TestDb::new();
        let result = reimport(&path, &fresh);

        assert_eq!(result.imported_count, 3, "{:?}", result.row_errors);
        assert!(result.snapped_values.is_empty(), "{:?}", result.snapped_values);
        assert!(result.derived_calibers.is_empty());
        let shape = |db: &TestDb| {
            let mut all: Vec<_> = all_firearms(db)
                .into_iter()
                .map(|f| (f.make, f.model, f.cartridge, f.caliber, f.action_type_id))
                .collect();
            all.sort();
            all
        };
        assert_eq!(shape(&fresh), shape(&db));
    }

    #[test]
    fn re_importing_a_collection_with_two_spellings_merges_them_and_reports_each_merge() {
        let db = TestDb::new();
        for (serial, make) in [
            ("A1", "Springfield Armory"),
            ("A2", "Springfield Armory"),
            ("A3", "Springfield Armory"),
            ("A4", "Springfield armory"),
        ] {
            on_record(&db, make, serial, None);
        }
        on_record(&db, "Glock", "G1", Some("9X19mm Parabellum"));

        let dest = TempDir::new().unwrap();
        let path = export_all(&db, &dest);
        let fresh = TestDb::new();
        let result = reimport(&path, &fresh);

        assert_eq!(result.imported_count, 5, "{:?}", result.row_errors);
        let makes: Vec<_> = all_firearms(&fresh).into_iter().map(|f| f.make).collect();
        assert_eq!(makes.iter().filter(|m| *m == "Springfield Armory").count(), 4);
        let glock = all_firearms(&fresh).into_iter().find(|f| f.make == "Glock").unwrap();
        assert_eq!(glock.cartridge.as_deref(), Some("9x19mm Parabellum"));

        assert_eq!(result.snapped_values.len(), 2);
        let make = result.snapped_values.iter().find(|s| s.field == EntryField::Make).unwrap();
        assert_eq!(make.sheet_value, "Springfield armory");
        assert_eq!(make.recorded_value, "Springfield Armory");
        let cartridge =
            result.snapped_values.iter().find(|s| s.field == EntryField::Cartridge).unwrap();
        assert_eq!(cartridge.sheet_value, "9X19mm Parabellum");
        assert_eq!(cartridge.recorded_value, "9x19mm Parabellum");
    }

    #[test]
    fn a_blank_caliber_is_filled_from_a_catalog_cartridge_and_listed() {
        let db = TestDb::new();
        let result = import_rows(
            &db,
            &[
                csv_firearm("Glock", "19", "A1", &[("caliber", ""), ("cartridge", "x")]),
                csv_firearm(
                    "Sig",
                    "P320",
                    "B2",
                    &[("caliber", ""), ("cartridge", "9x19mm Parabellum")],
                ),
            ],
        );
        // Row 1's cartridge is unknown and has no readable bore.
        assert_eq!(result.row_errors.len(), 1);
        assert_eq!(result.imported_count, 1);
        let sig = &all_firearms(&db)[0];
        assert_eq!(sig.caliber, "9mm");
        assert_eq!(sig.cartridge.as_deref(), Some("9x19mm Parabellum"));
        assert_eq!(result.derived_calibers.len(), 1);
        let derived = &result.derived_calibers[0];
        assert_eq!(derived.row, 2);
        assert_eq!(derived.cartridge, "9x19mm Parabellum");
        assert_eq!(derived.caliber, "9mm");
        assert_eq!(derived.source, CaliberSource::Catalog);
    }

    #[test]
    fn a_blank_caliber_with_a_custom_cartridge_is_guessed_and_marked_as_a_guess() {
        let db = TestDb::new();
        let result = import_rows(
            &db,
            &[csv_firearm(
                "Thompson",
                "Contender",
                "A1",
                &[("caliber", ""), ("cartridge", ".30 Custom Improved")],
            )],
        );
        assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
        assert_eq!(all_firearms(&db)[0].caliber, ".30");
        assert_eq!(result.derived_calibers[0].source, CaliberSource::Guess);
        assert_eq!(result.derived_calibers[0].caliber, ".30");
    }

    #[test]
    fn a_caliber_that_cannot_be_derived_is_a_row_error_and_nothing_is_listed() {
        let db = TestDb::new();
        let unreadable = import_rows(
            &db,
            &[csv_firearm(
                "Acme",
                "One",
                "A1",
                &[("caliber", ""), ("cartridge", "Wildcat Special")],
            )],
        );
        assert_eq!(
            only_error(&unreadable),
            "caliber: Caliber is required; it couldn't be worked out from the cartridge \"Wildcat Special\"."
        );
        assert!(unreadable.derived_calibers.is_empty());

        let both_blank = import_rows(&db, &[csv_firearm("Acme", "Two", "A2", &[("caliber", "")])]);
        assert_eq!(only_error(&both_blank), "caliber: Caliber is required.");
        assert_eq!(all_firearms(&db).len(), 0);
    }

    #[test]
    fn a_caliber_given_in_the_sheet_is_used_as_given_whatever_the_cartridge() {
        let db = TestDb::new();
        let result = import_rows(
            &db,
            &[csv_firearm(
                "Acme",
                "One",
                "A1",
                &[("caliber", ".22 LR"), ("cartridge", "9x19mm Parabellum")],
            )],
        );
        assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
        assert_eq!(all_firearms(&db)[0].caliber, ".22 LR");
        assert!(result.derived_calibers.is_empty());
    }

    #[test]
    fn a_make_that_is_a_variant_of_one_on_record_takes_the_recorded_spelling() {
        let db = TestDb::new();
        on_record(&db, "Smith & Wesson", "S1", None);
        let result = import_rows(
            &db,
            &[
                csv_firearm("smith and wesson", "M&P", "A1", &[]),
                // A different notation is never snapped.
                csv_firearm("S&W", "Model 10", "A2", &[]),
            ],
        );
        assert_eq!(result.imported_count, 2, "{:?}", result.row_errors);
        let makes: Vec<_> = all_firearms(&db).into_iter().map(|f| f.make).collect();
        assert_eq!(makes, ["Smith & Wesson", "Smith & Wesson", "S&W"]);
        assert_eq!(result.snapped_values.len(), 1);
        let snapped = &result.snapped_values[0];
        assert_eq!(snapped.row, 1);
        assert_eq!(snapped.field, EntryField::Make);
        assert_eq!(snapped.sheet_value, "smith and wesson");
        assert_eq!(snapped.recorded_value, "Smith & Wesson");
    }

    #[test]
    fn the_sheets_own_majority_spelling_wins_and_a_tie_goes_to_the_earliest_row() {
        let db = TestDb::new();
        let result = import_rows(
            &db,
            &[
                csv_firearm("springfield armory", "A", "A1", &[]),
                csv_firearm("Ruger", "B", "B1", &[]),
                csv_firearm("Ruger", "C", "B2", &[]),
                csv_firearm("Springfield Armory", "D", "A2", &[]),
                csv_firearm("RUGER", "E", "B3", &[]),
                csv_firearm("Springfield Armory", "F", "A3", &[]),
                csv_firearm("Colt", "G", "C1", &[]),
                csv_firearm("colt", "H", "C2", &[]),
            ],
        );
        assert_eq!(result.imported_count, 8, "{:?}", result.row_errors);
        let makes: Vec<_> = all_firearms(&db).into_iter().map(|f| f.make).collect();
        assert_eq!(makes.iter().filter(|m| *m == "Springfield Armory").count(), 3);
        assert_eq!(makes.iter().filter(|m| *m == "Ruger").count(), 3);
        // One "Colt" and one "colt": the earlier row's spelling.
        assert_eq!(makes.iter().filter(|m| *m == "Colt").count(), 2);

        let rows: Vec<_> =
            result.snapped_values.iter().map(|s| (s.row, s.sheet_value.as_str())).collect();
        assert_eq!(rows, [(1, "springfield armory"), (5, "RUGER"), (8, "colt")]);
    }

    #[test]
    fn snapping_uses_the_values_on_record_when_the_import_started_not_rows_added_by_it() {
        let db = TestDb::new();
        on_record(&db, "Springfield armory", "A0", None);
        // Three rows spell it "Springfield Armory", but the record was there first.
        let result = import_rows(
            &db,
            &[
                csv_firearm("Springfield Armory", "A", "A1", &[]),
                csv_firearm("Springfield Armory", "B", "A2", &[]),
                csv_firearm("Springfield Armory", "C", "A3", &[]),
            ],
        );
        assert_eq!(result.imported_count, 3, "{:?}", result.row_errors);
        assert_eq!(result.snapped_values.len(), 3);
        assert!(result.snapped_values.iter().all(|s| s.recorded_value == "Springfield armory"));
    }

    #[test]
    fn a_variant_row_becomes_a_conflict_with_the_existing_firearm() {
        let db = TestDb::new();
        record(&db, support::firearm("Smith & Wesson", "Model 10", "S1"));
        let store = ImportSessionStore::new();
        let dir = TempDir::new().unwrap();
        let path = write_csv(
            &dir,
            &csv_file(&[csv_firearm(
                "smith and wesson",
                "model 10",
                "S1",
                &[("cartridge", "9X19mm Parabellum")],
            )]),
        );
        let result = import_export_ops::import_collection(
            &db.conn,
            &support::import_files(&path, SpreadsheetFormat::Csv),
            &store,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(result.imported_count, 0);
        assert_eq!(result.conflicts.len(), 1, "{:?}", result.row_errors);
        assert_eq!(result.conflicts[0].make.as_deref(), Some("Smith & Wesson"));
        assert_eq!(result.conflicts[0].model.as_deref(), Some("Model 10"));
        // The conflict row is listed too.
        assert!(result.snapped_values.iter().any(|s| s.field == EntryField::Make));
        assert!(result.snapped_values.iter().any(|s| s.field == EntryField::Cartridge));

        let resolution = hoplodex_lib::commands::import_export::ConflictResolution {
            conflict_id: result.conflicts[0].conflict_id.clone(),
            action: "overwrite".into(),
        };
        import_export_ops::resolve_import_conflicts(
            &db.conn,
            &store,
            &result.session_id,
            &[resolution],
            None,
        )
        .unwrap();
        let saved = &all_firearms(&db)[0];
        assert_eq!(saved.make, "Smith & Wesson");
        assert_eq!(saved.cartridge.as_deref(), Some("9x19mm Parabellum"));
    }

    #[test]
    fn a_failed_row_is_not_listed_in_the_report() {
        let db = TestDb::new();
        on_record(&db, "Smith & Wesson", "S1", None);
        let result = import_rows(
            &db,
            &[csv_firearm("smith and wesson", "M&P", "A1", &[("firearm_type", "Blunderbuss")])],
        );
        assert_eq!(result.row_errors.len(), 1);
        assert!(result.snapped_values.is_empty());
        assert!(result.derived_calibers.is_empty());
    }

    #[test]
    fn an_action_is_matched_ignoring_case_and_spaces_unknown_and_disallowed_ones_are_row_errors() {
        let db = TestDb::new();
        let result = import_rows(
            &db,
            &[
                csv_firearm(
                    "Remington",
                    "700",
                    "A1",
                    &[("firearm_type", "Rifle"), ("action_type", " BOLT ACTION ")],
                ),
                csv_firearm("Acme", "Flint", "A2", &[("action_type", "flintlockish")]),
                csv_firearm("Acme", "Pump", "A3", &[("action_type", "Pump action")]),
                csv_firearm("Acme", "Plain", "A4", &[]),
            ],
        );
        assert_eq!(result.imported_count, 2);
        assert_eq!(result.row_errors.len(), 2);
        assert_eq!(result.row_errors[0].row, 2);
        assert_eq!(
            result.row_errors[0].message,
            "action_type: unknown action type \"flintlockish\""
        );
        assert_eq!(result.row_errors[1].row, 3);
        assert_eq!(
            result.row_errors[1].message,
            "action_type: Pump action doesn't apply to a Handgun."
        );
        let remington = all_firearms(&db).into_iter().find(|f| f.make == "Remington").unwrap();
        assert_eq!(remington.action_type_id, Some(BOLT_ACTION));
        assert_eq!(remington.firearm_type_id, RIFLE);
        let plain = all_firearms(&db).into_iter().find(|f| f.model == "Plain").unwrap();
        assert_eq!(plain.action_type_id, None);
        assert_eq!(plain.firearm_type_id, HANDGUN);
    }

    #[test]
    fn an_over_long_or_control_character_entry_is_a_row_error_naming_the_column() {
        let db = TestDb::new();
        let long = "x".repeat(101);
        for column in ["make", "model", "cartridge", "caliber"] {
            let too_long =
                import_rows(&db, &[csv_firearm("Acme", "One", "A1", &[(column, long.as_str())])]);
            let message = only_error(&too_long);
            assert!(message.starts_with(&format!("{column}: ")), "{message}");
            assert!(message.contains("at most 100 characters"), "{message}");

            let control =
                import_rows(&db, &[csv_firearm("Acme", "One", "A1", &[(column, "bad\u{7}text")])]);
            let message = only_error(&control);
            assert!(message.starts_with(&format!("{column}: ")), "{message}");
            assert!(message.contains("control characters"), "{message}");
        }
        assert_eq!(all_firearms(&db).len(), 0);
    }

    #[test]
    fn a_sheet_without_the_two_new_columns_imports_with_neither_and_later_columns_in_place() {
        let db = TestDb::new();
        let legacy: Vec<_> = FIREARM_COLUMNS
            .iter()
            .copied()
            .filter(|c| *c != "cartridge" && *c != "action_type")
            .collect();
        assert_eq!(legacy.len(), 40);
        let row = legacy
            .iter()
            .map(|c| match *c {
                "make" => "Glock",
                "model" => "19",
                "serial_number" => "A1",
                "no_serial_attested" => "FALSE",
                "caliber" => "9mm",
                "firearm_type" => "Handgun",
                "notes" => "kept",
                "origin" => "Imported",
                "country_of_manufacture" => "Austria",
                _ => "",
            })
            .collect::<Vec<_>>()
            .join(",");
        let result = import_text(&db, &format!("{}\n{row}\n", legacy.join(",")));
        assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
        let glock = &all_firearms(&db)[0];
        assert_eq!(glock.cartridge, None);
        assert_eq!(glock.action_type_id, None);
        assert_eq!(glock.notes.as_deref(), Some("kept"));
        assert_eq!(glock.country_of_manufacture.as_deref(), Some("Austria"));
    }

    #[test]
    fn columns_are_read_by_header_in_any_order_case_and_spacing_and_unknown_ones_are_ignored() {
        let db = TestDb::new();
        let text = "  Model , CALIBER,Serial_Number,unknown extra,Make ,no_serial_attested,Firearm_Type,Cartridge\n\
                    19,9mm,A1,ignored,Glock,FALSE,Handgun,9x19mm Parabellum\n";
        let result = import_text(&db, text);
        assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
        let glock = &all_firearms(&db)[0];
        assert_eq!((glock.make.as_str(), glock.model.as_str()), ("Glock", "19"));
        assert_eq!(glock.serial_number.as_deref(), Some("A1"));
        assert_eq!(glock.cartridge.as_deref(), Some("9x19mm Parabellum"));
    }

    #[test]
    fn a_header_naming_a_column_twice_fails_the_whole_import_and_imports_nothing() {
        let db = TestDb::new();
        let dir = TempDir::new().unwrap();
        let path = write_csv(
            &dir,
            "make,model,caliber,serial_number,Caliber,firearm_type\nGlock,19,9mm,A1,9mm,Handgun\n",
        );
        let error = import_export_ops::import_collection(
            &db.conn,
            &support::import_files(&path, SpreadsheetFormat::Csv),
            &ImportSessionStore::new(),
            &mut |_, _| {},
        )
        .unwrap_err();
        assert_eq!(error.code, "VALIDATION_ERROR");
        assert_eq!(error.message, "The import file has two \"caliber\" columns.");
        assert_eq!(all_firearms(&db).len(), 0);
    }

    #[test]
    fn twenty_variants_all_snap_and_different_notations_never_do() {
        let db = TestDb::new();
        on_record(&db, "Springfield Armory", "R0", None);
        let variants: Vec<String> = (0..20)
            .map(|i| match i % 4 {
                0 => "springfield armory".to_owned(),
                1 => "SPRINGFIELD ARMORY".to_owned(),
                2 => " Springfield  Armory ".to_owned(),
                _ => "Springfield-Armory".to_owned(),
            })
            .collect();
        let mut rows: Vec<String> = variants
            .iter()
            .enumerate()
            .map(|(i, make)| csv_firearm(make, "M", &format!("V{i}"), &[]))
            .collect();
        rows.push(csv_firearm("Springfield Arms", "M", "D1", &[]));
        rows.push(csv_firearm("S. Armory", "M", "D2", &[]));
        let result = import_rows(&db, &rows);
        assert_eq!(result.imported_count, 22, "{:?}", result.row_errors);
        // " Springfield  Armory " trims to a spelling that differs from the
        // record only by the doubled space, so it snaps too.
        assert_eq!(result.snapped_values.len(), 20);
        let makes: Vec<_> = all_firearms(&db).into_iter().map(|f| f.make).collect();
        assert_eq!(makes.iter().filter(|m| *m == "Springfield Armory").count(), 21);
        assert!(makes.contains(&"Springfield Arms".to_owned()));
        assert!(makes.contains(&"S. Armory".to_owned()));
    }
}

mod registration_spreadsheet {
    use super::*;
    use hoplodex_lib::commands::import_export::ImportResult;
    use hoplodex_lib::models::firearm::{Firearm, FirearmInput};
    use hoplodex_lib::services::entry_text::EntryField;
    use hoplodex_lib::services::spreadsheet::FIREARM_COLUMNS;

    const SUPPRESSOR_TYPE: i64 = 5;

    fn import_rows(db: &TestDb, rows: &[String]) -> ImportResult {
        let dir = TempDir::new().unwrap();
        let path = write_csv(&dir, &csv_file(rows));
        reimport(&path, db)
    }

    fn reimport(path: &std::path::Path, into: &TestDb) -> ImportResult {
        import_export_ops::import_collection(
            &into.conn,
            &support::import_files(path, SpreadsheetFormat::Csv),
            &ImportSessionStore::new(),
            &mut |_, _| {},
        )
        .unwrap()
    }

    fn all_firearms(db: &TestDb) -> Vec<Firearm> {
        let mut ids = import_export_ops::all_firearm_ids(&db.conn).unwrap();
        ids.sort();
        ids.into_iter().map(|id| firearm_ops::get_firearm(&db.conn, id).unwrap()).collect()
    }

    fn record(db: &TestDb, input: FirearmInput) {
        firearm_ops::create_firearm(&db.conn, &input, false).unwrap();
    }

    fn export_all(db: &TestDb, dest: &TempDir) -> std::path::PathBuf {
        let ids = import_export_ops::all_firearm_ids(&db.conn).unwrap();
        import_export_ops::export_collection(
            &db.conn,
            dest.path(),
            "backup",
            SpreadsheetFormat::Csv,
            &support::firearm_records(&ids),
            &mut |_, _| {},
        )
        .unwrap()
        .spreadsheet_path
    }

    fn only_error(result: &ImportResult) -> &str {
        assert_eq!(result.row_errors.len(), 1, "{:?}", result.row_errors);
        &result.row_errors[0].message
    }

    fn registered(
        serial: &str,
        class: i64,
        form: Option<&str>,
        approved: Option<&str>,
        to: Option<&str>,
    ) -> FirearmInput {
        FirearmInput {
            registration_class_id: Some(class),
            registration_form: form.map(str::to_owned),
            registration_approved: approved.map(str::to_owned),
            registered_to: to.map(str::to_owned),
            ..support::firearm("Glock", "19", serial)
        }
    }

    #[test]
    fn the_header_puts_the_four_registration_columns_after_the_original_marks() {
        assert_eq!(FIREARM_COLUMNS.len(), 42);
        let at = FIREARM_COLUMNS.iter().position(|c| *c == "original_serial_number").unwrap();
        assert_eq!(
            &FIREARM_COLUMNS[at + 1..at + 6],
            [
                "registered_as",
                "registration_form",
                "registration_approved",
                "registered_to",
                "mounted_on"
            ]
        );
    }

    #[test]
    fn a_collection_with_suppressors_and_every_classification_round_trips_exactly() {
        let db = TestDb::new();
        for class in 1..=6 {
            record(
                &db,
                registered(
                    &format!("S{class}"),
                    class,
                    Some("Form 4"),
                    Some("2024-03-05"),
                    Some(&format!("Owner {class}")),
                ),
            );
        }
        // A classification no longer offered still exports and imports.
        db.conn.execute("UPDATE registration_classes SET offered = 0 WHERE id = 6", []).unwrap();
        record(
            &db,
            FirearmInput {
                firearm_type_id: SUPPRESSOR_TYPE,
                ..registered("Q1", 1, None, None, None)
            },
        );
        record(&db, support::firearm("Ruger", "Mk IV", "C3"));

        let dest = TempDir::new().unwrap();
        let path = export_all(&db, &dest);
        let fresh = TestDb::new();
        let result = reimport(&path, &fresh);

        assert_eq!(result.imported_count, 8, "{:?}", result.row_errors);
        assert!(result.snapped_values.is_empty(), "{:?}", result.snapped_values);
        let shape = |db: &TestDb| {
            let mut all: Vec<_> = all_firearms(db)
                .into_iter()
                .map(|f| {
                    (
                        f.serial_number,
                        f.firearm_type_id,
                        f.registration_class_id,
                        f.registration_form,
                        f.registration_approved,
                        f.registered_to,
                    )
                })
                .collect();
            all.sort();
            all
        };
        assert_eq!(shape(&fresh), shape(&db));
    }

    #[test]
    fn same_notation_variants_of_a_form_and_a_name_merge_and_are_reported() {
        let db = TestDb::new();
        let rows: Vec<String> = [
            ("A1", "form 4", "Jane Doe"),
            ("A2", "Form 4", "Jane Doe"),
            ("A3", "Form 4", "jane doe"),
            ("A4", "FORM 4", "Jane Doe"),
        ]
        .iter()
        .map(|(serial, form, to)| {
            csv_firearm(
                "Glock",
                "19",
                serial,
                &[
                    ("registered_as", "Suppressor"),
                    ("registration_form", form),
                    ("registered_to", to),
                ],
            )
        })
        .collect();
        let result = import_rows(&db, &rows);

        assert_eq!(result.imported_count, 4, "{:?}", result.row_errors);
        for firearm in all_firearms(&db) {
            assert_eq!(firearm.registration_form.as_deref(), Some("Form 4"));
            assert_eq!(firearm.registered_to.as_deref(), Some("Jane Doe"));
        }
        let fields: Vec<_> = result.snapped_values.iter().map(|s| s.field).collect();
        assert!(fields.contains(&EntryField::RegistrationForm), "{fields:?}");
        assert!(fields.contains(&EntryField::RegisteredTo), "{fields:?}");
    }

    #[test]
    fn registered_as_matches_ignoring_case_and_spaces_and_a_classification_no_longer_offered() {
        let db = TestDb::new();
        db.conn.execute("UPDATE registration_classes SET offered = 0 WHERE id = 5", []).unwrap();
        let result = import_rows(
            &db,
            &[
                csv_firearm("Glock", "19", "A1", &[("registered_as", "  SHORT-BARRELED RIFLE ")]),
                csv_firearm("Glock", "19", "A2", &[("registered_as", "machine gun")]),
            ],
        );
        assert_eq!(result.imported_count, 2, "{:?}", result.row_errors);
        let classes: Vec<_> =
            all_firearms(&db).into_iter().map(|f| f.registration_class_id).collect();
        assert_eq!(classes, [Some(2), Some(5)]);
    }

    #[test]
    fn an_unknown_classification_is_a_row_error_and_other_rows_still_import() {
        let db = TestDb::new();
        let result = import_rows(
            &db,
            &[
                csv_firearm("Glock", "19", "A1", &[("registered_as", "Short barrel rifle")]),
                csv_firearm("Glock", "19", "A2", &[]),
            ],
        );
        assert_eq!(result.imported_count, 1);
        assert_eq!(
            only_error(&result),
            "registered_as: unknown classification \"Short barrel rifle\""
        );
    }

    #[test]
    fn details_without_a_classification_are_a_row_error() {
        let db = TestDb::new();
        for cell in [
            ("registration_form", "Form 4"),
            ("registration_approved", "2024-03-05"),
            ("registered_to", "Jane"),
        ] {
            let result = import_rows(&db, &[csv_firearm("Glock", "19", "A1", &[cell])]);
            assert_eq!(result.imported_count, 0);
            assert_eq!(
                only_error(&result),
                "registered_as: Registration details need a classification."
            );
        }
    }

    #[test]
    fn a_bad_approved_date_and_a_long_name_are_row_errors_naming_the_column() {
        let db = TestDb::new();
        let tomorrow = (chrono::Local::now().date_naive() + chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        let long = "x".repeat(101);
        for (cells, expected) in [
            (
                ("registration_approved", tomorrow.as_str()),
                "registration_approved: Approved date can't be in the future.",
            ),
            (
                ("registration_approved", "5 March"),
                "registration_approved: Approved date must be a date in YYYY-MM-DD format.",
            ),
            (
                ("registered_to", long.as_str()),
                "registered_to: Registered to can be at most 100 characters.",
            ),
        ] {
            let result = import_rows(
                &db,
                &[csv_firearm("Glock", "19", "A1", &[("registered_as", "Suppressor"), cells])],
            );
            assert_eq!(result.imported_count, 0);
            assert_eq!(only_error(&result), expected);
        }
    }

    #[test]
    fn a_suppressor_row_with_an_action_a_barrel_length_and_a_capacity_reports_all_three() {
        let db = TestDb::new();
        let result = import_rows(
            &db,
            &[
                csv_firearm(
                    "SilencerCo",
                    "Omega",
                    "A1",
                    &[
                        ("firearm_type", "Suppressor"),
                        ("action_type", "Semi-automatic"),
                        ("barrel_length_in", "4"),
                        ("capacity", "10"),
                    ],
                ),
                csv_firearm("Glock", "19", "A2", &[]),
            ],
        );
        assert_eq!(result.imported_count, 1);
        assert_eq!(
            only_error(&result),
            "action_type: Action doesn't apply to a Suppressor.; \
             barrel_length_in: Barrel length doesn't apply to a Suppressor.; \
             capacity: Capacity doesn't apply to a Suppressor."
        );
    }

    #[test]
    fn a_suppressor_rows_caliber_is_never_worked_out_from_its_cartridge() {
        // US4-7, FR-022, research.md §15: a Suppressor's caliber is its bore,
        // so a blank one is a row error and nothing is listed as derived,
        // while a Rifle row in the same sheet still derives.
        let db = TestDb::new();
        let result = import_rows(
            &db,
            &[
                csv_firearm(
                    "SilencerCo",
                    "Omega 300",
                    "S1",
                    &[
                        ("firearm_type", "suppressor"),
                        ("caliber", ""),
                        ("cartridge", ".300 Winchester Magnum"),
                    ],
                ),
                csv_firearm(
                    "Ruger",
                    "American",
                    "R1",
                    &[
                        ("firearm_type", "Rifle"),
                        ("caliber", ""),
                        ("cartridge", ".300 Winchester Magnum"),
                    ],
                ),
                csv_firearm(
                    "Dead Air",
                    "Mask",
                    "S2",
                    &[("firearm_type", "Suppressor"), ("caliber", ""), ("cartridge", "")],
                ),
            ],
        );
        assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
        let errors: Vec<(usize, &str)> =
            result.row_errors.iter().map(|e| (e.row, e.message.as_str())).collect();
        assert_eq!(
            errors,
            [
                (
                    1,
                    "caliber: Caliber is required; a Suppressor's isn't worked out from its cartridge."
                ),
                (3, "caliber: Caliber is required."),
            ]
        );
        assert_eq!(result.derived_calibers.len(), 1);
        assert_eq!(result.derived_calibers[0].row, 2);
        assert_eq!(result.derived_calibers[0].caliber, ".30");
    }

    #[test]
    fn a_suppressor_row_keeps_its_bore_and_rated_cartridge_as_given() {
        // US4-7: both values are imported as written, neither from the other.
        let db = TestDb::new();
        let result = import_rows(
            &db,
            &[csv_firearm(
                "SilencerCo",
                "Omega 300",
                "S1",
                &[
                    ("firearm_type", "Suppressor"),
                    ("caliber", ".46"),
                    ("cartridge", ".300 Winchester Magnum"),
                ],
            )],
        );
        assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
        assert!(result.derived_calibers.is_empty());
        let ids = import_export_ops::all_firearm_ids(&db.conn).unwrap();
        let suppressor = firearm_ops::get_firearm(&db.conn, ids[0]).unwrap();
        assert_eq!(suppressor.caliber, ".46");
        assert_eq!(suppressor.cartridge.as_deref(), Some(".300 Winchester Magnum"));
    }

    #[test]
    fn a_form_snaps_to_a_built_in_name_then_to_the_forms_on_record() {
        let db = TestDb::new();
        record(&db, registered("R1", 1, Some("Tax-paid transfer"), None, Some("Jane Doe")));
        let result = import_rows(
            &db,
            &[
                csv_firearm(
                    "Glock",
                    "19",
                    "A1",
                    &[
                        ("registered_as", "Suppressor"),
                        ("registration_form", "FORM 5"),
                        ("registered_to", "JANE DOE"),
                    ],
                ),
                csv_firearm(
                    "Glock",
                    "19",
                    "A2",
                    &[("registered_as", "Suppressor"), ("registration_form", "tax-paid TRANSFER")],
                ),
            ],
        );
        assert_eq!(result.imported_count, 2, "{:?}", result.row_errors);
        let imported = &all_firearms(&db)[1..];
        assert_eq!(imported[0].registration_form.as_deref(), Some("Form 5"));
        assert_eq!(imported[0].registered_to.as_deref(), Some("Jane Doe"));
        assert_eq!(imported[1].registration_form.as_deref(), Some("Tax-paid transfer"));
        assert_eq!(result.snapped_values.len(), 3, "{:?}", result.snapped_values);
    }

    #[test]
    fn a_sheet_without_the_four_columns_imports_with_no_classification() {
        let db = TestDb::new();
        let header: Vec<_> = FIREARM_COLUMNS
            .iter()
            .filter(|c| {
                !["registered_as", "registration_form", "registration_approved", "registered_to"]
                    .contains(c)
            })
            .copied()
            .collect();
        let cells: Vec<String> = header
            .iter()
            .map(|c| match *c {
                "make" => "Glock",
                "model" => "19",
                "serial_number" => "A1",
                "caliber" => "9mm",
                "firearm_type" => "Handgun",
                _ => "",
            })
            .map(str::to_owned)
            .collect();
        let dir = TempDir::new().unwrap();
        let path = write_csv(&dir, &format!("{}\n{}\n", header.join(","), cells.join(",")));
        let result = reimport(&path, &db);
        assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
        let firearm = &all_firearms(&db)[0];
        assert_eq!(firearm.registration_class_id, None);
        assert_eq!(firearm.registration_form, None);
    }
}
