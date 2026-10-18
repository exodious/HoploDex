//! Regenerates `tests/fixtures/releases/0.1.0.hoplodex`, the database that
//! release 0.1.0 made, which every later build must upgrade without losing a
//! value (009 FR-023, SC-004, research.md §12). It holds the human-testing
//! seed's "Main collection" (every kind of record, link, setting and
//! attachment, with small images and no `--extra` firearms), a policy draft
//! kept as pending changes, a backup record and changes waiting for a backup,
//! keyed with the pinned KDF and [`PASSPHRASE`].
//!
//! Until 0.1.0 is released the layout still changes in place, and
//! `tests/release_upgrade_test.rs` fails when the fixture's layout differs from
//! a new database's. Regenerate it then, and commit the result:
//!
//! ```sh
//! cargo run --example release_fixture
//! ```
//!
//! From the release on it is never regenerated: its hash goes in
//! `src/db/migrations/released.txt` (009 research.md §11).
//!
//! It writes that one file inside this checkout (or at the path it is given,
//! which must not exist yet) and nothing else: no `machine.json`, no keyring
//! entry.

use std::path::PathBuf;

use hoplodex_lib::db;
use hoplodex_lib::services::machine_settings::MachineIdentity;
use hoplodex_lib::services::passphrase::Passphrase;

#[path = "support/main_collection.rs"]
mod main_collection;

use main_collection::{Images, must, seed};

/// The fixture's passphrase (a fixture, not a secret).
pub const PASSPHRASE: &str = "release fixture 0.1.0";

/// The fixture, relative to `src-tauri`.
pub const FIXTURE: &str = "tests/fixtures/releases/0.1.0.hoplodex";

fn main() {
    // A `test-kdf` build keys with a KDF no other build uses (db/cipher.rs).
    // `upgrade-test-steps` adds test-only upgrade steps (009 research.md §14).
    if cfg!(feature = "test-kdf") || cfg!(feature = "upgrade-test-steps") {
        eprintln!(
            "The release fixture must be built without the test-kdf and upgrade-test-steps features: no other build could open what it makes."
        );
        std::process::exit(1);
    }
    let mut args = std::env::args().skip(1);
    let given = args.next();
    if args.next().is_some() || matches!(given.as_deref(), Some("-h" | "--help")) {
        eprintln!("Usage: cargo run --example release_fixture [-- <path of the file to write>]");
        std::process::exit(2);
    }
    let path = given
        .as_ref()
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE), PathBuf::from);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).expect("the fixtures folder");
    }
    // The default path is regenerated; a path given is never overwritten.
    if given.is_some() {
        assert!(!path.exists(), "{} exists already", path.display());
    } else {
        for suffix in ["", "-journal"] {
            match std::fs::remove_file(format!("{}{suffix}", path.display())) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => panic!("could not remove the old fixture: {err}"),
            }
        }
    }

    let machine = MachineIdentity { id: "f".repeat(32), display_name: "Fixture maker".into() };
    let passphrase = Passphrase::from_input(PASSPHRASE.to_owned());
    let conn = db::create_database(&path, &passphrase, &machine).expect("creating the fixture");
    seed(&conn, 0, Images::Small);

    must(
        conn.execute(
            "UPDATE collection_settings SET backups_enabled = 1, backup_keep_count = 3,
                    idle_lock_enabled = 1, idle_lock_minutes = 15",
            [],
        ),
        "the settings",
    );
    // A policy form left half typed when the database was locked: pending
    // changes, which the upgrade must keep (009 FR-015). The form version is
    // `FORM_VERSION` in `src/features/insurance/InsurancePolicyForm.tsx`.
    must(
        conn.execute(
            "INSERT INTO pending_changes (id, kind, mode, target_id, label, form_version,
                                          values_json, saved_at)
             VALUES (1, 'policy', 'add', NULL, 'Range Rider (add)', 1, ?1, ?2)",
            rusqlite::params![
                serde_json::json!({
                    "name": "Range Rider",
                    "insuranceCompany": "Hartwell Mutual",
                    "notes": "Quote for the new rider; half typed"
                })
                .to_string(),
                db::now_utc()
            ],
        ),
        "the pending changes",
    );
    // Changes waiting for a backup, and the last backup's time, as a session
    // that made changes after its last backup leaves them; closed as a normal
    // close leaves it, so it opens anywhere without a take-over.
    must(
        conn.execute(
            "UPDATE app_state SET disk_encryption_note_dismissed = 1, changes_waiting = 1,
                    last_backup_at = ?1, open_machine_id = NULL, open_machine_name = NULL,
                    open_since = NULL",
            [db::now_utc()],
        ),
        "the backup record",
    );
    drop(conn);

    let size = std::fs::metadata(&path).expect("the fixture").len();
    println!("Wrote {} ({size} bytes)", path.display());
}
