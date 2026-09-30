//! Integration tests for `list_firearms`'s search/group/`includeDisposed`
//! behavior (spec.md US2 Acceptance Scenarios 1-5), run against a real
//! temporary SQLCipher database — no mocks, per the constitution.

mod support;

use hoplodex_lib::commands::firearms::{GroupBy, ListFirearmsInput, ops};
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus, Origin};
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
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        cartridge: None,
        action_type_id: None,
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

/// Two firearms can share make and model, which is how the UI names a
/// firearm — browse rows carry the serial number so the UI can tell them
/// apart, plus the coverage assignment so the insurance view can list each
/// policy's firearms.
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

// specs/002-firearm-identification US4-1: grouping by origin, in a fixed
// order (Domestic, Imported, Re-imported, Unspecified) rather than
// alphabetically, with only the origins actually present in the result.
// specs/004-cartridges-action-types research.md §11 renamed "Not specified"
// to "Unspecified".

#[test]
fn group_by_origin_returns_groups_in_a_fixed_order_with_only_present_origins() {
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &FirearmInput { origin: Some(Origin::Reimported), ..firearm("Inland", "M1", "9mm", 2) },
        false,
    )
    .unwrap();
    ops::create_firearm(
        &db.conn,
        &FirearmInput { origin: Some(Origin::Imported), ..firearm("FN", "1922", "9mm", 1) },
        false,
    )
    .unwrap();
    ops::create_firearm(&db.conn, &firearm("Ruger", "10/22", ".22 LR", 2), false).unwrap();
    ops::create_firearm(
        &db.conn,
        &FirearmInput { origin: Some(Origin::Domestic), ..firearm("Colt", "1911", ".45", 1) },
        false,
    )
    .unwrap();

    let result = ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { group_by: Some(GroupBy::Origin), ..Default::default() },
    )
    .unwrap();

    let keys: Vec<_> = result.groups.iter().map(|g| g.key.as_str()).collect();
    assert_eq!(keys, vec!["Domestic", "Imported", "Re-imported", "Unspecified"]);
}

#[test]
fn group_by_origin_omits_origins_with_no_firearms() {
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &FirearmInput { origin: Some(Origin::Domestic), ..firearm("Colt", "1911", ".45", 1) },
        false,
    )
    .unwrap();
    ops::create_firearm(&db.conn, &firearm("Ruger", "10/22", ".22 LR", 2), false).unwrap();

    let result = ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { group_by: Some(GroupBy::Origin), ..Default::default() },
    )
    .unwrap();

    let keys: Vec<_> = result.groups.iter().map(|g| g.key.as_str()).collect();
    assert_eq!(keys, vec!["Domestic", "Unspecified"]);
}

// specs/004-cartridges-action-types US1-6, FR-008 and FR-027: the cartridge
// on each summary, and grouping by it.

fn chambered(make: &str, model: &str, cartridge: Option<&str>, caliber: &str) -> FirearmInput {
    FirearmInput { cartridge: cartridge.map(str::to_owned), ..firearm(make, model, caliber, 1) }
}

fn group_keys(conn: &rusqlite::Connection, group_by: GroupBy) -> Vec<(String, Vec<String>)> {
    ops::list_firearms(conn, &ListFirearmsInput { group_by: Some(group_by), ..Default::default() })
        .unwrap()
        .groups
        .into_iter()
        .map(|group| {
            let mut models: Vec<String> = group.firearms.into_iter().map(|f| f.model).collect();
            models.sort();
            (group.key, models)
        })
        .collect()
}

#[test]
fn summaries_carry_the_cartridge() {
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &chambered("Glock", "17", Some("9x19mm Parabellum"), "9mm"),
        false,
    )
    .unwrap();
    ops::create_firearm(&db.conn, &chambered("Thompson", "Hawken", None, ".50"), false).unwrap();

    let result = ops::list_firearms(&db.conn, &ListFirearmsInput::default()).unwrap();
    let all: Vec<_> = result.groups.iter().flat_map(|g| &g.firearms).collect();
    let glock = all.iter().find(|f| f.make == "Glock").unwrap();
    assert_eq!(glock.cartridge.as_deref(), Some("9x19mm Parabellum"));
    let hawken = all.iter().find(|f| f.make == "Thompson").unwrap();
    assert_eq!(hawken.cartridge, None);
}

#[test]
fn group_by_cartridge_keys_by_the_stored_text_with_unspecified_last() {
    let db = TestDb::new();
    for (model, cartridge, caliber) in [
        ("A", Some("9x19mm Parabellum"), "9mm"),
        ("B", Some("9X19mm Parabellum"), "9mm"),
        ("C", Some(".45 ACP"), ".45"),
        ("D", None, ".50"),
        ("E", Some("9x19mm Parabellum"), "9mm"),
        ("F", Some("Zulu Wildcat"), ".30"),
        ("G", None, "12 gauge"),
    ] {
        ops::create_firearm(&db.conn, &chambered("Maker", model, cartridge, caliber), false)
            .unwrap();
    }

    let groups = group_keys(&db.conn, GroupBy::Cartridge);
    let keys: Vec<&str> = groups.iter().map(|(key, _)| key.as_str()).collect();
    // Spelling variants already on record stay separate groups (spec Edge
    // Cases); the rest are alphabetical, and firearms with none come last.
    assert_eq!(
        keys,
        vec![".45 ACP", "9X19mm Parabellum", "9x19mm Parabellum", "Zulu Wildcat", "Unspecified"]
    );
    let parabellum = groups.iter().find(|(key, _)| key == "9x19mm Parabellum").unwrap();
    assert_eq!(parabellum.1, vec!["A", "E"]);
    assert_eq!(groups.last().unwrap().1, vec!["D", "G"]);
}

#[test]
fn group_by_caliber_gathers_every_cartridge_of_the_bore_class() {
    // US1-6: 9x19mm Parabellum, 9x18mm Makarov and a cartridge recorded as
    // just "9mm" all fall in the "9mm" group.
    let db = TestDb::new();
    ops::create_firearm(
        &db.conn,
        &chambered("Glock", "17", Some("9x19mm Parabellum"), "9mm"),
        false,
    )
    .unwrap();
    ops::create_firearm(
        &db.conn,
        &chambered("Makarov", "PM", Some("9x18mm Makarov"), "9mm"),
        false,
    )
    .unwrap();
    ops::create_firearm(&db.conn, &chambered("Hi-Point", "C9", Some("9mm"), "9mm"), false).unwrap();
    ops::create_firearm(&db.conn, &chambered("Colt", "1911", Some(".45 ACP"), ".45"), false)
        .unwrap();

    let groups = group_keys(&db.conn, GroupBy::Caliber);
    assert_eq!(
        groups,
        vec![
            (".45".to_string(), vec!["1911".to_string()]),
            ("9mm".to_string(), vec!["17".to_string(), "C9".to_string(), "PM".to_string()]),
        ]
    );

    let by_cartridge = group_keys(&db.conn, GroupBy::Cartridge);
    let keys: Vec<&str> = by_cartridge.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, vec![".45 ACP", "9mm", "9x18mm Makarov", "9x19mm Parabellum"]);
}

#[test]
fn group_by_cartridge_deserializes_from_its_wire_name() {
    let input: ListFirearmsInput =
        serde_json::from_value(serde_json::json!({ "groupBy": "cartridge" })).unwrap();
    assert_eq!(input.group_by, Some(GroupBy::Cartridge));
}

// specs/004-cartridges-action-types US3-4: the action's name on each
// summary, and grouping by it in the action list's order.

fn acting(model: &str, firearm_type_id: i64, action_type_id: Option<i64>) -> FirearmInput {
    FirearmInput { action_type_id, ..firearm("Maker", model, "9mm", firearm_type_id) }
}

#[test]
fn summaries_carry_the_action_name_or_none() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &acting("Bolty", 2, Some(3)), false).unwrap();
    ops::create_firearm(&db.conn, &acting("Plain", 2, None), false).unwrap();

    let result = ops::list_firearms(&db.conn, &ListFirearmsInput::default()).unwrap();
    let all: Vec<_> = result.groups.iter().flat_map(|g| &g.firearms).collect();
    let bolty = all.iter().find(|f| f.model == "Bolty").unwrap();
    assert_eq!(bolty.action_type_name.as_deref(), Some("Bolt action"));
    let plain = all.iter().find(|f| f.model == "Plain").unwrap();
    assert_eq!(plain.action_type_name, None);

    let json = serde_json::to_value(bolty).unwrap();
    assert_eq!(json["actionTypeName"], "Bolt action");
}

#[test]
fn group_by_action_type_follows_the_action_list_with_unspecified_last() {
    let db = TestDb::new();
    // Created out of list order, so alphabetical order would differ:
    // Revolver (2), Semi-automatic (1), Bolt action (3), Break action (6).
    for (model, type_id, action) in [
        ("A", 1, Some(2)),
        ("B", 1, Some(1)),
        ("C", 2, Some(3)),
        ("D", 3, Some(6)),
        ("E", 1, None),
        ("F", 1, Some(1)),
        ("G", 4, None),
    ] {
        ops::create_firearm(&db.conn, &acting(model, type_id, action), false).unwrap();
    }

    let groups = group_keys(&db.conn, GroupBy::ActionType);
    let keys: Vec<&str> = groups.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        vec!["Semi-automatic", "Revolver", "Bolt action", "Break action", "Unspecified"]
    );
    assert_eq!(groups[0].1, vec!["B", "F"]);
    assert_eq!(groups.last().unwrap().1, vec!["E", "G"]);
}

#[test]
fn group_by_action_type_deserializes_from_its_wire_name() {
    let input: ListFirearmsInput =
        serde_json::from_value(serde_json::json!({ "groupBy": "action_type" })).unwrap();
    assert_eq!(input.group_by, Some(GroupBy::ActionType));
}

// specs/005-regulated-item-types US1-4: a Suppressor groups under its own
// type and shows its own drawing.

#[test]
fn a_suppressor_groups_under_suppressor_with_its_own_drawing() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &firearm("SilencerCo", "Omega 300", ".30", 5), false).unwrap();
    ops::create_firearm(&db.conn, &firearm("Ruger", "10/22", ".22 LR", 2), false).unwrap();

    let result = ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { group_by: Some(GroupBy::Type), ..Default::default() },
    )
    .unwrap();

    let group = result.groups.iter().find(|g| g.key == "Suppressor").unwrap();
    assert_eq!(group.firearms.len(), 1);
    let summary = &group.firearms[0];
    assert_eq!(summary.generic_thumbnail_key, "suppressor");
    assert_eq!(summary.action_type_name, None);
}

/// specs/005-regulated-item-types US2-9, US2-10, FR-016.
#[test]
fn grouping_by_registration_follows_the_list_and_the_alphabet_with_unspecified_last() {
    let db = TestDb::new();
    let add = |serial: &str, class: Option<i64>, to: Option<&str>| {
        ops::create_firearm(
            &db.conn,
            &FirearmInput {
                serial_number: Some(serial.into()),
                registration_class_id: class,
                registered_to: to.map(str::to_owned),
                ..firearm("Make", &format!("M{serial}"), "9mm", 1)
            },
            false,
        )
        .unwrap();
    };
    add("1", Some(5), Some("Zed Trust"));
    add("2", None, None);
    add("3", Some(1), Some("Adams LLC"));
    add("4", Some(1), None);
    add("5", Some(2), Some("Zed Trust"));
    add("6", None, None);

    let grouped = |by| {
        ops::list_firearms(
            &db.conn,
            &ListFirearmsInput { group_by: Some(by), ..Default::default() },
        )
        .unwrap()
        .groups
    };
    let as_groups = grouped(GroupBy::RegisteredAs);
    let keys: Vec<(&str, usize)> =
        as_groups.iter().map(|g| (g.key.as_str(), g.firearms.len())).collect();
    assert_eq!(
        keys,
        [("Suppressor", 2), ("Short-barreled rifle", 1), ("Machine gun", 1), ("Unspecified", 2)]
    );
    assert_eq!(as_groups[0].firearms[0].registered_as.as_deref(), Some("Suppressor"));
    assert_eq!(as_groups[3].firearms[0].registered_as, None);

    let to_groups = grouped(GroupBy::RegisteredTo);
    let keys: Vec<(&str, usize)> =
        to_groups.iter().map(|g| (g.key.as_str(), g.firearms.len())).collect();
    // Unspecified holds every firearm with no "Registered to", classified or not.
    assert_eq!(keys, [("Adams LLC", 1), ("Zed Trust", 2), ("Unspecified", 3)]);
    let json = serde_json::to_value(&as_groups[0].firearms[0]).unwrap();
    assert_eq!(json["registeredAs"], "Suppressor");
}

#[test]
fn group_by_action_puts_automatic_after_break_action_and_before_falling_block() {
    // 005 US3: id 13 sorts seventh in the action list.
    let db = TestDb::new();
    for (model, action) in [("A", 7), ("B", 13), ("C", 6)] {
        ops::create_firearm(&db.conn, &acting(model, 2, Some(action)), false).unwrap();
    }
    let groups = group_keys(&db.conn, GroupBy::ActionType);
    let keys: Vec<&str> = groups.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, vec!["Break action", "Automatic or select-fire", "Falling block"]);
}
