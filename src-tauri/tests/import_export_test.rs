//! Integration test: `import_collection` creates new records and reports
//! failing rows without discarding successful ones (spec.md US5 Acceptance
//! Scenarios 2-3; FR-020), run against a real temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::commands::import_export::ImportSessionStore;
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use support::{csv_file, csv_firearm, TestDb};
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
    assert_eq!(imported.scheduled_coverage_amount, Some(350_000));
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
        firearm_ops::create_firearm(&db.conn, &support::firearm("Colt", "Python", "V1")).unwrap();
    hoplodex_lib::commands::insurance::ops::assign_firearm_coverage(
        &db.conn,
        created.id,
        Some(policy_id),
        Some(350_000),
    )
    .unwrap();
    let plain =
        firearm_ops::create_firearm(&db.conn, &support::firearm("Glock", "19", "A1")).unwrap();

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
    assert_eq!(colt.scheduled_coverage_amount, Some(350_000));
    let glock = all.iter().find(|f| f.make == "Glock").unwrap();
    assert_eq!(glock.insurance_policy_id, None);
}
