//! The record identifier on firearms (006 FR-019,
//! research.md §6): a random version 4 UUID, set once at creation, never
//! changed, and never sent over IPC. Real SQLCipher databases, no mocks.
//! The accessory half is 006 tasks.md T016 (006 FR-019, FR-022). The last
//! section is 009 T012: the same identifier on insurance policies, photos and
//! documents (009 FR-001 to FR-005).

use crate::support;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::firearms::{DisposeInput, HistoryChoice, ReverseDispositionInput, ops};
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::commands::insurance::ops as insurance;
use hoplodex_lib::commands::photos::ops as photo_ops;
use hoplodex_lib::models::accessory::{Accessory, AccessoryInput};
use hoplodex_lib::models::database::CloseReason;
use hoplodex_lib::models::document_attachment::DocumentSummary;
use hoplodex_lib::models::firearm::{DispositionType, Firearm};
use hoplodex_lib::models::photo::PhotoSummary;
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::backups;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::preview::availability::PdfAvailability;
use hoplodex_lib::services::record_id;
use hoplodex_lib::services::sandbox::Reach;
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use hoplodex_lib::session::scoped::Target;
use hoplodex_lib::session::{Session, lifecycle};
use rusqlite::Connection;
use support::{ManualClock, TestDb, passphrase, peek, sample_png_bytes, test_session_at};
use tempfile::TempDir;

/// The shape 006 FR-019 gives, checked without a regex dependency.
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
    ops::create_firearm(&db.conn, &support::firearm("Glock", "19", serial), false, None).unwrap()
}

fn dispose_input() -> DisposeInput {
    DisposeInput {
        disposition_type: DispositionType::Sold,
        recipient: "Jane Doe".into(),
        date: "2025-06-15".into(),
        price: 40000,
        with_mounted: Vec::new(),
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

// --- On accessories (006 T016) -------------------------------------------------------------

fn accessory_uid_in_database(conn: &Connection, id: i64) -> String {
    conn.query_row("SELECT uid FROM accessories WHERE id = ?1", [id], |r| r.get(0)).unwrap()
}

fn plain_accessory() -> AccessoryInput {
    serde_json::from_value(serde_json::json!({
        "accessoryKindId": 1,
        "make": "Leupold",
        "model": "VX-5HD",
        "status": "active",
    }))
    .unwrap()
}

fn create_accessory(db: &TestDb) -> Accessory {
    accessory_ops::create_accessory(&db.conn, &plain_accessory(), None).unwrap()
}

/// Inserts an accessory row with `uid` by raw SQL.
fn insert_accessory_row(conn: &Connection, uid: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO accessories (uid, accessory_kind_id, make, model, created_at, updated_at)
         VALUES (?1, 1, 'Leupold', 'VX-5HD', 'now', 'now')",
        [uid],
    )
}

/// Inserts a firearm row with `uid` by raw SQL.
fn insert_firearm_row(conn: &Connection, uid: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO firearms (uid, make, model, caliber, firearm_type_id, no_serial_attested,
                               created_at, updated_at)
         VALUES (?1, 'Glock', '19', '9mm', 1, 1, 'now', 'now')",
        [uid],
    )
}

#[test]
fn a_created_accessory_has_a_stored_identifier_the_column_check_accepts() {
    let db = TestDb::new();
    let created = create_accessory(&db);
    assert!(is_v4_uuid(&created.uid), "{}", created.uid);
    assert_eq!(accessory_uid_in_database(&db.conn, created.id), created.uid);
}

#[test]
fn two_accessories_get_distinct_identifiers_and_none_is_a_firearms() {
    let db = TestDb::new();
    let firearm = create(&db, "A1");
    let (first, second) = (create_accessory(&db), create_accessory(&db));
    assert_ne!(first.uid, second.uid);
    assert_ne!(first.uid, firearm.uid);
    assert_ne!(second.uid, firearm.uid);
}

#[test]
fn an_accessory_created_with_a_reused_id_gets_a_new_identifier() {
    let db = TestDb::new();
    let first = create_accessory(&db);
    let doomed = create_accessory(&db);
    accessory_ops::delete_accessory(&db.conn, doomed.id, true).unwrap();

    let replacement = create_accessory(&db);

    assert_eq!(replacement.id, doomed.id, "the highest id is reused");
    assert_ne!(replacement.uid, doomed.uid);
    assert_ne!(replacement.uid, first.uid);
}

#[test]
fn editing_disposing_reversing_and_scheduling_an_accessory_leave_the_identifier_unchanged() {
    let db = TestDb::new();
    let created = create_accessory(&db);
    let uid = created.uid.clone();

    let mut edited = plain_accessory();
    edited.notes = Some("Edited".into());
    let after_edit = accessory_ops::update_accessory(&db.conn, created.id, &edited).unwrap();
    assert_eq!(after_edit.uid, uid, "update_accessory");

    let dispose = || {
        accessory_ops::dispose_accessory(
            &db.conn,
            created.id,
            &serde_json::from_value(serde_json::json!({
                "dispositionType": "sold",
                "recipient": "Jane Doe",
                "date": "2025-06-15",
                "price": 40000,
            }))
            .unwrap(),
        )
        .unwrap()
    };
    let reverse = |history: &str| {
        accessory_ops::reverse_accessory_disposition(
            &db.conn,
            created.id,
            &serde_json::from_value(serde_json::json!({ "history": history })).unwrap(),
        )
        .unwrap()
    };

    assert_eq!(dispose().uid, uid, "dispose_accessory");
    for history in ["keep", "discard"] {
        assert_eq!(reverse(history).uid, uid, "reverse_accessory_disposition {history}");
        dispose();
    }
    reverse("discard");

    let policy = insurance::create_policy(
        &db.conn,
        &support::policy("Schedule", "2025-01-01", "2030-01-01", None),
    )
    .unwrap();
    let scheduled =
        insurance::assign_accessory_coverage(&db.conn, created.id, Some(policy.id), Some(1000))
            .unwrap();
    assert_eq!(scheduled.uid, uid, "assign_accessory_coverage");
    assert_eq!(accessory_uid_in_database(&db.conn, created.id), uid);
}

#[test]
fn the_database_refuses_to_change_an_accessorys_identifier() {
    let db = TestDb::new();
    let created = create_accessory(&db);

    let err = db
        .conn
        .execute(
            "UPDATE accessories SET uid = ?1 WHERE id = ?2",
            rusqlite::params![VALID, created.id],
        )
        .expect_err("the identifier is fixed");

    assert!(err.to_string().contains("a record identifier never changes"), "{err}");
    assert_eq!(accessory_uid_in_database(&db.conn, created.id), created.uid);
}

#[test]
fn a_raw_accessory_insert_with_a_malformed_identifier_fails_the_check() {
    let db = TestDb::new();
    for bad in [
        "not-a-uuid",
        "3f2a9c1e5b7d4e8a9c0f1a2b3c4d5e6f",
        "3f2a9c1e-5b7d-1e8a-9c0f-1a2b3c4d5e6f",
        "3f2a9c1e-5b7d-4e8a-7c0f-1a2b3c4d5e6f",
        "3F2A9C1E-5B7D-4E8A-9C0F-1A2B3C4D5E6F",
        "3f2a9c1e-5b7d-4e8a-9c0f-1a2b3c4d5e6g",
    ] {
        assert!(insert_accessory_row(&db.conn, bad).is_err(), "{bad} should fail the CHECK");
    }
}

#[test]
fn a_raw_accessory_insert_with_a_firearms_identifier_aborts() {
    let db = TestDb::new();
    let firearm = create(&db, "A1");

    let err = insert_accessory_row(&db.conn, &firearm.uid)
        .expect_err("one identifier names one record (accessories_uid_distinct)");

    assert!(err.to_string().contains("record identifier already used by a firearm"), "{err}");
    let accessories: i64 =
        db.conn.query_row("SELECT count(*) FROM accessories", [], |r| r.get(0)).unwrap();
    assert_eq!(accessories, 0);
}

#[test]
fn a_raw_firearm_insert_with_an_accessorys_identifier_aborts() {
    let db = TestDb::new();
    let accessory = create_accessory(&db);

    let err = insert_firearm_row(&db.conn, &accessory.uid)
        .expect_err("one identifier names one record (firearms_uid_distinct)");

    assert!(err.to_string().contains("record identifier already used by an accessory"), "{err}");
    let firearms: i64 =
        db.conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0)).unwrap();
    assert_eq!(firearms, 0);
}

#[test]
fn a_raw_insert_of_a_fresh_identifier_into_either_table_still_works() {
    let db = TestDb::new();
    assert_eq!(insert_accessory_row(&db.conn, &support::uid()).unwrap(), 1);
    assert_eq!(insert_firearm_row(&db.conn, &support::uid()).unwrap(), 1);
}

// --- On policies, photos and documents (009 T012) ---------------------------------------------

const PDF_BYTES: &[u8] = b"%PDF-1.4\n% a tiny document\n%%EOF\n";
const START: &str = "2026-09-25T14:30:05+02:00";
const NEW_PASSPHRASE: &str = "a much longer passphrase of several words";

/// The three tables 009 FR-001 gives an identifier.
const NEW_KINDS: [&str; 3] = ["insurance_policies", "photos", "document_attachments"];

/// What is not a valid identifier, for a raw insert: uppercase, a wrong version
/// digit, a wrong variant digit, 35 characters (009 US1-1, 009 FR-001).
const MALFORMED: [&str; 4] = [
    "3F2A9C1E-5B7D-4E8A-9C0F-1A2B3C4D5E6F",
    "3f2a9c1e-5b7d-1e8a-9c0f-1a2b3c4d5e6f",
    "3f2a9c1e-5b7d-4e8a-7c0f-1a2b3c4d5e6f",
    "3f2a9c1e-5b7d-4e8a-9c0f-1a2b3c4d5e6",
];

/// `(table, id, uid)` of every policy, photo and document, by table then id.
fn identifiers(conn: &Connection) -> Vec<(String, i64, String)> {
    let mut all = Vec::new();
    for table in NEW_KINDS {
        let mut stmt = conn.prepare(&format!("SELECT id, uid FROM {table} ORDER BY id")).unwrap();
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)));
        for row in rows.unwrap() {
            let (id, uid) = row.unwrap();
            all.push((table.to_owned(), id, uid));
        }
    }
    all
}

fn add_photo(conn: &Connection, owner: RecordRef) -> PhotoSummary {
    photo_ops::add_photo(conn, owner, &sample_png_bytes(), "front.png", "image/png").unwrap().into()
}

fn add_document(
    conn: &Connection,
    owner: RecordRef,
) -> hoplodex_lib::models::document_attachment::DocumentAttachment {
    document_ops::add_document(conn, owner, PDF_BYTES, "bill.pdf").unwrap()
}

/// A raw insert into `table`, owned by firearm 1 where it has an owner, with
/// `uid` left out when it is `None`.
fn insert_raw(conn: &Connection, table: &str, uid: Option<&str>) -> rusqlite::Result<usize> {
    let (columns, values) = match table {
        "insurance_policies" => (
            "name, policy_number, insurance_company, effective_start_date, effective_end_date, \
             created_at, updated_at",
            "'P', 'P-1', 'Acme', '2026-01-01', '2027-01-01', 'now', 'now'",
        ),
        "photos" => (
            "firearm_id, original_bytes, original_filename, mime_type, thumbnail_bytes, \
             sort_order, created_at",
            "1, x'00', 'a.png', 'image/png', x'00', 0, 'now'",
        ),
        _ => (
            "firearm_id, file_bytes, original_filename, mime_type, created_at",
            "1, x'00', 'a.pdf', 'application/pdf', 'now'",
        ),
    };
    match uid {
        Some(uid) => conn
            .execute(&format!("INSERT INTO {table} (uid, {columns}) VALUES (?1, {values})"), [uid]),
        None => conn.execute(&format!("INSERT INTO {table} ({columns}) VALUES ({values})"), []),
    }
}

/// A database holding a firearm (id 1) so a raw photo or document insert has an
/// owner.
fn db_with_owner() -> (TestDb, Firearm) {
    let db = TestDb::new();
    let firearm = create(&db, "A1");
    (db, firearm)
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0)).unwrap()
}

// 009 US1-1, 009 FR-001, 009 SC-001
#[test]
fn two_policies_with_the_same_insurer_and_dates_get_distinct_identifiers() {
    let db = TestDb::new();
    let first = insurance::create_policy(
        &db.conn,
        &support::policy("Home", "2025-01-01", "2030-01-01", None),
    )
    .unwrap();
    let second = insurance::create_policy(
        &db.conn,
        &support::policy("Safe", "2025-01-01", "2030-01-01", None),
    )
    .unwrap();

    let stored: Vec<String> = identifiers(&db.conn).into_iter().map(|(_, _, uid)| uid).collect();
    assert_eq!(stored.len(), 2);
    assert_ne!(stored[0], stored[1]);
    assert!(stored.iter().all(|uid| is_v4_uuid(uid) && record_id::parse(uid).is_some()));
    assert_eq!(first.name, "Home");
    assert_eq!(second.name, "Safe");
}

// 009 US1-1, 009 FR-001, 009 SC-001
#[test]
fn every_photo_and_document_made_through_ops_has_a_stored_identifier() {
    let (db, firearm) = db_with_owner();
    let accessory = create_accessory(&db);
    for owner in [RecordRef::Firearm(firearm.id), RecordRef::Accessory(accessory.id)] {
        add_photo(&db.conn, owner);
        add_photo(&db.conn, owner);
        add_document(&db.conn, owner);
        add_document(&db.conn, owner);
    }

    let stored = identifiers(&db.conn);
    let all: HashSet<&String> = stored.iter().map(|(_, _, uid)| uid).collect();
    assert_eq!(stored.len(), 8);
    assert_eq!(all.len(), 8, "no two records share one");
    for (table, id, uid) in &stored {
        assert_eq!(record_id::parse(uid).as_deref(), Some(uid.as_str()), "{table} {id}");
    }
}

// 009 US1-1, 009 FR-001
#[test]
fn the_database_refuses_a_missing_malformed_or_used_identifier_on_each_new_kind() {
    for table in NEW_KINDS {
        let (db, _) = db_with_owner();

        assert!(insert_raw(&db.conn, table, None).is_err(), "{table}: a missing uid");
        for bad in MALFORMED {
            assert!(insert_raw(&db.conn, table, Some(bad)).is_err(), "{table}: {bad}");
        }
        assert_eq!(count(&db.conn, table), 0, "{table}: nothing was stored");

        assert_eq!(insert_raw(&db.conn, table, Some(VALID)).unwrap(), 1, "{table}");
        assert!(insert_raw(&db.conn, table, Some(VALID)).is_err(), "{table}: a used uid");
        assert_eq!(count(&db.conn, table), 1, "{table}");
    }
}

// 009 US1-2, 009 FR-002
#[test]
fn the_database_refuses_to_change_an_identifier_on_each_new_kind() {
    for table in NEW_KINDS {
        let (db, _) = db_with_owner();
        let fresh = support::uid();
        insert_raw(&db.conn, table, Some(&fresh)).unwrap();

        let err = db
            .conn
            .execute(&format!("UPDATE {table} SET uid = ?1"), [VALID])
            .expect_err("the identifier is fixed");

        assert!(err.to_string().contains("a record identifier never changes"), "{table}: {err}");
        let kept: String =
            db.conn.query_row(&format!("SELECT uid FROM {table}"), [], |r| r.get(0)).unwrap();
        assert_eq!(kept, fresh, "{table}");
    }
}

// 009 US1-2, 009 FR-002
#[test]
fn editing_a_policy_a_photo_a_document_and_their_owners_leaves_every_identifier_unchanged() {
    let db = TestDb::new();
    let firearm = create(&db, "A1");
    let accessory = create_accessory(&db);
    let policy = insurance::create_policy(
        &db.conn,
        &support::policy("Home", "2025-01-01", "2030-01-01", None),
    )
    .unwrap();
    for owner in [RecordRef::Firearm(firearm.id), RecordRef::Accessory(accessory.id)] {
        add_photo(&db.conn, owner);
        add_photo(&db.conn, owner);
        add_document(&db.conn, owner);
    }
    let before = identifiers(&db.conn);
    assert_eq!(before.len(), 7);

    let mut edited = support::policy("Home", "2025-01-01", "2030-01-01", Some(5000));
    edited.notes = Some("Edited".into());
    insurance::update_policy(&db.conn, policy.id, &edited).unwrap();
    // A photo's caption is its file name, a document's details its name and
    // type, and a reorder is `sort_order`; none has an op, so by SQL, as the
    // statements they would run.
    let photos = photo_ops::list_photos(&db.conn, RecordRef::Firearm(firearm.id)).unwrap();
    photo_ops::set_thumbnail_photo(&db.conn, RecordRef::Firearm(firearm.id), photos[1].id).unwrap();
    db.conn
        .execute_batch(
            "UPDATE photos SET original_filename = 'renamed.png', sort_order = 9 - sort_order;
             UPDATE document_attachments SET original_filename = 'renamed.pdf', created_at = 'later';",
        )
        .unwrap();
    let mut changed = support::firearm("Glock", "19", "A1");
    changed.notes = Some("Edited".into());
    ops::update_firearm(&db.conn, firearm.id, &changed, false).unwrap();
    let mut changed_accessory = plain_accessory();
    changed_accessory.notes = Some("Edited".into());
    accessory_ops::update_accessory(&db.conn, accessory.id, &changed_accessory).unwrap();
    ops::dispose_firearm(&db.conn, firearm.id, &dispose_input()).unwrap();
    accessory_ops::dispose_accessory(
        &db.conn,
        accessory.id,
        &serde_json::from_value(serde_json::json!({
            "dispositionType": "sold",
            "recipient": "Jane Doe",
            "date": "2025-06-15",
            "price": 40000,
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(identifiers(&db.conn), before, "disposed of");

    ops::reverse_disposition(
        &db.conn,
        firearm.id,
        &ReverseDispositionInput {
            history: HistoryChoice::Keep,
            nickname: None,
            confirmed_warnings: false,
        },
    )
    .unwrap();
    accessory_ops::reverse_accessory_disposition(
        &db.conn,
        accessory.id,
        &serde_json::from_value(serde_json::json!({ "history": "keep" })).unwrap(),
    )
    .unwrap();

    assert_eq!(identifiers(&db.conn), before, "reversed");
}

/// "Mine", with its backups folder, a manual clock and machine settings, all in
/// temp directories (the shape `backup_test` and `passphrase_change_test` use).
struct World {
    dir: TempDir,
    _config: TempDir,
    scratch: TempDir,
    machine: MachineSettings,
    session: Session,
    clock: Arc<ManualClock>,
}

impl World {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let clock = ManualClock::at(START);
        let (session, _events) =
            test_session_at(&config.path().join("opened-documents"), clock.clone());
        let world = Self {
            dir: TempDir::new().unwrap(),
            scratch: TempDir::new().unwrap(),
            machine: MachineSettings::load(config.path()).unwrap(),
            _config: config,
            session,
            clock,
        };
        lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
        world
            .session
            .write(world.session.current_id(), |conn| {
                let firearm = ops::create_firearm(
                    conn,
                    &support::firearm("Colt", "Python", "A1"),
                    false,
                    None,
                )?;
                insurance::create_policy(
                    conn,
                    &support::policy("Home", "2025-01-01", "2030-01-01", None),
                )?;
                add_photo(conn, RecordRef::Firearm(firearm.id));
                add_document(conn, RecordRef::Firearm(firearm.id));
                Ok(())
            })
            .unwrap();
        world
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("Mine.hoplodex")
    }

    fn identifiers(&self) -> Vec<(String, i64, String)> {
        self.session.read(self.session.current_id(), |conn| Ok(identifiers(conn))).unwrap()
    }

    fn database_id(&self) -> String {
        self.session
            .inspect(self.session.current_id(), |open| Ok(open.database_id.clone()))
            .unwrap()
    }
}

// 009 US1-3, 009 FR-002, 009 SC-001
#[test]
fn a_backup_a_restore_and_a_passphrase_change_keep_every_identifier() {
    let world = World::new();
    let before = world.identifiers();
    assert_eq!(before.len(), 3);

    // Close makes the backup; its rows are the same.
    lifecycle::close_normal(
        &world.session,
        world.session.current_id(),
        &world.machine,
        CloseReason::Closed,
    )
    .unwrap();
    let folder = world.dir.path().join("HoploDex backups");
    let backup = backups::list(&folder, &{
        lifecycle::open(&world.session, &world.machine, &world.path(), &passphrase(), false)
            .unwrap();
        world.database_id()
    })
    .unwrap()
    .remove(0);
    assert_eq!(identifiers(&peek(std::path::Path::new(&backup.path))), before, "the backup");

    // Change the collection, restore the backup: the same rows come back.
    world
        .session
        .write(world.session.current_id(), |conn| {
            add_photo(conn, RecordRef::Firearm(1));
            Ok(())
        })
        .unwrap();
    assert_eq!(world.identifiers().len(), 4);
    world.clock.advance(chrono::Duration::hours(1));
    backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &backup.path,
        &passphrase(),
        Target::Open(world.session.current_id()),
        false,
        &Reach::Unconfined,
    )
    .unwrap();
    assert_eq!(world.identifiers(), before, "restored");

    backups_ops::change_passphrase(
        &world.session,
        world.session.current_id(),
        &world.machine,
        world.scratch.path(),
        &passphrase(),
        &Passphrase::from_input(NEW_PASSPHRASE.to_owned()),
    )
    .unwrap();
    assert_eq!(world.identifiers(), before, "after a passphrase change");
}

// 009 US1-4, 009 SC-002
#[test]
fn a_new_record_after_deleting_the_newest_gets_a_new_identifier() {
    let (db, firearm) = db_with_owner();
    let owner = RecordRef::Firearm(firearm.id);
    let policy = |name: &str| {
        insurance::create_policy(&db.conn, &support::policy(name, "2025-01-01", "2030-01-01", None))
            .unwrap()
    };
    policy("First");
    let doomed_policy = policy("Second");
    add_photo(&db.conn, owner);
    let doomed_photo = add_photo(&db.conn, owner);
    add_document(&db.conn, owner);
    let doomed_document = add_document(&db.conn, owner);
    let before = identifiers(&db.conn);
    let uid_of_row = |table: &str, id: i64| {
        before.iter().find(|(t, i, _)| t == table && *i == id).unwrap().2.clone()
    };
    let doomed = [
        ("insurance_policies", doomed_policy.id),
        ("photos", doomed_photo.id),
        ("document_attachments", doomed_document.id),
    ];

    insurance::delete_policy(&db.conn, doomed_policy.id, true, None).unwrap();
    photo_ops::delete_photo(&db.conn, doomed_photo.id, true).unwrap();
    document_ops::delete_document(&db.conn, doomed_document.id, true).unwrap();
    policy("Third");
    add_photo(&db.conn, owner);
    add_document(&db.conn, owner);

    let after = identifiers(&db.conn);
    for (table, id) in doomed {
        let reused = after.iter().find(|(t, i, _)| t == table && *i == id).unwrap();
        assert_ne!(reused.2, uid_of_row(table, id), "{table}: the id repeats, the uid does not");
        assert!(before.iter().all(|(_, _, uid)| *uid != reused.2), "{table}: matches no earlier");
    }
}

// 009 FR-005
#[test]
fn a_photo_or_document_may_share_an_identifier_with_a_firearm() {
    for table in ["photos", "document_attachments"] {
        let (db, firearm) = db_with_owner();
        insert_raw(&db.conn, table, Some(&firearm.uid))
            .unwrap_or_else(|e| panic!("{table}: no cross-kind trigger: {e}"));
        assert_eq!(count(&db.conn, table), 1, "{table}");
    }
}

// 009 FR-004
#[test]
fn history_and_the_built_in_lists_have_no_identifier_column() {
    let db = TestDb::new();
    for table in [
        "disposition_history",
        "firearm_types",
        "action_types",
        "registration_classes",
        "accessory_kinds",
    ] {
        let mut stmt = db.conn.prepare(&format!("PRAGMA table_info({table})")).unwrap();
        let names: Vec<String> =
            stmt.query_map([], |r| r.get(1)).unwrap().collect::<Result<_, _>>().unwrap();
        assert!(!names.is_empty(), "{table} exists");
        assert!(!names.iter().any(|n| n == "uid"), "{table} has a uid column");
    }
}

// 009 FR-003
#[test]
fn no_identifier_of_a_policy_photo_or_document_is_serialized_or_exported() {
    let (db, firearm) = db_with_owner();
    let owner = RecordRef::Firearm(firearm.id);
    let policy = insurance::create_policy(
        &db.conn,
        &support::policy("Home", "2025-01-01", "2030-01-01", None),
    )
    .unwrap();
    let photo = add_photo(&db.conn, owner);
    let document = add_document(&db.conn, owner);
    let pdf = PdfAvailability::Available;

    let mut returned = vec![
        ("create_policy", serde_json::to_value(&policy).unwrap()),
        (
            "get_policy",
            serde_json::to_value(insurance::get_policy(&db.conn, policy.id).unwrap()).unwrap(),
        ),
        (
            "list_policies",
            serde_json::to_value(insurance::list_policies(&db.conn).unwrap()).unwrap(),
        ),
        (
            "list_policy_views",
            serde_json::to_value(insurance::list_policy_views(&db.conn).unwrap()).unwrap(),
        ),
        ("add_photo", serde_json::to_value(&photo).unwrap()),
        (
            "get_photo",
            serde_json::to_value(PhotoSummary::from(
                photo_ops::get_photo(&db.conn, photo.id).unwrap(),
            ))
            .unwrap(),
        ),
        (
            "list_photos",
            serde_json::to_value(
                photo_ops::list_photos(&db.conn, owner)
                    .unwrap()
                    .into_iter()
                    .map(PhotoSummary::from)
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        ),
        (
            "add_document",
            serde_json::to_value(DocumentSummary::new(document.clone(), &pdf)).unwrap(),
        ),
        (
            "list_documents",
            serde_json::to_value(
                document_ops::list_documents(&db.conn, owner)
                    .unwrap()
                    .into_iter()
                    .map(|d| DocumentSummary::new(d, &pdf))
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        ),
    ];
    let dest = TempDir::new().unwrap();
    let records = import_export_ops::export_records(&db.conn, "all", None).unwrap();
    let exported = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "export",
        SpreadsheetFormat::Csv,
        &records,
        &mut |_, _| {},
        &Reach::Unconfined,
    )
    .unwrap();
    returned.push(("ExportResult", serde_json::to_value(&exported).unwrap()));

    let uids: Vec<String> = identifiers(&db.conn).into_iter().map(|(_, _, uid)| uid).collect();
    assert_eq!(uids.len(), 3);
    for (name, value) in &returned {
        let text = value.to_string();
        assert!(!text.contains("\"uid\""), "{name} has a uid key");
        for uid in &uids {
            assert!(!text.contains(uid.as_str()), "{name} carries an identifier");
        }
    }
    // Every file the export wrote: the spreadsheet and the photos.
    let mut files = vec![exported.spreadsheet_path.clone()];
    files.extend(
        std::fs::read_dir(&exported.photos_folder_path).unwrap().map(|e| e.unwrap().path()),
    );
    for file in files {
        let bytes = std::fs::read(&file).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for uid in &uids {
            assert!(!text.contains(uid.as_str()), "{} carries an identifier", file.display());
        }
    }
}
