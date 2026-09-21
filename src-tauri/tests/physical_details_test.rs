//! Integration tests for a firearm's optional physical details — barrel
//! length, overall length, weight, capacity, finish and condition (FR-039,
//! spec.md US1 Acceptance Scenario 17) — against a real temporary SQLCipher
//! database. Search on `finish` is covered in `fts_search_test.rs`.

mod support;

use hoplodex_lib::commands::firearms::{
    ops, DisposeFirearmInput, HistoryChoice, ReverseDispositionInput,
};
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::firearm::{Condition, DispositionType, FirearmInput};
use support::{firearm, policy, TestDb};

fn detailed(serial: &str) -> FirearmInput {
    FirearmInput {
        barrel_length_hundredths: Some(1625),
        overall_length_hundredths: Some(3600),
        weight_tenths_oz: Some(405),
        capacity: Some(15),
        finish: Some("Cerakote flat dark earth".into()),
        condition: Some(Condition::Excellent),
        ..firearm("Glock", "19", serial)
    }
}

fn assert_all_six(saved: &hoplodex_lib::models::firearm::Firearm) {
    assert_eq!(saved.barrel_length_hundredths, Some(1625));
    assert_eq!(saved.overall_length_hundredths, Some(3600));
    assert_eq!(saved.weight_tenths_oz, Some(405));
    assert_eq!(saved.capacity, Some(15));
    assert_eq!(saved.finish.as_deref(), Some("Cerakote flat dark earth"));
    assert_eq!(saved.condition, Some(Condition::Excellent));
}

fn field_error(input: &FirearmInput, field: &str) {
    let db = TestDb::new();
    let err = ops::create_firearm(&db.conn, input).expect_err("should be rejected");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(
        err.field_errors.as_ref().is_some_and(|errors| errors.contains_key(field)),
        "expected a field error for {field}, got {:?}",
        err.field_errors
    );
}

#[test]
fn all_six_round_trip_through_create_update_and_get() {
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &detailed("G-1")).unwrap();
    assert_all_six(&created);
    assert_all_six(&ops::get_firearm(&db.conn, created.id).unwrap());

    let edited = FirearmInput {
        barrel_length_hundredths: Some(400),
        overall_length_hundredths: Some(700),
        weight_tenths_oz: Some(215),
        capacity: Some(10),
        finish: Some("Blued".into()),
        condition: Some(Condition::NewInBox),
        ..detailed("G-1")
    };
    let updated = ops::update_firearm(&db.conn, created.id, &edited).unwrap();
    assert_eq!(updated.barrel_length_hundredths, Some(400));
    assert_eq!(updated.overall_length_hundredths, Some(700));
    assert_eq!(updated.weight_tenths_oz, Some(215));
    assert_eq!(updated.capacity, Some(10));
    assert_eq!(updated.finish.as_deref(), Some("Blued"));
    assert_eq!(updated.condition, Some(Condition::NewInBox));
    assert_eq!(ops::get_firearm(&db.conn, created.id).unwrap().condition, updated.condition);

    let cleared =
        ops::update_firearm(&db.conn, created.id, &firearm("Glock", "19", "G-1")).unwrap();
    assert_eq!(cleared.barrel_length_hundredths, None);
    assert_eq!(cleared.condition, None);
}

#[test]
fn all_six_null_is_accepted_for_every_firearm_type() {
    let db = TestDb::new();
    for type_id in 1..=4 {
        let input = FirearmInput {
            firearm_type_id: type_id,
            ..firearm("Make", "Model", &format!("SN-{type_id}"))
        };
        let saved = ops::create_firearm(&db.conn, &input).unwrap();
        assert_eq!(saved.barrel_length_hundredths, None);
        assert_eq!(saved.overall_length_hundredths, None);
        assert_eq!(saved.weight_tenths_oz, None);
        assert_eq!(saved.capacity, None);
        assert_eq!(saved.finish, None);
        assert_eq!(saved.condition, None);
    }
}

#[test]
fn a_length_or_weight_of_zero_or_less_is_rejected_naming_the_field() {
    for bad in [0, -1] {
        field_error(
            &FirearmInput { barrel_length_hundredths: Some(bad), ..detailed("A") },
            "barrelLengthHundredths",
        );
        field_error(
            &FirearmInput { overall_length_hundredths: Some(bad), ..detailed("A") },
            "overallLengthHundredths",
        );
        field_error(
            &FirearmInput { weight_tenths_oz: Some(bad), ..detailed("A") },
            "weightTenthsOz",
        );
    }
}

#[test]
fn a_capacity_below_one_is_rejected_naming_the_field() {
    for bad in [0, -5] {
        field_error(&FirearmInput { capacity: Some(bad), ..detailed("A") }, "capacity");
    }
    let db = TestDb::new();
    let one = FirearmInput { capacity: Some(1), ..detailed("A") };
    assert_eq!(ops::create_firearm(&db.conn, &one).unwrap().capacity, Some(1));
}

#[test]
fn a_condition_outside_the_closed_list_is_rejected() {
    let good = serde_json::to_value(Condition::LikeNew).unwrap();
    assert_eq!(good, "like_new");
    for grade in ["new_in_box", "like_new", "excellent", "good", "fair", "poor"] {
        assert!(serde_json::from_value::<Condition>(grade.into()).is_ok(), "{grade}");
    }
    assert!(serde_json::from_value::<Condition>("mint".into()).is_err());

    // And the database refuses one that got past the type.
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &detailed("C-1")).unwrap();
    let raw = db.conn.execute("UPDATE firearms SET condition = 'mint' WHERE id = ?1", [created.id]);
    assert!(raw.is_err(), "the CHECK constraint should refuse an unknown grade");
}

#[test]
fn a_blank_or_whitespace_only_finish_is_stored_as_null() {
    let db = TestDb::new();
    for (n, blank) in ["", "   ", "\n\t "].into_iter().enumerate() {
        let input = FirearmInput {
            finish: Some(blank.into()),
            ..firearm("Colt", "1911", &format!("F-{n}"))
        };
        assert_eq!(ops::create_firearm(&db.conn, &input).unwrap().finish, None, "{blank:?}");
    }
    let padded =
        FirearmInput { finish: Some("  Parkerized  ".into()), ..firearm("Colt", "1911", "F-9") };
    assert_eq!(
        ops::create_firearm(&db.conn, &padded).unwrap().finish.as_deref(),
        Some("Parkerized")
    );
}

#[test]
fn the_values_survive_disposal_reversal_and_coverage_assignment() {
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &detailed("S-1")).unwrap();

    let disposed = ops::dispose_firearm(
        &db.conn,
        created.id,
        &DisposeFirearmInput {
            disposition_type: DispositionType::Sold,
            recipient: "Jane Doe".into(),
            date: "2024-01-01".into(),
            price: 400,
        },
    )
    .unwrap();
    assert_all_six(&disposed);

    let restored = ops::reverse_disposition(
        &db.conn,
        created.id,
        &ReverseDispositionInput { history: HistoryChoice::Keep, nickname: None },
    )
    .unwrap();
    assert_all_six(&restored);

    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2020-01-01", "2099-01-01", None))
            .unwrap();
    let scheduled =
        insurance_ops::assign_firearm_coverage(&db.conn, created.id, Some(rider.id), Some(900))
            .unwrap();
    assert_all_six(&scheduled);
    assert_all_six(&ops::get_firearm(&db.conn, created.id).unwrap());
}
