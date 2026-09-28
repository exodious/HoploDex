//! Integration tests for the FR-009 original-marks warning
//! (specs/002-firearm-identification User Story 3), run against a real
//! temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::{DisposeFirearmInput, ops};
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput, Origin};
use support::{TestDb, firearm};

fn imported(make: &str, model: &str, serial: &str) -> FirearmInput {
    FirearmInput { origin: Some(Origin::Imported), ..firearm(make, model, serial) }
}

fn with_original_marks(
    make: &str,
    model: &str,
    serial: &str,
    original_make: &str,
    original_model: &str,
    original_serial: &str,
) -> FirearmInput {
    FirearmInput {
        original_make: Some(original_make.into()),
        original_model: Some(original_model.into()),
        original_serial_number: Some(original_serial.into()),
        ..imported(make, model, serial)
    }
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

#[test]
fn saving_a_firearm_whose_original_marks_match_another_warns_and_saves_nothing() {
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &with_original_marks("Ridgeline Arms", "Hi-Power", "RA-1", "FN", "High Power", "FN-1"),
        false,
    )
    .unwrap();

    let err = ops::create_firearm(
        &db.conn,
        &with_original_marks("Century Arms", "Hi-Power Clone", "CA-1", "FN", "High Power", "FN-1"),
        false,
    )
    .expect_err("matching original marks must warn, not save silently");
    assert_eq!(err.code, "ORIGINAL_MARKS_MATCH");
    assert!(err.message.contains("Ridgeline Arms"), "{}", err.message);

    let count: i64 = db.conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 1, "the second firearm was not saved");
}

#[test]
fn resending_with_confirmed_warnings_saves_it() {
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &with_original_marks("Ridgeline Arms", "Hi-Power", "RA-2", "FN", "High Power", "FN-2"),
        false,
    )
    .unwrap();

    let input =
        with_original_marks("Century Arms", "Hi-Power Clone", "CA-2", "FN", "High Power", "FN-2");
    ops::create_firearm(&db.conn, &input, false).expect_err("first attempt warns");

    let created = ops::create_firearm(&db.conn, &input, true)
        .expect("confirmed_warnings: true saves it (US3-5)");
    assert_eq!(created.original_serial_number.as_deref(), Some("FN-2"));

    let count: i64 = db.conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 2);
}

#[test]
fn a_partial_original_marks_set_never_warns() {
    let db = TestDb::new();
    let mut first =
        with_original_marks("Ridgeline Arms", "Hi-Power", "RA-3", "FN", "High Power", "FN-3");
    first.original_serial_number = None;
    ops::create_firearm(&db.conn, &first, false).unwrap();

    let mut second =
        with_original_marks("Century Arms", "Hi-Power Clone", "CA-3", "FN", "High Power", "FN-3");
    second.original_serial_number = None;
    ops::create_firearm(&db.conn, &second, false)
        .expect("a partial set on the existing record never triggers the warning (US3-8)");
}

#[test]
fn a_match_against_a_disposed_firearm_never_warns() {
    let db = TestDb::new();
    let first = ops::create_firearm(
        &db.conn,
        &with_original_marks("Ridgeline Arms", "Hi-Power", "RA-4", "FN", "High Power", "FN-4"),
        false,
    )
    .unwrap();
    dispose(&db, first.id);

    ops::create_firearm(
        &db.conn,
        &with_original_marks("Century Arms", "Hi-Power Clone", "CA-4", "FN", "High Power", "FN-4"),
        false,
    )
    .expect("a disposed firearm's original marks are never compared (US3-7)");
}

#[test]
fn matching_original_marks_with_different_main_serials_is_never_blocked_by_identity() {
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &with_original_marks("Ridgeline Arms", "Hi-Power", "RA-5", "FN", "High Power", "FN-5"),
        false,
    )
    .unwrap();

    // Different main make/model/serial, so FR-007/FR-008 never applies;
    // only the FR-009 warning should fire (US3-6).
    let err = ops::create_firearm(
        &db.conn,
        &with_original_marks(
            "Century Arms",
            "Totally Different Model",
            "ZZZ-999",
            "FN",
            "High Power",
            "FN-5",
        ),
        false,
    )
    .expect_err("original marks match, so the warning must fire");
    assert_eq!(err.code, "ORIGINAL_MARKS_MATCH");
}

#[test]
fn editing_a_firearm_into_a_match_also_warns() {
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &with_original_marks("Ridgeline Arms", "Hi-Power", "RA-6", "FN", "High Power", "FN-6"),
        false,
    )
    .unwrap();
    let other =
        ops::create_firearm(&db.conn, &imported("Century Arms", "Clone", "CA-6"), false).unwrap();

    let edited = FirearmInput {
        original_make: Some("FN".into()),
        original_model: Some("High Power".into()),
        original_serial_number: Some("FN-6".into()),
        ..FirearmInput::from(&other)
    };
    let err = ops::update_firearm(&db.conn, other.id, &edited, false)
        .expect_err("editing into a match warns just like creating one");
    assert_eq!(err.code, "ORIGINAL_MARKS_MATCH");

    let confirmed = ops::update_firearm(&db.conn, other.id, &edited, true).unwrap();
    assert_eq!(confirmed.original_serial_number.as_deref(), Some("FN-6"));
}

// specs/002-firearm-identification FR-006: the same non-judgment guarantee
// for the ORIGINAL_MARKS_MATCH warning.
#[test]
fn the_original_marks_match_message_never_judges_legality() {
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &with_original_marks(
            "Ridgeline Arms",
            "Hi-Power",
            "RA-WORDING",
            "FN",
            "High Power",
            "FN-WORDING",
        ),
        false,
    )
    .unwrap();

    let err = ops::create_firearm(
        &db.conn,
        &with_original_marks(
            "Century Arms",
            "Clone",
            "CA-WORDING",
            "FN",
            "High Power",
            "FN-WORDING",
        ),
        false,
    )
    .expect_err("warns");
    assert_eq!(err.code, "ORIGINAL_MARKS_MATCH");
    let message = err.message.to_lowercase();
    for word in ["legal", "illegal", "lawful", "unlawful", "permitted", "prohibited"] {
        assert!(!message.contains(word), "message should not judge legality: {message:?}");
    }
}
