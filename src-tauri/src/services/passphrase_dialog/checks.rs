//! The checks a new passphrase gets as it is typed (003 FR-003, contracts/
//! ui-databases.md §0), shared by every OS's dialog: what is wrong, whether
//! the primary button is enabled, and when a problem shows.
//!
//! The dialog hands the fields' text over borrowed, or in buffers it wipes
//! straight after: what is compared here is copied only into [`Zeroizing`]
//! buffers, and only the verdict leaves (003 FR-007).

use std::time::Duration;

use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use super::Shape;
use crate::services::passphrase::MIN_PASSPHRASE_CHARS;

/// How long typing pauses before a new problem shows. A problem put right
/// clears at once.
pub const PAUSE: Duration = Duration::from_millis(400);

pub const TOO_SHORT: &str = "Use at least 12 characters.";
pub const DIFFER: &str = "The passphrases don't match.";
pub const UNCHANGED: &str = "Choose a passphrase different from the current one.";
pub const NUL: &str = "A passphrase can't contain a NUL character.";

/// The fields as they are now.
#[derive(Debug, Clone, Copy, Default)]
pub struct Texts<'a> {
    /// The current passphrase, in a change.
    pub current: Option<&'a str>,
    /// The passphrase (open) or the new one (create, change).
    pub passphrase: &'a str,
    pub confirmation: &'a str,
    /// The acknowledgement is ticked (create).
    pub acknowledged: bool,
}

/// What is wrong, by field. The current passphrase has none of its own: it
/// is checked only when submitted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Problems {
    pub passphrase: Option<&'static str>,
    pub confirmation: Option<&'static str>,
}

/// What the fields allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    /// The primary button is enabled.
    pub ready: bool,
    pub problems: Problems,
}

/// The fields' verdict for a dialog of `shape`.
pub fn inspect(shape: Shape, texts: Texts<'_>) -> Verdict {
    let Shape::Choose { acknowledgement, .. } = shape else {
        return Verdict { ready: !texts.passphrase.is_empty(), problems: Problems::default() };
    };
    let current = texts.current.map(normalized);
    let passphrase = normalized(texts.passphrase);
    let confirmation = normalized(texts.confirmation);
    let long_enough = passphrase.chars().count() >= MIN_PASSPHRASE_CHARS;
    let unchanged = current.as_ref().is_some_and(|current| *current == passphrase);

    let mut problems = Problems::default();
    if passphrase.contains('\0') {
        problems.passphrase = Some(NUL);
    } else if !passphrase.is_empty() && unchanged {
        problems.passphrase = Some(UNCHANGED);
    } else if !confirmation.is_empty() && !long_enough {
        // Only once the confirmation is started: until then the strength hint
        // is advice enough, and a half-typed passphrase isn't an error.
        problems.passphrase = Some(TOO_SHORT);
    }
    // Any difference, a confirmation a character short included: the pause
    // before it shows is what lets the typing finish.
    if !confirmation.is_empty() && confirmation != passphrase {
        problems.confirmation = Some(DIFFER);
    }
    let ready = current.as_ref().is_none_or(|current| !current.is_empty())
        && long_enough
        && !passphrase.contains('\0')
        && !unchanged
        && confirmation == passphrase
        && (!acknowledgement || texts.acknowledged);
    Verdict { ready, problems }
}

fn normalized(text: &str) -> Zeroizing<String> {
    // NFC grows a string by at most three times: reserved up front so the
    // buffer is never reallocated, which would leave an unwiped copy.
    let mut out = Zeroizing::new(String::with_capacity(text.len() * 3));
    out.extend(text.nfc());
    out
}

/// When problems show: one already showing stays while it is still true,
/// a new one waits for [`PAUSE`] in the typing, and one put right goes at
/// once. The dialog calls [`typed`](Self::typed) on each change and
/// [`paused`](Self::paused) when a timer it restarts on each change fires.
#[derive(Debug, Default)]
pub struct Shown(Problems);

impl Shown {
    /// After a keystroke: the problems to show now.
    pub fn typed(&mut self, now: Problems) -> Problems {
        let keep = |shown: Option<&'static str>, now: Option<&'static str>| {
            if shown == now { shown } else { None }
        };
        self.0 = Problems {
            passphrase: keep(self.0.passphrase, now.passphrase),
            confirmation: keep(self.0.confirmation, now.confirmation),
        };
        self.0
    }

    /// The typing paused: everything found shows.
    pub fn paused(&mut self, now: Problems) -> Problems {
        self.0 = now;
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREATE: Shape = Shape::Choose { current: false, acknowledgement: true };
    const CHANGE: Shape = Shape::Choose { current: true, acknowledgement: false };

    fn texts<'a>(passphrase: &'a str, confirmation: &'a str) -> Texts<'a> {
        Texts { current: None, passphrase, confirmation, acknowledged: true }
    }

    #[test]
    fn an_existing_passphrase_needs_only_to_be_there() {
        assert!(!inspect(Shape::Enter, Texts::default()).ready);
        assert!(inspect(Shape::Enter, texts("x", "")).ready);
    }

    /// 003 FR-003: twelve characters, counted after NFC, and typed twice.
    #[test]
    fn a_new_passphrase_needs_twelve_characters_and_its_confirmation() {
        let good = "correct horse";
        assert!(inspect(CREATE, texts(good, good)).ready);
        assert!(!inspect(CREATE, texts(good, "correct hors")).ready);
        assert!(!inspect(CREATE, texts("short", "short")).ready);
        // Eleven e-acutes typed decomposed are eleven characters, not 22.
        let decomposed = "e\u{301}".repeat(11);
        assert!(!inspect(CREATE, texts(&decomposed, &decomposed)).ready);
        let composed_twelve = "\u{e9}".repeat(12);
        let decomposed_twelve = "e\u{301}".repeat(12);
        assert!(inspect(CREATE, texts(&composed_twelve, &decomposed_twelve)).ready);
    }

    /// 003 FR-004: no database is created until the user says the passphrase
    /// is stored.
    #[test]
    fn creating_waits_for_the_acknowledgement() {
        let good = "correct horse";
        let unticked = Texts { acknowledged: false, ..texts(good, good) };
        assert!(!inspect(CREATE, unticked).ready);
        let no_ack = Shape::Choose { current: false, acknowledgement: false };
        assert!(inspect(no_ack, unticked).ready);
    }

    #[test]
    fn the_length_problem_waits_for_the_confirmation_to_be_started() {
        assert_eq!(inspect(CREATE, texts("short", "")).problems, Problems::default());
        assert_eq!(inspect(CREATE, texts("short", "s")).problems.passphrase, Some(TOO_SHORT));
    }

    #[test]
    fn any_difference_in_the_confirmation_is_a_mismatch() {
        let found = inspect(CREATE, texts("correct horse", "correct hors"));
        assert_eq!(found.problems.confirmation, Some(DIFFER));
        assert_eq!(found.problems.passphrase, None);
    }

    #[test]
    fn a_change_needs_the_current_passphrase_and_a_different_new_one() {
        let new = "a new passphrase";
        let change = |current| Texts { current: Some(current), ..texts(new, new) };
        assert!(inspect(CHANGE, change("the old one")).ready);
        assert!(!inspect(CHANGE, change("")).ready);
        let same = inspect(CHANGE, change(new));
        assert!(!same.ready);
        assert_eq!(same.problems.passphrase, Some(UNCHANGED));
    }

    #[test]
    fn a_nul_is_refused_as_it_is_typed() {
        let nul = "correct\0horse";
        let found = inspect(CREATE, texts(nul, nul));
        assert!(!found.ready);
        assert_eq!(found.problems.passphrase, Some(NUL));
    }

    /// 003 contracts/ui-databases.md §0: a new problem waits for the pause, one
    /// showing stays while it holds, and one put right clears at once.
    #[test]
    fn problems_show_after_a_pause_and_clear_at_once() {
        let mut shown = Shown::default();
        let differ = Problems { passphrase: None, confirmation: Some(DIFFER) };
        assert_eq!(shown.typed(differ), Problems::default());
        assert_eq!(shown.paused(differ), differ);
        assert_eq!(shown.typed(differ), differ);
        assert_eq!(shown.typed(Problems::default()), Problems::default());
        assert_eq!(shown.typed(differ), Problems::default());
    }
}
