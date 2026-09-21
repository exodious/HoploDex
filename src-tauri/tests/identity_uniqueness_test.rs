//! Integration tests for make + model + serial-number uniqueness (FR-032,
//! spec.md US1 Acceptance Scenarios 10-11 and FR-029), run against a real temporary
//! SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::{ops, DisposeFirearmInput};
use hoplodex_lib::commands::CommandError;
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput};
use support::{firearm, TestDb};

/// A record attested as having no serial number, so it records none (FR-029).
fn no_serial(make: &str, model: &str) -> FirearmInput {
    FirearmInput { serial_number: None, no_serial_attested: true, ..firearm(make, model, "") }
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
fn scenario_10_a_duplicate_is_blocked_and_names_the_existing_record() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123")).unwrap();

    let err = ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"))
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

    let err = ops::create_firearm(&db.conn, &firearm(" glock ", "19 ", "  abc123"))
        .expect_err("case and padding don't make it a different firearm");
    blocked_message(&err);
}

#[test]
fn scenario_10_a_disposed_match_never_blocks_so_a_firearm_can_be_reacquired() {
    let db = TestDb::new();
    let first = ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123")).unwrap();
    dispose(&db, first.id);

    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"))
        .expect("reacquisition is a new record");
}

#[test]
fn scenario_11_a_no_serial_firearm_is_never_compared_with_the_others() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Colt", "1911", "12345")).unwrap();

    // Same make and model as an active firearm, but no serial to compare.
    ops::create_firearm(&db.conn, &no_serial("Colt", "1911")).unwrap();
    ops::create_firearm(&db.conn, &no_serial("Colt", "1911")).unwrap();
}

#[test]
fn scenario_11_a_firearm_with_no_serial_number_cannot_also_record_one() {
    let db = TestDb::new();
    let both = FirearmInput { no_serial_attested: true, ..firearm("Colt", "1911", "12345") };

    let err = ops::create_firearm(&db.conn, &both).expect_err("attested and serial-bearing");

    assert!(blocked_message(&err).contains("can't also have one"));
}

#[test]
fn a_blank_serial_number_is_stored_as_none() {
    let db = TestDb::new();
    let blank = FirearmInput { serial_number: Some("  ".into()), ..no_serial("Colt", "1911") };

    let saved = ops::create_firearm(&db.conn, &blank).unwrap();

    assert_eq!(saved.serial_number, None);
}

#[test]
fn records_with_no_serial_number_are_never_compared() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &no_serial("Homemade", "AR-15")).unwrap();
    ops::create_firearm(&db.conn, &no_serial("Homemade", "AR-15")).unwrap();
}

#[test]
fn the_rule_is_re_checked_on_edit_but_a_record_never_clashes_with_itself() {
    let db = TestDb::new();
    let a = ops::create_firearm(&db.conn, &firearm("Glock", "19", "AAA")).unwrap();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "BBB")).unwrap();

    // Saving A unchanged (or with other edits) is fine.
    let mut same = firearm("Glock", "19", "AAA");
    same.notes = Some("re-blued".into());
    assert!(ops::update_firearm(&db.conn, a.id, &same).is_ok());

    // Editing A's serial onto B's is a duplicate.
    let err =
        ops::update_firearm(&db.conn, a.id, &firearm("Glock", "19", "bbb")).expect_err("blocked");
    blocked_message(&err);
    assert_eq!(ops::get_firearm(&db.conn, a.id).unwrap().serial_number.as_deref(), Some("AAA"));

    // Marking A as having no serial number takes it out of the comparison.
    assert!(ops::update_firearm(&db.conn, a.id, &no_serial("Glock", "19")).is_ok());
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
    assert!(ops::update_firearm(&db.conn, first.id, &input).is_ok());
}

#[test]
fn the_database_refuses_a_duplicate_even_if_the_app_check_is_bypassed() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123")).unwrap();
    let other = ops::create_firearm(&db.conn, &firearm("Glock", "19", "XYZ")).unwrap();

    let backstop =
        db.conn.execute("UPDATE firearms SET serial_number = 'abc123' WHERE id = ?1", [other.id]);
    assert!(backstop.is_err(), "the partial unique index backs up FR-032");
}

#[test]
fn the_database_refuses_a_record_with_both_a_serial_number_and_the_attestation() {
    let db = TestDb::new();
    let firearm = ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123")).unwrap();

    let backstop =
        db.conn.execute("UPDATE firearms SET no_serial_attested = 1 WHERE id = ?1", [firearm.id]);

    assert!(backstop.is_err(), "the CHECK backs up FR-029");
}
