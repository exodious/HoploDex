//! Preview commands (contracts/tauri-commands.md "Preview commands"). Each
//! is thin: it goes through the `Session`, then calls the matching function
//! in `ops`.
//!
//! The preview lives in the open database (`OpenDatabase::preview`), so it
//! ends with the database on every path (research.md §20). These commands
//! need no write, so unlike the collection's they aren't held up by pending
//! changes; they refuse with `DATABASE_CLOSED` when nothing is open.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::State;
use zeroize::Zeroizing;

use crate::commands::CommandError;
use crate::models::document_attachment::PreviewKind;
use crate::services::document_types::{classify, from_recorded};
use crate::services::preview::availability::{PdfAvailability, PdfAvailabilityState};
use crate::services::preview::{
    HelperError, HelperHandle, HelperLimits, Preview, PreviewContent, PreviewInfo, PreviewLoading,
    SurfaceBounds, SurfaceFactory, text,
};
use crate::session::{OpenDatabase, Session};

/// The longest PDF.js may take to report its hook on Linux before the
/// surface is closed (research.md §8).
const HOOK_TIMEOUT: Duration = Duration::from_secs(5);

/// What the preview commands need beyond the session: where the helper
/// executable is, how long it may take, how surfaces are built, and whether
/// PDFs are previewed on this computer. The app builds one at startup; tests
/// build theirs with a recording surface factory and the real helper.
#[derive(Clone)]
pub struct PreviewEnv {
    /// The executable started again as `--render-helper`.
    pub helper_exe: PathBuf,
    pub helper_limits: HelperLimits,
    /// How long a PDF may go without PDF.js's hook on Linux.
    pub hook_timeout: Duration,
    pub surfaces: Arc<dyn SurfaceFactory>,
    pub availability: Arc<PdfAvailabilityState>,
}

impl PreviewEnv {
    pub fn new(
        helper_exe: PathBuf,
        surfaces: Arc<dyn SurfaceFactory>,
        availability: Arc<PdfAvailabilityState>,
    ) -> Self {
        Self {
            helper_exe,
            helper_limits: HelperLimits::default(),
            hook_timeout: HOOK_TIMEOUT,
            surfaces,
            availability,
        }
    }
}

/// Which `Load` is which, across sessions: a ticket is never reused, so a
/// load that outlives its database can't install itself into the next one.
static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);

pub mod ops {
    use super::*;

    fn closed() -> CommandError {
        CommandError::new(
            "PREVIEW_CLOSED",
            "This preview has been closed. Open the document again to see it.",
        )
    }

    fn failed(message: &str) -> CommandError {
        CommandError::new("PREVIEW_FAILED", message)
    }

    fn invalid(message: &str) -> CommandError {
        CommandError::new("VALIDATION_ERROR", message)
    }

    fn unsupported() -> CommandError {
        CommandError::new(
            "PREVIEW_UNSUPPORTED",
            "HoploDex can't show this kind of document. Open it in another app instead.",
        )
    }

    /// Ends whatever preview or load `open` has, for the one that replaces it
    /// or the end of the viewer. Returns the preview that was open, so a PDF
    /// can hand its surface to the next one before it is dropped.
    fn end_current(open: &mut OpenDatabase) -> Option<Preview> {
        if let Some(loading) = open.preview_loading.take()
            && let Some(helper) = loading.helper
        {
            helper.shutdown();
        }
        open.preview.take()
    }

    /// What `open_preview` decided under the session's lock.
    enum Begun {
        Done(PreviewInfo),
        Tiff { id: u64, ticket: u64, bytes: Zeroizing<Vec<u8>> },
    }

    fn begin(
        open: &mut OpenDatabase,
        env: &PreviewEnv,
        document_id: i64,
    ) -> Result<Begun, CommandError> {
        // The viewer shows one document: the one before it goes first,
        // whatever happens to this request.
        let previous = end_current(open);
        let document = crate::commands::documents::ops::get_document(&open.conn, document_id)?;
        if from_recorded(&document.mime_type).is_none() {
            return Err(unsupported());
        }
        // The same content check every attach and open goes through, so a
        // row whose bytes changed since is refused here too.
        let kind = classify(&document.original_filename, &document.file_bytes)?
            .preview_kind
            .ok_or_else(unsupported)?;
        match kind {
            PreviewKind::Text => {
                let text = text::decode(&document.file_bytes);
                open.preview_seq += 1;
                let id = open.preview_seq;
                open.preview = Some(Preview { id, document_id, content: PreviewContent::Text });
                drop(previous);
                Ok(Begun::Done(PreviewInfo::Text { preview_id: id, document_id, text }))
            }
            PreviewKind::Tiff => {
                drop(previous);
                open.preview_seq += 1;
                let id = open.preview_seq;
                let ticket = NEXT_TICKET.fetch_add(1, Ordering::Relaxed);
                open.preview_loading =
                    Some(PreviewLoading { id, ticket, document_id, helper: None });
                Ok(Begun::Tiff { id, ticket, bytes: Zeroizing::new(document.file_bytes) })
            }
            PreviewKind::Pdf => {
                if let PdfAvailability::Unavailable { reason } = env.availability.get() {
                    return Err(CommandError::new("PDF_PREVIEW_UNAVAILABLE", reason.message()));
                }
                // Building the surface and serving the document are T061.
                Err(failed("PDF previews aren't ready in this build."))
            }
        }
    }

    /// Opens a document's preview, replacing any open one.
    ///
    /// The session's lock is held to read and classify the document and to
    /// install the preview, never while a TIFF's helper loads it: that can
    /// take 20 s, and `close_preview` must be able to end it at once.
    pub fn open_preview(
        session: &Arc<Session>,
        env: &PreviewEnv,
        document_id: i64,
    ) -> Result<PreviewInfo, CommandError> {
        let (id, ticket, bytes) = match session.inspect_mut(|open| begin(open, env, document_id))? {
            Begun::Done(info) => return Ok(info),
            Begun::Tiff { id, ticket, bytes } => (id, ticket, bytes),
        };

        let still_ours = |open: &OpenDatabase| {
            open.preview_loading.as_ref().is_some_and(|loading| loading.ticket == ticket)
        };
        // Whatever goes wrong from here, the load stops being the open one.
        let give_up = |error: CommandError| {
            let _ = session.inspect_mut(|open| {
                if still_ours(open) {
                    open.preview_loading = None;
                }
                Ok(())
            });
            error
        };

        let mut helper = HelperHandle::spawn(&env.helper_exe, env.helper_limits).map_err(|_| {
            give_up(failed("The TIFF viewer couldn't be started, so this document can't be shown."))
        })?;
        // From here `close_preview` can end the load by killing the helper.
        let registered = session.inspect_mut(|open| {
            Ok(match open.preview_loading.as_mut().filter(|l| l.ticket == ticket) {
                Some(loading) => {
                    loading.helper = Some(helper.share());
                    true
                }
                None => false,
            })
        })?;
        if !registered {
            return Err(closed());
        }

        let pages = match helper.load(&bytes) {
            Ok(pages) => pages,
            Err(HelperError::Refused(_)) => {
                return Err(give_up(CommandError::new(
                    "PREVIEW_DAMAGED",
                    "This TIFF is damaged, so it can't be shown. Open it in another app to try \
                     there.",
                )));
            }
            Err(HelperError::Closed) => return Err(closed()),
            Err(_) => {
                return Err(give_up(failed(
                    "This TIFF couldn't be read. It may be damaged or too large to show.",
                )));
            }
        };
        drop(bytes);

        let mut helper = Some(helper);
        let installed = session.inspect_mut(|open| {
            if !still_ours(open) {
                return Ok(false);
            }
            open.preview_loading = None;
            open.preview = Some(Preview {
                id,
                document_id,
                content: PreviewContent::Tiff {
                    helper: helper.take().expect("installed once"),
                    pages: pages.clone(),
                },
            });
            Ok(true)
        })?;
        if installed {
            Ok(PreviewInfo::Tiff { preview_id: id, document_id, pages })
        } else {
            Err(closed())
        }
    }

    /// Closes the preview `id`, ending a TIFF's load still under way. A stale
    /// id is ignored.
    pub fn close_preview(session: &Session, id: u64) -> Result<(), CommandError> {
        let ended = session.inspect_mut(|open| {
            let loading_is_it = open.preview_loading.as_ref().is_some_and(|l| l.id == id);
            let shown_is_it = open.preview.as_ref().is_some_and(|p| p.id == id);
            if loading_is_it || shown_is_it {
                return Ok(end_current(open));
            }
            Ok(None)
        })?;
        // A surface or helper that is dropped here is closed outside the
        // session's lock.
        drop(ended);
        Ok(())
    }

    /// Closes the preview of document `document_id`, if it is the one shown
    /// (what `delete_document` does first).
    pub fn close_preview_of(session: &Session, document_id: i64) -> Result<(), CommandError> {
        let ended = session.inspect_mut(|open| {
            let loading_is_it =
                open.preview_loading.as_ref().is_some_and(|l| l.document_id == document_id);
            let shown_is_it = open.preview.as_ref().is_some_and(|p| p.document_id == document_id);
            if loading_is_it || shown_is_it {
                return Ok(end_current(open));
            }
            Ok(None)
        })?;
        drop(ended);
        Ok(())
    }

    /// Moves and sizes the PDF surface and shows or hides it. Before the
    /// document has been served (and on Linux hooked) it stays hidden
    /// whatever `visible` says.
    pub fn set_preview_bounds(
        session: &Session,
        id: u64,
        bounds: SurfaceBounds,
        visible: bool,
    ) -> Result<(), CommandError> {
        session.inspect(|open| {
            let preview = open.preview.as_ref().filter(|p| p.id == id).ok_or_else(closed)?;
            let PreviewContent::Pdf { surface, served, hooked, .. } = &preview.content else {
                return Err(invalid("This preview isn't a PDF."));
            };
            if !bounds.is_valid() {
                return Err(invalid(
                    "The viewer's place must be a size of at least 1 pixel, inside the window.",
                ));
            }
            let ready = *served && (*hooked || !cfg!(target_os = "linux"));
            surface.set_bounds(bounds.into(), visible && ready);
            Ok(())
        })
    }

    /// Gives the PDF surface the keyboard focus.
    pub fn focus_preview(session: &Session, id: u64) -> Result<(), CommandError> {
        session.inspect(|open| {
            let preview = open.preview.as_ref().filter(|p| p.id == id).ok_or_else(closed)?;
            let PreviewContent::Pdf { surface, .. } = &preview.content else {
                return Err(invalid("This preview isn't a PDF."));
            };
            surface.focus();
            Ok(())
        })
    }

    /// The surface's `on_download` hook: a download of the surface's own URL
    /// means the web view has no PDF viewer. T061 ends the preview and turns
    /// PDF preview off for the run; until then nothing is done.
    pub fn surface_download(
        _session: &Arc<Session>,
        _env: &PreviewEnv,
        _url: &str,
    ) -> Result<(), CommandError> {
        Ok(())
    }

    /// One TIFF page as PNG bytes, `width_px` wide at most (4096 px, 24
    /// megapixels). The session's lock isn't held while the helper renders.
    pub fn render_preview_page(
        session: &Session,
        id: u64,
        page: u32,
        width_px: u32,
    ) -> Result<Vec<u8>, CommandError> {
        let helper = session.inspect(|open| {
            let preview = open.preview.as_ref().filter(|p| p.id == id).ok_or_else(closed)?;
            let PreviewContent::Tiff { helper, pages } = &preview.content else {
                return Err(invalid("This preview isn't a TIFF."));
            };
            if page as usize >= pages.len() {
                return Err(invalid("This TIFF has no such page."));
            }
            if width_px < 1 {
                return Err(invalid("A page must be at least 1 pixel wide."));
            }
            Ok(helper.share())
        })?;
        match helper.render(page, width_px.min(crate::services::preview::tiff::MAX_WIDTH_PX)) {
            Ok(png) => Ok(png),
            Err(HelperError::PageFailed) => Err(CommandError::new(
                "PREVIEW_PAGE_FAILED",
                "This page couldn't be shown. The others still can be.",
            )),
            Err(HelperError::Closed) => Err(closed()),
            Err(_) => {
                // The helper kept dying: end the preview.
                let _ = close_preview(session, id);
                Err(failed("This TIFF couldn't be shown. It may be damaged or too large."))
            }
        }
    }
}

/// Opens a document's preview in the viewer (PDF: on the surface; TIFF: the
/// pages' sizes; text: the text), replacing any open one.
#[tauri::command]
pub async fn open_preview(
    document_id: i64,
    session: State<'_, Session>,
    env: State<'_, PreviewEnv>,
) -> Result<PreviewInfo, CommandError> {
    let session = session.shared();
    let env = env.inner().clone();
    tauri::async_runtime::spawn_blocking(move || ops::open_preview(&session, &env, document_id))
        .await
        .map_err(|e| CommandError::new("INTERNAL_ERROR", e.to_string()))?
}

/// Closes the preview `previewId`; a stale id is ignored.
#[tauri::command]
pub async fn close_preview(
    preview_id: u64,
    session: State<'_, Session>,
) -> Result<(), CommandError> {
    ops::close_preview(&session, preview_id)
}

/// One TIFF page as PNG bytes, received by the viewer as an `ArrayBuffer`.
#[tauri::command]
pub async fn render_preview_page(
    preview_id: u64,
    page: u32,
    width_px: u32,
    session: State<'_, Session>,
) -> Result<tauri::ipc::Response, CommandError> {
    let session = session.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        ops::render_preview_page(&session, preview_id, page, width_px)
    })
    .await
    .map_err(|e| CommandError::new("INTERNAL_ERROR", e.to_string()))?
    .map(tauri::ipc::Response::new)
}
