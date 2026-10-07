//! The one document type allowlist and `classify`, the one content check
//! (research.md §2, FR-016, FR-017).
//!
//! `add_document`, `add_document_from_path`, `open_preview` and
//! `open_document` all go through [`classify`], so a row stored before this
//! feature, or a file edited since, is judged by the same rule as a new one.
//! The checks are signature-level: they look at magic bytes, a ZIP's central
//! directory and one small part, and a compound file's directory. Nothing is
//! decompressed beyond a few hundred bytes and nothing is parsed, so they
//! are safe to run in the main process on hostile input.

use std::io::{Cursor, Read};

use serde::Serialize;

use crate::commands::CommandError;
use crate::models::document_attachment::PreviewKind;

/// One kind of document HoploDex keeps. There is one entry per canonical
/// MIME type, so Word and Spreadsheet each appear twice (`.doc` and
/// `.docx`, `.xls` and `.xlsx`).
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentType {
    pub label: &'static str,
    /// Lowercase, without the dot.
    pub extensions: &'static [&'static str],
    /// The type recorded for every document of this kind.
    pub mime_type: &'static str,
    /// The extension a copy of the document is written under.
    #[serde(skip)]
    pub canonical_extension: &'static str,
    pub preview_kind: Option<PreviewKind>,
    #[serde(skip)]
    signature: Signature,
}

/// What the content check looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Signature {
    Pdf,
    Tiff,
    Text,
    Rtf,
    /// A compound file holding one of these streams.
    Ole(&'static [&'static str]),
    /// A ZIP whose `[Content_Types].xml` names this main part.
    Ooxml(&'static str),
    /// A ZIP whose first entry `mimetype` holds this.
    Odf(&'static str),
}

const WORD_DOC_MAIN: &str = "wordprocessingml.document.main+xml";
const SHEET_MAIN: &str = "spreadsheetml.sheet.main+xml";

/// The list, in the table order of research.md §2.
static TYPES: [DocumentType; 11] = [
    DocumentType {
        label: "PDF",
        extensions: &["pdf"],
        mime_type: "application/pdf",
        canonical_extension: "pdf",
        preview_kind: Some(PreviewKind::Pdf),
        signature: Signature::Pdf,
    },
    DocumentType {
        label: "TIFF",
        extensions: &["tif", "tiff"],
        mime_type: "image/tiff",
        canonical_extension: "tif",
        preview_kind: Some(PreviewKind::Tiff),
        signature: Signature::Tiff,
    },
    DocumentType {
        label: "Plain text",
        extensions: &["txt"],
        mime_type: "text/plain",
        canonical_extension: "txt",
        preview_kind: Some(PreviewKind::Text),
        signature: Signature::Text,
    },
    DocumentType {
        label: "CSV",
        extensions: &["csv"],
        mime_type: "text/csv",
        canonical_extension: "csv",
        preview_kind: Some(PreviewKind::Text),
        signature: Signature::Text,
    },
    DocumentType {
        label: "RTF",
        extensions: &["rtf"],
        mime_type: "application/rtf",
        canonical_extension: "rtf",
        preview_kind: None,
        signature: Signature::Rtf,
    },
    DocumentType {
        label: "Word",
        extensions: &["doc"],
        mime_type: "application/msword",
        canonical_extension: "doc",
        preview_kind: None,
        signature: Signature::Ole(&["WordDocument"]),
    },
    DocumentType {
        label: "Word",
        extensions: &["docx"],
        mime_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        canonical_extension: "docx",
        preview_kind: None,
        signature: Signature::Ooxml(WORD_DOC_MAIN),
    },
    DocumentType {
        label: "Spreadsheet",
        extensions: &["xls"],
        mime_type: "application/vnd.ms-excel",
        canonical_extension: "xls",
        preview_kind: None,
        signature: Signature::Ole(&["Workbook", "Book"]),
    },
    DocumentType {
        label: "Spreadsheet",
        extensions: &["xlsx"],
        mime_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        canonical_extension: "xlsx",
        preview_kind: None,
        signature: Signature::Ooxml(SHEET_MAIN),
    },
    DocumentType {
        label: "OpenDocument text",
        extensions: &["odt"],
        mime_type: "application/vnd.oasis.opendocument.text",
        canonical_extension: "odt",
        preview_kind: None,
        signature: Signature::Odf("application/vnd.oasis.opendocument.text"),
    },
    DocumentType {
        label: "OpenDocument spreadsheet",
        extensions: &["ods"],
        mime_type: "application/vnd.oasis.opendocument.spreadsheet",
        canonical_extension: "ods",
        preview_kind: None,
        signature: Signature::Odf("application/vnd.oasis.opendocument.spreadsheet"),
    },
];

/// Every document type, in table order.
pub fn all() -> &'static [DocumentType] {
    &TYPES
}

/// The type whose canonical MIME type is `mime_type`: what a stored row
/// says it is. `None` for a row stored before this feature with another type
/// (a JPEG, say).
pub fn from_recorded(mime_type: &str) -> Option<&'static DocumentType> {
    TYPES.iter().find(|t| t.mime_type == mime_type)
}

/// Why a file isn't a document HoploDex keeps (FR-016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The extension isn't on the list.
    TypeNotAllowed { name: String },
    /// `.jpg`, `.jpeg` or `.png`: photos go under Photos.
    Photo { name: String },
    /// The extension is on the list but the content isn't what it says, or
    /// holds macros or markup.
    ContentMismatch { name: String, label: &'static str },
}

impl Refusal {
    /// The stable error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::TypeNotAllowed { .. } | Self::Photo { .. } => "DOCUMENT_TYPE_NOT_ALLOWED",
            Self::ContentMismatch { .. } => "DOCUMENT_CONTENT_MISMATCH",
        }
    }

    /// The message shown to the user.
    pub fn message(&self) -> String {
        match self {
            Self::TypeNotAllowed { name } => {
                format!("{name} can't be kept as a document. Documents can be {}.", type_names())
            }
            Self::Photo { name } => format!("{name} is a photo. Add it under Photos instead."),
            Self::ContentMismatch { name, label } => format!(
                "{name} isn't a real {label} document, or it holds macros, scripts or web page \
                 code, so it can't be kept as a document."
            ),
        }
    }
}

impl From<Refusal> for CommandError {
    fn from(refusal: Refusal) -> Self {
        CommandError::new(refusal.code(), refusal.message())
    }
}

/// The document types' labels, each once, as a sentence fragment.
fn type_names() -> String {
    let mut labels: Vec<&str> = Vec::new();
    for t in &TYPES {
        if !labels.contains(&t.label) {
            labels.push(t.label);
        }
    }
    let last = labels.pop().unwrap_or_default();
    format!("{} or {last}", labels.join(", "))
}

/// Judges a file by its name and content (research.md §2). Returns the type
/// to record for it, or why it isn't one.
pub fn classify(filename: &str, bytes: &[u8]) -> Result<&'static DocumentType, Refusal> {
    let name = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let extension = name.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase());
    let found =
        extension.as_deref().and_then(|ext| TYPES.iter().find(|t| t.extensions.contains(&ext)));
    let Some(document_type) = found else {
        return Err(match extension.as_deref() {
            Some("jpg" | "jpeg" | "png") => Refusal::Photo { name: name.to_owned() },
            _ => Refusal::TypeNotAllowed { name: name.to_owned() },
        });
    };
    if has_signature(document_type.signature, bytes) {
        Ok(document_type)
    } else {
        Err(Refusal::ContentMismatch { name: name.to_owned(), label: document_type.label })
    }
}

fn has_signature(signature: Signature, bytes: &[u8]) -> bool {
    match signature {
        Signature::Pdf => bytes[..bytes.len().min(1024)].windows(5).any(|w| w == b"%PDF-"),
        Signature::Tiff => {
            matches!(bytes.get(..4), Some(b"II*\0" | b"MM\0*" | b"II+\0" | b"MM\0+"))
        }
        Signature::Text => !starts_with_markup(bytes),
        Signature::Rtf => bytes.starts_with(b"{\\rtf"),
        Signature::Ole(streams) => is_plain_ole(bytes, streams),
        Signature::Ooxml(main_part) => is_plain_ooxml(bytes, main_part),
        Signature::Odf(mime) => is_plain_odf(bytes, mime),
    }
}

/// Whether the first 512 bytes, after an optional byte order mark and
/// whitespace, begin a web page, an image or a script (`<` and one of a few
/// words, in any case): enough to stop the OS from sniffing the file as one.
fn starts_with_markup(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(512)];
    // A UTF-16 byte order mark: keep every other byte (the ASCII range).
    let utf16: Vec<u8>;
    let head = match head {
        [0xFF, 0xFE, rest @ ..] => {
            utf16 = rest.chunks(2).map(|c| c[0]).collect();
            &utf16[..]
        }
        [0xFE, 0xFF, rest @ ..] => {
            utf16 = rest.chunks(2).filter_map(|c| c.get(1).copied()).collect();
            &utf16[..]
        }
        [0xEF, 0xBB, 0xBF, rest @ ..] => rest,
        _ => head,
    };
    let trimmed = match head.iter().position(|b| !b.is_ascii_whitespace()) {
        Some(at) => &head[at..],
        None => return false,
    };
    let Some(after) = trimmed.strip_prefix(b"<") else {
        return false;
    };
    ["!doctype", "html", "svg", "?xml", "script", "head"].iter().any(|tag| {
        after.len() >= tag.len() && after[..tag.len()].eq_ignore_ascii_case(tag.as_bytes())
    })
}

/// A compound file with one of `streams` at its root and no macro storage.
/// Only the directory is read.
fn is_plain_ole(bytes: &[u8], streams: &[&str]) -> bool {
    let Ok(file) = cfb::CompoundFile::open(Cursor::new(bytes)) else {
        return false;
    };
    let mut has_stream = false;
    for entry in file.walk() {
        let name = entry.name();
        if ["macros", "_vba_project_cur", "_vba_project", "vba"]
            .iter()
            .any(|macro_name| name.eq_ignore_ascii_case(macro_name))
        {
            return false;
        }
        if entry.is_stream() && entry.path().parent().is_some_and(|p| p.as_os_str() == "/") {
            has_stream |= streams.contains(&name);
        }
    }
    has_stream
}

/// The part of an OOXML package's `[Content_Types].xml` that is read, at
/// most: it is a few hundred bytes in practice.
const CONTENT_TYPES_LIMIT: u64 = 256 * 1024;

/// A ZIP whose `[Content_Types].xml` names `main_part` as the main part, with
/// no `vbaProject.bin` and no macro-enabled type. The directory and one small
/// part are read.
fn is_plain_ooxml(bytes: &[u8], main_part: &str) -> bool {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return false;
    };
    if archive.file_names().any(|n| n.to_ascii_lowercase().ends_with("vbaproject.bin")) {
        return false;
    }
    let mut types = Vec::new();
    let Ok(entry) = archive.by_name("[Content_Types].xml") else {
        return false;
    };
    if entry.take(CONTENT_TYPES_LIMIT).read_to_end(&mut types).is_err() {
        return false;
    }
    let types = String::from_utf8_lossy(&types);
    types.contains(main_part) && !types.contains("macroEnabled")
}

/// A ZIP whose first entry is `mimetype` holding `mime`, with no `Basic/` or
/// `Scripts/` folder. The directory and the first entry's few bytes are read.
fn is_plain_odf(bytes: &[u8], mime: &str) -> bool {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return false;
    };
    if archive.file_names().any(|n| n.starts_with("Basic/") || n.starts_with("Scripts/")) {
        return false;
    }
    let Ok(first) = archive.by_index(0) else {
        return false;
    };
    if first.name() != "mimetype" {
        return false;
    }
    let mut found = Vec::new();
    first.take(mime.len() as u64 + 1).read_to_end(&mut found).is_ok() && found == mime.as_bytes()
}
