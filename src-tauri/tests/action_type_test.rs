//! specs/004-cartridges-action-types User Story 3 through `ops`, against a
//! real temporary SQLCipher database: the fixed action list and its mapping
//! to firearm types (FR-017, FR-018), the rule that an action must be
//! allowed for the firearm's type (FR-019, SC-008) in the command layer and
//! in the trigger backstop.

mod support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::entries::ops as entry_ops;
use hoplodex_lib::commands::firearms::ops;
use hoplodex_lib::models::firearm::FirearmInput;
use support::{TestDb, firearm};

const HANDGUN: i64 = 1;
const RIFLE: i64 = 2;
const SHOTGUN: i64 = 3;
const OTHER: i64 = 4;

const SEMI_AUTOMATIC: i64 = 1;
const LEVER_ACTION: i64 = 4;
const PUMP_ACTION: i64 = 5;
const FALLING_BLOCK: i64 = 7;
const ROLLING_BLOCK: i64 = 8;
const INLINE_MUZZLELOADER: i64 = 12;
const AUTOMATIC: i64 = 13;
const SUPPRESSOR: i64 = 5;
const MACHINE_GUN: i64 = 5;

fn with_action(firearm_type_id: i64, action_type_id: Option<i64>, serial: &str) -> FirearmInput {
    FirearmInput { firearm_type_id, action_type_id, ..firearm("Maker", "Model", serial) }
}

fn field_error(err: &CommandError, field: &str) -> Option<String> {
    err.field_errors.as_ref().and_then(|errors| errors.get(field).cloned())
}

#[test]
fn the_action_list_is_in_fr_018_order() {
    // US3-1.
    let db = TestDb::new();
    let output = entry_ops::list_action_types(&db.conn).unwrap();
    let names: Vec<&str> = output.actions.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Semi-automatic",
            "Revolver",
            "Bolt action",
            "Lever action",
            "Pump action",
            "Break action",
            "Automatic or select-fire",
            "Falling block",
            "Rolling block",
            "Single shot (other)",
            "Flintlock",
            "Percussion",
            "Inline muzzleloader",
        ]
    );
    // 005 US3: id 13 sits seventh; ids 7-12 keep their ids, only their order moves.
    let action_ids: Vec<i64> = output.actions.iter().map(|a| a.id).collect();
    assert_eq!(action_ids, vec![1, 2, 3, 4, 5, 6, 13, 7, 8, 9, 10, 11, 12]);
}

#[test]
fn the_seeded_types_map_to_their_allowed_actions_in_list_order() {
    // US3-1, US3-2, FR-018.
    let db = TestDb::new();
    let output = entry_ops::list_action_types(&db.conn).unwrap();
    let allowed = &output.allowed_by_firearm_type;

    let list_order: Vec<i64> = vec![1, 2, 3, 4, 5, 6, 13, 7, 8, 9, 10, 11, 12];
    let mut handgun = list_order.clone();
    handgun.retain(|id| ![PUMP_ACTION, FALLING_BLOCK, INLINE_MUZZLELOADER].contains(id));
    assert_eq!(handgun.len(), 10);
    assert_eq!(allowed[&HANDGUN], handgun);

    assert_eq!(allowed[&RIFLE], list_order);

    let mut shotgun = list_order.clone();
    shotgun.retain(|id| *id != ROLLING_BLOCK);
    assert_eq!(shotgun.len(), 12);
    assert_eq!(allowed[&SHOTGUN], shotgun);

    assert!(
        allowed.get(&OTHER).is_none_or(|list| list.is_empty()),
        "Other maps nothing, which allows every action"
    );
    assert!(!allowed.contains_key(&SUPPRESSOR), "a Suppressor has no action list");
}

#[test]
fn a_type_added_later_allows_every_action_and_a_renamed_type_keeps_its_mapping() {
    // Spec Edge Cases: no command adds types yet, so raw SQL stands in.
    let db = TestDb::new();
    db.conn
        .execute(
            "INSERT INTO firearm_types (name, generic_thumbnail_key) VALUES ('Crossbow', 'other')",
            [],
        )
        .unwrap();
    let crossbow = db.conn.last_insert_rowid();
    let created =
        ops::create_firearm(&db.conn, &with_action(crossbow, Some(PUMP_ACTION), "X-1"), false)
            .unwrap();
    assert_eq!(created.action_type_id, Some(PUMP_ACTION));

    db.conn.execute("UPDATE firearm_types SET name = 'Pistol' WHERE id = 1", []).unwrap();
    let err = ops::create_firearm(&db.conn, &with_action(HANDGUN, Some(PUMP_ACTION), "X-2"), false)
        .unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(
        field_error(&err, "actionTypeId").as_deref(),
        Some("Pump action doesn't apply to a Pistol.")
    );
    let output = entry_ops::list_action_types(&db.conn).unwrap();
    assert_eq!(output.allowed_by_firearm_type[&HANDGUN].len(), 10);
}

#[test]
fn the_action_round_trips_and_none_saves_normally() {
    // US3-6, FR-021.
    let db = TestDb::new();
    let created =
        ops::create_firearm(&db.conn, &with_action(RIFLE, Some(LEVER_ACTION), "A-1"), false)
            .unwrap();
    assert_eq!(ops::get_firearm(&db.conn, created.id).unwrap().action_type_id, Some(LEVER_ACTION));

    let cleared = ops::update_firearm(
        &db.conn,
        created.id,
        &FirearmInput { action_type_id: None, ..FirearmInput::from(&created) },
        false,
    )
    .unwrap();
    assert_eq!(cleared.action_type_id, None);

    let plain = ops::create_firearm(&db.conn, &with_action(HANDGUN, None, "A-2"), false).unwrap();
    assert_eq!(ops::get_firearm(&db.conn, plain.id).unwrap().action_type_id, None);

    let changed = ops::update_firearm(
        &db.conn,
        plain.id,
        &FirearmInput { action_type_id: Some(SEMI_AUTOMATIC), ..FirearmInput::from(&plain) },
        false,
    )
    .unwrap();
    assert_eq!(changed.action_type_id, Some(SEMI_AUTOMATIC));
}

#[test]
fn mapped_oddities_save_and_disallowed_actions_are_field_errors() {
    // Spec clarification: Lever action on a Handgun and Falling block on a
    // Shotgun are allowed; Pump action on a Handgun is not.
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &with_action(HANDGUN, Some(LEVER_ACTION), "L-1"), false).unwrap();
    ops::create_firearm(&db.conn, &with_action(SHOTGUN, Some(FALLING_BLOCK), "F-1"), false)
        .unwrap();
    ops::create_firearm(&db.conn, &with_action(OTHER, Some(INLINE_MUZZLELOADER), "O-1"), false)
        .unwrap();

    let err = ops::create_firearm(&db.conn, &with_action(HANDGUN, Some(PUMP_ACTION), "P-1"), false)
        .unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(
        field_error(&err, "actionTypeId").as_deref(),
        Some("Pump action doesn't apply to a Handgun.")
    );

    let err =
        ops::create_firearm(&db.conn, &with_action(SHOTGUN, Some(ROLLING_BLOCK), "P-2"), false)
            .unwrap_err();
    assert_eq!(
        field_error(&err, "actionTypeId").as_deref(),
        Some("Rolling block doesn't apply to a Shotgun.")
    );
}

#[test]
fn an_unknown_action_id_is_choose_an_action_from_the_list() {
    let db = TestDb::new();
    for unknown in [0, 14, 9_999, -1] {
        let err = ops::create_firearm(&db.conn, &with_action(RIFLE, Some(unknown), "U-1"), false)
            .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(
            field_error(&err, "actionTypeId").as_deref(),
            Some("Choose an action from the list.")
        );
    }
    // A type that maps nothing still refuses an id that names no action.
    let err =
        ops::create_firearm(&db.conn, &with_action(OTHER, Some(99), "U-2"), false).unwrap_err();
    assert_eq!(
        field_error(&err, "actionTypeId").as_deref(),
        Some("Choose an action from the list.")
    );
}

#[test]
fn changing_the_type_to_one_that_disallows_the_action_is_rejected() {
    // US3-3 at the command layer: the form clears the action, and the
    // backend refuses a save that doesn't.
    let db = TestDb::new();
    let rifle = ops::create_firearm(&db.conn, &with_action(RIFLE, Some(PUMP_ACTION), "T-1"), false)
        .unwrap();

    let err = ops::update_firearm(
        &db.conn,
        rifle.id,
        &FirearmInput { firearm_type_id: HANDGUN, ..FirearmInput::from(&rifle) },
        false,
    )
    .unwrap_err();
    assert_eq!(
        field_error(&err, "actionTypeId").as_deref(),
        Some("Pump action doesn't apply to a Handgun.")
    );
    assert_eq!(
        ops::get_firearm(&db.conn, rifle.id).unwrap().firearm_type_id,
        RIFLE,
        "nothing changed"
    );

    let moved = ops::update_firearm(
        &db.conn,
        rifle.id,
        &FirearmInput {
            firearm_type_id: HANDGUN,
            action_type_id: None,
            ..FirearmInput::from(&rifle)
        },
        false,
    )
    .unwrap();
    assert_eq!(moved.firearm_type_id, HANDGUN);
    assert_eq!(moved.action_type_id, None);
}

#[test]
fn the_trigger_backstop_refuses_raw_writes_that_break_the_rule() {
    // SC-008: even a bug that bypasses the command layer cannot store a
    // disallowed action.
    let db = TestDb::new();
    let message = |result: rusqlite::Result<usize>| result.unwrap_err().to_string();
    const RULE: &str = "action type not allowed for this firearm type";

    let insert = message(db.conn.execute(
        "INSERT INTO firearms (make, model, serial_number, caliber, firearm_type_id, action_type_id, created_at, updated_at)
         VALUES ('M', 'X', 'S-1', '9mm', ?1, ?2, datetime('now'), datetime('now'))",
        [HANDGUN, PUMP_ACTION],
    ));
    assert!(insert.contains(RULE), "{insert}");

    db.conn
        .execute(
            "INSERT INTO firearms (make, model, serial_number, caliber, firearm_type_id, action_type_id, created_at, updated_at)
             VALUES ('M', 'X', 'S-2', '9mm', ?1, ?2, datetime('now'), datetime('now'))",
            [HANDGUN, LEVER_ACTION],
        )
        .unwrap();
    let id = db.conn.last_insert_rowid();

    let update_action = message(
        db.conn.execute("UPDATE firearms SET action_type_id = ?1 WHERE id = ?2", [PUMP_ACTION, id]),
    );
    assert!(update_action.contains(RULE), "{update_action}");

    db.conn
        .execute(
            "UPDATE firearms SET firearm_type_id = ?1, action_type_id = ?2 WHERE id = ?3",
            [RIFLE, PUMP_ACTION, id],
        )
        .unwrap();
    let update_type = message(
        db.conn.execute("UPDATE firearms SET firearm_type_id = ?1 WHERE id = ?2", [HANDGUN, id]),
    );
    assert!(update_type.contains(RULE), "{update_type}");

    // A type that maps nothing accepts anything, and NULL is always fine.
    db.conn.execute("UPDATE firearms SET firearm_type_id = ?1 WHERE id = ?2", [OTHER, id]).unwrap();
    db.conn.execute("UPDATE firearms SET action_type_id = NULL WHERE id = ?1", [id]).unwrap();
}

#[test]
fn automatic_or_select_fire_saves_with_or_without_a_classification() {
    // 005 US3-1, US3-2: the action is unrelated to any classification.
    let db = TestDb::new();
    for (type_id, serial) in
        [(HANDGUN, "AU-1"), (RIFLE, "AU-2"), (SHOTGUN, "AU-3"), (OTHER, "AU-4")]
    {
        let plain =
            ops::create_firearm(&db.conn, &with_action(type_id, Some(AUTOMATIC), serial), false)
                .unwrap();
        assert_eq!(plain.action_type_id, Some(AUTOMATIC));
        assert_eq!(plain.registration_class_id, None);

        let registered = FirearmInput {
            registration_class_id: Some(MACHINE_GUN),
            ..with_action(type_id, Some(AUTOMATIC), &format!("{serial}-R"))
        };
        let saved = ops::create_firearm(&db.conn, &registered, false).unwrap();
        assert_eq!(saved.action_type_id, Some(AUTOMATIC));
        assert_eq!(saved.registration_class_id, Some(MACHINE_GUN));
    }
}

#[test]
fn a_machine_gun_rifle_may_have_a_semi_automatic_action() {
    // 005 US3-3, FR-008: nothing ties the classification to the action.
    let db = TestDb::new();
    let input = FirearmInput {
        registration_class_id: Some(MACHINE_GUN),
        ..with_action(RIFLE, Some(SEMI_AUTOMATIC), "MG-1")
    };
    let saved = ops::create_firearm(&db.conn, &input, false).unwrap();
    assert_eq!(saved.action_type_id, Some(SEMI_AUTOMATIC));
}

#[test]
fn a_suppressor_refuses_the_automatic_action() {
    let db = TestDb::new();
    let err =
        ops::create_firearm(&db.conn, &with_action(SUPPRESSOR, Some(AUTOMATIC), "S-1"), false)
            .unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(field_error(&err, "actionTypeId").is_some());
}
