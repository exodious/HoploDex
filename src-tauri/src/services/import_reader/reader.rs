//! The child's side: opens the spreadsheet and sends its rows as frames,
//! stopping at the first limit it crosses (research.md §8, §9).
//!
//! The app starts itself again as `hoplodex --import-helper`. `main()` hands
//! the arguments to [`run`] before it sets up anything else, so the helper has
//! no Tauri, no keyring, no session and no logging. `run` confines the process
//! ([`confine`](crate::services::helper::confine)) and only then reads a frame.
//! It reads no file: the parent sends the file's bytes. Nothing the file says
//! is written anywhere: stderr is discarded by the parent, and no error from a
//! parser is kept, since an unreadable file is only `Unreadable`.
//!
//! A CSV is parsed as its chunks arrive, field by field, so a hostile field
//! costs no more than the buffer for the longest legal one. A workbook is kept
//! as received (at most [`MAX_FILE_BYTES`]) and then:
//! 1. its central directory is read, and more than [`MAX_ZIP_ENTRIES`] parts
//!    are refused;
//! 2. every part is inflated through a counting reader that stops at
//!    [`MAX_UNPACKED_BYTES`] in all (the true sizes, not the declared ones),
//!    and the shared strings' declared and present counts are read with
//!    `quick-xml`, before `calamine` is given the workbook, since `calamine`
//!    reserves what the shared strings declare;
//! 3. more than [`MAX_SHEETS`] sheets are refused;
//! 4. each sheet is read cell by cell with `worksheet_cells_reader`, which
//!    builds no dense grid, under the row, column, cell and text limits.
//!
//! Progress follows the same stages (research.md §11): the bytes arriving, the
//! parts inflated in step 2, and the parts `calamine` reads in step 4, the last
//! two in unpacked bytes against the sizes the archive declares.

use std::cell::Cell;
use std::io::{self, BufWriter, Cursor, Read, Seek, SeekFrom, Write};
use std::rc::Rc;

use calamine::{Data, DataRef, Reader, Xlsx, XlsxError};

use super::frames::{
    BATCH_SOFT_BYTES, Format, FromHelper, MAX_BATCH_ROWS, ToHelper, read_to_helper,
    write_from_helper, write_rows,
};
use super::limits::{
    HELPER_MEMORY_BYTES, ImportLimit, MAX_CELL_CHARS, MAX_CELLS, MAX_COLUMNS, MAX_FILE_BYTES,
    MAX_ROWS, MAX_SHARED_STRINGS, MAX_SHEETS, MAX_TOTAL_TEXT_BYTES, MAX_UNPACKED_BYTES,
    MAX_ZIP_ENTRIES,
};
use crate::services::helper::confine;

/// What the app starts its helper with.
pub const ARGUMENT: &str = "--import-helper";

/// Runs the helper to its end and exits the process: `args` are the
/// process's arguments after the program name, the first being
/// [`ARGUMENT`]. It never returns to the app's own start-up path.
pub fn run(args: &[String]) -> ! {
    // Nothing a panic says is wanted anywhere, and stderr is discarded.
    std::panic::set_hook(Box::new(|_| {}));
    let _ = args;
    if confine::confine(HELPER_MEMORY_BYTES).is_err() {
        // Not confined: read no file.
        std::process::exit(3);
    }
    let served = std::panic::catch_unwind(|| {
        serve(io::stdin().lock(), BufWriter::with_capacity(1 << 16, io::stdout().lock()))
    });
    std::process::exit(match served {
        Ok(Ok(())) => 0,
        Ok(Err(_)) => 4,
        // The parent sees the pipe close without `Finished`: `Unreadable`.
        Err(_) => 5,
    });
}

/// Why reading stopped before the end.
#[derive(Debug)]
enum Stop {
    /// A limit, and the workbook's sheet it was found in.
    Refused(ImportLimit, Option<String>),
    /// The file can't be read: not a table, a damaged archive, a parser error.
    Unreadable,
    /// The pipe to the parent failed or the parent sent a bad frame.
    Io(io::Error),
}

impl From<io::Error> for Stop {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

fn refused(limit: ImportLimit) -> Stop {
    Stop::Refused(limit, None)
}

/// One file: the `Begin`, the chunks, the `End`, and the rows (or the
/// refusal) back. Returns when the answer has been sent; an error is the
/// pipe's.
pub fn serve(mut input: impl Read, mut output: impl Write) -> io::Result<()> {
    let Some(ToHelper::Begin { format, file_size, .. }) = read_to_helper(&mut input)? else {
        // The parent went away, or said something else first.
        return Ok(());
    };
    let progress = Rc::new(Cell::new(0));
    let mut incoming = Incoming { input: &mut input, received: 0, progress: Rc::clone(&progress) };
    let mut sink = Sink::new(&mut output, file_size, progress);
    let read = match format {
        Format::Csv => read_csv(&mut incoming, &mut sink),
        Format::Xlsx => read_xlsx(&mut incoming, &mut sink, file_size),
    }
    .and_then(|()| sink.finish_sheet());
    match read {
        Ok(()) => write_from_helper(&mut output, &FromHelper::Finished),
        Err(Stop::Refused(limit, sheet)) => {
            write_from_helper(&mut output, &FromHelper::Refused { limit, sheet })
        }
        Err(Stop::Unreadable) => write_from_helper(&mut output, &FromHelper::Unreadable),
        Err(Stop::Io(err)) => Err(err),
    }
}

/// The file's chunks, as they arrive from the parent.
struct Incoming<'a, R> {
    input: &'a mut R,
    received: u64,
    /// Bytes of the file read so far, for the `Progress` frames.
    progress: Rc<Cell<u64>>,
}

impl<R: Read> Incoming<'_, R> {
    /// The next chunk, or `None` at the `End`. The file may not grow past
    /// [`MAX_FILE_BYTES`] whatever it declared.
    fn next(&mut self) -> Result<Option<Vec<u8>>, Stop> {
        match read_to_helper(self.input)? {
            Some(ToHelper::Chunk(bytes)) => {
                self.received += bytes.len() as u64;
                if self.received > MAX_FILE_BYTES {
                    return Err(refused(ImportLimit::FileSize));
                }
                Ok(Some(bytes))
            }
            Some(ToHelper::End) => Ok(None),
            // The parent closed the pipe mid-file, or sent a second `Begin`.
            _ => Err(Stop::Unreadable),
        }
    }
}

/// The rows going back to the parent, in batches, with the limits that count
/// across rows.
struct Sink<'a, W> {
    out: &'a mut W,
    batch: Vec<Vec<String>>,
    batch_bytes: usize,
    /// The sheet being read (a workbook's), for a refusal to name.
    sheet: Option<String>,
    /// Rows sent in this sheet, the header included.
    rows: usize,
    cells: u64,
    text_bytes: u64,
    /// Bytes of the file read (CSV) or parsed (workbook), against `total`.
    progress: Rc<Cell<u64>>,
    reported: u64,
    total: u64,
}

impl<'a, W: Write> Sink<'a, W> {
    fn new(out: &'a mut W, total: u64, progress: Rc<Cell<u64>>) -> Self {
        Self {
            out,
            batch: Vec::new(),
            batch_bytes: 0,
            sheet: None,
            rows: 0,
            cells: 0,
            text_bytes: 0,
            progress,
            reported: 0,
            total,
        }
    }

    fn refuse(&self, limit: ImportLimit) -> Stop {
        Stop::Refused(limit, self.sheet.clone())
    }

    /// A workbook's next sheet: what was read of the last is sent first.
    fn begin_sheet(&mut self, name: &str) -> Result<(), Stop> {
        self.finish_sheet()?;
        self.sheet = Some(name.to_owned());
        self.rows = 0;
        write_from_helper(self.out, &FromHelper::Sheet { name: name.to_owned() })?;
        Ok(())
    }

    fn finish_sheet(&mut self) -> Result<(), Stop> {
        self.flush()
    }

    /// A cell as it is read, before it is kept: its column, its length and
    /// the text read in all.
    fn check_cell(&mut self, text: &str, column: usize) -> Result<(), Stop> {
        if column >= MAX_COLUMNS {
            return Err(self.refuse(ImportLimit::Columns));
        }
        if text.len() > MAX_CELL_CHARS && text.chars().count() > MAX_CELL_CHARS {
            return Err(self.refuse(ImportLimit::CellText));
        }
        self.text_bytes += text.len() as u64;
        if self.text_bytes > MAX_TOTAL_TEXT_BYTES {
            return Err(self.refuse(ImportLimit::TotalText));
        }
        Ok(())
    }

    /// A row, complete: the header or a data row. A row is as long as its
    /// last cell, and every cell counts.
    fn push_row(&mut self, row: Vec<String>) -> Result<(), Stop> {
        self.rows += 1;
        if self.rows > MAX_ROWS + 1 {
            return Err(self.refuse(ImportLimit::Rows));
        }
        self.cells += row.len() as u64;
        if self.cells > MAX_CELLS {
            return Err(self.refuse(ImportLimit::Cells));
        }
        self.batch_bytes += row.iter().map(String::len).sum::<usize>();
        self.batch.push(row);
        if self.batch.len() >= MAX_BATCH_ROWS || self.batch_bytes >= BATCH_SOFT_BYTES {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Stop> {
        if !self.batch.is_empty() {
            write_rows(self.out, &self.batch)?;
            self.batch.clear();
            self.batch_bytes = 0;
        }
        self.report()
    }

    /// Where a workbook's reading has got to: `done` of `of` through the stage
    /// that starts at `start` and takes `weight` of the [`WORKBOOK_UNITS`].
    fn stage(&mut self, start: u64, weight: u64, done: u64, of: u64) -> Result<(), Stop> {
        self.progress.set(stage_value(start, weight, done, of));
        self.report()
    }

    /// A `Progress` frame when the file has been read further than the last
    /// one said. The declared size is only for the bar: it is clamped.
    fn report(&mut self) -> Result<(), Stop> {
        let done = self.progress.get();
        if done > self.reported {
            self.reported = done;
            let total = self.total.max(done);
            write_from_helper(self.out, &FromHelper::Progress { done, total })?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CSV

/// The longest field kept: [`MAX_CELL_CHARS`] characters of four bytes. A field
/// that fills a buffer one byte bigger is over the cell limit whatever its
/// characters, and is refused while it is still arriving.
const FIELD_BUFFER: usize = MAX_CELL_CHARS * 4 + 1;

const BOM: &[u8] = b"\xEF\xBB\xBF";

fn read_csv<R: Read, W: Write>(
    incoming: &mut Incoming<'_, R>,
    sink: &mut Sink<'_, W>,
) -> Result<(), Stop> {
    let mut parser = csv_core::ReaderBuilder::new().build();
    let mut field = vec![0u8; FIELD_BUFFER];
    let mut field_len = 0;
    let mut row: Vec<String> = Vec::new();
    let mut first_piece = true;
    let mut chunk: Vec<u8> = Vec::new();
    let mut at = 0;
    let mut ended = false;
    loop {
        // An empty chunk is not the end: an empty input is, to the parser.
        while at == chunk.len() && !ended {
            match incoming.next()? {
                Some(next) => {
                    chunk = next;
                    at = 0;
                    if first_piece {
                        // A byte-order mark starts some files. The parser
                        // takes an input that is only the mark for the end of
                        // the file, and drops the mark only if it comes whole,
                        // so it is dropped here, whatever the chunks.
                        while chunk.len() < BOM.len() && !ended {
                            match incoming.next()? {
                                Some(more) => chunk.extend_from_slice(&more),
                                None => ended = true,
                            }
                        }
                        first_piece = false;
                        if chunk.starts_with(BOM) {
                            at = BOM.len();
                        }
                    }
                    incoming.progress.set(incoming.received);
                    sink.report()?;
                }
                None => {
                    chunk.clear();
                    at = 0;
                    ended = true;
                }
            }
        }
        let (result, read, written) = parser.read_field(&chunk[at..], &mut field[field_len..]);
        at += read;
        field_len += written;
        match result {
            csv_core::ReadFieldResult::InputEmpty => {
                if ended {
                    break;
                }
            }
            csv_core::ReadFieldResult::OutputFull => {
                return Err(sink.refuse(ImportLimit::CellText));
            }
            csv_core::ReadFieldResult::Field { record_end } => {
                let text =
                    std::str::from_utf8(&field[..field_len]).map_err(|_| Stop::Unreadable)?;
                sink.check_cell(text, row.len())?;
                row.push(text.to_owned());
                field_len = 0;
                if record_end {
                    // A record may have more or fewer cells than the header: a
                    // missing cell reads as blank, and a cell with no header
                    // is ignored (a column is found by its header).
                    sink.push_row(std::mem::take(&mut row))?;
                }
            }
            csv_core::ReadFieldResult::End => break,
        }
    }
    incoming.progress.set(incoming.received);
    Ok(())
}

// ---------------------------------------------------------------------------
// Workbooks

/// What a workbook's reading is measured in. The bar follows three stages,
/// each by the parts' unpacked bytes against the sizes the archive declares
/// (research.md §11): the file arriving (by its own bytes, the only size known
/// before the archive can be read), the archive check, and the sheets being
/// parsed. The declared sizes are only for the bar, never trusted, so each is
/// clamped.
const WORKBOOK_UNITS: u64 = 1000;
const ARRIVE_AT: u64 = 0;
const ARRIVE_WEIGHT: u64 = 100;
const CHECK_AT: u64 = ARRIVE_AT + ARRIVE_WEIGHT;
const CHECK_WEIGHT: u64 = 200;
const PARSE_AT: u64 = CHECK_AT + CHECK_WEIGHT;
const PARSE_WEIGHT: u64 = WORKBOOK_UNITS - PARSE_AT;

/// `done` of `of` through the stage that starts at `start` and takes `weight`.
/// Past `of` is the stage's end; an `of` of nothing (the archive declares no
/// size) is too.
fn stage_value(start: u64, weight: u64, done: u64, of: u64) -> u64 {
    if of == 0 {
        return start + weight;
    }
    start + (u128::from(weight) * u128::from(done.min(of)) / u128::from(of)) as u64
}

/// A reader that stops, with an error of its own, once the parts read through
/// it have unpacked to more than [`MAX_UNPACKED_BYTES`] in all, and says how
/// far the archive check has got.
struct Counted<'a, 'b, R, W> {
    inner: R,
    total: &'a mut u64,
    /// The declared unpacked size of all the parts, for the bar.
    declared: u64,
    sink: &'a mut Sink<'b, W>,
}

impl<R: Read, W: Write> Read for Counted<'_, '_, R, W> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        *self.total += n as u64;
        if *self.total > MAX_UNPACKED_BYTES {
            return Err(io::Error::other("unpacked size"));
        }
        match self.sink.stage(CHECK_AT, CHECK_WEIGHT, *self.total, self.declared) {
            Ok(()) => Ok(n),
            // The parent's pipe failed: this error is what the parent sees,
            // as the archive failing, and the answer then fails the same way.
            Err(Stop::Io(err)) => Err(err),
            Err(_) => Err(io::Error::other("progress")),
        }
    }
}

/// Is `err` the counting reader stopping, as opposed to the archive failing?
fn over_unpacked_limit(total: u64) -> bool {
    total > MAX_UNPACKED_BYTES
}

/// A part of the archive as `check_archive` found it: where its stored bytes
/// are, and how big it is stored and unpacked.
#[derive(Clone, Copy)]
struct Part {
    start: u64,
    stored: u64,
    unpacked: u64,
}

/// `calamine`'s reader of the workbook, noting how much of each part it has
/// read. A part counts for its unpacked size in proportion to the stored bytes
/// read, so the central directory (which `calamine` reads first, at the end of
/// the file) and the order the parts are read in don't move the bar.
struct Tracked<'a> {
    inner: Cursor<&'a [u8]>,
    /// The parts that hold bytes, in the order the file holds them.
    parts: Vec<Part>,
    /// Stored bytes read so far of each part.
    read: Vec<u64>,
    /// Unpacked bytes of the parts read so far, and of all of them.
    done: u64,
    declared: u64,
    progress: Rc<Cell<u64>>,
}

impl<'a> Tracked<'a> {
    fn new(bytes: &'a [u8], mut parts: Vec<Part>, progress: Rc<Cell<u64>>) -> Self {
        parts.sort_by_key(|part| part.start);
        let declared = parts.iter().map(|part| part.unpacked).fold(0u64, u64::saturating_add);
        Self {
            inner: Cursor::new(bytes),
            read: vec![0; parts.len()],
            parts,
            done: 0,
            declared,
            progress,
        }
    }

    /// The bytes from `from` to `to` have been read.
    fn note(&mut self, from: u64, to: u64) {
        let first =
            self.parts.partition_point(|part| part.start.saturating_add(part.stored) <= from);
        for index in first..self.parts.len() {
            let part = self.parts[index];
            if part.start >= to {
                break;
            }
            let now = (to - part.start).min(part.stored);
            let before = self.read[index];
            if now > before {
                self.read[index] = now;
                let unpacked = |stored: u64| {
                    (u128::from(part.unpacked) * u128::from(stored) / u128::from(part.stored))
                        as u64
                };
                self.done = self.done.saturating_add(unpacked(now) - unpacked(before));
            }
        }
        self.progress.set(stage_value(PARSE_AT, PARSE_WEIGHT, self.done, self.declared));
    }
}

impl Read for Tracked<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let from = self.inner.position();
        let n = self.inner.read(buf)?;
        self.note(from, self.inner.position());
        Ok(n)
    }
}

impl Seek for Tracked<'_> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

/// Reads the shared strings part and refuses what it declares or holds past
/// the limits: `count` and `uniqueCount` before `calamine` reserves room for
/// them, the `<si>` elements present, and the text they hold.
fn scan_shared_strings(xml: impl Read) -> Result<(), Stop> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_reader(io::BufReader::new(xml));
    let mut buf = Vec::new();
    let (mut present, mut text) = (0u64, 0u64);
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => match e.local_name().as_ref() {
                b"sst" => {
                    for attribute in e.attributes().with_checks(false) {
                        let attribute = attribute.map_err(|_| Stop::Unreadable)?;
                        if matches!(attribute.key.as_ref(), b"count" | b"uniqueCount")
                            && declares_too_many(&attribute.value)
                        {
                            return Err(refused(ImportLimit::SharedStrings));
                        }
                    }
                }
                b"si" => {
                    present += 1;
                    if present > MAX_SHARED_STRINGS {
                        return Err(refused(ImportLimit::SharedStrings));
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) => text += t.len() as u64,
            Ok(Event::CData(t)) => text += t.len() as u64,
            Ok(Event::Eof) => return Ok(()),
            Ok(_) => {}
            // Includes the counting reader stopping: the caller looks at the
            // total.
            Err(_) => return Err(Stop::Unreadable),
        }
        if text > MAX_TOTAL_TEXT_BYTES {
            return Err(refused(ImportLimit::TotalText));
        }
    }
}

/// Does a `count` or `uniqueCount` attribute say more than
/// [`MAX_SHARED_STRINGS`]? A number too big for 64 bits does; text that is no
/// number says nothing (`calamine` ignores it too).
fn declares_too_many(value: &[u8]) -> bool {
    let digits = value.trim_ascii();
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return false;
    }
    match std::str::from_utf8(digits).ok().and_then(|n| n.parse::<u64>().ok()) {
        Some(n) => n > MAX_SHARED_STRINGS,
        None => true,
    }
}

/// Is this archive entry the workbook's shared strings?
fn is_shared_strings(name: &str) -> bool {
    name.rsplit('/').next().is_some_and(|file| file.eq_ignore_ascii_case("sharedStrings.xml"))
}

/// The archive's checks that need no `calamine`: the number of parts, the
/// true size of all of them, and the shared strings. Returns where the parts
/// are, for the bar.
fn check_archive<W: Write>(bytes: &[u8], sink: &mut Sink<'_, W>) -> Result<Vec<Part>, Stop> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| Stop::Unreadable)?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(refused(ImportLimit::ZipEntries));
    }
    // What the parts declare, for the bar (the headers only; nothing is
    // inflated yet).
    let mut parts = Vec::new();
    let mut declared = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(|_| Stop::Unreadable)?;
        if entry.is_dir() {
            continue;
        }
        declared = declared.saturating_add(entry.size());
        if let Some(start) = entry.data_start()
            && entry.compressed_size() > 0
        {
            parts.push(Part { start, stored: entry.compressed_size(), unpacked: entry.size() });
        }
    }
    let mut unpacked = 0u64;
    let mut block = vec![0u8; 1 << 16];
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|_| Stop::Unreadable)?;
        if entry.is_dir() {
            continue;
        }
        let shared = is_shared_strings(entry.name());
        let mut counted = Counted { inner: entry, total: &mut unpacked, declared, sink };
        let result = if shared {
            scan_shared_strings(&mut counted)
        } else {
            drain(&mut counted, &mut block)
        };
        // The counting reader stopped it, or it stopped itself.
        let over = over_unpacked_limit(*counted.total);
        match result {
            Err(Stop::Unreadable) | Ok(()) if over => {
                return Err(refused(ImportLimit::UnpackedSize));
            }
            other => other?,
        }
        sink.stage(CHECK_AT, CHECK_WEIGHT, unpacked, declared)?;
    }
    Ok(parts)
}

/// Reads a part to its end and keeps none of it.
fn drain(reader: &mut impl Read, block: &mut [u8]) -> Result<(), Stop> {
    loop {
        match reader.read(block) {
            Ok(0) => return Ok(()),
            Ok(_) => {}
            Err(_) => return Err(Stop::Unreadable),
        }
    }
}

fn read_xlsx<R: Read, W: Write>(
    incoming: &mut Incoming<'_, R>,
    sink: &mut Sink<'_, W>,
    declared: u64,
) -> Result<(), Stop> {
    // The bar counts the stages of a workbook's reading, not the file's bytes.
    sink.total = WORKBOOK_UNITS;
    // The file as received. Room for what it declares, up to the limit, is
    // asked for once, so the buffer is not copied as it grows.
    let mut bytes: Vec<u8> = Vec::new();
    bytes.try_reserve_exact(declared.min(MAX_FILE_BYTES) as usize).map_err(|_| Stop::Unreadable)?;
    while let Some(chunk) = incoming.next()? {
        bytes.extend_from_slice(&chunk);
        sink.stage(ARRIVE_AT, ARRIVE_WEIGHT, incoming.received, declared)?;
    }

    let parts = check_archive(&bytes, sink)?;

    // From here the bar follows how much of the parts the sheets have read.
    let progress = Rc::new(Cell::new(PARSE_AT));
    sink.progress = Rc::clone(&progress);
    let source = Tracked::new(&bytes[..], parts, progress);
    let mut book: Xlsx<_> = Xlsx::new(source).map_err(|_| Stop::Unreadable)?;
    let names = book.sheet_names();
    if names.len() > MAX_SHEETS {
        return Err(refused(ImportLimit::Sheets));
    }
    for name in names {
        sink.begin_sheet(&name)?;
        let mut cells = match book.worksheet_cells_reader(&name) {
            Ok(cells) => cells,
            // A chart sheet: nothing to read, as `worksheet_range` found.
            Err(XlsxError::NotAWorksheet(_)) => continue,
            Err(_) => return Err(Stop::Unreadable),
        };
        read_sheet(&mut cells, sink)?;
    }
    Ok(())
}

/// One sheet's cells, in the order the file holds them, made into rows. The
/// first row with a cell is the header. A row skipped between two rows with
/// cells is an empty one, so a row keeps its number, and a row is as long as
/// its last cell.
fn read_sheet<'a, RS: Read + Seek, W: Write>(
    cells: &mut calamine::XlsxCellReader<'a, RS>,
    sink: &mut Sink<'_, W>,
) -> Result<(), Stop> {
    let mut first: Option<u32> = None;
    // The row being built, and its number in the file.
    let mut current: Option<(u32, Vec<String>)> = None;
    while let Some(cell) = cells.next_cell().map_err(|_| Stop::Unreadable)? {
        let (row_number, column) = cell.get_position();
        let value = cell.get_value();
        // An empty cell (a styled blank) is no cell, as in `worksheet_range`.
        if matches!(value, DataRef::Empty) {
            continue;
        }
        let text: std::borrow::Cow<'_, str> = match value {
            DataRef::SharedString(s) => std::borrow::Cow::Borrowed(*s),
            DataRef::String(s) => std::borrow::Cow::Borrowed(s.as_str()),
            other => std::borrow::Cow::Owned(Data::from(other.clone()).to_string()),
        };
        let column = column as usize;
        // The column first: a cell in the sheet's far corner is a column
        // that is too far out, whatever row it is in.
        sink.check_cell(&text, column)?;
        let first_row = *first.get_or_insert(row_number);
        if row_number < first_row {
            return Err(Stop::Unreadable);
        }
        if (row_number - first_row) as usize > MAX_ROWS {
            return Err(sink.refuse(ImportLimit::Rows));
        }
        match &mut current {
            Some((number, row)) if *number == row_number => {
                if column < row.len() {
                    // The same cell twice: the later one stands.
                    row[column] = text.into_owned();
                } else {
                    row.resize(column, String::new());
                    row.push(text.into_owned());
                }
            }
            _ => {
                let mut row = vec![String::new(); column];
                row.push(text.into_owned());
                if let Some((number, done)) = current.replace((row_number, row)) {
                    if row_number < number {
                        return Err(Stop::Unreadable);
                    }
                    sink.push_row(done)?;
                    for _ in number + 1..row_number {
                        sink.push_row(Vec::new())?;
                    }
                }
            }
        }
    }
    if let Some((_, row)) = current {
        sink.push_row(row)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::import_reader::frames::{read_from_helper, write_chunk, write_to_helper};

    /// A CSV file sent in chunks of `chunk` bytes, and every frame the reader
    /// answers with.
    fn run_csv(bytes: &[u8], chunk: usize) -> Vec<FromHelper> {
        let mut input = Vec::new();
        write_to_helper(
            &mut input,
            &ToHelper::Begin {
                format: Format::Csv,
                file_size: bytes.len() as u64,
                file_name: "t.csv".into(),
            },
        )
        .unwrap();
        for part in bytes.chunks(chunk.max(1)) {
            write_chunk(&mut input, part).unwrap();
        }
        write_to_helper(&mut input, &ToHelper::End).unwrap();
        let mut output = Vec::new();
        serve(&input[..], &mut output).unwrap();
        let mut frames = Vec::new();
        let mut reader = &output[..];
        while let Some(frame) = read_from_helper(&mut reader).unwrap() {
            frames.push(frame);
        }
        frames
    }

    fn rows_of(frames: &[FromHelper]) -> Vec<Vec<String>> {
        frames
            .iter()
            .filter_map(|f| match f {
                FromHelper::Rows(rows) => Some(rows.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    fn csv_crate_rows(bytes: &[u8]) -> Vec<Vec<String>> {
        let mut reader = csv::ReaderBuilder::new().has_headers(false).from_reader(bytes);
        reader.records().map(|r| r.unwrap().iter().map(str::to_owned).collect()).collect()
    }

    #[test]
    fn a_csv_reads_as_the_csv_crate_reads_it_whatever_the_chunks() {
        let inputs: [&[u8]; 9] = [
            b"a,b,c\n1,2,3\n",
            b"a,b,c\r\n1,2,3\r\n",
            b"a,b,c\n1,2,3",
            b"a,b\n\n1,2\n\n\n3,4\n",
            b"a,b\n\"x,\ny\",\"say \"\"hi\"\"\"\n",
            b"\xEF\xBB\xBFa,b\n1,2\n",
            "make,model\nS\u{e9}curit\u{e9},\u{1F52B}\n".as_bytes(),
            b"a,,c\n,,\n",
            b"a\n\"\"\n\"abc\"def\n",
        ];
        for input in inputs {
            for chunk in [1, 2, 3, 7, input.len(), 4096] {
                let frames = run_csv(input, chunk);
                assert_eq!(frames.last(), Some(&FromHelper::Finished), "{input:?} by {chunk}");
                // The BOM is the reader's to drop; the csv crate drops it too.
                assert_eq!(rows_of(&frames), csv_crate_rows(input), "{input:?} by {chunk}");
            }
        }
    }

    #[test]
    fn an_empty_chunk_is_not_the_end_of_the_file() {
        let mut input = Vec::new();
        write_to_helper(
            &mut input,
            &ToHelper::Begin { format: Format::Csv, file_size: 8, file_name: String::new() },
        )
        .unwrap();
        for part in [&b"a,b\n"[..], b"", b"1,2\n"] {
            write_chunk(&mut input, part).unwrap();
        }
        write_to_helper(&mut input, &ToHelper::End).unwrap();
        let mut output = Vec::new();
        serve(&input[..], &mut output).unwrap();
        let mut reader = &output[..];
        let mut rows = Vec::new();
        while let Some(frame) = read_from_helper(&mut reader).unwrap() {
            if let FromHelper::Rows(more) = frame {
                rows.extend(more);
            }
        }
        assert_eq!(rows, [["a", "b"], ["1", "2"]]);
    }

    #[test]
    fn text_that_is_not_utf8_is_unreadable() {
        for input in [&b"a,b\n\xff,\xfe\n"[..], b"a,b\n1,\xc3\n"] {
            let frames = run_csv(input, 4096);
            assert_eq!(frames.last(), Some(&FromHelper::Unreadable), "{input:?}");
        }
    }

    #[test]
    fn rows_of_unequal_length_are_read_as_they_are() {
        let frames = run_csv(b"a,b\n1\n1,2,3\n", 4096);
        assert_eq!(frames.last(), Some(&FromHelper::Finished));
        assert_eq!(rows_of(&frames), [vec!["a", "b"], vec!["1"], vec!["1", "2", "3"]]);
    }

    #[test]
    fn a_cell_is_refused_past_32767_characters_counted_as_characters() {
        let line = |cell: &str| format!("a,b\n{cell},x\n").into_bytes();
        // The longest allowed, of four-byte characters: 131,068 bytes.
        let longest = "\u{1F52B}".repeat(MAX_CELL_CHARS);
        let frames = run_csv(&line(&longest), 1 << 20);
        assert_eq!(frames.last(), Some(&FromHelper::Finished));
        assert_eq!(rows_of(&frames)[1][0].chars().count(), MAX_CELL_CHARS);
        // One character more, of any width.
        for over in [format!("{longest}x"), "x".repeat(MAX_CELL_CHARS + 1)] {
            let frames = run_csv(&line(&over), 1 << 20);
            assert_eq!(
                frames.last(),
                Some(&FromHelper::Refused { limit: ImportLimit::CellText, sheet: None })
            );
        }
        // A field that would never end is refused while it arrives.
        let frames = run_csv(&line(&"y".repeat(10 * 1024 * 1024)), 1 << 20);
        assert_eq!(
            frames.last(),
            Some(&FromHelper::Refused { limit: ImportLimit::CellText, sheet: None })
        );
    }

    #[test]
    fn a_csv_is_refused_past_256_cells_in_a_row_and_100001_rows() {
        let wide = vec!["a"; MAX_COLUMNS + 1].join(",");
        let frames = run_csv(format!("{wide}\n").as_bytes(), 4096);
        assert_eq!(
            frames.last(),
            Some(&FromHelper::Refused { limit: ImportLimit::Columns, sheet: None })
        );
        let ok = vec!["a"; MAX_COLUMNS].join(",");
        assert_eq!(run_csv(format!("{ok}\n").as_bytes(), 4096).last(), Some(&FromHelper::Finished));

        let table = |data_rows: usize| format!("a\n{}", "x\n".repeat(data_rows)).into_bytes();
        assert_eq!(run_csv(&table(MAX_ROWS), 1 << 20).last(), Some(&FromHelper::Finished));
        assert_eq!(
            run_csv(&table(MAX_ROWS + 1), 1 << 20).last(),
            Some(&FromHelper::Refused { limit: ImportLimit::Rows, sheet: None })
        );
    }

    #[test]
    fn a_declared_shared_string_count_over_the_limit_is_seen_before_it_is_parsed() {
        for over in ["2000001", "18446744073709551615", "99999999999999999999999999", " 2000001 "] {
            assert!(declares_too_many(over.as_bytes()), "{over}");
        }
        for fine in ["0", "2000000", "", "many", "-1", "1.5"] {
            assert!(!declares_too_many(fine.as_bytes()), "{fine}");
        }
        let sst =
            |attrs: &str, body: &str| format!("<sst xmlns=\"x\" {attrs}>{body}</sst>").into_bytes();
        let scan = |xml: Vec<u8>| scan_shared_strings(&xml[..]);
        assert!(matches!(
            scan(sst("count=\"3\" uniqueCount=\"2000001\"", "")),
            Err(Stop::Refused(ImportLimit::SharedStrings, None))
        ));
        assert!(matches!(
            scan(sst("count=\"5000000000\"", "")),
            Err(Stop::Refused(ImportLimit::SharedStrings, None))
        ));
        assert!(
            scan(sst("count=\"3\" uniqueCount=\"2\"", "<si><t>a</t></si><si><t>b</t></si>"))
                .is_ok()
        );
        // Declared counts that don't match what is present are not a refusal.
        assert!(
            scan(sst("count=\"1\" uniqueCount=\"1\"", "<si><t>a</t></si><si><t>b</t></si>"))
                .is_ok()
        );
    }

    #[test]
    fn a_pipe_that_ends_before_the_file_does_is_unreadable() {
        let mut input = Vec::new();
        write_to_helper(
            &mut input,
            &ToHelper::Begin { format: Format::Csv, file_size: 8, file_name: String::new() },
        )
        .unwrap();
        write_chunk(&mut input, b"a,b\n1,2\n").unwrap();
        let mut output = Vec::new();
        serve(&input[..], &mut output).unwrap();
        let mut reader = &output[..];
        let mut last = None;
        while let Some(frame) = read_from_helper(&mut reader).unwrap() {
            last = Some(frame);
        }
        assert_eq!(last, Some(FromHelper::Unreadable));
    }
}
