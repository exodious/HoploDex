//! Performance validation (constitution Principle IV, spec.md's tested
//! scale up to 10,000 firearm records): `list_firearms` and
//! `get_value_summary` must return within the 500ms search/browse budget
//! at that scale, backed by the FTS5 index and indexed columns from
//! data-model.md — no mocks, a real temporary SQLCipher database.

mod support;

use std::time::Instant;

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::firearms::{GroupBy, ListFirearmsInput};
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::firearm::{FirearmInput, Origin};
use hoplodex_lib::services::insurance_status::InsuranceWarning;
use hoplodex_lib::services::valuation::get_value_summary;
use rusqlite::params;
use support::{firearm, policy, TestDb};

const RECORD_COUNT: usize = 10_000;
const BUDGET_MS: u128 = 500;

fn seed_10k_firearms(db: &TestDb) -> Vec<i64> {
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

    let tx = db.conn.unchecked_transaction().unwrap();
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

    let mut ids_stmt = db.conn.prepare("SELECT id FROM firearms").unwrap();
    let ids: Vec<i64> =
        ids_stmt.query_map([], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(ids.len(), RECORD_COUNT);
    ids
}

#[test]
fn list_firearms_completes_within_budget_at_10k_records() {
    let db = TestDb::new();
    seed_10k_firearms(&db);

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
    let db = TestDb::new();
    seed_10k_firearms(&db);

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
    let db = TestDb::new();
    seed_10k_firearms(&db);

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
    let db = TestDb::new();
    seed_10k_firearms(&db);

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
    let db = TestDb::new();
    seed_10k_firearms(&db);

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
    let db = TestDb::new();
    let ids = seed_10k_firearms(&db);

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
    let db = TestDb::new();
    seed_10k_firearms(&db);
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
    // Deleting vacuums the file to return the freed space (Constitution V),
    // which must stay inside the 1s completion budget (Constitution IV).
    let db = TestDb::new();
    let ids = seed_10k_firearms(&db);

    let started = Instant::now();
    firearm_ops::delete_firearm(&db.conn, ids[RECORD_COUNT / 2], true).unwrap();
    let elapsed = started.elapsed();

    assert!(
        elapsed.as_millis() < 1_000,
        "delete (with vacuum) took {}ms at {RECORD_COUNT} records; budget is 1000ms",
        elapsed.as_millis()
    );
}
