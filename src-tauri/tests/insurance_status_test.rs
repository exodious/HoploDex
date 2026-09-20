//! Integration tests for `services::insurance_status`'s per-firearm and
//! per-policy warning calculations (spec.md US3 Acceptance Scenarios 1-4,
//! 7-8; FR-016/017/024/028), run against a real temporary SQLCipher
//! database.

mod support;

use chrono::{Duration, Local};
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::firearm::{CoverageKind, FirearmInput, FirearmStatus};
use hoplodex_lib::models::insurance_policy::InsurancePolicyInput;
use hoplodex_lib::services::insurance_status::{
    firearm_warning, load_policy_aggregates, InsuranceWarning,
};
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

fn policy_with_end_date(end_date: &str) -> InsurancePolicyInput {
    InsurancePolicyInput {
        name: "Test Policy".into(),
        policy_number: "P-1".into(),
        insurance_company: "Acme".into(),
        company_contact: None,
        agent_name: None,
        agent_contact: None,
        blanket_coverage_limit: 100_000,
        effective_start_date: "2020-01-01".into(),
        effective_end_date: end_date.into(),
    }
}

#[test]
fn scenario_1_no_policy_is_uninsured() {
    let db = TestDb::new();
    let created = firearm_ops::create_firearm(&db.conn, &firearm("Glock", 50000)).unwrap();

    let policies = load_policy_aggregates(&db.conn).unwrap();
    let warning = firearm_warning(created.estimated_value, None, None, None, &policies);
    assert_eq!(warning, InsuranceWarning::Uninsured);
}

#[test]
fn scenario_2_and_3_individually_scheduled_under_and_then_sufficiently_insured() {
    let db = TestDb::new();
    let far_future =
        (Local::now().date_naive() + Duration::days(365)).format("%Y-%m-%d").to_string();
    let policy =
        insurance_ops::create_policy(&db.conn, &policy_with_end_date(&far_future)).unwrap();
    let created = firearm_ops::create_firearm(&db.conn, &firearm("Glock", 50000)).unwrap();
    insurance_ops::assign_firearm_coverage(
        &db.conn,
        created.id,
        Some(policy.id),
        Some(CoverageKind::IndividuallyScheduled),
        Some(30000),
    )
    .unwrap();

    let policies = load_policy_aggregates(&db.conn).unwrap();
    let under = firearm_warning(
        Some(50000),
        Some(policy.id),
        Some(CoverageKind::IndividuallyScheduled),
        Some(30000),
        &policies,
    );
    assert_eq!(under, InsuranceWarning::UnderInsured, "scheduled amount below value");

    // Raise scheduled amount to meet the value: warning clears.
    insurance_ops::assign_firearm_coverage(
        &db.conn,
        created.id,
        Some(policy.id),
        Some(CoverageKind::IndividuallyScheduled),
        Some(50000),
    )
    .unwrap();
    let sufficient = firearm_warning(
        Some(50000),
        Some(policy.id),
        Some(CoverageKind::IndividuallyScheduled),
        Some(50000),
        &policies,
    );
    assert_eq!(sufficient, InsuranceWarning::None);
}

#[test]
fn scenario_4_blanket_combined_value_exceeding_limit_is_flagged() {
    let db = TestDb::new();
    let far_future =
        (Local::now().date_naive() + Duration::days(365)).format("%Y-%m-%d").to_string();
    let mut policy_input = policy_with_end_date(&far_future);
    policy_input.blanket_coverage_limit = 80000;
    let policy = insurance_ops::create_policy(&db.conn, &policy_input).unwrap();

    let a = firearm_ops::create_firearm(&db.conn, &firearm("Glock", 50000)).unwrap();
    let b = firearm_ops::create_firearm(&db.conn, &firearm("Sig", 50000)).unwrap();
    insurance_ops::assign_firearm_coverage(
        &db.conn,
        a.id,
        Some(policy.id),
        Some(CoverageKind::Blanket),
        None,
    )
    .unwrap();
    insurance_ops::assign_firearm_coverage(
        &db.conn,
        b.id,
        Some(policy.id),
        Some(CoverageKind::Blanket),
        None,
    )
    .unwrap();

    let policies = load_policy_aggregates(&db.conn).unwrap();
    let aggregate = policies.get(&policy.id).unwrap();
    assert_eq!(aggregate.blanket_total, 100000, "combined value of both blanket firearms");
    assert!(aggregate.blanket_total > aggregate.blanket_limit);

    let warning =
        firearm_warning(Some(50000), Some(policy.id), Some(CoverageKind::Blanket), None, &policies);
    assert_eq!(
        warning,
        InsuranceWarning::UnderInsured,
        "group blanket-exceeded flags each covered firearm"
    );
}

#[test]
fn scenario_7_policy_expiring_within_30_days_is_flagged() {
    let db = TestDb::new();
    let soon = (Local::now().date_naive() + Duration::days(15)).format("%Y-%m-%d").to_string();
    let policy = insurance_ops::create_policy(&db.conn, &policy_with_end_date(&soon)).unwrap();

    let policies = load_policy_aggregates(&db.conn).unwrap();
    let aggregate = policies.get(&policy.id).unwrap();
    assert!(aggregate.is_expiring_soon);
    assert!(!aggregate.is_expired);
}

#[test]
fn scenario_8_expired_policy_flags_assigned_firearms_uninsured() {
    let db = TestDb::new();
    let past = (Local::now().date_naive() - Duration::days(5)).format("%Y-%m-%d").to_string();
    let policy = insurance_ops::create_policy(&db.conn, &policy_with_end_date(&past)).unwrap();
    let created = firearm_ops::create_firearm(&db.conn, &firearm("Glock", 50000)).unwrap();
    insurance_ops::assign_firearm_coverage(
        &db.conn,
        created.id,
        Some(policy.id),
        // Scheduled at/above value - would otherwise be "sufficiently
        // insured" if not for the policy having expired.
        Some(CoverageKind::IndividuallyScheduled),
        Some(50000),
    )
    .unwrap();

    let policies = load_policy_aggregates(&db.conn).unwrap();
    let aggregate = policies.get(&policy.id).unwrap();
    assert!(aggregate.is_expired);

    let warning = firearm_warning(
        Some(50000),
        Some(policy.id),
        Some(CoverageKind::IndividuallyScheduled),
        Some(50000),
        &policies,
    );
    assert_eq!(
        warning,
        InsuranceWarning::Uninsured,
        "an expired policy overrides otherwise-sufficient coverage"
    );
}

#[test]
fn no_estimated_value_is_never_flagged() {
    let db = TestDb::new();
    let policies = load_policy_aggregates(&db.conn).unwrap();
    let warning = firearm_warning(None, None, None, None, &policies);
    assert_eq!(warning, InsuranceWarning::None);
}
