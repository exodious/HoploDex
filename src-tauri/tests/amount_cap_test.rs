//! Issue #67: every monetary amount is capped at $99,999,999 (whole dollars),
//! and no total, however large its parts, can fail a listing. Run against a
//! real temporary SQLCipher database.
//!
//! The cap is a rule on entry (`models::rules::MAX_AMOUNT_DOLLARS`, mirrored
//! by `MAX_DOLLARS` in `src/lib/money.ts`): a firearm, an accessory, a
//! policy, a disposition and an imported row all go through it. The totals
//! are overflow-safe regardless: a row written before the cap existed, or by
//! some future path, may hold anything the column can, so they saturate
//! instead of raising SQLite's integer overflow or wrapping.

mod sheets;
mod support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::accessories::{ListAccessoriesInput, ops as accessory_ops};
use hoplodex_lib::commands::firearms::{DisposeInput, ops as firearm_ops};
use hoplodex_lib::commands::import_export::ImportSessionStore;
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::accessory::AccessoryInput;
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput};
use hoplodex_lib::services::insurance_status::InsuranceWarning;
use hoplodex_lib::services::valuation::get_value_summary;
use serde_json::json;
use sheets::{accessory_cells, accessory_table, csv_in, firearm_cells, firearm_table, import};
use support::{TestDb, firearm, policy};
use tempfile::TempDir;

const CAP: i64 = 99_999_999;
/// JavaScript's `Number.MAX_SAFE_INTEGER`: what a total saturates at.
const CEILING: i64 = 9_007_199_254_740_991;

fn accessory(make: &str) -> AccessoryInput {
    serde_json::from_value(
        json!({ "accessoryKindId": 1, "make": make, "model": "Model", "status": "active" }),
    )
    .unwrap()
}

fn field_error(err: &CommandError, field: &str) -> Option<String> {
    err.field_errors.as_ref().and_then(|fields| fields.get(field).cloned())
}

fn dispose_input(price: i64) -> DisposeInput {
    DisposeInput {
        disposition_type: DispositionType::Sold,
        recipient: "Jane Doe".into(),
        date: "2025-06-15".into(),
        price,
        with_mounted: Vec::new(),
    }
}

const TOO_MUCH: &str = " can't be more than $99,999,999.";

// --- The cap on entry --------------------------------------------------------------------------

#[test]
fn a_firearm_amount_at_the_cap_is_accepted_and_one_above_it_is_refused() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2020-01-01", "2099-12-31", None))
            .unwrap();

    let mut at_cap = firearm("Glock", "19", "AT-CAP");
    at_cap.estimated_value = Some(CAP);
    at_cap.acquisition_price = Some(CAP);
    at_cap.insurance_policy_id = Some(rider.id);
    at_cap.scheduled_coverage_amount = Some(CAP);
    let created = firearm_ops::create_firearm(&db.conn, &at_cap, false, None).unwrap();
    assert_eq!(created.estimated_value, Some(CAP));
    assert_eq!(created.scheduled_coverage_amount, Some(CAP));

    let mut over = firearm("Glock", "19", "OVER");
    over.estimated_value = Some(CAP + 1);
    over.acquisition_price = Some(CAP + 1);
    over.insurance_policy_id = Some(rider.id);
    over.scheduled_coverage_amount = Some(CAP + 1);
    let err = firearm_ops::create_firearm(&db.conn, &over, false, None).unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    for (field, label) in [
        ("estimatedValue", "Estimated value"),
        ("acquisitionPrice", "Acquisition price"),
        ("scheduledCoverageAmount", "Scheduled coverage amount"),
    ] {
        assert_eq!(field_error(&err, field), Some(format!("{label}{TOO_MUCH}")), "{field}");
    }

    // An edit is held to the same rule, and nothing is written.
    let mut edit = at_cap.clone();
    edit.estimated_value = Some(i64::MAX);
    let err = firearm_ops::update_firearm(&db.conn, created.id, &edit, false).unwrap_err();
    assert!(field_error(&err, "estimatedValue").is_some());
    assert_eq!(firearm_ops::get_firearm(&db.conn, created.id).unwrap().estimated_value, Some(CAP));

    // Scheduling through the coverage dialog's command is held to it too.
    let err =
        insurance_ops::assign_firearm_coverage(&db.conn, created.id, Some(rider.id), Some(CAP + 1))
            .unwrap_err();
    assert!(field_error(&err, "scheduledCoverageAmount").is_some());
}

#[test]
fn an_accessory_amount_at_the_cap_is_accepted_and_one_above_it_is_refused() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2020-01-01", "2099-12-31", None))
            .unwrap();

    let at_cap = AccessoryInput {
        estimated_value: Some(CAP),
        acquisition_price: Some(CAP),
        insurance_policy_id: Some(rider.id),
        scheduled_coverage_amount: Some(CAP),
        ..accessory("Leupold")
    };
    let created = accessory_ops::create_accessory(&db.conn, &at_cap, None).unwrap();
    assert_eq!(created.estimated_value, Some(CAP));

    let over = AccessoryInput {
        estimated_value: Some(CAP + 1),
        acquisition_price: Some(CAP + 1),
        scheduled_coverage_amount: Some(CAP + 1),
        ..at_cap.clone()
    };
    let err = accessory_ops::create_accessory(&db.conn, &over, None).unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    for (field, label) in [
        ("estimatedValue", "Estimated value"),
        ("acquisitionPrice", "Acquisition price"),
        ("scheduledCoverageAmount", "Scheduled coverage amount"),
    ] {
        assert_eq!(field_error(&err, field), Some(format!("{label}{TOO_MUCH}")), "{field}");
    }

    let err = accessory_ops::update_accessory(&db.conn, created.id, &over).unwrap_err();
    assert!(field_error(&err, "estimatedValue").is_some());
    let err = insurance_ops::assign_accessory_coverage(
        &db.conn,
        created.id,
        Some(rider.id),
        Some(CAP + 1),
    )
    .unwrap_err();
    assert!(field_error(&err, "scheduledCoverageAmount").is_some());
}

#[test]
fn a_disposition_price_at_the_cap_is_accepted_and_one_above_it_is_refused() {
    let db = TestDb::new();
    let gun =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "D1"), false, None).unwrap();
    let err = firearm_ops::dispose_firearm(&db.conn, gun.id, &dispose_input(CAP + 1)).unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(field_error(&err, "dispositionPrice"), Some(format!("Disposition price{TOO_MUCH}")));
    let sold = firearm_ops::dispose_firearm(&db.conn, gun.id, &dispose_input(CAP)).unwrap();
    assert_eq!(sold.disposition_price, Some(CAP));

    let part = accessory_ops::create_accessory(&db.conn, &accessory("Sling"), None).unwrap();
    let err = accessory_ops::dispose_accessory(
        &db.conn,
        part.id,
        &serde_json::from_value(json!({
            "dispositionType": "sold", "recipient": "Jane", "date": "2025-06-15", "price": CAP + 1,
        }))
        .unwrap(),
    )
    .unwrap_err();
    assert!(field_error(&err, "dispositionPrice").is_some());
    let sold = accessory_ops::dispose_accessory(
        &db.conn,
        part.id,
        &serde_json::from_value(json!({
            "dispositionType": "sold", "recipient": "Jane", "date": "2025-06-15", "price": CAP,
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(sold.disposition_price, Some(CAP));
}

#[test]
fn a_policy_limit_at_the_cap_is_accepted_and_one_above_it_is_refused() {
    let db = TestDb::new();
    let created = insurance_ops::create_policy(
        &db.conn,
        &policy("At cap", "2025-01-01", "2026-01-01", Some(CAP)),
    )
    .unwrap();
    assert_eq!(created.blanket_coverage_limit, Some(CAP));

    let err = insurance_ops::create_policy(
        &db.conn,
        &policy("Over", "2020-01-01", "2021-01-01", Some(CAP + 1)),
    )
    .unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(
        field_error(&err, "blanketCoverageLimit"),
        Some(format!("Blanket coverage limit{TOO_MUCH}"))
    );

    let err = insurance_ops::update_policy(
        &db.conn,
        created.id,
        &policy("At cap", "2025-01-01", "2026-01-01", Some(i64::MAX)),
    )
    .unwrap_err();
    assert!(field_error(&err, "blanketCoverageLimit").is_some());
}

#[test]
fn an_imported_row_over_the_cap_is_a_row_error_and_one_at_the_cap_imports() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();
    let firearms = firearm_table(&[
        firearm_cells("Glock", "19", "AT-CAP", &[("estimated_value", "99,999,999")]),
        firearm_cells("Glock", "19", "OVER", &[("estimated_value", "100000000")]),
        firearm_cells("Glock", "19", "HUGE", &[("acquisition_price", "9223372036854775807")]),
        firearm_cells("Glock", "19", "BEYOND", &[("estimated_value", "99999999999999999999")]),
    ]);
    let accessories = accessory_table(&[
        accessory_cells("Optic", &[("estimated_value", "99999999")]),
        accessory_cells("Optic", &[("scheduled_coverage_amount", "100000000")]),
        accessory_cells("Optic", &[("estimated_value", "100000000")]),
    ]);
    let firearm_path = csv_in(dir.path(), "firearms.csv", &firearms);
    let accessory_path = csv_in(dir.path(), "accessories.csv", &accessories);

    let result = import(&db.conn, &store, &[&firearm_path, &accessory_path]).unwrap();

    assert_eq!(result.imported_count, 2, "{:?}", result.row_errors);
    assert_eq!(result.imported_accessory_count, 1, "{:?}", result.row_errors);
    let errors = sheets::entries(&result.row_errors);
    let at = |table: &str, row: u64| -> Vec<&String> {
        errors.iter().filter(|(t, r, _)| t == table && *r == row).map(|(_, _, m)| m).collect()
    };
    assert!(at("firearms", 1).is_empty());
    assert!(
        at("firearms", 2)[0].contains("estimated_value") && at("firearms", 2)[0].contains(TOO_MUCH)
    );
    assert!(
        at("firearms", 3)[0].contains("acquisition_price")
            && at("firearms", 3)[0].contains(TOO_MUCH)
    );
    assert!(at("firearms", 4)[0].contains("estimated_value"));
    assert!(at("accessories", 1).is_empty());
    assert!(at("accessories", 2)[0].contains("scheduled_coverage_amount"));
    assert!(
        at("accessories", 3)[0].contains("estimated_value")
            && at("accessories", 3)[0].contains(TOO_MUCH)
    );

    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    let all: Vec<_> = listing.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].estimated_value, Some(CAP));
}

// --- Totals that cannot fail -------------------------------------------------------------------

/// A firearm or accessory row as an older database could hold it: written
/// straight to the table, past every rule.
fn insert_old_firearm(db: &TestDb, serial: &str, value: i64) {
    db.conn
        .execute(
            "INSERT INTO firearms (uid, make, model, serial_number, caliber, firearm_type_id,
                                   estimated_value, created_at, updated_at)
             VALUES (?1, 'Old', 'Rifle', ?2, '9mm', 1, ?3, 'now', 'now')",
            rusqlite::params![support::uid(), serial, value],
        )
        .unwrap();
}

fn insert_old_accessory(db: &TestDb, value: i64) {
    db.conn
        .execute(
            "INSERT INTO accessories (uid, accessory_kind_id, make, model, estimated_value,
                                      created_at, updated_at)
             VALUES (?1, 1, 'Old', 'Optic', ?2, 'now', 'now')",
            rusqlite::params![support::uid(), value],
        )
        .unwrap();
}

#[test]
fn totals_over_many_near_cap_rows_are_exact_and_mix_firearms_and_accessories() {
    let db = TestDb::new();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Blanket", "2020-01-01", "2099-12-31", Some(CAP)),
    )
    .unwrap();
    for n in 0..120 {
        let mut input = firearm("Glock", "19", &format!("SN-{n}"));
        input.estimated_value = Some(CAP);
        firearm_ops::create_firearm(&db.conn, &input, false, None).unwrap();
    }
    for _ in 0..80 {
        let input = AccessoryInput { estimated_value: Some(CAP), ..accessory("Leupold") };
        accessory_ops::create_accessory(&db.conn, &input, None).unwrap();
    }

    let summary = get_value_summary(&db.conn).unwrap();
    assert_eq!(summary.firearms_total, 120 * CAP);
    assert_eq!(summary.accessories_total, 80 * CAP);
    assert_eq!(summary.collection_total, 200 * CAP);
    let blanket = summary.blanket.expect("the blanket policy is in force");
    assert_eq!(blanket.total, 200 * CAP);
    assert_eq!((blanket.firearm_count, blanket.accessory_count), (120, 80));
    assert!(blanket.under_insured);

    let listing = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    assert!(
        listing
            .groups
            .iter()
            .flat_map(|g| &g.firearms)
            .all(|f| f.insurance_warning == InsuranceWarning::UnderInsured)
    );
}

#[test]
fn rows_above_the_cap_from_an_older_database_still_list_and_value() {
    let db = TestDb::new();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Blanket", "2020-01-01", "2099-12-31", Some(i64::MAX)),
    )
    .unwrap_err(); // a limit like that is refused now
    db.conn
        .execute(
            "INSERT INTO insurance_policies (name, policy_number, insurance_company,
                 blanket_coverage_limit, effective_start_date, effective_end_date,
                 created_at, updated_at)
             VALUES ('Old blanket', 'OB-1', 'Acme', ?1, '2020-01-01', '2099-12-31', 'now', 'now')",
            [i64::MAX],
        )
        .unwrap();

    // Each of these fits in an i64, but two of them do not sum to one.
    for n in 0..3 {
        insert_old_firearm(&db, &format!("OLD-{n}"), i64::MAX);
    }
    for _ in 0..3 {
        insert_old_accessory(&db, i64::MAX - 1);
    }
    let normal = firearm_ops::create_firearm(
        &db.conn,
        &{
            let mut input = firearm("Sig", "P226", "NORMAL");
            input.estimated_value = Some(1_000);
            input
        },
        false,
        None,
    )
    .unwrap();

    // Nothing fails, and every total saturates at what the web view can hold.
    let summary = get_value_summary(&db.conn).unwrap();
    assert_eq!(summary.firearms_total, CEILING);
    assert_eq!(summary.accessories_total, CEILING);
    assert_eq!(summary.collection_total, CEILING);
    let blanket = summary.blanket.expect("in force");
    assert_eq!(blanket.total, CEILING);
    assert_eq!(blanket.limit, CEILING);
    assert_eq!((blanket.firearm_count, blanket.accessory_count), (4, 3));

    let firearms = firearm_ops::list_firearms(&db.conn, &Default::default()).unwrap();
    assert_eq!(firearms.groups.iter().map(|g| g.firearms.len()).sum::<usize>(), 4);
    let accessories = accessory_ops::list_accessories(
        &db.conn,
        &ListAccessoriesInput { query: None, group_by: None, include_disposed: false },
    )
    .unwrap();
    assert_eq!(accessories.groups.iter().map(|g| g.accessories.len()).sum::<usize>(), 3);

    // Individual records stay reachable for repair, and an over-cap value can
    // be brought down to a valid one.
    let old = firearms
        .groups
        .iter()
        .flat_map(|g| &g.firearms)
        .find(|f| f.estimated_value == Some(i64::MAX))
        .expect("an old record");
    let mut repaired = FirearmInput::from(&firearm_ops::get_firearm(&db.conn, old.id).unwrap());
    repaired.estimated_value = Some(CAP);
    firearm_ops::update_firearm(&db.conn, old.id, &repaired, true).unwrap();
    assert_eq!(firearm_ops::get_firearm(&db.conn, normal.id).unwrap().estimated_value, Some(1_000));
}
