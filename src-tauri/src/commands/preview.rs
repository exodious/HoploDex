//! Preview commands (contracts/tauri-commands.md "Preview commands"). Each
//! is thin: it goes through the `Session`, then calls the matching function
//! in `ops`.
//!
//! The preview lives in the open database (`OpenDatabase::preview`), so it
//! ends with the database on every path (research.md §20). These commands
//! need no write, so unlike the collection's they aren't held up by pending
//! changes; they refuse with `DATABASE_CLOSED` when nothing is open.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Manager, Runtime, State};
use zeroize::Zeroizing;

use crate::commands::CommandError;
use crate::models::document_attachment::PreviewKind;
use crate::services::document_types::{classify, from_recorded};
use crate::services::preview::availability::{
    PdfAvailability, PdfAvailabilityState, UnavailableReason,
};
use crate::services::preview::{
    HelperError, HelperHandle, HelperLimits, PdfEndReason, Preview, PreviewContent, PreviewInfo,
    PreviewLoading, PreviewSurface, SurfaceBounds, SurfaceFactory, SurfaceSpec, protocol_handler,
    surface::{AppSurface, Hooks},
    text, tripwire,
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
    /// How long a PDF may go without PDF.js's hook on Linux, from its
    /// document first being served.
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

/// Builds the PDF surface as a child web view of the main window, with the
/// app's hooks: what the surface sees reaches the session through `ops`.
pub struct AppSurfaces<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> AppSurfaces<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }

    /// The hooks of one surface. Each runs its work on a thread of its own:
    /// they are called from the main thread, which must not wait on the
    /// session, since the thread that holds it may be waiting for the main
    /// thread to build or close a web view.
    fn hooks(&self) -> Arc<Hooks> {
        fn off_thread(name: &str, work: impl FnOnce() + Send + 'static) {
            if let Err(err) = std::thread::Builder::new().name(name.into()).spawn(work) {
                log::error!("could not start {name}: {err}");
            }
        }
        let shared = |app: &AppHandle<R>| app.state::<Session>().shared();
        let (a, b, c, d, e) = (
            self.app.clone(),
            self.app.clone(),
            self.app.clone(),
            self.app.clone(),
            self.app.clone(),
        );
        Hooks::new(
            Box::new(move || {
                let session = shared(&a);
                let app = a.clone();
                off_thread("preview-escape", move || {
                    give_focus_back(&app);
                    ops::surface_key(&session, "preview:escape");
                });
            }),
            Box::new(move || {
                let session = shared(&b);
                let app = b.clone();
                off_thread("preview-focus-chrome", move || {
                    give_focus_back(&app);
                    ops::surface_key(&session, "preview:focus-chrome");
                });
            }),
            // The idle clock takes no lock of the session's database.
            Box::new(move || crate::session::idle::note_activity(&c.state::<Session>())),
            Box::new(move |reason| {
                let session = shared(&d);
                off_thread("preview-ended", move || ops::surface_ended(&session, reason));
            }),
            Box::new(move |url| {
                let (app, url) = (e.clone(), url.to_string());
                off_thread("preview-download", move || {
                    let env = app.state::<PreviewEnv>().inner().clone();
                    let _ = ops::surface_download(&app.state::<Session>().shared(), &env, &url);
                });
            }),
        )
    }
}

/// Escape and F6 pressed in the surface leave the keyboard focus with it, as
/// a web view of its own (the page's own `focus()` can't move it): the main
/// web view takes it back, so the keys that follow reach the viewer (found by
/// `us13-document-preview.e2e.ts`, where → after F6 went to the PDF).
fn give_focus_back<R: Runtime>(app: &AppHandle<R>) {
    if let Some(main) = app.get_webview("main")
        && let Err(err) = main.set_focus()
    {
        log::warn!("could not give the focus back to the main web view: {err}");
    }
}

impl<R: Runtime> SurfaceFactory for AppSurfaces<R> {
    fn build(&self, spec: SurfaceSpec) -> Result<Arc<dyn PreviewSurface>, String> {
        let window = self.app.get_window("main").ok_or("the main window is gone")?;
        let tripwire = tripwire::shared().ok_or("the tripwire isn't bound")?;
        let proxy_url =
            format!("http://{}", tripwire.addr()).parse().map_err(|e| format!("{e}"))?;
        let data_directory =
            crate::app_dirs::preview_webview_data_dir(&self.app).map_err(|e| e.to_string())?;
        AppSurface::build(&window, spec, proxy_url, data_directory, self.hooks())
    }
}

/// Where the viewer's page area last was, as `set_preview_bounds` sent it: a
/// new surface is built there, hidden (contracts/tauri-commands.md
/// "open_preview"). There is one main window, so one place.
static LAST_BOUNDS: Mutex<SurfaceBounds> =
    Mutex::new(SurfaceBounds { x: 0.0, y: 0.0, width: 800.0, height: 600.0 });

fn last_bounds() -> SurfaceBounds {
    *LAST_BOUNDS.lock().unwrap_or_else(|e| e.into_inner())
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
        let previous = open.end_preview();
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
            PreviewKind::Pdf => begin_pdf(open, env, document_id, document.file_bytes, previous),
        }
    }

    /// The PDF path of `begin`: a new token, and the surface of the PDF
    /// that was shown if there was one, else a new one, hidden at the last
    /// bounds sent, until its document has been served.
    fn begin_pdf(
        open: &mut OpenDatabase,
        env: &PreviewEnv,
        document_id: i64,
        bytes: Vec<u8>,
        previous: Option<Preview>,
    ) -> Result<Begun, CommandError> {
        if let PdfAvailability::Unavailable { reason } = env.availability.get() {
            return Err(CommandError::new("PDF_PREVIEW_UNAVAILABLE", reason.message()));
        }
        // Bound once for the run; nothing can bind that port but this
        // process, so a request that reaches it is a request the surface's
        // other layers failed to stop (research.md §6).
        if tripwire::shared().is_none() {
            return Err(failed(
                "The PDF viewer couldn't be set up, so this document can't be shown.",
            ));
        }
        let mut token = [0u8; 16];
        getrandom::fill(&mut token).map_err(|_| failed("This document couldn't be shown."))?;
        let url = protocol_handler::document_url(&token);

        let kept = match previous {
            Some(Preview { content: PreviewContent::Pdf { surface, secret, .. }, .. }) => {
                Some((surface, secret))
            }
            other => {
                drop(other);
                None
            }
        };
        let (surface, secret) = match kept {
            Some((surface, secret)) => {
                // Hidden while the next document loads: its page, the old
                // document, is still on it until the navigation lands.
                surface.set_bounds(last_bounds().into(), false);
                surface.navigate(&url);
                (surface, secret)
            }
            None => {
                let secret = crate::db::random_hex(16)
                    .map_err(|_| failed("This document couldn't be shown."))?;
                // The surface is built with the session's lock held, and
                // `add_child` waits for the main thread. That can't deadlock
                // as long as nothing on the main thread waits for the lock:
                // every command is `async` (so runs on the runtime's
                // threads), the OS backends in `platform/` run on threads of
                // their own, the protocol handler and each surface hook hand
                // their work to another thread, and the idle clock's
                // `note_activity` takes only the clock's own mutex.
                let spec = SurfaceSpec { url, secret: secret.clone(), bounds: last_bounds() };
                let surface = env.surfaces.build(spec).map_err(|reason| {
                    log::error!("the PDF surface couldn't be built: {reason}");
                    failed("The PDF viewer couldn't be started, so this document can't be shown.")
                })?;
                (surface, secret)
            }
        };
        open.preview_seq += 1;
        let id = open.preview_seq;
        open.preview = Some(Preview {
            id,
            document_id,
            content: PreviewContent::Pdf {
                token,
                bytes: Zeroizing::new(bytes),
                surface,
                secret,
                served: false,
                hooked: false,
                hook_timeout: env.hook_timeout,
            },
        });
        Ok(Begun::Done(PreviewInfo::Pdf { preview_id: id, document_id }))
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
            Begun::Done(info) => {
                if let PreviewInfo::Pdf { preview_id, .. } = &info {
                    watch_for_the_serve(session, env, *preview_id);
                }
                return Ok(info);
            }
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
                return Ok(open.end_preview());
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
                return Ok(open.end_preview());
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
            *LAST_BOUNDS.lock().unwrap_or_else(|e| e.into_inner()) = bounds;
            let ready = *served && (*hooked || !cfg!(target_os = "linux"));
            surface.set_bounds(bounds.into(), visible && ready);
            Ok(())
        })
    }

    /// Hides the PDF surface if one is shown, where it is, so that a native
    /// dialog `open_document` shows is not sitting under it (research.md
    /// §16). The viewer stays open; the frontend shows the surface again
    /// with `set_preview_bounds`. Nothing happens with no PDF preview, or no
    /// open database.
    pub fn hide_pdf_surface(session: &Session) {
        let _ = session.inspect(|open| {
            if let Some(Preview { content: PreviewContent::Pdf { surface, .. }, .. }) =
                open.preview.as_ref()
            {
                surface.set_bounds(last_bounds().into(), false);
            }
            Ok(())
        });
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

    /// Closes the PDF preview that is open and `matches`, and tells the viewer
    /// why (`preview:pdf-ended`). Returns whether there was one. The surface
    /// is closed outside the session's lock.
    fn end_pdf(
        session: &Session,
        matches: impl FnOnce(&Preview) -> bool,
        reason: PdfEndReason,
    ) -> bool {
        let ended = session.inspect_mut(|open| {
            let is_it = open
                .preview
                .as_ref()
                .is_some_and(|p| matches!(p.content, PreviewContent::Pdf { .. }) && matches(p));
            Ok(if is_it { open.end_preview() } else { None })
        });
        let Ok(Some(preview)) = ended else { return false };
        let preview_id = preview.id;
        drop(preview);
        session.events().emit_to_main(
            "preview:pdf-ended",
            json!({ "previewId": preview_id, "reason": reason }),
        );
        true
    }

    /// The surface itself ended the preview (the macOS watch caught a copy,
    /// a Windows interface was missing): closes the PDF shown and says why.
    pub fn surface_ended(session: &Session, reason: PdfEndReason) {
        end_pdf(session, |_| true, reason);
    }

    /// Escape or F6 was pressed in the surface: the viewer is told (`event`
    /// is `preview:escape` or `preview:focus-chrome`).
    pub fn surface_key(session: &Session, event: &str) {
        let shown = session.inspect(|open| {
            Ok(open
                .preview
                .as_ref()
                .filter(|p| matches!(p.content, PreviewContent::Pdf { .. }))
                .map(|p| p.id))
        });
        if let Ok(Some(preview_id)) = shown {
            session.events().emit_to_main(event, json!({ "previewId": preview_id }));
        }
    }

    /// The surface's `on_download` hook: a download of the surface's own URL
    /// means the web view has no PDF viewer. The download is refused by the
    /// caller; this ends the preview and turns PDF preview off for the run
    /// (research.md §4, §7). Any other URL is not the sign.
    pub fn surface_download(
        session: &Arc<Session>,
        env: &PreviewEnv,
        url: &str,
    ) -> Result<(), CommandError> {
        let Ok(url) = url.parse::<tauri::Url>() else { return Ok(()) };
        let own = session.inspect(|open| {
            Ok(open.preview.as_ref().is_some_and(|p| match &p.content {
                PreviewContent::Pdf { token, .. } => protocol_handler::is_document_url(&url, token),
                _ => false,
            }))
        })?;
        if own {
            env.availability
                .set(PdfAvailability::Unavailable { reason: UnavailableReason::NoViewer });
            end_pdf(session, |_| true, PdfEndReason::NoViewer);
        }
        Ok(())
    }

    /// Linux: the surface must ask for the document within
    /// `env.hook_timeout` (5 s, research.md §8's figure; no other is
    /// recorded) of `open_preview`, or the preview fails as a missing hook
    /// does, so the viewer doesn't wait for a surface that never asks. The
    /// hook's own 5 s starts at the first serve (`hook_timed_out`).
    fn watch_for_the_serve(session: &Arc<Session>, env: &PreviewEnv, preview_id: u64) {
        if !cfg!(target_os = "linux") {
            return;
        }
        let (session, timeout) = (Arc::clone(session), env.hook_timeout);
        let spawned =
            std::thread::Builder::new().name("preview-serve-timer".into()).spawn(move || {
                std::thread::sleep(timeout);
                end_pdf(
                    &session,
                    |p| {
                        p.id == preview_id
                            && matches!(p.content, PreviewContent::Pdf { served: false, .. })
                    },
                    PdfEndReason::Failed,
                );
            });
        if let Err(err) = spawned {
            log::error!("could not start the PDF serve timer: {err}");
        }
    }

    /// Linux: PDF.js's hook did not report within the preview's
    /// `hook_timeout` of its document being served (the protocol handler
    /// starts that timer at the first serve, research.md §8), so the surface
    /// is closed rather than shown with a PDF's scripting on. A no-op if the
    /// preview has ended, been replaced or been hooked since.
    pub fn hook_timed_out(session: &Session, preview_id: u64) {
        end_pdf(
            session,
            |p| {
                p.id == preview_id && matches!(p.content, PreviewContent::Pdf { hooked: false, .. })
            },
            PdfEndReason::Failed,
        );
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

/// Moves and sizes the PDF surface over the viewer's page area and shows or
/// hides it; it stays hidden until `preview:pdf-ready`.
#[tauri::command]
pub async fn set_preview_bounds(
    preview_id: u64,
    bounds: SurfaceBounds,
    visible: bool,
    session: State<'_, Session>,
) -> Result<(), CommandError> {
    ops::set_preview_bounds(&session, preview_id, bounds, visible)
}

/// Gives the PDF surface the keyboard focus.
#[tauri::command]
pub async fn focus_preview(
    preview_id: u64,
    session: State<'_, Session>,
) -> Result<(), CommandError> {
    ops::focus_preview(&session, preview_id)
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
