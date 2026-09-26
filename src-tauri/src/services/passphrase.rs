//! Passphrases as the backend holds them (research.md §1, FR-003, FR-007):
//! normalized to Unicode NFC so the same passphrase typed on any platform
//! gives the same key, never trimmed, and wiped from memory when dropped.

use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use crate::commands::CommandError;

/// The fewest Unicode scalar values a new passphrase may have.
pub const MIN_PASSPHRASE_CHARS: usize = 12;

/// A passphrase received from the frontend. It has no `Debug` or `Display`,
/// so it cannot end up in a log or an error message, and its memory is
/// zeroed when it is dropped. Build one as soon as the command receives the
/// string, and drop it when the command returns: the backend never keeps a
/// passphrase between commands.
pub struct Passphrase(Zeroizing<String>);

impl Passphrase {
    /// Takes the typed string (wiping it once normalized) and normalizes it
    /// to NFC.
    pub fn from_input(input: String) -> Self {
        let input = Zeroizing::new(input);
        // NFC grows a string by at most three times; reserving that up front
        // keeps the normalized copy from being reallocated, which would leave
        // an unwiped copy in freed memory.
        let mut normalized = Zeroizing::new(String::with_capacity(input.len() * 3));
        normalized.extend(input.nfc());
        Self(normalized)
    }

    /// The normalized passphrase, for `PRAGMA key` and `ATTACH … KEY` only.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The rules for a passphrase being set (create, change): at least
/// [`MIN_PASSPHRASE_CHARS`] Unicode scalar values after normalization, and
/// no NUL, which cannot pass through SQLite's C string API. Checking an
/// existing passphrase never applies these: whatever opens the file is right.
pub fn validate_new_passphrase(passphrase: &Passphrase) -> Result<(), CommandError> {
    let text = passphrase.as_str();
    let problem = if text.contains('\0') {
        Some("A passphrase can't contain a NUL character.")
    } else if text.chars().count() < MIN_PASSPHRASE_CHARS {
        Some("Use at least 12 characters.")
    } else {
        None
    };
    match problem {
        None => Ok(()),
        Some(message) => Err(CommandError::validation(
            "Check the passphrase.",
            HashMap::from([("passphrase".to_owned(), message.to_owned())]),
        )),
    }
}
