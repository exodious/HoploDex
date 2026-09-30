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
//!        cargo run --example human_seed -- --print-passphrase

use std::io::Cursor;
use std::path::{Path, PathBuf};

use chrono::{Duration, Local};
use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::firearms::{
    DisposeFirearmInput, HistoryChoice, ListFirearmsInput, ReverseDispositionInput,
};
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::commands::photos::ops as photo_ops;
use hoplodex_lib::db;
use hoplodex_lib::models::firearm::{
    Condition, DispositionType, FirearmInput, FirearmStatus, Origin,
};
use hoplodex_lib::models::insurance_policy::InsurancePolicyInput;
use hoplodex_lib::services::backups::{self, BackupJob, resolve_folder as backup_folder};
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::spreadsheet::COLUMNS;
use rusqlite::Connection;

#[path = "support/sandbox.rs"]
mod sandbox;

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

// Ids seeded by migration 0003_seed_firearm_types.
const HANDGUN: i64 = 1;
const RIFLE: i64 = 2;
const SHOTGUN: i64 = 3;
const OTHER: i64 = 4;
const SUPPRESSOR: i64 = 5;
// Ids of the registration classifications seeded by 0003_seed_firearm_types.
const REGISTERED_SUPPRESSOR: i64 = 1;
const REGISTERED_SBR: i64 = 2;
const REGISTERED_SBS: i64 = 3;
const REGISTERED_MACHINE_GUN: i64 = 5;
/// 005 US3: "Automatic or select-fire", unrelated to any classification.
const ACTION_AUTOMATIC: i64 = 13;

// Ids seeded by migration 0003_seed_firearm_types (specs/004-cartridges-action-types FR-018).
const SEMI_AUTOMATIC: i64 = 1;
const REVOLVER: i64 = 2;
const BOLT_ACTION: i64 = 3;
const LEVER_ACTION: i64 = 4;
const PUMP_ACTION: i64 = 5;
const PERCUSSION: i64 = 11;

/// A real photo from `seed-photos/`, whose README records where each came
/// from and its licence, as the `(name, bytes, mime type)` that `photos`
/// takes.
macro_rules! seed_photo {
    ($name:literal) => {
        ($name, include_bytes!(concat!("seed-photos/", $name)).to_vec(), "image/jpeg")
    };
}

struct Args {
    dir: PathBuf,
    extra: usize,
    reset: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(".human-testing"),
        extra: 0,
        reset: false,
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
         --print-passphrase    print the seeded databases' passphrase and exit"
    );
    std::process::exit(if problem.is_empty() { 0 } else { 2 });
}

fn main() {
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

    seed_sandbox(&args.dir, args.extra);
    let samples = write_import_samples(&args.dir.join("import-samples"));

    let identity = must(MachineSettings::load(&paths.config), "machine.json").identity();
    let conn = must(
        db::open_database(&paths.main, &passphrase(), &identity, false),
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
/// checked is a sandbox.
pub fn seed_sandbox(dir: &Path, extra: usize) -> SandboxPaths {
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
             VALUES (1, 'firearm', 'edit', ?1, 'Beretta 92FS — edit', 1, ?2, ?3)",
            rusqlite::params![
                shared_firearm,
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
    seed(&main, extra);
    must(
        main.execute(
            "UPDATE collection_settings SET backups_enabled = 1, backup_keep_count = 3,
                    backup_location = ?1, idle_lock_enabled = 1, idle_lock_minutes = 15",
            [paths.main_backups.to_string_lossy()],
        ),
        "the main database's settings",
    );
    // Two backups, as two days' closes would have made them, so the restore
    // dialog has a choice and a seeded backup carries the backup stamp.
    let now = Local::now().fixed_offset();
    for made_at in [now - Duration::days(1), now] {
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
        must(firearm_ops::create_firearm(conn, &input, false), &label).id
    };
    let beretta = add(FirearmInput {
        estimated_value: Some(650),
        ..base("Beretta", "92FS", "BER92-0417", "9mm", HANDGUN)
    });
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

// ---------------------------------------------------------------------------
// Seed data
// ---------------------------------------------------------------------------

/// Fails loudly: a seed that half-applies would give misleading test data.
fn must<T, E: std::fmt::Debug>(result: Result<T, E>, what: &str) -> T {
    result.unwrap_or_else(|err| panic!("seeding {what} failed: {err:?}"))
}

fn text(value: &str) -> Option<String> {
    Some(value.to_owned())
}

/// A valid active firearm with only the required fields; records add what
/// they exercise with struct-update syntax.
fn base(make: &str, model: &str, serial: &str, caliber: &str, type_id: i64) -> FirearmInput {
    FirearmInput {
        make: make.into(),
        model: model.into(),
        serial_number: text(serial),
        no_serial_attested: false,
        caliber: caliber.into(),
        firearm_type_id: type_id,
        nickname: None,
        notes: None,
        accessories: None,
        barrel_length_hundredths: None,
        overall_length_hundredths: None,
        weight_tenths_oz: None,
        capacity: None,
        finish: None,
        condition: None,
        status: FirearmStatus::Active,
        estimated_value: None,
        acquisition_source: None,
        acquisition_date: None,
        acquisition_price: None,
        disposition_type: None,
        disposition_recipient: None,
        disposition_date: None,
        disposition_price: None,
        insurance_policy_id: None,
        scheduled_coverage_amount: None,
        origin: None,
        year_of_manufacture: None,
        country_of_manufacture: None,
        importer_name: None,
        original_make: None,
        original_model: None,
        original_serial_number: None,
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        cartridge: None,
        action_type_id: None,
    }
}

fn day(offset_from_today: i64) -> String {
    (Local::now().date_naive() + Duration::days(offset_from_today)).format("%Y-%m-%d").to_string()
}

struct Policies {
    collector: i64,
    expired_rider: i64,
    vault: i64,
}

/// Policy dates are relative to today so every insurance state is on show
/// whenever the database is seeded: in force, expiring soon, expired, and
/// an expired blanket policy that a renewal has replaced.
fn seed_policies(conn: &Connection) -> Policies {
    let policy = |name: &str,
                  number: &str,
                  company: &str,
                  limit: Option<i64>,
                  start: i64,
                  end: i64,
                  notes: Option<&str>| {
        InsurancePolicyInput {
            name: name.into(),
            policy_number: number.into(),
            insurance_company: company.into(),
            company_contact: text("1-800-555-0142"),
            agent_name: text("Pat Alvarez"),
            agent_contact: text("pat.alvarez@example.com"),
            notes: notes.map(str::to_owned),
            blanket_coverage_limit: limit,
            effective_start_date: day(start),
            effective_end_date: day(end),
        }
    };
    let create = |input: InsurancePolicyInput| {
        must(insurance_ops::create_policy(conn, &input), &input.name.clone()).id
    };

    // The renewal starts the day after the old blanket policy ends, so the
    // old one is expired-but-replaced and raises no warning (FR-028).
    create(policy(
        "Homeowners Blanket 2025",
        "HB-2025-88120",
        "Cascade Mutual",
        Some(10_000),
        -530,
        -166,
        None,
    ));
    Policies {
        collector: create(policy(
            "Collector Schedule",
            "CS-771204",
            "Heritage Fine Arts",
            None,
            -335,
            20,
            Some(
                "Renewal quote requested.\nAppraisals for the scheduled firearms are in the safe.",
            ),
        )),
        expired_rider: create(policy(
            "Range Bag Rider",
            "RB-30918",
            "Cascade Mutual",
            None,
            -700,
            -40,
            None,
        )),
        vault: create(policy(
            "Vault Schedule",
            "VS-559031",
            "Heritage Fine Arts",
            None,
            -100,
            265,
            None,
        )),
    }
}

/// The blanket policy in force today. The caller sizes `limit` so the
/// unscheduled firearms sit comfortably under it whatever `--extra` adds:
/// the app starts with a healthy blanket that one more firearm can tip over.
fn seed_blanket(conn: &Connection, limit: i64) {
    let input = InsurancePolicyInput {
        name: "Homeowners Blanket 2026".into(),
        policy_number: "HB-2026-90311".into(),
        insurance_company: "Cascade Mutual".into(),
        company_contact: text("1-800-555-0142"),
        agent_name: text("Pat Alvarez"),
        agent_contact: text("pat.alvarez@example.com"),
        notes: text(
            "Covers every firearm not scheduled individually.\nClaims line: 1-800-555-0142.",
        ),
        blanket_coverage_limit: Some(limit),
        effective_start_date: day(-165),
        effective_end_date: day(200),
    };
    must(insurance_ops::create_policy(conn, &input), "the current blanket policy");
}

/// `pub` so `tests/human_seed_coverage_test.rs` can run it.
pub fn seed(conn: &Connection, extra: usize) {
    let policies = seed_policies(conn);
    let extras = generated_firearms(extra);
    let extras_value: i64 = extras.iter().filter_map(|f| f.estimated_value).sum();
    seed_blanket(conn, 12_000 + extras_value);

    let add = |input: FirearmInput| {
        let label = format!("{} {}", input.make, input.model);
        must(firearm_ops::create_firearm(conn, &input, false), &label).id
    };
    let dispose = |id: i64, kind: DispositionType, recipient: &str, date: &str, price: i64| {
        let input = DisposeFirearmInput {
            disposition_type: kind,
            recipient: recipient.into(),
            date: date.into(),
            price,
        };
        must(firearm_ops::dispose_firearm(conn, id, &input), "a disposition");
    };
    let photos = |firearm_id: i64, images: &[(&str, Vec<u8>, &str)]| -> Vec<i64> {
        images
            .iter()
            .map(|(name, bytes, mime)| {
                must(photo_ops::add_photo(conn, firearm_id, bytes, name, mime), name).id
            })
            .collect()
    };
    let documents = |firearm_id: i64, files: &[(&str, Vec<u8>, &str)]| {
        for (name, bytes, mime) in files {
            must(document_ops::add_document(conn, firearm_id, bytes, name, mime), name);
        }
    };

    // -- Active, unscheduled (covered by the blanket policy) ---------------

    let glock = add(FirearmInput {
        nickname: text("Daily"),
        notes: text(
            "Trigger job done by a gunsmith in 2022. Fires reliably with 115 gr FMJ and 124 gr JHP.",
        ),
        accessories: text("Three 15-round magazines, Streamlight TLR-7 light, Kydex holster"),
        estimated_value: Some(550),
        acquisition_source: text("Ridgeline Arms"),
        acquisition_date: text("2021-03-14"),
        acquisition_price: Some(529),
        barrel_length_hundredths: Some(402),
        overall_length_hundredths: Some(740),
        weight_tenths_oz: Some(236),
        capacity: Some(15),
        finish: text("Black nDLC"),
        condition: Some(Condition::Excellent),
        // specs/004-cartridges-action-types: a built-in cartridge and its
        // bore class.
        cartridge: text("9x19mm Parabellum"),
        // specs/004-cartridges-action-types US3: an action allowed for each
        // seeded type (Handgun, Rifle, Shotgun, Other), below.
        action_type_id: Some(SEMI_AUTOMATIC),
        ..base("Glock", "19 Gen5", "BXKT482", "9mm", HANDGUN)
    });
    photos(
        glock,
        &[
            seed_photo!("glock-19-gen3.jpg"),
            seed_photo!("glock-19-gen4-fde.jpg"),
            seed_photo!("glock-19-atf.jpg"),
        ],
    );
    documents(
        glock,
        &[
            (
                "receipt-ridgeline-arms.pdf",
                simple_pdf(&["Ridgeline Arms", "Sales receipt", "Glock 19 Gen5 - 529.00"]),
                "application/pdf",
            ),
            (
                "owners-manual-notes.txt",
                b"Field strip: clear, lock slide back, pull down the takedown tabs.\n".to_vec(),
                "text/plain",
            ),
        ],
    );

    // Deliberately no photos: shows the generic rifle thumbnail.
    add(FirearmInput {
        nickname: text("Plinker"),
        estimated_value: Some(380),
        acquisition_source: text("Gun show"),
        acquisition_date: text("2018-09-02"),
        acquisition_price: Some(340),
        barrel_length_hundredths: Some(1620),
        overall_length_hundredths: Some(3700),
        weight_tenths_oz: Some(736),
        capacity: Some(10),
        finish: text("Matte black"),
        condition: Some(Condition::Good),
        ..base("Ruger", "10/22 Takedown", "0012-34567", ".22 LR", RIFLE)
    });

    add(FirearmInput {
        notes: text(
            "Old duty gun; the bluing is worn at the muzzle and the forend has a hairline crack near the tang. Keep an eye on it. Bring the original barrel-length paperwork when transferring.",
        ),
        accessories: text(
            "Extra 20-inch barrel, Vang Comp magazine tube extension, sling swivels, 4-shell side saddle, Limbsaver recoil pad, spare bead sight, cleaning kit",
        ),
        estimated_value: Some(650),
        acquisition_date: text("2012-11-23"),
        barrel_length_hundredths: Some(2800),
        overall_length_hundredths: Some(4850),
        weight_tenths_oz: Some(1160),
        capacity: Some(4),
        finish: text("Blued, walnut stock"),
        condition: Some(Condition::Good),
        ..base("Remington", "870 Wingmaster", "RS12345M", "12 gauge", SHOTGUN)
    });

    let benelli = add(FirearmInput {
        nickname: text("Home Defense"),
        estimated_value: Some(1_900),
        acquisition_source: text("Ridgeline Arms"),
        acquisition_date: text("2020-02-08"),
        acquisition_price: Some(1_749),
        barrel_length_hundredths: Some(1850),
        overall_length_hundredths: Some(4000),
        weight_tenths_oz: Some(1248),
        capacity: Some(5),
        finish: text("Matte black"),
        condition: Some(Condition::LikeNew),
        ..base("Benelli", "M4 Super 90", "M123456", "12 gauge", SHOTGUN)
    });
    photos(benelli, &[seed_photo!("benelli-m4.jpg")]);

    // Same identity as a disposed record below: a firearm reacquired as a
    // new record (FR-032 only protects active records).
    let disposed_p365 = add(FirearmInput {
        nickname: text("Carry"),
        estimated_value: Some(600),
        acquisition_date: text("2022-01-15"),
        ..base("Sig Sauer", "P365 XL", "66A123456", "9mm", HANDGUN)
    });
    dispose(disposed_p365, DispositionType::Sold, "Dana Whitfield", "2023-04-02", 520);
    add(FirearmInput {
        nickname: text("Carry"),
        notes: text("Bought back from the same friend I sold it to."),
        estimated_value: Some(600),
        acquisition_source: text("Dana Whitfield"),
        acquisition_date: text("2025-10-11"),
        acquisition_price: Some(500),
        finish: text("Stainless slide, black frame"),
        condition: Some(Condition::NewInBox),
        ..base("Sig Sauer", "P365 XL", "66A123456", "9mm", HANDGUN)
    });

    let garand = add(FirearmInput {
        notes: text("Garand thumb is not a myth."),
        estimated_value: Some(1_600),
        acquisition_source: text("Civilian Marksmanship Program"),
        acquisition_date: text("2016-05-19"),
        acquisition_price: Some(1_050),
        barrel_length_hundredths: Some(2400),
        overall_length_hundredths: Some(4360),
        weight_tenths_oz: Some(1520),
        capacity: Some(8),
        finish: text("Parkerized"),
        condition: Some(Condition::Fair),
        ..base("Springfield Armory", "M1 Garand", "1234567", ".30-06", RIFLE)
    });
    photos(
        garand,
        &[
            seed_photo!("m1-garand-left.jpg"),
            seed_photo!("m1-garand-right.jpg"),
            seed_photo!("m1-garand-receiver.jpg"),
            seed_photo!("m1-garand-stock.jpg"),
            seed_photo!("m1-garand-sling.jpg"),
        ],
    );

    add(FirearmInput {
        no_serial_attested: true,
        serial_number: None,
        nickname: text("Project Gun"),
        notes: text("Built from an 80% lower; no serial number, attested."),
        estimated_value: Some(700),
        acquisition_date: text("2023-06-10"),
        ..base("Homebuilt", "AR-15 80% Lower Build", "", "5.56 NATO", RIFLE)
    });

    let remington_replica = add(FirearmInput {
        no_serial_attested: true,
        serial_number: None,
        notes: text("Black powder; no serial number."),
        estimated_value: Some(250),
        acquisition_date: text("2019-08-03"),
        barrel_length_hundredths: Some(800),
        overall_length_hundredths: Some(1300),
        weight_tenths_oz: Some(400),
        capacity: Some(6),
        finish: text("Blued"),
        condition: Some(Condition::Poor),
        action_type_id: Some(PERCUSSION),
        ..base("Pedersoli", "1858 Remington Replica", "", ".44 black powder", OTHER)
    });
    photos(remington_replica, &[seed_photo!("remington-new-model-army.jpg")]);

    // Only the required fields: no value, so no insurance warning either.
    add(FirearmInput {
        action_type_id: Some(PUMP_ACTION),
        ..base("Mossberg", "500", "V0123456", "12 gauge", SHOTGUN)
    });

    // specs/004-cartridges-action-types: a muzzleloader records only its
    // caliber (spec Edge Cases), and "7.62x39" finds the SKS (US1-9). No
    // value, so the blanket's totals are unchanged.
    add(FirearmInput {
        notes: text("Percussion cap; patched round ball over 90 gr FFg."),
        action_type_id: Some(PERCUSSION),
        ..base("Thompson/Center", "Hawken", "TC-50H-1182", ".50", RIFLE)
    });
    add(FirearmInput {
        cartridge: text("7.62x39mm"),
        ..base("Norinco", "SKS", "NOR-2419907", ".30", RIFLE)
    });

    add(FirearmInput {
        nickname: text("Ünïcödé Tëst"),
        notes: text("Accented text: crème brûlée, façade, señor, Zażółć gęślą jaźń."),
        estimated_value: Some(700),
        acquisition_date: text("2024-03-30"),
        ..base("Česká zbrojovka", "CZ 75 B", "ČZ-00042", "9mm", HANDGUN)
    });

    add(FirearmInput {
        estimated_value: Some(1_000),
        acquisition_date: text("2017-12-01"),
        action_type_id: Some(LEVER_ACTION),
        ..base("Henry", "Big Boy", "BB0123456", ".44 Magnum", RIFLE)
    });

    add(FirearmInput {
        nickname: text("Backup"),
        estimated_value: Some(300),
        acquisition_date: text("2024-08-17"),
        ..base("Taurus", "G3C", "ABC12345", "9mm", HANDGUN)
    });

    // Disposed and reacquired twice, keeping both past dispositions
    // (FR-033): shows the disposition history on the record.
    let p320 = add(FirearmInput {
        nickname: text("Reacquired"),
        estimated_value: Some(500),
        acquisition_date: text("2019-04-20"),
        ..base("Sig Sauer", "P320", "58B123456", "9mm", HANDGUN)
    });
    photos(p320, &[seed_photo!("sig-p320-m18.jpg")]);
    dispose(p320, DispositionType::Sold, "Dave Rossi", "2022-05-10", 450);
    must(
        firearm_ops::reverse_disposition(
            conn,
            p320,
            &ReverseDispositionInput {
                history: HistoryChoice::Keep,
                nickname: None,
                confirmed_warnings: false,
            },
        ),
        "reversing a disposition",
    );
    dispose(p320, DispositionType::Traded, "Ridgeline Arms", "2023-08-02", 400);
    must(
        firearm_ops::reverse_disposition(
            conn,
            p320,
            &ReverseDispositionInput {
                history: HistoryChoice::Keep,
                nickname: None,
                confirmed_warnings: false,
            },
        ),
        "reversing a disposition",
    );

    // -- Scheduled under Collector Schedule (in force, expires in 20 days) --

    let s_and_w = add(FirearmInput {
        nickname: text("The Revolver"),
        notes: text("Wood grips are replacements; the originals are in the safe."),
        estimated_value: Some(850),
        acquisition_date: text("2015-07-04"),
        insurance_policy_id: Some(policies.collector),
        scheduled_coverage_amount: Some(1_000),
        cartridge: text(".357 Magnum"),
        action_type_id: Some(REVOLVER),
        ..base("Smith & Wesson", "Model 686 Plus", "CFK1290", ".357", HANDGUN)
    });
    let ids =
        photos(s_and_w, &[seed_photo!("sw-686-cylinder.jpg"), seed_photo!("sw-686-side.jpg")]);
    // Not the first photo: a chosen thumbnail rather than the default.
    must(photo_ops::set_thumbnail_photo(conn, s_and_w, ids[1]), "choosing a thumbnail");

    let winchester = add(FirearmInput {
        nickname: text("Elk Rifle"),
        estimated_value: Some(1_400),
        acquisition_date: text("2014-10-12"),
        insurance_policy_id: Some(policies.collector),
        scheduled_coverage_amount: Some(1_500),
        action_type_id: Some(BOLT_ACTION),
        ..base("Winchester", "Model 70 Featherweight", "G2841175", ".270 Win", RIFLE)
    });
    photos(winchester, &[seed_photo!("winchester-model-70-featherweight.jpg")]);

    // -- Scheduled under Vault Schedule (in force) --------------------------

    // Scheduled for less than its value: under-insured.
    let colt = add(FirearmInput {
        nickname: text("Grandpa's 1911"),
        notes: text("Inherited. Series 70. Family piece, never sell."),
        estimated_value: Some(2_400),
        acquisition_source: text("Inherited"),
        acquisition_date: text("2009-06-01"),
        insurance_policy_id: Some(policies.vault),
        scheduled_coverage_amount: Some(2_000),
        cartridge: text(".45 ACP"),
        ..base("Colt", "1911 Government Model", "70S12345", ".45", HANDGUN)
    });
    photos(
        colt,
        &[seed_photo!("1911a1-field-stripped.jpg"), seed_photo!("colt-m1911-markings.jpg")],
    );

    // Scheduled for exactly its value: adequately insured.
    add(FirearmInput {
        nickname: text("Duck Gun"),
        estimated_value: Some(1_800),
        acquisition_date: text("2013-01-20"),
        insurance_policy_id: Some(policies.vault),
        scheduled_coverage_amount: Some(1_800),
        action_type_id: Some(SEMI_AUTOMATIC),
        ..base("Browning", "Auto-5 Light Twelve", "1V12345", "12 gauge", SHOTGUN)
    });

    // Long text everywhere, to check truncation and wrapping in tiles/rows.
    let commemorative = add(FirearmInput {
        nickname: text("The Really Long Nickname Used To Check How Tiles And Rows Truncate"),
        notes: text("Commemorative presentation piece. ".repeat(12).trim_end()),
        estimated_value: Some(3_200),
        acquisition_date: text("2011-11-11"),
        insurance_policy_id: Some(policies.vault),
        scheduled_coverage_amount: Some(3_500),
        ..base(
            "Smith & Wesson",
            "Model 1911 A1 Government Commemorative Limited Edition Engraved Presentation Grade",
            "LONG-SERIAL-0000000000000000001",
            ".45 ACP",
            HANDGUN,
        )
    });
    // Twice as many photos as any other record, each a different colour and
    // shape, in both formats, one of them a large full-size original: to see
    // how the photo strip and the viewer cope.
    const PRESENTATION_SHAPES: [(u32, u32, bool); 10] = [
        (1200, 800, false),
        (800, 1200, false),
        (2400, 1600, true),
        (1000, 1000, false),
        (320, 240, true),
        (1600, 400, false),
        (600, 1800, true),
        (1024, 768, true),
        (800, 600, false),
        (1600, 1200, true),
    ];
    for (i, &(width, height, jpeg)) in PRESENTATION_SHAPES.iter().enumerate() {
        let (extension, mime) = if jpeg { ("jpg", "image/jpeg") } else { ("png", "image/png") };
        let name = format!("presentation-{:02}.{extension}", i + 1);
        let bytes = gradient_image(width, height, i as u32 * 36, jpeg);
        must(photo_ops::add_photo(conn, commemorative, &bytes, &name, mime), &name);
    }

    // -- Scheduled under an expired policy: uninsured despite the amount ----

    add(FirearmInput {
        estimated_value: Some(650),
        acquisition_date: text("2020-09-09"),
        insurance_policy_id: Some(policies.expired_rider),
        scheduled_coverage_amount: Some(700),
        // specs/004-cartridges-action-types US1-2: a wildcat, whose caliber
        // the form guesses from its name.
        notes: text("Rebarreled by a gunsmith to a wildcat of his own."),
        cartridge: text(".30 Custom Improved"),
        ..base("Savage", "110", "S0011223", ".30", RIFLE)
    });

    // -- specs/002-firearm-identification: origin, year, country, importer --

    add(FirearmInput {
        notes: text(
            "Bring-back from a relative's WWII service; the importer's stamp is on the barrel band.",
        ),
        estimated_value: Some(1200),
        acquisition_source: text("Family estate"),
        acquisition_date: text("2015-08-14"),
        origin: Some(Origin::Reimported),
        importer_name: text("Century International Arms"),
        ..base("Inland", "M1 Carbine", "IN-2245567", ".30 Carbine", RIFLE)
    });

    add(FirearmInput {
        notes: text("Purchased new from the importer; original box and paperwork kept."),
        estimated_value: Some(850),
        acquisition_source: text("Ridgeline Arms"),
        acquisition_date: text("2022-05-02"),
        acquisition_price: Some(799),
        origin: Some(Origin::Imported),
        year_of_manufacture: Some(1943),
        country_of_manufacture: text("Belgium"),
        importer_name: text("Global Arms Import Co."),
        ..base("FN", "Model 1922", "FN-88431", ".32 ACP", HANDGUN)
    });

    // -- specs/002-firearm-identification User Story 2: original maker's marks --

    // The importer assigned its own serial number (per the paperwork); the
    // maker's own make, model and serial are entered as the original marks
    // (contracts/ui-identification.md §8 example 3).
    add(FirearmInput {
        notes: text(
            "Importer re-stamped a new serial on the receiver; the maker's original \
                      marks are still legible underneath.",
        ),
        estimated_value: Some(720),
        acquisition_source: text("Ridgeline Arms"),
        acquisition_date: text("2021-03-19"),
        acquisition_price: Some(650),
        origin: Some(Origin::Imported),
        country_of_manufacture: text("Austria"),
        importer_name: text("Global Arms Import Co."),
        original_make: text("Glock"),
        original_model: text("19"),
        original_serial_number: text("AWC442"),
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        ..base("Ridgeline Arms", "Imported Glock 19", "RA-70019", "9mm", HANDGUN)
    });

    // The importer adopted the maker's own model and serial as the main
    // marks: no separate original-marks entry is needed (US2-3).
    let imported_beretta = add(FirearmInput {
        notes: text("Importer's stamp only; the maker's marks are already the main marks."),
        estimated_value: Some(480),
        acquisition_source: text("Online auction"),
        acquisition_date: text("2020-10-02"),
        origin: Some(Origin::Imported),
        country_of_manufacture: text("Italy"),
        importer_name: text("Global Arms Import Co."),
        ..base("Beretta", "92FS", "BER556213", "9mm", HANDGUN)
    });

    photos(imported_beretta, &[seed_photo!("beretta-92fs-atf.jpg")]);

    // A domestic firearm, so every `origin` value appears in the seed
    // (human_seed_coverage_test's CHECK-value sweep).
    add(FirearmInput {
        notes: text("Made in the U.S.A.; no import paperwork involved."),
        estimated_value: Some(600),
        acquisition_source: text("Ridgeline Arms"),
        acquisition_date: text("2019-06-01"),
        acquisition_price: Some(560),
        origin: Some(Origin::Domestic),
        ..base("Ruger", "GP100", "RU-100234", ".357 Magnum", HANDGUN)
    });

    // -- specs/002-firearm-identification User Story 3: identity's year
    // exception and the original-marks warning --

    // Two pre-1968 revolvers whose maker restarted serial numbering: FR-008
    // accepts the pair because each carries a year of manufacture and the
    // years differ (contracts/ui-identification.md §8 example 6).
    add(FirearmInput {
        notes: text("Maker restarted its serial range that year; see the paired 1962 example."),
        estimated_value: Some(450),
        acquisition_source: text("Estate sale"),
        acquisition_date: text("2016-05-20"),
        origin: Some(Origin::Domestic),
        year_of_manufacture: Some(1955),
        ..base("Smith & Wesson", "Model 10", "S-100", ".38 Special", HANDGUN)
    });
    add(FirearmInput {
        notes: text("Same make, model and serial as the 1955 example; the year tells them apart."),
        estimated_value: Some(470),
        acquisition_source: text("Gun show"),
        acquisition_date: text("2018-09-08"),
        origin: Some(Origin::Domestic),
        year_of_manufacture: Some(1962),
        ..base("Smith & Wesson", "Model 10", "S-100", ".38 Special", HANDGUN)
    });

    // A matching original-marks pair, so the FR-009 warning is visible when
    // editing either one: both carry the same original maker, model and
    // serial number despite different main marks. Seeded with
    // `confirmed_warnings: true` since the warning would otherwise block the
    // second insert (research.md §5).
    let original_marks_first = FirearmInput {
        notes: text("Original-marks warning demo, firearm 1 of 2 (edit either to see it)."),
        estimated_value: Some(700),
        acquisition_source: text("Ridgeline Arms"),
        acquisition_date: text("2021-11-02"),
        origin: Some(Origin::Imported),
        country_of_manufacture: text("Belgium"),
        original_make: text("Fabrique Nationale"),
        original_model: text("High Power"),
        original_serial_number: text("FN-70044"),
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        ..base("Ridgeline Arms", "Imported Hi-Power A", "RA-90001", "9mm", HANDGUN)
    };
    must(firearm_ops::create_firearm(conn, &original_marks_first, false), "original-marks demo 1");
    let original_marks_second = FirearmInput {
        notes: text(
            "Original-marks warning demo, firearm 2 of 2 (shares firearm 1's original marks).",
        ),
        estimated_value: Some(710),
        acquisition_source: text("Ridgeline Arms"),
        acquisition_date: text("2021-11-09"),
        origin: Some(Origin::Imported),
        country_of_manufacture: text("Belgium"),
        original_make: text("Fabrique Nationale"),
        original_model: text("High Power"),
        original_serial_number: text("FN-70044"),
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        ..base("Ridgeline Arms", "Imported Hi-Power B", "RA-90002", "9mm", HANDGUN)
    };
    must(firearm_ops::create_firearm(conn, &original_marks_second, true), "original-marks demo 2");

    // -- Disposed ------------------------------------------------------------

    let kel_tec = add(FirearmInput {
        notes: text("Sold at a gun show; paperwork filed."),
        estimated_value: Some(250),
        acquisition_date: text("2017-03-03"),
        acquisition_price: Some(220),
        ..base("Kel-Tec", "P32", "R3K123", ".32 ACP", HANDGUN)
    });
    dispose(kel_tec, DispositionType::Sold, "Mark Tanaka", "2025-02-11", 250);

    let marlin = add(FirearmInput {
        estimated_value: Some(700),
        acquisition_date: text("2010-04-04"),
        ..base("Marlin", "336", "MR445566", ".30-30 Win", RIFLE)
    });
    dispose(marlin, DispositionType::Gifted, "Nephew Tom (Christmas)", "2024-12-25", 0);

    let lcp = add(FirearmInput {
        estimated_value: Some(280),
        acquisition_date: text("2019-01-19"),
        ..base("Ruger", "LCP", "371-00099", ".380 ACP", HANDGUN)
    });
    dispose(
        lcp,
        DispositionType::LostStolen,
        "Reported to police, case 23-118342",
        "2023-07-02",
        0,
    );

    let patriot = add(FirearmInput {
        estimated_value: Some(450),
        acquisition_date: text("2021-06-06"),
        // specs/004-cartridges-action-types US2-7: disposed, so its unique
        // cartridge is still suggested.
        cartridge: text("6.5 Patriot Wildcat"),
        ..base("Mossberg", "Patriot", "MP778899", "6.5mm", RIFLE)
    });
    dispose(patriot, DispositionType::Traded, "Ridgeline Arms", "2025-06-30", 400);

    let cracked = add(FirearmInput {
        estimated_value: Some(0),
        acquisition_date: text("2019-04-20"),
        ..base("Remington", "Model 700 (cracked receiver)", "RR700-2231", ".30-06", RIFLE)
    });
    dispose(cracked, DispositionType::Destroyed, "County buyback program", "2025-02-11", 0);

    // -- specs/004-cartridges-action-types US2: suggestion and snapping data -
    //
    // Typing "sw", "smith w" or "s&w" in Make lists "Smith & Wesson" (on
    // several firearms above) and "S&W" (on one, a different notation that
    // is kept as typed); "Springfield armory" is a variant of the three
    // "Springfield Armory" firearms, and snaps to that spelling. With make
    // "Ruger" on the form, Model lists "10/22" and "Mini-14" before Marlin's
    // "336".

    add(FirearmInput {
        estimated_value: Some(475),
        acquisition_date: text("2020-08-14"),
        cartridge: text(".38 Special"),
        ..base("S&W", "Model 60", "S&W-60-1", ".357", HANDGUN)
    });
    // specs/005-regulated-item-types US1: a Suppressor has no action, barrel
    // length or capacity, and its caliber is a rating. No classification.
    add(FirearmInput {
        estimated_value: Some(900),
        acquisition_date: text("2024-05-18"),
        overall_length_hundredths: Some(780),
        weight_tenths_oz: Some(130),
        finish: text("Cerakote graphite black"),
        ..base("SilencerCo", "Omega 300", "OM300-20418", ".30", SUPPRESSOR)
    });
    // specs/005-regulated-item-types US2: registration is a record of what
    // the owner enters, independent of the type. A Suppressor registered as
    // Suppressor on Form 4, with the approved form attached.
    let registered_suppressor = add(FirearmInput {
        estimated_value: Some(1100),
        acquisition_source: text("Ridgeline Arms"),
        acquisition_date: text("2025-09-12"),
        overall_length_hundredths: Some(690),
        weight_tenths_oz: Some(115),
        finish: text("Anodized black"),
        registration_class_id: Some(REGISTERED_SUPPRESSOR),
        registration_form: text("Form 4"),
        registration_approved: text("2026-02-10"),
        registered_to: text("Smith Family Trust"),
        ..base("Dead Air", "Sandman-K", "SMK-51207", ".30", SUPPRESSOR)
    });
    documents(
        registered_suppressor,
        &[(
            "Form 4 approval.pdf",
            simple_pdf(&["Approved Form 4", "Registered to Smith Family Trust"]),
            "application/pdf",
        )],
    );
    // A Rifle made into a short-barreled rifle on a Form 1, to the owner.
    add(FirearmInput {
        estimated_value: Some(1300),
        acquisition_date: text("2024-11-03"),
        barrel_length_hundredths: Some(1050),
        registration_class_id: Some(REGISTERED_SBR),
        registration_form: text("Form 1"),
        registration_approved: text("2025-06-20"),
        registered_to: text("Alex Rivera"),
        ..base("Daniel Defense", "DDM4 V7", "DD-770231", ".223", RIFLE)
    });
    // A 10.5 in barrel with no classification: nothing about it is judged.
    add(FirearmInput {
        estimated_value: Some(950),
        acquisition_date: text("2023-07-29"),
        barrel_length_hundredths: Some(1050),
        ..base("Ruger", "Mini Thirty", "580-90021", ".30", RIFLE)
    });
    // A select-fire rifle registered as a Machine gun (US3): the action and
    // the classification are entered separately.
    add(FirearmInput {
        estimated_value: Some(14500),
        acquisition_date: text("2025-09-12"),
        action_type_id: Some(ACTION_AUTOMATIC),
        registration_class_id: Some(REGISTERED_MACHINE_GUN),
        registration_form: text("Form 4"),
        registration_approved: text("2025-12-02"),
        registered_to: text("Alex Rivera"),
        ..base("Colt", "M16A1", "CM-3317902", ".223", RIFLE)
    });
    // A classification with no approved date yet.
    add(FirearmInput {
        estimated_value: Some(700),
        acquisition_date: text("2026-01-15"),
        barrel_length_hundredths: Some(1200),
        registration_class_id: Some(REGISTERED_SBS),
        registration_form: text("Form 1"),
        registered_to: text("Alex Rivera"),
        ..base("Mossberg", "590 Shockwave", "MS-400518", "12 gauge", SHOTGUN)
    });
    // Disposed of, yet its registration stays, under a unique name to search.
    let disposed_registered = add(FirearmInput {
        estimated_value: Some(800),
        acquisition_date: text("2022-04-09"),
        overall_length_hundredths: Some(720),
        registration_class_id: Some(REGISTERED_SUPPRESSOR),
        registration_form: text("Form 4"),
        registration_approved: text("2022-08-01"),
        registered_to: text("Zephyr Holdings LLC"),
        ..base("SilencerCo", "Sparrow", "SP-7781", ".22", SUPPRESSOR)
    });
    dispose(disposed_registered, DispositionType::Sold, "Kestrel Outfitters", "2025-03-03", 650);
    // Typed in lower case: the form snaps it to "Smith Family Trust", and
    // this record shows what is kept when it is not snapped.
    add(FirearmInput {
        estimated_value: Some(600),
        acquisition_date: text("2024-02-20"),
        registration_class_id: Some(REGISTERED_SUPPRESSOR),
        registered_to: text("Smith family trust"),
        ..base("Griffin Armament", "Optimus", "GA-30412", ".22", SUPPRESSOR)
    });
    add(FirearmInput {
        estimated_value: Some(300),
        acquisition_date: text("2019-02-02"),
        ..base("Ruger", "10/22", "0013-99001", ".22", RIFLE)
    });
    add(FirearmInput {
        estimated_value: Some(900),
        acquisition_date: text("2021-10-09"),
        cartridge: text(".223 Remington"),
        ..base("Ruger", "Mini-14", "580-11234", ".22", RIFLE)
    });
    add(FirearmInput {
        estimated_value: Some(550),
        acquisition_date: text("2022-03-12"),
        cartridge: text("9x19mm Parabellum"),
        ..base("Springfield Armory", "XD-M Elite", "XM-330021", "9mm", HANDGUN)
    });
    add(FirearmInput {
        estimated_value: Some(520),
        acquisition_date: text("2023-01-21"),
        ..base("Springfield Armory", "Hellcat", "HC-778812", "9mm", HANDGUN)
    });
    add(FirearmInput {
        estimated_value: Some(1100),
        acquisition_date: text("2023-05-05"),
        ..base("Springfield armory", "Saint Victor", "SV-556677", ".223", RIFLE)
    });

    // -- Generated filler for scrolling, grouping and search ----------------

    for input in extras {
        add(input);
    }
}

/// Deterministic filler: plain, valued, unscheduled firearms with distinct
/// serial numbers, spread across makes, calibers and types.
fn generated_firearms(count: usize) -> Vec<FirearmInput> {
    const MODELS: &[(&str, &str, &str, i64, i64)] = &[
        ("Glock", "17", "9mm", HANDGUN, 550),
        ("Beretta", "92FS", "9mm", HANDGUN, 650),
        ("Ruger", "GP100", ".357 Magnum", HANDGUN, 800),
        ("Smith & Wesson", "M&P Shield", "9mm", HANDGUN, 450),
        ("Kimber", "Micro 9", "9mm", HANDGUN, 700),
        ("Savage", "Axis", ".243 Win", RIFLE, 400),
        ("Marlin", "1895", ".45-70", RIFLE, 900),
        ("Tikka", "T3x", "6.5 Creedmoor", RIFLE, 1_000),
        ("Ruger", "American", ".308 Win", RIFLE, 550),
        ("Mossberg", "590A1", "12 gauge", SHOTGUN, 600),
        ("Stoeger", "Coach Gun", "20 gauge", SHOTGUN, 450),
        ("CVA", "Wolf", ".50 black powder", OTHER, 300),
    ];
    (0..count)
        .map(|i| {
            let (make, model, caliber, type_id, value) = MODELS[i % MODELS.len()];
            let serial = format!("GEN{:05}", i + 1);
            FirearmInput {
                estimated_value: Some(value),
                acquisition_date: Some(format!(
                    "20{:02}-{:02}-{:02}",
                    10 + i % 15,
                    1 + i % 12,
                    1 + i % 28
                )),
                ..base(make, model, &serial, caliber, type_id)
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Generated files
// ---------------------------------------------------------------------------

/// A diagonal colour gradient with stripes, so each seeded photo looks
/// different in the tiles and the full-size view. PNG or JPEG.
fn gradient_image(width: u32, height: u32, hue: u32, jpeg: bool) -> Vec<u8> {
    let image = image::RgbImage::from_fn(width, height, |x, y| {
        let t = (x + y) as f32 / (width + height) as f32;
        let striped = ((x + y) / 40) % 2 == 0;
        let (r, g, b) =
            hsv_to_rgb((hue as f32 + t * 40.0) % 360.0, 0.55, if striped { 0.85 } else { 0.7 });
        image::Rgb([r, g, b])
    });
    let mut bytes = Vec::new();
    let format = if jpeg { image::ImageFormat::Jpeg } else { image::ImageFormat::Png };
    image.write_to(&mut Cursor::new(&mut bytes), format).expect("encode a seed image");
    bytes
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let byte = |channel: f32| ((channel + m) * 255.0).round() as u8;
    (byte(r), byte(g), byte(b))
}

/// A one-page PDF with real text, small enough to build by hand, so opening
/// a document hands the OS viewer something it can actually display.
fn simple_pdf(lines: &[&str]) -> Vec<u8> {
    let escape = |line: &str| line.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
    let mut content = String::from("BT /F1 16 Tf 72 720 Td 22 TL\n");
    for line in lines {
        content.push_str(&format!("({}) Tj T*\n", escape(line)));
    }
    content.push_str("ET");

    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 5 0 R \
         /Resources << /Font << /F1 4 0 R >> >> >>"
            .to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{body}\nendobj\n", index + 1));
    }
    let xref = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
    for offset in offsets {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    ));
    pdf.into_bytes()
}

/// Spreadsheets to try File > Import with, each shaped to hit a different
/// path: a clean import, conflicts with the seeded collection, and rows that
/// fail validation. Written next to the database, never imported here.
pub fn write_import_samples(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).expect("create the import samples directory");

    let row = |cells: &[(&str, &str)]| -> Vec<String> {
        for (name, _) in cells {
            assert!(COLUMNS.contains(name), "unknown spreadsheet column {name}");
        }
        COLUMNS
            .iter()
            .map(|column| {
                cells.iter().find(|(name, _)| name == column).map_or("", |(_, v)| *v).to_owned()
            })
            .collect()
    };
    let write = |name: &str, rows: Vec<Vec<String>>| {
        let mut writer = csv::Writer::from_path(dir.join(name)).expect("create an import sample");
        writer.write_record(COLUMNS).expect("write the header");
        for record in rows {
            writer.write_record(record).expect("write a row");
        }
        writer.flush().expect("flush an import sample");
    };
    // specs/004-cartridges-action-types FR-023: a sheet exported before
    // `cartridge` and `action_type` existed, so its header lacks both.
    let write_without = |name: &str, dropped: &[&str], rows: Vec<Vec<String>>| {
        let kept: Vec<usize> =
            (0..COLUMNS.len()).filter(|&index| !dropped.contains(&COLUMNS[index])).collect();
        let mut writer = csv::Writer::from_path(dir.join(name)).expect("create an import sample");
        writer.write_record(kept.iter().map(|&index| COLUMNS[index])).expect("write the header");
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
                ("insurance_policy_name", "Vault Schedule"),
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
            // (contracts/ui-identification.md §8 example 3).
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
            // specs/002-firearm-identification US4-3: not one of the three
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
            // US4-7: a year later than the current local year.
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
            // US4-4: country of manufacture is never allowed on a
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
            // US4-6: original marks matching the seeded warning-demo pair
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

    // specs/004-cartridges-action-types US4: the report's "Calibers filled in
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

    // specs/005-regulated-item-types US4: a round-trip sheet, as export
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
    // a future approved date, and a Suppressor row with an action, a barrel
    // length and a capacity (all three reported on the one row).
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

    dir.to_path_buf()
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
