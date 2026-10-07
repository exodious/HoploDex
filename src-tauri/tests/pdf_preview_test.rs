//! The PDF preview, behind the app's `PreviewSurface` (feature 007, T031;
//! research.md §4, §6-§8, §17, §20, §21): `open_preview` for a PDF, the
//! `hdpreview` protocol handler called directly, the surface's bounds and
//! its end, `PdfAvailability` and the hold, and what the preview refuses.
//! Real SQLCipher databases in temp directories; the web view is a recorder.
//!
//! Assumed API (T048, T049, T058, T060, T061, T057; each item's shape is
//! also in tests/support/preview_support.rs):
//! - `commands::preview::PreviewEnv { hook_timeout, .. }` and, in
//!   `commands::preview::ops`: `open_preview(&Arc<Session>, &PreviewEnv,
//!   document_id) -> Result<PreviewInfo>`, `close_preview(&Session, id)`,
//!   `set_preview_bounds(&Session, id, SurfaceBounds, visible)`,
//!   `focus_preview(&Session, id)`, `render_preview_page(&Session, id, page,
//!   width_px) -> Result<Vec<u8>>`, `close_preview_of(&Session, document_id)`
//!   (what the `delete_document` command calls first) and
//!   `surface_download(&Arc<Session>, &PreviewEnv, url)` (the surface's
//!   `on_download` hook);
//! - `services::preview::protocol_handler::handle(&Session,
//!   &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>>`;
//! - `PdfAvailability::at_startup(&MachineSettings, running_version: &str,
//!   os_check_passed: bool)`;
//! - `MachineSettings::sweep_hold_leftovers()`.

#[path = "support/preview_support.rs"]
mod preview_support;
mod support;

use std::fs;
#[cfg(target_os = "linux")]
use std::time::Duration;

use hoplodex_lib::commands::documents::ops as document_ops;
use hoplodex_lib::commands::preview::ops as preview;
use hoplodex_lib::models::document_attachment::DocumentSummary;
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::preview::SurfaceBounds;
use hoplodex_lib::services::preview::availability::{PdfAvailability, UnavailableReason};
use hoplodex_lib::services::preview::protocol_handler;
use serde_json::{Value, json};
use support::document_fixture;
use support::hostile_documents as hostile;
use tauri::Url;
use tauri::http::{Request, Response};
use tempfile::TempDir;

use preview_support::{Fixture, SurfaceLog, insert_raw_document};

const LINUX: bool = cfg!(target_os = "linux");

fn pdf_bytes() -> Vec<u8> {
    document_fixture("three-pages.pdf")
}

/// The protocol handler, asked for `url` as the surface asks.
fn get(world: &Fixture, url: &str) -> Response<Vec<u8>> {
    let request = Request::builder().method("GET").uri(url).body(Vec::new()).unwrap();
    protocol_handler::handle(&world.session, &request)
}

/// `url` with its path replaced.
fn at(url: &str, path: &str) -> String {
    let mut parsed = Url::parse(url).unwrap();
    parsed.set_path(path);
    parsed.to_string()
}

/// The token of `hdpreview://localhost/<token>/document.pdf` (on Windows
/// `http://hdpreview.localhost/<token>/document.pdf`).
fn token_of(url: &str) -> String {
    let parsed = Url::parse(url).unwrap();
    let host = if cfg!(windows) { "hdpreview.localhost" } else { "localhost" };
    let scheme = if cfg!(windows) { "http" } else { "hdpreview" };
    assert_eq!((parsed.scheme(), parsed.host_str()), (scheme, Some(host)), "{url}");
    let segments: Vec<_> = parsed.path_segments().unwrap().collect();
    assert_eq!(segments.len(), 2, "{url}");
    assert_eq!(segments[1], "document.pdf", "the last segment is always document.pdf");
    segments[0].to_owned()
}

fn header<'a>(response: &'a Response<Vec<u8>>, name: &str) -> Option<&'a str> {
    response.headers().get(name).map(|v| v.to_str().unwrap())
}

fn is_404(response: &Response<Vec<u8>>) -> bool {
    response.status().as_u16() == 404 && response.body().is_empty()
}

/// Serves the document to the surface, and on Linux reports PDF.js's hook as
/// its frame script does, so the preview is ready.
fn serve_and_hook(world: &Fixture, surface: &SurfaceLog) -> Response<Vec<u8>> {
    let served = get(world, &surface.url());
    assert_eq!(served.status().as_u16(), 200);
    if LINUX {
        let hooked = get(world, &at(&surface.url(), &format!("/hooked/{}", surface.secret())));
        assert_eq!(hooked.status().as_u16(), 204);
    }
    served
}

fn ready_events(world: &Fixture) -> Vec<Value> {
    world.events.payloads("preview:pdf-ready")
}

fn ended_events(world: &Fixture) -> Vec<Value> {
    world.events.payloads("preview:pdf-ended")
}

fn code_of<T: std::fmt::Debug>(result: Result<T, hoplodex_lib::commands::CommandError>) -> String {
    result.unwrap_err().code
}

// --- Opening, and the protocol (FR-001, FR-002, research.md §4) ----------------

#[test]
fn a_pdf_opens_on_the_surface_and_the_handler_serves_exactly_its_bytes() {
    let world = Fixture::new();
    let bytes = pdf_bytes();
    let id = world.add("2024 appraisal.pdf", &bytes);

    let info = world.open_ok(id);

    assert_eq!(info["kind"], "pdf");
    assert_eq!(info["documentId"], id);
    assert!(info.get("pages").is_none() && info.get("text").is_none(), "{info}");
    let surface = world.preview.surfaces.only();
    let token = token_of(&surface.url());
    assert_eq!(token.len(), 32, "a 128-bit token, as hex");
    assert!(token.bytes().all(|b| b.is_ascii_hexdigit()), "{token}");
    assert!(ready_events(&world).is_empty(), "nothing is ready before the document is served");
    assert!(!surface.ever_shown(), "the surface stays hidden until the document is served");

    let served = get(&world, &surface.url());

    assert_eq!(served.status().as_u16(), 200);
    assert_eq!(header(&served, "content-type"), Some("application/pdf"));
    assert_eq!(header(&served, "cache-control"), Some("no-store"));
    assert_eq!(header(&served, "accept-ranges"), None);
    assert_eq!(served.body(), &bytes, "exactly the document's bytes");
    if LINUX {
        assert!(ready_events(&world).is_empty(), "on Linux the hook has to be seen as well");
        let wrong = get(&world, &at(&surface.url(), "/hooked/not-the-secret"));
        assert!(is_404(&wrong));
        assert!(ready_events(&world).is_empty(), "a wrong secret hooks nothing");
        let hooked = get(&world, &at(&surface.url(), &format!("/hooked/{}", surface.secret())));
        assert_eq!(hooked.status().as_u16(), 204);
    }
    assert_eq!(ready_events(&world), vec![json!({ "previewId": Fixture::id_of(&info) })]);
}

#[test]
fn any_other_path_the_previous_token_and_a_closed_preview_get_404() {
    let world = Fixture::new();
    let first = world.add("a.pdf", &pdf_bytes());
    let second = world.add("b.pdf", &document_fixture("three-pages.pdf"));
    world.open_ok(first);
    let first_url = world.preview.surfaces.only().url();
    let info = world.open_ok(second);
    let url = world.preview.surfaces.only().url();
    assert_ne!(first_url, url, "a new token for each document shown");

    assert!(is_404(&get(&world, &first_url)), "the previous token");
    assert!(is_404(&get(&world, &at(&url, "/"))));
    assert!(is_404(&get(&world, &at(&url, &format!("/{}/", token_of(&url))))));
    assert!(is_404(&get(&world, &at(&url, &format!("/{}/other.pdf", token_of(&url))))));
    assert!(is_404(&get(&world, &at(&url, &format!("/{}/document.pdf/x", token_of(&url))))));
    assert!(is_404(&get(&world, &at(&url, "/hooked/wrong"))));
    assert_eq!(get(&world, &url).status().as_u16(), 200, "the current token still works");

    world.close(Fixture::id_of(&info));

    assert!(is_404(&get(&world, &url)), "the token after close_preview");
}

#[test]
fn another_pdf_keeps_the_surface_navigates_it_and_revokes_the_old_token() {
    let world = Fixture::new();
    let (first_bytes, second) = (pdf_bytes(), hostile::marker_pdf(&[b'x'; 64]));
    let first = world.add("a.pdf", &first_bytes);
    let second_id = world.add("b.pdf", &second.bytes);
    world.open_ok(first);
    let surface = world.preview.surfaces.only();
    let old_url = surface.url();
    serve_and_hook(&world, &surface);

    let info = world.open_ok(second_id);

    assert_eq!(world.preview.surfaces.built().len(), 1, "the surface is kept for the next PDF");
    assert_eq!(surface.urls().len(), 2, "and navigated to the new document");
    assert!(!surface.is_closed());
    let new_url = surface.url();
    assert_ne!(new_url, old_url);
    assert!(is_404(&get(&world, &old_url)), "the old token is revoked");
    assert_eq!(get(&world, &new_url).body(), &second.bytes);
    assert_eq!(info["documentId"], second_id);
}

#[test]
fn moving_to_a_tiff_or_to_text_closes_the_surface() {
    let world = Fixture::new();
    let pdf = world.add("a.pdf", &pdf_bytes());
    let tiff = world.add("b.tif", &document_fixture("one-page.tif"));
    let text = world.add("c.txt", b"hello");

    world.open_ok(pdf);
    let first = world.preview.surfaces.latest();
    let url = first.url();
    assert!(!first.is_closed());
    world.open_ok(tiff);
    assert!(first.is_closed(), "PDF to TIFF closes the surface");
    assert!(is_404(&get(&world, &url)));

    world.open_ok(pdf);
    let second = world.preview.surfaces.latest();
    assert_eq!(world.preview.surfaces.built().len(), 2, "a PDF after that builds a new one");
    world.open_ok(text);
    assert!(second.is_closed(), "PDF to text closes the surface");
}

#[test]
fn close_preview_closes_the_surface_ignores_a_stale_id_and_replaced_ids_are_refused() {
    let world = Fixture::new();
    let (a, b) = (world.add("a.pdf", &pdf_bytes()), world.add("b.pdf", &pdf_bytes()));
    let first = Fixture::id_of(&world.open_ok(a));
    let second = Fixture::id_of(&world.open_ok(b));
    assert_ne!(first, second);
    let surface = world.preview.surfaces.only();
    let bounds = SurfaceBounds { x: 1.0, y: 2.0, width: 300.0, height: 400.0 };

    assert_eq!(
        code_of(preview::set_preview_bounds(&world.session, first, bounds, true)),
        "PREVIEW_CLOSED"
    );
    assert_eq!(code_of(preview::focus_preview(&world.session, first)), "PREVIEW_CLOSED");
    assert_eq!(
        code_of(preview::render_preview_page(&world.session, first, 0, 100)),
        "PREVIEW_CLOSED"
    );
    world.close(first);
    assert!(!surface.is_closed(), "closing a replaced preview leaves the current one");
    assert_eq!(get(&world, &surface.url()).status().as_u16(), 200);
    assert_eq!(surface.focus_calls(), 0);

    preview::focus_preview(&world.session, second).unwrap();
    assert_eq!(surface.focus_calls(), 1);
    world.close(second);
    assert!(surface.is_closed());
    assert_eq!(
        code_of(preview::set_preview_bounds(&world.session, second, bounds, true)),
        "PREVIEW_CLOSED"
    );
    world.close(second);
}

// --- The surface's bounds (research.md §4) --------------------------------------

#[test]
fn bounds_are_validated_and_the_surface_stays_hidden_until_ready() {
    let world = Fixture::new();
    let id = world.add("a.pdf", &pdf_bytes());
    let preview_id = Fixture::id_of(&world.open_ok(id));
    let surface = world.preview.surfaces.only();
    let ok = SurfaceBounds { x: 10.0, y: 20.0, width: 300.0, height: 400.0 };
    let with = |f: fn(&mut SurfaceBounds)| {
        let mut bounds = ok;
        f(&mut bounds);
        bounds
    };
    let refused: [SurfaceBounds; 9] = [
        with(|b| b.x = -1.0),
        with(|b| b.y = -0.5),
        with(|b| b.width = -10.0),
        with(|b| b.height = -10.0),
        with(|b| b.x = f64::NAN),
        with(|b| b.y = f64::INFINITY),
        with(|b| b.width = f64::NAN),
        with(|b| b.width = 0.5),
        with(|b| b.height = 0.0),
    ];
    for bounds in refused {
        assert_eq!(
            code_of(preview::set_preview_bounds(&world.session, preview_id, bounds, true)),
            "VALIDATION_ERROR",
            "{bounds:?}"
        );
    }
    assert!(surface.bounds_calls().is_empty(), "a refused size never reaches the surface");

    // Before preview:pdf-ready it stays hidden, whatever `visible` says.
    preview::set_preview_bounds(&world.session, preview_id, ok, true).unwrap();
    assert!(!surface.ever_shown(), "shown before the document was served");

    serve_and_hook(&world, &surface);
    assert_eq!(ready_events(&world).len(), 1);
    preview::set_preview_bounds(&world.session, preview_id, ok, true).unwrap();
    let (rect, visible) = *surface.bounds_calls().last().unwrap();
    assert!(visible, "shown once ready");
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (10.0, 20.0, 300.0, 400.0));
    preview::set_preview_bounds(&world.session, preview_id, ok, false).unwrap();
    assert!(!surface.bounds_calls().last().unwrap().1, "hidden again while something covers it");
}

// --- The hook, and a download (research.md §7, §8) ------------------------------

#[cfg(target_os = "linux")]
#[test]
fn without_the_hook_the_surface_is_closed_and_the_viewer_told_it_failed() {
    let mut world = Fixture::new();
    world.preview.env.hook_timeout = Duration::from_millis(300);
    let id = world.add("a.pdf", &pdf_bytes());
    let preview_id = Fixture::id_of(&world.open_ok(id));
    let surface = world.preview.surfaces.only();

    assert_eq!(get(&world, &surface.url()).status().as_u16(), 200);

    assert!(
        preview_support::wait_for(Duration::from_secs(5), || !ended_events(&world).is_empty()),
        "no preview:pdf-ended"
    );
    assert_eq!(ended_events(&world), vec![json!({ "previewId": preview_id, "reason": "failed" })]);
    assert!(surface.is_closed());
    assert!(!surface.ever_shown());
    assert!(ready_events(&world).is_empty(), "never shown with scripting on");
}

#[cfg(target_os = "linux")]
#[test]
fn a_hook_in_time_keeps_the_preview_open() {
    let mut world = Fixture::new();
    world.preview.env.hook_timeout = Duration::from_millis(400);
    let id = world.add("a.pdf", &pdf_bytes());
    world.open_ok(id);
    let surface = world.preview.surfaces.only();

    serve_and_hook(&world, &surface);
    std::thread::sleep(Duration::from_millis(900));

    assert!(ended_events(&world).is_empty());
    assert!(!surface.is_closed());
}

#[test]
fn a_download_of_the_surfaces_own_url_means_no_viewer_and_ends_the_preview() {
    let world = Fixture::new();
    let id = world.add("a.pdf", &pdf_bytes());
    let preview_id = Fixture::id_of(&world.open_ok(id));
    let surface = world.preview.surfaces.only();

    // Some other URL is not the sign.
    let _ =
        preview::surface_download(&world.session, &world.preview.env, "https://example.com/x.pdf");
    assert_eq!(world.preview.availability.get(), PdfAvailability::Available);
    assert!(!surface.is_closed());

    let _ = preview::surface_download(&world.session, &world.preview.env, &surface.url());

    assert_eq!(
        world.preview.availability.get(),
        PdfAvailability::Unavailable { reason: UnavailableReason::NoViewer }
    );
    assert_eq!(
        ended_events(&world),
        vec![json!({ "previewId": preview_id, "reason": "noViewer" })]
    );
    assert!(surface.is_closed());
    assert_eq!(code_of(world.open(id)), "PDF_PREVIEW_UNAVAILABLE");
}

// --- PdfAvailability and the hold (FR-003a, research.md §7, §17) -----------------

#[test]
fn when_pdf_preview_is_off_pdfs_fail_but_tiff_and_text_still_preview() {
    for reason in
        [UnavailableReason::CheckFailed, UnavailableReason::Held, UnavailableReason::NoViewer]
    {
        let world = Fixture::new();
        let pdf = world.add("a.pdf", &pdf_bytes());
        let tiff = world.add("b.tif", &document_fixture("one-page.tif"));
        let text = world.add("c.txt", b"hello there");
        world.preview.availability.set(PdfAvailability::Unavailable { reason });

        let refused = world.open(pdf).unwrap_err();

        assert_eq!(refused.code, "PDF_PREVIEW_UNAVAILABLE", "{reason:?}");
        assert_eq!(refused.message, reason.message(), "the reason's own sentence");
        assert!(world.preview.surfaces.built().is_empty(), "no surface for a PDF that is off");
        let state = world.preview.availability.get();
        let listed: Vec<(String, bool)> = world
            .session
            .read(|conn| document_ops::list_documents(conn, RecordRef::Firearm(world.firearm)))
            .unwrap()
            .into_iter()
            .map(|d| {
                let summary = serde_json::to_value(DocumentSummary::new(d, &state)).unwrap();
                (
                    summary["originalFilename"].as_str().unwrap().to_owned(),
                    summary["previewAvailable"].as_bool().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            listed,
            [("a.pdf".into(), false), ("b.tif".into(), true), ("c.txt".into(), true)],
            "{reason:?}"
        );
        assert_eq!(world.open_ok(tiff)["kind"], "tiff");
        assert_eq!(world.open_ok(text)["kind"], "text");
        assert_eq!(world.open_ok(text)["text"], "hello there");
    }
}

#[test]
fn the_hold_and_the_os_check_decide_availability_at_startup() {
    let config = TempDir::new().unwrap();
    let machine = MachineSettings::load(config.path()).unwrap();
    let held = PdfAvailability::Unavailable { reason: UnavailableReason::Held };

    assert_eq!(PdfAvailability::at_startup(&machine, "1.2.3", true), PdfAvailability::Available);
    assert_eq!(
        PdfAvailability::at_startup(&machine, "1.2.3", false),
        PdfAvailability::Unavailable { reason: UnavailableReason::CheckFailed }
    );

    machine.hold_pdf_preview("1.2.3");
    assert_eq!(PdfAvailability::at_startup(&machine, "1.2.3", true), held, "this version's hold");
    assert_eq!(machine.pdf_preview_hold().unwrap().version.as_deref(), Some("1.2.3"));

    let leftover = config.path().join("WebKitPDFs-abc");
    machine.add_hold_leftover(&leftover);
    assert_eq!(
        PdfAvailability::at_startup(&machine, "1.3.0", true),
        PdfAvailability::Available,
        "another version tries again"
    );
    let hold = MachineSettings::load(config.path()).unwrap().pdf_preview_hold().unwrap();
    assert_eq!(hold.version, None, "and its hold is cleared at startup");
    assert_eq!(hold.leftovers, [leftover], "the leftovers stay until each path is gone");
}

#[test]
fn the_sweep_deletes_only_the_recorded_leftovers() {
    let config = TempDir::new().unwrap();
    let scratch = TempDir::new().unwrap();
    let machine = MachineSettings::load(config.path()).unwrap();
    let (caught, gone, other) = (
        scratch.path().join("WebKitPDFs-caught"),
        scratch.path().join("WebKitPDFs-gone"),
        scratch.path().join("WebKitPDFs-another-apps"),
    );
    for folder in [&caught, &other] {
        fs::create_dir(folder).unwrap();
        fs::write(folder.join("document.pdf"), b"%PDF-1.4 copy").unwrap();
    }
    machine.hold_pdf_preview("1.2.3");
    machine.add_hold_leftover(&caught);
    machine.add_hold_leftover(&gone);

    machine.sweep_hold_leftovers();

    assert!(!caught.exists(), "a recorded leftover is deleted");
    assert_eq!(
        fs::read(other.join("document.pdf")).unwrap(),
        b"%PDF-1.4 copy",
        "another WebKitPDFs folder is left alone"
    );
    let hold = MachineSettings::load(config.path()).unwrap().pdf_preview_hold().unwrap();
    assert!(hold.leftovers.is_empty(), "the deleted and the already gone are dropped");
    assert_eq!(hold.version.as_deref(), Some("1.2.3"), "the hold itself stays");
}

// --- What a preview refuses (FR-006, US2-4) --------------------------------------

#[test]
fn a_pdf_that_is_not_one_fails_before_any_surface_exists() {
    let world = Fixture::new();
    let html = hostile::html_named_pdf();
    let id = insert_raw_document(
        &world.session,
        world.firearm,
        &html.name,
        "application/pdf",
        &html.bytes,
    );

    assert_eq!(code_of(world.open(id)), "DOCUMENT_CONTENT_MISMATCH");

    assert!(world.preview.surfaces.built().is_empty());
}

#[test]
fn a_docx_and_a_row_from_before_the_feature_are_not_previewed_and_a_missing_one_is_not_found() {
    let world = Fixture::new();
    let docx = world.add("sample.docx", &document_fixture("sample.docx"));
    let jpeg =
        insert_raw_document(&world.session, world.firearm, "Old scan.jpg", "image/jpeg", b"raw");

    assert_eq!(code_of(world.open(docx)), "PREVIEW_UNSUPPORTED");
    assert_eq!(code_of(world.open(jpeg)), "PREVIEW_UNSUPPORTED");
    assert_eq!(code_of(world.open(9_999_999)), "NOT_FOUND");

    assert!(world.preview.surfaces.built().is_empty());
}

// --- Deleting the document shown (contracts "delete_document") --------------------

#[test]
fn deleting_the_document_shown_closes_its_preview_first() {
    let world = Fixture::new();
    let (shown, other) = (world.add("a.pdf", &pdf_bytes()), world.add("b.pdf", &pdf_bytes()));
    let preview_id = Fixture::id_of(&world.open_ok(shown));
    let surface = world.preview.surfaces.only();
    let url = surface.url();
    let delete = |id: i64| {
        // What the `delete_document` command does: close the preview of the
        // document, then delete it.
        preview::close_preview_of(&world.session, id).unwrap();
        world.session.write(|conn| document_ops::delete_document(conn, id, true)).unwrap();
    };

    delete(other);
    assert!(!surface.is_closed(), "another document's deletion leaves the preview");
    assert_eq!(get(&world, &url).status().as_u16(), 200);

    delete(shown);

    assert!(surface.is_closed());
    assert!(is_404(&get(&world, &url)));
    assert_eq!(
        code_of(preview::set_preview_bounds(
            &world.session,
            preview_id,
            SurfaceBounds { x: 0.0, y: 0.0, width: 10.0, height: 10.0 },
            true
        )),
        "PREVIEW_CLOSED"
    );
}
