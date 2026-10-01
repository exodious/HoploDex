//! Integration tests for `services::valuation`'s always-current value
//! summary (spec.md US3 Acceptance Scenarios 5-6, 11, 13; FR-015/036), run
//! against a real temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::{DisposeInput, ops as firearm_ops};
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::valuation::{get_value_summary, get_value_summary_as_of};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use support::{TestDb, date, firearm, policy};

const TODAY: &str = "2026-06-15";

fn valued(make: &str, value: i64) -> FirearmInput {
    FirearmInput { estimated_value: Some(value), ..firearm(make, "M", &format!("{make}-SN")) }
}

#[test]
fn scenario_5_total_updates_immediately_after_every_mutation() {
    let db = TestDb::new();

    let initial = get_value_summary(&db.conn).unwrap();
    assert_eq!(initial.collection_total, 0);

    let a = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50000), false).unwrap();
    assert_eq!(get_value_summary(&db.conn).unwrap().collection_total, 50000);

    let b = firearm_ops::create_firearm(&db.conn, &valued("Sig", 30000), false).unwrap();
    assert_eq!(get_value_summary(&db.conn).unwrap().collection_total, 80000);

    firearm_ops::update_firearm(&db.conn, a.id, &valued("Glock", 60000), false).unwrap();
    assert_eq!(get_value_summary(&db.conn).unwrap().collection_total, 90000);

    firearm_ops::dispose_firearm(
        &db.conn,
        a.id,
        &DisposeInput {
            disposition_type: DispositionType::Sold,
            recipient: "Jane".into(),
            date: "2025-01-01".into(),
            price: 55000,
            with_mounted: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(
        get_value_summary(&db.conn).unwrap().collection_total,
        30000,
        "disposed firearms are excluded from the active total (FR-025)"
    );

    firearm_ops::delete_firearm(&db.conn, b.id, true).unwrap();
    assert_eq!(get_value_summary(&db.conn).unwrap().collection_total, 0);
}

#[test]
fn scenario_6_breaks_down_into_the_blanket_policy_scheduled_firearms_and_uninsured() {
    let db = TestDb::new();
    let blanket = insurance_ops::create_policy(
        &db.conn,
        &policy("Blanket", "2026-01-01", "2026-12-31", Some(60_000)),
    )
    .unwrap();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2026-01-01", "2026-12-31", None))
            .unwrap();

    let scheduled = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    insurance_ops::assign_firearm_coverage(&db.conn, scheduled.id, Some(rider.id), Some(40_000))
        .unwrap();
    firearm_ops::create_firearm(&db.conn, &valued("Sig", 30_000), false).unwrap();
    firearm_ops::create_firearm(&db.conn, &valued("Ruger", 40_000), false).unwrap();

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();

    assert_eq!(summary.collection_total, 120_000);
    let b = summary.blanket.expect("a blanket policy is in force");
    assert_eq!((b.policy_id, b.policy_name.as_str()), (blanket.id, "Blanket"));
    assert_eq!((b.limit, b.total, b.firearm_count), (60_000, 70_000, 2));
    assert!(b.under_insured, "the unscheduled firearms are worth more than the limit");

    assert_eq!(summary.by_policy.len(), 1, "only policies with scheduled firearms are listed");
    let r = &summary.by_policy[0];
    assert_eq!(r.policy_id, rider.id);
    assert_eq!(r.individually_scheduled.len(), 1);
    let entry = &r.individually_scheduled[0];
    assert_eq!(
        (entry.record, entry.estimated_value, entry.scheduled_amount, entry.under_insured),
        (RecordRef::Firearm(scheduled.id), 50_000, 40_000, true)
    );

    assert!(summary.uninsured.is_empty(), "a blanket policy covers the rest");
}

#[test]
fn with_no_blanket_policy_in_force_unscheduled_firearms_are_the_uninsured_group() {
    let db = TestDb::new();
    let a = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    let b = firearm_ops::create_firearm(&db.conn, &valued("Sig", 30_000), false).unwrap();

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();

    assert!(summary.blanket.is_none());
    let mut ids: Vec<_> =
        summary.uninsured.iter().map(|u| (u.record.id(), u.estimated_value)).collect();
    ids.sort();
    assert_eq!(ids, vec![(a.id, 50_000), (b.id, 30_000)]);
    assert!(summary.uninsured.iter().all(|u| matches!(u.record, RecordRef::Firearm(_))));
}

#[test]
fn scenario_13_entering_a_blanket_policy_moves_the_firearms_out_of_uninsured_without_editing_them()
{
    let db = TestDb::new();
    firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    assert_eq!(get_value_summary_as_of(&db.conn, date(TODAY)).unwrap().uninsured.len(), 1);

    insurance_ops::create_policy(
        &db.conn,
        &policy("Blanket", "2026-01-01", "2026-12-31", Some(100_000)),
    )
    .unwrap();

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();
    assert!(summary.uninsured.is_empty());
    assert!(!summary.blanket.unwrap().under_insured);
}

#[test]
fn a_policy_with_scheduled_firearms_reports_its_expiry_state() {
    let db = TestDb::new();
    let expired = insurance_ops::create_policy(
        &db.conn,
        &policy("Old rider", "2025-01-01", "2026-05-01", None),
    )
    .unwrap();
    let f = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    insurance_ops::assign_firearm_coverage(&db.conn, f.id, Some(expired.id), Some(90_000)).unwrap();

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();

    let entry = &summary.by_policy[0];
    assert!(entry.is_expired);
    assert!(
        entry.individually_scheduled[0].under_insured,
        "a firearm on an expired policy is counted as uninsured whatever its amount"
    );
}

#[test]
fn disposed_firearms_leave_every_part_of_the_summary() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2026-01-01", "2026-12-31", None))
            .unwrap();
    let f = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    insurance_ops::assign_firearm_coverage(&db.conn, f.id, Some(rider.id), Some(50_000)).unwrap();
    firearm_ops::dispose_firearm(
        &db.conn,
        f.id,
        &DisposeInput {
            disposition_type: DispositionType::Sold,
            recipient: "Jane".into(),
            date: "2026-02-01".into(),
            price: 1,
            with_mounted: Vec::new(),
        },
    )
    .unwrap();

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();
    assert_eq!(summary.collection_total, 0);
    assert!(summary.by_policy.is_empty());
    assert!(summary.uninsured.is_empty());
}

// --- Accessories (specs/006-accessory-links US1-7, US1-8, FR-008, FR-009, SC-005) --
//
// Accessory inputs are built from the IPC shape (`AccessoryInput`'s camelCase
// JSON), so these tests do not depend on how the struct is spelled.

fn parse<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("the JSON must fit the type")
}

fn accessory_json(model: &str, value: Option<i64>) -> Value {
    json!({
        "accessoryKindId": 1,
        "make": "Leupold",
        "model": model,
        "estimatedValue": value,
        "status": "active",
    })
}

fn create_accessory(db: &TestDb, model: &str, value: Option<i64>) -> i64 {
    accessory_ops::create_accessory(&db.conn, &parse(accessory_json(model, value))).unwrap().id
}

fn dispose_accessory(db: &TestDb, id: i64) {
    accessory_ops::dispose_accessory(
        &db.conn,
        id,
        &parse(json!({
            "dispositionType": "sold",
            "recipient": "Jane",
            "date": "2026-02-01",
            "price": 1,
        })),
    )
    .unwrap();
}

#[test]
fn the_collection_total_is_the_firearms_subtotal_plus_the_accessories_subtotal() {
    let db = TestDb::new();
    firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    create_accessory(&db, "VX-5HD", Some(1000));
    create_accessory(&db, "Aimpoint", Some(180));
    create_accessory(&db, "No value", None);

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();

    assert_eq!(summary.accessories_total, 1180, "active accessories of 1000 and 180");
    assert_eq!(summary.firearms_total, 50_000);
    assert_eq!(summary.collection_total, summary.firearms_total + summary.accessories_total);
    assert_eq!(summary.collection_total, 51_180);
}

#[test]
fn an_accessories_only_collection_has_a_total_and_no_firearms() {
    let db = TestDb::new();
    create_accessory(&db, "VX-5HD", Some(1000));

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();

    assert_eq!((summary.firearms_total, summary.accessories_total), (0, 1000));
    assert_eq!(summary.collection_total, 1000);
}

#[test]
fn the_total_follows_every_accessory_change_and_counts_active_records_only() {
    let db = TestDb::new();
    let a = create_accessory(&db, "VX-5HD", Some(1000));
    let b = create_accessory(&db, "Aimpoint", Some(180));
    let total = |db: &TestDb| get_value_summary(&db.conn).unwrap().accessories_total;
    assert_eq!(total(&db), 1180);

    accessory_ops::update_accessory(&db.conn, a, &parse(accessory_json("VX-5HD", Some(1500))))
        .unwrap();
    assert_eq!(total(&db), 1680);

    dispose_accessory(&db, a);
    assert_eq!(total(&db), 180, "a disposed accessory counts nowhere");
    assert_eq!(get_value_summary(&db.conn).unwrap().collection_total, 180);

    accessory_ops::reverse_accessory_disposition(
        &db.conn,
        a,
        &parse(json!({ "history": "discard" })),
    )
    .unwrap();
    assert_eq!(total(&db), 1680, "restoring brings it back");

    accessory_ops::delete_accessory(&db.conn, b, true).unwrap();
    assert_eq!(total(&db), 1500);
}

#[test]
fn the_blanket_total_includes_unscheduled_active_accessories_with_a_count_of_each() {
    let db = TestDb::new();
    let blanket = insurance_ops::create_policy(
        &db.conn,
        &policy("Blanket", "2026-01-01", "2026-12-31", Some(60_000)),
    )
    .unwrap();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2026-01-01", "2026-12-31", None))
            .unwrap();
    firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    let scheduled_firearm =
        firearm_ops::create_firearm(&db.conn, &valued("Sig", 9_000), false).unwrap();
    insurance_ops::assign_firearm_coverage(
        &db.conn,
        scheduled_firearm.id,
        Some(rider.id),
        Some(9_000),
    )
    .unwrap();
    create_accessory(&db, "VX-5HD", Some(1000));
    create_accessory(&db, "Aimpoint", Some(180));
    let scheduled_accessory = create_accessory(&db, "Eotech", Some(500));
    insurance_ops::assign_accessory_coverage(
        &db.conn,
        scheduled_accessory,
        Some(rider.id),
        Some(500),
    )
    .unwrap();
    let gone = create_accessory(&db, "Sold", Some(7_000));
    dispose_accessory(&db, gone);

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();

    let b = summary.blanket.expect("a blanket policy is in force");
    assert_eq!(b.policy_id, blanket.id);
    assert_eq!(b.total, 50_000 + 1000 + 180, "unscheduled active firearms and accessories");
    assert_eq!((b.firearm_count, b.accessory_count), (1, 2));
    assert!(!b.under_insured, "51,180 is within the 60,000 limit");
    // The scheduled and the disposed records are in none of it.
    assert_eq!(summary.collection_total, 50_000 + 9_000 + 1000 + 180 + 500);
}

#[test]
fn a_blanket_total_over_the_limit_counts_the_accessories_that_push_it_there() {
    let db = TestDb::new();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Blanket", "2026-01-01", "2026-12-31", Some(1_100)),
    )
    .unwrap();
    create_accessory(&db, "VX-5HD", Some(1000));

    let within = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap().blanket.unwrap();
    assert_eq!((within.total, within.under_insured), (1000, false));

    create_accessory(&db, "Aimpoint", Some(180));

    let over = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap().blanket.unwrap();
    assert_eq!((over.total, over.limit, over.under_insured), (1180, 1_100, true));
    assert_eq!((over.firearm_count, over.accessory_count), (0, 2));
}

#[test]
fn a_scheduled_accessory_is_listed_under_its_policy_by_record_and_checked_against_its_value() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2026-01-01", "2026-12-31", None))
            .unwrap();
    let optic = create_accessory(&db, "VX-5HD", Some(1000));
    let light = create_accessory(&db, "Light", Some(200));
    insurance_ops::assign_accessory_coverage(&db.conn, optic, Some(rider.id), Some(600)).unwrap();
    insurance_ops::assign_accessory_coverage(&db.conn, light, Some(rider.id), Some(200)).unwrap();

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();

    assert_eq!(summary.by_policy.len(), 1);
    let listed = &summary.by_policy[0].individually_scheduled;
    assert_eq!(listed.len(), 2);
    let under = |id| {
        let entry = listed.iter().find(|e| e.record == RecordRef::Accessory(id)).unwrap();
        (entry.estimated_value, entry.scheduled_amount, entry.under_insured)
    };
    assert_eq!(under(optic), (1000, 600, true), "scheduled below its value");
    assert_eq!(under(light), (200, 200, false));
    assert!(summary.uninsured.is_empty());
    assert_eq!(summary.collection_total, 1200);
}

#[test]
fn an_accessory_with_no_policy_in_force_is_uninsured_by_record() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    let accessory = create_accessory(&db, "VX-5HD", Some(1000));
    create_accessory(&db, "No value", None);

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();

    assert!(summary.blanket.is_none());
    let mut uninsured: Vec<_> =
        summary.uninsured.iter().map(|u| (u.record, u.estimated_value)).collect();
    uninsured.sort_by_key(|(record, _)| (record.kind() as u8, record.id()));
    assert_eq!(
        uninsured,
        vec![(RecordRef::Firearm(firearm.id), 50_000), (RecordRef::Accessory(accessory), 1000)],
        "a record with no value is not listed"
    );
}

#[test]
fn a_disposed_accessory_leaves_every_part_of_the_summary() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2026-01-01", "2026-12-31", None))
            .unwrap();
    let scheduled = create_accessory(&db, "VX-5HD", Some(1000));
    insurance_ops::assign_accessory_coverage(&db.conn, scheduled, Some(rider.id), Some(1000))
        .unwrap();
    let unscheduled = create_accessory(&db, "Aimpoint", Some(180));
    dispose_accessory(&db, scheduled);
    dispose_accessory(&db, unscheduled);

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();

    assert_eq!(
        (summary.firearms_total, summary.accessories_total, summary.collection_total),
        (0, 0, 0)
    );
    assert!(summary.by_policy.is_empty());
    assert!(summary.uninsured.is_empty());
}

#[test]
fn the_wire_format_of_the_summary_names_the_records_and_both_subtotals() {
    let db = TestDb::new();
    let accessory = create_accessory(&db, "VX-5HD", Some(1000));

    let wire =
        serde_json::to_value(get_value_summary_as_of(&db.conn, date(TODAY)).unwrap()).unwrap();

    assert_eq!(wire["firearmsTotal"], json!(0));
    assert_eq!(wire["accessoriesTotal"], json!(1000));
    assert_eq!(wire["collectionTotal"], json!(1000));
    assert_eq!(
        wire["uninsured"],
        json!([{ "record": { "kind": "accessory", "id": accessory }, "estimatedValue": 1000 }])
    );
}
