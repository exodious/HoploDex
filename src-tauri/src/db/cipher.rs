//! The pinned SQLCipher settings of the database file format
//! (contracts/database-file.md, research.md §1, FR-011). Every connection
//! and every attached schema gets them straight after its key and before its
//! first read, so a different SQLCipher build default can never write a file
//! another platform cannot open.

use rusqlite::Connection;

/// `(pragma, value)` in the order they are applied. Changing any of them
/// makes every existing file unreadable (`tests/portability_test.rs`).
pub const CIPHER_SETTINGS: &[(&str, &str)] = &[
    ("cipher_page_size", "4096"),
    ("kdf_iter", "1000000"),
    ("cipher_kdf_algorithm", "PBKDF2_HMAC_SHA512"),
    ("cipher_hmac_algorithm", "HMAC_SHA512"),
    ("cipher_plaintext_header_size", "0"),
];

/// Applies [`CIPHER_SETTINGS`] to `schema` (`"main"` or an attached
/// schema's name). Call it after the key and before anything reads the
/// schema.
pub fn apply_cipher_settings(conn: &Connection, schema: &str) -> rusqlite::Result<()> {
    assert!(
        !schema.is_empty() && schema.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "schema names are our own identifiers"
    );
    // The values are the constants above, never input, so they are written
    // into the statement as SQLCipher documents them.
    let sql: String = CIPHER_SETTINGS
        .iter()
        .map(|(name, value)| format!("PRAGMA {schema}.{name} = {value};"))
        .collect();
    conn.execute_batch(&sql)
}

/// Stops SQLCipher writing to stderr (it logs every failed HMAC check, which
/// is every wrong passphrase) for the rest of the process. Release builds
/// only: debug builds keep the log (research.md §1).
pub fn silence_cipher_log() {
    #[cfg(not(debug_assertions))]
    {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let silenced = Connection::open_in_memory()
                .and_then(|conn| conn.execute_batch("PRAGMA cipher_log_level = NONE;"));
            if let Err(err) = silenced {
                log::warn!("could not silence the SQLCipher log: {err}");
            }
        });
    }
}
