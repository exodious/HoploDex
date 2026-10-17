//! The duplicate checks use an index (008 research.md
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

// ---- 009 US2: policy names (009 FR-006, FR-007, 009 research.md §2) ----

fn assert_searches_policy_name_index(conn: &Connection, name: &str, sql: &str) {
    let steps = plan(conn, sql);
    assert!(
        steps.iter().any(|s| s.contains("SEARCH insurance_policies USING INDEX")
            && s.contains("idx_insurance_policies_name")),
        "{name} should search `idx_insurance_policies_name`, the plan was {steps:?}"
    );
    assert!(
        !steps.iter().any(|s| s.contains("SCAN insurance_policies")),
        "{name} scans every policy: {steps:?}"
    );
}

/// `sql` with each run of whitespace made one space, so a source file's line
/// breaks and indentation don't matter.
fn flat(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The statements are inline in their functions rather than named constants,
/// so this test reaches them by text: it plans the statement as the
/// contract gives it (009 contracts/tauri-commands.md) and fails when the
/// source no longer holds that text, so the plan checked is the plan run.
fn assert_in_source(source: &str, file: &str, sql: &str) {
    assert!(
        flat(source).contains(&flat(sql)),
        "{file} no longer holds `{}`; this test and the code must say the same",
        flat(sql)
    );
}

#[test]
fn the_policy_name_check_and_import_lookups_use_the_name_index() {
    const NAME_CLASH_SQL: &str = "SELECT name FROM insurance_policies
        WHERE trim(name) = trim(?1) COLLATE NOCASE AND id IS NOT ?2";
    const IMPORT_LOOKUP_SQL: &str =
        "SELECT id FROM insurance_policies WHERE trim(name) = trim(:name) COLLATE NOCASE";

    let db = TestDb::new();
    for n in 0..20 {
        hoplodex_lib::commands::insurance::ops::create_policy(
            &db.conn,
            &support::policy(&format!("Policy {n}"), "2020-01-01", "2099-01-01", None),
        )
        .unwrap();
    }

    assert_searches_policy_name_index(&db.conn, "the form's name check", NAME_CLASH_SQL);
    assert_searches_policy_name_index(&db.conn, "import's lookup", IMPORT_LOOKUP_SQL);

    assert_in_source(include_str!("../src/commands/insurance.rs"), "insurance.rs", NAME_CLASH_SQL);
    let import = include_str!("../src/commands/import_export.rs");
    assert_eq!(
        flat(import).matches(&flat(IMPORT_LOOKUP_SQL)).count(),
        2,
        "import_export.rs holds the lookup once for the firearm row and once for the accessory row"
    );
}
