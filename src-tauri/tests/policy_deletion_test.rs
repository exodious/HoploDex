//! Integration tests for deleting an insurance policy that has firearms
//! scheduled under it (FR-034, spec.md US3 Acceptance Scenarios 9-10, 15),
//! run against a real temporary SQLCipher database. "Today" in the status
//! checks is the real date, so policy dates are wide enough not to matter.

mod support;

use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::insurance::ScheduledFirearmsAction;
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::firearm::FirearmInput;
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::insurance_status::{InsuranceWarning, load_context, record_warning};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use support::{TestDb, firearm, policy};

const LONG_AGO: &str = "2001-01-01";
const SOON_PAST: &str = "2001-12-31";
const FAR_FUTURE: &str = "2098-01-01";

fn valued(make: &str, value: i64) -> FirearmInput {
    FirearmInput { estimated_value: Some(value), ..firearm(make, "M", &format!("{make}-SN")) }
}

fn rider(db: &TestDb, name: &str, end: &str) -> i64 {
    insurance_ops::create_policy(&db.conn, &policy(name, LONG_AGO, end, None)).unwrap().id
}

fn blanket(db: &TestDb, name: &str, start: &str, end: &str) -> i64 {
    insurance_ops::create_policy(&db.conn, &policy(name, start, end, Some(1_000_000))).unwrap().id
}

fn schedule(db: &TestDb, make: &str, policy_id: i64, amount: i64) -> i64 {
    let f = firearm_ops::create_firearm(&db.conn, &valued(make, 50_000), false, None).unwrap();
    insurance_ops::assign_firearm_coverage(&db.conn, f.id, Some(policy_id), Some(amount)).unwrap();
    f.id
}

fn warning(db: &TestDb, id: i64) -> InsuranceWarning {
    let f = firearm_ops::get_firearm(&db.conn, id).unwrap();
    let ctx = load_context(&db.conn).unwrap();
    record_warning(f.estimated_value, f.insurance_policy_id, f.scheduled_coverage_amount, &ctx)
}

fn move_to(target: i64) -> ScheduledFirearmsAction {
    ScheduledFirearmsAction::Move { target_policy_id: target }
}

fn unschedule(confirm: bool) -> ScheduledFirearmsAction {
    ScheduledFirearmsAction::Unschedule { confirm_unschedule: Some(confirm) }
}

fn policy_exists(db: &TestDb, id: i64) -> bool {
    insurance_ops::get_policy(&db.conn, id).is_ok()
}

// --- Impact preview ---

#[test]
fn the_impact_lists_what_the_dialog_needs_before_anything_changes() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let other = rider(&db, "Other rider", FAR_FUTURE);
    let expired_other = rider(&db, "Old rider", SOON_PAST);
    let f = firearm_ops::create_firearm(
        &db.conn,
        &FirearmInput { nickname: Some("Range gun".into()), ..valued("Colt", 50_000) },
        false,
        None,
    )
    .unwrap();
    insurance_ops::assign_firearm_coverage(&db.conn, f.id, Some(doomed), Some(40_000)).unwrap();

    let impact = insurance_ops::get_policy_deletion_impact(&db.conn, doomed).unwrap();

    assert!(!impact.is_expired);
    assert!(!impact.is_blanket_in_force);
    assert_eq!(impact.scheduled_record_count, 1);
    assert_eq!(impact.scheduled_records[0].record, RecordRef::Firearm(f.id));
    assert_eq!(impact.scheduled_records[0].make.as_deref(), Some("Colt"));
    assert_eq!(impact.scheduled_records[0].nickname.as_deref(), Some("Range gun"));
    let mut others: Vec<_> = impact.other_policies.iter().map(|p| (p.id, p.is_expired)).collect();
    others.sort();
    assert_eq!(others, vec![(other, false), (expired_other, true)], "everything but itself");
    assert_eq!(impact.blanket_record_count, 0);
}

#[test]
fn the_impact_says_where_unscheduled_firearms_would_land() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    schedule(&db, "Colt", doomed, 40_000);

    let none = insurance_ops::get_policy_deletion_impact(&db.conn, doomed).unwrap();
    assert_eq!(none.unschedule_outcome, "uninsured", "no blanket policy in force");

    blanket(&db, "Blanket", LONG_AGO, FAR_FUTURE);
    let covered = insurance_ops::get_policy_deletion_impact(&db.conn, doomed).unwrap();
    assert_eq!(covered.unschedule_outcome, "blanket");
}

#[test]
fn scenario_15_deleting_the_blanket_policy_in_force_reports_how_many_lose_its_coverage() {
    let db = TestDb::new();
    let current = blanket(&db, "Current blanket", LONG_AGO, FAR_FUTURE);
    firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false, None).unwrap();
    firearm_ops::create_firearm(&db.conn, &valued("Sig", 50_000), false, None).unwrap();
    let scheduled_elsewhere = rider(&db, "Rider", FAR_FUTURE);
    schedule(&db, "Ruger", scheduled_elsewhere, 50_000);

    let impact = insurance_ops::get_policy_deletion_impact(&db.conn, current).unwrap();

    assert!(impact.is_blanket_in_force);
    assert_eq!(
        impact.blanket_record_count, 2,
        "the two unscheduled firearms, not the scheduled one"
    );
    assert_eq!(impact.unschedule_outcome, "uninsured");
}

#[test]
fn a_blanket_policy_that_is_not_in_force_takes_no_coverage_with_it() {
    let db = TestDb::new();
    let old = blanket(&db, "Old blanket", "2000-01-01", SOON_PAST);
    firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false, None).unwrap();

    let impact = insurance_ops::get_policy_deletion_impact(&db.conn, old).unwrap();

    assert!(!impact.is_blanket_in_force);
    assert_eq!(impact.blanket_record_count, 0);
    assert!(impact.is_expired);
}

#[test]
fn the_impact_of_a_missing_policy_is_not_found() {
    let db = TestDb::new();
    let err = insurance_ops::get_policy_deletion_impact(&db.conn, 9999).unwrap_err();
    assert_eq!(err.code, "NOT_FOUND");
}

// --- Deleting ---

#[test]
fn a_policy_with_no_scheduled_firearms_deletes_without_a_resolution() {
    let db = TestDb::new();
    let id = rider(&db, "Empty", FAR_FUTURE);

    let result = insurance_ops::delete_policy(&db.conn, id, true, None).unwrap();

    assert!(result.deleted);
    assert_eq!((result.moved_count, result.unscheduled_count), (0, 0));
    assert!(!policy_exists(&db, id));
}

#[test]
fn deleting_requires_explicit_confirmation() {
    let db = TestDb::new();
    let id = rider(&db, "Empty", FAR_FUTURE);

    let err = insurance_ops::delete_policy(&db.conn, id, false, None).unwrap_err();
    assert_eq!(err.code, "CONFIRMATION_REQUIRED");
    assert!(policy_exists(&db, id));
}

#[test]
fn scenario_9_an_unresolved_deletion_is_refused_and_changes_nothing() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let f = schedule(&db, "Colt", doomed, 40_000);

    let err = insurance_ops::delete_policy(&db.conn, doomed, true, None).unwrap_err();

    assert_eq!(err.code, "POLICY_HAS_FIREARMS");
    assert!(policy_exists(&db, doomed));
    assert_eq!(firearm_ops::get_firearm(&db.conn, f).unwrap().insurance_policy_id, Some(doomed));
}

#[test]
fn scenario_9_move_re_points_every_scheduled_firearm_and_keeps_its_amount() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let target = rider(&db, "Target", FAR_FUTURE);
    let a = schedule(&db, "Colt", doomed, 40_000);
    let b = schedule(&db, "Sig", doomed, 55_000);

    let result =
        insurance_ops::delete_policy(&db.conn, doomed, true, Some(&move_to(target))).unwrap();

    assert_eq!((result.deleted, result.moved_count, result.unscheduled_count), (true, 2, 0));
    assert!(!policy_exists(&db, doomed));
    let moved_a = firearm_ops::get_firearm(&db.conn, a).unwrap();
    let moved_b = firearm_ops::get_firearm(&db.conn, b).unwrap();
    assert_eq!(
        (moved_a.insurance_policy_id, moved_a.scheduled_coverage_amount),
        (Some(target), Some(40_000))
    );
    assert_eq!(
        (moved_b.insurance_policy_id, moved_b.scheduled_coverage_amount),
        (Some(target), Some(55_000))
    );
    assert_eq!(
        warning(&db, a),
        InsuranceWarning::UnderInsured,
        "the kept amount is still checked against its value"
    );
}

#[test]
fn a_move_needs_a_real_target_other_than_the_policy_itself_and_changes_nothing_if_it_fails() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let f = schedule(&db, "Colt", doomed, 40_000);

    for target in [doomed, 9999] {
        let err = insurance_ops::delete_policy(&db.conn, doomed, true, Some(&move_to(target)))
            .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR", "target {target}");
    }

    assert!(policy_exists(&db, doomed), "the deletion was refused as a whole");
    assert_eq!(firearm_ops::get_firearm(&db.conn, f).unwrap().insurance_policy_id, Some(doomed));
}

#[test]
fn scenario_9_unscheduling_a_non_expired_policy_needs_the_stronger_confirmation() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let f = schedule(&db, "Colt", doomed, 40_000);

    for resolution in
        [ScheduledFirearmsAction::Unschedule { confirm_unschedule: None }, unschedule(false)]
    {
        let err =
            insurance_ops::delete_policy(&db.conn, doomed, true, Some(&resolution)).unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
    }
    assert!(policy_exists(&db, doomed));
    assert_eq!(firearm_ops::get_firearm(&db.conn, f).unwrap().insurance_policy_id, Some(doomed));

    let result =
        insurance_ops::delete_policy(&db.conn, doomed, true, Some(&unschedule(true))).unwrap();
    assert_eq!((result.moved_count, result.unscheduled_count), (0, 1));
    let cleared = firearm_ops::get_firearm(&db.conn, f).unwrap();
    assert_eq!((cleared.insurance_policy_id, cleared.scheduled_coverage_amount), (None, None));
}

#[test]
fn scenario_10_unscheduling_an_expired_policy_needs_only_the_warning() {
    let db = TestDb::new();
    let expired = rider(&db, "Expired", SOON_PAST);
    let f = schedule(&db, "Colt", expired, 90_000);
    assert_eq!(warning(&db, f), InsuranceWarning::Uninsured, "already uninsured");

    let result = insurance_ops::delete_policy(
        &db.conn,
        expired,
        true,
        Some(&ScheduledFirearmsAction::Unschedule { confirm_unschedule: None }),
    )
    .unwrap();

    assert_eq!(result.unscheduled_count, 1);
    assert!(!policy_exists(&db, expired));
}

#[test]
fn unscheduled_firearms_fall_to_the_blanket_policy_in_force() {
    let db = TestDb::new();
    blanket(&db, "Blanket", LONG_AGO, FAR_FUTURE);
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let f = schedule(&db, "Colt", doomed, 10_000);
    assert_eq!(warning(&db, f), InsuranceWarning::UnderInsured);

    insurance_ops::delete_policy(&db.conn, doomed, true, Some(&unschedule(true))).unwrap();

    assert_eq!(warning(&db, f), InsuranceWarning::None, "now covered by the blanket policy");
}

#[test]
fn unscheduled_firearms_are_uninsured_when_no_blanket_policy_is_in_force() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let f = schedule(&db, "Colt", doomed, 90_000);
    assert_eq!(warning(&db, f), InsuranceWarning::None);

    insurance_ops::delete_policy(&db.conn, doomed, true, Some(&unschedule(true))).unwrap();

    assert_eq!(warning(&db, f), InsuranceWarning::Uninsured);
}

#[test]
fn deleting_the_blanket_policy_in_force_leaves_the_unscheduled_firearms_uninsured() {
    let db = TestDb::new();
    let current = blanket(&db, "Current", LONG_AGO, FAR_FUTURE);
    let f =
        firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false, None).unwrap().id;
    assert_eq!(warning(&db, f), InsuranceWarning::None);

    insurance_ops::delete_policy(&db.conn, current, true, None).unwrap();

    assert_eq!(warning(&db, f), InsuranceWarning::Uninsured);
}

#[test]
fn firearms_that_were_disposed_do_not_block_a_deletion_and_are_cleared_with_it() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let f = schedule(&db, "Colt", doomed, 40_000);
    db.conn
        .execute(
            "UPDATE firearms SET status = 'disposed', disposition_type = 'sold',
                    disposition_recipient = 'Jane', disposition_date = '2020-01-01',
                    disposition_price = 1 WHERE id = ?1",
            [f],
        )
        .unwrap();

    let impact = insurance_ops::get_policy_deletion_impact(&db.conn, doomed).unwrap();
    assert_eq!(impact.scheduled_record_count, 0, "a disposed firearm isn't insured");

    insurance_ops::delete_policy(&db.conn, doomed, true, None).unwrap();

    assert!(!policy_exists(&db, doomed));
    assert_eq!(firearm_ops::get_firearm(&db.conn, f).unwrap().insurance_policy_id, None);
}

#[test]
fn the_foreign_key_still_refuses_a_raw_delete_as_a_backstop() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    schedule(&db, "Colt", doomed, 40_000);

    let raw = db.conn.execute("DELETE FROM insurance_policies WHERE id = ?1", [doomed]);

    assert!(raw.is_err(), "ON DELETE RESTRICT: a bug can never silently un-insure firearms");
}

#[test]
fn deleting_a_missing_policy_is_not_found() {
    let db = TestDb::new();
    let err = insurance_ops::delete_policy(&db.conn, 9999, true, None).unwrap_err();
    assert_eq!(err.code, "NOT_FOUND");
}

#[test]
fn the_wire_format_of_the_resolution_matches_the_contract() {
    let moved: ScheduledFirearmsAction =
        serde_json::from_str(r#"{"action":"move","targetPolicyId":4}"#).unwrap();
    assert!(matches!(moved, ScheduledFirearmsAction::Move { target_policy_id: 4 }));

    let plain: ScheduledFirearmsAction =
        serde_json::from_str(r#"{"action":"unschedule"}"#).unwrap();
    assert!(matches!(plain, ScheduledFirearmsAction::Unschedule { confirm_unschedule: None }));

    let confirmed: ScheduledFirearmsAction =
        serde_json::from_str(r#"{"action":"unschedule","confirmUnschedule":true}"#).unwrap();
    assert!(matches!(
        confirmed,
        ScheduledFirearmsAction::Unschedule { confirm_unschedule: Some(true) }
    ));
}

// --- Accessories (specs/006-accessory-links FR-009) ---------------------------
//
// A policy's scheduled records are firearms and accessories together. Accessory
// inputs are built from the IPC shape (`AccessoryInput`'s camelCase JSON), so
// these tests do not depend on how the struct is spelled.

fn parse<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("the JSON must fit the type")
}

fn accessory(db: &TestDb, model: &str, value: i64) -> i64 {
    let input = json!({
        "accessoryKindId": 1,
        "make": "Leupold",
        "model": model,
        "estimatedValue": value,
        "status": "active",
    });
    accessory_ops::create_accessory(&db.conn, &parse(input), None).unwrap().id
}

fn schedule_accessory(db: &TestDb, model: &str, policy_id: i64, amount: i64) -> i64 {
    let id = accessory(db, model, 1_000);
    insurance_ops::assign_accessory_coverage(&db.conn, id, Some(policy_id), Some(amount)).unwrap();
    id
}

/// An accessory's `(insurance_policy_id, scheduled_coverage_amount)` as stored.
fn accessory_schedule(db: &TestDb, id: i64) -> (Option<i64>, Option<i64>) {
    db.conn
        .query_row(
            "SELECT insurance_policy_id, scheduled_coverage_amount FROM accessories WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

#[test]
fn the_impact_lists_scheduled_firearms_and_accessories_together_by_label() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let f = schedule(&db, "Colt", doomed, 40_000);
    let a = schedule_accessory(&db, "VX-5HD", doomed, 600);
    let elsewhere = rider(&db, "Elsewhere", FAR_FUTURE);
    schedule_accessory(&db, "Aimpoint", elsewhere, 100);

    let impact = insurance_ops::get_policy_deletion_impact(&db.conn, doomed).unwrap();

    assert_eq!(impact.scheduled_record_count, 2);
    let records: Vec<RecordRef> = impact.scheduled_records.iter().map(|l| l.record).collect();
    assert_eq!(records.len(), 2);
    assert!(records.contains(&RecordRef::Firearm(f)));
    assert!(records.contains(&RecordRef::Accessory(a)));
    let label = impact.scheduled_records.iter().find(|l| l.record == RecordRef::Accessory(a));
    let label = label.unwrap();
    assert_eq!(label.type_name, "Optic", "an accessory's label carries its kind's name");
    assert_eq!(label.make.as_deref(), Some("Leupold"));
    assert_eq!(label.model.as_deref(), Some("VX-5HD"));
}

#[test]
fn the_impact_does_not_list_a_disposed_accessory() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let a = schedule_accessory(&db, "VX-5HD", doomed, 600);
    accessory_ops::dispose_accessory(
        &db.conn,
        a,
        &parse(json!({
            "dispositionType": "sold",
            "recipient": "Jane",
            "date": "2026-02-01",
            "price": 1,
        })),
    )
    .unwrap();

    let impact = insurance_ops::get_policy_deletion_impact(&db.conn, doomed).unwrap();

    assert_eq!(impact.scheduled_record_count, 0);
    assert!(impact.scheduled_records.is_empty());
}

#[test]
fn deleting_the_blanket_policy_in_force_counts_unscheduled_records_of_both_kinds() {
    let db = TestDb::new();
    let current = blanket(&db, "Current blanket", LONG_AGO, FAR_FUTURE);
    firearm_ops::create_firearm(&db.conn, &valued("Glock", 50_000), false, None).unwrap();
    accessory(&db, "VX-5HD", 1_000);
    accessory(&db, "Aimpoint", 180);
    let scheduled_elsewhere = rider(&db, "Rider", FAR_FUTURE);
    schedule(&db, "Ruger", scheduled_elsewhere, 50_000);
    schedule_accessory(&db, "Eotech", scheduled_elsewhere, 500);

    let impact = insurance_ops::get_policy_deletion_impact(&db.conn, current).unwrap();

    assert!(impact.is_blanket_in_force);
    assert_eq!(
        impact.blanket_record_count, 3,
        "one unscheduled firearm and two unscheduled accessories, not the scheduled ones"
    );
}

#[test]
fn an_unresolved_deletion_with_only_a_scheduled_accessory_is_refused_and_changes_nothing() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let a = schedule_accessory(&db, "VX-5HD", doomed, 600);

    let err = insurance_ops::delete_policy(&db.conn, doomed, true, None).unwrap_err();

    assert_eq!(err.code, "POLICY_HAS_FIREARMS");
    assert!(policy_exists(&db, doomed));
    assert_eq!(accessory_schedule(&db, a), (Some(doomed), Some(600)));
}

#[test]
fn move_re_points_firearms_and_accessories_together_and_keeps_every_amount() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let target = rider(&db, "Target", FAR_FUTURE);
    let f = schedule(&db, "Colt", doomed, 40_000);
    let a = schedule_accessory(&db, "VX-5HD", doomed, 600);
    let b = schedule_accessory(&db, "Aimpoint", doomed, 900);

    let result =
        insurance_ops::delete_policy(&db.conn, doomed, true, Some(&move_to(target))).unwrap();

    assert_eq!((result.deleted, result.moved_count, result.unscheduled_count), (true, 3, 0));
    assert!(!policy_exists(&db, doomed));
    assert_eq!(accessory_schedule(&db, a), (Some(target), Some(600)));
    assert_eq!(accessory_schedule(&db, b), (Some(target), Some(900)));
    let moved = firearm_ops::get_firearm(&db.conn, f).unwrap();
    assert_eq!(
        (moved.insurance_policy_id, moved.scheduled_coverage_amount),
        (Some(target), Some(40_000))
    );
}

#[test]
fn a_failed_move_changes_no_accessory_either() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let f = schedule(&db, "Colt", doomed, 40_000);
    let a = schedule_accessory(&db, "VX-5HD", doomed, 600);

    let err =
        insurance_ops::delete_policy(&db.conn, doomed, true, Some(&move_to(9999))).unwrap_err();

    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(policy_exists(&db, doomed));
    assert_eq!(accessory_schedule(&db, a), (Some(doomed), Some(600)));
    assert_eq!(firearm_ops::get_firearm(&db.conn, f).unwrap().insurance_policy_id, Some(doomed));
}

#[test]
fn unscheduling_clears_the_policy_and_amount_of_firearms_and_accessories_in_one_step() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let f = schedule(&db, "Colt", doomed, 40_000);
    let a = schedule_accessory(&db, "VX-5HD", doomed, 600);

    let result =
        insurance_ops::delete_policy(&db.conn, doomed, true, Some(&unschedule(true))).unwrap();

    assert_eq!((result.moved_count, result.unscheduled_count), (0, 2));
    assert!(!policy_exists(&db, doomed));
    assert_eq!(accessory_schedule(&db, a), (None, None));
    let cleared = firearm_ops::get_firearm(&db.conn, f).unwrap();
    assert_eq!((cleared.insurance_policy_id, cleared.scheduled_coverage_amount), (None, None));
}

#[test]
fn disposed_accessories_do_not_block_a_deletion_and_are_cleared_with_it() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let a = schedule_accessory(&db, "VX-5HD", doomed, 600);
    db.conn
        .execute(
            "UPDATE accessories SET status = 'disposed', disposition_type = 'sold',
                    disposition_recipient = 'Jane', disposition_date = '2020-01-01',
                    disposition_price = 1 WHERE id = ?1",
            [a],
        )
        .unwrap();

    insurance_ops::delete_policy(&db.conn, doomed, true, None).unwrap();

    assert!(!policy_exists(&db, doomed));
    assert_eq!(accessory_schedule(&db, a), (None, None));
}

#[test]
fn the_foreign_key_also_refuses_a_raw_delete_under_a_scheduled_accessory() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    schedule_accessory(&db, "VX-5HD", doomed, 600);

    let raw = db.conn.execute("DELETE FROM insurance_policies WHERE id = ?1", [doomed]);

    assert!(raw.is_err(), "ON DELETE RESTRICT: a bug can never silently un-insure accessories");
}

#[test]
fn the_wire_format_of_the_impact_names_the_records() {
    let db = TestDb::new();
    let doomed = rider(&db, "Doomed", FAR_FUTURE);
    let a = schedule_accessory(&db, "VX-5HD", doomed, 600);

    let wire =
        serde_json::to_value(insurance_ops::get_policy_deletion_impact(&db.conn, doomed).unwrap())
            .unwrap();

    assert_eq!(wire["scheduledRecordCount"], json!(1));
    assert_eq!(wire["blanketRecordCount"], json!(0));
    assert_eq!(wire["scheduledRecords"][0]["record"], json!({ "kind": "accessory", "id": a }));
    assert!(wire.get("scheduledFirearms").is_none(), "the firearm-only fields are gone");
    assert!(wire.get("scheduledFirearmCount").is_none());
    assert!(wire.get("blanketFirearmCount").is_none());
}
