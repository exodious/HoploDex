//! The files the user drops onto the main window, recorded for the open
//! session (code scanning alert `paths/cwe-73` on `add_document_from_path`):
//! `add_photo_from_path` and `add_document_from_path` read only those, so the
//! page can't name another file (`session::grants`).

use tauri::{Builder, DragDropEvent, Manager, Runtime, Window, WindowEvent};

use crate::session::Session;

/// The main window's label, the only one whose drops are recorded.
pub const MAIN_WINDOW: &str = "main";

/// Records the files of a drop onto the window labelled `label`: a drop onto
/// the main window, for the open session. Every other event is ignored.
pub fn record(session: &Session, label: &str, event: &DragDropEvent) {
    if let DragDropEvent::Drop { paths, .. } = event
        && label == MAIN_WINDOW
    {
        session.record_drop(paths.clone());
    }
}

fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if let WindowEvent::DragDrop(drop) = event {
        record(&window.state::<Session>(), window.label(), drop);
    }
}

/// Adds the drop recorder to the app. Recorded before the page hears of the
/// drop: Tauri only queues its event for the page.
pub fn register<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder.on_window_event(on_window_event)
}
