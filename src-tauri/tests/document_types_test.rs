//! `services::document_types`: the one list, `classify`, and the
//! `list_document_types` command's op (feature 007, FR-016, FR-017,
//! research.md §2).

mod support;

use std::time::{Duration, Instant};

use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::models::document_attachment::PreviewKind;
use hoplodex_lib::services::document_types::{self, Refusal, classify, from_recorded};
use serde_json::json;
use support::document_fixture;
use support::hostile_documents::{self as hostile, Refusal as Owed};

/// Every type of research.md §2's table: a fixture, the name to give it, the
/// canonical MIME type and extension, and how it is previewed.
fn accepted_cases()
-> Vec<(&'static str, &'static str, &'static str, &'static str, Option<PreviewKind>)> {
    use PreviewKind::*;
    vec![
        ("three-pages.pdf", "receipt.pdf", "application/pdf", "pdf", Some(Pdf)),
        ("one-page.tif", "scan.tif", "image/tiff", "tif", Some(Tiff)),
        ("one-page.tif", "scan.tiff", "image/tiff", "tif", Some(Tiff)),
        (
            "sample.docx",
            "bill.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "docx",
            None,
        ),
        ("sample.doc", "bill.doc", "application/msword", "doc", None),
        (
            "sample.xlsx",
            "log.xlsx",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "xlsx",
            None,
        ),
        ("sample.xls", "log.xls", "application/vnd.ms-excel", "xls", None),
        ("sample.odt", "letter.odt", "application/vnd.oasis.opendocument.text", "odt", None),
        ("sample.ods", "log.ods", "application/vnd.oasis.opendocument.spreadsheet", "ods", None),
    ]
}

#[test]
fn accepts_every_type_from_the_fixtures_with_its_canonical_type() {
    for (fixture, name, mime, extension, preview) in accepted_cases() {
        let found = classify(name, &document_fixture(fixture))
            .unwrap_or_else(|e| panic!("{name} should be accepted: {e:?}"));
        assert_eq!(found.mime_type, mime, "{name}");
        assert_eq!(found.canonical_extension, extension, "{name}");
        assert_eq!(found.preview_kind, preview, "{name}");
    }
    let text = classify("note.txt", b"Bought at the range.\n").unwrap();
    assert_eq!((text.mime_type, text.preview_kind), ("text/plain", Some(PreviewKind::Text)));
    let csv = classify("rounds.csv", b"date,rounds\n2026-01-01,150\n").unwrap();
    assert_eq!((csv.mime_type, csv.preview_kind), ("text/csv", Some(PreviewKind::Text)));
    let rtf = classify("letter.rtf", b"{\\rtf1\\ansi Hello}").unwrap();
    assert_eq!((rtf.mime_type, rtf.preview_kind), ("application/rtf", None));
}

#[test]
fn compares_the_extension_without_regard_to_case() {
    assert_eq!(classify("RECEIPT.PDF", &document_fixture("three-pages.pdf")).unwrap().label, "PDF");
    assert_eq!(classify("Scan.Tiff", &document_fixture("one-page.tif")).unwrap().label, "TIFF");
    assert_eq!(classify("Bill.DocX", &document_fixture("sample.docx")).unwrap().label, "Word");
    assert_eq!(classify("NOTE.TXT", b"hello").unwrap().mime_type, "text/plain");
}

#[test]
fn a_damaged_pdf_or_tiff_that_keeps_its_signature_is_accepted() {
    // The viewer reports them in its own words (FR-006).
    for fixture in ["three-pages-truncated.pdf", "three-pages-bitflipped.pdf"] {
        assert!(classify("damaged.pdf", &document_fixture(fixture)).is_ok(), "{fixture}");
    }
    assert!(classify("locked.pdf", &document_fixture("password-protected.pdf")).is_ok());
    for tiff in hostile::hostile_tiffs() {
        assert!(classify(&tiff.name, &tiff.bytes).is_ok(), "{}", tiff.name);
    }
    assert!(classify("big.tif", &hostile::large_tiff(1_000_000).bytes).is_ok());
}

#[test]
fn the_pdf_signature_counts_within_the_first_1024_bytes_only() {
    let mut inside = vec![b' '; 1019];
    inside.extend_from_slice(b"%PDF-1.4");
    assert!(classify("a.pdf", &inside).is_ok(), "ends at byte 1024");

    let mut beyond = vec![b' '; 1020];
    beyond.extend_from_slice(b"%PDF-1.4");
    assert_eq!(classify("a.pdf", &beyond).unwrap_err().code(), "DOCUMENT_CONTENT_MISMATCH");
    assert!(classify("a.pdf", b"").is_err());
}

#[test]
fn every_tiff_byte_order_and_bigtiff_is_a_tiff() {
    for head in [b"II*\0", b"MM\0*", b"II+\0", b"MM\0+"] {
        let mut bytes = head.to_vec();
        bytes.extend_from_slice(&[0; 8]);
        assert!(classify("a.tif", &bytes).is_ok(), "{head:?}");
    }
    for head in [&b"II\0*"[..], b"MM*\0", b"II*"] {
        assert!(classify("a.tif", head).is_err(), "{head:?}");
    }
}

#[test]
fn refuses_every_hostile_document_with_its_code() {
    let all = hostile::refused_documents();
    assert!(all.len() > 30, "the generators are all here");
    for hostile in all {
        let refusal = classify(&hostile.document.name, &hostile.document.bytes)
            .err()
            .unwrap_or_else(|| panic!("{} should be refused", hostile.document.name));
        assert_eq!(refusal.code(), hostile.refusal.code(), "{}", hostile.document.name);
        assert_eq!(
            matches!(refusal, Refusal::Photo { .. }),
            hostile.refusal == Owed::Photo,
            "{}: only .jpg, .jpeg and .png get the photo message",
            hostile.document.name
        );
    }
}

#[test]
fn the_photo_message_sends_the_user_to_photos_and_the_other_names_the_types() {
    let photo = classify("scan.jpg", &hostile::jpg().bytes).unwrap_err();
    assert_eq!(photo.message(), "scan.jpg is a photo. Add it under Photos instead.");

    let exe = classify("setup.exe", &hostile::exe().bytes).unwrap_err();
    for label in [
        "PDF",
        "TIFF",
        "Plain text",
        "CSV",
        "RTF",
        "Word",
        "Spreadsheet",
        "OpenDocument text",
        "OpenDocument spreadsheet",
    ] {
        assert!(exe.message().contains(label), "{label} in {}", exe.message());
    }
    assert!(exe.message().contains("setup.exe"));
}

#[test]
fn a_refusal_becomes_a_command_error_with_its_code_and_message() {
    let error: hoplodex_lib::commands::CommandError =
        classify("scan.png", &hostile::png().bytes).unwrap_err().into();
    assert_eq!(error.code, "DOCUMENT_TYPE_NOT_ALLOWED");
    assert_eq!(error.message, "scan.png is a photo. Add it under Photos instead.");

    let error: hoplodex_lib::commands::CommandError =
        classify("receipt.pdf", &hostile::html_named_pdf().bytes).unwrap_err().into();
    assert_eq!(error.code, "DOCUMENT_CONTENT_MISMATCH");
    assert!(error.message.contains("receipt.pdf"));
}

#[test]
fn hyperlinks_pictures_charts_and_rtf_fields_are_accepted() {
    for plain in
        [hostile::docx_with_hyperlink(), hostile::docx_with_chart_workbook(), hostile::plain_rtf()]
    {
        assert!(classify(&plain.name, &plain.bytes).is_ok(), "{}", plain.name);
    }
}

#[test]
fn an_embedded_object_or_outside_link_is_named_in_the_refusal() {
    let refused = classify("letter.rtf", &hostile::rtf_with_object().bytes).unwrap_err();
    assert_eq!(
        refused.message(),
        "letter.rtf isn't a real RTF document, or it holds macros, scripts, web page code, \
         embedded objects or links that load outside content, so it can't be kept as a document."
    );
}

#[test]
fn a_file_with_no_extension_is_not_allowed() {
    assert_eq!(classify("receipt", b"%PDF-1.4").unwrap_err().code(), "DOCUMENT_TYPE_NOT_ALLOWED");
    assert_eq!(classify("receipt.", b"%PDF-1.4").unwrap_err().code(), "DOCUMENT_TYPE_NOT_ALLOWED");
}

#[test]
fn only_the_last_extension_and_the_last_path_component_count() {
    assert!(classify("receipt.exe.pdf", &document_fixture("three-pages.pdf")).is_ok());
    assert!(classify("C:\\Users\\me\\receipt.pdf", &document_fixture("three-pages.pdf")).is_ok());
    assert!(classify("receipt.pdf.exe", &document_fixture("three-pages.pdf")).is_err());
}

#[test]
fn text_may_mention_markup_but_not_start_with_it() {
    assert!(classify("note.txt", b"Use <html> tags in the page.\n").is_ok());
    assert!(classify("note.txt", b"a < b and <svg is later\n").is_ok());
    assert!(classify("note.txt", b"< html>").is_ok(), "a space after < is not markup");
    assert!(classify("note.txt", b"<3 the range\n").is_ok());
    assert!(classify("empty.txt", b"").is_ok());
    assert!(classify("note.txt", b"   \n").is_ok());

    // Only the first 512 bytes count, after the whitespace.
    let mut late = vec![b'a'; 600];
    late.extend_from_slice(b"<html>");
    assert!(classify("note.txt", &late).is_ok());

    // A UTF-16 byte order mark does not hide it.
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("<html>".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    assert_eq!(classify("note.txt", &utf16).unwrap_err().code(), "DOCUMENT_CONTENT_MISMATCH");
}

#[test]
fn a_zip_declaring_gigabytes_is_classified_from_its_directory() {
    let bomb = hostile::docx_declaring_gigabytes();
    let started = Instant::now();
    let result = classify(&bomb.name, &bomb.bytes);
    let took = started.elapsed();
    assert!(result.is_ok(), "{result:?}");
    assert!(took < Duration::from_millis(50), "took {took:?}");
}

#[test]
fn an_office_package_is_judged_by_its_main_part_and_its_parts() {
    let docx = document_fixture("sample.docx");
    // A Word package named as a spreadsheet, and the reverse.
    assert_eq!(classify("a.xlsx", &docx).unwrap_err().code(), "DOCUMENT_CONTENT_MISMATCH");
    assert_eq!(
        classify("a.docx", &document_fixture("sample.xlsx")).unwrap_err().code(),
        "DOCUMENT_CONTENT_MISMATCH"
    );
    // OpenDocument text named as a spreadsheet; compound file named as ZIP.
    assert!(classify("a.ods", &document_fixture("sample.odt")).is_err());
    assert!(classify("a.docx", &document_fixture("sample.doc")).is_err());
    assert!(classify("a.xls", &document_fixture("sample.doc")).is_err(), "no Workbook stream");
    assert!(classify("a.doc", &document_fixture("sample.xls")).is_err(), "no WordDocument stream");
    assert!(classify("a.rtf", &docx).is_err());
}

#[test]
fn the_list_is_in_table_order_with_its_labels_and_preview_kinds() {
    let all = document_types::all();
    let mut labels: Vec<&str> = Vec::new();
    for t in all {
        if labels.last() != Some(&t.label) {
            labels.push(t.label);
        }
    }
    assert_eq!(
        labels,
        [
            "PDF",
            "TIFF",
            "Plain text",
            "CSV",
            "RTF",
            "Word",
            "Spreadsheet",
            "OpenDocument text",
            "OpenDocument spreadsheet"
        ]
    );
    let extensions: Vec<&[&str]> = all.iter().map(|t| t.extensions).collect();
    assert_eq!(
        extensions,
        [
            &["pdf"][..],
            &["tif", "tiff"],
            &["txt"],
            &["csv"],
            &["rtf"],
            &["doc"],
            &["docx"],
            &["xls"],
            &["xlsx"],
            &["odt"],
            &["ods"]
        ]
    );
    let kinds: Vec<(&str, Option<PreviewKind>)> =
        all.iter().map(|t| (t.extensions[0], t.preview_kind)).collect();
    assert_eq!(
        kinds,
        [
            ("pdf", Some(PreviewKind::Pdf)),
            ("tif", Some(PreviewKind::Tiff)),
            ("txt", Some(PreviewKind::Text)),
            ("csv", Some(PreviewKind::Text)),
            ("rtf", None),
            ("doc", None),
            ("docx", None),
            ("xls", None),
            ("xlsx", None),
            ("odt", None),
            ("ods", None),
        ]
    );
}

#[test]
fn from_recorded_finds_each_canonical_type_and_nothing_else() {
    for t in document_types::all() {
        assert_eq!(from_recorded(t.mime_type), Some(t));
    }
    for other in ["image/jpeg", "image/png", "application/octet-stream", "", "APPLICATION/PDF"] {
        assert!(from_recorded(other).is_none(), "{other}");
    }
}

#[test]
fn list_document_types_gives_the_list_in_the_contracts_shape() {
    let listed = document_ops::list_document_types();
    assert_eq!(listed.len(), 11);
    let value = serde_json::to_value(listed).unwrap();
    assert_eq!(
        value[0],
        json!({
            "label": "PDF",
            "extensions": ["pdf"],
            "mimeType": "application/pdf",
            "previewKind": "pdf"
        })
    );
    assert_eq!(value[1]["extensions"], json!(["tif", "tiff"]));
    assert_eq!(value[1]["previewKind"], json!("tiff"));
    assert_eq!(value[4]["previewKind"], json!(null));
    assert_eq!(value[10]["label"], json!("OpenDocument spreadsheet"));
}
