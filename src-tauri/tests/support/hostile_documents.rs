//! Generators for hostile and malformed documents (feature 007, T004,
//! research.md §2, §11 and §23).
//!
//! Each returns bytes (and a file name) for a test to attach, classify or
//! preview. Nothing large is checked in: the large TIFF is made to the size
//! the caller gives, and everything else is a few hundred bytes to a few
//! kilobytes, built by hand here with no dependency beyond `std`. They are
//! signature-level documents: each has what `services::document_types`
//! looks at (magic bytes, a ZIP's entries, a compound file's storages) and
//! little more. None can do anything if opened; the "scripts" are single
//! harmless lines.
//!
//! [`refused_documents`] lists every one that `classify` must refuse, with
//! the code it must refuse it with.
#![allow(dead_code)]

/// What a document is refused with (data-model.md, "Validation").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `DOCUMENT_TYPE_NOT_ALLOWED`: the extension isn't on the list.
    TypeNotAllowed,
    /// `DOCUMENT_TYPE_NOT_ALLOWED` with the photo message: `.jpg`, `.jpeg`
    /// and `.png` belong under Photos.
    Photo,
    /// `DOCUMENT_CONTENT_MISMATCH`: the extension is on the list, the
    /// content isn't what it says, or holds macros.
    ContentMismatch,
}

impl Refusal {
    /// The stable error code.
    pub fn code(self) -> &'static str {
        match self {
            Self::TypeNotAllowed | Self::Photo => "DOCUMENT_TYPE_NOT_ALLOWED",
            Self::ContentMismatch => "DOCUMENT_CONTENT_MISMATCH",
        }
    }
}

/// A generated document: the file name to attach it under and its bytes.
#[derive(Clone, Debug)]
pub struct Generated {
    pub name: String,
    pub bytes: Vec<u8>,
}

impl Generated {
    fn new(name: &str, bytes: Vec<u8>) -> Self {
        Self { name: name.to_owned(), bytes }
    }
}

/// A generated document and how it must be refused.
#[derive(Clone, Debug)]
pub struct Hostile {
    pub document: Generated,
    pub refusal: Refusal,
}

/// Every non-TIFF hostile document of T004, each with the refusal that
/// `classify` owes it.
pub fn refused_documents() -> Vec<Hostile> {
    use Refusal::*;
    let mut all = vec![
        (exe(), TypeNotAllowed),
        (bat(), TypeNotAllowed),
        (ps1(), TypeNotAllowed),
        (sh(), TypeNotAllowed),
        (lnk(), TypeNotAllowed),
        (html(), TypeNotAllowed),
        (svg(), TypeNotAllowed),
        (gif(), TypeNotAllowed),
        (heic(), TypeNotAllowed),
        (msi(), TypeNotAllowed),
        (dmg(), TypeNotAllowed),
        (pkg(), TypeNotAllowed),
        (deb(), TypeNotAllowed),
        (docm(), TypeNotAllowed),
        (jpg(), Photo),
        (jpeg(), Photo),
        (png(), Photo),
        (docx_macro_enabled_content_type(), ContentMismatch),
        (docx_with_vba_project(), ContentMismatch),
        (doc_with_macros_storage(), ContentMismatch),
        (xls_with_vba_project(), ContentMismatch),
        (odt_with_basic(), ContentMismatch),
        (html_named_pdf(), ContentMismatch),
        (zip_named_docx(), ContentMismatch),
        (text_named_tiff(), ContentMismatch),
    ];
    for tag in MARKUP_STARTS {
        all.push((markup_text("note.txt", tag), ContentMismatch));
        all.push((markup_text("rounds.csv", tag), ContentMismatch));
    }
    all.into_iter().map(|(document, refusal)| Hostile { document, refusal }).collect()
}

// ---------------------------------------------------------------- bytes

fn le16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

fn le32(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

// ---------------------------------------------------- executables, scripts

/// A PE file: `MZ`, `e_lfanew` at 0x3C, `PE\0\0` and a machine type.
pub fn exe() -> Generated {
    let mut b = vec![0u8; 0x200];
    b[..2].copy_from_slice(b"MZ");
    b[0x3C..0x40].copy_from_slice(&le32(0x80));
    b[0x80..0x84].copy_from_slice(b"PE\0\0");
    b[0x84..0x86].copy_from_slice(&le16(0x8664));
    Generated::new("setup.exe", b)
}

pub fn bat() -> Generated {
    Generated::new("run.bat", b"@echo off\r\necho hello\r\n".to_vec())
}

pub fn ps1() -> Generated {
    Generated::new("run.ps1", b"Write-Host 'hello'\r\n".to_vec())
}

pub fn sh() -> Generated {
    Generated::new("run.sh", b"#!/bin/sh\necho hello\n".to_vec())
}

/// A Windows shortcut: header size 0x4C and the shell link CLSID.
pub fn lnk() -> Generated {
    let mut b = vec![0u8; 0x4C];
    b[..4].copy_from_slice(&le32(0x4C));
    b[4..20].copy_from_slice(&[
        0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x46,
    ]);
    Generated::new("shortcut.lnk", b)
}

pub fn html() -> Generated {
    Generated::new(
        "page.html",
        b"<!DOCTYPE html><html><body><script>/* never run */</script></body></html>".to_vec(),
    )
}

pub fn svg() -> Generated {
    Generated::new(
        "drawing.svg",
        b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"><script>/* never run */</script></svg>"
            .to_vec(),
    )
}

pub fn gif() -> Generated {
    let mut b = b"GIF89a".to_vec();
    b.extend_from_slice(&[1, 0, 1, 0, 0, 0, 0, 0x3B]);
    Generated::new("image.gif", b)
}

/// An `ftyp` box with the `heic` brand.
pub fn heic() -> Generated {
    let mut b = 24u32.to_be_bytes().to_vec(); // big-endian box size
    b.extend_from_slice(b"ftypheic");
    b.extend_from_slice(&[0, 0, 0, 0]);
    b.extend_from_slice(b"mif1heic");
    Generated::new("photo.heic", b)
}

pub fn jpg() -> Generated {
    Generated::new("scan.jpg", jpeg_bytes())
}

pub fn jpeg() -> Generated {
    Generated::new("scan.jpeg", jpeg_bytes())
}

fn jpeg_bytes() -> Vec<u8> {
    let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
    b.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
    b.extend_from_slice(&[0xFF, 0xD9]);
    b
}

pub fn png() -> Generated {
    let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    b.extend_from_slice(&[0, 0, 0, 0]);
    b.extend_from_slice(b"IEND");
    b.extend_from_slice(&[0xAE, 0x42, 0x60, 0x82]);
    Generated::new("scan.png", b)
}

// ---------------------------------------------------------------- installers

/// An OLE compound file with an installer-style stream.
pub fn msi() -> Generated {
    let b = cfb(vec![stream("\u{5}SummaryInformation", vec![0; 64])]);
    Generated::new("setup.msi", b)
}

/// A disk image: some data, then the 512-byte UDIF `koly` trailer.
pub fn dmg() -> Generated {
    let mut b = vec![0u8; 1024];
    let mut koly = vec![0u8; 512];
    koly[..4].copy_from_slice(b"koly");
    koly[4..8].copy_from_slice(&4u32.to_be_bytes()); // version
    koly[8..12].copy_from_slice(&512u32.to_be_bytes()); // header size
    b.extend_from_slice(&koly);
    Generated::new("app.dmg", b)
}

/// A flat package: the XAR header, `xar!`, header size 28, version 1.
pub fn pkg() -> Generated {
    let mut b = b"xar!".to_vec();
    b.extend_from_slice(&28u16.to_be_bytes());
    b.extend_from_slice(&1u16.to_be_bytes());
    b.extend_from_slice(&0u64.to_be_bytes()); // compressed TOC length
    b.extend_from_slice(&0u64.to_be_bytes()); // uncompressed TOC length
    b.extend_from_slice(&1u32.to_be_bytes()); // checksum algorithm: SHA-1
    Generated::new("app.pkg", b)
}

/// An `ar` archive with the `debian-binary` member.
pub fn deb() -> Generated {
    let mut b = b"!<arch>\n".to_vec();
    b.extend_from_slice(
        format!("{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n", "debian-binary", 0, 0, 0, "100644", 4)
            .as_bytes(),
    );
    b.extend_from_slice(b"2.0\n");
    Generated::new("app.deb", b)
}

// ------------------------------------------------------------------ macros

const WORD_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const WORD_MACRO_MAIN: &str = "application/vnd.ms-word.document.macroEnabled.main+xml";

fn word_package(main_type: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let types = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
         <Override PartName=\"/word/document.xml\" ContentType=\"{main_type}\"/></Types>"
    );
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
        <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
        </Relationships>";
    let document = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
        <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>\
        <w:p><w:r><w:t>HoploDex test</w:t></w:r></w:p></w:body></w:document>";
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("word/document.xml", document.as_bytes()),
    ];
    entries.extend_from_slice(extra);
    zip_stored(&entries)
}

/// A macro-enabled Word document, under its real extension.
pub fn docm() -> Generated {
    Generated::new("macros.docm", word_package(WORD_MACRO_MAIN, &[("word/vbaProject.bin", b"VBA")]))
}

/// A `.docm`'s package renamed `.docx`: the main part's type says macros.
pub fn docx_macro_enabled_content_type() -> Generated {
    Generated::new("renamed.docx", word_package(WORD_MACRO_MAIN, &[]))
}

/// A DOCX that declares itself an ordinary document but holds
/// `word/vbaProject.bin`.
pub fn docx_with_vba_project() -> Generated {
    Generated::new(
        "hidden-macros.docx",
        word_package(WORD_MAIN, &[("word/vbaProject.bin", b"VBA")]),
    )
}

/// A DOC (a `WordDocument` stream) with a `Macros` storage.
pub fn doc_with_macros_storage() -> Generated {
    let b = cfb(vec![
        stream("WordDocument", word97_fib()),
        storage("Macros", vec![stream("VBA", vec![0; 32])]),
    ]);
    Generated::new("macros.doc", b)
}

fn word97_fib() -> Vec<u8> {
    let mut b = vec![0u8; 64];
    b[..2].copy_from_slice(&le16(0xA5EC));
    b[2..4].copy_from_slice(&le16(0x00C1));
    b
}

/// An XLS (a `Workbook` stream) with a `_VBA_PROJECT_CUR` storage.
pub fn xls_with_vba_project() -> Generated {
    let b = cfb(vec![
        stream("Workbook", vec![0x09, 0x08, 0x04, 0x00, 0, 0, 0x10, 0]),
        storage("_VBA_PROJECT_CUR", vec![stream("PROJECT", vec![0; 32])]),
    ]);
    Generated::new("macros.xls", b)
}

/// An ODT with a `Basic/` script library.
pub fn odt_with_basic() -> Generated {
    let entries: Vec<(&str, &[u8])> = vec![
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        ("content.xml", b"<office:document-content/>"),
        ("Basic/script-lc.xml", b"<library:libraries/>"),
        ("Basic/Standard/Module1.xml", b"<script:module/>"),
    ];
    Generated::new("macros.odt", zip_stored(&entries))
}

// -------------------------------------------------------- content mismatch

/// HTML under a `.pdf` name.
pub fn html_named_pdf() -> Generated {
    Generated::new(
        "receipt.pdf",
        b"<!DOCTYPE html><html><body><script>/* never run */</script></body></html>".to_vec(),
    )
}

/// A ZIP of the wrong sort (no `[Content_Types].xml`) named `.docx`.
pub fn zip_named_docx() -> Generated {
    Generated::new("notes.docx", zip_stored(&[("readme.txt", b"hello")]))
}

/// Plain text named `.tif`.
pub fn text_named_tiff() -> Generated {
    Generated::new("scan.tif", b"not a TIFF at all".to_vec())
}

/// What a `.txt` or `.csv` must not begin with after an optional BOM and
/// whitespace: `<` followed by one of these, in any case (research.md §2).
pub const MARKUP_STARTS: [&str; 6] = ["!DOCTYPE html", "html", "svg", "?xml", "script", "head"];

/// A file named `name` that starts with a BOM, whitespace and then `<tag`
/// (the tag's case alternates by its length, so both cases are covered
/// across [`MARKUP_STARTS`]).
pub fn markup_text(name: &str, tag: &str) -> Generated {
    let tag = if tag.len() % 2 == 0 { tag.to_uppercase() } else { tag.to_lowercase() };
    let mut b = vec![0xEF, 0xBB, 0xBF];
    b.extend_from_slice(b" \r\n\t  <");
    b.extend_from_slice(tag.as_bytes());
    b.extend_from_slice(b">\n<p>not text</p>\n");
    Generated::new(name, b)
}

// -------------------------------------------------------------------- TIFF

/// One IFD entry: tag, type (3 SHORT, 4 LONG, 2 ASCII), count and the
/// 4-byte value or offset field, as it is written.
type Entry = (u16, u16, u32, u32);

/// A classic little-endian TIFF being laid out: the 8-byte header, then
/// whatever the caller appends.
fn tiff_header(first_ifd: u32) -> Vec<u8> {
    let mut b = b"II*\0".to_vec();
    b.extend_from_slice(&le32(first_ifd));
    b
}

fn ifd(mut entries: Vec<Entry>, next: u32) -> Vec<u8> {
    entries.sort_by_key(|e| e.0);
    let mut b = le16(entries.len() as u16).to_vec();
    for (tag, typ, count, value) in entries {
        b.extend_from_slice(&le16(tag));
        b.extend_from_slice(&le16(typ));
        b.extend_from_slice(&le32(count));
        b.extend_from_slice(&le32(value));
    }
    b.extend_from_slice(&le32(next));
    b
}

/// The entries of an uncompressed 8-bit grayscale page of one strip.
/// `strip_offset` is where its data starts; `strip_bytes` the byte count it
/// claims.
fn gray_entries(width: u32, height: u32, strip_offset: u32, strip_bytes: u32) -> Vec<Entry> {
    vec![
        (256, 4, 1, width),
        (257, 4, 1, height),
        (258, 3, 1, 8),
        (259, 3, 1, 1),
        (262, 3, 1, 1),
        (273, 4, 1, strip_offset),
        (277, 3, 1, 1),
        (278, 4, 1, height),
        (279, 4, 1, strip_bytes),
    ]
}

const IFD_LEN: u32 = 2 + 9 * 12 + 4;

/// A small, valid, uncompressed grayscale TIFF of `pages` pages of 48 × 48
/// pixels.
pub fn tiff(pages: usize) -> Generated {
    let (w, h) = (48u32, 48u32);
    let mut b = tiff_header(8);
    for page in 0..pages {
        let ifd_at = b.len() as u32;
        let data_at = ifd_at + IFD_LEN;
        let last = page + 1 == pages;
        let next = if last { 0 } else { data_at + w * h };
        b.extend(ifd(gray_entries(w, h, data_at, w * h), next));
        for y in 0..h {
            for x in 0..w {
                b.push((x * 5 + y * 3 + page as u32 * 40) as u8);
            }
        }
    }
    Generated::new("scan.tif", b)
}

/// A 3-page TIFF cut off inside its second page's pixels.
pub fn tiff_truncated() -> Generated {
    let mut g = tiff(3);
    let keep = 8 + (IFD_LEN + 48 * 48) as usize + IFD_LEN as usize + 700;
    g.bytes.truncate(keep);
    g.name = "truncated.tif".into();
    g
}

/// A 3-page TIFF with bits flipped through its IFDs and pixels, the header
/// (so the content check) left alone.
pub fn tiff_bit_flipped() -> Generated {
    let mut g = tiff(3);
    for pos in (8..g.bytes.len()).step_by(13) {
        g.bytes[pos] ^= 1 << (pos % 8);
    }
    g.name = "bit-flipped.tif".into();
    g
}

/// A TIFF that claims an endless number of pages: its one IFD's next-IFD
/// offset points back at itself, so a reader that follows the chain counts
/// 2^31 and more (a page-number tag can't say it: it is two 16-bit values).
pub fn tiff_endless_pages() -> Generated {
    let mut b = tiff_header(8);
    let data_at = 8 + IFD_LEN;
    b.extend(ifd(gray_entries(2, 2, data_at, 4), 8));
    b.extend_from_slice(&[0, 64, 128, 255]);
    Generated::new("endless.tif", b)
}

/// A TIFF whose one page claims 100,000 × 100,000 pixels (10 GB at 8 bits)
/// with one byte of data behind it.
pub fn tiff_huge_dimensions() -> Generated {
    let mut b = tiff_header(8);
    let data_at = 8 + IFD_LEN;
    b.extend(ifd(gray_entries(100_000, 100_000, data_at, 1), 0));
    b.push(0);
    Generated::new("huge-dimensions.tif", b)
}

/// All four hostile TIFFs, for a test that runs each the same way.
pub fn hostile_tiffs() -> Vec<Generated> {
    vec![tiff_truncated(), tiff_bit_flipped(), tiff_endless_pages(), tiff_huge_dimensions()]
}

/// A valid uncompressed 8-bit grayscale TIFF of one page, at least
/// `min_bytes` long (a 100 MB document for the large-document case). Its
/// pixels repeat a 4 KiB pattern, so it is cheap to make and the same every
/// time.
pub fn large_tiff(min_bytes: usize) -> Generated {
    let width = 8192u32;
    let height = (min_bytes as u32).div_ceil(width).max(1);
    let data_len = width * height;
    let mut b = tiff_header(8);
    b.extend(ifd(gray_entries(width, height, 8 + IFD_LEN, data_len), 0));
    b.reserve(data_len as usize);
    let pattern: Vec<u8> = (0..4096u32).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8).collect();
    let mut left = data_len as usize;
    while left > 0 {
        let n = left.min(pattern.len());
        b.extend_from_slice(&pattern[..n]);
        left -= n;
    }
    Generated::new("large.tif", b)
}

// ------------------------------------------------------------------ markers

/// A one-page PDF holding `marker`: raw in a content-stream
/// comment and as the page's text, uncompressed, so a scan of the disk for
/// the marker finds any copy of the document (research.md §23, SC-002).
pub fn marker_pdf(marker: &[u8; 64]) -> Generated {
    let mut text = Vec::new();
    for &c in marker {
        if matches!(c, b'(' | b')' | b'\\') {
            text.push(b'\\');
        }
        text.push(c);
    }
    let mut content = b"% ".to_vec();
    content.extend(marker.iter().filter(|&&c| c != b'\n' && c != b'\r'));
    content.extend_from_slice(b"\nBT /F1 10 Tf 36 700 Td (");
    content.extend(text);
    content.extend_from_slice(b") Tj ET");
    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 5 0 R \
          /Resources << /Font << /F1 4 0 R >> >> >>"
            .to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>".to_vec(),
    ];
    let mut stream_obj = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
    stream_obj.extend(content);
    stream_obj.extend_from_slice(b"\nendstream");
    objects.push(stream_obj);

    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend(format!("{} 0 obj\n", i + 1).into_bytes());
        pdf.extend(body);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).into_bytes());
    for off in offsets {
        pdf.extend(format!("{off:010} 00000 n \n").into_bytes());
    }
    pdf.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .into_bytes(),
    );
    Generated::new("marker.pdf", pdf)
}

/// A valid one-page TIFF whose ImageDescription tag holds `marker`.
pub fn marker_tiff(marker: &[u8; 64]) -> Generated {
    let (w, h) = (16u32, 16u32);
    let data_at = 8 + IFD_LEN + 12; // one more entry than a plain page
    let desc_at = data_at + w * h;
    let mut entries = gray_entries(w, h, data_at, w * h);
    entries.push((270, 2, 65, desc_at));
    let mut b = tiff_header(8);
    b.extend(ifd(entries, 0));
    b.extend((0..w * h).map(|i| i as u8));
    b.extend_from_slice(marker);
    b.push(0);
    Generated::new("marker.tif", b)
}

/// A text file that is `marker` and a newline.
pub fn marker_text(marker: &[u8; 64]) -> Generated {
    let mut b = marker.to_vec();
    b.push(b'\n');
    Generated::new("marker.txt", b)
}

// -------------------------------------------------------------------- ZIP

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// A ZIP with every entry stored (no compression), in the order given, so
/// an ODF package's `mimetype` can come first.
fn zip_stored(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let mut common = Vec::new();
        common.extend_from_slice(&le16(10)); // version needed
        common.extend_from_slice(&le16(0)); // flags
        common.extend_from_slice(&le16(0)); // stored
        common.extend_from_slice(&le16(0)); // time
        common.extend_from_slice(&le16(0x21)); // date: 1980-01-01
        common.extend_from_slice(&le32(crc));
        common.extend_from_slice(&le32(data.len() as u32));
        common.extend_from_slice(&le32(data.len() as u32));
        common.extend_from_slice(&le16(name.len() as u16));
        common.extend_from_slice(&le16(0)); // extra length

        out.extend_from_slice(&le32(0x0403_4B50));
        out.extend_from_slice(&common);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        central.extend_from_slice(&le32(0x0201_4B50));
        central.extend_from_slice(&le16(20)); // version made by
        central.extend_from_slice(&common);
        central.extend_from_slice(&le16(0)); // comment length
        central.extend_from_slice(&le16(0)); // disk number
        central.extend_from_slice(&le16(0)); // internal attributes
        central.extend_from_slice(&le32(0)); // external attributes
        central.extend_from_slice(&le32(offset));
        central.extend_from_slice(name.as_bytes());
    }
    let central_at = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&le32(0x0605_4B50));
    out.extend_from_slice(&le16(0));
    out.extend_from_slice(&le16(0));
    out.extend_from_slice(&le16(entries.len() as u16));
    out.extend_from_slice(&le16(entries.len() as u16));
    out.extend_from_slice(&le32(central.len() as u32));
    out.extend_from_slice(&le32(central_at));
    out.extend_from_slice(&le16(0));
    out
}

// ------------------------------------------------------ compound file (OLE)

/// An entry of a compound file's root: a stream or a storage.
struct Node {
    name: String,
    data: Option<Vec<u8>>,
    children: Vec<Node>,
}

fn stream(name: &str, data: Vec<u8>) -> Node {
    Node { name: name.to_owned(), data: Some(data), children: Vec::new() }
}

fn storage(name: &str, children: Vec<Node>) -> Node {
    Node { name: name.to_owned(), data: None, children }
}

const FREE: u32 = 0xFFFF_FFFF;
const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
const FAT_SECTOR: u32 = 0xFFFF_FFFD;

struct DirEntry {
    name: String,
    kind: u8,
    left: u32,
    right: u32,
    child: u32,
    start: u32,
    size: u64,
}

impl DirEntry {
    fn bytes(&self) -> [u8; 128] {
        let mut e = [0u8; 128];
        if !self.name.is_empty() {
            let units: Vec<u16> = self.name.encode_utf16().chain([0]).collect();
            for (i, u) in units.iter().enumerate() {
                e[i * 2..i * 2 + 2].copy_from_slice(&u.to_le_bytes());
            }
            e[64..66].copy_from_slice(&((units.len() * 2) as u16).to_le_bytes());
        }
        e[66] = self.kind;
        e[67] = 1; // black
        e[68..72].copy_from_slice(&self.left.to_le_bytes());
        e[72..76].copy_from_slice(&self.right.to_le_bytes());
        e[76..80].copy_from_slice(&self.child.to_le_bytes());
        e[116..120].copy_from_slice(&self.start.to_le_bytes());
        e[120..128].copy_from_slice(&self.size.to_le_bytes());
        e
    }
}

/// Lays out the siblings `nodes` (and below) as a right-leaning chain in
/// compound-file name order (shorter first, then upper case), which is a
/// valid, if unbalanced, tree. Returns the first sibling's index.
fn lay_out(
    mut nodes: Vec<Node>,
    entries: &mut Vec<DirEntry>,
    data: &mut Vec<(usize, Vec<u8>)>,
) -> u32 {
    nodes.sort_by_key(|n| (n.name.encode_utf16().count(), n.name.to_uppercase()));
    let base = entries.len();
    let count = nodes.len();
    for (i, n) in nodes.iter().enumerate() {
        entries.push(DirEntry {
            name: n.name.clone(),
            kind: if n.data.is_some() { 2 } else { 1 },
            left: FREE,
            right: if i + 1 < count { (base + i + 1) as u32 } else { FREE },
            child: FREE,
            start: END_OF_CHAIN,
            size: 0,
        });
    }
    for (i, n) in nodes.into_iter().enumerate() {
        match n.data {
            Some(bytes) => data.push((base + i, bytes)),
            None => {
                let child = lay_out(n.children, entries, data);
                entries[base + i].child = child;
            }
        }
    }
    base as u32
}

/// A version-3 compound file of `root`'s entries. Every stream is padded to
/// 4096 bytes (the mini-stream cutoff), so none needs the mini stream; one
/// FAT sector, so the file is at most about 60 KB.
fn cfb(root: Vec<Node>) -> Vec<u8> {
    const SECTOR: usize = 512;
    let mut entries = vec![DirEntry {
        name: "Root Entry".into(),
        kind: 5,
        left: FREE,
        right: FREE,
        child: FREE,
        start: END_OF_CHAIN,
        size: 0,
    }];
    let mut data = Vec::new();
    let child = lay_out(root, &mut entries, &mut data);
    entries[0].child = child;

    let dir_sectors = entries.len().div_ceil(4);
    let mut fat = vec![FREE; 128];
    fat[0] = FAT_SECTOR;
    for i in 0..dir_sectors {
        fat[1 + i] = if i + 1 < dir_sectors { (2 + i) as u32 } else { END_OF_CHAIN };
    }
    let mut next = 1 + dir_sectors;
    let mut body = Vec::new();
    for (index, mut bytes) in data {
        let size = bytes.len().max(4096);
        bytes.resize(size.div_ceil(SECTOR) * SECTOR, 0);
        let sectors = bytes.len() / SECTOR;
        entries[index].start = next as u32;
        entries[index].size = bytes.len() as u64;
        for i in 0..sectors {
            fat[next + i] = if i + 1 < sectors { (next + i + 1) as u32 } else { END_OF_CHAIN };
        }
        next += sectors;
        body.extend(bytes);
    }
    assert!(next <= 128, "compound file too big for one FAT sector");

    let mut header = vec![0u8; SECTOR];
    header[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
    header[24..26].copy_from_slice(&le16(0x3E));
    header[26..28].copy_from_slice(&le16(3));
    header[28..30].copy_from_slice(&le16(0xFFFE));
    header[30..32].copy_from_slice(&le16(9)); // 512-byte sectors
    header[32..34].copy_from_slice(&le16(6)); // 64-byte mini sectors
    header[44..48].copy_from_slice(&le32(1)); // FAT sectors
    header[48..52].copy_from_slice(&le32(1)); // first directory sector
    header[56..60].copy_from_slice(&le32(4096)); // mini-stream cutoff
    header[60..64].copy_from_slice(&le32(END_OF_CHAIN)); // first mini FAT sector
    header[68..72].copy_from_slice(&le32(END_OF_CHAIN)); // first DIFAT sector
    header[76..80].copy_from_slice(&le32(0)); // DIFAT[0]: the FAT is sector 0
    for i in 1..109 {
        header[76 + 4 * i..80 + 4 * i].copy_from_slice(&le32(FREE));
    }

    let mut out = header;
    out.extend(fat.iter().flat_map(|v| v.to_le_bytes()));
    for entry in &entries {
        out.extend_from_slice(&entry.bytes());
    }
    // Fill the last directory sector with unused entries.
    let unused = DirEntry {
        name: String::new(),
        kind: 0,
        left: FREE,
        right: FREE,
        child: FREE,
        start: 0,
        size: 0,
    };
    for _ in entries.len()..dir_sectors * 4 {
        out.extend_from_slice(&unused.bytes());
    }
    out.extend(body);
    out
}
