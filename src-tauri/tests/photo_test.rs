//! Integration tests for `add_photo`/`set_thumbnail_photo`/`delete_photo`,
//! including thumbnail-fallback behavior (spec.md US4 Acceptance Scenarios
//! 1-3), run against a real temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::photos::ops as photo_ops;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use support::{sample_png_bytes, TestDb};

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
    }
}

#[test]
fn scenario_1_first_photo_becomes_the_thumbnail() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();

    let photo = photo_ops::add_photo(
        &db.conn,
        firearm.id,
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
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    let first =
        photo_ops::add_photo(&db.conn, firearm.id, &sample_png_bytes(), "a.png", "image/png")
            .unwrap();
    let second =
        photo_ops::add_photo(&db.conn, firearm.id, &sample_png_bytes(), "b.png", "image/png")
            .unwrap();

    // Second photo does not automatically become the thumbnail.
    let after_second = firearm_ops::get_firearm(&db.conn, firearm.id).unwrap();
    assert_eq!(after_second.thumbnail_photo_id, Some(first.id));

    let updated = photo_ops::set_thumbnail_photo(&db.conn, firearm.id, second.id).unwrap();
    assert_eq!(updated.thumbnail_photo_id, Some(second.id));
}

#[test]
fn scenario_3_deleting_the_thumbnail_falls_back_to_next_oldest_then_to_generic() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    let first =
        photo_ops::add_photo(&db.conn, firearm.id, &sample_png_bytes(), "a.png", "image/png")
            .unwrap();
    let second =
        photo_ops::add_photo(&db.conn, firearm.id, &sample_png_bytes(), "b.png", "image/png")
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
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    let photo =
        photo_ops::add_photo(&db.conn, firearm.id, &sample_png_bytes(), "a.png", "image/png")
            .unwrap();

    assert!(photo_ops::delete_photo(&db.conn, photo.id, false).is_err());
}

#[test]
fn rejects_an_unsupported_mime_type() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();

    let err = photo_ops::add_photo(
        &db.conn,
        firearm.id,
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
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Range Day.PNG");
    std::fs::write(&path, sample_png_bytes()).unwrap();

    let photo = photo_ops::add_photo_from_path(&db.conn, firearm.id, &path).unwrap();

    assert_eq!(photo.original_filename, "Range Day.PNG");
    assert_eq!(photo.mime_type, "image/png");
    assert_eq!(photo.original_bytes, sample_png_bytes());
    let updated = firearm_ops::get_firearm(&db.conn, firearm.id).unwrap();
    assert_eq!(updated.thumbnail_photo_id, Some(photo.id), "the first photo is the thumbnail");
}

#[test]
fn a_dropped_path_that_is_not_a_photo_is_rejected() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("receipt.pdf");
    std::fs::write(&path, b"%PDF-1.4").unwrap();

    let err = photo_ops::add_photo_from_path(&db.conn, firearm.id, &path)
        .expect_err("a PDF isn't a photo");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(photo_ops::list_photos(&db.conn, firearm.id).unwrap().is_empty());
}

#[test]
fn a_dropped_path_that_no_longer_exists_is_reported() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    let dir = tempfile::tempdir().unwrap();

    let err = photo_ops::add_photo_from_path(&db.conn, firearm.id, &dir.path().join("gone.png"))
        .expect_err("a missing file can't be added");
    assert_eq!(err.code, "NOT_FOUND");
}
