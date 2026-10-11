//! Feature 008 research.md §10, data-model.md "Validation: free-text field
//! maximums": every free-text field has a maximum, counted in Unicode scalar
//! values after trimming. A value at the maximum is saved, one more
//! character is refused with `"<Label> can be at most <n> characters."`
//! under the field's IPC name, and an imported row over it is a row error,
//! not a refusal of the file. Run against a real temporary SQLCipher
//! database.

mod sheets;
mod support;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::{DisposeInput, ops as firearm_ops};
use hoplodex_lib::commands::import_export::ImportSessionStore;
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::accessory::AccessoryInput;
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput, FirearmStatus, Origin};
use hoplodex_lib::models::insurance_policy::InsurancePolicyInput;
use serde_json::json;
use sheets::{accessory_cells, accessory_table, csv_in, entries, firearm_cells, firearm_table};
use support::{TestDb, firearm, policy};
use tempfile::TempDir;

const SHORT: usize = 200;
const LONG: usize = 4_000;
const ENTRY: usize = 100;

fn field_error(err: &CommandError, field: &str) -> Option<String> {
    err.field_errors.as_ref().and_then(|fields| fields.get(field).cloned())
}

/// The message, with the maximum written as the app writes numbers ("4,000").
fn too_long(label: &str, max: usize) -> String {
    let n = match max {
        4_000 => "4,000".to_string(),
        n => n.to_string(),
    };
    format!("{label} can be at most {n} characters.")
}

/// `n` ASCII characters.
fn text_of(n: usize) -> String {
    "a".repeat(n)
}

// --- Fields ---------------------------------------------------------------------------------

type SetFirearm = fn(&mut FirearmInput, String);

/// (IPC name, label in the message, maximum, setter) for each firearm field.
fn firearm_fields() -> Vec<(&'static str, &'static str, usize, SetFirearm)> {
    vec![
        ("make", "Make", ENTRY, |f, v| f.make = v),
        ("model", "Model", ENTRY, |f, v| f.model = v),
        ("cartridge", "Cartridge", ENTRY, |f, v| f.cartridge = Some(v)),
        ("caliber", "Caliber", ENTRY, |f, v| f.caliber = v),
        ("registrationForm", "Form", ENTRY, |f, v| f.registration_form = Some(v)),
        ("registeredTo", "Registered to", ENTRY, |f, v| f.registered_to = Some(v)),
        ("nickname", "Nickname", SHORT, |f, v| f.nickname = Some(v)),
        ("serialNumber", "Serial number", SHORT, |f, v| f.serial_number = Some(v)),
        ("finish", "Finish", SHORT, |f, v| f.finish = Some(v)),
        ("acquisitionSource", "Acquired from", SHORT, |f, v| f.acquisition_source = Some(v)),
        ("dispositionRecipient", "Transferred to", SHORT, |f, v| f.disposition_recipient = Some(v)),
        ("countryOfManufacture", "Country of manufacture", SHORT, |f, v| {
            f.country_of_manufacture = Some(v)
        }),
        ("importerName", "Importer", SHORT, |f, v| f.importer_name = Some(v)),
        ("originalMake", "Original maker", SHORT, |f, v| f.original_make = Some(v)),
        ("originalModel", "Original model", SHORT, |f, v| f.original_model = Some(v)),
        ("originalSerialNumber", "Original serial number", SHORT, |f, v| {
            f.original_serial_number = Some(v)
        }),
        ("notes", "Notes", LONG, |f, v| f.notes = Some(v)),
        ("accessories", "Accessories", LONG, |f, v| f.accessories = Some(v)),
    ]
}

/// A firearm that is valid with any one of the fields above set: imported
/// (so the origin details apply), classified (so the registration details
/// apply) and disposed of (so the recipient applies).
fn base_firearm(tag: usize) -> FirearmInput {
    let mut f = firearm("Make", "Model", &format!("SN-{tag}"));
    f.origin = Some(Origin::Imported);
    f.registration_class_id = Some(1);
    f.status = FirearmStatus::Disposed;
    f.disposition_type = Some(DispositionType::Sold);
    f.disposition_recipient = Some("Jane Doe".into());
    f.disposition_date = Some("2025-06-15".into());
    f
}

type SetAccessory = fn(&mut AccessoryInput, String);

fn accessory_fields() -> Vec<(&'static str, &'static str, usize, SetAccessory)> {
    vec![
        ("make", "Make", ENTRY, |a, v| a.make = v),
        ("model", "Model", ENTRY, |a, v| a.model = v),
        ("cartridge", "Cartridge", ENTRY, |a, v| a.cartridge = Some(v)),
        ("caliber", "Caliber", ENTRY, |a, v| a.caliber = Some(v)),
        ("serialNumber", "Serial number", SHORT, |a, v| a.serial_number = Some(v)),
        ("acquisitionSource", "Acquired from", SHORT, |a, v| a.acquisition_source = Some(v)),
        ("dispositionRecipient", "Transferred to", SHORT, |a, v| a.disposition_recipient = Some(v)),
        ("notes", "Notes", LONG, |a, v| a.notes = Some(v)),
    ]
}

fn base_accessory() -> AccessoryInput {
    serde_json::from_value(json!({
        "accessoryKindId": 1, "make": "Make", "model": "Model", "status": "disposed",
        "dispositionType": "sold", "dispositionRecipient": "Jane Doe",
        "dispositionDate": "2025-06-15",
    }))
    .unwrap()
}

type SetPolicy = fn(&mut InsurancePolicyInput, String);

fn policy_fields() -> Vec<(&'static str, &'static str, usize, SetPolicy)> {
    vec![
        ("name", "Policy name", SHORT, |p, v| p.name = v),
        ("policyNumber", "Policy number", SHORT, |p, v| p.policy_number = v),
        ("insuranceCompany", "Insurance company", SHORT, |p, v| p.insurance_company = v),
        ("companyContact", "Company contact", SHORT, |p, v| p.company_contact = Some(v)),
        ("agentName", "Agent name", SHORT, |p, v| p.agent_name = Some(v)),
        ("agentContact", "Agent contact", SHORT, |p, v| p.agent_contact = Some(v)),
        ("notes", "Notes", LONG, |p, v| p.notes = Some(v)),
    ]
}

// --- Saving -------------------------------------------------------------------------------

#[test]
fn a_firearm_field_at_its_maximum_is_saved_and_one_more_character_is_refused() {
    let db = TestDb::new();
    for (i, (field, label, max, set)) in firearm_fields().into_iter().enumerate() {
        let mut at_max = base_firearm(i);
        set(&mut at_max, text_of(max));
        let created = firearm_ops::create_firearm(&db.conn, &at_max, false, None)
            .unwrap_or_else(|e| panic!("{field} at {max}: {e:?}"));

        // An edit is held to it as well, and nothing is written.
        let mut over = at_max.clone();
        set(&mut over, text_of(max + 1));
        let err = firearm_ops::update_firearm(&db.conn, created.id, &over, false).expect_err(field);
        assert_eq!(err.code, "VALIDATION_ERROR", "{field}");
        assert_eq!(field_error(&err, field), Some(too_long(label, max)), "{field}");
        assert!(firearm_ops::get_firearm(&db.conn, created.id).is_ok());

        let mut fresh = base_firearm(1000 + i);
        set(&mut fresh, text_of(max + 1));
        let err = firearm_ops::create_firearm(&db.conn, &fresh, false, None).expect_err(field);
        assert_eq!(field_error(&err, field), Some(too_long(label, max)), "{field}");
    }
}

#[test]
fn an_accessory_field_at_its_maximum_is_saved_and_one_more_character_is_refused() {
    let db = TestDb::new();
    for (field, label, max, set) in accessory_fields() {
        let mut at_max = base_accessory();
        set(&mut at_max, text_of(max));
        let created = accessory_ops::create_accessory(&db.conn, &at_max, None)
            .unwrap_or_else(|e| panic!("{field} at {max}: {e:?}"));

        let mut over = at_max.clone();
        set(&mut over, text_of(max + 1));
        let err = accessory_ops::update_accessory(&db.conn, created.id, &over).expect_err(field);
        assert_eq!(field_error(&err, field), Some(too_long(label, max)), "{field}");
        let err = accessory_ops::create_accessory(&db.conn, &over, None).expect_err(field);
        assert_eq!(field_error(&err, field), Some(too_long(label, max)), "{field}");
    }
}

#[test]
fn a_policy_field_at_its_maximum_is_saved_and_one_more_character_is_refused() {
    let db = TestDb::new();
    for (field, label, max, set) in policy_fields() {
        let mut at_max = policy("P", "2025-01-01", "2026-01-01", None);
        set(&mut at_max, text_of(max));
        let created = insurance_ops::create_policy(&db.conn, &at_max)
            .unwrap_or_else(|e| panic!("{field} at {max}: {e:?}"));

        let mut over = at_max.clone();
        set(&mut over, text_of(max + 1));
        let err = insurance_ops::update_policy(&db.conn, created.id, &over).expect_err(field);
        assert_eq!(field_error(&err, field), Some(too_long(label, max)), "{field}");
        let err = insurance_ops::create_policy(&db.conn, &over).expect_err(field);
        assert_eq!(field_error(&err, field), Some(too_long(label, max)), "{field}");
    }
}

#[test]
fn the_recipient_of_either_dispose_dialog_has_a_maximum() {
    let db = TestDb::new();
    let dispose = |recipient: String| DisposeInput {
        disposition_type: DispositionType::Sold,
        recipient,
        date: "2025-06-15".into(),
        price: 100,
        with_mounted: Vec::new(),
    };

    let gun =
        firearm_ops::create_firearm(&db.conn, &firearm("Glock", "19", "D1"), false, None).unwrap();
    let err =
        firearm_ops::dispose_firearm(&db.conn, gun.id, &dispose(text_of(SHORT + 1))).unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(field_error(&err, "recipient"), Some(too_long("Transferred to", SHORT)));
    assert_eq!(err.message, too_long("Transferred to", SHORT));
    assert_eq!(
        firearm_ops::get_firearm(&db.conn, gun.id).unwrap().status,
        FirearmStatus::Active,
        "nothing is written"
    );
    let sold = firearm_ops::dispose_firearm(&db.conn, gun.id, &dispose(text_of(SHORT))).unwrap();
    assert_eq!(sold.disposition_recipient.as_deref().map(str::len), Some(SHORT));

    let part = accessory_ops::create_accessory(
        &db.conn,
        &serde_json::from_value(
            json!({ "accessoryKindId": 1, "make": "Make", "model": "Model", "status": "active" }),
        )
        .unwrap(),
        None,
    )
    .unwrap();
    let err = accessory_ops::dispose_accessory(&db.conn, part.id, &dispose(text_of(SHORT + 1)))
        .unwrap_err();
    assert_eq!(field_error(&err, "recipient"), Some(too_long("Transferred to", SHORT)));
    assert_eq!(err.message, too_long("Transferred to", SHORT));
    accessory_ops::dispose_accessory(&db.conn, part.id, &dispose(text_of(SHORT))).unwrap();
}

// --- Counting -----------------------------------------------------------------------------

#[test]
fn characters_are_counted_in_unicode_scalar_values_after_trimming() {
    let db = TestDb::new();

    // A 4-byte character is one character: 200 of them are 800 bytes.
    let mut f = firearm("Glock", "19", "COUNT-1");
    f.nickname = Some("😀".repeat(SHORT));
    firearm_ops::create_firearm(&db.conn, &f, false, None).unwrap();

    f.serial_number = Some("COUNT-2".into());
    f.nickname = Some("😀".repeat(SHORT + 1));
    let err = firearm_ops::create_firearm(&db.conn, &f, false, None).unwrap_err();
    assert_eq!(field_error(&err, "nickname"), Some(too_long("Nickname", SHORT)));

    // Surrounding whitespace is trimmed first, so it never counts.
    f.nickname = Some(format!("  {}\t\n ", "è".repeat(SHORT)));
    firearm_ops::create_firearm(&db.conn, &f, false, None).unwrap();
    f.serial_number = Some("COUNT-3".into());
    f.nickname = None;
    f.notes = Some(format!("\n {} \n", "é".repeat(LONG)));
    firearm_ops::create_firearm(&db.conn, &f, false, None).unwrap();
    f.serial_number = Some("COUNT-4".into());
    f.notes = Some(format!(" {}b ", "é".repeat(LONG)));
    let err = firearm_ops::create_firearm(&db.conn, &f, false, None).unwrap_err();
    assert_eq!(field_error(&err, "notes"), Some(too_long("Notes", LONG)));

    // Whitespace inside the text counts.
    let mut p = policy("Inner", "2025-01-01", "2026-01-01", None);
    p.agent_name = Some(format!("{} {}", "x".repeat(100), "y".repeat(100)));
    let err = insurance_ops::create_policy(&db.conn, &p).unwrap_err();
    assert_eq!(field_error(&err, "agentName"), Some(too_long("Agent name", SHORT)));
}

// --- Importing ----------------------------------------------------------------------------

/// The spreadsheet column of each IPC name above.
fn column_of(field: &str) -> String {
    let mut column = String::new();
    for c in field.chars() {
        if c.is_ascii_uppercase() {
            column.push('_');
        }
        column.push(c.to_ascii_lowercase());
    }
    column
}

#[test]
fn an_imported_cell_over_its_maximum_is_a_row_error_and_one_at_it_imports() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let store = ImportSessionStore::new();

    // The entry fields have their own import handling (snapping, settling);
    // the fields new to §10 are the ones tried here, each in an imported,
    // disposed row so that its details apply.
    let firearm_cols: Vec<_> =
        firearm_fields().into_iter().filter(|(_, _, max, _)| *max != ENTRY).collect();
    let mut firearm_rows = Vec::new();
    let mut expected_firearm = Vec::new(); // (row, column, label, max, accepted)
    for (i, (field, label, max, _)) in firearm_cols.iter().enumerate() {
        for (n, over) in [(0, false), (1, true)] {
            let column = column_of(field);
            let value = text_of(max + usize::from(over));
            let serial = format!("S{i}-{n}");
            let mut cells = vec![
                ("origin", "Imported"),
                ("status", "disposed"),
                ("disposition_type", "Sold"),
                ("disposition_recipient", "Jane Doe"),
                ("disposition_date", "2025-06-15"),
                ("disposition_price", "100"),
            ];
            cells.push((column.as_str(), value.as_str()));
            if *field == "serialNumber" {
                firearm_rows.push(firearm_cells("Make", &format!("M{i}-{n}"), &value, &cells));
            } else {
                firearm_rows.push(firearm_cells("Make", &format!("M{i}-{n}"), &serial, &cells));
            }
            expected_firearm.push((firearm_rows.len() as u64, column, *label, *max, !over));
        }
    }

    let accessory_cols: Vec<_> =
        accessory_fields().into_iter().filter(|(_, _, max, _)| *max != ENTRY).collect();
    let mut accessory_rows = Vec::new();
    let mut expected_accessory = Vec::new();
    for (i, (field, label, max, _)) in accessory_cols.iter().enumerate() {
        for (n, over) in [(0, false), (1, true)] {
            let column = column_of(field);
            let value = text_of(max + usize::from(over));
            let model = format!("A{i}-{n}");
            let cells = [
                ("model", model.as_str()),
                ("status", "disposed"),
                ("disposition_type", "Sold"),
                ("disposition_recipient", "Jane Doe"),
                ("disposition_date", "2025-06-15"),
                ("disposition_price", "100"),
                (column.as_str(), value.as_str()),
            ];
            accessory_rows.push(accessory_cells("Optic", &cells));
            expected_accessory.push((accessory_rows.len() as u64, column, *label, *max, !over));
        }
    }

    let firearm_path = csv_in(dir.path(), "firearms.csv", &firearm_table(&firearm_rows));
    let accessory_path = csv_in(dir.path(), "accessories.csv", &accessory_table(&accessory_rows));
    let result =
        sheets::import(&db.conn, &store, &[&firearm_path, &accessory_path]).expect("the file");

    let errors = entries(&result.row_errors);
    let at = |table: &str, row: u64| -> Vec<String> {
        errors
            .iter()
            .filter(|(t, r, _)| t == table && *r == row)
            .map(|(_, _, m)| m.clone())
            .collect()
    };
    for (table, expected) in [("firearms", &expected_firearm), ("accessories", &expected_accessory)]
    {
        for (row, column, label, max, accepted) in expected {
            let found = at(table, *row);
            if *accepted {
                assert!(found.is_empty(), "{table} row {row} {column} at {max}: {found:?}");
            } else {
                assert_eq!(
                    found,
                    vec![format!("{column}: {}", too_long(label, *max))],
                    "{table} row {row}"
                );
            }
        }
    }
    // `imported_count` counts the accessory rows too.
    assert_eq!(
        result.imported_count,
        firearm_cols.len() + accessory_cols.len(),
        "{:?}",
        result.row_errors
    );
    assert_eq!(result.imported_accessory_count, accessory_cols.len(), "{:?}", result.row_errors);
}
