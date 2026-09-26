//! Keeps the human-testing seed (`examples/human_seed.rs`, run by
//! `scripts/human-testing.sh`) in step with the data model. The seed builds
//! its records through the same `ops` layer as the app, so a new column
//! compiles fine and quietly stays empty; a person testing by hand would then
//! never see the feature. This runs the real seed into a temporary sandbox
//! and fails when any column is empty in every row of both seeded databases,
//! still at its default in every row, or never holds one of the values a
//! `CHECK ... IN` allows, and when an import sample leaves a spreadsheet
//! column blank. Both databases count together: the single-row tables
//! (`collection_settings`, `app_state`, `pending_changes`) can hold only one
//! value per database.
//!
//! When it fails, seed a record that uses the new field in
//! `examples/human_seed.rs` (and add it to the import samples there). Only
//! list something in `NEVER_SEEDED` when it truly cannot be seeded, and say why.

use std::collections::BTreeSet;

use hoplodex_lib::db;
use hoplodex_lib::services::machine_settings::MachineIdentity;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::spreadsheet::COLUMNS;
use rusqlite::Connection;
use tempfile::TempDir;

#[allow(dead_code)]
#[path = "../examples/human_seed.rs"]
mod human_seed;

/// `(table, column)` pairs the seed cannot populate, each with the reason.
const NEVER_SEEDED: &[(&str, &str)] = &[
    // A single-row table: its id is always its default, 1.
    ("collection_settings", "id"),
];

/// `(table, column)` pairs whose allowed values need not all appear:
/// - a retained disposition (FR-033) uses the values
///   `firearms.disposition_type` is checked against;
/// - `pending_changes` holds at most one row per database (FR-039), and the
///   forms each kind and mode come from are seeded as ordinary records.
const PARTIAL_VALUES_OK: &[(&str, &str)] = &[
    ("disposition_history", "disposition_type"),
    ("pending_changes", "kind"),
    ("pending_changes", "mode"),
];

/// Spreadsheet columns no import sample needs: `photo_filenames` is written
/// on export and ignored on import (FR-019).
const NOT_IMPORTED: &[&str] = &["photo_filenames"];

/// Tables that hold no user data: migration bookkeeping, the seeded lookup
/// list, and the full-text index's shadow tables.
fn is_user_table(name: &str) -> bool {
    !(name.starts_with("sqlite_")
        || name.starts_with("firearms_fts")
        || name == "schema_migrations"
        || name == "firearm_types")
}

struct Column {
    name: String,
    /// The column's `DEFAULT`, as SQL text.
    default: Option<String>,
}

fn user_tables(conn: &Connection) -> Vec<(String, String)> {
    let mut stmt = conn
        .prepare("SELECT name, sql FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap();
    stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .filter(|(name, _)| is_user_table(name))
        .collect()
}

fn columns(conn: &Connection, table: &str) -> Vec<Column> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).unwrap();
    stmt.query_map([], |row| Ok(Column { name: row.get(1)?, default: row.get(4)? }))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// The values, as SQL literals, that a `CHECK (<column> IN ('a', 'b', ...))`
/// or `CHECK (<column> IN (0, 1))` allows, if the table's SQL has one for
/// this column.
fn allowed_values(table_sql: &str, column: &str) -> Vec<String> {
    let needle = format!("{column} IN (");
    let Some(start) = table_sql.find(&needle) else {
        return Vec::new();
    };
    let list = &table_sql[start + needle.len()..];
    let list = &list[..list.find(')').unwrap_or(list.len())];
    list.split(',')
        .map(str::trim)
        .filter(|item| {
            (item.starts_with('\'') && item.ends_with('\'')) || item.parse::<i64>().is_ok()
        })
        .map(str::to_owned)
        .collect()
}

/// The sum of a count over every seeded database.
fn count(conns: &[Connection], sql: &str) -> i64 {
    conns.iter().map(|conn| conn.query_row(sql, [], |row| row.get::<_, i64>(0)).unwrap()).sum()
}

#[test]
fn the_seed_uses_every_column_of_every_table() {
    let dir = TempDir::new().unwrap();
    let paths = human_seed::seed_sandbox(dir.path(), 0);
    let open = |path| {
        // "Shared collection" is marked open on another computer.
        let machine = MachineIdentity { id: "1".repeat(32), display_name: "Test machine".into() };
        db::open_database(
            path,
            &Passphrase::from_input(human_seed::PASSPHRASE.into()),
            &machine,
            true,
        )
        .unwrap()
    };
    let conns = [open(&paths.main), open(&paths.shared)];

    let mut unseeded = Vec::new();
    for (table, sql) in user_tables(&conns[0]) {
        for column in columns(&conns[0], &table) {
            let name = &column.name;
            if NEVER_SEEDED.contains(&(table.as_str(), name.as_str())) {
                continue;
            }
            // A column that has a default is only exercised by a row that
            // differs from it.
            let used = match &column.default {
                Some(default) => format!(
                    "SELECT COUNT(*) FROM {table} WHERE {name} IS NOT NULL AND {name} IS NOT ({default})"
                ),
                None => format!("SELECT COUNT(*) FROM {table} WHERE {name} IS NOT NULL"),
            };
            if count(&conns, &used) == 0 {
                unseeded.push(format!("{table}.{name} is empty in every seeded row"));
                continue;
            }
            if PARTIAL_VALUES_OK.contains(&(table.as_str(), name.as_str())) {
                continue;
            }
            for value in allowed_values(&sql, name) {
                let has = format!("SELECT COUNT(*) FROM {table} WHERE {name} = {value}");
                if count(&conns, &has) == 0 {
                    unseeded.push(format!("{table}.{name} is never {value}"));
                }
            }
        }
    }

    assert!(
        unseeded.is_empty(),
        "examples/human_seed.rs leaves data model fields unexercised, so a person testing by \
         hand would never see them. Seed a record that uses each one:\n  {}",
        unseeded.join("\n  ")
    );
}

#[test]
fn the_import_samples_use_every_spreadsheet_column() {
    let dir = TempDir::new().unwrap();
    human_seed::write_import_samples(dir.path());

    let mut used: BTreeSet<usize> = BTreeSet::new();
    let mut files = 0;
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "csv") {
            continue;
        }
        files += 1;
        let mut reader = csv::Reader::from_path(&path).unwrap();
        assert_eq!(
            reader.headers().unwrap().iter().collect::<Vec<_>>(),
            COLUMNS,
            "{} does not have the export's columns",
            path.display()
        );
        for record in reader.records() {
            for (index, cell) in record.unwrap().iter().enumerate() {
                if !cell.trim().is_empty() {
                    used.insert(index);
                }
            }
        }
    }
    assert!(files > 0, "no import samples were written");

    let blank: Vec<_> = COLUMNS
        .iter()
        .enumerate()
        .filter(|(index, name)| !used.contains(index) && !NOT_IMPORTED.contains(name))
        .map(|(_, name)| *name)
        .collect();
    assert!(
        blank.is_empty(),
        "no import sample in examples/human_seed.rs fills these spreadsheet columns: {blank:?}"
    );
}
