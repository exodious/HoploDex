//! Integration tests for `add_document`/`get_document`/`delete_document`
//! (spec.md US4 Acceptance Scenario 4; FR-010), run against a real
//! temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use support::TestDb;

fn sample_firearm() -> FirearmInput {
    FirearmInput {
        make: "Glock".into(),
        model: "19".into(),
        serial_number: Some("DOC-1".into()),
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
    }
}

const SAMPLE_PDF_BYTES: &[u8] = b"%PDF-1.4 sample receipt contents";

#[test]
fn scenario_4_attaches_and_reopens_a_document() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();

    let attached = document_ops::add_document(
        &db.conn,
        firearm.id,
        SAMPLE_PDF_BYTES,
        "receipt.pdf",
        "application/pdf",
    )
    .unwrap();
    assert_eq!(attached.original_filename, "receipt.pdf");

    let reopened = document_ops::get_document(&db.conn, attached.id).unwrap();
    assert_eq!(reopened.file_bytes, SAMPLE_PDF_BYTES);
    assert_eq!(reopened.mime_type, "application/pdf");
}

#[test]
fn deletes_a_document_only_when_confirmed() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    let attached = document_ops::add_document(
        &db.conn,
        firearm.id,
        SAMPLE_PDF_BYTES,
        "receipt.pdf",
        "application/pdf",
    )
    .unwrap();

    assert!(document_ops::delete_document(&db.conn, attached.id, false).is_err());
    assert!(document_ops::get_document(&db.conn, attached.id).is_ok());

    let result = document_ops::delete_document(&db.conn, attached.id, true).unwrap();
    assert!(result.deleted);
    assert!(document_ops::get_document(&db.conn, attached.id).is_err());
}

#[test]
fn lists_every_document_for_a_firearm() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    document_ops::add_document(&db.conn, firearm.id, SAMPLE_PDF_BYTES, "a.pdf", "application/pdf")
        .unwrap();
    document_ops::add_document(&db.conn, firearm.id, SAMPLE_PDF_BYTES, "b.pdf", "application/pdf")
        .unwrap();

    let listed = document_ops::list_documents(&db.conn, firearm.id).unwrap();
    assert_eq!(listed.len(), 2);
}

/// `open_document` hands the OS a temporary copy of the document (FR-010:
/// "reopen them from the record") — the copy must hold the original bytes
/// and keep the original filename, reduced to a safe single path component.
#[test]
fn writes_a_temporary_copy_under_a_safe_filename() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    let dir = tempfile::TempDir::new().unwrap();

    let attached = document_ops::add_document(
        &db.conn,
        firearm.id,
        SAMPLE_PDF_BYTES,
        "receipt.pdf",
        "application/pdf",
    )
    .unwrap();
    let path = document_ops::write_document_copy(dir.path(), &attached).unwrap();
    assert_eq!(path, dir.path().join("receipt.pdf"));
    assert_eq!(std::fs::read(&path).unwrap(), SAMPLE_PDF_BYTES);

    let hostile = document_ops::add_document(
        &db.conn,
        firearm.id,
        SAMPLE_PDF_BYTES,
        "../../escape:me?.pdf",
        "application/pdf",
    )
    .unwrap();
    let path = document_ops::write_document_copy(dir.path(), &hostile).unwrap();
    assert_eq!(path, dir.path().join("escape_me_.pdf"));

    let unnamed =
        document_ops::add_document(&db.conn, firearm.id, SAMPLE_PDF_BYTES, "..", "text/plain")
            .unwrap();
    let path = document_ops::write_document_copy(dir.path(), &unnamed).unwrap();
    assert_eq!(path, dir.path().join("document"));
}

#[test]
fn attaches_a_document_dropped_as_a_file_path() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Appraisal 2026.pdf");
    std::fs::write(&path, SAMPLE_PDF_BYTES).unwrap();

    let attached = document_ops::add_document_from_path(&db.conn, firearm.id, &path).unwrap();

    assert_eq!(attached.original_filename, "Appraisal 2026.pdf");
    assert_eq!(attached.mime_type, "application/pdf");
    assert_eq!(attached.file_bytes, SAMPLE_PDF_BYTES);
}

#[test]
fn a_dropped_folder_is_not_attached() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    let dir = tempfile::tempdir().unwrap();

    let err = document_ops::add_document_from_path(&db.conn, firearm.id, dir.path())
        .expect_err("folders can't be attachments");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(document_ops::list_documents(&db.conn, firearm.id).unwrap().is_empty());
}

/// Regression for FR-035 / SC-010: the decrypted copies `open_document`
/// writes must not outlive the session. Clearing the folder overwrites each
/// file's contents before unlinking it — a hard link taken beforehand keeps
/// the old inode reachable, so it shows whether the bytes were really
/// overwritten and not merely unlinked.
#[test]
fn clearing_opened_documents_overwrites_then_removes_each_copy() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false).unwrap();
    let scratch = tempfile::TempDir::new().unwrap();
    let opened = scratch.path().join("opened-documents");

    let attached = document_ops::add_document(
        &db.conn,
        firearm.id,
        SAMPLE_PDF_BYTES,
        "receipt.pdf",
        "application/pdf",
    )
    .unwrap();
    let copy = document_ops::write_document_copy(&opened.join("1"), &attached).unwrap();
    let survivor = scratch.path().join("survivor");
    std::fs::hard_link(&copy, &survivor).unwrap();

    let leftovers = document_ops::clear_opened_documents(&opened);

    assert!(leftovers.is_empty());
    assert!(!opened.exists(), "the folder itself is removed");
    let remaining = std::fs::read(&survivor).unwrap();
    assert_eq!(remaining.len(), SAMPLE_PDF_BYTES.len());
    assert!(
        remaining.iter().all(|&b| b == 0),
        "contents were overwritten before the file was unlinked"
    );
}

#[test]
fn clearing_a_missing_opened_documents_folder_is_a_no_op() {
    let scratch = tempfile::TempDir::new().unwrap();
    assert!(document_ops::clear_opened_documents(&scratch.path().join("never-created")).is_empty());
}

/// Startup retries whatever the previous exit could not delete: a copy that
/// can't be removed is reported and left in place, and the next sweep (once
/// the obstacle is gone) removes it.
#[cfg(unix)]
#[test]
fn a_copy_that_cannot_be_deleted_is_reported_and_retried_by_the_next_sweep() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = tempfile::TempDir::new().unwrap();
    let opened = scratch.path().join("opened-documents");
    let folder = opened.join("7");
    std::fs::create_dir_all(&folder).unwrap();
    let stuck = folder.join("stuck.pdf");
    std::fs::write(&stuck, SAMPLE_PDF_BYTES).unwrap();

    // A read-only folder forbids unlinking its entries.
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(folder.join("probe"), b"").is_ok() {
        // Running as a user the permission bits don't bind (root): the
        // obstacle can't be simulated here.
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let leftovers = document_ops::clear_opened_documents(&opened);
    assert_eq!(leftovers, vec![stuck.clone()]);
    assert!(stuck.exists());

    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(document_ops::clear_opened_documents(&opened).is_empty());
    assert!(!opened.exists());
}
