//! The record identifier on firearms (specs/006-accessory-links FR-019,
//! research.md §6): a random version 4 UUID, set once at creation, never
//! changed, and never sent over IPC. Real SQLCipher databases, no mocks.

mod support;

use std::collections::HashSet;

use hoplodex_lib::commands::firearms::{
    DisposeFirearmInput, HistoryChoice, ReverseDispositionInput, ops,
};
use hoplodex_lib::commands::insurance::ops as insurance;
use hoplodex_lib::models::firearm::{DispositionType, Firearm};
use hoplodex_lib::services::record_id;
use rusqlite::Connection;
use support::TestDb;

/// The shape FR-019 gives, checked without a regex dependency.
fn is_v4_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => *b == b'-',
            14 => *b == b'4',
            19 => matches!(*b, b'8' | b'9' | b'a' | b'b'),
            _ => matches!(*b, b'0'..=b'9' | b'a'..=b'f'),
        })
}

const VALID: &str = "3f2a9c1e-5b7d-4e8a-9c0f-1a2b3c4d5e6f";

// --- The service --------------------------------------------------------------

#[test]
fn a_generated_identifier_is_a_lowercase_hyphenated_version_4_uuid() {
    let generated = record_id::generate();
    assert!(is_v4_uuid(&generated), "{generated}");
}

#[test]
fn a_thousand_generated_identifiers_are_distinct() {
    let all: HashSet<String> = (0..1000).map(|_| record_id::generate()).collect();
    assert_eq!(all.len(), 1000);
    assert!(all.iter().all(|uid| is_v4_uuid(uid)));
}

#[test]
fn parse_trims_and_lowercases_a_valid_identifier() {
    assert_eq!(record_id::parse(VALID).as_deref(), Some(VALID));
    assert_eq!(record_id::parse(&format!("  {VALID}\t")).as_deref(), Some(VALID));
    assert_eq!(record_id::parse(&VALID.to_uppercase()).as_deref(), Some(VALID));
    assert_eq!(record_id::parse(&format!(" {} ", VALID.to_uppercase())).as_deref(), Some(VALID));
}

#[test]
fn parse_rejects_what_is_not_a_version_4_identifier() {
    assert_eq!(record_id::parse(""), None);
    assert_eq!(record_id::parse("   "), None);
    // Version 1.
    assert_eq!(record_id::parse("3f2a9c1e-5b7d-1e8a-9c0f-1a2b3c4d5e6f"), None);
    // 32 hex digits, no hyphens.
    assert_eq!(record_id::parse("3f2a9c1e5b7d4e8a9c0f1a2b3c4d5e6f"), None);
    // A variant digit outside 8 9 a b.
    assert_eq!(record_id::parse("3f2a9c1e-5b7d-4e8a-7c0f-1a2b3c4d5e6f"), None);
    assert_eq!(record_id::parse("3f2a9c1e-5b7d-4e8a-cc0f-1a2b3c4d5e6f"), None);
    // A non-hex letter.
    assert_eq!(record_id::parse("3f2a9c1e-5b7d-4e8a-9c0f-1a2b3c4d5e6g"), None);
    // Too long or too short.
    assert_eq!(record_id::parse(&format!("{VALID}0")), None);
    assert_eq!(record_id::parse(&VALID[1..]), None);
}

// --- On firearms -----------------------------------------------------------------

fn uid_in_database(conn: &Connection, id: i64) -> String {
    conn.query_row("SELECT uid FROM firearms WHERE id = ?1", [id], |r| r.get(0)).unwrap()
}

fn create(db: &TestDb, serial: &str) -> Firearm {
    ops::create_firearm(&db.conn, &support::firearm("Glock", "19", serial), false).unwrap()
}

fn dispose_input() -> DisposeFirearmInput {
    DisposeFirearmInput {
        disposition_type: DispositionType::Sold,
        recipient: "Jane Doe".into(),
        date: "2025-06-15".into(),
        price: 40000,
    }
}

#[test]
fn a_created_firearm_has_a_stored_identifier_the_column_check_accepts() {
    let db = TestDb::new();
    let created = create(&db, "A1");
    assert!(is_v4_uuid(&created.uid), "{}", created.uid);
    assert_eq!(uid_in_database(&db.conn, created.id), created.uid);
}

#[test]
fn two_firearms_get_distinct_identifiers() {
    let db = TestDb::new();
    let (first, second) = (create(&db, "A1"), create(&db, "B2"));
    assert_ne!(first.uid, second.uid);
}

#[test]
fn editing_disposing_reversing_and_scheduling_leave_the_identifier_unchanged() {
    let db = TestDb::new();
    let created = create(&db, "A1");
    let uid = created.uid.clone();

    let mut edited = support::firearm("Glock", "19", "A1");
    edited.notes = Some("Edited".into());
    let after_edit = ops::update_firearm(&db.conn, created.id, &edited, false).unwrap();
    assert_eq!(after_edit.uid, uid, "update_firearm");

    let disposed = ops::dispose_firearm(&db.conn, created.id, &dispose_input()).unwrap();
    assert_eq!(disposed.uid, uid, "dispose_firearm");

    for history in [HistoryChoice::Keep, HistoryChoice::Discard] {
        let reversed = ops::reverse_disposition(
            &db.conn,
            created.id,
            &ReverseDispositionInput { history, nickname: None, confirmed_warnings: false },
        )
        .unwrap();
        assert_eq!(reversed.uid, uid, "reverse_disposition {history:?}");
        ops::dispose_firearm(&db.conn, created.id, &dispose_input()).unwrap();
    }

    let policy = insurance::create_policy(
        &db.conn,
        &support::policy("Schedule", "2025-01-01", "2030-01-01", None),
    )
    .unwrap();
    ops::reverse_disposition(
        &db.conn,
        created.id,
        &ReverseDispositionInput {
            history: HistoryChoice::Discard,
            nickname: None,
            confirmed_warnings: false,
        },
    )
    .unwrap();
    let scheduled =
        insurance::assign_firearm_coverage(&db.conn, created.id, Some(policy.id), Some(1000))
            .unwrap();
    assert_eq!(scheduled.uid, uid, "assign_firearm_coverage");
    assert_eq!(uid_in_database(&db.conn, created.id), uid);
}

#[test]
fn a_firearm_created_with_a_reused_id_gets_a_new_identifier() {
    let db = TestDb::new();
    let first = create(&db, "A1");
    let doomed = create(&db, "B2");
    ops::delete_firearm(&db.conn, doomed.id, true).unwrap();

    let replacement = create(&db, "C3");

    assert_eq!(replacement.id, doomed.id, "the highest id is reused");
    assert_ne!(replacement.uid, doomed.uid);
    assert_ne!(replacement.uid, first.uid);
}

#[test]
fn the_database_refuses_to_change_an_identifier() {
    let db = TestDb::new();
    let created = create(&db, "A1");

    let err = db
        .conn
        .execute("UPDATE firearms SET uid = ?1 WHERE id = ?2", rusqlite::params![VALID, created.id])
        .expect_err("the identifier is fixed");

    assert!(err.to_string().contains("a record identifier never changes"), "{err}");
    assert_eq!(uid_in_database(&db.conn, created.id), created.uid);
}

#[test]
fn a_raw_insert_with_a_malformed_identifier_fails_the_check() {
    let db = TestDb::new();
    for bad in [
        "not-a-uuid",
        "3f2a9c1e5b7d4e8a9c0f1a2b3c4d5e6f",
        "3f2a9c1e-5b7d-1e8a-9c0f-1a2b3c4d5e6f",
        "3f2a9c1e-5b7d-4e8a-7c0f-1a2b3c4d5e6f",
        "3F2A9C1E-5B7D-4E8A-9C0F-1A2B3C4D5E6F",
        "3f2a9c1e-5b7d-4e8a-9c0f-1a2b3c4d5e6g",
    ] {
        let result = db.conn.execute(
            "INSERT INTO firearms (uid, make, model, caliber, firearm_type_id, no_serial_attested,
                                   created_at, updated_at)
             VALUES (?1, 'Glock', '19', '9mm', 1, 1, 'now', 'now')",
            [bad],
        );
        assert!(result.is_err(), "{bad} should fail the CHECK");
    }
}

#[test]
fn the_identifier_is_never_serialized_by_a_firearm_or_its_detail() {
    let db = TestDb::new();
    let created = create(&db, "A1");

    let firearm = serde_json::to_value(&created).unwrap();
    let detail =
        serde_json::to_value(ops::get_firearm_detail(&db.conn, created.id).unwrap()).unwrap();

    for (name, value) in [("Firearm", firearm), ("FirearmDetail", detail)] {
        assert!(value.get("uid").is_none(), "{name} has a uid key");
        assert!(!value.to_string().contains(&created.uid), "{name} carries the identifier");
    }
}
