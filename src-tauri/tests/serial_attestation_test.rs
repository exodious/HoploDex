//! Integration tests for the serial-number attestation rule (spec.md
//! Acceptance Scenarios 6-7 / FR-029), run against a real temporary
//! SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::ops;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use support::TestDb;

fn base_input() -> FirearmInput {
    FirearmInput {
        make: "Homemade".into(),
        model: "80% build".into(),
        serial_number: None,
        no_serial_attested: false,
        caliber: ".223".into(),
        firearm_type_id: 2,
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
fn scenario_6_blank_serial_with_attestation_saves_successfully() {
    let db = TestDb::new();

    let mut input = base_input();
    input.serial_number = None;
    input.no_serial_attested = true;

    let created =
        ops::create_firearm(&db.conn, &input, false).expect("attested blank serial should save");
    assert_eq!(created.serial_number, None);
    assert!(created.no_serial_attested);
}

#[test]
fn scenario_7_blank_serial_without_attestation_is_blocked() {
    let db = TestDb::new();

    let mut input = base_input();
    input.serial_number = None;
    input.no_serial_attested = false;

    let result = ops::create_firearm(&db.conn, &input, false);
    let err = result.expect_err("unattested blank serial must be blocked");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(
        err.field_errors.as_ref().is_some_and(|f| f.contains_key("serialNumber")),
        "error should name the serialNumber field: {err:?}"
    );
}

#[test]
fn empty_string_serial_is_treated_the_same_as_blank() {
    let db = TestDb::new();

    let mut input = base_input();
    input.serial_number = Some("   ".into());
    input.no_serial_attested = false;

    assert!(ops::create_firearm(&db.conn, &input, false).is_err());
}

#[test]
fn providing_a_serial_number_never_requires_attestation() {
    let db = TestDb::new();

    let mut input = base_input();
    input.serial_number = Some("XYZ789".into());
    input.no_serial_attested = false;

    let created = ops::create_firearm(&db.conn, &input, false)
        .expect("a provided serial should always be fine");
    assert_eq!(created.serial_number.as_deref(), Some("XYZ789"));
}
