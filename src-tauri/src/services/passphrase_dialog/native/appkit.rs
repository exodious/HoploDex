//! The passphrase dialog on macOS: an AppKit alert, shown as a sheet on the
//! main window, its fields `NSSecureTextField`s (which turn on Secure Event
//! Input while focused, so no other process can read the keys). Its buttons
//! are retargeted, so pressing one doesn't end the sheet: the dialog stays up,
//! busy, while the backend tries what was typed.
//!
//! Each field's text is read into a buffer that is wiped straight after,
//! through `getCharacters:range:`; what AppKit keeps of it is the OS's own
//! input path. Everything here runs on the main thread. The dialog's state is
//! kept in a thread-local map by its id; the [`DialogWindow`] the caller gets
//! posts its `retry` and `close` there.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};

use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibilityElementProtocol, NSAlert, NSButton, NSColor, NSControlStateValueOn,
    NSControlTextEditingDelegate, NSLayoutAttribute, NSLevelIndicator, NSLevelIndicatorStyle,
    NSModalResponse, NSProgressIndicator, NSProgressIndicatorStyle, NSSecureTextField,
    NSStackView, NSTextField, NSTextFieldDelegate, NSUserInterfaceLayoutOrientation, NSView,
    NSWindow,
};
use objc2_foundation::{
    NSArray, NSNotification, NSObject, NSObjectNSDelayedPerforming, NSObjectProtocol, NSRange,
    NSString,
};
use zeroize::Zeroizing;

use super::{Controller, View};
use crate::services::passphrase::Passphrase;
use crate::services::passphrase_dialog::checks::{PAUSE, Texts};
use crate::services::passphrase_dialog::strength::ADVICE;
use crate::services::passphrase_dialog::{
    ACKNOWLEDGEMENT, Answer, DialogWindow, Event, Field, Request, Retry,
};

/// How wide the fields and the wrapping text are, in points.
const WIDTH: f64 = 320.0;

static IDS: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static DIALOGS: RefCell<HashMap<u64, Rc<Parts>>> = RefCell::new(HashMap::new());
}

fn find(id: u64) -> Option<Rc<Parts>> {
    DIALOGS.with(|dialogs| dialogs.borrow().get(&id).cloned())
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
            .ns_window()
            .map_err(|err| format!("no NSWindow: {err}"))
            .and_then(|parent| {
                let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
                // SAFETY: Tauri's pointer to the main window's NSWindow, which
                // outlives this call; it is retained before use.
                let parent = unsafe { Retained::retain(parent.cast::<NSWindow>()) }
                    .ok_or("the main window has no NSWindow")?;
                build(mtm, id, parent, request, events);
                Ok(())
            });
        let _ = built.send(made);
    })
    .map_err(|err| format!("could not reach the main thread: {err}"))?;
    result.recv().map_err(|_| "the main thread went away".to_owned())??;
    Ok(Box::new(AppKitWindow { app: app.clone(), id }))
}

/// The caller's handle: posts to the main thread.
struct AppKitWindow<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
    id: u64,
}

impl<R: tauri::Runtime> DialogWindow for AppKitWindow<R> {
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

struct TargetIvars {
    id: u64,
}

define_class!(
    /// Receives the dialog's actions and its fields' changes.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "HoploDexPassphraseDialogTarget"]
    #[ivars = TargetIvars]
    struct Target;

    impl Target {
        #[unsafe(method(submit:))]
        fn submit(&self, _sender: Option<&AnyObject>) {
            if let Some(parts) = find(self.ivars().id) {
                parts.submit();
            }
        }

        #[unsafe(method(cancel:))]
        fn cancel(&self, _sender: Option<&AnyObject>) {
            if let Some(parts) = find(self.ivars().id)
                && !parts.busy.get()
            {
                parts.send_end(Event::Cancelled);
            }
        }

        #[unsafe(method(toggleShow:))]
        fn toggle_show(&self, _sender: Option<&AnyObject>) {
            if let Some(parts) = find(self.ivars().id) {
                parts.toggle_show();
            }
        }

        #[unsafe(method(acknowledged:))]
        fn acknowledged(&self, _sender: Option<&AnyObject>) {
            if let Some(parts) = find(self.ivars().id) {
                parts.changed(None);
            }
        }

        #[unsafe(method(paused))]
        fn paused(&self) {
            if let Some(parts) = find(self.ivars().id) {
                let view = parts.with_texts(|texts| parts.controller.borrow_mut().paused(texts));
                parts.apply(&view);
            }
        }
    }

    unsafe impl NSObjectProtocol for Target {}

    unsafe impl NSControlTextEditingDelegate for Target {
        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, notification: &NSNotification) {
            let Some(parts) = find(self.ivars().id) else { return };
            let changed = notification.object();
            let field = parts.rows.iter().find_map(|row| {
                let ours = |view: &NSView| {
                    changed.as_deref().is_some_and(|object| {
                        std::ptr::eq(object as *const AnyObject, view as *const NSView as *const AnyObject)
                    })
                };
                (ours(&row.secure) || ours(&row.plain)).then_some(row.field)
            });
            parts.changed(field);
        }
    }

    unsafe impl NSTextFieldDelegate for Target {}
);

impl Target {
    fn new(mtm: MainThreadMarker, id: u64) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TargetIvars { id });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

/// One field: its secure and plain versions (one shows at a time, by the
/// Show checkbox) and the line under it.
struct Row {
    field: Field,
    secure: Retained<NSSecureTextField>,
    plain: Retained<NSTextField>,
    line: Retained<NSTextField>,
}

struct Parts {
    id: u64,
    alert: Retained<NSAlert>,
    parent: Retained<NSWindow>,
    rows: Vec<Row>,
    meter: Retained<NSLevelIndicator>,
    meter_label: Retained<NSTextField>,
    meter_row: Retained<NSStackView>,
    suggestion: Retained<NSTextField>,
    acknowledgement: Option<Retained<NSButton>>,
    show: Retained<NSButton>,
    busy_row: Retained<NSStackView>,
    spinner: Retained<NSProgressIndicator>,
    primary: Retained<NSButton>,
    cancel: Retained<NSButton>,
    target: Retained<Target>,
    events: Sender<Event>,
    controller: RefCell<Controller>,
    busy: Cell<bool>,
    shown: Cell<bool>,
    ended: Cell<bool>,
}

fn ns(text: &str) -> Retained<NSString> {
    NSString::from_str(text)
}

/// A label that wraps at the dialog's width.
fn wrapped(mtm: MainThreadMarker, text: &str) -> Retained<NSTextField> {
    let label = NSTextField::wrappingLabelWithString(&ns(text), mtm);
    label.setPreferredMaxLayoutWidth(WIDTH);
    label
}

fn width(view: &NSView, points: f64) {
    view.widthAnchor().constraintEqualToConstant(points).setActive(true);
}

fn stack(mtm: MainThreadMarker, vertical: bool, spacing: f64) -> Retained<NSStackView> {
    let stack = NSStackView::stackViewWithViews(&NSArray::new(), mtm);
    stack.setOrientation(if vertical {
        NSUserInterfaceLayoutOrientation::Vertical
    } else {
        NSUserInterfaceLayoutOrientation::Horizontal
    });
    stack.setSpacing(spacing);
    if vertical {
        stack.setAlignment(NSLayoutAttribute::Leading);
    }
    stack
}

fn build(
    mtm: MainThreadMarker,
    id: u64,
    parent: Retained<NSWindow>,
    request: Request,
    events: Sender<Event>,
) {
    let shape = request.shape;
    let target = Target::new(mtm, id);
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(&request.title));
    alert.setInformativeText(&ns(&request.message));
    let primary = alert.addButtonWithTitle(&ns(&request.primary));
    let cancel = alert.addButtonWithTitle(&ns("Cancel"));
    let any_target: &AnyObject = &target;
    // SAFETY: the target lives as long as the dialog (`Parts` holds it), and
    // both selectors are its own.
    unsafe {
        primary.setTarget(Some(any_target));
        primary.setAction(Some(sel!(submit:)));
        cancel.setTarget(Some(any_target));
        cancel.setAction(Some(sel!(cancel:)));
    }
    primary.setEnabled(false);

    let body = stack(mtm, true, 6.0);
    let meter = NSLevelIndicator::new(mtm);
    meter.setLevelIndicatorStyle(NSLevelIndicatorStyle::DiscreteCapacity);
    meter.setMinValue(0.0);
    meter.setMaxValue(5.0);
    width(&meter, 160.0);
    let meter_label = NSTextField::labelWithString(&ns(""), mtm);
    let meter_row = stack(mtm, false, 8.0);
    meter_row.addArrangedSubview(&meter);
    meter_row.addArrangedSubview(&meter_label);
    meter_row.setHidden(true);
    let suggestion = wrapped(mtm, "");
    suggestion.setTextColor(Some(&NSColor::secondaryLabelColor()));
    suggestion.setHidden(true);

    let mut rows = Vec::new();
    for &field in shape.fields() {
        let label = NSTextField::labelWithString(&ns(shape.label(field)), mtm);
        let secure = NSSecureTextField::new(mtm);
        let plain = NSTextField::new(mtm);
        for entry in [&**secure as &NSTextField, &*plain] {
            entry.setAccessibilityLabel(Some(&ns(shape.label(field))));
            entry.setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(&*target)));
            width(entry, WIDTH);
        }
        plain.setHidden(true);
        let line = wrapped(mtm, "");
        line.setTextColor(Some(&NSColor::systemRedColor()));
        line.setHidden(true);
        body.addArrangedSubview(&label);
        body.addArrangedSubview(&secure);
        body.addArrangedSubview(&plain);
        body.addArrangedSubview(&line);
        if field == Field::Passphrase && shape.has_strength() {
            body.addArrangedSubview(&meter_row);
            body.addArrangedSubview(&suggestion);
            let advice = wrapped(mtm, ADVICE);
            advice.setTextColor(Some(&NSColor::secondaryLabelColor()));
            body.addArrangedSubview(&advice);
        }
        rows.push(Row { field, secure, plain, line });
    }

    // SAFETY: the target and its selectors, as above.
    let show = unsafe {
        NSButton::checkboxWithTitle_target_action(
            &ns(shape.show_label()),
            Some(any_target),
            Some(sel!(toggleShow:)),
            mtm,
        )
    };
    body.addArrangedSubview(&show);
    let acknowledgement = shape.has_acknowledgement().then(|| {
        // SAFETY: as above.
        let check = unsafe {
            NSButton::checkboxWithTitle_target_action(
                &ns(ACKNOWLEDGEMENT),
                Some(any_target),
                Some(sel!(acknowledged:)),
                mtm,
            )
        };
        // A checkbox's title doesn't wrap: its cell is told to.
        check.cell().inspect(|cell| cell.setWraps(true));
        width(&check, WIDTH);
        body.addArrangedSubview(&check);
        check
    });

    let spinner = NSProgressIndicator::new(mtm);
    spinner.setStyle(NSProgressIndicatorStyle::Spinning);
    spinner.setDisplayedWhenStopped(false);
    let busy_row = stack(mtm, false, 8.0);
    busy_row.addArrangedSubview(&spinner);
    busy_row.addArrangedSubview(&NSTextField::labelWithString(&ns(&request.busy), mtm));
    busy_row.setHidden(true);
    body.addArrangedSubview(&busy_row);

    body.setFrameSize(body.fittingSize());
    alert.setAccessoryView(Some(&body));
    alert.layout();

    let parts = Rc::new(Parts {
        id,
        alert: alert.clone(),
        parent: parent.clone(),
        rows,
        meter,
        meter_label,
        meter_row,
        suggestion,
        acknowledgement,
        show,
        busy_row,
        spinner,
        primary,
        cancel,
        target,
        events,
        controller: RefCell::new(Controller::new(shape)),
        busy: Cell::new(false),
        shown: Cell::new(false),
        ended: Cell::new(false),
    });
    DIALOGS.with(|dialogs| dialogs.borrow_mut().insert(id, Rc::clone(&parts)));
    parts.changed(None);

    // However the sheet ends (the window closing included), the caller hears
    // of it.
    let ended = block2::RcBlock::new(move |_: NSModalResponse| {
        if let Some(parts) = DIALOGS.with(|dialogs| dialogs.borrow_mut().remove(&id)) {
            parts.send_end(Event::Cancelled);
        }
    });
    alert.beginSheetModalForWindow_completionHandler(&parent, Some(&ended));
    if let Some(first) = parts.rows.first() {
        alert.window().makeFirstResponder(Some(&first.secure));
    }
}

/// `text` (UTF-16) as UTF-8 in a buffer wiped when dropped, sized up front
/// so it is never reallocated.
fn decoded(text: &[u16]) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(text.len() * 3));
    out.extend(char::decode_utf16(text.iter().copied()).map(|c| c.unwrap_or('\u{FFFD}')));
    out
}

/// What `field` holds, copied once into a wiped buffer.
fn text_of(field: &NSTextField) -> Zeroizing<String> {
    autoreleasepool(|_| {
        let value = field.stringValue();
        let len = value.length();
        let mut units = Zeroizing::new(vec![0u16; len]);
        if let Some(buffer) = NonNull::new(units.as_mut_ptr()) {
            // SAFETY: `units` holds `len` UTF-16 units, the string's length.
            unsafe { value.getCharacters_range(buffer, NSRange::new(0, len)) };
        }
        decoded(&units)
    })
}

impl Parts {
    fn row(&self, field: Field) -> Option<&Row> {
        self.rows.iter().find(|row| row.field == field)
    }

    /// The field of `row` that shows now.
    fn visible<'a>(&self, row: &'a Row) -> &'a NSTextField {
        if self.shown.get() { &row.plain } else { &row.secure }
    }

    /// Runs `with` on the fields' text, copied into wiped buffers.
    fn with_texts<T>(&self, with: impl FnOnce(Texts<'_>) -> T) -> T {
        let text = |field| self.row(field).map(|row| text_of(self.visible(row)));
        let (current, passphrase, confirmation) =
            (text(Field::Current), text(Field::Passphrase), text(Field::Confirmation));
        let texts = Texts {
            current: current.as_deref().map(String::as_str),
            passphrase: passphrase.as_deref().map_or("", String::as_str),
            confirmation: confirmation.as_deref().map_or("", String::as_str),
            acknowledged: self
                .acknowledgement
                .as_ref()
                .is_some_and(|check| check.state() == NSControlStateValueOn),
        };
        with(texts)
    }

    /// A field changed (`None`: the acknowledgement): the checks run now, and
    /// again when the typing pauses.
    fn changed(&self, field: Option<Field>) {
        let view = self.with_texts(|texts| self.controller.borrow_mut().typed(field, texts));
        self.apply(&view);
        let target: &NSObject = &self.target;
        let any: &AnyObject = target;
        // SAFETY: the target's own selector, which takes no argument.
        unsafe {
            NSObject::cancelPreviousPerformRequestsWithTarget_selector_object(
                any,
                sel!(paused),
                None,
            );
            target.performSelector_withObject_afterDelay(sel!(paused), None, PAUSE.as_secs_f64());
        }
    }

    fn apply(&self, view: &View) {
        for row in &self.rows {
            let line = view.line(row.field);
            row.line.setStringValue(&ns(line.unwrap_or("")));
            row.line.setHidden(line.is_none());
        }
        match &view.estimate {
            Some(estimate) => {
                self.meter.setDoubleValue(f64::from(estimate.score) + 1.0);
                self.meter_label.setStringValue(&ns(estimate.label()));
                self.meter_row.setHidden(false);
                self.suggestion.setStringValue(&ns(&estimate.suggestion));
                self.suggestion.setHidden(estimate.suggestion.is_empty());
            }
            None => {
                self.meter_row.setHidden(true);
                self.suggestion.setHidden(true);
            }
        }
        self.primary.setEnabled(view.ready && !self.busy.get());
    }

    /// The primary button (or Return): what was typed goes to the caller,
    /// once, and the dialog waits, busy.
    fn submit(&self) {
        if self.busy.get() {
            return;
        }
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
        self.busy.set(busy);
        for row in &self.rows {
            row.secure.setEnabled(!busy);
            row.plain.setEnabled(!busy);
        }
        self.show.setEnabled(!busy);
        if let Some(check) = &self.acknowledgement {
            check.setEnabled(!busy);
        }
        self.cancel.setEnabled(!busy);
        self.busy_row.setHidden(!busy);
        // SAFETY: AppKit's own sender-less calls.
        unsafe {
            if busy {
                self.primary.setEnabled(false);
                self.spinner.startAnimation(None);
            } else {
                self.spinner.stopAnimation(None);
            }
        }
    }

    fn toggle_show(&self) {
        let shown = !self.shown.get();
        for row in &self.rows {
            let (from, to): (&NSTextField, &NSTextField) =
                if shown { (&row.secure, &row.plain) } else { (&row.plain, &row.secure) };
            autoreleasepool(|_| to.setStringValue(&from.stringValue()));
            from.setStringValue(&ns(""));
            from.setHidden(true);
            to.setHidden(false);
        }
        self.shown.set(shown);
        if let Some(first) = self.rows.first() {
            self.alert.window().makeFirstResponder(Some(self.visible(first)));
        }
    }

    fn retry(&self, retry: &Retry) {
        self.set_busy(false);
        if let Some(row) = self.row(retry.field) {
            row.secure.setStringValue(&ns(""));
            row.plain.setStringValue(&ns(""));
            self.alert.window().makeFirstResponder(Some(self.visible(row)));
        }
        let view = self.with_texts(|texts| self.controller.borrow_mut().retried(retry, texts));
        self.apply(&view);
    }

    fn send_end(&self, event: Event) {
        if !self.ended.replace(true) {
            let _ = self.events.send(event);
        }
    }

    /// Empties the fields and takes the sheet down.
    fn end(&self) {
        self.ended.set(true);
        let any: &AnyObject = &self.target;
        // SAFETY: the target, with no pending call wanted any more.
        unsafe { NSObject::cancelPreviousPerformRequestsWithTarget(any) };
        for row in &self.rows {
            row.secure.setStringValue(&ns(""));
            row.plain.setStringValue(&ns(""));
        }
        self.parent.endSheet(&self.alert.window());
        let _ = self.id;
    }
}

// The selector type is named so a wrong method signature fails to compile.
#[allow(dead_code)]
const _: Option<Sel> = None;
