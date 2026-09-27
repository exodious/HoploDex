//! Integration tests for `services::valuation`'s always-current value
//! summary (spec.md US3 Acceptance Scenarios 5-6, 11, 13; FR-015/036), run
//! against a real temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::{DisposeFirearmInput, ops as firearm_ops};
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput};
use hoplodex_lib::services::valuation::{get_value_summary, get_value_summary_as_of};
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
        &DisposeFirearmInput {
            disposition_type: DispositionType::Sold,
            recipient: "Jane".into(),
            date: "2025-01-01".into(),
            price: 55000,
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
        (entry.firearm_id, entry.estimated_value, entry.scheduled_amount, entry.under_insured),
        (scheduled.id, 50_000, 40_000, true)
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
        summary.uninsured.iter().map(|u| (u.firearm_id, u.estimated_value)).collect();
    ids.sort();
    assert_eq!(ids, vec![(a.id, 50_000), (b.id, 30_000)]);
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
        &DisposeFirearmInput {
            disposition_type: DispositionType::Sold,
            recipient: "Jane".into(),
            date: "2026-02-01".into(),
            price: 1,
        },
    )
    .unwrap();

    let summary = get_value_summary_as_of(&db.conn, date(TODAY)).unwrap();
    assert_eq!(summary.collection_total, 0);
    assert!(summary.by_policy.is_empty());
    assert!(summary.uninsured.is_empty());
}
