//! Performance validation (constitution Principle IV, spec.md's tested
//! scale up to 10,000 firearm records): `list_firearms` and
//! `get_value_summary` must return within the 500ms search/browse budget
//! at that scale, backed by the FTS5 index and indexed columns from
//! data-model.md — no mocks, a real temporary SQLCipher database.
//!
//! Feature 006 adds every row of research.md §22 at 10,000 firearms and
//! 10,000 accessories with 5,000 mounts (FR-026, SC-007).
//!
//! Feature 007 adds the rows of research.md §22 for the document preview
//! (SC-001, SC-007), each on a database of 10,000 firearms and 10,000
//! accessories: `open_preview` of a 10 MB TIFF (with the real helper), of a
//! 10 MB PDF (to the recording surface and the `hdpreview` handler serving
//! it) and of a 10 MB text, the text's decode, and searching a document name
//! among 30,000 documents.
//!
//! Feature 003 adds opening a database, key derivation included, to the 1s
//! action budget (SC-003), the first progress event of a backup, a
//! passphrase change, a restore and a move of backups to the 100ms feedback
//! budget (SC-005), and the take-over check every save makes (research.md
//! §6).

#[path = "support/preview_support.rs"]
mod preview_support;
mod support;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::accessories::{
    AccessoryGroupBy, ListAccessoriesInput, ListAccessoriesOutput,
};
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops::{self as databases_ops, Unlock};
use hoplodex_lib::commands::entries::{SuggestEntriesInput, ops as entry_ops};
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::firearms::{DisposeInput, GroupBy, ListFirearmsInput};
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::commands::mounts::ops as mount_ops;
use hoplodex_lib::models::accessory::AccessoryInput;
use hoplodex_lib::models::database::{
    BackupLocationInput, BackupOutcome, BackupSettingsInput, CloseReason, ExistingBackupsChoice,
};
use hoplodex_lib::models::firearm::{FirearmInput, Origin};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::backups;
use hoplodex_lib::services::entry_text::EntryField;
use hoplodex_lib::services::insurance_status::InsuranceWarning;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::preview::protocol_handler;
use hoplodex_lib::services::preview::text as preview_text;
use hoplodex_lib::services::valuation::get_value_summary;
use hoplodex_lib::session::{Session, lifecycle};
use preview_support::Fixture;
use rusqlite::{Connection, params};
use serde_json::json;
use support::{TEST_PASSPHRASE, TestDb, TestEvents, firearm, passphrase, policy};
use tempfile::TempDir;

const RECORD_COUNT: usize = 10_000;
const BUDGET_MS: u128 = 500;

/// The tests run one at a time, so that none is timed while another seeds
/// 10,000 records or copies a database beside it.
fn one_at_a_time() -> MutexGuard<'static, ()> {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A closed database holding the 10,000 seeded firearms, made once for the
/// whole test binary (seeding is most of a test's time) and copied for each
/// test that needs only that.
fn seeded_firearms() -> TestDb {
    static TEMPLATE: OnceLock<PathBuf> = OnceLock::new();
    let template = TEMPLATE.get_or_init(|| {
        let template = TestDb::new();
        seed_10k_firearms(&template.conn);
        let (conn, dir) = template.into_parts();
        // Closed, so that the copies are of a file nothing holds open.
        drop(conn);
        let dir = dir.keep();
        // Statics are never dropped, so the directory is removed as the
        // process exits.
        static REMOVE_AT_EXIT: OnceLock<PathBuf> = OnceLock::new();
        extern "C" fn remove_template() {
            if let Some(dir) = REMOVE_AT_EXIT.get() {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
        REMOVE_AT_EXIT.set(dir.clone()).unwrap();
        // The C runtime's, on every platform (`libc` is a Unix-only dependency).
        unsafe extern "C" {
            fn atexit(callback: extern "C" fn()) -> std::ffi::c_int;
        }
        // SAFETY: `remove_template` is a plain function that touches only a
        // static it reads.
        unsafe { atexit(remove_template) };
        dir.join(TestDb::FILE_NAME)
    });
    TestDb::copy_of(template)
}

fn firearm_ids(conn: &Connection) -> Vec<i64> {
    let mut ids_stmt = conn.prepare("SELECT id FROM firearms").unwrap();
    let ids: Vec<i64> =
        ids_stmt.query_map([], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(ids.len(), RECORD_COUNT);
    ids
}

fn seed_10k_firearms(conn: &Connection) -> Vec<i64> {
    let makes = ["Glock", "Sig", "Ruger", "Smith & Wesson", "Colt"];
    let calibers = ["9mm", ".45 ACP", ".22 LR", ".223", ".308"];
    let types = [1, 2, 3, 4];
    let finishes = ["Blued", "Parkerized", "Cerakote", "Stainless", "Nickel"];
    let conditions = ["new_in_box", "like_new", "excellent", "good", "fair", "poor"];

    // specs/002-firearm-identification: every record also carries an origin
    // and a year, and import-marked ones (half the table) carry a country
    // (Imported only), an importer, and original marks — so the widened
    // firearms_fts index and idx_firearms_original_serial carry the full
    // load a real 10,000-record collection would (T052).
    let origins = [None, Some("domestic"), Some("imported"), Some("reimported")];
    // specs/004-cartridges-action-types: most records carry a cartridge, a
    // few a unique custom one, and some none (SC-004).
    let cartridges = [Some("9x19mm Parabellum"), Some(".45 ACP"), None, Some(".223 Remington")];

    // Semi-automatic and Bolt action are allowed for every seeded type; a
    // third of the records have none.
    let actions = [Some(1), Some(3), None];

    let tx = conn.unchecked_transaction().unwrap();
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO firearms (
                    uid, make, model, serial_number, no_serial_attested, caliber, firearm_type_id,
                    notes, nickname, barrel_length_hundredths, overall_length_hundredths,
                    weight_tenths_oz, capacity, finish, condition,
                    status, origin, year_of_manufacture, country_of_manufacture,
                    importer_name, original_make, original_model, original_serial_number,
                    cartridge, action_type_id, created_at, updated_at
                ) VALUES (
                    ?23, ?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                    'active', ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, datetime('now'), datetime('now')
                )",
            )
            .unwrap();
        for i in 0..RECORD_COUNT {
            let make = makes[i % makes.len()];
            let caliber = calibers[i % calibers.len()];
            let firearm_type_id = types[i % types.len()];
            let serial = format!("PERF-{i}");
            let notes = if i == RECORD_COUNT / 2 {
                "a uniquely findable note xyzzy"
            } else {
                "routine notes"
            };
            let origin = origins[i % origins.len()];
            let import_marked = matches!(origin, Some("imported") | Some("reimported"));
            // A fifth of import-marked rows carry the heavier importer and
            // original-marks fields — enough to load the widened index and
            // the FR-009 partial index realistically, without doubling the
            // whole seed's bulk (every row already carries an origin/year).
            let full_marks = import_marked && i % 5 == 0;
            let country = (origin == Some("imported") && full_marks).then(|| "Belgium".to_string());
            let importer = full_marks.then(|| "Global Arms Import Co.".to_string());
            let original_serial = full_marks.then(|| format!("ORIG-{i}"));
            stmt.execute(params![
                make,
                format!("Model {i}"),
                serial,
                caliber,
                firearm_type_id,
                notes,
                // Every record has a nickname, which is unique among active
                // firearms (FR-031): the index and the FTS column both carry
                // the full load.
                format!("Nick {i}"),
                // Every record carries all six physical details (FR-039), so
                // `finish` loads the FTS index like the other text columns.
                1_000 + (i % 3_000) as i64,
                2_000 + (i % 4_000) as i64,
                100 + (i % 900) as i64,
                1 + (i % 30) as i64,
                if i == RECORD_COUNT / 3 {
                    "zorblax".to_string()
                } else {
                    format!("{} finish", finishes[i % finishes.len()])
                },
                conditions[i % conditions.len()],
                origin,
                1400 + (i % 600) as i64,
                country,
                importer,
                importer.as_ref().map(|_| "Fabrique Nationale"),
                importer.as_ref().map(|_| "High Power"),
                original_serial,
                if i % 97 == 0 {
                    Some(format!("Custom Wildcat {i}"))
                } else {
                    cartridges[i % cartridges.len()].map(str::to_owned)
                },
                actions[i % actions.len()],
                support::uid(),
            ])
            .unwrap();
        }
    }
    tx.commit().unwrap();

    firearm_ids(conn)
}

#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn list_firearms_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = seeded_firearms();

    let started = Instant::now();
    let result = firearm_ops::list_firearms(&db.conn, &ListFirearmsInput::default()).unwrap();
    let elapsed = started.elapsed();

    let total: usize = result.groups.iter().map(|g| g.firearms.len()).sum();
    assert_eq!(total, RECORD_COUNT);
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "list_firearms (no filter) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );
}

#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn list_firearms_search_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = seeded_firearms();

    let started = Instant::now();
    let result = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("xyzzy".into()), ..Default::default() },
    )
    .unwrap();
    let elapsed = started.elapsed();

    let total: usize = result.groups.iter().map(|g| g.firearms.len()).sum();
    assert_eq!(total, 1, "the FTS search should find exactly the one uniquely-noted record");
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "list_firearms (FTS search) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );
}

#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn list_firearms_finish_search_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = seeded_firearms();

    let started = Instant::now();
    let result = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("zorblax".into()), ..Default::default() },
    )
    .unwrap();
    let elapsed = started.elapsed();

    let total: usize = result.groups.iter().map(|g| g.firearms.len()).sum();
    assert_eq!(total, 1, "the FTS search should find exactly the one uniquely-finished record");
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "list_firearms (finish search) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );

    // A word shared by every record still stays inside the budget.
    let started = Instant::now();
    let common = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("finish".into()), ..Default::default() },
    )
    .unwrap();
    assert!(common.groups.iter().map(|g| g.firearms.len()).sum::<usize>() > 1);
    assert!(
        started.elapsed().as_millis() < BUDGET_MS,
        "list_firearms (common finish word) took {}ms, over the {BUDGET_MS}ms budget",
        started.elapsed().as_millis()
    );
}

#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn list_firearms_grouped_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = seeded_firearms();

    let started = Instant::now();
    let result = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { group_by: Some(GroupBy::Caliber), ..Default::default() },
    )
    .unwrap();
    let elapsed = started.elapsed();

    assert!(result.groups.len() > 1);
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "list_firearms (grouped) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );

    // specs/002-firearm-identification T052: the widened firearms_fts index
    // (origin, year, country, importer, original marks) and the FR-009
    // original_marks_clash lookup, checked on this test's already-seeded
    // database rather than building another 10,000-record one.
    let started = Instant::now();
    let by_origin = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("imported".into()), ..Default::default() },
    )
    .unwrap();
    let elapsed = started.elapsed();
    let total: usize = by_origin.groups.iter().map(|g| g.firearms.len()).sum();
    assert!(total > 1_000, "imported and re-imported records should both match");
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "list_firearms (common origin search) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );

    let started = Instant::now();
    let by_original_serial = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("ORIG-5010".into()), ..Default::default() },
    )
    .unwrap();
    let elapsed = started.elapsed();
    assert_eq!(by_original_serial.groups.iter().map(|g| g.firearms.len()).sum::<usize>(), 1);
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "list_firearms (original serial search) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );

    let input = FirearmInput {
        origin: Some(Origin::Imported),
        original_make: Some("Distinctive Maker".into()),
        original_model: Some("Distinctive Model".into()),
        original_serial_number: Some("PERF-NEW-ORIGINAL".into()),
        registration_class_id: None,
        registration_form: None,
        registration_approved: None,
        registered_to: None,
        ..firearm("Distinctive Make", "Distinctive Model", "PERF-NEW-MAIN")
    };
    let started = Instant::now();
    firearm_ops::create_firearm(&db.conn, &input, false, None).unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "create_firearm with original marks (FR-009 lookup) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );
}

/// specs/004-cartridges-action-types SC-004 (research.md §11): grouping by
/// cartridge within the 1s action budget, and a search by a cartridge
/// within the 500ms search budget.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn list_firearms_by_cartridge_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = seeded_firearms();

    let started = Instant::now();
    let grouped = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { group_by: Some(GroupBy::Cartridge), ..Default::default() },
    )
    .unwrap();
    let elapsed = started.elapsed();
    // Printed for the pull request's performance note (`--nocapture`).
    eprintln!("SC-004: list_firearms grouped by cartridge took {elapsed:?}");
    assert_eq!(grouped.groups.last().map(|g| g.key.as_str()), Some("Unspecified"));
    assert!(grouped.groups.len() > 100, "the custom cartridges are groups of their own");
    assert!(
        elapsed.as_millis() < 1_000,
        "list_firearms (grouped by cartridge) took {}ms, over the 1000ms budget",
        elapsed.as_millis()
    );

    let started = Instant::now();
    let searched = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("Parabellum".into()), ..Default::default() },
    )
    .unwrap();
    let elapsed = started.elapsed();
    eprintln!("SC-004: list_firearms cartridge search took {elapsed:?}");
    assert!(searched.groups.iter().map(|g| g.firearms.len()).sum::<usize>() > 2_000);
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "list_firearms (cartridge search) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );
}

/// specs/004-cartridges-action-types SC-004: grouping by action type within
/// the 1s action budget, and a search by an action's name within the 500ms
/// search budget.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn list_firearms_by_action_type_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = seeded_firearms();

    let started = Instant::now();
    let grouped = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { group_by: Some(GroupBy::ActionType), ..Default::default() },
    )
    .unwrap();
    let elapsed = started.elapsed();
    eprintln!("SC-004: list_firearms grouped by action type took {elapsed:?}");
    let keys: Vec<&str> = grouped.groups.iter().map(|g| g.key.as_str()).collect();
    assert_eq!(keys, vec!["Semi-automatic", "Bolt action", "Unspecified"]);
    assert!(
        elapsed.as_millis() < 1_000,
        "list_firearms (grouped by action type) took {}ms, over the 1000ms budget",
        elapsed.as_millis()
    );

    let started = Instant::now();
    let searched = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("Bolt action".into()), ..Default::default() },
    )
    .unwrap();
    let elapsed = started.elapsed();
    eprintln!("SC-004: list_firearms action search took {elapsed:?}");
    assert!(searched.groups.iter().map(|g| g.firearms.len()).sum::<usize>() > 2_000);
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "list_firearms (action search) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );
}

/// specs/004-cartridges-action-types SC-004 (research.md §5): the
/// suggestion list for each field, computed per keystroke at 10,000
/// firearms with 10,000 distinct models (the worst case), within 50ms: half
/// of the 100ms budget, the rest being IPC and rendering.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn suggest_entries_completes_within_50ms_at_10k_records_with_10k_distinct_models() {
    let _alone = one_at_a_time();
    let db = seeded_firearms();

    for (field, text, make) in [
        (EntryField::Make, "sw", None),
        (EntryField::Model, "model 12", Some("Ruger")),
        (EntryField::Model, "", None),
        (EntryField::Cartridge, "9", None),
        (EntryField::Cartridge, "", None),
        (EntryField::Caliber, ".2", None),
    ] {
        let input = SuggestEntriesInput { field, text: text.into(), make: make.map(str::to_owned) };
        let started = Instant::now();
        let output = entry_ops::suggest_entries(&db.conn, &input).unwrap();
        let elapsed = started.elapsed();
        eprintln!("SC-004: suggest_entries({field:?}, {text:?}) took {elapsed:?}");
        assert!(!output.suggestions.is_empty());
        assert!(
            elapsed.as_millis() < 50,
            "suggest_entries({field:?}, {text:?}) took {}ms at {RECORD_COUNT} records, over the 50ms budget",
            elapsed.as_millis()
        );
    }
}

#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn list_firearms_nickname_search_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = seeded_firearms();

    let started = Instant::now();
    let result = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("Nick 4242".into()), ..Default::default() },
    )
    .unwrap();
    let elapsed = started.elapsed();

    let found: Vec<_> = result.groups.iter().flat_map(|g| &g.firearms).collect();
    assert!(found.iter().any(|f| f.nickname.as_deref() == Some("Nick 4242")));
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "list_firearms (nickname search) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );
}

/// The value summary does real work at this scale: the blanket policy in
/// force totals every unscheduled firearm, a fifth of the collection is
/// scheduled under a schedule-only policy, and most carry a value.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn get_value_summary_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = seeded_firearms();
    let ids = firearm_ids(&db.conn);

    insurance_ops::create_policy(
        &db.conn,
        &policy("Perf blanket", "2020-01-01", "2099-01-01", Some(100_000_000)),
    )
    .unwrap();
    let rider = insurance_ops::create_policy(
        &db.conn,
        &policy("Perf rider", "2020-01-01", "2099-01-01", None),
    )
    .unwrap();

    db.conn.execute_batch("UPDATE firearms SET estimated_value = 50000 WHERE id % 3 = 0").unwrap();
    for &id in ids.iter().step_by(5) {
        insurance_ops::assign_firearm_coverage(&db.conn, id, Some(rider.id), Some(40_000)).unwrap();
    }

    let started = Instant::now();
    let summary = get_value_summary(&db.conn).unwrap();
    let elapsed = started.elapsed();

    assert!(summary.collection_total > 0);
    assert!(summary.blanket.as_ref().is_some_and(|b| b.firearm_count > 0));
    assert!(!summary.by_policy.is_empty());
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "get_value_summary took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );
}

/// Browse rows carry each firearm's insurance warning, which needs the
/// blanket computation too.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn list_firearms_with_blanket_coverage_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = seeded_firearms();
    insurance_ops::create_policy(
        &db.conn,
        &policy("Perf blanket", "2020-01-01", "2099-01-01", Some(1_000_000)),
    )
    .unwrap();
    db.conn.execute_batch("UPDATE firearms SET estimated_value = 50000 WHERE id % 3 = 0").unwrap();

    let started = Instant::now();
    let result = firearm_ops::list_firearms(&db.conn, &ListFirearmsInput::default()).unwrap();
    let elapsed = started.elapsed();

    let flagged = result
        .groups
        .iter()
        .flat_map(|g| &g.firearms)
        .filter(|f| f.insurance_warning != InsuranceWarning::None)
        .count();
    assert!(flagged > 0, "the blanket total is far over its limit, so firearms are flagged");
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "list_firearms (with blanket warnings) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );
}

#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn deleting_a_firearm_completes_within_the_interactive_budget_at_10k_records() {
    let _alone = one_at_a_time();
    // Deleting vacuums the file to return the freed space (Constitution V),
    // which must stay inside the 1s completion budget (Constitution IV).
    let db = seeded_firearms();
    let ids = firearm_ids(&db.conn);

    let started = Instant::now();
    firearm_ops::delete_firearm(&db.conn, ids[RECORD_COUNT / 2], true).unwrap();
    let elapsed = started.elapsed();

    assert!(
        elapsed.as_millis() < 1_000,
        "delete (with vacuum) took {}ms at {RECORD_COUNT} records; budget is 1000ms",
        elapsed.as_millis()
    );
}

// --- Feature 003: opening, progress and the take-over check -----------------

/// A 10,000-firearm database made and filled through the session, as the
/// app would, then dropped without a close (its own open marker, which
/// the next open here clears). Machine settings live in a temp config
/// directory.
struct LargeDatabase {
    dir: TempDir,
    _config: TempDir,
    machine: MachineSettings,
    session: Session,
    events: Arc<TestEvents>,
}

impl LargeDatabase {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let events = Arc::new(TestEvents::default());
        let session = Session::new(events.clone(), Some(config.path().join("opened-documents")));
        let large = Self {
            dir: TempDir::new().unwrap(),
            machine: MachineSettings::load(config.path()).unwrap(),
            _config: config,
            session,
            events,
        };
        lifecycle::create(&large.session, &large.machine, &large.path(), &passphrase()).unwrap();
        large
            .session
            .write(|conn| {
                seed_10k_firearms(conn);
                Ok(())
            })
            .unwrap();
        large
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("Large.hoplodex")
    }

    fn open(&self, typed: &str) {
        let typed = Passphrase::from_input(typed.to_owned());
        databases_ops::open_database(
            &self.session,
            &self.machine,
            &self.path().to_string_lossy(),
            Unlock::typed(&typed),
            false,
        )
        .unwrap();
    }

    /// How long after `run` starts the first `event` is sent.
    fn first_event_after<T>(&self, event: &'static str, run: impl FnOnce() -> T) -> (T, Duration) {
        let started = Arc::new(Mutex::new(None::<Instant>));
        let first = Arc::new(Mutex::new(None::<Duration>));
        let (since, seen) = (started.clone(), first.clone());
        self.events.on_event(move |name, _| {
            let mut seen = seen.lock().unwrap();
            if name == event && seen.is_none() {
                *seen = Some(since.lock().unwrap().unwrap().elapsed());
            }
        });
        *started.lock().unwrap() = Some(Instant::now());
        let result = run();
        let waited = first.lock().unwrap().unwrap_or_else(|| panic!("no {event} event"));
        (result, waited)
    }
}

fn assert_progress_in_time(event: &str, waited: Duration) {
    // Printed for the pull request's performance note (`--nocapture`).
    eprintln!("SC-005: first {event} after {waited:?}");
    assert!(
        waited.as_millis() < 100,
        "SC-005: the first {event} came {}ms after the operation started, at {RECORD_COUNT} \
         records; budget is 100ms",
        waited.as_millis()
    );
}

#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn opening_a_database_of_10k_firearms_takes_at_most_a_second_including_key_derivation() {
    let _alone = one_at_a_time();
    let large = LargeDatabase::new();
    drop(large.session.take());

    let started = Instant::now();
    large.open(TEST_PASSPHRASE);
    let elapsed = started.elapsed();
    eprintln!("SC-003: opened {RECORD_COUNT} firearms in {elapsed:?}");

    assert!(
        elapsed.as_millis() < 1_000,
        "SC-003: opening took {}ms at {RECORD_COUNT} records; budget is 1000ms",
        elapsed.as_millis()
    );
}

#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn backup_passphrase_change_restore_and_move_each_report_progress_within_100ms() {
    let _alone = one_at_a_time();
    let large = LargeDatabase::new();
    let new_passphrase = "a much longer passphrase of several words";

    // The seeding was a change, so this close makes a backup.
    let (outcome, waited) = large.first_event_after("backup:progress", || {
        lifecycle::close_normal(&large.session, &large.machine, CloseReason::Closed).unwrap()
    });
    assert_eq!(outcome.backup, BackupOutcome::Made);
    assert_progress_in_time("backup:progress", waited);

    large.open(TEST_PASSPHRASE);
    let scratch = TempDir::new().unwrap();
    let (changed, waited) = large.first_event_after("passphrase_change:progress", || {
        backups_ops::change_passphrase(
            &large.session,
            &large.machine,
            scratch.path(),
            &passphrase(),
            &Passphrase::from_input(new_passphrase.to_owned()),
        )
    });
    changed.unwrap();
    assert_progress_in_time("passphrase_change:progress", waited);

    let backup = backups_ops::list_backups(&large.session, &large.machine, None).unwrap().backups
        [0]
    .path
    .clone();
    let (restored, waited) = large.first_event_after("restore:progress", || {
        backups_ops::restore_backup(&large.session, &large.machine, &backup, &passphrase(), None)
    });
    restored.unwrap();
    assert_progress_in_time("restore:progress", waited);

    // Moving the backups (the close's and the restore's) to a new location,
    // by the slower copy-and-verify path, as between drives (FR-026).
    let new_folder = large.dir.path().join("elsewhere");
    std::fs::create_dir(&new_folder).unwrap();
    let _copying = backups::testing::fail_hard_links();
    let (moved, waited) = large.first_event_after("backups_move:progress", || {
        databases_ops::update_backup_settings(
            &large.session,
            &large.machine,
            &BackupSettingsInput {
                enabled: true,
                keep_count: 5,
                location: BackupLocationInput::Custom {
                    path: new_folder.to_string_lossy().into_owned(),
                },
                existing_backups: Some(ExistingBackupsChoice::Move),
            },
        )
    });
    assert_eq!(serde_json::to_value(moved.unwrap().existing_backups).unwrap()["movedCount"], 2);
    assert_progress_in_time("backups_move:progress", waited);
}

#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn the_take_over_check_on_every_save_costs_nothing_measurable_against_the_budgets() {
    let _alone = one_at_a_time();
    let large = LargeDatabase::new();
    const WRITES: u32 = 10_000;

    // An empty write is only the checks: the fingerprint before, and its
    // refresh after (research.md §6).
    let started = Instant::now();
    for _ in 0..WRITES {
        large.session.write(|_| Ok(())).unwrap();
    }
    let per_write = started.elapsed() / WRITES;
    eprintln!("take-over check: {per_write:?} per save over {WRITES} saves");

    // A thousandth of the 1s action budget, and two thousandths of the
    // 500ms search budget, is far below anything a user could notice.
    assert!(
        per_write < Duration::from_millis(1),
        "one save's take-over check took {per_write:?} on average over {WRITES} saves"
    );
}

/// specs/005-regulated-item-types SC-006 (research.md §7, §8): with a share
/// of the collection registered, grouping by the classification and by
/// "Registered to" stays within the 1s action budget, a search for a
/// "Registered to" value within the 500ms search budget, and the form and
/// "Registered to" suggestions within 50ms.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn registration_grouping_search_and_suggestions_complete_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = seeded_firearms();
    db.conn
        .execute_batch(
            "UPDATE firearms SET
                 registration_class_id = 1 + (id % 6),
                 registration_form = CASE id % 5 WHEN 0 THEN 'Form 4' WHEN 1 THEN 'Form 1'
                                     WHEN 2 THEN 'eForm 4' ELSE NULL END,
                 registration_approved = '2026-02-10',
                 registered_to = CASE WHEN id % 50 = 0 THEN 'Smith Family Trust'
                                      ELSE 'Owner ' || (id % 400) END
             WHERE id % 4 = 0",
        )
        .unwrap();

    for group_by in [GroupBy::RegisteredAs, GroupBy::RegisteredTo] {
        let started = Instant::now();
        let result = firearm_ops::list_firearms(
            &db.conn,
            &ListFirearmsInput { group_by: Some(group_by), ..Default::default() },
        )
        .unwrap();
        let elapsed = started.elapsed();
        eprintln!("SC-006: list_firearms grouped by {group_by:?} took {elapsed:?}");
        assert!(result.groups.len() > 1);
        assert!(
            elapsed.as_millis() < BUDGET_MS,
            "list_firearms grouped by {group_by:?} took {}ms, over the {BUDGET_MS}ms budget",
            elapsed.as_millis()
        );
    }

    let started = Instant::now();
    let found = firearm_ops::list_firearms(
        &db.conn,
        &ListFirearmsInput { query: Some("smith family".into()), ..Default::default() },
    )
    .unwrap();
    let elapsed = started.elapsed();
    eprintln!("SC-006: list_firearms registered to search took {elapsed:?}");
    assert!(found.groups.iter().map(|g| g.firearms.len()).sum::<usize>() > 10);
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "list_firearms (registered to search) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );

    for (field, text) in [
        (EntryField::RegistrationForm, "F"),
        (EntryField::RegistrationForm, ""),
        (EntryField::RegisteredTo, "own"),
        (EntryField::RegisteredTo, ""),
    ] {
        let input = SuggestEntriesInput { field, text: text.into(), make: None };
        let started = Instant::now();
        let output = entry_ops::suggest_entries(&db.conn, &input).unwrap();
        let elapsed = started.elapsed();
        eprintln!("SC-006: suggest_entries({field:?}, {text:?}) took {elapsed:?}");
        assert!(!output.suggestions.is_empty());
        assert!(
            elapsed.as_millis() < 50,
            "suggest_entries({field:?}, {text:?}) took {}ms at {RECORD_COUNT} records, over the 50ms budget",
            elapsed.as_millis()
        );
    }
}

// --- Feature 006: accessories and mounts ----------------------------------------

const ACCESSORY_COUNT: usize = 10_000;
/// Chains of five accessories, one on the next, each chain on its own
/// firearm: 996 chains and the dispose family below make 5,000 mounts.
const CHAINS: usize = 996;
const CHAIN_DEPTH: usize = 5;

/// A seeded database for specs/006-accessory-links FR-026 and SC-007 (research.md
/// §22): 10,000 firearms and 10,000 accessories with 5,000 mounts, including
/// chains five deep.
struct MountedScale {
    db: TestDb,
    firearms: Vec<i64>,
    accessories: Vec<i64>,
}

impl MountedScale {
    fn new() -> Self {
        let db = TestDb::new();
        let firearms = seed_10k_firearms(&db.conn);
        let accessories = seed_10k_accessories(&db.conn);
        let scale = Self { db, firearms, accessories };
        scale.seed_mounts();
        scale
    }

    fn firearm(&self, index: usize) -> RecordRef {
        RecordRef::Firearm(self.firearms[index])
    }

    fn accessory(&self, index: usize) -> RecordRef {
        RecordRef::Accessory(self.accessories[index])
    }

    /// Accessory `5c + d` is on accessory `5c + d - 1` (the first on firearm
    /// `c`). Firearm `CHAINS` carries the dispose family: four branches of
    /// five accessories, 20 records below it.
    fn seed_mounts(&self) {
        let tx = self.db.conn.unchecked_transaction().unwrap();
        {
            let mut on_firearm = tx
                .prepare("INSERT INTO mounts (item_accessory_id, host_firearm_id) VALUES (?1, ?2)")
                .unwrap();
            let mut on_accessory = tx
                .prepare(
                    "INSERT INTO mounts (item_accessory_id, host_accessory_id) VALUES (?1, ?2)",
                )
                .unwrap();
            let branches = (0..CHAINS).map(|c| (c, c)).chain((0..4).map(|b| (CHAINS + b, CHAINS)));
            for (branch, firearm_index) in branches {
                let chain = &self.accessories[branch * CHAIN_DEPTH..(branch + 1) * CHAIN_DEPTH];
                on_firearm.execute(params![chain[0], self.firearms[firearm_index]]).unwrap();
                for pair in chain.windows(2) {
                    on_accessory.execute(params![pair[1], pair[0]]).unwrap();
                }
            }
        }
        tx.commit().unwrap();
        let mounts: i64 =
            self.db.conn.query_row("SELECT count(*) FROM mounts", [], |row| row.get(0)).unwrap();
        assert_eq!(mounts, 5_000);
    }
}

fn seed_10k_accessories(conn: &Connection) -> Vec<i64> {
    let makes = ["Leupold", "Trijicon", "SureFire", "Magpul", "Vortex", "Walther", "Midwest"];
    let calibers = [Some("9mm"), Some(".223"), None, Some(".308")];
    let cartridges = [Some("9x19mm Parabellum"), None, Some(".223 Remington")];
    let tx = conn.unchecked_transaction().unwrap();
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO accessories (
                    uid, accessory_kind_id, make, model, serial_number, caliber, cartridge,
                    notes, status, estimated_value, acquisition_source, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'active', ?9, ?10,
                           datetime('now'), datetime('now'))",
            )
            .unwrap();
        for i in 0..ACCESSORY_COUNT {
            stmt.execute(params![
                support::uid(),
                1 + (i % 12) as i64,
                makes[i % makes.len()],
                format!("Acc Model {i}"),
                (i % 4 != 0).then(|| format!("ACC-{i}")),
                calibers[i % calibers.len()],
                cartridges[i % cartridges.len()],
                if i == ACCESSORY_COUNT / 2 {
                    "a uniquely findable note accxyzzy"
                } else {
                    "routine accessory notes"
                },
                (i % 3 == 0).then_some(100 + (i % 900) as i64),
                "Gun show",
            ])
            .unwrap();
        }
    }
    tx.commit().unwrap();
    let mut ids_stmt = conn.prepare("SELECT id FROM accessories ORDER BY id").unwrap();
    let ids: Vec<i64> =
        ids_stmt.query_map([], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(ids.len(), ACCESSORY_COUNT);
    ids
}

/// Runs `run`, prints its time for the pull request's performance note
/// (`--nocapture`), and holds it to `budget_ms`. The budgets are the release
/// build's (DEVELOPMENT.md, "Test"): an unoptimised build, which a plain
/// `cargo test` is, gets three times the budget so that it still catches a
/// scan gone quadratic.
fn within<T>(what: &str, budget_ms: u128, run: impl FnOnce() -> T) -> T {
    let budget_ms = if cfg!(debug_assertions) { budget_ms * 3 } else { budget_ms };
    let started = Instant::now();
    let result = run();
    let elapsed = started.elapsed();
    eprintln!("SC-007: {what} took {elapsed:?} (budget {budget_ms}ms)");
    assert!(
        elapsed.as_millis() < budget_ms,
        "{what} took {}ms at {RECORD_COUNT} firearms and {ACCESSORY_COUNT} accessories; budget \
         is {budget_ms}ms",
        elapsed.as_millis()
    );
    result
}

fn count(output: &ListAccessoriesOutput) -> usize {
    output.groups.iter().map(|g| g.accessories.len()).sum()
}

fn accessory_input(kind: i64, make: &str, model: &str) -> AccessoryInput {
    serde_json::from_value(
        json!({ "accessoryKindId": kind, "make": make, "model": model, "status": "active" }),
    )
    .unwrap()
}

/// The actions of FR-026 (research.md §22): each within the 1s budget.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn mount_and_record_actions_complete_within_the_action_budget_at_10k_plus_10k_records() {
    let _alone = one_at_a_time();
    let scale = MountedScale::new();
    let conn = &scale.db.conn;

    // Opening a record with its chain and the list mounted on it.
    let head = scale.firearm(0).id();
    let detail = within("get_firearm with 5 mounted below", 1_000, || {
        firearm_ops::get_firearm_detail(conn, head).unwrap()
    });
    assert_eq!(detail.mount.mounted.len(), CHAIN_DEPTH);
    let tail = scale.accessory(CHAIN_DEPTH - 1).id();
    let detail = within("get_accessory with a chain of 5", 1_000, || {
        accessory_ops::get_accessory(conn, tail).unwrap()
    });
    assert_eq!(detail.mount.chain.len(), CHAIN_DEPTH);

    // Moving the first accessory of a chain moves the four below it too, and
    // taking it off again leaves them on it.
    let moving = scale.accessory(5 * 10);
    let moved = within("mount_record moving a subtree", 1_000, || {
        let input = json!({ "item": moving, "host": scale.firearm(7_000) });
        mount_ops::mount_record(conn, &serde_json::from_value(input).unwrap()).unwrap()
    });
    assert_eq!(moved.host.map(|h| h.record), Some(scale.firearm(7_000)));
    let detail = firearm_ops::get_firearm_detail(conn, scale.firearm(7_000).id()).unwrap();
    assert_eq!(detail.mount.mounted.len(), CHAIN_DEPTH);
    within("mount_record unmounting", 1_000, || {
        let input = json!({ "item": moving, "host": null });
        mount_ops::mount_record(conn, &serde_json::from_value(input).unwrap()).unwrap()
    });
    let detail = accessory_ops::get_accessory(conn, moving.id()).unwrap();
    assert!(detail.mount.chain.is_empty());
    assert_eq!(detail.mount.mounted.len(), CHAIN_DEPTH - 1);

    // Creating and updating with a host chosen on the form.
    let deepest = scale.accessory(CHAIN_DEPTH - 1);
    let created = within("create_firearm with mountedOn", 1_000, || {
        let input = FirearmInput {
            mounted_on: Some(deepest),
            ..firearm("Mounted Make", "Mounted Model", "PERF-MOUNTED")
        };
        firearm_ops::create_firearm(conn, &input, false, None).unwrap()
    });
    let plain =
        firearm_ops::create_firearm(conn, &firearm("Plain", "Plain", "PERF-PLAIN"), false, None)
            .unwrap();
    within("update_firearm with mountedOn", 1_000, || {
        let input = FirearmInput {
            mounted_on: Some(RecordRef::Firearm(created.id)),
            ..firearm("Plain", "Plain", "PERF-PLAIN")
        };
        firearm_ops::update_firearm(conn, plain.id, &input, false).unwrap()
    });
    let new_accessory = within("create_accessory with mountedOn", 1_000, || {
        let input = AccessoryInput {
            mounted_on: Some(scale.firearm(8_000)),
            ..accessory_input(1, "Leupold", "New Optic")
        };
        accessory_ops::create_accessory(conn, &input, None).unwrap()
    });
    within("update_accessory with mountedOn", 1_000, || {
        let input = AccessoryInput {
            mounted_on: Some(scale.accessory(9_000)),
            ..accessory_input(1, "Leupold", "New Optic")
        };
        accessory_ops::update_accessory(conn, new_accessory.id, &input).unwrap()
    });

    // Disposing of a host with 20 records below, all disposed with it.
    let host = scale.firearm(CHAINS).id();
    let below = firearm_ops::get_firearm_detail(conn, host).unwrap().mount.mounted;
    assert_eq!(below.len(), 20);
    let with: Vec<_> =
        below.iter().map(|m| json!({ "record": m.label.record, "price": 10 })).collect();
    let dispose: DisposeInput = serde_json::from_value(json!({
        "dispositionType": "sold", "recipient": "Jane Doe", "date": "2025-06-15",
        "price": 400, "withMounted": with,
    }))
    .unwrap();
    within("dispose_firearm with 20 records below", 1_000, || {
        firearm_ops::dispose_firearm(conn, host, &dispose).unwrap()
    });
    let disposed: i64 = conn
        .query_row("SELECT count(*) FROM accessories WHERE status = 'disposed'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(disposed, 20);
}

/// The browse and search operations of FR-026 (research.md §22): each within
/// the 500ms budget.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn accessory_listing_grouping_and_candidates_complete_within_budget_at_10k_plus_10k_records() {
    let _alone = one_at_a_time();
    let scale = MountedScale::new();
    let conn = &scale.db.conn;

    let all = within("list_accessories (no filter)", BUDGET_MS, || {
        accessory_ops::list_accessories(conn, &ListAccessoriesInput::default()).unwrap()
    });
    assert_eq!(count(&all), ACCESSORY_COUNT);

    // The three-character threshold: FTS5 above it, LIKE at one or two.
    for (query, expected) in [("accxyzzy", Some(1)), ("Leupold", None), ("Acc Model 4242", None)] {
        let found = within(&format!("list_accessories search {query:?}"), BUDGET_MS, || {
            accessory_ops::list_accessories(
                conn,
                &ListAccessoriesInput { query: Some(query.into()), ..Default::default() },
            )
            .unwrap()
        });
        assert!(count(&found) >= 1, "{query:?} should match");
        if let Some(expected) = expected {
            assert_eq!(count(&found), expected);
        }
    }
    let short = within("list_accessories search \"ac\" (LIKE)", BUDGET_MS, || {
        accessory_ops::list_accessories(
            conn,
            &ListAccessoriesInput { query: Some("ac".into()), ..Default::default() },
        )
        .unwrap()
    });
    assert!(count(&short) > 1_000);

    for group_by in [
        AccessoryGroupBy::Kind,
        AccessoryGroupBy::Make,
        AccessoryGroupBy::Caliber,
        AccessoryGroupBy::Cartridge,
        AccessoryGroupBy::MountedOn,
    ] {
        let grouped =
            within(&format!("list_accessories grouped by {group_by:?}"), BUDGET_MS, || {
                accessory_ops::list_accessories(
                    conn,
                    &ListAccessoriesInput { group_by: Some(group_by), ..Default::default() },
                )
                .unwrap()
            });
        assert_eq!(count(&grouped), ACCESSORY_COUNT);
        if group_by == AccessoryGroupBy::MountedOn {
            // One group for each host (996 firearms, and the accessories
            // that carry others) and "Not mounted" last.
            assert!(grouped.groups.len() > 4_000, "{} groups", grouped.groups.len());
            assert_eq!(grouped.groups.last().map(|g| g.key.as_str()), Some("Not mounted"));
        }
    }

    // The collection page with each firearm's host and count of mounted records.
    let firearms = within("list_firearms with mountedOn and mountedCounts", BUDGET_MS, || {
        firearm_ops::list_firearms(conn, &ListFirearmsInput::default()).unwrap()
    });
    let rows: Vec<_> = firearms.groups.iter().flat_map(|g| &g.firearms).collect();
    assert_eq!(rows.len(), RECORD_COUNT);
    let head = rows.iter().find(|f| f.id == scale.firearm(0).id()).unwrap();
    assert_eq!(head.mounted_counts.total(), CHAIN_DEPTH);
    // Searched, with each hit's host and mounted count (FR-026, SC-007): FTS5
    // at three characters or more, LIKE at one or two.
    for (query, expected) in [("xyzzy", Some(1)), ("Nick 4242", Some(1)), ("gl", None)] {
        let found = within(&format!("list_firearms search {query:?}"), BUDGET_MS, || {
            firearm_ops::list_firearms(
                conn,
                &ListFirearmsInput { query: Some(query.into()), ..Default::default() },
            )
            .unwrap()
        });
        let hits: usize = found.groups.iter().map(|g| g.firearms.len()).sum();
        assert!(hits >= 1, "{query:?} should match");
        if let Some(expected) = expected {
            assert_eq!(hits, expected, "{query:?}");
        }
    }
    let grouped = within("list_firearms grouped, with mount details", BUDGET_MS, || {
        firearm_ops::list_firearms(
            conn,
            &ListFirearmsInput { group_by: Some(GroupBy::Caliber), ..Default::default() },
        )
        .unwrap()
    });
    assert!(grouped.groups.len() > 1);

    // The choices for a host and for an item, with and without a query.
    for (what, input) in [
        (
            "hosts for an accessory",
            json!({ "role": "host", "record": scale.accessory(5), "query": "" }),
        ),
        (
            "hosts for an accessory, searched",
            json!({ "role": "host", "record": scale.accessory(5), "query": "Model 12" }),
        ),
        ("hosts for a new record", json!({ "role": "host", "query": "Glock" })),
        ("items for a firearm", json!({ "role": "item", "record": scale.firearm(3), "query": "" })),
        (
            "items for a firearm, searched",
            json!({ "role": "item", "record": scale.firearm(3), "query": "PERF-47" }),
        ),
        (
            "items for an accessory, short query",
            json!({ "role": "item", "record": scale.accessory(2), "query": "a" }),
        ),
    ] {
        let found = within(&format!("list_mount_candidates, {what}"), BUDGET_MS, || {
            mount_ops::list_mount_candidates(conn, &serde_json::from_value(input).unwrap()).unwrap()
        });
        assert!(!found.candidates.is_empty(), "{what} found nothing");
        assert!(found.candidates.len() <= 50);
    }
}

/// research.md §22: the suggestion lists now read both tables, still within
/// the 50ms of SC-004.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn suggest_entries_over_both_tables_completes_within_50ms_at_10k_plus_10k_records() {
    let _alone = one_at_a_time();
    let scale = MountedScale::new();

    for (field, text, make) in [
        (EntryField::Make, "le", None),
        (EntryField::Make, "", None),
        (EntryField::Model, "acc model 12", Some("Leupold")),
        (EntryField::Model, "", None),
        (EntryField::Cartridge, "9", None),
        (EntryField::Cartridge, "", None),
        (EntryField::Caliber, ".2", None),
    ] {
        let input = SuggestEntriesInput { field, text: text.into(), make: make.map(str::to_owned) };
        let output = within(&format!("suggest_entries({field:?}, {text:?})"), 50, || {
            entry_ops::suggest_entries(&scale.db.conn, &input).unwrap()
        });
        assert!(!output.suggestions.is_empty());
    }
}

// --- Feature 007: the document preview ------------------------------------------

const DOCUMENT_COUNT: usize = 30_000;
const TEN_MB: usize = 10 * 1024 * 1024;
/// SC-001's budget for showing a document.
const PREVIEW_BUDGET_MS: u128 = 1_000;

/// An open session over a database of 10,000 firearms and 10,000 accessories
/// (SC-001's collection size), with the recording surface and the real
/// helper of the preview tests. Seeded through the session, as it is the
/// preview's own.
fn document_scale() -> Fixture {
    let mut world = Fixture::new();
    // The fixture's own firearm gives way to the seeded ones (the seed holds
    // the count at exactly 10,000), and documents attach to the first.
    let fixture_firearm = world.firearm;
    world.firearm = world
        .session
        .write(|conn| {
            conn.execute("DELETE FROM firearms WHERE id = ?1", [fixture_firearm]).unwrap();
            let firearms = seed_10k_firearms(conn);
            seed_10k_accessories(conn);
            Ok(firearms[0])
        })
        .unwrap();
    world
}

/// 30,000 small documents, two on a firearm for every one on an accessory,
/// with names like a collection's ("2021 Appraisal 4711.pdf"), and one
/// findable name on each side. One KiB each: the table's weight in rows and
/// in `document_names_fts` is what a name search reads, not the bytes.
fn seed_30k_documents(conn: &Connection) {
    let kinds = [
        "Receipt",
        "Appraisal",
        "Insurance rider",
        "Transfer form 4473",
        "Manual",
        "Warranty card",
        "Photo log",
        "Registration",
    ];
    let firearms: Vec<i64> = conn
        .prepare("SELECT id FROM firearms ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let accessories: Vec<i64> = conn
        .prepare("SELECT id FROM accessories ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let bytes = vec![b'%'; 1024];
    let tx = conn.unchecked_transaction().unwrap();
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO document_attachments
                    (firearm_id, accessory_id, file_bytes, original_filename, mime_type, created_at)
                 VALUES (?1, ?2, ?3, ?4, 'application/pdf', datetime('now'))",
            )
            .unwrap();
        for i in 0..DOCUMENT_COUNT {
            let on_accessory = i % 3 == 0;
            let owner = if on_accessory {
                accessories[(i / 3) % accessories.len()]
            } else {
                firearms[i % firearms.len()]
            };
            let name = match i {
                12_346 => "zebraquartz bill of sale.pdf".to_string(),
                23_457 => "quixoticmarmot.pdf".to_string(),
                _ => format!("{} {} {i}.pdf", 2000 + i % 25, kinds[i % kinds.len()]),
            };
            stmt.execute(params![
                (!on_accessory).then_some(owner),
                on_accessory.then_some(owner),
                bytes,
                name
            ])
            .unwrap();
        }
    }
    tx.commit().unwrap();
    let held: i64 =
        conn.query_row("SELECT count(*) FROM document_attachments", [], |row| row.get(0)).unwrap();
    assert_eq!(held, DOCUMENT_COUNT as i64);
}

/// Runs `run` and prints its time for the pull request's performance note,
/// like [`within`], but gives the time back so that a row made of several
/// steps is held to its budget as a whole. `SC-001`'s budgets are the
/// release build's, with `within`'s three times for an unoptimised one.
fn timed<T>(what: &str, run: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let result = run();
    let elapsed = started.elapsed();
    eprintln!("SC-001: {what} took {elapsed:?} (budget {PREVIEW_BUDGET_MS}ms)");
    (result, elapsed)
}

fn assert_within_preview_budget(what: &str, elapsed: Duration) {
    let budget_ms = if cfg!(debug_assertions) { PREVIEW_BUDGET_MS * 3 } else { PREVIEW_BUDGET_MS };
    assert!(
        elapsed.as_millis() < budget_ms,
        "{what} took {}ms at {RECORD_COUNT} firearms and {ACCESSORY_COUNT} accessories; budget \
         is {budget_ms}ms",
        elapsed.as_millis()
    );
}

/// A one-page LZW-compressed 8-bit grayscale TIFF at 300 DPI of at least
/// `min_bytes`: letter size (2550 × 3300) of noise, which LZW hardly shrinks,
/// made taller only if it comes out smaller than asked (research.md §22's
/// "decode page 1 (LZW or G4, 300 DPI letter)").
fn lzw_letter_tiff(min_bytes: usize) -> Vec<u8> {
    use tiff::encoder::colortype::Gray8;
    use tiff::encoder::{Compression, Rational, TiffEncoder};
    use tiff::tags::ResolutionUnit;

    let width = 2550u32;
    let mut height = 3300u32;
    loop {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let pixels: Vec<u8> = (0..width as usize * height as usize)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 32) as u8
            })
            .collect();
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut encoder =
                TiffEncoder::new(&mut out).unwrap().with_compression(Compression::Lzw);
            let mut image = encoder.new_image::<Gray8>(width, height).unwrap();
            image.resolution(ResolutionUnit::Inch, Rational { n: 300, d: 1 });
            image.write_data(&pixels).unwrap();
        }
        let bytes = out.into_inner();
        if bytes.len() >= min_bytes {
            return bytes;
        }
        height = (height as u64 * min_bytes as u64 * 21 / 20 / bytes.len() as u64) as u32;
    }
}

/// A valid one-page PDF of at least `min_bytes`: its page's content stream
/// carries comment lines (the size of a scan embedded in one).
fn large_pdf(min_bytes: usize) -> Vec<u8> {
    let mut content = b"BT /F1 12 Tf 72 700 Td (A 10 MB appraisal) Tj ET\n".to_vec();
    let line = b"% padding padding padding padding padding padding padding padding\n";
    while content.len() < min_bytes {
        content.extend_from_slice(line);
    }
    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 5 0 R \
          /Resources << /Font << /F1 4 0 R >> >> >>"
            .to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ];
    let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
    stream.extend(content);
    stream.extend_from_slice(b"\nendstream");
    objects.push(stream);
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend(format!("{} 0 obj\n", i + 1).into_bytes());
        pdf.extend(body);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).into_bytes());
    for offset in offsets {
        pdf.extend(format!("{offset:010} 00000 n \n").into_bytes());
    }
    pdf.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .into_bytes(),
    );
    pdf
}

/// At least `min_bytes` of text lines, as a spreadsheet export or a typed
/// list would be: CR LF line ends, accented letters (`utf8`) or, when not,
/// a stray 0xE9 near the start that makes the whole text Windows-1252.
fn large_text(min_bytes: usize, utf8: bool) -> Vec<u8> {
    let mut text = String::new();
    let mut i = 0;
    while text.len() < min_bytes {
        text.push_str(&format!(
            "{i},Colt,1911,caf\u{e9} receipt,\"$1,250.00\",2021-05-0{}\r\n",
            i % 9 + 1
        ));
        i += 1;
    }
    if utf8 {
        text.into_bytes()
    } else {
        let mut bytes = b"caf\xE9,".to_vec();
        bytes.extend(
            text.chars()
                .map(|c| if c == '\u{e9}' { 'e' } else { c })
                .collect::<String>()
                .into_bytes(),
        );
        bytes
    }
}

/// research.md §22 "open_preview -> page 1, 10 MB TIFF": the helper started,
/// the TIFF piped to it and its first page rendered, as the viewer asks for
/// it, within 1 s. The real helper binary and a 300 DPI letter page in LZW.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn open_preview_of_a_10_mb_tiff_shows_page_one_within_a_second_at_10k_plus_10k_records() {
    let _alone = one_at_a_time();
    let world = document_scale();
    let tiff = lzw_letter_tiff(TEN_MB);
    assert!(tiff.len() >= TEN_MB, "{} bytes", tiff.len());
    let id = world.add("scan.tif", &tiff);

    let (info, opened) = timed("open_preview of a 10 MB TIFF", || world.open_ok(id));
    assert_eq!(info["kind"], "tiff");
    let preview_id = Fixture::id_of(&info);
    // The first screen asks for page 1 at the width of the viewer's column.
    let (png, rendered) = timed("render_preview_page of its page 1 (1400 px wide)", || {
        hoplodex_lib::commands::preview::ops::render_preview_page(
            &world.session,
            preview_id,
            0,
            1400,
        )
        .unwrap()
    });
    assert_eq!(preview_support::png_size(&png).0, 1400);
    eprintln!("SC-001: a 10 MB TIFF, open and page 1, took {:?} in all", opened + rendered);
    assert_within_preview_budget("open_preview and page 1 of a 10 MB TIFF", opened + rendered);
    world.close(preview_id);
}

/// The `hdpreview` handler, asked for `url` as the surface asks.
fn get(world: &Fixture, url: &str) -> tauri::http::Response<Vec<u8>> {
    let request = tauri::http::Request::builder().method("GET").uri(url).body(Vec::new()).unwrap();
    protocol_handler::handle(&world.session, &request)
}

/// research.md §22 "open_preview -> PDF surface shown, 10 MB": what Rust does
/// up to the surface showing it, with the recording surface and the
/// `hdpreview` handler serving the document to it. The web view's own
/// creation and first paint are the surface check's and the us13 E2E's.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn open_preview_of_a_10_mb_pdf_is_served_to_its_surface_within_a_second_at_10k_plus_10k_records() {
    let _alone = one_at_a_time();
    let world = document_scale();
    let pdf = large_pdf(TEN_MB);
    assert!(pdf.len() >= TEN_MB);
    let id = world.add("scan.pdf", &pdf);

    let ((info, served), elapsed) =
        timed("open_preview of a 10 MB PDF, served to the surface", || {
            let info = world.open_ok(id);
            let surface = world.preview.surfaces.only();
            let served = get(&world, &surface.url());
            // On Linux the surface's frame script also reports PDF.js's hook,
            // which is what makes the preview ready there.
            if cfg!(target_os = "linux") {
                let mut hook = tauri::Url::parse(&surface.url()).unwrap();
                hook.set_path(&format!("/hooked/{}", surface.secret()));
                assert_eq!(get(&world, hook.as_str()).status().as_u16(), 204);
            }
            (info, served)
        });
    assert_eq!(info["kind"], "pdf");
    assert_eq!(served.status().as_u16(), 200);
    assert_eq!(served.body().len(), pdf.len(), "the whole document is served");
    assert_eq!(world.events.payloads("preview:pdf-ready").len(), 1);
    assert_within_preview_budget("open_preview of a 10 MB PDF up to its surface", elapsed);
}

/// research.md §22 "10 MB text shown": the decode of a 10 MB text, by each of
/// the two encodings it can take (UTF-8, and the Windows-1252 fallback
/// after a failed validation), and `open_preview` of a text of that size,
/// whose answer is serialised as the viewer receives it.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn a_10_mb_text_is_decoded_and_opened_within_a_second_at_10k_plus_10k_records() {
    let _alone = one_at_a_time();
    for (what, utf8) in [("UTF-8", true), ("Windows-1252", false)] {
        let bytes = large_text(TEN_MB, utf8);
        let (text, decoded) =
            timed(&format!("decode of a 10 MB {what} text"), || preview_text::decode(&bytes));
        assert!(text.len() >= TEN_MB * 9 / 10);
        assert!(!text.contains('\r'), "CR LF is read as a line feed");
        assert_within_preview_budget(&format!("decoding a 10 MB {what} text"), decoded);
    }

    let world = document_scale();
    let id = world.add("export.txt", &large_text(TEN_MB, true));
    let (info, opened) = timed("open_preview of a 10 MB text", || world.open_ok(id));
    assert_eq!(info["kind"], "text");
    assert!(info["text"].as_str().is_some_and(|text| text.len() >= TEN_MB * 9 / 10));
    assert_within_preview_budget("open_preview of a 10 MB text", opened);
}

/// SC-007, research.md §22: a search finds a record by the name of its own
/// document within 500 ms at 10,000 firearms, 10,000 accessories and 30,000
/// documents, for the trigram index (three characters or more) and for the
/// `LIKE` of one or two. Two 10 MB documents sit on the first firearm, so
/// that reading a name beside a large file's bytes is in the timing too.
#[test]
#[ignore = "release only: see DEVELOPMENT.md"]
fn searching_a_document_name_completes_within_budget_at_10k_plus_10k_records_and_30k_documents() {
    let _alone = one_at_a_time();
    let world = document_scale();
    world
        .session
        .write(|conn| {
            seed_30k_documents(conn);
            Ok(())
        })
        .unwrap();
    world.add("big appraisal.pdf", &large_pdf(TEN_MB));
    world.add("big scan.tif", &lzw_letter_tiff(TEN_MB));
    let session = &world.session;

    let firearms = |query: &str| {
        let input = ListFirearmsInput { query: Some(query.into()), ..Default::default() };
        within(&format!("list_firearms search {query:?} (a document name)"), BUDGET_MS, || {
            session.read(|conn| firearm_ops::list_firearms(conn, &input)).unwrap()
        })
        .groups
        .iter()
        .map(|group| group.firearms.len())
        .sum::<usize>()
    };
    let accessories = |query: &str| {
        let input = ListAccessoriesInput { query: Some(query.into()), ..Default::default() };
        within(&format!("list_accessories search {query:?} (a document name)"), BUDGET_MS, || {
            session.read(|conn| accessory_ops::list_accessories(conn, &input)).unwrap()
        })
        .groups
        .iter()
        .map(|group| group.accessories.len())
        .sum::<usize>()
    };

    // One record has the name: the trigram index, then the one-or-two
    // character `LIKE`.
    assert_eq!(firearms("zebraquartz"), 1);
    assert_eq!(accessories("quixoticmarmot"), 1);
    assert_eq!(firearms("zebraquartz bill"), 1);
    assert!(firearms("zq") <= 1);
    assert!(firearms("ze") >= 1);
    assert_eq!(accessories("qx"), 0);
    assert!(accessories("qu") >= 1);
    // A name most records' documents share, which is every record that has
    // one and every kind of the seeded names.
    assert!(firearms("appraisal") > 1_000);
    assert!(accessories("appraisal") > 300);
    assert!(firearms("big app") >= 1);
    assert!(firearms("ap") > 1_000);
    assert!(accessories("ap") > 300);
}
