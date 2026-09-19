//! Reading files the user drops onto the window. The webview never sees the
//! file itself — the desktop shell hands the frontend the dropped paths
//! (WebKitGTK exposes no `File` for a dragged-in file) — so the bytes, name,
//! and type are read here instead, and the result is stored exactly as if it
//! had come from the file picker.

use std::path::Path;

use crate::commands::CommandError;

pub struct AttachmentFile {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub mime_type: &'static str,
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
    let mime_type = mime_type_for(&filename);
    Ok(AttachmentFile { bytes, filename, mime_type })
}

/// The type to record for a file, going by its extension. Unknown
/// extensions are stored as generic binary data, which is all the app needs
/// them for: opening a document hands it to the OS by file name.
pub fn mime_type_for(filename: &str) -> &'static str {
    let extension = filename.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase());
    match extension.as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("heic") => "image/heic",
        Some("tif" | "tiff") => "image/tiff",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("csv") => "text/csv",
        Some("rtf") => "application/rtf",
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xls") => "application/vnd.ms-excel",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("odt") => "application/vnd.oasis.opendocument.text",
        Some("ods") => "application/vnd.oasis.opendocument.spreadsheet",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_type_ignores_extension_case() {
        assert_eq!(mime_type_for("Receipt.PDF"), "application/pdf");
        assert_eq!(mime_type_for("range day.JPeG"), "image/jpeg");
    }

    #[test]
    fn mime_type_falls_back_to_generic_binary() {
        assert_eq!(mime_type_for("appraisal"), "application/octet-stream");
        assert_eq!(mime_type_for("notes.unknownext"), "application/octet-stream");
    }

    #[test]
    fn a_folder_is_rejected_by_name() {
        let dir = std::env::temp_dir();
        let err = read_attachment_file(&dir).err().expect("folders can't be attachments");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }
}
