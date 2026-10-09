//! Reading files the user drops onto the window. The webview never sees the
//! file itself — the desktop shell hands the frontend the dropped paths
//! (WebKitGTK exposes no `File` for a dragged-in file) — so the bytes and name
//! are read here instead, and the result is stored exactly as if it
//! had come from the file picker.

use std::path::Path;

use crate::commands::CommandError;

pub struct AttachmentFile {
    pub bytes: Vec<u8>,
    pub filename: String,
}

pub fn read_attachment_file(path: &Path) -> Result<AttachmentFile, CommandError> {
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
    let bytes = std::fs::read(path).map_err(|err| {
        log::error!("couldn't read {}: {err}", path.display());
        CommandError::not_found(format!("{filename} couldn't be read."))
    })?;
    Ok(AttachmentFile { bytes, filename })
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

/// The type to record for a photo, going by its extension. Anything else is
/// generic binary data, which the photo rules refuse. Documents don't use
/// this: their type comes from `services::document_types::classify`, which
/// looks at the content (research.md §2).
pub fn mime_type_for(filename: &str) -> &'static str {
    let extension = filename.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase());
    match extension.as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("heic") => "image/heic",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_type_ignores_extension_case() {
        assert_eq!(mime_type_for("Front.PNG"), "image/png");
        assert_eq!(mime_type_for("range day.JPeG"), "image/jpeg");
    }

    #[test]
    fn mime_type_falls_back_to_generic_binary() {
        assert_eq!(mime_type_for("appraisal"), "application/octet-stream");
        assert_eq!(mime_type_for("notes.unknownext"), "application/octet-stream");
        assert_eq!(mime_type_for("receipt.pdf"), "application/octet-stream");
    }

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
    fn a_folder_is_rejected_by_name() {
        let dir = std::env::temp_dir();
        let err = read_attachment_file(&dir).err().expect("folders can't be attachments");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }
}
