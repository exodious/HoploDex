//! Integration tests for make + model + serial-number uniqueness (FR-032,
//! spec.md US1 Acceptance Scenarios 10-11 and FR-029), run against a real temporary
//! SQLCipher database.

mod support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::firearms::{DisposeInput, ops};
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput, Origin};
use support::{TestDb, firearm};

/// A record attested as having no serial number, so it records none (FR-029).
fn no_serial(make: &str, model: &str) -> FirearmInput {
    FirearmInput { serial_number: None, no_serial_attested: true, ..firearm(make, model, "") }
}

fn dispose(db: &TestDb, id: i64) {
    ops::dispose_firearm(
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

fn blocked_message(err: &CommandError) -> String {
    assert_eq!(err.code, "VALIDATION_ERROR", "{err:?}");
    err.field_errors
        .as_ref()
        .and_then(|fields| fields.get("serialNumber"))
        .unwrap_or_else(|| panic!("no serialNumber error in {err:?}"))
        .clone()
}

#[test]
fn scenario_10_a_duplicate_is_blocked_and_names_the_existing_record() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false).unwrap();

    let err = ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false)
        .expect_err("a second active Glock 19 ABC123 must be blocked");

    let message = blocked_message(&err);
    assert!(message.contains("Glock") && message.contains("ABC123"), "{message}");
    let count: i64 = db.conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn the_same_serial_on_a_different_make_or_model_is_not_a_duplicate() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false).unwrap();

    assert!(ops::create_firearm(&db.conn, &firearm("Glock", "26", "ABC123"), false).is_ok());
    assert!(ops::create_firearm(&db.conn, &firearm("Sig", "19", "ABC123"), false).is_ok());
}

#[test]
fn comparison_ignores_letter_case_and_surrounding_whitespace() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false).unwrap();

    let err = ops::create_firearm(&db.conn, &firearm(" glock ", "19 ", "  abc123"), false)
        .expect_err("case and padding don't make it a different firearm");
    blocked_message(&err);
}

#[test]
fn scenario_10_a_disposed_match_never_blocks_so_a_firearm_can_be_reacquired() {
    let db = TestDb::new();
    let first = ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false).unwrap();
    dispose(&db, first.id);

    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false)
        .expect("reacquisition is a new record");
}

#[test]
fn scenario_11_a_no_serial_firearm_is_never_compared_with_the_others() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Colt", "1911", "12345"), false).unwrap();

    // Same make and model as an active firearm, but no serial to compare.
    ops::create_firearm(&db.conn, &no_serial("Colt", "1911"), false).unwrap();
    ops::create_firearm(&db.conn, &no_serial("Colt", "1911"), false).unwrap();
}

#[test]
fn scenario_11_a_firearm_with_no_serial_number_cannot_also_record_one() {
    let db = TestDb::new();
    let both = FirearmInput { no_serial_attested: true, ..firearm("Colt", "1911", "12345") };

    let err = ops::create_firearm(&db.conn, &both, false).expect_err("attested and serial-bearing");

    assert!(blocked_message(&err).contains("can't also have one"));
}

#[test]
fn a_blank_serial_number_is_stored_as_none() {
    let db = TestDb::new();
    let blank = FirearmInput { serial_number: Some("  ".into()), ..no_serial("Colt", "1911") };

    let saved = ops::create_firearm(&db.conn, &blank, false).unwrap();

    assert_eq!(saved.serial_number, None);
}

#[test]
fn records_with_no_serial_number_are_never_compared() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &no_serial("Homemade", "AR-15"), false).unwrap();
    ops::create_firearm(&db.conn, &no_serial("Homemade", "AR-15"), false).unwrap();
}

#[test]
fn the_rule_is_re_checked_on_edit_but_a_record_never_clashes_with_itself() {
    let db = TestDb::new();
    let a = ops::create_firearm(&db.conn, &firearm("Glock", "19", "AAA"), false).unwrap();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "BBB"), false).unwrap();

    // Saving A unchanged (or with other edits) is fine.
    let mut same = firearm("Glock", "19", "AAA");
    same.notes = Some("re-blued".into());
    assert!(ops::update_firearm(&db.conn, a.id, &same, false).is_ok());

    // Editing A's serial onto B's is a duplicate.
    let err = ops::update_firearm(&db.conn, a.id, &firearm("Glock", "19", "bbb"), false)
        .expect_err("blocked");
    blocked_message(&err);
    assert_eq!(ops::get_firearm(&db.conn, a.id).unwrap().serial_number.as_deref(), Some("AAA"));

    // Marking A as having no serial number takes it out of the comparison.
    assert!(ops::update_firearm(&db.conn, a.id, &no_serial("Glock", "19"), false).is_ok());
}

#[test]
fn editing_a_disposed_record_is_not_checked_against_active_ones() {
    let db = TestDb::new();
    let first = ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false).unwrap();
    dispose(&db, first.id);
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false).unwrap();

    // The disposed record is history; correcting a note on it can't clash.
    let mut corrected = ops::get_firearm(&db.conn, first.id).unwrap();
    corrected.notes = Some("sold to a friend".into());
    let input = FirearmInput {
        status: corrected.status,
        disposition_type: corrected.disposition_type,
        disposition_recipient: corrected.disposition_recipient.clone(),
        disposition_date: corrected.disposition_date.clone(),
        disposition_price: corrected.disposition_price,
        notes: corrected.notes.clone(),
        ..firearm("Glock", "19", "ABC123")
    };
    assert!(ops::update_firearm(&db.conn, first.id, &input, false).is_ok());
}

#[test]
fn the_database_refuses_a_duplicate_even_if_the_app_check_is_bypassed() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false).unwrap();
    let other = ops::create_firearm(&db.conn, &firearm("Glock", "19", "XYZ"), false).unwrap();

    let backstop =
        db.conn.execute("UPDATE firearms SET serial_number = 'abc123' WHERE id = ?1", [other.id]);
    assert!(backstop.is_err(), "the identity trigger backs up FR-032");
}

// specs/002-firearm-identification User Story 3: the year-of-manufacture
// exception to the identity rule (FR-007/FR-008, data-model.md's "Identity
// uniqueness"; amends 001 FR-032).

fn with_year(make: &str, model: &str, serial: &str, year: Option<i64>) -> FirearmInput {
    FirearmInput { year_of_manufacture: year, ..firearm(make, model, serial) }
}

#[test]
fn a_duplicate_with_no_year_on_either_record_is_blocked_and_points_to_the_year() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-1", None), false).unwrap();

    let err = ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-1", None), false)
        .expect_err("no year on either record still blocks (US3-1)");
    let message = blocked_message(&err);
    assert!(message.contains("Colt") && message.contains("SAA-1"), "{message}");
    assert!(
        message.to_lowercase().contains("year of manufacture"),
        "message should point to recording a year: {message}"
    );
}

#[test]
fn a_duplicate_with_different_years_on_each_record_is_accepted() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-2", Some(1943)), false).unwrap();

    ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-2", Some(1944)), false)
        .expect("both records have a year and the years differ (FR-008)");
}

#[test]
fn a_duplicate_with_the_same_year_on_both_records_is_blocked() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-3", Some(1943)), false).unwrap();

    let err = ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-3", Some(1943)), false)
        .expect_err("the same year on both does not distinguish them");
    blocked_message(&err);
}

#[test]
fn a_duplicate_with_a_year_missing_on_one_record_is_blocked() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-4", Some(1943)), false).unwrap();

    let err = ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-4", None), false)
        .expect_err("a missing year on one side still blocks (FR-008)");
    blocked_message(&err);
}

#[test]
fn the_same_rule_applies_to_a_pre_1968_domestic_pair() {
    // US3-3: pre-1968 domestic firearms whose maker restarted serial
    // numbering are told apart the same way as any other pair.
    let db = TestDb::new();
    let smith_1955 = FirearmInput {
        origin: Some(Origin::Domestic),
        year_of_manufacture: Some(1955),
        ..firearm("Smith & Wesson", "Model 10", "S-100")
    };
    let smith_1962 = FirearmInput {
        origin: Some(Origin::Domestic),
        year_of_manufacture: Some(1962),
        ..firearm("Smith & Wesson", "Model 10", "S-100")
    };
    let smith_no_year = FirearmInput {
        origin: Some(Origin::Domestic),
        ..firearm("Smith & Wesson", "Model 10", "S-100")
    };

    ops::create_firearm(&db.conn, &smith_1955, false).unwrap();
    ops::create_firearm(&db.conn, &smith_1962, false).expect("differing years are accepted");
    let err = ops::create_firearm(&db.conn, &smith_no_year, false)
        .expect_err("a third record with no year still clashes with the others");
    blocked_message(&err);
}

#[test]
fn editing_a_second_firearms_year_to_match_the_first_is_blocked() {
    // US3-4a
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-5", Some(1943)), false).unwrap();
    let second =
        ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-5", Some(1944)), false)
            .unwrap();

    let err = ops::update_firearm(
        &db.conn,
        second.id,
        &with_year("Colt", "1873", "SAA-5", Some(1943)),
        false,
    )
    .expect_err("editing the year to match removes the only thing distinguishing them");
    blocked_message(&err);
}

#[test]
fn the_database_backstop_allows_a_pair_distinguished_by_year() {
    // T028: the raw-SQL backstop applies the same year exception the
    // command layer does.
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-6", Some(1943)), false).unwrap();

    let bypassed = db.conn.execute(
        "INSERT INTO firearms (
            uid, make, model, serial_number, no_serial_attested, caliber, firearm_type_id,
            status, year_of_manufacture, created_at, updated_at
        ) VALUES (?1, 'Colt', '1873', 'SAA-6', 0, '.45 Colt', 1, 'active', 1944, datetime('now'), datetime('now'))",
        [support::uid()],
    );
    assert!(bypassed.is_ok(), "distinguished by year, the trigger must allow it: {bypassed:?}");
}

#[test]
fn the_database_backstop_blocks_a_null_year_pair_bypassing_the_command_layer() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-7", None), false).unwrap();

    let bypassed = db.conn.execute(
        "INSERT INTO firearms (
            uid, make, model, serial_number, no_serial_attested, caliber, firearm_type_id,
            status, created_at, updated_at
        ) VALUES (?1, 'Colt', '1873', 'SAA-7', 0, '.45 Colt', 1, 'active', datetime('now'), datetime('now'))",
        [support::uid()],
    );
    assert!(bypassed.is_err(), "two null-year duplicates must still be blocked");
}

#[test]
fn the_database_backstop_blocks_an_equal_year_pair_bypassing_the_command_layer() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-8", Some(1943)), false).unwrap();

    let bypassed = db.conn.execute(
        "INSERT INTO firearms (
            uid, make, model, serial_number, no_serial_attested, caliber, firearm_type_id,
            status, year_of_manufacture, created_at, updated_at
        ) VALUES (?1, 'Colt', '1873', 'SAA-8', 0, '.45 Colt', 1, 'active', 1943, datetime('now'), datetime('now'))",
        [support::uid()],
    );
    assert!(bypassed.is_err(), "an equal year on both must still be blocked");
}

#[test]
fn the_database_backstop_blocks_a_year_no_year_pair_bypassing_the_command_layer() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &with_year("Colt", "1873", "SAA-9", Some(1943)), false).unwrap();

    let bypassed = db.conn.execute(
        "INSERT INTO firearms (
            uid, make, model, serial_number, no_serial_attested, caliber, firearm_type_id,
            status, created_at, updated_at
        ) VALUES (?1, 'Colt', '1873', 'SAA-9', 0, '.45 Colt', 1, 'active', datetime('now'), datetime('now'))",
        [support::uid()],
    );
    assert!(bypassed.is_err(), "a year on only one side must still be blocked");
}

#[test]
fn the_database_refuses_a_record_with_both_a_serial_number_and_the_attestation() {
    let db = TestDb::new();
    let firearm = ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false).unwrap();

    let backstop =
        db.conn.execute("UPDATE firearms SET no_serial_attested = 1 WHERE id = ?1", [firearm.id]);

    assert!(backstop.is_err(), "the CHECK backs up FR-029");
}

// specs/002-firearm-identification FR-006: the app never states or implies
// a mark, year or import is legal or illegal.
#[test]
fn the_fr_008_identity_clash_message_never_judges_legality() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &with_year("Colt", "1873", "WORDING-CHECK-1", None), false)
        .unwrap();

    let err =
        ops::create_firearm(&db.conn, &with_year("Colt", "1873", "WORDING-CHECK-1", None), false)
            .expect_err("blocked");
    let message = blocked_message(&err).to_lowercase();
    for word in ["legal", "illegal", "lawful", "unlawful", "permitted", "prohibited"] {
        assert!(!message.contains(word), "message should not judge legality: {message:?}");
    }
}

// specs/005-regulated-item-types US1-6, FR-005: a Suppressor is identified
// like any firearm.

fn suppressor(serial: &str) -> FirearmInput {
    FirearmInput {
        firearm_type_id: 5,
        caliber: ".30".into(),
        ..firearm("SilencerCo", "Omega 300", serial)
    }
}

#[test]
fn a_duplicate_suppressor_is_blocked_with_or_without_a_classification() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &suppressor("ABC123"), false).unwrap();

    let err = ops::create_firearm(&db.conn, &suppressor("ABC123"), false)
        .expect_err("a second active Omega 300 ABC123 must be blocked");
    let message = blocked_message(&err);
    assert!(message.contains("SilencerCo") && message.contains("ABC123"), "{message}");

    let classified = FirearmInput { registration_class_id: Some(1), ..suppressor("ABC123") };
    let err = ops::create_firearm(&db.conn, &classified, false)
        .expect_err("a classification doesn't make it a different firearm");
    blocked_message(&err);
    assert_eq!(
        db.conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get::<_, i64>(0)).unwrap(),
        1
    );
}

#[test]
fn a_suppressor_with_no_serial_attestation_saves() {
    let db = TestDb::new();
    let input = FirearmInput { firearm_type_id: 5, ..no_serial("Homemade", "Solvent trap") };
    assert!(ops::create_firearm(&db.conn, &input, false).is_ok());
}
