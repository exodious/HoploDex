//! Integration tests for `services::valuation`'s always-current value
//! summary (spec.md US3 Acceptance Scenarios 5-6; FR-015), run against a
//! real temporary SQLCipher database.

mod support;

use chrono::{Duration, Local};
use hoplodex_lib::commands::firearms::{ops as firearm_ops, DisposeFirearmInput};
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::firearm::{CoverageKind, DispositionType, FirearmInput, FirearmStatus};
use hoplodex_lib::models::insurance_policy::InsurancePolicyInput;
use hoplodex_lib::services::valuation::get_value_summary;
use support::TestDb;

fn firearm(make: &str, value: i64) -> FirearmInput {
    FirearmInput {
        make: make.into(),
        model: "M".into(),
        serial_number: Some(format!("{make}-SN")),
        no_serial_attested: false,
        caliber: "9mm".into(),
        firearm_type_id: 1,
        notes: None,
        accessories: None,
        status: FirearmStatus::Active,
        estimated_value: Some(value),
        acquisition_source: None,
        acquisition_date: None,
        acquisition_price: None,
        disposition_type: None,
        disposition_recipient: None,
        disposition_date: None,
        disposition_price: None,
        insurance_policy_id: None,
        coverage_kind: None,
        nickname: None,
        scheduled_coverage_amount: None,
    }
}

fn far_future_policy(name: &str) -> InsurancePolicyInput {
    let end = (Local::now().date_naive() + Duration::days(365)).format("%Y-%m-%d").to_string();
    InsurancePolicyInput {
        name: name.into(),
        policy_number: "P".into(),
        insurance_company: "Acme".into(),
        company_contact: None,
        agent_name: None,
        agent_contact: None,
        blanket_coverage_limit: 1_000_000,
        effective_start_date: "2020-01-01".into(),
        effective_end_date: end,
    }
}

#[test]
fn scenario_5_total_updates_immediately_after_every_mutation() {
    let db = TestDb::new();

    let initial = get_value_summary(&db.conn).unwrap();
    assert_eq!(initial.collection_total, 0);

    let a = firearm_ops::create_firearm(&db.conn, &firearm("Glock", 50000)).unwrap();
    assert_eq!(get_value_summary(&db.conn).unwrap().collection_total, 50000);

    let b = firearm_ops::create_firearm(&db.conn, &firearm("Sig", 30000)).unwrap();
    assert_eq!(get_value_summary(&db.conn).unwrap().collection_total, 80000);

    let mut edited = firearm("Glock", 60000);
    firearm_ops::update_firearm(&db.conn, a.id, &edited).unwrap();
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

    // silence unused-variable warning for edited (input reused above)
    let _ = &mut edited;
}

#[test]
fn scenario_6_breaks_down_by_policy_and_unassigned_group() {
    let db = TestDb::new();
    let policy_a = insurance_ops::create_policy(&db.conn, &far_future_policy("Policy A")).unwrap();
    let policy_b = insurance_ops::create_policy(&db.conn, &far_future_policy("Policy B")).unwrap();

    let f1 = firearm_ops::create_firearm(&db.conn, &firearm("Glock", 50000)).unwrap();
    insurance_ops::assign_firearm_coverage(
        &db.conn,
        f1.id,
        Some(policy_a.id),
        Some(CoverageKind::IndividuallyScheduled),
        Some(50000),
    )
    .unwrap();

    let f2 = firearm_ops::create_firearm(&db.conn, &firearm("Sig", 30000)).unwrap();
    insurance_ops::assign_firearm_coverage(
        &db.conn,
        f2.id,
        Some(policy_b.id),
        Some(CoverageKind::Blanket),
        None,
    )
    .unwrap();

    let f3 = firearm_ops::create_firearm(&db.conn, &firearm("Ruger", 20000)).unwrap();

    let summary = get_value_summary(&db.conn).unwrap();
    assert_eq!(summary.collection_total, 100000);
    assert_eq!(summary.by_policy.len(), 2);
    assert_eq!(summary.unassigned.len(), 1);
    assert_eq!(summary.unassigned[0].firearm_id, f3.id);

    let a_summary = summary.by_policy.iter().find(|p| p.policy_id == policy_a.id).unwrap();
    assert_eq!(a_summary.individually_scheduled.len(), 1);
    assert_eq!(a_summary.individually_scheduled[0].firearm_id, f1.id);

    let b_summary = summary.by_policy.iter().find(|p| p.policy_id == policy_b.id).unwrap();
    assert_eq!(b_summary.blanket_total, 30000);
}
