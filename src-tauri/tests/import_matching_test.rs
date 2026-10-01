//! Integration test: import matching/conflict resolution — make+model+
//! serial key, no-serial-always-new, apply-to-remaining (spec.md FR-026,
//! FR-030), run against a real temporary SQLCipher database.

mod sheets;
mod support;

use hoplodex_lib::commands::firearms::{DisposeInput, ops as firearm_ops};
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::commands::import_export::{ConflictResolution, ImportSessionStore};
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput, FirearmStatus};
use hoplodex_lib::models::record::RecordRef;
use support::{TestDb, csv_file, csv_firearm};
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
fn a_matching_make_model_serial_produces_a_conflict_not_a_silent_overwrite() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();

    let csv = csv_file(&[csv_firearm(
        "Glock",
        "19",
        "ABC123",
        &[("notes", "updated notes"), ("estimated_value", "700.00")],
    )]);
    let path = write_csv(&dir, &csv);

    let result = import_export_ops::import_collection(
        &db.conn,
        &[sheets::import_file(&path)],
        &store,
        &mut |_, _| {},
    )
    .unwrap();

    assert_eq!(result.imported_count, 0, "a matching row must not be silently inserted");
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].existing_record, RecordRef::Firearm(existing.id));

    // The original record must be untouched until resolved.
    let unchanged = firearm_ops::get_firearm(&db.conn, existing.id).unwrap();
    assert_eq!(unchanged.notes.as_deref(), Some("original notes"));
}

#[test]
fn no_serial_attested_rows_are_always_inserted_as_new() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();

    // Same make/model as the existing record, but no_serial_attested=TRUE:
    // must never match, always inserted as new (FR-030).
    let csv = csv_file(&[csv_firearm("Glock", "19", "", &[])]);
    let path = write_csv(&dir, &csv);

    let result = import_export_ops::import_collection(
        &db.conn,
        &[sheets::import_file(&path)],
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
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();

    let csv = csv_file(&[csv_firearm(
        "Glock",
        "19",
        "ABC123",
        &[("notes", "updated notes"), ("estimated_value", "700.00")],
    )]);
    let path = write_csv(&dir, &csv);
    let import_result = import_export_ops::import_collection(
        &db.conn,
        &[sheets::import_file(&path)],
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
        &[sheets::import_file(&path)],
        store,
        &mut |_, _| {},
    )
    .unwrap()
}

fn dispose(db: &TestDb, id: i64) {
    firearm_ops::dispose_firearm(
        &db.conn,
        id,
        &DisposeInput {
            disposition_type: DispositionType::Sold,
            recipient: "Jane Doe".into(),
            date: "2025-06-15".into(),
            price: 40000,
            with_mounted: Vec::new(),
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
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();
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
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();

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
    let active = firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();
    let mut second = existing_firearm();
    second.model = "26".into();
    second.serial_number = Some("XYZ789".into());
    let disposed = firearm_ops::create_firearm(&db.conn, &second, false).unwrap();
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
    firearm_ops::create_firearm(&db.conn, &first, false).unwrap();
    let mut second = existing_firearm();
    second.model = "26".into();
    second.serial_number = Some("XYZ789".into());
    let target = firearm_ops::create_firearm(&db.conn, &second, false).unwrap();

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
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();

    let result = import_glock(&db, &store, &[csv_firearm(" glock ", "19", " abc123 ", &[])]);

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].existing_record, RecordRef::Firearm(existing.id));
}

#[test]
fn a_disposed_and_an_active_match_conflict_with_the_active_one() {
    let db = TestDb::new();
    let store = ImportSessionStore::new();
    let old = firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();
    dispose(&db, old.id);
    let current = firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();

    let result = import_glock(&db, &store, &[csv_firearm("Glock", "19", "ABC123", &[])]);

    assert_eq!(result.conflicts[0].existing_record, RecordRef::Firearm(current.id));
    assert!(!result.conflicts[0].duplicate_allowed);
}

#[test]
fn a_row_with_both_a_serial_number_and_the_attestation_is_rejected() {
    let db = TestDb::new();
    let store = ImportSessionStore::new();
    firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();

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
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();

    let csv = csv_file(&[csv_firearm(
        "Glock",
        "19",
        "ABC123",
        &[("notes", "should not apply"), ("estimated_value", "700.00")],
    )]);
    let path = write_csv(&dir, &csv);
    let import_result = import_export_ops::import_collection(
        &db.conn,
        &[sheets::import_file(&path)],
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
    let e1 = firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();
    let mut second = existing_firearm();
    second.model = "26".into();
    second.serial_number = Some("XYZ789".into());
    let e2 = firearm_ops::create_firearm(&db.conn, &second, false).unwrap();

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
        &[sheets::import_file(&path)],
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
    let existing = firearm_ops::create_firearm(&db.conn, &existing_firearm(), false).unwrap();
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
        &[sheets::import_file(&path)],
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

// specs/002-firearm-identification User Story 3/4: the year-of-manufacture
// exception in import matching (FR-008, US4-5a, research.md §4). The
// spreadsheet's `year_of_manufacture` column is added by a later story
// (T047), so this exercises `find_match` directly against a real database —
// still no mocks, per the constitution.

use hoplodex_lib::services::import_matching::find_match;

#[test]
fn a_row_with_identical_main_marks_and_differing_years_on_both_sides_is_not_a_match() {
    let db = TestDb::new();
    let existing =
        FirearmInput { year_of_manufacture: Some(1943), ..existing_firearm_with_serial("SAA-1") };
    firearm_ops::create_firearm(&db.conn, &existing, false).unwrap();

    let found = find_match(&db.conn, "Glock", "19", Some("SAA-1"), false, Some(1944)).unwrap();
    assert_eq!(found, None, "both have a year and they differ: not a match (US4-5a)");
}

#[test]
fn a_row_with_identical_main_marks_and_no_year_on_either_side_still_matches() {
    let db = TestDb::new();
    let existing = existing_firearm_with_serial("SAA-2");
    let created = firearm_ops::create_firearm(&db.conn, &existing, false).unwrap();

    let found = find_match(&db.conn, "Glock", "19", Some("SAA-2"), false, None).unwrap();
    assert_eq!(found, Some(created.id));
}

#[test]
fn a_row_with_identical_main_marks_and_a_year_on_only_one_side_still_matches() {
    let db = TestDb::new();
    let existing =
        FirearmInput { year_of_manufacture: Some(1943), ..existing_firearm_with_serial("SAA-3") };
    let created = firearm_ops::create_firearm(&db.conn, &existing, false).unwrap();

    let found = find_match(&db.conn, "Glock", "19", Some("SAA-3"), false, None).unwrap();
    assert_eq!(found, Some(created.id), "a missing year on one side does not exempt the row");
}

fn existing_firearm_with_serial(serial: &str) -> FirearmInput {
    FirearmInput { serial_number: Some(serial.into()), ..existing_firearm() }
}

// --- specs/006-accessory-links US5: the record identifier first (tasks.md T101) -----------------
//
// contracts/spreadsheet-format.md "Record ID" (Matching): an identifier that
// belongs to an existing record of the row's own kind, active or disposed,
// is the row's match, ahead of the make, model and serial number key. For a
// firearm row anything else falls back to that key, as before; an accessory
// row has no other key, so it is new.

mod record_id_matching {
    use super::*;
    use hoplodex_lib::commands::accessories::ops as accessory_ops;
    use hoplodex_lib::models::accessory::AccessoryInput;
    use hoplodex_lib::models::record::RecordRef;
    use serde_json::{Value, json};

    const OPTIC: i64 = 1;

    fn add_firearm(db: &TestDb, make: &str, model: &str, serial: &str) -> RecordRef {
        let input = FirearmInput {
            make: make.into(),
            model: model.into(),
            serial_number: Some(serial.into()),
            ..existing_firearm()
        };
        RecordRef::Firearm(firearm_ops::create_firearm(&db.conn, &input, false).unwrap().id)
    }

    fn add_accessory(db: &TestDb, make: &str, model: &str, serial: &str) -> RecordRef {
        let mut input: AccessoryInput =
            serde_json::from_value(json!({ "accessoryKindId": OPTIC, "status": "active" }))
                .unwrap();
        input.make = Some(make.into());
        input.model = Some(model.into());
        input.serial_number = Some(serial.into());
        RecordRef::Accessory(accessory_ops::create_accessory(&db.conn, &input).unwrap().id)
    }

    fn uid_of(db: &TestDb, record: RecordRef) -> String {
        db.conn
            .query_row(
                &format!("SELECT uid FROM {} WHERE id = ?1", record.table()),
                [record.id()],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn import_firearm_rows(
        db: &TestDb,
        rows: &[Vec<String>],
    ) -> hoplodex_lib::commands::import_export::ImportResult {
        let dir = TempDir::new().unwrap();
        let path = sheets::csv_in(dir.path(), "firearms.csv", &sheets::firearm_table(rows));
        sheets::import(&db.conn, &ImportSessionStore::new(), &[&path]).unwrap()
    }

    fn import_accessory_rows(
        db: &TestDb,
        rows: &[Vec<String>],
    ) -> hoplodex_lib::commands::import_export::ImportResult {
        let dir = TempDir::new().unwrap();
        let path = sheets::csv_in(dir.path(), "accessories.csv", &sheets::accessory_table(rows));
        sheets::import(&db.conn, &ImportSessionStore::new(), &[&path]).unwrap()
    }

    fn existing_record(conflict: &impl serde::Serialize) -> RecordRef {
        let value: Value = serde_json::to_value(conflict).unwrap();
        serde_json::from_value(value["existingRecord"].clone()).unwrap()
    }

    #[test]
    fn a_firearm_row_matches_the_record_its_identifier_names_ahead_of_its_make_model_and_serial() {
        let db = TestDb::new();
        let by_key = add_firearm(&db, "Glock", "19", "ABC123");
        let by_id = add_firearm(&db, "Sig", "P226", "DEF456");

        // The identifier is the Sig's; the make, model and serial number are the Glock's.
        let row = sheets::firearm_cells(
            "Glock",
            "19",
            "ABC123",
            &[("record_id", &uid_of(&db, by_id)), ("notes", "from the sheet")],
        );
        let result = import_firearm_rows(&db, &[row]);

        assert_eq!(result.imported_count, 0);
        assert_eq!(result.conflicts.len(), 1, "{:?}", result.row_errors);
        assert_eq!(existing_record(&result.conflicts[0]), by_id);
        assert_ne!(existing_record(&result.conflicts[0]), by_key);
    }

    #[test]
    fn an_identifier_is_enough_even_when_the_rows_marks_differ_from_the_record() {
        let db = TestDb::new();
        let existing = add_firearm(&db, "Glock", "19", "ABC123");

        let row = sheets::firearm_cells(
            "Glock",
            "19 Gen5",
            "ZZZ999",
            &[("record_id", &uid_of(&db, existing))],
        );
        let result = import_firearm_rows(&db, &[row]);

        assert_eq!(result.conflicts.len(), 1, "{:?}", result.row_errors);
        assert_eq!(existing_record(&result.conflicts[0]), existing);
    }

    #[test]
    fn an_identifier_in_capitals_with_surrounding_spaces_still_matches() {
        let db = TestDb::new();
        let existing = add_firearm(&db, "Glock", "19", "ABC123");
        let written = format!("  {}  ", uid_of(&db, existing).to_uppercase());

        let row = sheets::firearm_cells("Sig", "P226", "DEF456", &[("record_id", &written)]);
        let result = import_firearm_rows(&db, &[row]);

        assert_eq!(result.conflicts.len(), 1, "{:?}", result.row_errors);
        assert_eq!(existing_record(&result.conflicts[0]), existing);
    }

    #[test]
    fn a_disposed_firearm_is_matched_by_its_identifier() {
        let db = TestDb::new();
        let sold = add_firearm(&db, "Glock", "19", "ABC123");
        dispose(&db, sold.id());

        let row =
            sheets::firearm_cells("Sig", "P226", "DEF456", &[("record_id", &uid_of(&db, sold))]);
        let result = import_firearm_rows(&db, &[row]);

        assert_eq!(result.conflicts.len(), 1, "{:?}", result.row_errors);
        assert_eq!(existing_record(&result.conflicts[0]), sold);
    }

    #[test]
    fn with_no_identifier_or_one_matching_nothing_the_make_model_serial_key_is_used_as_before() {
        let db = TestDb::new();
        let existing = add_firearm(&db, "Glock", "19", "ABC123");
        let nothing = hoplodex_lib::services::record_id::generate();

        let result = import_firearm_rows(
            &db,
            &[
                sheets::firearm_cells("Glock", "19", "ABC123", &[]),
                sheets::firearm_cells("glock", "19", "abc123", &[("record_id", &nothing)]),
            ],
        );

        assert_eq!(result.imported_count, 0, "{:?}", result.row_errors);
        assert_eq!(result.conflicts.len(), 2);
        assert!(result.conflicts.iter().all(|c| existing_record(c) == existing));
    }

    #[test]
    fn a_firearm_row_with_an_unknown_identifier_and_no_key_match_is_new_and_keeps_its_identifier() {
        let db = TestDb::new();
        add_firearm(&db, "Glock", "19", "ABC123");
        let fresh = hoplodex_lib::services::record_id::generate();

        let row = sheets::firearm_cells("Sig", "P226", "DEF456", &[("record_id", &fresh)]);
        let result = import_firearm_rows(&db, &[row]);

        assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
        let stored: String = db
            .conn
            .query_row("SELECT uid FROM firearms WHERE make = 'Sig'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, fresh);
    }

    #[test]
    fn an_accessory_row_matches_only_by_its_identifier() {
        let db = TestDb::new();
        let optic = add_accessory(&db, "Leupold", "VX-5HD", "OP-1");

        let result = import_accessory_rows(
            &db,
            &[sheets::accessory_cells(
                "Optic",
                &[("record_id", &uid_of(&db, optic)), ("make", "Vortex"), ("model", "Razor")],
            )],
        );

        assert_eq!(result.imported_count, 0, "{:?}", result.row_errors);
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(existing_record(&result.conflicts[0]), optic);
        let conflict: Value = serde_json::to_value(&result.conflicts[0]).unwrap();
        assert_eq!(conflict["table"], "accessories");
        assert_eq!(conflict["duplicateAllowed"], true);
    }

    #[test]
    fn an_accessory_row_with_the_same_kind_make_model_and_serial_but_no_identifier_is_new() {
        let db = TestDb::new();
        add_accessory(&db, "Leupold", "VX-5HD", "OP-1");
        let unmatched = hoplodex_lib::services::record_id::generate();

        let result = import_accessory_rows(
            &db,
            &[
                sheets::accessory_cells(
                    "Optic",
                    &[("make", "Leupold"), ("model", "VX-5HD"), ("serial_number", "OP-1")],
                ),
                sheets::accessory_cells(
                    "Optic",
                    &[
                        ("record_id", &unmatched),
                        ("make", "Leupold"),
                        ("model", "VX-5HD"),
                        ("serial_number", "OP-1"),
                    ],
                ),
            ],
        );

        assert_eq!(result.imported_count, 2, "{:?}", result.row_errors);
        assert!(result.conflicts.is_empty());
        let count: i64 =
            db.conn.query_row("SELECT count(*) FROM accessories", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 3);
        let kept: i64 = db
            .conn
            .query_row("SELECT count(*) FROM accessories WHERE uid = ?1", [&unmatched], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(kept, 1, "a created record keeps the row's identifier");
    }

    #[test]
    fn a_firearms_identifier_does_not_match_an_accessory_row_and_the_reverse() {
        let db = TestDb::new();
        let firearm = add_firearm(&db, "Glock", "19", "ABC123");
        let optic = add_accessory(&db, "Leupold", "VX-5HD", "OP-1");

        let accessories = import_accessory_rows(
            &db,
            &[sheets::accessory_cells("Optic", &[("record_id", &uid_of(&db, firearm))])],
        );
        assert_eq!(accessories.imported_count, 0);
        assert!(accessories.conflicts.is_empty());
        assert_eq!(
            accessories.row_errors.len(),
            1,
            "an identifier of the other kind is a row error"
        );

        let firearms = import_firearm_rows(
            &db,
            &[sheets::firearm_cells(
                "Sig",
                "P226",
                "DEF456",
                &[("record_id", &uid_of(&db, optic))],
            )],
        );
        assert_eq!(firearms.imported_count, 0);
        assert!(firearms.conflicts.is_empty());
        assert_eq!(firearms.row_errors.len(), 1);
    }
}
