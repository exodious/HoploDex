//! The PDF surface: a child web view in the main window and every per-OS
//! measure on it (research.md §4-§10). One file per OS, as `platform/` does.
//!
//! This is the first part (T005): the web view itself, with the settings
//! every OS shares (its label, no capability naming it, `incognito`, a proxy
//! to a loopback port the caller holds, navigation and new windows denied
//! but for its own document), its own browser process on Windows, and its
//! bounds. The filters, settings, signals and input monitors that research.md
//! §6-§10 add per OS go in `linux.rs`, `macos.rs` and `windows.rs` as later
//! tasks reach them.
//!
//! Threading: `open` and `navigate` may be called from any thread except
//! inside a `with_webview` closure. Like the spike, the document's load
//! (`Webview::navigate`) is made from a thread of its own, since on macOS and
//! Windows it waits on a lock the main thread can hold.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as os;
#[cfg(target_os = "macos")]
use macos as os;
#[cfg(windows)]
use windows as os;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use tauri::{
    LogicalPosition, LogicalSize, Runtime, Url, Webview, WebviewUrl, Window,
    webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder},
};

/// Script run in every frame of the surface (research.md §8, §10).
#[allow(dead_code)]
const FRAME_SCRIPT: &str = include_str!("frame_script.js");

/// The surface's label (research.md §4). No capability names it (§5).
pub const LABEL: &str = "preview";

/// The surface's place in the main window, in logical pixels from the
/// window's top-left corner of its content area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Called when the surface's page starts and finishes loading, with the
/// page's URL.
pub type PageLoadHook = Box<dyn Fn(PageLoadEvent, &Url) + Send + Sync + 'static>;

/// What the caller decides about a surface.
pub struct SurfaceConfig {
    /// The first document's URL. The only URL the surface may show.
    pub url: Url,
    /// Where the surface's network goes: `http://127.0.0.1:<port>`, a port the
    /// caller holds (research.md §6, the tripwire).
    pub proxy_url: Url,
    /// Windows: the surface's own user data folder, so it has a browser
    /// process of its own (`app_dirs::preview_webview_data_dir`). Other OS
    /// ignore it.
    pub data_directory: PathBuf,
    pub bounds: Rect,
    pub on_page_load: Option<PageLoadHook>,
}

/// An open surface. Dropping it doesn't close the web view: call `close`.
pub struct Surface<R: Runtime> {
    webview: Webview<R>,
    /// The one URL the navigation handler allows.
    allowed: Arc<Mutex<Url>>,
}

/// `place` where Tauri's own geometry works (macOS and Windows).
#[cfg(not(target_os = "linux"))]
fn place_with_tauri<R: Runtime>(
    webview: &Webview<R>,
    bounds: Rect,
    visible: bool,
) -> tauri::Result<()> {
    webview.set_bounds(tauri::Rect {
        position: LogicalPosition::new(bounds.x, bounds.y).into(),
        size: LogicalSize::new(bounds.width, bounds.height).into(),
    })?;
    if visible { webview.show() } else { webview.hide() }
}

/// `bounds` where Tauri's own geometry works (macOS and Windows).
#[cfg(not(target_os = "linux"))]
fn bounds_from_tauri<R: Runtime>(webview: &Webview<R>) -> tauri::Result<Rect> {
    let scale = webview.window().scale_factor()?;
    let b = webview.bounds()?;
    let (p, s) = (b.position.to_logical::<f64>(scale), b.size.to_logical::<f64>(scale));
    Ok(Rect { x: p.x, y: p.y, width: s.width, height: s.height })
}

/// Whether a navigation of the surface is allowed: its current document,
/// `about:blank`, and on Linux the PDF.js viewer's own scheme (research.md
/// §6).
fn navigation_allowed(allowed: &Url, target: &Url) -> bool {
    target == allowed
        || target.as_str() == "about:blank"
        || (cfg!(target_os = "linux") && target.scheme() == "webkit-pdfjs-viewer")
}

impl<R: Runtime> Surface<R> {
    /// Adds the surface to `window` (the main window) at `config.bounds`,
    /// hidden until `set_bounds` shows it, and loads `config.url`.
    pub fn open(window: &Window<R>, config: SurfaceConfig) -> tauri::Result<Self> {
        let allowed = Arc::new(Mutex::new(config.url.clone()));
        let nav_allowed = allowed.clone();
        // Built blank, so the OS's filters and settings (later tasks) are in
        // place before the document's request is made.
        let mut builder =
            WebviewBuilder::new(LABEL, WebviewUrl::External("about:blank".parse().unwrap()))
                .incognito(true)
                .on_navigation(move |url| {
                    let allowed = nav_allowed.lock().unwrap_or_else(|e| e.into_inner());
                    navigation_allowed(&allowed, url)
                })
                .on_new_window(|_, _| NewWindowResponse::Deny);
        if let Some(hook) = config.on_page_load {
            builder = builder.on_page_load(move |_, payload| hook(payload.event(), payload.url()));
        }
        let builder = os::customize(builder, &config.proxy_url, &config.data_directory);
        let webview = window.add_child(
            builder,
            LogicalPosition::new(config.bounds.x, config.bounds.y),
            LogicalSize::new(config.bounds.width, config.bounds.height),
        )?;
        os::attach(window, &webview)?;
        let surface = Surface { webview, allowed };
        surface.navigate(config.url);
        Ok(surface)
    }

    /// Shows another document in the surface (the viewer moving from one PDF
    /// straight to another, research.md §4).
    pub fn navigate(&self, url: Url) {
        *self.allowed.lock().unwrap_or_else(|e| e.into_inner()) = url.clone();
        let webview = self.webview.clone();
        thread::spawn(move || {
            if let Err(e) = webview.navigate(url) {
                log::warn!("the preview surface could not load its document: {e}");
            }
        });
    }

    /// Places the surface over `bounds` and shows or hides it.
    pub fn set_bounds(&self, bounds: Rect, visible: bool) -> tauri::Result<()> {
        os::place(&self.webview, bounds, visible)
    }

    /// Where the surface is now, as the OS has placed it. Don't call it on
    /// the main thread.
    pub fn bounds(&self) -> tauri::Result<Rect> {
        os::bounds(&self.webview)
    }

    /// The surface's web view, for a check that wants to read its bounds or
    /// run a script in it. The app doesn't need it.
    pub fn webview(&self) -> &Webview<R> {
        &self.webview
    }

    /// Closes the web view.
    pub fn close(self) -> tauri::Result<()> {
        self.webview.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        s.parse().unwrap()
    }

    #[test]
    fn only_the_current_document_and_blank_may_load() {
        let doc = url("hdpreview://localhost/abc/document.pdf");
        assert!(navigation_allowed(&doc, &doc));
        assert!(navigation_allowed(&doc, &url("about:blank")));
        assert!(!navigation_allowed(&doc, &url("hdpreview://localhost/abc/other.pdf")));
        assert!(!navigation_allowed(&doc, &url("https://example.com/")));
        assert!(!navigation_allowed(&doc, &url("blob:hdpreview://localhost/abc")));
        assert_eq!(
            navigation_allowed(&doc, &url("webkit-pdfjs-viewer://pdfjs/web/viewer.html")),
            cfg!(target_os = "linux")
        );
    }
}
