//! Integration tests for User Story 1 (specs/002-firearm-identification):
//! origin, year of manufacture, country of manufacture and importer name,
//! run against a real temporary SQLCipher database — no mocks, per the
//! constitution.

mod support;

use hoplodex_lib::commands::firearms::ops;
use hoplodex_lib::models::firearm::{FirearmInput, Origin};
use support::{TestDb, firearm};

fn field_error(input: &FirearmInput, field: &str) {
    let db = TestDb::new();
    let err = ops::create_firearm(&db.conn, input, false, None).expect_err("should be rejected");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(
        err.field_errors.as_ref().is_some_and(|errors| errors.contains_key(field)),
        "expected a field error for {field}, got {:?}",
        err.field_errors
    );
}

#[test]
fn origin_year_country_and_importer_round_trip_for_imported() {
    let db = TestDb::new();
    let input = FirearmInput {
        origin: Some(Origin::Imported),
        year_of_manufacture: Some(1943),
        country_of_manufacture: Some("Belgium".into()),
        importer_name: Some("Global Arms Import Co.".into()),
        ..firearm("Browning", "Hi-Power", "IMP-1")
    };
    let created = ops::create_firearm(&db.conn, &input, false, None).unwrap();
    assert_eq!(created.origin, Some(Origin::Imported));
    assert_eq!(created.year_of_manufacture, Some(1943));
    assert_eq!(created.country_of_manufacture.as_deref(), Some("Belgium"));
    assert_eq!(created.importer_name.as_deref(), Some("Global Arms Import Co."));

    let fetched = ops::get_firearm(&db.conn, created.id).unwrap();
    assert_eq!(fetched.origin, Some(Origin::Imported));
    assert_eq!(fetched.year_of_manufacture, Some(1943));
    assert_eq!(fetched.country_of_manufacture.as_deref(), Some("Belgium"));
    assert_eq!(fetched.importer_name.as_deref(), Some("Global Arms Import Co."));

    let edited = FirearmInput {
        country_of_manufacture: Some("Austria".into()),
        importer_name: Some("Different Importer".into()),
        ..FirearmInput::from(&created)
    };
    let updated = ops::update_firearm(&db.conn, created.id, &edited, false).unwrap();
    assert_eq!(updated.country_of_manufacture.as_deref(), Some("Austria"));
    assert_eq!(updated.importer_name.as_deref(), Some("Different Importer"));
}

#[test]
fn origin_year_and_importer_round_trip_for_reimported_with_no_country() {
    let db = TestDb::new();
    let input = FirearmInput {
        origin: Some(Origin::Reimported),
        year_of_manufacture: Some(1944),
        importer_name: Some("Century International Arms".into()),
        ..firearm("Inland", "M1 Carbine", "REI-1")
    };
    let created = ops::create_firearm(&db.conn, &input, false, None).unwrap();
    assert_eq!(created.origin, Some(Origin::Reimported));
    assert_eq!(created.year_of_manufacture, Some(1944));
    assert_eq!(created.importer_name.as_deref(), Some("Century International Arms"));
    // US1-2: countryOfManufacture is always null for reimported.
    assert_eq!(created.country_of_manufacture, None);

    let fetched = ops::get_firearm(&db.conn, created.id).unwrap();
    assert_eq!(fetched.country_of_manufacture, None);
}

#[test]
fn a_firearm_with_no_origin_shows_null_with_nothing_else_offered() {
    let db = TestDb::new();
    let created =
        ops::create_firearm(&db.conn, &firearm("Ruger", "10/22", "NO-ORIGIN-1"), false, None)
            .unwrap();
    assert_eq!(created.origin, None);
    assert_eq!(created.year_of_manufacture, None);
    assert_eq!(created.country_of_manufacture, None);
    assert_eq!(created.importer_name, None);
}

#[test]
fn a_firearm_created_before_this_feature_still_round_trips_unchanged() {
    // US1-8: a pre-existing record has no origin data at all; get_firearm
    // must not choke on nulls in the new columns.
    let db = TestDb::new();
    let created =
        ops::create_firearm(&db.conn, &firearm("Colt", "1911", "PRE-1"), false, None).unwrap();
    let fetched = ops::get_firearm(&db.conn, created.id).unwrap();
    assert_eq!(fetched.make, "Colt");
    assert_eq!(fetched.origin, None);
    assert_eq!(fetched.year_of_manufacture, None);
    assert_eq!(fetched.country_of_manufacture, None);
    assert_eq!(fetched.importer_name, None);
    assert_eq!(fetched.original_make, None);
    assert_eq!(fetched.original_model, None);
    assert_eq!(fetched.original_serial_number, None);
}

#[test]
fn a_year_of_1943_saves_and_returns() {
    let db = TestDb::new();
    let input =
        FirearmInput { year_of_manufacture: Some(1943), ..firearm("Colt", "1911A1", "Y-1943") };
    let created = ops::create_firearm(&db.conn, &input, false, None).unwrap();
    assert_eq!(created.year_of_manufacture, Some(1943));
}

#[test]
fn a_year_that_is_not_a_whole_four_digit_number_is_rejected() {
    // A value that decodes but is out of the 1400-9999 DB range is the
    // clearest "not four digits" case reachable through the typed API.
    field_error(
        &FirearmInput { year_of_manufacture: Some(43), ..firearm("Colt", "1911A1", "BAD-1") },
        "yearOfManufacture",
    );
}

#[test]
fn a_year_later_than_the_current_local_year_is_rejected() {
    let next_year =
        chrono::Local::now().date_naive().format("%Y").to_string().parse::<i64>().unwrap() + 1;
    field_error(
        &FirearmInput {
            year_of_manufacture: Some(next_year),
            ..firearm("Colt", "1911A1", "BAD-2")
        },
        "yearOfManufacture",
    );
}

#[test]
fn a_year_earlier_than_1400_is_rejected() {
    field_error(
        &FirearmInput { year_of_manufacture: Some(1399), ..firearm("Colt", "1911A1", "BAD-3") },
        "yearOfManufacture",
    );
}

#[test]
fn a_non_blank_importer_name_on_a_domestic_record_is_rejected() {
    field_error(
        &FirearmInput {
            origin: Some(Origin::Domestic),
            importer_name: Some("Some Importer".into()),
            ..firearm("Colt", "1911A1", "BAD-4")
        },
        "importerName",
    );
}

#[test]
fn a_non_blank_importer_name_on_an_unspecified_origin_record_is_rejected() {
    field_error(
        &FirearmInput {
            importer_name: Some("Some Importer".into()),
            ..firearm("Colt", "1911A1", "BAD-5")
        },
        "importerName",
    );
}

#[test]
fn a_non_blank_country_on_a_reimported_record_is_rejected() {
    field_error(
        &FirearmInput {
            origin: Some(Origin::Reimported),
            country_of_manufacture: Some("Germany".into()),
            ..firearm("Inland", "M1 Carbine", "BAD-6")
        },
        "countryOfManufacture",
    );
}

#[test]
fn a_non_blank_country_on_a_domestic_record_is_rejected() {
    field_error(
        &FirearmInput {
            origin: Some(Origin::Domestic),
            country_of_manufacture: Some("Germany".into()),
            ..firearm("Colt", "1911A1", "BAD-7")
        },
        "countryOfManufacture",
    );
}

// specs/002-firearm-identification User Story 2: the original manufacturer's
// make, model and serial number, alongside the main marks, on an imported or
// re-imported firearm (FR-004).

#[test]
fn original_marks_round_trip_on_an_imported_firearm_distinct_from_the_main_marks() {
    let db = TestDb::new();
    let input = FirearmInput {
        origin: Some(Origin::Imported),
        original_make: Some("Fabrique Nationale".into()),
        original_model: Some("High Power".into()),
        original_serial_number: Some("FN-99001".into()),
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        ..firearm("Ridgeline Arms", "Imported Hi-Power", "RA-5001")
    };
    let created = ops::create_firearm(&db.conn, &input, false, None).unwrap();
    assert_eq!(created.make, "Ridgeline Arms");
    assert_eq!(created.serial_number.as_deref(), Some("RA-5001"));
    assert_eq!(created.original_make.as_deref(), Some("Fabrique Nationale"));
    assert_eq!(created.original_model.as_deref(), Some("High Power"));
    assert_eq!(created.original_serial_number.as_deref(), Some("FN-99001"));

    let fetched = ops::get_firearm(&db.conn, created.id).unwrap();
    assert_eq!(fetched.original_make.as_deref(), Some("Fabrique Nationale"));
    assert_eq!(fetched.original_model.as_deref(), Some("High Power"));
    assert_eq!(fetched.original_serial_number.as_deref(), Some("FN-99001"));

    let edited = FirearmInput {
        original_serial_number: Some("FN-99002".into()),
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        ..FirearmInput::from(&created)
    };
    let updated = ops::update_firearm(&db.conn, created.id, &edited, false).unwrap();
    assert_eq!(updated.original_serial_number.as_deref(), Some("FN-99002"));
}

#[test]
fn original_marks_round_trip_on_a_reimported_firearm() {
    let db = TestDb::new();
    let input = FirearmInput {
        origin: Some(Origin::Reimported),
        importer_name: Some("Century International Arms".into()),
        original_make: Some("Inland".into()),
        original_model: Some("M1 Carbine".into()),
        original_serial_number: Some("IN-2245567".into()),
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        ..firearm("Inland", "M1 Carbine", "IN-2245567")
    };
    let created = ops::create_firearm(&db.conn, &input, false, None).unwrap();
    assert_eq!(created.original_make.as_deref(), Some("Inland"));
    assert_eq!(created.original_model.as_deref(), Some("M1 Carbine"));
    assert_eq!(created.original_serial_number.as_deref(), Some("IN-2245567"));
}

#[test]
fn a_partial_set_of_original_marks_is_accepted_as_entered() {
    let db = TestDb::new();
    let input = FirearmInput {
        origin: Some(Origin::Imported),
        original_make: Some("Fabrique Nationale".into()),
        ..firearm("Ridgeline Arms", "Imported Hi-Power", "RA-5002")
    };
    let created = ops::create_firearm(&db.conn, &input, false, None).unwrap();
    assert_eq!(created.original_make.as_deref(), Some("Fabrique Nationale"));
    assert_eq!(created.original_model, None);
    assert_eq!(created.original_serial_number, None);
}

#[test]
fn leaving_all_three_original_marks_blank_saves_normally() {
    let db = TestDb::new();
    let input =
        FirearmInput { origin: Some(Origin::Imported), ..firearm("FN", "Model 1922", "FN-1") };
    let created = ops::create_firearm(&db.conn, &input, false, None).unwrap();
    assert_eq!(created.original_make, None);
    assert_eq!(created.original_model, None);
    assert_eq!(created.original_serial_number, None);
}

#[test]
fn a_non_blank_original_make_on_a_domestic_record_is_rejected() {
    field_error(
        &FirearmInput {
            origin: Some(Origin::Domestic),
            original_make: Some("Some Maker".into()),
            ..firearm("Colt", "1911A1", "BAD-8")
        },
        "originalMake",
    );
}

#[test]
fn a_non_blank_original_model_on_an_unspecified_origin_record_is_rejected() {
    field_error(
        &FirearmInput {
            original_model: Some("Some Model".into()),
            ..firearm("Colt", "1911A1", "BAD-9")
        },
        "originalModel",
    );
}

#[test]
fn a_non_blank_original_serial_number_on_a_domestic_record_is_rejected() {
    field_error(
        &FirearmInput {
            origin: Some(Origin::Domestic),
            original_serial_number: Some("SN-1".into()),
            registration_class_id: None,
            registration_form: None,
            registration_approved: None,
            registered_to: None,
            ..firearm("Colt", "1911A1", "BAD-10")
        },
        "originalSerialNumber",
    );
}
