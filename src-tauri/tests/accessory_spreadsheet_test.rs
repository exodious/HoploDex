//! specs/006-accessory-links User Story 5 through `ops`, against real
//! temporary SQLCipher databases and real files (no mocks): a collection of
//! firearms, accessories and mounts exported as two tables and imported
//! again (SC-002, SC-003, US5-3, US5-4), the mount warnings (FR-023, US5-5),
//! the row errors of the accessory table and of `record_id` (FR-022, FR-024,
//! US5-6), either table alone and a sheet from before the feature (US5-7,
//! US5-8), and the stops before any row is saved (FR-022).
//!
//! Written before the implementation (tasks.md T100), against
//! contracts/spreadsheet-format.md and contracts/tauri-commands.md. The ops
//! this file assumes:
//!
//! - `import_export::ops::export_records(conn, scope, filter) -> ExportRecords`
//!   (`scope` is `"all"` or `"filtered"`) and
//!   `export_collection(conn, dest, base, format, &ExportRecords, progress)`;
//! - `import_export::ops::import_collection(conn, &[ImportFile], store,
//!   progress)`, `ImportFile { file_path, format }`, through
//!   `sheets::import`;
//! - `ImportResult`, `RowError`, `ImportConflict`, `SnappedValue` and
//!   `DerivedCaliberReport` carrying `table` ("firearms" | "accessories"),
//!   `ImportConflict` carrying `existingRecord` and `kindName`, and
//!   `ImportResult::imported_accessory_count`;
//! - `resolve_import_conflicts` unchanged.

mod sheets;
mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::commands::import_export::{
    ConflictResolution, ImportResult, ImportSessionStore, ResolveResult,
};
use hoplodex_lib::commands::insurance::ops as insurance;
use hoplodex_lib::commands::mounts::ops as mount_ops;
use hoplodex_lib::models::accessory::AccessoryInput;
use hoplodex_lib::models::firearm::{DispositionType, FirearmInput, FirearmStatus};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::record_id;
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use rusqlite::Connection;
use rusqlite::types::Value as SqlValue;
use serde_json::{Value, json};
use sheets::{Table, accessory_cells, accessory_table, firearm_cells, firearm_table};
use support::TestDb;
use tempfile::TempDir;

const OPTIC: i64 = 1;
const LIGHT_OR_LASER: i64 = 2;
const MAGAZINE: i64 = 3;
const STOCK: i64 = 4;
const UPPER: i64 = 5;
const BARREL: i64 = 6;
const MUZZLE: i64 = 7;
const CONVERSION_KIT: i64 = 8;
const MOUNT_OR_RAIL: i64 = 9;
const SLING: i64 = 10;
const CASE: i64 = 11;
const OTHER: i64 = 12;

const RIFLE: i64 = 2;
const SHOTGUN: i64 = 3;
const SUPPRESSOR: i64 = 5;

// --- Building a collection ---------------------------------------------------------------------

fn accessory(kind: i64) -> AccessoryInput {
    serde_json::from_value(json!({ "accessoryKindId": kind, "status": "active" })).unwrap()
}

/// An accessory of `kind` with the optional fields named in `fields` (the
/// contract's camelCase names).
fn accessory_with(kind: i64, fields: Value) -> AccessoryInput {
    let mut value = json!({ "accessoryKindId": kind, "status": "active" });
    for (name, field) in fields.as_object().unwrap() {
        value[name] = field.clone();
    }
    serde_json::from_value(value).unwrap()
}

fn new_firearm(db: &TestDb, input: &FirearmInput) -> RecordRef {
    RecordRef::Firearm(firearm_ops::create_firearm(&db.conn, input, false).unwrap().id)
}

fn new_accessory(db: &TestDb, input: &AccessoryInput) -> RecordRef {
    RecordRef::Accessory(accessory_ops::create_accessory(&db.conn, input).unwrap().id)
}

fn mounted(input: AccessoryInput, host: RecordRef) -> AccessoryInput {
    AccessoryInput { mounted_on: Some(host), ..input }
}

fn uid_of(conn: &Connection, record: RecordRef) -> String {
    conn.query_row(
        &format!("SELECT uid FROM {} WHERE id = ?1", record.table()),
        [record.id()],
        |row| row.get(0),
    )
    .unwrap()
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0)).unwrap()
}

/// The policies the sample collection uses, by name.
fn add_policies(db: &TestDb) -> i64 {
    let home = insurance::create_policy(
        &db.conn,
        &support::policy("Home", "2020-01-01", "2099-01-01", None),
    )
    .unwrap();
    insurance::create_policy(
        &db.conn,
        &support::policy("Umbrella", "2020-01-01", "2099-01-01", Some(100_000)),
    )
    .unwrap();
    home.id
}

/// SC-002's collection: an accessory of every kind, scheduled and not,
/// active and disposed (one with no price), and mounts of firearms and
/// accessories on both kinds of host. Returns the number of firearms and
/// accessories.
fn sample_collection(db: &TestDb) -> (usize, usize) {
    let home = add_policies(db);

    let rifle = new_firearm(
        db,
        &FirearmInput {
            firearm_type_id: RIFLE,
            caliber: ".308 Winchester".into(),
            cartridge: Some(".308 Winchester".into()),
            estimated_value: Some(2000),
            acquisition_source: Some("Gun show".into()),
            acquisition_date: Some("2022-03-04".into()),
            acquisition_price: Some(1800),
            insurance_policy_id: Some(home),
            scheduled_coverage_amount: Some(1900),
            notes: Some("Match rifle".into()),
            accessories: Some("Bipod, sling".into()),
            ..support::firearm("Ruger", "Precision", "R1")
        },
    );
    let handgun = new_firearm(
        db,
        &FirearmInput {
            serial_number: None,
            no_serial_attested: true,
            estimated_value: Some(600),
            ..support::firearm("Glock", "19", "")
        },
    );
    // A firearm mounted on a firearm.
    let launcher = new_firearm(
        db,
        &FirearmInput {
            firearm_type_id: SHOTGUN,
            caliber: "12 gauge".into(),
            mounted_on: Some(rifle),
            ..support::firearm("Mossberg", "590", "M1")
        },
    );
    new_firearm(
        db,
        &FirearmInput {
            status: FirearmStatus::Disposed,
            disposition_type: Some(DispositionType::Sold),
            disposition_recipient: Some("Jane Doe".into()),
            disposition_date: Some("2025-06-15".into()),
            disposition_price: Some(900),
            ..support::firearm("Sig", "P226", "S1")
        },
    );
    let upper = new_accessory(
        db,
        &accessory_with(
            UPPER,
            json!({ "make": "Daniel Defense", "model": "DDM4 Upper", "caliber": ".223 Remington",
                    "cartridge": ".223 Remington", "estimatedValue": 700 }),
        ),
    );
    // A firearm mounted on an accessory.
    new_firearm(
        db,
        &FirearmInput {
            firearm_type_id: SUPPRESSOR,
            registration_class_id: Some(1),
            mounted_on: Some(upper),
            ..support::firearm("SilencerCo", "Sparrow", "Q1")
        },
    );

    let optic = new_accessory(
        db,
        &mounted(
            accessory_with(
                OPTIC,
                json!({ "make": "Leupold", "model": "VX-5HD", "serialNumber": "OP-1",
                        "estimatedValue": 1100, "acquisitionSource": "Online",
                        "acquisitionDate": "2023-05-01", "acquisitionPrice": 1000,
                        "insurancePolicyId": home, "scheduledCoverageAmount": 900 }),
            ),
            rifle,
        ),
    );
    let light = new_accessory(
        db,
        &mounted(
            accessory_with(
                LIGHT_OR_LASER,
                json!({ "make": "SureFire", "model": "X300", "serialNumber": "L-1",
                        "estimatedValue": 250 }),
            ),
            launcher,
        ),
    );
    // An accessory mounted on an accessory.
    new_accessory(
        db,
        &mounted(
            accessory_with(LIGHT_OR_LASER, json!({ "make": "Crimson Trace", "model": "Laser" })),
            light,
        ),
    );
    new_accessory(
        db,
        &mounted(
            accessory_with(
                MAGAZINE,
                json!({ "make": "Magpul", "model": "PMAG", "caliber": "9mm",
                        "cartridge": "9x19mm Parabellum", "notes": "Holds 15" }),
            ),
            handgun,
        ),
    );
    new_accessory(db, &accessory_with(STOCK, json!({ "make": "Magpul", "model": "CTR" })));
    new_accessory(db, &accessory_with(BARREL, json!({ "make": "Criterion", "model": "Hybrid" })));
    new_accessory(
        db,
        &accessory_with(MUZZLE, json!({ "make": "Griffin Armament", "model": "Brake" })),
    );
    new_accessory(db, &accessory_with(CONVERSION_KIT, json!({ "make": "Advantage Arms" })));
    new_accessory(db, &mounted(accessory_with(MOUNT_OR_RAIL, json!({ "make": "ADM" })), optic));
    new_accessory(db, &accessory_with(SLING, json!({ "make": "Blue Force Gear" })));
    new_accessory(db, &accessory_with(CASE, json!({ "make": "Pelican", "model": "1750" })));
    new_accessory(db, &accessory(OTHER));
    // Disposed, with a price and without one.
    new_accessory(
        db,
        &accessory_with(
            OPTIC,
            json!({ "make": "Old", "model": "Scope", "status": "disposed",
                    "dispositionType": "sold", "dispositionRecipient": "Jane Doe",
                    "dispositionDate": "2025-06-15", "dispositionPrice": 300 }),
        ),
    );
    new_accessory(
        db,
        &accessory_with(
            SLING,
            json!({ "make": "Old", "model": "Sling", "status": "disposed",
                    "dispositionType": "gifted", "dispositionRecipient": "Sam",
                    "dispositionDate": "2025-07-01" }),
        ),
    );
    // A kind no longer offered still exports and imports.
    db.conn.execute("UPDATE accessory_kinds SET offered = 0 WHERE id = ?1", [OTHER]).unwrap();

    (count(&db.conn, "firearms") as usize, count(&db.conn, "accessories") as usize)
}

/// Every record of both tables, keyed by table and identifier, with every
/// column but the database's own keys and timestamps: the policy by name
/// and the mount by the host's identifier. Two databases with the same
/// dump hold the same records, fields, identifiers and mounts.
fn dump(conn: &Connection) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut all = BTreeMap::new();
    for (table, item_column) in
        [("firearms", "item_firearm_id"), ("accessories", "item_accessory_id")]
    {
        let sql = format!(
            "SELECT t.*,
                    (SELECT name FROM insurance_policies p WHERE p.id = t.insurance_policy_id)
                        AS policy_name,
                    (SELECT COALESCE(
                        (SELECT uid FROM firearms f WHERE f.id = m.host_firearm_id),
                        (SELECT uid FROM accessories a WHERE a.id = m.host_accessory_id))
                     FROM mounts m WHERE m.{item_column} = t.id) AS mounted_on
             FROM {table} t"
        );
        let mut stmt = conn.prepare(&sql).unwrap();
        let names: Vec<String> = stmt.column_names().iter().map(|n| n.to_string()).collect();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let mut record = BTreeMap::new();
            for (index, name) in names.iter().enumerate() {
                if ["id", "created_at", "updated_at", "thumbnail_photo_id", "insurance_policy_id"]
                    .contains(&name.as_str())
                {
                    continue;
                }
                let text = match row.get::<_, SqlValue>(index).unwrap() {
                    SqlValue::Null => "(none)".to_owned(),
                    SqlValue::Integer(i) => i.to_string(),
                    SqlValue::Real(r) => r.to_string(),
                    SqlValue::Text(t) => t,
                    SqlValue::Blob(_) => "(blob)".to_owned(),
                };
                record.insert(name.clone(), text);
            }
            all.insert(format!("{table}:{}", record["uid"]), record);
        }
    }
    all
}

fn export_all(
    db: &TestDb,
    dest: &Path,
    base: &str,
    format: SpreadsheetFormat,
) -> (PathBuf, Option<PathBuf>) {
    let records = import_export_ops::export_records(&db.conn, "all", None).unwrap();
    let result = import_export_ops::export_collection(
        &db.conn,
        dest,
        base,
        format,
        &records,
        &mut |_, _| {},
    )
    .unwrap();
    (result.spreadsheet_path, result.accessory_spreadsheet_path)
}

fn import(db: &TestDb, store: &ImportSessionStore, paths: &[&Path]) -> ImportResult {
    sheets::import(&db.conn, store, paths).unwrap()
}

fn import_err(db: &TestDb, paths: &[&Path]) -> CommandError {
    sheets::import(&db.conn, &ImportSessionStore::new(), paths).unwrap_err()
}

fn resolve(
    db: &TestDb,
    store: &ImportSessionStore,
    result: &ImportResult,
    action: &str,
) -> ResolveResult {
    import_export_ops::resolve_import_conflicts(
        &db.conn,
        store,
        &result.session_id,
        &[],
        Some(action),
    )
    .unwrap()
}

fn resolve_one(
    db: &TestDb,
    store: &ImportSessionStore,
    result: &ImportResult,
    conflict_id: &str,
    action: &str,
) -> ResolveResult {
    let resolutions =
        [ConflictResolution { conflict_id: conflict_id.into(), action: action.into() }];
    import_export_ops::resolve_import_conflicts(
        &db.conn,
        store,
        &result.session_id,
        &resolutions,
        None,
    )
    .unwrap()
}

fn conflicts_json(result: &ImportResult) -> Vec<Value> {
    result.conflicts.iter().map(|c| serde_json::to_value(c).unwrap()).collect()
}

fn mount_of(conn: &Connection, record: RecordRef) -> Option<String> {
    let item_column = match record {
        RecordRef::Firearm(_) => "item_firearm_id",
        RecordRef::Accessory(_) => "item_accessory_id",
    };
    conn.query_row(
        &format!(
            "SELECT COALESCE(
                (SELECT uid FROM firearms f WHERE f.id = m.host_firearm_id),
                (SELECT uid FROM accessories a WHERE a.id = m.host_accessory_id))
             FROM mounts m WHERE m.{item_column} = ?1"
        ),
        [record.id()],
        |row| row.get(0),
    )
    .ok()
}

/// The record with this identifier in either table.
fn find(conn: &Connection, uid: &str) -> RecordRef {
    if let Ok(id) = conn.query_row("SELECT id FROM firearms WHERE uid = ?1", [uid], |r| r.get(0)) {
        return RecordRef::Firearm(id);
    }
    RecordRef::Accessory(
        conn.query_row("SELECT id FROM accessories WHERE uid = ?1", [uid], |r| r.get(0))
            .unwrap_or_else(|_| panic!("no record {uid}")),
    )
}

fn fresh_uid() -> String {
    record_id::generate()
}

// --- SC-002: the round trip into an empty database (US5-3) ----------------------------------------

/// Imports `paths` into an empty database that has the sample collection's
/// policies by name, and checks that it holds exactly what `source` holds.
fn assert_round_trips(source: &TestDb, paths: &[&Path], expected: (usize, usize)) {
    let fresh = TestDb::new();
    add_policies(&fresh);

    let result = import(&fresh, &ImportSessionStore::new(), paths);

    assert!(result.row_errors.is_empty(), "{:?}", result.row_errors);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert!(result.conflicts.is_empty());
    assert!(result.snapped_values.is_empty(), "{:?}", result.snapped_values);
    assert!(result.derived_calibers.is_empty());
    assert_eq!(result.imported_count, expected.0 + expected.1);
    assert_eq!(result.imported_accessory_count, expected.1);
    assert_eq!(dump(&fresh.conn), dump(&source.conn));
}

#[test]
fn a_workbook_with_both_sheets_reproduces_every_record_field_identifier_and_mount() {
    let db = TestDb::new();
    let expected = sample_collection(&db);
    let dest = TempDir::new().unwrap();

    let (workbook, second) = export_all(&db, dest.path(), "book", SpreadsheetFormat::Xlsx);

    assert_eq!(second, None);
    assert_round_trips(&db, &[&workbook], expected);
}

#[test]
fn two_single_sheet_workbooks_reproduce_the_collection_in_either_order() {
    let db = TestDb::new();
    let expected = sample_collection(&db);
    let dest = TempDir::new().unwrap();
    let (workbook, _) = export_all(&db, dest.path(), "book", SpreadsheetFormat::Xlsx);
    let sheets = sheets::read_workbook(&workbook);
    let firearms = dest.path().join("firearms.xlsx");
    let accessories = dest.path().join("accessories.xlsx");
    sheets::write_workbook(&firearms, &[("Firearms", &sheets[0].1)]);
    sheets::write_workbook(&accessories, &[("Accessories", &sheets[1].1)]);

    assert_round_trips(&db, &[&firearms, &accessories], expected);
    assert_round_trips(&db, &[&accessories, &firearms], expected);
}

#[test]
fn two_csv_files_reproduce_the_collection_in_either_order() {
    let db = TestDb::new();
    let expected = sample_collection(&db);
    let dest = TempDir::new().unwrap();

    let (firearms, accessories) = export_all(&db, dest.path(), "files", SpreadsheetFormat::Csv);
    let accessories = accessories.expect("the accessory file");

    assert_round_trips(&db, &[&firearms, &accessories], expected);
    assert_round_trips(&db, &[&accessories, &firearms], expected);
}

#[test]
fn a_workbook_and_a_csv_file_may_be_picked_together() {
    let db = TestDb::new();
    let expected = sample_collection(&db);
    let dest = TempDir::new().unwrap();
    let (workbook, _) = export_all(&db, dest.path(), "book", SpreadsheetFormat::Xlsx);
    let sheets = sheets::read_workbook(&workbook);
    let firearms = dest.path().join("firearms.xlsx");
    sheets::write_workbook(&firearms, &[("Firearms", &sheets[0].1)]);
    let accessories = sheets::csv_in(dest.path(), "accessories.csv", &sheets[1].1);

    assert_round_trips(&db, &[&firearms, &accessories], expected);
}

#[test]
fn a_disposed_row_may_have_a_blank_disposition_price_in_either_table() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let disposed = [
        ("status", "disposed"),
        ("disposition_type", "gifted"),
        ("disposition_recipient", "Sam"),
        ("disposition_date", "2025-07-01"),
    ];
    let firearms = sheets::csv_in(
        dir.path(),
        "firearms.csv",
        &firearm_table(&[firearm_cells("Sig", "P226", "S1", &disposed)]),
    );
    let accessories = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells("Sling", &disposed)]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&firearms, &accessories]);

    assert!(result.row_errors.is_empty(), "{:?}", result.row_errors);
    assert_eq!(result.imported_count, 2);
    let priced: i64 = db
        .conn
        .query_row(
            "SELECT (SELECT count(*) FROM firearms WHERE disposition_price IS NOT NULL)
                  + (SELECT count(*) FROM accessories WHERE disposition_price IS NOT NULL)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(priced, 0);
}

// --- SC-003: importing the collection into itself (US5-4) -------------------------------------------

#[test]
fn re_importing_an_export_with_skip_for_every_conflict_creates_and_changes_nothing() {
    let db = TestDb::new();
    let (firearms, accessories) = sample_collection(&db);
    let dest = TempDir::new().unwrap();
    let (first, second) = export_all(&db, dest.path(), "again", SpreadsheetFormat::Csv);
    let before = dump(&db.conn);
    let store = ImportSessionStore::new();

    let result = import(&db, &store, &[&first, second.as_deref().unwrap()]);

    assert!(result.row_errors.is_empty(), "{:?}", result.row_errors);
    assert_eq!(result.imported_count, 0);
    // Every record is its own match, by identifier: the firearm with no
    // serial number, the disposed ones and every accessory too.
    assert_eq!(result.conflicts.len(), firearms + accessories);
    let tables: Vec<_> = conflicts_json(&result).iter().map(|c| c["table"].clone()).collect();
    assert_eq!(tables.iter().filter(|t| *t == "firearms").count(), firearms);
    assert_eq!(tables.iter().filter(|t| *t == "accessories").count(), accessories);

    let resolved = resolve(&db, &store, &result, "skip");

    assert_eq!(resolved.resolved_count, firearms + accessories);
    assert!(resolved.unresolved.is_empty());
    assert_eq!(count(&db.conn, "firearms"), firearms as i64);
    assert_eq!(count(&db.conn, "accessories"), accessories as i64);
    assert_eq!(dump(&db.conn), before);
}

#[test]
fn a_duplicate_is_a_new_record_with_a_new_identifier() {
    let db = TestDb::new();
    add_policies(&db);
    // A firearm with no serial number may be added again; so may any accessory.
    let handgun = new_firearm(
        &db,
        &FirearmInput {
            serial_number: None,
            no_serial_attested: true,
            ..support::firearm("Glock", "19", "")
        },
    );
    let optic = new_accessory(&db, &accessory_with(OPTIC, json!({ "make": "Leupold" })));
    let dest = TempDir::new().unwrap();
    let (first, second) = export_all(&db, dest.path(), "dup", SpreadsheetFormat::Csv);
    let old = [uid_of(&db.conn, handgun), uid_of(&db.conn, optic)];
    let store = ImportSessionStore::new();

    let result = import(&db, &store, &[&first, second.as_deref().unwrap()]);
    assert_eq!(result.conflicts.len(), 2, "{:?}", result.row_errors);
    assert!(conflicts_json(&result).iter().all(|c| c["duplicateAllowed"] == true));
    let resolved = resolve(&db, &store, &result, "duplicate");

    assert_eq!(resolved.resolved_count, 2, "{:?}", resolved.unresolved);
    assert_eq!(count(&db.conn, "firearms"), 2);
    assert_eq!(count(&db.conn, "accessories"), 2);
    let uids: Vec<String> = db
        .conn
        .prepare("SELECT uid FROM firearms UNION ALL SELECT uid FROM accessories")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(uids.len(), 4);
    assert_eq!(uids.iter().collect::<std::collections::HashSet<_>>().len(), 4);
    for uid in &old {
        assert!(uids.contains(uid), "the original keeps its identifier");
    }
}

#[test]
fn overwrite_keeps_the_identifier_takes_the_rows_mount_and_a_blank_one_unmounts() {
    let db = TestDb::new();
    let rifle = new_firearm(
        &db,
        &FirearmInput { firearm_type_id: RIFLE, ..support::firearm("Ruger", "Precision", "R1") },
    );
    let other = new_firearm(&db, &support::firearm("Glock", "19", "G1"));
    let optic =
        new_accessory(&db, &mounted(accessory_with(OPTIC, json!({ "make": "Leupold" })), rifle));
    let light = new_accessory(&db, &accessory_with(LIGHT_OR_LASER, json!({ "make": "SureFire" })));
    let (rifle_uid, optic_uid, light_uid) =
        (uid_of(&db.conn, rifle), uid_of(&db.conn, optic), uid_of(&db.conn, light));
    let dest = TempDir::new().unwrap();
    let (first, second) = export_all(&db, dest.path(), "overwrite", SpreadsheetFormat::Csv);

    // The collection moves on: the Optic leaves the Rifle, the Light goes on it.
    for (item, host) in [(optic, None), (light, Some(rifle))] {
        mount_ops::mount_record(
            &db.conn,
            &serde_json::from_value(json!({ "item": item, "host": host })).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(mount_of(&db.conn, optic), None);
    assert_eq!(mount_of(&db.conn, light), Some(rifle_uid.clone()));

    let store = ImportSessionStore::new();
    let result = import(&db, &store, &[&first, second.as_deref().unwrap()]);
    assert_eq!(result.conflicts.len(), 4, "{:?}", result.row_errors);
    let resolved = resolve(&db, &store, &result, "overwrite");

    assert_eq!(resolved.resolved_count, 4, "{:?}", resolved.unresolved);
    assert_eq!(count(&db.conn, "firearms"), 2);
    assert_eq!(count(&db.conn, "accessories"), 2);
    assert_eq!(uid_of(&db.conn, optic), optic_uid);
    assert_eq!(uid_of(&db.conn, light), light_uid);
    assert_eq!(mount_of(&db.conn, optic), Some(rifle_uid), "the row's mount is applied");
    assert_eq!(mount_of(&db.conn, light), None, "a blank cell unmounts");
    let _ = other;
}

#[test]
fn a_single_conflict_is_resolved_on_its_own_and_the_rest_are_left() {
    let db = TestDb::new();
    let rifle = new_firearm(
        &db,
        &FirearmInput { firearm_type_id: RIFLE, ..support::firearm("Ruger", "Precision", "R1") },
    );
    let optic =
        new_accessory(&db, &mounted(accessory_with(OPTIC, json!({ "make": "Leupold" })), rifle));
    let dest = TempDir::new().unwrap();
    let (first, second) = export_all(&db, dest.path(), "one", SpreadsheetFormat::Csv);
    mount_ops::mount_record(
        &db.conn,
        &serde_json::from_value(json!({ "item": optic, "host": null })).unwrap(),
    )
    .unwrap();
    let store = ImportSessionStore::new();
    let result = import(&db, &store, &[&first, second.as_deref().unwrap()]);
    let accessory_conflict = conflicts_json(&result)
        .into_iter()
        .find(|c| c["table"] == "accessories")
        .expect("the Optic conflicts");
    assert_eq!(accessory_conflict["kindName"], "Optic");
    assert_eq!(
        accessory_conflict["existingRecord"],
        json!({ "kind": "accessory", "id": optic.id() })
    );

    resolve_one(
        &db,
        &store,
        &result,
        accessory_conflict["conflictId"].as_str().unwrap(),
        "overwrite",
    );

    assert_eq!(mount_of(&db.conn, optic), Some(uid_of(&db.conn, rifle)));
}

// --- Mount warnings (FR-023, US5-5) ---------------------------------------------------------------------

fn disposed_firearm(db: &TestDb) -> RecordRef {
    new_firearm(
        db,
        &FirearmInput {
            status: FirearmStatus::Disposed,
            disposition_type: Some(DispositionType::Sold),
            disposition_recipient: Some("Jane Doe".into()),
            disposition_date: Some("2025-06-15".into()),
            disposition_price: Some(900),
            ..support::firearm("Sig", "P226", "S1")
        },
    )
}

#[test]
fn a_mount_that_cannot_be_made_warns_with_the_row_and_the_reason_and_the_record_imports_unmounted()
{
    let db = TestDb::new();
    let active = new_firearm(&db, &support::firearm("Glock", "19", "G1"));
    let disposed = disposed_firearm(&db);
    let (active_uid, disposed_uid) = (uid_of(&db.conn, active), uid_of(&db.conn, disposed));
    let unknown = fresh_uid();
    let ids: Vec<String> = (0..4).map(|_| fresh_uid()).collect();
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[
            accessory_cells("Optic", &[("record_id", &ids[0]), ("mounted_on", &unknown)]),
            accessory_cells("Optic", &[("record_id", &ids[1]), ("mounted_on", &disposed_uid)]),
            accessory_cells("Optic", &[("record_id", &ids[2]), ("mounted_on", "not-a-uid")]),
            accessory_cells("Optic", &[("record_id", &ids[3]), ("mounted_on", &active_uid)]),
        ]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&path]);

    assert!(result.row_errors.is_empty(), "{:?}", result.row_errors);
    assert_eq!(result.imported_count, 4);
    assert_eq!(
        sheets::entries(&result.warnings),
        [
            (
                "accessories".to_owned(),
                1,
                format!(
                    "Mounted on {unknown}: no firearm or accessory has this record ID. Imported unmounted."
                )
            ),
            (
                "accessories".to_owned(),
                2,
                format!("Mounted on {disposed_uid}: that record is disposed. Imported unmounted.")
            ),
            (
                "accessories".to_owned(),
                3,
                "Mounted on not-a-uid: not a record ID. Imported unmounted.".to_owned()
            ),
        ]
    );
    for id in &ids[..3] {
        assert_eq!(mount_of(&db.conn, find(&db.conn, id)), None, "{id}");
    }
    assert_eq!(mount_of(&db.conn, find(&db.conn, &ids[3])), Some(active_uid), "the control");
}

#[test]
fn a_disposed_row_with_a_mount_imports_unmounted_with_a_warning() {
    let db = TestDb::new();
    let host = new_firearm(&db, &support::firearm("Glock", "19", "G1"));
    let host_uid = uid_of(&db.conn, host);
    let id = fresh_uid();
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells(
            "Optic",
            &[
                ("record_id", &id),
                ("mounted_on", &host_uid),
                ("status", "disposed"),
                ("disposition_type", "sold"),
                ("disposition_recipient", "Jane"),
                ("disposition_date", "2025-06-15"),
                ("disposition_price", "300"),
            ],
        )]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&path]);

    assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
    let warnings = sheets::messages_for(&result.warnings, "accessories", 1);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].starts_with(&format!("Mounted on {host_uid}: ")), "{warnings:?}");
    assert!(warnings[0].ends_with("Imported unmounted."), "{warnings:?}");
    assert_eq!(mount_of(&db.conn, find(&db.conn, &id)), None);
}

/// Two rows mounted on each other: the later row in file order, the firearm
/// table first whichever file was picked first, is the one left unmounted.
#[test]
fn of_two_rows_mounted_on_each_other_the_later_in_file_order_is_left_unmounted() {
    let (firearm_id, accessory_id) = (fresh_uid(), fresh_uid());
    let dir = TempDir::new().unwrap();
    let firearms = sheets::csv_in(
        dir.path(),
        "firearms.csv",
        &firearm_table(&[firearm_cells(
            "Ruger",
            "Precision",
            "R1",
            &[("record_id", &firearm_id), ("mounted_on", &accessory_id)],
        )]),
    );
    let accessories = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells(
            "Optic",
            &[("record_id", &accessory_id), ("mounted_on", &firearm_id)],
        )]),
    );

    for order in [[&firearms, &accessories], [&accessories, &firearms]] {
        let db = TestDb::new();
        let result =
            import(&db, &ImportSessionStore::new(), &[order[0].as_path(), order[1].as_path()]);

        assert!(result.row_errors.is_empty(), "{:?}", result.row_errors);
        assert_eq!(result.imported_count, 2);
        let firearm = find(&db.conn, &firearm_id);
        let accessory = find(&db.conn, &accessory_id);
        assert_eq!(
            mount_of(&db.conn, firearm),
            Some(accessory_id.clone()),
            "the earlier row keeps it"
        );
        assert_eq!(mount_of(&db.conn, accessory), None, "the later row is unmounted");
        let warnings = sheets::entries(&result.warnings);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!((warnings[0].0.as_str(), warnings[0].1), ("accessories", 1));
        assert!(
            warnings[0].2.starts_with(&format!(
                "Mounted on {firearm_id}: it would be mounted on itself through "
            )),
            "{warnings:?}"
        );
        assert!(warnings[0].2.ends_with(". Imported unmounted."), "{warnings:?}");
    }
}

#[test]
fn a_row_mounted_on_a_row_of_the_same_import_resolves_even_if_that_row_comes_later() {
    let (host_id, item_id) = (fresh_uid(), fresh_uid());
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    // The accessory table names a firearm of the firearm table, and the
    // firearm table an accessory of the accessory table.
    let firearms = sheets::csv_in(
        dir.path(),
        "firearms.csv",
        &firearm_table(&[firearm_cells("Ruger", "Precision", "R1", &[("record_id", &host_id)])]),
    );
    let accessories = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells(
            "Optic",
            &[("record_id", &item_id), ("mounted_on", &host_id)],
        )]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&accessories, &firearms]);

    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(mount_of(&db.conn, find(&db.conn, &item_id)), Some(host_id));
}

#[test]
fn a_host_row_that_matched_an_existing_record_by_make_model_and_serial_counts_as_the_host() {
    // The sheet's host row has no identifier the database knows, but its
    // make, model and serial number match a record, so it is a conflict: a
    // row mounted on its identifier is still mounted on that record once the
    // conflict is settled by the sheet's own row.
    let db = TestDb::new();
    let existing = new_firearm(&db, &support::firearm("Glock", "19", "G1"));
    let host_id = fresh_uid();
    let dir = TempDir::new().unwrap();
    let firearms = sheets::csv_in(
        dir.path(),
        "firearms.csv",
        &firearm_table(&[firearm_cells("Glock", "19", "G1", &[("record_id", &host_id)])]),
    );
    let accessories = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells(
            "Optic",
            &[("record_id", &fresh_uid()), ("mounted_on", &host_id)],
        )]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&firearms, &accessories]);

    assert_eq!(result.conflicts.len(), 1, "{:?}", result.row_errors);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    let optic = RecordRef::Accessory(
        db.conn.query_row("SELECT id FROM accessories", [], |r| r.get(0)).unwrap(),
    );
    assert_eq!(mount_of(&db.conn, optic), Some(uid_of(&db.conn, existing)));
}

#[test]
fn resolving_a_conflict_as_overwrite_or_duplicate_reports_a_mount_that_cannot_be_made() {
    let db = TestDb::new();
    let optic = new_accessory(&db, &accessory_with(OPTIC, json!({ "make": "Leupold" })));
    let optic_uid = uid_of(&db.conn, optic);
    let unknown = fresh_uid();
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells(
            "Optic",
            &[("record_id", &optic_uid), ("make", "Leupold"), ("mounted_on", &unknown)],
        )]),
    );

    for action in ["overwrite", "duplicate"] {
        let store = ImportSessionStore::new();
        let result = import(&db, &store, &[&path]);
        assert_eq!(result.conflicts.len(), 1);
        assert!(result.warnings.is_empty(), "the warning waits for the decision");

        let resolved = resolve(&db, &store, &result, action);

        assert_eq!(resolved.resolved_count, 1, "{:?}", resolved.unresolved);
        assert_eq!(
            sheets::entries(&resolved.warnings),
            [(
                "accessories".to_owned(),
                1,
                format!(
                    "Mounted on {unknown}: no firearm or accessory has this record ID. Imported unmounted."
                )
            )],
            "{action}"
        );
    }
}

#[test]
fn skip_leaves_the_record_and_its_mount_and_warns_of_nothing() {
    let db = TestDb::new();
    let rifle = new_firearm(
        &db,
        &FirearmInput { firearm_type_id: RIFLE, ..support::firearm("Ruger", "Precision", "R1") },
    );
    let optic =
        new_accessory(&db, &mounted(accessory_with(OPTIC, json!({ "make": "Leupold" })), rifle));
    let optic_uid = uid_of(&db.conn, optic);
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells(
            "Optic",
            &[("record_id", &optic_uid), ("mounted_on", &fresh_uid())],
        )]),
    );
    let store = ImportSessionStore::new();
    let result = import(&db, &store, &[&path]);

    let resolved = resolve(&db, &store, &result, "skip");

    assert!(resolved.warnings.is_empty());
    assert_eq!(mount_of(&db.conn, optic), Some(uid_of(&db.conn, rifle)));
}

// --- Row errors (US5-6, FR-022, FR-024) ---------------------------------------------------------------------

#[test]
fn accessory_rows_with_a_blank_or_unknown_kind_a_bad_amount_or_a_bad_date_fail_alone() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[
            accessory_cells("", &[("make", "Blank")]),
            accessory_cells("Frobnicator", &[("make", "Unknown")]),
            accessory_cells("Optic", &[("make", "Amount"), ("estimated_value", "12.5")]),
            accessory_cells("Optic", &[("make", "Date"), ("acquisition_date", "2026-13-45")]),
            accessory_cells("Optic", &[("make", "Good")]),
        ]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&path]);

    assert_eq!(result.imported_count, 1, "the valid row still imports");
    let errors = sheets::entries(&result.row_errors);
    let rows: Vec<_> = errors.iter().map(|(table, row, _)| (table.as_str(), *row)).collect();
    assert_eq!(
        rows,
        [("accessories", 1), ("accessories", 2), ("accessories", 3), ("accessories", 4)]
    );
    assert!(errors[0].2.contains("kind"), "{errors:?}");
    assert!(errors[1].2.contains("kind"), "{errors:?}");
    assert!(errors[2].2.contains("estimated_value"), "{errors:?}");
    assert!(errors[3].2.contains("acquisition_date"), "{errors:?}");
    assert_eq!(count(&db.conn, "accessories"), 1);
}

#[test]
fn a_kind_matches_ignoring_case_and_surrounding_spaces_and_includes_a_kind_no_longer_offered() {
    let db = TestDb::new();
    db.conn.execute("UPDATE accessory_kinds SET offered = 0 WHERE id = ?1", [SLING]).unwrap();
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[
            accessory_cells("  optic ", &[]),
            accessory_cells("SLING", &[]),
            accessory_cells("light or LASER", &[]),
        ]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&path]);

    assert_eq!(result.imported_count, 3, "{:?}", result.row_errors);
    let kinds: Vec<i64> = db
        .conn
        .prepare("SELECT accessory_kind_id FROM accessories ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(kinds, [OPTIC, SLING, LIGHT_OR_LASER]);
}

#[test]
fn record_id_errors_are_row_errors_and_the_other_rows_import() {
    let db = TestDb::new();
    let repeated = fresh_uid();
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[
            accessory_cells("Optic", &[("record_id", "xyz")]),
            accessory_cells("Optic", &[("record_id", &repeated)]),
            accessory_cells("Optic", &[("record_id", &repeated)]),
            accessory_cells("Optic", &[]),
        ]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&path]);

    assert_eq!(result.imported_count, 2, "{:?}", result.row_errors);
    assert_eq!(
        sheets::entries(&result.row_errors),
        [
            ("accessories".to_owned(), 1, "record_id: \"xyz\" is not a record ID".to_owned()),
            ("accessories".to_owned(), 3, format!("record_id: {repeated} is also used by row 2")),
        ]
    );
}

#[test]
fn a_record_id_repeated_across_the_two_tables_is_a_row_error_on_the_later_row() {
    let db = TestDb::new();
    let shared = fresh_uid();
    let dir = TempDir::new().unwrap();
    let firearms = sheets::csv_in(
        dir.path(),
        "firearms.csv",
        &firearm_table(&[firearm_cells("Glock", "19", "G1", &[("record_id", &shared)])]),
    );
    let accessories = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells("Optic", &[("record_id", &shared)])]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&firearms, &accessories]);

    assert_eq!(result.imported_count, 1);
    let errors = sheets::entries(&result.row_errors);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!((errors[0].0.as_str(), errors[0].1), ("accessories", 1));
    assert!(
        errors[0].2.starts_with(&format!("record_id: {shared} is also used by row ")),
        "{errors:?}"
    );
}

#[test]
fn an_identifier_that_belongs_to_the_other_kind_of_record_is_a_row_error() {
    let db = TestDb::new();
    let firearm = new_firearm(&db, &support::firearm("Glock", "19", "G1"));
    let optic = new_accessory(&db, &accessory_with(OPTIC, json!({ "make": "Leupold" })));
    let (firearm_uid, optic_uid) = (uid_of(&db.conn, firearm), uid_of(&db.conn, optic));
    let dir = TempDir::new().unwrap();
    let firearms = sheets::csv_in(
        dir.path(),
        "firearms.csv",
        &firearm_table(&[firearm_cells("Sig", "P226", "S1", &[("record_id", &optic_uid)])]),
    );
    let accessories = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells("Optic", &[("record_id", &firearm_uid)])]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&firearms, &accessories]);

    assert_eq!(result.imported_count, 0);
    assert!(result.conflicts.is_empty());
    assert_eq!(
        sheets::entries(&result.row_errors),
        [
            ("firearms".to_owned(), 1, format!("record_id: {optic_uid} belongs to an accessory")),
            ("accessories".to_owned(), 1, format!("record_id: {firearm_uid} belongs to a firearm")),
        ]
    );
}

#[test]
fn accessory_text_is_snapped_to_spellings_on_record_in_both_tables_and_listed() {
    let db = TestDb::new();
    // A make on a firearm and one on an accessory, neither in the catalog.
    new_firearm(&db, &support::firearm("Frobnitz Arms", "Model 1", "F1"));
    new_accessory(&db, &accessory_with(OPTIC, json!({ "make": "Zeta Optics" })));
    let dir = TempDir::new().unwrap();
    let firearms = sheets::csv_in(
        dir.path(),
        "firearms.csv",
        &firearm_table(&[firearm_cells("zeta optics", "Model 2", "F2", &[])]),
    );
    let accessories = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells("Optic", &[("make", "FROBNITZ ARMS")])]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&firearms, &accessories]);

    assert_eq!(result.imported_count, 2, "{:?}", result.row_errors);
    let makes: Vec<String> = db
        .conn
        .prepare("SELECT make FROM firearms UNION SELECT make FROM accessories")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(makes.contains(&"Zeta Optics".to_owned()), "{makes:?}");
    assert!(makes.contains(&"Frobnitz Arms".to_owned()), "{makes:?}");
    assert!(
        !makes.contains(&"zeta optics".to_owned()) && !makes.contains(&"FROBNITZ ARMS".to_owned())
    );

    let snapped: Vec<Value> =
        result.snapped_values.iter().map(|s| serde_json::to_value(s).unwrap()).collect();
    assert_eq!(snapped.len(), 2, "{snapped:?}");
    let in_table = |table: &str| snapped.iter().find(|s| s["table"] == table).expect(table).clone();
    let firearm_row = in_table("firearms");
    assert_eq!(
        (
            firearm_row["row"].clone(),
            firearm_row["field"].clone(),
            firearm_row["sheetValue"].clone(),
            firearm_row["recordedValue"].clone()
        ),
        (json!(1), json!("make"), json!("zeta optics"), json!("Zeta Optics"))
    );
    let accessory_row = in_table("accessories");
    assert_eq!(
        (
            accessory_row["row"].clone(),
            accessory_row["field"].clone(),
            accessory_row["sheetValue"].clone(),
            accessory_row["recordedValue"].clone()
        ),
        (json!(1), json!("make"), json!("FROBNITZ ARMS"), json!("Frobnitz Arms"))
    );
}

#[test]
fn an_accessory_spelling_matches_one_used_earlier_in_the_same_import() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[
            accessory_cells("Optic", &[("make", "Zeta Optics")]),
            accessory_cells("Optic", &[("make", "zeta optics")]),
        ]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&path]);

    assert_eq!(result.imported_count, 2);
    let distinct: i64 = db
        .conn
        .query_row("SELECT count(DISTINCT make) FROM accessories", [], |r| r.get(0))
        .unwrap();
    assert_eq!(distinct, 1);
    assert_eq!(result.snapped_values.len(), 1);
}

#[test]
fn a_blank_caliber_of_an_accessory_is_derived_from_its_cartridge_and_the_table_is_named() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells(
            "Magazine",
            &[("caliber", ""), ("cartridge", "9x19mm Parabellum")],
        )]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&path]);

    assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
    let caliber: String =
        db.conn.query_row("SELECT caliber FROM accessories", [], |r| r.get(0)).unwrap();
    assert_eq!(caliber, "9mm");
    let derived = serde_json::to_value(&result.derived_calibers).unwrap();
    assert_eq!(derived[0]["table"], "accessories");
    assert_eq!(derived[0]["row"], 1);
    assert_eq!(derived[0]["caliber"], "9mm");
}

#[test]
fn every_report_entry_names_the_table_it_came_from() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let firearms = sheets::csv_in(
        dir.path(),
        "firearms.csv",
        &firearm_table(&[
            firearm_cells("", "19", "G1", &[]),
            firearm_cells("Glock", "19", "G2", &[("mounted_on", &fresh_uid())]),
        ]),
    );
    let accessories = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[
            accessory_cells("", &[]),
            accessory_cells("Optic", &[("mounted_on", "no")]),
        ]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&firearms, &accessories]);

    let errors = sheets::entries(&result.row_errors);
    assert_eq!(
        errors.iter().map(|(t, r, _)| (t.as_str(), *r)).collect::<Vec<_>>(),
        [("firearms", 1), ("accessories", 1)]
    );
    let warnings = sheets::entries(&result.warnings);
    assert_eq!(
        warnings.iter().map(|(t, r, _)| (t.as_str(), *r)).collect::<Vec<_>>(),
        [("firearms", 2), ("accessories", 2)]
    );
    assert_eq!(result.imported_count, 2);
    assert_eq!(result.imported_accessory_count, 1);
}

// --- Either table alone, and a sheet from before the feature (US5-7, US5-8) -------------------------

#[test]
fn a_firearm_sheet_from_before_the_feature_imports_as_it_did() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let table = sheets::without_columns(
        &firearm_table(&[
            firearm_cells("Glock", "19", "G1", &[]),
            firearm_cells("Sig", "P226", "S1", &[]),
        ]),
        &["record_id", "mounted_on"],
    );
    assert!(!table[0].contains(&"record_id".to_owned()));
    let path = sheets::csv_in(dir.path(), "old.csv", &table);

    let result = import(&db, &ImportSessionStore::new(), &[&path]);

    assert_eq!(result.imported_count, 2, "{:?}", result.row_errors);
    assert!(result.row_errors.is_empty() && result.warnings.is_empty());
    assert_eq!(count(&db.conn, "firearms"), 2);
    assert_eq!(count(&db.conn, "accessories"), 0);
    assert_eq!(count(&db.conn, "mounts"), 0);
    // Each record got an identifier of its own.
    let ids: i64 =
        db.conn.query_row("SELECT count(DISTINCT uid) FROM firearms", [], |r| r.get(0)).unwrap();
    assert_eq!(ids, 2);
}

#[test]
fn the_firearm_table_alone_mounts_on_an_accessory_already_in_the_database_and_changes_no_accessory()
{
    let db = TestDb::new();
    let upper = new_accessory(&db, &accessory_with(UPPER, json!({ "make": "Daniel Defense" })));
    let upper_uid = uid_of(&db.conn, upper);
    let before = dump(&db.conn);
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "firearms.csv",
        &firearm_table(&[firearm_cells(
            "SilencerCo",
            "Sparrow",
            "Q1",
            &[("mounted_on", &upper_uid)],
        )]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&path]);

    assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
    assert_eq!(result.imported_accessory_count, 0);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    let firearm =
        RecordRef::Firearm(db.conn.query_row("SELECT id FROM firearms", [], |r| r.get(0)).unwrap());
    assert_eq!(mount_of(&db.conn, firearm), Some(upper_uid.clone()));
    assert_eq!(count(&db.conn, "accessories"), 1);
    let accessories_after: Vec<_> =
        dump(&db.conn).into_iter().filter(|(key, _)| key.starts_with("accessories:")).collect();
    let accessories_before: Vec<_> =
        before.into_iter().filter(|(key, _)| key.starts_with("accessories:")).collect();
    assert_eq!(accessories_after, accessories_before);
}

#[test]
fn the_accessory_table_alone_resolves_its_mounts_against_the_database() {
    let db = TestDb::new();
    let rifle = new_firearm(
        &db,
        &FirearmInput { firearm_type_id: RIFLE, ..support::firearm("Ruger", "Precision", "R1") },
    );
    let rifle_uid = uid_of(&db.conn, rifle);
    let before = dump(&db.conn);
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells(
            "Optic",
            &[("make", "Leupold"), ("mounted_on", &rifle_uid)],
        )]),
    );

    let result = import(&db, &ImportSessionStore::new(), &[&path]);

    assert_eq!(result.imported_count, 1, "{:?}", result.row_errors);
    assert_eq!(result.imported_accessory_count, 1);
    let optic = RecordRef::Accessory(
        db.conn.query_row("SELECT id FROM accessories", [], |r| r.get(0)).unwrap(),
    );
    assert_eq!(mount_of(&db.conn, optic), Some(rifle_uid));
    let firearms_after: Vec<_> =
        dump(&db.conn).into_iter().filter(|(key, _)| key.starts_with("firearms:")).collect();
    let firearms_before: Vec<_> =
        before.into_iter().filter(|(key, _)| key.starts_with("firearms:")).collect();
    assert_eq!(firearms_after, firearms_before, "no firearm is imported or changed");
}

// --- Stops before any row is saved (FR-022) --------------------------------------------------------------

fn assert_stopped_with(err: &CommandError, db: &TestDb) {
    assert_eq!(err.code, "VALIDATION_ERROR", "{err:?}");
    assert_eq!(count(&db.conn, "firearms"), 0, "no row is saved");
    assert_eq!(count(&db.conn, "accessories"), 0, "no row is saved");
}

#[test]
fn a_header_with_both_firearm_type_and_kind_stops_the_import_naming_the_file() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let good = sheets::csv_in(
        dir.path(),
        "firearms.csv",
        &firearm_table(&[firearm_cells("Glock", "19", "G1", &[])]),
    );
    let both = sheets::csv_in(
        dir.path(),
        "both.csv",
        &sheets::table_with(&["firearm_type", "kind", "make"], &[&["Handgun", "Optic", "x"]]),
    );

    let err = import_err(&db, &[&good, &both]);

    assert_stopped_with(&err, &db);
    assert_eq!(
        err.message,
        "both.csv: this isn't a HoploDex firearm or accessory table. Its header needs a firearm_type or a kind column."
    );
}

#[test]
fn a_header_with_neither_stops_the_import_naming_the_file() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let neither = sheets::csv_in(
        dir.path(),
        "neither.csv",
        &sheets::table_with(&["make", "model"], &[&["Glock", "19"]]),
    );

    let err = import_err(&db, &[&neither]);

    assert_stopped_with(&err, &db);
    assert_eq!(
        err.message,
        "neither.csv: this isn't a HoploDex firearm or accessory table. Its header needs a firearm_type or a kind column."
    );
}

#[test]
fn two_firearm_tables_stop_the_import_naming_both_files() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let first = sheets::csv_in(
        dir.path(),
        "first.csv",
        &firearm_table(&[firearm_cells("Glock", "19", "G1", &[])]),
    );
    let second = sheets::csv_in(
        dir.path(),
        "second.csv",
        &firearm_table(&[firearm_cells("Sig", "P226", "S1", &[])]),
    );

    let err = import_err(&db, &[&first, &second]);

    assert_stopped_with(&err, &db);
    assert_eq!(
        err.message,
        "first.csv and second.csv both hold firearms. Pick one firearm table and at most one accessory table."
    );
}

#[test]
fn two_accessory_tables_stop_the_import_naming_both_files() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let first =
        sheets::csv_in(dir.path(), "first.csv", &accessory_table(&[accessory_cells("Optic", &[])]));
    let second = sheets::csv_in(
        dir.path(),
        "second.csv",
        &accessory_table(&[accessory_cells("Sling", &[])]),
    );

    let err = import_err(&db, &[&first, &second]);

    assert_stopped_with(&err, &db);
    assert!(
        err.message.contains("first.csv") && err.message.contains("second.csv"),
        "{}",
        err.message
    );
}

#[test]
fn a_workbook_holding_two_firearm_sheets_stops_the_import_naming_the_file() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let table = firearm_table(&[firearm_cells("Glock", "19", "G1", &[])]);
    let path = dir.path().join("book.xlsx");
    sheets::write_workbook(&path, &[("Firearms", &table), ("More firearms", &table)]);

    let err = import_err(&db, &[&path]);

    assert_stopped_with(&err, &db);
    assert!(err.message.contains("book.xlsx"), "{}", err.message);
}

#[test]
fn a_non_blank_sheet_that_is_neither_table_stops_the_import_naming_the_file_and_the_sheet() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("book.xlsx");
    sheets::write_workbook(
        &path,
        &[
            ("Firearms", &firearm_table(&[firearm_cells("Glock", "19", "G1", &[])])),
            ("Notes", &sheets::table_with(&["Remember"], &[&["to buy ammo"]])),
        ],
    );

    let err = import_err(&db, &[&path]);

    assert_stopped_with(&err, &db);
    assert!(err.message.contains("book.xlsx") && err.message.contains("Notes"), "{}", err.message);
}

#[test]
fn three_files_stop_the_import() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let firearms = sheets::csv_in(
        dir.path(),
        "firearms.csv",
        &firearm_table(&[firearm_cells("Glock", "19", "G1", &[])]),
    );
    let accessories = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &accessory_table(&[accessory_cells("Optic", &[])]),
    );
    let more =
        sheets::csv_in(dir.path(), "more.csv", &accessory_table(&[accessory_cells("Sling", &[])]));

    let err = import_err(&db, &[&firearms, &accessories, &more]);

    assert_stopped_with(&err, &db);
}

#[test]
fn a_blank_sheet_is_ignored() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("book.xlsx");
    let blank: Table = Vec::new();
    sheets::write_workbook(
        &path,
        &[
            ("Firearms", &firearm_table(&[firearm_cells("Glock", "19", "G1", &[])])),
            ("Spare", &blank),
            ("Accessories", &accessory_table(&[accessory_cells("Optic", &[])])),
        ],
    );

    let result = import(&db, &ImportSessionStore::new(), &[&path]);

    assert!(result.row_errors.is_empty(), "{:?}", result.row_errors);
    assert_eq!(result.imported_count, 2);
}

#[test]
fn a_duplicate_known_header_in_the_accessory_table_still_stops_the_import() {
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let path = sheets::csv_in(
        dir.path(),
        "accessories.csv",
        &sheets::table_with(&["kind", "make", "make"], &[&["Optic", "a", "b"]]),
    );

    let err = import_err(&db, &[&path]);

    assert_stopped_with(&err, &db);
}
