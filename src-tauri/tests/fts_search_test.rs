//! Confirms the FTS5 index (data-model.md's "Virtual table: firearms_fts")
//! actually matches both free-form notes and structured fields, per FR-013.

mod support;

use hoplodex_lib::commands::firearms::{ops, ListFirearmsInput};
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use support::TestDb;

fn base_input() -> FirearmInput {
    FirearmInput {
        make: "Colt".into(),
        model: "1911".into(),
        serial_number: Some("CO1911".into()),
        no_serial_attested: false,
        caliber: ".45 ACP".into(),
        firearm_type_id: 1,
        notes: Some("inherited from grandfather, minor pitting on barrel".into()),
        accessories: Some("leather holster; spare magazine".into()),
        barrel_length_hundredths: None,
        overall_length_hundredths: None,
        weight_tenths_oz: None,
        capacity: None,
        finish: None,
        condition: None,
        status: FirearmStatus::Active,
        estimated_value: None,
        acquisition_source: None,
        acquisition_date: None,
        acquisition_price: None,
        disposition_type: None,
        disposition_recipient: None,
        disposition_date: None,
        disposition_price: None,
        insurance_policy_id: None,
        nickname: None,
        scheduled_coverage_amount: None,
    }
}

fn search(conn: &rusqlite::Connection, query: &str) -> usize {
    let result = ops::list_firearms(
        conn,
        &ListFirearmsInput { query: Some(query.into()), ..Default::default() },
    )
    .unwrap();
    result.groups.iter().flat_map(|g| &g.firearms).count()
}

#[test]
fn matches_structured_fields() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &base_input()).unwrap();

    assert_eq!(search(&db.conn, "Colt"), 1, "should match make");
    assert_eq!(search(&db.conn, "1911"), 1, "should match model");
    assert_eq!(search(&db.conn, "CO1911"), 1, "should match serial number");
    assert_eq!(search(&db.conn, "45 ACP"), 1, "should match caliber");
}

#[test]
fn matches_free_form_notes_and_accessories() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &base_input()).unwrap();

    assert_eq!(search(&db.conn, "grandfather"), 1, "should match free-form notes");
    assert_eq!(search(&db.conn, "holster"), 1, "should match accessories");
}

#[test]
fn matches_the_joined_firearm_type_name() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &base_input()).unwrap();

    assert_eq!(search(&db.conn, "Handgun"), 1, "should match firearm_type.name via the join");
}

#[test]
fn stays_in_sync_after_update_and_delete() {
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &base_input()).unwrap();

    let mut edited = base_input();
    edited.notes = Some("re-blued and refinished".into());
    ops::update_firearm(&db.conn, created.id, &edited).unwrap();

    assert_eq!(search(&db.conn, "grandfather"), 0, "stale note text must not still match");
    assert_eq!(search(&db.conn, "re-blued"), 1, "updated note text must match");

    ops::delete_firearm(&db.conn, created.id, true).unwrap();
    assert_eq!(search(&db.conn, "re-blued"), 0, "deleted firearm must not match");
}

#[test]
fn no_match_returns_an_empty_result() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &base_input()).unwrap();

    assert_eq!(search(&db.conn, "nonexistentxyz"), 0);
}

/// Search runs as the user types, so a partially typed last word must
/// already match ("Rem" finds Remington, "CO19" finds serial CO1911) —
/// whole-word-only matching showed nothing until the word was complete.
#[test]
fn matches_a_partially_typed_last_word() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &base_input()).unwrap();

    assert_eq!(search(&db.conn, "Col"), 1, "prefix of make");
    assert_eq!(search(&db.conn, "CO19"), 1, "prefix of serial number");
    assert_eq!(search(&db.conn, "minor pit"), 1, "phrase ending in a partial word");
    assert_eq!(search(&db.conn, "pitting minor"), 0, "earlier words still form a phrase");
    assert_eq!(search(&db.conn, "\""), 0, "a stray quote is not a syntax error");
}

/// FR-039 / US1 Acceptance Scenario 17: a word in a firearm's finish is found.
#[test]
fn matches_a_word_in_the_finish_including_after_an_edit_and_a_delete() {
    let db = TestDb::new();
    let created = ops::create_firearm(
        &db.conn,
        &FirearmInput { finish: Some("Cerakote flat dark earth".into()), ..base_input() },
    )
    .unwrap();
    assert_eq!(search(&db.conn, "Cerakote"), 1, "should match finish");

    let edited = FirearmInput { finish: Some("Parkerized".into()), ..base_input() };
    ops::update_firearm(&db.conn, created.id, &edited).unwrap();
    assert_eq!(search(&db.conn, "Cerakote"), 0, "the old finish is no longer indexed");
    assert_eq!(search(&db.conn, "Parkerized"), 1, "the edited finish is indexed");

    ops::delete_firearm(&db.conn, created.id, true).unwrap();
    assert_eq!(search(&db.conn, "Parkerized"), 0, "a deleted firearm is not found");
}
