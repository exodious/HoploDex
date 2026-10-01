//! Integration tests for `add_photo`/`set_thumbnail_photo`/`delete_photo`,
//! including thumbnail-fallback behavior (spec.md US4 Acceptance Scenarios
//! 1-3), run against a real temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::photos::ops as photo_ops;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::models::record::RecordRef;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use support::{TestDb, sample_png_bytes};

fn sample_firearm() -> FirearmInput {
    FirearmInput {
        make: "Glock".into(),
        model: "19".into(),
        serial_number: Some("PH-1".into()),
        no_serial_attested: false,
        caliber: "9mm".into(),
        firearm_type_id: 1,
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
        mounted_on: None,
    }
}

#[test]
fn scenario_1_first_photo_becomes_the_thumbnail() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();

    let photo = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "range-day.png",
        "image/png",
    )
    .unwrap();

    assert!(!photo.thumbnail_bytes.is_empty(), "a real thumbnail must be generated");
    let updated = firearm_ops::get_firearm(&db.conn, firearm.id).unwrap();
    assert_eq!(updated.thumbnail_photo_id, Some(photo.id));
}

#[test]
fn scenario_2_a_second_photo_can_be_explicitly_selected_as_thumbnail() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let first = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "a.png",
        "image/png",
    )
    .unwrap();
    let second = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "b.png",
        "image/png",
    )
    .unwrap();

    // Second photo does not automatically become the thumbnail.
    let after_second = firearm_ops::get_firearm(&db.conn, firearm.id).unwrap();
    assert_eq!(after_second.thumbnail_photo_id, Some(first.id));

    let chosen =
        photo_ops::set_thumbnail_photo(&db.conn, RecordRef::Firearm(firearm.id), second.id)
            .unwrap();
    // The result is `{ thumbnailPhotoId }` (contracts/tauri-commands.md "Photos
    // and documents (amended)"), not the whole record.
    assert_eq!(serde_json::to_value(&chosen).unwrap(), json!({ "thumbnailPhotoId": second.id }));
    let updated = firearm_ops::get_firearm(&db.conn, firearm.id).unwrap();
    assert_eq!(updated.thumbnail_photo_id, Some(second.id));
}

#[test]
fn scenario_3_deleting_the_thumbnail_falls_back_to_next_oldest_then_to_generic() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let first = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "a.png",
        "image/png",
    )
    .unwrap();
    let second = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "b.png",
        "image/png",
    )
    .unwrap();

    photo_ops::delete_photo(&db.conn, first.id, true).unwrap();
    let after_first_delete = firearm_ops::get_firearm(&db.conn, firearm.id).unwrap();
    assert_eq!(
        after_first_delete.thumbnail_photo_id,
        Some(second.id),
        "falls back to the next-oldest remaining photo"
    );

    photo_ops::delete_photo(&db.conn, second.id, true).unwrap();
    let after_all_deleted = firearm_ops::get_firearm(&db.conn, firearm.id).unwrap();
    assert_eq!(
        after_all_deleted.thumbnail_photo_id, None,
        "falls back to null (generic thumbnail) once no photos remain"
    );
}

#[test]
fn delete_photo_requires_confirmation() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let photo = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "a.png",
        "image/png",
    )
    .unwrap();

    assert!(photo_ops::delete_photo(&db.conn, photo.id, false).is_err());
}

#[test]
fn rejects_an_unsupported_mime_type() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();

    let err = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "a.exe",
        "application/x-msdownload",
    )
    .expect_err("non-image mime types must be rejected");
    assert_eq!(err.code, "VALIDATION_ERROR");
}

#[test]
fn adds_a_photo_dropped_as_a_file_path() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Range Day.PNG");
    std::fs::write(&path, sample_png_bytes()).unwrap();

    let photo =
        photo_ops::add_photo_from_path(&db.conn, RecordRef::Firearm(firearm.id), &path).unwrap();

    assert_eq!(photo.original_filename, "Range Day.PNG");
    assert_eq!(photo.mime_type, "image/png");
    assert_eq!(photo.original_bytes, sample_png_bytes());
    let updated = firearm_ops::get_firearm(&db.conn, firearm.id).unwrap();
    assert_eq!(updated.thumbnail_photo_id, Some(photo.id), "the first photo is the thumbnail");
}

#[test]
fn a_dropped_path_that_is_not_a_photo_is_rejected() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("receipt.pdf");
    std::fs::write(&path, b"%PDF-1.4").unwrap();

    let err = photo_ops::add_photo_from_path(&db.conn, RecordRef::Firearm(firearm.id), &path)
        .expect_err("a PDF isn't a photo");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(photo_ops::list_photos(&db.conn, RecordRef::Firearm(firearm.id)).unwrap().is_empty());
}

#[test]
fn a_dropped_path_that_no_longer_exists_is_reported() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let dir = tempfile::tempdir().unwrap();

    let err = photo_ops::add_photo_from_path(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &dir.path().join("gone.png"),
    )
    .expect_err("a missing file can't be added");
    assert_eq!(err.code, "NOT_FOUND");
}

// --- Accessory photos (specs/006-accessory-links US1-12, FR-007a) -------------
//
// Photos belong to an owner, `RecordRef::Firearm(id)` or
// `RecordRef::Accessory(id)` (research.md §3, §21). Accessory inputs are built
// from the IPC shape (`AccessoryInput`'s camelCase JSON, contracts/tauri-
// commands.md), so these tests do not depend on how the struct is spelled.

fn parse<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("the JSON must fit the type")
}

/// An active Optic (kind 1) with a make and model.
fn sample_accessory(serial: &str) -> Value {
    json!({
        "accessoryKindId": 1,
        "make": "Leupold",
        "model": "VX-5HD",
        "serialNumber": serial,
        "status": "active",
    })
}

fn create_accessory(db: &TestDb, serial: &str) -> i64 {
    accessory_ops::create_accessory(&db.conn, &parse(sample_accessory(serial)), None).unwrap().id
}

fn accessory_thumbnail(db: &TestDb, id: i64) -> Option<i64> {
    db.conn
        .query_row("SELECT thumbnail_photo_id FROM accessories WHERE id = ?1", [id], |r| r.get(0))
        .unwrap()
}

fn count(db: &TestDb, table: &str) -> i64 {
    db.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
}

#[test]
fn an_accessorys_first_photo_becomes_its_thumbnail() {
    let db = TestDb::new();
    let id = create_accessory(&db, "A-1");
    let owner = RecordRef::Accessory(id);

    let first =
        photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "a.png", "image/png").unwrap();
    let second =
        photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "b.png", "image/png").unwrap();

    assert_eq!(first.owner, owner);
    assert!(!first.thumbnail_bytes.is_empty(), "a real thumbnail must be generated");
    assert_eq!(accessory_thumbnail(&db, id), Some(first.id));
    assert_ne!(second.id, first.id, "a second photo does not replace the thumbnail");
}

#[test]
fn an_accessory_photo_dropped_as_a_file_path_becomes_its_thumbnail() {
    let db = TestDb::new();
    let id = create_accessory(&db, "A-1");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Scope.PNG");
    std::fs::write(&path, sample_png_bytes()).unwrap();

    let photo = photo_ops::add_photo_from_path(&db.conn, RecordRef::Accessory(id), &path).unwrap();

    assert_eq!(photo.owner, RecordRef::Accessory(id));
    assert_eq!(photo.original_filename, "Scope.PNG");
    assert_eq!(photo.original_bytes, sample_png_bytes());
    assert_eq!(accessory_thumbnail(&db, id), Some(photo.id));
}

#[test]
fn an_accessorys_thumbnail_can_be_switched_and_the_result_is_the_photo_id() {
    let db = TestDb::new();
    let id = create_accessory(&db, "A-1");
    let owner = RecordRef::Accessory(id);
    let first =
        photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "a.png", "image/png").unwrap();
    let second =
        photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "b.png", "image/png").unwrap();
    assert_eq!(accessory_thumbnail(&db, id), Some(first.id));

    let chosen = photo_ops::set_thumbnail_photo(&db.conn, owner, second.id).unwrap();

    assert_eq!(serde_json::to_value(&chosen).unwrap(), json!({ "thumbnailPhotoId": second.id }));
    assert_eq!(accessory_thumbnail(&db, id), Some(second.id));
}

/// A firearm and an accessory can have the same numeric id, so the owner's
/// kind is what keeps their photos apart.
#[test]
fn photos_are_listed_by_owner_and_only_that_owners() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let accessory = create_accessory(&db, "A-1");
    assert_eq!(firearm.id, accessory, "both tables start at 1, which is the point of this test");
    let other = create_accessory(&db, "A-2");

    let on_firearm = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "firearm.png",
        "image/png",
    )
    .unwrap();
    let on_accessory = photo_ops::add_photo(
        &db.conn,
        RecordRef::Accessory(accessory),
        &sample_png_bytes(),
        "accessory.png",
        "image/png",
    )
    .unwrap();
    photo_ops::add_photo(
        &db.conn,
        RecordRef::Accessory(other),
        &sample_png_bytes(),
        "other.png",
        "image/png",
    )
    .unwrap();

    let ids = |owner| -> Vec<i64> {
        photo_ops::list_photos(&db.conn, owner).unwrap().iter().map(|p| p.id).collect()
    };
    assert_eq!(ids(RecordRef::Firearm(firearm.id)), vec![on_firearm.id]);
    assert_eq!(ids(RecordRef::Accessory(accessory)), vec![on_accessory.id]);
    assert_eq!(ids(RecordRef::Accessory(other)).len(), 1);
}

#[test]
fn naming_a_photo_with_the_wrong_owner_is_not_found() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let accessory = create_accessory(&db, "A-1");
    let other = create_accessory(&db, "A-2");
    let firearm_photo = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "f.png",
        "image/png",
    )
    .unwrap();
    let accessory_photo = photo_ops::add_photo(
        &db.conn,
        RecordRef::Accessory(accessory),
        &sample_png_bytes(),
        "a.png",
        "image/png",
    )
    .unwrap();

    // Another accessory's photo, a firearm's photo under the accessory's
    // id (the same number), and an accessory's photo under the firearm's id.
    for (owner, photo_id) in [
        (RecordRef::Accessory(other), accessory_photo.id),
        (RecordRef::Accessory(accessory), firearm_photo.id),
        (RecordRef::Firearm(firearm.id), accessory_photo.id),
    ] {
        let err = photo_ops::set_thumbnail_photo(&db.conn, owner, photo_id)
            .expect_err("a photo of another owner can't be the thumbnail");
        assert_eq!(err.code, "NOT_FOUND", "{owner:?} / photo {photo_id}");
    }
    assert_eq!(accessory_thumbnail(&db, other), None, "nothing was changed");
    assert_eq!(accessory_thumbnail(&db, accessory), Some(accessory_photo.id));
}

#[test]
fn deleting_an_accessorys_thumbnail_falls_back_to_the_next_oldest_then_to_generic() {
    let db = TestDb::new();
    let id = create_accessory(&db, "A-1");
    let owner = RecordRef::Accessory(id);
    let first =
        photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "a.png", "image/png").unwrap();
    let second =
        photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "b.png", "image/png").unwrap();

    photo_ops::delete_photo(&db.conn, first.id, true).unwrap();
    assert_eq!(accessory_thumbnail(&db, id), Some(second.id));

    photo_ops::delete_photo(&db.conn, second.id, true).unwrap();
    assert_eq!(accessory_thumbnail(&db, id), None);
}

#[test]
fn a_photo_row_needs_exactly_one_owner() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let accessory = create_accessory(&db, "A-1");
    let insert = |firearm_id: Option<i64>, accessory_id: Option<i64>| {
        db.conn.execute(
            "INSERT INTO photos (
                firearm_id, accessory_id, original_bytes, original_filename, mime_type,
                thumbnail_bytes, sort_order, created_at
            ) VALUES (?1, ?2, x'00', 'p.png', 'image/png', x'00', 0, datetime('now'))",
            rusqlite::params![firearm_id, accessory_id],
        )
    };

    let both = insert(Some(firearm.id), Some(accessory)).expect_err("both owners");
    assert!(both.to_string().contains("CHECK constraint failed"), "{both}");
    let neither = insert(None, None).expect_err("no owner");
    assert!(neither.to_string().contains("CHECK constraint failed"), "{neither}");
    assert_eq!(count(&db, "photos"), 0);

    insert(None, Some(accessory)).expect("one owner is fine");
    insert(Some(firearm.id), None).expect("one owner is fine");
}

#[test]
fn deleting_an_accessory_deletes_its_photos_and_only_its_photos() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let gone = create_accessory(&db, "A-1");
    let kept = create_accessory(&db, "A-2");
    for owner in
        [RecordRef::Accessory(gone), RecordRef::Accessory(gone), RecordRef::Accessory(kept)]
    {
        photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "p.png", "image/png").unwrap();
    }
    photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "f.png",
        "image/png",
    )
    .unwrap();
    assert_eq!(count(&db, "photos"), 4);

    accessory_ops::delete_accessory(&db.conn, gone, true).unwrap();

    assert_eq!(count(&db, "photos"), 2);
    assert!(photo_ops::list_photos(&db.conn, RecordRef::Accessory(gone)).unwrap().is_empty());
    assert_eq!(photo_ops::list_photos(&db.conn, RecordRef::Accessory(kept)).unwrap().len(), 1);
    assert_eq!(photo_ops::list_photos(&db.conn, RecordRef::Firearm(firearm.id)).unwrap().len(), 1);
}
