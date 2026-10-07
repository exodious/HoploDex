//! The one document type allowlist and `classify`, the one content check
//! (research.md §2, FR-016, FR-017).
//!
//! `add_document`, `add_document_from_path`, `open_preview` and
//! `open_document` all go through [`classify`], so a row stored before this
//! feature, or a file edited since, is judged by the same rule as a new one.
//! The checks are signature-level: they look at magic bytes, a ZIP's central
//! directory, its content types and relationship parts, a compound file's
//! directory, and an RTF file's control words. An OOXML package's
//! relationship parts are the only thing decompressed (a few megabytes at
//! most, all together) and the only thing parsed, by `quick-xml`, which
//! expands no entities beyond XML's five and refuses a DTD here, so the
//! checks are safe to run in the main process on hostile input.

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
    /// holds macros, markup, embedded objects or links to outside content.
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
                "{name} isn't a real {label} document, or it holds macros, scripts, web page \
                 code, embedded objects or links that load outside content, so it can't be kept \
                 as a document."
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
        Signature::Rtf => bytes.starts_with(b"{\\rtf") && !rtf_loads_outside_content(bytes),
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

/// The most of an OOXML package's relationship parts that is read, all
/// together. A document's are a few kilobytes; a spreadsheet with thousands
/// of hyperlinks, a megabyte or two.
const RELATIONSHIPS_LIMIT: u64 = 8 * 1024 * 1024;

/// A ZIP whose `[Content_Types].xml` names `main_part` as the main part, with
/// no `vbaProject.bin`, no macro-enabled type, and relationship parts that
/// keep everything inside the package (research.md §2). The directory, the
/// content types and the relationship parts are read.
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
    if !types.contains(main_part) || types.contains("macroEnabled") {
        return false;
    }
    // Every entry that is, or may be, a relationship part, by index, so a
    // second entry of the same name is read too.
    let mut budget = RELATIONSHIPS_LIMIT;
    for index in 0..archive.len() {
        let Some(name) = archive.name_for_index(index) else {
            return false;
        };
        let name = name.to_ascii_lowercase();
        let is_relationships =
            name.ends_with(".rels") || name.split(['/', '\\']).any(|segment| segment == "_rels");
        if !is_relationships {
            continue;
        }
        let Ok(entry) = archive.by_index(index) else {
            return false;
        };
        let mut xml = Vec::new();
        if entry.take(budget + 1).read_to_end(&mut xml).is_err() || xml.len() as u64 > budget {
            return false;
        }
        budget -= xml.len() as u64;
        if !relationships_stay_inside(&xml) {
            return false;
        }
    }
    true
}

/// The relationship types that bring in active content: OLE objects (embedded
/// or linked), ActiveX controls, macros and macro sheets, and a workbook's
/// links to other workbooks or DDE servers. Compared by the type's last
/// segment, in any case, so the Transitional and Strict URIs both match.
const ACTIVE_RELATIONSHIPS: [&str; 8] = [
    "oleobject",
    "control",
    "activexcontrolbinary",
    "vbaproject",
    "wordvbadata",
    "externallink",
    "xlmacrosheet",
    "xlintlmacrosheet",
];

/// Whether one relationship part is UTF-8 XML whose relationships keep
/// everything inside the package: none of [`ACTIVE_RELATIONSHIPS`], and
/// nothing outside it but hyperlinks, which only a click follows. Outside
/// means `TargetMode="External"` or an absolute target (a URI scheme, a
/// drive letter or a UNC path); a linked template, image, frame or
/// subdocument is loaded when the document opens. Values are read as XML
/// reads them, with character references resolved, so `&#69;xternal` is
/// `External`. Anything the parser or this check can't read is refused.
fn relationships_stay_inside(xml: &[u8]) -> bool {
    use quick_xml::XmlVersion;
    use quick_xml::events::Event;

    let xml = xml.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(xml);
    // UTF-16 and UTF-32 have NULs; another encoding is refused below.
    if xml.contains(&0) {
        return false;
    }
    let Ok(text) = std::str::from_utf8(xml) else {
        return false;
    };
    let mut reader = quick_xml::Reader::from_str(text);
    let decoder = reader.decoder();
    let mut found_root = false;
    loop {
        let element = match reader.read_event() {
            Ok(Event::Eof) => return found_root,
            Ok(Event::Decl(decl)) => {
                match decl.encoding() {
                    None => {}
                    Some(Ok(encoding)) if encoding.eq_ignore_ascii_case(b"utf-8") => {}
                    Some(_) => return false,
                }
                continue;
            }
            // A package may not declare a DTD (ECMA-376 Part 2, M1.17), and
            // one could define entities.
            Ok(Event::DocType(_)) | Err(_) => return false,
            Ok(Event::Start(element) | Event::Empty(element)) => element,
            Ok(_) => continue,
        };
        match element.local_name().as_ref() {
            b"Relationships" => found_root = true,
            b"Relationship" => {
                let (mut kind, mut target, mut mode) = (None, None, None);
                for attribute in element.attributes() {
                    let Ok(attribute) = attribute else {
                        return false;
                    };
                    let Ok(value) =
                        attribute.decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
                    else {
                        return false;
                    };
                    match attribute.key.local_name().as_ref() {
                        b"Type" => kind = Some(value.trim().to_ascii_lowercase()),
                        b"Target" => target = Some(value.trim().to_owned()),
                        b"TargetMode" => mode = Some(value.trim().to_ascii_lowercase()),
                        _ => {}
                    }
                }
                let kind = kind.unwrap_or_default();
                let last = kind.rsplit('/').next().unwrap_or_default();
                if ACTIVE_RELATIONSHIPS.contains(&last) {
                    return false;
                }
                let target = target.unwrap_or_default();
                let absolute = target.starts_with("\\\\")
                    || target.starts_with("//")
                    || target.split_once(':').is_some_and(|(head, _)| !head.contains('/'));
                let outside = mode.as_deref() == Some("external") || absolute;
                if outside && last != "hyperlink" {
                    return false;
                }
            }
            _ => {}
        }
    }
}

/// Whether RTF holds an object, embedded or linked (any `\obj…` control
/// word: `\object`, `\objdata`, `\objemb`, `\objlink`, `\objautlink`, …),
/// or names a template to load (`\template`). Line breaks are dropped first,
/// since an RTF reader skips them, and case is ignored. This is a search,
/// not a tokenizer, so `\bin` data can't hide a word from it; the price is
/// that text that writes out a backslash and `obj` (`\\obj` in RTF) is
/// refused too, which paperwork doesn't do.
fn rtf_loads_outside_content(bytes: &[u8]) -> bool {
    let squeezed: Vec<u8> =
        bytes.iter().filter(|b| !matches!(b, b'\r' | b'\n')).map(u8::to_ascii_lowercase).collect();
    [&b"\\obj"[..], b"\\template"]
        .iter()
        .any(|word| squeezed.windows(word.len()).any(|window| window == *word))
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
