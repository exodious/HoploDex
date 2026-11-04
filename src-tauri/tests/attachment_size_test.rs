//! The size limits on what is attached (001 FR-007 and FR-010, amended
//! 2026-11-04): a photo at most 15 MiB, a document at most 30 MiB, and one
//! record's photos and documents at most 1 GiB together, each judged before
//! the file is read; and a file's type judged from its name and first bytes
//! before the rest is read. Run against a real temporary SQLCipher database,
//! with the large files made here (sparse where only their size matters).

use std::fs::File;
use std::path::{Path, PathBuf};

use crate::support;

use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::photos::ops as photo_ops;
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::sandbox::Reach;
use serde_json::json;
use support::{TestDb, document_fixture, sample_png_bytes};

const MIB: u64 = 1024 * 1024;
/// The limits, as the amendment states them (`models::rules` keeps them).
const MAX_PHOTO_BYTES: u64 = 15 * MIB;
const MAX_DOCUMENT_BYTES: u64 = 30 * MIB;
const MAX_ATTACHMENT_BYTES_PER_RECORD: u64 = 1024 * MIB;

fn new_firearm(db: &TestDb, serial: &str) -> RecordRef {
    let input = support::firearm("Glock", "19", serial);
    RecordRef::Firearm(firearm_ops::create_firearm(&db.conn, &input, false, None).unwrap().id)
}

fn count(db: &TestDb, table: &str) -> i64 {
    db.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0)).unwrap()
}

/// `head` followed by zeros to `size` bytes. A PNG decoder stops at the
/// image's end, so a PNG padded this way is still the same photo.
fn padded(head: &[u8], size: u64) -> Vec<u8> {
    let mut bytes = head.to_vec();
    bytes.resize(size as usize, 0);
    bytes
}

/// A file of `size` bytes holding only `head`, the rest a hole that is never
/// written, so a file of any size costs no disk.
fn sparse_file(dir: &Path, name: &str, head: &[u8], size: u64) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, head).unwrap();
    File::options().write(true).open(&path).unwrap().set_len(size).unwrap();
    path
}

/// 64 GiB: more than a test machine can allocate.
const HUGE: u64 = 64 * 1024 * 1024 * 1024;

// --- One photo, at most 15 MiB -------------------------------------------------

#[test]
fn a_photo_of_15_mib_is_added_and_one_byte_more_is_refused_before_it_is_read() {
    let db = TestDb::new();
    let owner = new_firearm(&db, "PS-1");
    let dir = tempfile::tempdir().unwrap();

    // At the limit, from a file; just under, as bytes from the file chooser.
    let at = sparse_file(dir.path(), "at.png", &sample_png_bytes(), MAX_PHOTO_BYTES);
    photo_ops::add_photo_from_path(&db.conn, owner, &at, &Reach::Unconfined).unwrap();
    let under = padded(&sample_png_bytes(), MAX_PHOTO_BYTES - 1);
    photo_ops::add_photo(&db.conn, owner, &under, "under.png", "image/png").unwrap();
    assert_eq!(count(&db, "photos"), 2);

    // Just over: refused by its size, not its content, which is no PNG at all
    // (an all-zero file would otherwise fail its header check).
    let over = sparse_file(dir.path(), "over.png", b"", MAX_PHOTO_BYTES + 1);
    let err = photo_ops::add_photo_from_path(&db.conn, owner, &over, &Reach::Unconfined)
        .expect_err("a photo over 15 MiB is refused");
    assert_eq!(err.code, "ATTACHMENT_TOO_LARGE");
    assert_eq!(err.message, "A photo can be at most 15 MiB.");
    assert_eq!(err.details.as_deref(), Some(&json!({ "limit": MAX_PHOTO_BYTES })));

    // A file far past what could be read into memory is refused the same way
    // (Unix only: NTFS reserves the space of a file extended without a hole).
    if cfg!(unix) {
        let huge = sparse_file(dir.path(), "huge.png", b"", HUGE);
        let err =
            photo_ops::add_photo_from_path(&db.conn, owner, &huge, &Reach::Unconfined).unwrap_err();
        assert_eq!(err.code, "ATTACHMENT_TOO_LARGE");
    }

    // And as bytes.
    let over = padded(&sample_png_bytes(), MAX_PHOTO_BYTES + 1);
    let err = photo_ops::add_photo(&db.conn, owner, &over, "over.png", "image/png").unwrap_err();
    assert_eq!(err.code, "ATTACHMENT_TOO_LARGE");
    assert_eq!(count(&db, "photos"), 2);
}

// --- One document, at most 30 MiB ----------------------------------------------

#[test]
fn a_document_of_30_mib_is_attached_and_one_byte_more_is_refused_before_it_is_read() {
    let db = TestDb::new();
    let owner = new_firearm(&db, "DS-1");
    let dir = tempfile::tempdir().unwrap();

    let at = sparse_file(dir.path(), "at.txt", b"Receipt", MAX_DOCUMENT_BYTES);
    document_ops::add_document_from_path(&db.conn, owner, &at, &Reach::Unconfined).unwrap();
    let under = padded(b"%PDF-1.4 receipt", MAX_DOCUMENT_BYTES - 1);
    document_ops::add_document(&db.conn, owner, &under, "under.pdf").unwrap();
    assert_eq!(count(&db, "document_attachments"), 2);

    // An all-zero `.pdf` isn't a PDF: the size is what is reported.
    let over = sparse_file(dir.path(), "over.pdf", b"", MAX_DOCUMENT_BYTES + 1);
    let err = document_ops::add_document_from_path(&db.conn, owner, &over, &Reach::Unconfined)
        .expect_err("a document over 30 MiB is refused");
    assert_eq!(err.code, "ATTACHMENT_TOO_LARGE");
    assert_eq!(err.message, "A document can be at most 30 MiB.");
    assert_eq!(err.details.as_deref(), Some(&json!({ "limit": MAX_DOCUMENT_BYTES })));

    if cfg!(unix) {
        let huge = sparse_file(dir.path(), "huge.pdf", b"", HUGE);
        let err = document_ops::add_document_from_path(&db.conn, owner, &huge, &Reach::Unconfined)
            .unwrap_err();
        assert_eq!(err.code, "ATTACHMENT_TOO_LARGE");
    }

    let over = padded(b"%PDF-1.4 receipt", MAX_DOCUMENT_BYTES + 1);
    let err = document_ops::add_document(&db.conn, owner, &over, "over.pdf").unwrap_err();
    assert_eq!(err.code, "ATTACHMENT_TOO_LARGE");
    assert_eq!(count(&db, "document_attachments"), 2);
}

// --- A record's photos and documents, at most 1 GiB together --------------------

/// Stores a document of `size` zero bytes for `owner`, as SQLite makes it,
/// so a record nears its total without the test holding the bytes.
fn store_zero_document(db: &TestDb, owner: RecordRef, size: u64) {
    let (firearm_id, accessory_id) = owner.owner_columns();
    db.conn
        .execute(
            "INSERT INTO document_attachments
                (uid, firearm_id, accessory_id, file_bytes, original_filename, mime_type, created_at)
             VALUES (?1, ?2, ?3, zeroblob(?4), 'stored.pdf', 'application/pdf', datetime('now'))",
            rusqlite::params![support::uid(), firearm_id, accessory_id, size as i64],
        )
        .unwrap();
}

#[test]
fn a_record_takes_attachments_up_to_1_gib_together_and_refuses_one_past_it() {
    let db = TestDb::new();
    let owner = new_firearm(&db, "TS-1");
    let other = new_firearm(&db, "TS-2");
    let photo = sample_png_bytes();
    let photo_size = photo.len() as u64;
    // Room left for the photo and 100 bytes more.
    // In two, since SQLite holds no blob over 1,000,000,000 bytes.
    let filler = MAX_ATTACHMENT_BYTES_PER_RECORD - photo_size - 100;
    store_zero_document(&db, owner, filler / 2);
    store_zero_document(&db, owner, filler - filler / 2);

    // Photos count toward the same total as documents.
    photo_ops::add_photo(&db.conn, owner, &photo, "a.png", "image/png").unwrap();

    // 101 bytes is one past what's left; 99 is just under.
    let err = document_ops::add_document(&db.conn, owner, &[b'a'; 101], "over.txt")
        .expect_err("a document past the record's total is refused");
    assert_eq!(err.code, "ATTACHMENT_TOO_LARGE");
    assert_eq!(err.message, "This firearm's photos and documents can total at most 1 GiB.");
    assert_eq!(err.details.as_deref(), Some(&json!({ "limit": MAX_ATTACHMENT_BYTES_PER_RECORD })));
    document_ops::add_document(&db.conn, owner, &[b'a'; 99], "under.txt").unwrap();

    // One byte left: two is refused, from a file too, and one fills it exactly.
    let dir = tempfile::tempdir().unwrap();
    let two = dir.path().join("two.txt");
    std::fs::write(&two, b"ab").unwrap();
    let err = document_ops::add_document_from_path(&db.conn, owner, &two, &Reach::Unconfined)
        .unwrap_err();
    assert_eq!(err.code, "ATTACHMENT_TOO_LARGE");
    document_ops::add_document(&db.conn, owner, b"a", "at.txt").unwrap();

    // Full: a photo is refused as a document is, and nothing is stored.
    let err = photo_ops::add_photo(&db.conn, owner, &photo, "b.png", "image/png").unwrap_err();
    assert_eq!(err.code, "ATTACHMENT_TOO_LARGE");
    let path = dir.path().join("b.png");
    std::fs::write(&path, &photo).unwrap();
    let err =
        photo_ops::add_photo_from_path(&db.conn, owner, &path, &Reach::Unconfined).unwrap_err();
    assert_eq!(err.code, "ATTACHMENT_TOO_LARGE");
    assert_eq!(count(&db, "photos"), 1);
    assert_eq!(count(&db, "document_attachments"), 4);

    // The total is each record's own.
    photo_ops::add_photo(&db.conn, other, &photo, "c.png", "image/png").unwrap();
}

#[test]
fn what_a_record_already_holds_past_a_limit_stays_and_still_opens() {
    let db = TestDb::new();
    let owner = new_firearm(&db, "TS-3");
    // A document over 30 MiB, as a database made before the limit holds it.
    let big = padded(b"%PDF-1.4 old scan", MAX_DOCUMENT_BYTES + 1);
    let (firearm_id, _) = owner.owner_columns();
    db.conn
        .execute(
            "INSERT INTO document_attachments
                (uid, firearm_id, file_bytes, original_filename, mime_type, created_at)
             VALUES (?1, ?2, ?3, 'old.pdf', 'application/pdf', datetime('now'))",
            rusqlite::params![support::uid(), firearm_id, big],
        )
        .unwrap();
    let id = db.conn.last_insert_rowid();

    let stored = document_ops::get_document(&db.conn, id).unwrap();
    assert_eq!(stored.file_bytes.len() as u64, MAX_DOCUMENT_BYTES + 1);
    assert_eq!(document_ops::list_documents(&db.conn, owner).unwrap().len(), 1);
    // The record still takes a small one.
    document_ops::add_document(&db.conn, owner, b"%PDF-1.4 receipt", "new.pdf").unwrap();
}

// --- A type judged by its name and first bytes ---------------------------------

#[test]
fn a_document_whose_first_bytes_are_not_its_type_is_refused_and_nothing_is_stored() {
    let db = TestDb::new();
    let owner = new_firearm(&db, "HS-1");
    let dir = tempfile::tempdir().unwrap();

    // A real Word document with bytes before its first entry: the whole-file
    // check finds the package, but a `.docx` must begin as one.
    let mut prefixed = b"not a zip header".to_vec();
    prefixed.extend(document_fixture("sample.docx"));
    let path = dir.path().join("prefixed.docx");
    std::fs::write(&path, &prefixed).unwrap();
    let err = document_ops::add_document_from_path(&db.conn, owner, &path, &Reach::Unconfined)
        .expect_err("a .docx that doesn't begin as a ZIP is refused");
    assert_eq!(err.code, "DOCUMENT_CONTENT_MISMATCH");
    let err = document_ops::add_document(&db.conn, owner, &prefixed, "prefixed.docx").unwrap_err();
    assert_eq!(err.code, "DOCUMENT_CONTENT_MISMATCH");

    // A `.pdf` that begins as a compound file.
    let mut ole = b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1".to_vec();
    ole.resize(4096, 0);
    let path = sparse_file(dir.path(), "scan.pdf", &ole, MAX_DOCUMENT_BYTES);
    let err = document_ops::add_document_from_path(&db.conn, owner, &path, &Reach::Unconfined)
        .unwrap_err();
    assert_eq!(err.code, "DOCUMENT_CONTENT_MISMATCH");
    assert_eq!(count(&db, "document_attachments"), 0);

    // The unprefixed document is attached.
    let path = dir.path().join("plain.docx");
    std::fs::write(&path, document_fixture("sample.docx")).unwrap();
    document_ops::add_document_from_path(&db.conn, owner, &path, &Reach::Unconfined).unwrap();
}

#[test]
fn a_photo_whose_first_bytes_are_not_its_type_is_refused_and_nothing_is_stored() {
    let db = TestDb::new();
    let owner = new_firearm(&db, "HS-2");
    let dir = tempfile::tempdir().unwrap();

    // A PNG named as a JPEG.
    let path = dir.path().join("photo.jpg");
    std::fs::write(&path, sample_png_bytes()).unwrap();
    let err =
        photo_ops::add_photo_from_path(&db.conn, owner, &path, &Reach::Unconfined).unwrap_err();
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(err.message, "photo.jpg isn't a real JPEG photo.");
    let err = photo_ops::add_photo(&db.conn, owner, &sample_png_bytes(), "photo.jpg", "image/jpeg")
        .unwrap_err();
    assert_eq!(err.message, "photo.jpg isn't a real JPEG photo.");

    // Not a PNG at all, under a PNG's name.
    let path = dir.path().join("photo.png");
    std::fs::write(&path, b"%PDF-1.4 a receipt").unwrap();
    let err =
        photo_ops::add_photo_from_path(&db.conn, owner, &path, &Reach::Unconfined).unwrap_err();
    assert_eq!(err.message, "photo.png isn't a real PNG photo.");
    assert_eq!(count(&db, "photos"), 0);
}
