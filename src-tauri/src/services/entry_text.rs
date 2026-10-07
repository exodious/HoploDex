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

/// The six fields with suggestions and snapping (FR-009; specs/005-regulated-item-types
/// research.md §7 adds the registration form and "Registered to").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryField {
    Make,
    Model,
    Cartridge,
    Caliber,
    RegistrationForm,
    RegisteredTo,
}

impl EntryField {
    pub const ALL: [EntryField; 6] = [
        Self::Make,
        Self::Model,
        Self::Cartridge,
        Self::Caliber,
        Self::RegistrationForm,
        Self::RegisteredTo,
    ];

    /// The `firearms` column, which is also the spreadsheet column. The IPC
    /// name is [`Self::ipc_name`].
    pub fn column(&self) -> &'static str {
        match self {
            Self::Make => "make",
            Self::Model => "model",
            Self::Cartridge => "cartridge",
            Self::Caliber => "caliber",
            Self::RegistrationForm => "registration_form",
            Self::RegisteredTo => "registered_to",
        }
    }

    /// The field's name over IPC: a `FieldErrors` key and the `field` of the
    /// entry commands.
    pub fn ipc_name(&self) -> &'static str {
        match self {
            Self::RegistrationForm => "registrationForm",
            Self::RegisteredTo => "registeredTo",
            other => other.column(),
        }
    }

    /// How a message names the field.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Make => "Make",
            Self::Model => "Model",
            Self::Cartridge => "Cartridge",
            Self::Caliber => "Caliber",
            Self::RegistrationForm => "Form",
            Self::RegisteredTo => "Registered to",
        }
    }

    /// Make, model and caliber are required; the rest are optional (FR-001).
    pub fn required(&self) -> bool {
        matches!(self, Self::Make | Self::Model | Self::Caliber)
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
    let mut tokens = Vec::new();
    ascii_for_each_token(folded, |token| tokens.push(token.to_owned()));
    tokens
}

/// The tokens of already folded ASCII text, one call each, with nothing
/// allocated.
fn ascii_for_each_token(folded: &str, mut each: impl FnMut(&str)) {
    let bytes = folded.as_bytes();
    fn space(byte: &u8) -> bool {
        (*byte as char).is_whitespace()
    }
    fn digit_beside<'a>(mut range: impl Iterator<Item = &'a u8>) -> bool {
        range.find(|byte| !space(byte)).is_some_and(u8::is_ascii_digit)
    }
    let mut start = None;
    for (index, byte) in bytes.iter().enumerate() {
        let separator = space(byte)
            || matches!(byte, b'-' | b'/' | b'.' | b'&')
            || (*byte == b'x'
                && digit_beside(bytes[..index].iter().rev())
                && digit_beside(bytes[index + 1..].iter()));
        match (separator, start) {
            (true, Some(from)) => {
                each(&folded[from..index]);
                start = None;
            }
            (false, None) => start = Some(index),
            _ => {}
        }
    }
    if let Some(from) = start {
        each(&folded[from..]);
    }
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

/// The words of a value and its entry key, in buffers that are reused from
/// one value to the next. The key is the words concatenated, so each word is
/// a slice of it: a suggestion query that goes through every value on record
/// (20,000 models over two tables) normalizes each into one of these without
/// allocating, where [`words`] and [`entry_key`] allocate a `String` per word
/// and another for the key.
#[derive(Debug, Clone, Default)]
pub struct EntryWords {
    key: String,
    /// Where each word ends in `key`.
    ends: Vec<usize>,
    /// The folded ASCII text being split.
    scratch: String,
}

impl EntryWords {
    pub fn new(text: &str) -> Self {
        let mut words = Self::default();
        words.set(text);
        words
    }

    /// Replaces the contents with the words of `text`: the same words and
    /// key as [`words`] and [`entry_key`].
    pub fn set(&mut self, text: &str) {
        self.key.clear();
        self.ends.clear();
        if text.is_ascii() {
            self.scratch.clear();
            self.scratch.push_str(text);
            self.scratch.make_ascii_lowercase();
            let (key, ends) = (&mut self.key, &mut self.ends);
            ascii_for_each_token(&self.scratch, |token| {
                if token != "and" {
                    key.push_str(token);
                    ends.push(key.len());
                }
            });
        } else {
            for word in words(text) {
                self.key.push_str(&word);
                self.ends.push(self.key.len());
            }
        }
    }

    /// The entry key.
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }

    pub fn words(&self) -> impl Iterator<Item = &str> {
        let mut from = 0;
        self.ends.iter().map(move |&end| {
            let word = &self.key[from..end];
            from = end;
            word
        })
    }
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

/// [`check_entry_text`] for a field that may be left blank on this record
/// (an accessory's make and model, specs/006-accessory-links FR-001): a blank
/// value passes whatever the field's own requirement, and any other is
/// judged by the same rules.
pub fn check_optional_entry_text(field: EntryField, value: &str) -> Result<(), String> {
    if value.trim().is_empty() { Ok(()) } else { check_entry_text(field, value) }
}
