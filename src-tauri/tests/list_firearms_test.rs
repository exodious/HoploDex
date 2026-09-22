//! Integration tests for `list_firearms`'s search/group/`includeDisposed`
//! behavior (spec.md US2 Acceptance Scenarios 1-5), run against a real
//! temporary SQLCipher database — no mocks, per the constitution.

mod support;

use hoplodex_lib::commands::firearms::{ops, GroupBy, ListFirearmsInput};
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use support::TestDb;

fn firearm(make: &str, model: &str, caliber: &str, firearm_type_id: i64) -> FirearmInput {
    FirearmInput {
        make: make.into(),
        model: model.into(),
        serial_number: Some(format!("{make}-{model}")),
        no_serial_attested: false,
        caliber: caliber.into(),
        firearm_type_id,
        notes: None,
        accessories: None,
        barrel_length_hundredths: None,
        overall_length_hundredths: None,
        weight_tenths_oz: None,
        capacity: None,
        finish: None,
        condition: None,
        status: FirearmStatus::Active,
        estimated_value: None,
        acquisition_source: None,
        acquisition_date: None,
        acquisition_price: None,
        disposition_type: None,
        disposition_recipient: None,
        disposition_date: None,
        disposition_price: None,
        insurance_policy_id: None,
        nickname: None,
        scheduled_coverage_amount: None,
        origin: None,
        year_of_manufacture: None,
        country_of_manufacture: None,
        importer_name: None,
        original_make: None,
        original_model: None,
        original_serial_number: None,
    }
}

#[test]
fn scenario_1_list_and_tile_views_return_the_same_data() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "9mm", 1), false).unwrap();
    ops::create_firearm(&db.conn, &firearm("Ruger", "10/22", ".22 LR", 2), false).unwrap();

    let list_view = ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { view: Some("list".into()), ..Default::default() },
    )
    .unwrap();
    let tile_view = ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { view: Some("tile".into()), ..Default::default() },
    )
    .unwrap();

    assert_eq!(list_view.groups, tile_view.groups, "view is informational only");
    assert_eq!(list_view.groups.iter().map(|g| g.firearms.len()).sum::<usize>(), 2);
}

#[test]
fn scenario_2_group_by_type_buckets_firearms_correctly() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "9mm", 1), false).unwrap();
    ops::create_firearm(&db.conn, &firearm("Sig", "P320", "9mm", 1), false).unwrap();
    ops::create_firearm(&db.conn, &firearm("Ruger", "10/22", ".22 LR", 2), false).unwrap();

    let result = ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { group_by: Some(GroupBy::Type), ..Default::default() },
    )
    .unwrap();

    let handgun_group = result.groups.iter().find(|g| g.key == "Handgun").unwrap();
    assert_eq!(handgun_group.firearms.len(), 2);
    let rifle_group = result.groups.iter().find(|g| g.key == "Rifle").unwrap();
    assert_eq!(rifle_group.firearms.len(), 1);
}

#[test]
fn scenario_3_search_matches_free_form_notes_only() {
    let db = TestDb::new();
    let mut noted = firearm("Glock", "19", "9mm", 1);
    noted.notes = Some("cracked handle".into());
    ops::create_firearm(&db.conn, &noted, false).unwrap();
    ops::create_firearm(&db.conn, &firearm("Ruger", "10/22", ".22 LR", 2), false).unwrap();

    let result = ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("cracked handle".into()), ..Default::default() },
    )
    .unwrap();

    let all: Vec<_> = result.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].make, "Glock");
}

#[test]
fn scenario_4_search_a_shared_caliber_returns_all_matches() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "9mm", 1), false).unwrap();
    ops::create_firearm(&db.conn, &firearm("Sig", "P320", "9mm", 1), false).unwrap();
    ops::create_firearm(&db.conn, &firearm("Ruger", "10/22", ".22 LR", 2), false).unwrap();

    let result = ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("9mm".into()), ..Default::default() },
    )
    .unwrap();

    let all: Vec<_> = result.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(all.len(), 2);
}

#[test]
fn scenario_5_clearing_search_and_group_shows_the_full_collection() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("Glock", "19", "9mm", 1), false).unwrap();
    ops::create_firearm(&db.conn, &firearm("Ruger", "10/22", ".22 LR", 2), false).unwrap();

    let result = ops::list_firearms(&db.conn, &ListFirearmsInput::default()).unwrap();

    let all: Vec<_> = result.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(all.len(), 2);
}

#[test]
fn disposed_firearms_are_excluded_by_default_but_included_on_request() {
    let db = TestDb::new();
    let created = ops::create_firearm(&db.conn, &firearm("Glock", "19", "9mm", 1), false).unwrap();
    ops::dispose_firearm(
        &db.conn,
        created.id,
        &hoplodex_lib::commands::firearms::DisposeFirearmInput {
            disposition_type: hoplodex_lib::models::firearm::DispositionType::Sold,
            recipient: "Jane".into(),
            date: "2025-01-01".into(),
            price: 100,
        },
    )
    .unwrap();
    ops::create_firearm(&db.conn, &firearm("Ruger", "10/22", ".22 LR", 2), false).unwrap();

    let default_result = ops::list_firearms(&db.conn, &ListFirearmsInput::default()).unwrap();
    let default_all: Vec<_> = default_result.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(default_all.len(), 1, "disposed firearms excluded by default (FR-025)");

    let with_disposed = ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { include_disposed: true, ..Default::default() },
    )
    .unwrap();
    let all_with_disposed: Vec<_> = with_disposed.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(all_with_disposed.len(), 2);
}

/// Two firearms can share make and model (spec_TODO: "the name assigned to a
/// firearm in the UI is a combination of the make and model") — browse rows
/// carry the serial number so the UI can tell them apart, plus the coverage
/// assignment so the insurance view can list each policy's firearms.
#[test]
fn summaries_carry_serial_number_and_coverage_assignment() {
    let db = TestDb::new();
    let policy = hoplodex_lib::commands::insurance::ops::create_policy(
        &db.conn,
        &hoplodex_lib::models::insurance_policy::InsurancePolicyInput {
            name: "Rider".into(),
            policy_number: "R-1".into(),
            insurance_company: "Acme".into(),
            company_contact: None,
            agent_name: None,
            agent_contact: None,
            notes: None,
            blanket_coverage_limit: None,
            effective_start_date: "2020-01-01".into(),
            effective_end_date: "2099-01-01".into(),
        },
    )
    .unwrap();

    let mut covered = firearm("Glock", "19", "9mm", 1);
    covered.serial_number = Some("AAA111".into());
    covered.insurance_policy_id = Some(policy.id);
    covered.scheduled_coverage_amount = Some(60_000);
    ops::create_firearm(&db.conn, &covered, false).unwrap();

    let mut twin = firearm("Glock", "19", "9mm", 1);
    twin.serial_number = None;
    twin.no_serial_attested = true;
    ops::create_firearm(&db.conn, &twin, false).unwrap();

    let result = ops::list_firearms(&db.conn, &ListFirearmsInput::default()).unwrap();
    let summaries = &result.groups[0].firearms;
    let with_serial = summaries.iter().find(|f| f.serial_number.is_some()).unwrap();
    assert_eq!(with_serial.serial_number.as_deref(), Some("AAA111"));
    assert_eq!(with_serial.insurance_policy_id, Some(policy.id));
    assert_eq!(with_serial.scheduled_coverage_amount, Some(60_000));

    let without_serial = summaries.iter().find(|f| f.serial_number.is_none()).unwrap();
    assert_eq!(without_serial.insurance_policy_id, None);
    assert_eq!(without_serial.scheduled_coverage_amount, None);
}
