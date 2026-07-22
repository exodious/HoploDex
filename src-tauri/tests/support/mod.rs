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

/// A tiny (20x20, solid red) but genuinely valid PNG, so
/// `services::photos`'s real image-decoding thumbnail generator has real
/// bytes to decode — no mocks, per the constitution.
pub fn sample_png_bytes() -> Vec<u8> {
    vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 20, 0, 0, 0, 20, 8,
        2, 0, 0, 0, 2, 235, 138, 90, 0, 0, 0, 26, 73, 68, 65, 84, 120, 218, 99, 248, 207, 192, 64,
        54, 98, 24, 213, 60, 170, 121, 84, 243, 168, 230, 129, 213, 12, 0, 49, 205, 142, 128, 132,
        11, 139, 140, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]
}
