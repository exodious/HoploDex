//! Integration tests for `add_photo`/`set_thumbnail_photo`/`delete_photo`,
//! including thumbnail-fallback behavior (001 US4-1 to US4-3), run against a
//! real temporary SQLCipher database.

use crate::support;

use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::photos::ops as photo_ops;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::sandbox::Reach;
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
    )
    .unwrap();
    let second = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "b.png",
    )
    .unwrap();

    // Second photo does not automatically become the thumbnail.
    let after_second = firearm_ops::get_firearm(&db.conn, firearm.id).unwrap();
    assert_eq!(after_second.thumbnail_photo_id, Some(first.id));

    let chosen =
        photo_ops::set_thumbnail_photo(&db.conn, RecordRef::Firearm(firearm.id), second.id)
            .unwrap();
    // The result is `{ thumbnailPhotoId }` (006 contracts/tauri-commands.md "Photos
    // and documents (amended)"), not the whole record.
    assert_eq!(serde_json::to_value(&chosen).unwrap(), json!({ "thumbnailPhotoId": second.id }));
    let updated = firearm_ops::get_firearm(&db.conn, firearm.id).unwrap();
    assert_eq!(updated.thumbnail_photo_id, Some(second.id));
}

// 001 US4-3: with no photo left, `thumbnail_photo_id` is None, which is
// what makes the record show its type's generic drawing (FirearmThumbnail.test.tsx).
#[test]
fn scenario_3_deleting_the_thumbnail_falls_back_to_next_oldest_then_to_generic() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let first = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "a.png",
    )
    .unwrap();
    let second = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &sample_png_bytes(),
        "b.png",
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
    )
    .unwrap();

    assert!(photo_ops::delete_photo(&db.conn, photo.id, false).is_err());
}

#[test]
fn rejects_a_file_that_is_neither_jpeg_nor_png() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();

    let err = photo_ops::add_photo(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        b"MZ\x90\x00 a program",
        "a.exe",
    )
    .expect_err("only JPEG and PNG photos are kept");
    assert_eq!(err.code, "VALIDATION_ERROR");
}

#[test]
fn adds_a_photo_dropped_as_a_file_path() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Range Day.PNG");
    std::fs::write(&path, sample_png_bytes()).unwrap();

    let photo = photo_ops::add_photo_from_path(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &path,
        &Reach::Unconfined,
    )
    .unwrap();

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

    let err = photo_ops::add_photo_from_path(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &path,
        &Reach::Unconfined,
    )
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
        &Reach::Unconfined,
    )
    .expect_err("a missing file can't be added");
    assert_eq!(err.code, "NOT_FOUND");
}

// --- Accessory photos (006 US1-12, FR-007a) -----------------------------------
//
// Photos belong to an owner, `RecordRef::Firearm(id)` or
// `RecordRef::Accessory(id)` (006 research.md §3, §21). Accessory inputs are built
// from the IPC shape (`AccessoryInput`'s camelCase JSON, 006
// contracts/tauri-commands.md), so these tests do not depend on how the struct
// is spelled.

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

    let first = photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "a.png").unwrap();
    let second = photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "b.png").unwrap();

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

    let photo = photo_ops::add_photo_from_path(
        &db.conn,
        RecordRef::Accessory(id),
        &path,
        &Reach::Unconfined,
    )
    .unwrap();

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
    let first = photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "a.png").unwrap();
    let second = photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "b.png").unwrap();
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
    )
    .unwrap();
    let on_accessory = photo_ops::add_photo(
        &db.conn,
        RecordRef::Accessory(accessory),
        &sample_png_bytes(),
        "accessory.png",
    )
    .unwrap();
    photo_ops::add_photo(&db.conn, RecordRef::Accessory(other), &sample_png_bytes(), "other.png")
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
    )
    .unwrap();
    let accessory_photo = photo_ops::add_photo(
        &db.conn,
        RecordRef::Accessory(accessory),
        &sample_png_bytes(),
        "a.png",
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
    let first = photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "a.png").unwrap();
    let second = photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "b.png").unwrap();

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
                uid, firearm_id, accessory_id, original_bytes, original_filename, mime_type,
                thumbnail_bytes, sort_order, created_at
            ) VALUES (?3, ?1, ?2, x'00', 'p.png', 'image/png', x'00', 0, datetime('now'))",
            rusqlite::params![firearm_id, accessory_id, support::uid()],
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
        photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "p.png").unwrap();
    }
    photo_ops::add_photo(&db.conn, RecordRef::Firearm(firearm.id), &sample_png_bytes(), "f.png")
        .unwrap();
    assert_eq!(count(&db, "photos"), 4);

    accessory_ops::delete_accessory(&db.conn, gone, true).unwrap();

    assert_eq!(count(&db, "photos"), 2);
    assert!(photo_ops::list_photos(&db.conn, RecordRef::Accessory(gone)).unwrap().is_empty());
    assert_eq!(photo_ops::list_photos(&db.conn, RecordRef::Accessory(kept)).unwrap().len(), 1);
    assert_eq!(photo_ops::list_photos(&db.conn, RecordRef::Firearm(firearm.id)).unwrap().len(), 1);
}

// --- At most 50 photos a record (001 FR-007, amended 2026-10-11; issue #95) ------

/// Gives `owner` its 50 photos.
fn fill_with_photos(db: &TestDb, owner: RecordRef) {
    for n in 0..50 {
        photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), &format!("{n}.png")).unwrap();
    }
}

#[test]
fn a_firearm_with_50_photos_refuses_another_and_stores_nothing() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let owner = RecordRef::Firearm(firearm.id);
    fill_with_photos(&db, owner);

    let err = photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "51.png")
        .expect_err("a 51st photo is refused");
    assert_eq!(err.code, "ATTACHMENT_LIMIT_REACHED");
    assert_eq!(err.message, "This firearm has 50 photos, the most it can have.");
    assert_eq!(err.details.as_deref(), Some(&json!({ "limit": 50 })));
    assert_eq!(count(&db, "photos"), 50);

    // A dropped file is refused the same way.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dropped.png");
    std::fs::write(&path, sample_png_bytes()).unwrap();
    let err =
        photo_ops::add_photo_from_path(&db.conn, owner, &path, &Reach::Unconfined).unwrap_err();
    assert_eq!(err.code, "ATTACHMENT_LIMIT_REACHED");
    assert_eq!(count(&db, "photos"), 50);
}

#[test]
fn a_full_record_is_refused_before_the_photo_is_decoded() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let owner = RecordRef::Firearm(firearm.id);
    fill_with_photos(&db, owner);

    // Bytes that wouldn't decode: the limit is what's reported, so no
    // thumbnail is made for a photo that can't be kept.
    let err = photo_ops::add_photo(&db.conn, owner, b"not an image", "51.png").unwrap_err();
    assert_eq!(err.code, "ATTACHMENT_LIMIT_REACHED");
}

#[test]
fn deleting_a_photo_from_a_full_record_makes_room_for_one_more() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let owner = RecordRef::Firearm(firearm.id);
    fill_with_photos(&db, owner);

    let first = photo_ops::list_photos(&db.conn, owner).unwrap()[0].id;
    photo_ops::delete_photo(&db.conn, first, true).unwrap();
    photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "again.png").unwrap();
    let err = photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "more.png").unwrap_err();
    assert_eq!(err.code, "ATTACHMENT_LIMIT_REACHED");
}

#[test]
fn the_photo_limit_is_each_records_own() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let accessory = create_accessory(&db, "A-1");
    fill_with_photos(&db, RecordRef::Accessory(accessory));

    let err = photo_ops::add_photo(
        &db.conn,
        RecordRef::Accessory(accessory),
        &sample_png_bytes(),
        "51.png",
    )
    .unwrap_err();
    assert_eq!(err.code, "ATTACHMENT_LIMIT_REACHED");
    assert_eq!(err.message, "This accessory has 50 photos, the most it can have.");

    // Another record, firearm or accessory, still takes photos.
    let other = create_accessory(&db, "A-2");
    for owner in [RecordRef::Firearm(firearm.id), RecordRef::Accessory(other)] {
        photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "a.png").unwrap();
    }
}

// --- Bounds on what a photo costs to read (code scanning alerts) -----------------

/// code scanning alert untrusted-images/cwe-770/src-tauri/src/commands/photos.rs#get_photo_thumbnail:
/// a stored thumbnail over 1 MiB, which no HoploDex makes, is refused by
/// `get_photo_thumbnail` and lists empty, and neither reads it: one longer
/// than any connection reads (`db::MAX_VALUE_BYTES`) would fail as `TooBig`
/// if it were. One of exactly 1 MiB still reads, and an empty one is empty.
#[test]
fn a_stored_thumbnail_over_1_mib_is_refused_unread_and_lists_empty() {
    use hoplodex_lib::db;
    use hoplodex_lib::models::photo::MAX_THUMBNAIL_BYTES;
    use rusqlite::limits::Limit;

    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let owner = RecordRef::Firearm(firearm.id);
    let photo = photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "a.png").unwrap();
    let store = |length: i64| {
        db.conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, 1_000_000_000).unwrap();
        db.conn
            .execute(
                "UPDATE photos SET thumbnail_bytes = zeroblob(?1) WHERE id = ?2",
                rusqlite::params![length, photo.id],
            )
            .unwrap();
        db.conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, db::MAX_VALUE_BYTES).unwrap();
    };
    let max = MAX_THUMBNAIL_BYTES as i64;

    // An empty one is empty, not too large.
    store(0);
    assert!(photo_ops::get_photo_thumbnail(&db.conn, photo.id).unwrap().is_empty());

    store(max);
    assert_eq!(photo_ops::get_photo_thumbnail(&db.conn, photo.id).unwrap().len() as i64, max);
    assert_eq!(
        photo_ops::list_photos(&db.conn, owner).unwrap()[0].thumbnail_bytes.len() as i64,
        max
    );

    for length in [max + 1, i64::from(db::MAX_VALUE_BYTES) + 1] {
        store(length);
        let err = photo_ops::get_photo_thumbnail(&db.conn, photo.id).unwrap_err();
        assert_eq!(err.code, "ATTACHMENT_TOO_LARGE", "{length}");
        assert_eq!(
            err.message,
            "This photo's thumbnail is too large to show. HoploDex shows one of at most 1 MiB."
        );
        assert_eq!(err.details.as_deref(), Some(&json!({ "limit": MAX_THUMBNAIL_BYTES })));
        let listed = photo_ops::list_photos(&db.conn, owner).unwrap();
        assert!(listed[0].thumbnail_bytes.is_empty(), "{length}");
    }
}

/// A JPEG of `width` × `height` as its headers state it, made from a small
/// real one: its frame marker set to `frame` (0xC0 baseline, 0xC2
/// progressive), and with `first_scan_one_component` a first scan that holds
/// only the first of its components, as a multi-scan baseline JPEG's does.
/// Only the headers are true, as in the alert's own example: a few hundred
/// bytes that, before the limits, took 1.5 GiB to decode and were accepted.
fn crafted_jpeg(
    gray: bool,
    frame: u8,
    width: u16,
    height: u16,
    first_scan_one_component: bool,
) -> Vec<u8> {
    use image::codecs::jpeg::JpegEncoder;
    use image::{DynamicImage, ImageEncoder};

    let image = if gray { DynamicImage::new_luma8(16, 16) } else { DynamicImage::new_rgb8(16, 16) };
    let mut jpeg = Vec::new();
    JpegEncoder::new(&mut jpeg)
        .write_image(image.as_bytes(), 16, 16, image.color().into())
        .unwrap();
    let mut at = 2;
    loop {
        assert_eq!(jpeg[at], 0xFF);
        let marker = jpeg[at + 1];
        let length = usize::from(u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]));
        if marker == 0xC0 {
            jpeg[at + 1] = frame;
            jpeg[at + 5..at + 7].copy_from_slice(&height.to_be_bytes());
            jpeg[at + 7..at + 9].copy_from_slice(&width.to_be_bytes());
        }
        if marker == 0xDA {
            if frame == 0xC2 {
                // A progressive JPEG's first scan is a DC one: Se is 0.
                let components = usize::from(jpeg[at + 4]);
                jpeg[at + 6 + 2 * components] = 0;
            }
            if first_scan_one_component {
                let first = [jpeg[at + 5], jpeg[at + 6]];
                let scan = [0xFF, 0xDA, 0x00, 0x08, 0x01, first[0], first[1], 0x00, 0x3F, 0x00];
                jpeg.splice(at..at + 2 + length, scan);
            }
            return jpeg;
        }
        at += 2 + length;
    }
}

/// code scanning alert untrusted-images/cwe-789/src-tauri/src/models/photo.rs#generate_thumbnail:
/// a photo over 16,384 pixels on a side, or a progressive or multi-scan JPEG
/// over 64 megapixels (which zune-jpeg decodes through coefficient planes
/// outside the 512 MiB budget), is refused from its headers and nothing is
/// stored. The alert's own example, a progressive 23,170 × 23,170 grayscale
/// JPEG of a few KiB, took 1.5 GiB and was accepted.
#[test]
fn a_photo_past_the_pixel_limits_is_refused_and_nothing_is_stored() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let owner = RecordRef::Firearm(firearm.id);

    for (name, bytes) in [
        ("progressive", crafted_jpeg(true, 0xC2, 23_170, 23_170, false)),
        ("progressive, one row past 64 MP", crafted_jpeg(true, 0xC2, 8_192, 8_193, false)),
        ("multi-scan baseline", crafted_jpeg(false, 0xC0, 8_192, 8_193, true)),
        ("baseline, one pixel past the side", crafted_jpeg(true, 0xC0, 16_385, 8, false)),
    ] {
        let err = photo_ops::add_photo(&db.conn, owner, &bytes, "big.jpg").unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR", "{name}");
        assert_eq!(err.message, "big.jpg has more pixels than HoploDex reads.", "{name}");
    }
    assert_eq!(count(&db, "photos"), 0);

    // At 64 MP the pixel count alone doesn't refuse a progressive one (its
    // made-up scan data may still fail to decode), and a real baseline JPEG
    // is added as before.
    let at_limit = crafted_jpeg(true, 0xC2, 8_192, 8_192, false);
    if let Err(err) = photo_ops::add_photo(&db.conn, owner, &at_limit, "at.jpg") {
        assert_ne!(err.message, "at.jpg has more pixels than HoploDex reads.");
    }
    let real = crafted_jpeg(false, 0xC0, 16, 16, false);
    photo_ops::add_photo(&db.conn, owner, &real, "real.jpg").unwrap();
}
