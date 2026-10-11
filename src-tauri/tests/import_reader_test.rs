//! The import's reader against the real import helper (feature 008, T050;
//! research.md §8, §9, §11): `import_reader::read_files` starts
//! `CARGO_BIN_EXE_hoplodex --import-helper` for each file and returns the
//! tables it read, or refuses the whole import.
//!
//! - Each file of T049's hostile corpus (`support/hostile_spreadsheets.rs`) is
//!   refused within 5 s, with `IMPORT_LIMIT_EXCEEDED` naming the limit, the
//!   file and a workbook's sheet, or with `VALIDATION_ERROR` "<file>: could not
//!   read the file."; this process's resident memory rises by no more than
//!   512 MB while it is read, and the helper is gone afterwards. The helper may
//!   die on a crafted file; the app does not.
//! - A stop by the user ends the reading within 1 s at several points in a
//!   large valid file (`OPERATION_STOPPED`); a lock, a sleep or a quit, seen as
//!   the session no longer being the current one, ends it within 1 s too, with
//!   `DATABASE_CLOSED` whether or not the user also stopped it (research.md
//!   §11). Nothing is returned, so nothing is imported.
//! - What the app's own export writes (a workbook, and two CSV files) reads
//!   back as the tables `read_table` expects.
//!
//! The memory is this process's own, so every test here takes one lock: under
//! `cargo test` they run one at a time (nextest gives each its own process).

#[path = "support/preview_support.rs"]
mod preview_support;
mod support;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::accessories::ops as accessory_ops;
use hoplodex_lib::commands::firearms::ops as firearm_ops;
use hoplodex_lib::commands::import_export::ImportFile;
use hoplodex_lib::commands::import_export::ops as import_export_ops;
use hoplodex_lib::models::accessory::AccessoryInput;
use hoplodex_lib::services::import_reader::limits::ImportLimit;
use hoplodex_lib::services::import_reader::read_files;
use hoplodex_lib::services::spreadsheet::{ReadTable, SpreadsheetFormat, TableRows};
use serde_json::{Value, json};
use support::TestDb;
use support::hostile_spreadsheets::{
    self as hostile, Expect, large_valid_csv, large_valid_workbook, valid_export_csv,
    valid_export_workbook,
};
use tempfile::TempDir;

const HELPER: &str = env!("CARGO_BIN_EXE_hoplodex");
const IMPORT_HELPER: &str = "--import-helper";

/// The most a file may cost this process while it is read (SC-002).
const MEMORY_BUDGET: u64 = 512 * 1024 * 1024;
/// How long a refusal may take (SC-003).
const REFUSAL_BUDGET: Duration = Duration::from_secs(5);
/// The corpus cases whose refusal comes only after five million cells have
/// been parsed (a workbook of sixteen sheets past the cell limit in its
/// thirteenth). In a debug build `calamine` and `quick-xml` run unoptimized,
/// because their generic code is compiled with this crate, and the case takes
/// about 10 s (about 3.5 s optimized): the budget is 4x in a debug build.
const SLOW_IN_DEBUG: &[&str] = &["cells-16-sheets"];
/// How long a stop, a lock or a sleep may take to end the reading (SC-004).
const STOP_BUDGET: Duration = Duration::from_secs(1);

fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn import_file(path: &Path) -> ImportFile {
    let format = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xlsx")) {
        SpreadsheetFormat::Xlsx
    } else {
        SpreadsheetFormat::Csv
    };
    ImportFile { file_path: path.to_path_buf(), format }
}

fn files(paths: &[PathBuf]) -> Vec<ImportFile> {
    paths.iter().map(|p| import_file(p)).collect()
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

// ------------------------------------------------------------------ memory

/// This process's resident memory, in bytes. (On macOS `libc` marks its Mach
/// calls deprecated in favor of a crate this project doesn't use.)
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

/// Samples this process's resident memory while a file is read.
struct PeakMemory {
    stop: Arc<AtomicBool>,
    peak: Arc<AtomicU64>,
    sampler: Option<thread::JoinHandle<()>>,
    before: u64,
}

impl PeakMemory {
    fn start() -> Self {
        let before = resident_bytes();
        let stop = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicU64::new(before));
        let sampler = {
            let (stop, peak) = (Arc::clone(&stop), Arc::clone(&peak));
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    peak.fetch_max(resident_bytes(), Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(2));
                }
            })
        };
        Self { stop, peak, sampler: Some(sampler), before }
    }

    /// How far above where it began the memory rose.
    fn finish(mut self) -> u64 {
        self.stop.store(true, Ordering::Relaxed);
        self.sampler.take().unwrap().join().unwrap();
        self.peak.load(Ordering::Relaxed).saturating_sub(self.before)
    }
}

// ----------------------------------------------------------------- reading

fn read(files: &[ImportFile]) -> Result<Vec<ReadTable>, CommandError> {
    read_files(Path::new(HELPER), files, &|| false, &|| true, &mut |_, _, _| {})
}

fn no_helper_runs() -> bool {
    preview_support::wait_for(Duration::from_secs(3), || {
        preview_support::helper_children_of(IMPORT_HELPER).is_empty()
    })
}

fn limit_of(error: &CommandError) -> (String, Option<String>, ImportLimit) {
    assert_eq!(error.code, "IMPORT_LIMIT_EXCEEDED", "{error:?}");
    let details = error.details.as_deref().expect("details");
    let file = details["file"].as_str().expect("details.file").to_owned();
    let sheet = details.get("sheet").map(|s| s.as_str().expect("details.sheet").to_owned());
    let limit: ImportLimit = serde_json::from_value(details["limit"].clone()).expect("limit");
    (file, sheet, limit)
}

fn run_case(name: &str) {
    let _serial = serial();
    let case = hostile::corpus().into_iter().find(|c| c.name == name).expect("a corpus case");
    let dir = TempDir::new().unwrap();
    let paths = case.build(dir.path());
    let files = files(&paths);

    let memory = PeakMemory::start();
    let began = Instant::now();
    let outcome = read(&files);
    let took = began.elapsed();
    let rise = memory.finish();
    println!("{name}: refused in {took:?}, memory rose {} MB", rise / (1024 * 1024));

    let error = outcome.expect_err("the file is refused");
    match case.expect {
        Expect::Refused { file, .. } => {
            let (named, sheet, limit) = limit_of(&error);
            assert_eq!(named, file_name(&paths[file]), "{error:?}");
            assert!(
                case.expect.accepts_refusal(file, limit, sheet.as_deref()),
                "{name}: refused for {limit:?} in {sheet:?}, expected {:?}: {error:?}",
                case.expect
            );
            assert_eq!(error.message, limit.message(&named, sheet.as_deref()));
        }
        Expect::Unreadable => {
            assert_eq!(error.code, "VALIDATION_ERROR", "{error:?}");
            assert_eq!(
                error.message,
                format!("{}: could not read the file.", file_name(&paths[0])),
                "no parser detail is shown"
            );
        }
    }
    let budget = if cfg!(debug_assertions) && SLOW_IN_DEBUG.contains(&name) {
        REFUSAL_BUDGET * 4
    } else {
        REFUSAL_BUDGET
    };
    assert!(took < budget, "{name}: refused in {took:?}, over {budget:?}");
    assert!(rise <= MEMORY_BUDGET, "{name}: memory rose {} MB", rise / (1024 * 1024));
    assert!(no_helper_runs(), "{name}: the helper is still running");
}

macro_rules! corpus_cases {
    ($($test:ident => $case:literal),* $(,)?) => {
        $(
            #[test]
            fn $test() {
                run_case($case);
            }
        )*
        const CASES: &[&str] = &[$($case),*];
    };
}

corpus_cases! {
    shared_strings_declared_as_u64_max => "shared-u64-max",
    shared_strings_declared_one_over_the_limit => "shared-2000001",
    two_cells_in_the_far_corners => "corner-cells",
    a_zip_bomb => "zip-bomb",
    a_thousand_and_one_parts => "zip-entries",
    seventeen_sheets => "sheets-17",
    a_workbook_of_100001_rows => "rows-xlsx",
    a_workbook_row_of_257_cells => "columns-xlsx",
    sixteen_sheets_past_the_cell_limit => "cells-16-sheets",
    sparse_rows_padded_past_the_cell_limit => "cells-sparse",
    a_workbook_cell_of_32768_characters => "cell-text-xlsx",
    a_workbook_past_the_text_limit => "total-text",
    a_csv_of_100001_rows => "rows-csv",
    a_csv_row_of_257_cells => "columns-csv",
    a_csv_cell_of_32768_characters => "cell-text-csv",
    a_csv_field_of_200_mb => "field-200mb",
    a_csv_field_of_300_mb => "field-300mb",
    a_csv_of_25_million_empty_cells => "commas",
    a_csv_one_byte_over_256_mib => "csv-over-256mib",
    a_workbook_cut_in_half => "truncated",
    a_workbook_without_its_directory_end => "truncated-directory",
    a_workbook_with_flipped_bits => "bit-flipped",
    a_workbook_with_flipped_bits_in_its_parts => "bit-flipped-data",
    a_workbook_with_a_flipped_directory_signature => "bit-flipped-directory",
    an_archive_that_is_not_a_spreadsheet => "not-a-spreadsheet",
    a_second_file_past_the_row_limit => "two-files-second-rows",
    two_files_past_the_cell_limit_together => "two-files-cells-together",
}

#[test]
fn every_case_of_the_corpus_is_run() {
    let mut corpus: Vec<&str> = hostile::corpus().iter().map(|c| c.name).collect();
    let mut run = CASES.to_vec();
    corpus.sort_unstable();
    run.sort_unstable();
    assert_eq!(corpus, run);
}

// ----------------------------------------------------- what is accepted

fn makes(table: &ReadTable) -> Vec<String> {
    match &table.rows {
        TableRows::Firearms(rows) => rows.iter().map(|r| r.make.clone().unwrap()).collect(),
        TableRows::Accessories(rows) => rows.iter().map(|r| r.make.clone().unwrap()).collect(),
    }
}

#[test]
fn a_valid_workbook_and_a_valid_csv_read_as_firearm_tables() {
    let _serial = serial();
    let dir = TempDir::new().unwrap();
    let workbook = valid_export_workbook(&dir.path().join("valid.xlsx"));
    let csv = valid_export_csv(&dir.path().join("valid.csv"));

    for path in [&workbook, &csv] {
        let tables = read(&[import_file(path)]).unwrap();
        assert_eq!(tables.len(), 1, "{}", path.display());
        assert_eq!(tables[0].source, file_name(path));
        assert_eq!(makes(&tables[0]), ["Springfield"; 3], "{}", path.display());
    }
    assert!(no_helper_runs());
}

#[test]
fn progress_counts_the_bytes_of_the_files_up_to_their_total() {
    let _serial = serial();
    let dir = TempDir::new().unwrap();
    let first = large_valid_csv(&dir.path().join("a.csv"), 3_000);
    let second = valid_export_workbook(&dir.path().join("b.xlsx"));
    let total =
        std::fs::metadata(&first).unwrap().len() + std::fs::metadata(&second).unwrap().len();

    let mut seen: Vec<(u64, u64)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    read_files(
        Path::new(HELPER),
        &[import_file(&first), import_file(&second)],
        &|| false,
        &|| true,
        &mut |file, done, of| {
            seen.push((done, of));
            if names.last().map(String::as_str) != Some(file) {
                names.push(file.to_owned());
            }
        },
    )
    .unwrap();

    // Each report names the file being read (contracts/ui.md §2): a.csv
    // first, then b.xlsx.
    assert_eq!(names, ["a.csv", "b.xlsx"]);

    assert!(seen.iter().all(|(_, of)| *of == total), "{seen:?}");
    assert!(seen.windows(2).all(|w| w[0].0 <= w[1].0), "never goes back: {seen:?}");
    assert_eq!(seen.last(), Some(&(total, total)));
}

/// A workbook reports how far its reading has got while it arrives, while the
/// archive is checked and while the sheets are parsed (research.md §11), not
/// only at its end.
#[test]
fn a_workbook_reports_progress_between_nothing_and_everything() {
    let _serial = serial();
    let dir = TempDir::new().unwrap();
    let path = large_valid_workbook(&dir.path().join("large.xlsx"), 4_000);
    let total = std::fs::metadata(&path).unwrap().len();

    let mut seen: Vec<u64> = Vec::new();
    read_files(
        Path::new(HELPER),
        &[import_file(&path)],
        &|| false,
        &|| true,
        &mut |_, done, of| {
            assert_eq!(of, total);
            seen.push(done);
        },
    )
    .unwrap();

    assert!(seen.windows(2).all(|w| w[0] <= w[1]), "never goes back: {seen:?}");
    assert_eq!(seen.last(), Some(&total));
    let between: Vec<u64> = seen.iter().copied().filter(|d| *d > 0 && *d < total).collect();
    assert!(
        between.len() >= 10,
        "values between nothing and everything while it is read: {seen:?}"
    );
    // Not one jump from the arrival to the end: the sheets' parsing is
    // reported in steps of its own, past the half-way mark.
    assert!(
        between.iter().filter(|d| **d > total / 2).count() >= 3,
        "the parsing is reported as it goes: {seen:?}"
    );
}

fn seeded_collection() -> TestDb {
    let db = TestDb::new();
    for (make, model, serial) in
        [("Glock", "19", "A1"), ("Sig Sauer", "P226", "B2"), ("Colt", "Python", "C3")]
    {
        firearm_ops::create_firearm(&db.conn, &support::firearm(make, model, serial), false, None)
            .unwrap();
    }
    let optic: AccessoryInput = serde_json::from_value(
        json!({ "accessoryKindId": 1, "make": "Leupold", "model": "VX-5HD", "status": "active" }),
    )
    .unwrap();
    accessory_ops::create_accessory(&db.conn, &optic, None).unwrap();
    db
}

fn export(db: &TestDb, dir: &Path, format: SpreadsheetFormat) -> (PathBuf, Option<PathBuf>) {
    let records = import_export_ops::export_records(&db.conn, "all", None).unwrap();
    let result = import_export_ops::export_collection(
        &db.conn,
        dir,
        "export",
        format,
        &records,
        &mut |_, _| {},
    )
    .unwrap();
    (result.spreadsheet_path, result.accessory_spreadsheet_path)
}

#[test]
fn an_export_as_a_workbook_reads_back_as_its_two_tables() {
    let _serial = serial();
    let db = seeded_collection();
    let dir = TempDir::new().unwrap();
    let (workbook, none) = export(&db, dir.path(), SpreadsheetFormat::Xlsx);
    assert!(none.is_none());

    let tables = read(&[import_file(&workbook)]).unwrap();

    assert_eq!(tables.len(), 2);
    let name = file_name(&workbook);
    assert_eq!(tables[0].source, format!("{name}, sheet \"Firearms\""));
    assert_eq!(tables[1].source, format!("{name}, sheet \"Accessories\""));
    let mut firearms = makes(&tables[0]);
    firearms.sort();
    assert_eq!(firearms, ["Colt", "Glock", "Sig Sauer"]);
    assert_eq!(makes(&tables[1]), ["Leupold"]);
    assert!(matches!(tables[0].rows, TableRows::Firearms(_)));
    assert!(matches!(tables[1].rows, TableRows::Accessories(_)));
}

#[test]
fn an_export_as_two_csv_files_reads_back_as_one_table_each() {
    let _serial = serial();
    let db = seeded_collection();
    let dir = TempDir::new().unwrap();
    let (firearms, accessories) = export(&db, dir.path(), SpreadsheetFormat::Csv);
    let accessories = accessories.expect("a second file");

    let tables = read(&[import_file(&firearms), import_file(&accessories)]).unwrap();

    assert_eq!(tables.len(), 2);
    assert_eq!(tables[0].source, file_name(&firearms));
    assert_eq!(tables[1].source, file_name(&accessories));
    assert_eq!(makes(&tables[0]).len(), 3);
    assert_eq!(makes(&tables[1]), ["Leupold"]);
}

#[test]
fn a_cell_protected_against_formulas_in_a_csv_is_unprotected_and_a_header_is_not() {
    let _serial = serial();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("protected.csv");
    std::fs::write(
        &path,
        "firearm_type,make,model\nrifle,\"'=SUM(A1)\",\"'-x\"\nrifle,\"'plain\",m\n",
    )
    .unwrap();

    let tables = read(&[import_file(&path)]).unwrap();

    assert_eq!(makes(&tables[0]), ["=SUM(A1)", "'plain"]);
}

// ------------------------------------------------- stops, locks, deaths

/// A read of a large valid file that something ends once `trigger` of its bytes
/// have been parsed: the time from then until the reading returns.
fn ended_after(
    percent: u64,
    cancel_after: bool,
    current_until: bool,
) -> (Result<Vec<ReadTable>, CommandError>, Duration) {
    let dir = TempDir::new().unwrap();
    let path = large_valid_csv(&dir.path().join("large.csv"), 100_000);
    let triggered: Mutex<Option<Instant>> = Mutex::new(None);
    let cancelled = AtomicBool::new(false);
    let current = AtomicBool::new(true);

    let outcome = read_files(
        Path::new(HELPER),
        &[import_file(&path)],
        &|| cancelled.load(Ordering::SeqCst),
        &|| current.load(Ordering::SeqCst),
        &mut |_, done, total| {
            if done * 100 >= total * percent {
                let mut at = triggered.lock().unwrap();
                if at.is_none() {
                    *at = Some(Instant::now());
                    if cancel_after {
                        cancelled.store(true, Ordering::SeqCst);
                    }
                    if !current_until {
                        current.store(false, Ordering::SeqCst);
                    }
                }
            }
        },
    );
    let returned = Instant::now();
    let at = triggered.lock().unwrap().expect("the trigger point was reached while reading");
    (outcome, returned.duration_since(at))
}

#[test]
fn cancelling_stops_the_reading_within_a_second_wherever_it_is() {
    let _serial = serial();
    for percent in [1, 25, 60] {
        let (outcome, took) = ended_after(percent, true, true);
        let error = outcome.expect_err("stopped");
        assert_eq!(error.code, "OPERATION_STOPPED", "at {percent}%: {error:?}");
        assert_eq!(error.details.as_deref().unwrap()["importedCount"], json!(0));
        println!("stopped at {percent}%: {took:?}");
        assert!(took < STOP_BUDGET, "stopped at {percent}% in {took:?}");
        assert!(no_helper_runs(), "the helper is gone after a stop at {percent}%");
    }
}

#[test]
fn a_session_that_ends_goes_ahead_at_once_and_the_reading_ends_within_a_second() {
    let _serial = serial();
    for percent in [1, 25, 60] {
        let (outcome, took) = ended_after(percent, false, false);
        let error = outcome.expect_err("closed");
        assert_eq!(error.code, "DATABASE_CLOSED", "at {percent}%: {error:?}");
        println!("closed at {percent}%: {took:?}");
        assert!(took < STOP_BUDGET, "closed at {percent}% in {took:?}");
        assert!(no_helper_runs(), "the helper is gone after a close at {percent}%");
    }
}

#[test]
fn a_stop_made_by_the_session_ending_is_never_the_users_stop() {
    let _serial = serial();
    // A sleep stops the operation and begins the close: both are seen, and
    // the answer is the session's.
    let (outcome, took) = ended_after(10, true, false);
    let error = outcome.expect_err("closed");
    assert_eq!(error.code, "DATABASE_CLOSED", "{error:?}");
    assert!(took < STOP_BUDGET, "{took:?}");
}

#[test]
fn an_import_already_stopped_starts_no_helper() {
    let _serial = serial();
    let dir = TempDir::new().unwrap();
    let path = valid_export_csv(&dir.path().join("valid.csv"));

    let stopped =
        read_files(Path::new(HELPER), &[import_file(&path)], &|| true, &|| true, &mut |_, _, _| {})
            .unwrap_err();
    assert_eq!(stopped.code, "OPERATION_STOPPED");
    let closed = read_files(
        Path::new(HELPER),
        &[import_file(&path)],
        &|| true,
        &|| false,
        &mut |_, _, _| {},
    )
    .unwrap_err();
    assert_eq!(closed.code, "DATABASE_CLOSED");
}

#[cfg(unix)]
#[test]
fn a_helper_that_dies_or_says_nonsense_makes_the_file_unreadable() {
    let _serial = serial();
    let dir = TempDir::new().unwrap();
    let path = valid_export_csv(&dir.path().join("valid.csv"));

    // `true` ends at once; `echo` writes a line, which isn't a frame. (macOS
    // keeps `true` in /usr/bin, not /bin.)
    let true_program = if cfg!(target_os = "macos") { "/usr/bin/true" } else { "/bin/true" };
    for program in [true_program, "/bin/echo"] {
        let error = read_files(
            Path::new(program),
            &[import_file(&path)],
            &|| false,
            &|| true,
            &mut |_, _, _| {},
        )
        .unwrap_err();
        assert_eq!(error.code, "VALIDATION_ERROR", "{program}: {error:?}");
        assert_eq!(error.message, "valid.csv: could not read the file.");
    }
}

/// The job object's memory limit on Windows (research.md §8: the job's
/// `JobMemoryLimit` is `RLIMIT_DATA` there): a helper that holds more than its
/// job allows dies, without an answer, and nothing else does. The helper is
/// lied to, as a crafted file would: it is told the workbook is empty, then
/// sent 100 MiB of it, which it keeps. The control is the same traffic into the
/// app's own 512 MiB job, which the helper survives and answers `Unreadable`
/// (the bytes aren't an archive). A real read in another job, in flight while
/// the first helper dies, is the bystander.
#[cfg(windows)]
#[test]
fn a_helper_over_its_jobs_memory_limit_dies_and_only_it_does() {
    use hoplodex_lib::services::helper::job;
    use hoplodex_lib::services::import_reader::frames::{
        Format, FromHelper, ToHelper, read_from_helper, write_chunk, write_to_helper,
    };
    use hoplodex_lib::services::import_reader::limits::HELPER_MEMORY_BYTES;

    const SMALL_JOB: usize = 64 * 1024 * 1024;
    const SENT: usize = 100;

    /// Sends the lying workbook; what the helper answered (`None` when its
    /// output ended with no frame) and its exit code.
    fn lie_to_a_helper(memory_limit: usize) -> (Option<FromHelper>, u32, usize) {
        let (mut child, mut stdin, mut stdout) =
            job::spawn(Path::new(HELPER), IMPORT_HELPER, memory_limit, &[]).unwrap();
        let begin = ToHelper::Begin {
            format: Format::Xlsx,
            file_size: 0,
            file_name: "liar.xlsx".to_owned(),
        };
        let mut accepted = 0;
        if write_to_helper(&mut stdin, &begin).is_ok() {
            let megabyte = vec![0u8; 1024 * 1024];
            while accepted < SENT && write_chunk(&mut stdin, &megabyte).is_ok() {
                accepted += 1;
            }
            let _ = write_to_helper(&mut stdin, &ToHelper::End);
        }
        let mut answer = None;
        while let Ok(Some(frame)) = read_from_helper(&mut stdout) {
            answer = Some(frame);
        }
        let code = child.wait().unwrap();
        (answer, code, accepted)
    }

    let _serial = serial();
    let dir = TempDir::new().unwrap();
    let path = large_valid_csv(&dir.path().join("bystander.csv"), 100_000);
    let reading = AtomicBool::new(false);
    thread::scope(|scope| {
        let bystander = scope.spawn(|| {
            read_files(
                Path::new(HELPER),
                &[import_file(&path)],
                &|| false,
                &|| true,
                &mut |_, done, _| {
                    if done > 0 {
                        reading.store(true, Ordering::SeqCst);
                    }
                },
            )
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        while !reading.load(Ordering::SeqCst) && !bystander.is_finished() {
            assert!(Instant::now() < deadline, "the bystander never got going");
            thread::sleep(Duration::from_millis(1));
        }

        let (answer, code, accepted) = lie_to_a_helper(SMALL_JOB);
        println!("64 MiB job: answer {answer:?}, exit code {code:#x}, {accepted} MiB accepted");
        assert!(answer.is_none(), "a helper over its job's limit answered: {answer:?}");
        assert_ne!(code, 0, "a helper over its job's limit ended cleanly");
        assert!(accepted < SENT, "the helper took all {SENT} MiB inside a 64 MiB job");

        let (answer, code, accepted) = lie_to_a_helper(HELPER_MEMORY_BYTES);
        println!("512 MiB job: answer {answer:?}, exit code {code:#x}, {accepted} MiB accepted");
        assert_eq!(answer, Some(FromHelper::Unreadable));
        assert_eq!(code, 0);
        assert_eq!(accepted, SENT);

        let tables = bystander.join().unwrap().expect("a read in another job carries on");
        assert_eq!(tables.len(), 1);
    });
    assert!(no_helper_runs());
}

#[test]
fn a_file_that_is_not_there_is_unreadable_and_names_the_file() {
    let _serial = serial();
    let dir = TempDir::new().unwrap();
    let error = read(&[import_file(&dir.path().join("missing.csv"))]).unwrap_err();
    assert_eq!(error.code, "VALIDATION_ERROR");
    assert_eq!(error.message, "missing.csv: could not read the file.");
}

#[test]
fn the_error_of_a_refusal_has_the_contracts_shape() {
    let _serial = serial();
    let dir = TempDir::new().unwrap();
    let path = hostile::rows_100001_csv(dir.path());

    let error = read(&[import_file(&path)]).unwrap_err();

    let value: Value = serde_json::to_value(&error).unwrap();
    assert_eq!(value["code"], "IMPORT_LIMIT_EXCEEDED");
    assert_eq!(value["details"], json!({ "file": "rows-100001.csv", "limit": "rows" }));
    assert_eq!(
        value["message"],
        "rows-100001.csv: more than 100,000 rows. HoploDex imports at most 100,000 rows per table."
    );
}
