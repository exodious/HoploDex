//! specs/006-accessory-links User Story 1's plain Accessories list through
//! `ops` (tasks.md T017, FR-016), against a real temporary SQLCipher
//! database: one group "All" ordered by make, model, then kind, disposed
//! accessories only on request, and each summary's kind, thumbnail, value
//! and insurance flag; then User Story 4's grouping and search (T092,
//! FR-017, FR-018).
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
    ops::create_accessory(&db.conn, input, None).unwrap().id
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
            None,
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

// --- Grouping (specs/006-accessory-links US4, FR-017, "Group order") ----------------------

fn keys(output: &Value) -> Vec<String> {
    groups(output).iter().map(|g| g["key"].as_str().unwrap().to_owned()).collect()
}

fn group_ids(output: &Value) -> Vec<Vec<i64>> {
    groups(output)
        .iter()
        .map(|g| {
            g["accessories"].as_array().unwrap().iter().map(|s| s["id"].as_i64().unwrap()).collect()
        })
        .collect()
}

#[test]
fn kind_groups_follow_the_kind_lists_order_not_the_alphabet() {
    let db = TestDb::new();
    let sling = create(&db, &accessory(SLING, "Magpul", "MS1"));
    let magazine = create(&db, &accessory(MAGAZINE, "Walther", "P38"));
    let optic_b = create(&db, &accessory(OPTIC, "Zeiss", "Conquest"));
    let optic_a = create(&db, &accessory(OPTIC, "Aimpoint", "T-2"));

    let output = list(&db, json!({ "groupBy": "kind" }));

    // Optic, Magazine, Sling is the list's order; alphabetical would put
    // Magazine first.
    assert_eq!(keys(&output), ["Optic", "Magazine", "Sling"]);
    assert_eq!(group_ids(&output), [vec![optic_a, optic_b], vec![magazine], vec![sling]]);
    assert!(groups(&output).iter().all(|g| g["host"].is_null()));
}

#[test]
fn make_groups_are_alphabetical_with_unspecified_last() {
    let db = TestDb::new();
    let none = create(&db, &{
        let mut input = accessory(SLING, "x", "y");
        input.make = None;
        input
    });
    let walther = create(&db, &accessory(MAGAZINE, "Walther", "P38"));
    let magpul_b = create(&db, &accessory(SLING, "Magpul", "B"));
    let magpul_a = create(&db, &accessory(OPTIC, "Magpul", "A"));
    let aimpoint = create(&db, &accessory(OPTIC, "aimpoint", "T-2"));

    let output = list(&db, json!({ "groupBy": "make" }));

    assert_eq!(keys(&output), ["aimpoint", "Magpul", "Walther", "Unspecified"]);
    assert_eq!(
        group_ids(&output),
        [vec![aimpoint], vec![magpul_a, magpul_b], vec![walther], vec![none]],
        "within a group: make, model, then kind"
    );
}

#[test]
fn caliber_and_cartridge_groups_are_alphabetical_with_unspecified_last() {
    let db = TestDb::new();
    let with = |kind, make: &str, caliber: Option<&str>, cartridge: Option<&str>| {
        let mut input = accessory(kind, make, "m");
        input.caliber = caliber.map(Into::into);
        input.cartridge = cartridge.map(Into::into);
        create(&db, &input)
    };
    let plain = with(SLING, "Magpul", None, None);
    let b = with(MAGAZINE, "Walther", Some(".45"), Some("Beta cartridge"));
    let a = with(MAGAZINE, "Glock", Some("9mm"), Some("Alpha cartridge"));

    let by_caliber = list(&db, json!({ "groupBy": "caliber" }));
    assert_eq!(keys(&by_caliber), [".45", "9mm", "Unspecified"]);
    assert_eq!(group_ids(&by_caliber), [vec![b], vec![a], vec![plain]]);

    let by_cartridge = list(&db, json!({ "groupBy": "cartridge" }));
    assert_eq!(keys(&by_cartridge), ["Alpha cartridge", "Beta cartridge", "Unspecified"]);
    assert_eq!(group_ids(&by_cartridge), [vec![a], vec![b], vec![plain]]);
}

fn firearm_record(
    db: &TestDb,
    make: &str,
    model: &str,
    serial: &str,
    nickname: Option<&str>,
) -> RecordRef {
    RecordRef::Firearm(
        firearm_ops::create_firearm(
            &db.conn,
            &FirearmInput {
                nickname: nickname.map(Into::into),
                firearm_type_id: 2,
                ..firearm(make, model, serial)
            },
            false,
            None,
        )
        .unwrap()
        .id,
    )
}

#[test]
fn mounted_on_has_one_group_per_host_record_sorted_by_name_and_not_mounted_last() {
    let db = TestDb::new();
    let zeta = firearm_record(&db, "Zeta", "Rifle", "Z-1", None);
    let alpha = firearm_record(&db, "Alpha", "Carbine", "A-1", Some("Deer rifle"));
    let upper = create(&db, &accessory(UPPER, "BCM", "RECCE-16"));
    let loose = create(&db, &accessory(SLING, "Magpul", "MS1"));
    let on_zeta = create(&db, &accessory(OPTIC, "Vortex", "Razor"));
    let on_alpha_b = create(&db, &accessory(SLING, "Magpul", "B"));
    let on_alpha_a = create(&db, &accessory(OPTIC, "Magpul", "A"));
    let on_upper = create(&db, &accessory(OPTIC, "Aimpoint", "T-2"));
    mount(&db, RecordRef::Accessory(upper), alpha);
    mount(&db, RecordRef::Accessory(on_zeta), zeta);
    mount(&db, RecordRef::Accessory(on_alpha_b), alpha);
    mount(&db, RecordRef::Accessory(on_alpha_a), alpha);
    mount(&db, RecordRef::Accessory(on_upper), RecordRef::Accessory(upper));

    let output = list(&db, json!({ "groupBy": "mounted_on" }));

    // Names: "Alpha Carbine “Deer rifle”", "BCM RECCE-16 · Upper receiver",
    // "Zeta Rifle"; "Not mounted" last.
    assert_eq!(
        keys(&output),
        [
            "Alpha Carbine \u{201C}Deer rifle\u{201D}",
            "BCM RECCE-16 · Upper receiver",
            "Zeta Rifle",
            "Not mounted"
        ]
    );
    assert_eq!(
        group_ids(&output),
        [vec![upper, on_alpha_a, on_alpha_b], vec![on_upper], vec![on_zeta], vec![loose]],
        "within a group: make, model, then kind"
    );
    let gs = groups(&output);
    assert_eq!(gs[0]["host"]["record"], json!({ "kind": "firearm", "id": alpha.id() }));
    assert_eq!(gs[0]["host"]["nickname"], "Deer rifle");
    assert_eq!(gs[1]["host"]["record"], json!({ "kind": "accessory", "id": upper }));
    assert_eq!(gs[2]["host"]["record"], json!({ "kind": "firearm", "id": zeta.id() }));
    assert!(gs[3]["host"].is_null(), "the unmounted group has no host");
}

#[test]
fn two_hosts_with_the_same_name_are_two_groups() {
    let db = TestDb::new();
    let first = firearm_record(&db, "Colt", "Python", "P-1", None);
    let second = firearm_record(&db, "Colt", "Python", "P-2", None);
    let on_first = create(&db, &accessory(OPTIC, "Vortex", "Razor"));
    let on_second = create(&db, &accessory(SLING, "Magpul", "MS1"));
    mount(&db, RecordRef::Accessory(on_first), first);
    mount(&db, RecordRef::Accessory(on_second), second);

    let output = list(&db, json!({ "groupBy": "mounted_on" }));

    assert_eq!(keys(&output), ["Colt Python", "Colt Python"]);
    assert_eq!(group_ids(&output), [vec![on_first], vec![on_second]], "by name, then record");
    let gs = groups(&output);
    assert_eq!(gs[0]["host"]["record"]["id"], first.id());
    assert_eq!(gs[0]["host"]["serialNumber"], "P-1");
    assert_eq!(gs[1]["host"]["record"]["id"], second.id());
    assert_eq!(gs[1]["host"]["serialNumber"], "P-2");
}

#[test]
fn a_group_with_no_matching_accessory_is_not_returned() {
    let db = TestDb::new();
    create(&db, &accessory(OPTIC, "Leupold", "VX"));

    let output = list(&db, json!({ "groupBy": "mounted_on", "query": "nothing like this" }));

    assert!(groups(&output).is_empty());
}

#[test]
fn grouping_and_search_leave_out_disposed_accessories_until_asked() {
    let db = TestDb::new();
    let active = create(&db, &accessory(OPTIC, "Leupold", "VX"));
    let sold = create(&db, &accessory(OPTIC, "Leupold", "Mark 4"));
    dispose(&db, sold);

    let without = list(&db, json!({ "groupBy": "kind", "query": "Leupold" }));
    assert_eq!(group_ids(&without), [vec![active]]);

    let with = list(&db, json!({ "groupBy": "kind", "query": "Leupold", "includeDisposed": true }));
    assert_eq!(group_ids(&with), [vec![sold, active]], "Mark 4 before VX");
}

// --- Search (US4-4, FR-018) ----------------------------------------------------------------

/// An accessory with a different distinctive word in each searchable field.
fn searchable(db: &TestDb) -> i64 {
    let mut input = accessory(OPTIC, "Nightforce", "ATACR");
    input.serial_number = Some("SN-48151623".into());
    input.caliber = Some(".308".into());
    input.cartridge = Some("7.62x51mm NATO".into());
    input.acquisition_source = Some("Brownells warehouse".into());
    input.notes = Some("zeroed at one hundred yards".into());
    create(db, &input)
}

fn found(db: &TestDb, query: &str) -> Vec<i64> {
    let output = list(db, json!({ "query": query }));
    groups(&output)
        .iter()
        .flat_map(|g| g["accessories"].as_array().unwrap())
        .map(|s| s["id"].as_i64().unwrap())
        .collect()
}

#[test]
fn a_search_matches_inside_each_of_the_eight_text_fields() {
    let db = TestDb::new();
    let id = searchable(&db);
    let other = create(&db, &accessory(SLING, "Magpul", "MS1"));

    for query in [
        "Optic",     // the kind's name
        "ightfor",   // make, from the middle
        "ATAC",      // model
        "48151",     // serial number
        "308",       // caliber
        "x51mm",     // cartridge
        "rownell",   // acquisition source
        "hundred y", // notes, across a space
    ] {
        assert_eq!(found(&db, query), [id], "query {query:?}");
    }
    assert_eq!(found(&db, "Sling"), [other], "the kind's name finds the other");
    assert!(found(&db, "no such text").is_empty());
}

#[test]
fn one_and_two_character_queries_go_through_like_over_the_same_eight_values() {
    let db = TestDb::new();
    let id = searchable(&db);

    for query in [
        "Op", // kind name
        "Ni", // make
        "AT", // model
        "SN", // serial
        ".3", // caliber
        "7.", // cartridge
        "Br", // acquisition source
        "ze", // notes
        "z",  // one character
    ] {
        assert_eq!(found(&db, query), [id], "query {query:?}");
    }
    // LIKE wildcards are literal.
    assert!(found(&db, "%").is_empty());
    assert!(found(&db, "_").is_empty());
}

#[test]
fn a_search_is_case_insensitive_and_follows_edits() {
    let db = TestDb::new();
    let id = searchable(&db);
    assert_eq!(found(&db, "NIGHTFORCE"), [id]);

    let mut edited = accessory(OPTIC, "Vortex", "Razor");
    edited.notes = Some("rezeroed".into());
    ops::update_accessory(&db.conn, id, &edited).unwrap();

    assert!(found(&db, "Nightforce").is_empty());
    assert_eq!(found(&db, "Vortex"), [id]);
    assert_eq!(found(&db, "rezeroed"), [id]);
}

#[test]
fn a_blank_query_is_no_search() {
    let db = TestDb::new();
    let id = create(&db, &accessory(OPTIC, "Leupold", "VX"));

    assert_eq!(found(&db, ""), [id]);
    assert_eq!(found(&db, "   "), [id]);
}

#[test]
fn a_query_with_quotes_or_fts_syntax_is_matched_as_text() {
    let db = TestDb::new();
    let mut input = accessory(OPTIC, "Leupold", "VX \"Pro\" OR NOT");
    input.notes = Some("a * b".into());
    let id = create(&db, &input);

    assert_eq!(found(&db, "\"Pro\""), [id]);
    assert_eq!(found(&db, "OR NOT"), [id]);
    assert_eq!(found(&db, "a * b"), [id]);
}

#[test]
fn search_and_grouping_combine() {
    let db = TestDb::new();
    let optic = create(&db, &accessory(OPTIC, "Magpul", "Optic one"));
    let sling = create(&db, &accessory(SLING, "Magpul", "MS1"));
    create(&db, &accessory(OPTIC, "Zeiss", "Conquest"));

    let output = list(&db, json!({ "query": "Magpul", "groupBy": "kind" }));

    assert_eq!(keys(&output), ["Optic", "Sling"]);
    assert_eq!(group_ids(&output), [vec![optic], vec![sling]]);
}
