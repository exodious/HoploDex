//! specs/006-accessory-links User Story 1 through `ops`, against a real
//! temporary SQLCipher database (no mocks): the fixed kind list (FR-002),
//! saving and reopening an accessory with every optional field (FR-001,
//! FR-003, FR-004), the validation rules (data-model.md "Validation rules"),
//! disposing, restoring and deleting one (FR-006), and the free-text
//! `accessories` field staying as it is (FR-007).
//!
//! Written before the implementation (tasks.md T015). Inputs and outputs the
//! contract describes as JSON (`AccessoryInput` in, `AccessoryDetail`,
//! `ListAccessoriesInput` and `DisposeInput` in or out) are built from and
//! read as JSON in the contract's own camelCase names, so the tests need
//! nothing from the structs but the names in contracts/tauri-commands.md.

mod support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::accessories::ops;
use hoplodex_lib::commands::entries::ops as entry_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::insurance::ops as insurance;
use hoplodex_lib::models::accessory::{Accessory, AccessoryInput};
use hoplodex_lib::models::firearm::FirearmStatus;
use serde_json::{Value, json};
use support::TestDb;

const OPTIC: i64 = 1;
const MAGAZINE: i64 = 3;
const CONVERSION_KIT: i64 = 8;
const SLING: i64 = 10;

/// An active accessory of `kind` with its required make and model and every
/// optional field empty.
fn accessory(kind: i64) -> AccessoryInput {
    serde_json::from_value(
        json!({ "accessoryKindId": kind, "make": "Make", "model": "Model", "status": "active" }),
    )
    .unwrap()
}

/// A disposed accessory of `kind` whose disposition fields the test sets.
fn disposed_accessory(kind: i64) -> AccessoryInput {
    AccessoryInput { status: FirearmStatus::Disposed, ..accessory(kind) }
}

/// The contract's `DisposeInput`: sold to Jane on 2025-06-15 for $400.
fn dispose(db: &TestDb, id: i64) -> Accessory {
    ops::dispose_accessory(
        &db.conn,
        id,
        &serde_json::from_value(json!({
            "dispositionType": "sold",
            "recipient": "Jane Doe",
            "date": "2025-06-15",
            "price": 400,
        }))
        .unwrap(),
    )
    .unwrap()
}

fn reverse(db: &TestDb, id: i64, history: &str) -> Accessory {
    ops::reverse_accessory_disposition(
        &db.conn,
        id,
        &serde_json::from_value(json!({ "history": history })).unwrap(),
    )
    .unwrap()
}

/// `get_accessory` as the frontend receives it.
fn detail(db: &TestDb, id: i64) -> Value {
    serde_json::to_value(ops::get_accessory(&db.conn, id).unwrap()).unwrap()
}

/// The ids `list_accessories` returns, across its groups.
fn listed_ids(db: &TestDb, include_disposed: bool) -> Vec<i64> {
    let output = ops::list_accessories(
        &db.conn,
        &serde_json::from_value(json!({ "includeDisposed": include_disposed })).unwrap(),
    )
    .unwrap();
    let output = serde_json::to_value(output).unwrap();
    let mut ids: Vec<i64> = output["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|group| group["accessories"].as_array().unwrap().iter())
        .map(|summary| summary["id"].as_i64().unwrap())
        .collect();
    ids.sort_unstable();
    ids
}

fn field_error(err: &CommandError, field: &str) -> Option<String> {
    err.field_errors.as_ref().and_then(|errors| errors.get(field).cloned())
}

fn assert_field_error(err: CommandError, field: &str, message: &str) {
    assert_eq!(err.code, "VALIDATION_ERROR", "{err:?}");
    assert_eq!(field_error(&err, field).as_deref(), Some(message), "{err:?}");
}

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

fn iso(date: chrono::NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn count(db: &TestDb, table: &str) -> i64 {
    db.conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0)).unwrap()
}

// --- The kinds (US1-2, FR-002) ------------------------------------------------

#[test]
fn the_fourteen_kinds_are_listed_in_order_with_other_last_and_no_suppressor() {
    let db = TestDb::new();

    let kinds = entry_ops::list_accessory_kinds(&db.conn).unwrap().kinds;

    let names: Vec<&str> = kinds.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Optic",
            "Light or laser",
            "Magazine",
            "Stock or brace",
            "Upper receiver",
            "Barrel",
            "Trigger",
            "Muzzle device",
            "Conversion kit",
            "Mount or rail",
            "Bipod",
            "Sling",
            "Case",
            "Other",
        ]
    );
    assert!(!names.iter().any(|name| name.contains("Suppressor")));
    let orders: Vec<i64> = kinds.iter().map(|k| k.sort_order).collect();
    assert!(orders.windows(2).all(|pair| pair[0] < pair[1]), "in sortOrder: {orders:?}");
    // Trigger and Bipod were added after the first twelve: new ids, listed
    // beside their neighbours, so no existing id changes meaning.
    assert_eq!(kinds.iter().map(|k| k.id).collect::<Vec<_>>(), [1, 2, 3, 4, 5, 6, 13, 7, 8, 9, 14, 10, 11, 12]);
    assert!(kinds.iter().all(|k| k.offered), "every kind is offered");
    assert_eq!(kinds[0].generic_thumbnail_key, "optic");
    assert_eq!(kinds[6].generic_thumbnail_key, "trigger");
    assert_eq!(kinds[10].generic_thumbnail_key, "bipod");
    assert_eq!(kinds[13].generic_thumbnail_key, "accessory");
}

#[test]
fn a_kind_no_longer_offered_is_still_listed_still_held_and_still_choosable_in_the_command() {
    // FR-002: the row is never removed; the command accepts any kind that exists.
    let db = TestDb::new();
    let held = ops::create_accessory(&db.conn, &accessory(CONVERSION_KIT), None).unwrap();

    db.conn.execute("UPDATE accessory_kinds SET offered = 0 WHERE id = 8", []).unwrap();

    let kinds = entry_ops::list_accessory_kinds(&db.conn).unwrap().kinds;
    assert_eq!(kinds.len(), 14);
    for kind in &kinds {
        assert_eq!(kind.offered, kind.id != CONVERSION_KIT, "kind {}", kind.id);
    }

    // It reopens and saves an edit with its kind unchanged.
    assert_eq!(detail(&db, held.id)["accessoryKindId"], CONVERSION_KIT);
    let mut edited = accessory(CONVERSION_KIT);
    edited.notes = Some("Edited".into());
    let saved = ops::update_accessory(&db.conn, held.id, &edited).unwrap();
    assert_eq!(saved.accessory_kind_id, CONVERSION_KIT);
    assert_eq!(detail(&db, held.id)["notes"], "Edited");

    // And a new one may still be created with it.
    assert!(ops::create_accessory(&db.conn, &accessory(CONVERSION_KIT), None).is_ok());
}

// --- Saving and reopening (US1-3 to US1-5, FR-001) ------------------------------

#[test]
fn an_accessory_with_only_a_kind_make_and_model_saves_and_reopens_with_every_optional_field_empty()
{
    // US1-5.
    let mut db = TestDb::new();
    let created = ops::create_accessory(&db.conn, &accessory(SLING), None).unwrap();
    db.reopen();

    let shown = detail(&db, created.id);

    assert_eq!(shown["id"], created.id);
    assert_eq!(shown["accessoryKindId"], SLING);
    assert_eq!(shown["make"], "Make");
    assert_eq!(shown["model"], "Model");
    assert_eq!(shown["status"], "active");
    for field in [
        "serialNumber",
        "caliber",
        "cartridge",
        "notes",
        "estimatedValue",
        "acquisitionSource",
        "acquisitionDate",
        "acquisitionPrice",
        "dispositionType",
        "dispositionRecipient",
        "dispositionDate",
        "dispositionPrice",
        "insurancePolicyId",
        "scheduledCoverageAmount",
        "thumbnailPhotoId",
    ] {
        assert_eq!(shown[field], Value::Null, "{field} should be null");
    }
    assert_eq!(shown["dispositionHistory"], json!([]));
}

#[test]
fn an_optic_saves_and_every_value_comes_back() {
    // US1-3.
    let mut db = TestDb::new();
    let mut input = accessory(OPTIC);
    input.make = "Leupold".into();
    input.model = "VX-5HD 3-15x44".into();
    input.serial_number = Some("LEU-5521".into());
    input.estimated_value = Some(1000);
    input.acquisition_price = Some(1100);
    input.acquisition_date = Some("2025-11-02".into());
    input.notes = Some("Zeroed at 100 yards.".into());

    let created = ops::create_accessory(&db.conn, &input, None).unwrap();
    db.reopen();
    let shown = detail(&db, created.id);

    assert_eq!(shown["accessoryKindId"], OPTIC);
    assert_eq!(shown["make"], "Leupold");
    assert_eq!(shown["model"], "VX-5HD 3-15x44");
    assert_eq!(shown["serialNumber"], "LEU-5521");
    assert_eq!(shown["estimatedValue"], 1000);
    assert_eq!(shown["acquisitionPrice"], 1100);
    assert_eq!(shown["acquisitionDate"], "2025-11-02");
    assert_eq!(shown["notes"], "Zeroed at 100 yards.");
    assert_eq!(shown["status"], "active");
}

#[test]
fn a_pair_of_magazines_saves_as_one_record() {
    // US1-4.
    let db = TestDb::new();
    let mut input = accessory(MAGAZINE);
    input.make = "Walther".into();
    input.model = "P38 magazines, pair".into();
    input.estimated_value = Some(180);

    let created = ops::create_accessory(&db.conn, &input, None).unwrap();

    let shown = detail(&db, created.id);
    assert_eq!(shown["model"], "P38 magazines, pair");
    assert_eq!(shown["estimatedValue"], 180);
    assert_eq!(count(&db, "accessories"), 1);
}

#[test]
fn editing_saves_the_new_values() {
    let db = TestDb::new();
    let created = ops::create_accessory(&db.conn, &accessory(OPTIC), None).unwrap();
    let mut edited = accessory(MAGAZINE);
    edited.make = "Magpul".into();
    edited.estimated_value = Some(25);

    let saved = ops::update_accessory(&db.conn, created.id, &edited).unwrap();

    assert_eq!(saved.id, created.id);
    assert_eq!(saved.uid, created.uid);
    let shown = detail(&db, created.id);
    assert_eq!(shown["accessoryKindId"], MAGAZINE);
    assert_eq!(shown["make"], "Magpul");
    assert_eq!(shown["estimatedValue"], 25);
}

#[test]
fn getting_or_updating_an_accessory_that_does_not_exist_is_not_found() {
    let db = TestDb::new();
    assert_eq!(ops::get_accessory(&db.conn, 999).unwrap_err().code, "NOT_FOUND");
    assert_eq!(
        ops::update_accessory(&db.conn, 999, &accessory(OPTIC)).unwrap_err().code,
        "NOT_FOUND"
    );
}

// --- The kind is required (FR-001) ------------------------------------------------

#[test]
fn a_missing_or_unknown_kind_fails_on_the_kind_field_with_the_exact_message() {
    let db = TestDb::new();
    for kind in [0, 99, -1] {
        let err = ops::create_accessory(&db.conn, &accessory(kind), None).unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR", "kind {kind}");
        let errors = err.field_errors.as_ref().expect("field errors");
        assert_eq!(errors.len(), 1, "kind {kind}: only the kind is wrong: {errors:?}");
        assert_eq!(errors["accessoryKindId"], "Choose a kind.", "kind {kind}");
    }
    assert_eq!(count(&db, "accessories"), 0);

    let held = ops::create_accessory(&db.conn, &accessory(OPTIC), None).unwrap();
    let err = ops::update_accessory(&db.conn, held.id, &accessory(99)).unwrap_err();
    assert_field_error(err, "accessoryKindId", "Choose a kind.");
    assert_eq!(detail(&db, held.id)["accessoryKindId"], OPTIC);
}

// --- No identity rules (FR-004) ------------------------------------------------------

#[test]
fn two_accessories_with_the_same_make_model_and_serial_number_both_save() {
    let db = TestDb::new();
    let mut input = accessory(OPTIC);
    input.make = "Leupold".into();
    input.model = "VX-5HD".into();
    input.serial_number = Some("SN-1".into());

    let first = ops::create_accessory(&db.conn, &input, None).unwrap();
    let second = ops::create_accessory(&db.conn, &input, None).unwrap();

    assert_ne!(first.id, second.id);
    assert_eq!(count(&db, "accessories"), 2);
}

// --- Entry rules and trimming (FR-003) ------------------------------------------------

#[test]
fn make_model_caliber_and_cartridge_follow_004s_entry_rules() {
    let db = TestDb::new();
    let too_long = "a".repeat(101);

    let mut make = accessory(OPTIC);
    make.make = too_long.clone();
    assert_field_error(
        ops::create_accessory(&db.conn, &make, None).unwrap_err(),
        "make",
        "Make can be at most 100 characters.",
    );

    let mut model = accessory(OPTIC);
    model.model = too_long.clone();
    assert_field_error(
        ops::create_accessory(&db.conn, &model, None).unwrap_err(),
        "model",
        "Model can be at most 100 characters.",
    );

    let mut caliber = accessory(OPTIC);
    caliber.caliber = Some(too_long.clone());
    assert_field_error(
        ops::create_accessory(&db.conn, &caliber, None).unwrap_err(),
        "caliber",
        "Caliber can be at most 100 characters.",
    );

    let mut cartridge = accessory(OPTIC);
    cartridge.cartridge = Some(too_long);
    assert_field_error(
        ops::create_accessory(&db.conn, &cartridge, None).unwrap_err(),
        "cartridge",
        "Cartridge can be at most 100 characters.",
    );

    let mut control = accessory(OPTIC);
    control.make = "Leu\u{7}pold".into();
    control.model = "VX\u{0}5".into();
    let err = ops::create_accessory(&db.conn, &control, None).unwrap_err();
    assert_eq!(
        field_error(&err, "make").as_deref(),
        Some("Make can't contain control characters.")
    );
    assert_eq!(
        field_error(&err, "model").as_deref(),
        Some("Model can't contain control characters.")
    );

    // Exactly 100 characters, counted after trimming, is fine.
    let mut limit = accessory(OPTIC);
    limit.make = format!("  {}  ", "a".repeat(100));
    assert!(ops::create_accessory(&db.conn, &limit, None).is_ok());
    assert_eq!(count(&db, "accessories"), 1, "the refused ones saved nothing");
}

#[test]
fn a_blank_make_or_model_is_refused_on_create_and_on_update() {
    // FR-001: so that no two accessories are named by their kind alone.
    let db = TestDb::new();
    let mut input = accessory(OPTIC);
    input.make = "   ".into();
    input.model = String::new();

    let err = ops::create_accessory(&db.conn, &input, None).unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(field_error(&err, "make").as_deref(), Some("Make is required."));
    assert_eq!(field_error(&err, "model").as_deref(), Some("Model is required."));
    assert_eq!(count(&db, "accessories"), 0);

    let held = ops::create_accessory(&db.conn, &accessory(OPTIC), None).unwrap();
    let mut blanked = accessory(OPTIC);
    blanked.model = " ".into();
    assert_field_error(
        ops::update_accessory(&db.conn, held.id, &blanked).unwrap_err(),
        "model",
        "Model is required.",
    );
    assert_eq!(detail(&db, held.id)["model"], "Model");
}

#[test]
fn a_blank_caliber_or_cartridge_is_accepted_and_stored_as_null() {
    let db = TestDb::new();
    let mut input = accessory(OPTIC);
    input.make = "  Leupold ".into();
    input.caliber = Some(" ".into());
    input.cartridge = Some("".into());

    let created = ops::create_accessory(&db.conn, &input, None).unwrap();

    let shown = detail(&db, created.id);
    assert_eq!(shown["make"], "Leupold", "trimmed");
    for field in ["caliber", "cartridge"] {
        assert_eq!(shown[field], Value::Null, "{field}");
    }
}

#[test]
fn an_unchanged_over_long_stored_value_passes_on_update_but_a_changed_one_does_not() {
    // 004 FR-015 applies on entry only, so a longer value already stored
    // (the column has no length CHECK) stays valid until it is edited.
    let db = TestDb::new();
    let long_make = "M".repeat(120);
    db.conn
        .execute(
            "INSERT INTO accessories (id, uid, accessory_kind_id, make, model, created_at, updated_at)
             VALUES (1, ?1, 1, ?2, 'Model', 'now', 'now')",
            rusqlite::params![support::uid(), long_make],
        )
        .unwrap();

    let mut unchanged = accessory(OPTIC);
    unchanged.make = long_make.clone();
    unchanged.notes = Some("Only the note changed.".into());
    assert!(ops::update_accessory(&db.conn, 1, &unchanged).is_ok());
    assert_eq!(detail(&db, 1)["make"], long_make.as_str());

    let mut changed = accessory(OPTIC);
    changed.make = "M".repeat(121);
    assert_field_error(
        ops::update_accessory(&db.conn, 1, &changed).unwrap_err(),
        "make",
        "Make can be at most 100 characters.",
    );
}

#[test]
fn serial_number_acquisition_source_and_notes_are_only_trimmed_and_blank_is_null() {
    let db = TestDb::new();
    let mut input = accessory(OPTIC);
    input.serial_number = Some("  SN 1\t".into());
    input.acquisition_source = Some("  Gun show  ".into());
    input.notes = Some("  A note  ".into());
    let created = ops::create_accessory(&db.conn, &input, None).unwrap();
    let shown = detail(&db, created.id);
    assert_eq!(shown["serialNumber"], "SN 1");
    assert_eq!(shown["acquisitionSource"], "Gun show");
    assert_eq!(shown["notes"], "A note");

    let mut blank = accessory(OPTIC);
    blank.serial_number = Some("   ".into());
    blank.acquisition_source = Some("".into());
    blank.notes = Some("\n".into());
    let created = ops::create_accessory(&db.conn, &blank, None).unwrap();
    let shown = detail(&db, created.id);
    for field in ["serialNumber", "acquisitionSource", "notes"] {
        assert_eq!(shown[field], Value::Null, "{field}");
    }

    // No entry rules beyond trimming: length and control characters are not judged.
    let mut free = accessory(OPTIC);
    free.serial_number = Some(format!("S{}\u{7}", "9".repeat(150)));
    free.acquisition_source = Some("s".repeat(150));
    assert!(ops::create_accessory(&db.conn, &free, None).is_ok());
}

// --- The firearm's amount, date, disposition and insurance rules (data-model.md) -----

#[test]
fn an_acquisition_date_of_tomorrow_and_a_negative_amount_fail_with_the_firearms_messages() {
    let db = TestDb::new();
    let mut input = accessory(OPTIC);
    input.acquisition_date = Some(iso(today() + chrono::Duration::days(1)));
    input.estimated_value = Some(-1);
    input.acquisition_price = Some(-5);

    let err = ops::create_accessory(&db.conn, &input, None).unwrap_err();

    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(
        field_error(&err, "acquisitionDate").as_deref(),
        Some("Acquisition date can't be in the future.")
    );
    assert_eq!(
        field_error(&err, "estimatedValue").as_deref(),
        Some("Estimated value can't be negative.")
    );
    assert_eq!(
        field_error(&err, "acquisitionPrice").as_deref(),
        Some("Acquisition price can't be negative.")
    );

    // Today is allowed.
    let mut today_input = accessory(OPTIC);
    today_input.acquisition_date = Some(iso(today()));
    assert!(ops::create_accessory(&db.conn, &today_input, None).is_ok());
}

#[test]
fn a_disposed_accessory_needs_a_type_a_recipient_and_a_date() {
    let db = TestDb::new();

    let err = ops::create_accessory(&db.conn, &disposed_accessory(OPTIC), None).unwrap_err();

    assert_eq!(err.code, "VALIDATION_ERROR");
    for field in ["dispositionType", "dispositionRecipient", "dispositionDate"] {
        assert_eq!(
            field_error(&err, field).as_deref(),
            Some("Required when marking as disposed."),
            "{field}"
        );
    }
}

#[test]
fn disposition_details_on_an_active_accessory_are_refused() {
    let db = TestDb::new();
    let mut input = accessory(OPTIC);
    input.disposition_recipient = Some("Jane".into());

    let err = ops::create_accessory(&db.conn, &input, None).unwrap_err();

    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(field_error(&err, "status").is_some(), "{err:?}");
}

#[test]
fn a_disposition_date_before_the_acquisition_date_is_refused() {
    let db = TestDb::new();
    let mut input = accessory(OPTIC);
    input.acquisition_date = Some("2025-06-01".into());
    let created = ops::create_accessory(&db.conn, &input, None).unwrap();

    let err = ops::dispose_accessory(
        &db.conn,
        created.id,
        &serde_json::from_value(json!({
            "dispositionType": "sold",
            "recipient": "Jane",
            "date": "2025-05-01",
            "price": 10,
        }))
        .unwrap(),
    )
    .unwrap_err();

    assert_field_error(
        err,
        "dispositionDate",
        "Disposition date can't be earlier than the acquisition date.",
    );
    assert_eq!(detail(&db, created.id)["status"], "active");
}

#[test]
fn a_policy_without_an_amount_and_an_amount_without_a_policy_both_fail() {
    let db = TestDb::new();
    let policy = insurance::create_policy(
        &db.conn,
        &support::policy("Schedule", "2020-01-01", "2099-01-01", None),
    )
    .unwrap();

    let mut no_amount = accessory(OPTIC);
    no_amount.insurance_policy_id = Some(policy.id);
    assert_field_error(
        ops::create_accessory(&db.conn, &no_amount, None).unwrap_err(),
        "scheduledCoverageAmount",
        "Enter the amount scheduled on the policy.",
    );

    let mut no_policy = accessory(OPTIC);
    no_policy.scheduled_coverage_amount = Some(500);
    assert_field_error(
        ops::create_accessory(&db.conn, &no_policy, None).unwrap_err(),
        "insurancePolicyId",
        "Choose the policy this amount is scheduled on.",
    );

    let mut both = accessory(OPTIC);
    both.insurance_policy_id = Some(policy.id);
    both.scheduled_coverage_amount = Some(500);
    assert!(ops::create_accessory(&db.conn, &both, None).is_ok());
}

// --- Dispose, restore, delete (US1-9, US1-10, FR-006) ----------------------------------

#[test]
fn a_disposed_accessory_leaves_the_default_list_and_is_listed_with_include_disposed() {
    // US1-9.
    let db = TestDb::new();
    let kept = ops::create_accessory(&db.conn, &accessory(OPTIC), None).unwrap();
    let gone = ops::create_accessory(&db.conn, &accessory(SLING), None).unwrap();

    let disposed = dispose(&db, gone.id);

    assert_eq!(serde_json::to_value(&disposed).unwrap()["status"], "disposed");
    assert_eq!(listed_ids(&db, false), [kept.id]);
    let mut both = vec![kept.id, gone.id];
    both.sort_unstable();
    assert_eq!(listed_ids(&db, true), both);

    let shown = detail(&db, gone.id);
    assert_eq!(shown["status"], "disposed");
    assert_eq!(shown["dispositionType"], "sold");
    assert_eq!(shown["dispositionRecipient"], "Jane Doe");
    assert_eq!(shown["dispositionDate"], "2025-06-15");
    assert_eq!(shown["dispositionPrice"], 400);
}

#[test]
fn disposing_an_accessory_with_a_missing_field_changes_nothing() {
    let db = TestDb::new();
    let created = ops::create_accessory(&db.conn, &accessory(OPTIC), None).unwrap();

    let err = ops::dispose_accessory(
        &db.conn,
        created.id,
        &serde_json::from_value(json!({
            "dispositionType": "sold",
            "recipient": "  ",
            "date": "2025-06-15",
            "price": 400,
        }))
        .unwrap(),
    )
    .unwrap_err();

    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(field_error(&err, "dispositionRecipient").is_some());
    assert_eq!(detail(&db, created.id)["status"], "active");
}

#[test]
fn reversing_with_keep_retains_a_history_entry_and_with_discard_none() {
    // US1-9, FR-006.
    let db = TestDb::new();
    let kept = ops::create_accessory(&db.conn, &accessory(OPTIC), None).unwrap();
    let discarded = ops::create_accessory(&db.conn, &accessory(SLING), None).unwrap();
    dispose(&db, kept.id);
    dispose(&db, discarded.id);

    let restored_kept = reverse(&db, kept.id, "keep");
    let restored_discarded = reverse(&db, discarded.id, "discard");

    for restored in [&restored_kept, &restored_discarded] {
        let restored = serde_json::to_value(restored).unwrap();
        assert_eq!(restored["status"], "active");
        for field in
            ["dispositionType", "dispositionRecipient", "dispositionDate", "dispositionPrice"]
        {
            assert_eq!(restored[field], Value::Null, "{field}");
        }
    }
    let history = detail(&db, kept.id)["dispositionHistory"].clone();
    assert_eq!(history.as_array().unwrap().len(), 1, "{history}");
    assert_eq!(detail(&db, discarded.id)["dispositionHistory"], json!([]));
    assert_eq!(listed_ids(&db, false).len(), 2, "both are active and listed again");
}

#[test]
fn restoring_does_not_recheck_identity_so_a_duplicate_of_an_active_one_comes_back() {
    // FR-006: no nickname, key or marks re-check for an accessory.
    let db = TestDb::new();
    let mut input = accessory(OPTIC);
    input.make = "Leupold".into();
    input.model = "VX".into();
    input.serial_number = Some("SN-1".into());
    let first = ops::create_accessory(&db.conn, &input, None).unwrap();
    dispose(&db, first.id);
    let second = ops::create_accessory(&db.conn, &input, None).unwrap();

    let restored = reverse(&db, first.id, "keep");

    assert_eq!(serde_json::to_value(&restored).unwrap()["status"], "active");
    assert_ne!(first.id, second.id);
    assert_eq!(listed_ids(&db, false).len(), 2);
}

#[test]
fn deleting_needs_confirmation_and_then_removes_the_row() {
    // US1-10.
    let db = TestDb::new();
    let created = ops::create_accessory(&db.conn, &accessory(OPTIC), None).unwrap();

    let unconfirmed = ops::delete_accessory(&db.conn, created.id, false);
    assert!(unconfirmed.is_err(), "delete without confirmation must be blocked");
    assert!(ops::get_accessory(&db.conn, created.id).is_ok(), "the record must still exist");

    let confirmed = ops::delete_accessory(&db.conn, created.id, true).unwrap();
    assert!(confirmed.deleted);
    assert_eq!(ops::get_accessory(&db.conn, created.id).unwrap_err().code, "NOT_FOUND");
    assert_eq!(count(&db, "accessories"), 0);
    assert_eq!(
        ops::delete_accessory(&db.conn, created.id, true).unwrap_err().code,
        "NOT_FOUND",
        "a second delete finds nothing"
    );
}

// --- The free-text field stays (US1-11, FR-007) -----------------------------------------

#[test]
fn a_firearms_free_text_accessories_stays_unchanged_and_creates_no_accessory() {
    let db = TestDb::new();
    let mut input = support::firearm("Ruger", "10/22", "R-1");
    input.accessories = Some("Leupold scope".into());
    let firearm = firearm_ops::create_firearm(&db.conn, &input, false, None).unwrap();

    ops::create_accessory(&db.conn, &accessory(OPTIC), None).unwrap();
    let mut other = accessory(MAGAZINE);
    other.make = "Leupold".into();
    ops::create_accessory(&db.conn, &other, None).unwrap();

    let after = firearm_ops::get_firearm(&db.conn, firearm.id).unwrap();
    assert_eq!(after.accessories.as_deref(), Some("Leupold scope"));
    assert_eq!(count(&db, "accessories"), 2, "only the two created; none from the text");
}

// --- The identifier never leaves the backend (FR-019) --------------------------------------

#[test]
fn the_identifier_is_never_serialized_by_an_accessory_or_its_detail() {
    let db = TestDb::new();
    let created = ops::create_accessory(&db.conn, &accessory(OPTIC), None).unwrap();

    let plain = serde_json::to_value(&created).unwrap();
    let full = serde_json::to_value(ops::get_accessory(&db.conn, created.id).unwrap()).unwrap();

    for (name, value) in [("Accessory", plain), ("AccessoryDetail", full)] {
        assert!(value.get("uid").is_none(), "{name} has a uid key");
        assert!(!value.to_string().contains(&created.uid), "{name} carries the identifier");
    }
}
