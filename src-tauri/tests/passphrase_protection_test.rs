//! The passphrase-keyed file format (FR-002, FR-003, FR-006, FR-011;
//! research.md §1, §1a, §2; contracts/database-file.md). Every case runs
//! against a real SQLCipher file in a temp directory, made with the
//! production cipher settings.

mod support;

use std::fs;
use std::path::Path;

use hoplodex_lib::db::{self, cipher, DbError, OpenError};
use hoplodex_lib::services::passphrase::{validate_new_passphrase, Passphrase};
use rusqlite::types::Value;
use rusqlite::Connection;
use support::{passphrase, test_machine, TestDb, TEST_PASSPHRASE};
use tempfile::TempDir;

fn phrase(text: &str) -> Passphrase {
    Passphrase::from_input(text.to_owned())
}

/// A pragma's value as text, whether SQLCipher reports it as text or as a
/// number.
fn pragma_text(conn: &Connection, name: &str) -> String {
    let value: Value = conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0)).unwrap();
    match value {
        Value::Text(text) => text,
        Value::Integer(n) => n.to_string(),
        other => panic!("PRAGMA {name} returned {other:?}"),
    }
}

#[test]
fn a_database_reopens_with_its_passphrase_and_keeps_its_rows() {
    let mut db = TestDb::new();
    db.conn.execute("UPDATE collection_settings SET backup_keep_count = 7", []).unwrap();
    db.reopen();

    let keep: i64 = db
        .conn
        .query_row("SELECT backup_keep_count FROM collection_settings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(keep, 7);
}

#[test]
fn a_wrong_passphrase_is_refused_and_the_file_is_untouched() {
    let db = TestDb::new();
    let path = db.path();
    let (conn, _dir) = db.into_parts();
    drop(conn);
    let before = fs::read(&path).unwrap();

    let result = db::open_database(&path, &phrase("not the right passphrase"));

    assert!(matches!(result, Err(OpenError::PassphraseIncorrect)), "{result:?}");
    assert_eq!(fs::read(&path).unwrap(), before, "a refused open must not write the file");
}

#[test]
fn a_new_passphrase_needs_twelve_characters_and_no_nul() {
    let short = validate_new_passphrase(&phrase("elevenchars")).unwrap_err();
    assert_eq!(short.code, "VALIDATION_ERROR");
    assert_eq!(
        short.field_errors.as_ref().unwrap().get("passphrase").map(String::as_str),
        Some("Use at least 12 characters.")
    );

    let nul = validate_new_passphrase(&phrase("twelve chars\0 and a nul")).unwrap_err();
    assert_eq!(nul.code, "VALIDATION_ERROR");
    assert!(nul.field_errors.as_ref().unwrap().contains_key("passphrase"));

    // Twelve Unicode scalar values, not twelve bytes; spaces count.
    assert!(validate_new_passphrase(&phrase("ünïcödé  1 2")).is_ok());
    assert!(validate_new_passphrase(&phrase(TEST_PASSPHRASE)).is_ok());
}

#[test]
fn composed_and_decomposed_forms_of_a_passphrase_open_the_same_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("accents.hoplodex");
    // "café crème brûlée" typed with precomposed letters, then with base
    // letters followed by combining accents (as some macOS input produces).
    let composed = "caf\u{e9} cr\u{e8}me br\u{fb}l\u{e9}e";
    let decomposed = "cafe\u{301} cre\u{300}me bru\u{302}le\u{301}e";
    assert_ne!(composed, decomposed);

    drop(db::create_database(&path, &phrase(composed), &test_machine()).unwrap());

    assert!(db::open_database(&path, &phrase(decomposed)).is_ok());
}

#[test]
fn a_new_database_is_fully_encrypted_with_the_pinned_settings_and_state() {
    let db = TestDb::new();

    let header = &fs::read(db.path()).unwrap()[..16];
    assert_ne!(header, b"SQLite format 3\0", "the whole file must be encrypted");

    for (name, expected) in [
        ("cipher_page_size", "4096"),
        ("kdf_iter", "1000000"),
        ("cipher_kdf_algorithm", "PBKDF2_HMAC_SHA512"),
        ("cipher_hmac_algorithm", "HMAC_SHA512"),
        ("cipher_plaintext_header_size", "0"),
    ] {
        assert_eq!(pragma_text(&db.conn, name), expected, "PRAGMA {name}");
    }
    assert_eq!(pragma_text(&db.conn, "foreign_keys"), "1");
    assert_eq!(pragma_text(&db.conn, "secure_delete"), "1");
    assert_eq!(pragma_text(&db.conn, "locking_mode"), "exclusive");

    let (database_id, changes_waiting): (String, i64) = db
        .conn
        .query_row("SELECT database_id, changes_waiting FROM app_state", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(database_id.len(), 32);
    assert!(database_id.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
    assert_eq!(changes_waiting, 0, "creating a database is not a change to back up");

    let settings: (i64, i64, String, i64, i64, i64) = db
        .conn
        .query_row(
            "SELECT backups_enabled, backup_keep_count, backup_location, idle_lock_enabled,
                    idle_lock_minutes, lock_on_screen_lock
             FROM collection_settings",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .unwrap();
    assert_eq!(settings, (1, 5, "default".to_owned(), 1, 10, 0));
}

#[test]
fn two_databases_get_different_identities() {
    let a = TestDb::new();
    let b = TestDb::new();
    let id = |db: &TestDb| -> String {
        db.conn.query_row("SELECT database_id FROM app_state", [], |r| r.get(0)).unwrap()
    };
    assert_ne!(id(&a), id(&b));
}

#[test]
fn create_refuses_an_existing_path_and_leaves_it_alone() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("taken.hoplodex");
    fs::write(&path, b"somebody else's file").unwrap();

    let result = db::create_database(&path, &passphrase(), &test_machine());

    assert!(matches!(result, Err(DbError::Exists(ref p)) if p == &path), "{result:?}");
    assert_eq!(fs::read(&path).unwrap(), b"somebody else's file");
}

/// An SQLCipher file with the right passphrase and settings that HoploDex
/// did not make: it opens, but has no `app_state`.
fn foreign_database(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "key", TEST_PASSPHRASE).unwrap();
    cipher::apply_cipher_settings(&conn, "main").unwrap();
    conn.execute_batch("CREATE TABLE notes (body TEXT); INSERT INTO notes VALUES ('hello');")
        .unwrap();
}

#[test]
fn a_foreign_sqlcipher_file_is_reported_like_a_wrong_passphrase() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("foreign.hoplodex");
    foreign_database(&path);
    let before = fs::read(&path).unwrap();

    let result = db::open_database(&path, &passphrase());

    assert!(matches!(result, Err(OpenError::PassphraseIncorrect)), "{result:?}");
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn a_plain_sqlite_file_is_reported_like_a_wrong_passphrase() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("plain.hoplodex");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE t (x); INSERT INTO t VALUES (1);").unwrap();
    }

    let result = db::open_database(&path, &passphrase());

    assert!(matches!(result, Err(OpenError::PassphraseIncorrect)), "{result:?}");
}

#[test]
fn verify_passphrase_checks_a_candidate_without_touching_the_open_file() {
    let db = TestDb::new();
    let scratch = TempDir::new().unwrap();
    // The database stays open (and exclusively locked) throughout.
    let before = fs::read(db.path()).unwrap();

    assert!(db::verify_passphrase(&db.path(), &passphrase(), scratch.path()).unwrap());
    assert!(
        !db::verify_passphrase(&db.path(), &phrase("definitely not it"), scratch.path()).unwrap()
    );

    assert_eq!(fs::read(db.path()).unwrap(), before, "the main file must never be written");
    assert_eq!(
        fs::read_dir(scratch.path()).unwrap().count(),
        0,
        "the probe copy must be deleted whatever the result"
    );
    // The connection still works.
    db.conn.execute("UPDATE collection_settings SET idle_lock_minutes = 11", []).unwrap();
}
