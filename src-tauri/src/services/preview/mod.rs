//! Document preview (feature 007, research.md §20): the `hdpreview`
//! protocol, the PDF surface, the TIFF render helper, text, and the
//! availability rules that decide which of them a document gets.
//!
//! This file holds what the session keeps of an open preview ([`Preview`]
//! and [`PreviewContent`], data-model.md "Preview"), and the traits the
//! preview commands reach the web view through ([`PreviewSurface`],
//! [`SurfaceFactory`]), so the session's tests run with a recorder where the
//! app has a child web view.

pub mod availability;
pub mod confine;
pub mod helper;
pub mod helper_handle;
#[cfg(windows)]
pub mod helper_job;
pub mod helper_protocol;
pub mod protocol_handler;
#[cfg(target_os = "linux")]
pub mod sandbox_probe;
pub mod surface;
pub mod text;
pub mod tiff;
pub mod tripwire;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub use helper_handle::{HelperError, HelperHandle, HelperLimits, HelperShared};
use surface::Rect;

/// A TIFF page's size in points (1/72 in), at the page's own DPI (200 when
/// it has none).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PageSize {
    pub width: f64,
    pub height: f64,
}

/// The PDF surface's place in the window, as the frontend sends it (logical
/// pixels from the window's content area).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SurfaceBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl SurfaceBounds {
    /// Finite, not negative, and at least 1 px each way.
    pub fn is_valid(&self) -> bool {
        [self.x, self.y, self.width, self.height].iter().all(|v| v.is_finite())
            && self.x >= 0.0
            && self.y >= 0.0
            && self.width >= 1.0
            && self.height >= 1.0
    }
}

impl From<SurfaceBounds> for Rect {
    fn from(b: SurfaceBounds) -> Self {
        Rect { x: b.x, y: b.y, width: b.width, height: b.height }
    }
}

/// Why Rust closed the PDF surface (`preview:pdf-ended`, contracts/tauri-commands.md
/// "Shapes").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PdfEndReason {
    /// macOS: a copy was written and deleted; PDF preview is off on this
    /// computer until HoploDex is updated (FR-003a).
    CopyCaught,
    /// The web view has no PDF viewer: it turned the PDF into a download.
    NoViewer,
    /// The surface couldn't be set up safely, or PDF.js's hook didn't run.
    Failed,
}

/// What `open_preview` answers (contracts/tauri-commands.md "Shapes").
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", rename_all_fields = "camelCase")]
pub enum PreviewInfo {
    /// The surface shows it; wait for `preview:pdf-ready`.
    Pdf {
        preview_id: u64,
        document_id: i64,
    },
    Tiff {
        preview_id: u64,
        document_id: i64,
        pages: Vec<PageSize>,
    },
    Text {
        preview_id: u64,
        document_id: i64,
        text: String,
    },
}

/// The PDF surface, a child web view of the main window, as the preview
/// commands use it (research.md §20). The app's implementation wraps the
/// `tauri::Webview`; session tests use a recorder. `close` is also what its
/// `Drop` does.
pub trait PreviewSurface: Send + Sync {
    /// Shows another document.
    fn navigate(&self, url: &str);
    /// Moves and sizes the surface and shows or hides it.
    fn set_bounds(&self, rect: Rect, visible: bool);
    /// Gives it the keyboard focus.
    fn focus(&self);
    fn close(&self);
}

/// What a new surface is built on.
#[derive(Debug, Clone)]
pub struct SurfaceSpec {
    /// The first document's URL, the only one it may show.
    pub url: String,
    /// The fresh 128-bit secret written into the frame script
    /// (`GET /hooked/<secret>`, research.md §8).
    pub secret: String,
    /// Where it is, until the frontend says otherwise.
    pub bounds: SurfaceBounds,
}

/// Builds surfaces: the app's builds the child web view, the tests' record.
pub trait SurfaceFactory: Send + Sync {
    fn build(&self, spec: SurfaceSpec) -> Result<Arc<dyn PreviewSurface>, String>;
}

/// The one open preview of an open database. Gone with it at lock, close,
/// switch, sleep, shutdown or quit (research.md §20).
pub struct Preview {
    /// Counts up through the session, so a request for a replaced preview is
    /// told so.
    pub id: u64,
    pub document_id: i64,
    pub content: PreviewContent,
}

/// A preview whose helper is still loading the document, kept apart from
/// [`Preview`] so that `close_preview` and the next `open_preview` can end it
/// while the load runs without the session's lock (research.md §11, §20).
pub struct PreviewLoading {
    pub id: u64,
    /// Never reused, so a load that outlives its database can't install
    /// itself into the next one (whose ids start again).
    pub ticket: u64,
    pub document_id: i64,
    /// Set once the helper has started.
    pub helper: Option<Arc<HelperShared>>,
}

pub enum PreviewContent {
    Pdf {
        /// The URL's one-time path segment.
        token: [u8; 16],
        bytes: Zeroizing<Vec<u8>>,
        /// Shared with the viewer's next PDF; closed when the last holder
        /// goes.
        surface: Arc<dyn PreviewSurface>,
        /// The surface's secret (`SurfaceSpec::secret`), which the next PDF
        /// shown on the same surface inherits with it.
        secret: String,
        /// The surface has been given the document.
        served: bool,
        /// Linux: PDF.js's `webviewerloaded` hook reported (research.md §8).
        hooked: bool,
    },
    Tiff {
        /// The helper process, killed and waited for when this is dropped. It
        /// keeps the document's bytes for a restart.
        helper: HelperHandle,
        pages: Vec<PageSize>,
    },
    /// Nothing is kept: the decoded text was returned by `open_preview`.
    Text,
}
