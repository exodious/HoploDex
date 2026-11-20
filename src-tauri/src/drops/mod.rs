//! The files the user drops onto the main window, recorded for the open
//! session (code scanning alert `paths/cwe-73` on `add_document_from_path`):
//! `add_photo_from_path` and `add_document_from_path` read only those, so the
//! page can't name another file (`session::grants`).
//!
//! The main web view is a child of its window (Tauri's `unstable` feature),
//! so a drop reaches the app as the web view's `DragDrop` event, never the
//! window's. A drag that starts in the app itself, in the page or in another
//! of its web views, is not a drop of the user's files: each OS's module
//! keeps such a drag from reaching the recorder with a path
//! ([`refuse_app_drags`]).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use tauri::{Builder, DragDropEvent, Manager, Runtime, Webview, WebviewEvent, WebviewWindow};

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

fn on_webview_event<R: Runtime>(webview: &Webview<R>, event: &WebviewEvent) {
    if let WebviewEvent::DragDrop(drop) = event {
        record(&webview.state::<Session>(), webview.label(), drop);
    }
}

/// Adds the drop recorder to the app. Recorded before the page hears of the
/// drop: Tauri only queues its event for the page.
pub fn register<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder.on_webview_event(on_webview_event)
}

/// Keeps a drag that starts in the app from reaching the recorder with a
/// path, on every web view of the process. Called once from `setup()`, after
/// the main window exists.
pub fn refuse_app_drags<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let _ = window;
        linux::refuse_app_drags();
        Ok(())
    }
    #[cfg(target_os = "macos")]
    return macos::refuse_app_drags(window);
    #[cfg(windows)]
    return windows::refuse_app_drags(window);
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = window;
        Ok(())
    }
}
