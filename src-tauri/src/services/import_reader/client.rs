//! The app's side: starts the reading child, receives its frames, and ends it
//! on cancel, lock, close or timeout (research.md §8, §11).
//!
//! One child reads one file. The file goes to it in 1 MiB chunks from a thread
//! of its own, and its rows come back in batches, which are made into import
//! rows as they arrive (`TableReader`), so neither the file nor the cells of
//! its table are held whole. The calling thread watches: every 100 ms it asks
//! whether the session is still the one that began the import and whether the
//! user stopped it, and ends the child at once if not. The child's death, 120 s
//! without a frame, a malformed frame, or a frame that makes no sense are
//! `Unreadable`.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use super::frames::{
    Format, FromHelper, MAX_CHUNK, ToHelper, read_from_helper, write_chunk, write_to_helper,
};
use super::limits::{
    HELPER_IDLE_TIMEOUT, HELPER_MEMORY_BYTES, ImportLimit, MAX_CELLS, MAX_COLUMNS, MAX_FILE_BYTES,
    MAX_ROWS, MAX_TOTAL_TEXT_BYTES,
};
use super::reader::ARGUMENT;
use crate::commands::CommandError;
use crate::commands::import_export::ImportFile;
use crate::models::database::OperationKind;
use crate::services::helper::handle::{HelperProcess, Stdin};
use crate::services::spreadsheet::{
    ReadTable, SpreadsheetFormat, TableKind, TableReader, unprotect_csv_cell,
};

/// How often the watcher looks at the cancel flag and the session (SC-004).
const POLL: Duration = Duration::from_millis(100);

/// A backstop for the whole reading, past which the child is killed whatever
/// it is doing. The limit that matters is [`HELPER_IDLE_TIMEOUT`]; this keeps
/// the shared exchange's watchdog from ending a long, healthy reading.
const WHOLE_READING: Duration = Duration::from_secs(12 * 60 * 60);

/// What the files of an import have used so far, which the limits count
/// together, and the tables they have given.
#[derive(Default)]
struct Budget {
    cells: u64,
    text_bytes: u64,
    /// The kinds of table already read. A second table of a kind is an error
    /// (FR-022) that needs no rows, so its rows are not kept: what is held
    /// is at most one firearm table and one accessory table, whatever the
    /// cells in the files.
    seen: Vec<TableKind>,
}

/// How reading one file ended.
enum End {
    Finished,
    Refused(ImportLimit, Option<String>),
    Unreadable,
    /// The user stopped it, and the session is still the one that began it.
    Stopped,
    /// The session ended or is closing.
    Closed,
}

/// What the thread that sends the file ended with.
enum Sent {
    Done,
    /// The file grew past the size limit while it was sent.
    Grew,
    /// The file couldn't be opened or read.
    ReadFailed,
    /// The child stopped taking bytes. The child's own frames say why.
    PipeFailed,
}

enum Event {
    Frame(FromHelper),
    /// The child's output ended, or held something that isn't a frame.
    Closed,
    Sent(Sent),
}

/// Reads and recognises every non-blank sheet of the files, without saving
/// anything (FR-022: the files are read whole, and pass, before a row is
/// saved).
///
/// - `helper_exe` is the executable started again as `--import-helper`: the
///   app's own, or in a test `CARGO_BIN_EXE_hoplodex`.
/// - `cancel` says the user stopped the import; `is_current` that the session
///   that began it is still the open one. Both are asked every 100 ms and the
///   child is killed on either. A stop that the user made while the session is
///   current is `OPERATION_STOPPED`; one made after it ended, or by the
///   session ending (a lock, a sleep, a quit), is `DATABASE_CLOSED`
///   (research.md §11).
/// - `progress` is given the bytes of the files parsed so far and their total.
///
/// A file over a limit is `IMPORT_LIMIT_EXCEEDED`; one that can't be read is
/// `VALIDATION_ERROR` "<file>: could not read the file."; either refuses the
/// whole import, the other file included.
pub fn read_files(
    helper_exe: &Path,
    files: &[ImportFile],
    cancel: &dyn Fn() -> bool,
    is_current: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Vec<ReadTable>, CommandError> {
    // The size of each file, from its metadata, before any child starts.
    let mut sizes = Vec::with_capacity(files.len());
    for file in files {
        let name = file_name(&file.file_path);
        let size = std::fs::metadata(&file.file_path)
            .map_err(|_| CommandError::import_unreadable(&name))?
            .len();
        if size > MAX_FILE_BYTES {
            return Err(CommandError::import_limit_exceeded(&name, None, ImportLimit::FileSize));
        }
        sizes.push(size);
    }
    let total: u64 = sizes.iter().sum();
    let mut budget = Budget::default();
    let mut tables = Vec::new();
    let mut before = 0;
    for (file, size) in files.iter().zip(sizes) {
        let mut offset_progress = |done: u64, _of: u64| progress(before + done.min(size), total);
        tables.extend(read_one(
            helper_exe,
            file,
            size,
            &mut budget,
            cancel,
            is_current,
            &mut offset_progress,
        )?);
        before += size;
        progress(before, total);
    }
    Ok(tables)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// The sheets of the file being read, as their rows arrive.
struct Sheets {
    format: SpreadsheetFormat,
    /// Each sheet's name (none for a CSV) and its rows so far.
    sheets: Vec<(Option<String>, TableReader)>,
    /// Rows received in the sheet being read, the header included.
    rows: usize,
}

impl Sheets {
    fn current_sheet(&self) -> Option<String> {
        self.sheets.last().and_then(|(name, _)| name.clone())
    }

    fn refuse(&self, limit: ImportLimit) -> End {
        End::Refused(limit, self.current_sheet())
    }

    /// One frame from the child: `Some` when it ends the reading.
    fn take(
        &mut self,
        frame: FromHelper,
        budget: &mut Budget,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Option<End> {
        match frame {
            FromHelper::Progress { done, total } => progress(done, total),
            FromHelper::Sheet { name } => {
                self.sheets.push((Some(name), TableReader::new()));
                self.rows = 0;
            }
            FromHelper::Rows(rows) => {
                if self.sheets.is_empty() {
                    // A CSV is one sheet with no name.
                    self.sheets.push((None, TableReader::new()));
                }
                for mut row in rows {
                    // The limits again, as a second line (research.md §9).
                    self.rows += 1;
                    if self.rows > MAX_ROWS + 1 {
                        return Some(self.refuse(ImportLimit::Rows));
                    }
                    if row.len() > MAX_COLUMNS {
                        return Some(self.refuse(ImportLimit::Columns));
                    }
                    budget.cells += row.len() as u64;
                    if budget.cells > MAX_CELLS {
                        return Some(self.refuse(ImportLimit::Cells));
                    }
                    budget.text_bytes += row.iter().map(|cell| cell.len() as u64).sum::<u64>();
                    if budget.text_bytes > MAX_TOTAL_TEXT_BYTES {
                        return Some(self.refuse(ImportLimit::TotalText));
                    }
                    if self.format == SpreadsheetFormat::Csv && self.rows > 1 {
                        for cell in &mut row {
                            let plain = unprotect_csv_cell(cell);
                            if plain.len() != cell.len() {
                                *cell = plain.to_owned();
                            }
                        }
                    }
                    let (_, table) = self.sheets.last_mut().expect("a sheet was pushed");
                    table.push_row(row);
                    if self.rows == 1
                        && let Some(kind) = table.kind()
                    {
                        // The header has just decided which table this is.
                        if budget.seen.contains(&kind) {
                            table.discard_rows();
                        } else {
                            budget.seen.push(kind);
                        }
                    }
                }
            }
            FromHelper::Finished => return Some(End::Finished),
            FromHelper::Refused { limit, sheet } => return Some(End::Refused(limit, sheet)),
            FromHelper::Unreadable => return Some(End::Unreadable),
        }
        None
    }

    /// The tables, once the child has finished: a sheet with no header is
    /// dropped, and the rest are named by the file, and by the sheet if there
    /// are several.
    fn tables(self, file: &str) -> Result<Vec<ReadTable>, CommandError> {
        let several = self.sheets.iter().filter(|(_, table)| table.has_header()).count() > 1;
        let mut tables = Vec::new();
        for (sheet, reader) in self.sheets {
            let source = match sheet {
                Some(sheet) if several => format!("{file}, sheet \"{sheet}\""),
                _ => file.to_owned(),
            };
            tables.extend(reader.finish(source)?);
        }
        Ok(tables)
    }
}

fn read_one(
    helper_exe: &Path,
    file: &ImportFile,
    size: u64,
    budget: &mut Budget,
    cancel: &dyn Fn() -> bool,
    is_current: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Vec<ReadTable>, CommandError> {
    let name = file_name(&file.file_path);
    // Nothing is started for an import that has already been stopped.
    if let Some(end) = stopped(cancel, is_current) {
        return Err(failure(end, &name));
    }
    let process =
        HelperProcess::spawn(helper_exe, ARGUMENT, HELPER_MEMORY_BYTES).map_err(|err| {
            log::error!("could not start the import reader: {err}");
            CommandError::new("INTERNAL_ERROR", "The import reader could not be started.")
        })?;
    let mut sheets = Sheets { format: file.format, sheets: Vec::new(), rows: 0 };
    let ended = process.exchange(WHOLE_READING, |stdin, stdout| {
        let started = Instant::now();
        // When the last chunk went to the child, in milliseconds since
        // `started`: sending the file is activity too.
        let sent_at = AtomicU64::new(0);
        let end = thread::scope(|scope| {
            let (events, received) = mpsc::channel();
            let frames = events.clone();
            scope.spawn(move || {
                loop {
                    match read_from_helper(stdout) {
                        Ok(Some(frame)) => {
                            if frames.send(Event::Frame(frame)).is_err() {
                                break;
                            }
                        }
                        Ok(None) | Err(_) => {
                            let _ = frames.send(Event::Closed);
                            break;
                        }
                    }
                }
            });
            let sent_at = &sent_at;
            scope.spawn(move || {
                let _ = events.send(Event::Sent(send_file(stdin, file, size, started, sent_at)));
            });

            let mut last_frame = started;
            let mut last_poll = Instant::now() - POLL;
            let end = loop {
                if last_poll.elapsed() >= POLL {
                    last_poll = Instant::now();
                    if let Some(end) = stopped(cancel, is_current) {
                        break end;
                    }
                    let quiet_since = last_frame
                        .max(started + Duration::from_millis(sent_at.load(Ordering::Relaxed)));
                    if quiet_since.elapsed() > HELPER_IDLE_TIMEOUT {
                        break End::Unreadable;
                    }
                }
                match received.recv_timeout(POLL) {
                    Ok(Event::Frame(frame)) => {
                        last_frame = Instant::now();
                        if let Some(end) = sheets.take(frame, budget, progress) {
                            break end;
                        }
                    }
                    // The child ended without `Finished`, `Refused` or
                    // `Unreadable`: it died, or sent what isn't a frame.
                    Ok(Event::Closed) => break End::Unreadable,
                    Ok(Event::Sent(Sent::Done)) => {}
                    Ok(Event::Sent(Sent::Grew)) => {
                        break sheets.refuse(ImportLimit::FileSize);
                    }
                    Ok(Event::Sent(Sent::ReadFailed)) => break End::Unreadable,
                    // The child's own answer may be on its way (it can end
                    // the reading, and stop listening, with a refusal): wait
                    // for it, or for its output to end.
                    Ok(Event::Sent(Sent::PipeFailed)) => {}
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break End::Unreadable,
                }
            };
            // Whatever ended it, the child ends now, which also ends the two
            // threads blocked on its pipes.
            process.kill();
            end
        });
        Ok(Some(end))
    });
    // Gone, and waited for, before this returns.
    process.reap();
    match ended.unwrap_or(End::Unreadable) {
        End::Finished => {
            progress(size, size);
            sheets.tables(&name)
        }
        end => Err(failure(end, &name)),
    }
}

/// Has the import been stopped or has its session ended? The session first:
/// `OPERATION_STOPPED` is the user's, and only while the session still stands
/// (research.md §11).
fn stopped(cancel: &dyn Fn() -> bool, is_current: &dyn Fn() -> bool) -> Option<End> {
    if !is_current() {
        Some(End::Closed)
    } else if cancel() {
        Some(End::Stopped)
    } else {
        None
    }
}

fn failure(end: End, file: &str) -> CommandError {
    match end {
        End::Finished | End::Unreadable => CommandError::import_unreadable(file),
        End::Refused(limit, sheet) => {
            CommandError::import_limit_exceeded(file, sheet.as_deref(), limit)
        }
        End::Stopped => CommandError::operation_stopped(OperationKind::Import, Some(0), None),
        End::Closed => CommandError::database_closed(),
    }
}

/// Sends `Begin`, the file in chunks, and `End`. The file is counted as it
/// is read: one that grows past the limit is refused whatever its size was.
fn send_file(
    stdin: &mut Stdin,
    file: &ImportFile,
    size: u64,
    started: Instant,
    sent_at: &AtomicU64,
) -> Sent {
    let Ok(mut source) = File::open(&file.file_path) else { return Sent::ReadFailed };
    let begin = ToHelper::Begin {
        format: match file.format {
            SpreadsheetFormat::Csv => Format::Csv,
            SpreadsheetFormat::Xlsx => Format::Xlsx,
        },
        file_size: size,
        file_name: file_name(&file.file_path),
    };
    if write_to_helper(stdin, &begin).is_err() {
        return Sent::PipeFailed;
    }
    let mut chunk = vec![0u8; MAX_CHUNK];
    let mut sent = 0u64;
    loop {
        let filled = match fill(&mut source, &mut chunk) {
            Ok(filled) => filled,
            Err(_) => return Sent::ReadFailed,
        };
        if filled == 0 {
            break;
        }
        sent += filled as u64;
        if sent > MAX_FILE_BYTES {
            return Sent::Grew;
        }
        if write_chunk(stdin, &chunk[..filled]).is_err() {
            return Sent::PipeFailed;
        }
        sent_at.store(started.elapsed().as_millis() as u64, Ordering::Relaxed);
    }
    match write_to_helper(stdin, &ToHelper::End) {
        Ok(()) => Sent::Done,
        Err(_) => Sent::PipeFailed,
    }
}

/// Reads until `buf` is full or the file ends.
fn fill(source: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match source.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
    Ok(filled)
}
