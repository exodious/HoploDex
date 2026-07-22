use hoplodex_lib::db;
use rusqlite::Connection;
use tempfile::TempDir;

/// A real, migrated, encrypted SQLCipher database in a temp directory —
/// never a mock connection, per the constitution's Testing Standards
/// principle. `_dir` is held only to keep the temp directory alive for the
/// lifetime of the returned handle.
pub struct TestDb {
    pub conn: Connection,
    _dir: TempDir,
}

impl TestDb {
    pub fn new() -> Self {
        let dir = TempDir::new().expect("failed to create temp dir for test DB");
        let db_path = dir.path().join("test.db");
        let key_hex = db::generate_key_hex().expect("failed to generate test DB key");
        let conn =
            db::open_encrypted(&db_path, &key_hex).expect("failed to open encrypted test DB");
        Self { conn, _dir: dir }
    }
}

impl Default for TestDb {
    fn default() -> Self {
        Self::new()
    }
}
