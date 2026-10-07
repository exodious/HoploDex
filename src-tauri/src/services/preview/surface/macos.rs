//! The surface on macOS: WKWebView's content rule list, switches, input
//! monitor and the copy watch (research.md §6, §7, §10).

use tauri::{Runtime, Url, Webview, Window, webview::WebviewBuilder};

use std::{path::Path, sync::Arc};

use super::{Hooks, Rect};

/// The proxy: `proxy_url` (Tauri's `macos-proxy` feature). Loopback bypasses
/// it on macOS, so the content filter, a later task, is the layer that
/// matters there (research.md §6).
pub(super) fn customize<R: Runtime>(
    builder: WebviewBuilder<R>,
    proxy_url: &Url,
    _data_directory: &Path,
) -> WebviewBuilder<R> {
    builder.proxy_url(proxy_url.clone())
}

/// Nothing to arrange: `add_child` places the web view at its bounds.
pub(super) fn attach<R: Runtime>(
    _window: &Window<R>,
    _webview: &Webview<R>,
    _hooks: &Arc<Hooks>,
) -> tauri::Result<()> {
    Ok(())
}

pub(super) fn place<R: Runtime>(
    webview: &Webview<R>,
    bounds: Rect,
    visible: bool,
) -> tauri::Result<()> {
    super::place_with_tauri(webview, bounds, visible)
}

pub(super) fn bounds<R: Runtime>(webview: &Webview<R>) -> tauri::Result<Rect> {
    super::bounds_from_tauri(webview)
}
