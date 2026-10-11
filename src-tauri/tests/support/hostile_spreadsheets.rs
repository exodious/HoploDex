//! Generators for hostile and malformed import files (feature 008, T049,
//! research.md §8 and §9).
//!
//! Nothing is checked in and nothing is built in memory: each generator
//! streams its file into a folder the caller gives, so a test asks for one
//! case at a time and deletes it. The corpus is [`corpus`]: a list of
//! [`Case`]s, each with the outcome the reader must give for it
//! ([`Expect`]) and a `build` that writes the import's file or files into a
//! folder and returns their paths in the order they are imported. The
//! generators are also public for a test that wants one file, and
//! [`valid_export_workbook`], [`valid_export_csv`] and [`large_valid_csv`]
//! make files the reader must accept.
//!
//! Every table's header is `FIREARM_COLUMNS`, so a file that gets as far as
//! recognition is a firearm table and the limit, not "this isn't a HoploDex
//! table", is what stops it.
//!
//! The zip bomb's compressed stream is written by hand (dynamic Huffman, one
//! bit for the 258-byte match and one for its distance, so 1,032:1, the most
//! deflate can do): compressing a gibibyte for each run would take most of a
//! minute in a debug build.
#![allow(dead_code)]

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use hoplodex_lib::services::import_reader::limits::{
    ImportLimit, MAX_CELL_CHARS, MAX_COLUMNS, MAX_FILE_BYTES, MAX_ROWS, MAX_SHARED_STRINGS,
    MAX_SHEETS, MAX_TOTAL_TEXT_BYTES, MAX_UNPACKED_BYTES, MAX_ZIP_ENTRIES,
};
use hoplodex_lib::services::spreadsheet::FIREARM_COLUMNS;
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

/// What reading a case's files must give.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    /// `IMPORT_LIMIT_EXCEEDED` for the file at `file` (its index in the
    /// case's files), with one of `limits` (more than one only where the
    /// file passes several at once and which is found first is the reader's
    /// choice) and, for a workbook's sheet-level limit, the sheet's name.
    Refused { file: usize, limits: &'static [ImportLimit], sheet: Option<&'static str> },
    /// `VALIDATION_ERROR` "<file>: could not read the file."
    Unreadable,
}

impl Expect {
    /// Is this refusal one the case allows?
    pub fn accepts_refusal(&self, file: usize, limit: ImportLimit, sheet: Option<&str>) -> bool {
        match self {
            Self::Refused { file: f, limits, sheet: s } => {
                *f == file && limits.contains(&limit) && *s == sheet
            }
            Self::Unreadable => false,
        }
    }
}

/// One file or pair of files of the corpus.
#[derive(Clone, Copy)]
pub struct Case {
    /// A short name, for failure messages and a folder name.
    pub name: &'static str,
    pub expect: Expect,
    build: fn(&Path) -> Vec<PathBuf>,
}

impl Case {
    /// Writes the case's files into `dir` (which must exist) and returns
    /// their paths in the order they are imported.
    pub fn build(&self, dir: &Path) -> Vec<PathBuf> {
        (self.build)(dir)
    }
}

const fn refused(
    limit: &'static [ImportLimit],
    sheet: Option<&'static str>,
    build: fn(&Path) -> Vec<PathBuf>,
    name: &'static str,
) -> Case {
    Case { name, expect: Expect::Refused { file: 0, limits: limit, sheet }, build }
}

const fn refused_second(
    limit: &'static [ImportLimit],
    build: fn(&Path) -> Vec<PathBuf>,
    name: &'static str,
) -> Case {
    Case { name, expect: Expect::Refused { file: 1, limits: limit, sheet: None }, build }
}

const fn unreadable(build: fn(&Path) -> Vec<PathBuf>, name: &'static str) -> Case {
    Case { name, expect: Expect::Unreadable, build }
}

/// Every hostile case (T049), each with the outcome it must give.
pub fn corpus() -> Vec<Case> {
    use ImportLimit::*;
    vec![
        // Workbooks.
        refused(&[SharedStrings], None, |d| one(shared_strings_u64_max(d)), "shared-u64-max"),
        refused(&[SharedStrings], None, |d| one(shared_strings_2000001(d)), "shared-2000001"),
        refused(&[Columns, Rows], Some("Sheet1"), |d| one(corner_cells(d)), "corner-cells"),
        refused(&[UnpackedSize], None, |d| one(zip_bomb(d)), "zip-bomb"),
        refused(&[ZipEntries], None, |d| one(zip_entries_1001(d)), "zip-entries"),
        refused(&[Sheets], None, |d| one(sheets_17(d)), "sheets-17"),
        refused(&[Rows], Some("Sheet1"), |d| one(rows_100001_xlsx(d)), "rows-xlsx"),
        refused(&[Columns], Some("Sheet1"), |d| one(columns_257_xlsx(d)), "columns-xlsx"),
        refused(&[Cells], Some("Sheet13"), |d| one(cells_6_4m(d)), "cells-16-sheets"),
        refused(&[Cells], Some("Sheet1"), |d| one(cells_sparse(d)), "cells-sparse"),
        refused(&[CellText], Some("Sheet1"), |d| one(cell_32768_xlsx(d)), "cell-text-xlsx"),
        refused(&[TotalText], Some("Sheet1"), |d| one(text_over_256mib(d)), "total-text"),
        // CSV.
        refused(&[Rows], None, |d| one(rows_100001_csv(d)), "rows-csv"),
        refused(&[Columns], None, |d| one(columns_257_csv(d)), "columns-csv"),
        refused(&[CellText], None, |d| one(cell_32768_csv(d)), "cell-text-csv"),
        refused(&[CellText], None, |d| one(field_200mb_csv(d)), "field-200mb"),
        refused(&[FileSize], None, |d| one(field_300mb_csv(d)), "field-300mb"),
        refused(&[Cells], None, |d| one(commas_25_6m_cells(d)), "commas"),
        refused(&[FileSize], None, |d| one(csv_over_256mib(d)), "csv-over-256mib"),
        // Not workbooks.
        unreadable(|d| one(truncated_workbook(d)), "truncated"),
        unreadable(|d| one(truncated_directory(d)), "truncated-directory"),
        unreadable(|d| one(bit_flipped_workbook(d)), "bit-flipped"),
        unreadable(|d| one(bit_flipped_data(d)), "bit-flipped-data"),
        unreadable(|d| one(bit_flipped_directory(d)), "bit-flipped-directory"),
        unreadable(|d| one(archive_not_a_spreadsheet(d)), "not-a-spreadsheet"),
        // Two files.
        refused_second(&[Rows], second_file_passes_a_limit, "two-files-second-rows"),
        refused_second(&[Cells], cells_pass_together, "two-files-cells-together"),
    ]
}

fn one(path: PathBuf) -> Vec<PathBuf> {
    vec![path]
}

// ---------------------------------------------------------------------------
// Valid files

const HEADER_ROW_CELLS: usize = FIREARM_COLUMNS.len();

/// A small export-shaped workbook ("Firearms" sheet, shared strings, written
/// by `rust_xlsxwriter` as the app's export does) that the reader must accept:
/// three data rows.
pub fn valid_export_workbook(path: &Path) -> PathBuf {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Firearms").expect("sheet name");
    for (col, header) in FIREARM_COLUMNS.iter().enumerate() {
        sheet.write_string(0, col as u16, *header).expect("header");
    }
    for row in 1..=3u32 {
        for (col, header) in FIREARM_COLUMNS.iter().enumerate() {
            let value = match *header {
                "record_id" => format!("00000000-0000-4000-8000-00000000000{row}"),
                "firearm_type" => "rifle".to_owned(),
                "make" => "Springfield".to_owned(),
                "model" => format!("M1 Garand {row}"),
                _ => String::new(),
            };
            if !value.is_empty() {
                sheet.write_string(row, col as u16, value).expect("cell");
            }
        }
    }
    workbook.save(path).expect("save the valid workbook");
    path.to_owned()
}

/// A small export-shaped CSV the reader must accept: three data rows.
pub fn valid_export_csv(path: &Path) -> PathBuf {
    let mut out = csv_file(path);
    header_line(&mut out);
    for row in 1..=3 {
        let cells: Vec<String> = FIREARM_COLUMNS
            .iter()
            .map(|header| match *header {
                "record_id" => format!("00000000-0000-4000-8000-00000000000{row}"),
                "firearm_type" => "rifle".to_owned(),
                "make" => "Springfield".to_owned(),
                "model" => format!("M1 Garand {row}"),
                _ => String::new(),
            })
            .collect();
        writeln!(out, "{}", cells.join(",")).unwrap();
    }
    finish(out);
    path.to_owned()
}

/// A large CSV within every limit, for a test that stops or locks in the
/// middle of reading: `data_rows` (at most 100,000) rows of all 42 columns
/// with 16-character cells, about 700 bytes a row, 70 MB at the most.
pub fn large_valid_csv(path: &Path, data_rows: usize) -> PathBuf {
    assert!(data_rows <= MAX_ROWS, "a valid file has at most {MAX_ROWS} data rows");
    let mut out = csv_file(path);
    header_line(&mut out);
    for row in 0..data_rows {
        let cells: Vec<String> = FIREARM_COLUMNS
            .iter()
            .enumerate()
            .map(|(col, header)| match *header {
                "firearm_type" => "rifle".to_owned(),
                _ => format!("r{row:07}c{col:06}x"),
            })
            .collect();
        writeln!(out, "{}", cells.join(",")).unwrap();
    }
    finish(out);
    path.to_owned()
}

/// A valid export-shaped workbook of `data_rows` rows of all 42 columns with
/// 16-character cells, written by `rust_xlsxwriter` as the app's export does:
/// big enough that reading it reports progress more than once.
pub fn large_valid_workbook(path: &Path, data_rows: u32) -> PathBuf {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Firearms").expect("sheet name");
    for (col, header) in FIREARM_COLUMNS.iter().enumerate() {
        sheet.write_string(0, col as u16, *header).expect("header");
    }
    for row in 0..data_rows {
        for (col, header) in FIREARM_COLUMNS.iter().enumerate() {
            let value = match *header {
                "firearm_type" => "rifle".to_owned(),
                _ => format!("r{row:07}c{col:06}x"),
            };
            sheet.write_string(row + 1, col as u16, value).expect("cell");
        }
    }
    workbook.save(path).expect("save the large workbook");
    path.to_owned()
}

// ---------------------------------------------------------------------------
// Workbooks

const SHARED_STRINGS_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

/// Shared strings that declare `count` and `uniqueCount` of 2^64 - 1 with
/// three strings present.
pub fn shared_strings_u64_max(dir: &Path) -> PathBuf {
    shared_strings_workbook(dir, "shared-u64-max.xlsx", &u64::MAX.to_string())
}

/// Shared strings that declare 2,000,001 (one past the limit) with three
/// strings present.
pub fn shared_strings_2000001(dir: &Path) -> PathBuf {
    shared_strings_workbook(dir, "shared-2000001.xlsx", &(MAX_SHARED_STRINGS + 1).to_string())
}

fn shared_strings_workbook(dir: &Path, name: &str, declared: &str) -> PathBuf {
    let path = dir.join(name);
    let shared = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <sst xmlns=\"{SHARED_STRINGS_NS}\" count=\"{declared}\" uniqueCount=\"{declared}\">\
         <si><t>firearm_type</t></si><si><t>rifle</t></si><si><t>Acme</t></si></sst>"
    );
    let book = Workbook { sheets: names(1), shared_strings: Some(shared), filler: 0 };
    book.write(&path, |_, w| {
        w.write_all(b"<sheetData><row r=\"1\"><c r=\"A1\" t=\"s\"><v>0</v></c></row>")?;
        w.write_all(
            b"<row r=\"2\"><c r=\"A2\" t=\"s\"><v>1</v></c><c r=\"B2\" t=\"s\"><v>2</v></c></row>",
        )?;
        w.write_all(b"</sheetData>")
    });
    path
}

/// Cells at A1 and XFD1048576, nothing between: a dense grid of the sheet's
/// extent would be 17 billion cells. Passes the column limit, and the row
/// limit if rows are counted by position.
pub fn corner_cells(dir: &Path) -> PathBuf {
    let path = dir.join("corner-cells.xlsx");
    Workbook::plain(1).write(&path, |_, w| {
        w.write_all(b"<sheetData>")?;
        row_xml(w, 1, &[(1, "firearm_type")])?;
        row_xml(w, 1_048_576, &[(16_384, "x")])?;
        w.write_all(b"</sheetData>")
    });
    path
}

/// A sheet part of 1 GiB and a little more of white space between the
/// worksheet's start and its data, deflated to about 1,040,000 bytes. Only
/// the unpacked size can stop it: it has no rows, cells or strings beyond
/// the header. The sizes in the archive are true.
pub fn zip_bomb(dir: &Path) -> PathBuf {
    let path = dir.join("zip-bomb.xlsx");
    let book = Workbook::plain(1);
    let prefix = format!("{}<worksheet xmlns=\"{SHARED_STRINGS_NS}\">", XML_DECLARATION);
    let suffix = {
        let mut rows = Vec::new();
        rows.extend_from_slice(b"<sheetData>");
        header_row_xml(&mut rows, 1).unwrap();
        rows.extend_from_slice(b"</sheetData></worksheet>");
        rows
    };
    // 258-byte matches: the run is 1 + 258 * M bytes, M a multiple of four so
    // that the matches fill whole bytes of zero bits.
    const MATCHES: u64 = 4_164_000;
    let run = 1 + 258 * MATCHES;
    assert!(run >= MAX_UNPACKED_BYTES, "the bomb must unpack past the limit");

    let file = File::create(&path).expect("create the bomb");
    let mut out = CountingWriter::new(BufWriter::new(file));
    let mut central = Vec::new();
    for (name, bytes) in book.small_parts() {
        let entry = RawEntry {
            name: name.clone(),
            method: 0,
            crc: crc32(0, &bytes),
            compressed: bytes.len() as u64,
            uncompressed: bytes.len() as u64,
            offset: out.written,
        };
        entry.write_local(&mut out).unwrap();
        out.write_all(&bytes).unwrap();
        central.push(entry);
    }

    // The sheet's deflate stream: stored block, dynamic block, stored block.
    let mut stream_len = 0u64;
    let uncompressed = prefix.len() as u64 + run + suffix.len() as u64;
    let crc = {
        let mut crc = crc32(0, prefix.as_bytes());
        crc = crc32_combine(crc, space_run_crc(run), run);
        crc32_combine(crc, crc32(0, &suffix), suffix.len() as u64)
    };
    // The stream starts on a byte boundary: a non-final stored block's
    // header byte (three zero bits, padded), then the dynamic block, which
    // ends with the three header bits of the final stored block.
    let mut head = vec![0u8];
    stored_body(&mut head, prefix.as_bytes());
    let dynamic = dynamic_run_block(MATCHES);
    let mut tail = Vec::new();
    stored_body(&mut tail, &suffix);
    stream_len += (head.len() + dynamic.len() + tail.len()) as u64;
    assert!(
        stream_len <= 1024 * 1024,
        "a 1,000x bomb must deflate to at most 1 MiB, got {stream_len}"
    );

    let entry = RawEntry {
        name: "xl/worksheets/sheet1.xml".into(),
        method: 8,
        crc,
        compressed: stream_len,
        uncompressed,
        offset: out.written,
    };
    entry.write_local(&mut out).unwrap();
    out.write_all(&head).unwrap();
    out.write_all(&dynamic).unwrap();
    out.write_all(&tail).unwrap();
    central.push(entry);
    finish_raw_zip(&mut out, &central).unwrap();
    out.flush().unwrap();
    path
}

/// 1,001 zip entries: the workbook's parts and empty ones for the rest.
pub fn zip_entries_1001(dir: &Path) -> PathBuf {
    let path = dir.join("zip-entries.xlsx");
    let mut book = Workbook::plain(1);
    book.filler = MAX_ZIP_ENTRIES + 1 - book.small_parts().len() - 1;
    book.write(&path, one_row_sheet);
    path
}

/// Seventeen sheets, each with a header and one row.
pub fn sheets_17(dir: &Path) -> PathBuf {
    let path = dir.join("sheets-17.xlsx");
    Workbook::plain(MAX_SHEETS + 1).write(&path, one_row_sheet);
    path
}

/// A sheet "Sheet1" with the header and 100,001 data rows.
pub fn rows_100001_xlsx(dir: &Path) -> PathBuf {
    let path = dir.join("rows-100001.xlsx");
    Workbook::plain(1).write(&path, |_, w| {
        w.write_all(b"<sheetData>")?;
        header_row_xml(w, 1)?;
        for row in 2..=(MAX_ROWS as u32 + 2) {
            short_row_xml(w, row)?;
        }
        w.write_all(b"</sheetData>")
    });
    path
}

/// A header of 257 cells.
pub fn columns_257_xlsx(dir: &Path) -> PathBuf {
    let path = dir.join("columns-257.xlsx");
    Workbook::plain(1).write(&path, |_, w| {
        w.write_all(b"<sheetData>")?;
        let cells = wide_header(MAX_COLUMNS + 1);
        let refs: Vec<(u32, &str)> =
            cells.iter().enumerate().map(|(i, c)| (i as u32 + 1, c.as_str())).collect();
        row_xml(w, 1, &refs)?;
        w.write_all(b"</sheetData>")
    });
    path
}

/// Sixteen sheets (the most allowed) of 100,000 rows (the header and 99,999
/// data rows) of four short cells: 6.4 million cells, past the 5,000,000 a
/// little into the thirteenth sheet, with every other limit kept.
pub fn cells_6_4m(dir: &Path) -> PathBuf {
    let path = dir.join("cells-16-sheets.xlsx");
    Workbook::plain(MAX_SHEETS).write(&path, |_, w| {
        w.write_all(b"<sheetData>")?;
        header_row_xml(w, 1)?;
        for row in 2..=MAX_ROWS as u32 {
            short_row_xml(w, row)?;
        }
        w.write_all(b"</sheetData>")
    });
    path
}

/// A sparse sheet: 20,000 rows with one cell each, at column IV (256, the
/// most allowed). Each row is padded to 256 cells, 5.12 million in all.
pub fn cells_sparse(dir: &Path) -> PathBuf {
    let path = dir.join("cells-sparse.xlsx");
    Workbook::plain(1).write(&path, |_, w| {
        w.write_all(b"<sheetData>")?;
        header_row_xml(w, 1)?;
        for row in 2..=20_001 {
            row_xml(w, row, &[(MAX_COLUMNS as u32, "x")])?;
        }
        w.write_all(b"</sheetData>")
    });
    path
}

/// One cell of 32,768 characters.
pub fn cell_32768_xlsx(dir: &Path) -> PathBuf {
    let path = dir.join("cell-32768.xlsx");
    Workbook::plain(1).write(&path, |_, w| {
        w.write_all(b"<sheetData>")?;
        header_row_xml(w, 1)?;
        row_xml(w, 2, &[(2, &"x".repeat(MAX_CELL_CHARS + 1))])?;
        w.write_all(b"</sheetData>")
    });
    path
}

/// 8,400 cells that all read as the one shared string of 32,767 characters
/// (the longest allowed): 275 MB of text from a 3 KB file, within every other
/// limit (the reader sees 8,400 cells of 32,767 characters and 8,400 short
/// XML elements).
pub fn text_over_256mib(dir: &Path) -> PathBuf {
    let path = dir.join("total-text.xlsx");
    assert!(8_400 * MAX_CELL_CHARS as u64 > MAX_TOTAL_TEXT_BYTES);
    // The header's names, then the long string.
    let mut shared = format!(
        "{XML_DECLARATION}<sst xmlns=\"{SHARED_STRINGS_NS}\" count=\"{}\" uniqueCount=\"{}\">",
        HEADER_ROW_CELLS + 8_400,
        HEADER_ROW_CELLS + 1
    );
    for name in FIREARM_COLUMNS {
        shared.push_str(&format!("<si><t>{name}</t></si>"));
    }
    shared.push_str(&format!("<si><t>{}</t></si></sst>", "x".repeat(MAX_CELL_CHARS)));
    let long_index = HEADER_ROW_CELLS;
    let book = Workbook { sheets: names(1), shared_strings: Some(shared), filler: 0 };
    book.write(&path, |_, w| {
        w.write_all(b"<sheetData><row r=\"1\">")?;
        for col in 1..=HEADER_ROW_CELLS {
            write!(w, "<c r=\"{}1\" t=\"s\"><v>{}</v></c>", column_name(col as u32), col - 1)?;
        }
        w.write_all(b"</row>")?;
        for row in 2..=2_101 {
            write!(w, "<row r=\"{row}\">")?;
            for col in ["A", "B", "C", "D"] {
                write!(w, "<c r=\"{col}{row}\" t=\"s\"><v>{long_index}</v></c>")?;
            }
            w.write_all(b"</row>")?;
        }
        w.write_all(b"</sheetData>")
    });
    path
}

/// The valid export workbook, cut in half.
pub fn truncated_workbook(dir: &Path) -> PathBuf {
    let bytes = valid_workbook_bytes(dir);
    let path = dir.join("truncated.xlsx");
    std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
    path
}

/// The valid export workbook without the end of its central directory (its
/// last ten bytes).
pub fn truncated_directory(dir: &Path) -> PathBuf {
    let bytes = valid_workbook_bytes(dir);
    let path = dir.join("truncated-directory.xlsx");
    std::fs::write(&path, &bytes[..bytes.len() - 10]).unwrap();
    path
}

/// The valid export workbook with one bit flipped in every 31st byte, which
/// reaches every part (a flipped compressed stream fails to inflate or
/// fails its checksum).
pub fn bit_flipped_workbook(dir: &Path) -> PathBuf {
    let mut bytes = valid_workbook_bytes(dir);
    for (i, byte) in bytes.iter_mut().enumerate() {
        if i % 31 == 7 {
            *byte ^= 0x10;
        }
    }
    let path = dir.join("bit-flipped.xlsx");
    std::fs::write(&path, &bytes).unwrap();
    path
}

/// The valid export workbook with one bit flipped in every 31st byte of the
/// parts' own data, so the directory reads and the parts don't.
pub fn bit_flipped_data(dir: &Path) -> PathBuf {
    let mut bytes = valid_workbook_bytes(dir);
    let at = bytes.len() - 22;
    let directory_start = u32::from_le_bytes(bytes[at + 16..at + 20].try_into().unwrap()) as usize;
    // Past the first local header's fixed fields and name.
    for byte in bytes[64..directory_start].iter_mut().step_by(31) {
        *byte ^= 0x10;
    }
    let path = dir.join("bit-flipped-data.xlsx");
    std::fs::write(&path, &bytes).unwrap();
    path
}

/// The valid export workbook with one bit flipped in the signature of its
/// end-of-directory record.
pub fn bit_flipped_directory(dir: &Path) -> PathBuf {
    let mut bytes = valid_workbook_bytes(dir);
    let at = bytes.len() - 22;
    assert_eq!(&bytes[at..at + 4], b"PK\x05\x06", "no archive comment");
    bytes[at] ^= 0x01;
    let path = dir.join("bit-flipped-directory.xlsx");
    std::fs::write(&path, &bytes).unwrap();
    path
}

/// A valid archive, named `.xlsx`, that holds two text files and no
/// workbook.
pub fn archive_not_a_spreadsheet(dir: &Path) -> PathBuf {
    let path = dir.join("not-a-spreadsheet.xlsx");
    let mut zip = ZipWriter::new(BufWriter::new(File::create(&path).unwrap()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("readme.txt", options).unwrap();
    zip.write_all(b"This is an archive, but it holds no workbook.\n").unwrap();
    zip.start_file("docs/more.txt", options).unwrap();
    zip.write_all(b"More text.\n").unwrap();
    zip.finish().unwrap().flush().unwrap();
    path
}

fn valid_workbook_bytes(dir: &Path) -> Vec<u8> {
    let source = valid_export_workbook(&dir.join("valid-source.xlsx"));
    let bytes = std::fs::read(&source).unwrap();
    std::fs::remove_file(source).unwrap();
    bytes
}

// ---------------------------------------------------------------------------
// CSV

/// The header and 100,001 short data rows.
pub fn rows_100001_csv(dir: &Path) -> PathBuf {
    let path = dir.join("rows-100001.csv");
    let mut out = csv_file(&path);
    header_line(&mut out);
    for _ in 0..=MAX_ROWS {
        out.write_all(b"a,Acme,M1,n\n").unwrap();
    }
    finish(out);
    path
}

/// A row of 257 fields.
pub fn columns_257_csv(dir: &Path) -> PathBuf {
    let path = dir.join("columns-257.csv");
    let mut out = csv_file(&path);
    header_line(&mut out);
    out.write_all(vec!["a"; MAX_COLUMNS + 1].join(",").as_bytes()).unwrap();
    out.write_all(b"\n").unwrap();
    finish(out);
    path
}

/// A field of 32,768 characters.
pub fn cell_32768_csv(dir: &Path) -> PathBuf {
    let path = dir.join("cell-32768.csv");
    let mut out = csv_file(&path);
    header_line(&mut out);
    writeln!(out, "a,{}", "x".repeat(MAX_CELL_CHARS + 1)).unwrap();
    finish(out);
    path
}

/// A single field of 200,000,000 bytes: under the file limit, so the cell
/// limit has to stop it while it is still arriving.
pub fn field_200mb_csv(dir: &Path) -> PathBuf {
    big_field_csv(dir, "field-200mb.csv", 200_000_000)
}

/// A single field of 300,000,000 bytes. The file passes the file-size limit
/// before the field can pass the cell limit, so it is refused from its
/// metadata.
pub fn field_300mb_csv(dir: &Path) -> PathBuf {
    big_field_csv(dir, "field-300mb.csv", 300_000_000)
}

fn big_field_csv(dir: &Path, name: &str, field_bytes: u64) -> PathBuf {
    let path = dir.join(name);
    let mut out = csv_file(&path);
    header_line(&mut out);
    out.write_all(b"a,").unwrap();
    repeat_bytes(&mut out, b'x', field_bytes);
    out.write_all(b"\n").unwrap();
    finish(out);
    path
}

/// The header and 100,000 lines of 255 commas: 25.6 million empty cells in a
/// 25 MB file, within every limit but the cell count.
pub fn commas_25_6m_cells(dir: &Path) -> PathBuf {
    let path = dir.join("commas.csv");
    let mut out = csv_file(&path);
    let mut header = FIREARM_COLUMNS.join(",");
    header.push_str(&",".repeat(MAX_COLUMNS - HEADER_ROW_CELLS));
    writeln!(out, "{header}").unwrap();
    let line = format!("{}\n", ",".repeat(MAX_COLUMNS - 1));
    for _ in 0..MAX_ROWS {
        out.write_all(line.as_bytes()).unwrap();
    }
    finish(out);
    path
}

/// A valid table of 256 MiB and one byte.
pub fn csv_over_256mib(dir: &Path) -> PathBuf {
    let path = dir.join("over-256mib.csv");
    let mut out = csv_file(&path);
    header_line(&mut out);
    let header_bytes = FIREARM_COLUMNS.join(",").len() as u64 + 1;
    let line = format!("a,Acme,{}\n", "x".repeat(1_000));
    let mut remaining = MAX_FILE_BYTES + 1 - header_bytes;
    while remaining > 0 {
        let take = remaining.min(line.len() as u64) as usize;
        out.write_all(&line.as_bytes()[..take]).unwrap();
        remaining -= take as u64;
    }
    finish(out);
    assert_eq!(std::fs::metadata(&path).unwrap().len(), MAX_FILE_BYTES + 1);
    path
}

// ---------------------------------------------------------------------------
// Two files

/// A valid table, then a table of 100,001 rows.
pub fn second_file_passes_a_limit(dir: &Path) -> Vec<PathBuf> {
    let first = valid_export_csv(&dir.join("first.csv"));
    let second = rows_100001_csv(dir);
    vec![first, second]
}

/// Two tables of 12,000 rows of 250 cells (3,000,042 cells each, under the
/// 5,000,000 limit) that pass it together, in the second file.
pub fn cells_pass_together(dir: &Path) -> Vec<PathBuf> {
    let files = ["cells-a.csv", "cells-b.csv"].map(|name| {
        let path = dir.join(name);
        let mut out = csv_file(&path);
        header_line(&mut out);
        let line = format!("{}\n", ",".repeat(249));
        for _ in 0..12_000 {
            out.write_all(line.as_bytes()).unwrap();
        }
        finish(out);
        path
    });
    files.to_vec()
}

// ---------------------------------------------------------------------------
// CSV helpers

fn csv_file(path: &Path) -> BufWriter<File> {
    BufWriter::with_capacity(1 << 20, File::create(path).expect("create a corpus file"))
}

fn finish(mut out: BufWriter<File>) {
    out.flush().expect("write a corpus file");
}

fn header_line(out: &mut impl Write) {
    writeln!(out, "{}", FIREARM_COLUMNS.join(",")).unwrap();
}

fn repeat_bytes(out: &mut impl Write, byte: u8, count: u64) {
    let block = vec![byte; 1 << 20];
    let mut remaining = count;
    while remaining > 0 {
        let take = remaining.min(block.len() as u64) as usize;
        out.write_all(&block[..take]).unwrap();
        remaining -= take as u64;
    }
}

// ---------------------------------------------------------------------------
// Workbook parts

const XML_DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>";

fn names(count: usize) -> Vec<String> {
    (1..=count).map(|i| format!("Sheet{i}")).collect()
}

/// `["firearm_type", "make", ...]`, `width` cells: the table's header, padded
/// with filler names or cut.
fn wide_header(width: usize) -> Vec<String> {
    (0..width)
        .map(|i| match FIREARM_COLUMNS.get(i) {
            Some(name) => (*name).to_owned(),
            None => format!("extra_{i}"),
        })
        .collect()
}

struct Workbook {
    sheets: Vec<String>,
    shared_strings: Option<String>,
    /// Empty entries to add after the parts.
    filler: usize,
}

impl Workbook {
    fn plain(sheets: usize) -> Self {
        Self { sheets: names(sheets), shared_strings: None, filler: 0 }
    }

    /// Every part but the sheets, as (name, bytes).
    fn small_parts(&self) -> Vec<(String, Vec<u8>)> {
        let ns = SHARED_STRINGS_NS;
        let mut types = format!(
            "{XML_DECLARATION}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
             <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
             <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
             <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>"
        );
        let mut sheets = String::new();
        let mut rels = format!(
            "{XML_DECLARATION}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">"
        );
        for (i, name) in self.sheets.iter().enumerate() {
            let n = i + 1;
            types.push_str(&format!(
                "<Override PartName=\"/xl/worksheets/sheet{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
            ));
            sheets.push_str(&format!("<sheet name=\"{name}\" sheetId=\"{n}\" r:id=\"rId{n}\"/>"));
            rels.push_str(&format!(
                "<Relationship Id=\"rId{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{n}.xml\"/>"
            ));
        }
        let mut parts = Vec::new();
        if self.shared_strings.is_some() {
            types.push_str(
                "<Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/>",
            );
            rels.push_str(
                "<Relationship Id=\"rIdSS\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings\" Target=\"sharedStrings.xml\"/>",
            );
        }
        types.push_str("</Types>");
        rels.push_str("</Relationships>");
        parts.push(("[Content_Types].xml".to_owned(), types.into_bytes()));
        parts.push((
            "_rels/.rels".to_owned(),
            format!(
                "{XML_DECLARATION}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
                 <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"
            )
            .into_bytes(),
        ));
        parts.push((
            "xl/workbook.xml".to_owned(),
            format!(
                "{XML_DECLARATION}<workbook xmlns=\"{ns}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>{sheets}</sheets></workbook>"
            )
            .into_bytes(),
        ));
        parts.push(("xl/_rels/workbook.xml.rels".to_owned(), rels.into_bytes()));
        if let Some(shared) = &self.shared_strings {
            parts.push(("xl/sharedStrings.xml".to_owned(), shared.clone().into_bytes()));
        }
        parts
    }

    /// Writes the workbook. `body(i, w)` writes sheet `i`'s content between
    /// `<worksheet>` and `</worksheet>`, streaming.
    fn write(&self, path: &Path, mut body: impl FnMut(usize, &mut dyn Write) -> io::Result<()>) {
        let file = File::create(path).expect("create a workbook");
        let mut zip = ZipWriter::new(BufWriter::with_capacity(1 << 20, file));
        // Level 1: the corpus is generated on every run, and a hostile file
        // doesn't need to be small.
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(1));
        for (name, bytes) in self.small_parts() {
            zip.start_file(name, options).unwrap();
            zip.write_all(&bytes).unwrap();
        }
        for i in 0..self.sheets.len() {
            zip.start_file(format!("xl/worksheets/sheet{}.xml", i + 1), options).unwrap();
            let mut w = BufWriter::with_capacity(1 << 20, &mut zip);
            write!(w, "{XML_DECLARATION}<worksheet xmlns=\"{SHARED_STRINGS_NS}\">").unwrap();
            body(i, &mut w).expect("write a sheet");
            w.write_all(b"</worksheet>").unwrap();
            w.flush().unwrap();
        }
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for i in 0..self.filler {
            zip.start_file(format!("customXml/item{i}.xml"), stored).unwrap();
        }
        zip.finish().unwrap().flush().unwrap();
    }
}

fn one_row_sheet(_: usize, w: &mut dyn Write) -> io::Result<()> {
    w.write_all(b"<sheetData>")?;
    header_row_xml(w, 1)?;
    row_xml(w, 2, &[(1, "a"), (2, "b")])?;
    w.write_all(b"</sheetData>")
}

/// `A`, `B`, ..., `Z`, `AA`, ..., `XFD` for column 1, 2, ...
fn column_name(mut col: u32) -> String {
    let mut name = Vec::new();
    while col > 0 {
        col -= 1;
        name.push(b'A' + (col % 26) as u8);
        col /= 26;
    }
    name.reverse();
    String::from_utf8(name).unwrap()
}

fn escape(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains(['&', '<', '>']) {
        text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").into()
    } else {
        text.into()
    }
}

/// `<row r="n">` with inline-string cells at (1-based column, text).
fn row_xml(w: &mut dyn Write, row: u32, cells: &[(u32, &str)]) -> io::Result<()> {
    write!(w, "<row r=\"{row}\">")?;
    for (col, text) in cells {
        write!(
            w,
            "<c r=\"{}{row}\" t=\"inlineStr\"><is><t>{}</t></is></c>",
            column_name(*col),
            escape(text)
        )?;
    }
    w.write_all(b"</row>")
}

/// The table's header, `FIREARM_COLUMNS`, as row `row`.
fn header_row_xml(w: &mut dyn Write, row: u32) -> io::Result<()> {
    let cells: Vec<(u32, &str)> =
        FIREARM_COLUMNS.iter().enumerate().map(|(i, name)| (i as u32 + 1, *name)).collect();
    row_xml(w, row, &cells)
}

/// A data row of four short cells.
fn short_row_xml(w: &mut dyn Write, row: u32) -> io::Result<()> {
    row_xml(w, row, &[(1, "a"), (2, "Acme"), (3, "M1"), (4, "n")])
}

// ---------------------------------------------------------------------------
// The bomb's archive and deflate stream, written by hand

struct CountingWriter<W: Write> {
    inner: W,
    written: u64,
}

impl<W: Write> CountingWriter<W> {
    fn new(inner: W) -> Self {
        Self { inner, written: 0 }
    }
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct RawEntry {
    name: String,
    /// 0 stored, 8 deflated.
    method: u16,
    crc: u32,
    compressed: u64,
    uncompressed: u64,
    offset: u64,
}

impl RawEntry {
    fn write_local(&self, w: &mut impl Write) -> io::Result<()> {
        w.write_all(&0x0403_4b50u32.to_le_bytes())?;
        w.write_all(&20u16.to_le_bytes())?; // version needed
        w.write_all(&0u16.to_le_bytes())?; // flags
        w.write_all(&self.method.to_le_bytes())?;
        w.write_all(&0u16.to_le_bytes())?; // time
        w.write_all(&0x0021u16.to_le_bytes())?; // date: 1980-01-01
        w.write_all(&self.crc.to_le_bytes())?;
        w.write_all(&u32::try_from(self.compressed).unwrap().to_le_bytes())?;
        w.write_all(&u32::try_from(self.uncompressed).unwrap().to_le_bytes())?;
        w.write_all(&(self.name.len() as u16).to_le_bytes())?;
        w.write_all(&0u16.to_le_bytes())?; // extra length
        w.write_all(self.name.as_bytes())
    }

    fn write_central(&self, w: &mut impl Write) -> io::Result<()> {
        w.write_all(&0x0201_4b50u32.to_le_bytes())?;
        w.write_all(&20u16.to_le_bytes())?; // made by
        w.write_all(&20u16.to_le_bytes())?; // needed
        w.write_all(&0u16.to_le_bytes())?; // flags
        w.write_all(&self.method.to_le_bytes())?;
        w.write_all(&0u16.to_le_bytes())?;
        w.write_all(&0x0021u16.to_le_bytes())?;
        w.write_all(&self.crc.to_le_bytes())?;
        w.write_all(&u32::try_from(self.compressed).unwrap().to_le_bytes())?;
        w.write_all(&u32::try_from(self.uncompressed).unwrap().to_le_bytes())?;
        w.write_all(&(self.name.len() as u16).to_le_bytes())?;
        w.write_all(&[0u8; 2 + 2 + 2 + 2 + 4])?; // extra, comment, disk, internal, external
        w.write_all(&u32::try_from(self.offset).unwrap().to_le_bytes())?;
        w.write_all(self.name.as_bytes())
    }
}

fn finish_raw_zip(out: &mut CountingWriter<impl Write>, entries: &[RawEntry]) -> io::Result<()> {
    let directory_start = out.written;
    let mut directory = Vec::new();
    for entry in entries {
        entry.write_central(&mut directory)?;
    }
    out.write_all(&directory)?;
    out.write_all(&0x0605_4b50u32.to_le_bytes())?;
    out.write_all(&[0u8; 4])?; // disk numbers
    out.write_all(&(entries.len() as u16).to_le_bytes())?;
    out.write_all(&(entries.len() as u16).to_le_bytes())?;
    out.write_all(&(directory.len() as u32).to_le_bytes())?;
    out.write_all(&u32::try_from(directory_start).unwrap().to_le_bytes())?;
    out.write_all(&0u16.to_le_bytes()) // comment length
}

/// What follows a stored deflate block's three header bits (which the
/// previous block's last byte or a byte of its own holds, see the callers):
/// its length, the length's complement and `data` (at most 65,535 bytes).
fn stored_body(out: &mut Vec<u8>, data: &[u8]) {
    let length = u16::try_from(data.len()).expect("a stored block is at most 65,535 bytes");
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(&(!length).to_le_bytes());
    out.extend_from_slice(data);
}

struct Bits {
    bytes: Vec<u8>,
    filled: u32,
}

impl Bits {
    /// `count` low bits of `value`, least significant first.
    fn put(&mut self, value: u32, count: u32) {
        for i in 0..count {
            if self.filled.is_multiple_of(8) {
                self.bytes.push(0);
            }
            if (value >> i) & 1 == 1 {
                *self.bytes.last_mut().unwrap() |= 1 << (self.filled % 8);
            }
            self.filled += 1;
        }
    }

    /// A Huffman code, most significant bit first.
    fn code(&mut self, code: u32, length: u32) {
        for i in (0..length).rev() {
            self.put((code >> i) & 1, 1);
        }
    }
}

/// One non-final dynamic-Huffman block that is one space, then `matches`
/// matches of length 258 at distance 1, then the end of the block and
/// the three header bits of a final stored block (`1`, `00`), padded to a
/// byte. `matches` is a multiple of four. The literal/length code is
/// 285 = `0`, 32 = `10`, 256 = `11` and the distance code is 0 = `0`, so a
/// match takes two bits.
fn dynamic_run_block(matches: u64) -> Vec<u8> {
    assert_eq!(matches % 4, 0);
    let mut bits = Bits { bytes: Vec::new(), filled: 0 };
    bits.put(0, 1); // not the last block
    bits.put(2, 2); // dynamic Huffman
    bits.put(29, 5); // HLIT: 286 literal/length codes
    bits.put(0, 5); // HDIST: 1 distance code
    bits.put(14, 4); // HCLEN: 18 code length codes
    // Code length code lengths in the order 16 17 18 0 8 7 9 6 10 5 11 4 12
    // 3 13 2 14 1: symbol 18 has length 1, symbols 2 and 1 length 2.
    for length in [0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2] {
        bits.put(length, 3);
    }
    // The code length alphabet's codes: 18 = `0`, 1 = `10`, 2 = `11`.
    let zeros = |bits: &mut Bits, run: u32| {
        bits.code(0b0, 1);
        bits.put(run - 11, 7);
    };
    zeros(&mut bits, 32); // symbols 0-31 unused
    bits.code(0b11, 2); // symbol 32: length 2
    zeros(&mut bits, 138); // 33-170
    zeros(&mut bits, 85); // 171-255
    bits.code(0b11, 2); // symbol 256: length 2
    zeros(&mut bits, 28); // 257-284
    bits.code(0b10, 2); // symbol 285: length 1
    bits.code(0b10, 2); // distance symbol 0: length 1
    // The data.
    bits.code(0b10, 2); // a space
    for _ in 0..matches * 2 {
        bits.put(0, 1); // symbol 285, then distance 0
    }
    bits.code(0b11, 2); // end of block
    bits.put(1, 1); // the next block is the last...
    bits.put(0, 2); // ...and stored
    bits.bytes
}

// ---------------------------------------------------------------------------
// CRC-32

fn crc32(crc: u32, data: &[u8]) -> u32 {
    let mut crc = !crc;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn gf2_times(matrix: &[u32; 32], mut vector: u32) -> u32 {
    let mut sum = 0;
    let mut i = 0;
    while vector != 0 {
        if vector & 1 == 1 {
            sum ^= matrix[i];
        }
        vector >>= 1;
        i += 1;
    }
    sum
}

fn gf2_square(square: &mut [u32; 32], matrix: &[u32; 32]) {
    for n in 0..32 {
        square[n] = gf2_times(matrix, matrix[n]);
    }
}

/// The CRC of two blocks from their CRCs and the second's length (zlib's
/// `crc32_combine`).
fn crc32_combine(mut crc1: u32, crc2: u32, mut len2: u64) -> u32 {
    if len2 == 0 {
        return crc1;
    }
    let mut odd = [0u32; 32];
    let mut even = [0u32; 32];
    odd[0] = 0xedb8_8320;
    let mut row = 1u32;
    for entry in odd.iter_mut().skip(1) {
        *entry = row;
        row <<= 1;
    }
    gf2_square(&mut even, &odd);
    gf2_square(&mut odd, &even);
    loop {
        gf2_square(&mut even, &odd);
        if len2 & 1 == 1 {
            crc1 = gf2_times(&even, crc1);
        }
        len2 >>= 1;
        if len2 == 0 {
            break;
        }
        gf2_square(&mut odd, &even);
        if len2 & 1 == 1 {
            crc1 = gf2_times(&odd, crc1);
        }
        len2 >>= 1;
        if len2 == 0 {
            break;
        }
    }
    crc1 ^ crc2
}

/// The CRC of `n` spaces.
fn space_run_crc(n: u64) -> u32 {
    const BLOCK: u64 = 1 << 20;
    let block_crc = crc32(0, &vec![b' '; BLOCK as usize]);
    let mut crc = 0;
    let mut remaining = n;
    while remaining >= BLOCK {
        crc = crc32_combine(crc, block_crc, BLOCK);
        remaining -= BLOCK;
    }
    if remaining > 0 {
        crc = crc32_combine(crc, crc32(0, &vec![b' '; remaining as usize]), remaining);
    }
    crc
}
