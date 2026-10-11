//! The frames the reading child and the app exchange over the child's pipes
//! (research.md §8, data-model.md "Import helper frames").
//!
//! They ride on [`services::helper::frames`]' framing (a big-endian length,
//! then a tag byte and the payload), with a lower limit of their own, which
//! they give it: either side refuses a frame over [`MAX_IMPORT_FRAME`] before
//! it allocates for it.
//! Every number is big-endian and every string is a `u32` length and UTF-8
//! bytes. A decoder checks each count against the bytes that remain before it
//! allocates, so a hostile count costs nothing.

use std::io::{self, Read, Write};

use super::limits::ImportLimit;
use crate::services::helper::frames::{invalid, read_frame, write_frame};

/// The largest frame either side accepts: 64 MiB, more than the largest
/// possible row (256 cells of 32,767 characters of 4 bytes).
pub const MAX_IMPORT_FRAME: usize = 64 * 1024 * 1024;
/// The most of the file one `Chunk` holds.
pub const MAX_CHUNK: usize = 1024 * 1024;
/// The most rows one `Rows` frame holds.
pub const MAX_BATCH_ROWS: usize = 500;
/// A `Rows` frame ends early once its cells pass this many bytes; a single
/// row larger than that goes alone.
pub const BATCH_SOFT_BYTES: usize = 2 * 1024 * 1024;

/// What the file is, which decides how the helper parses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Csv,
    Xlsx,
}

/// Parent to helper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToHelper {
    /// The file is about to arrive. `file_name` is for messages only.
    Begin { format: Format, file_size: u64, file_name: String },
    /// Up to [`MAX_CHUNK`] bytes of the file.
    Chunk(Vec<u8>),
    /// The whole file has been sent.
    End,
}

/// Helper to parent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FromHelper {
    /// How far the reading has got, for the bar: `done` of `total`, the bytes
    /// of a CSV, the stages of a workbook (reader.rs). Only the ratio means
    /// anything.
    Progress { done: u64, total: u64 },
    /// The sheet the following `Rows` belong to (an XLSX sheet; a CSV is one
    /// unnamed sheet and sends none).
    Sheet { name: String },
    /// Up to [`MAX_BATCH_ROWS`] rows, the header row first in a sheet's first
    /// batch.
    Rows(Vec<Vec<String>>),
    /// The file was read to its end within every limit.
    Finished,
    /// The file passed a limit.
    Refused { limit: ImportLimit, sheet: Option<String> },
    /// The file couldn't be read. The reason is never shown or logged.
    Unreadable,
}

const BEGIN: u8 = 0x01;
const CHUNK: u8 = 0x02;
const END: u8 = 0x03;

const PROGRESS: u8 = 0x11;
const SHEET: u8 = 0x12;
const ROWS: u8 = 0x13;
const FINISHED: u8 = 0x14;
const REFUSED: u8 = 0x15;
const UNREADABLE: u8 = 0x16;

fn send(w: &mut impl Write, tag: u8, payload: &[u8]) -> io::Result<()> {
    write_frame(w, tag, payload, MAX_IMPORT_FRAME)
}

/// The next frame, or `None` at a clean end of input. A length over
/// [`MAX_IMPORT_FRAME`] is refused before any of the frame is read.
fn receive(r: &mut impl Read) -> io::Result<Option<(u8, Vec<u8>)>> {
    read_frame(r, MAX_IMPORT_FRAME)
}

fn put_u32(out: &mut Vec<u8>, n: usize) -> io::Result<()> {
    let n = u32::try_from(n).map_err(|_| invalid("count too large"))?;
    out.extend_from_slice(&n.to_be_bytes());
    Ok(())
}

fn put_str(out: &mut Vec<u8>, s: &str) -> io::Result<()> {
    put_u32(out, s.len())?;
    out.extend_from_slice(s.as_bytes());
    Ok(())
}

/// A payload being taken apart.
struct Take<'a>(&'a [u8]);

impl<'a> Take<'a> {
    fn bytes(&mut self, n: usize) -> io::Result<&'a [u8]> {
        if n > self.0.len() {
            return Err(invalid("frame shorter than it says"));
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }

    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    fn u32(&mut self) -> io::Result<usize> {
        Ok(u32::from_be_bytes(self.bytes(4)?.try_into().expect("4 bytes")) as usize)
    }

    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_be_bytes(self.bytes(8)?.try_into().expect("8 bytes")))
    }

    fn string(&mut self) -> io::Result<String> {
        let n = self.u32()?;
        String::from_utf8(self.bytes(n)?.to_vec()).map_err(|_| invalid("text isn't UTF-8"))
    }

    fn done(&self) -> io::Result<()> {
        if self.0.is_empty() { Ok(()) } else { Err(invalid("frame longer than it says")) }
    }
}

fn limit_code(limit: ImportLimit) -> u8 {
    ImportLimit::ALL.iter().position(|l| *l == limit).expect("every limit is listed") as u8
}

fn limit_from_code(code: u8) -> io::Result<ImportLimit> {
    ImportLimit::ALL.get(code as usize).copied().ok_or_else(|| invalid("unknown limit"))
}

/// Sends a `Chunk` without copying the bytes into a [`ToHelper`] first.
pub fn write_chunk(w: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    if bytes.len() > MAX_CHUNK {
        return Err(invalid("chunk too large"));
    }
    send(w, CHUNK, bytes)
}

pub fn write_to_helper(w: &mut impl Write, frame: &ToHelper) -> io::Result<()> {
    match frame {
        ToHelper::Begin { format, file_size, file_name } => {
            let mut payload = vec![match format {
                Format::Csv => 0,
                Format::Xlsx => 1,
            }];
            payload.extend_from_slice(&file_size.to_be_bytes());
            put_str(&mut payload, file_name)?;
            send(w, BEGIN, &payload)
        }
        ToHelper::Chunk(bytes) => write_chunk(w, bytes),
        ToHelper::End => send(w, END, &[]),
    }
}

/// The next frame from the parent, or `None` when it has closed the pipe.
pub fn read_to_helper(r: &mut impl Read) -> io::Result<Option<ToHelper>> {
    let Some((tag, payload)) = receive(r)? else { return Ok(None) };
    let mut take = Take(&payload);
    let frame = match tag {
        BEGIN => {
            let format = match take.u8()? {
                0 => Format::Csv,
                1 => Format::Xlsx,
                _ => return Err(invalid("unknown format")),
            };
            let file_size = take.u64()?;
            let file_name = take.string()?;
            take.done()?;
            ToHelper::Begin { format, file_size, file_name }
        }
        CHUNK => {
            if payload.len() > MAX_CHUNK {
                return Err(invalid("chunk too large"));
            }
            return Ok(Some(ToHelper::Chunk(payload)));
        }
        END => {
            take.done()?;
            ToHelper::End
        }
        _ => return Err(invalid("unknown frame")),
    };
    Ok(Some(frame))
}

/// Sends a `Rows` frame without copying the rows into a [`FromHelper`] first.
pub fn write_rows(w: &mut impl Write, rows: &[Vec<String>]) -> io::Result<()> {
    if rows.len() > MAX_BATCH_ROWS {
        return Err(invalid("too many rows in a batch"));
    }
    let mut payload = Vec::new();
    put_u32(&mut payload, rows.len())?;
    for row in rows {
        put_u32(&mut payload, row.len())?;
        for cell in row {
            put_str(&mut payload, cell)?;
        }
    }
    send(w, ROWS, &payload)
}

pub fn write_from_helper(w: &mut impl Write, frame: &FromHelper) -> io::Result<()> {
    match frame {
        FromHelper::Progress { done, total } => {
            let mut payload = [0u8; 16];
            payload[..8].copy_from_slice(&done.to_be_bytes());
            payload[8..].copy_from_slice(&total.to_be_bytes());
            send(w, PROGRESS, &payload)
        }
        FromHelper::Sheet { name } => {
            let mut payload = Vec::with_capacity(4 + name.len());
            put_str(&mut payload, name)?;
            send(w, SHEET, &payload)
        }
        FromHelper::Rows(rows) => write_rows(w, rows),
        FromHelper::Finished => send(w, FINISHED, &[]),
        FromHelper::Refused { limit, sheet } => {
            let mut payload = vec![limit_code(*limit)];
            match sheet {
                Some(name) => {
                    payload.push(1);
                    put_str(&mut payload, name)?;
                }
                None => payload.push(0),
            }
            send(w, REFUSED, &payload)
        }
        FromHelper::Unreadable => send(w, UNREADABLE, &[]),
    }
}

/// The next frame from the helper, or `None` when it has closed the pipe.
pub fn read_from_helper(r: &mut impl Read) -> io::Result<Option<FromHelper>> {
    let Some((tag, payload)) = receive(r)? else { return Ok(None) };
    let mut take = Take(&payload);
    let frame = match tag {
        PROGRESS => {
            let (done, total) = (take.u64()?, take.u64()?);
            take.done()?;
            FromHelper::Progress { done, total }
        }
        SHEET => {
            let name = take.string()?;
            take.done()?;
            FromHelper::Sheet { name }
        }
        ROWS => {
            let row_count = take.u32()?;
            if row_count > MAX_BATCH_ROWS {
                return Err(invalid("too many rows in a batch"));
            }
            let mut rows = Vec::with_capacity(row_count);
            for _ in 0..row_count {
                let cell_count = take.u32()?;
                // Every cell takes at least its four length bytes.
                if cell_count > take.0.len() / 4 {
                    return Err(invalid("row longer than the frame"));
                }
                let mut row = Vec::with_capacity(cell_count);
                for _ in 0..cell_count {
                    row.push(take.string()?);
                }
                rows.push(row);
            }
            take.done()?;
            FromHelper::Rows(rows)
        }
        FINISHED => {
            take.done()?;
            FromHelper::Finished
        }
        REFUSED => {
            let limit = limit_from_code(take.u8()?)?;
            let sheet = match take.u8()? {
                0 => None,
                1 => Some(take.string()?),
                _ => return Err(invalid("bad sheet marker")),
            };
            take.done()?;
            FromHelper::Refused { limit, sheet }
        }
        UNREADABLE => {
            take.done()?;
            FromHelper::Unreadable
        }
        _ => return Err(invalid("unknown frame")),
    };
    Ok(Some(frame))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_helper_round_trip(frame: ToHelper) {
        let mut wire = Vec::new();
        write_to_helper(&mut wire, &frame).unwrap();
        assert_eq!(read_to_helper(&mut &wire[..]).unwrap(), Some(frame));
    }

    fn from_helper_round_trip(frame: FromHelper) {
        let mut wire = Vec::new();
        write_from_helper(&mut wire, &frame).unwrap();
        let mut reader = &wire[..];
        assert_eq!(read_from_helper(&mut reader).unwrap(), Some(frame));
        assert_eq!(read_from_helper(&mut reader).unwrap(), None, "nothing after it");
    }

    #[test]
    fn the_parents_frames_round_trip() {
        to_helper_round_trip(ToHelper::Begin {
            format: Format::Xlsx,
            file_size: u64::MAX,
            file_name: "inventaire d'été.xlsx".into(),
        });
        to_helper_round_trip(ToHelper::Begin {
            format: Format::Csv,
            file_size: 0,
            file_name: String::new(),
        });
        to_helper_round_trip(ToHelper::Chunk(vec![0, 1, 2, 255]));
        to_helper_round_trip(ToHelper::Chunk(vec![7; MAX_CHUNK]));
        to_helper_round_trip(ToHelper::Chunk(Vec::new()));
        to_helper_round_trip(ToHelper::End);
    }

    #[test]
    fn the_helpers_frames_round_trip() {
        from_helper_round_trip(FromHelper::Progress { done: 12, total: u64::MAX });
        from_helper_round_trip(FromHelper::Sheet { name: "Sheet \"3\" \u{1F52B}".into() });
        from_helper_round_trip(FromHelper::Rows(Vec::new()));
        from_helper_round_trip(FromHelper::Rows(vec![
            vec!["a".into(), String::new(), "\u{1F52B}\n,\"".into()],
            Vec::new(),
            vec![String::new(); 256],
        ]));
        from_helper_round_trip(FromHelper::Finished);
        from_helper_round_trip(FromHelper::Unreadable);
        from_helper_round_trip(FromHelper::Refused { limit: ImportLimit::Rows, sheet: None });
        for limit in ImportLimit::ALL {
            from_helper_round_trip(FromHelper::Refused { limit, sheet: Some("S".into()) });
        }
    }

    #[test]
    fn frames_follow_one_another() {
        let mut wire = Vec::new();
        write_from_helper(&mut wire, &FromHelper::Sheet { name: "A".into() }).unwrap();
        write_from_helper(&mut wire, &FromHelper::Rows(vec![vec!["x".into()]])).unwrap();
        write_from_helper(&mut wire, &FromHelper::Finished).unwrap();
        let mut reader = &wire[..];
        assert_eq!(
            read_from_helper(&mut reader).unwrap(),
            Some(FromHelper::Sheet { name: "A".into() })
        );
        assert_eq!(
            read_from_helper(&mut reader).unwrap(),
            Some(FromHelper::Rows(vec![vec!["x".into()]]))
        );
        assert_eq!(read_from_helper(&mut reader).unwrap(), Some(FromHelper::Finished));
        assert_eq!(read_from_helper(&mut reader).unwrap(), None);
    }

    #[test]
    fn a_frame_over_the_import_limit_is_refused_by_both_ends() {
        // Sending: one byte over, with no 64 MiB cell needed to see it.
        struct Sink;
        impl Write for Sink {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let payload = vec![0u8; MAX_IMPORT_FRAME];
        assert!(send(&mut Sink, CHUNK, &payload).is_err());
        assert!(send(&mut Sink, CHUNK, &payload[..MAX_IMPORT_FRAME - 1]).is_ok());
        assert!(write_chunk(&mut Sink, &vec![0u8; MAX_CHUNK + 1]).is_err());

        // Receiving: the length alone is enough, and it is below the TIFF
        // mode's limit, which shows the limit is the mode's own.
        let mut wire = (MAX_IMPORT_FRAME as u32 + 1).to_be_bytes().to_vec();
        wire.push(CHUNK);
        for err in [
            read_to_helper(&mut &wire[..]).unwrap_err(),
            read_from_helper(&mut &wire[..]).unwrap_err(),
        ] {
            assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        }
        const { assert!(MAX_IMPORT_FRAME < crate::services::helper::frames::MAX_FRAME) };
        assert!(read_from_helper(&mut &0u32.to_be_bytes()[..]).is_err());
    }

    #[test]
    fn a_frame_exactly_at_the_limit_is_read() {
        let mut wire = Vec::new();
        wire.extend_from_slice(&(MAX_IMPORT_FRAME as u32).to_be_bytes());
        wire.push(UNREADABLE);
        wire.extend_from_slice(&vec![0u8; MAX_IMPORT_FRAME - 1]);
        // The length is accepted; the surplus payload is what is wrong.
        let err = read_from_helper(&mut &wire[..]).unwrap_err();
        assert_eq!(err.to_string(), "frame longer than it says");
    }

    #[test]
    fn a_truncated_or_malformed_frame_is_an_error() {
        let mut wire = Vec::new();
        write_from_helper(&mut wire, &FromHelper::Rows(vec![vec!["abc".into(); 3]])).unwrap();
        for cut in 1..wire.len() {
            assert!(read_from_helper(&mut &wire[..cut]).is_err(), "cut at {cut}");
        }

        let frame = |tag: u8, payload: &[u8]| {
            let mut wire = Vec::new();
            write_frame(&mut wire, tag, payload, MAX_IMPORT_FRAME).unwrap();
            wire
        };
        let bad: &[(u8, &[u8])] = &[
            (0x7f, &[]),                                   // unknown tag
            (FINISHED, &[0]),                              // payload where there is none
            (PROGRESS, &[0; 15]),                          // short
            (SHEET, &[0, 0, 0, 2, 0xff, 0xfe]),            // not UTF-8
            (SHEET, &[0, 0, 0, 9, b'a']),                  // string longer than the frame
            (ROWS, &[0, 0, 1, 0xf5]),                      // 501 rows
            (ROWS, &[0, 0, 0, 1, 0xff, 0xff, 0xff, 0xff]), // hostile cell count
            (REFUSED, &[99, 0]),                           // unknown limit
            (REFUSED, &[0, 2]),                            // bad sheet marker
            (REFUSED, &[0, 0, 0]),                         // trailing byte
        ];
        for (tag, payload) in bad {
            let wire = frame(*tag, payload);
            assert!(read_from_helper(&mut &wire[..]).is_err(), "tag {tag:#x} {payload:?}");
        }

        let mut begin = frame(BEGIN, &[9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(read_to_helper(&mut &begin[..]).is_err(), "unknown format");
        begin = frame(CHUNK, &vec![0; MAX_CHUNK + 1]);
        assert!(read_to_helper(&mut &begin[..]).is_err(), "oversized chunk");
        assert!(read_to_helper(&mut &frame(PROGRESS, &[0; 16])[..]).is_err(), "wrong direction");
    }

    #[test]
    fn a_batch_of_more_than_500_rows_is_not_sent() {
        let rows = vec![Vec::new(); MAX_BATCH_ROWS + 1];
        assert!(write_rows(&mut Vec::new(), &rows).is_err());
        assert!(write_rows(&mut Vec::new(), &rows[..MAX_BATCH_ROWS]).is_ok());
    }
}
