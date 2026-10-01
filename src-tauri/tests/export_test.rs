//! Integration test: `export_collection` produces a spreadsheet + photos
//! folder matching contracts/spreadsheet-format.md (spec.md US5 Acceptance
//! Scenario 1), run against a real temporary SQLCipher database and a real
//! temporary destination folder — no mocks.

mod sheets;
mod support;

use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::{ListFirearmsInput, ops as firearm_ops};
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::commands::import_export::{ExportRecords, ExportResult};
use hoplodex_lib::models::accessory::AccessoryInput;
use hoplodex_lib::models::firearm::{FirearmInput, FirearmStatus};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use serde_json::{Value, json};
use support::{TestDb, sample_png_bytes};
use tempfile::TempDir;

/// The record set of an export of these firearms alone, no accessory.
fn firearms(ids: &[i64]) -> ExportRecords {
    ExportRecords { firearm_ids: ids.to_vec(), accessory_ids: vec![] }
}

fn firearm_with_photo(make: &str) -> FirearmInput {
    FirearmInput {
        make: make.into(),
        model: "M".into(),
        serial_number: Some(format!("{make}-SN")),
        no_serial_attested: false,
        caliber: "9mm".into(),
        firearm_type_id: 1,
        notes: Some("some notes".into()),
        accessories: None,
        barrel_length_hundredths: None,
        overall_length_hundredths: None,
        weight_tenths_oz: None,
        capacity: None,
        finish: None,
        condition: None,
        status: FirearmStatus::Active,
        estimated_value: Some(50000),
        acquisition_source: None,
        acquisition_date: None,
        acquisition_price: None,
        disposition_type: None,
        disposition_recipient: None,
        disposition_date: None,
        disposition_price: None,
        insurance_policy_id: None,
        nickname: None,
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
        mounted_on: None,
    }
}

#[test]
fn scenario_1_exports_a_spreadsheet_and_photos_folder() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();

    let f1 =
        firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Glock"), false, None).unwrap();
    let f2 =
        firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Sig"), false, None).unwrap();
    hoplodex_lib::commands::photos::ops::add_photo(
        &db.conn,
        RecordRef::Firearm(f1.id),
        &sample_png_bytes(),
        "range-day.png",
        "image/png",
    )
    .unwrap();

    let mut progress_calls = Vec::new();
    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "test-export",
        SpreadsheetFormat::Csv,
        &firearms(&[f1.id, f2.id]),
        &mut |done, total| progress_calls.push((done, total)),
    )
    .unwrap();

    assert_eq!(result.exported_firearm_count, 2);
    assert_eq!(result.exported_photo_count, 1);
    assert!(result.spreadsheet_path.exists());
    assert!(result.photos_folder_path.exists());
    assert!(!progress_calls.is_empty(), "progress must be reported for bulk export");

    let contents = std::fs::read_to_string(&result.spreadsheet_path).unwrap();
    assert!(contents.contains("Glock"));
    assert!(contents.contains("Sig"));
    assert!(contents.contains("make"), "header row must be present");

    let photo_files: Vec<_> = std::fs::read_dir(&result.photos_folder_path)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(photo_files.len(), 1);
    assert!(photo_files[0].ends_with("range-day.png"));
}

#[test]
fn exports_as_xlsx_when_requested() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let f1 =
        firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Glock"), false, None).unwrap();

    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "test-export",
        SpreadsheetFormat::Xlsx,
        &firearms(&[f1.id]),
        &mut |_, _| {},
    )
    .unwrap();

    assert!(result.spreadsheet_path.extension().unwrap() == "xlsx");
    assert!(result.spreadsheet_path.exists());
}

/// FR-039 / spreadsheet-format.md "Physical details": plain numbers with no
/// unit text and no trailing zeros, and the condition's display name,
/// between `scheduled_coverage_amount` and `photo_filenames`.
#[test]
fn physical_details_are_exported_as_plain_numbers_between_coverage_and_photos() {
    use hoplodex_lib::models::firearm::Condition;

    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let created = firearm_ops::create_firearm(
        &db.conn,
        &FirearmInput {
            barrel_length_hundredths: Some(1625),
            overall_length_hundredths: Some(1800),
            weight_tenths_oz: Some(405),
            capacity: Some(15),
            finish: Some("Cerakote".into()),
            condition: Some(Condition::LikeNew),
            ..firearm_with_photo("Glock")
        },
        false,
        None,
    )
    .unwrap();

    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "physical",
        SpreadsheetFormat::Csv,
        &firearms(&[created.id]),
        &mut |_, _| {},
    )
    .unwrap();

    let mut reader = csv::Reader::from_path(&result.spreadsheet_path).unwrap();
    let headers = reader.headers().unwrap().clone();
    let record = reader.records().next().unwrap().unwrap();
    let column = |name: &str| headers.iter().position(|h| h == name).unwrap();
    let cell = |name: &str| record.get(column(name)).unwrap();

    assert_eq!(cell("barrel_length_in"), "16.25");
    assert_eq!(cell("overall_length_in"), "18");
    assert_eq!(cell("weight_oz"), "40.5");
    assert_eq!(cell("capacity"), "15");
    assert_eq!(cell("finish"), "Cerakote");
    assert_eq!(cell("condition"), "Like new");

    let order: Vec<_> = headers.iter().collect();
    let after = column("scheduled_coverage_amount");
    assert_eq!(
        &order[after + 1..],
        [
            "barrel_length_in",
            "overall_length_in",
            "weight_oz",
            "capacity",
            "finish",
            "condition",
            "origin",
            "year_of_manufacture",
            "country_of_manufacture",
            "importer_name",
            "original_make",
            "original_model",
            "original_serial_number",
            "registered_as",
            "registration_form",
            "registration_approved",
            "registered_to",
            "mounted_on",
            "photo_filenames"
        ]
    );
}

#[test]
fn blank_physical_details_are_exported_as_blank_cells() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Sig"), false, None).unwrap();
    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "blank",
        SpreadsheetFormat::Csv,
        &firearms(&[created.id]),
        &mut |_, _| {},
    )
    .unwrap();

    let mut reader = csv::Reader::from_path(&result.spreadsheet_path).unwrap();
    let headers = reader.headers().unwrap().clone();
    let record = reader.records().next().unwrap().unwrap();
    for name in
        ["barrel_length_in", "overall_length_in", "weight_oz", "capacity", "finish", "condition"]
    {
        let cell = record.get(headers.iter().position(|h| h == name).unwrap()).unwrap();
        assert_eq!(cell, "", "{name}");
    }
}

#[test]
fn the_export_header_is_the_42_columns_with_cartridge_and_action_type_after_caliber() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let created =
        firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Glock"), false, None).unwrap();
    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "header",
        SpreadsheetFormat::Csv,
        &firearms(&[created.id]),
        &mut |_, _| {},
    )
    .unwrap();

    let mut reader = csv::Reader::from_path(&result.spreadsheet_path).unwrap();
    let headers: Vec<_> = reader.headers().unwrap().iter().map(str::to_owned).collect();
    assert_eq!(headers, hoplodex_lib::services::spreadsheet::FIREARM_COLUMNS);
    assert_eq!(headers.len(), 42);
    let caliber_at = headers.iter().position(|h| h == "caliber").unwrap();
    assert_eq!(headers[caliber_at + 1], "cartridge");
    assert_eq!(headers[caliber_at + 2], "action_type");
    assert_eq!(headers[caliber_at + 3], "firearm_type");
}

/// specs/005-regulated-item-types US4-1: the four registration columns hold
/// the classification's name, the form, the approved date and "Registered
/// to"; a Suppressor exports its type with blank action, barrel length and
/// capacity; a firearm with no classification has all four blank.
#[test]
fn export_writes_the_registration_columns_and_blanks_what_a_suppressor_lacks() {
    let db = TestDb::new();
    let dest = TempDir::new().unwrap();
    let suppressor = firearm_ops::create_firearm(
        &db.conn,
        &FirearmInput {
            firearm_type_id: 5,
            registration_class_id: Some(1),
            registration_form: Some("Form 4".into()),
            registration_approved: Some("2024-03-05".into()),
            registered_to: Some("Jane Doe".into()),
            ..firearm_with_photo("Omega")
        },
        false,
        None,
    )
    .unwrap();
    let plain =
        firearm_ops::create_firearm(&db.conn, &firearm_with_photo("Glock"), false, None).unwrap();

    let result = import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "registration",
        SpreadsheetFormat::Csv,
        &firearms(&[suppressor.id, plain.id]),
        &mut |_, _| {},
    )
    .unwrap();
    let mut reader = csv::Reader::from_path(&result.spreadsheet_path).unwrap();
    let headers = reader.headers().unwrap().clone();
    let at = |name: &str| headers.iter().position(|h| h == name).unwrap();
    let rows: Vec<_> = reader.records().map(Result::unwrap).collect();

    assert_eq!(&rows[0][at("firearm_type")], "Suppressor");
    assert_eq!(&rows[0][at("registered_as")], "Suppressor");
    assert_eq!(&rows[0][at("registration_form")], "Form 4");
    assert_eq!(&rows[0][at("registration_approved")], "2024-03-05");
    assert_eq!(&rows[0][at("registered_to")], "Jane Doe");
    for blank in ["action_type", "barrel_length_in", "capacity"] {
        assert_eq!(&rows[0][at(blank)], "", "{blank}");
    }
    for blank in ["registered_as", "registration_form", "registration_approved", "registered_to"] {
        assert_eq!(&rows[1][at(blank)], "", "{blank}");
    }
}

// --- specs/006-accessory-links US5: two tables (tasks.md T099) ------------------------------
//
// contracts/spreadsheet-format.md "Two tables", "Firearm table: new columns",
// "Accessory table: columns" and "Record ID"; contracts/tauri-commands.md
// `get_export_scope` and `export_collection`. The record set an export writes
// is `ops::export_records(conn, scope, filter)` (`"all"` or `"filtered"`, the
// collection page's filter), which `ops::get_export_scope` counts.

const OPTIC: i64 = 1;
const LIGHT_OR_LASER: i64 = 2;
const UPPER: i64 = 5;
const RIFLE: i64 = 2;
const SUPPRESSOR: i64 = 5;

const FIREARM_HEADER: [&str; 42] = [
    "record_id",
    "make",
    "model",
    "nickname",
    "serial_number",
    "no_serial_attested",
    "caliber",
    "cartridge",
    "action_type",
    "firearm_type",
    "notes",
    "accessories",
    "status",
    "estimated_value",
    "acquisition_source",
    "acquisition_date",
    "acquisition_price",
    "disposition_type",
    "disposition_recipient",
    "disposition_date",
    "disposition_price",
    "insurance_policy_name",
    "scheduled_coverage_amount",
    "barrel_length_in",
    "overall_length_in",
    "weight_oz",
    "capacity",
    "finish",
    "condition",
    "origin",
    "year_of_manufacture",
    "country_of_manufacture",
    "importer_name",
    "original_make",
    "original_model",
    "original_serial_number",
    "registered_as",
    "registration_form",
    "registration_approved",
    "registered_to",
    "mounted_on",
    "photo_filenames",
];

const ACCESSORY_HEADER: [&str; 21] = [
    "record_id",
    "kind",
    "make",
    "model",
    "serial_number",
    "caliber",
    "cartridge",
    "notes",
    "status",
    "estimated_value",
    "acquisition_source",
    "acquisition_date",
    "acquisition_price",
    "disposition_type",
    "disposition_recipient",
    "disposition_date",
    "disposition_price",
    "insurance_policy_name",
    "scheduled_coverage_amount",
    "mounted_on",
    "photo_filenames",
];

fn accessory(kind: i64, make: &str, model: &str) -> AccessoryInput {
    let mut input: AccessoryInput =
        serde_json::from_value(json!({ "accessoryKindId": kind, "status": "active" })).unwrap();
    input.make = Some(make.into());
    input.model = Some(model.into());
    input
}

fn on(input: AccessoryInput, host: RecordRef) -> AccessoryInput {
    AccessoryInput { mounted_on: Some(host), ..input }
}

fn new_firearm(db: &TestDb, input: &FirearmInput) -> RecordRef {
    RecordRef::Firearm(firearm_ops::create_firearm(&db.conn, input, false, None).unwrap().id)
}

fn new_accessory(db: &TestDb, input: &AccessoryInput) -> RecordRef {
    RecordRef::Accessory(accessory_ops::create_accessory(&db.conn, input, None).unwrap().id)
}

fn firearm_on(input: FirearmInput, host: RecordRef) -> FirearmInput {
    FirearmInput { mounted_on: Some(host), ..input }
}

fn uid_of(db: &TestDb, record: RecordRef) -> String {
    db.conn
        .query_row(
            &format!("SELECT uid FROM {} WHERE id = ?1", record.table()),
            [record.id()],
            |row| row.get(0),
        )
        .unwrap()
}

fn export_all(db: &TestDb, dest: &TempDir, base: &str, format: SpreadsheetFormat) -> ExportResult {
    let records = import_export_ops::export_records(&db.conn, "all", None).unwrap();
    import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        base,
        format,
        &records,
        &mut |_, _| {},
    )
    .unwrap()
}

fn export_filtered(db: &TestDb, dest: &TempDir, query: &str) -> ExportResult {
    let filter = ListFirearmsInput { query: Some(query.into()), ..Default::default() };
    let records = import_export_ops::export_records(&db.conn, "filtered", Some(&filter)).unwrap();
    import_export_ops::export_collection(
        &db.conn,
        dest.path(),
        "filtered",
        SpreadsheetFormat::Csv,
        &records,
        &mut |_, _| {},
    )
    .unwrap()
}

fn accessory_csv_path(result: &ExportResult) -> std::path::PathBuf {
    result.accessory_spreadsheet_path.clone().expect("an accessory file was written")
}

#[test]
fn the_firearm_header_has_record_id_first_and_mounted_on_after_registered_to() {
    let db = TestDb::new();
    new_firearm(&db, &firearm_with_photo("Glock"));
    let dest = TempDir::new().unwrap();

    let result = export_all(&db, &dest, "header", SpreadsheetFormat::Csv);

    let table = sheets::read_csv(&result.spreadsheet_path);
    assert_eq!(table[0], FIREARM_HEADER);
    assert_eq!(hoplodex_lib::services::spreadsheet::FIREARM_COLUMNS, FIREARM_HEADER);
    let at = |name: &str| sheets::column(&table, name);
    assert_eq!(at("record_id"), 0);
    assert_eq!(at("mounted_on"), at("registered_to") + 1);
    assert_eq!(at("photo_filenames"), at("mounted_on") + 1);
}

#[test]
fn the_accessory_header_is_the_21_columns_in_order() {
    let db = TestDb::new();
    new_accessory(&db, &accessory(OPTIC, "Leupold", "VX-5HD"));
    let dest = TempDir::new().unwrap();

    let result = export_all(&db, &dest, "header", SpreadsheetFormat::Csv);

    let table = sheets::read_csv(&accessory_csv_path(&result));
    assert_eq!(table[0], ACCESSORY_HEADER);
    assert_eq!(hoplodex_lib::services::spreadsheet::ACCESSORY_COLUMNS, ACCESSORY_HEADER);
}

#[test]
fn an_xlsx_export_is_one_workbook_with_sheets_firearms_and_accessories() {
    let db = TestDb::new();
    new_firearm(&db, &firearm_with_photo("Glock"));
    new_accessory(&db, &accessory(OPTIC, "Leupold", "VX-5HD"));
    let dest = TempDir::new().unwrap();

    let result = export_all(&db, &dest, "book", SpreadsheetFormat::Xlsx);

    assert_eq!(result.spreadsheet_path, dest.path().join("book.xlsx"));
    assert_eq!(result.accessory_spreadsheet_path, None);
    assert_eq!((result.exported_firearm_count, result.exported_accessory_count), (1, 1));
    let sheets = sheets::read_workbook(&result.spreadsheet_path);
    let names: Vec<_> = sheets.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["Firearms", "Accessories"]);
    assert_eq!(sheets[0].1[0], FIREARM_HEADER);
    assert_eq!(sheets[1].1[0], ACCESSORY_HEADER);
    assert_eq!(sheets[0].1.len(), 2);
    assert_eq!(sheets[1].1.len(), 2);
}

#[test]
fn a_csv_export_writes_the_accessory_file_beside_the_firearm_file_only_when_one_is_exported() {
    let db = TestDb::new();
    new_firearm(&db, &firearm_with_photo("Glock"));
    let dest = TempDir::new().unwrap();

    let without = export_all(&db, &dest, "plain", SpreadsheetFormat::Csv);
    assert_eq!(without.spreadsheet_path, dest.path().join("plain.csv"));
    assert_eq!(without.accessory_spreadsheet_path, None);
    assert_eq!(without.exported_accessory_count, 0);
    assert!(!dest.path().join("plain-accessories.csv").exists());

    new_accessory(&db, &accessory(OPTIC, "Leupold", "VX-5HD"));
    let with = export_all(&db, &dest, "full", SpreadsheetFormat::Csv);
    assert_eq!(with.spreadsheet_path, dest.path().join("full.csv"));
    assert_eq!(with.accessory_spreadsheet_path, Some(dest.path().join("full-accessories.csv")));
    assert_eq!(with.exported_accessory_count, 1);
    assert!(dest.path().join("full-accessories.csv").exists());
}

#[test]
fn an_export_of_accessories_alone_still_writes_both_tables() {
    // A collection of accessories with no firearm: the firearm table is
    // written with its header only, so the pair stays recognisable.
    let db = TestDb::new();
    new_accessory(&db, &accessory(OPTIC, "Leupold", "VX-5HD"));
    let dest = TempDir::new().unwrap();

    let result = export_all(&db, &dest, "only", SpreadsheetFormat::Csv);

    assert_eq!(result.exported_firearm_count, 0);
    assert_eq!(result.exported_accessory_count, 1);
    let firearms = sheets::read_csv(&result.spreadsheet_path);
    assert_eq!(firearms, vec![FIREARM_HEADER.iter().map(|c| c.to_string()).collect::<Vec<_>>()]);
}

#[test]
fn record_id_is_never_blank_and_is_the_records_own_identifier() {
    let db = TestDb::new();
    let rifle =
        new_firearm(&db, &FirearmInput { firearm_type_id: RIFLE, ..firearm_with_photo("A") });
    let handgun = new_firearm(&db, &firearm_with_photo("B"));
    let optic = new_accessory(&db, &accessory(OPTIC, "Leupold", "VX-5HD"));
    let bare = new_accessory(&db, &accessory(OPTIC, "Leupold", "Mark 5"));
    let dest = TempDir::new().unwrap();

    let result = export_all(&db, &dest, "ids", SpreadsheetFormat::Csv);

    let firearms = sheets::read_csv(&result.spreadsheet_path);
    let accessories = sheets::read_csv(&accessory_csv_path(&result));
    for table in [&firearms, &accessories] {
        for row in 1..table.len() {
            let id = sheets::cell(table, row - 1, "record_id");
            assert!(hoplodex_lib::services::record_id::parse(id).is_some(), "{id:?}");
        }
    }
    let find = |table: &sheets::Table, make: &str| {
        let row = table[1..].iter().position(|r| r[sheets::column(table, "make")] == make);
        sheets::cell(table, row.expect(make), "record_id").to_owned()
    };
    assert_eq!(find(&firearms, "A"), uid_of(&db, rifle));
    assert_eq!(find(&firearms, "B"), uid_of(&db, handgun));
    let optics: Vec<_> = (0..accessories.len() - 1)
        .map(|row| sheets::cell(&accessories, row, "record_id").to_owned())
        .collect();
    assert!(optics.contains(&uid_of(&db, optic)) && optics.contains(&uid_of(&db, bare)));
}

#[test]
fn mounted_on_is_the_direct_hosts_identifier_whether_a_firearm_or_an_accessory() {
    let db = TestDb::new();
    let rifle =
        new_firearm(&db, &FirearmInput { firearm_type_id: RIFLE, ..firearm_with_photo("Rifle") });
    // A firearm on a firearm, an accessory on a firearm, an accessory on an
    // accessory, and a firearm on an accessory.
    let launcher = new_firearm(&db, &firearm_on(firearm_with_photo("Launcher"), rifle));
    let light = new_accessory(&db, &on(accessory(LIGHT_OR_LASER, "SureFire", "X300"), launcher));
    let laser = new_accessory(&db, &on(accessory(LIGHT_OR_LASER, "Crimson", "Laser"), light));
    let upper = new_accessory(&db, &accessory(UPPER, "Daniel", "Upper"));
    let suppressor = new_firearm(
        &db,
        &firearm_on(
            FirearmInput { firearm_type_id: SUPPRESSOR, ..firearm_with_photo("Quiet") },
            upper,
        ),
    );
    let dest = TempDir::new().unwrap();

    let result = export_all(&db, &dest, "mounts", SpreadsheetFormat::Csv);

    let firearms = sheets::read_csv(&result.spreadsheet_path);
    let accessories = sheets::read_csv(&accessory_csv_path(&result));
    let mounted_on = |table: &sheets::Table, make: &str| {
        let row = table[1..].iter().position(|r| r[sheets::column(table, "make")] == make);
        sheets::cell(table, row.expect(make), "mounted_on").to_owned()
    };
    assert_eq!(mounted_on(&firearms, "Rifle"), "");
    assert_eq!(mounted_on(&firearms, "Launcher"), uid_of(&db, rifle));
    assert_eq!(mounted_on(&firearms, "Quiet"), uid_of(&db, upper));
    assert_eq!(mounted_on(&accessories, "SureFire"), uid_of(&db, launcher));
    assert_eq!(mounted_on(&accessories, "Crimson"), uid_of(&db, light));
    assert_eq!(mounted_on(&accessories, "Daniel"), "");
    let _ = (laser, suppressor);
}

#[test]
fn accessory_photos_are_named_with_an_a_prefix_and_counted() {
    let db = TestDb::new();
    let rifle = new_firearm(&db, &firearm_with_photo("Rifle"));
    let optic = new_accessory(&db, &accessory(OPTIC, "Leupold", "VX-5HD"));
    let firearm_photo = hoplodex_lib::commands::photos::ops::add_photo(
        &db.conn,
        rifle,
        &sample_png_bytes(),
        "range.png",
        "image/png",
    )
    .unwrap();
    let accessory_photo = hoplodex_lib::commands::photos::ops::add_photo(
        &db.conn,
        optic,
        &sample_png_bytes(),
        "range.png",
        "image/png",
    )
    .unwrap();
    let dest = TempDir::new().unwrap();

    let result = export_all(&db, &dest, "photos", SpreadsheetFormat::Csv);

    assert_eq!(result.exported_photo_count, 2);
    let mut files: Vec<_> = std::fs::read_dir(&result.photos_folder_path)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    files.sort();
    let mut expected =
        vec![format!("{}_range.png", rifle.id()), format!("a{}_range.png", optic.id())];
    expected.sort();
    assert_eq!(files, expected, "photo ids {} and {}", firearm_photo.id, accessory_photo.id);
    // The cells list the names the files were given.
    let firearms = sheets::read_csv(&result.spreadsheet_path);
    let accessories = sheets::read_csv(&accessory_csv_path(&result));
    assert_eq!(sheets::cell(&firearms, 0, "photo_filenames"), format!("{}_range.png", rifle.id()));
    assert_eq!(
        sheets::cell(&accessories, 0, "photo_filenames"),
        format!("a{}_range.png", optic.id())
    );
}

#[test]
fn an_accessory_row_holds_its_kind_policy_and_scheduled_amount() {
    use hoplodex_lib::commands::insurance::ops as insurance;
    let db = TestDb::new();
    let policy = insurance::create_policy(
        &db.conn,
        &support::policy("Home", "2020-01-01", "2099-01-01", None),
    )
    .unwrap();
    let mut input = accessory(OPTIC, "Leupold", "VX-5HD");
    input.serial_number = Some("OP-1".into());
    input.estimated_value = Some(1100);
    input.insurance_policy_id = Some(policy.id);
    input.scheduled_coverage_amount = Some(900);
    new_accessory(&db, &input);
    let dest = TempDir::new().unwrap();

    let result = export_all(&db, &dest, "row", SpreadsheetFormat::Csv);

    let table = sheets::read_csv(&accessory_csv_path(&result));
    assert_eq!(sheets::cell(&table, 0, "kind"), "Optic");
    assert_eq!(sheets::cell(&table, 0, "serial_number"), "OP-1");
    assert_eq!(sheets::cell(&table, 0, "status"), "active");
    assert_eq!(sheets::cell(&table, 0, "estimated_value"), "1100");
    assert_eq!(sheets::cell(&table, 0, "insurance_policy_name"), "Home");
    assert_eq!(sheets::cell(&table, 0, "scheduled_coverage_amount"), "900");
}

/// FR-007: the free-text `accessories` column of a firearm is exported as it
/// always was, whatever accessory records exist.
#[test]
fn the_free_text_accessories_column_is_unchanged() {
    let db = TestDb::new();
    let rifle = new_firearm(
        &db,
        &FirearmInput { accessories: Some("Bipod, sling".into()), ..firearm_with_photo("Rifle") },
    );
    new_accessory(&db, &on(accessory(OPTIC, "Leupold", "VX-5HD"), rifle));
    let dest = TempDir::new().unwrap();

    let result = export_all(&db, &dest, "free", SpreadsheetFormat::Csv);

    let firearms = sheets::read_csv(&result.spreadsheet_path);
    assert_eq!(sheets::cell(&firearms, 0, "accessories"), "Bipod, sling");
}

#[test]
fn the_whole_collection_export_holds_every_record_active_and_disposed() {
    let db = TestDb::new();
    new_firearm(&db, &firearm_with_photo("Active"));
    new_accessory(&db, &accessory(OPTIC, "Unmounted", "Optic"));
    let sold = new_accessory(&db, &accessory(OPTIC, "Sold", "Optic"));
    accessory_ops::dispose_accessory(
        &db.conn,
        sold.id(),
        &serde_json::from_value(json!({
            "dispositionType": "sold", "recipient": "Jane", "date": "2025-06-15", "price": 400
        }))
        .unwrap(),
    )
    .unwrap();
    let dest = TempDir::new().unwrap();

    let result = export_all(&db, &dest, "all", SpreadsheetFormat::Csv);

    assert_eq!((result.exported_firearm_count, result.exported_accessory_count), (1, 2));
}

// --- US5-9: the filtered export includes everything mounted on the matches --------------

/// The `make` of every data row of a table.
fn makes(table: &sheets::Table) -> Vec<String> {
    let mut makes: Vec<_> =
        (0..table.len() - 1).map(|row| sheets::cell(table, row, "make").to_owned()).collect();
    makes.sort();
    makes
}

fn filtered_makes(result: &ExportResult) -> (Vec<String>, Vec<String>) {
    let firearms = makes(&sheets::read_csv(&result.spreadsheet_path));
    let accessories = result
        .accessory_spreadsheet_path
        .as_ref()
        .map(|path| makes(&sheets::read_csv(path)))
        .unwrap_or_default();
    (firearms, accessories)
}

fn rifle(make: &str) -> FirearmInput {
    FirearmInput { firearm_type_id: RIFLE, ..firearm_with_photo(make) }
}

#[test]
fn a_filtered_export_holds_what_is_mounted_on_the_matches_firearm_or_accessory_but_not_the_rest() {
    let db = TestDb::new();
    let ruger = new_firearm(&db, &rifle("Ruger"));
    new_accessory(&db, &on(accessory(OPTIC, "Leupold", "VX-5HD"), ruger));
    // A firearm on the matched firearm comes along, though it doesn't match.
    new_firearm(
        &db,
        &firearm_on(
            FirearmInput { firearm_type_id: SUPPRESSOR, ..firearm_with_photo("SilencerCo") },
            ruger,
        ),
    );
    new_accessory(&db, &accessory(LIGHT_OR_LASER, "Streamlight", "Unmounted"));
    new_firearm(&db, &firearm_with_photo("Glock"));
    let dest = TempDir::new().unwrap();

    let result = export_filtered(&db, &dest, "Ruger");

    let (firearms, accessories) = filtered_makes(&result);
    assert_eq!(firearms, ["Ruger", "SilencerCo"]);
    assert_eq!(accessories, ["Leupold"], "the unmounted Light is not exported");
    assert_eq!((result.exported_firearm_count, result.exported_accessory_count), (2, 1));
}

#[test]
fn a_filtered_export_follows_mounts_at_every_depth() {
    let db = TestDb::new();
    let ruger = new_firearm(&db, &rifle("Ruger"));
    let launcher = new_firearm(&db, &firearm_on(firearm_with_photo("Mossberg"), ruger));
    let light = new_accessory(&db, &on(accessory(LIGHT_OR_LASER, "SureFire", "X300"), launcher));
    new_accessory(&db, &on(accessory(LIGHT_OR_LASER, "Crimson", "Laser"), light));
    let dest = TempDir::new().unwrap();

    let result = export_filtered(&db, &dest, "Ruger");

    let (firearms, accessories) = filtered_makes(&result);
    assert_eq!(firearms, ["Mossberg", "Ruger"]);
    assert_eq!(accessories, ["Crimson", "SureFire"]);
}

#[test]
fn a_filter_matching_a_mounted_firearm_leaves_its_host_out_but_the_row_still_names_it() {
    let db = TestDb::new();
    let ruger = new_firearm(&db, &rifle("Ruger"));
    let launcher = new_firearm(&db, &firearm_on(firearm_with_photo("Mossberg"), ruger));
    let light = new_accessory(&db, &on(accessory(LIGHT_OR_LASER, "SureFire", "X300"), launcher));
    new_accessory(&db, &on(accessory(LIGHT_OR_LASER, "Crimson", "Laser"), light));
    let dest = TempDir::new().unwrap();

    let result = export_filtered(&db, &dest, "Mossberg");

    let (firearms, accessories) = filtered_makes(&result);
    assert_eq!(firearms, ["Mossberg"], "not the Rifle");
    assert_eq!(accessories, ["Crimson", "SureFire"]);
    let table = sheets::read_csv(&result.spreadsheet_path);
    assert_eq!(sheets::cell(&table, 0, "mounted_on"), uid_of(&db, ruger));
}

#[test]
fn get_export_scope_counts_what_an_export_writes_and_says_what_it_discloses() {
    let db = TestDb::new();
    let ruger = new_firearm(&db, &rifle("Ruger"));
    new_accessory(&db, &on(accessory(OPTIC, "Leupold", "VX-5HD"), ruger));
    new_accessory(&db, &accessory(LIGHT_OR_LASER, "Streamlight", "Unmounted"));
    new_firearm(&db, &firearm_with_photo("Glock"));

    let all = import_export_ops::get_export_scope(&db.conn, "all", None).unwrap();
    let all = serde_json::to_value(all).unwrap();
    assert_eq!(all["firearmCount"], 2);
    assert_eq!(all["accessoryCount"], 2);
    assert_eq!(all["includesAccessories"], true);
    assert_eq!(all["includesRegistration"], false);

    let filter = ListFirearmsInput { query: Some("Ruger".into()), ..Default::default() };
    let filtered =
        import_export_ops::get_export_scope(&db.conn, "filtered", Some(&filter)).unwrap();
    let filtered = serde_json::to_value(filtered).unwrap();
    assert_eq!(filtered["firearmCount"], 1);
    assert_eq!(filtered["accessoryCount"], 1);
    assert_eq!(filtered["includesAccessories"], true);
}

#[test]
fn get_export_scope_reports_no_accessories_and_the_registration_of_the_scope() {
    let db = TestDb::new();
    new_firearm(
        &db,
        &FirearmInput {
            firearm_type_id: SUPPRESSOR,
            registration_class_id: Some(1),
            ..firearm_with_photo("Omega")
        },
    );
    new_firearm(&db, &firearm_with_photo("Glock"));
    // An accessory outside the filtered scope: only the whole collection holds one.
    new_accessory(&db, &accessory(OPTIC, "Leupold", "VX-5HD"));

    let all: Value =
        serde_json::to_value(import_export_ops::get_export_scope(&db.conn, "all", None).unwrap())
            .unwrap();
    assert_eq!(all["includesRegistration"], true);
    assert_eq!(all["includesAccessories"], true);

    let filter = ListFirearmsInput { query: Some("Glock".into()), ..Default::default() };
    let filtered: Value = serde_json::to_value(
        import_export_ops::get_export_scope(&db.conn, "filtered", Some(&filter)).unwrap(),
    )
    .unwrap();
    assert_eq!(filtered["firearmCount"], 1);
    assert_eq!(filtered["accessoryCount"], 0);
    assert_eq!(filtered["includesRegistration"], false);
    assert_eq!(filtered["includesAccessories"], false);
}
