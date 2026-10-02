//! Integration tests for InsurancePolicy create/update — the optional
//! blanket limit and the at-most-one-blanket-policy-in-force rule (FR-027,
//! FR-036, spec.md US3 Acceptance Scenario 12) — and schedule-only coverage
//! assignment (FR-014), run against a real temporary SQLCipher database.
//! Deleting a policy is covered in `policy_deletion_test.rs`.

mod support;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use support::{TestDb, firearm, policy};

fn blanket(
    name: &str,
    start: &str,
    end: &str,
) -> hoplodex_lib::models::insurance_policy::InsurancePolicyInput {
    policy(name, start, end, Some(500_000))
}

#[test]
fn creates_and_updates_a_policy() {
    let db = TestDb::new();
    let created =
        insurance_ops::create_policy(&db.conn, &blanket("Homeowners", "2025-01-01", "2026-01-01"))
            .unwrap();
    assert_eq!(created.name, "Homeowners");
    assert_eq!(created.blanket_coverage_limit, Some(500_000));

    let mut edited = blanket("Homeowners (updated)", "2025-01-01", "2026-01-01");
    edited.policy_number = "POL-2".into();
    let updated = insurance_ops::update_policy(&db.conn, created.id, &edited).unwrap();
    assert_eq!(updated.name, "Homeowners (updated)");
    assert_eq!(updated.policy_number, "POL-2");
}

#[test]
fn the_blanket_limit_is_optional_so_a_policy_can_be_schedule_only() {
    let db = TestDb::new();
    let created = insurance_ops::create_policy(
        &db.conn,
        &policy("Collectibles rider", "2025-01-01", "2026-01-01", None),
    )
    .unwrap();

    assert_eq!(created.blanket_coverage_limit, None);
    let reloaded = insurance_ops::get_policy(&db.conn, created.id).unwrap();
    assert_eq!(reloaded.blanket_coverage_limit, None);
}

#[test]
fn a_negative_blanket_limit_is_rejected() {
    let db = TestDb::new();
    let err = insurance_ops::create_policy(
        &db.conn,
        &policy("Bad", "2025-01-01", "2026-01-01", Some(-1)),
    )
    .expect_err("negative limit");
    assert_eq!(err.code, "VALIDATION_ERROR");
}

#[test]
fn rejects_an_end_date_not_after_the_start_date() {
    let db = TestDb::new();
    let err = insurance_ops::create_policy(&db.conn, &blanket("Bad", "2025-01-01", "2024-01-01"))
        .expect_err("should be rejected");
    assert_eq!(err.code, "VALIDATION_ERROR");
}

#[test]
fn scenario_12_overlapping_blanket_policies_are_blocked_naming_the_other_policy() {
    let db = TestDb::new();
    insurance_ops::create_policy(&db.conn, &blanket("Old policy", "2025-01-01", "2026-01-01"))
        .unwrap();

    let err =
        insurance_ops::create_policy(&db.conn, &blanket("New policy", "2025-12-01", "2026-12-01"))
            .expect_err("a month of overlap");

    assert_eq!(err.code, "VALIDATION_ERROR");
    let text = format!("{} {:?}", err.message, err.field_errors);
    assert!(text.contains("Old policy"), "the message names the other policy: {text}");
    assert_eq!(insurance_ops::list_policies(&db.conn).unwrap().len(), 1, "nothing was saved");
}

#[test]
fn scenario_12_a_shared_boundary_day_is_accepted() {
    let db = TestDb::new();
    insurance_ops::create_policy(&db.conn, &blanket("Old", "2025-01-01", "2026-01-01")).unwrap();

    // The day one policy ends is the day the next begins.
    let renewal =
        insurance_ops::create_policy(&db.conn, &blanket("Renewal", "2026-01-01", "2027-01-01"));
    assert!(renewal.is_ok(), "{renewal:?}");
}

#[test]
fn more_than_one_shared_day_is_an_overlap() {
    let db = TestDb::new();
    insurance_ops::create_policy(&db.conn, &blanket("Old", "2025-01-01", "2026-01-02")).unwrap();

    let err =
        insurance_ops::create_policy(&db.conn, &blanket("Renewal", "2026-01-01", "2027-01-01"))
            .expect_err("two days shared");
    assert_eq!(err.code, "VALIDATION_ERROR");
}

#[test]
fn a_policy_that_fully_contains_or_sits_inside_another_overlaps_it() {
    let db = TestDb::new();
    insurance_ops::create_policy(&db.conn, &blanket("Middle", "2025-06-01", "2025-09-01")).unwrap();

    assert!(
        insurance_ops::create_policy(&db.conn, &blanket("Wider", "2025-01-01", "2026-01-01"))
            .is_err()
    );
    assert!(
        insurance_ops::create_policy(&db.conn, &blanket("Inside", "2025-07-01", "2025-08-01"))
            .is_err()
    );
}

#[test]
fn schedule_only_policies_never_conflict_with_a_blanket_policy() {
    let db = TestDb::new();
    insurance_ops::create_policy(&db.conn, &blanket("Blanket", "2025-01-01", "2026-01-01"))
        .unwrap();

    let rider = policy("Rider", "2025-03-01", "2025-12-01", None);
    assert!(insurance_ops::create_policy(&db.conn, &rider).is_ok());
    let another = policy("Another rider", "2025-03-01", "2025-12-01", None);
    assert!(insurance_ops::create_policy(&db.conn, &another).is_ok());
}

#[test]
fn an_edit_that_would_create_an_overlap_is_blocked() {
    let db = TestDb::new();
    let first =
        insurance_ops::create_policy(&db.conn, &blanket("First", "2025-01-01", "2026-01-01"))
            .unwrap();
    let second =
        insurance_ops::create_policy(&db.conn, &blanket("Second", "2026-01-01", "2027-01-01"))
            .unwrap();

    // Renewal: pushing the first policy's end date into the second.
    let err = insurance_ops::update_policy(
        &db.conn,
        first.id,
        &blanket("First", "2025-01-01", "2026-06-01"),
    )
    .expect_err("extends into the second policy");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(format!("{err:?}").contains("Second"));

    // Moving the second policy's start back into the first.
    assert!(
        insurance_ops::update_policy(
            &db.conn,
            second.id,
            &blanket("Second", "2025-06-01", "2027-01-01")
        )
        .is_err()
    );

    // Unchanged dates, or other edits, are fine: a policy never conflicts with itself.
    assert!(
        insurance_ops::update_policy(
            &db.conn,
            first.id,
            &blanket("First (renamed)", "2025-01-01", "2026-01-01")
        )
        .is_ok()
    );
}

#[test]
fn turning_a_schedule_only_policy_into_a_blanket_one_is_checked_too() {
    let db = TestDb::new();
    insurance_ops::create_policy(&db.conn, &blanket("Blanket", "2025-01-01", "2026-01-01"))
        .unwrap();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2025-03-01", "2025-12-01", None))
            .unwrap();

    let promoted = policy("Rider", "2025-03-01", "2025-12-01", Some(100_000));
    let err = insurance_ops::update_policy(&db.conn, rider.id, &promoted).expect_err("overlap");
    assert_eq!(err.code, "VALIDATION_ERROR");
}

#[test]
fn assign_firearm_coverage_schedules_a_firearm_and_null_unschedules_it() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2025-01-01", "2026-01-01", None))
            .unwrap();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "INS-1"), false, None)
            .unwrap();

    let scheduled =
        insurance_ops::assign_firearm_coverage(&db.conn, created.id, Some(rider.id), Some(60_000))
            .unwrap();
    assert_eq!(scheduled.insurance_policy_id, Some(rider.id));
    assert_eq!(scheduled.scheduled_coverage_amount, Some(60_000));

    let unscheduled =
        insurance_ops::assign_firearm_coverage(&db.conn, created.id, None, None).unwrap();
    assert_eq!(unscheduled.insurance_policy_id, None);
    assert_eq!(unscheduled.scheduled_coverage_amount, None);
}

#[test]
fn scheduling_requires_an_amount_and_an_existing_policy() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2025-01-01", "2026-01-01", None))
            .unwrap();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "INS-1"), false, None)
            .unwrap();

    let no_amount =
        insurance_ops::assign_firearm_coverage(&db.conn, created.id, Some(rider.id), None)
            .expect_err("a scheduled firearm needs its own amount");
    assert_eq!(no_amount.code, "VALIDATION_ERROR");

    let no_policy =
        insurance_ops::assign_firearm_coverage(&db.conn, created.id, Some(9999), Some(1000))
            .expect_err("unknown policy");
    assert_eq!(no_policy.code, "NOT_FOUND");
}

// --- Whole-dollar amounts (FR-037) ---

#[test]
fn a_negative_scheduled_amount_is_rejected_with_a_field_error() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2025-01-01", "2026-01-01", None))
            .unwrap();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "NEG-1"), false, None)
            .unwrap();

    let err =
        insurance_ops::assign_firearm_coverage(&db.conn, created.id, Some(rider.id), Some(-60_000))
            .expect_err("negative amount");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(err.field_errors.as_ref().unwrap().contains_key("scheduledCoverageAmount"));
    assert_eq!(firearm_ops::get_firearm(&db.conn, created.id).unwrap().insurance_policy_id, None);
}

#[test]
fn a_negative_blanket_limit_names_the_field_on_create_and_update() {
    let db = TestDb::new();
    let err = insurance_ops::create_policy(
        &db.conn,
        &policy("Bad", "2025-01-01", "2026-01-01", Some(-1)),
    )
    .expect_err("negative limit");
    assert!(err.field_errors.as_ref().unwrap().contains_key("blanketCoverageLimit"));

    let created =
        insurance_ops::create_policy(&db.conn, &blanket("Good", "2025-01-01", "2026-01-01"))
            .unwrap();
    let err = insurance_ops::update_policy(
        &db.conn,
        created.id,
        &policy("Good", "2025-01-01", "2026-01-01", Some(-1)),
    )
    .expect_err("negative limit");
    assert!(err.field_errors.as_ref().unwrap().contains_key("blanketCoverageLimit"));
}

#[test]
fn a_blanket_limit_of_zero_is_allowed_and_stored_as_dollars() {
    let db = TestDb::new();
    let created = insurance_ops::create_policy(
        &db.conn,
        &policy("Zero", "2025-01-01", "2026-01-01", Some(0)),
    )
    .unwrap();
    assert_eq!(created.blanket_coverage_limit, Some(0));

    let stored: Option<i64> = db
        .conn
        .query_row(
            "SELECT blanket_coverage_limit FROM insurance_policies WHERE id = ?1",
            [created.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, Some(0));
}

#[test]
fn a_fractional_amount_is_refused_when_the_arguments_are_decoded_never_rounded() {
    use hoplodex_lib::models::insurance_policy::InsurancePolicyInput;

    let mut input = serde_json::json!({
        "name": "P", "policyNumber": "1", "insuranceCompany": "Acme",
        "effectiveStartDate": "2025-01-01", "effectiveEndDate": "2026-01-01",
        "blanketCoverageLimit": 500000
    });
    assert!(serde_json::from_value::<InsurancePolicyInput>(input.clone()).is_ok());

    input["blanketCoverageLimit"] = serde_json::json!(500000.5);
    assert!(serde_json::from_value::<InsurancePolicyInput>(input).is_err());
}

#[test]
fn the_database_itself_refuses_a_negative_limit_or_scheduled_amount() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2025-01-01", "2026-01-01", None))
            .unwrap();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "CHK-1"), false, None)
            .unwrap();

    assert!(
        db.conn
            .execute(
                "UPDATE insurance_policies SET blanket_coverage_limit = -1 WHERE id = ?1",
                [rider.id]
            )
            .is_err()
    );
    assert!(
        db.conn
            .execute(
                "UPDATE firearms SET insurance_policy_id = ?1, scheduled_coverage_amount = -1
             WHERE id = ?2",
                [rider.id, created.id]
            )
            .is_err()
    );
}

/// FR-027 / US3 Acceptance Scenario 16: a policy carries optional free-form
/// notes; a blank entry is stored as no notes.
#[test]
fn notes_are_stored_and_returned_from_create_update_and_list() {
    let db = TestDb::new();
    let mut input = blanket("Homeowners", "2025-01-01", "2026-01-01");
    input.notes = Some("  Renews in January.\nAsk about the rider.  ".into());

    let created = insurance_ops::create_policy(&db.conn, &input).unwrap();
    assert_eq!(created.notes.as_deref(), Some("Renews in January.\nAsk about the rider."));

    input.notes = Some("Renewal quote received.".into());
    let updated = insurance_ops::update_policy(&db.conn, created.id, &input).unwrap();
    assert_eq!(updated.notes.as_deref(), Some("Renewal quote received."));

    let listed = insurance_ops::list_policy_views(&db.conn).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].policy.notes.as_deref(), Some("Renewal quote received."));
}

#[test]
fn blank_or_whitespace_only_notes_are_stored_as_null() {
    let db = TestDb::new();
    let mut input = policy("Rider", "2025-01-01", "2026-01-01", None);

    for blank in ["", "   ", "\n\t "] {
        input.notes = Some(blank.into());
        let created = insurance_ops::create_policy(&db.conn, &input).unwrap();
        assert_eq!(created.notes, None, "{blank:?} on create should be stored as null");

        input.notes = Some("Something".into());
        insurance_ops::update_policy(&db.conn, created.id, &input).unwrap();
        input.notes = Some(blank.into());
        let updated = insurance_ops::update_policy(&db.conn, created.id, &input).unwrap();
        assert_eq!(updated.notes, None, "{blank:?} on update should be stored as null");
    }
}

#[test]
fn notes_are_cleared_when_updated_with_null() {
    let db = TestDb::new();
    let mut input = blanket("Homeowners", "2025-01-01", "2026-01-01");
    input.notes = Some("Keep the original appraisal with this policy.".into());
    let created = insurance_ops::create_policy(&db.conn, &input).unwrap();
    assert!(created.notes.is_some());

    input.notes = None;
    let updated = insurance_ops::update_policy(&db.conn, created.id, &input).unwrap();
    assert_eq!(updated.notes, None);
    assert_eq!(insurance_ops::get_policy(&db.conn, created.id).unwrap().notes, None);
}
