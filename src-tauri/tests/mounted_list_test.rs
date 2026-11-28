//! 014 US5: an edit form's Mounted list saved with the record, through `ops`,
//! against a real temporary SQLCipher database (no mocks): additions that move
//! a record with everything on it, removals, all or none when one change is
//! refused, a list stale against the database, a loop, the bound, a disposed
//! record, and the candidates judged against the form's own Mounted on
//! (014 FR-024 to FR-027, contracts/tauri-commands.md).

use crate::support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::mounts::ops;
use hoplodex_lib::models::accessory::AccessoryInput;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::mounts::{MAX_MOUNT_CHANGES, MountChange, STALE_MOUNTED_LIST};
use serde_json::json;
use support::{TestDb, firearm};

const OPTIC: i64 = 1;
const LIGHT: i64 = 2;

fn add_firearm(db: &TestDb, serial: &str) -> RecordRef {
    let input = firearm("Ruger", serial, serial);
    RecordRef::Firearm(firearm_ops::create_firearm(&db.conn, &input, false, None).unwrap().id)
}

fn add_accessory(db: &TestDb, kind: i64, model: &str) -> RecordRef {
    let input: AccessoryInput = serde_json::from_value(
        json!({ "accessoryKindId": kind, "make": "Aimpoint", "model": model, "status": "active" }),
    )
    .unwrap();
    RecordRef::Accessory(accessory_ops::create_accessory(&db.conn, &input, None).unwrap().id)
}

fn host_of(db: &TestDb, item: RecordRef) -> Option<RecordRef> {
    let column = match item {
        RecordRef::Firearm(_) => "item_firearm_id",
        RecordRef::Accessory(_) => "item_accessory_id",
    };
    db.conn
        .query_row(
            &format!("SELECT host_firearm_id, host_accessory_id FROM mounts WHERE {column} = ?1"),
            [item.id()],
            |row| {
                let firearm: Option<i64> = row.get(0)?;
                let accessory: Option<i64> = row.get(1)?;
                Ok(firearm.map(RecordRef::Firearm).or(accessory.map(RecordRef::Accessory)))
            },
        )
        .unwrap_or(None)
}

fn place(db: &TestDb, item: RecordRef, host: RecordRef) {
    let input = json!({ "item": item, "host": host });
    ops::mount_record(&db.conn, &serde_json::from_value(input).unwrap()).unwrap();
}

fn change(item: RecordRef, mounted: bool) -> MountChange {
    MountChange { item, mounted }
}

/// Saves the firearm's edit form as it is, with `notes` and the list's changes.
fn save_firearm(
    db: &TestDb,
    record: RecordRef,
    notes: &str,
    changes: &[MountChange],
) -> Result<(), CommandError> {
    let current = firearm_ops::get_firearm(&db.conn, record.id()).unwrap();
    let input = FirearmInput { notes: Some(notes.into()), ..FirearmInput::from(&current) };
    firearm_ops::update_firearm_with_mounted(&db.conn, record.id(), &input, false, changes)
        .map(|_| ())
}

fn notes(db: &TestDb, record: RecordRef) -> Option<String> {
    firearm_ops::get_firearm(&db.conn, record.id()).unwrap().notes
}

fn assert_mounted_error(err: CommandError, message: &str) {
    assert_eq!(err.code, "VALIDATION_ERROR", "{err:?}");
    assert_eq!(
        err.field_errors.as_ref().and_then(|e| e.get("mounted")).map(String::as_str),
        Some(message),
        "{err:?}"
    );
}

// 014 US5-7: the notes, an unmount and a move are saved together.
#[test]
fn saves_the_record_an_unmount_and_a_move_together() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, "R1");
    let pistol = add_firearm(&db, "P1");
    let scope = add_accessory(&db, OPTIC, "Scope");
    let red_dot = add_accessory(&db, OPTIC, "Red dot");
    let light = add_accessory(&db, LIGHT, "Light");
    place(&db, scope, rifle);
    place(&db, red_dot, pistol);
    place(&db, light, red_dot);

    save_firearm(&db, rifle, "Reworked", &[change(scope, false), change(red_dot, true)]).unwrap();

    assert_eq!(notes(&db, rifle).as_deref(), Some("Reworked"));
    assert_eq!(host_of(&db, scope), None);
    assert_eq!(host_of(&db, red_dot), Some(rifle));
    // What is on the red dot moves with it (006 FR-010).
    assert_eq!(host_of(&db, light), Some(red_dot));
}

// 014 FR-027: one refused change saves none of them, nor the record.
#[test]
fn saves_nothing_when_one_change_is_refused() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, "R1");
    let scope = add_accessory(&db, OPTIC, "Scope");
    let other = add_accessory(&db, OPTIC, "Other");
    place(&db, scope, rifle);

    // `other` is not on the rifle, so removing it is stale.
    let err = save_firearm(&db, rifle, "Changed", &[change(scope, false), change(other, false)])
        .unwrap_err();

    assert_mounted_error(err, STALE_MOUNTED_LIST);
    assert_eq!(host_of(&db, scope), Some(rifle));
    assert_eq!(notes(&db, rifle), None);
}

// 014 FR-026: a loop is refused on the Mounted list, and nothing is saved.
#[test]
fn refuses_a_loop() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, "R1");
    let upper = add_accessory(&db, OPTIC, "Upper");
    place(&db, rifle, upper);

    let err = save_firearm(&db, rifle, "Loop", &[change(upper, true)]).unwrap_err();

    assert_mounted_error(
        err,
        "An accessory can't be mounted on itself, or on something mounted on it.",
    );
    assert_eq!(host_of(&db, upper), None);
    assert_eq!(notes(&db, rifle), None);
}

// 014 contracts/tauri-commands.md: an item listed twice, or too many changes.
#[test]
fn refuses_an_item_listed_twice_and_more_than_the_bound() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, "R1");
    let scope = add_accessory(&db, OPTIC, "Scope");

    let err =
        save_firearm(&db, rifle, "x", &[change(scope, true), change(scope, false)]).unwrap_err();
    assert_mounted_error(err, STALE_MOUNTED_LIST);

    let many: Vec<MountChange> = (0..=MAX_MOUNT_CHANGES as i64)
        .map(|id| change(RecordRef::Accessory(id + 1), true))
        .collect();
    let err = save_firearm(&db, rifle, "x", &many).unwrap_err();
    assert_mounted_error(err, "Save fewer changes to what is mounted at once.");
    assert_eq!(host_of(&db, scope), None);
}

// 014 FR-024: a disposed record has no Mounted list; its changes are ignored.
#[test]
fn ignores_the_list_when_the_record_is_saved_as_disposed() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, "R1");
    let scope = add_accessory(&db, OPTIC, "Scope");
    let current = firearm_ops::get_firearm(&db.conn, rifle.id()).unwrap();
    let input = FirearmInput {
        status: FirearmStatus::Disposed,
        disposition_type: Some(serde_json::from_value(json!("sold")).unwrap()),
        disposition_recipient: Some("Jane Doe".into()),
        disposition_date: Some("2025-06-15".into()),
        disposition_price: Some(400),
        ..FirearmInput::from(&current)
    };

    firearm_ops::update_firearm_with_mounted(
        &db.conn,
        rifle.id(),
        &input,
        false,
        &[change(scope, true)],
    )
    .unwrap();

    assert_eq!(host_of(&db, scope), None);
}

// 014 US5 on an accessory: the same list on an accessory's form.
#[test]
fn an_accessory_saves_its_list_too() {
    let db = TestDb::new();
    let upper = add_accessory(&db, OPTIC, "Upper");
    let scope = add_accessory(&db, OPTIC, "Scope");
    let current =
        serde_json::to_value(accessory_ops::get_accessory(&db.conn, upper.id()).unwrap()).unwrap();
    let input: AccessoryInput = serde_json::from_value(current).unwrap();

    accessory_ops::update_accessory_with_mounted(
        &db.conn,
        upper.id(),
        &input,
        &[change(scope, true)],
    )
    .unwrap();

    assert_eq!(host_of(&db, scope), Some(upper));
}

// 014 US5-8, FR-026: the candidates are judged against the form's Mounted on.
#[test]
fn candidates_follow_the_forms_own_mounted_on() {
    let db = TestDb::new();
    let rifle = add_firearm(&db, "R1");
    let pistol = add_firearm(&db, "P1");
    let red_dot = add_accessory(&db, OPTIC, "Red dot");

    let list = |host_mounted_on: serde_json::Value| -> Vec<RecordRef> {
        let mut input = json!({ "role": "item", "record": rifle, "query": "" });
        if !host_mounted_on.is_null() {
            input["hostMountedOn"] = host_mounted_on;
        }
        let output =
            ops::list_mount_candidates(&db.conn, &serde_json::from_value(input).unwrap()).unwrap();
        serde_json::to_value(output).unwrap()["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| serde_json::from_value(c["label"]["record"].clone()).unwrap())
            .collect()
    };

    // As saved, nothing is above the rifle: the red dot and the pistol may go on it.
    assert!(list(serde_json::Value::Null).contains(&red_dot));
    // The form puts the rifle on the red dot: the red dot can't go on the rifle.
    let on_red_dot = list(json!({ "on": red_dot }));
    assert!(!on_red_dot.contains(&red_dot));
    assert!(on_red_dot.contains(&pistol));
    // Mounted on cleared in the form: nothing above it.
    assert!(list(json!({ "on": null })).contains(&red_dot));
}
