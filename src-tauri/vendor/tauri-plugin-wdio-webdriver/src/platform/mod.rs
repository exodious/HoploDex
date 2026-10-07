pub(crate) mod alert_state;
mod executor;

pub use alert_state::AlertStateManager;
pub use executor::*;

#[cfg(target_os = "windows")]
pub use windows::AsyncScriptState;
#[cfg(target_os = "windows")]
pub use windows::ScriptExecutionLocks;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "ios")]
mod ios;

use std::ops::Deref;
use std::sync::Arc;
use tauri::Runtime;

use crate::webdriver::Timeouts;

/// A web view together with the window that holds it.
///
/// HoploDex patch (#79): upstream drives a `tauri::WebviewWindow`, and
/// `AppHandle::webview_windows()` leaves out any window that holds more than one
/// web view, so a window with a child web view disappeared from WebDriver. Tauri
/// can't build a `WebviewWindow` from a `Webview` and its `Window`, so this
/// carries both. Web view calls go through `Deref` to the `Webview`; the
/// window's own calls (size, position, state) go through `native_window()`.
#[derive(Debug, Clone)]
pub struct WebviewHandle<R: Runtime> {
    webview: tauri::Webview<R>,
    window: tauri::Window<R>,
}

impl<R: Runtime> WebviewHandle<R> {
    /// The web view labelled `label`, wherever its window holds others or not.
    pub fn find(app: &tauri::AppHandle<R>, label: &str) -> Option<Self> {
        use tauri::Manager;
        app.get_webview(label).map(Self::from_webview)
    }

    /// Every web view, by label.
    pub fn all(app: &tauri::AppHandle<R>) -> std::collections::HashMap<String, Self> {
        use tauri::Manager;
        app.webviews()
            .into_iter()
            .map(|(label, webview)| (label, Self::from_webview(webview)))
            .collect()
    }

    fn from_webview(webview: tauri::Webview<R>) -> Self {
        let window = webview.window();
        Self { webview, window }
    }

    /// The window holding this web view.
    pub fn native_window(&self) -> &tauri::Window<R> {
        &self.window
    }
}

impl<R: Runtime> Deref for WebviewHandle<R> {
    type Target = tauri::Webview<R>;

    fn deref(&self) -> &Self::Target {
        &self.webview
    }
}

/// Create a platform-specific executor for the given window
#[cfg(target_os = "macos")]
pub fn create_executor<R: Runtime + 'static>(
    window: WebviewHandle<R>,
    timeouts: Timeouts,
    frame_context: Vec<FrameId>,
) -> Arc<dyn PlatformExecutor<R>> {
    Arc::new(macos::MacOSExecutor::new(window, timeouts, frame_context))
}

/// Create a platform-specific executor for the given window
#[cfg(target_os = "windows")]
pub fn create_executor<R: Runtime + 'static>(
    window: WebviewHandle<R>,
    timeouts: Timeouts,
    frame_context: Vec<FrameId>,
) -> Arc<dyn PlatformExecutor<R>> {
    Arc::new(windows::WindowsExecutor::new(
        window,
        timeouts,
        frame_context,
    ))
}

/// Create a platform-specific executor for the given window
#[cfg(target_os = "linux")]
pub fn create_executor<R: Runtime + 'static>(
    window: WebviewHandle<R>,
    timeouts: Timeouts,
    frame_context: Vec<FrameId>,
) -> Arc<dyn PlatformExecutor<R>> {
    Arc::new(linux::LinuxExecutor::new(window, timeouts, frame_context))
}

/// Create a platform-specific executor for the given window
#[cfg(target_os = "android")]
pub fn create_executor<R: Runtime + 'static>(
    window: WebviewHandle<R>,
    timeouts: Timeouts,
    frame_context: Vec<FrameId>,
) -> Arc<dyn PlatformExecutor<R>> {
    Arc::new(android::AndroidExecutor::new(
        window,
        timeouts,
        frame_context,
    ))
}

/// Create a platform-specific executor for the given window
#[cfg(target_os = "ios")]
pub fn create_executor<R: Runtime + 'static>(
    window: WebviewHandle<R>,
    timeouts: Timeouts,
    frame_context: Vec<FrameId>,
) -> Arc<dyn PlatformExecutor<R>> {
    Arc::new(ios::IOSExecutor::new(window, timeouts, frame_context))
}

/// Register platform-specific webview handlers at webview creation time.
/// This is called from the plugin's `on_webview_ready` hook.
/// Note: Mobile platforms (Android/iOS) handle this via native plugins.
pub fn register_webview_handlers<R: Runtime>(webview: &tauri::Webview<R>) {
    #[cfg(target_os = "windows")]
    windows::register_webview_handlers(webview);
    #[cfg(target_os = "macos")]
    macos::register_webview_handlers(webview);
    #[cfg(target_os = "linux")]
    linux::register_webview_handlers(webview);

    let _ = webview; // Avoid unused variable warning on platforms without handlers
}

/// Start the macOS headless run-loop pump as early as possible — from the plugin's `setup` hook, on
/// the main thread — so it covers cold-start / deeplink navigation before the first webview is ready.
/// `Once`-guarded in `macos::start_runloop_pump`, so the `on_webview_ready` call remains a harmless
/// fallback. macOS only. See #540.
#[cfg(target_os = "macos")]
pub fn start_runloop_pump_early<R: Runtime>(app: &tauri::AppHandle<R>) {
    let _ = app.run_on_main_thread(|| unsafe { macos::start_runloop_pump() });
}
