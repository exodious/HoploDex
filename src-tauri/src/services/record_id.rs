//! specs/006-accessory-links research.md §6: the record identifier (FR-019),
//! a random version 4 UUID, lowercase and hyphenated.

use std::fmt::Write;

/// A new identifier: 16 random bytes with the version nibble set to 4 and
/// the variant bits to `10`, as lowercase hyphenated hex.
pub fn generate() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the operating system's random source is unavailable");
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut text = String::with_capacity(36);
    for (i, byte) in bytes.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        write!(text, "{byte:02x}").expect("writing to a String cannot fail");
    }
    text
}

/// A spreadsheet cell as an identifier: trimmed, in any letter case, and
/// only the version 4 form `generate` makes. Returns it lowercase; `None`
/// when the cell is anything else.
pub fn parse(cell: &str) -> Option<String> {
    let text = cell.trim().to_ascii_lowercase();
    let valid = text.len() == 36
        && text.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            14 => b == b'4',
            19 => matches!(b, b'8' | b'9' | b'a' | b'b'),
            _ => matches!(b, b'0'..=b'9' | b'a'..=b'f'),
        });
    valid.then_some(text)
}
