//! Integration test: import matching/conflict resolution — make+model+
//! serial key, no-serial-always-new, apply-to-remaining (spec.md FR-026,
//! FR-030), run against a real temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::{ops as firearm_ops, DisposeFirearmInput};
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::commands::import_export::{ConflictResolution, ImportSessionStore};
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput, FirearmStatus};
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use support::{csv_file, csv_firearm, TestDb};
use tempfile::TempDir;

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
    }
}

#[test]
fn a_matching_make_model_serial_produces_a_conflict_not_a_silent_overwrite() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();

    let csv = csv_file(&[csv_firearm(
        "Glock",
        "19",
        "ABC123",
        &[("notes", "updated notes"), ("estimated_value", "700.00")],
    )]);
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
    let csv = csv_file(&[csv_firearm("Glock", "19", "", &[])]);
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

    let csv = csv_file(&[csv_firearm(
        "Glock",
        "19",
        "ABC123",
        &[("notes", "updated notes"), ("estimated_value", "700.00")],
    )]);
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
    assert_eq!(updated.estimated_value, Some(700));
}

fn import_glock(
    db: &TestDb,
    store: &ImportSessionStore,
    rows: &[String],
) -> hoplodex_lib::commands::import_export::ImportResult {
    let dir = TempDir::new().unwrap();
    let path = write_csv(&dir, &csv_file(rows));
    import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        store,
        &mut |_, _| {},
    )
    .unwrap()
}

fn dispose(db: &TestDb, id: i64) {
    firearm_ops::dispose_firearm(
        &db.conn,
        id,
        &DisposeFirearmInput {
            disposition_type: DispositionType::Sold,
            recipient: "Jane Doe".into(),
            date: "2025-06-15".into(),
            price: 40000,
        },
    )
    .unwrap();
}

fn active_count(db: &TestDb) -> usize {
    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    listing.groups.iter().map(|g| g.firearms.len()).sum()
}

#[test]
fn a_duplicate_of_a_disposed_record_is_allowed_as_a_reacquisition() {
    let db = TestDb::new();
    let store = ImportSessionStore::new();
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();
    dispose(&db, existing.id);

    let import_result = import_glock(
        &db,
        &store,
        &[csv_firearm("Glock", "19", "ABC123", &[("notes", "reacquired")])],
    );
    assert_eq!(import_result.conflicts.len(), 1);
    assert!(import_result.conflicts[0].duplicate_allowed, "a disposed match never blocks");
    let conflict_id = import_result.conflicts[0].conflict_id.clone();

    let resolved = import_export_ops::resolve_import_conflicts(
        &db.conn,
        &store,
        &import_result.session_id,
        &[ConflictResolution { conflict_id, action: "duplicate".into() }],
        None,
    )
    .unwrap();

    assert_eq!(resolved.resolved_count, 1);
    assert!(resolved.unresolved.is_empty());
    assert_eq!(active_count(&db), 1, "the reacquired record is active; the old one is disposed");
}

#[test]
fn a_duplicate_of_an_active_record_is_not_offered_and_is_refused_and_reported() {
    let db = TestDb::new();
    let store = ImportSessionStore::new();
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();

    let import_result = import_glock(
        &db,
        &store,
        &[csv_firearm("Glock", "19", "ABC123", &[("notes", "duplicate row")])],
    );
    assert_eq!(import_result.conflicts.len(), 1);
    assert!(
        !import_result.conflicts[0].duplicate_allowed,
        "FR-032 would block it, so only skip and overwrite are offered"
    );
    let conflict_id = import_result.conflicts[0].conflict_id.clone();

    let resolved = import_export_ops::resolve_import_conflicts(
        &db.conn,
        &store,
        &import_result.session_id,
        &[ConflictResolution { conflict_id: conflict_id.clone(), action: "duplicate".into() }],
        None,
    )
    .unwrap();

    assert_eq!(resolved.resolved_count, 0);
    assert_eq!(resolved.unresolved.len(), 1);
    assert_eq!(resolved.unresolved[0].row, 1);
    assert_eq!(active_count(&db), 1, "nothing was inserted");

    // Left unresolved, not lost: it can still be skipped or overwritten.
    let retried = import_export_ops::resolve_import_conflicts(
        &db.conn,
        &store,
        &import_result.session_id,
        &[ConflictResolution { conflict_id, action: "overwrite".into() }],
        None,
    )
    .unwrap();
    assert_eq!(retried.resolved_count, 1);
    let updated = firearm_ops::get_firearm(&db.conn, existing.id).unwrap();
    assert_eq!(updated.notes.as_deref(), Some("duplicate row"));
}

#[test]
fn apply_to_remaining_duplicate_skips_only_the_conflicts_it_may_not_apply_to() {
    let db = TestDb::new();
    let store = ImportSessionStore::new();
    let active = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();
    let mut second = existing_firearm();
    second.model = "26".into();
    second.serial_number = Some("XYZ789".into());
    let disposed = firearm_ops::create_firearm(&db.conn, &second).unwrap();
    dispose(&db, disposed.id);

    let import_result = import_glock(
        &db,
        &store,
        &[csv_firearm("Glock", "19", "ABC123", &[]), csv_firearm("Glock", "26", "XYZ789", &[])],
    );
    assert_eq!(import_result.conflicts.len(), 2);

    let resolved = import_export_ops::resolve_import_conflicts(
        &db.conn,
        &store,
        &import_result.session_id,
        &[],
        Some("duplicate"),
    )
    .unwrap();

    assert_eq!(resolved.resolved_count, 1, "only the reacquisition was duplicated");
    assert_eq!(resolved.unresolved.len(), 1);
    assert_eq!(resolved.unresolved[0].row, 1);
    assert_eq!(active_count(&db), 2);
    assert!(firearm_ops::get_firearm(&db.conn, active.id).is_ok());
}

#[test]
fn a_conflict_that_would_fail_validation_on_overwrite_is_reported_not_fatal() {
    let db = TestDb::new();
    let store = ImportSessionStore::new();
    let mut first = existing_firearm();
    first.nickname = Some("Taken".into());
    firearm_ops::create_firearm(&db.conn, &first).unwrap();
    let mut second = existing_firearm();
    second.model = "26".into();
    second.serial_number = Some("XYZ789".into());
    let target = firearm_ops::create_firearm(&db.conn, &second).unwrap();

    // Overwriting the second record would give it the first one's nickname.
    let import_result = import_glock(
        &db,
        &store,
        &[
            csv_firearm("Glock", "26", "XYZ789", &[("nickname", "taken")]),
            csv_firearm("Glock", "19", "ABC123", &[("notes", "fine")]),
        ],
    );
    assert_eq!(import_result.conflicts.len(), 2);

    let resolved = import_export_ops::resolve_import_conflicts(
        &db.conn,
        &store,
        &import_result.session_id,
        &[],
        Some("overwrite"),
    )
    .unwrap();

    assert_eq!(resolved.resolved_count, 1, "the other conflict still resolves");
    assert_eq!(resolved.unresolved.len(), 1);
    assert!(resolved.unresolved[0].message.to_lowercase().contains("nickname"));
    assert_eq!(firearm_ops::get_firearm(&db.conn, target.id).unwrap().nickname, None);
}

#[test]
fn matching_ignores_letter_case_and_surrounding_whitespace() {
    let db = TestDb::new();
    let store = ImportSessionStore::new();
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();

    let result = import_glock(&db, &store, &[csv_firearm(" glock ", "19", " abc123 ", &[])]);

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].existing_firearm_id, existing.id);
}

#[test]
fn a_disposed_and_an_active_match_conflict_with_the_active_one() {
    let db = TestDb::new();
    let store = ImportSessionStore::new();
    let old = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();
    dispose(&db, old.id);
    let current = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();

    let result = import_glock(&db, &store, &[csv_firearm("Glock", "19", "ABC123", &[])]);

    assert_eq!(result.conflicts[0].existing_firearm_id, current.id);
    assert!(!result.conflicts[0].duplicate_allowed);
}

#[test]
fn a_row_with_both_a_serial_number_and_the_attestation_is_rejected() {
    let db = TestDb::new();
    let store = ImportSessionStore::new();
    firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();

    let result = import_glock(
        &db,
        &store,
        &[csv_firearm("Glock", "19", "ABC123", &[("no_serial_attested", "TRUE")])],
    );

    assert_eq!(result.imported_count, 0);
    assert!(result.conflicts.is_empty(), "an exempt row is never matched (FR-030)");
    assert_eq!(result.row_errors.len(), 1);
    assert_eq!(result.row_errors[0].row, 1);
    assert!(result.row_errors[0].message.contains("no serial number"));
    assert_eq!(active_count(&db), 1);
}

#[test]
fn a_row_repeating_an_earlier_row_of_the_same_file_conflicts_with_the_record_it_created() {
    let db = TestDb::new();
    let store = ImportSessionStore::new();

    let result = import_glock(
        &db,
        &store,
        &[csv_firearm("Sig", "P226", "S1", &[]), csv_firearm("Sig", "P226", "S1", &[])],
    );

    assert_eq!(result.imported_count, 1);
    assert!(result.row_errors.is_empty());
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].row, 2);
    assert!(!result.conflicts[0].duplicate_allowed);
}

#[test]
fn resolving_a_conflict_as_skip_leaves_the_original_untouched() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();

    let csv = csv_file(&[csv_firearm(
        "Glock",
        "19",
        "ABC123",
        &[("notes", "should not apply"), ("estimated_value", "700.00")],
    )]);
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

    let csv = csv_file(&[
        csv_firearm(
            "Glock",
            "19",
            "ABC123",
            &[("notes", "row one"), ("estimated_value", "700.00")],
        ),
        csv_firearm(
            "Glock",
            "26",
            "XYZ789",
            &[("notes", "row two"), ("estimated_value", "800.00")],
        ),
    ]);
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

/// FR-039: overwriting an existing record from a file updates the physical
/// details along with the rest.
#[test]
fn overwriting_a_conflict_updates_the_physical_details() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm()).unwrap();
    assert_eq!(existing.capacity, None);

    let path = write_csv(
        &dir,
        &csv_file(&[csv_firearm(
            "Glock",
            "19",
            "ABC123",
            &[("barrel_length_in", "4.02"), ("capacity", "15"), ("finish", "nDLC")],
        )]),
    );
    let imported = import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &store,
        &mut |_, _| {},
    )
    .unwrap();
    let conflict_id = imported.conflicts[0].conflict_id.clone();
    import_export_ops::resolve_import_conflicts(
        &db.conn,
        &store,
        &imported.session_id,
        &[ConflictResolution { conflict_id, action: "overwrite".into() }],
        None,
    )
    .unwrap();

    let updated = firearm_ops::get_firearm(&db.conn, existing.id).unwrap();
    assert_eq!(updated.barrel_length_hundredths, Some(402));
    assert_eq!(updated.capacity, Some(15));
    assert_eq!(updated.finish.as_deref(), Some("nDLC"));
}
