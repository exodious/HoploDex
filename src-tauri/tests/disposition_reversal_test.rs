//! Integration tests for reversing a disposition (FR-033, spec.md US1
//! Acceptance Scenario 12), run against a real temporary SQLCipher
//! database.

mod support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::{DisposeInput, HistoryChoice, ReverseDispositionInput, ops};
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput, FirearmStatus};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use support::{TestDb, firearm};

fn dispose(db: &TestDb, id: i64, recipient: &str, date: &str) {
    ops::dispose_firearm(
        &db.conn,
        id,
        &DisposeInput {
            disposition_type: DispositionType::Sold,
            recipient: recipient.into(),
            date: date.into(),
            price: 40000,
            with_mounted: Vec::new(),
        },
    )
    .unwrap();
}

fn reverse(history: HistoryChoice) -> ReverseDispositionInput {
    ReverseDispositionInput { history, nickname: None, confirmed_warnings: false }
}

fn history_count(db: &TestDb, id: i64) -> i64 {
    db.conn
        .query_row("SELECT count(*) FROM disposition_history WHERE firearm_id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
}

fn message(err: &CommandError) -> String {
    format!("{} {:?}", err.message, err.field_errors)
}

#[test]
fn keeping_the_history_restores_the_firearm_and_retains_the_disposition() {
    let db = TestDb::new();
    let created =
        ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false, None).unwrap();
    dispose(&db, created.id, "Jane Doe", "2025-06-15");

    let restored = ops::reverse_disposition(&db.conn, created.id, &reverse(HistoryChoice::Keep))
        .expect("reversal should succeed");

    assert_eq!(restored.status, FirearmStatus::Active);
    assert_eq!(restored.disposition_type, None);
    assert_eq!(restored.disposition_recipient, None);
    assert_eq!(restored.disposition_date, None);
    assert_eq!(restored.disposition_price, None);

    let detail = ops::get_firearm_detail(&db.conn, created.id).unwrap();
    assert_eq!(detail.disposition_history.len(), 1);
    let kept = &detail.disposition_history[0];
    assert_eq!(kept.disposition_type, DispositionType::Sold);
    assert_eq!(kept.disposition_recipient, "Jane Doe");
    assert_eq!(kept.disposition_date, "2025-06-15");
    assert_eq!(kept.disposition_price, Some(40000));
    assert!(!kept.reversed_at.is_empty());
}

#[test]
fn discarding_restores_the_firearm_and_stores_nothing() {
    let db = TestDb::new();
    let created =
        ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false, None).unwrap();
    dispose(&db, created.id, "Jane Doe", "2025-06-15");

    let restored =
        ops::reverse_disposition(&db.conn, created.id, &reverse(HistoryChoice::Discard)).unwrap();

    assert_eq!(restored.status, FirearmStatus::Active);
    assert_eq!(restored.disposition_type, None);
    assert_eq!(history_count(&db, created.id), 0);
    assert!(ops::get_firearm_detail(&db.conn, created.id).unwrap().disposition_history.is_empty());
}

#[test]
fn only_a_disposed_firearm_can_be_reversed() {
    let db = TestDb::new();
    let created =
        ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false, None).unwrap();

    let err = ops::reverse_disposition(&db.conn, created.id, &reverse(HistoryChoice::Keep))
        .expect_err("an active firearm has no disposition to reverse");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(history_count(&db, created.id), 0);

    let missing = ops::reverse_disposition(&db.conn, 9999, &reverse(HistoryChoice::Keep))
        .expect_err("unknown id");
    assert_eq!(missing.code, "NOT_FOUND");
}

#[test]
fn a_reversal_that_clashes_on_nickname_is_blocked_naming_the_record_and_changes_nothing() {
    let db = TestDb::new();
    let original = ops::create_firearm(
        &db.conn,
        &FirearmInput { nickname: Some("Old Faithful".into()), ..firearm("Glock", "19", "A1") },
        false,
        None,
    )
    .unwrap();
    dispose(&db, original.id, "Jane Doe", "2025-06-15");
    // The nickname was released; another firearm took it.
    ops::create_firearm(
        &db.conn,
        &FirearmInput { nickname: Some("old faithful".into()), ..firearm("Sig", "P226", "S1") },
        false,
        None,
    )
    .unwrap();

    let err = ops::reverse_disposition(&db.conn, original.id, &reverse(HistoryChoice::Keep))
        .expect_err("the restored record would duplicate an active nickname");

    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(message(&err).contains("Sig") && message(&err).contains("P226"), "{}", message(&err));
    let unchanged = ops::get_firearm(&db.conn, original.id).unwrap();
    assert_eq!(unchanged.status, FirearmStatus::Disposed, "nothing was changed");
    assert_eq!(unchanged.disposition_recipient.as_deref(), Some("Jane Doe"));
    assert_eq!(history_count(&db, original.id), 0, "and no history was written");
}

#[test]
fn a_reversal_can_rename_to_resolve_a_nickname_clash_in_the_same_step() {
    let db = TestDb::new();
    let original = ops::create_firearm(
        &db.conn,
        &FirearmInput { nickname: Some("Old Faithful".into()), ..firearm("Glock", "19", "A1") },
        false,
        None,
    )
    .unwrap();
    dispose(&db, original.id, "Jane Doe", "2025-06-15");
    ops::create_firearm(
        &db.conn,
        &FirearmInput { nickname: Some("Old Faithful".into()), ..firearm("Sig", "P226", "S1") },
        false,
        None,
    )
    .unwrap();

    let restored = ops::reverse_disposition(
        &db.conn,
        original.id,
        &ReverseDispositionInput {
            history: HistoryChoice::Keep,
            nickname: Some("  Old Faithful II ".into()),
            confirmed_warnings: false,
        },
    )
    .unwrap();

    assert_eq!(restored.status, FirearmStatus::Active);
    assert_eq!(restored.nickname.as_deref(), Some("Old Faithful II"));
    assert_eq!(history_count(&db, original.id), 1);
}

#[test]
fn a_reversal_that_clashes_on_make_model_serial_is_blocked_and_changes_nothing() {
    let db = TestDb::new();
    let original =
        ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false, None).unwrap();
    dispose(&db, original.id, "Jane Doe", "2025-06-15");
    // Reacquired as a new record while the old one stayed disposed.
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false, None).unwrap();

    let err = ops::reverse_disposition(&db.conn, original.id, &reverse(HistoryChoice::Discard))
        .expect_err("two active Glock 19 ABC123 would break FR-032");

    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(message(&err).contains("ABC123"));
    assert_eq!(ops::get_firearm(&db.conn, original.id).unwrap().status, FirearmStatus::Disposed);
}

#[test]
fn a_firearm_with_no_serial_number_is_restored_without_being_compared() {
    let db = TestDb::new();
    let no_serial = FirearmInput {
        serial_number: None,
        no_serial_attested: true,
        ..firearm("Colt", "1911", "")
    };
    let original = ops::create_firearm(&db.conn, &no_serial, false, None).unwrap();
    dispose(&db, original.id, "Jane Doe", "2025-06-15");
    ops::create_firearm(&db.conn, &firearm("Colt", "1911", "12345"), false, None).unwrap();

    let restored =
        ops::reverse_disposition(&db.conn, original.id, &reverse(HistoryChoice::Keep)).unwrap();

    assert_eq!(restored.status, FirearmStatus::Active);
}

#[test]
fn repeated_dispose_and_restore_accumulates_history_newest_first() {
    let db = TestDb::new();
    let created =
        ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false, None).unwrap();

    dispose(&db, created.id, "First Buyer", "2024-01-10");
    ops::reverse_disposition(&db.conn, created.id, &reverse(HistoryChoice::Keep)).unwrap();
    dispose(&db, created.id, "Second Buyer", "2025-02-20");
    ops::reverse_disposition(&db.conn, created.id, &reverse(HistoryChoice::Keep)).unwrap();

    let history = ops::get_firearm_detail(&db.conn, created.id).unwrap().disposition_history;
    let recipients: Vec<_> = history.iter().map(|h| h.disposition_recipient.as_str()).collect();
    assert_eq!(recipients, vec!["Second Buyer", "First Buyer"]);
}

#[test]
fn retained_history_is_removed_with_the_firearm() {
    let db = TestDb::new();
    let created =
        ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false, None).unwrap();
    dispose(&db, created.id, "Jane Doe", "2025-06-15");
    ops::reverse_disposition(&db.conn, created.id, &reverse(HistoryChoice::Keep)).unwrap();
    assert_eq!(history_count(&db, created.id), 1);

    ops::delete_firearm(&db.conn, created.id, true).unwrap();

    assert_eq!(history_count(&db, created.id), 0, "deleting a firearm really deletes its history");
}

// specs/002-firearm-identification User Story 3: the year-of-manufacture
// identity exception and the FR-009 warning apply to a restored firearm too
// (FR-008, FR-009), inside the same reversal transaction.

#[test]
fn restoring_into_an_identity_clash_undistinguished_by_year_is_blocked_and_changes_nothing() {
    // US3-4b
    let db = TestDb::new();
    let original = ops::create_firearm(
        &db.conn,
        &FirearmInput { year_of_manufacture: Some(1943), ..firearm("Colt", "1873", "SAA-1") },
        false,
        None,
    )
    .unwrap();
    dispose(&db, original.id, "Jane Doe", "2025-06-15");
    // Reacquired with the same year, so nothing distinguishes them.
    ops::create_firearm(
        &db.conn,
        &FirearmInput { year_of_manufacture: Some(1943), ..firearm("Colt", "1873", "SAA-1") },
        false,
        None,
    )
    .unwrap();

    let err = ops::reverse_disposition(&db.conn, original.id, &reverse(HistoryChoice::Keep))
        .expect_err("undistinguished by year, the restore must be blocked (FR-008)");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(message(&err).to_lowercase().contains("year of manufacture"), "{}", message(&err));
    assert_eq!(ops::get_firearm(&db.conn, original.id).unwrap().status, FirearmStatus::Disposed);
    assert_eq!(history_count(&db, original.id), 0, "the kept-history row was rolled back too");
}

#[test]
fn restoring_into_an_original_marks_match_warns_and_only_proceeds_when_confirmed() {
    // US3-9
    let db = TestDb::new();
    let original = ops::create_firearm(
        &db.conn,
        &FirearmInput {
            origin: Some(hoplodex_lib::models::firearm::Origin::Imported),
            original_make: Some("FN".into()),
            original_model: Some("High Power".into()),
            original_serial_number: Some("FN-1".into()),
            registration_class_id: None,
            registration_form: None,
            registration_approved: None,
            registered_to: None,
            ..firearm("Ridgeline Arms", "Hi-Power", "RA-1")
        },
        false,
        None,
    )
    .unwrap();
    dispose(&db, original.id, "Jane Doe", "2025-06-15");
    ops::create_firearm(
        &db.conn,
        &FirearmInput {
            origin: Some(hoplodex_lib::models::firearm::Origin::Imported),
            original_make: Some("FN".into()),
            original_model: Some("High Power".into()),
            original_serial_number: Some("FN-1".into()),
            registration_class_id: None,
            registration_form: None,
            registration_approved: None,
            registered_to: None,
            ..firearm("Century Arms", "Clone", "CA-1")
        },
        false,
        None,
    )
    .unwrap();

    let err = ops::reverse_disposition(&db.conn, original.id, &reverse(HistoryChoice::Keep))
        .expect_err("the restored record's original marks match an active firearm's");
    assert_eq!(err.code, "ORIGINAL_MARKS_MATCH");
    assert_eq!(ops::get_firearm(&db.conn, original.id).unwrap().status, FirearmStatus::Disposed);
    assert_eq!(history_count(&db, original.id), 0, "nothing committed until confirmed");

    let confirmed = ops::reverse_disposition(
        &db.conn,
        original.id,
        &ReverseDispositionInput {
            history: HistoryChoice::Keep,
            nickname: None,
            confirmed_warnings: true,
        },
    )
    .expect("confirmed_warnings: true lets the restore proceed");
    assert_eq!(confirmed.status, FirearmStatus::Active);
    assert_eq!(history_count(&db, original.id), 1);
}

#[test]
fn the_history_choice_is_required_on_the_wire() {
    // The frontend asks the user; the backend never defaults it silently.
    let missing: Result<ReverseDispositionInput, _> = serde_json::from_str("{}");
    assert!(missing.is_err());
    let keep: ReverseDispositionInput = serde_json::from_str(r#"{"history":"keep"}"#).unwrap();
    assert!(matches!(keep.history, HistoryChoice::Keep));
    let discard: ReverseDispositionInput =
        serde_json::from_str(r#"{"history":"discard","nickname":null}"#).unwrap();
    assert!(matches!(discard.history, HistoryChoice::Discard));
}

/// specs/005-regulated-item-types FR-013: disposing of a registered firearm
/// and reversing it leaves the classification and details as they were.
#[test]
fn registration_is_unchanged_after_disposal_and_after_reversal() {
    let db = TestDb::new();
    let created = ops::create_firearm(
        &db.conn,
        &FirearmInput {
            registration_class_id: Some(1),
            registration_form: Some("Form 4".into()),
            registration_approved: Some("2026-02-10".into()),
            registered_to: Some("Smith Family Trust".into()),
            ..firearm("SilencerCo", "Omega", "REG1")
        },
        false,
        None,
    )
    .unwrap();
    let registration = |f: &hoplodex_lib::models::Firearm| {
        (
            f.registration_class_id,
            f.registration_form.clone(),
            f.registration_approved.clone(),
            f.registered_to.clone(),
        )
    };
    let before = registration(&created);
    dispose(&db, created.id, "Jane Doe", "2026-03-01");
    assert_eq!(registration(&ops::get_firearm(&db.conn, created.id).unwrap()), before);
    let restored =
        ops::reverse_disposition(&db.conn, created.id, &reverse(HistoryChoice::Keep)).unwrap();
    assert_eq!(registration(&restored), before);
}

// --- Accessories (specs/006-accessory-links FR-006) ----------------------------
//
// An accessory is disposed and restored as a firearm is, with its history kept
// or discarded, but a restore re-checks no nickname or identity: an accessory
// has neither (FR-004). Accessory inputs are built from the IPC shape
// (camelCase JSON), so these tests do not depend on how the structs are spelled.

fn parse<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("the JSON must fit the type")
}

fn accessory_json(serial: Option<&str>) -> Value {
    json!({
        "accessoryKindId": 1,
        "make": "Leupold",
        "model": "VX-5HD",
        "serialNumber": serial,
        "estimatedValue": 1000,
        "status": "active",
    })
}

fn create_accessory(db: &TestDb, serial: Option<&str>) -> i64 {
    accessory_ops::create_accessory(&db.conn, &parse(accessory_json(serial)), None).unwrap().id
}

fn dispose_accessory(db: &TestDb, id: i64, recipient: &str, date: &str) {
    accessory_ops::dispose_accessory(
        &db.conn,
        id,
        &parse(json!({
            "dispositionType": "sold",
            "recipient": recipient,
            "date": date,
            "price": 40000,
        })),
    )
    .unwrap();
}

fn reverse_accessory(db: &TestDb, id: i64, history: &str) -> Result<Value, CommandError> {
    accessory_ops::reverse_accessory_disposition(
        &db.conn,
        id,
        &parse(json!({ "history": history })),
    )
    .map(|restored| serde_json::to_value(restored).unwrap())
}

/// `get_accessory`'s output as it goes over IPC.
fn detail(db: &TestDb, id: i64) -> Value {
    serde_json::to_value(accessory_ops::get_accessory(&db.conn, id).unwrap()).unwrap()
}

fn accessory_history_count(db: &TestDb, id: i64) -> i64 {
    db.conn
        .query_row(
            "SELECT count(*) FROM disposition_history WHERE accessory_id = ?1 AND firearm_id IS NULL",
            [id],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn keeping_the_history_restores_the_accessory_and_retains_a_disposition_it_owns() {
    let db = TestDb::new();
    let id = create_accessory(&db, Some("SN-1"));
    dispose_accessory(&db, id, "Jane Doe", "2025-06-15");
    assert_eq!(detail(&db, id)["status"], json!("disposed"));

    let restored = reverse_accessory(&db, id, "keep").expect("reversal should succeed");

    assert_eq!(restored["status"], json!("active"));
    for field in ["dispositionType", "dispositionRecipient", "dispositionDate", "dispositionPrice"]
    {
        assert_eq!(restored[field], Value::Null, "{field} is cleared");
    }
    assert_eq!(restored["make"], json!("Leupold"), "everything else is as it was");
    assert_eq!(restored["serialNumber"], json!("SN-1"));
    assert_eq!(restored["estimatedValue"], json!(1000));

    let history = detail(&db, id)["dispositionHistory"].clone();
    assert_eq!(history.as_array().unwrap().len(), 1);
    let kept = &history[0];
    assert_eq!(kept["owner"], json!({ "kind": "accessory", "id": id }));
    assert_eq!(kept["dispositionType"], json!("sold"));
    assert_eq!(kept["dispositionRecipient"], json!("Jane Doe"));
    assert_eq!(kept["dispositionDate"], json!("2025-06-15"));
    assert_eq!(kept["dispositionPrice"], json!(40000));
    assert!(!kept["reversedAt"].as_str().unwrap().is_empty());
    assert!(kept.get("firearmId").is_none(), "an entry has an owner, not a firearm id");
    assert_eq!(accessory_history_count(&db, id), 1, "stored with the accessory column only");
}

#[test]
fn discarding_restores_the_accessory_and_stores_nothing() {
    let db = TestDb::new();
    let id = create_accessory(&db, None);
    dispose_accessory(&db, id, "Jane Doe", "2025-06-15");

    let restored = reverse_accessory(&db, id, "discard").unwrap();

    assert_eq!(restored["status"], json!("active"));
    assert_eq!(restored["dispositionType"], Value::Null);
    assert_eq!(accessory_history_count(&db, id), 0);
    assert!(detail(&db, id)["dispositionHistory"].as_array().unwrap().is_empty());
}

#[test]
fn an_accessory_disposed_and_restored_twice_keeps_both_entries_newest_first() {
    let db = TestDb::new();
    let id = create_accessory(&db, None);
    dispose_accessory(&db, id, "First buyer", "2024-01-01");
    reverse_accessory(&db, id, "keep").unwrap();
    dispose_accessory(&db, id, "Second buyer", "2025-01-01");
    reverse_accessory(&db, id, "keep").unwrap();

    let history = detail(&db, id)["dispositionHistory"].clone();

    let recipients: Vec<&str> = history
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["dispositionRecipient"].as_str().unwrap())
        .collect();
    assert_eq!(recipients, vec!["Second buyer", "First buyer"]);
}

#[test]
fn a_restored_accessorys_history_is_its_own_and_not_another_records() {
    let db = TestDb::new();
    let firearm =
        ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false, None).unwrap();
    let id = create_accessory(&db, None);
    assert_eq!(firearm.id, id, "both tables start at 1, which is the point of this test");
    dispose(&db, firearm.id, "Firearm buyer", "2025-06-15");
    ops::reverse_disposition(&db.conn, firearm.id, &reverse(HistoryChoice::Keep)).unwrap();
    dispose_accessory(&db, id, "Accessory buyer", "2025-07-01");
    reverse_accessory(&db, id, "keep").unwrap();

    let of_accessory = detail(&db, id)["dispositionHistory"].clone();
    let of_firearm = serde_json::to_value(
        ops::get_firearm_detail(&db.conn, firearm.id).unwrap().disposition_history,
    )
    .unwrap();

    assert_eq!(of_accessory.as_array().unwrap().len(), 1);
    assert_eq!(of_accessory[0]["dispositionRecipient"], json!("Accessory buyer"));
    assert_eq!(of_accessory[0]["owner"], json!({ "kind": "accessory", "id": id }));
    assert_eq!(of_firearm.as_array().unwrap().len(), 1);
    assert_eq!(of_firearm[0]["dispositionRecipient"], json!("Firearm buyer"));
    assert_eq!(of_firearm[0]["owner"], json!({ "kind": "firearm", "id": firearm.id }));
}

#[test]
fn restoring_an_accessory_re_checks_no_identity_even_when_an_identical_one_exists() {
    let db = TestDb::new();
    let original = create_accessory(&db, Some("SAME-SN"));
    dispose_accessory(&db, original, "Jane Doe", "2025-06-15");
    // Same kind, make, model and serial number as the disposed one (FR-004:
    // an accessory has no identity rule).
    let twin = create_accessory(&db, Some("SAME-SN"));

    let restored = reverse_accessory(&db, original, "keep")
        .expect("no nickname or identity check applies to an accessory");

    assert_eq!(restored["status"], json!("active"));
    assert_eq!(detail(&db, twin)["status"], json!("active"));
    assert_eq!(accessory_history_count(&db, original), 1);
}

#[test]
fn only_a_disposed_accessory_can_be_reversed() {
    let db = TestDb::new();
    let id = create_accessory(&db, None);

    let err = reverse_accessory(&db, id, "keep")
        .expect_err("an active accessory has no disposition to reverse");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(accessory_history_count(&db, id), 0);

    let missing = reverse_accessory(&db, 9999, "keep").expect_err("unknown id");
    assert_eq!(missing.code, "NOT_FOUND");
}

#[test]
fn a_firearms_retained_disposition_names_the_firearm_as_its_owner() {
    let db = TestDb::new();
    let created =
        ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false, None).unwrap();
    dispose(&db, created.id, "Jane Doe", "2025-06-15");
    ops::reverse_disposition(&db.conn, created.id, &reverse(HistoryChoice::Keep)).unwrap();

    let history = serde_json::to_value(
        ops::get_firearm_detail(&db.conn, created.id).unwrap().disposition_history,
    )
    .unwrap();

    assert_eq!(history[0]["owner"], json!({ "kind": "firearm", "id": created.id }));
    assert!(history[0].get("firearmId").is_none());
    let (firearm_id, accessory_id): (Option<i64>, Option<i64>) = db
        .conn
        .query_row("SELECT firearm_id, accessory_id FROM disposition_history", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((firearm_id, accessory_id), (Some(created.id), None));
}
