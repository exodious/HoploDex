//! Keeps the human-testing seed (`examples/human_seed.rs`, run by
//! `scripts/human-testing.sh`) in step with the data model. The seed builds
//! its records through the same `ops` layer as the app, so a new column
//! compiles fine and quietly stays empty; a person testing by hand would then
//! never see the feature. This runs the real seed into a temporary sandbox
//! and fails when any column is empty in every row of both seeded databases,
//! still at its default in every row, or never holds one of the values a
//! `CHECK ... IN` allows, and when an import sample leaves a spreadsheet
//! column blank. Both databases, and the newest seeded backup, count
//! together: the single-row tables (`collection_settings`, `app_state`,
//! `pending_changes`) can hold only one value per database, and the backup
//! stamp (`app_state.backup_made_at`, `backup_of_name`) is only ever set in
//! a backup.
//!
//! When it fails, seed a record that uses the new field in
//! `examples/human_seed.rs` (and add it to the import samples there). Only
//! list something in `NEVER_SEEDED` when it truly cannot be seeded, and say why.

use std::collections::BTreeSet;

mod support;

use hoplodex_lib::db;
use hoplodex_lib::services::machine_settings::MachineIdentity;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::spreadsheet::{ACCESSORY_COLUMNS, FIREARM_COLUMNS};
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
/// lists (firearm types, specs/004-cartridges-action-types' action types
/// and their mapping, research.md §10, and specs/005-regulated-item-types'
/// registration classifications, research.md §14, and specs/006-accessory-links'
/// accessory kinds, research.md §23), and the full-text indexes' shadow tables.
fn is_user_table(name: &str) -> bool {
    !(name.starts_with("sqlite_")
        || name.starts_with("firearms_fts")
        || name.starts_with("accessories_fts")
        || name == "accessory_kinds"
        || name == "schema_migrations"
        || name == "firearm_types"
        || name == "registration_classes"
        || name == "action_types"
        || name == "firearm_type_actions")
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
    let mut seeded_backups: Vec<_> = std::fs::read_dir(&paths.main_backups)
        .expect("the seed makes backups of the main database")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "hoplodex"))
        .collect();
    seeded_backups.sort();
    assert_eq!(seeded_backups.len(), 2, "yesterday's and today's backups");
    let conns = [open(&paths.main), open(&paths.shared), open(seeded_backups.last().unwrap())];

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

    // Import reads columns by header (specs/004-cartridges-action-types
    // FR-023), so one sample may be shaped like a sheet exported before
    // `cartridge` and `action_type` existed, one like a sheet exported
    // before the four registration columns of specs/005-regulated-item-types
    // (US4-7), and one like a sheet exported before `record_id` and
    // `mounted_on` (specs/006-accessory-links US5-7). A sheet is the firearm
    // table or the accessory table by its header (FR-022). No other header
    // is allowed.
    const REGISTRATION: [&str; 4] =
        ["registered_as", "registration_form", "registration_approved", "registered_to"];
    let firearm_headers: Vec<Vec<&str>> = vec![
        FIREARM_COLUMNS.to_vec(),
        FIREARM_COLUMNS
            .iter()
            .copied()
            .filter(|c| !matches!(*c, "cartridge" | "action_type"))
            .collect(),
        FIREARM_COLUMNS.iter().copied().filter(|c| !REGISTRATION.contains(c)).collect(),
        FIREARM_COLUMNS
            .iter()
            .copied()
            .filter(|c| !matches!(*c, "record_id" | "mounted_on"))
            .collect(),
    ];
    let mut firearm_used: BTreeSet<String> = BTreeSet::new();
    let mut accessory_used: BTreeSet<String> = BTreeSet::new();
    let mut files = 0;
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "csv") {
            continue;
        }
        files += 1;
        let mut reader = csv::Reader::from_path(&path).unwrap();
        let headers: Vec<String> = reader.headers().unwrap().iter().map(str::to_owned).collect();
        let is_accessory_table = headers == ACCESSORY_COLUMNS;
        assert!(
            is_accessory_table || firearm_headers.iter().any(|known| &headers == known),
            "{} does not have either table's columns",
            path.display()
        );
        let used = if is_accessory_table { &mut accessory_used } else { &mut firearm_used };
        for record in reader.records() {
            for (header, cell) in headers.iter().zip(record.unwrap().iter()) {
                if !cell.trim().is_empty() {
                    used.insert(header.clone());
                }
            }
        }
    }
    assert!(files > 0, "no import samples were written");

    for (table, columns, used) in [
        ("firearm", FIREARM_COLUMNS, &firearm_used),
        ("accessory", ACCESSORY_COLUMNS, &accessory_used),
    ] {
        let blank: Vec<_> = columns
            .iter()
            .filter(|name| !used.contains(**name) && !NOT_IMPORTED.contains(name))
            .copied()
            .collect();
        assert!(
            blank.is_empty(),
            "no import sample in examples/human_seed.rs fills these {table} table columns: {blank:?}"
        );
    }
}

/// The cartridge import sample does what its comments in the seed say, so a
/// person trying File > Import sees each section of the report.
#[test]
fn the_cartridges_import_sample_shows_each_part_of_the_report() {
    use hoplodex_lib::commands::firearms::ops as firearm_ops;
    use hoplodex_lib::commands::import_export::{ImportSessionStore, ops as import_export_ops};
    use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;

    let dir = TempDir::new().unwrap();
    human_seed::write_import_samples(dir.path());
    let db = support::TestDb::new();
    firearm_ops::create_firearm(
        &db.conn,
        &support::firearm("Smith & Wesson", "Model 10", "S-1"),
        false,
    )
    .unwrap();

    let import = |name: &str| {
        import_export_ops::import_collection(
            &db.conn,
            &support::import_files(dir.path().join(name), SpreadsheetFormat::Csv),
            &ImportSessionStore::new(),
            &mut |_, _| {},
        )
        .unwrap()
    };

    let result = import("import-cartridges.csv");
    assert_eq!(result.imported_count, 3, "{:?}", result.row_errors);
    assert_eq!(result.row_errors.iter().map(|e| e.row).collect::<Vec<_>>(), [4, 5, 6]);
    assert_eq!(result.derived_calibers.len(), 2);
    assert_eq!(result.snapped_values.len(), 2, "{:?}", result.snapped_values);

    let legacy = import("import-before-cartridges.csv");
    assert_eq!(legacy.imported_count, 1, "{:?}", legacy.row_errors);

    // specs/005-regulated-item-types US4: the registration samples.
    let registrations = import("import-registrations.csv");
    assert_eq!(registrations.imported_count, 3, "{:?}", registrations.row_errors);
    assert!(!registrations.snapped_values.is_empty(), "{:?}", registrations.snapped_values);

    let errors = import("import-registration-errors.csv");
    assert_eq!(errors.imported_count, 0, "{:?}", errors.row_errors);
    assert_eq!(errors.row_errors.iter().map(|e| e.row).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
    // A Suppressor's caliber isn't worked out from its rated cartridge.
    assert_eq!(
        errors.row_errors[4].message,
        "caliber: Caliber is required; a Suppressor's isn't worked out from its cartridge."
    );
    assert!(errors.derived_calibers.is_empty());

    let before = import("import-before-registrations.csv");
    assert_eq!(before.imported_count, 1, "{:?}", before.row_errors);
}

/// The accessory import samples do what their comments in the seed say
/// (specs/006-accessory-links US5), so a person trying File > Import sees
/// the pair of tables, the workbook, each mount warning and the row errors.
#[test]
fn the_accessory_import_samples_show_each_part_of_the_report() {
    use hoplodex_lib::commands::import_export::ops as import_export_ops;
    use hoplodex_lib::commands::import_export::{ImportFile, ImportSessionStore};
    use hoplodex_lib::commands::insurance::ops as insurance;
    use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;

    let dir = TempDir::new().unwrap();
    human_seed::write_import_samples(dir.path());
    let fresh = || {
        let db = support::TestDb::new();
        insurance::create_policy(
            &db.conn,
            &support::policy("Vault Schedule", "2020-01-01", "2099-01-01", None),
        )
        .unwrap();
        db
    };
    let file = |name: &str| {
        let path = dir.path().join(name);
        let format =
            if name.ends_with(".xlsx") { SpreadsheetFormat::Xlsx } else { SpreadsheetFormat::Csv };
        ImportFile { file_path: path, format }
    };
    let import = |db: &support::TestDb, names: &[&str]| {
        let files: Vec<ImportFile> = names.iter().map(|name| file(name)).collect();
        import_export_ops::import_collection(
            &db.conn,
            &files,
            &ImportSessionStore::new(),
            &mut |_, _| {},
        )
        .unwrap()
    };
    let count = |db: &support::TestDb, sql: &str| -> i64 {
        db.conn.query_row(sql, [], |row| row.get(0)).unwrap()
    };

    // The two files together, and the same two tables as one workbook.
    for names in [
        vec!["import-mounts-firearms.csv", "import-accessories.csv"],
        vec!["import-collection.xlsx"],
    ] {
        let db = fresh();
        let result = import(&db, &names);
        assert!(result.row_errors.is_empty(), "{names:?}: {:?}", result.row_errors);
        assert!(result.warnings.is_empty(), "{names:?}: {:?}", result.warnings);
        assert_eq!((result.imported_count, result.imported_accessory_count), (9, 7), "{names:?}");
        // The optic on the rifle, the rail on the optic, the suppressor on the upper.
        assert_eq!(count(&db, "SELECT COUNT(*) FROM mounts"), 3, "{names:?}");
        assert_eq!(result.derived_calibers.len(), 1, "{names:?}");
    }

    let db = fresh();
    let warnings = import(&db, &["import-accessory-mount-warnings.csv"]);
    assert_eq!(warnings.imported_count, 7, "{:?}", warnings.row_errors);
    assert_eq!(
        warnings.warnings.iter().map(|w| w.row).collect::<Vec<_>>(),
        [1, 3, 4, 6, 7],
        "{:?}",
        warnings.warnings
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM mounts"), 1, "only the loop's first row mounts");

    let db = fresh();
    let errors = import(&db, &["import-accessory-errors.csv"]);
    assert_eq!(errors.imported_count, 2, "{:?}", errors.row_errors);
    assert_eq!(errors.row_errors.iter().map(|e| e.row).collect::<Vec<_>>(), [1, 2, 3, 4, 5, 7, 8]);

    let db = fresh();
    let before = import(&db, &["import-before-accessories.csv"]);
    assert_eq!(before.imported_count, 1, "{:?}", before.row_errors);
}
