//! specs/005-regulated-item-types User Story 2 through `ops`, against a real
//! temporary SQLCipher database: an optional "registered as" classification
//! with a form, an approved date and "registered to" on any firearm
//! (FR-007 to FR-014), and nothing about legal status is ever derived.

mod support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::entries::ops as entry_ops;
use hoplodex_lib::commands::firearms::ops;
use hoplodex_lib::commands::firearms::{
    DisposeFirearmInput, HistoryChoice, ReverseDispositionInput,
};
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput};
use support::{TestDb, firearm, policy};

const RIFLE: i64 = 2;
const SUPPRESSOR: i64 = 5;
const SUPPRESSOR_CLASS: i64 = 1;

fn registered(serial: &str, type_id: i64) -> FirearmInput {
    FirearmInput {
        firearm_type_id: type_id,
        registration_class_id: Some(SUPPRESSOR_CLASS),
        registration_form: Some("Form 4".into()),
        registration_approved: Some("2026-02-10".into()),
        registered_to: Some("Smith Family Trust".into()),
        ..firearm("SilencerCo", "Omega 300", serial)
    }
}

fn field_error(err: &CommandError, field: &str) -> Option<String> {
    err.field_errors.as_ref().and_then(|errors| errors.get(field).cloned())
}

fn tomorrow() -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string()
}

#[test]
fn the_list_has_the_six_classifications_in_order_all_offered() {
    // US2-1.
    let db = TestDb::new();
    let output = entry_ops::list_registration_classes(&db.conn).unwrap();
    let names: Vec<&str> = output.classes.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Suppressor",
            "Short-barreled rifle",
            "Short-barreled shotgun",
            "Any other weapon",
            "Machine gun",
            "Destructive device"
        ]
    );
    assert!(output.classes.iter().all(|c| c.offered));
    let json = serde_json::to_value(&output.classes[0]).unwrap();
    assert_eq!(json["offered"], true);
}

#[test]
fn registration_saves_and_reopens_intact_on_a_suppressor_and_a_rifle() {
    // US2-3, US2-4, FR-011: nothing derived is returned.
    let db = TestDb::new();
    for (serial, type_id) in [("S1", SUPPRESSOR), ("R1", RIFLE)] {
        let input = FirearmInput { caliber: ".30".into(), ..registered(serial, type_id) };
        let created = ops::create_firearm(&db.conn, &input, false).unwrap();
        let fetched = ops::get_firearm(&db.conn, created.id).unwrap();
        assert_eq!(fetched.registration_class_id, Some(SUPPRESSOR_CLASS));
        assert_eq!(fetched.registration_form.as_deref(), Some("Form 4"));
        assert_eq!(fetched.registration_approved.as_deref(), Some("2026-02-10"));
        assert_eq!(fetched.registered_to.as_deref(), Some("Smith Family Trust"));
        let json = serde_json::to_value(&fetched).unwrap();
        for derived in ["registrationStatus", "isRegulated", "legalStatus", "status_note"] {
            assert!(json.get(derived).is_none(), "{derived} must not exist");
        }
    }
    // A classification alone, and with some details.
    let bare = FirearmInput {
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        ..registered("B1", RIFLE)
    };
    let created = ops::create_firearm(&db.conn, &bare, false).unwrap();
    assert_eq!(created.registration_class_id, Some(SUPPRESSOR_CLASS));
    assert_eq!(created.registration_form, None);
    let some = FirearmInput {
        registration_approved: None,
        registered_to: None,
        ..registered("B2", RIFLE)
    };
    let created = ops::create_firearm(&db.conn, &some, false).unwrap();
    assert_eq!(created.registration_form.as_deref(), Some("Form 4"));
    assert_eq!(created.registered_to, None);
}

#[test]
fn details_without_a_classification_are_refused() {
    // FR-009, in the command layer and in the table's CHECK.
    let db = TestDb::new();
    let message = "Choose what the firearm is registered as first.";
    for (field, input) in [
        (
            "registrationForm",
            FirearmInput { registration_form: Some("Form 4".into()), ..firearm("A", "B", "1") },
        ),
        (
            "registrationApproved",
            FirearmInput {
                registration_approved: Some("2026-02-10".into()),
                ..firearm("A", "B", "2")
            },
        ),
        (
            "registeredTo",
            FirearmInput { registered_to: Some("A Trust".into()), ..firearm("A", "B", "3") },
        ),
    ] {
        let err = ops::create_firearm(&db.conn, &input, false).expect_err(field);
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(field_error(&err, field).as_deref(), Some(message), "{field}");
    }
    for column in ["registration_form", "registration_approved", "registered_to"] {
        let result = db.conn.execute(
            &format!(
                "INSERT INTO firearms (make, model, caliber, firearm_type_id, no_serial_attested,
                    {column}, created_at, updated_at)
                 VALUES ('A', 'B', '9mm', 1, 1, 'x', datetime('now'), datetime('now'))"
            ),
            [],
        );
        assert!(result.is_err(), "{column} without a classification must fail the CHECK");
    }
}

#[test]
fn an_unknown_classification_is_refused() {
    let db = TestDb::new();
    let input = FirearmInput { registration_class_id: Some(99), ..firearm("A", "B", "1") };
    let err = ops::create_firearm(&db.conn, &input, false).unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(
        field_error(&err, "registrationClassId").as_deref(),
        Some("Choose a classification from the list.")
    );
    let created = ops::create_firearm(&db.conn, &firearm("A", "B", "2"), false).unwrap();
    let err = ops::update_firearm(&db.conn, created.id, &input, false).unwrap_err();
    assert_eq!(
        field_error(&err, "registrationClassId").as_deref(),
        Some("Choose a classification from the list.")
    );
}

#[test]
fn the_approved_date_must_be_a_date_and_not_in_the_future() {
    // US2-6, FR-010.
    let db = TestDb::new();
    let future = FirearmInput { registration_approved: Some(tomorrow()), ..registered("1", RIFLE) };
    let err = ops::create_firearm(&db.conn, &future, false).unwrap_err();
    assert_eq!(
        field_error(&err, "registrationApproved").as_deref(),
        Some("Approved date can't be in the future.")
    );
    let bad =
        FirearmInput { registration_approved: Some("2026-13-40".into()), ..registered("2", RIFLE) };
    let err = ops::create_firearm(&db.conn, &bad, false).unwrap_err();
    assert_eq!(
        field_error(&err, "registrationApproved").as_deref(),
        Some("Approved date must be a date in YYYY-MM-DD format.")
    );
    let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
    let ok = FirearmInput { registration_approved: Some(today), ..registered("3", RIFLE) };
    ops::create_firearm(&db.conn, &ok, false).unwrap();
}

#[test]
fn form_and_registered_to_follow_the_entry_rules_and_are_trimmed() {
    // 004 FR-015, on entry only.
    let db = TestDb::new();
    let long = FirearmInput { registration_form: Some("F".repeat(101)), ..registered("1", RIFLE) };
    let err = ops::create_firearm(&db.conn, &long, false).unwrap_err();
    assert_eq!(
        field_error(&err, "registrationForm").as_deref(),
        Some("Form can be at most 100 characters.")
    );
    let tab = FirearmInput { registered_to: Some("Smith\tTrust".into()), ..registered("2", RIFLE) };
    let err = ops::create_firearm(&db.conn, &tab, false).unwrap_err();
    assert_eq!(
        field_error(&err, "registeredTo").as_deref(),
        Some("Registered to can't contain control characters.")
    );

    let padded = FirearmInput {
        registration_form: Some("  Form 4 ".into()),
        registered_to: Some("  A Trust\n".into()),
        registration_approved: Some(" 2026-02-10 ".into()),
        ..registered("3", RIFLE)
    };
    let created = ops::create_firearm(&db.conn, &padded, false).unwrap();
    assert_eq!(created.registration_form.as_deref(), Some("Form 4"));
    assert_eq!(created.registered_to.as_deref(), Some("A Trust"));
    assert_eq!(created.registration_approved.as_deref(), Some("2026-02-10"));

    // An over-long value already stored passes while unchanged.
    let id = created.id;
    db.conn
        .execute(
            "UPDATE firearms SET registered_to = ?1 WHERE id = ?2",
            rusqlite::params!["T".repeat(150), id],
        )
        .unwrap();
    let stored = ops::get_firearm(&db.conn, id).unwrap();
    let unchanged = FirearmInput { notes: Some("edited".into()), ..FirearmInput::from(&stored) };
    ops::update_firearm(&db.conn, id, &unchanged, false).unwrap();
    // But an edit of that very field is checked.
    let edited = FirearmInput {
        registered_to: Some(format!("{}x", "T".repeat(150))),
        ..FirearmInput::from(&stored)
    };
    assert!(ops::update_firearm(&db.conn, id, &edited, false).is_err());
}

#[test]
fn changing_the_classification_keeps_details_and_clearing_all_clears_them() {
    // FR-012.
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &registered("1", RIFLE), false).unwrap();
    let other = FirearmInput { registration_class_id: Some(2), ..FirearmInput::from(&created) };
    let changed = ops::update_firearm(&db.conn, created.id, &other, false).unwrap();
    assert_eq!(changed.registration_class_id, Some(2));
    assert_eq!(changed.registration_form.as_deref(), Some("Form 4"));
    assert_eq!(changed.registered_to.as_deref(), Some("Smith Family Trust"));

    let cleared = FirearmInput {
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        ..FirearmInput::from(&changed)
    };
    let cleared = ops::update_firearm(&db.conn, created.id, &cleared, false).unwrap();
    assert_eq!(cleared.registration_class_id, None);
    assert_eq!(cleared.registration_form, None);
    assert_eq!(cleared.registration_approved, None);
    assert_eq!(cleared.registered_to, None);
}

#[test]
fn disposal_reversal_and_coverage_keep_the_details_and_deleting_removes_them() {
    // FR-013, US2-12.
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &registered("1", RIFLE), false).unwrap();
    let same = |f: &hoplodex_lib::models::Firearm| {
        assert_eq!(f.registration_class_id, Some(SUPPRESSOR_CLASS));
        assert_eq!(f.registration_form.as_deref(), Some("Form 4"));
        assert_eq!(f.registration_approved.as_deref(), Some("2026-02-10"));
        assert_eq!(f.registered_to.as_deref(), Some("Smith Family Trust"));
    };
    let disposed = ops::dispose_firearm(
        &db.conn,
        created.id,
        &DisposeFirearmInput {
            disposition_type: DispositionType::Sold,
            recipient: "Jane Doe".into(),
            date: "2026-03-01".into(),
            price: 100,
        },
    )
    .unwrap();
    same(&disposed);
    let restored = ops::reverse_disposition(
        &db.conn,
        created.id,
        &ReverseDispositionInput {
            history: HistoryChoice::Discard,
            nickname: None,
            confirmed_warnings: false,
        },
    )
    .unwrap();
    same(&restored);

    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2026-01-01", "2026-12-31", None))
            .unwrap();
    let covered =
        insurance_ops::assign_firearm_coverage(&db.conn, created.id, Some(rider.id), Some(500))
            .unwrap();
    same(&covered);

    ops::delete_firearm(&db.conn, created.id, true).unwrap();
    let left: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM firearms WHERE registered_to = 'Smith Family Trust'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
}

#[test]
fn a_classification_no_longer_offered_is_still_held_and_accepted() {
    // SC-004, research.md §5.
    let db = TestDb::new();
    let held = ops::create_firearm(
        &db.conn,
        &FirearmInput { registration_class_id: Some(5), ..firearm("A", "B", "1") },
        false,
    )
    .unwrap();
    db.conn.execute("UPDATE registration_classes SET offered = 0 WHERE id = 5", []).unwrap();

    let classes = entry_ops::list_registration_classes(&db.conn).unwrap().classes;
    assert_eq!(classes.len(), 6);
    let machine_gun = classes.iter().find(|c| c.id == 5).unwrap();
    assert!(!machine_gun.offered);
    assert!(classes.iter().filter(|c| c.id != 5).all(|c| c.offered));

    let edit = FirearmInput { notes: Some("edited".into()), ..FirearmInput::from(&held) };
    let saved = ops::update_firearm(&db.conn, held.id, &edit, false).unwrap();
    assert_eq!(saved.registration_class_id, Some(5));
    let fresh = ops::create_firearm(
        &db.conn,
        &FirearmInput { registration_class_id: Some(5), ..firearm("A", "B", "2") },
        false,
    )
    .unwrap();
    assert_eq!(fresh.registration_class_id, Some(5));
}

#[test]
fn every_type_saves_with_no_classification_and_with_each_of_the_six() {
    // FR-008, FR-014, SC-002: the classification is independent of type and
    // nothing judges it.
    let db = TestDb::new();
    let mut n = 0;
    for type_id in 1..=5 {
        let applies = type_id != SUPPRESSOR;
        for class in std::iter::once(None).chain((1..=6).map(Some)) {
            n += 1;
            let input = FirearmInput {
                firearm_type_id: type_id,
                registration_class_id: class,
                barrel_length_hundredths: applies.then_some(1050),
                ..firearm("Make", "Model", &format!("T{n}"))
            };
            let created = ops::create_firearm(&db.conn, &input, false)
                .unwrap_or_else(|e| panic!("type {type_id} class {class:?}: {e:?}"));
            assert_eq!(created.registration_class_id, class);
        }
    }
    // Short barrels with no classification save as they are.
    for (serial, type_id, barrel) in [("SB1", RIFLE, 1050), ("SB2", 3, 1400), ("SB3", RIFLE, 1050)]
    {
        let input = FirearmInput {
            firearm_type_id: type_id,
            barrel_length_hundredths: Some(barrel),
            ..firearm("Make", "Model", serial)
        };
        let created = ops::create_firearm(&db.conn, &input, false).unwrap();
        assert_eq!(created.registration_class_id, None);
        assert_eq!(created.barrel_length_hundredths, Some(barrel));
    }
}
