//! Integration tests for make + model + serial-number uniqueness (FR-032,
//! spec.md US1 Acceptance Scenarios 10-11), run against a real temporary
//! SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::{ops, DisposeFirearmInput};
use hoplodex_lib::commands::CommandError;
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput};
use support::{firearm, TestDb};

/// A record attested as having no required serial number that nevertheless
/// records one (FR-032b).
fn exempt(make: &str, model: &str, serial: &str) -> FirearmInput {
    FirearmInput { no_serial_attested: true, ..firearm(make, model, serial) }
}

fn dispose(db: &TestDb, id: i64) {
    ops::dispose_firearm(
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

fn blocked_message(err: &CommandError) -> String {
    assert_eq!(err.code, "VALIDATION_ERROR", "{err:?}");
    err.field_errors
        .as_ref()
        .and_then(|fields| fields.get("serialNumber"))
        .unwrap_or_else(|| panic!("no serialNumber error in {err:?}"))
        .clone()
}

#[test]
fn scenario_10_a_non_exempt_duplicate_is_blocked_and_names_the_existing_record() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123")).unwrap();

    let err = ops::create_firearm_with_warnings(&db.conn, &firearm("Glock", "19", "ABC123"))
        .expect_err("a second active Glock 19 ABC123 must be blocked");

    let message = blocked_message(&err);
    assert!(message.contains("Glock") && message.contains("ABC123"), "{message}");
    let count: i64 = db.conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn the_same_serial_on_a_different_make_or_model_is_not_a_duplicate() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123")).unwrap();

    assert!(ops::create_firearm(&db.conn, &firearm("Glock", "26", "ABC123")).is_ok());
    assert!(ops::create_firearm(&db.conn, &firearm("Sig", "19", "ABC123")).is_ok());
}

#[test]
fn comparison_ignores_letter_case_and_surrounding_whitespace() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123")).unwrap();

    let err = ops::create_firearm_with_warnings(&db.conn, &firearm(" glock ", "19 ", "  abc123"))
        .expect_err("case and padding don't make it a different firearm");
    blocked_message(&err);
}

#[test]
fn scenario_10_a_disposed_match_never_blocks_so_a_firearm_can_be_reacquired() {
    let db = TestDb::new();
    let first = ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123")).unwrap();
    dispose(&db, first.id);

    let saved = ops::create_firearm_with_warnings(&db.conn, &firearm("Glock", "19", "ABC123"))
        .expect("reacquisition is a new record");
    assert!(saved.warnings.is_empty(), "a disposed match doesn't warn either");
}

#[test]
fn scenario_11_an_exempt_duplicate_saves_with_a_warning_naming_the_match() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Colt", "1911", "12345")).unwrap();

    let saved =
        ops::create_firearm_with_warnings(&db.conn, &exempt("Colt", "1911", "12345")).unwrap();

    assert_eq!(saved.warnings.len(), 1);
    assert!(saved.warnings[0].contains("Colt") && saved.warnings[0].contains("12345"));
    assert!(ops::get_firearm(&db.conn, saved.firearm.id).is_ok(), "the save succeeded");
}

#[test]
fn an_exempt_record_with_no_match_gets_no_warning() {
    let db = TestDb::new();
    let saved =
        ops::create_firearm_with_warnings(&db.conn, &exempt("Colt", "1911", "12345")).unwrap();
    assert!(saved.warnings.is_empty());
}

#[test]
fn a_non_exempt_record_matching_an_active_exempt_one_is_still_blocked() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &exempt("Colt", "1911", "12345")).unwrap();

    let err = ops::create_firearm_with_warnings(&db.conn, &firearm("Colt", "1911", "12345"))
        .expect_err("the new record is subject to serialization, so it can't duplicate");
    blocked_message(&err);
}

#[test]
fn records_with_no_serial_number_are_never_compared() {
    let db = TestDb::new();
    let none = |make: &str, model: &str| FirearmInput {
        serial_number: None,
        no_serial_attested: true,
        ..firearm(make, model, "")
    };
    ops::create_firearm(&db.conn, &none("Homemade", "AR-15")).unwrap();

    let saved = ops::create_firearm_with_warnings(&db.conn, &none("Homemade", "AR-15")).unwrap();
    assert!(saved.warnings.is_empty());
}

#[test]
fn the_rule_is_re_checked_on_edit_but_a_record_never_clashes_with_itself() {
    let db = TestDb::new();
    let a = ops::create_firearm(&db.conn, &firearm("Glock", "19", "AAA")).unwrap();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "BBB")).unwrap();

    // Saving A unchanged (or with other edits) is fine.
    let mut same = firearm("Glock", "19", "AAA");
    same.notes = Some("re-blued".into());
    assert!(ops::update_firearm_with_warnings(&db.conn, a.id, &same).is_ok());

    // Editing A's serial onto B's is a duplicate.
    let err = ops::update_firearm_with_warnings(&db.conn, a.id, &firearm("Glock", "19", "bbb"))
        .expect_err("blocked");
    blocked_message(&err);
    assert_eq!(ops::get_firearm(&db.conn, a.id).unwrap().serial_number.as_deref(), Some("AAA"));

    // An exempt edit onto B's serial saves with a warning.
    let saved =
        ops::update_firearm_with_warnings(&db.conn, a.id, &exempt("Glock", "19", "BBB")).unwrap();
    assert_eq!(saved.warnings.len(), 1);
}

#[test]
fn editing_a_disposed_record_is_not_checked_against_active_ones() {
    let db = TestDb::new();
    let first = ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123")).unwrap();
    dispose(&db, first.id);
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123")).unwrap();

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
    assert!(ops::update_firearm_with_warnings(&db.conn, first.id, &input).is_ok());
}

#[test]
fn the_database_refuses_a_non_exempt_duplicate_even_if_the_app_check_is_bypassed() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123")).unwrap();
    let other = ops::create_firearm(&db.conn, &firearm("Glock", "19", "XYZ")).unwrap();

    let backstop =
        db.conn.execute("UPDATE firearms SET serial_number = 'abc123' WHERE id = ?1", [other.id]);
    assert!(backstop.is_err(), "the partial unique index backs up FR-032a");
}
