//! 015 (the blank import spreadsheet) through `ops::save_import_template`,
//! against real temporary SQLCipher databases and real files: the template's
//! tables and headers in both formats (015 US1-3, FR-002), its names
//! (015 US1-4, FR-007), that it holds and changes nothing of the collection
//! (015 FR-004), that it imports untouched with nothing added and filled in
//! as an export would (015 US2, FR-010), read through the real import helper,
//! and the workbook's notes and text columns (015 US3, FR-005, FR-006).

use crate::sheets;
use crate::support;

use std::io::Read;
use std::path::{Path, PathBuf};

use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::ops::{self as import_export_ops, TEMPLATE_BASE_NAME};
use hoplodex_lib::commands::import_export::{
    ImportResult, ImportSessionStore, ImportTemplateResult,
};
use hoplodex_lib::services::import_template::{self, TemplateLists};
use hoplodex_lib::services::record_id;
use hoplodex_lib::services::sandbox::Reach;
use hoplodex_lib::services::spreadsheet::{SpreadsheetFormat, TableKind};
use rusqlite::Connection;
use support::TestDb;
use tempfile::TempDir;

fn save(db: &TestDb, folder: &Path, format: SpreadsheetFormat) -> ImportTemplateResult {
    import_export_ops::save_import_template(&db.conn, folder, format, &Reach::Unconfined).unwrap()
}

fn header(table: TableKind) -> Vec<String> {
    import_template::columns(table).into_iter().map(str::to_owned).collect()
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

fn import(db: &TestDb, paths: &[&Path]) -> ImportResult {
    sheets::import(&db.conn, &ImportSessionStore::new(), paths).unwrap()
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0)).unwrap()
}

/// A database holding one firearm, whose make the template must never carry.
fn db_with_a_firearm() -> TestDb {
    let db = TestDb::new();
    firearm_ops::create_firearm(
        &db.conn,
        &support::firearm("PrivateMake", "19", "PM-1"),
        false,
        None,
    )
    .unwrap();
    db
}

/// A row of `table`'s template columns from named cells, blank elsewhere.
fn row(table: TableKind, cells: &[(&str, &str)]) -> Vec<String> {
    sheets::row_of(&import_template::columns(table), cells)
}

// --- 015 US1: the files ------------------------------------------------------------------------

#[test]
fn a_workbook_template_holds_the_two_tables_headers_only() {
    // 015 US1-3, FR-002, FR-003.
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let saved = save(&db, dir.path(), SpreadsheetFormat::Xlsx);
    assert_eq!(file_name(&saved.spreadsheet_path), "hoplodex-import-template.xlsx");
    assert_eq!(saved.accessory_spreadsheet_path, None);

    let sheets = sheets::read_workbook(&saved.spreadsheet_path);
    let names: Vec<&str> = sheets.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["Firearms", "Accessories"]);
    assert_eq!(sheets[0].1, vec![header(TableKind::Firearms)]);
    assert_eq!(sheets[1].1, vec![header(TableKind::Accessories)]);
}

#[test]
fn a_csv_template_is_the_two_files_header_lines() {
    // 015 US1-3, FR-002, FR-003.
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let saved = save(&db, dir.path(), SpreadsheetFormat::Csv);
    let accessories = saved.accessory_spreadsheet_path.unwrap();
    assert_eq!(file_name(&saved.spreadsheet_path), "hoplodex-import-template.csv");
    assert_eq!(file_name(&accessories), "hoplodex-import-template-accessories.csv");
    assert_eq!(sheets::read_csv(&saved.spreadsheet_path), vec![header(TableKind::Firearms)]);
    assert_eq!(sheets::read_csv(&accessories), vec![header(TableKind::Accessories)]);
}

#[test]
fn a_second_save_takes_a_new_name_and_leaves_the_first_as_it_was() {
    // 015 US1-4, FR-007.
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    for format in [SpreadsheetFormat::Xlsx, SpreadsheetFormat::Csv] {
        let first = save(&db, dir.path(), format);
        let before = std::fs::read(&first.spreadsheet_path).unwrap();
        let second = save(&db, dir.path(), format);
        let extension = format.extension();
        assert_eq!(
            file_name(&second.spreadsheet_path),
            format!("{TEMPLATE_BASE_NAME}-2.{extension}")
        );
        if let Some(accessories) = &second.accessory_spreadsheet_path {
            assert_eq!(file_name(accessories), format!("{TEMPLATE_BASE_NAME}-2-accessories.csv"));
        }
        assert_eq!(std::fs::read(&first.spreadsheet_path).unwrap(), before);
    }
}

#[test]
fn a_name_partly_taken_is_skipped_and_what_was_there_is_untouched() {
    // 015 FR-007: the accessory file's name is the user's own file, so the
    // firearm file made for that name is removed and the next name is used.
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let theirs = dir.path().join("hoplodex-import-template-accessories.csv");
    std::fs::write(&theirs, "theirs").unwrap();

    let saved = save(&db, dir.path(), SpreadsheetFormat::Csv);
    assert_eq!(file_name(&saved.spreadsheet_path), "hoplodex-import-template-2.csv");
    assert_eq!(std::fs::read_to_string(&theirs).unwrap(), "theirs");
    let mut names: Vec<String> =
        std::fs::read_dir(dir.path()).unwrap().map(|e| file_name(&e.unwrap().path())).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "hoplodex-import-template-2-accessories.csv",
            "hoplodex-import-template-2.csv",
            "hoplodex-import-template-accessories.csv",
        ]
    );
}

#[test]
fn saving_changes_nothing_in_the_database_and_carries_nothing_of_it() {
    // 015 FR-004.
    let db = db_with_a_firearm();
    let dir = TempDir::new().unwrap();
    let changes = db.conn.total_changes();
    let csv = save(&db, dir.path(), SpreadsheetFormat::Csv);
    let xlsx = save(&db, dir.path(), SpreadsheetFormat::Xlsx);
    assert_eq!(db.conn.total_changes(), changes);

    let mut text = std::fs::read_to_string(&csv.spreadsheet_path).unwrap();
    text += &std::fs::read_to_string(csv.accessory_spreadsheet_path.unwrap()).unwrap();
    text += &parts_text(&xlsx.spreadsheet_path);
    assert!(!text.contains("PrivateMake"));
    assert!(!text.contains("PM-1"));
}

// --- 015 US2: importing the template -----------------------------------------------------------

fn assert_nothing_imported(result: &ImportResult) {
    assert_eq!(result.imported_count, 0);
    assert_eq!(result.updated_count, 0);
    assert_eq!(result.skipped_count, 0);
    assert!(result.row_errors.is_empty(), "{:?}", result.row_errors);
    assert!(result.conflicts.is_empty());
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
}

#[test]
fn a_blank_template_imports_nothing_in_either_format() {
    // 015 US2-1, US3-2, FR-010: into an empty database and one with a record.
    for db in [TestDb::new(), db_with_a_firearm()] {
        let firearms = count(&db.conn, "firearms");
        let dir = TempDir::new().unwrap();
        let xlsx = save(&db, dir.path(), SpreadsheetFormat::Xlsx);
        assert_nothing_imported(&import(&db, &[&xlsx.spreadsheet_path]));
        let csv = save(&db, dir.path(), SpreadsheetFormat::Csv);
        let accessories = csv.accessory_spreadsheet_path.unwrap();
        assert_nothing_imported(&import(&db, &[&csv.spreadsheet_path, &accessories]));
        assert_eq!(count(&db.conn, "firearms"), firearms);
        assert_eq!(count(&db.conn, "accessories"), 0);
    }
}

/// A firearm row with a made-up identifier, an accessory mounted on it by
/// that identifier, and a firearm row with no make, as a user fills them in.
fn filled_rows(uid: &str) -> (Vec<Vec<String>>, Vec<Vec<String>>) {
    let firearm = row(
        TableKind::Firearms,
        &[
            ("record_id", uid),
            ("make", "TemplateMake"),
            ("model", "T1"),
            ("serial_number", "00123"),
            ("caliber", "9mm"),
            ("firearm_type", "Handgun"),
            ("acquisition_date", "2024-03-01"),
        ],
    );
    let invalid = row(
        TableKind::Firearms,
        &[
            ("model", "NoMake"),
            ("serial_number", "NM-1"),
            ("caliber", "9mm"),
            ("firearm_type", "Handgun"),
        ],
    );
    let accessory = row(
        TableKind::Accessories,
        &[("kind", "Optic"), ("make", "OpticMake"), ("model", "O1"), ("mounted_on", uid)],
    );
    (vec![firearm, invalid], vec![accessory])
}

fn assert_filled_import(db: &TestDb, result: &ImportResult, uid: &str) {
    // 015 US2-2, US2-3.
    assert_eq!(result.imported_count, 2, "{:?}", result.row_errors);
    assert_eq!(result.imported_accessory_count, 1);
    let errors = sheets::entries(&result.row_errors);
    assert_eq!(errors.len(), 1);
    assert_eq!((errors[0].0.as_str(), errors[0].1), ("firearms", 2));
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);

    let (serial, acquired): (String, String) = db
        .conn
        .query_row(
            "SELECT serial_number, acquisition_date FROM firearms WHERE uid = ?1",
            [uid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((serial.as_str(), acquired.as_str()), ("00123", "2024-03-01"));
    let host: String = db
        .conn
        .query_row(
            "SELECT f.uid FROM mounts m JOIN accessories a ON a.id = m.item_accessory_id
             JOIN firearms f ON f.id = m.host_firearm_id WHERE a.make = 'OpticMake'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(host, uid);
}

#[test]
fn a_filled_workbook_template_imports_its_rows_and_makes_the_mount() {
    // 015 US2-2, US2-3: filled through the template's own workbook, so its
    // notes and text columns are in the file the import reads.
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let uid = record_id::generate();
    let (firearms, accessories) = filled_rows(&uid);
    let mut workbook = import_template::workbook(&TemplateLists::read(&db.conn).unwrap()).unwrap();
    for (name, rows) in [("Firearms", &firearms), ("Accessories", &accessories)] {
        let sheet = workbook.worksheet_from_name(name).unwrap();
        for (r, cells) in rows.iter().enumerate() {
            for (c, value) in cells.iter().enumerate().filter(|(_, v)| !v.is_empty()) {
                sheet.write_string(r as u32 + 1, c as u16, value).unwrap();
            }
        }
    }
    let path = dir.path().join("filled.xlsx");
    workbook.save(&path).unwrap();

    let result = import(&db, &[&path]);
    assert_filled_import(&db, &result, &uid);
}

#[test]
fn a_filled_csv_template_imports_its_rows_and_makes_the_mount() {
    // 015 US2-2 to US2-4: rows added under the saved files' header lines, the
    // two files picked together.
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let saved = save(&db, dir.path(), SpreadsheetFormat::Csv);
    let accessory_path = saved.accessory_spreadsheet_path.clone().unwrap();
    let uid = record_id::generate();
    let (firearms, accessories) = filled_rows(&uid);
    append_rows(&saved.spreadsheet_path, &firearms);
    append_rows(&accessory_path, &accessories);

    let result = import(&db, &[&saved.spreadsheet_path, &accessory_path]);
    assert_filled_import(&db, &result, &uid);
}

fn append_rows(path: &PathBuf, rows: &[Vec<String>]) {
    let mut table = sheets::read_csv(path);
    table.extend_from_slice(rows);
    sheets::write_csv(path, &table);
}

// --- 015 US3: the workbook's notes and text columns --------------------------------------------

/// Every part of the workbook, as text.
fn parts(path: &Path) -> Vec<(String, String)> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    (0..archive.len())
        .map(|i| {
            let mut part = archive.by_index(i).unwrap();
            let mut text = String::new();
            part.read_to_string(&mut text).unwrap();
            (part.name().unwrap().to_string(), text)
        })
        .collect()
}

fn parts_text(path: &Path) -> String {
    parts(path).into_iter().map(|(_, text)| text).collect()
}

fn part<'a>(parts: &'a [(String, String)], name: &str) -> &'a str {
    &parts.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("no part {name}")).1
}

/// The cell references `<comment ref="…">` names in a comments part.
fn comment_refs(xml: &str) -> Vec<String> {
    xml.split("<comment ref=\"")
        .skip(1)
        .map(|rest| rest.split('"').next().unwrap().to_owned())
        .collect()
}

/// A number attribute of an XML element's text, `min="3"`.
fn attribute(element: &str, name: &str) -> usize {
    let start = element.find(&format!("{name}=\"")).unwrap() + name.len() + 2;
    element[start..].split('"').next().unwrap().parse().unwrap()
}

fn column_letters(n: usize) -> String {
    let (mut n, mut letters) = (n + 1, String::new());
    while n > 0 {
        letters.insert(0, (b'A' + ((n - 1) % 26) as u8) as char);
        n = (n - 1) / 26;
    }
    letters
}

#[test]
fn every_header_of_the_workbook_has_its_note_and_every_column_is_text() {
    // 015 US3-1, FR-005, FR-006.
    let db = TestDb::new();
    let dir = TempDir::new().unwrap();
    let saved = save(&db, dir.path(), SpreadsheetFormat::Xlsx);
    let parts = parts(&saved.spreadsheet_path);
    let lists = TemplateLists::read(&db.conn).unwrap();

    for (n, table) in [(1, TableKind::Firearms), (2, TableKind::Accessories)] {
        let columns = import_template::columns(table);
        let comments = part(&parts, &format!("xl/comments{n}.xml"));
        let expected: Vec<String> =
            (0..columns.len()).map(|c| format!("{}1", column_letters(c))).collect();
        assert_eq!(comment_refs(comments), expected, "{table:?}");
        for column in &columns {
            let note = import_template::note(table, column, &lists).unwrap();
            // The text up to the first character XML would escape.
            let plain: String = note.chars().take_while(|c| !"&<>\"'".contains(*c)).collect();
            assert!(comments.contains(&plain), "{table:?} {column}: {plain}");
        }

        // Every column has a style (neighbours of one width share a <col>), and
        // the one style the workbook gives columns has the number format Text (49).
        let sheet = part(&parts, &format!("xl/worksheets/sheet{n}.xml"));
        let styled: Vec<(usize, usize)> = sheet
            .split("<col ")
            .skip(1)
            .filter(|col| col.contains("style=\""))
            .map(|col| (attribute(col, "min"), attribute(col, "max")))
            .collect();
        for c in 1..=columns.len() {
            assert!(
                styled.iter().any(|(min, max)| (*min..=*max).contains(&c)),
                "{table:?} column {c}"
            );
        }
    }
    assert!(part(&parts, "xl/styles.xml").contains("numFmtId=\"49\""));
}

#[test]
fn the_notes_name_the_open_databases_lists() {
    // 015 FR-005: a kind no longer offered isn't named.
    let db = TestDb::new();
    db.conn.execute("UPDATE accessory_kinds SET offered = 0 WHERE name = 'Sling'", []).unwrap();
    let lists = TemplateLists::read(&db.conn).unwrap();
    let kind = import_template::note(TableKind::Accessories, "kind", &lists).unwrap();
    assert!(kind.contains("Optic"), "{kind}");
    assert!(!kind.contains("Sling"), "{kind}");
    let types = import_template::note(TableKind::Firearms, "firearm_type", &lists).unwrap();
    assert!(types.contains("Handgun, Rifle, Shotgun, Suppressor, Other."), "{types}");
}
