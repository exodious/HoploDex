//! Seeds a sandbox for human (non-automated) testing: realistic collections
//! to poke at, to judge the look and feel and to check the functions behave
//! as expected. See `scripts/human-testing.sh`, which runs this and then
//! launches the app against the result.
//!
//! Everything goes through the same `ops` layer the app's commands use, so
//! the data obeys every rule (uniqueness, coverage, dispositions) and never
//! needs a schema of its own to keep up to date.
//!
//! The sandbox (`--dir`) holds two databases, both protected by
//! [`PASSPHRASE`], under `<dir>/HoploDex/`, and the `machine.json` that lists
//! them under `<dir>/config/io.github.exodious.HoploDex/` (the app finds it through
//! `XDG_CONFIG_HOME=<dir>/config`):
//! - "Main collection": the full collection, with non-default backup and lock
//!   settings, backed up (two backups, yesterday's and today's, in
//!   `<dir>/Backups`) and closed cleanly;
//! - "Shared collection": a few firearms, changes waiting for a backup,
//!   pending changes from a locked edit, and left open by another computer
//!   ("Workshop PC"), so the take-over prompt shows.
//!
//! It writes only into a directory it created, checked by
//! `support/sandbox.rs`, never to the real data, config or documents
//! directory, and never touches the keyring (constitution 1.2.0).
//!
//! Usage: cargo run --example human_seed -- [--dir <path>] [--extra <n>] [--reset]
//!                                          [--pending-from <version>]
//!        cargo run --example human_seed -- --print-passphrase

use std::path::{Path, PathBuf};

use chrono::{Duration, Local};
use hoplodex_lib::APP_VERSION;
use hoplodex_lib::commands::firearms::ListFirearmsInput;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::commands::photos::ops as photo_ops;
use hoplodex_lib::db;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::backups::{
    self, BackupJob, BackupKind, resolve_folder as backup_folder,
};
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::spreadsheet::{ACCESSORY_COLUMNS, FIREARM_COLUMNS};
use rusqlite::Connection;

#[path = "support/main_collection.rs"]
mod main_collection;
#[path = "support/sandbox.rs"]
mod sandbox;

use main_collection::*;

/// The passphrase of both seeded databases. A test fixture, not a secret:
/// it is printed, and `scripts/human-testing.sh` and the E2E harness read it
/// through `--print-passphrase`.
pub const PASSPHRASE: &str = "human testing passphrase";

const APP_IDENTIFIER: &str = "io.github.exodious.HoploDex";
pub const MAIN_NAME: &str = "Main collection";
pub const SHARED_NAME: &str = "Shared collection";
/// The computer that left "Shared collection" open.
const OTHER_MACHINE_ID: &str = "0badc0de0badc0de0badc0de0badc0de";
const OTHER_MACHINE_NAME: &str = "Workshop PC";

/// The version of the firearm edit form's kept drafts that this build knows
/// (`FORM_VERSION` in `src/features/firearms/FirearmForm.tsx`): the seeded
/// pending changes carry it, so they can be resumed. Keep it in step.
const FIREARM_FORM_VERSION: i64 = 4;
/// The form version of pending changes kept by another version of HoploDex
/// (`--pending-from`): one this build doesn't know, so they can't be resumed.
const OTHER_FORM_VERSION: i64 = 3;

struct Args {
    dir: PathBuf,
    extra: usize,
    reset: bool,
    /// The version the shared collection's pending changes were kept by.
    pending_from: Option<String>,
}

fn parse_args() -> Args {
    let mut args = Args {
        dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(".human-testing"),
        extra: 0,
        reset: false,
        pending_from: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dir" => args.dir = it.next().unwrap_or_else(|| usage("--dir needs a path")).into(),
            "--extra" => {
                args.extra = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or_else(|| usage("--extra needs a number"));
            }
            "--reset" => args.reset = true,
            "--pending-from" => {
                args.pending_from = Some(
                    it.next()
                        .filter(|version| !version.is_empty())
                        .unwrap_or_else(|| usage("--pending-from needs a version")),
                );
            }
            "--print-passphrase" => {
                println!("{PASSPHRASE}");
                std::process::exit(0);
            }
            "-h" | "--help" => usage(""),
            other => usage(&format!("unknown argument {other}")),
        }
    }
    args
}

fn usage(problem: &str) -> ! {
    if !problem.is_empty() {
        eprintln!("error: {problem}\n");
    }
    eprintln!(
        "Usage: cargo run --example human_seed -- [options]\n\n\
         --dir <path>          the sandbox to seed (default: <repo>/.human-testing); it must be\n\
         \x20                     new, empty, or one this seed made\n\
         --extra <n>           also generate n plain firearms, to test scrolling and search\n\
         --reset               delete and recreate the seeded databases\n\
         --pending-from <ver>  the shared collection's pending changes were kept by HoploDex\n\
         \x20                     <ver> (an older form that can't be resumed); without it, by this build\n\
         --print-passphrase    print the seeded databases' passphrase and exit"
    );
    std::process::exit(if problem.is_empty() { 0 } else { 2 });
}

fn main() {
    // A `test-kdf` build keys with a KDF no other build uses (db/cipher.rs).
    // `upgrade-test-steps` adds test-only upgrade steps (009 research.md §14).
    if cfg!(feature = "test-kdf") || cfg!(feature = "upgrade-test-steps") {
        eprintln!(
            "The human-testing seed must be built without the test-kdf and upgrade-test-steps features: no other build could open what it makes."
        );
        std::process::exit(1);
    }
    let args = parse_args();
    if let Err(problem) = sandbox::check_sandbox(&args.dir, &|name| std::env::var_os(name)) {
        eprintln!("{problem}");
        std::process::exit(1);
    }

    let paths = SandboxPaths::new(&args.dir);
    if paths.main.exists() || paths.shared.exists() {
        if !args.reset {
            eprintln!(
                "{} is already seeded. Pass --reset to delete it and start over.",
                args.dir.display()
            );
            std::process::exit(1);
        }
        paths.remove();
    }

    seed_sandbox(&args.dir, args.extra, args.pending_from.as_deref());
    let samples = write_import_samples(&args.dir.join("import-samples"));

    let identity = must(MachineSettings::load(&paths.config), "machine.json").identity();
    let conn = must(
        db::open_database(&paths.main, &passphrase(), &identity, false)
            .map(db::Opened::expect_ready),
        "reopening the main database",
    );
    print_summary(&conn);
    println!("\nDatabases:      {}", paths.databases.display());
    println!("Passphrase:     {PASSPHRASE}");
    println!("Import samples: {}", samples.display());
    println!("\nLaunch the app against it with:\n  scripts/human-testing.sh");
}

fn passphrase() -> Passphrase {
    Passphrase::from_input(PASSPHRASE.to_owned())
}

/// Where the seed puts things inside the sandbox.
pub struct SandboxPaths {
    pub databases: PathBuf,
    pub main: PathBuf,
    pub shared: PathBuf,
    pub main_backups: PathBuf,
    pub config: PathBuf,
}

impl SandboxPaths {
    pub fn new(dir: &Path) -> Self {
        let databases = dir.join("HoploDex");
        Self {
            main: databases.join(format!("{MAIN_NAME}.hoplodex")),
            shared: databases.join(format!("{SHARED_NAME}.hoplodex")),
            databases,
            main_backups: dir.join("Backups"),
            config: dir.join("config").join(APP_IDENTIFIER),
        }
    }

    /// Deletes what an earlier seed made (inside the checked sandbox only).
    fn remove(&self) {
        for database in [&self.main, &self.shared] {
            for suffix in ["", "-journal"] {
                let _ = std::fs::remove_file(format!("{}{suffix}", database.display()));
            }
        }
        let _ = std::fs::remove_dir_all(&self.main_backups);
        let _ = std::fs::remove_dir_all(self.databases.join("HoploDex backups"));
        let _ = std::fs::remove_file(self.config.join("machine.json"));
    }
}

/// Seeds both databases and `machine.json` into `dir`, which the caller has
/// checked is a sandbox. The shared collection's pending changes were kept by
/// this build, or, with `pending_from`, by that version of HoploDex, with a
/// form version this build doesn't know (008 FR-015, FR-016).
pub fn seed_sandbox(dir: &Path, extra: usize, pending_from: Option<&str>) -> SandboxPaths {
    let paths = SandboxPaths::new(dir);
    must(std::fs::create_dir_all(&paths.databases), "the databases folder");
    let machine = must(MachineSettings::load(&paths.config), "machine.json");
    let identity = machine.identity();

    // "Shared collection": open on another computer, with changes waiting
    // and a locked edit kept as pending changes.
    let shared = must(
        db::create_database(&paths.shared, &passphrase(), &identity),
        "creating the shared database",
    );
    let shared_firearm = seed_shared(&shared);
    must(
        shared.execute_batch(
            "UPDATE collection_settings SET backups_enabled = 0, idle_lock_enabled = 0,
                                            lock_on_screen_lock = 1;",
        ),
        "the shared database's settings",
    );
    must(
        shared.execute(
            "INSERT INTO pending_changes (id, kind, mode, target_id, label, form_version,
                                          values_json, saved_at)
             VALUES (1, 'firearm', 'edit', ?1, 'Beretta 92FS — edit', ?2, ?3, ?4)",
            rusqlite::params![
                shared_firearm,
                if pending_from.is_some() { OTHER_FORM_VERSION } else { FIREARM_FORM_VERSION },
                serde_json::json!({
                    "make": "Beretta", "model": "92FS", "serialNumber": "BER92-0417",
                    "caliber": "9mm", "notes": "Swapped the grips for the walnut set; half typed"
                })
                .to_string(),
                db::now_utc()
            ],
        ),
        "the shared database's pending changes",
    );
    // The version that kept them: this build, which creating the database
    // recorded, or the one `--pending-from` names.
    if let Some(version) = pending_from {
        must(
            shared.execute("UPDATE app_state SET last_saved_version = ?1", [version]),
            "the version that kept the shared database's pending changes",
        );
    }
    must(
        shared.execute(
            "UPDATE app_state SET open_machine_id = ?1, open_machine_name = ?2, open_since = ?3",
            rusqlite::params![OTHER_MACHINE_ID, OTHER_MACHINE_NAME, db::now_utc()],
        ),
        "the shared database's open marker",
    );
    machine.touch_recent(
        &paths.shared,
        SHARED_NAME,
        &database_id(&shared),
        &backup_folder(&paths.shared, "default"),
    );
    drop(shared);

    // "Main collection": everything, custom settings, backed up and closed.
    let main = must(
        db::create_database(&paths.main, &passphrase(), &identity),
        "creating the main database",
    );
    seed(&main, extra, Images::Photos);
    must(
        main.execute(
            "UPDATE collection_settings SET backups_enabled = 1, backup_keep_count = 3,
                    backup_location = ?1, idle_lock_enabled = 1, idle_lock_minutes = 15",
            [paths.main_backups.to_string_lossy()],
        ),
        "the main database's settings",
    );
    // Two backups, as two days' closes would have made them, so the restore
    // dialog has a choice and a seeded backup carries the backup stamp, and a
    // third a minute later, made before an upgrade (009 FR-013), so the
    // dialog shows its "before upgrading" label and a backup opened directly
    // shows its note.
    let now = Local::now().fixed_offset();
    let backups_made = [
        (now - Duration::days(1), BackupKind::Regular),
        (now, BackupKind::Regular),
        (now + Duration::minutes(1), BackupKind::BeforeUpgrade { to_version: APP_VERSION }),
    ];
    for (made_at, kind) in backups_made {
        must(
            backups::make_backup(
                &machine,
                BackupJob {
                    conn: &main,
                    database_path: &paths.main,
                    name: MAIN_NAME,
                    database_id: &database_id(&main),
                    folder: &paths.main_backups,
                    make_folder: true,
                    now: made_at,
                    kind,
                    cancel: &|| false,
                    progress: &mut |_, _| {},
                },
            ),
            "a backup of the main database",
        );
    }
    must(
        main.execute(
            "UPDATE app_state SET disk_encryption_note_dismissed = 1, changes_waiting = 0,
                    last_backup_at = ?1, open_machine_id = NULL, open_machine_name = NULL,
                    open_since = NULL",
            [backups::utc_text(&now)],
        ),
        "the main database's backup record",
    );
    // Opened last, so the chooser selects it.
    machine.touch_recent(&paths.main, MAIN_NAME, &database_id(&main), &paths.main_backups);
    paths
}

fn database_id(conn: &Connection) -> String {
    must(conn.query_row("SELECT database_id FROM app_state", [], |row| row.get(0)), "a database id")
}

/// A few firearms of their own; returns the one the pending edit is for.
fn seed_shared(conn: &Connection) -> i64 {
    let add = |input: FirearmInput| {
        let label = format!("{} {}", input.make, input.model);
        must(firearm_ops::create_firearm(conn, &input, false, None), &label).id
    };
    let beretta = add(FirearmInput {
        estimated_value: Some(650),
        ..base("Beretta", "92FS", "BER92-0417", "9mm", HANDGUN)
    });
    // spec 008 US1: this photo, the first in this database, has the id 1 that
    // the main database's first photo (the Glock's thumbnail) has, with other
    // bytes, so a thumbnail kept across a switch shows the wrong image
    // (e2e/specs/session-isolation.e2e.ts).
    let (name, bytes, mime) = seed_photo!("beretta-92fs-atf.jpg");
    let photo =
        must(photo_ops::add_photo(conn, RecordRef::Firearm(beretta), &bytes, name, mime), name);
    assert_eq!(photo.id, 1, "the shared database's first photo must have id 1");
    add(FirearmInput {
        estimated_value: Some(1_100),
        notes: text("Kept at the workshop"),
        ..base("Tikka", "T3x Lite", "TK-558120", ".308 Win", RIFLE)
    });
    add(FirearmInput {
        estimated_value: Some(400),
        ..base("Mossberg", "500", "MOS-V441872", "12 ga", SHOTGUN)
    });
    beretta
}

/// Spreadsheets to try File > Import with, each shaped to hit a different
/// path: a clean import, conflicts with the seeded collection, and rows that
/// fail validation. Written next to the database, never imported here.
pub fn write_import_samples(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).expect("create the import samples directory");

    let row = |cells: &[(&str, &str)]| -> Vec<String> {
        for (name, _) in cells {
            assert!(FIREARM_COLUMNS.contains(name), "unknown spreadsheet column {name}");
        }
        FIREARM_COLUMNS
            .iter()
            .map(|column| {
                cells.iter().find(|(name, _)| name == column).map_or("", |(_, v)| *v).to_owned()
            })
            .collect()
    };
    let write = |name: &str, rows: Vec<Vec<String>>| {
        let mut writer = csv::Writer::from_path(dir.join(name)).expect("create an import sample");
        writer.write_record(FIREARM_COLUMNS).expect("write the header");
        for record in rows {
            writer.write_record(record).expect("write a row");
        }
        writer.flush().expect("flush an import sample");
    };
    // 004 FR-023: a sheet exported before
    // `cartridge` and `action_type` existed, so its header lacks both.
    let write_without = |name: &str, dropped: &[&str], rows: Vec<Vec<String>>| {
        let kept: Vec<usize> = (0..FIREARM_COLUMNS.len())
            .filter(|&index| !dropped.contains(&FIREARM_COLUMNS[index]))
            .collect();
        let mut writer = csv::Writer::from_path(dir.join(name)).expect("create an import sample");
        writer
            .write_record(kept.iter().map(|&index| FIREARM_COLUMNS[index]))
            .expect("write the header");
        for record in rows {
            writer.write_record(kept.iter().map(|&index| &record[index])).expect("write a row");
        }
        writer.flush().expect("flush an import sample");
    };

    write(
        "import-clean.csv",
        vec![
            row(&[
                ("make", "Beretta"),
                ("model", "A300 Ultima"),
                ("serial_number", "A300-1001"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "12 gauge"),
                ("firearm_type", "Shotgun"),
                ("estimated_value", "1100"),
                ("acquisition_date", "2024-09-14"),
                ("barrel_length_in", "28"),
                ("overall_length_in", "48.5"),
                ("weight_oz", "113.5"),
                ("capacity", "4"),
                ("finish", "Black anodized"),
                ("condition", "New in box"),
                ("origin", "Domestic"),
                ("year_of_manufacture", "2023"),
            ]),
            row(&[
                ("make", "Tikka"),
                ("model", "T3x Lite"),
                ("nickname", "Deer Rifle"),
                ("serial_number", "T3-55210"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "6.5 Creedmoor"),
                ("firearm_type", "Rifle"),
                ("estimated_value", "$1,050"),
                // 009 FR-006: another case and surrounding spaces still find
                // the policy (the import's comparison).
                ("insurance_policy_name", " vault SCHEDULE "),
                ("scheduled_coverage_amount", "1200"),
                // A zero fraction beyond the precision, and any letter case.
                ("barrel_length_in", "24.500"),
                ("capacity", "3"),
                ("condition", "like new"),
                ("origin", "Imported"),
                ("country_of_manufacture", "Finland"),
                ("importer_name", "Beretta USA"),
            ]),
            // specs/002-firearm-identification: an importer-assigned main
            // serial with the original maker's marks entered separately
            // (002 contracts/ui-identification.md §8 example 3).
            row(&[
                ("make", "Ridgeline Arms"),
                ("model", "Imported CZ 75"),
                ("serial_number", "RA-70099"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("estimated_value", "600"),
                ("origin", "Imported"),
                ("country_of_manufacture", "Czech Republic"),
                ("importer_name", "Global Arms Import Co."),
                ("original_make", "CZ"),
                ("original_model", "75"),
                ("original_serial_number", "CZ-33221"),
            ]),
            // A record that is already disposed of, with its acquisition details.
            row(&[
                ("make", "Winchester"),
                ("model", "Model 94"),
                ("serial_number", "W94-88231"),
                ("no_serial_attested", "FALSE"),
                ("caliber", ".30-30 Win"),
                ("firearm_type", "Rifle"),
                ("accessories", "Leather sling, original box"),
                ("status", "disposed"),
                ("acquisition_source", "Estate sale"),
                ("acquisition_date", "2016-05-21"),
                ("acquisition_price", "450"),
                ("disposition_type", "sold"),
                ("disposition_recipient", "Ridgeline Arms"),
                ("disposition_date", "2024-11-02"),
                ("disposition_price", "700"),
            ]),
            row(&[
                ("make", "Homebuilt"),
                ("model", "Flintlock Pistol Kit"),
                ("no_serial_attested", "TRUE"),
                ("caliber", ".50 black powder"),
                ("firearm_type", "Other"),
                ("estimated_value", "180"),
                ("notes", "Kit build; no serial number."),
            ]),
        ],
    );

    write(
        "import-conflicts.csv",
        vec![
            // Same make, model and serial as the seeded Glock, with a new value and note.
            row(&[
                ("make", "Glock"),
                ("model", "19 Gen5"),
                ("serial_number", "BXKT482"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("estimated_value", "600"),
                ("notes", "Updated by import."),
                ("finish", "Cerakote"),
            ]),
            // Matches the seeded Ruger 10/22 ignoring letter case.
            row(&[
                ("make", "RUGER"),
                ("model", "10/22 TAKEDOWN"),
                ("serial_number", "0012-34567"),
                ("no_serial_attested", "FALSE"),
                ("caliber", ".22 LR"),
                ("firearm_type", "Rifle"),
                ("estimated_value", "400"),
            ]),
            // Matches a disposed record only: still a conflict, but one where
            // "import as a duplicate" is allowed alongside overwrite and skip.
            row(&[
                ("make", "Kel-Tec"),
                ("model", "P32"),
                ("serial_number", "R3K123"),
                ("no_serial_attested", "FALSE"),
                ("caliber", ".32 ACP"),
                ("firearm_type", "Handgun"),
                ("estimated_value", "260"),
            ]),
            // No conflict at all.
            row(&[
                ("make", "Walther"),
                ("model", "PPK/S"),
                ("serial_number", "WA-20214"),
                ("no_serial_attested", "FALSE"),
                ("caliber", ".380 ACP"),
                ("firearm_type", "Handgun"),
                ("estimated_value", "750"),
            ]),
        ],
    );

    let tomorrow = (Local::now().date_naive() + Duration::days(1)).format("%Y-%m-%d").to_string();
    write(
        "import-errors.csv",
        vec![
            row(&[
                ("model", "No Make Given"),
                ("serial_number", "E-001"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
            ]),
            row(&[
                ("make", "Future"),
                ("model", "Acquisition"),
                ("serial_number", "E-002"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("acquisition_date", tomorrow.as_str()),
            ]),
            row(&[
                ("make", "Fractional"),
                ("model", "Price"),
                ("serial_number", "E-003"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("estimated_value", "500.50"),
            ]),
            // "Daily" is already an active firearm's nickname.
            row(&[
                ("make", "Nickname"),
                ("model", "Clash"),
                ("nickname", "Daily"),
                ("serial_number", "E-004"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
            ]),
            row(&[
                ("make", "Unknown"),
                ("model", "Type"),
                ("serial_number", "E-005"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Trebuchet"),
            ]),
            row(&[
                ("make", "Both"),
                ("model", "Serial And Attested"),
                ("serial_number", "E-006"),
                ("no_serial_attested", "TRUE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
            ]),
            row(&[
                ("make", "Text"),
                ("model", "Not A Number"),
                ("serial_number", "E-008"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("barrel_length_in", "sixteen"),
                ("weight_oz", "heavy"),
            ]),
            row(&[
                ("make", "Empty"),
                ("model", "Magazine"),
                ("serial_number", "E-009"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("capacity", "0"),
            ]),
            row(&[
                ("make", "Unknown"),
                ("model", "Condition"),
                ("serial_number", "E-010"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("condition", "Mint"),
            ]),
            row(&[
                ("make", "Good"),
                ("model", "Row Among The Bad"),
                ("serial_number", "E-007"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("estimated_value", "400"),
            ]),
            // 002 US4-3: not one of the three
            // accepted spellings (no `Reimported` alias).
            row(&[
                ("make", "Bad"),
                ("model", "Origin Spelling"),
                ("serial_number", "E-011"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("origin", "Reimported"),
            ]),
            // 002 US4-7: a year later than the current local year.
            row(&[
                ("make", "Future"),
                ("model", "Manufacture Year"),
                ("serial_number", "E-012"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("origin", "Domestic"),
                (
                    "year_of_manufacture",
                    &(Local::now().date_naive().format("%Y").to_string().parse::<i64>().unwrap()
                        + 1)
                    .to_string(),
                ),
            ]),
            // 002 US4-4: country of manufacture is never allowed on a
            // re-imported row (it is displayed as the United States, never
            // stored).
            row(&[
                ("make", "Bad"),
                ("model", "Reimported Country"),
                ("serial_number", "E-013"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("origin", "Re-imported"),
                ("country_of_manufacture", "Germany"),
            ]),
            // 002 US4-6: original marks matching the seeded warning-demo pair
            // (Ridgeline Arms Imported Hi-Power A/B) — imports, but appears
            // under the import report's Warnings, not as a row error.
            row(&[
                ("make", "Century Arms"),
                ("model", "Hi-Power Clone"),
                ("serial_number", "E-014"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("origin", "Imported"),
                ("original_make", "Fabrique Nationale"),
                ("original_model", "High Power"),
                ("original_serial_number", "FN-70044"),
            ]),
        ],
    );

    // 004 US4: the report's "Calibers filled in
    // from the cartridge" and "Spellings matched to existing values"
    // sections, and the action and entry-rule row errors. Import it into the
    // seeded collection, which has "Smith & Wesson" on several firearms.
    write(
        "import-cartridges.csv",
        vec![
            // A blank caliber beside a catalog cartridge: filled in as 9mm
            // (built-in).
            row(&[
                ("make", "Sig Sauer"),
                ("model", "P365"),
                ("serial_number", "C-001"),
                ("no_serial_attested", "FALSE"),
                ("cartridge", "9x19mm Parabellum"),
                ("action_type", "Semi-automatic"),
                ("firearm_type", "Handgun"),
            ]),
            // A custom cartridge whose bore can be guessed: filled in as .30
            // (guessed).
            row(&[
                ("make", "Thompson/Center"),
                ("model", "Contender"),
                ("serial_number", "C-002"),
                ("no_serial_attested", "FALSE"),
                ("cartridge", ".30 Custom Improved"),
                ("action_type", "Single shot (other)"),
                ("firearm_type", "Rifle"),
            ]),
            // A same-notation variant of a make on record, and of the
            // catalog's cartridge name: both are matched to the spelling in
            // use and listed. The action is matched ignoring letter case.
            row(&[
                ("make", "smith and wesson"),
                ("model", "Model 15"),
                ("serial_number", "C-003"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("cartridge", "9X19mm PARABELLUM"),
                ("action_type", "REVOLVER"),
                ("firearm_type", "Handgun"),
            ]),
            // A custom cartridge with no readable bore and no caliber: a
            // row error.
            row(&[
                ("make", "Wildcat Arms"),
                ("model", "One-Off"),
                ("serial_number", "C-004"),
                ("no_serial_attested", "FALSE"),
                ("cartridge", "Wildcat Special"),
                ("firearm_type", "Rifle"),
            ]),
            // An action that isn't on the list: a row error.
            row(&[
                ("make", "Pedersoli"),
                ("model", "Kentucky"),
                ("serial_number", "C-005"),
                ("no_serial_attested", "FALSE"),
                ("caliber", ".45"),
                ("action_type", "flintlockish"),
                ("firearm_type", "Rifle"),
            ]),
            // An action the firearm's type doesn't have: a row error.
            row(&[
                ("make", "Mossberg"),
                ("model", "Handgun Pump"),
                ("serial_number", "C-006"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "12 gauge"),
                ("action_type", "Pump action"),
                ("firearm_type", "Handgun"),
            ]),
        ],
    );

    // A sheet exported before this feature: no `cartridge` or `action_type`
    // column, and every later column still lands in its own field.
    write_without(
        "import-before-cartridges.csv",
        &["cartridge", "action_type"],
        vec![row(&[
            ("make", "Ruger"),
            ("model", "10/22"),
            ("serial_number", "L-001"),
            ("no_serial_attested", "FALSE"),
            ("caliber", ".22 LR"),
            ("firearm_type", "Rifle"),
            ("notes", "From a sheet exported before cartridges and actions were recorded."),
        ])],
    );

    // 005 US4: a round-trip sheet, as export
    // writes it: a suppressor and two registered firearms, one of them with a
    // same-notation variant of a form on record ("FORM 4") that import
    // matches and lists under "Spellings matched to existing values".
    write(
        "import-registrations.csv",
        vec![
            row(&[
                ("make", "Dead Air"),
                ("model", "Sandman-S"),
                ("serial_number", "R-001"),
                ("no_serial_attested", "FALSE"),
                ("caliber", ".30"),
                ("firearm_type", "Suppressor"),
                ("registered_as", "Suppressor"),
                ("registration_form", "Form 4"),
                ("registration_approved", "2024-03-05"),
                ("registered_to", "Sean Example"),
            ]),
            row(&[
                ("make", "Ruger"),
                ("model", "Mini-14 Tactical"),
                ("serial_number", "R-002"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "5.56mm"),
                ("firearm_type", "Rifle"),
                ("registered_as", "short-barreled rifle"),
                ("registration_form", "FORM 1"),
                ("registered_to", "Example Arms Trust"),
            ]),
            row(&[
                ("make", "Colt"),
                ("model", "M16A1"),
                ("serial_number", "R-003"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "5.56mm"),
                ("action_type", "Automatic or select-fire"),
                ("firearm_type", "Rifle"),
                ("registered_as", "Machine gun"),
                ("registration_form", "Form 4"),
                ("registration_approved", "1985-06-01"),
            ]),
        ],
    );

    // Row errors: an unknown classification, details with no classification,
    // a future approved date, a Suppressor row with an action, a barrel
    // length and a capacity (all three reported on the one row), and a
    // Suppressor row with a rated cartridge and a blank caliber, which isn't
    // worked out from the cartridge (005 research.md §15).
    write(
        "import-registration-errors.csv",
        vec![
            row(&[
                ("make", "Wildcat Arms"),
                ("model", "Short Barrel"),
                ("serial_number", "E-001"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "12 gauge"),
                ("firearm_type", "Shotgun"),
                ("registered_as", "Short barrel shotgun"),
            ]),
            row(&[
                ("make", "Wildcat Arms"),
                ("model", "No Classification"),
                ("serial_number", "E-002"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("registration_form", "Form 4"),
                ("registered_to", "Sean Example"),
            ]),
            row(&[
                ("make", "Wildcat Arms"),
                ("model", "Future Approval"),
                ("serial_number", "E-003"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("firearm_type", "Handgun"),
                ("registered_as", "Suppressor"),
                ("registration_approved", "2999-01-01"),
            ]),
            row(&[
                ("make", "Wildcat Arms"),
                ("model", "Silencer With Action"),
                ("serial_number", "E-004"),
                ("no_serial_attested", "FALSE"),
                ("caliber", "9mm"),
                ("action_type", "Semi-automatic"),
                ("barrel_length_in", "4"),
                ("capacity", "10"),
                ("firearm_type", "Suppressor"),
                ("registered_as", "Suppressor"),
            ]),
            row(&[
                ("make", "Wildcat Arms"),
                ("model", "Rated Only"),
                ("serial_number", "E-005"),
                ("no_serial_attested", "FALSE"),
                ("caliber", ""),
                ("cartridge", ".300 Winchester Magnum"),
                ("firearm_type", "Suppressor"),
            ]),
        ],
    );

    // A sheet exported before this feature: none of the four registration
    // columns, and the row imports with no classification.
    write_without(
        "import-before-registrations.csv",
        &["registered_as", "registration_form", "registration_approved", "registered_to"],
        vec![row(&[
            ("make", "Savage"),
            ("model", "Model 110"),
            ("serial_number", "P-001"),
            ("no_serial_attested", "FALSE"),
            ("caliber", ".308"),
            ("firearm_type", "Rifle"),
            ("notes", "From a sheet exported before registrations were recorded."),
        ])],
    );

    write_accessory_import_samples(dir, &row);
    write_over_limit_import_sample(dir);

    // Issue #56: a row matching the seeded "Stripped lower" by make, model
    // and serial number marks it sold, so choosing "Replace existing" asks
    // which of the records mounted on it (an upper carrying a scope, which
    // carries a red dot, and a light) go with it.
    write(
        "import-dispose-receiver.csv",
        vec![row(&[
            ("make", "LaRue Tactical"),
            ("model", "PredatAR lower"),
            ("nickname", "Stripped lower"),
            ("serial_number", "LT-L-30915"),
            ("no_serial_attested", "FALSE"),
            ("caliber", ".223"),
            ("firearm_type", "Rifle"),
            ("status", "disposed"),
            ("disposition_type", "sold"),
            ("disposition_recipient", "Kestrel Outfitters"),
            ("disposition_date", "2025-09-12"),
            ("disposition_price", "400"),
        ])],
    );

    // 006 US5-7: a sheet exported before this feature
    // has neither `record_id` nor `mounted_on`, and imports as it always did.
    write_without(
        "import-before-accessories.csv",
        &["record_id", "mounted_on"],
        vec![row(&[
            ("make", "Remington"),
            ("model", "870 Express"),
            ("serial_number", "A-001"),
            ("no_serial_attested", "FALSE"),
            ("caliber", "12 gauge"),
            ("firearm_type", "Shotgun"),
            ("notes", "From a sheet exported before accessories were recorded."),
        ])],
    );

    dir.to_path_buf()
}

/// 008 FR-006: a workbook the import refuses as a whole,
/// to see the refusal in the dialog's banner. `import-refused-too-many-sheets.xlsx`
/// has 20 sheets, each with `x` in A1, where an import takes at most 16
/// (`import_reader::limits::MAX_SHEETS`); nothing in it is read as a record.
fn write_over_limit_import_sample(dir: &Path) {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    for number in 1..=20 {
        let sheet = workbook.add_worksheet();
        sheet.set_name(format!("Over the limit {number}")).expect("name a sheet");
        sheet.write_string(0, 0, "x").expect("write a cell");
    }
    workbook
        .save(dir.join("import-refused-too-many-sheets.xlsx"))
        .expect("save the over-limit sample workbook");
}

/// Builds a sample row from named cells.
type RowBuilder = dyn Fn(&[(&str, &str)]) -> Vec<String>;

/// Record identifiers the accessory samples name each other by. They are
/// fixed so a sample's rows can be mounted on one another; they exist in no
/// seeded database, so no sample matches a seeded record by identifier.
mod sample_ids {
    pub const RIFLE: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c01";
    pub const SUPPRESSOR: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c02";
    pub const OPTIC: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c11";
    pub const MAGAZINE: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c12";
    pub const BARREL: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c13";
    pub const SLING: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c14";
    pub const UPPER: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c15";
    pub const RAIL: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c16";
    /// Held by no record anywhere.
    pub const NOBODY: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7cff";
    pub const WARN_A: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c21";
    pub const WARN_DISPOSED: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c22";
    pub const WARN_LOOP_1: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c23";
    pub const WARN_LOOP_2: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c24";
    pub const WARN_ITEM: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c25";
    pub const ERROR_TWICE: &str = "7f3c9a10-2b4d-4e6a-8c1f-5d0b3a9e7c31";
}

/// 006 US5: the accessory table and the record
/// identifier and mount columns of both tables.
/// - `import-mounts-firearms.csv` + `import-accessories.csv`: pick the two
///   files together. A rifle, an optic on it, a suppressor on an upper, and
///   more; every accessory column but `photo_filenames` is filled.
/// - `import-collection.xlsx`: both tables as the two sheets of one workbook.
/// - `import-accessory-mount-warnings.csv`: each of the four mount warnings
///   and a disposed record that can't be mounted.
/// - `import-accessory-errors.csv`: accessory row errors.
fn write_accessory_import_samples(dir: &Path, firearm_row: &RowBuilder) {
    use sample_ids as id;

    let row = |cells: &[(&str, &str)]| -> Vec<String> {
        for (name, _) in cells {
            assert!(ACCESSORY_COLUMNS.contains(name), "unknown spreadsheet column {name}");
        }
        ACCESSORY_COLUMNS
            .iter()
            .map(|column| {
                cells.iter().find(|(name, _)| name == column).map_or("", |(_, v)| *v).to_owned()
            })
            .collect()
    };
    let write = |name: &str, columns: &[&str], rows: &[Vec<String>]| {
        let mut writer = csv::Writer::from_path(dir.join(name)).expect("create an import sample");
        writer.write_record(columns).expect("write the header");
        for record in rows {
            writer.write_record(record).expect("write a row");
        }
        writer.flush().expect("flush an import sample");
    };

    let firearms = vec![
        firearm_row(&[
            ("record_id", id::RIFLE),
            ("make", "Daniel Defense"),
            ("model", "DDM4 V7"),
            ("serial_number", "DD-70011"),
            ("no_serial_attested", "FALSE"),
            ("caliber", "5.56mm"),
            ("firearm_type", "Rifle"),
            ("estimated_value", "1800"),
        ]),
        // A firearm on an accessory (an upper) of the accessory file.
        firearm_row(&[
            ("record_id", id::SUPPRESSOR),
            ("make", "Dead Air"),
            ("model", "Wolfman"),
            ("serial_number", "DA-60021"),
            ("no_serial_attested", "FALSE"),
            ("caliber", ".30"),
            ("firearm_type", "Suppressor"),
            ("estimated_value", "950"),
            ("mounted_on", id::UPPER),
        ]),
    ];
    write("import-mounts-firearms.csv", FIREARM_COLUMNS, &firearms);

    let accessories = vec![
        // Every column but photo_filenames, on an optic mounted on the rifle.
        row(&[
            ("record_id", id::OPTIC),
            ("kind", "Optic"),
            ("make", "Leupold"),
            ("model", "VX-5HD"),
            ("serial_number", "OP-5521"),
            ("caliber", ".308 Winchester"),
            ("cartridge", ".308 Winchester"),
            ("notes", "Mounted for the match season."),
            ("status", "active"),
            ("estimated_value", "1100"),
            ("acquisition_source", "Online"),
            ("acquisition_date", "2023-05-01"),
            ("acquisition_price", "1000"),
            ("insurance_policy_name", "Vault Schedule"),
            ("scheduled_coverage_amount", "900"),
            ("mounted_on", id::RIFLE),
        ]),
        // A blank caliber, worked out from the cartridge and listed in the
        // report.
        row(&[
            ("record_id", id::MAGAZINE),
            ("kind", "Magazine"),
            ("make", "Magpul"),
            ("model", "PMAG"),
            ("cartridge", "9x19mm Parabellum"),
            ("estimated_value", "20"),
        ]),
        row(&[
            ("record_id", id::BARREL),
            ("kind", "Barrel"),
            ("make", "Criterion"),
            ("model", "Hybrid barrel"),
        ]),
        // A disposed accessory, with its disposition.
        row(&[
            ("record_id", id::SLING),
            ("kind", "Sling"),
            ("make", "Blue Force Gear"),
            ("model", "Vickers sling"),
            ("status", "disposed"),
            ("disposition_type", "sold"),
            ("disposition_recipient", "Sam Example"),
            ("disposition_date", "2025-07-01"),
            ("disposition_price", "15"),
        ]),
        row(&[
            ("record_id", id::UPPER),
            ("kind", "Upper receiver"),
            ("make", "Daniel Defense"),
            ("model", "DDM4 Upper"),
            ("estimated_value", "700"),
        ]),
        // An accessory on an accessory.
        row(&[
            ("record_id", id::RAIL),
            ("kind", "Mount or rail"),
            ("make", "ADM"),
            ("model", "AD-RECON mount"),
            ("mounted_on", id::OPTIC),
        ]),
        // No identifier: gets a new one. The kind is matched ignoring case.
        row(&[("kind", "case"), ("make", "Pelican"), ("model", "1750")]),
    ];
    write("import-accessories.csv", ACCESSORY_COLUMNS, &accessories);

    // Both tables in one workbook (006 FR-022), the accessory sheet mounted on
    // the firearm sheet and the reverse.
    let mut workbook = rust_xlsxwriter::Workbook::new();
    for (name, columns, rows) in
        [("Firearms", FIREARM_COLUMNS, &firearms), ("Accessories", ACCESSORY_COLUMNS, &accessories)]
    {
        let sheet = workbook.add_worksheet();
        sheet.set_name(name).expect("name a sheet");
        for (c, header) in columns.iter().enumerate() {
            sheet.write_string(0, c as u16, *header).expect("write a header");
        }
        for (r, record) in rows.iter().enumerate() {
            for (c, cell) in record.iter().enumerate() {
                sheet.write_string((r + 1) as u32, c as u16, cell).expect("write a cell");
            }
        }
    }
    workbook.save(dir.join("import-collection.xlsx")).expect("save the sample workbook");

    // Each mount warning, in the report's Warnings (006 FR-023). Every row
    // imports, unmounted.
    let disposed = [
        ("status", "disposed"),
        ("disposition_type", "gifted"),
        ("disposition_recipient", "Sam Example"),
        ("disposition_date", "2025-07-01"),
    ];
    let mut host_gone = vec![
        ("record_id", id::WARN_DISPOSED),
        ("kind", "Sling"),
        ("make", "Magpul"),
        ("model", "MS1 sling"),
    ];
    host_gone.extend_from_slice(&disposed);
    let mut item_gone = vec![
        ("record_id", id::WARN_ITEM),
        ("kind", "Optic"),
        ("make", "Burris"),
        ("model", "Fullfield IV"),
    ];
    item_gone.extend_from_slice(&disposed);
    item_gone.push(("mounted_on", id::WARN_A));
    write(
        "import-accessory-mount-warnings.csv",
        ACCESSORY_COLUMNS,
        &[
            // No record has this identifier.
            row(&[
                ("record_id", id::WARN_A),
                ("kind", "Optic"),
                ("make", "Nikon"),
                ("model", "Monarch 3"),
                ("mounted_on", id::NOBODY),
            ]),
            row(&host_gone),
            // Its host is disposed.
            row(&[
                ("kind", "Optic"),
                ("make", "Bushnell"),
                ("model", "Elite 4500"),
                ("mounted_on", id::WARN_DISPOSED),
            ]),
            // Not an identifier at all.
            row(&[
                ("kind", "Optic"),
                ("make", "Sig Sauer"),
                ("model", "Romeo5"),
                ("mounted_on", "not-a-record-id"),
            ]),
            // Two rows mounted on each other: the later is left unmounted.
            row(&[
                ("record_id", id::WARN_LOOP_1),
                ("kind", "Light or laser"),
                ("make", "Streamlight"),
                ("model", "TLR-1"),
                ("mounted_on", id::WARN_LOOP_2),
            ]),
            row(&[
                ("record_id", id::WARN_LOOP_2),
                ("kind", "Light or laser"),
                ("make", "Crimson Trace"),
                ("model", "LiNQ"),
                ("mounted_on", id::WARN_LOOP_1),
            ]),
            // A disposed record is never mounted.
            row(&item_gone),
        ],
    );

    write(
        "import-accessory-errors.csv",
        ACCESSORY_COLUMNS,
        &[
            row(&[("make", "No Kind Given"), ("model", "X")]),
            row(&[("kind", "Optic"), ("model", "No Make Given")]),
            row(&[("kind", "Optic"), ("make", "No Model Given")]),
            row(&[("kind", "Frobnicator"), ("make", "Unknown Kind"), ("model", "X")]),
            row(&[
                ("kind", "Optic"),
                ("make", "Fractional"),
                ("model", "X"),
                ("estimated_value", "12.50"),
            ]),
            row(&[
                ("kind", "Optic"),
                ("make", "Bad Date"),
                ("model", "X"),
                ("acquisition_date", "next week"),
            ]),
            row(&[
                ("record_id", "xyz"),
                ("kind", "Optic"),
                ("make", "Bad Record ID"),
                ("model", "X"),
            ]),
            row(&[
                ("record_id", id::ERROR_TWICE),
                ("kind", "Optic"),
                ("make", "First Use"),
                ("model", "X"),
            ]),
            row(&[
                ("record_id", id::ERROR_TWICE),
                ("kind", "Optic"),
                ("make", "Second Use"),
                ("model", "X"),
            ]),
            row(&[
                ("kind", "Optic"),
                ("make", "Disposed Without Details"),
                ("model", "X"),
                ("status", "disposed"),
            ]),
            row(&[("kind", "Optic"), ("make", "Good Row Among The Bad"), ("model", "X")]),
        ],
    );
}

// ---------------------------------------------------------------------------
// Read-back summary
// ---------------------------------------------------------------------------

/// Reads the result back through the same queries the app's views use, so
/// what is printed is what the app will show.
fn print_summary(conn: &Connection) {
    let listing = must(
        firearm_ops::list_firearms(
            conn,
            &ListFirearmsInput { include_disposed: true, ..Default::default() },
        ),
        "the read-back listing",
    );
    let all: Vec<_> = listing.groups.iter().flat_map(|g| &g.firearms).collect();
    let active = all.iter().filter(|f| f.status == FirearmStatus::Active).count();
    println!("Seeded {} firearms ({} active, {} disposed)", all.len(), active, all.len() - active);

    let mut by_warning = std::collections::BTreeMap::<String, Vec<String>>::new();
    for f in all.iter().filter(|f| f.status == FirearmStatus::Active) {
        let key = format!("{:?}", f.insurance_warning);
        by_warning.entry(key).or_default().push(format!("{} {}", f.make, f.model));
    }
    for (warning, names) in &by_warning {
        let shown: Vec<_> = names.iter().take(4).cloned().collect();
        let more = names.len().saturating_sub(shown.len());
        let tail = if more > 0 { format!(", +{more} more") } else { String::new() };
        println!("  {warning:<13} {:>3}  {}{tail}", names.len(), shown.join("; "));
    }

    let value = must(insurance_ops::get_value_summary(conn), "the value summary");
    println!("Collection value: ${}", value.collection_total);
    if let Some(blanket) = value.blanket {
        println!(
            "Blanket ({}): ${} of ${} limit across {} firearms, ${} of headroom",
            blanket.policy_name,
            blanket.total,
            blanket.limit,
            blanket.firearm_count,
            blanket.limit - blanket.total
        );
    }
}
