//! Integration tests for User Story 1's create/edit/dispose/delete
//! lifecycle (spec.md Acceptance Scenarios 1-5), run against a real
//! temporary SQLCipher database — no mocks, per the constitution.

mod support;

use hoplodex_lib::commands::firearms::{ops, DisposeFirearmInput};
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput, FirearmStatus};
use support::TestDb;

fn sample_input() -> FirearmInput {
    FirearmInput {
        make: "Glock".into(),
        model: "19".into(),
        serial_number: Some("ABC123".into()),
        no_serial_attested: false,
        caliber: "9mm".into(),
        firearm_type_id: 1,
        notes: None,
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
        scheduled_coverage_amount: None,
    }
}

#[test]
fn scenario_1_create_firearm_persists_all_core_fields() {
    let db = TestDb::new();

    let created = ops::create_firearm(&db.conn, &sample_input()).expect("create should succeed");

    assert!(created.id > 0);
    assert_eq!(created.make, "Glock");
    assert_eq!(created.model, "19");
    assert_eq!(created.serial_number.as_deref(), Some("ABC123"));
    assert_eq!(created.caliber, "9mm");
    assert_eq!(created.firearm_type_id, 1);

    // Reopening (a fresh query, not the in-memory struct) confirms it
    // actually persisted rather than merely being echoed back.
    let reopened = ops::get_firearm(&db.conn, created.id).expect("firearm should be found");
    assert_eq!(reopened.make, "Glock");
    assert_eq!(reopened.serial_number.as_deref(), Some("ABC123"));
}

#[test]
fn scenario_2_editing_a_field_persists_on_reopen() {
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &sample_input()).unwrap();

    let mut edited = sample_input();
    edited.notes = Some("scratch on left side".into());
    let updated = ops::update_firearm(&db.conn, created.id, &edited).unwrap();
    assert_eq!(updated.notes.as_deref(), Some("scratch on left side"));

    let reopened = ops::get_firearm(&db.conn, created.id).unwrap();
    assert_eq!(reopened.notes.as_deref(), Some("scratch on left side"));
}

#[test]
fn scenario_3_acquisition_details_are_saved_and_shown() {
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &sample_input()).unwrap();

    let mut edited = sample_input();
    edited.acquisition_source = Some("Local gun shop".into());
    edited.acquisition_date = Some("2025-03-01".into());
    edited.acquisition_price = Some(45000);
    let updated = ops::update_firearm(&db.conn, created.id, &edited).unwrap();

    assert_eq!(updated.acquisition_source.as_deref(), Some("Local gun shop"));
    assert_eq!(updated.acquisition_date.as_deref(), Some("2025-03-01"));
    assert_eq!(updated.acquisition_price, Some(45000));
}

#[test]
fn scenario_4_disposing_flips_status_and_retains_history() {
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &sample_input()).unwrap();

    let disposed = ops::dispose_firearm(
        &db.conn,
        created.id,
        &DisposeFirearmInput {
            disposition_type: DispositionType::Sold,
            recipient: "Jane Doe".into(),
            date: "2025-06-15".into(),
            price: 40000,
        },
    )
    .expect("dispose should succeed");

    assert_eq!(disposed.status, FirearmStatus::Disposed);
    assert_eq!(disposed.disposition_type, Some(DispositionType::Sold));
    assert_eq!(disposed.disposition_recipient.as_deref(), Some("Jane Doe"));
    assert_eq!(disposed.disposition_date.as_deref(), Some("2025-06-15"));
    assert_eq!(disposed.disposition_price, Some(40000));
    // Full history retained: original identifying fields untouched.
    assert_eq!(disposed.make, "Glock");
    assert_eq!(disposed.serial_number.as_deref(), Some("ABC123"));
}

#[test]
fn scenario_5_delete_requires_confirmation_then_removes_the_record() {
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &sample_input()).unwrap();

    let unconfirmed = ops::delete_firearm(&db.conn, created.id, false);
    assert!(unconfirmed.is_err(), "delete without confirmation must be blocked");
    assert!(ops::get_firearm(&db.conn, created.id).is_ok(), "record must still exist");

    let confirmed =
        ops::delete_firearm(&db.conn, created.id, true).expect("confirmed delete should succeed");
    assert!(confirmed.deleted);
    assert!(ops::get_firearm(&db.conn, created.id).is_err(), "record must be gone");
}

#[test]
fn deleting_a_firearm_cascades_to_its_photos_and_documents() {
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &sample_input()).unwrap();

    db.conn
        .execute(
            "INSERT INTO photos (firearm_id, original_bytes, original_filename, mime_type, thumbnail_bytes, sort_order, created_at)
             VALUES (?1, x'00', 'a.jpg', 'image/jpeg', x'00', 0, datetime('now'))",
            [created.id],
        )
        .unwrap();
    db.conn
        .execute(
            "INSERT INTO document_attachments (firearm_id, file_bytes, original_filename, mime_type, created_at)
             VALUES (?1, x'00', 'a.pdf', 'application/pdf', datetime('now'))",
            [created.id],
        )
        .unwrap();

    ops::delete_firearm(&db.conn, created.id, true).unwrap();

    let photo_count: i64 = db
        .conn
        .query_row("SELECT count(*) FROM photos WHERE firearm_id = ?1", [created.id], |r| r.get(0))
        .unwrap();
    let doc_count: i64 = db
        .conn
        .query_row(
            "SELECT count(*) FROM document_attachments WHERE firearm_id = ?1",
            [created.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(photo_count, 0);
    assert_eq!(doc_count, 0);
}
