//! The app's passphrase dialog, in each OS's own toolkit: GTK on Linux,
//! AppKit on macOS and Win32 on Windows. What it shows comes from
//! [`Controller`], shared by all three, so the rules and words are the same
//! everywhere and only the drawing differs.

#[cfg(target_os = "macos")]
mod appkit;
#[cfg(target_os = "linux")]
mod gtk;
#[cfg(windows)]
mod win32;

use std::sync::mpsc::Sender;

#[cfg(target_os = "macos")]
use appkit as platform;
#[cfg(target_os = "linux")]
use gtk as platform;
#[cfg(windows)]
use win32 as platform;

use super::checks::{self, Problems, Shown, Texts, Verdict};
use super::strength::{self, Estimate};
use super::{DialogWindow, Event, Field, PassphraseDialog, Request, Retry, Shape};

/// The OS's own dialog, modal to the app's main window.
pub struct NativeDialog<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> NativeDialog<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: tauri::Runtime> PassphraseDialog for NativeDialog<R> {
    fn open(
        &self,
        request: &Request,
        events: Sender<Event>,
    ) -> Result<Box<dyn DialogWindow>, String> {
        use tauri::Manager;

        let window = self.app.get_window("main").ok_or("the main window is gone")?;
        platform::open(&self.app, window, request.clone(), events)
    }
}

/// What a dialog shows, worked out from its fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct View {
    /// The primary button is enabled.
    pub ready: bool,
    /// The line under each field: a refusal from the last try, or a problem
    /// the checks found.
    pub current: Option<String>,
    pub passphrase: Option<String>,
    pub confirmation: Option<String>,
    /// The strength hint's estimate, for a new passphrase that has one.
    pub estimate: Option<Estimate>,
}

impl View {
    /// The line under `field`.
    pub fn line(&self, field: Field) -> Option<&str> {
        match field {
            Field::Current => self.current.as_deref(),
            Field::Passphrase => self.passphrase.as_deref(),
            Field::Confirmation => self.confirmation.as_deref(),
        }
    }
}

/// The state of one dialog that every OS's version keeps the same way: the
/// problems showing (which wait for a pause in the typing, 003 contracts/
/// ui-databases.md §0) and the last try's refusal, which stays under its
/// field until that field is typed in.
pub(crate) struct Controller {
    shape: Shape,
    shown: Shown,
    retry: Option<Retry>,
    estimate: Option<Estimate>,
}

impl Controller {
    pub fn new(shape: Shape) -> Self {
        Self { shape, shown: Shown::default(), retry: None, estimate: None }
    }

    /// `field` changed, or the acknowledgement was ticked (`None`).
    pub fn typed(&mut self, field: Option<Field>, texts: Texts<'_>) -> View {
        if field.is_some() && self.retry.as_ref().map(|retry| retry.field) == field {
            self.retry = None;
        }
        if field == Some(Field::Passphrase) {
            self.rate(texts);
        }
        let verdict = checks::inspect(self.shape, texts);
        let problems = self.shown.typed(verdict.problems);
        self.view(verdict, problems)
    }

    /// The typing paused.
    pub fn paused(&mut self, texts: Texts<'_>) -> View {
        let verdict = checks::inspect(self.shape, texts);
        let problems = self.shown.paused(verdict.problems);
        self.view(verdict, problems)
    }

    /// The last try was refused: `retry`'s field has been emptied.
    pub fn retried(&mut self, retry: &Retry, texts: Texts<'_>) -> View {
        self.retry = Some(retry.clone());
        if retry.field == Field::Passphrase {
            self.rate(texts);
        }
        let verdict = checks::inspect(self.shape, texts);
        let problems = self.shown.typed(verdict.problems);
        self.view(verdict, problems)
    }

    /// Whether the primary button may send these fields: the checks pass.
    pub fn ready(&self, texts: Texts<'_>) -> bool {
        checks::inspect(self.shape, texts).ready
    }

    /// Scores the new passphrase, only when it changed.
    fn rate(&mut self, texts: Texts<'_>) {
        self.estimate = (self.shape.has_strength() && !texts.passphrase.is_empty())
            .then(|| strength::estimate(texts.passphrase))
            .flatten();
    }

    fn view(&self, verdict: Verdict, problems: Problems) -> View {
        let refused = |field| {
            self.retry.as_ref().filter(|retry| retry.field == field).map(|r| r.message.clone())
        };
        let view = View {
            ready: verdict.ready,
            current: refused(Field::Current),
            passphrase: refused(Field::Passphrase)
                .or_else(|| problems.passphrase.map(str::to_owned)),
            confirmation: refused(Field::Confirmation)
                .or_else(|| problems.confirmation.map(str::to_owned)),
            estimate: self.estimate.clone(),
        };
        #[cfg(feature = "e2e")]
        super::e2e::note_view(&view);
        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREATE: Shape = Shape::Choose { current: false, acknowledgement: false };

    fn texts<'a>(passphrase: &'a str, confirmation: &'a str) -> Texts<'a> {
        Texts { current: None, passphrase, confirmation, acknowledged: false }
    }

    #[test]
    fn a_refusal_stays_under_its_field_until_that_field_is_typed_in() {
        let mut controller = Controller::new(Shape::Enter);
        let retry = Retry { field: Field::Passphrase, message: "That didn't open it.".to_owned() };
        let view = controller.retried(&retry, texts("", ""));
        assert_eq!(view.line(Field::Passphrase), Some("That didn't open it."));
        assert!(!view.ready);
        let view = controller.typed(Some(Field::Passphrase), texts("a", ""));
        assert_eq!(view.line(Field::Passphrase), None);
        assert!(view.ready);
    }

    #[test]
    fn a_problem_shows_once_the_typing_pauses() {
        let mut controller = Controller::new(CREATE);
        let typed = controller.typed(Some(Field::Confirmation), texts("correct horse", "c"));
        assert_eq!(typed.line(Field::Confirmation), None);
        let paused = controller.paused(texts("correct horse", "c"));
        assert_eq!(paused.line(Field::Confirmation), Some(checks::DIFFER));
        assert!(!paused.ready);
    }

    #[test]
    fn a_refusal_of_the_current_passphrase_leaves_the_new_one_alone() {
        let change = Shape::Choose { current: true, acknowledgement: false };
        let mut controller = Controller::new(change);
        let retry =
            Retry { field: Field::Current, message: "That isn't the current one.".to_owned() };
        let texts = Texts { current: Some(""), ..texts("a new passphrase", "a new passphrase") };
        let view = controller.retried(&retry, texts);
        assert_eq!(view.line(Field::Current), Some("That isn't the current one."));
        assert!(!view.ready);
        let view = controller.typed(Some(Field::Passphrase), texts);
        assert_eq!(view.line(Field::Current), Some("That isn't the current one."));
    }

    #[test]
    fn the_strength_hint_follows_the_new_passphrase_and_nothing_else() {
        let mut controller = Controller::new(CREATE);
        let weak = controller.typed(Some(Field::Passphrase), texts("password", "")).estimate;
        assert_eq!(weak, strength::estimate("password"));
        let confirming = controller.typed(Some(Field::Confirmation), texts("password", "pass"));
        assert_eq!(confirming.estimate, weak);
        assert_eq!(controller.paused(texts("password", "pass")).estimate, weak);
        let stronger = "correct horse battery staple";
        let view = controller.typed(Some(Field::Passphrase), texts(stronger, "pass"));
        assert_eq!(view.estimate, strength::estimate(stronger));
        assert_ne!(view.estimate, weak);
        let retry = Retry { field: Field::Passphrase, message: "Not that one.".to_owned() };
        assert_eq!(controller.retried(&retry, texts("", "pass")).estimate, None);
    }
}
