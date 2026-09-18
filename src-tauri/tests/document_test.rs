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
        coverage_kind: None,
        scheduled_coverage_amount: None,
    }
}

const SAMPLE_PDF_BYTES: &[u8] = b"%PDF-1.4 sample receipt contents";

#[test]
fn scenario_4_attaches_and_reopens_a_document() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm()).unwrap();

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
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm()).unwrap();
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
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm()).unwrap();
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
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm()).unwrap();
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
