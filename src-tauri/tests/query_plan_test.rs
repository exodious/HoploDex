//! The duplicate checks use an index (specs/008-hardening-batch research.md
//! §19). An import checks every row against the collection, and a query
//! that scans every firearm made 3,000 + 3,000 rows take 73 s; a rewrite
//! that stops the planner using the index brings that back without failing
//! any behaviour test, so this one reads the plan.

use crate::support;

use hoplodex_lib::commands::firearms::ops;
use hoplodex_lib::services::import_matching::FIND_MATCH_SQL;
use rusqlite::Connection;
use support::{TestDb, firearm};

/// The `detail` of each row of `EXPLAIN QUERY PLAN <sql>`.
fn plan(conn: &Connection, sql: &str) -> Vec<String> {
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
    // Every parameter bound (to a value the query can't match): the plan
    // doesn't depend on them, but SQLite wants each bound.
    let params = vec![rusqlite::types::Value::Text("x".into()); stmt.parameter_count()];
    stmt.query_map(rusqlite::params_from_iter(params), |row| row.get::<_, String>(3))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn assert_searches_index(conn: &Connection, name: &str, sql: &str, index: &str) {
    let steps = plan(conn, sql);
    assert!(
        steps.iter().any(|s| s.contains("SEARCH firearms USING INDEX") && s.contains(index)),
        "{name} should search `{index}`, the plan was {steps:?}"
    );
    assert!(
        !steps.iter().any(|s| s.contains("SCAN firearms")),
        "{name} scans every firearm: {steps:?}"
    );
}

#[test]
fn the_import_match_and_the_duplicate_checks_use_an_index() {
    let db = TestDb::new();
    for n in 0..20 {
        ops::create_firearm(&db.conn, &firearm("Glock", "19", &format!("S{n}")), false, None)
            .unwrap();
    }

    assert_searches_index(&db.conn, "find_match", FIND_MATCH_SQL, "idx_firearms_identity_key");
    assert_searches_index(
        &db.conn,
        "find_identity_clash",
        ops::IDENTITY_CLASH_SQL,
        "idx_firearms_identity_key",
    );
    assert_searches_index(
        &db.conn,
        "find_nickname_clash",
        ops::NICKNAME_CLASH_SQL,
        "idx_firearms_active_nickname",
    );
}
