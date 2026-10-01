//! specs/005-regulated-item-types User Story 1 through `ops`, against a real
//! temporary SQLCipher database: Suppressor is a fifth firearm type that
//! omits the action, barrel length and capacity (FR-001 to FR-004), in the
//! command layer and in the trigger backstop (FR-003, SC-005). Its caliber is
//! its bore and its cartridge its rating, never derived (FR-002).

mod support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::entries::{SettleEntryInput, ops as entry_ops};
use hoplodex_lib::commands::firearms::ops;
use hoplodex_lib::models::firearm::FirearmInput;
use hoplodex_lib::services::entry_text::EntryField;
use support::{TestDb, firearm};

const RIFLE: i64 = 2;
const SUPPRESSOR: i64 = 5;
const BOLT_ACTION: i64 = 3;

fn suppressor(serial: &str) -> FirearmInput {
    FirearmInput {
        firearm_type_id: SUPPRESSOR,
        caliber: ".30".into(),
        ..firearm("SilencerCo", "Omega 300", serial)
    }
}

fn field_error(err: &CommandError, field: &str) -> Option<String> {
    err.field_errors.as_ref().and_then(|errors| errors.get(field).cloned())
}

#[test]
fn the_type_list_has_five_types_and_suppressor_omits_three_fields() {
    // US1-1, FR-001. Listed in `sort_order`, which puts Other last.
    let db = TestDb::new();
    let output = entry_ops::list_firearm_types(&db.conn).unwrap();
    let names: Vec<&str> = output.types.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Handgun", "Rifle", "Shotgun", "Suppressor", "Other"]);
    let ids: Vec<i64> = output.types.iter().map(|t| t.id).collect();
    assert_eq!(ids, [1, 2, 3, 5, 4]);
    for t in &output.types {
        let omits = t.id == SUPPRESSOR;
        assert_eq!(t.action_type_applies, !omits, "{}", t.name);
        assert_eq!(t.barrel_length_applies, !omits, "{}", t.name);
        assert_eq!(t.capacity_applies, !omits, "{}", t.name);
        // FR-002, research.md §15: only a Suppressor's caliber is never
        // worked out from its cartridge.
        assert_eq!(t.caliber_from_cartridge, !omits, "{}", t.name);
    }
    let suppressor = &output.types[3];
    assert_eq!(suppressor.generic_thumbnail_key, "suppressor");

    let json = serde_json::to_value(suppressor).unwrap();
    assert_eq!(json["genericThumbnailKey"], "suppressor");
    assert_eq!(json["actionTypeApplies"], false);
    assert_eq!(json["barrelLengthApplies"], false);
    assert_eq!(json["capacityApplies"], false);
    assert_eq!(json["caliberFromCartridge"], false);
}

#[test]
fn a_suppressor_saves_and_reopens_intact() {
    // US1-3.
    let db = TestDb::new();
    let input = FirearmInput {
        overall_length_hundredths: Some(800),
        weight_tenths_oz: Some(120),
        finish: Some("Cerakote black".into()),
        condition: Some(hoplodex_lib::models::firearm::Condition::Excellent),
        ..suppressor("ABC123")
    };
    let created = ops::create_firearm(&db.conn, &input, false).unwrap();
    let fetched = ops::get_firearm(&db.conn, created.id).unwrap();
    assert_eq!(fetched.firearm_type_id, SUPPRESSOR);
    assert_eq!(fetched.make, "SilencerCo");
    assert_eq!(fetched.model, "Omega 300");
    assert_eq!(fetched.serial_number.as_deref(), Some("ABC123"));
    assert_eq!(fetched.caliber, ".30");
    assert_eq!(fetched.overall_length_hundredths, Some(800));
    assert_eq!(fetched.weight_tenths_oz, Some(120));
    assert_eq!(fetched.finish.as_deref(), Some("Cerakote black"));
    assert!(fetched.condition.is_some());
    assert_eq!(fetched.action_type_id, None);
    assert_eq!(fetched.barrel_length_hundredths, None);
    assert_eq!(fetched.capacity, None);
}

#[test]
fn a_suppressor_with_an_inapplicable_field_is_refused_per_field() {
    // FR-003, FR-022.
    let db = TestDb::new();
    let action = FirearmInput { action_type_id: Some(1), ..suppressor("S-1") };
    let err = ops::create_firearm(&db.conn, &action, false).unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(
        field_error(&err, "actionTypeId").as_deref(),
        Some("Action doesn't apply to a Suppressor.")
    );
    assert_eq!(err.field_errors.as_ref().unwrap().len(), 1);

    let barrel = FirearmInput { barrel_length_hundredths: Some(500), ..suppressor("S-2") };
    let err = ops::create_firearm(&db.conn, &barrel, false).unwrap_err();
    assert_eq!(
        field_error(&err, "barrelLengthHundredths").as_deref(),
        Some("Barrel length doesn't apply to a Suppressor.")
    );
    assert_eq!(err.field_errors.as_ref().unwrap().len(), 1);

    let capacity = FirearmInput { capacity: Some(5), ..suppressor("S-3") };
    let err = ops::create_firearm(&db.conn, &capacity, false).unwrap_err();
    assert_eq!(
        field_error(&err, "capacity").as_deref(),
        Some("Capacity doesn't apply to a Suppressor.")
    );
    assert_eq!(err.field_errors.as_ref().unwrap().len(), 1);

    let all = FirearmInput {
        action_type_id: Some(1),
        barrel_length_hundredths: Some(500),
        capacity: Some(5),
        ..suppressor("S-4")
    };
    let err = ops::create_firearm(&db.conn, &all, false).unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(err.field_errors.as_ref().unwrap().len(), 3);

    // The fields message, never 004's action-allowed one: a Suppressor has no
    // mapped actions, which 004 reads as "every action".
    assert_eq!(
        field_error(&err, "actionTypeId").as_deref(),
        Some("Action doesn't apply to a Suppressor.")
    );
    let count: i64 = db.conn.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn changing_a_rifle_to_a_suppressor_needs_the_fields_cleared() {
    // US1-5, FR-004.
    let db = TestDb::new();
    let rifle = FirearmInput {
        firearm_type_id: RIFLE,
        action_type_id: Some(BOLT_ACTION),
        barrel_length_hundredths: Some(2000),
        capacity: Some(5),
        ..firearm("Ruger", "American", "R-1")
    };
    let created = ops::create_firearm(&db.conn, &rifle, false).unwrap();

    let still_set = FirearmInput { firearm_type_id: SUPPRESSOR, ..FirearmInput::from(&created) };
    let err = ops::update_firearm(&db.conn, created.id, &still_set, false).unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(err.field_errors.as_ref().unwrap().len(), 3);
    assert!(field_error(&err, "actionTypeId").is_some());
    assert!(field_error(&err, "barrelLengthHundredths").is_some());
    assert!(field_error(&err, "capacity").is_some());
    // Nothing was changed.
    assert_eq!(ops::get_firearm(&db.conn, created.id).unwrap().firearm_type_id, RIFLE);

    let cleared = FirearmInput {
        firearm_type_id: SUPPRESSOR,
        action_type_id: None,
        barrel_length_hundredths: None,
        capacity: None,
        ..FirearmInput::from(&created)
    };
    let saved = ops::update_firearm(&db.conn, created.id, &cleared, false).unwrap();
    assert_eq!(saved.firearm_type_id, SUPPRESSOR);

    let back = FirearmInput { firearm_type_id: RIFLE, ..FirearmInput::from(&saved) };
    let saved = ops::update_firearm(&db.conn, created.id, &back, false).unwrap();
    assert_eq!(saved.firearm_type_id, RIFLE);
    assert_eq!(saved.action_type_id, None);
    assert_eq!(saved.barrel_length_hundredths, None);
    assert_eq!(saved.capacity, None);
}

#[test]
fn a_suppressor_takes_a_cartridge_and_needs_none() {
    // US1-7, FR-002.
    let db = TestDb::new();
    let settled = entry_ops::settle_entry(
        &db.conn,
        &SettleEntryInput { field: EntryField::Cartridge, text: "9x19mm Parabellum".into() },
    )
    .unwrap();
    assert_eq!(settled.derived_caliber.unwrap().caliber, "9mm");

    let with = FirearmInput {
        cartridge: Some("9x19mm Parabellum".into()),
        caliber: "9mm".into(),
        ..suppressor("C-1")
    };
    assert!(ops::create_firearm(&db.conn, &with, false).is_ok());
    assert!(ops::create_firearm(&db.conn, &suppressor("C-2"), false).is_ok());
}

#[test]
fn a_suppressor_keeps_its_bore_and_rated_cartridge_apart() {
    // US1-1, US1-3, FR-002, research.md §15: the caliber is the bore and the
    // cartridge the rating. ".22 WMR" would derive ".22" for a rifle, but a
    // suppressor's two values are whatever the owner entered, and nothing on
    // save ties one to the other.
    let db = TestDb::new();
    let input = FirearmInput {
        caliber: ".22".into(),
        cartridge: Some(".22 WMR".into()),
        ..suppressor("BORE-1")
    };
    let created = ops::create_firearm(&db.conn, &input, false).unwrap();
    let fetched = ops::get_firearm(&db.conn, created.id).unwrap();
    assert_eq!(fetched.caliber, ".22");
    assert_eq!(fetched.cartridge.as_deref(), Some(".22 WMR"));

    // A bore that the rated cartridge would never derive is kept too.
    let wide = FirearmInput {
        caliber: ".46".into(),
        cartridge: Some(".300 Winchester Magnum".into()),
        ..suppressor("BORE-2")
    };
    let created = ops::create_firearm(&db.conn, &wide, false).unwrap();
    let fetched = ops::get_firearm(&db.conn, created.id).unwrap();
    assert_eq!(fetched.caliber, ".46");
    assert_eq!(fetched.cartridge.as_deref(), Some(".300 Winchester Magnum"));
}

#[test]
fn the_triggers_refuse_a_raw_write_the_command_layer_would_have_caught() {
    // FR-003, SC-005.
    let db = TestDb::new();
    let insert = |type_id: i64, column: &str, serial: &str| {
        db.conn.execute(
            &format!(
                "INSERT INTO firearms (uid, make, model, serial_number, caliber, firearm_type_id,
                                       {column}, created_at, updated_at)
                 VALUES ('{}', 'M', 'X', ?1, '9mm', ?2, 1, datetime('now'), datetime('now'))",
                support::uid()
            ),
            rusqlite::params![serial, type_id],
        )
    };
    for column in ["action_type_id", "barrel_length_hundredths", "capacity"] {
        let err = insert(SUPPRESSOR, column, &format!("T5-{column}")).unwrap_err();
        assert!(err.to_string().contains("does not apply"), "{column}: {err}");
        for type_id in 1..=4 {
            insert(type_id, column, &format!("T{type_id}-{column}")).unwrap();
        }
    }

    // A raw UPDATE of the type onto a row that holds a barrel length.
    db.conn
        .execute(
            "INSERT INTO firearms (uid, make, model, serial_number, caliber, firearm_type_id,
                                   barrel_length_hundredths, created_at, updated_at)
             VALUES (?1, 'M', 'Y', 'UPD-1', '9mm', 2, 2000, datetime('now'), datetime('now'))",
            [support::uid()],
        )
        .unwrap();
    let err = db
        .conn
        .execute("UPDATE firearms SET firearm_type_id = 5 WHERE serial_number = 'UPD-1'", [])
        .unwrap_err();
    assert!(err.to_string().contains("does not apply"), "{err}");
    db.conn
        .execute("UPDATE firearms SET firearm_type_id = 3 WHERE serial_number = 'UPD-1'", [])
        .unwrap();
}
