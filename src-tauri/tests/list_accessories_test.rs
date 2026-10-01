//! specs/006-accessory-links User Story 1's plain Accessories list through
//! `ops` (tasks.md T017, FR-016), against a real temporary SQLCipher
//! database: one group "All" ordered by make, model, then kind, disposed
//! accessories only on request, and each summary's kind, thumbnail, value
//! and insurance flag. Search and grouping are User Story 4's
//! (`accessory_search_test.rs`).
//!
//! Input and output are the contract's JSON (`ListAccessoriesInput`,
//! `{ groups: AccessoryGroup[] }`), in its own camelCase names.

mod support;

use hoplodex_lib::commands::accessories::ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::insurance::ops as insurance;
use hoplodex_lib::commands::mounts::ops as mount_ops;
use hoplodex_lib::models::accessory::AccessoryInput;
use hoplodex_lib::models::firearm::FirearmInput;
use hoplodex_lib::models::record::RecordRef;
use serde_json::{Value, json};
use support::{TestDb, firearm, policy};

const OPTIC: i64 = 1;
const MAGAZINE: i64 = 3;
const SLING: i64 = 10;
const UPPER: i64 = 5;

fn accessory(kind: i64, make: &str, model: &str) -> AccessoryInput {
    let mut input: AccessoryInput =
        serde_json::from_value(json!({ "accessoryKindId": kind, "status": "active" })).unwrap();
    input.make = Some(make.into());
    input.model = Some(model.into());
    input
}

fn create(db: &TestDb, input: &AccessoryInput) -> i64 {
    ops::create_accessory(&db.conn, input).unwrap().id
}

/// `list_accessories` for the contract's `ListAccessoriesInput`, as the
/// frontend receives it.
fn list(db: &TestDb, input: Value) -> Value {
    let output = ops::list_accessories(&db.conn, &serde_json::from_value(input).unwrap()).unwrap();
    serde_json::to_value(output).unwrap()
}

fn groups(output: &Value) -> &Vec<Value> {
    output["groups"].as_array().unwrap()
}

/// The summaries of the one group "All", in order.
fn all(output: &Value) -> Vec<&Value> {
    assert_eq!(groups(output).len(), 1, "one group: {output}");
    assert_eq!(groups(output)[0]["key"], "All");
    groups(output)[0]["accessories"].as_array().unwrap().iter().collect()
}

fn ids(summaries: &[&Value]) -> Vec<i64> {
    summaries.iter().map(|s| s["id"].as_i64().unwrap()).collect()
}

fn dispose(db: &TestDb, id: i64) {
    ops::dispose_accessory(
        &db.conn,
        id,
        &serde_json::from_value(json!({
            "dispositionType": "sold",
            "recipient": "Jane Doe",
            "date": "2025-06-15",
            "price": 100,
        }))
        .unwrap(),
    )
    .unwrap();
}

// --- Order and groups (FR-016) -----------------------------------------------------------

#[test]
fn with_no_grouping_there_is_one_group_all_ordered_by_make_model_then_kind() {
    let db = TestDb::new();
    let z_optic = create(&db, &accessory(OPTIC, "Zeiss", "Conquest"));
    let a_sling_b = create(&db, &accessory(SLING, "Armalite", "B-model"));
    let a_sling_a = create(&db, &accessory(SLING, "Armalite", "A-model"));
    // Same make and model: the kind decides (Optic before Sling).
    let m_sling = create(&db, &accessory(SLING, "Magpul", "Same"));
    let m_optic = create(&db, &accessory(OPTIC, "Magpul", "Same"));
    let m_magazine = create(&db, &accessory(MAGAZINE, "Magpul", "Same"));

    let output = list(&db, json!({}));

    assert_eq!(
        ids(&all(&output)),
        [a_sling_a, a_sling_b, m_optic, m_magazine, m_sling, z_optic],
        "make, then model, then kind"
    );
}

#[test]
fn the_input_may_name_every_option_as_null_or_false() {
    let db = TestDb::new();
    let id = create(&db, &accessory(OPTIC, "Leupold", "VX"));

    let output = list(&db, json!({ "query": null, "groupBy": null, "includeDisposed": false }));

    assert_eq!(ids(&all(&output)), [id]);
}

#[test]
fn disposed_accessories_are_listed_only_when_asked_for() {
    let db = TestDb::new();
    let active = create(&db, &accessory(OPTIC, "Leupold", "VX"));
    let sold = create(&db, &accessory(SLING, "Magpul", "MS1"));
    dispose(&db, sold);

    assert_eq!(ids(&all(&list(&db, json!({})))), [active]);
    assert_eq!(ids(&all(&list(&db, json!({ "includeDisposed": false })))), [active]);

    let output = list(&db, json!({ "includeDisposed": true }));
    let summaries = all(&output);
    assert_eq!(ids(&summaries), [active, sold]);
    assert_eq!(summaries[0]["status"], "active");
    assert_eq!(summaries[1]["status"], "disposed");
}

// --- What a summary carries ---------------------------------------------------------------

#[test]
fn a_summary_carries_the_kind_its_generic_picture_and_the_basics() {
    let db = TestDb::new();
    let mut input = accessory(OPTIC, "Leupold", "VX-5HD");
    input.serial_number = Some("LEU-1".into());
    input.caliber = Some("5.56".into());
    input.cartridge = Some("5.56x45mm NATO".into());
    input.estimated_value = Some(1000);
    let id = create(&db, &input);
    let sling = create(&db, &accessory(SLING, "Magpul", "MS1"));

    let output = list(&db, json!({}));
    let summaries = all(&output);

    let optic = summaries.iter().find(|s| s["id"] == id).unwrap();
    assert_eq!(optic["accessoryKindId"], OPTIC);
    assert_eq!(optic["kindName"], "Optic");
    assert_eq!(optic["genericThumbnailKey"], "optic");
    assert_eq!(optic["make"], "Leupold");
    assert_eq!(optic["model"], "VX-5HD");
    assert_eq!(optic["serialNumber"], "LEU-1");
    assert_eq!(optic["caliber"], "5.56");
    assert_eq!(optic["cartridge"], "5.56x45mm NATO");
    assert_eq!(optic["status"], "active");
    assert_eq!(optic["estimatedValue"], 1000);
    assert_eq!(optic["thumbnailPhotoId"], Value::Null, "no photo yet");
    assert_eq!(optic["insurancePolicyId"], Value::Null);
    assert_eq!(optic["scheduledCoverageAmount"], Value::Null);

    let sling = summaries.iter().find(|s| s["id"] == sling).unwrap();
    assert_eq!(sling["kindName"], "Sling");
    assert_eq!(sling["genericThumbnailKey"], "sling");
    assert_eq!(sling["estimatedValue"], Value::Null);
    assert_eq!(sling["serialNumber"], Value::Null);
}

#[test]
fn a_summary_carries_the_thumbnail_photo_id() {
    let db = TestDb::new();
    let id = create(&db, &accessory(OPTIC, "Leupold", "VX"));
    db.conn
        .execute(
            "INSERT INTO photos (accessory_id, original_bytes, original_filename, mime_type,
                                 thumbnail_bytes, sort_order, created_at)
             VALUES (?1, x'00', 'a.jpg', 'image/jpeg', x'00', 0, datetime('now'))",
            [id],
        )
        .unwrap();
    let photo_id = db.conn.last_insert_rowid();
    db.conn
        .execute(
            "UPDATE accessories SET thumbnail_photo_id = ?1 WHERE id = ?2",
            rusqlite::params![photo_id, id],
        )
        .unwrap();

    let output = list(&db, json!({}));

    assert_eq!(all(&output)[0]["thumbnailPhotoId"], photo_id);
}

// --- The insurance flag (FR-016, 001 FR-017 as for a firearm) ---------------------------------

fn warning(output: &Value, id: i64) -> String {
    let summaries = all(output);
    summaries.iter().find(|s| s["id"] == id).unwrap()["insuranceWarning"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn an_accessory_is_uninsured_when_no_policy_is_in_force() {
    let db = TestDb::new();
    let valued = create(&db, &{
        let mut input = accessory(OPTIC, "Leupold", "VX");
        input.estimated_value = Some(1000);
        input
    });
    // No value set is not itself a warning condition.
    let unvalued = create(&db, &accessory(SLING, "Magpul", "MS1"));

    let output = list(&db, json!({}));

    assert_eq!(warning(&output, valued), "uninsured");
    assert_eq!(warning(&output, unvalued), "none");
}

#[test]
fn a_blanket_policy_in_force_covers_an_unscheduled_accessory() {
    let db = TestDb::new();
    let id = create(&db, &{
        let mut input = accessory(OPTIC, "Leupold", "VX");
        input.estimated_value = Some(1000);
        input
    });
    insurance::create_policy(
        &db.conn,
        &policy("Blanket", "2020-01-01", "2099-01-01", Some(50_000)),
    )
    .unwrap();

    let output = list(&db, json!({}));

    assert_eq!(warning(&output, id), "none");
    assert_eq!(all(&output)[0]["insurancePolicyId"], Value::Null, "nothing stored per record");
}

#[test]
fn a_scheduled_amount_below_the_value_is_under_insured_and_a_sufficient_one_is_not() {
    let db = TestDb::new();
    let under = create(&db, &{
        let mut input = accessory(OPTIC, "Leupold", "VX");
        input.estimated_value = Some(1000);
        input
    });
    let enough = create(&db, &{
        let mut input = accessory(MAGAZINE, "Walther", "P38");
        input.estimated_value = Some(180);
        input
    });
    let rider =
        insurance::create_policy(&db.conn, &policy("Rider", "2020-01-01", "2099-01-01", None))
            .unwrap();
    insurance::assign_accessory_coverage(&db.conn, under, Some(rider.id), Some(500)).unwrap();
    insurance::assign_accessory_coverage(&db.conn, enough, Some(rider.id), Some(180)).unwrap();

    let output = list(&db, json!({}));

    assert_eq!(warning(&output, under), "under_insured");
    assert_eq!(warning(&output, enough), "none");
    let summaries = all(&output);
    let under_summary = summaries.iter().find(|s| s["id"] == under).unwrap();
    assert_eq!(under_summary["insurancePolicyId"], rider.id);
    assert_eq!(under_summary["scheduledCoverageAmount"], 500);
    assert_eq!(under_summary["estimatedValue"], 1000);
}

#[test]
fn unscheduling_returns_an_accessory_to_the_blanket_rules() {
    let db = TestDb::new();
    let id = create(&db, &{
        let mut input = accessory(OPTIC, "Leupold", "VX");
        input.estimated_value = Some(1000);
        input
    });
    let rider =
        insurance::create_policy(&db.conn, &policy("Rider", "2020-01-01", "2099-01-01", None))
            .unwrap();
    insurance::assign_accessory_coverage(&db.conn, id, Some(rider.id), Some(1000)).unwrap();
    assert_eq!(warning(&list(&db, json!({})), id), "none");

    insurance::assign_accessory_coverage(&db.conn, id, None, None).unwrap();

    let output = list(&db, json!({}));
    assert_eq!(warning(&output, id), "uninsured", "no blanket policy is in force");
    assert_eq!(all(&output)[0]["insurancePolicyId"], Value::Null);
    assert_eq!(all(&output)[0]["scheduledCoverageAmount"], Value::Null);
}

// --- Mounted on (specs/006-accessory-links US2, FR-013) -----------------------------------

fn mount(db: &TestDb, item: RecordRef, host: RecordRef) {
    let input = serde_json::from_value(json!({ "item": item, "host": host })).unwrap();
    mount_ops::mount_record(&db.conn, &input).unwrap();
}

#[test]
fn an_accessory_names_its_direct_host_only_and_an_unmounted_one_has_none() {
    let db = TestDb::new();
    let receiver = RecordRef::Firearm(
        firearm_ops::create_firearm(
            &db.conn,
            &FirearmInput {
                nickname: Some("Deer rifle".into()),
                firearm_type_id: 2,
                ..firearm("LaRue", "PredatAR", "L-1")
            },
            false,
        )
        .unwrap()
        .id,
    );
    let upper = create(&db, &accessory(UPPER, "BCM", "RECCE-16"));
    let optic = create(&db, &accessory(OPTIC, "Vortex", "Razor"));
    let loose = create(&db, &accessory(SLING, "Magpul", "MS1"));
    mount(&db, RecordRef::Accessory(upper), receiver);
    mount(&db, RecordRef::Accessory(optic), RecordRef::Accessory(upper));

    let output = list(&db, json!({}));
    let summaries = all(&output);
    let find = |id: i64| *summaries.iter().find(|s| s["id"] == id).unwrap();

    let on_upper = &find(optic)["mountedOn"];
    assert_eq!(on_upper["record"], json!({ "kind": "accessory", "id": upper }));
    assert_eq!(on_upper["make"], "BCM");
    assert_eq!(on_upper["model"], "RECCE-16");
    assert_eq!(on_upper["typeName"], "Upper receiver");
    assert_eq!(on_upper["status"], "active");
    let on_receiver = &find(upper)["mountedOn"];
    assert_eq!(on_receiver["record"], json!({ "kind": "firearm", "id": receiver.id() }));
    assert_eq!(on_receiver["nickname"], "Deer rifle");
    assert_eq!(find(loose)["mountedOn"], Value::Null);
}
