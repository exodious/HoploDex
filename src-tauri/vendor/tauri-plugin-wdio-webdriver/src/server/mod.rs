use std::net::SocketAddr;
use std::sync::Arc;

use tauri::{AppHandle, Runtime};
use tokio::runtime::Runtime as TokioRuntime;
use tokio::sync::RwLock;

pub mod handlers;
pub mod response;
pub mod router;

use crate::platform::{create_executor, FrameId, PlatformExecutor, WebviewHandle};
use crate::server::response::WebDriverErrorResponse;
use crate::webdriver::{SessionManager, Timeouts};

/// Shared state for the `WebDriver` server
pub struct AppState<R: Runtime> {
    pub app: AppHandle<R>,
    pub sessions: RwLock<SessionManager>,
}

impl<R: Runtime + 'static> AppState<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            sessions: RwLock::new(SessionManager::new()),
        }
    }

    /// Get a platform executor for a specific window by label
    pub fn get_executor_for_window(
        &self,
        window_label: &str,
        timeouts: Timeouts,
        frame_context: Vec<FrameId>,
    ) -> Result<Arc<dyn PlatformExecutor<R>>, WebDriverErrorResponse> {
        WebviewHandle::find(&self.app, window_label)
            .map(|window| create_executor(window, timeouts, frame_context))
            .ok_or_else(WebDriverErrorResponse::no_such_window)
    }

    /// Whether a window with this label is still registered
    pub fn has_window_label(&self, window_label: &str) -> bool {
        WebviewHandle::find(&self.app, window_label).is_some()
    }

    /// Get all window labels
    pub fn get_window_labels(&self) -> Vec<String> {
        WebviewHandle::all(&self.app).into_keys().collect()
    }
}

/// Start the `WebDriver` HTTP server on the specified port
pub fn start<R: Runtime + 'static>(app: AppHandle<R>, port: u16) {
    std::thread::spawn(move || {
        let rt = match TokioRuntime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("Failed to create Tokio runtime for WebDriver server: {}", e);
                return;
            }
        };

        rt.block_on(async {
            let state = Arc::new(AppState::new(app));
            let router = router::create_router(state);

            // On Android, bind to all interfaces for WiFi accessibility
            // On other platforms, bind to localhost only for security
            #[cfg(target_os = "android")]
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            #[cfg(not(target_os = "android"))]
            let addr = SocketAddr::from(([127, 0, 0, 1], port));

            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!(
                        "Failed to bind WebDriver server to {} — port may already be in use: {}",
                        addr, e
                    );
                    return;
                }
            };

            tracing::info!("WebDriver server listening on http://{}", addr);

            if let Err(e) = axum::serve(listener, router).await {
                tracing::error!("WebDriver server error: {}", e);
            }
        });
    });
}
