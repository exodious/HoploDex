use std::sync::Arc;

use axum::extract::State;
use serde_json::json;
use tauri::Runtime;

use super::response::{WebDriverResponse, WebDriverResult};
use super::AppState;

pub mod actions;
pub mod alert;
pub mod cookie;
pub mod direct_eval;
pub mod document;
pub mod element;
pub mod frame;
pub mod navigation;
pub mod print;
pub mod screenshot;
pub mod script;
pub mod session;
pub mod shadow;
pub mod timeouts;
pub mod window;

/// GET `/status` - `WebDriver` server status
///
/// Returns ready=true only when at least one webview window is available.
/// This ensures the WebView2/WebKit/WKWebView is fully initialized before
/// clients attempt to create sessions.
pub async fn status<R: Runtime>(state: State<Arc<AppState<R>>>) -> WebDriverResult {
    // Check if any webview windows are available
    // This is a proxy for WebView2 readiness - windows become available
    // only after the webview is fully initialized
    let window_labels = state.get_window_labels();
    // HoploDex patch (#77): and only once a page has finished loading, since
    // a script sent before that may never report back (macOS).
    let ready = !window_labels.is_empty()
        && super::PAGE_LOADED.load(std::sync::atomic::Ordering::SeqCst);

    Ok(WebDriverResponse::success(json!({
        "ready": ready,
        "message": if ready {
            "tauri-plugin-webdriver is ready"
        } else {
            "waiting for webview initialization"
        }
    })))
}
