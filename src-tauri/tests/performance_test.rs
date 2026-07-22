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
use hoplodex_lib::models::firearm::CoverageKind;
use hoplodex_lib::models::insurance_policy::InsurancePolicyInput;
use hoplodex_lib::services::valuation::get_value_summary;
use rusqlite::params;
use support::TestDb;

const RECORD_COUNT: usize = 10_000;
const BUDGET_MS: u128 = 500;

fn seed_10k_firearms(db: &TestDb) -> Vec<i64> {
    let makes = ["Glock", "Sig", "Ruger", "Smith & Wesson", "Colt"];
    let calibers = ["9mm", ".45 ACP", ".22 LR", ".223", ".308"];
    let types = [1, 2, 3, 4];

    let tx = db.conn.unchecked_transaction().unwrap();
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO firearms (
                    make, model, serial_number, no_serial_attested, caliber, firearm_type_id,
                    notes, status, created_at, updated_at
                ) VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, 'active', datetime('now'), datetime('now'))",
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
            stmt.execute(params![
                make,
                format!("Model {i}"),
                serial,
                caliber,
                firearm_type_id,
                notes
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
}

#[test]
fn get_value_summary_completes_within_budget_at_10k_records() {
    let db = TestDb::new();
    let ids = seed_10k_firearms(&db);

    // Give a meaningful fraction of firearms an estimated value and
    // insurance coverage so the summary computation has real aggregation
    // work to do, not just zeros.
    let policy = insurance_ops::create_policy(
        &db.conn,
        &InsurancePolicyInput {
            name: "Perf Policy".into(),
            policy_number: "PERF-1".into(),
            insurance_company: "Acme".into(),
            company_contact: None,
            agent_name: None,
            agent_contact: None,
            blanket_coverage_limit: 100_000_000,
            effective_start_date: "2020-01-01".into(),
            effective_end_date: "2035-01-01".into(),
        },
    )
    .unwrap();

    db.conn.execute_batch("UPDATE firearms SET estimated_value = 50000 WHERE id % 3 = 0").unwrap();
    for &id in ids.iter().step_by(5) {
        insurance_ops::assign_firearm_coverage(
            &db.conn,
            id,
            Some(policy.id),
            Some(CoverageKind::Blanket),
            None,
        )
        .unwrap();
    }

    let started = Instant::now();
    let summary = get_value_summary(&db.conn).unwrap();
    let elapsed = started.elapsed();

    assert!(summary.collection_total > 0);
    assert!(
        elapsed.as_millis() < BUDGET_MS,
        "get_value_summary took {}ms, over the {BUDGET_MS}ms budget",
        elapsed.as_millis()
    );
}
