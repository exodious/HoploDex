//! Each stored release's database upgrades to this build without losing a value
//! (009 research.md §12). The stored database is `tests/fixtures/releases/0.1.0.hoplodex`,
//! made by `examples/release_fixture.rs`.

use crate::support::TestDb;

use std::path::{Path, PathBuf};

use hoplodex_lib::db::cipher::CIPHER_SETTINGS;
use rusqlite::Connection;
use tempfile::TempDir;

/// The fixture's passphrase (`examples/release_fixture.rs`).
const FIXTURE_PASSPHRASE: &str = "release fixture 0.1.0";

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/releases/0.1.0.hoplodex")
}

/// A copy of the fixture in a temp directory, read with a raw keyed
/// connection: the key and the pinned cipher settings (whatever this build's
/// KDF, since the fixture was keyed by a shipped build), and no upgrade step
/// run. Read-only, so the copy is left exactly as it was.
fn raw_fixture() -> (TempDir, Connection) {
    let dir = TempDir::new().unwrap();
    let copy = dir.path().join("0.1.0.hoplodex");
    std::fs::copy(fixture_path(), &copy).expect("the stored 0.1.0 database");
    let conn = Connection::open_with_flags(
        &copy,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap();
    conn.pragma_update(None, "key", FIXTURE_PASSPHRASE).unwrap();
    for (name, value) in CIPHER_SETTINGS {
        conn.pragma_update(None, name, value).unwrap();
    }
    (dir, conn)
}

/// Every object in `sqlite_schema` as `type name: sql`, in a fixed order.
fn schema(conn: &Connection) -> Vec<String> {
    conn.prepare(
        "SELECT type || ' ' || name || ': ' || coalesce(sql, '') FROM sqlite_schema
         ORDER BY type, name",
    )
    .unwrap()
    .query_map([], |row| row.get(0))
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

fn migration_names(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT name FROM schema_migrations ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// 009 FR-023, research.md §12: until 0.1.0 is released, the layout still
/// changes in place (CLAUDE.md), so the stored 0.1.0 database has to be what
/// a new database is. From the release on it is never regenerated, and this
/// check stops applying to it.
#[test]
fn release_fixture_matches_layout() {
    let (_dir, fixture) = raw_fixture();
    let new = TestDb::new();

    let (stored, current) = (schema(&fixture), schema(&new.conn));
    let hint = "src-tauri/tests/fixtures/releases/0.1.0.hoplodex is out of step with the layout a \
                new database has (until 0.1.0 is released the layout changes in place); rerun \
                `cargo run --example release_fixture` and commit the result";
    assert!(stored.len() > 20, "the fixture's schema was not read");
    assert_eq!(stored, current, "{hint}: sqlite_schema differs");
    assert_eq!(
        migration_names(&fixture),
        migration_names(&new.conn),
        "{hint}: schema_migrations differs"
    );
}
