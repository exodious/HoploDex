//! Integration test: import matching/conflict resolution — make+model+
//! serial key, no-serial-always-new, apply-to-remaining (spec.md FR-026,
//! FR-030), run against a real temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::commands::import_export::{ConflictResolution, ImportSessionStore};
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use support::TestDb;
use tempfile::TempDir;

const HEADER: &str = "make,model,serial_number,no_serial_attested,caliber,firearm_type,notes,accessories,status,estimated_value,acquisition_source,acquisition_date,acquisition_price,disposition_type,disposition_recipient,disposition_date,disposition_price,insurance_policy_name,coverage_kind,scheduled_coverage_amount,photo_filenames";

fn write_csv(dir: &TempDir, contents: &str) -> std::path::PathBuf {
    let path = dir.path().join("import.csv");
    std::fs::write(&path, contents).unwrap();
    path
}

fn existing_firearm() -> FirearmInput {
    FirearmInput {
        make: "Glock".into(),
        model: "19".into(),
        serial_number: Some("ABC123".into()),
        no_serial_attested: false,
        caliber: "9mm".into(),
        firearm_type_id: 1,
        notes: Some("original notes".into()),
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
        coverage_kind: None,
        nickname: None,
        scheduled_coverage_amount: None,
    }
}

#[test]
fn a_matching_make_model_serial_produces_a_conflict_not_a_silent_overwrite() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();

    let csv =
        format!("{HEADER}\nGlock,19,ABC123,FALSE,9mm,Handgun,updated notes,,,700.00,,,,,,,,,,,\n");
    let path = write_csv(&dir, &csv);

    let result = import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |_, _| {},
    )
    .unwrap();

    assert_eq!(result.imported_count, 0, "a matching row must not be silently inserted");
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].existing_firearm_id, existing.id);

    // The original record must be untouched until resolved.
    let unchanged = firearm_ops::get_firearm(&db.conn, existing.id).unwrap();
    assert_eq!(unchanged.notes.as_deref(), Some("original notes"));
}

#[test]
fn no_serial_attested_rows_are_always_inserted_as_new() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();

    // Same make/model as the existing record, but no_serial_attested=TRUE:
    // must never match, always inserted as new (FR-030).
    let csv = format!("{HEADER}\nGlock,19,,TRUE,9mm,Handgun,,,,500.00,,,,,,,,,,,\n");
    let path = write_csv(&dir, &csv);

    let result = import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |_, _| {},
    )
    .unwrap();

    assert_eq!(result.imported_count, 1);
    assert_eq!(result.conflicts.len(), 0);
}

#[test]
fn resolving_a_conflict_as_overwrite_updates_the_existing_record() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();

    let csv =
        format!("{HEADER}\nGlock,19,ABC123,FALSE,9mm,Handgun,updated notes,,,700.00,,,,,,,,,,,\n");
    let path = write_csv(&dir, &csv);
    let import_result = import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |_, _| {},
    )
    .unwrap();
    let conflict_id = import_result.conflicts[0].conflict_id.clone();

    let resolve_result = import_export_ops::resolve_import_conflicts(
        &db.conn,
        &store,
        &import_result.session_id,
        &[ConflictResolution { conflict_id, action: "overwrite".into() }],
        None,
    )
    .unwrap();
    assert_eq!(resolve_result.resolved_count, 1);

    let updated = firearm_ops::get_firearm(&db.conn, existing.id).unwrap();
    assert_eq!(updated.notes.as_deref(), Some("updated notes"));
    assert_eq!(updated.estimated_value, Some(70000));
}

#[test]
fn resolving_a_conflict_as_duplicate_inserts_a_new_record_alongside_the_original() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();

    let csv =
        format!("{HEADER}\nGlock,19,ABC123,FALSE,9mm,Handgun,duplicate row,,,700.00,,,,,,,,,,,\n");
    let path = write_csv(&dir, &csv);
    let import_result = import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |_, _| {},
    )
    .unwrap();
    let conflict_id = import_result.conflicts[0].conflict_id.clone();

    import_export_ops::resolve_import_conflicts(
        &db.conn,
        &store,
        &import_result.session_id,
        &[ConflictResolution { conflict_id, action: "duplicate".into() }],
        None,
    )
    .unwrap();

    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    let all: Vec<_> = listing.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(all.len(), 2, "both the original and the duplicate must exist");
}

#[test]
fn resolving_a_conflict_as_skip_leaves_the_original_untouched() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();

    let csv = format!(
        "{HEADER}\nGlock,19,ABC123,FALSE,9mm,Handgun,should not apply,,,700.00,,,,,,,,,,,\n"
    );
    let path = write_csv(&dir, &csv);
    let import_result = import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |_, _| {},
    )
    .unwrap();
    let conflict_id = import_result.conflicts[0].conflict_id.clone();

    import_export_ops::resolve_import_conflicts(
        &db.conn,
        &store,
        &import_result.session_id,
        &[ConflictResolution { conflict_id, action: "skip".into() }],
        None,
    )
    .unwrap();

    let unchanged = firearm_ops::get_firearm(&db.conn, existing.id).unwrap();
    assert_eq!(unchanged.notes.as_deref(), Some("original notes"));
    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    let all: Vec<_> = listing.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(all.len(), 1);
}

#[test]
fn apply_to_remaining_resolves_conflicts_not_explicitly_listed() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let e1 = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();
    let mut second = existing_firearm();
    second.model = "26".into();
    second.serial_number = Some("XYZ789".into());
    let e2 = firearm_ops::create_firearm(&db.conn, &second).unwrap();

    let csv = format!(
        "{HEADER}\nGlock,19,ABC123,FALSE,9mm,Handgun,row one,,,700.00,,,,,,,,,,,\nGlock,26,XYZ789,FALSE,9mm,Handgun,row two,,,800.00,,,,,,,,,,,\n"
    );
    let path = write_csv(&dir, &csv);
    let import_result = import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(import_result.conflicts.len(), 2);

    // Resolve neither explicitly; applyToRemaining=overwrite must cover both.
    let resolve_result = import_export_ops::resolve_import_conflicts(
        &db.conn,
        &store,
        &import_result.session_id,
        &[],
        Some("overwrite"),
    )
    .unwrap();
    assert_eq!(resolve_result.resolved_count, 2);

    assert_eq!(
        firearm_ops::get_firearm(&db.conn, e1.id).unwrap().notes.as_deref(),
        Some("row one")
    );
    assert_eq!(
        firearm_ops::get_firearm(&db.conn, e2.id).unwrap().notes.as_deref(),
        Some("row two")
    );
}
