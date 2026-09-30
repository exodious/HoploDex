//! The entry key and the entry rules for make, model, cartridge and caliber
//! (specs/004-cartridges-action-types research.md §3 and §9).
//!
//! Two values are same-notation variants (FR-013) exactly when their
//! [`entry_key`]s are equal. Suggestion deduplication, matching, snapping in
//! the form and on import, and the import's sheet pass all compare through
//! it, so they cannot drift apart.

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

/// FR-015: the longest value, in characters, the four fields accept.
pub const MAX_ENTRY_CHARS: usize = 100;

/// The four fields with suggestions and snapping (FR-009).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryField {
    Make,
    Model,
    Cartridge,
    Caliber,
}

impl EntryField {
    pub const ALL: [EntryField; 4] = [Self::Make, Self::Model, Self::Cartridge, Self::Caliber];

    /// The `firearms` column, which is also the IPC field name and the
    /// spreadsheet column.
    pub fn column(&self) -> &'static str {
        match self {
            Self::Make => "make",
            Self::Model => "model",
            Self::Cartridge => "cartridge",
            Self::Caliber => "caliber",
        }
    }

    /// How a message names the field.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Make => "Make",
            Self::Model => "Model",
            Self::Cartridge => "Cartridge",
            Self::Caliber => "Caliber",
        }
    }

    /// Cartridge is optional (FR-001); the other three are required.
    pub fn required(&self) -> bool {
        !matches!(self, Self::Cartridge)
    }
}

/// Every Unicode dash punctuation character (general category `Pd`).
const DASHES: &[char] = &[
    '\u{002D}',
    '\u{058A}',
    '\u{05BE}',
    '\u{1400}',
    '\u{1806}',
    '\u{2010}',
    '\u{2011}',
    '\u{2012}',
    '\u{2013}',
    '\u{2014}',
    '\u{2015}',
    '\u{2E17}',
    '\u{2E1A}',
    '\u{2E3A}',
    '\u{2E3B}',
    '\u{2E40}',
    '\u{2E5D}',
    '\u{301C}',
    '\u{3030}',
    '\u{30A0}',
    '\u{FE31}',
    '\u{FE32}',
    '\u{FE58}',
    '\u{FE63}',
    '\u{FF0D}',
    '\u{10EAD}',
];

/// Steps 1 and 2 of research.md §3: NFKC, Unicode lower case, `×` as `x`
/// and every dash as `-`.
pub fn fold(text: &str) -> String {
    // NFKC leaves ASCII alone, and nearly every make, model and cartridge is
    // ASCII: skipping it keeps a 10,000-value suggestion query fast.
    if text.is_ascii() {
        // Only `-` is a dash in ASCII, and there is no `×`.
        return text.to_ascii_lowercase();
    }
    text.nfkc()
        .collect::<String>()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            '×' => 'x',
            c if DASHES.contains(&c) => '-',
            c => c,
        })
        .collect()
}

/// Whether the first non-space character in `range` is a digit.
fn digit_beside<'a>(mut range: impl Iterator<Item = &'a char>) -> bool {
    range.find(|c| !c.is_whitespace()).is_some_and(|c| c.is_ascii_digit())
}

/// Step 3: the tokens, split on whitespace, `-`, `/`, `.`, `&`, and on an
/// `x` between two digits (spaces around it allowed: `9x19`, `9 x 19`).
fn tokens(text: &str) -> Vec<String> {
    let folded = fold(text);
    if folded.is_ascii() {
        return ascii_tokens(&folded);
    }
    let chars: Vec<char> = folded.chars().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();
    for (index, &c) in chars.iter().enumerate() {
        let separator = c.is_whitespace()
            || matches!(c, '-' | '/' | '.' | '&')
            || (c == 'x'
                && digit_beside(chars[..index].iter().rev())
                && digit_beside(chars[index + 1..].iter()));
        if separator {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// [`tokens`] for ASCII text, which is nearly all of it: the same rule
/// without decoding to characters, since a 10,000-value suggestion query
/// tokenizes every value on record.
fn ascii_tokens(folded: &str) -> Vec<String> {
    let bytes = folded.as_bytes();
    fn space(byte: &u8) -> bool {
        (*byte as char).is_whitespace()
    }
    fn digit_beside<'a>(mut range: impl Iterator<Item = &'a u8>) -> bool {
        range.find(|byte| !space(byte)).is_some_and(u8::is_ascii_digit)
    }
    let mut tokens = Vec::new();
    let mut start = None;
    for (index, byte) in bytes.iter().enumerate() {
        let separator = space(byte)
            || matches!(byte, b'-' | b'/' | b'.' | b'&')
            || (*byte == b'x'
                && digit_beside(bytes[..index].iter().rev())
                && digit_beside(bytes[index + 1..].iter()));
        match (separator, start) {
            (true, Some(from)) => {
                tokens.push(folded[from..index].to_owned());
                start = None;
            }
            (false, None) => start = Some(index),
            _ => {}
        }
    }
    if let Some(from) = start {
        tokens.push(folded[from..].to_owned());
    }
    tokens
}

/// The words of a value: its tokens without `and` (research.md §3, step 4).
/// Used for word-prefix matching and initialisms.
pub fn words(text: &str) -> Vec<String> {
    tokens(text).into_iter().filter(|token| token != "and").collect()
}

/// The comparison form of a value (research.md §3): its words joined with
/// nothing between them. "Smith & Wesson", "smith and wesson" and
/// "Smith&Wesson" are all `smithwesson`.
pub fn entry_key(text: &str) -> String {
    words(text).concat()
}

/// The first character of each word: "Smith & Wesson" is `sw`.
pub fn initialism(text: &str) -> String {
    words(text).iter().filter_map(|word| word.chars().next()).collect()
}

/// FR-015: once trimmed, at most [`MAX_ENTRY_CHARS`] characters (Unicode
/// scalar values) and no control characters (Unicode `Cc`); make, model and
/// caliber must also be non-empty. The message names the field.
pub fn check_entry_text(field: EntryField, value: &str) -> Result<(), String> {
    let value = value.trim();
    let label = field.label();
    if value.is_empty() {
        return if field.required() { Err(format!("{label} is required.")) } else { Ok(()) };
    }
    if value.chars().count() > MAX_ENTRY_CHARS {
        return Err(format!("{label} can be at most {MAX_ENTRY_CHARS} characters."));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{label} can't contain control characters."));
    }
    Ok(())
}
