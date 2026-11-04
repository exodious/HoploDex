//! The strength hint shown below a new passphrase as it is typed (003 FR-003):
//! the advice, always, and a five-step meter with a label and zxcvbn's
//! suggestion once something is typed. It only advises: a weak passphrase is
//! never refused for being weak.
//!
//! The estimate is the `zxcvbn` crate's (Dropbox's algorithm, with its own
//! word lists), worked out in the app process, since the passphrase is typed
//! only in the native dialog (amended 2026-11-04 to 003 FR-003 and 010 FR-015).
//! The crate copies the passphrase and its pieces into buffers of its own
//! while it scores, which are freed without being wiped
//! (`docs/attack-surface.md`, `key-passphrase`).

/// The meter's label for each score.
pub const LABELS: [&str; 5] = ["Very weak", "Weak", "Fair", "Strong", "Very strong"];

/// Shown below the new passphrase whatever it is.
pub const ADVICE: &str = "Longer is stronger: several unrelated words make a good passphrase.";

/// How strong a passphrase looks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estimate {
    /// 0 (very weak) to 4 (very strong).
    pub score: u8,
    /// zxcvbn's warning, or its first suggestion, or nothing.
    pub suggestion: String,
}

impl Estimate {
    /// The meter's label.
    pub fn label(&self) -> &'static str {
        LABELS[usize::from(self.score.min(4))]
    }
}

/// How much of a passphrase is scored.
const SCORED_CHARS: usize = 100;

/// Builds zxcvbn's dictionaries, which the first estimate would otherwise
/// do while the user types, off the thread that asks.
pub fn warm_up() {
    std::thread::spawn(|| drop(estimate("warm up")));
}

/// The estimate for `passphrase`, or `None` for an empty one.
pub fn estimate(passphrase: &str) -> Option<Estimate> {
    if passphrase.is_empty() {
        return None;
    }
    // shortcut: only the first 100 characters are scored, which keeps each
    // keystroke quick (zxcvbn's matching grows faster than the length); past
    // that a passphrase is long enough for the hint not to matter.
    let scored =
        passphrase.char_indices().nth(SCORED_CHARS).map_or(passphrase, |(at, _)| &passphrase[..at]);
    let entropy = zxcvbn::zxcvbn(scored, &[]);
    let feedback = entropy.feedback();
    let suggestion = feedback
        .and_then(|feedback| {
            feedback
                .warning()
                .map(|warning| warning.to_string())
                .or_else(|| feedback.suggestions().first().map(ToString::to_string))
        })
        .unwrap_or_default();
    Some(Estimate { score: u8::from(entropy.score()), suggestion })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hint tells a long but common passphrase from one of unrelated
    /// words (003 FR-003), and nothing is shown for an empty field.
    #[test]
    fn the_estimate_separates_common_passphrases_from_strong_ones() {
        assert_eq!(estimate(""), None);
        let weak = estimate("passwordpassword").expect("an estimate");
        assert!(weak.score <= 1, "{weak:?}");
        assert!(!weak.suggestion.is_empty());
        let strong = estimate("vivid otter ledger crane").expect("an estimate");
        assert_eq!(strong.score, 4, "{strong:?}");
        assert_eq!(strong.label(), "Very strong");
    }
}
