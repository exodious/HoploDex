use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

const KEYRING_SERVICE: &str = "com.hoplodex.app";
const KEYRING_USER: &str = "sqlcipher-key";
const DB_FILE_NAME: &str = "hoplodex.db";

/// Versioned SQL migrations, applied in order. Names are stored in
/// `schema_migrations` so a given database is never migrated twice.
const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_initial", include_str!("migrations/0001_initial.sql")),
    ("0002_fts5", include_str!("migrations/0002_fts5.sql")),
    ("0003_seed_firearm_types", include_str!("migrations/0003_seed_firearm_types.sql")),
];

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("keyring error: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not resolve app data directory: {0}")]
    Path(#[from] tauri::Error),
    #[error("could not generate encryption key: {0}")]
    Random(#[from] getrandom::Error),
}

/// Shared, mutex-guarded connection managed as Tauri state. A single-user
/// desktop app has no need for a connection pool (Principle I: no
/// speculative abstraction).
pub struct DbHandle(pub Mutex<Connection>);

/// Generates a random 256-bit key and renders it as lowercase hex, suitable
/// for SQLCipher's raw-key `PRAGMA key = "x'<hex>'"` form (skips the
/// PBKDF2 passphrase derivation, appropriate for a machine-generated key
/// rather than a user-memorized password).
pub fn generate_key_hex() -> Result<String, DbError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Reads the SQLCipher passphrase from the OS-native credential store,
/// generating and persisting one on first run (research.md §5).
#[cfg(not(feature = "mock-keyring"))]
pub fn get_or_create_passphrase() -> Result<String, DbError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)?;
    match entry.get_password() {
        Ok(existing) => Ok(existing),
        Err(keyring::Error::NoEntry) => {
            let key_hex = generate_key_hex()?;
            entry.set_password(&key_hex)?;
            Ok(key_hex)
        }
        Err(other) => Err(other.into()),
    }
}

/// E2E-only variant of [`get_or_create_passphrase`]. Headless/CI
/// environments have no way to unlock the real platform credential store
/// (a Secret Service passphrase prompt has no one to answer it), so E2E
/// builds (`--features mock-keyring`) use keyring-core's in-memory mock
/// store instead — same first-run-generates-a-key behavior, just not
/// backed by the OS.
///
/// The mock store lives in memory, so every launch gets a new key and can't
/// open a database made by another process. `HOPLODEX_E2E_DB_KEY` (64 hex
/// digits) fixes the key instead, so the E2E screenshot pass can seed a
/// collection with `examples/human_seed.rs` and then open it in the app.
#[cfg(feature = "mock-keyring")]
pub fn get_or_create_passphrase() -> Result<String, DbError> {
    if let Ok(key_hex) = std::env::var("HOPLODEX_E2E_DB_KEY") {
        assert!(
            key_hex.len() == 64 && key_hex.chars().all(|c| c.is_ascii_hexdigit()),
            "HOPLODEX_E2E_DB_KEY must be 64 hex digits"
        );
        return Ok(key_hex.to_ascii_lowercase());
    }
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        keyring_core::set_default_store(
            keyring_core::mock::Store::new().expect("mock keyring store"),
        );
    });
    let entry = keyring_core::Entry::new(KEYRING_SERVICE, KEYRING_USER)?;
    match entry.get_password() {
        Ok(existing) => Ok(existing),
        Err(keyring::Error::NoEntry) => {
            let key_hex = generate_key_hex()?;
            entry.set_password(&key_hex)?;
            Ok(key_hex)
        }
        Err(other) => Err(other.into()),
    }
}

/// Opens (creating if absent) a SQLCipher database at `path`, unlocks it
/// with the given raw hex key, enables foreign-key enforcement, and applies
/// any pending migrations. Used directly by integration tests against a
/// real temporary database (no mocks, per the constitution) as well as by
/// [`init_app_db`] in the running app.
pub fn open_encrypted(path: &Path, key_hex: &str) -> Result<Connection, DbError> {
    debug_assert!(
        key_hex.len() == 64 && key_hex.chars().all(|c| c.is_ascii_hexdigit()),
        "key must be 32 raw bytes as lowercase hex, to safely inline into the PRAGMA below"
    );
    let conn = Connection::open(path)?;
    // SQLCipher's raw-key form requires the double-quoted `x'<hex>'` blob
    // syntax exactly as written here; rusqlite's `pragma_update` would
    // single-quote-escape a string value instead, which SQLCipher does not
    // recognize as raw-key syntax. Safe to inline directly: key_hex is
    // always our own generated/validated hex, never external input.
    conn.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))?;
    // Confirms the key is correct: SQLCipher only surfaces a bad key as an
    // error on the first real read against the database.
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))?;
    // Constitution V: deleted content is overwritten with zeros where it
    // lay, not just unlinked. Stated here rather than left to the SQLCipher
    // build's compile-time default, so it holds for every connection.
    conn.pragma_update(None, "secure_delete", "ON")?;
    apply_migrations(&conn)?;
    Ok(conn)
}

/// Gives the space a delete freed back to the filesystem by rebuilding the
/// file (`VACUUM`), so a deleted photo or document leaves neither its bytes
/// nor an oversized file behind (Constitution V). Call after the `DELETE`
/// has committed: `VACUUM` cannot run inside a transaction.
///
/// The delete itself has already succeeded and `secure_delete` has already
/// zeroed the content, so a failure here is logged and not reported as a
/// failed delete.
pub fn reclaim_freed_space(conn: &Connection) {
    if let Err(err) = conn.execute_batch("VACUUM") {
        log::warn!("could not vacuum the database after a delete: {err}");
    }
}

fn apply_migrations(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            name TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL
        )",
    )?;

    for (name, sql) in MIGRATIONS {
        let already_applied: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE name = ?1)",
            [name],
            |row| row.get(0),
        )?;
        if already_applied {
            continue;
        }

        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (name, applied_at) VALUES (?1, datetime('now'))",
            [name],
        )?;
        tx.commit()?;
    }

    Ok(())
}

/// Resolves the OS app-data directory, gets/creates the encryption key via
/// the OS keyring, and opens the app's single encrypted database file.
pub fn init_app_db(app: &tauri::AppHandle) -> Result<Connection, DbError> {
    use tauri::Manager;

    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join(DB_FILE_NAME);
    let key_hex = get_or_create_passphrase()?;
    open_encrypted(&db_path, &key_hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_migrates_and_reopens_a_real_temp_database() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let key_hex = generate_key_hex().unwrap();

        let conn = open_encrypted(&db_path, &key_hex).unwrap();
        let type_count: i64 =
            conn.query_row("SELECT count(*) FROM firearm_types", [], |r| r.get(0)).unwrap();
        assert_eq!(type_count, 4, "seed migration should insert 4 firearm types");
        drop(conn);

        // Reopening with the same key must succeed and must not re-apply
        // (and thus fail to re-insert unique-constrained) migrations.
        let conn2 = open_encrypted(&db_path, &key_hex).unwrap();
        let type_count2: i64 =
            conn2.query_row("SELECT count(*) FROM firearm_types", [], |r| r.get(0)).unwrap();
        assert_eq!(type_count2, 4);
    }

    #[test]
    fn wrong_key_on_reopen_fails() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let key_hex = generate_key_hex().unwrap();
        open_encrypted(&db_path, &key_hex).unwrap();

        let wrong_key = generate_key_hex().unwrap();
        assert!(open_encrypted(&db_path, &wrong_key).is_err());
    }
}
