//! A database file plus its passphrase opens on any supported computer
//! (FR-011, SC-001, SC-002, research.md §20). The fixture was made on Linux
//! by `examples/portable_fixture.rs`; opening it here compares every field
//! and byte to the known values, so on macOS and Windows this is the
//! cross-platform check, and anywhere it fails if the pinned cipher
//! settings ever change.

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use hoplodex_lib::commands::documents::ops as documents;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::photos::ops as photos;
use hoplodex_lib::db::{self, OpenError};
use hoplodex_lib::services::machine_settings::MachineIdentity;
use hoplodex_lib::services::passphrase::Passphrase;
use rusqlite::Connection;
use tempfile::TempDir;

#[allow(dead_code)]
#[path = "../examples/portable_fixture.rs"]
mod portable_fixture;

/// A computer that has never seen the fixture.
fn stranger() -> MachineIdentity {
    MachineIdentity { id: "3".repeat(32), display_name: "Somewhere else".into() }
}

/// A copy of the fixture in a temp directory: opening writes the open
/// marker, and the committed file must not change.
fn copy_of_fixture() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let copy = dir.path().join("portable-v1.hoplodex");
    fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join(portable_fixture::FIXTURE), &copy).unwrap();
    (dir, copy)
}

fn open(path: &Path, passphrase: &str) -> Result<Connection, OpenError> {
    db::open_database(path, &Passphrase::from_input(passphrase.to_owned()), &stranger(), false)
}

#[test]
fn the_fixture_opens_with_its_passphrase_and_holds_the_known_records() {
    let (_dir, copy) = copy_of_fixture();

    let conn = open(&copy, portable_fixture::PASSPHRASE).unwrap();

    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM firearms")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(ids.len(), 1);
    let firearm = firearms::get_firearm(&conn, ids[0]).unwrap();
    let expected = portable_fixture::firearm();
    assert_eq!(firearm.make, expected.make);
    assert_eq!(firearm.model, expected.model);
    assert_eq!(firearm.serial_number, expected.serial_number);
    assert_eq!(firearm.caliber, expected.caliber);
    assert_eq!(firearm.nickname, expected.nickname);
    assert_eq!(firearm.notes, expected.notes);
    assert_eq!(firearm.barrel_length_hundredths, expected.barrel_length_hundredths);
    assert_eq!(firearm.capacity, expected.capacity);
    assert_eq!(firearm.estimated_value, expected.estimated_value);
    assert_eq!(firearm.acquisition_date, expected.acquisition_date);
    assert_eq!(firearm.acquisition_price, expected.acquisition_price);

    let photos = photos::list_photos(&conn, firearm.id).unwrap();
    assert_eq!(photos.len(), 1);
    assert_eq!(photos[0].original_filename, portable_fixture::PHOTO_NAME);
    assert_eq!(photos[0].original_bytes, portable_fixture::PHOTO_PNG);
    assert!(!photos[0].thumbnail_bytes.is_empty());

    let documents = documents::list_documents(&conn, firearm.id).unwrap();
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].original_filename, portable_fixture::DOCUMENT_NAME);
    assert_eq!(documents[0].file_bytes, portable_fixture::DOCUMENT);
}

#[test]
fn the_file_starts_with_its_salt_not_a_readable_header() {
    let bytes =
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(portable_fixture::FIXTURE)).unwrap();

    assert!(bytes.len() >= 4096 && bytes.len().is_multiple_of(4096), "whole 4096-byte pages");
    assert_ne!(&bytes[..16], b"SQLite format 3\0", "the whole file is encrypted");
    assert!(bytes[..16].iter().any(|&b| b != 0), "the first 16 bytes are a random salt");
    let text = String::from_utf8_lossy(&bytes);
    for plaintext in ["Smith & Wesson", "K123456", "Bill of sale", "firearms"] {
        assert!(!text.contains(plaintext), "{plaintext:?} is readable in the file");
    }
}

#[test]
fn a_wrong_passphrase_does_not_open_it() {
    let (_dir, copy) = copy_of_fixture();

    let result = open(&copy, "portable fixture, cafe edition");

    assert!(matches!(result, Err(OpenError::PassphraseIncorrect)), "{result:?}");
}

#[test]
fn opening_needs_only_the_file_and_its_passphrase() {
    let (_dir, copy) = copy_of_fixture();
    // Every place this computer keeps anything of HoploDex's, empty: no
    // config directory, no machine.json, no recent list.
    let home = TempDir::new().unwrap();
    for var in ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "APPDATA"] {
        std::env::set_var(var, home.path());
    }
    #[cfg(feature = "mock-keyring")]
    keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());

    open(&copy, portable_fixture::PASSPHRASE).unwrap();

    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0, "nothing machine-local is made");
    #[cfg(feature = "mock-keyring")]
    assert!(keyring_core::Entry::search(&std::collections::HashMap::new()).unwrap().is_empty());
}
