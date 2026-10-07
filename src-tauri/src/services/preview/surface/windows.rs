//! The surface on Windows: WebView2's settings, browser arguments and
//! events (research.md §6-§10).

use tauri::{Runtime, Url, Webview, Window, webview::WebviewBuilder};

use std::{path::Path, sync::Arc};

use super::{Hooks, Rect};

/// wry's default browser arguments, which it adds only when an app gives
/// none (wry 0.55's `webview2/mod.rs`), so they are written out here.
const WRY_DEFAULT_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

/// The surface's browser arguments, in full (research.md §6): wry's defaults,
/// the proxy (which also takes loopback, and is the layer that covers Edge's
/// viewer frames, which the filter can't see) and WebRTC kept to the proxy.
/// `proxy_url` itself is not used, since wry would add it only to arguments
/// nobody gave.
fn browser_args(proxy_url: &Url) -> String {
    let proxy = format!(
        "{}:{}",
        proxy_url.host_str().unwrap_or("127.0.0.1"),
        proxy_url.port_or_known_default().unwrap_or(80)
    );
    format!(
        "{WRY_DEFAULT_ARGS} --proxy-server=http://{proxy} --proxy-bypass-list=<-loopback> \
         --webrtc-ip-handling-policy=disable_non_proxied_udp"
    )
}

/// A user data folder of its own, so the surface has a browser process of its
/// own: browser arguments belong to that process, which every web view
/// sharing a folder shares (research.md §6).
pub(super) fn customize<R: Runtime>(
    builder: WebviewBuilder<R>,
    proxy_url: &Url,
    data_directory: &Path,
) -> WebviewBuilder<R> {
    builder
        .data_directory(data_directory.to_path_buf())
        .additional_browser_args(&browser_args(proxy_url))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_args_hold_wrys_defaults_and_the_proxy() {
        let args = browser_args(&"http://127.0.0.1:4242".parse().unwrap());
        assert!(args.starts_with(WRY_DEFAULT_ARGS));
        assert!(args.contains("--proxy-server=http://127.0.0.1:4242 "));
        assert!(args.contains("--proxy-bypass-list=<-loopback>"));
        assert!(args.contains("--webrtc-ip-handling-policy=disable_non_proxied_udp"));
    }
}
