//! The E2E build's passphrase dialog. No WebDriver can type into a native
//! dialog, so by default [`E2eDialog`] answers from a queue the spec fills:
//! the file named by `HOPLODEX_E2E_PASSPHRASES`, one JSON answer a line,
//! taken from the top. With `HOPLODEX_E2E_PASSPHRASE_DIALOG=native` the real
//! dialog shows instead, for the specs that type into it with real input,
//! wrapped in [`Logged`]. Either way each prompt, refusal, view and close is
//! appended to `HOPLODEX_E2E_PASSPHRASE_LOG` as a JSON line, for the specs to
//! read. Never in a release build (`scripts/check-no-webdriver.mjs`).
//!
//! An answer line: `{"passphrase": "…"}`, with `"current"` for a change,
//! `"confirmation"` when it should differ from the passphrase and
//! `"acknowledged": false` to leave the acknowledgement unticked; or
//! `{"cancel": true}`. With nothing queued the dialog cancels.

use std::io::Write;
use std::sync::Mutex;
use std::sync::mpsc::Sender;

use serde::Deserialize;
use serde_json::{Value, json};

use super::checks::{self, Texts};
use super::native::View;
use super::{Answer, DialogWindow, Event, Field, PassphraseDialog, Request, Retry, Shape};
use crate::services::passphrase::Passphrase;

/// Whether the E2E build shows the real dialog (the spec types into it).
pub fn native_requested() -> bool {
    std::env::var("HOPLODEX_E2E_PASSPHRASE_DIALOG").as_deref() == Ok("native")
}

/// Appends `line` to the log.
fn note(line: Value) {
    let Some(log) = std::env::var_os("HOPLODEX_E2E_PASSPHRASE_LOG") else { return };
    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .and_then(|mut file| writeln!(file, "{line}"));
    if let Err(err) = written {
        log::warn!("could not write the passphrase dialog's log: {err}");
    }
}

fn field_name(field: Field) -> &'static str {
    match field {
        Field::Current => "current",
        Field::Passphrase => "passphrase",
        Field::Confirmation => "confirmation",
    }
}

fn note_asked(request: &Request) {
    note(json!({ "asked": {
        "title": request.title,
        "message": request.message,
        "primary": request.primary,
        "fields": request.shape.fields().iter().map(|&f| request.shape.label(f)).collect::<Vec<_>>(),
    }}));
}

/// The real dialog is on the screen with the focus in its first field, so a
/// spec's keys reach it.
pub(crate) fn note_shown() {
    note(json!({ "shown": true }));
}

fn note_retry(retry: &Retry) {
    note(json!({ "retry": { "field": field_name(retry.field), "message": retry.message } }));
}

/// The last view noted, so a keystroke that changes nothing isn't.
static LAST_VIEW: Mutex<Option<Value>> = Mutex::new(None);

/// What the real dialog shows now: whether its button is enabled, the line
/// under each field and the strength label. Never the fields' text.
pub(super) fn note_view(view: &View) {
    let line = json!({ "view": {
        "ready": view.ready,
        "current": view.current,
        "passphrase": view.passphrase,
        "confirmation": view.confirmation,
        "strength": view.estimate.as_ref().map(|estimate| estimate.label()),
    }});
    let mut last = LAST_VIEW.lock().expect("view mutex poisoned");
    if last.as_ref() != Some(&line) {
        note(line.clone());
        *last = Some(line);
    }
}

/// The real dialog, logged.
pub struct Logged<D>(pub D);

impl<D: PassphraseDialog> PassphraseDialog for Logged<D> {
    fn open(
        &self,
        request: &Request,
        events: Sender<Event>,
    ) -> Result<Box<dyn DialogWindow>, String> {
        note_asked(request);
        *LAST_VIEW.lock().expect("view mutex poisoned") = None;
        let window = self.0.open(request, events)?;
        Ok(Box::new(LoggedWindow(window)))
    }
}

struct LoggedWindow(Box<dyn DialogWindow>);

impl DialogWindow for LoggedWindow {
    fn retry(&self, retry: &Retry) {
        note_retry(retry);
        self.0.retry(retry);
    }

    fn close(&self) {
        note(json!({ "closed": true }));
        self.0.close();
    }
}

/// One queued answer.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Queued {
    passphrase: String,
    current: Option<String>,
    confirmation: Option<String>,
    acknowledged: Option<bool>,
    cancel: bool,
}

/// Answers from the queue file.
pub struct E2eDialog;

/// Takes the top answer off the queue, if any.
fn take_queued() -> Option<Queued> {
    let path = std::env::var_os("HOPLODEX_E2E_PASSPHRASES")?;
    let text = std::fs::read_to_string(&path).ok()?;
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let first = lines.next()?;
    let rest: Vec<&str> = lines.collect();
    let rest = if rest.is_empty() { String::new() } else { format!("{}\n", rest.join("\n")) };
    if let Err(err) = std::fs::write(&path, rest) {
        log::warn!("could not take an answer off the passphrase queue: {err}");
    }
    match serde_json::from_str(first) {
        Ok(queued) => Some(queued),
        Err(err) => {
            log::warn!("a queued passphrase answer isn't JSON: {err}");
            None
        }
    }
}

/// What pressing the primary button with the top answer would send: the
/// checks a person meets in the real dialog are applied first, and a
/// refusal (the button stays disabled) is noted and cancels.
fn next_event(shape: Shape) -> Event {
    let Some(queued) = take_queued().filter(|queued| !queued.cancel) else {
        note(json!({ "cancelled": true }));
        return Event::Cancelled;
    };
    let confirmation = queued.confirmation.as_deref().unwrap_or(&queued.passphrase);
    let texts = Texts {
        current: shape
            .fields()
            .contains(&Field::Current)
            .then(|| queued.current.as_deref().unwrap_or_default()),
        passphrase: &queued.passphrase,
        confirmation,
        acknowledged: queued.acknowledged.unwrap_or(true),
    };
    let verdict = checks::inspect(shape, texts);
    if !verdict.ready {
        note(json!({ "refused": {
            "passphrase": verdict.problems.passphrase,
            "confirmation": verdict.problems.confirmation,
        }}));
        return Event::Cancelled;
    }
    Event::Submitted(Answer {
        passphrase: Passphrase::from_typed(texts.passphrase),
        current: texts.current.map(Passphrase::from_typed),
    })
}

impl PassphraseDialog for E2eDialog {
    fn open(
        &self,
        request: &Request,
        events: Sender<Event>,
    ) -> Result<Box<dyn DialogWindow>, String> {
        note_asked(request);
        let _ = events.send(next_event(request.shape));
        Ok(Box::new(E2eWindow { shape: request.shape, events }))
    }
}

struct E2eWindow {
    shape: Shape,
    events: Sender<Event>,
}

impl DialogWindow for E2eWindow {
    fn retry(&self, retry: &Retry) {
        note_retry(retry);
        let _ = self.events.send(next_event(self.shape));
    }

    fn close(&self) {
        note(json!({ "closed": true }));
    }
}
