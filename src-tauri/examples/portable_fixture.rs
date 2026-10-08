//! Regenerates `tests/fixtures/portable-v1.hoplodex`, the committed database
//! that `tests/portability_test.rs` opens on whatever computer runs the tests
//! (SC-001, SC-002, research.md §20). It is made through the app's own
//! `db::create_database` and `ops`, with [`PASSPHRASE`], and holds one
//! firearm, one photo and one document, whose known values the test compares.
//!
//! Regenerate it only when the file format changes on purpose, and commit
//! the result:
//!
//! ```sh
//! cargo run --example portable_fixture
//! ```
//!
//! It writes that one file inside this checkout and nothing else: no
//! `machine.json`, no keyring entry.

use std::path::PathBuf;

use hoplodex_lib::commands::documents::ops as documents;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::photos::ops as photos;
use hoplodex_lib::db;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::machine_settings::MachineIdentity;
use hoplodex_lib::services::passphrase::Passphrase;

/// The fixture's passphrase. Its "é" checks that a precomposed character
/// gives the same key everywhere (research.md §1).
pub const PASSPHRASE: &str = "portable fixture, café edition";

/// The fixture, relative to `src-tauri`.
pub const FIXTURE: &str = "tests/fixtures/portable-v1.hoplodex";

pub const PHOTO_NAME: &str = "left side.png";
/// A 20×20 solid red PNG, small but real, so the thumbnail is generated from
/// genuine image data.
pub const PHOTO_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 20, 0, 0, 0, 20, 8, 2,
    0, 0, 0, 2, 235, 138, 90, 0, 0, 0, 26, 73, 68, 65, 84, 120, 218, 99, 248, 207, 192, 64, 54, 98,
    24, 213, 60, 170, 121, 84, 243, 168, 230, 129, 213, 12, 0, 49, 205, 142, 128, 132, 11, 139,
    140, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

pub const DOCUMENT_NAME: &str = "bill of sale.txt";
pub const DOCUMENT: &[u8] =
    "Bill of sale: one Smith & Wesson Model 19, serial K123456, for $850.\n".as_bytes();

/// The fixture's one firearm.
pub fn firearm() -> FirearmInput {
    FirearmInput {
        make: "Smith & Wesson".into(),
        model: "Model 19".into(),
        serial_number: Some("K123456".into()),
        no_serial_attested: false,
        caliber: ".357 Magnum".into(),
        firearm_type_id: 1,
        nickname: Some("Combat Magnum".into()),
        notes: Some("Made on Linux; must open unchanged on macOS and Windows.".into()),
        accessories: None,
        barrel_length_hundredths: Some(400),
        overall_length_hundredths: None,
        weight_tenths_oz: None,
        capacity: Some(6),
        finish: None,
        condition: None,
        status: FirearmStatus::Active,
        estimated_value: Some(850),
        acquisition_source: None,
        acquisition_date: Some("1998-05-14".into()),
        acquisition_price: Some(400),
        disposition_type: None,
        disposition_recipient: None,
        disposition_date: None,
        disposition_price: None,
        insurance_policy_id: None,
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
        mounted_on: None,
    }
}

fn main() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).expect("the fixtures folder");
    }
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("could not remove the old fixture: {err}"),
    }

    let machine = MachineIdentity { id: "f".repeat(32), display_name: "Fixture maker".into() };
    let passphrase = Passphrase::from_input(PASSPHRASE.to_owned());
    let conn = db::create_database(&path, &passphrase, &machine).expect("creating the fixture");
    let firearm = firearms::create_firearm(&conn, &firearm(), false, None).expect("the firearm");
    photos::add_photo(&conn, RecordRef::Firearm(firearm.id), PHOTO_PNG, PHOTO_NAME, "image/png")
        .expect("the photo");
    documents::add_document(&conn, RecordRef::Firearm(firearm.id), DOCUMENT, DOCUMENT_NAME)
        .expect("the document");
    // Closed as a normal close leaves it, so it opens anywhere without a
    // take-over.
    conn.execute(
        "UPDATE app_state SET open_machine_id = NULL, open_machine_name = NULL, open_since = NULL",
        [],
    )
    .expect("clearing the open marker");
    drop(conn);

    println!("Wrote {}", path.display());
}
