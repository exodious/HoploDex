//! Integration tests for the optional firearm nickname (FR-031, spec.md
//! US1 Acceptance Scenarios 8-9), run against a real temporary SQLCipher
//! database. Reversal clashes are covered in `disposition_reversal_test.rs`.

mod support;

use hoplodex_lib::commands::firearms::{DisposeFirearmInput, ListFirearmsInput, ops};
use hoplodex_lib::commands::import_export::ImportSessionStore;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput};
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use support::{TestDb, csv_file, csv_firearm, firearm};
use tempfile::TempDir;

fn nicknamed(make: &str, model: &str, serial: &str, nickname: &str) -> FirearmInput {
    FirearmInput { nickname: Some(nickname.into()), ..firearm(make, model, serial) }
}

fn dispose(db: &TestDb, id: i64) {
    ops::dispose_firearm(
        &db.conn,
        id,
        &DisposeFirearmInput {
            disposition_type: DispositionType::Sold,
            recipient: "Jane Doe".into(),
            date: "2025-06-15".into(),
            price: 40000,
        },
    )
    .unwrap();
}

fn field_error(err: &hoplodex_lib::commands::CommandError, field: &str) -> String {
    err.field_errors
        .as_ref()
        .and_then(|fields| fields.get(field))
        .unwrap_or_else(|| panic!("no {field} error in {err:?}"))
        .clone()
}

#[test]
fn scenario_8_two_identical_firearms_are_told_apart_by_nickname() {
    let db = TestDb::new();
    let a =
        ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "A1", "Range gun"), false).unwrap();
    let b =
        ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "B2", "Carry gun"), false).unwrap();
    let c = ops::create_firearm(&db.conn, &firearm("Glock", "19", "C3"), false).unwrap();

    assert_eq!(a.nickname.as_deref(), Some("Range gun"));
    assert_eq!(b.nickname.as_deref(), Some("Carry gun"));
    assert_eq!(c.nickname, None, "a blank nickname saves normally");

    let listing = ops::list_firearms(&db.conn, &ListFirearmsInput::default()).unwrap();
    let mut nicknames: Vec<_> =
        listing.groups.iter().flat_map(|g| &g.firearms).map(|f| f.nickname.clone()).collect();
    nicknames.sort();
    assert_eq!(nicknames, vec![None, Some("Carry gun".into()), Some("Range gun".into())]);
}

#[test]
fn a_blank_nickname_is_stored_as_null_and_others_are_trimmed() {
    let db = TestDb::new();
    let blank =
        ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "A1", "   "), false).unwrap();
    let empty = ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "B2", ""), false).unwrap();
    let padded =
        ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "C3", "  Old Faithful "), false)
            .unwrap();

    assert_eq!(blank.nickname, None);
    assert_eq!(empty.nickname, None);
    assert_eq!(padded.nickname.as_deref(), Some("Old Faithful"));
}

#[test]
fn scenario_9_a_duplicate_nickname_is_blocked_ignoring_case_and_whitespace() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "A1", "Old Faithful"), false).unwrap();

    let err =
        ops::create_firearm(&db.conn, &nicknamed("Sig", "P226", "B2", "  old FAITHFUL "), false)
            .expect_err("duplicate nickname must be blocked");

    assert_eq!(err.code, "VALIDATION_ERROR");
    let message = field_error(&err, "nickname");
    assert!(
        message.contains("Glock") && message.contains("19"),
        "the message names the conflicting firearm: {message}"
    );
    let listing = ops::list_firearms(&db.conn, &ListFirearmsInput::default()).unwrap();
    assert_eq!(listing.groups.iter().map(|g| g.firearms.len()).sum::<usize>(), 1);
}

#[test]
fn a_record_can_keep_its_own_nickname_on_edit_but_not_take_another_s() {
    let db = TestDb::new();
    let first =
        ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "A1", "Old Faithful"), false)
            .unwrap();
    let second =
        ops::create_firearm(&db.conn, &nicknamed("Sig", "P226", "B2", "Backup"), false).unwrap();

    let mut edit = nicknamed("Glock", "19", "A1", "old faithful");
    edit.notes = Some("re-blued".into());
    assert!(
        ops::update_firearm(&db.conn, first.id, &edit, false).is_ok(),
        "own nickname is not a clash"
    );

    let steal = nicknamed("Sig", "P226", "B2", "OLD FAITHFUL");
    let err = ops::update_firearm(&db.conn, second.id, &steal, false).expect_err("blocked");
    assert_eq!(err.code, "VALIDATION_ERROR");
    assert_eq!(ops::get_firearm(&db.conn, second.id).unwrap().nickname.as_deref(), Some("Backup"));
}

#[test]
fn disposing_a_firearm_releases_its_nickname() {
    let db = TestDb::new();
    let first =
        ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "A1", "Old Faithful"), false)
            .unwrap();
    dispose(&db, first.id);

    let reused =
        ops::create_firearm(&db.conn, &nicknamed("Sig", "P226", "B2", "Old Faithful"), false);
    assert!(reused.is_ok(), "a disposed firearm's nickname can be reused");
    // The disposed record keeps its own nickname as history.
    assert_eq!(
        ops::get_firearm(&db.conn, first.id).unwrap().nickname.as_deref(),
        Some("Old Faithful")
    );
}

#[test]
fn the_nickname_is_searchable_like_any_other_field() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "A1", "Old Faithful"), false).unwrap();
    ops::create_firearm(&db.conn, &firearm("Sig", "P226", "B2"), false).unwrap();

    let found = ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("faithf".into()), ..Default::default() },
    )
    .unwrap();
    let hits: Vec<_> = found.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].make, "Glock");
}

#[test]
fn changing_a_nickname_updates_the_search_index() {
    let db = TestDb::new();
    let created =
        ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "A1", "Old Faithful"), false)
            .unwrap();
    ops::update_firearm(&db.conn, created.id, &nicknamed("Glock", "19", "A1", "Retired"), false)
        .unwrap();

    let search = |query: &str| {
        ops::list_firearms(
            &db.conn,
            &ListFirearmsInput { query: Some(query.into()), ..Default::default() },
        )
        .unwrap()
        .groups
        .iter()
        .map(|g| g.firearms.len())
        .sum::<usize>()
    };
    assert_eq!(search("faithful"), 0);
    assert_eq!(search("retired"), 1);
}

#[test]
fn the_database_itself_refuses_a_second_active_firearm_with_the_same_nickname() {
    let db = TestDb::new();
    let first =
        ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "A1", "Old Faithful"), false)
            .unwrap();
    let other = ops::create_firearm(&db.conn, &firearm("Sig", "P226", "B2"), false).unwrap();
    let rename = "UPDATE firearms SET nickname = 'OLD FAITHFUL' WHERE id = ?1";

    assert!(
        db.conn.execute(rename, [other.id]).is_err(),
        "the partial unique index is the backstop for the app-level check"
    );

    // Disposed rows are outside the index.
    db.conn.execute("UPDATE firearms SET status = 'disposed' WHERE id = ?1", [first.id]).unwrap();
    assert!(db.conn.execute(rename, [other.id]).is_ok());
}

// --- Export / import (FR-031) ---

fn import(db: &TestDb, rows: &[String]) -> hoplodex_lib::commands::import_export::ImportResult {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("import.csv");
    std::fs::write(&path, csv_file(rows)).unwrap();
    import_export_ops::import_collection(
        &db.conn,
        &path,
        SpreadsheetFormat::Csv,
        &ImportSessionStore::new(),
        &mut |_, _| {},
    )
    .unwrap()
}

#[test]
fn the_nickname_round_trips_through_export_and_import() {
    let source = TestDb::new();
    let created =
        ops::create_firearm(&source.conn, &nicknamed("Glock", "19", "A1", "Old Faithful"), false)
            .unwrap();
    let dest = TempDir::new().unwrap();
    let exported = import_export_ops::export_collection(
        &source.conn,
        dest.path(),
        "backup",
        SpreadsheetFormat::Csv,
        &[created.id],
        &mut |_, _| {},
    )
    .unwrap();
    let text = std::fs::read_to_string(&exported.spreadsheet_path).unwrap();
    let header = text.lines().next().unwrap();
    assert!(header.starts_with("make,model,nickname,"), "nickname follows model: {header}");
    assert!(text.contains("Old Faithful"));

    let target = TestDb::new();
    let result = import_export_ops::import_collection(
        &target.conn,
        &exported.spreadsheet_path,
        SpreadsheetFormat::Csv,
        &ImportSessionStore::new(),
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);

    let listing = ops::list_firearms(&target.conn, &ListFirearmsInput::default()).unwrap();
    let imported = &listing.groups[0].firearms[0];
    assert_eq!(imported.nickname.as_deref(), Some("Old Faithful"));
}

#[test]
fn a_blank_nickname_cell_imports_as_no_nickname() {
    let db = TestDb::new();
    let result = import(&db, &[csv_firearm("Glock", "19", "A1", &[("nickname", "  ")])]);

    assert_eq!(result.imported_count, 1);
    let listing = ops::list_firearms(&db.conn, &ListFirearmsInput::default()).unwrap();
    assert_eq!(listing.groups[0].firearms[0].nickname, None);
}

#[test]
fn a_duplicate_nickname_is_a_row_error() {
    let db = TestDb::new();
    ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "A1", "Old Faithful"), false).unwrap();

    let result = import(
        &db,
        &[
            csv_firearm("Sig", "P226", "B2", &[("nickname", "old faithful")]),
            csv_firearm("Ruger", "GP100", "C3", &[("nickname", "Revolver")]),
            // Clashes with the row imported just above it.
            csv_firearm("Colt", "Python", "D4", &[("nickname", "REVOLVER")]),
        ],
    );

    assert_eq!(result.imported_count, 1);
    let rows: Vec<_> = result.row_errors.iter().map(|e| e.row).collect();
    assert_eq!(rows, vec![1, 3]);
    assert!(result.row_errors[0].message.to_lowercase().contains("nickname"));
}

#[test]
fn nickname_plays_no_part_in_import_matching() {
    let db = TestDb::new();
    let existing =
        ops::create_firearm(&db.conn, &nicknamed("Glock", "19", "A1", "Old Faithful"), false)
            .unwrap();

    // Same make/model/serial, different nickname: still a conflict on the
    // identifying key (FR-030), not a new record.
    let result = import(&db, &[csv_firearm("Glock", "19", "A1", &[("nickname", "Other")])]);

    assert_eq!(result.imported_count, 0);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].existing_firearm_id, existing.id);
}
