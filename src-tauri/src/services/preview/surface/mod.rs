//! The PDF surface: a child web view in the main window and every per-OS
//! measure on it (research.md §4-§10). One file per OS, as `platform/` does.
//!
//! [`Surface`] is the web view itself, with the settings every OS shares (its
//! label, no capability naming it, `incognito`, a proxy to a loopback port the
//! caller holds, navigation and new windows denied but for its own document,
//! downloads refused, developer tools off in a release build, the frame script
//! with the surface's secret in it), its own browser process on Windows, and
//! its bounds. [`AppSurface`] wraps it as the app's [`PreviewSurface`]. The
//! filters, settings, signals and input monitors that research.md §6-§10 add
//! per OS go in `linux.rs`, `macos.rs` and `windows.rs`; they reach the app
//! through [`Hooks`].
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
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use tauri::{
    LogicalPosition, LogicalSize, Runtime, Url, Webview, WebviewUrl, Window,
    webview::{DownloadEvent, NewWindowResponse, PageLoadEvent, WebviewBuilder},
};

use super::{PdfEndReason, PreviewSurface, SurfaceSpec};

/// Script run in every frame of the surface (research.md §8, §10), with
/// [`SECRET_PLACEHOLDER`] replaced by the surface's secret.
const FRAME_SCRIPT: &str = include_str!("frame_script.js");

/// What `frame_script.js` holds the surface's secret as.
const SECRET_PLACEHOLDER: &str = "__HOPLODEX_SURFACE_SECRET__";

/// The most often the surface's input counts as activity for the idle lock
/// (research.md §10).
const ACTIVITY_INTERVAL: Duration = Duration::from_secs(1);

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

type Action = Box<dyn Fn() + Send + Sync + 'static>;

/// What the surface tells the app, which the per-OS code calls (research.md
/// §7, §10). The calls may come from the main thread or any other, so each
/// must return at once.
pub struct Hooks {
    on_escape: Action,
    on_focus_chrome: Action,
    on_activity: Action,
    on_end: Box<dyn Fn(PdfEndReason) + Send + Sync + 'static>,
    on_download: Box<dyn Fn(&Url) + Send + Sync + 'static>,
    last_activity: Mutex<Option<Instant>>,
}

impl Hooks {
    pub fn new(
        on_escape: Action,
        on_focus_chrome: Action,
        on_activity: Action,
        on_end: Box<dyn Fn(PdfEndReason) + Send + Sync + 'static>,
        on_download: Box<dyn Fn(&Url) + Send + Sync + 'static>,
    ) -> Arc<Self> {
        Arc::new(Self {
            on_escape,
            on_focus_chrome,
            on_activity,
            on_end,
            on_download,
            last_activity: Mutex::new(None),
        })
    }

    /// Hooks that do nothing, for a check that doesn't run the app.
    pub fn none() -> Arc<Self> {
        Self::new(
            Box::new(|| {}),
            Box::new(|| {}),
            Box::new(|| {}),
            Box::new(|_| {}),
            Box::new(|_| {}),
        )
    }

    /// Escape was pressed in the surface (and consumed).
    pub fn escape(&self) {
        (self.on_escape)();
    }

    /// F6 was pressed in the surface (and consumed).
    pub fn focus_chrome(&self) {
        (self.on_focus_chrome)();
    }

    /// Input reached the surface: calls the idle clock's `note_activity`, at
    /// most once a second.
    pub fn activity(&self) {
        let mut last = self.last_activity.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if last.is_some_and(|at| now.duration_since(at) < ACTIVITY_INTERVAL) {
            return;
        }
        *last = Some(now);
        drop(last);
        (self.on_activity)();
    }

    /// The surface must close for `reason`: the preview ends and the viewer
    /// is told.
    pub fn end(&self, reason: PdfEndReason) {
        (self.on_end)(reason);
    }

    /// The web view asked to download `url`, which is refused whatever it is.
    pub fn download(&self, url: &Url) {
        (self.on_download)(url);
    }
}

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
    /// The 128-bit secret, as hex, written into the frame script
    /// (`GET /hooked/<secret>`, research.md §8).
    pub secret: String,
    pub hooks: Arc<Hooks>,
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
                .on_new_window(|_, _| NewWindowResponse::Deny)
                // Never a download: of the surface's own URL it is the sign
                // that the web view has no PDF viewer (research.md §4).
                .on_download({
                    let hooks = config.hooks.clone();
                    move |_, event| {
                        if let DownloadEvent::Requested { url, .. } = &event {
                            hooks.download(url);
                        }
                        false
                    }
                })
                .initialization_script_for_all_frames(
                    FRAME_SCRIPT.replace(SECRET_PLACEHOLDER, &config.secret),
                )
                // Off in a release build (research.md §8).
                .devtools(cfg!(debug_assertions));
        if let Some(hook) = config.on_page_load {
            builder = builder.on_page_load(move |_, payload| hook(payload.event(), payload.url()));
        }
        let builder = os::customize(builder, &config.proxy_url, &config.data_directory);
        let webview = window.add_child(
            builder,
            LogicalPosition::new(config.bounds.x, config.bounds.y),
            LogicalSize::new(config.bounds.width, config.bounds.height),
        )?;
        // A surface the OS couldn't set up (a filter that wouldn't install)
        // is closed, never left in the window.
        if let Err(e) = os::attach(window, &webview, &config.hooks) {
            let _ = webview.close();
            return Err(e);
        }
        let surface = Surface { webview, allowed };
        surface.set_bounds(config.bounds, false)?;
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

/// How many [`AppSurface`]s are open, so that the folder one leaves is never
/// deleted from under the next.
static OPEN_SURFACES: AtomicUsize = AtomicUsize::new(0);

/// Deletes the surface's user data folder, which holds no document content
/// (spike) but is the browser process's own on Windows: at startup, and after
/// the surface's browser process has exited (research.md §6). Not while a
/// surface is open: the next one uses the same folder.
pub fn clear_data_directory(folder: &Path) {
    if OPEN_SURFACES.load(Ordering::SeqCst) == 0 {
        let _ = std::fs::remove_dir_all(folder);
    }
}

/// The app's [`PreviewSurface`]: a [`Surface`] that closes with its last
/// holder (research.md §20).
pub struct AppSurface<R: Runtime> {
    surface: Mutex<Option<Surface<R>>>,
    data_directory: PathBuf,
}

impl<R: Runtime> AppSurface<R> {
    /// Builds a surface in `window` from `spec`, hidden, with the network
    /// going to `proxy_url`. Call it from a thread other than the main one:
    /// the web view is built there.
    pub fn build(
        window: &Window<R>,
        spec: SurfaceSpec,
        proxy_url: Url,
        data_directory: PathBuf,
        hooks: Arc<Hooks>,
    ) -> Result<Arc<dyn PreviewSurface>, String> {
        let url: Url = spec.url.parse().map_err(|e| format!("not a URL: {e}"))?;
        let config = SurfaceConfig {
            url,
            proxy_url,
            data_directory: data_directory.clone(),
            bounds: Rect {
                x: spec.bounds.x,
                y: spec.bounds.y,
                width: spec.bounds.width,
                height: spec.bounds.height,
            },
            secret: spec.secret,
            hooks,
            on_page_load: None,
        };
        let surface = Surface::open(window, config).map_err(|e| e.to_string())?;
        OPEN_SURFACES.fetch_add(1, Ordering::SeqCst);
        Ok(Arc::new(Self { surface: Mutex::new(Some(surface)), data_directory }))
    }

    fn with_surface(&self, f: impl FnOnce(&Surface<R>)) {
        if let Some(surface) = self.surface.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            f(surface);
        }
    }
}

impl<R: Runtime> PreviewSurface for AppSurface<R> {
    fn navigate(&self, url: &str) {
        match url.parse() {
            Ok(url) => self.with_surface(|surface| surface.navigate(url)),
            Err(e) => log::warn!("the preview surface was given a URL it can't parse: {e}"),
        }
    }

    fn set_bounds(&self, rect: Rect, visible: bool) {
        self.with_surface(|surface| {
            if let Err(e) = surface.set_bounds(rect, visible) {
                log::warn!("could not place the preview surface: {e}");
            }
        });
    }

    fn focus(&self) {
        self.with_surface(|surface| {
            if let Err(e) = surface.webview().set_focus() {
                log::warn!("could not focus the preview surface: {e}");
            }
        });
    }

    fn close(&self) {
        let Some(surface) = self.surface.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return;
        };
        // Closing is a message to the main thread, so it doesn't wait on it.
        if let Err(e) = surface.close() {
            log::warn!("could not close the preview surface: {e}");
        }
        OPEN_SURFACES.fetch_sub(1, Ordering::SeqCst);
        if cfg!(windows) {
            // The browser process takes a moment to exit and let go of its
            // folder.
            let folder = self.data_directory.clone();
            thread::spawn(move || {
                for _ in 0..20 {
                    thread::sleep(Duration::from_millis(500));
                    if OPEN_SURFACES.load(Ordering::SeqCst) != 0 {
                        return;
                    }
                    if std::fs::remove_dir_all(&folder).is_ok() || !folder.exists() {
                        return;
                    }
                }
            });
        }
    }
}

impl<R: Runtime> Drop for AppSurface<R> {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        s.parse().unwrap()
    }

    #[test]
    fn activity_is_passed_on_at_most_once_a_second() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let hooks = Hooks::new(
            Box::new(|| {}),
            Box::new(|| {}),
            Box::new(move || {
                counted.fetch_add(1, Ordering::SeqCst);
            }),
            Box::new(|_| {}),
            Box::new(|_| {}),
        );
        for _ in 0..50 {
            hooks.activity();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // A second later it counts again.
        *hooks.last_activity.lock().unwrap() = Some(Instant::now() - ACTIVITY_INTERVAL);
        hooks.activity();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn the_frame_script_has_its_secret_to_write_in_once() {
        assert_eq!(FRAME_SCRIPT.matches(SECRET_PLACEHOLDER).count(), 1);
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
