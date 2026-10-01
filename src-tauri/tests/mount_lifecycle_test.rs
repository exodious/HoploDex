//! specs/006-accessory-links User Story 3 through `ops`, against a real
//! temporary SQLCipher database (no mocks): disposing a host with some of
//! what is mounted on it (FR-014, research.md §9), restoring, and deleting
//! hosts and items (FR-015).
//!
//! Written before the implementation (tasks.md T079). `DisposeInput` is
//! built as the contract's own camelCase JSON, so the tests need nothing
//! from the struct but its `withMounted` field.

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
const UPPER: i64 = 5;

const RIFLE: i64 = 2;
const SUPPRESSOR: i64 = 5;

const STALE: &str = "What is mounted has changed. Close the dialog and try again.";

// --- Building records and driving the commands ----------------------------------------------

fn add_rifle(db: &TestDb, serial: &str) -> RecordRef {
    let input = FirearmInput {
        firearm_type_id: RIFLE,
        caliber: ".308 Winchester".into(),
        ..firearm("Ruger", "Precision", serial)
    };
    RecordRef::Firearm(firearm_ops::create_firearm(&db.conn, &input, false).unwrap().id)
}

/// A firearm that can be mounted on a rifle (a suppressor, or the launcher
/// of a rifle-and-launcher combination).
fn add_launcher(db: &TestDb, serial: &str) -> RecordRef {
    let input = FirearmInput {
        firearm_type_id: SUPPRESSOR,
        caliber: ".22 WMR".into(),
        ..firearm("Mossberg", "Launcher", serial)
    };
    RecordRef::Firearm(firearm_ops::create_firearm(&db.conn, &input, false).unwrap().id)
}

fn add_accessory(db: &TestDb, kind: i64, make: &str, model: &str) -> RecordRef {
    let mut input: AccessoryInput =
        serde_json::from_value(json!({ "accessoryKindId": kind, "status": "active" })).unwrap();
    input.make = Some(make.into());
    input.model = Some(model.into());
    RecordRef::Accessory(accessory_ops::create_accessory(&db.conn, &input).unwrap().id)
}

fn mount(db: &TestDb, item: RecordRef, host: RecordRef) {
    let input = json!({ "item": item, "host": host });
    ops::mount_record(&db.conn, &serde_json::from_value(input).unwrap()).unwrap();
}

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

fn record_of(label: &Value) -> RecordRef {
    serde_json::from_value(label["record"].clone()).unwrap()
}

fn status(db: &TestDb, record: RecordRef) -> String {
    detail(db, record)["status"].as_str().unwrap().to_owned()
}

fn is_active(db: &TestDb, record: RecordRef) -> bool {
    status(db, record) == "active"
}

fn host_of(db: &TestDb, record: RecordRef) -> Option<RecordRef> {
    serde_json::from_value(detail(db, record)["mountedOn"].clone()).unwrap()
}

fn exists(db: &TestDb, record: RecordRef) -> bool {
    match record {
        RecordRef::Firearm(id) => firearm_ops::get_firearm(&db.conn, id).is_ok(),
        RecordRef::Accessory(id) => accessory_ops::get_accessory(&db.conn, id).is_ok(),
    }
}

/// How many `mounts` rows involve the record, as item or as host.
fn rows_involving(db: &TestDb, record: RecordRef) -> i64 {
    let (item, host) = match record {
        RecordRef::Firearm(_) => ("item_firearm_id", "host_firearm_id"),
        RecordRef::Accessory(_) => ("item_accessory_id", "host_accessory_id"),
    };
    db.conn
        .query_row(
            &format!("SELECT count(*) FROM mounts WHERE {item} = ?1 OR {host} = ?1"),
            [record.id()],
            |r| r.get(0),
        )
        .unwrap()
}

fn mount_rows(db: &TestDb) -> i64 {
    db.conn.query_row("SELECT count(*) FROM mounts", [], |r| r.get(0)).unwrap()
}

/// The contract's `DisposeInput` for the host: sold to Jane Doe on
/// 2025-06-15 for `price`, with the listed records disposed of too.
fn dispose_json(price: i64, with: &[(RecordRef, Option<i64>)]) -> Value {
    let with: Vec<Value> =
        with.iter().map(|(record, price)| json!({ "record": record, "price": price })).collect();
    json!({
        "dispositionType": "sold",
        "recipient": "Jane Doe",
        "date": "2025-06-15",
        "price": price,
        "withMounted": with,
    })
}

fn try_dispose(
    db: &TestDb,
    record: RecordRef,
    price: i64,
    with: &[(RecordRef, Option<i64>)],
) -> Result<(), CommandError> {
    let input = serde_json::from_value(dispose_json(price, with)).unwrap();
    match record {
        RecordRef::Firearm(id) => firearm_ops::dispose_firearm(&db.conn, id, &input).map(|_| ()),
        RecordRef::Accessory(id) => {
            accessory_ops::dispose_accessory(&db.conn, id, &input).map(|_| ())
        }
    }
}

fn dispose(db: &TestDb, record: RecordRef, price: i64, with: &[(RecordRef, Option<i64>)]) {
    try_dispose(db, record, price, with).unwrap();
}

fn restore(db: &TestDb, record: RecordRef) {
    match record {
        RecordRef::Firearm(id) => {
            let input = serde_json::from_value(json!({ "history": "discard" })).unwrap();
            firearm_ops::reverse_disposition(&db.conn, id, &input).unwrap();
        }
        RecordRef::Accessory(id) => {
            let input = serde_json::from_value(json!({ "history": "discard" })).unwrap();
            accessory_ops::reverse_accessory_disposition(&db.conn, id, &input).unwrap();
        }
    }
}

fn delete(db: &TestDb, record: RecordRef) {
    match record {
        RecordRef::Firearm(id) => {
            firearm_ops::delete_firearm(&db.conn, id, true).unwrap();
        }
        RecordRef::Accessory(id) => {
            accessory_ops::delete_accessory(&db.conn, id, true).unwrap();
        }
    }
}

/// The disposition fields of a record, as the frontend receives them.
fn disposition(db: &TestDb, record: RecordRef) -> (Value, Value, Value, Value) {
    let shown = detail(db, record);
    (
        shown["dispositionType"].clone(),
        shown["dispositionRecipient"].clone(),
        shown["dispositionDate"].clone(),
        shown["dispositionPrice"].clone(),
    )
}

fn assert_disposed_as_sold(db: &TestDb, record: RecordRef, price: Value) {
    assert_eq!(status(db, record), "disposed");
    assert_eq!(
        disposition(db, record),
        (json!("sold"), json!("Jane Doe"), json!("2025-06-15"), price)
    );
    assert_eq!(rows_involving(db, record), 0, "a disposed record is in no mount");
}

fn assert_kept_unmounted(db: &TestDb, record: RecordRef) {
    assert!(is_active(db, record));
    assert_eq!(host_of(db, record), None);
    assert_eq!(rows_involving(db, record), 0);
}

// --- Disposing a host with some of what is on it (US3-1, US3-2) ------------------------------

#[test]
fn disposing_a_rifle_with_its_optic_disposes_both_and_leaves_the_suppressor_unmounted() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    let can = add_launcher(&db, "S-1");
    mount(&db, optic, rifle);
    mount(&db, can, rifle);

    dispose(&db, rifle, 1200, &[(optic, Some(300))]);

    assert_disposed_as_sold(&db, rifle, json!(1200));
    assert_disposed_as_sold(&db, optic, json!(300));
    assert_kept_unmounted(&db, can);
    assert_eq!(mount_rows(&db), 0);
}

#[test]
fn a_null_price_disposes_the_optic_with_no_price() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, optic, rifle);

    dispose(&db, rifle, 1200, &[(optic, None)]);

    assert_disposed_as_sold(&db, rifle, json!(1200));
    assert_disposed_as_sold(&db, optic, Value::Null);
}

#[test]
fn leaving_with_mounted_out_is_the_same_as_an_empty_list() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, optic, rifle);
    let mut input = dispose_json(1200, &[]);
    input.as_object_mut().unwrap().remove("withMounted");

    let id = rifle.id();
    firearm_ops::dispose_firearm(&db.conn, id, &serde_json::from_value(input).unwrap()).unwrap();

    assert_disposed_as_sold(&db, rifle, json!(1200));
    assert_kept_unmounted(&db, optic);
}

// --- Disposing an item, and restoring (US3-3, US3-4) -------------------------------------------

#[test]
fn disposing_a_mounted_optic_unmounts_it_and_restoring_either_record_restores_it_unmounted() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    let can = add_launcher(&db, "S-1");
    mount(&db, optic, rifle);
    mount(&db, can, rifle);

    dispose(&db, optic, 250, &[]);

    assert_disposed_as_sold(&db, optic, json!(250));
    assert_eq!(host_of(&db, can), Some(rifle), "the other item stays mounted");

    restore(&db, optic);
    assert_kept_unmounted(&db, optic);
    assert_eq!(host_of(&db, can), Some(rifle), "restoring one leaves the other as it was");

    // Now the host: dispose the rifle with the optic, restore each in turn.
    mount(&db, optic, rifle);
    dispose(&db, rifle, 1200, &[(optic, Some(300))]);
    restore(&db, rifle);
    assert_kept_unmounted(&db, rifle);
    assert_eq!(status(&db, optic), "disposed", "the optic is not restored with the rifle");
    restore(&db, optic);
    assert_kept_unmounted(&db, optic);
    assert_eq!(mount_rows(&db), 0);
}

// --- A firearm carrying a firearm carrying an accessory (US3-7) --------------------------------

#[test]
fn a_launcher_and_its_light_are_kept_or_disposed_as_chosen() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let launcher = add_launcher(&db, "L-1");
    let light = add_accessory(&db, LIGHT, "SureFire", "X300");
    mount(&db, launcher, rifle);
    mount(&db, light, launcher);

    // Keep both: the launcher is unmounted, the light stays on the launcher.
    dispose(&db, rifle, 1000, &[]);
    assert_eq!(status(&db, rifle), "disposed");
    assert_eq!(host_of(&db, launcher), None);
    assert_eq!(host_of(&db, light), Some(launcher));
    assert_eq!(mount_rows(&db), 1);
    restore(&db, rifle);

    // Dispose the launcher with the rifle and keep the light: it is left
    // active and unmounted.
    mount(&db, launcher, rifle);
    dispose(&db, rifle, 1000, &[(launcher, Some(150))]);
    assert_disposed_as_sold(&db, launcher, json!(150));
    assert_kept_unmounted(&db, light);
    assert_eq!(mount_rows(&db), 0);
}

#[test]
fn disposing_everything_below_a_rifle_disposes_every_listed_record() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let launcher = add_launcher(&db, "L-1");
    let light = add_accessory(&db, LIGHT, "SureFire", "X300");
    mount(&db, launcher, rifle);
    mount(&db, light, launcher);

    dispose(&db, rifle, 1000, &[(launcher, Some(150)), (light, None)]);

    assert_disposed_as_sold(&db, rifle, json!(1000));
    assert_disposed_as_sold(&db, launcher, json!(150));
    assert_disposed_as_sold(&db, light, Value::Null);
    assert_eq!(mount_rows(&db), 0);
}

// --- An upper carrying an optic (US3-8) ---------------------------------------------------------

#[test]
fn an_upper_and_its_optic_are_kept_or_disposed_as_chosen() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let upper = add_accessory(&db, UPPER, "BCM", "RECCE-16");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, upper, rifle);
    mount(&db, optic, upper);

    // Keep both: the upper is unmounted, the optic stays on it.
    dispose(&db, rifle, 1000, &[]);
    assert_eq!(host_of(&db, upper), None);
    assert_eq!(host_of(&db, optic), Some(upper));
    restore(&db, rifle);

    // Dispose the upper with the rifle, keep the optic.
    mount(&db, upper, rifle);
    dispose(&db, rifle, 1000, &[(upper, Some(80))]);
    assert_disposed_as_sold(&db, upper, json!(80));
    assert_kept_unmounted(&db, optic);
    restore(&db, rifle);
    restore(&db, upper);

    // Disposing the upper alone unmounts it from the rifle and lets its
    // optic be disposed with it.
    mount(&db, upper, rifle);
    mount(&db, optic, upper);
    dispose(&db, upper, 80, &[(optic, Some(300))]);
    assert_disposed_as_sold(&db, upper, json!(80));
    assert_disposed_as_sold(&db, optic, json!(300));
    assert_kept_unmounted(&db, rifle);
    assert_eq!(mount_rows(&db), 0);
}

#[test]
fn disposing_an_upper_alone_can_keep_its_optic_on_it() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let upper = add_accessory(&db, UPPER, "BCM", "RECCE-16");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, upper, rifle);
    mount(&db, optic, upper);

    dispose(&db, upper, 80, &[]);

    assert_disposed_as_sold(&db, upper, json!(80));
    assert_kept_unmounted(&db, optic);
    assert_kept_unmounted(&db, rifle);
}

#[test]
fn dispose_accessory_disposes_a_listed_firearm_mounted_on_it() {
    let db = TestDb::new();
    let case = add_accessory(&db, 11, "Pelican", "1750");
    let rifle = add_rifle(&db, "R-1");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, rifle, case);
    mount(&db, optic, rifle);

    dispose(&db, case, 60, &[(rifle, Some(900))]);

    assert_disposed_as_sold(&db, case, json!(60));
    assert_disposed_as_sold(&db, rifle, json!(900));
    assert_kept_unmounted(&db, optic);
}

// --- A stale list (the contract's withMounted error) ----------------------------------------------

fn assert_stale(err: CommandError) {
    assert_eq!(err.code, "VALIDATION_ERROR", "{err:?}");
    let errors = err.field_errors.as_ref().expect("fieldErrors");
    assert_eq!(errors.get("withMounted").map(String::as_str), Some(STALE), "{err:?}");
}

#[test]
fn a_listed_record_that_is_not_below_the_host_fails_and_changes_nothing() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let other = add_rifle(&db, "R-2");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    let light = add_accessory(&db, LIGHT, "SureFire", "X300");
    let loose = add_accessory(&db, LIGHT, "Streamlight", "TLR-1");
    mount(&db, optic, rifle);
    mount(&db, light, other);
    let rows = mount_rows(&db);

    // Mounted on another host, not mounted at all, the host itself, and a
    // record that is gone: each fails, even beside a record that is valid.
    let gone = RecordRef::Accessory(9999);
    for stale in [light, loose, rifle, gone] {
        let err = try_dispose(&db, rifle, 1200, &[(optic, Some(300)), (stale, None)])
            .expect_err("stale list");
        assert_stale(err);
        assert_eq!(status(&db, rifle), "active");
        assert_eq!(status(&db, optic), "active");
        assert_eq!(host_of(&db, optic), Some(rifle));
        assert_eq!(host_of(&db, light), Some(other));
        assert_eq!(mount_rows(&db), rows);
    }
}

#[test]
fn a_record_moved_off_the_host_since_the_dialog_opened_is_stale() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let other = add_rifle(&db, "R-2");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, optic, rifle);
    mount(&db, optic, other); // moved by another computer

    let err = try_dispose(&db, rifle, 1200, &[(optic, Some(300))]).expect_err("stale list");

    assert_stale(err);
    assert_eq!(host_of(&db, optic), Some(other));
    assert_eq!(status(&db, rifle), "active");
}

#[test]
fn a_failing_disposal_leaves_the_listed_records_as_they_were() {
    // The host's own save can fail (a negative price): nothing is disposed.
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, optic, rifle);

    let err = try_dispose(&db, rifle, -1, &[(optic, Some(300))]).expect_err("negative price");

    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(status(&db, optic), "active");
    assert_eq!(host_of(&db, optic), Some(rifle));
    assert_eq!(status(&db, rifle), "active");
}

// --- Deleting (US3-5, US3-6, US3-9, FR-015) --------------------------------------------------------

#[test]
fn deleting_a_rifle_leaves_its_records_in_the_collection_and_those_further_down_where_they_are() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let upper = add_accessory(&db, UPPER, "BCM", "RECCE-16");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    let can = add_launcher(&db, "S-1");
    mount(&db, upper, rifle);
    mount(&db, can, rifle);
    mount(&db, optic, upper);

    delete(&db, rifle);

    assert!(!exists(&db, rifle));
    assert_kept_unmounted(&db, can);
    assert_eq!(host_of(&db, upper), None);
    assert!(is_active(&db, upper));
    assert_eq!(host_of(&db, optic), Some(upper), "mounted further down stays on its own host");
    assert_eq!(mount_rows(&db), 1);
}

#[test]
fn deleting_a_mounted_accessory_removes_it_and_its_mount() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    let light = add_accessory(&db, LIGHT, "SureFire", "X300");
    mount(&db, optic, rifle);
    mount(&db, light, rifle);

    delete(&db, optic);

    assert!(!exists(&db, optic));
    let mounted = detail(&db, rifle)["mount"]["mounted"].clone();
    let listed: Vec<RecordRef> =
        mounted.as_array().unwrap().iter().map(|e| record_of(&e["label"])).collect();
    assert_eq!(listed, [light], "the host no longer lists it");
    assert_eq!(mount_rows(&db), 1);
}

#[test]
fn deleting_an_upper_with_an_optic_leaves_the_optic_unmounted() {
    let db = TestDb::new();
    let rifle = add_rifle(&db, "R-1");
    let upper = add_accessory(&db, UPPER, "BCM", "RECCE-16");
    let optic = add_accessory(&db, OPTIC, "Vortex", "Razor");
    mount(&db, upper, rifle);
    mount(&db, optic, upper);

    delete(&db, upper);

    assert!(!exists(&db, upper));
    assert_kept_unmounted(&db, optic);
    assert_kept_unmounted(&db, rifle);
    assert_eq!(mount_rows(&db), 0);
}
