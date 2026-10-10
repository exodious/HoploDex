//! Issue #64: an import's unresolved conflicts hold the file's plaintext rows
//! and the open database's record ids, so they are kept with the open that
//! read them and resolve nowhere else. Two real databases in one `Session`,
//! the commands' own session-level `ops`.

mod sheets;
mod support;

use std::path::PathBuf;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::{
    ConflictResolution, ImportResult, MAX_HELD_IMPORTS, ops as import_ops,
};
use hoplodex_lib::db;
use hoplodex_lib::session::{OpenDatabase, Session};
use support::{TestDb, csv_file, csv_firearm, firearm, passphrase, test_machine};
use tempfile::TempDir;

/// A session with a database open, and where its file is.
struct World {
    session: Session,
    path: PathBuf,
    /// Keeps the database's folder.
    _dir: TempDir,
}

fn open_with(db: TestDb) -> World {
    let path = db.path();
    let (conn, dir) = db.into_parts();
    let session = Session::default();
    session.install(OpenDatabase::new(conn, &path).unwrap());
    World { session, path, _dir: dir }
}

/// Makes `path` the session's open database the way a lock and an unlock
/// or a switch do: the open one is taken out and dropped first.
fn reopen(session: &Session, path: &std::path::Path) {
    drop(session.take());
    let conn = db::open_database(path, &passphrase(), &test_machine(), false).unwrap();
    session.install(OpenDatabase::new(conn, path).unwrap());
}

fn make(session: &Session, make: &str, model: &str, serial: &str, notes: &str) -> i64 {
    let mut input = firearm(make, model, serial);
    input.notes = Some(notes.into());
    session.write(|conn| firearm_ops::create_firearm(conn, &input, false, None)).unwrap().id
}

fn notes_of(session: &Session, id: i64) -> Option<String> {
    session.read(|conn| firearm_ops::get_firearm(conn, id)).unwrap().notes
}

fn count(session: &Session) -> i64 {
    session
        .read(|conn| {
            conn.query_row("SELECT count(*) FROM firearms", [], |row| row.get::<_, i64>(0))
                .map_err(hoplodex_lib::commands::CommandError::from_db)
        })
        .unwrap()
}

/// Imports a row matching Glock 19 ABC123 with "imported" notes, so it
/// conflicts with that firearm.
fn import_conflict(session: &Session, dir: &TempDir) -> ImportResult {
    let path = dir.path().join("import.csv");
    std::fs::write(
        &path,
        csv_file(&[csv_firearm("Glock", "19", "ABC123", &[("notes", "imported")])]),
    )
    .unwrap();
    let result = import_ops::import_in_session(
        session,
        &[sheets::import_file(&path)],
        &mut |_, _| {},
        &|| false,
        &|_| {},
    )
    .unwrap();
    assert_eq!(result.conflicts.len(), 1);
    result
}

fn resolve(
    session: &Session,
    import: &ImportResult,
    action: &str,
) -> Result<
    hoplodex_lib::commands::import_export::ResolveResult,
    hoplodex_lib::commands::CommandError,
> {
    import_ops::resolve_in_session(
        session,
        &import.session_id,
        &[ConflictResolution {
            conflict_id: import.conflicts[0].conflict_id.clone(),
            action: action.into(),
            with_mounted: Vec::new(),
        }],
        None,
    )
}

#[test]
fn the_conflicts_of_one_database_resolve_in_it() {
    let a = open_with(TestDb::new());
    let record = make(&a.session, "Glock", "19", "ABC123", "original");
    let scratch = TempDir::new().unwrap();
    let import = import_conflict(&a.session, &scratch);

    let resolved = resolve(&a.session, &import, "overwrite").unwrap();

    assert_eq!(resolved.resolved_count, 1);
    assert_eq!(notes_of(&a.session, record).as_deref(), Some("imported"));
    // Resolved, so gone: it can't be applied a second time.
    assert_eq!(resolve(&a.session, &import, "overwrite").unwrap_err().code, "NOT_FOUND");
}

#[test]
fn conflicts_from_database_a_cannot_be_applied_to_database_b() {
    let a = open_with(TestDb::new());
    make(&a.session, "Glock", "19", "ABC123", "original");
    let scratch = TempDir::new().unwrap();
    let import = import_conflict(&a.session, &scratch);

    // Switch: B is opened in A's place. B's first firearm has the same
    // numeric id as the one the conflict matched in A.
    let b = TestDb::new();
    let b_path = b.path();
    let (b_conn, _b_dir) = b.into_parts();
    drop(a.session.take());
    a.session.install(OpenDatabase::new(b_conn, &b_path).unwrap());
    let b_record = make(&a.session, "Colt", "Python", "P1", "b's own");
    assert_eq!(count(&a.session), 1);

    for action in ["overwrite", "duplicate"] {
        let refused = resolve(&a.session, &import, action).unwrap_err();
        assert_eq!(refused.code, "NOT_FOUND", "{action}");
    }

    assert_eq!(notes_of(&a.session, b_record).as_deref(), Some("b's own"));
    assert_eq!(count(&a.session), 1, "nothing of A was inserted into B");
}

#[test]
fn conflicts_do_not_survive_a_lock_and_reopen_of_the_same_database() {
    let a = open_with(TestDb::new());
    let record = make(&a.session, "Glock", "19", "ABC123", "original");
    let scratch = TempDir::new().unwrap();
    let import = import_conflict(&a.session, &scratch);
    assert_eq!(a.session.inspect(|open| Ok(open.imports.len())).unwrap(), 1);

    reopen(&a.session, &a.path);

    assert_eq!(a.session.inspect(|open| Ok(open.imports.len())).unwrap(), 0);
    assert_eq!(resolve(&a.session, &import, "overwrite").unwrap_err().code, "NOT_FOUND");
    assert_eq!(notes_of(&a.session, record).as_deref(), Some("original"));
    assert_eq!(count(&a.session), 1);
}

/// The firearm a conflict matched is deleted and another takes its numeric
/// id; `action` on the conflict must leave that other record, and the
/// collection, alone.
fn a_replaced_record_is_not_touched(action: &str) {
    let a = open_with(TestDb::new());
    let original = make(&a.session, "Glock", "19", "ABC123", "original");
    let scratch = TempDir::new().unwrap();
    let import = import_conflict(&a.session, &scratch);

    a.session.write(|conn| firearm_ops::delete_firearm(conn, original, true)).unwrap();
    let replacement = make(&a.session, "Colt", "Python", "P1", "the replacement");
    assert_eq!(replacement, original, "the new record reuses the numeric id");

    let resolved = resolve(&a.session, &import, action).unwrap();

    assert_eq!(resolved.resolved_count, 0);
    assert_eq!(resolved.unresolved.len(), 1);
    assert_eq!(notes_of(&a.session, replacement).as_deref(), Some("the replacement"));
    assert_eq!(count(&a.session), 1);
}

#[test]
fn an_overwrite_is_refused_when_the_matched_record_was_replaced() {
    a_replaced_record_is_not_touched("overwrite");
}

#[test]
fn a_duplicate_is_refused_when_the_matched_record_was_replaced() {
    a_replaced_record_is_not_touched("duplicate");
}

#[test]
fn only_the_newest_few_imports_are_held() {
    let a = open_with(TestDb::new());
    make(&a.session, "Glock", "19", "ABC123", "original");
    let scratch = TempDir::new().unwrap();

    let mut imports = Vec::new();
    for _ in 0..=MAX_HELD_IMPORTS {
        imports.push(import_conflict(&a.session, &scratch));
        // The session id counts microseconds.
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    assert_eq!(a.session.inspect(|open| Ok(open.imports.len())).unwrap(), MAX_HELD_IMPORTS);
    let oldest = resolve(&a.session, &imports[0], "skip").unwrap_err();
    assert_eq!(oldest.code, "NOT_FOUND");
    let newest = resolve(&a.session, imports.last().unwrap(), "skip").unwrap();
    assert_eq!(newest.resolved_count, 1);
}
