//! specs/006-accessory-links User Story 2 through `ops`, against a real
//! temporary SQLCipher database (no mocks): mounting a firearm or an
//! accessory on another record from the form or from `mount_record`
//! (FR-009, FR-012), the chain and the depth-first list below a record
//! (FR-013), the loop and active-host rules (FR-010), that nothing about a
//! kind, type or caliber is judged (FR-011), the candidate lists
//! (`list_mount_candidates`), disposing a host or an item (FR-014), and the
//! table's own backstops (data-model.md "Rules").
//!
//! Written before the implementation (tasks.md T054). Commands the contract
//! describes as JSON (`MountRecord`'s input and output, `list_mount_candidates`,
//! `DisposeInput`, the details) are built from and read as JSON in the
//! contract's own camelCase names, so the tests need nothing from the structs
//! but `mounted_on: Option<RecordRef>` on `FirearmInput` and `AccessoryInput`
//! (research.md §8) and the `ops` names in contracts/tauri-commands.md.

mod support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::mounts::ops;
use hoplodex_lib::models::accessory::AccessoryInput;
use hoplodex_lib::models::firearm::FirearmInput;
use hoplodex_lib::models::record::RecordRef;
use serde_json::{Value, json};
use support::{TestDb, firearm};

const OPTIC: i64 = 1;
const LIGHT: i64 = 2;
const MAGAZINE: i64 = 3;
const UPPER: i64 = 5;
const SLING: i64 = 10;
const CASE: i64 = 11;

const HANDGUN: i64 = 1;
const RIFLE: i64 = 2;
const SUPPRESSOR: i64 = 5;

const SELF_OR_BELOW: &str = "A firearm can't be mounted on itself, or on something mounted on it.";
const CHOOSE_ACTIVE: &str = "Choose an active firearm or accessory.";

// --- Building records --------------------------------------------------------------------

fn rifle(serial: &str) -> FirearmInput {
    FirearmInput {
        firearm_type_id: RIFLE,
        caliber: ".308 Winchester".into(),
        ..firearm("Ruger", "Precision", serial)
    }
}

fn suppressor(serial: &str) -> FirearmInput {
    FirearmInput {
        firearm_type_id: SUPPRESSOR,
        caliber: ".22 WMR".into(),
        ..firearm("SilencerCo", "Sparrow", serial)
    }
}

fn accessory(kind: i64, make: &str, model: &str) -> AccessoryInput {
    let mut input: AccessoryInput =
        serde_json::from_value(json!({ "accessoryKindId": kind, "status": "active" })).unwrap();
    input.make = Some(make.into());
    input.model = Some(model.into());
    input
}

fn add_firearm(db: &TestDb, input: &FirearmInput) -> RecordRef {
    RecordRef::Firearm(firearm_ops::create_firearm(&db.conn, input, false).unwrap().id)
}

fn add_accessory(db: &TestDb, input: &AccessoryInput) -> RecordRef {
    RecordRef::Accessory(accessory_ops::create_accessory(&db.conn, input).unwrap().id)
}

/// A firearm named by make and model alone, of the given type.
fn plain_firearm(db: &TestDb, make: &str, model: &str, serial: &str, type_id: i64) -> RecordRef {
    add_firearm(db, &FirearmInput { firearm_type_id: type_id, ..firearm(make, model, serial) })
}

fn plain_accessory(db: &TestDb, kind: i64, make: &str, model: &str) -> RecordRef {
    add_accessory(db, &accessory(kind, make, model))
}

// --- Driving the commands ------------------------------------------------------------------

/// `mount_record` for the contract's `{ item, host }`, output as JSON.
fn mount(db: &TestDb, item: RecordRef, host: Option<RecordRef>) -> Result<Value, CommandError> {
    let input = json!({ "item": item, "host": host });
    ops::mount_record(&db.conn, &serde_json::from_value(input).unwrap())
        .map(|output| serde_json::to_value(output).unwrap())
}

fn candidates(db: &TestDb, input: Value) -> Vec<Value> {
    let output =
        ops::list_mount_candidates(&db.conn, &serde_json::from_value(input).unwrap()).unwrap();
    serde_json::to_value(output).unwrap()["candidates"].as_array().unwrap().clone()
}

/// The records `list_mount_candidates` returns, in order.
fn candidate_records(db: &TestDb, input: Value) -> Vec<RecordRef> {
    candidates(db, input).iter().map(|c| record_of(&c["label"])).collect()
}

fn record_of(label: &Value) -> RecordRef {
    serde_json::from_value(label["record"].clone()).unwrap()
}

/// `get_firearm` or `get_accessory` as the frontend receives it.
fn detail(db: &TestDb, record: RecordRef) -> Value {
    match record {
        RecordRef::Firearm(id) => {
            serde_json::to_value(firearm_ops::get_firearm_detail(&db.conn, id).unwrap()).unwrap()
        }
        RecordRef::Accessory(id) => {
            serde_json::to_value(accessory_ops::get_accessory(&db.conn, id).unwrap()).unwrap()
        }
    }
}

/// The records on a record's chain, direct host first.
fn chain(db: &TestDb, record: RecordRef) -> Vec<RecordRef> {
    detail(db, record)["mount"]["chain"].as_array().unwrap().iter().map(record_of).collect()
}

/// `(record, host, depth)` of everything below a record, depth-first.
fn mounted(db: &TestDb, record: RecordRef) -> Vec<(RecordRef, RecordRef, u64)> {
    detail(db, record)["mount"]["mounted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                record_of(&entry["label"]),
                serde_json::from_value(entry["host"].clone()).unwrap(),
                entry["depth"].as_u64().unwrap(),
            )
        })
        .collect()
}

/// The direct host a record's own `mountedOn` names.
fn mounted_on(db: &TestDb, record: RecordRef) -> Option<RecordRef> {
    serde_json::from_value(detail(db, record)["mountedOn"].clone()).unwrap()
}

fn dispose_input() -> Value {
    json!({ "dispositionType": "sold", "recipient": "Jane Doe", "date": "2025-06-15", "price": 400 })
}

fn dispose(db: &TestDb, record: RecordRef) {
    let input = serde_json::from_value(dispose_input()).unwrap();
    match record {
        RecordRef::Firearm(id) => {
            firearm_ops::dispose_firearm(&db.conn, id, &input).unwrap();
        }
        RecordRef::Accessory(id) => {
            accessory_ops::dispose_accessory(&db.conn, id, &input).unwrap();
        }
    }
}

/// Saves the record's form with `mountedOn` set (the form's own path).
fn save_mounted_on(
    db: &TestDb,
    record: RecordRef,
    host: Option<RecordRef>,
) -> Result<(), CommandError> {
    match record {
        RecordRef::Firearm(id) => {
            let current = firearm_ops::get_firearm(&db.conn, id).unwrap();
            let input = FirearmInput { mounted_on: host, ..FirearmInput::from(&current) };
            firearm_ops::update_firearm(&db.conn, id, &input, false).map(|_| ())
        }
        RecordRef::Accessory(id) => {
            let current =
                serde_json::to_value(accessory_ops::get_accessory(&db.conn, id).unwrap()).unwrap();
            let mut input: AccessoryInput = serde_json::from_value(current).unwrap();
            input.mounted_on = host;
            accessory_ops::update_accessory(&db.conn, id, &input).map(|_| ())
        }
    }
}

fn field_error(err: &CommandError, field: &str) -> Option<String> {
    err.field_errors.as_ref().and_then(|errors| errors.get(field).cloned())
}

fn assert_field_error(err: CommandError, field: &str, message: &str) {
    assert_eq!(err.code, "VALIDATION_ERROR", "{err:?}");
    assert_eq!(field_error(&err, field).as_deref(), Some(message), "{err:?}");
}

fn mount_rows(db: &TestDb) -> i64 {
    db.conn.query_row("SELECT count(*) FROM mounts", [], |r| r.get(0)).unwrap()
}

/// How many `mounts` rows have this record as their item.
fn rows_for_item(db: &TestDb, item: RecordRef) -> i64 {
    let column = match item {
        RecordRef::Firearm(_) => "item_firearm_id",
        RecordRef::Accessory(_) => "item_accessory_id",
    };
    db.conn
        .query_row(&format!("SELECT count(*) FROM mounts WHERE {column} = ?1"), [item.id()], |r| {
            r.get(0)
        })
        .unwrap()
}

// --- Mounting and the two views of it (US2-1, US2-2) ---------------------------------------

#[test]
fn saving_an_optics_mounted_on_shows_on_both_records() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let optic = plain_accessory(&db, OPTIC, "Leupold", "VX-5HD");

    save_mounted_on(&db, optic, Some(rifle)).unwrap();

    assert_eq!(mounted_on(&db, optic), Some(rifle), "the accessory's own field");
    let optic_chain = detail(&db, optic)["mount"]["chain"].clone();
    assert_eq!(optic_chain.as_array().unwrap().len(), 1);
    let host_label = &optic_chain[0];
    assert_eq!(record_of(host_label), rifle);
    assert_eq!(host_label["make"], "Ruger");
    assert_eq!(host_label["model"], "Precision");
    assert_eq!(host_label["typeName"], "Rifle");
    assert_eq!(host_label["status"], "active");
    assert_eq!(mounted(&db, rifle), [(optic, rifle, 1)], "the rifle lists the optic at depth 1");
    assert_eq!(mounted_on(&db, rifle), None);
    assert!(chain(&db, rifle).is_empty());
}

#[test]
fn a_new_accessory_can_be_created_already_mounted() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let mut input = accessory(OPTIC, "Leupold", "VX-5HD");
    input.mounted_on = Some(rifle);

    let optic = add_accessory(&db, &input);

    assert_eq!(chain(&db, optic), [rifle]);
    assert_eq!(mounted(&db, rifle), [(optic, rifle, 1)]);
}

#[test]
fn a_new_firearm_can_be_created_already_mounted_on_an_accessory() {
    let db = TestDb::new();
    let case = plain_accessory(&db, CASE, "Pelican", "1750");
    let input = FirearmInput { mounted_on: Some(case), ..rifle("R-1") };

    let rifle = add_firearm(&db, &input);

    assert_eq!(mounted_on(&db, rifle), Some(case));
    assert_eq!(mounted(&db, case), [(rifle, case, 1)]);
}

#[test]
fn mount_record_puts_a_suppressor_firearm_on_a_rifle_and_shows_on_both() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let can = add_firearm(&db, &suppressor("S-1"));

    let output = mount(&db, can, Some(rifle)).unwrap();

    assert_eq!(record_of(&output["item"]), can);
    assert_eq!(record_of(&output["host"]), rifle);
    assert_eq!(output["item"]["typeName"], "Suppressor");
    assert_eq!(mounted_on(&db, can), Some(rifle));
    assert_eq!(chain(&db, can), [rifle]);
    assert_eq!(mounted(&db, rifle), [(can, rifle, 1)]);
}

// --- Moving and unmounting (US2-3, US2-5, FR-012, FR-013) ----------------------------------

#[test]
fn mounting_on_another_host_moves_the_item_in_one_step() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let ar = plain_firearm(&db, "Colt", "AR-15", "AR-1", RIFLE);
    let can = add_firearm(&db, &suppressor("S-1"));
    mount(&db, can, Some(rifle)).unwrap();

    let output = mount(&db, can, Some(ar)).unwrap();

    assert_eq!(record_of(&output["host"]), ar);
    assert_eq!(rows_for_item(&db, can), 1, "one row, not two");
    assert_eq!(mount_rows(&db), 1);
    assert!(mounted(&db, rifle).is_empty()); // it left the rifle
    assert_eq!(mounted(&db, ar), [(can, ar, 1)]);
    assert_eq!(chain(&db, can), [ar]);
}

#[test]
fn unmounting_deletes_the_row_and_keeps_no_record_of_it() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let optic = plain_accessory(&db, OPTIC, "Leupold", "VX-5HD");
    mount(&db, optic, Some(rifle)).unwrap();
    assert_eq!(mount_rows(&db), 1);

    let output = mount(&db, optic, None).unwrap();

    assert_eq!(record_of(&output["item"]), optic);
    assert_eq!(output["host"], Value::Null);
    assert_eq!(mount_rows(&db), 0, "no row, so no history (FR-013)");
    assert_eq!(mounted_on(&db, optic), None);
    assert!(chain(&db, optic).is_empty());
    assert!(mounted(&db, rifle).is_empty());
    // Unmounting what is not mounted is not an error.
    assert_eq!(mount(&db, optic, None).unwrap()["host"], Value::Null);
    assert_eq!(mount_rows(&db), 0);
}

#[test]
fn saving_the_form_with_no_mounted_on_unmounts_and_with_another_moves() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let ar = plain_firearm(&db, "Colt", "AR-15", "AR-1", RIFLE);
    let optic = plain_accessory(&db, OPTIC, "Leupold", "VX-5HD");
    save_mounted_on(&db, optic, Some(rifle)).unwrap();

    save_mounted_on(&db, optic, Some(ar)).unwrap();
    assert_eq!(chain(&db, optic), [ar]);
    assert_eq!(mount_rows(&db), 1);

    save_mounted_on(&db, optic, None).unwrap();
    assert!(chain(&db, optic).is_empty());
    assert_eq!(mount_rows(&db), 0);
}

// --- The depth-first list and the chain (US2-10, US2-12) -----------------------------------

#[test]
fn a_receiver_lists_what_is_on_it_depth_first_and_a_red_dot_names_its_chain() {
    let db = TestDb::new();
    let receiver = plain_firearm(&db, "LaRue", "PredatAR", "L-1", RIFLE);
    let upper = plain_accessory(&db, UPPER, "BCM", "RECCE-16");
    let scope = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    let red_dot = plain_accessory(&db, OPTIC, "Aimpoint", "T-2");
    let light = plain_accessory(&db, LIGHT, "SureFire", "M300");
    mount(&db, upper, Some(receiver)).unwrap();
    mount(&db, scope, Some(upper)).unwrap();
    mount(&db, red_dot, Some(scope)).unwrap();
    mount(&db, light, Some(upper)).unwrap();

    assert_eq!(
        mounted(&db, receiver),
        [(upper, receiver, 1), (scope, upper, 2), (red_dot, scope, 3), (light, upper, 2)],
        "depth-first, siblings in the order they were recorded"
    );
    assert_eq!(chain(&db, red_dot), [scope, upper, receiver]);
    assert_eq!(chain(&db, light), [upper, receiver]);
    assert_eq!(
        mounted(&db, upper),
        [(scope, upper, 1), (red_dot, scope, 2), (light, upper, 1)],
        "an accessory's own list starts at its depth 1"
    );
    assert!(mounted(&db, red_dot).is_empty());
}

// --- The subtree follows (US2-9, US2-13, US2-14) -------------------------------------------

#[test]
fn unmounting_an_upper_keeps_its_optic_and_mounting_it_elsewhere_moves_both() {
    let db = TestDb::new();
    let first = plain_firearm(&db, "LaRue", "PredatAR", "L-1", RIFLE);
    let second = plain_firearm(&db, "Colt", "AR-15", "C-1", RIFLE);
    let upper = plain_accessory(&db, UPPER, "BCM", "RECCE-16");
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, upper, Some(first)).unwrap();
    mount(&db, optic, Some(upper)).unwrap();

    mount(&db, upper, None).unwrap();

    assert!(chain(&db, upper).is_empty());
    assert_eq!(chain(&db, optic), [upper], "the optic stays on the upper");
    assert!(mounted(&db, first).is_empty());

    mount(&db, upper, Some(second)).unwrap();

    assert_eq!(mounted(&db, second), [(upper, second, 1), (optic, upper, 2)]);
    assert_eq!(chain(&db, optic), [upper, second]);
    assert_eq!(mount_rows(&db), 2);
}

#[test]
fn moving_an_optic_to_a_second_upper_removes_it_from_the_first() {
    let db = TestDb::new();
    let receiver = plain_firearm(&db, "LaRue", "PredatAR", "L-1", RIFLE);
    let first = plain_accessory(&db, UPPER, "BCM", "RECCE-16");
    let second = plain_accessory(&db, UPPER, "Daniel Defense", "MK18");
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, first, Some(receiver)).unwrap();
    mount(&db, optic, Some(first)).unwrap();

    mount(&db, optic, Some(second)).unwrap();

    assert!(mounted(&db, first).is_empty());
    assert_eq!(mounted(&db, second), [(optic, second, 1)]);
    assert_eq!(mounted(&db, receiver), [(first, receiver, 1)], "the first upper stays");
    assert_eq!(chain(&db, optic), [second]);
    assert_eq!(rows_for_item(&db, optic), 1);
}

// --- Nothing is judged (US2-7, US2-8, US2-15, FR-011) --------------------------------------

#[test]
fn an_unlikely_mount_is_made_without_a_warning_or_a_check_of_kind_type_or_caliber() {
    let db = TestDb::new();
    let optic_a = plain_accessory(&db, OPTIC, "Leupold", "VX-5HD");
    let optic_b = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    let upper = plain_accessory(&db, UPPER, "BCM", "RECCE-16");
    let sling = plain_accessory(&db, SLING, "Magpul", "MS1");
    let can = add_firearm(&db, &suppressor("S-1"));
    let rifle = add_firearm(&db, &rifle("R-1"));
    let pistol = plain_firearm(&db, "Glock", "19", "G-1", HANDGUN);
    let magazines = add_accessory(&db, &{
        let mut pair = accessory(MAGAZINE, "Magpul", "PMAG");
        pair.notes = Some("A pair".into());
        pair
    });

    // An Optic on an Optic; an Upper on a Sling; a Suppressor firearm on an
    // Upper; a .22 WMR Suppressor on a .308 Rifle; magazines on a pistol.
    for (item, host) in
        [(optic_a, optic_b), (upper, sling), (can, upper), (can, rifle), (magazines, pistol)]
    {
        let output = mount(&db, item, Some(host)).unwrap();
        assert_eq!(record_of(&output["host"]), host);
        assert!(output.get("warnings").is_none(), "no warning: {output}");
    }

    assert_eq!(chain(&db, optic_a), [optic_b]);
    assert_eq!(chain(&db, upper), [sling]);
    assert_eq!(chain(&db, can), [rifle]);
    assert_eq!(chain(&db, magazines), [pistol]);
}

#[test]
fn a_form_save_judges_nothing_either() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let can = add_firearm(&db, &suppressor("S-1"));
    let upper = plain_accessory(&db, UPPER, "BCM", "RECCE-16");

    save_mounted_on(&db, can, Some(rifle)).unwrap();
    save_mounted_on(&db, rifle, Some(upper)).unwrap();

    assert_eq!(chain(&db, can), [rifle, upper]);
}

// --- Loops, disposed and missing hosts (US2-16, FR-010) ------------------------------------

#[test]
fn a_record_cannot_be_mounted_on_itself_through_mount_record_or_the_form() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let optic = plain_accessory(&db, OPTIC, "Leupold", "VX-5HD");

    assert_field_error(mount(&db, rifle, Some(rifle)).unwrap_err(), "host", SELF_OR_BELOW);
    assert_field_error(mount(&db, optic, Some(optic)).unwrap_err(), "host", SELF_OR_BELOW);
    assert_field_error(
        save_mounted_on(&db, rifle, Some(rifle)).unwrap_err(),
        "mountedOn",
        SELF_OR_BELOW,
    );
    assert_field_error(
        save_mounted_on(&db, optic, Some(optic)).unwrap_err(),
        "mountedOn",
        SELF_OR_BELOW,
    );
    assert_eq!(mount_rows(&db), 0);
}

#[test]
fn an_upper_cannot_be_mounted_on_its_own_optic_or_anything_further_down() {
    let db = TestDb::new();
    let upper = plain_accessory(&db, UPPER, "BCM", "RECCE-16");
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    let red_dot = plain_accessory(&db, OPTIC, "Aimpoint", "T-2");
    mount(&db, optic, Some(upper)).unwrap();
    mount(&db, red_dot, Some(optic)).unwrap();

    assert_field_error(mount(&db, upper, Some(optic)).unwrap_err(), "host", SELF_OR_BELOW);
    assert_field_error(mount(&db, upper, Some(red_dot)).unwrap_err(), "host", SELF_OR_BELOW);
    assert_field_error(
        save_mounted_on(&db, upper, Some(optic)).unwrap_err(),
        "mountedOn",
        SELF_OR_BELOW,
    );

    // Nothing changed.
    assert_eq!(mount_rows(&db), 2);
    assert_eq!(chain(&db, red_dot), [optic, upper]);
    assert!(chain(&db, upper).is_empty());
}

#[test]
fn a_failed_move_leaves_the_item_where_it_was() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let upper = plain_accessory(&db, UPPER, "BCM", "RECCE-16");
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, upper, Some(rifle)).unwrap();
    mount(&db, optic, Some(upper)).unwrap();

    // The rifle's own host would be below it.
    assert_field_error(mount(&db, rifle, Some(optic)).unwrap_err(), "host", SELF_OR_BELOW);
    assert_eq!(chain(&db, optic), [upper, rifle]);
    assert!(chain(&db, rifle).is_empty());
}

#[test]
fn a_disposed_or_missing_host_is_refused() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let sold_case = plain_accessory(&db, CASE, "Pelican", "1750");
    let optic = plain_accessory(&db, OPTIC, "Leupold", "VX-5HD");
    dispose(&db, sold_case);

    assert_field_error(mount(&db, optic, Some(sold_case)).unwrap_err(), "host", CHOOSE_ACTIVE);
    assert_field_error(
        mount(&db, optic, Some(RecordRef::Firearm(9_999))).unwrap_err(),
        "host",
        CHOOSE_ACTIVE,
    );
    assert_field_error(
        mount(&db, optic, Some(RecordRef::Accessory(9_999))).unwrap_err(),
        "host",
        CHOOSE_ACTIVE,
    );
    assert_field_error(
        save_mounted_on(&db, optic, Some(sold_case)).unwrap_err(),
        "mountedOn",
        CHOOSE_ACTIVE,
    );
    let mut new_record = accessory(OPTIC, "Vortex", "Razor");
    new_record.mounted_on = Some(RecordRef::Firearm(9_999));
    assert_field_error(
        accessory_ops::create_accessory(&db.conn, &new_record).unwrap_err(),
        "mountedOn",
        CHOOSE_ACTIVE,
    );
    assert_eq!(mount_rows(&db), 0);
    // Mounting on an active host still works.
    mount(&db, optic, Some(rifle)).unwrap();
}

#[test]
fn a_missing_item_is_not_found() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));

    for missing in [RecordRef::Firearm(9_999), RecordRef::Accessory(9_999)] {
        let err = mount(&db, missing, Some(rifle)).unwrap_err();
        assert_eq!(err.code, "NOT_FOUND", "{err:?}");
        let err = mount(&db, missing, None).unwrap_err();
        assert_eq!(err.code, "NOT_FOUND", "{err:?}");
    }
}

// --- list_mount_candidates (US2-3a, US2-4, US2-6, US2-16) ----------------------------------

#[test]
fn host_candidates_leave_out_the_record_what_is_below_it_and_anything_disposed() {
    let db = TestDb::new();
    let upper = plain_accessory(&db, UPPER, "BCM", "RECCE-16");
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    let red_dot = plain_accessory(&db, OPTIC, "Aimpoint", "T-2");
    let receiver = plain_firearm(&db, "LaRue", "PredatAR", "L-1", RIFLE);
    let sling = plain_accessory(&db, SLING, "Magpul", "MS1");
    let sold = plain_accessory(&db, CASE, "Pelican", "1750");
    let sold_firearm = plain_firearm(&db, "Glock", "19", "G-1", HANDGUN);
    mount(&db, optic, Some(upper)).unwrap();
    mount(&db, red_dot, Some(optic)).unwrap();
    mount(&db, upper, Some(receiver)).unwrap();
    dispose(&db, sold);
    dispose(&db, sold_firearm);

    let found: Vec<RecordRef> =
        candidate_records(&db, json!({ "role": "host", "record": upper, "query": "" }));

    let mut found_sorted = found.clone();
    found_sorted.sort_by_key(|r| (r.kind() as u8, r.id()));
    let mut expected = vec![receiver, sling];
    expected.sort_by_key(|r| (r.kind() as u8, r.id()));
    assert_eq!(found_sorted, expected, "active records that are not the upper or below it");

    // For a new record there is nothing to leave out but the disposed.
    let for_new = candidate_records(&db, json!({ "role": "host", "record": null, "query": "" }));
    assert_eq!(for_new.len(), 5);
    assert!(!for_new.contains(&sold) && !for_new.contains(&sold_firearm));
}

#[test]
fn item_candidates_leave_out_the_host_its_chain_and_what_is_directly_on_it() {
    let db = TestDb::new();
    let case = plain_accessory(&db, CASE, "Pelican", "1750");
    let ar = plain_firearm(&db, "Colt", "AR-15", "C-1", RIFLE);
    let on_ar = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    let red_dot = plain_accessory(&db, OPTIC, "Aimpoint", "T-2");
    let rifle =
        add_firearm(&db, &FirearmInput { nickname: Some("Deer rifle".into()), ..rifle("R-1") });
    let on_rifle = plain_accessory(&db, OPTIC, "Leupold", "VX-5HD");
    let loose = plain_accessory(&db, LIGHT, "SureFire", "M300");
    mount(&db, ar, Some(case)).unwrap();
    mount(&db, on_ar, Some(ar)).unwrap();
    mount(&db, red_dot, Some(on_ar)).unwrap();
    mount(&db, on_rifle, Some(rifle)).unwrap();

    let found = candidates(&db, json!({ "role": "item", "record": ar, "query": "" }));

    let records: Vec<RecordRef> = found.iter().map(|c| record_of(&c["label"])).collect();
    assert!(!records.contains(&ar), "not the host");
    assert!(!records.contains(&case), "not the host's chain");
    assert!(!records.contains(&on_ar), "not what is already directly on it");
    for kept in [red_dot, rifle, on_rifle, loose] {
        assert!(records.contains(&kept), "{kept:?} is a candidate: {records:?}");
    }
    assert_eq!(records.len(), 4);
    let for_record = |record: RecordRef| {
        found.iter().find(|c| record_of(&c["label"]) == record).unwrap().clone()
    };
    // US2-3a: a candidate that is mounted says where, with the host's label.
    let on_rifle_entry = for_record(on_rifle);
    assert_eq!(record_of(&on_rifle_entry["mountedOn"]), rifle);
    assert_eq!(on_rifle_entry["mountedOn"]["nickname"], "Deer rifle");
    assert_eq!(on_rifle_entry["label"]["make"], "Leupold");
    assert_eq!(record_of(&for_record(red_dot)["mountedOn"]), on_ar, "its direct host only");
    assert_eq!(for_record(loose)["mountedOn"], Value::Null);
    assert_eq!(for_record(rifle)["mountedOn"], Value::Null);
}

#[test]
fn candidates_match_inside_make_model_nickname_and_serial_number_without_regard_to_case() {
    let db = TestDb::new();
    let deer =
        add_firearm(&db, &FirearmInput { nickname: Some("Deer rifle".into()), ..rifle("SN-8841") });
    let glock = plain_firearm(&db, "Glock", "19", "G-1", HANDGUN);
    let optic = add_accessory(&db, &{
        let mut input = accessory(OPTIC, "Leupold", "VX-5HD");
        input.serial_number = Some("XQ-77".into());
        input
    });

    let query = |text: &str| {
        candidate_records(&db, json!({ "role": "host", "record": null, "query": text }))
    };

    assert_eq!(query("deer R"), [deer], "nickname, inside the value, any case");
    assert_eq!(query("RECISION"), [deer], "model");
    assert_eq!(query("ruger"), [deer], "make");
    assert_eq!(query("sn-88"), [deer], "serial number");
    assert_eq!(query("lock"), [glock]);
    assert_eq!(query("vx-5"), [optic]);
    assert_eq!(query("xq-7"), [optic], "an accessory's serial number");
    assert_eq!(query("  deer  "), [deer], "surrounding whitespace is ignored");
    assert!(query("no such thing").is_empty());
}

#[test]
fn candidates_come_by_name_then_id() {
    let db = TestDb::new();
    let zeta = plain_firearm(&db, "Zeta", "One", "Z-1", RIFLE);
    let alpha_two = plain_firearm(&db, "Alpha", "Same", "A-2", RIFLE);
    let alpha_one = plain_firearm(&db, "Alpha", "Same", "A-1", RIFLE);
    let alpha_other = plain_firearm(&db, "Alpha", "Other", "A-3", RIFLE);

    let found = candidate_records(&db, json!({ "role": "host", "record": null, "query": "" }));

    assert_eq!(found, [alpha_other, alpha_two, alpha_one, zeta], "make, model, then id");
}

#[test]
fn an_empty_query_returns_the_first_fifty_and_the_limit_is_at_most_a_hundred() {
    let db = TestDb::new();
    for n in 0..105 {
        plain_accessory(&db, SLING, "Magpul", &format!("MS{n:03}"));
    }

    let default = candidates(&db, json!({ "role": "host", "record": null, "query": "" }));
    let ten = candidates(&db, json!({ "role": "host", "record": null, "query": "", "limit": 10 }));
    let more =
        candidates(&db, json!({ "role": "host", "record": null, "query": "", "limit": 1000 }));

    assert_eq!(default.len(), 50);
    assert_eq!(default[0]["label"]["model"], "MS000");
    assert_eq!(default[49]["label"]["model"], "MS049");
    assert_eq!(ten.len(), 10);
    assert_eq!(more.len(), 100, "capped");
}

// --- Disposing (FR-014's default) ----------------------------------------------------------

#[test]
fn disposing_a_rifle_leaves_what_was_mounted_on_it_active_and_unmounted() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    let red_dot = plain_accessory(&db, OPTIC, "Aimpoint", "T-2");
    let can = add_firearm(&db, &suppressor("S-1"));
    mount(&db, optic, Some(rifle)).unwrap();
    mount(&db, can, Some(rifle)).unwrap();
    mount(&db, red_dot, Some(optic)).unwrap();

    dispose(&db, rifle);

    for kept in [optic, can] {
        assert_eq!(detail(&db, kept)["status"], "active");
        assert_eq!(mounted_on(&db, kept), None);
        assert!(chain(&db, kept).is_empty());
    }
    assert!(mounted(&db, rifle).is_empty()); // a disposed record lists nothing
    assert_eq!(chain(&db, red_dot), [optic], "kept records keep their mounts on kept records");
    assert_eq!(mount_rows(&db), 1);
}

#[test]
fn disposing_a_mounted_optic_unmounts_it() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, optic, Some(rifle)).unwrap();

    dispose(&db, optic);

    assert!(mounted(&db, rifle).is_empty());
    assert_eq!(detail(&db, optic)["status"], "disposed");
    assert!(chain(&db, optic).is_empty());
    assert_eq!(mount_rows(&db), 0);
}

// --- The table's own backstops (data-model.md "Rules") -------------------------------------

fn sql_error(db: &TestDb, sql: &str) -> String {
    db.conn.execute(sql, []).unwrap_err().to_string()
}

#[test]
fn a_mount_row_with_a_disposed_item_or_host_aborts() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    let sold_optic = plain_accessory(&db, OPTIC, "Leupold", "VX-5HD");
    let sold_case = plain_accessory(&db, CASE, "Pelican", "1750");
    dispose(&db, sold_optic);
    dispose(&db, sold_case);
    let (r, o, so, sc) = (rifle.id(), optic.id(), sold_optic.id(), sold_case.id());

    let item = sql_error(
        &db,
        &format!("INSERT INTO mounts (item_accessory_id, host_firearm_id) VALUES ({so}, {r})"),
    );
    let host = sql_error(
        &db,
        &format!("INSERT INTO mounts (item_accessory_id, host_accessory_id) VALUES ({o}, {sc})"),
    );

    assert!(item.contains("a mount needs an active item and an active host"), "{item}");
    assert!(host.contains("a mount needs an active item and an active host"), "{host}");
    assert_eq!(mount_rows(&db), 0);
}

#[test]
fn a_second_row_for_the_same_item_fails_unique() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let ar = plain_firearm(&db, "Colt", "AR-15", "C-1", RIFLE);
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, optic, Some(rifle)).unwrap();

    let message = sql_error(
        &db,
        &format!(
            "INSERT INTO mounts (item_accessory_id, host_firearm_id) VALUES ({}, {})",
            optic.id(),
            ar.id()
        ),
    );

    assert!(message.contains("UNIQUE"), "{message}");
    assert_eq!(mount_rows(&db), 1);
}

#[test]
fn a_one_step_self_mount_fails_the_check() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");

    let firearm_message = sql_error(
        &db,
        &format!(
            "INSERT INTO mounts (item_firearm_id, host_firearm_id) VALUES ({0}, {0})",
            rifle.id()
        ),
    );
    let accessory_message = sql_error(
        &db,
        &format!(
            "INSERT INTO mounts (item_accessory_id, host_accessory_id) VALUES ({0}, {0})",
            optic.id()
        ),
    );

    assert!(firearm_message.contains("CHECK"), "{firearm_message}");
    assert!(accessory_message.contains("CHECK"), "{accessory_message}");
    assert_eq!(mount_rows(&db), 0);
}

#[test]
fn disposing_a_mounted_record_by_sql_aborts_whether_it_is_the_item_or_the_host() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let upper = plain_accessory(&db, UPPER, "BCM", "RECCE-16");
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, upper, Some(rifle)).unwrap();
    mount(&db, optic, Some(upper)).unwrap();

    let firearm_host = sql_error(
        &db,
        &format!("UPDATE firearms SET status = 'disposed' WHERE id = {}", rifle.id()),
    );
    let accessory_host = sql_error(
        &db,
        &format!("UPDATE accessories SET status = 'disposed' WHERE id = {}", upper.id()),
    );
    let accessory_item = sql_error(
        &db,
        &format!("UPDATE accessories SET status = 'disposed' WHERE id = {}", optic.id()),
    );

    for message in [firearm_host, accessory_host, accessory_item] {
        assert!(
            message.contains("a disposed record cannot be mounted or carry mounts"),
            "{message}"
        );
    }
    assert_eq!(mount_rows(&db), 2);
}

#[test]
fn a_firearm_item_disposed_by_sql_aborts_too() {
    let db = TestDb::new();
    let upper = plain_accessory(&db, UPPER, "BCM", "RECCE-16");
    let can = add_firearm(&db, &suppressor("S-1"));
    mount(&db, can, Some(upper)).unwrap();

    let message =
        sql_error(&db, &format!("UPDATE firearms SET status = 'disposed' WHERE id = {}", can.id()));

    assert!(message.contains("a disposed record cannot be mounted or carry mounts"), "{message}");
}

#[test]
fn deleting_either_record_cascades_its_rows_away() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, &rifle("R-1"));
    let upper = plain_accessory(&db, UPPER, "BCM", "RECCE-16");
    let optic = plain_accessory(&db, OPTIC, "Vortex", "Razor");
    let can = add_firearm(&db, &suppressor("S-1"));
    mount(&db, upper, Some(rifle)).unwrap();
    mount(&db, optic, Some(upper)).unwrap();
    mount(&db, can, Some(upper)).unwrap();
    assert_eq!(mount_rows(&db), 3);

    // The accessory host: its own mount and what is on it all go.
    db.conn.execute("DELETE FROM accessories WHERE id = ?1", [upper.id()]).unwrap();
    assert_eq!(mount_rows(&db), 0);
    assert_eq!(detail(&db, optic)["status"], "active", "the records stay");
    assert_eq!(mounted_on(&db, optic), None);

    mount(&db, optic, Some(rifle)).unwrap();
    assert_eq!(mount_rows(&db), 1);
    // The firearm host, through the command.
    firearm_ops::delete_firearm(&db.conn, rifle.id(), true).unwrap();
    assert_eq!(mount_rows(&db), 0);
    assert_eq!(mounted_on(&db, optic), None);
}

// --- SC-004: a random walk over every operation (tasks.md T080) ----------------------------

/// A fixed-seed generator (SplitMix64), so the walk is the same every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n`.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        (!items.is_empty()).then(|| items[self.below(items.len())])
    }
}

/// Every record's status, read from the tables themselves.
fn status_map(db: &TestDb) -> std::collections::HashMap<RecordRef, String> {
    let mut found = std::collections::HashMap::new();
    for (table, make) in [
        ("firearms", RecordRef::Firearm as fn(i64) -> RecordRef),
        ("accessories", RecordRef::Accessory as fn(i64) -> RecordRef),
    ] {
        let mut stmt = db.conn.prepare(&format!("SELECT id, status FROM {table}")).unwrap();
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)));
        for row in rows.unwrap() {
            let (id, status) = row.unwrap();
            found.insert(make(id), status);
        }
    }
    found
}

/// Every `mounts` row as `(item, host)`; a second row for one item is
/// reported as a count mismatch by the caller.
fn mount_pairs(db: &TestDb) -> Vec<(RecordRef, RecordRef)> {
    let mut stmt = db
        .conn
        .prepare(
            "SELECT item_firearm_id, item_accessory_id, host_firearm_id, host_accessory_id
             FROM mounts",
        )
        .unwrap();
    let pair = |firearm: Option<i64>, accessory: Option<i64>| match (firearm, accessory) {
        (Some(id), None) => RecordRef::Firearm(id),
        (None, Some(id)) => RecordRef::Accessory(id),
        _ => panic!("a mounts row names exactly one record per side"),
    };
    stmt.query_map([], |r| Ok((pair(r.get(0)?, r.get(1)?), pair(r.get(2)?, r.get(3)?))))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn graph(db: &TestDb) -> hoplodex_lib::services::mounts::MountGraph {
    hoplodex_lib::services::mounts::MountGraph::load(&db.conn).unwrap()
}

/// SC-004's three invariants, which must hold after every step.
fn assert_mounts_sound(db: &TestDb, step: usize, what: &str) {
    let statuses = status_map(db);
    let pairs = mount_pairs(db);
    let items: std::collections::HashSet<RecordRef> = pairs.iter().map(|(item, _)| *item).collect();
    assert_eq!(items.len(), pairs.len(), "step {step} ({what}): an item has two rows");
    for (item, host) in &pairs {
        for record in [item, host] {
            assert_eq!(
                statuses.get(record).map(String::as_str),
                Some("active"),
                "step {step} ({what}): a mount involves {record:?}, which is disposed or deleted"
            );
        }
    }
    let graph = graph(db);
    for record in statuses.keys() {
        let (mut at, mut steps) = (*record, 0);
        while let Some(host) = graph.host_of(at) {
            steps += 1;
            assert!(
                host != *record && steps <= statuses.len(),
                "step {step} ({what}): {record:?} is part of a loop"
            );
            at = host;
        }
    }
}

/// Whether `record` is on the chain above (or is) `other`.
fn is_below(
    graph: &hoplodex_lib::services::mounts::MountGraph,
    record: RecordRef,
    other: RecordRef,
) -> bool {
    graph.chain(record).contains(&other)
}

/// The rule of FR-010, stated independently of the code under test: the
/// item and the host are active and the host is not the item or below it.
fn mount_allowed(db: &TestDb, item: RecordRef, host: Option<RecordRef>) -> bool {
    let Some(host) = host else { return true };
    let statuses = status_map(db);
    let active = |r: &RecordRef| statuses.get(r).map(String::as_str) == Some("active");
    active(&item) && active(&host) && host != item && !is_below(&graph(db), host, item)
}

fn dispose_with(
    db: &TestDb,
    record: RecordRef,
    with: &[(RecordRef, Option<i64>)],
) -> Result<(), CommandError> {
    let with: Vec<Value> =
        with.iter().map(|(record, price)| json!({ "record": record, "price": price })).collect();
    let mut input = dispose_input();
    input["withMounted"] = Value::Array(with);
    let input = serde_json::from_value(input).unwrap();
    match record {
        RecordRef::Firearm(id) => firearm_ops::dispose_firearm(&db.conn, id, &input).map(|_| ()),
        RecordRef::Accessory(id) => {
            accessory_ops::dispose_accessory(&db.conn, id, &input).map(|_| ())
        }
    }
}

fn restore_record(db: &TestDb, record: RecordRef) {
    let input = json!({ "history": "discard" });
    match record {
        RecordRef::Firearm(id) => {
            firearm_ops::reverse_disposition(&db.conn, id, &serde_json::from_value(input).unwrap())
                .unwrap();
        }
        RecordRef::Accessory(id) => {
            accessory_ops::reverse_accessory_disposition(
                &db.conn,
                id,
                &serde_json::from_value(input).unwrap(),
            )
            .unwrap();
        }
    }
}

fn delete_record(db: &TestDb, record: RecordRef) {
    match record {
        RecordRef::Firearm(id) => {
            firearm_ops::delete_firearm(&db.conn, id, true).unwrap();
        }
        RecordRef::Accessory(id) => {
            accessory_ops::delete_accessory(&db.conn, id, true).unwrap();
        }
    }
}

/// Counts of what the walk did, so it can't pass by doing nothing.
#[derive(Default, Debug)]
struct Coverage {
    mounted: usize,
    refused: usize,
    moved_with_records_below: usize,
    disposed_with_records: usize,
    disposed_keeping_records: usize,
    stale: usize,
    restored: usize,
    deleted_with_records_below: usize,
    created_mounted: usize,
}

#[test]
fn two_thousand_random_operations_never_break_the_mount_rules() {
    const STEPS: usize = 2_000;
    const MOST_RECORDS: usize = 14;
    let db = TestDb::new();
    let mut rng = Rng(0x00C0_FFEE_2006_0004);
    let mut serial = 0;
    let mut next_serial = || {
        serial += 1;
        format!("W-{serial}")
    };
    let mut records: Vec<RecordRef> = Vec::new();
    for _ in 0..3 {
        records.push(add_firearm(&db, &rifle(&next_serial())));
        records.push(add_firearm(&db, &suppressor(&next_serial())));
    }
    for kind in [OPTIC, LIGHT, MAGAZINE, UPPER, SLING, CASE] {
        records.push(plain_accessory(&db, kind, "Make", "Model"));
    }
    let mut seen = Coverage::default();

    for step in 1..=STEPS {
        let statuses = status_map(&db);
        let active: Vec<RecordRef> =
            records.iter().copied().filter(|r| statuses[r] == "active").collect();
        let disposed: Vec<RecordRef> =
            records.iter().copied().filter(|r| statuses[r] == "disposed").collect();
        let pairs_before = mount_pairs(&db);
        let graph_before = graph(&db);
        let what: String;

        match rng.below(100) {
            // A form save with a Mounted on value, or a mount_record call.
            0..=44 => {
                let by_form = rng.below(2) == 0;
                let item = if by_form { rng.pick(&active) } else { rng.pick(&records) };
                let Some(item) = item else { continue };
                let host = if rng.below(4) == 0 { None } else { rng.pick(&records) };
                what = format!(
                    "{} {item:?} on {host:?}",
                    if by_form { "form" } else { "mount_record" }
                );
                let below_before: Vec<RecordRef> =
                    graph_before.below(item).iter().map(|(r, _, _)| *r).collect();
                let expected = mount_allowed(&db, item, host)
                    || (!by_form && host.is_none())
                    || graph_before.host_of(item) == host;
                let result = if by_form {
                    save_mounted_on(&db, item, host)
                } else {
                    mount(&db, item, host).map(|_| ())
                };
                assert_eq!(result.is_ok(), expected, "step {step} ({what}): {result:?}");
                if result.is_ok() {
                    if host.is_some() {
                        seen.mounted += 1;
                    }
                    assert_eq!(graph(&db).host_of(item), host, "step {step} ({what})");
                    if !below_before.is_empty() {
                        seen.moved_with_records_below += 1;
                    }
                    let after = graph(&db);
                    for below in below_before {
                        assert!(
                            is_below(&after, below, item),
                            "step {step} ({what}): {below:?} is no longer below {item:?}"
                        );
                    }
                } else {
                    seen.refused += 1;
                    let mut after = mount_pairs(&db);
                    let mut before = pairs_before.clone();
                    after.sort_by_key(|p| format!("{p:?}"));
                    before.sort_by_key(|p| format!("{p:?}"));
                    assert_eq!(after, before, "step {step} ({what}): a refused mount changed rows");
                }
            }
            // Disposing, with a random subset of what is below.
            45..=64 => {
                let Some(host) = rng.pick(&active) else { continue };
                let below: Vec<RecordRef> =
                    graph_before.below(host).iter().map(|(r, _, _)| *r).collect();
                let mut chosen: Vec<(RecordRef, Option<i64>)> = Vec::new();
                for record in &below {
                    if rng.below(2) == 0 {
                        let price =
                            if rng.below(3) == 0 { None } else { Some(rng.below(500) as i64) };
                        chosen.push((*record, price));
                    }
                }
                let outsiders: Vec<RecordRef> =
                    records.iter().copied().filter(|r| !below.contains(r)).collect();
                let stale = rng.below(6) == 0 && !outsiders.is_empty();
                if stale {
                    let outsider = rng.pick(&outsiders).unwrap();
                    let at = rng.below(chosen.len() + 1);
                    chosen.insert(at, (outsider, None));
                }
                what = format!("dispose {host:?} with {chosen:?}");
                let result = dispose_with(&db, host, &chosen);
                if stale {
                    seen.stale += 1;
                    let err = result.expect_err(&format!("step {step} ({what}) should be stale"));
                    assert_field_error(
                        err,
                        "withMounted",
                        "What is mounted has changed. Close the dialog and try again.",
                    );
                    assert!(statuses_after_eq(&db, &statuses), "step {step} ({what})");
                    let mut after = mount_pairs(&db);
                    let mut before = pairs_before.clone();
                    after.sort_by_key(|p| format!("{p:?}"));
                    before.sort_by_key(|p| format!("{p:?}"));
                    assert_eq!(after, before, "step {step} ({what}): a stale call changed rows");
                } else {
                    result.unwrap_or_else(|e| panic!("step {step} ({what}): {e:?}"));
                    if chosen.is_empty() {
                        seen.disposed_keeping_records += usize::from(!below.is_empty());
                    } else {
                        seen.disposed_with_records += 1;
                    }
                    let after_statuses = status_map(&db);
                    assert_eq!(after_statuses[&host], "disposed", "step {step} ({what})");
                    let chosen_records: Vec<RecordRef> = chosen.iter().map(|(r, _)| *r).collect();
                    for (record, price) in &chosen {
                        assert_eq!(after_statuses[record], "disposed", "step {step} ({what})");
                        let shown = detail(&db, *record);
                        assert_eq!(shown["dispositionPrice"], json!(price), "step {step} ({what})");
                        assert_eq!(shown["dispositionRecipient"], "Jane Doe");
                    }
                    let after = graph(&db);
                    for r in below.iter().filter(|r| !chosen_records.contains(r)) {
                        assert_eq!(after_statuses[r], "active", "step {step} ({what}): {r:?} kept");
                        // A kept record keeps a mount on a kept record, and
                        // loses one on a disposed record.
                        let before_host = graph_before.host_of(*r).unwrap();
                        let expected = (before_host != host
                            && !chosen_records.contains(&before_host))
                        .then_some(before_host);
                        assert_eq!(after.host_of(*r), expected, "step {step} ({what}): {r:?}");
                    }
                }
            }
            // Restoring a disposed record: it comes back unmounted.
            65..=77 => {
                let Some(record) = rng.pick(&disposed) else { continue };
                what = format!("restore {record:?}");
                restore_record(&db, record);
                seen.restored += 1;
                assert_eq!(status_map(&db)[&record], "active", "step {step} ({what})");
                assert_eq!(graph(&db).host_of(record), None, "step {step} ({what})");
                assert!(graph(&db).below(record).is_empty(), "step {step} ({what})");
            }
            // Deleting anything: what was on it stays, unmounted.
            78..=87 => {
                let Some(record) = rng.pick(&records) else { continue };
                what = format!("delete {record:?}");
                let hosts: Vec<(RecordRef, RecordRef)> = pairs_before.clone();
                delete_record(&db, record);
                records.retain(|r| *r != record);
                if hosts.iter().any(|(_, host)| *host == record) {
                    seen.deleted_with_records_below += 1;
                }
                let after = graph(&db);
                let after_statuses = status_map(&db);
                assert!(!after_statuses.contains_key(&record), "step {step} ({what})");
                for (item, host) in hosts {
                    if item == record {
                        continue;
                    }
                    let expected = (host != record).then_some(host);
                    assert_eq!(after.host_of(item), expected, "step {step} ({what}): {item:?}");
                    assert!(after_statuses.contains_key(&item), "step {step} ({what}): {item:?}");
                }
            }
            // Creating a record, sometimes already mounted.
            _ => {
                if records.len() >= MOST_RECORDS {
                    continue;
                }
                let host = if rng.below(3) == 0 { rng.pick(&active) } else { None };
                what = format!("create on {host:?}");
                let created = match rng.below(3) {
                    0 => add_firearm(
                        &db,
                        &FirearmInput { mounted_on: host, ..rifle(&next_serial()) },
                    ),
                    1 => add_firearm(
                        &db,
                        &FirearmInput { mounted_on: host, ..suppressor(&next_serial()) },
                    ),
                    _ => {
                        let mut input = accessory(OPTIC, "Make", "Model");
                        input.mounted_on = host;
                        add_accessory(&db, &input)
                    }
                };
                records.push(created);
                if host.is_some() {
                    seen.created_mounted += 1;
                    assert_eq!(graph(&db).host_of(created), host, "step {step} ({what})");
                }
            }
        }
        assert_mounts_sound(&db, step, &what);
    }

    // Guard: the walk did exercise every operation, in the interesting ways.
    assert!(seen.mounted >= 100, "{seen:?}");
    assert!(seen.refused >= 50, "{seen:?}");
    assert!(seen.moved_with_records_below >= 20, "{seen:?}");
    assert!(seen.disposed_with_records >= 20, "{seen:?}");
    assert!(seen.disposed_keeping_records >= 10, "{seen:?}");
    assert!(seen.stale >= 10, "{seen:?}");
    assert!(seen.restored >= 20, "{seen:?}");
    assert!(seen.deleted_with_records_below >= 10, "{seen:?}");
    assert!(seen.created_mounted >= 10, "{seen:?}");
}

/// Whether every record still has the status it had.
fn statuses_after_eq(db: &TestDb, before: &std::collections::HashMap<RecordRef, String>) -> bool {
    &status_map(db) == before
}
