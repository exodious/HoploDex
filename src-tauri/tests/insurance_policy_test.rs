//! Integration tests for InsurancePolicy CRUD and the delete-blocked-while-
//! assigned behavior (data-model.md's "Validation rules", Edge Cases),
//! run against a real temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::firearm::{CoverageKind, FirearmInput, FirearmStatus};
use hoplodex_lib::models::insurance_policy::InsurancePolicyInput;
use support::TestDb;

fn sample_policy() -> InsurancePolicyInput {
    InsurancePolicyInput {
        name: "Homeowners Rider".into(),
        policy_number: "POL-1".into(),
        insurance_company: "Acme Insurance".into(),
        company_contact: None,
        agent_name: None,
        agent_contact: None,
        blanket_coverage_limit: 500_000,
        effective_start_date: "2025-01-01".into(),
        effective_end_date: "2026-01-01".into(),
    }
}

fn sample_firearm() -> FirearmInput {
    FirearmInput {
        make: "Glock".into(),
        model: "19".into(),
        serial_number: Some("INS-1".into()),
        no_serial_attested: false,
        caliber: "9mm".into(),
        firearm_type_id: 1,
        notes: None,
        accessories: None,
        status: FirearmStatus::Active,
        estimated_value: Some(50000),
        acquisition_source: None,
        acquisition_date: None,
        acquisition_price: None,
        disposition_type: None,
        disposition_recipient: None,
        disposition_date: None,
        disposition_price: None,
        insurance_policy_id: None,
        coverage_kind: None,
        scheduled_coverage_amount: None,
    }
}

#[test]
fn creates_and_updates_a_policy() {
    let db = TestDb::new();
    let created = insurance_ops::create_policy(&db.conn, &sample_policy()).unwrap();
    assert_eq!(created.name, "Homeowners Rider");
    assert_eq!(created.blanket_coverage_limit, 500_000);

    let mut edited = sample_policy();
    edited.name = "Homeowners Rider (updated)".into();
    let updated = insurance_ops::update_policy(&db.conn, created.id, &edited).unwrap();
    assert_eq!(updated.name, "Homeowners Rider (updated)");
}

#[test]
fn rejects_an_end_date_not_after_the_start_date() {
    let db = TestDb::new();
    let mut invalid = sample_policy();
    invalid.effective_end_date = "2024-01-01".into();

    let err = insurance_ops::create_policy(&db.conn, &invalid).expect_err("should be rejected");
    assert_eq!(err.code, "VALIDATION_ERROR");
}

#[test]
fn deleting_a_policy_with_assigned_firearms_is_blocked() {
    let db = TestDb::new();
    let policy = insurance_ops::create_policy(&db.conn, &sample_policy()).unwrap();

    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm()).unwrap();
    insurance_ops::assign_firearm_coverage(
        &db.conn,
        firearm.id,
        Some(policy.id),
        Some(CoverageKind::IndividuallyScheduled),
        Some(60000),
    )
    .unwrap();

    let err = insurance_ops::delete_policy(&db.conn, policy.id, true)
        .expect_err("delete must be blocked while firearms are assigned");
    assert_eq!(err.code, "POLICY_HAS_FIREARMS");

    // Unassign, then deletion succeeds.
    insurance_ops::assign_firearm_coverage(&db.conn, firearm.id, None, None, None).unwrap();
    let result = insurance_ops::delete_policy(&db.conn, policy.id, true).unwrap();
    assert!(result.deleted);
}

#[test]
fn assign_firearm_coverage_sets_and_clears_coverage() {
    let db = TestDb::new();
    let policy = insurance_ops::create_policy(&db.conn, &sample_policy()).unwrap();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm()).unwrap();

    let assigned = insurance_ops::assign_firearm_coverage(
        &db.conn,
        firearm.id,
        Some(policy.id),
        Some(CoverageKind::Blanket),
        None,
    )
    .unwrap();
    assert_eq!(assigned.insurance_policy_id, Some(policy.id));
    assert_eq!(assigned.coverage_kind, Some(CoverageKind::Blanket));

    let cleared =
        insurance_ops::assign_firearm_coverage(&db.conn, firearm.id, None, None, None).unwrap();
    assert_eq!(cleared.insurance_policy_id, None);
    assert_eq!(cleared.coverage_kind, None);
}
