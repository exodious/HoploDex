//! Integration test: `import_collection` creates new records and reports
//! failing rows without discarding successful ones (spec.md US5 Acceptance
//! Scenarios 2-3; FR-020), run against a real temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::commands::import_export::ImportSessionStore;
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use support::TestDb;
use tempfile::TempDir;

fn write_csv(dir: &TempDir, contents: &str) -> std::path::PathBuf {
    let path = dir.path().join("import.csv");
    std::fs::write(&path, contents).unwrap();
    path
}

const HEADER: &str = "make,model,serial_number,no_serial_attested,caliber,firearm_type,notes,accessories,status,estimated_value,acquisition_source,acquisition_date,acquisition_price,disposition_type,disposition_recipient,disposition_date,disposition_price,insurance_policy_name,coverage_kind,scheduled_coverage_amount,photo_filenames";

#[test]
fn scenario_2_imports_new_records_from_a_spreadsheet() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();

    let csv = format!(
        "{HEADER}\nGlock,19,ABC123,FALSE,9mm,Handgun,,,,500.00,,,,,,,,,,,\nSig,P226,DEF456,FALSE,9mm,Handgun,,,,600.00,,,,,,,,,,,\n"
    );
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
    let csv = format!(
        "{HEADER}\n,19,ABC123,FALSE,9mm,Handgun,,,,500.00,,,,,,,,,,,\nSig,P226,DEF456,FALSE,9mm,Handgun,,,,600.00,,,,,,,,,,,\n"
    );
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

    let csv = format!("{HEADER}\nGlock,19,ABC123,FALSE,9mm,NotARealType,,,,500.00,,,,,,,,,,,\n");
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

fn import_one_row(row: &str) -> hoplodex_lib::commands::import_export::ImportResult {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let path = write_csv(&dir, &format!("{HEADER}\n{row}\n"));
    import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |_, _| {},
    )
    .unwrap()
}

#[test]
fn a_future_acquisition_date_is_a_row_error() {
    let tomorrow = (chrono::Local::now().date_naive() + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let result = import_one_row(&format!(
        "Glock,19,ABC123,FALSE,9mm,Handgun,,,,500.00,,{tomorrow},,,,,,,,,"
    ));

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
    let result = import_one_row(&format!(
        "Glock,19,ABC123,FALSE,9mm,Handgun,,,disposed,500.00,,,,sold,Jane,{tomorrow},400.00,,,,"
    ));

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.row_errors.len(), 1);
    assert!(result.row_errors[0].message.to_lowercase().contains("disposition"));
}

#[test]
fn a_disposition_before_the_acquisition_date_is_a_row_error() {
    let result = import_one_row(
        "Glock,19,ABC123,FALSE,9mm,Handgun,,,disposed,500.00,,2025-03-01,,sold,Jane,2025-02-28,400.00,,,,",
    );

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.row_errors.len(), 1);
    assert!(result.row_errors[0].message.to_lowercase().contains("acquisition"));
}

#[test]
fn a_valid_disposition_on_or_after_the_acquisition_date_imports() {
    let result = import_one_row(
        "Glock,19,ABC123,FALSE,9mm,Handgun,,,disposed,500.00,,2025-03-01,,sold,Jane,2025-03-01,400.00,,,,",
    );

    assert!(result.row_errors.is_empty(), "{:?}", result.row_errors);
    assert_eq!(result.imported_count, 1);
}
