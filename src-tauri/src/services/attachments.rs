//! Reading files the user drops onto the window. The webview never sees the
//! file itself — the desktop shell hands the frontend the dropped paths
//! (WebKitGTK exposes no `File` for a dragged-in file) — so the bytes and name
//! are read here instead, and the result is stored exactly as if it
//! had come from the file picker.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use rusqlite::{Connection, named_params};

use crate::commands::CommandError;
use crate::models::record::RecordRef;
use crate::models::rules::{MAX_ATTACHMENT_BYTES_PER_RECORD, size_label};
use crate::services::document_types::HEADER_BYTES;

pub struct AttachmentFile {
    pub bytes: Vec<u8>,
    pub filename: String,
}

/// Reads a file to attach without ever holding more than it was admitted at
/// (001 FR-007 and FR-010, amended 2026-11-04). `admit` judges the size the
/// opened file's metadata gives before any byte is read, `check_head` its
/// name and first [`HEADER_BYTES`] before the rest is, and a file that grows
/// past its admitted size while it is read is refused rather than read on.
pub fn read_attachment_file(
    path: &Path,
    admit: impl FnOnce(u64) -> Result<(), CommandError>,
    check_head: impl FnOnce(&str, &[u8]) -> Result<(), CommandError>,
) -> Result<AttachmentFile, CommandError> {
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| CommandError::not_found("That isn't a file."))?;
    if path.is_dir() {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            format!("{filename} is a folder. Drop the files inside it instead."),
        ));
    }
    let unreadable = |err: std::io::Error| {
        // Not the path: its file name becomes the attachment's (logging.rs).
        log::error!("couldn't read a file to attach: {err}");
        CommandError::not_found(format!("{filename} couldn't be read."))
    };
    // The size comes from the open handle, so it is the size of what is read.
    let file = File::open(path).map_err(unreadable)?;
    let size = file.metadata().map_err(unreadable)?.len();
    admit(size)?;
    // Admitted, so `size` is at most a document's limit and fits in memory.
    let mut bytes = Vec::with_capacity(size as usize);
    (&file).take(size.min(HEADER_BYTES as u64)).read_to_end(&mut bytes).map_err(unreadable)?;
    check_head(&filename, &bytes)?;
    // One byte past `size` tells a file that grew from one that didn't.
    (&file).take(size + 1 - bytes.len() as u64).read_to_end(&mut bytes).map_err(unreadable)?;
    if bytes.len() as u64 > size {
        return Err(CommandError::new(
            "VALIDATION_ERROR",
            format!("{filename} changed while it was being read. Try again."),
        ));
    }
    Ok(AttachmentFile { bytes, filename })
}

/// Refuses a photo or document (`what`, singular) of `size` bytes over
/// `limit` as `ATTACHMENT_TOO_LARGE` (001 FR-007 and FR-010, amended
/// 2026-11-04).
pub fn require_size(what: &str, size: u64, limit: u64) -> Result<(), CommandError> {
    if size > limit {
        Err(CommandError::attachment_too_large(
            format!("A {what} can be at most {}.", size_label(limit)),
            limit,
        ))
    } else {
        Ok(())
    }
}

/// Refuses `adding` more bytes for `owner` when its photos and documents
/// together would then hold more than `MAX_ATTACHMENT_BYTES_PER_RECORD`, as
/// `ATTACHMENT_TOO_LARGE` (001 FR-007 and FR-010, amended 2026-11-04). What is
/// stored counts whatever its size, so a record already past the total takes
/// no more, while what it holds stays.
pub fn require_total_room(
    conn: &Connection,
    owner: RecordRef,
    adding: u64,
) -> Result<(), CommandError> {
    let column = owner.owner_column();
    // `length()` of a blob reads no content (SQLite's OPFLAG_LENGTHARG).
    let stored: i64 = conn
        .query_row(
            &format!(
                "SELECT (SELECT COALESCE(SUM(length(original_bytes)), 0) FROM photos
                         WHERE {column} = :id)
                      + (SELECT COALESCE(SUM(length(file_bytes)), 0) FROM document_attachments
                         WHERE {column} = :id)"
            ),
            named_params! { ":id": owner.id() },
            |row| row.get(0),
        )
        .map_err(CommandError::from_db)?;
    if (stored as u64).saturating_add(adding) > MAX_ATTACHMENT_BYTES_PER_RECORD {
        Err(CommandError::attachment_too_large(
            format!(
                "This {}'s photos and documents can total at most {}.",
                owner.noun(),
                size_label(MAX_ATTACHMENT_BYTES_PER_RECORD)
            ),
            MAX_ATTACHMENT_BYTES_PER_RECORD,
        ))
    } else {
        Ok(())
    }
}

/// Refuses one more row in `table` (`photos` or `document_attachments`) for
/// `owner` once it has `limit` of them, as `ATTACHMENT_LIMIT_REACHED` naming
/// `what` (001 FR-007 and FR-010, amended 2026-10-11). A photo is checked
/// before it is decoded, so a full record costs no thumbnail.
pub fn require_room(
    conn: &Connection,
    owner: RecordRef,
    table: &str,
    what: &str,
    limit: u32,
) -> Result<(), CommandError> {
    let count: u32 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE {} = :id", owner.owner_column()),
            named_params! { ":id": owner.id() },
            |row| row.get(0),
        )
        .map_err(CommandError::from_db)?;
    if count >= limit {
        Err(CommandError::attachment_limit_reached(owner, what, limit))
    } else {
        Ok(())
    }
}

/// The longest name kept, in characters (extension included).
const MAX_NAME_CHARS: usize = 120;

/// Reduces a file name to a name that is safe to write into a folder on any
/// supported OS (issue #68): the last component only, with the separators of
/// both OSes (a drive or UNC prefix goes with the components before it),
/// characters Windows forbids and control characters (NUL too) replaced,
/// trailing dots and spaces dropped, never `.` or `..`, and short enough to
/// leave room for a prefix. `fallback` is returned when nothing is left.
///
/// Used where a name is stored (photos and documents) and again where one
/// is written to disk, since a database made before the check may already
/// hold an unsafe name.
pub fn safe_basename(name: &str, fallback: &str) -> String {
    let last = name.rsplit(['/', '\\']).find(|part| !part.is_empty()).unwrap_or_default();
    let cleaned: String = last
        .chars()
        .map(|c| if c.is_control() || r#"<>:"|?*"#.contains(c) { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim_matches(|c: char| c.is_whitespace());
    let cleaned = cleaned.trim_end_matches(|c: char| c == '.' || c.is_whitespace());
    if cleaned.is_empty() {
        return fallback.to_owned();
    }
    if cleaned.chars().count() <= MAX_NAME_CHARS {
        return cleaned.to_owned();
    }
    // Too long: shorten the stem and keep a short extension.
    let (stem, extension) = match cleaned.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && extension.chars().count() <= 16 => {
            (stem, format!(".{extension}"))
        }
        _ => (cleaned, String::new()),
    };
    let room = MAX_NAME_CHARS - extension.chars().count();
    let stem: String = stem.chars().take(room).collect();
    format!("{}{extension}", stem.trim_end_matches(|c: char| c == '.' || c.is_whitespace()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_basename_keeps_a_plain_name() {
        assert_eq!(safe_basename("range day.png", "photo"), "range day.png");
    }

    #[test]
    fn safe_basename_keeps_only_the_last_component_of_either_os() {
        assert_eq!(safe_basename("../../etc/passwd.png", "photo"), "passwd.png");
        assert_eq!(safe_basename(r"..\..\Windows\win.png", "photo"), "win.png");
        assert_eq!(safe_basename("a/b\\c.png", "photo"), "c.png");
        assert_eq!(safe_basename(r"C:\Users\me\x.png", "photo"), "x.png");
        assert_eq!(safe_basename(r"\\server\share\x.png", "photo"), "x.png");
        assert_eq!(safe_basename("C:x.png", "photo"), "C_x.png");
        assert_eq!(safe_basename("dir/", "photo"), "dir");
    }

    #[test]
    fn safe_basename_falls_back_when_nothing_is_left() {
        for name in ["", ".", "..", "...", " . ", "/", "\\", "../..", "a/..", "a/."] {
            assert_eq!(safe_basename(name, "photo"), "photo", "{name:?}");
        }
        assert_eq!(safe_basename("", "document"), "document");
    }

    #[test]
    fn safe_basename_replaces_control_characters_and_forbidden_ones() {
        assert_eq!(safe_basename("a\0b\n.png", "photo"), "a_b_.png");
        assert_eq!(safe_basename("a<b>:c\"d|e?f*.png", "photo"), "a_b__c_d_e_f_.png");
    }

    #[test]
    fn safe_basename_drops_trailing_dots_and_spaces_and_caps_the_length() {
        assert_eq!(safe_basename("x.png. . ", "photo"), "x.png");
        let long = format!("{}.jpeg", "a".repeat(500));
        let safe = safe_basename(&long, "photo");
        assert_eq!(safe.chars().count(), 120);
        assert!(safe.ends_with(".jpeg"));
    }

    #[test]
    fn the_head_is_judged_before_the_rest_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.pdf");
        std::fs::write(&path, vec![b'x'; 3 * HEADER_BYTES]).unwrap();
        let mut seen = (0, 0);
        let err = read_attachment_file(
            &path,
            |size| {
                seen.0 = size;
                Ok(())
            },
            |_, head| {
                seen.1 = head.len();
                Err(CommandError::new("DOCUMENT_CONTENT_MISMATCH", "no"))
            },
        )
        .err()
        .expect("the head's refusal ends the read");
        assert_eq!(err.code, "DOCUMENT_CONTENT_MISMATCH");
        assert_eq!(seen, (3 * HEADER_BYTES as u64, HEADER_BYTES));
    }

    #[test]
    fn a_file_that_grows_past_its_admitted_size_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("growing.txt");
        std::fs::write(&path, b"receipt").unwrap();
        let err = read_attachment_file(
            &path,
            |_| {
                // Written to after its size is taken, before it is read.
                use std::io::Write;
                let mut file = File::options().append(true).open(&path).unwrap();
                file.write_all(b" and more").unwrap();
                Ok(())
            },
            |_, _| Ok(()),
        )
        .err()
        .expect("a file that grew is refused");
        assert_eq!(err.message, "growing.txt changed while it was being read. Try again.");
    }

    #[test]
    fn a_file_read_as_admitted_is_whole() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("whole.txt");
        let bytes: Vec<u8> = (0..5000u32).map(|n| n as u8).collect();
        std::fs::write(&path, &bytes).unwrap();
        let file = read_attachment_file(&path, |_| Ok(()), |_, _| Ok(())).ok().unwrap();
        assert_eq!(file.bytes, bytes);
        assert_eq!(file.filename, "whole.txt");
    }

    #[test]
    fn a_folder_is_rejected_by_name() {
        let dir = std::env::temp_dir();
        let err = read_attachment_file(&dir, |_| Ok(()), |_, _| Ok(()))
            .err()
            .expect("folders can't be attachments");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }
}
