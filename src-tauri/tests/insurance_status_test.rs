//! Integration tests for `services::insurance_status` under implicit
//! blanket coverage (spec.md US3 Acceptance Scenarios 1-4, 7-8, 11, 13-14;
//! FR-016/017/024/028/036), run against a real temporary SQLCipher
//! database. "Today" is passed in, so boundary days and expiry are exact.

mod support;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::firearm::FirearmInput;
use hoplodex_lib::services::insurance_status::{
    InsuranceWarning, firearm_warning, load_context_as_of,
};
use support::{TestDb, date, firearm, policy};

const TODAY: &str = "2026-06-15";

fn valued(make: &str, value: i64) -> FirearmInput {
    FirearmInput { estimated_value: Some(value), ..firearm(make, "M", &format!("{make}-SN")) }
}

fn warning_for(db: &TestDb, firearm_id: i64, today: &str) -> InsuranceWarning {
    let f = firearm_ops::get_firearm(&db.conn, firearm_id).unwrap();
    let ctx = load_context_as_of(&db.conn, date(today)).unwrap();
    firearm_warning(f.estimated_value, f.insurance_policy_id, f.scheduled_coverage_amount, &ctx)
}

#[test]
fn scenario_1_and_13_with_no_blanket_policy_in_force_an_unscheduled_firearm_is_uninsured() {
    let db = TestDb::new();
    let f = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    assert_eq!(warning_for(&db, f.id, TODAY), InsuranceWarning::Uninsured);

    // A policy that has ended, or hasn't started, isn't in force.
    insurance_ops::create_policy(
        &db.conn,
        &policy("Past", "2024-01-01", "2025-01-01", Some(1_000_000)),
    )
    .unwrap();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Future", "2027-01-01", "2028-01-01", Some(1_000_000)),
    )
    .unwrap();
    assert_eq!(warning_for(&db, f.id, TODAY), InsuranceWarning::Uninsured);

    // Entering a policy whose dates include today clears the flag with no
    // edit to the firearm.
    insurance_ops::create_policy(
        &db.conn,
        &policy("Current", "2026-01-01", "2026-12-31", Some(1_000_000)),
    )
    .unwrap();
    assert_eq!(warning_for(&db, f.id, TODAY), InsuranceWarning::None);
}

#[test]
fn scenario_11_a_new_firearm_is_covered_by_the_blanket_policy_with_no_assignment_step() {
    let db = TestDb::new();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Blanket", "2026-01-01", "2026-12-31", Some(1_000_000)),
    )
    .unwrap();
    let f = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();

    assert_eq!(f.insurance_policy_id, None, "nothing is stored per firearm for blanket coverage");
    let ctx = load_context_as_of(&db.conn, date(TODAY)).unwrap();
    let blanket = ctx.blanket.expect("the policy is in force");
    assert_eq!(blanket.policy_name, "Blanket");
    assert_eq!(blanket.total, 50_000);
    assert_eq!(blanket.firearm_count, 1);
    assert_eq!(warning_for(&db, f.id, TODAY), InsuranceWarning::None);
}

#[test]
fn scenario_4_unscheduled_value_exceeding_the_blanket_limit_flags_every_unscheduled_firearm() {
    let db = TestDb::new();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Blanket", "2026-01-01", "2026-12-31", Some(80_000)),
    )
    .unwrap();
    let a = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    let b = firearm_ops::create_firearm(&db.conn, &valued("Sig", 50_000), false).unwrap();

    let ctx = load_context_as_of(&db.conn, date(TODAY)).unwrap();
    let blanket = ctx.blanket.as_ref().unwrap();
    assert_eq!((blanket.total, blanket.limit), (100_000, 80_000));
    assert_eq!(warning_for(&db, a.id, TODAY), InsuranceWarning::UnderInsured);
    assert_eq!(warning_for(&db, b.id, TODAY), InsuranceWarning::UnderInsured);
}

#[test]
fn scheduling_a_firearm_takes_it_out_of_the_blanket_total() {
    let db = TestDb::new();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Blanket", "2026-01-01", "2026-12-31", Some(80_000)),
    )
    .unwrap();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2026-01-01", "2026-12-31", None))
            .unwrap();
    let a = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    let b = firearm_ops::create_firearm(&db.conn, &valued("Sig", 50_000), false).unwrap();
    insurance_ops::assign_firearm_coverage(&db.conn, b.id, Some(rider.id), Some(50_000)).unwrap();

    let ctx = load_context_as_of(&db.conn, date(TODAY)).unwrap();
    assert_eq!(ctx.blanket.as_ref().unwrap().total, 50_000);
    assert_eq!(warning_for(&db, a.id, TODAY), InsuranceWarning::None);
    assert_eq!(warning_for(&db, b.id, TODAY), InsuranceWarning::None);
}

#[test]
fn disposed_firearms_do_not_count_toward_the_blanket_total() {
    let db = TestDb::new();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Blanket", "2026-01-01", "2026-12-31", Some(80_000)),
    )
    .unwrap();
    firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    let gone = firearm_ops::create_firearm(&db.conn, &valued("Sig", 50_000), false).unwrap();
    firearm_ops::dispose_firearm(
        &db.conn,
        gone.id,
        &hoplodex_lib::commands::firearms::DisposeFirearmInput {
            disposition_type: hoplodex_lib::models::firearm::DispositionType::Sold,
            recipient: "Jane".into(),
            date: "2026-02-01".into(),
            price: 1,
        },
    )
    .unwrap();

    let ctx = load_context_as_of(&db.conn, date(TODAY)).unwrap();
    assert_eq!(ctx.blanket.unwrap().total, 50_000);
}

#[test]
fn scenario_2_and_3_a_scheduled_firearm_is_under_insured_until_its_amount_meets_its_value() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2026-01-01", "2026-12-31", None))
            .unwrap();
    let f = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();

    insurance_ops::assign_firearm_coverage(&db.conn, f.id, Some(rider.id), Some(30_000)).unwrap();
    assert_eq!(warning_for(&db, f.id, TODAY), InsuranceWarning::UnderInsured);

    insurance_ops::assign_firearm_coverage(&db.conn, f.id, Some(rider.id), Some(50_000)).unwrap();
    assert_eq!(warning_for(&db, f.id, TODAY), InsuranceWarning::None);
}

#[test]
fn scenario_8_a_firearm_scheduled_under_an_expired_policy_is_uninsured_whatever_its_amount() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2025-01-01", "2026-06-10", None))
            .unwrap();
    let f = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();
    insurance_ops::assign_firearm_coverage(&db.conn, f.id, Some(rider.id), Some(90_000)).unwrap();

    assert_eq!(warning_for(&db, f.id, TODAY), InsuranceWarning::Uninsured);
    // The same firearm on the policy's last day is still covered.
    assert_eq!(warning_for(&db, f.id, "2026-06-10"), InsuranceWarning::None);
}

#[test]
fn a_blanket_policy_that_has_expired_leaves_unscheduled_firearms_uninsured() {
    let db = TestDb::new();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Old", "2025-01-01", "2026-06-10", Some(1_000_000)),
    )
    .unwrap();
    let f = firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false).unwrap();

    assert_eq!(warning_for(&db, f.id, TODAY), InsuranceWarning::Uninsured);
}

#[test]
fn scenario_12_on_the_boundary_day_the_later_starting_policy_is_the_one_in_force() {
    let db = TestDb::new();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Outgoing", "2025-06-15", "2026-06-15", Some(100_000)),
    )
    .unwrap();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Incoming", "2026-06-15", "2027-06-15", Some(200_000)),
    )
    .unwrap();

    let name = |today: &str| {
        load_context_as_of(&db.conn, date(today)).unwrap().blanket.map(|b| (b.policy_name, b.limit))
    };
    assert_eq!(name("2026-06-14"), Some(("Outgoing".into(), 100_000)));
    assert_eq!(name("2026-06-15"), Some(("Incoming".into(), 200_000)), "the boundary day");
    assert_eq!(name("2026-06-16"), Some(("Incoming".into(), 200_000)));
}

#[test]
fn scenario_7_a_policy_expiring_within_30_days_is_flagged_and_the_expiry_is_exclusive_of_today() {
    let db = TestDb::new();
    let soon =
        insurance_ops::create_policy(&db.conn, &policy("Soon", "2025-01-01", "2026-07-15", None))
            .unwrap();
    let later =
        insurance_ops::create_policy(&db.conn, &policy("Later", "2025-01-01", "2026-07-16", None))
            .unwrap();
    let today_end =
        insurance_ops::create_policy(&db.conn, &policy("Today", "2025-01-01", "2026-06-15", None))
            .unwrap();

    let ctx = load_context_as_of(&db.conn, date(TODAY)).unwrap();
    assert!(ctx.policies[&soon.id].is_expiring_soon, "30 days out is within the window");
    assert!(!ctx.policies[&later.id].is_expiring_soon, "31 days out isn't");
    let today = &ctx.policies[&today_end.id];
    assert!(today.is_in_force && !today.is_expired, "still in force on its last day");
    assert!(today.is_expiring_soon);
}

#[test]
fn scenario_7_and_8_expiring_and_expired_warnings_show_for_a_schedule_only_policy() {
    let db = TestDb::new();
    let soon =
        insurance_ops::create_policy(&db.conn, &policy("Soon", "2025-01-01", "2026-06-30", None))
            .unwrap();
    let past =
        insurance_ops::create_policy(&db.conn, &policy("Past", "2025-01-01", "2026-06-01", None))
            .unwrap();

    let ctx = load_context_as_of(&db.conn, date(TODAY)).unwrap();
    assert!(ctx.policies[&soon.id].expiring_warning);
    assert!(!ctx.policies[&soon.id].expired_warning);
    assert!(ctx.policies[&past.id].is_expired && ctx.policies[&past.id].expired_warning);
}

#[test]
fn scenario_14_no_expiring_warning_while_a_successor_blanket_policy_starts_by_the_next_day() {
    let db = TestDb::new();
    let ending = insurance_ops::create_policy(
        &db.conn,
        &policy("Ending", "2025-07-01", "2026-06-30", Some(100_000)),
    )
    .unwrap();

    let before = load_context_as_of(&db.conn, date(TODAY)).unwrap();
    assert!(before.policies[&ending.id].expiring_warning, "no successor yet");

    // Starts the day after this one ends: no gap, so no warning.
    insurance_ops::create_policy(
        &db.conn,
        &policy("Next", "2026-07-01", "2027-06-30", Some(100_000)),
    )
    .unwrap();
    let after = load_context_as_of(&db.conn, date(TODAY)).unwrap();
    assert!(!after.policies[&ending.id].expiring_warning);
    assert!(
        after.policies[&ending.id].is_expiring_soon,
        "the fact is unchanged; only the warning is suppressed"
    );
}

#[test]
fn scenario_14_a_successor_starting_after_a_gap_does_not_suppress_the_expiring_warning() {
    let db = TestDb::new();
    let ending = insurance_ops::create_policy(
        &db.conn,
        &policy("Ending", "2025-07-01", "2026-06-30", Some(100_000)),
    )
    .unwrap();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Late", "2026-07-15", "2027-07-14", Some(100_000)),
    )
    .unwrap();

    let ctx = load_context_as_of(&db.conn, date(TODAY)).unwrap();
    assert!(ctx.policies[&ending.id].expiring_warning, "two uncovered weeks");
}

#[test]
fn scenario_14_no_expired_warning_once_a_later_starting_blanket_policy_exists() {
    let db = TestDb::new();
    let old = insurance_ops::create_policy(
        &db.conn,
        &policy("Old", "2024-01-01", "2025-01-01", Some(100_000)),
    )
    .unwrap();
    let lonely = ctx_expired_warning(&db, old.id);
    assert!(lonely, "with nothing after it, the lapse is worth a warning");

    insurance_ops::create_policy(
        &db.conn,
        &policy("Current", "2026-01-01", "2026-12-31", Some(100_000)),
    )
    .unwrap();
    let ctx = load_context_as_of(&db.conn, date(TODAY)).unwrap();
    assert!(ctx.policies[&old.id].is_expired, "still expired");
    assert!(!ctx.policies[&old.id].expired_warning, "but it is history now");
}

fn ctx_expired_warning(db: &TestDb, id: i64) -> bool {
    load_context_as_of(&db.conn, date(TODAY)).unwrap().policies[&id].expired_warning
}

#[test]
fn a_schedule_only_policy_is_never_suppressed_by_a_blanket_policy() {
    let db = TestDb::new();
    let rider =
        insurance_ops::create_policy(&db.conn, &policy("Rider", "2024-01-01", "2025-01-01", None))
            .unwrap();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Current", "2026-01-01", "2026-12-31", Some(100_000)),
    )
    .unwrap();

    let ctx = load_context_as_of(&db.conn, date(TODAY)).unwrap();
    assert!(ctx.policies[&rider.id].expired_warning);
}

#[test]
fn no_estimated_value_is_never_flagged() {
    let db = TestDb::new();
    let f = firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "A1"), false).unwrap();
    assert_eq!(warning_for(&db, f.id, TODAY), InsuranceWarning::None);
}
