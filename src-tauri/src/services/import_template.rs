//! The blank import spreadsheet (015 FR-002 to FR-006): the firearm and
//! accessory tables' header rows, with the columns import reads and no data
//! row, for the user to fill in and import. In a workbook each header carries
//! a note saying what the column takes, and every column is formatted as text
//! so a spreadsheet tool keeps what is typed as typed (015 research.md §4).
//! A note is not a cell, and the import reads only cells, so no note is ever
//! imported. Written with export's writers (`services::spreadsheet`).

use std::io::{Seek, Write};

use rusqlite::Connection;
use rust_xlsxwriter::{Format, Note, Workbook};

use crate::commands::CommandError;
use crate::models::firearm::{Condition, Origin};
use crate::services::spreadsheet::{
    ACCESSORY_COLUMNS, EXPORT_ONLY_COLUMNS, FIREARM_COLUMNS, TableKind, export_error,
    write_csv_table,
};

/// The fixed lists the notes name, read from the open database so a note
/// can't name a value its import refuses (015 data-model.md).
#[derive(Debug, Clone, Default)]
pub struct TemplateLists {
    pub firearm_types: Vec<String>,
    pub action_types: Vec<String>,
    /// The kinds still offered.
    pub accessory_kinds: Vec<String>,
    /// The classifications still offered.
    pub registration_classes: Vec<String>,
}

impl TemplateLists {
    pub fn read(conn: &Connection) -> Result<Self, CommandError> {
        let names = |sql: &str| -> Result<Vec<String>, CommandError> {
            let mut stmt = conn.prepare(sql).map_err(CommandError::from_db)?;
            let rows = stmt.query_map([], |row| row.get(0)).map_err(CommandError::from_db)?;
            rows.collect::<Result<_, _>>().map_err(CommandError::from_db)
        };
        Ok(Self {
            firearm_types: names("SELECT name FROM firearm_types ORDER BY sort_order")?,
            action_types: names("SELECT name FROM action_types ORDER BY sort_order")?,
            accessory_kinds: names(
                "SELECT name FROM accessory_kinds WHERE offered ORDER BY sort_order",
            )?,
            registration_classes: names(
                "SELECT name FROM registration_classes WHERE offered ORDER BY sort_order",
            )?,
        })
    }
}

/// A table's columns in the template: its column list without the
/// export-only ones, in order (015 FR-003).
pub fn columns(table: TableKind) -> Vec<&'static str> {
    let all = match table {
        TableKind::Firearms => FIREARM_COLUMNS,
        TableKind::Accessories => ACCESSORY_COLUMNS,
    };
    all.iter().copied().filter(|column| !EXPORT_ONLY_COLUMNS.contains(column)).collect()
}

/// The two tables in the order they are written, with the workbook's sheet names
/// (006 contracts/spreadsheet-format.md "Two tables").
const TABLES: [(TableKind, &str); 2] =
    [(TableKind::Firearms, "Firearms"), (TableKind::Accessories, "Accessories")];

const DOLLARS: &str =
    "Whole dollars, for example 450. A $ sign and commas are fine; cents are not.";

const DATE: &str = "as YYYY-MM-DD, for example 2024-03-01. Not a future date.";

const ORIGIN_MARKS: &str =
    "Optional. The original maker's marks, only when origin is Imported or Re-imported.";

fn one_of(values: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let values: Vec<String> = values.into_iter().map(|v| v.as_ref().to_owned()).collect();
    format!("One of: {}.", values.join(", "))
}

/// What a header's note says about its column (015 FR-005,
/// contracts/spreadsheet-format.md "The notes"), or `None` for a column the
/// template doesn't have.
pub fn note(table: TableKind, column: &str, lists: &TemplateLists) -> Option<String> {
    use TableKind::{Accessories, Firearms};
    let text = match (table, column) {
        (Firearms, "record_id") => "Optional. Leave blank for a new firearm; HoploDex gives it an ID. \
             The ID of a firearm you already have matches that firearm. To mount an accessory on a \
             firearm in this file, give the firearm a made-up ID (a version 4 UUID, such as \
             3f2b8c1e-5d4a-4e6f-9b7c-2a1d0e9f8c7b, a different one for each) and put the same ID \
             in the accessory's mounted_on."
            .to_owned(),
        (Accessories, "record_id") => "Optional. Leave blank for a new accessory; HoploDex gives it an \
             ID. The ID of an accessory you already have matches that accessory."
            .to_owned(),
        (_, "make") => "Required. The maker, for example Ruger.".to_owned(),
        (_, "model") => "Required. For example 10/22.".to_owned(),
        (Firearms, "nickname") => {
            "Optional. A name of your own for it, different from every other active firearm's."
                .to_owned()
        }
        (Firearms, "serial_number") => "Required, unless no_serial_attested is TRUE; then leave it \
             blank. As marked, leading zeros included."
            .to_owned(),
        (Accessories, "serial_number") => {
            "Optional. As marked, leading zeros included.".to_owned()
        }
        (Firearms, "no_serial_attested") => {
            "Optional. TRUE when the firearm has no serial number; otherwise FALSE or blank."
                .to_owned()
        }
        (Firearms, "caliber") => "Required, unless cartridge is given and the caliber can be worked \
             out from it (never for a Suppressor). For example 9mm."
            .to_owned(),
        (Accessories, "caliber") => {
            "Optional. Worked out from cartridge when blank, if it can be.".to_owned()
        }
        (_, "cartridge") => {
            "Optional. The cartridge it is chambered for, for example 9x19mm Parabellum.".to_owned()
        }
        (Firearms, "action_type") => format!(
            "Optional. {} It must suit firearm_type; a Suppressor has none.",
            one_of(&lists.action_types)
        ),
        (Firearms, "firearm_type") => format!("Required. {}", one_of(&lists.firearm_types)),
        (Accessories, "kind") => format!("Required. {}", one_of(&lists.accessory_kinds)),
        (_, "notes") => "Optional. Any text.".to_owned(),
        (Firearms, "accessories") => "Optional. Any text, such as a list separated by semicolons. To \
             record an accessory as an item of its own, use the Accessories sheet."
            .to_owned(),
        (_, "status") => "Optional. active (the default) or disposed.".to_owned(),
        (_, "estimated_value") => format!("Optional. {DOLLARS}"),
        (_, "acquisition_source") => {
            "Optional. Where it came from, for example a shop or a person.".to_owned()
        }
        (_, "acquisition_date") => format!("Optional. The date it was acquired, {DATE}"),
        (_, "acquisition_price") => format!("Optional. {DOLLARS}"),
        (_, "disposition_type") => {
            "Required when status is disposed. One of: sold, traded, gifted, destroyed, lost_stolen."
                .to_owned()
        }
        (_, "disposition_recipient") => {
            "Required when status is disposed. Who it went to.".to_owned()
        }
        (_, "disposition_date") => "Required when status is disposed. YYYY-MM-DD, not a future date \
             and not before acquisition_date."
            .to_owned(),
        (_, "disposition_price") => format!("Optional, even when disposed. {DOLLARS}"),
        (_, "insurance_policy_name") => "Optional. The name of an insurance policy already in this \
             database, to schedule it under that policy."
            .to_owned(),
        (_, "scheduled_coverage_amount") => format!(
            "Required when insurance_policy_name is given; blank otherwise. {DOLLARS}"
        ),
        (Firearms, "barrel_length_in") => "Optional. Inches, up to two decimal places, for example \
             16.25. Not for a Suppressor."
            .to_owned(),
        (Firearms, "overall_length_in") => {
            "Optional. Inches, up to two decimal places, for example 36.5.".to_owned()
        }
        (Firearms, "weight_oz") => {
            "Optional. Ounces, up to one decimal place, for example 40.5.".to_owned()
        }
        (Firearms, "capacity") => {
            "Optional. A whole number of rounds, 1 or more. Not for a Suppressor.".to_owned()
        }
        (Firearms, "finish") => "Optional. For example Blued.".to_owned(),
        (Firearms, "condition") => {
            format!("Optional. {}", one_of(Condition::ALL.iter().map(Condition::label)))
        }
        (Firearms, "origin") => format!(
            "Optional. {} Blank when not known.",
            one_of([Origin::Domestic, Origin::Imported, Origin::Reimported].map(|o| o.label()))
        ),
        (Firearms, "year_of_manufacture") => {
            "Optional. A four-digit year, for example 1943.".to_owned()
        }
        (Firearms, "country_of_manufacture") => "Optional. Only when origin is Imported.".to_owned(),
        (Firearms, "importer_name") => {
            "Optional. Only when origin is Imported or Re-imported.".to_owned()
        }
        (Firearms, "original_make" | "original_model" | "original_serial_number") => {
            ORIGIN_MARKS.to_owned()
        }
        (Firearms, "registered_as") => format!("Optional. {}", one_of(&lists.registration_classes)),
        (Firearms, "registration_form") => {
            "Optional. For example Form 4. Needs registered_as.".to_owned()
        }
        (Firearms, "registration_approved") => {
            format!("Optional. The date it was approved, {DATE} Needs registered_as.")
        }
        (Firearms, "registered_to") => {
            "Optional. Who it is registered to. Needs registered_as.".to_owned()
        }
        (Firearms, "mounted_on") => "Optional. To record this firearm as mounted on another item, \
             that item's record_id, from either sheet or from your collection."
            .to_owned(),
        (Accessories, "mounted_on") => "Optional. The record_id of the firearm or accessory it is \
             mounted on, from either sheet or from your collection. To mount it on a firearm in \
             this file, give that firearm a made-up record_id and use the same one here. Or leave \
             it blank and mount it in HoploDex after the import."
            .to_owned(),
        _ => return None,
    };
    Some(text)
}

/// The template as a workbook: a "Firearms" and an "Accessories" sheet, each
/// with its header row and no data row, a note on every header, and every
/// column formatted as text (015 FR-005, FR-006).
pub fn workbook(lists: &TemplateLists) -> Result<Workbook, CommandError> {
    let failed = |e| export_error("Failed writing the blank spreadsheet", e);
    let mut workbook = Workbook::new();
    // Excel's built-in Text format (`@`), which every spreadsheet tool knows by number.
    let text = Format::new().set_num_format_index(49);
    for (table, name) in TABLES {
        let sheet = workbook.add_worksheet();
        sheet.set_name(name).map_err(failed)?;
        for (col, column) in columns(table).into_iter().enumerate() {
            let col = col as u16;
            let note = note(table, column, lists).ok_or_else(|| {
                CommandError::new("INTERNAL_ERROR", format!("The column {column} has no note."))
            })?;
            // Room for the text at a note's small type, about 48 characters a line.
            let height = 20 + 13 * (note.chars().count() as u32 / 48 + 1);
            sheet.set_column_format(col, &text).map_err(failed)?;
            sheet.set_column_width(col, column.len().max(10) as f64 + 2.0).map_err(failed)?;
            sheet.write_string(0, col, column).map_err(failed)?;
            let note = Note::new(note).add_author_prefix(false).set_width(320).set_height(height);
            sheet.insert_note(0, col, &note).map_err(failed)?;
        }
    }
    Ok(workbook)
}

/// Writes [`workbook`] to `out`, a file created new for it.
pub fn write_workbook(
    out: impl Write + Seek + Send,
    lists: &TemplateLists,
) -> Result<(), CommandError> {
    workbook(lists)?
        .save_to_writer(out)
        .map_err(|e| export_error("Failed saving the blank spreadsheet", e))
}

/// Writes the template as CSV: each table's header line, and nothing else
/// (015 FR-005), to files created new for them.
pub fn write_csv(firearms: impl Write, accessories: impl Write) -> Result<(), CommandError> {
    write_csv_table::<0>(firearms, &columns(TableKind::Firearms), std::iter::empty())?;
    write_csv_table::<0>(accessories, &columns(TableKind::Accessories), std::iter::empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::spreadsheet::{TableRows, read_table};

    fn lists() -> TemplateLists {
        TemplateLists {
            firearm_types: vec!["TypeA".into(), "TypeB".into()],
            action_types: vec!["ActionA".into()],
            accessory_kinds: vec!["KindA".into(), "KindB".into()],
            registration_classes: vec!["ClassA".into()],
        }
    }

    #[test]
    fn the_template_has_every_column_but_the_export_only_ones_in_order() {
        // 015 FR-003.
        let firearms = columns(TableKind::Firearms);
        assert_eq!(firearms.len(), FIREARM_COLUMNS.len() - 1);
        assert_eq!(firearms.first(), Some(&"record_id"));
        assert_eq!(firearms.last(), Some(&"mounted_on"));
        assert!(!firearms.contains(&"photo_filenames"));
        let accessories = columns(TableKind::Accessories);
        assert_eq!(accessories.len(), ACCESSORY_COLUMNS.len() - 1);
        assert!(!accessories.contains(&"photo_filenames"));
        for (table, all) in
            [(TableKind::Firearms, FIREARM_COLUMNS), (TableKind::Accessories, ACCESSORY_COLUMNS)]
        {
            let kept: Vec<&str> =
                all.iter().copied().filter(|c| !EXPORT_ONLY_COLUMNS.contains(c)).collect();
            assert_eq!(columns(table), kept);
        }
    }

    #[test]
    fn import_reads_every_column_of_the_template() {
        // 015 SC-004: a value in every template column reaches the import's row,
        // so the template has no column import ignores. One import reads that
        // isn't in the column list panics in `from_cells`, so it can't be missed.
        for (table, _) in TABLES {
            let headers: Vec<String> = columns(table).into_iter().map(str::to_owned).collect();
            let values: Vec<String> = (0..headers.len()).map(|i| format!("value{i}x")).collect();
            let read =
                read_table(&headers, std::iter::once(values.clone()), "t.csv".into()).unwrap().rows;
            let row = match (table, &read) {
                (TableKind::Firearms, TableRows::Firearms(rows)) => format!("{:?}", rows[0]),
                (TableKind::Accessories, TableRows::Accessories(rows)) => {
                    format!("{:?}", rows[0])
                }
                _ => panic!("{table:?} read as the other table"),
            };
            for (value, column) in values.iter().zip(&headers) {
                assert!(row.contains(&format!("\"{value}\"")), "{table:?} {column} isn't read");
            }
        }
    }

    #[test]
    fn every_column_of_the_template_has_a_note() {
        // 015 FR-005.
        for (table, _) in TABLES {
            for column in columns(table) {
                let note = note(table, column, &lists());
                assert!(note.is_some_and(|n| n.len() > 10), "{table:?} {column}");
            }
        }
        assert_eq!(note(TableKind::Firearms, "photo_filenames", &lists()), None);
        assert_eq!(note(TableKind::Accessories, "firearm_type", &lists()), None);
    }

    #[test]
    fn the_notes_name_the_lists_they_are_given() {
        // 015 FR-005: the fixed lists come from the open database.
        let lists = lists();
        let note = |table, column| note(table, column, &lists).unwrap();
        assert!(note(TableKind::Firearms, "firearm_type").contains("One of: TypeA, TypeB."));
        assert!(note(TableKind::Firearms, "action_type").contains("One of: ActionA."));
        assert!(note(TableKind::Accessories, "kind").contains("One of: KindA, KindB."));
        assert!(note(TableKind::Firearms, "registered_as").contains("One of: ClassA."));
        assert!(note(TableKind::Firearms, "condition").contains("New in box, Like new"));
        assert!(note(TableKind::Firearms, "origin").contains("Domestic, Imported, Re-imported"));
    }
}
