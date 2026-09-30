//! specs/004-cartridges-action-types User Story 1 through `ops`, against a
//! real temporary SQLCipher database: the cartridge is stored as given,
//! make, model, cartridge and caliber follow FR-015's entry rules, and
//! `settle_entry` reports the caliber a cartridge derives (FR-003, FR-005).

mod support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::entries::{SettleEntryInput, SettleEntryOutput, ops as entry_ops};
use hoplodex_lib::commands::firearms::ops;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::services::cartridges::CaliberSource;
use hoplodex_lib::services::entry_text::EntryField;
use rusqlite::params;
use support::{TestDb, firearm};

fn with_cartridge(cartridge: Option<&str>, caliber: &str, serial: &str) -> FirearmInput {
    FirearmInput {
        cartridge: cartridge.map(str::to_owned),
        caliber: caliber.into(),
        ..firearm("Glock", "17", serial)
    }
}

fn field_error(err: &CommandError, field: &str) -> Option<String> {
    err.field_errors.as_ref().and_then(|errors| errors.get(field).cloned())
}

fn settle(db: &TestDb, field: EntryField, text: &str) -> SettleEntryOutput {
    entry_ops::settle_entry(&db.conn, &SettleEntryInput { field, text: text.into() }).unwrap()
}

#[test]
fn a_catalog_cartridge_round_trips_with_its_caliber() {
    // US1-1.
    let db = TestDb::new();
    let created = ops::create_firearm(
        &db.conn,
        &with_cartridge(Some("9x19mm Parabellum"), "9mm", "C-1"),
        false,
    )
    .unwrap();
    let fetched = ops::get_firearm(&db.conn, created.id).unwrap();
    assert_eq!(fetched.cartridge.as_deref(), Some("9x19mm Parabellum"));
    assert_eq!(fetched.caliber, "9mm");

    let edited = ops::update_firearm(
        &db.conn,
        created.id,
        &FirearmInput { cartridge: Some(".45 ACP".into()), ..FirearmInput::from(&fetched) },
        false,
    )
    .unwrap();
    assert_eq!(edited.cartridge.as_deref(), Some(".45 ACP"));
    assert_eq!(edited.caliber, "9mm", "the backend never re-derives the caliber");
}

#[test]
fn a_custom_cartridge_and_a_caliber_only_record_round_trip() {
    // US1-2 and US1-5.
    let db = TestDb::new();
    let custom = ops::create_firearm(
        &db.conn,
        &with_cartridge(Some(".30 Custom Improved"), ".30", "C-2"),
        false,
    )
    .unwrap();
    let plain = ops::create_firearm(&db.conn, &with_cartridge(None, ".50", "C-3"), false).unwrap();

    let custom = ops::get_firearm(&db.conn, custom.id).unwrap();
    assert_eq!(custom.cartridge.as_deref(), Some(".30 Custom Improved"));
    assert_eq!(custom.caliber, ".30");
    let plain = ops::get_firearm(&db.conn, plain.id).unwrap();
    assert_eq!(plain.cartridge, None);
    assert_eq!(plain.caliber, ".50");
}

#[test]
fn a_blank_cartridge_is_stored_as_null_and_the_four_fields_are_trimmed() {
    let db = TestDb::new();
    let blank =
        ops::create_firearm(&db.conn, &with_cartridge(Some("   "), "9mm", "C-4"), false).unwrap();
    assert_eq!(blank.cartridge, None);
    let stored: Option<String> = db
        .conn
        .query_row("SELECT cartridge FROM firearms WHERE id = ?1", [blank.id], |row| row.get(0))
        .unwrap();
    assert_eq!(stored, None);

    let padded = ops::create_firearm(
        &db.conn,
        &FirearmInput {
            make: "  Smith & Wesson ".into(),
            model: " 686 ".into(),
            caliber: "  .357 ".into(),
            cartridge: Some(" .357 Magnum  ".into()),
            ..firearm("x", "y", "C-5")
        },
        false,
    )
    .unwrap();
    assert_eq!(
        (padded.make.as_str(), padded.model.as_str(), padded.caliber.as_str()),
        ("Smith & Wesson", "686", ".357")
    );
    assert_eq!(padded.cartridge.as_deref(), Some(".357 Magnum"));
}

#[test]
fn create_rejects_an_over_long_or_control_character_value_in_each_field() {
    // FR-015.
    let db = TestDb::new();
    let long = "x".repeat(101);
    let exactly_100 = "é".repeat(100);
    for (field, label) in
        [("make", "Make"), ("model", "Model"), ("cartridge", "Cartridge"), ("caliber", "Caliber")]
    {
        let set = |value: &str| {
            let mut input = with_cartridge(Some("9x19mm Parabellum"), "9mm", "C-6");
            match field {
                "make" => input.make = value.into(),
                "model" => input.model = value.into(),
                "cartridge" => input.cartridge = Some(value.into()),
                _ => input.caliber = value.into(),
            }
            input
        };

        let err = ops::create_firearm(&db.conn, &set(&long), false).unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(
            field_error(&err, field).as_deref(),
            Some(format!("{label} can be at most 100 characters.").as_str())
        );

        let err = ops::create_firearm(&db.conn, &set("Bad\u{7}value"), false).unwrap_err();
        assert_eq!(
            field_error(&err, field).as_deref(),
            Some(format!("{label} can't contain control characters.").as_str())
        );
        let err = ops::create_firearm(&db.conn, &set("Bad\tvalue"), false).unwrap_err();
        assert!(field_error(&err, field).is_some(), "a tab in {field} is a control character");

        // 100 characters, counted as characters, not bytes: accepted.
        let saved = ops::create_firearm(&db.conn, &set(&exactly_100), false).unwrap();
        ops::delete_firearm(&db.conn, saved.id, true).unwrap();
    }
}

#[test]
fn update_checks_only_the_fields_whose_value_changed() {
    // Spec Assumptions: an existing record over the cap stays valid until
    // that field is edited (data-model.md).
    let db = TestDb::new();
    let created =
        ops::create_firearm(&db.conn, &with_cartridge(None, "9mm", "C-7"), false).unwrap();
    let long_make = "M".repeat(120);
    db.conn
        .execute("UPDATE firearms SET make = ?1 WHERE id = ?2", params![long_make, created.id])
        .unwrap();
    let stored = ops::get_firearm(&db.conn, created.id).unwrap();

    let renamed = ops::update_firearm(
        &db.conn,
        created.id,
        &FirearmInput { notes: Some("New grips".into()), ..FirearmInput::from(&stored) },
        false,
    )
    .unwrap();
    assert_eq!(renamed.make, long_make, "the untouched make is kept");
    assert_eq!(renamed.notes.as_deref(), Some("New grips"));

    // Dispose goes through the same update: still fine.
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

    let stored = ops::get_firearm(&db.conn, created.id).unwrap();
    let err = ops::update_firearm(
        &db.conn,
        created.id,
        &FirearmInput { make: "N".repeat(110), ..FirearmInput::from(&stored) },
        false,
    )
    .unwrap_err();
    assert_eq!(field_error(&err, "make").as_deref(), Some("Make can be at most 100 characters."));
    assert_eq!(stored.status, FirearmStatus::Disposed);
}

#[test]
fn create_stores_the_values_given_without_snapping_or_deriving() {
    // research.md §6 and FR-004 / US1-8: the record keeps its own text,
    // whatever the catalog says.
    let db = TestDb::new();
    let created = ops::create_firearm(
        &db.conn,
        &with_cartridge(Some("9X19MM parabellum"), "Nine millimetre", "C-8"),
        false,
    )
    .unwrap();
    let fetched = ops::get_firearm(&db.conn, created.id).unwrap();
    assert_eq!(fetched.cartridge.as_deref(), Some("9X19MM parabellum"));
    assert_eq!(fetched.caliber, "Nine millimetre");

    let different =
        ops::create_firearm(&db.conn, &with_cartridge(Some(".45 ACP"), ".451", "C-9"), false)
            .unwrap();
    let fetched = ops::get_firearm(&db.conn, different.id).unwrap();
    assert_eq!((fetched.cartridge.as_deref(), fetched.caliber.as_str()), (Some(".45 ACP"), ".451"));
}

#[test]
fn settling_a_cartridge_returns_the_caliber_it_derives() {
    let db = TestDb::new();
    for (text, caliber, source) in [
        ("9x19mm Parabellum", "9mm", CaliberSource::Catalog),
        ("9mm Luger", "9mm", CaliberSource::Catalog),
        ("6.5x47 Wildcat", "6.5mm", CaliberSource::Guess),
        (".30 Custom Improved", ".30", CaliberSource::Guess),
    ] {
        let settled = settle(&db, EntryField::Cartridge, text);
        let derived = settled.derived_caliber.unwrap_or_else(|| panic!("{text:?} derived none"));
        assert_eq!((derived.caliber.as_str(), derived.source), (caliber, source), "{text:?}");
    }
    assert!(settle(&db, EntryField::Cartridge, "Wildcat Special").derived_caliber.is_none());
    assert!(settle(&db, EntryField::Cartridge, "").derived_caliber.is_none());
}

#[test]
fn settling_trims_and_keeps_the_text_and_derives_only_for_a_cartridge() {
    let db = TestDb::new();
    let settled = settle(&db, EntryField::Cartridge, "  9mm Luger ");
    assert_eq!(settled.value, "9mm Luger", "an alias is kept as typed");
    assert_eq!(settled.changed_by, None);

    for field in [EntryField::Make, EntryField::Model, EntryField::Caliber] {
        let settled = settle(&db, field, " 9x19mm Parabellum ");
        assert_eq!(settled.value, "9x19mm Parabellum");
        assert!(settled.derived_caliber.is_none(), "{field:?} derives nothing");
    }
}

#[test]
fn settle_entry_serializes_in_the_contract_s_shape() {
    let db = TestDb::new();
    let json =
        serde_json::to_value(settle(&db, EntryField::Cartridge, ".30 Custom Improved")).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "value": ".30 Custom Improved",
            "changedBy": null,
            "derivedCaliber": { "caliber": ".30", "source": "guess" },
        })
    );
    let input: SettleEntryInput =
        serde_json::from_value(serde_json::json!({ "field": "cartridge", "text": "9x19" }))
            .unwrap();
    assert_eq!(input.field, EntryField::Cartridge);
}

#[test]
fn an_input_without_cartridge_or_action_deserializes() {
    // FR-021: callers built before this feature keep working.
    let json = serde_json::json!({
        "make": "Colt", "model": "1911", "serialNumber": "OLD-1", "noSerialAttested": false,
        "caliber": ".45", "firearmTypeId": 1, "nickname": null, "notes": null,
        "accessories": null, "barrelLengthHundredths": null, "overallLengthHundredths": null,
        "weightTenthsOz": null, "capacity": null, "finish": null, "condition": null,
        "status": "active", "estimatedValue": null, "acquisitionSource": null,
        "acquisitionDate": null, "acquisitionPrice": null, "dispositionType": null,
        "dispositionRecipient": null, "dispositionDate": null, "dispositionPrice": null,
        "insurancePolicyId": null, "scheduledCoverageAmount": null, "origin": null,
        "yearOfManufacture": null, "countryOfManufacture": null, "importerName": null,
        "originalMake": null, "originalModel": null, "originalSerialNumber": null,
    });
    let input: FirearmInput = serde_json::from_value(json).unwrap();
    assert_eq!((input.cartridge.as_deref(), input.action_type_id), (None, None));

    let db = TestDb::new();
    ops::create_firearm(&db.conn, &input, false).unwrap();
}
