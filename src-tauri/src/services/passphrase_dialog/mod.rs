//! The native passphrase dialog: the one place a passphrase is typed
//! (_amended 2026-11-04_ to 003 FR-007, contracts/ui-databases.md §0).
//!
//! The web view's processes keep copies of whatever is typed into a page,
//! and nothing in the page can wipe them, so a passphrase is never typed
//! there. The backend shows a dialog of the OS's own (GTK, AppKit, Win32),
//! modal to the main window, copies what was typed once into a
//! [`Passphrase`], and empties the control. No passphrase is ever an IPC
//! argument or result. Every word of the dialog is built here, from what the
//! backend knows, so a compromised page can't word it, as with
//! `services::consent`.
//!
//! [`PassphraseDialog`] is the seam: the app's [`NativeDialog`], an E2E
//! build's `E2eDialog` (which answers from a queue file), and the tests'
//! scripted dialog (`tests/passphrase_dialog_test.rs`). [`ask`] drives any of
//! them: the dialog stays up, busy, while the backend tries what was typed,
//! and comes back with the reason under the field when it is wrong.

pub mod checks;
#[cfg(feature = "e2e")]
pub mod e2e;
mod native;
pub mod strength;

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};

pub use native::NativeDialog;

use crate::services::passphrase::Passphrase;
use crate::session::{Session, SessionId};

/// The 003 FR-004 acknowledgement, ticked in the dialog that creates a
/// database.
pub const ACKNOWLEDGEMENT: &str = "I have stored this passphrase somewhere safe. If it is \
     forgotten, nobody, including HoploDex, can open this database or recover the collection.";

/// The label of a field in a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// The passphrase being changed (change only).
    Current,
    /// The passphrase typed to open something, or the new one being set.
    Passphrase,
    /// The new passphrase again.
    Confirmation,
}

/// What the dialog asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A passphrase that exists: one field.
    Enter,
    /// A new passphrase, typed twice and checked as it is typed (003 FR-003),
    /// after the current one when `current` is set (a change), and with the
    /// acknowledgement when `acknowledgement` is set (a create).
    Choose { current: bool, acknowledgement: bool },
}

impl Shape {
    /// Its fields, top to bottom.
    pub fn fields(self) -> &'static [Field] {
        match self {
            Shape::Enter => &[Field::Passphrase],
            Shape::Choose { current: false, .. } => &[Field::Passphrase, Field::Confirmation],
            Shape::Choose { current: true, .. } => {
                &[Field::Current, Field::Passphrase, Field::Confirmation]
            }
        }
    }

    /// The label above `field`.
    pub fn label(self, field: Field) -> &'static str {
        let change = matches!(self, Shape::Choose { current: true, .. });
        match (field, change) {
            (Field::Current, _) => "Current passphrase",
            (Field::Passphrase, false) => "Passphrase",
            (Field::Passphrase, true) => "New passphrase",
            (Field::Confirmation, false) => "Confirm passphrase",
            (Field::Confirmation, true) => "Confirm new passphrase",
        }
    }

    /// The label of the checkbox that shows what is typed.
    pub fn show_label(self) -> &'static str {
        if self.fields().len() == 1 { "Show passphrase" } else { "Show passphrases" }
    }

    /// Whether the strength hint shows below the new passphrase (003 FR-003).
    pub fn has_strength(self) -> bool {
        matches!(self, Shape::Choose { .. })
    }

    /// Whether the acknowledgement must be ticked.
    pub fn has_acknowledgement(self) -> bool {
        matches!(self, Shape::Choose { acknowledgement: true, .. })
    }
}

/// One dialog's words and fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub shape: Shape,
    /// The window's title.
    pub title: String,
    /// The text above the fields.
    pub message: String,
    /// The primary (default) button.
    pub primary: String,
    /// What the dialog says while the backend tries what was typed.
    pub busy: String,
}

/// What was typed, when the primary button was pressed: each field copied
/// once, NFC-normalized, and wiped when dropped. The confirmation isn't
/// passed on: the button is enabled only while it matches.
pub struct Answer {
    /// The passphrase typed to open something, or the new one.
    pub passphrase: Passphrase,
    /// The current passphrase, for a change.
    pub current: Option<Passphrase>,
}

/// Why what was typed didn't do: the dialog shows `message` under `field`,
/// empties that field and puts the focus in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retry {
    pub field: Field,
    pub message: String,
}

/// What [`ask`]'s step decided about an answer.
pub enum Step {
    /// Done with the dialog: it closes.
    Done,
    /// Ask again, saying why.
    Retry(Retry),
}

/// What happened in the dialog, or to it.
pub enum Event {
    /// The primary button, with every check passing. The dialog is busy
    /// until [`DialogWindow::retry`] or [`DialogWindow::close`].
    Submitted(Answer),
    /// Cancel, Escape or the window's close button.
    Cancelled,
    /// The system took it down: the screen locked, the computer is going to
    /// sleep, or the database it was for closed (003 FR-007).
    Dismissed,
}

/// How [`ask`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    /// A step said [`Step::Done`].
    Done,
    /// Cancelled, dismissed, or the dialog couldn't be shown.
    Cancelled,
}

/// A dialog on the screen.
pub trait DialogWindow: Send {
    /// Out of busy, with `retry`'s message under its field, that field
    /// emptied and focused.
    fn retry(&self, retry: &Retry);
    /// Empties every field and takes the dialog down. Called once, last.
    fn close(&self);
}

/// Shows passphrase dialogs.
pub trait PassphraseDialog: Send + Sync {
    /// Puts the dialog for `request` up, modal to the main window with the
    /// focus in its first field, and returns at once: what the user does
    /// arrives on `events`, an [`Event::Cancelled`] included when the window
    /// goes away by any other route. `Err` when it can't be shown.
    fn open(
        &self,
        request: &Request,
        events: Sender<Event>,
    ) -> Result<Box<dyn DialogWindow>, String>;
}

/// The dialogs on the screen, so the system can take them down (003 FR-007):
/// every one at a screen lock or a sleep, and those for an open database when
/// it closes. Held by the [`Session`].
#[derive(Default)]
pub struct Prompts {
    up: Mutex<Vec<Registered>>,
    next: AtomicU64,
}

struct Registered {
    id: u64,
    bound: Option<SessionId>,
    events: Sender<Event>,
}

impl Prompts {
    fn register(&self, bound: Option<SessionId>, events: Sender<Event>) -> Registration<'_> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.lock().push(Registered { id, bound, events });
        Registration { prompts: self, id }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Registered>> {
        self.up.lock().expect("prompts mutex poisoned")
    }

    /// Takes down every dialog: the screen locked or the computer is going
    /// to sleep.
    pub fn dismiss_all(&self) {
        for up in self.lock().iter() {
            let _ = up.events.send(Event::Dismissed);
        }
    }

    /// Takes down the dialogs asked for the open database `id`, which is
    /// closing.
    pub fn dismiss_for(&self, id: SessionId) {
        for up in self.lock().iter().filter(|up| up.bound == Some(id)) {
            let _ = up.events.send(Event::Dismissed);
        }
    }

    /// How many dialogs are up, for the tests.
    pub fn count(&self) -> usize {
        self.lock().len()
    }
}

struct Registration<'a> {
    prompts: &'a Prompts,
    id: u64,
}

impl Drop for Registration<'_> {
    fn drop(&mut self) {
        self.prompts.lock().retain(|up| up.id != self.id);
    }
}

/// Shows `request` and hands each answer to `step`, until a step is done
/// or the user cancels. `bound` is the open database the dialog is for (a
/// change, saving the passphrase), whose close takes it down. The idle lock
/// waits while it is up, as for a file chooser (007 research.md §16). Blocks,
/// so never on an async thread or the main thread.
pub fn ask(
    session: &Session,
    dialog: &dyn PassphraseDialog,
    bound: Option<SessionId>,
    request: &Request,
    mut step: impl FnMut(Answer) -> Step,
) -> Asked {
    let _paused = crate::commands::documents::ops::IdlePause::new(session);
    if request.shape.has_strength() {
        strength::warm_up();
    }
    let (events, answers) = mpsc::channel();
    let _registered = session.prompts().register(bound, events.clone());
    let window = match dialog.open(request, events) {
        Ok(window) => window,
        Err(err) => {
            log::error!("could not show the passphrase dialog: {err}");
            return Asked::Cancelled;
        }
    };
    loop {
        match answers.recv() {
            Ok(Event::Submitted(answer)) => match step(answer) {
                Step::Done => {
                    window.close();
                    return Asked::Done;
                }
                Step::Retry(retry) => window.retry(&retry),
            },
            Ok(Event::Cancelled | Event::Dismissed) | Err(_) => {
                window.close();
                return Asked::Cancelled;
            }
        }
    }
}
