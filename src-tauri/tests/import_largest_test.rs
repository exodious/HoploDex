//! The largest export the app can write, imported back (feature 008, T053;
//! research.md §9, §10; SC-003): 10,000 firearms and 10,000 accessories, every
//! text field at its maximum, in ASCII. It is exported through the app's own
//! export as one workbook and as two CSV files; each is read through
//! `import_reader::read_files` (the real import helper) and imported into a
//! fresh database through `import_collection_stoppable`. Nothing may be refused
//! and every row must import. The reading time, the import time and this
//! process's peak resident memory are printed for the pull request (T098-T100
//! record them).
//!
//! It takes minutes and a few hundred MB of disk, and its timings mean
//! something only in a release build, so it is `#[ignore]` and does not exist
//! in a debug build:
//!
//! ```text
//! cargo test --release --test import_largest_test -- --ignored --nocapture
//! ```
//!
//! `HOPLODEX_LARGEST_ROWS` (default 10,000) sets the number of firearms and of
//! accessories, for a quick check of the test itself.
#![cfg(not(debug_assertions))]

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::{
    ImportFile, ImportResult, ImportSessionStore, ops as import_export_ops,
};
use hoplodex_lib::commands::insurance::ops as insurance_ops;
use hoplodex_lib::models::accessory::AccessoryInput;
use hoplodex_lib::models::firearm::{
    Condition, DispositionType, FirearmInput, FirearmStatus, Origin,
};
use hoplodex_lib::services::entry_text::MAX_ENTRY_CHARS;
use hoplodex_lib::services::import_reader::read_files;
use hoplodex_lib::services::spreadsheet::SpreadsheetFormat;
use support::TestDb;
use tempfile::TempDir;

const HELPER: &str = env!("CARGO_BIN_EXE_hoplodex");

/// research.md §10's maximums, written out here (`models::rules` is private)
/// so that a change to them has to change this test too.
const MAX_SHORT_TEXT_CHARS: usize = 200;
const MAX_LONG_TEXT_CHARS: usize = 4_000;

fn rows() -> usize {
    std::env::var("HOPLODEX_LARGEST_ROWS").ok().and_then(|n| n.parse().ok()).unwrap_or(10_000)
}

// ------------------------------------------------------------------- text

/// Exactly `len` ASCII characters: `prefix` (which makes the value unique when
/// it names the row), then letters from a generator seeded by `seed`, with a
/// space after every eighth and none at either end, so trimming keeps them all.
/// Random letters compress to about half, as research.md §9 assumes.
fn text(prefix: &str, seed: u64, len: usize) -> String {
    assert!(prefix.len() <= len);
    let mut out = String::with_capacity(len);
    out.push_str(prefix);
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    while out.len() < len {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let at_end = out.len() + 1 == len;
        if out.len() % 9 == 8 && !at_end && !out.ends_with(' ') {
            out.push(' ');
        } else {
            out.push((b'a' + ((state >> 33) % 26) as u8) as char);
        }
    }
    assert_eq!(out.len(), len);
    out
}

/// One of `pool` values of `len` characters (the entry fields repeat across a
/// real collection: a few hundred makes and models, not 10,000).
fn from_pool(tag: &str, n: usize, pool: usize, len: usize) -> String {
    let which = n % pool;
    text(&format!("{tag}{which:04}-"), which as u64 + 1, len)
}

// ---------------------------------------------------------------- records

/// A policy name at its maximum, and the same policy in both databases: an
/// import finds a policy by name.
fn policy_input() -> hoplodex_lib::models::insurance_policy::InsurancePolicyInput {
    let mut policy = support::policy(
        &text("policy-", 7, MAX_SHORT_TEXT_CHARS),
        "2020-01-01",
        "2099-01-01",
        None,
    );
    policy.policy_number = text("number-", 8, MAX_SHORT_TEXT_CHARS);
    policy
}

fn firearm_input(n: usize, policy_id: i64) -> FirearmInput {
    let short = |tag: &str| Some(text(&format!("{tag}{n:05}-"), n as u64, MAX_SHORT_TEXT_CHARS));
    let long = |tag: &str| Some(text(&format!("{tag}{n:05}-"), n as u64, MAX_LONG_TEXT_CHARS));
    let mut firearm = support::firearm(
        &from_pool("make", n, 50, MAX_ENTRY_CHARS),
        &from_pool("model", n, 500, MAX_ENTRY_CHARS),
        &text(&format!("sn{n:05}-"), n as u64, MAX_SHORT_TEXT_CHARS),
    );
    firearm.nickname = short("nick");
    firearm.caliber = from_pool("cal", n, 50, MAX_ENTRY_CHARS);
    firearm.cartridge = Some(from_pool("cart", n, 50, MAX_ENTRY_CHARS));
    firearm.notes = long("notes");
    firearm.accessories = long("acc");
    firearm.finish = short("fin");
    firearm.condition = Some(Condition::Good);
    firearm.barrel_length_hundredths = Some(1_650);
    firearm.overall_length_hundredths = Some(3_600);
    firearm.weight_tenths_oz = Some(1_200);
    firearm.capacity = Some(30);
    firearm.estimated_value = Some(1_000);
    firearm.acquisition_source = short("src");
    firearm.acquisition_date = Some("2020-02-03".into());
    firearm.acquisition_price = Some(900);
    firearm.status = FirearmStatus::Disposed;
    firearm.disposition_type = Some(DispositionType::Sold);
    firearm.disposition_recipient = short("to");
    firearm.disposition_date = Some("2021-04-05".into());
    firearm.disposition_price = Some(950);
    firearm.insurance_policy_id = Some(policy_id);
    firearm.scheduled_coverage_amount = Some(1_000);
    firearm.origin = Some(Origin::Imported);
    firearm.year_of_manufacture = Some(1990);
    firearm.country_of_manufacture = short("country");
    firearm.importer_name = short("imp");
    firearm.original_make = short("omake");
    firearm.original_model = short("omodel");
    firearm.original_serial_number = short("oserial");
    firearm.registration_class_id = Some(1);
    firearm.registration_form = Some(from_pool("form", n, 10, MAX_ENTRY_CHARS));
    firearm.registration_approved = Some("2019-06-07".into());
    firearm.registered_to = Some(from_pool("owner", n, 10, MAX_ENTRY_CHARS));
    firearm
}

fn accessory_input(n: usize, policy_id: i64) -> AccessoryInput {
    let short = |tag: &str| Some(text(&format!("{tag}{n:05}-"), n as u64, MAX_SHORT_TEXT_CHARS));
    AccessoryInput {
        accessory_kind_id: 1,
        make: from_pool("amake", n, 50, MAX_ENTRY_CHARS),
        model: from_pool("amodel", n, 500, MAX_ENTRY_CHARS),
        serial_number: Some(text(&format!("asn{n:05}-"), n as u64, MAX_SHORT_TEXT_CHARS)),
        caliber: Some(from_pool("acal", n, 50, MAX_ENTRY_CHARS)),
        cartridge: Some(from_pool("acart", n, 50, MAX_ENTRY_CHARS)),
        notes: Some(text(&format!("anotes{n:05}-"), n as u64, MAX_LONG_TEXT_CHARS)),
        status: FirearmStatus::Disposed,
        estimated_value: Some(300),
        acquisition_source: short("asrc"),
        acquisition_date: Some("2020-02-03".into()),
        acquisition_price: Some(250),
        disposition_type: Some(DispositionType::Sold),
        disposition_recipient: short("ato"),
        disposition_date: Some("2021-04-05".into()),
        disposition_price: Some(275),
        insurance_policy_id: Some(policy_id),
        scheduled_coverage_amount: Some(300),
        mounted_on: None,
    }
}

/// A database holding the largest collection, in one transaction.
fn largest_collection(count: usize) -> TestDb {
    let db = TestDb::new();
    let policy = insurance_ops::create_policy(&db.conn, &policy_input()).unwrap();
    let began = Instant::now();
    let transaction = db.conn.unchecked_transaction().unwrap();
    for n in 0..count {
        firearm_ops::create_firearm(&db.conn, &firearm_input(n, policy.id), true, None)
            .unwrap_or_else(|e| panic!("firearm {n}: {e:?}"));
        accessory_ops::create_accessory(&db.conn, &accessory_input(n, policy.id), None)
            .unwrap_or_else(|e| panic!("accessory {n}: {e:?}"));
    }
    transaction.commit().unwrap();
    println!("made {count} firearms and {count} accessories in {:.1?}", began.elapsed());
    db
}

// ----------------------------------------------------------------- memory

/// This process's resident memory, in bytes. (The same reading as
/// tests/import_reader_test.rs; on macOS `libc` marks its Mach calls
/// deprecated in favor of a crate this project doesn't use.)
#[allow(deprecated)]
fn resident_bytes() -> u64 {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").expect("/proc/self/status");
        let kb = status
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))
            .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
            .expect("VmRSS");
        kb * 1024
    }
    #[cfg(target_os = "macos")]
    {
        let mut info: libc::mach_task_basic_info = unsafe { std::mem::zeroed() };
        let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
        // SAFETY: `info` is the structure `MACH_TASK_BASIC_INFO` fills, and
        // `count` is its size in words.
        let result = unsafe {
            libc::task_info(
                libc::mach_task_self(),
                libc::MACH_TASK_BASIC_INFO,
                &mut info as *mut _ as libc::task_info_t,
                &mut count,
            )
        };
        assert_eq!(result, 0, "task_info");
        info.resident_size
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::ProcessStatus::{
            K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;

        // SAFETY: the structure is zeroed and sized, and the handle is the
        // process's own pseudo handle.
        unsafe {
            let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            assert_ne!(K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb), 0);
            counters.WorkingSetSize as u64
        }
    }
}

/// Samples this process's resident memory in a thread of its own.
struct PeakMemory {
    stop: Arc<AtomicBool>,
    peak: Arc<AtomicU64>,
    sampler: Option<thread::JoinHandle<()>>,
}

impl PeakMemory {
    fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicU64::new(resident_bytes()));
        let sampler = {
            let (stop, peak) = (Arc::clone(&stop), Arc::clone(&peak));
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    peak.fetch_max(resident_bytes(), Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(5));
                }
            })
        };
        Self { stop, peak, sampler: Some(sampler) }
    }

    /// The highest resident memory sampled so far.
    fn peak(&self) -> u64 {
        self.peak.fetch_max(resident_bytes(), Ordering::Relaxed);
        self.peak.load(Ordering::Relaxed)
    }
}

impl Drop for PeakMemory {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(sampler) = self.sampler.take() {
            let _ = sampler.join();
        }
    }
}

fn mb(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

// ------------------------------------------------------------------ import

fn import_file(path: &Path, format: SpreadsheetFormat) -> ImportFile {
    ImportFile { file_path: path.to_path_buf(), format }
}

fn size_of(path: &Path) -> u64 {
    std::fs::metadata(path).unwrap().len()
}

/// Reads `files` and imports them into a fresh database, asserting that
/// nothing is refused and every row imports, and printing what SC-003 asks.
fn read_and_import(label: &str, files: &[ImportFile], count: usize) {
    let db = TestDb::new();
    // An import finds the policy by name.
    insurance_ops::create_policy(&db.conn, &policy_input()).unwrap();

    let memory = PeakMemory::start();
    let before = resident_bytes();

    let began = Instant::now();
    let tables = read_files(Path::new(HELPER), files, &|| false, &|| true, &mut |_, _, _| {})
        .unwrap_or_else(|e| panic!("{label}: the reader refused the largest export: {e:?}"));
    let reading = began.elapsed();
    let reading_peak = memory.peak();

    let began = Instant::now();
    let result: ImportResult = import_export_ops::import_collection_stoppable(
        &db.conn,
        tables,
        &ImportSessionStore::new(),
        &mut |_, _| {},
        &|| false,
        &|_| {},
    )
    .unwrap_or_else(|e| panic!("{label}: the import failed: {e:?}"));
    let importing = began.elapsed();
    let peak = memory.peak();

    println!(
        "{label}: reading {reading:.1?} (parent peak {} MB, {} MB above where it began), \
         import {importing:.1?}, parent peak overall {} MB ({} MB above where it began)",
        mb(reading_peak),
        mb(reading_peak.saturating_sub(before)),
        mb(peak),
        mb(peak.saturating_sub(before)),
    );

    assert!(result.row_errors.is_empty(), "{label}: row errors: {:?}", &result.row_errors[..3]);
    assert!(result.conflicts.is_empty(), "{label}: conflicts");
    assert_eq!(result.skipped_count, 0, "{label}");
    assert_eq!(result.updated_count, 0, "{label}");
    assert_eq!(result.imported_count, 2 * count, "{label}: every row imports");
    assert_eq!(result.imported_accessory_count, count, "{label}");
    let saved = |table: &str| -> i64 {
        db.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
    };
    assert_eq!(saved("firearms"), count as i64, "{label}");
    assert_eq!(saved("accessories"), count as i64, "{label}");
}

#[test]
#[ignore = "release only: minutes, and hundreds of MB of disk (T053; run by T098-T100)"]
fn import_largest_export_as_a_workbook_and_as_two_csv_files() {
    let count = rows();
    let dir = TempDir::new().unwrap();
    let (mut workbook, mut firearms_csv, mut accessories_csv) = (None, None, None);
    {
        let source = largest_collection(count);
        let records = import_export_ops::export_records(&source.conn, "all", None).unwrap();
        assert_eq!((records.firearm_ids.len(), records.accessory_ids.len()), (count, count));
        for (format, base) in
            [(SpreadsheetFormat::Xlsx, "workbook"), (SpreadsheetFormat::Csv, "csv")]
        {
            let began = Instant::now();
            let folder = dir.path().join(base);
            std::fs::create_dir(&folder).unwrap();
            let exported = import_export_ops::export_collection(
                &source.conn,
                &folder,
                "largest",
                format,
                &records,
                &mut |_, _| {},
            )
            .unwrap();
            println!(
                "exported as {base} in {:.1?}: {} KB{}",
                began.elapsed(),
                size_of(&exported.spreadsheet_path) / 1024,
                exported
                    .accessory_spreadsheet_path
                    .as_ref()
                    .map_or(String::new(), |p| format!(" + {} KB", size_of(p) / 1024)),
            );
            match format {
                SpreadsheetFormat::Xlsx => workbook = Some(exported.spreadsheet_path),
                SpreadsheetFormat::Csv => {
                    firearms_csv = Some(exported.spreadsheet_path);
                    accessories_csv = exported.accessory_spreadsheet_path;
                }
            }
        }
    }
    let workbook: PathBuf = workbook.expect("a workbook");
    let firearms_csv: PathBuf = firearms_csv.expect("the firearm CSV file");
    let accessories_csv: PathBuf = accessories_csv.expect("the accessory CSV file");

    read_and_import("workbook", &[import_file(&workbook, SpreadsheetFormat::Xlsx)], count);
    read_and_import(
        "two CSV files",
        &[
            import_file(&firearms_csv, SpreadsheetFormat::Csv),
            import_file(&accessories_csv, SpreadsheetFormat::Csv),
        ],
        count,
    );
}
