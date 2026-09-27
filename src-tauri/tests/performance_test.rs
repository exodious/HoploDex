//! Performance validation (constitution Principle IV, spec.md's tested
//! scale up to 10,000 firearm records): `list_firearms` and
//! `get_value_summary` must return within the 500ms search/browse budget
//! at that scale, backed by the FTS5 index and indexed columns from
//! data-model.md — no mocks, a real temporary SQLCipher database.
//!
//! Feature 003 adds opening a database, key derivation included, to the 1s
//! action budget (SC-003), the first progress event of a backup, a
//! passphrase change, a restore and a move of backups to the 100ms feedback
//! budget (SC-005), and the take-over check every save makes (research.md
//! §6).

mod support;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops::{self as databases_ops, Unlock};
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::firearms::{GroupBy, ListFirearmsInput};
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::database::{
    BackupLocationInput, BackupOutcome, BackupSettingsInput, CloseReason, ExistingBackupsChoice,
};
use hoplodex_lib::models::firearm::{FirearmInput, Origin};
use hoplodex_lib::services::backups;
use hoplodex_lib::services::insurance_status::InsuranceWarning;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::valuation::get_value_summary;
use hoplodex_lib::session::{Session, lifecycle};
use rusqlite::{Connection, params};
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

    let tx = conn.unchecked_transaction().unwrap();
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO firearms (
                    make, model, serial_number, no_serial_attested, caliber, firearm_type_id,
                    notes, nickname, barrel_length_hundredths, overall_length_hundredths,
                    weight_tenths_oz, capacity, finish, condition,
                    status, origin, year_of_manufacture, country_of_manufacture,
                    importer_name, original_make, original_model, original_serial_number,
                    created_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                    'active', ?14, ?15, ?16, ?17, ?18, ?19, ?20, datetime('now'), datetime('now')
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
            ])
            .unwrap();
        }
    }
    tx.commit().unwrap();

    let mut ids_stmt = conn.prepare("SELECT id FROM firearms").unwrap();
    let ids: Vec<i64> =
        ids_stmt.query_map([], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(ids.len(), RECORD_COUNT);
    ids
}

#[test]
fn list_firearms_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = TestDb::new();
    seed_10k_firearms(&db.conn);

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
fn list_firearms_search_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = TestDb::new();
    seed_10k_firearms(&db.conn);

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
fn list_firearms_finish_search_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = TestDb::new();
    seed_10k_firearms(&db.conn);

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
fn list_firearms_grouped_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = TestDb::new();
    seed_10k_firearms(&db.conn);

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
        ..firearm("Distinctive Make", "Distinctive Model", "PERF-NEW-MAIN")
    };
    let started = Instant::now();
    firearm_ops::create_firearm(&db.conn, &input, false).unwrap();
    let elapsed = started.elapsed();
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "create_firearm with original marks (FR-009 lookup) took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );
}

#[test]
fn list_firearms_nickname_search_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = TestDb::new();
    seed_10k_firearms(&db.conn);

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
fn get_value_summary_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = TestDb::new();
    let ids = seed_10k_firearms(&db.conn);

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
fn list_firearms_with_blanket_coverage_completes_within_budget_at_10k_records() {
    let _alone = one_at_a_time();
    let db = TestDb::new();
    seed_10k_firearms(&db.conn);
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
fn deleting_a_firearm_completes_within_the_interactive_budget_at_10k_records() {
    let _alone = one_at_a_time();
    // Deleting vacuums the file to return the freed space (Constitution V),
    // which must stay inside the 1s completion budget (Constitution IV).
    let db = TestDb::new();
    let ids = seed_10k_firearms(&db.conn);

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
