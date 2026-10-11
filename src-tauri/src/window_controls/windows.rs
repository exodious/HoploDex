//! Windows: the native browser controls (research.md §13, §14, §15).

use tauri::{Runtime, WebviewWindow};

/// Does nothing yet. T079 (specs/008-hardening-batch/tasks.md) implements it,
/// on Windows: `ICoreWebView2_11::add_ContextMenuRequested` keeping only
/// FR-011's items, browser accelerator keys and swipe navigation off. The
/// signature is the other OSes', so `window_controls::apply` calls it alike.
pub(crate) fn apply<R: Runtime>(_window: &WebviewWindow<R>, _log_items: bool) -> tauri::Result<()> {
    Ok(())
}
