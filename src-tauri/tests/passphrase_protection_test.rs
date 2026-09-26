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

    let result =
        db::open_database(&path, &phrase("not the right passphrase"), &test_machine(), false);

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

    assert!(db::open_database(&path, &phrase(decomposed), &test_machine(), false).is_ok());
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

    let result = db::open_database(&path, &passphrase(), &test_machine(), false);

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

    let result = db::open_database(&path, &passphrase(), &test_machine(), false);

    assert!(matches!(result, Err(OpenError::PassphraseIncorrect)), "{result:?}");
}

#[test]
fn verify_passphrase_checks_a_candidate_without_touching_the_open_file() {
    let db = TestDb::new();
    let scratch = TempDir::new().unwrap();
    // The database stays open (and exclusively locked) throughout.
    let before = fs::read(db.path()).unwrap();

    assert!(db::verify_passphrase(&db.conn, &passphrase(), scratch.path()).unwrap());
    assert!(!db::verify_passphrase(&db.conn, &phrase("definitely not it"), scratch.path()).unwrap());

    assert_eq!(fs::read(db.path()).unwrap(), before, "the main file must never be written");
    assert_eq!(
        fs::read_dir(scratch.path()).unwrap().count(),
        0,
        "the probe copy must be deleted whatever the result"
    );
    // The connection still works.
    db.conn.execute("UPDATE collection_settings SET idle_lock_minutes = 11", []).unwrap();
}

// --- User Story 1: through `commands::databases::ops` -----------------------

mod through_commands {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::Arc;

    use hoplodex_lib::commands::databases::ops;
    use hoplodex_lib::commands::firearms::ops as firearms;
    use hoplodex_lib::commands::CommandError;
    use hoplodex_lib::models::database::{CloseReason, NoteKind};
    use hoplodex_lib::services::machine_settings::MachineSettings;
    use hoplodex_lib::services::passphrase::Passphrase;
    use hoplodex_lib::session::lifecycle;
    use hoplodex_lib::session::Session;
    use tempfile::TempDir;

    use crate::support::{self, passphrase, test_session, TestEvents, TEST_PASSPHRASE};

    /// A throwaway world: a folder for databases, a config directory for
    /// `machine.json`, and an empty session.
    struct World {
        folder: TempDir,
        _config: TempDir,
        machine: MachineSettings,
        session: Session,
        _events: Arc<TestEvents>,
        _opened_documents: TempDir,
    }

    impl World {
        fn new() -> Self {
            let config = TempDir::new().unwrap();
            let opened_documents = TempDir::new().unwrap();
            let (session, events) = test_session(opened_documents.path());
            Self {
                folder: TempDir::new().unwrap(),
                machine: MachineSettings::load(config.path()).unwrap(),
                _config: config,
                session,
                _events: events,
                _opened_documents: opened_documents,
            }
        }

        fn folder(&self) -> String {
            self.folder.path().to_string_lossy().into_owned()
        }

        fn create(&self, name: &str) -> Result<(), CommandError> {
            ops::create_database(
                &self.session,
                &self.machine,
                &self.folder(),
                name,
                &passphrase(),
                true,
            )
            .map(|_| ())
        }

        fn close(&self) {
            lifecycle::close_normal(&self.session, &self.machine, CloseReason::Closed).unwrap();
        }

        fn path(&self, name: &str) -> std::path::PathBuf {
            self.folder.path().join(format!("{name}.hoplodex"))
        }
    }

    fn field_error(err: &CommandError, field: &str) -> Option<String> {
        err.field_errors.as_ref().and_then(|errors| errors.get(field).cloned())
    }

    #[test]
    fn a_created_database_keeps_a_firearm_across_close_and_reopen() {
        let world = World::new();
        let status = ops::create_database(
            &world.session,
            &world.machine,
            &world.folder(),
            "Main",
            &passphrase(),
            true,
        )
        .unwrap();
        assert_eq!(status.name, "Main");
        assert_eq!(status.path, world.path("Main").to_string_lossy());
        assert!(status.notes.disk_encryption, "a new database shows the disk-encryption note");

        world
            .session
            .write(|conn| {
                firearms::create_firearm(conn, &support::firearm("Colt", "Python", "V1"), false)
            })
            .unwrap();
        world.close();
        assert!(!world.session.is_open());

        let status = ops::open_database(
            &world.session,
            &world.machine,
            &world.path("Main").to_string_lossy(),
            ops::Unlock::typed(&passphrase()),
            false,
        )
        .unwrap();
        assert_eq!(status.name, "Main");
        let makes: Vec<String> = world
            .session
            .read(|conn| {
                let mut stmt = conn.prepare("SELECT make FROM firearms").unwrap();
                let rows = stmt.query_map([], |r| r.get(0)).unwrap();
                Ok(rows.map(Result::unwrap).collect())
            })
            .unwrap();
        assert_eq!(makes, ["Colt"]);

        let recent = world.machine.recent();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].path, world.path("Main"));
    }

    #[test]
    fn a_wrong_passphrase_is_refused_through_the_command_and_the_file_is_untouched() {
        let world = World::new();
        world.create("Main").unwrap();
        world.close();
        let before = fs::read(world.path("Main")).unwrap();

        let err = ops::open_database(
            &world.session,
            &world.machine,
            &world.path("Main").to_string_lossy(),
            ops::Unlock::typed(&Passphrase::from_input("not the passphrase at all".into())),
            false,
        )
        .unwrap_err();

        assert_eq!(err.code, "PASSPHRASE_INCORRECT");
        assert!(!err.message.contains(TEST_PASSPHRASE));
        assert!(!world.session.is_open());
        assert_eq!(fs::read(world.path("Main")).unwrap(), before);
    }

    #[test]
    fn create_refuses_a_short_passphrase_and_a_missing_acknowledgement() {
        let world = World::new();

        let err = ops::create_database(
            &world.session,
            &world.machine,
            &world.folder(),
            "Main",
            &Passphrase::from_input("too short".into()),
            false,
        )
        .unwrap_err();

        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(field_error(&err, "passphrase").as_deref(), Some("Use at least 12 characters."));
        assert!(field_error(&err, "acknowledgedUnrecoverable").is_some(), "FR-004");
        assert!(!world.path("Main").exists(), "nothing is created when the input is refused");
        assert!(!world.session.is_open());
    }

    #[test]
    fn a_foreign_sqlcipher_file_is_passphrase_incorrect_through_the_command() {
        let world = World::new();
        let path = world.path("Foreign");
        super::foreign_database(&path);

        let err = ops::open_database(
            &world.session,
            &world.machine,
            &path.to_string_lossy(),
            ops::Unlock::typed(&passphrase()),
            false,
        )
        .unwrap_err();

        assert_eq!(err.code, "PASSPHRASE_INCORRECT");
    }

    #[test]
    fn dismissing_the_disk_encryption_note_is_kept_and_is_not_a_change_to_back_up() {
        let world = World::new();
        world.create("Main").unwrap();

        ops::dismiss_note(&world.session, NoteKind::DiskEncryption).unwrap();

        let status = ops::database_status(&world.session, &world.machine).unwrap();
        assert!(!status.notes.disk_encryption);
        let (dismissed, waiting): (i64, i64) = world
            .session
            .read(|conn| {
                Ok(conn
                    .query_row(
                        "SELECT disk_encryption_note_dismissed, changes_waiting FROM app_state",
                        [],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .unwrap())
            })
            .unwrap();
        assert_eq!((dismissed, waiting), (1, 0));

        world.close();
        let status = ops::open_database(
            &world.session,
            &world.machine,
            &world.path("Main").to_string_lossy(),
            ops::Unlock::typed(&passphrase()),
            false,
        )
        .unwrap();
        assert!(!status.notes.disk_encryption, "the note stays dismissed (FR-008)");
    }

    #[test]
    fn the_status_reports_the_default_settings() {
        let world = World::new();
        world.create("Main").unwrap();

        let status = ops::database_status(&world.session, &world.machine).unwrap();

        assert!(status.settings.backups.enabled);
        assert_eq!(status.settings.backups.keep_count, 5);
        assert_eq!(
            status.settings.backups.location.path,
            world.folder.path().join("HoploDex backups").to_string_lossy()
        );
        assert!(status.settings.backups.location.available);
        assert!(status.settings.lock.idle_enabled);
        assert_eq!(status.settings.lock.idle_minutes, 10);
        assert!(!status.settings.lock.on_screen_lock);
        assert!(!status.passphrase_saved);
        assert!(status.pending_changes.is_none());
    }

    #[test]
    fn the_status_needs_an_open_database() {
        let world = World::new();
        let err = ops::database_status(&world.session, &world.machine).unwrap_err();
        assert_eq!(err.code, "DATABASE_CLOSED");
    }

    #[test]
    fn create_validates_the_name() {
        let world = World::new();
        let name_error = |name: &str| {
            let err = ops::create_database(
                &world.session,
                &world.machine,
                &world.folder(),
                name,
                &passphrase(),
                true,
            )
            .unwrap_err();
            assert_eq!(err.code, "VALIDATION_ERROR", "{name:?}");
            field_error(&err, "name").unwrap_or_else(|| panic!("no name error for {name:?}"))
        };

        name_error("");
        name_error(&"x".repeat(121));
        for bad in ["a/b", "a\\b", "a<b", "a>b", "a:b", "a\"b", "a|b", "a?b", "a*b", "a\u{7}b"] {
            name_error(bad);
        }
        name_error(".");
        name_error("..");
        name_error("ends with a space ");
        name_error("ends with a dot.");

        // 120 accented characters are 240 bytes: too long for the files
        // made from the name, such as `<name>.hoplodex-journal`.
        name_error(&"é".repeat(120));

        // 120 characters, or 200 bytes, with accents and spaces inside, are
        // fine.
        world.create(&"a b".repeat(40)).unwrap();
        world.close();
        world.create(&"é".repeat(100)).unwrap();
        assert!(world.path(&"é".repeat(100)).exists());
    }

    #[test]
    fn create_validates_the_folder() {
        let world = World::new();
        let folder_error = |folder: &str| {
            let err = ops::create_database(
                &world.session,
                &world.machine,
                folder,
                "Main",
                &passphrase(),
                true,
            )
            .unwrap_err();
            assert_eq!(err.code, "VALIDATION_ERROR", "{folder:?}");
            field_error(&err, "folder").unwrap_or_else(|| panic!("no folder error for {folder:?}"))
        };

        folder_error("");
        folder_error("relative/folder");
        let file = world.folder.path().join("a file");
        fs::write(&file, b"").unwrap();
        folder_error(&file.to_string_lossy());

        let read_only = world.folder.path().join("read-only");
        fs::create_dir(&read_only).unwrap();
        fs::set_permissions(&read_only, fs::Permissions::from_mode(0o555)).unwrap();
        let message = folder_error(&read_only.to_string_lossy());
        fs::set_permissions(&read_only, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(message.contains("write"), "{message}");
    }

    #[test]
    fn create_makes_a_missing_folder_so_the_suggestion_can_be_accepted() {
        let world = World::new();
        let folder = world.folder.path().join("Documents").join("HoploDex");

        ops::create_database(
            &world.session,
            &world.machine,
            &folder.to_string_lossy(),
            "My collection",
            &passphrase(),
            true,
        )
        .unwrap();

        assert!(folder.join("My collection.hoplodex").is_file());
    }

    #[test]
    fn create_refuses_to_overwrite_an_existing_file() {
        let world = World::new();
        fs::write(world.path("Main"), b"precious").unwrap();

        let err = world.create("Main").unwrap_err();

        assert_eq!(err.code, "DATABASE_EXISTS");
        assert_eq!(
            err.details.as_deref().and_then(|d| d["path"].as_str()),
            Some(world.path("Main").to_string_lossy().as_ref())
        );
        assert_eq!(fs::read(world.path("Main")).unwrap(), b"precious");
        assert!(!world.session.is_open());
    }

    #[test]
    fn the_chooser_lists_recent_databases_and_suggests_a_documents_folder() {
        let world = World::new();
        let documents = world.folder.path().join("Documents");

        let state =
            ops::chooser_state(&world.session, &world.machine, Some(documents.clone()), None);
        assert!(state.recent.is_empty());
        assert_eq!(state.selected_path, None);
        assert_eq!(state.suggested.folder, documents.join("HoploDex").to_string_lossy());
        assert_eq!(state.suggested.name, "My collection");
        assert!(!state.keyring_available);
        assert!(!state.screen_lock_supported);

        world.create("First").unwrap();
        world.close();
        world.create("Second").unwrap();
        world.close();

        let state = ops::chooser_state(&world.session, &world.machine, Some(documents), None);
        let names: Vec<&str> = state.recent.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Second", "First"], "most recent first");
        assert!(state.recent.iter().all(|r| r.available));
        assert_eq!(state.selected_path.as_deref(), Some(state.recent[0].path.as_str()));
    }

    #[test]
    fn the_suggestion_falls_back_to_home_documents_and_never_the_data_directory() {
        let home = Path::new("/home/someone");
        assert_eq!(
            ops::suggested_folder(None, Some(home.to_owned())),
            home.join("Documents").join("HoploDex")
        );
    }

    /// SC-002: nothing about a database is kept in the keyring unless the
    /// user asks (FR-017), and creating or opening never asks.
    #[cfg(feature = "mock-keyring")]
    #[test]
    fn create_and_open_write_nothing_to_the_keyring() {
        use std::collections::HashMap;

        keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());
        let world = World::new();
        world.create("Main").unwrap();
        world.close();
        ops::open_database(
            &world.session,
            &world.machine,
            &world.path("Main").to_string_lossy(),
            ops::Unlock::typed(&passphrase()),
            false,
        )
        .unwrap();

        let entries = keyring_core::Entry::search(&HashMap::new()).unwrap();
        assert!(entries.is_empty(), "the keyring holds {} entries", entries.len());
    }
}
