//! The passphrase dialog on Linux: a GTK dialog, transient for and modal to
//! the main window. Each field is a `GtkEntry` with its visibility off,
//! whose buffer GTK wipes when text is deleted from it or it is freed. The
//! checks read the buffers' text in place (`gtk_entry_buffer_get_text`
//! returns the buffer's own storage), so nothing here copies a passphrase
//! but [`Passphrase::from_typed`] on submit.
//!
//! Everything here runs on the main thread, where GTK lives. The dialog's
//! state is kept in a thread-local map by its id; the [`DialogWindow`] the
//! caller gets posts its `retry` and `close` there.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};

use gtk::glib::translate::ToGlibPtr;
use gtk::prelude::*;
use gtk::{ResponseType, glib};

use super::{Controller, View};
use crate::services::passphrase::Passphrase;
use crate::services::passphrase_dialog::checks::{PAUSE, Texts};
use crate::services::passphrase_dialog::strength::ADVICE;
use crate::services::passphrase_dialog::{
    ACKNOWLEDGEMENT, Answer, DialogWindow, Event, Field, Request, Retry,
};

static IDS: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static DIALOGS: RefCell<HashMap<u64, Rc<Parts>>> = RefCell::new(HashMap::new());
}

/// One field: its label, entry and the line under it.
struct Row {
    field: Field,
    entry: gtk::Entry,
    line: gtk::Label,
}

/// One dialog's widgets and state.
struct Parts {
    id: u64,
    dialog: gtk::Dialog,
    rows: Vec<Row>,
    meter: gtk::LevelBar,
    meter_label: gtk::Label,
    meter_row: gtk::Box,
    suggestion: gtk::Label,
    acknowledgement: Option<gtk::CheckButton>,
    show: gtk::CheckButton,
    busy_row: gtk::Box,
    spinner: gtk::Spinner,
    events: Sender<Event>,
    controller: RefCell<Controller>,
    /// The pause timer, restarted on each change.
    timer: RefCell<Option<glib::SourceId>>,
    busy: RefCell<bool>,
    /// An event that ends the dialog has been sent, or `close` came.
    ended: RefCell<bool>,
}

pub(super) fn open<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    window: tauri::Window<R>,
    request: Request,
    events: Sender<Event>,
) -> Result<Box<dyn DialogWindow>, String> {
    let id = IDS.fetch_add(1, Ordering::Relaxed);
    let (built, result) = mpsc::channel();
    app.run_on_main_thread(move || {
        let made = window
            .gtk_window()
            .map_err(|err| format!("no GTK window: {err}"))
            .map(|parent| build(id, &parent, request, events));
        let _ = built.send(made);
    })
    .map_err(|err| format!("could not reach the main thread: {err}"))?;
    result.recv().map_err(|_| "the main thread went away".to_owned())??;
    Ok(Box::new(GtkWindow { app: app.clone(), id }))
}

/// The caller's handle: posts to the main thread.
struct GtkWindow<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
    id: u64,
}

impl<R: tauri::Runtime> DialogWindow for GtkWindow<R> {
    fn retry(&self, retry: &Retry) {
        let (id, retry) = (self.id, retry.clone());
        let _ = self.app.run_on_main_thread(move || {
            if let Some(parts) = find(id) {
                parts.retry(&retry);
            }
        });
    }

    fn close(&self) {
        let id = self.id;
        let _ = self.app.run_on_main_thread(move || {
            if let Some(parts) = DIALOGS.with(|dialogs| dialogs.borrow_mut().remove(&id)) {
                parts.end();
            }
        });
    }
}

fn find(id: u64) -> Option<Rc<Parts>> {
    DIALOGS.with(|dialogs| dialogs.borrow().get(&id).cloned())
}

/// The text in `entry`, borrowed from its buffer: valid until the buffer
/// changes, which nothing here does while it is held.
fn text_of(entry: &gtk::Entry) -> &str {
    let buffer = entry.buffer();
    // SAFETY: the buffer outlives the borrow (the entry holds it and the
    // caller holds the entry), and GTK returns its own NUL-terminated UTF-8.
    let raw = unsafe {
        let text = gtk::ffi::gtk_entry_buffer_get_text(buffer.to_glib_none().0);
        CStr::from_ptr(text)
    };
    raw.to_str().unwrap_or_default()
}

const CSS: &str = "label.hd-passphrase-line { color: @error_color; }";

fn build(id: u64, parent: &gtk::ApplicationWindow, request: Request, events: Sender<Event>) {
    let shape = request.shape;
    let dialog = gtk::Dialog::builder()
        .title(&request.title)
        .transient_for(parent)
        .modal(true)
        .destroy_with_parent(true)
        .resizable(false)
        .build();
    let provider = gtk::CssProvider::new();
    if let Err(err) = provider.load_from_data(CSS.as_bytes()) {
        log::warn!("the passphrase dialog's style didn't load: {err}");
    }
    dialog.add_button("Cancel", ResponseType::Cancel);
    let primary = dialog.add_button(&request.primary, ResponseType::Accept);
    primary.style_context().add_class("suggested-action");
    dialog.set_default_response(ResponseType::Accept);
    dialog.set_response_sensitive(ResponseType::Accept, false);

    let content = dialog.content_area();
    content.set_spacing(6);
    content.set_margin_top(18);
    content.set_margin_bottom(12);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let message = wrapped(&request.message);
    message.set_margin_bottom(12);
    content.pack_start(&message, false, false, 0);

    let mut rows = Vec::new();
    let meter = gtk::LevelBar::for_interval(0.0, 5.0);
    meter.set_mode(gtk::LevelBarMode::Discrete);
    meter.set_hexpand(true);
    let meter_label = gtk::Label::new(None);
    let meter_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    meter_row.pack_start(&meter, true, true, 0);
    meter_row.pack_start(&meter_label, false, false, 0);
    meter_row.set_no_show_all(true);
    let suggestion = wrapped("");
    suggestion.style_context().add_class("dim-label");
    suggestion.set_no_show_all(true);
    for &field in shape.fields() {
        let label = gtk::Label::with_mnemonic(&format!("_{}", shape.label(field)));
        label.set_xalign(0.0);
        let entry = gtk::Entry::new();
        entry.set_visibility(false);
        entry.set_input_purpose(gtk::InputPurpose::Password);
        entry.set_activates_default(true);
        entry.set_width_chars(40);
        label.set_mnemonic_widget(Some(&entry));
        let line = wrapped("");
        line.style_context().add_class("hd-passphrase-line");
        line.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        line.set_no_show_all(true);
        content.pack_start(&label, false, false, 0);
        content.pack_start(&entry, false, false, 0);
        content.pack_start(&line, false, false, 0);
        if field == Field::Passphrase && shape.has_strength() {
            content.pack_start(&meter_row, false, false, 0);
            content.pack_start(&suggestion, false, false, 0);
            let advice = wrapped(ADVICE);
            advice.style_context().add_class("dim-label");
            content.pack_start(&advice, false, false, 0);
        }
        rows.push(Row { field, entry, line });
    }

    let show = gtk::CheckButton::with_mnemonic(&format!("_{}", shape.show_label()));
    show.set_margin_top(6);
    content.pack_start(&show, false, false, 0);
    let acknowledgement = shape.has_acknowledgement().then(|| {
        let check = gtk::CheckButton::new();
        check.add(&wrapped(ACKNOWLEDGEMENT));
        content.pack_start(&check, false, false, 0);
        check
    });

    let spinner = gtk::Spinner::new();
    let busy_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    busy_row.pack_start(&spinner, false, false, 0);
    busy_row.pack_start(&wrapped(&request.busy), false, false, 0);
    busy_row.set_margin_top(6);
    busy_row.set_no_show_all(true);
    content.pack_start(&busy_row, false, false, 0);

    let parts = Rc::new(Parts {
        id,
        dialog: dialog.clone(),
        rows,
        meter,
        meter_label,
        meter_row,
        suggestion,
        acknowledgement,
        show,
        busy_row,
        spinner,
        events,
        controller: RefCell::new(Controller::new(shape)),
        timer: RefCell::new(None),
        busy: RefCell::new(false),
        ended: RefCell::new(false),
    });
    DIALOGS.with(|dialogs| dialogs.borrow_mut().insert(id, Rc::clone(&parts)));

    for row in &parts.rows {
        let field = row.field;
        row.entry.connect_changed(move |_| {
            if let Some(parts) = find(id) {
                parts.changed(Some(field));
            }
        });
    }
    if let Some(check) = &parts.acknowledgement {
        check.connect_toggled(move |_| {
            if let Some(parts) = find(id) {
                parts.changed(None);
            }
        });
    }
    parts.show.connect_toggled(move |show| {
        if let Some(parts) = find(id) {
            for row in &parts.rows {
                row.entry.set_visibility(show.is_active());
            }
        }
    });
    // Closing while the backend tries what was typed would leave it
    // working for nobody: the window's close button waits too.
    dialog.connect_delete_event(move |_, _| match find(id) {
        Some(parts) if *parts.busy.borrow() => glib::Propagation::Stop,
        _ => glib::Propagation::Proceed,
    });
    dialog.connect_response(move |_, response| {
        let Some(parts) = find(id) else { return };
        if *parts.busy.borrow() {
            return;
        }
        if response == ResponseType::Accept {
            parts.submit();
        } else {
            parts.send_end(Event::Cancelled);
        }
    });
    // However it goes (the parent closing included), the caller hears of it.
    dialog.connect_destroy(move |_| {
        if let Some(parts) = DIALOGS.with(|dialogs| dialogs.borrow_mut().remove(&id)) {
            parts.send_end(Event::Cancelled);
        }
    });

    parts.changed(None);
    if let Some(first) = parts.rows.first() {
        // The window has the keyboard once its first field does.
        first.entry.connect_focus_in_event(|_, _| {
            #[cfg(feature = "e2e")]
            crate::services::passphrase_dialog::e2e::note_shown();
            glib::Propagation::Proceed
        });
    }
    dialog.show_all();
    if let Some(first) = parts.rows.first() {
        first.entry.grab_focus();
    }
    dialog.present();
}

/// A label that wraps at the dialog's width and reads left to right.
fn wrapped(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_line_wrap(true);
    label.set_max_width_chars(60);
    label.set_xalign(0.0);
    label
}

impl Parts {
    fn entry(&self, field: Field) -> Option<&gtk::Entry> {
        self.rows.iter().find(|row| row.field == field).map(|row| &row.entry)
    }

    /// Runs `with` on the fields' text, borrowed in place.
    fn with_texts<T>(&self, with: impl FnOnce(Texts<'_>) -> T) -> T {
        let text = |field| self.entry(field).map(text_of);
        let texts = Texts {
            current: text(Field::Current),
            passphrase: text(Field::Passphrase).unwrap_or_default(),
            confirmation: text(Field::Confirmation).unwrap_or_default(),
            acknowledged: self.acknowledgement.as_ref().is_some_and(|check| check.is_active()),
        };
        with(texts)
    }

    /// A field changed (`None`: the acknowledgement): the checks run now,
    /// and again when the typing pauses.
    fn changed(&self, field: Option<Field>) {
        let view = self.with_texts(|texts| self.controller.borrow_mut().typed(field, texts));
        self.apply(&view);
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
        let id = self.id;
        let timer = glib::timeout_add_local_once(PAUSE, move || {
            if let Some(parts) = find(id) {
                parts.timer.borrow_mut().take();
                let view = parts.with_texts(|texts| parts.controller.borrow_mut().paused(texts));
                parts.apply(&view);
            }
        });
        *self.timer.borrow_mut() = Some(timer);
    }

    fn apply(&self, view: &View) {
        for row in &self.rows {
            match view.line(row.field) {
                Some(line) => {
                    row.line.set_text(line);
                    row.line.show();
                }
                None => {
                    row.line.set_text("");
                    row.line.hide();
                }
            }
        }
        match &view.estimate {
            Some(estimate) => {
                self.meter.set_value(f64::from(estimate.score) + 1.0);
                self.meter_label.set_text(estimate.label());
                self.meter_row.show_all();
                self.suggestion.set_text(&estimate.suggestion);
                self.suggestion.set_visible(!estimate.suggestion.is_empty());
            }
            None => {
                self.meter_row.hide();
                self.suggestion.hide();
            }
        }
        let busy = *self.busy.borrow();
        self.dialog.set_response_sensitive(ResponseType::Accept, view.ready && !busy);
    }

    /// The primary button: what was typed goes to the caller, once, and the
    /// dialog waits, busy.
    fn submit(&self) {
        let answer = self.with_texts(|texts| {
            self.controller.borrow().ready(texts).then(|| Answer {
                passphrase: Passphrase::from_typed(texts.passphrase),
                current: texts.current.map(Passphrase::from_typed),
            })
        });
        let Some(answer) = answer else { return };
        self.set_busy(true);
        let _ = self.events.send(Event::Submitted(answer));
    }

    fn set_busy(&self, busy: bool) {
        *self.busy.borrow_mut() = busy;
        for row in &self.rows {
            row.entry.set_sensitive(!busy);
        }
        self.show.set_sensitive(!busy);
        if let Some(check) = &self.acknowledgement {
            check.set_sensitive(!busy);
        }
        self.dialog.set_response_sensitive(ResponseType::Cancel, !busy);
        if busy {
            self.dialog.set_response_sensitive(ResponseType::Accept, false);
            self.spinner.start();
            self.busy_row.show_all();
        } else {
            self.spinner.stop();
            self.busy_row.hide();
        }
    }

    fn retry(&self, retry: &Retry) {
        self.set_busy(false);
        if let Some(entry) = self.entry(retry.field) {
            // GTK wipes the deleted text in the buffer.
            entry.set_text("");
            entry.grab_focus();
        }
        let view = self.with_texts(|texts| self.controller.borrow_mut().retried(retry, texts));
        self.apply(&view);
    }

    fn send_end(&self, event: Event) {
        if !self.ended.replace(true) {
            let _ = self.events.send(event);
        }
    }

    /// Empties the fields and takes the dialog down.
    fn end(&self) {
        self.ended.replace(true);
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
        for row in &self.rows {
            row.entry.set_text("");
        }
        // SAFETY: GTK's own call for a toplevel this module made and still
        // holds; nothing uses the dialog afterwards.
        unsafe { self.dialog.destroy() };
    }
}
