//! Plain text and CSV, decoded for the viewer (research.md §12).
//!
//! [`decode`] is total: every input gives a string, bounded by the input's
//! size, so no helper process is needed. It is also inert: it only chooses an
//! encoding and neutralizes control characters. Nothing is parsed, linked or
//! laid out; the viewer shows the string as a React text child of a `<pre>`.

use encoding_rs::{UTF_16BE, UTF_16LE, WINDOWS_1252};

const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
const UTF16_LE_BOM: [u8; 2] = [0xFF, 0xFE];
const UTF16_BE_BOM: [u8; 2] = [0xFE, 0xFF];

/// Decodes a text document's bytes (FR-005):
///
/// - a UTF-8 byte order mark is stripped;
/// - UTF-16 LE or BE is taken from its mark;
/// - otherwise UTF-8 if the whole text is valid, else Windows-1252, the usual
///   encoding of legacy spreadsheet exports (what it can't map is U+FFFD);
/// - C0 and C1 control characters other than tab, line feed and carriage
///   return become U+FFFD;
/// - CR LF and a lone CR become LF.
pub fn decode(bytes: &[u8]) -> String {
    let text = if let Some(rest) = bytes.strip_prefix(&UTF8_BOM) {
        decode_without_mark(rest)
    } else if let Some(rest) = bytes.strip_prefix(&UTF16_LE_BOM) {
        UTF_16LE.decode_without_bom_handling(rest).0.into_owned()
    } else if let Some(rest) = bytes.strip_prefix(&UTF16_BE_BOM) {
        UTF_16BE.decode_without_bom_handling(rest).0.into_owned()
    } else {
        decode_without_mark(bytes)
    };
    inert(&text)
}

fn decode_without_mark(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => WINDOWS_1252.decode_without_bom_handling(bytes).0.into_owned(),
    }
}

/// Replaces the control characters and normalizes line endings.
fn inert(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            '\t' | '\n' => out.push(c),
            c if c.is_control() && c != '\u{7f}' => out.push('\u{FFFD}'),
            c => out.push(c),
        }
    }
    out
}
