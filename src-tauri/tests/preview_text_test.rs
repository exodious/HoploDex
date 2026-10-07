//! specs/007-document-preview User Story 1's text and CSV previews (tasks.md
//! T030, US1-4, research.md §12): `services::preview::text::decode` turns the
//! stored bytes into the string the viewer shows as a text child of a `<pre>`.
//! It is total (every input gives a string) and inert (nothing is parsed,
//! linked or laid out).
//!
//! Assumed API: `pub fn decode(bytes: &[u8]) -> String`.

use hoplodex_lib::services::preview::text::decode;

const REPLACEMENT: char = '\u{FFFD}';

fn utf16(text: &str, bom: [u8; 2], to_bytes: fn(u16) -> [u8; 2]) -> Vec<u8> {
    let mut bytes = bom.to_vec();
    for unit in text.encode_utf16() {
        bytes.extend(to_bytes(unit));
    }
    bytes
}

fn utf16_le(text: &str) -> Vec<u8> {
    utf16(text, [0xFF, 0xFE], u16::to_le_bytes)
}

fn utf16_be(text: &str) -> Vec<u8> {
    utf16(text, [0xFE, 0xFF], u16::to_be_bytes)
}

// --- Encodings ---------------------------------------------------------------------------

#[test]
fn plain_utf8_is_returned_as_it_is() {
    assert_eq!(
        decode("Colt 1911, caf\u{e9} \u{1F52B}".as_bytes()),
        "Colt 1911, caf\u{e9} \u{1F52B}"
    );
    assert_eq!(decode(b""), "");
}

#[test]
fn a_utf8_byte_order_mark_is_stripped() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend("make,model\nColt,1911".as_bytes());

    let text = decode(&bytes);

    assert_eq!(text, "make,model\nColt,1911");
    assert!(!text.starts_with('\u{FEFF}'));
}

#[test]
fn utf16_is_read_by_its_byte_order_mark_in_either_order() {
    let text = "Receipt \u{e9}t\u{e9}\nline two \u{1F52B}";

    assert_eq!(decode(&utf16_le(text)), text, "little endian");
    assert_eq!(decode(&utf16_be(text)), text, "big endian");
}

#[test]
fn a_utf16_byte_order_mark_is_not_part_of_the_text() {
    assert!(!decode(&utf16_le("abc")).contains('\u{FEFF}'));
    assert!(!decode(&utf16_be("abc")).contains('\u{FEFF}'));
}

#[test]
fn bytes_that_are_not_valid_utf8_are_read_as_windows_1252() {
    // 0xE9 is "\u{e9}" and 0x80 the euro sign in Windows-1252, and a lone one
    // is not UTF-8.
    assert_eq!(decode(b"caf\xE9"), "caf\u{e9}");
    assert_eq!(decode(b"price \x80 100"), "price \u{20AC} 100");
    // Smart quotes, the usual legacy spreadsheet export.
    assert_eq!(decode(b"\x93quoted\x94"), "\u{201C}quoted\u{201D}");
}

#[test]
fn the_whole_text_is_read_as_one_encoding() {
    // Valid UTF-8 in one place and a stray byte in another: the stray byte
    // makes the whole text Windows-1252, it doesn't leave half decoded.
    let mut bytes = "caf\u{e9} ".as_bytes().to_vec();
    bytes.push(0xE9);

    assert_eq!(decode(&bytes), "caf\u{c3}\u{a9} \u{e9}");
}

#[test]
fn a_byte_windows_1252_cannot_map_comes_out_as_a_replacement_character() {
    // 0x81 has no character in Windows-1252: the text is not lost, and what
    // can't be mapped is U+FFFD, never a control character.
    let text = decode(b"a\x81b");

    assert_eq!(text, format!("a{REPLACEMENT}b"));
}

#[test]
fn any_bytes_at_all_decode_without_failing() {
    let every_byte: Vec<u8> = (0..=255).collect();
    let _ = decode(&every_byte);
    let _ = decode(&[0xFF, 0xFE, 0x00]); // a UTF-16 mark and half a unit
    let _ = decode(&[0xFE, 0xFF, 0xD8]); // a lone high surrogate
    let _ = decode(&[0xEF, 0xBB]); // half a UTF-8 mark
}

// --- Control characters ------------------------------------------------------------------

#[test]
fn c0_controls_other_than_tab_line_feed_and_carriage_return_become_replacement_characters() {
    for control in (0x00u8..=0x1F).filter(|b| !b"\t\n\r".contains(b)) {
        let text = decode(&[b'a', control, b'b']);

        assert_eq!(text, format!("a{REPLACEMENT}b"), "control {control:#04x}");
    }
}

#[test]
fn c1_controls_become_replacement_characters() {
    for code in 0x80u32..=0x9F {
        let control = char::from_u32(code).unwrap();
        let text = decode(format!("a{control}b").as_bytes());

        assert_eq!(text, format!("a{REPLACEMENT}b"), "control U+{code:04X}");
    }
}

#[test]
fn escape_sequences_and_a_nul_are_made_inert() {
    // A terminal escape and a NUL are the controls a text file can smuggle in.
    assert_eq!(decode(b"\x1b[31mred\x1b[0m"), format!("{REPLACEMENT}[31mred{REPLACEMENT}[0m"));
    assert_eq!(decode(b"a\0b"), format!("a{REPLACEMENT}b"));
}

#[test]
fn controls_in_utf16_text_are_replaced_too() {
    assert_eq!(decode(&utf16_le("a\u{7}b\u{85}c")), format!("a{REPLACEMENT}b{REPLACEMENT}c"));
    assert_eq!(decode(&utf16_be("a\u{1b}b")), format!("a{REPLACEMENT}b"));
}

#[test]
fn tab_and_line_feed_stay() {
    assert_eq!(decode(b"a\tb\nc"), "a\tb\nc");
}

// --- Line endings ------------------------------------------------------------------------

#[test]
fn carriage_return_line_feed_and_a_lone_carriage_return_become_line_feed() {
    assert_eq!(decode(b"one\r\ntwo\rthree\nfour"), "one\ntwo\nthree\nfour");
    assert_eq!(decode(b"a\r\n\r\nb"), "a\n\nb", "a blank line stays one");
    assert_eq!(decode(b"a\r\r\nb"), "a\n\nb");
    assert_eq!(decode(b"end\r"), "end\n");
    assert_eq!(decode(&utf16_le("x\r\ny\rz")), "x\ny\nz", "in UTF-16 too");
}

// --- Inert content -----------------------------------------------------------------------

#[test]
fn markup_formulas_and_links_come_back_unchanged_as_text() {
    for text in [
        "<b>bold</b>",
        "<script>alert(1)</script>",
        "<img src=\"https://example.com/x.png\">",
        "=SUM(A1:A3)",
        "=HYPERLINK(\"https://example.com\",\"click\")",
        "+1+1",
        "@SUM(A1)",
        "https://example.com",
        "[link](https://example.com) &amp; &lt;",
        "a,b,\"c, d\"\n1,2,3",
    ] {
        assert_eq!(decode(text.as_bytes()), text, "{text:?}");
        assert_eq!(decode(&utf16_le(text)), text, "{text:?} as UTF-16");
    }
}
