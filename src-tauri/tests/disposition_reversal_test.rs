//! Integration tests for reversing a disposition (FR-033, spec.md US1
//! Acceptance Scenario 12), run against a real temporary SQLCipher
//! database.

mod support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::firearms::{
    DisposeFirearmInput, HistoryChoice, ReverseDispositionInput, ops,
};
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput, FirearmStatus};
use support::{TestDb, firearm};

fn dispose(db: &TestDb, id: i64, recipient: &str, date: &str) {
    ops::dispose_firearm(
        &db.conn,
        id,
        &DisposeFirearmInput {
            disposition_type: DispositionType::Sold,
            recipient: recipient.into(),
            date: date.into(),
            price: 40000,
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
    let created = ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false).unwrap();
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
    let created = ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false).unwrap();
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
    let created = ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false).unwrap();

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
    )
    .unwrap();
    dispose(&db, original.id, "Jane Doe", "2025-06-15");
    // The nickname was released; another firearm took it.
    ops::create_firearm(
        &db.conn,
        &FirearmInput { nickname: Some("old faithful".into()), ..firearm("Sig", "P226", "S1") },
        false,
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
    )
    .unwrap();
    dispose(&db, original.id, "Jane Doe", "2025-06-15");
    ops::create_firearm(
        &db.conn,
        &FirearmInput { nickname: Some("Old Faithful".into()), ..firearm("Sig", "P226", "S1") },
        false,
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
    let original = ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false).unwrap();
    dispose(&db, original.id, "Jane Doe", "2025-06-15");
    // Reacquired as a new record while the old one stayed disposed.
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "ABC123"), false).unwrap();

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
    let original = ops::create_firearm(&db.conn, &no_serial, false).unwrap();
    dispose(&db, original.id, "Jane Doe", "2025-06-15");
    ops::create_firearm(&db.conn, &firearm("Colt", "1911", "12345"), false).unwrap();

    let restored =
        ops::reverse_disposition(&db.conn, original.id, &reverse(HistoryChoice::Keep)).unwrap();

    assert_eq!(restored.status, FirearmStatus::Active);
}

#[test]
fn repeated_dispose_and_restore_accumulates_history_newest_first() {
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false).unwrap();

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
    let created = ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false).unwrap();
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
    )
    .unwrap();
    dispose(&db, original.id, "Jane Doe", "2025-06-15");
    // Reacquired with the same year, so nothing distinguishes them.
    ops::create_firearm(
        &db.conn,
        &FirearmInput { year_of_manufacture: Some(1943), ..firearm("Colt", "1873", "SAA-1") },
        false,
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
