//! The pinned SQLCipher settings of the database file format
//! (contracts/database-file.md, research.md §1, FR-011). Every connection
//! and every attached schema gets them straight after its key and before its
//! first read, so a different SQLCipher build default can never write a file
//! another platform cannot open.

use rusqlite::Connection;

// `test-kdf` makes every database a build keys unreadable to every other
// build, so it must never reach one that opens a user's files.
#[cfg(all(
    feature = "test-kdf",
    any(feature = "custom-protocol", feature = "e2e", not(debug_assertions))
))]
compile_error!(
    "the `test-kdf` feature is for `cargo test` builds only: never with `custom-protocol` or `e2e`, \
     and never without debug assertions (a release, E2E or shipped build)"
);

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
        .map(|&(name, value)| format!("PRAGMA {schema}.{name} = {};", applied(name, value)))
        .collect();
    conn.execute_batch(&sql)
}

/// The value [`apply_cipher_settings`] applies for a setting: the pinned one,
/// except `kdf_iter` in a `test-kdf` build ([`test_kdf`]).
#[cfg(not(feature = "test-kdf"))]
fn applied(_name: &str, value: &'static str) -> &'static str {
    value
}

#[cfg(feature = "test-kdf")]
fn applied(name: &str, value: &'static str) -> &'static str {
    if name == "kdf_iter" { test_kdf::kdf_iter(value) } else { value }
}

/// Test builds only (the `test-kdf` feature, #77 step 11): keys every
/// database with [`FAST_KDF_ITER`](test_kdf::FAST_KDF_ITER) PBKDF2 iterations
/// instead of the pinned 1,000,000, which took most of the Rust tests' CPU.
/// Every other setting, and every code path, is the same as the app's. A test
/// that needs the real KDF (the committed fixture in `portability_test`, the
/// pinned settings) calls [`use_real_kdf`](test_kdf::use_real_kdf) first; it
/// applies to the whole process, so such tests have a test program of their own.
#[cfg(feature = "test-kdf")]
pub mod test_kdf {
    use std::sync::OnceLock;

    /// The iterations a `test-kdf` build applies.
    pub const FAST_KDF_ITER: &str = "1000";

    /// Whether this process uses the pinned KDF, fixed by the first key.
    static REAL: OnceLock<bool> = OnceLock::new();

    /// Makes this process key with the pinned `kdf_iter`. Panics once a
    /// database has been keyed with the fast one, so a process never mixes
    /// the two.
    pub fn use_real_kdf() {
        assert!(
            *REAL.get_or_init(|| true),
            "use_real_kdf() after this process keyed a database with the test KDF"
        );
    }

    pub(super) fn kdf_iter(pinned: &'static str) -> &'static str {
        if *REAL.get_or_init(|| false) { pinned } else { FAST_KDF_ITER }
    }
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
