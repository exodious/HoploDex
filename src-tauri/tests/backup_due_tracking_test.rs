//! What makes a backup due (FR-025, research.md §5): triggers on every
//! collection table set `app_state.changes_waiting` in the same transaction
//! as the change, and housekeeping writes never do.

mod support;

use hoplodex_lib::db;
use hoplodex_lib::models::database::CloseReason;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::session::lifecycle;
use rusqlite::Connection;
use support::{passphrase, test_machine, test_session, TestDb, TEST_PASSPHRASE};
use tempfile::TempDir;

/// Tables that are not collection data and must never make a backup due.
fn is_housekeeping(table: &str) -> bool {
    matches!(table, "app_state" | "pending_changes" | "schema_migrations")
        || table == "firearms_fts"
        || table.starts_with("firearms_fts_")
}

fn changes_waiting(conn: &Connection) -> bool {
    conn.query_row("SELECT changes_waiting FROM app_state", [], |r| r.get(0)).unwrap()
}

fn reset(conn: &Connection) {
    conn.execute("UPDATE app_state SET changes_waiting = 0", []).unwrap();
    assert!(!changes_waiting(conn));
}

#[test]
fn every_table_is_housekeeping_or_tracks_changes() {
    let db = TestDb::new();
    let tables: Vec<String> = db
        .conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let triggers: Vec<String> = db
        .conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'trigger'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();

    let mut untracked = Vec::new();
    for table in tables.iter().filter(|t| !is_housekeeping(t)) {
        for when in ["insert", "update", "delete"] {
            let name = format!("{table}_marks_backup_due_after_{when}");
            if !triggers.contains(&name) {
                untracked.push(name);
            }
        }
    }
    assert!(
        untracked.is_empty(),
        "a collection table must mark a backup due on every change (FR-025); add these \
         triggers, or list the table as housekeeping here if it truly is: {untracked:?}"
    );
    assert!(tables.contains(&"collection_settings".to_owned()));
}

/// One insert, update and delete for each collection table, in an order
/// the foreign keys allow.
const STEPS: &[(&str, &str)] = &[
    ("firearm_types insert", "INSERT INTO firearm_types (id, name, generic_thumbnail_key) VALUES (9, 'Cannon', 'other')"),
    ("firearm_types update", "UPDATE firearm_types SET name = 'Big cannon' WHERE id = 9"),
    ("insurance_policies insert", "INSERT INTO insurance_policies (id, name, policy_number, insurance_company, effective_start_date, effective_end_date, created_at, updated_at) VALUES (1, 'P', 'P-1', 'Acme', '2026-01-01', '2027-01-01', 'now', 'now')"),
    ("insurance_policies update", "UPDATE insurance_policies SET notes = 'n' WHERE id = 1"),
    ("firearms insert", "INSERT INTO firearms (id, make, model, serial_number, caliber, firearm_type_id, created_at, updated_at) VALUES (1, 'Glock', '19', 'ABC', '9mm', 1, 'now', 'now')"),
    ("firearms update", "UPDATE firearms SET notes = 'n' WHERE id = 1"),
    ("photos insert", "INSERT INTO photos (id, firearm_id, original_bytes, original_filename, mime_type, thumbnail_bytes, sort_order, created_at) VALUES (1, 1, x'00', 'a.png', 'image/png', x'00', 0, 'now')"),
    ("photos update", "UPDATE photos SET sort_order = 1 WHERE id = 1"),
    ("photos delete", "DELETE FROM photos WHERE id = 1"),
    ("document_attachments insert", "INSERT INTO document_attachments (id, firearm_id, file_bytes, original_filename, mime_type, created_at) VALUES (1, 1, x'00', 'a.pdf', 'application/pdf', 'now')"),
    ("document_attachments update", "UPDATE document_attachments SET original_filename = 'b.pdf' WHERE id = 1"),
    ("document_attachments delete", "DELETE FROM document_attachments WHERE id = 1"),
    ("disposition_history insert", "INSERT INTO disposition_history (id, firearm_id, disposition_type, disposition_recipient, disposition_date, reversed_at) VALUES (1, 1, 'sold', 'R', '2026-01-02', 'now')"),
    ("disposition_history update", "UPDATE disposition_history SET disposition_recipient = 'S' WHERE id = 1"),
    ("disposition_history delete", "DELETE FROM disposition_history WHERE id = 1"),
    ("firearms delete", "DELETE FROM firearms WHERE id = 1"),
    ("insurance_policies delete", "DELETE FROM insurance_policies WHERE id = 1"),
    ("firearm_types delete", "DELETE FROM firearm_types WHERE id = 9"),
    ("collection_settings update", "UPDATE collection_settings SET idle_lock_minutes = 20"),
    ("collection_settings delete", "DELETE FROM collection_settings"),
    ("collection_settings insert", "INSERT INTO collection_settings (id) VALUES (1)"),
];

#[test]
fn every_insert_update_and_delete_on_collection_data_marks_a_backup_due() {
    let db = TestDb::new();

    for (what, sql) in STEPS {
        reset(&db.conn);
        db.conn.execute(sql, []).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert!(changes_waiting(&db.conn), "{what} must mark a backup due");
    }
}

#[test]
fn housekeeping_writes_do_not_mark_a_backup_due() {
    let db = TestDb::new();

    for sql in [
        "UPDATE app_state SET open_machine_id = NULL, open_machine_name = NULL, open_since = NULL",
        "UPDATE app_state SET open_machine_id = 'aa', open_machine_name = 'PC', open_since = 'now'",
        "UPDATE app_state SET last_backup_at = '2026-09-26T10:00:00Z'",
        "UPDATE app_state SET disk_encryption_note_dismissed = 1",
        "INSERT INTO pending_changes (id, kind, mode, target_id, label, form_version, values_json, saved_at)
         VALUES (1, 'firearm', 'edit', 1, 'Glock 19 — edit', 1, '{}', 'now')",
        "UPDATE pending_changes SET values_json = '{\"a\":1}'",
        "DELETE FROM pending_changes",
    ] {
        db.conn.execute(sql, []).unwrap_or_else(|e| panic!("{sql}: {e}"));
        assert!(!changes_waiting(&db.conn), "{sql} is housekeeping");
    }
}

#[test]
fn creating_opening_and_closing_without_changes_leaves_nothing_waiting() {
    let dir = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let machine = MachineSettings::load(config.path()).unwrap();
    let (session, _events) = test_session(&dir.path().join("opened-documents"));
    let path = dir.path().join("Quiet.hoplodex");

    lifecycle::create(&session, &machine, &path, &passphrase()).unwrap();
    lifecycle::close_normal(&session, CloseReason::Closed).unwrap();
    lifecycle::open(&session, &machine, &path, &passphrase(), false).unwrap();
    lifecycle::close_normal(&session, CloseReason::Closed).unwrap();

    let conn = db::open_database(&path, &passphrase(), &test_machine(), false).unwrap();
    assert!(!changes_waiting(&conn));
}

#[test]
fn sqlcipher_export_into_an_attached_copy_does_not_fire_the_triggers() {
    let db = TestDb::new();
    db.conn
        .execute_batch(
            "INSERT INTO firearms (make, model, serial_number, caliber, firearm_type_id, created_at, updated_at)
             VALUES ('Glock', '19', 'ABC', '9mm', 1, 'now', 'now');",
        )
        .unwrap();
    reset(&db.conn);
    let copy = db.dir().join("copy.hoplodex");

    db.conn
        .execute(
            "ATTACH DATABASE ?1 AS copy KEY ?2",
            rusqlite::params![copy.to_string_lossy(), TEST_PASSPHRASE],
        )
        .unwrap();
    db::cipher::apply_cipher_settings(&db.conn, "copy").unwrap();
    db.conn.query_row("SELECT sqlcipher_export('copy')", [], |_| Ok(())).unwrap();
    db.conn.execute("DETACH DATABASE copy", []).unwrap();

    assert!(!changes_waiting(&db.conn));
    let copied = db::open_database(&copy, &passphrase(), &test_machine(), false).unwrap();
    assert!(!changes_waiting(&copied), "the export must not fire the copy's triggers");
    let firearms: i64 =
        copied.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0)).unwrap();
    assert_eq!(firearms, 1);
}
