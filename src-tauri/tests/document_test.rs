//! Integration tests for `add_document`/`get_document`/`delete_document`
//! (spec.md US4 Acceptance Scenario 4; FR-010), run against a real
//! temporary SQLCipher database.

mod support;

use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::models::document_attachment::{DocumentAttachment, DocumentSummary, PreviewKind};
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::preview::availability::{PdfAvailability, UnavailableReason};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use support::hostile_documents as hostile;
use support::{TestDb, document_fixture};

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
        mounted_on: None,
    }
}

const SAMPLE_PDF_BYTES: &[u8] = b"%PDF-1.4 sample receipt contents";

#[test]
fn scenario_4_attaches_and_reopens_a_document() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();

    let attached = document_ops::add_document(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        SAMPLE_PDF_BYTES,
        "receipt.pdf",
    )
    .unwrap();
    assert_eq!(attached.original_filename, "receipt.pdf");

    let reopened = document_ops::get_document(&db.conn, attached.id).unwrap();
    assert_eq!(reopened.file_bytes, SAMPLE_PDF_BYTES);
    assert_eq!(reopened.mime_type, "application/pdf");
}

/// Issue #68: a name is only a name, whatever the webview sends.
#[test]
fn a_document_name_with_a_path_in_it_is_stored_as_its_basename() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();

    for (sent, stored) in [
        ("../../etc/receipt.pdf", "receipt.pdf"),
        (r"C:\Users\me\receipt.pdf", "receipt.pdf"),
        ("a/b\\c.pdf", "c.pdf"),
    ] {
        let attached = document_ops::add_document(
            &db.conn,
            RecordRef::Firearm(firearm.id),
            SAMPLE_PDF_BYTES,
            sent,
        )
        .unwrap();
        assert_eq!(attached.original_filename, stored, "{sent}");
        assert_eq!(attached.mime_type, "application/pdf");
    }
}

#[test]
fn deletes_a_document_only_when_confirmed() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let attached = document_ops::add_document(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        SAMPLE_PDF_BYTES,
        "receipt.pdf",
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
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    document_ops::add_document(&db.conn, RecordRef::Firearm(firearm.id), SAMPLE_PDF_BYTES, "a.pdf")
        .unwrap();
    document_ops::add_document(&db.conn, RecordRef::Firearm(firearm.id), SAMPLE_PDF_BYTES, "b.pdf")
        .unwrap();

    let listed = document_ops::list_documents(&db.conn, RecordRef::Firearm(firearm.id)).unwrap();
    assert_eq!(listed.len(), 2);
}

#[test]
fn attaches_a_document_dropped_as_a_file_path() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Appraisal 2026.pdf");
    std::fs::write(&path, SAMPLE_PDF_BYTES).unwrap();

    let attached =
        document_ops::add_document_from_path(&db.conn, RecordRef::Firearm(firearm.id), &path)
            .unwrap();

    assert_eq!(attached.original_filename, "Appraisal 2026.pdf");
    assert_eq!(attached.mime_type, "application/pdf");
    assert_eq!(attached.file_bytes, SAMPLE_PDF_BYTES);
}

#[test]
fn a_dropped_folder_is_not_attached() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let dir = tempfile::tempdir().unwrap();

    let err =
        document_ops::add_document_from_path(&db.conn, RecordRef::Firearm(firearm.id), dir.path())
            .expect_err("folders can't be attachments");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert!(
        document_ops::list_documents(&db.conn, RecordRef::Firearm(firearm.id)).unwrap().is_empty()
    );
}

/// Regression for FR-035 / SC-010: the decrypted copies `open_document`
/// writes must not outlive the session. Clearing the folder overwrites each
/// file's contents before unlinking it — a hard link taken beforehand keeps
/// the old inode reachable, so it shows whether the bytes were really
/// overwritten and not merely unlinked.
#[test]
fn clearing_opened_documents_overwrites_then_removes_each_copy() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let scratch = tempfile::TempDir::new().unwrap();
    let opened = scratch.path().join("opened-documents");

    let attached = document_ops::add_document(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        SAMPLE_PDF_BYTES,
        "receipt.pdf",
    )
    .unwrap();
    // What `open_document` leaves there (tests/open_document_test.rs writes
    // it through the command's own path).
    std::fs::create_dir_all(opened.join("1")).unwrap();
    let copy = opened.join("1").join("receipt.pdf");
    std::fs::write(&copy, &attached.file_bytes).unwrap();
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

// --- Accessory documents (specs/006-accessory-links US1-12, FR-007a) ----------
//
// Documents belong to an owner, `RecordRef::Firearm(id)` or
// `RecordRef::Accessory(id)` (research.md §3, §21). Accessory inputs are built
// from the IPC shape (`AccessoryInput`'s camelCase JSON, contracts/tauri-
// commands.md), so these tests do not depend on how the struct is spelled.

fn parse<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("the JSON must fit the type")
}

/// An active Optic (kind 1) with a make and model.
fn create_accessory(db: &TestDb, serial: &str) -> i64 {
    let input: Value = json!({
        "accessoryKindId": 1,
        "make": "Leupold",
        "model": "VX-5HD",
        "serialNumber": serial,
        "status": "active",
    });
    accessory_ops::create_accessory(&db.conn, &parse(input), None).unwrap().id
}

fn count(db: &TestDb, table: &str) -> i64 {
    db.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
}

#[test]
fn attaches_and_reopens_a_document_on_an_accessory() {
    let db = TestDb::new();
    let id = create_accessory(&db, "A-1");

    let attached = document_ops::add_document(
        &db.conn,
        RecordRef::Accessory(id),
        SAMPLE_PDF_BYTES,
        "warranty.pdf",
    )
    .unwrap();

    assert_eq!(attached.owner, RecordRef::Accessory(id));
    assert_eq!(attached.original_filename, "warranty.pdf");
    // `open_document` reads it back by id.
    let reopened = document_ops::get_document(&db.conn, attached.id).unwrap();
    assert_eq!(reopened.file_bytes, SAMPLE_PDF_BYTES);
    assert_eq!(reopened.mime_type, "application/pdf");
}

#[test]
fn attaches_a_document_dropped_on_an_accessory_as_a_file_path() {
    let db = TestDb::new();
    let id = create_accessory(&db, "A-1");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Manual.pdf");
    std::fs::write(&path, SAMPLE_PDF_BYTES).unwrap();

    let attached =
        document_ops::add_document_from_path(&db.conn, RecordRef::Accessory(id), &path).unwrap();

    assert_eq!(attached.owner, RecordRef::Accessory(id));
    assert_eq!(attached.original_filename, "Manual.pdf");
    assert_eq!(attached.mime_type, "application/pdf");
    assert_eq!(attached.file_bytes, SAMPLE_PDF_BYTES);
}

/// A firearm and an accessory can have the same numeric id, so the owner's
/// kind is what keeps their documents apart.
#[test]
fn documents_are_listed_by_owner_and_only_that_owners() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let accessory = create_accessory(&db, "A-1");
    let other = create_accessory(&db, "A-2");
    assert_eq!(firearm.id, accessory, "both tables start at 1, which is the point of this test");
    let add = |owner, name: &str| {
        document_ops::add_document(&db.conn, owner, SAMPLE_PDF_BYTES, name).unwrap()
    };
    let on_firearm = add(RecordRef::Firearm(firearm.id), "firearm.pdf");
    let on_accessory = add(RecordRef::Accessory(accessory), "accessory.pdf");
    add(RecordRef::Accessory(other), "other.pdf");
    add(RecordRef::Accessory(other), "other-2.pdf");

    let ids = |owner| -> Vec<i64> {
        document_ops::list_documents(&db.conn, owner).unwrap().iter().map(|d| d.id).collect()
    };
    assert_eq!(ids(RecordRef::Firearm(firearm.id)), vec![on_firearm.id]);
    assert_eq!(ids(RecordRef::Accessory(accessory)), vec![on_accessory.id]);
    assert_eq!(ids(RecordRef::Accessory(other)).len(), 2);
}

#[test]
fn a_document_row_needs_exactly_one_owner() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let accessory = create_accessory(&db, "A-1");
    let insert = |firearm_id: Option<i64>, accessory_id: Option<i64>| {
        db.conn.execute(
            "INSERT INTO document_attachments (
                firearm_id, accessory_id, file_bytes, original_filename, mime_type, created_at
            ) VALUES (?1, ?2, x'00', 'd.pdf', 'application/pdf', datetime('now'))",
            rusqlite::params![firearm_id, accessory_id],
        )
    };

    let both = insert(Some(firearm.id), Some(accessory)).expect_err("both owners");
    assert!(both.to_string().contains("CHECK constraint failed"), "{both}");
    let neither = insert(None, None).expect_err("no owner");
    assert!(neither.to_string().contains("CHECK constraint failed"), "{neither}");
    assert_eq!(count(&db, "document_attachments"), 0);

    insert(None, Some(accessory)).expect("one owner is fine");
    insert(Some(firearm.id), None).expect("one owner is fine");
}

/// The same `CHECK` guards the retained dispositions, so it is tested here
/// beside the other two tables that share the owner pair (research.md §3).
#[test]
fn a_disposition_history_row_needs_exactly_one_owner() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let accessory = create_accessory(&db, "A-1");
    let insert = |firearm_id: Option<i64>, accessory_id: Option<i64>| {
        db.conn.execute(
            "INSERT INTO disposition_history (
                firearm_id, accessory_id, disposition_type, disposition_recipient,
                disposition_date, disposition_price, reversed_at
            ) VALUES (?1, ?2, 'sold', 'A buyer', '2026-01-02', NULL, datetime('now'))",
            rusqlite::params![firearm_id, accessory_id],
        )
    };

    let both = insert(Some(firearm.id), Some(accessory)).expect_err("both owners");
    assert!(both.to_string().contains("CHECK constraint failed"), "{both}");
    let neither = insert(None, None).expect_err("no owner");
    assert!(neither.to_string().contains("CHECK constraint failed"), "{neither}");
    assert_eq!(count(&db, "disposition_history"), 0);

    insert(None, Some(accessory)).expect("one owner is fine");
    insert(Some(firearm.id), None).expect("one owner is fine");
}

#[test]
fn deleting_an_accessory_deletes_its_documents_and_only_its_documents() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let gone = create_accessory(&db, "A-1");
    let kept = create_accessory(&db, "A-2");
    let add =
        |owner| document_ops::add_document(&db.conn, owner, SAMPLE_PDF_BYTES, "d.pdf").unwrap();
    add(RecordRef::Accessory(gone));
    add(RecordRef::Accessory(gone));
    add(RecordRef::Accessory(kept));
    add(RecordRef::Firearm(firearm.id));
    assert_eq!(count(&db, "document_attachments"), 4);

    accessory_ops::delete_accessory(&db.conn, gone, true).unwrap();

    assert_eq!(count(&db, "document_attachments"), 2);
    assert!(document_ops::list_documents(&db.conn, RecordRef::Accessory(gone)).unwrap().is_empty());
    assert_eq!(
        document_ops::list_documents(&db.conn, RecordRef::Accessory(kept)).unwrap().len(),
        1
    );
    assert_eq!(
        document_ops::list_documents(&db.conn, RecordRef::Firearm(firearm.id)).unwrap().len(),
        1
    );
}

// ---- Feature 007: the type rule at attach (FR-016) and what a list derives

/// A row `add_document` would refuse, as a database from before this
/// feature could hold it.
fn insert_raw_document(
    db: &TestDb,
    owner: RecordRef,
    name: &str,
    mime_type: &str,
) -> DocumentAttachment {
    let (firearm_id, accessory_id) = owner.owner_columns();
    db.conn
        .execute(
            "INSERT INTO document_attachments
                (firearm_id, accessory_id, file_bytes, original_filename, mime_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
            rusqlite::params![firearm_id, accessory_id, b"raw".to_vec(), name, mime_type],
        )
        .unwrap();
    document_ops::get_document(&db.conn, db.conn.last_insert_rowid()).unwrap()
}

fn document_count(db: &TestDb) -> i64 {
    db.conn.query_row("SELECT COUNT(*) FROM document_attachments", [], |r| r.get(0)).unwrap()
}

#[test]
fn refuses_what_is_not_a_document_and_stores_nothing() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let owner = RecordRef::Firearm(firearm.id);
    let dir = tempfile::tempdir().unwrap();

    for (file, code) in [
        (hostile::jpg(), "DOCUMENT_TYPE_NOT_ALLOWED"),
        (hostile::exe(), "DOCUMENT_TYPE_NOT_ALLOWED"),
        (hostile::html_named_pdf(), "DOCUMENT_CONTENT_MISMATCH"),
        (hostile::docm(), "DOCUMENT_TYPE_NOT_ALLOWED"),
        (hostile::docx_with_remote_template(), "DOCUMENT_CONTENT_MISMATCH"),
        (hostile::rtf_with_object(), "DOCUMENT_CONTENT_MISMATCH"),
    ] {
        let err = document_ops::add_document(&db.conn, owner, &file.bytes, &file.name)
            .expect_err(&file.name);
        assert_eq!(err.code, code, "{}", file.name);

        let path = dir.path().join(&file.name);
        std::fs::write(&path, &file.bytes).unwrap();
        let err =
            document_ops::add_document_from_path(&db.conn, owner, &path).expect_err(&file.name);
        assert_eq!(err.code, code, "{} from a path", file.name);

        assert_eq!(document_count(&db), 0, "{} left a row", file.name);
    }
}

#[test]
fn a_refused_photo_says_to_add_it_under_photos() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let file = hostile::png();
    let err = document_ops::add_document(
        &db.conn,
        RecordRef::Firearm(firearm.id),
        &file.bytes,
        "front.png",
    )
    .unwrap_err();
    assert_eq!(err.message, "front.png is a photo. Add it under Photos instead.");
}

#[test]
fn a_missing_owner_is_still_not_found() {
    let db = TestDb::new();
    let err =
        document_ops::add_document(&db.conn, RecordRef::Firearm(999), SAMPLE_PDF_BYTES, "a.pdf")
            .unwrap_err();
    assert_eq!(err.code, "NOT_FOUND");
}

#[test]
fn records_the_canonical_type_whatever_the_names_case() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let owner = RecordRef::Firearm(firearm.id);
    let dir = tempfile::tempdir().unwrap();

    let upper =
        document_ops::add_document(&db.conn, owner, SAMPLE_PDF_BYTES, "RECEIPT.PDF").unwrap();
    assert_eq!(upper.mime_type, "application/pdf");
    let tiff =
        document_ops::add_document(&db.conn, owner, &document_fixture("one-page.tif"), "Scan.Tiff")
            .unwrap();
    assert_eq!(tiff.mime_type, "image/tiff");
    let docx =
        document_ops::add_document(&db.conn, owner, &document_fixture("sample.docx"), "Bill.DOCX")
            .unwrap();
    assert_eq!(
        docx.mime_type,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );

    let path = dir.path().join("Round Count.CSV");
    std::fs::write(&path, b"date,rounds\n2026-01-01,150\n").unwrap();
    let csv = document_ops::add_document_from_path(&db.conn, owner, &path).unwrap();
    assert_eq!(csv.mime_type, "text/csv");
}

#[test]
fn list_documents_derives_the_preview_fields_from_the_recorded_type() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let owner = RecordRef::Firearm(firearm.id);
    document_ops::add_document(&db.conn, owner, SAMPLE_PDF_BYTES, "a.pdf").unwrap();
    document_ops::add_document(&db.conn, owner, &document_fixture("sample.docx"), "b.docx")
        .unwrap();
    document_ops::add_document(&db.conn, owner, &document_fixture("one-page.tif"), "c.tif")
        .unwrap();
    document_ops::add_document(&db.conn, owner, b"hello", "d.txt").unwrap();
    insert_raw_document(&db, owner, "Old scan.jpg", "image/jpeg");

    let summaries = |pdf: PdfAvailability| -> Vec<DocumentSummary> {
        document_ops::list_documents(&db.conn, owner)
            .unwrap()
            .into_iter()
            .map(|d| DocumentSummary::new(d, &pdf))
            .collect()
    };
    let fields = |list: &[DocumentSummary]| -> Vec<(Option<PreviewKind>, bool, bool)> {
        list.iter().map(|d| (d.preview_kind, d.preview_available, d.openable)).collect()
    };

    let available = summaries(PdfAvailability::Available);
    assert_eq!(
        fields(&available),
        [
            (Some(PreviewKind::Pdf), true, true),
            (None, false, true),
            (Some(PreviewKind::Tiff), true, true),
            (Some(PreviewKind::Text), true, true),
            (None, false, false),
        ]
    );

    for reason in
        [UnavailableReason::Held, UnavailableReason::CheckFailed, UnavailableReason::NoViewer]
    {
        let off = summaries(PdfAvailability::Unavailable { reason });
        // Only the PDF changes: TIFF and text still preview (FR-003a).
        assert_eq!(
            fields(&off),
            [
                (Some(PreviewKind::Pdf), false, true),
                (None, false, true),
                (Some(PreviewKind::Tiff), true, true),
                (Some(PreviewKind::Text), true, true),
                (None, false, false),
            ],
            "{reason:?}"
        );
    }
}

#[test]
fn the_summary_serializes_with_the_contracts_names() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let owner = RecordRef::Firearm(firearm.id);
    let pdf = document_ops::add_document(&db.conn, owner, SAMPLE_PDF_BYTES, "a.pdf").unwrap();
    let docx =
        document_ops::add_document(&db.conn, owner, &document_fixture("sample.docx"), "b.docx")
            .unwrap();

    let value =
        serde_json::to_value(DocumentSummary::new(pdf, &PdfAvailability::Available)).unwrap();
    assert_eq!(value["previewKind"], "pdf");
    assert_eq!(value["previewAvailable"], true);
    assert_eq!(value["openable"], true);
    let value =
        serde_json::to_value(DocumentSummary::new(docx, &PdfAvailability::Available)).unwrap();
    assert_eq!(value["previewKind"], Value::Null);
}

#[test]
fn a_row_of_another_type_stays_listed_and_can_be_deleted() {
    let db = TestDb::new();
    let firearm = firearm_ops::create_firearm(&db.conn, &sample_firearm(), false, None).unwrap();
    let owner = RecordRef::Firearm(firearm.id);
    let old = insert_raw_document(&db, owner, "Old scan.jpg", "image/jpeg");

    let listed = document_ops::list_documents(&db.conn, owner).unwrap();
    assert_eq!(listed.len(), 1);
    let summary =
        DocumentSummary::new(listed.into_iter().next().unwrap(), &PdfAvailability::Available);
    assert_eq!(
        (summary.preview_kind, summary.preview_available, summary.openable),
        (None, false, false)
    );

    assert!(document_ops::delete_document(&db.conn, old.id, true).unwrap().deleted);
    assert_eq!(document_count(&db), 0);
}
