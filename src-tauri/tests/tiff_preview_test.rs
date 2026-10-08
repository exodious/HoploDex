//! The TIFF preview against the real render helper (feature 007, T032;
//! research.md §11, §14, §20): `open_preview` starts `CARGO_BIN_EXE_hoplodex
//! --render-helper`, `render_preview_page` asks it for pages, and the
//! crafted TIFFs of T004 fail cleanly. Real SQLCipher databases in temp
//! directories. How the preview ends at a lock, close, switch, sleep and
//! shutdown is lock_test.rs's (T036).
//!
//! Assumed API (T048, T049, T053, T054; shapes in
//! tests/support/preview_support.rs): `commands::preview::PreviewEnv {
//! helper_limits: HelperLimits, .. }` and `ops::{open_preview, close_preview,
//! render_preview_page(&Session, id, page, width_px) -> Result<Vec<u8>>}`;
//! `services::preview::HelperLimits { load, render }` (`Duration`s), whose
//! `Default` is the contract's 20 s and 10 s. Assumes preview ids count up
//! by one in a session, and that `close_preview(id)` ends a `Load` still
//! running for that id without waiting for the session's lock.

#[path = "support/preview_support.rs"]
mod preview_support;
mod support;

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use hoplodex_lib::commands::preview::ops as preview;
use serde_json::Value;
use support::document_fixture;
use support::hostile_documents as hostile;
use tempfile::TempDir;

use preview_support::{Fixture, any_file_holds, assert_gone, png_size, wait_for_helper_child};
// Only the Unix-only kill test below waits for a helper by process id.
#[cfg(unix)]
use preview_support::wait_until_gone;

/// The tests of this file share one process, and several of them look at
/// every render helper that process has as a child (`helper_children`), so
/// they run one at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner())
}

fn code_of<T: std::fmt::Debug>(result: Result<T, hoplodex_lib::commands::CommandError>) -> String {
    result.unwrap_err().code
}

fn render(world: &Fixture, id: u64, page: u32, width: u32) -> Result<Vec<u8>, String> {
    preview::render_preview_page(&world.session, id, page, width).map_err(|e| e.code)
}

fn pages_of(info: &Value) -> Vec<(f64, f64)> {
    info["pages"]
        .as_array()
        .unwrap_or_else(|| panic!("no pages in {info}"))
        .iter()
        .map(|p| (p["width"].as_f64().unwrap(), p["height"].as_f64().unwrap()))
        .collect()
}

/// The first page's size in points as the TIFF's own tags give it: its
/// pixels over its DPI, or 200 DPI without the tags (research.md §11).
fn expected_first_page_points(tiff: &[u8]) -> (f64, f64) {
    let little = &tiff[..2] == b"II";
    let u16_at = |at: usize| {
        let b = [tiff[at], tiff[at + 1]];
        if little { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) }
    };
    let u32_at = |at: usize| {
        let b = [tiff[at], tiff[at + 1], tiff[at + 2], tiff[at + 3]];
        if little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) }
    };
    let ifd = u32_at(4) as usize;
    let (mut width, mut height) = (0.0, 0.0);
    let (mut x_res, mut y_res, mut unit) = (None, None, 2u16);
    for i in 0..u16_at(ifd) as usize {
        let entry = ifd + 2 + 12 * i;
        let (tag, kind) = (u16_at(entry), u16_at(entry + 2));
        let value = if kind == 3 { u16_at(entry + 8) as u32 } else { u32_at(entry + 8) };
        let rational = |at: usize| u32_at(at) as f64 / u32_at(at + 4) as f64;
        match tag {
            256 => width = value as f64,
            257 => height = value as f64,
            282 => x_res = Some(rational(value as usize)),
            283 => y_res = Some(rational(value as usize)),
            296 => unit = value as u16,
            _ => {}
        }
    }
    let per_inch = |res: Option<f64>| match (res, unit) {
        (Some(r), 2) => r,
        (Some(r), 3) => r * 2.54,
        _ => 200.0,
    };
    (width * 72.0 / per_inch(x_res), height * 72.0 / per_inch(y_res))
}

/// Sets the compression tag of page `page` (0-based) of a little-endian TIFF
/// to a value no decoder knows, leaving every other page alone.
fn break_compression_of(tiff: &mut [u8], page: usize, value: u16) {
    let mut ifd = u32::from_le_bytes(tiff[4..8].try_into().unwrap()) as usize;
    for current in 0.. {
        let count = u16::from_le_bytes([tiff[ifd], tiff[ifd + 1]]) as usize;
        if current == page {
            for i in 0..count {
                let entry = ifd + 2 + 12 * i;
                if u16::from_le_bytes([tiff[entry], tiff[entry + 1]]) == 259 {
                    tiff[entry + 8..entry + 10].copy_from_slice(&value.to_le_bytes());
                    return;
                }
            }
            panic!("no compression tag on page {page}");
        }
        let next = ifd + 2 + 12 * count;
        ifd = u32::from_le_bytes(tiff[next..next + 4].try_into().unwrap()) as usize;
        assert_ne!(ifd, 0, "no page {page}");
    }
}

// --- Opening and rendering (US1-2, US1-3) ---------------------------------------

#[test]
fn a_single_page_opens_with_its_size_in_points_at_its_dpi() {
    let _serial = serial();
    let world = Fixture::new();
    let bytes = document_fixture("one-page.tif");
    let (want_w, want_h) = expected_first_page_points(&bytes);
    let id = world.add("one-page.tif", &bytes);

    let info = world.open_ok(id);

    assert_eq!(info["kind"], "tiff");
    assert_eq!(info["documentId"], id);
    let pages = pages_of(&info);
    assert_eq!(pages.len(), 1);
    assert!((pages[0].0 - want_w).abs() < 0.5 && (pages[0].1 - want_h).abs() < 0.5, "{pages:?}");
    assert!(world.helper_pid().is_some_and(|pid| !preview_support::process_is_gone(pid)));
}

#[test]
fn without_dpi_tags_a_page_is_sized_at_200_dpi() {
    let _serial = serial();
    let world = Fixture::new();
    // 48 x 48 pixels and no resolution tags: 48 * 72 / 200 points.
    let id = world.add("scan.tif", &hostile::tiff(1).bytes);

    let pages = pages_of(&world.open_ok(id));

    assert_eq!(pages.len(), 1);
    assert!((pages[0].0 - 17.28).abs() < 0.01 && (pages[0].1 - 17.28).abs() < 0.01, "{pages:?}");
}

#[test]
fn four_pages_open_and_each_renders_to_the_width_asked() {
    let _serial = serial();
    let world = Fixture::new();
    let id = world.add("four-pages-mixed.tif", &document_fixture("four-pages-mixed.tif"));

    let info = world.open_ok(id);
    let preview_id = Fixture::id_of(&info);

    assert_eq!(pages_of(&info).len(), 4);
    let helper = world.helper_pid().unwrap();
    for page in 0..4 {
        let png =
            render(&world, preview_id, page, 100).unwrap_or_else(|e| panic!("page {page}: {e}"));
        let (width, height) = png_size(&png);
        assert_eq!(width, 100, "page {page}");
        assert!(height > 0);
    }
    assert_eq!(world.helper_pid(), Some(helper), "one helper for the whole preview");
}

/// The human-testing seed's scan (SOURCE.md): three US Letter pages at
/// 200 DPI, 1-bit CCITT G4, so the viewer shows a real-sized document and
/// not a thumbnail zoomed to fit.
#[test]
fn the_seeded_bill_of_sale_scan_is_three_letter_pages_at_200_dpi() {
    let _serial = serial();
    let world = Fixture::new();
    let id = world.add("bill-of-sale-scan.tif", &document_fixture("bill-of-sale-scan.tif"));

    let info = world.open_ok(id);

    assert_eq!(info["kind"], "tiff");
    let pages = pages_of(&info);
    assert_eq!(pages.len(), 3);
    for (n, (width, height)) in pages.iter().enumerate() {
        assert!((width - 612.0).abs() < 0.5 && (height - 792.0).abs() < 0.5, "page {n}: {pages:?}");
    }
    let png = render(&world, Fixture::id_of(&info), 0, 800).unwrap();
    let (width, height) = png_size(&png);
    assert_eq!(width, 800);
    assert!((1034..=1036).contains(&height), "800 px wide, so about 1035 high: {height}");
}

#[test]
fn a_page_with_an_unsupported_compression_fails_alone() {
    let _serial = serial();
    let world = Fixture::new();
    let mut tiff = hostile::tiff(3).bytes;
    break_compression_of(&mut tiff, 1, 9999);
    let preview_id = Fixture::id_of(&world.open_ok(world.add("mixed.tif", &tiff)));

    assert!(render(&world, preview_id, 0, 48).is_ok());
    assert_eq!(render(&world, preview_id, 1, 48).unwrap_err(), "PREVIEW_PAGE_FAILED");
    assert!(render(&world, preview_id, 2, 48).is_ok(), "the others still render");
    assert!(render(&world, preview_id, 0, 48).is_ok());
}

#[test]
fn an_unreadable_first_ifd_fails_the_open_as_damaged() {
    let _serial = serial();
    let world = Fixture::new();
    let mut past_the_end = hostile::tiff(1).bytes;
    past_the_end[4..8].copy_from_slice(&1_000_000u32.to_le_bytes());
    let header_only = b"II*\0\x08\0\0\0".to_vec();

    for (name, bytes) in [("past-the-end.tif", past_the_end), ("header-only.tif", header_only)] {
        let id = world.add(name, &bytes);

        assert_eq!(code_of(world.open(id)), "PREVIEW_DAMAGED", "{name}");

        assert_eq!(world.helper_pid(), None, "no preview is left open");
        assert!(world.session.read(|_| Ok(())).is_ok(), "the session is usable");
    }
}

#[test]
fn each_crafted_tiff_fails_cleanly_and_the_session_stays_usable() {
    let _serial = serial();
    let mut world = Fixture::new();
    // The 2^31-page chain and the 100,000 x 100,000 page have to stop at a
    // limit: keep the wait short.
    world.preview.env.helper_limits.load = Duration::from_secs(8);
    world.preview.env.helper_limits.render = Duration::from_secs(8);
    let good = world.add("good.tif", &document_fixture("one-page.tif"));

    for crafted in hostile::hostile_tiffs() {
        let id = world.add(&crafted.name, &crafted.bytes);
        let started = Instant::now();
        match world.open(id) {
            Err(e) => assert!(
                ["PREVIEW_DAMAGED", "PREVIEW_FAILED"].contains(&e.code.as_str()),
                "{}: {}",
                crafted.name,
                e.code
            ),
            Ok(info) => {
                let preview_id = Fixture::id_of(&info);
                for page in 0..pages_of(&info).len().min(3) as u32 {
                    if let Err(code) = render(&world, preview_id, page, 64) {
                        assert!(
                            ["PREVIEW_PAGE_FAILED", "PREVIEW_FAILED"].contains(&code.as_str()),
                            "{} page {page}: {code}",
                            crafted.name
                        );
                    }
                }
                world.close(preview_id);
            }
        }
        assert!(started.elapsed() < Duration::from_secs(40), "{} took too long", crafted.name);
        assert_eq!(world.helper_pid(), None, "{}", crafted.name);
        assert!(world.session.read(|_| Ok(())).is_ok(), "{}: the session is usable", crafted.name);
        assert_eq!(
            world.open_ok(good)["kind"],
            "tiff",
            "{}: and so is the next preview",
            crafted.name
        );
    }
}

// --- Closing at once (spec Edge Cases, "A large document") ------------------------

/// A valid 8192-pixel-wide TIFF of at least 100 MB, made once.
fn large() -> &'static hostile::Generated {
    static LARGE: OnceLock<hostile::Generated> = OnceLock::new();
    LARGE.get_or_init(|| {
        let large = hostile::large_tiff(100_000_000);
        assert!(large.bytes.len() >= 100_000_000);
        large
    })
}

#[test]
fn closing_during_the_load_of_a_large_tiff_is_prompt_and_kills_the_helper() {
    let _serial = serial();
    let world = Fixture::new();
    let large_id = world.add(&large().name, &large().bytes);
    // Learn where the session's preview ids are, so the id of the next open
    // is known before `open_preview` returns it.
    let text = world.add("note.txt", b"hello");
    let current = Fixture::id_of(&world.open_ok(text));
    world.close(current);
    let next = current + 1;

    let opener = {
        let (session, preview_env) = (world.session.clone(), &world.preview.env);
        std::thread::scope(|scope| {
            let opening = scope.spawn(|| preview::open_preview(&session, preview_env, large_id));
            let pid = wait_for_helper_child(Duration::from_secs(10));
            if pid.is_none() && cfg!(not(target_os = "linux")) {
                std::thread::sleep(Duration::from_millis(20));
            }

            let started = Instant::now();
            preview::close_preview(&world.session, next).unwrap();
            let closed_in = started.elapsed();
            let usable_at = Instant::now();
            world.session.read(|_| Ok(())).unwrap();

            assert!(closed_in < Duration::from_millis(100), "close_preview took {closed_in:?}");
            assert!(usable_at.elapsed() < Duration::from_millis(100), "the session was held up");
            if let Some(pid) = pid {
                assert_gone(pid, "after close_preview");
            }
            opening.join().unwrap()
        })
    };

    // Either the load had finished (and the close ended the preview) or it
    // was cut short; no preview is left open either way.
    drop(opener);
    assert_eq!(world.helper_pid(), None);
    assert!(preview_support::helper_children().is_empty());
}

// --- A crashing helper (research.md §11 "Failures") -------------------------------

#[cfg(unix)]
#[test]
fn a_killed_helper_fails_that_page_is_restarted_and_a_third_crash_ends_the_preview() {
    let _serial = serial();
    let world = Fixture::new();
    let id = world.add("three.tif", &hostile::tiff(3).bytes);
    let preview_id = Fixture::id_of(&world.open_ok(id));
    let kill = |pid: u32| {
        // SAFETY: SIGKILL to a child this test started.
        assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGKILL) }, 0);
        assert!(wait_until_gone(pid, Duration::from_secs(3)));
    };

    let mut seen = vec![world.helper_pid().unwrap()];
    for crash in 1..=2 {
        kill(*seen.last().unwrap());
        assert_eq!(
            render(&world, preview_id, 0, 48).unwrap_err(),
            "PREVIEW_PAGE_FAILED",
            "crash {crash} fails that page"
        );
        assert!(render(&world, preview_id, 1, 48).is_ok(), "restarted for the next page");
        let pid = world.helper_pid().expect("the preview stays open");
        assert!(!seen.contains(&pid), "a new helper");
        seen.push(pid);
    }

    kill(*seen.last().unwrap());
    assert_eq!(render(&world, preview_id, 0, 48).unwrap_err(), "PREVIEW_FAILED", "the third crash");

    assert_eq!(
        render(&world, preview_id, 1, 48).unwrap_err(),
        "PREVIEW_CLOSED",
        "and it is closed"
    );
    assert_eq!(world.helper_pid(), None);
    assert!(preview_support::helper_children().is_empty());
}

// --- Page sizes and requests (research.md §14) ------------------------------------

#[test]
fn the_width_is_clamped_to_4096_pixels() {
    let _serial = serial();
    let world = Fixture::new();
    // 8192 x 100 pixels.
    let wide = hostile::large_tiff(8192 * 100);
    let preview_id = Fixture::id_of(&world.open_ok(world.add(&wide.name, &wide.bytes)));

    let png = render(&world, preview_id, 0, 10_000).unwrap();

    let (width, height) = png_size(&png);
    assert_eq!(width, 4096);
    assert_eq!(height, 50, "the aspect ratio is kept");
}

#[test]
fn the_page_is_clamped_to_24_megapixels_for_its_aspect_ratio() {
    let _serial = serial();
    let mut world = Fixture::new();
    // The decode of 100 MB takes a while in a debug build.
    world.preview.env.helper_limits.render = Duration::from_secs(120);
    let preview_id = Fixture::id_of(&world.open_ok(world.add(&large().name, &large().bytes)));
    let native = png_aspect_of_large();

    // At 4096 wide this page would be about 25 megapixels.
    let png = render(&world, preview_id, 0, 4096).unwrap();

    let (width, height) = png_size(&png);
    let pixels = u64::from(width) * u64::from(height);
    assert!(width <= 4096);
    assert!(pixels <= 24 * 1024 * 1024, "{width} x {height} is {pixels} pixels");
    assert!(pixels > 20_000_000, "{width} x {height}: clamped by more than it needs");
    let aspect = f64::from(width) / f64::from(height);
    assert!((aspect - native).abs() / native < 0.01, "{width} x {height}");
}

/// Width over height of `large()`: 8192 pixels wide, as many rows as its
/// size needs (`hostile::large_tiff`).
fn png_aspect_of_large() -> f64 {
    8192.0 / f64::from(100_000_000u32.div_ceil(8192))
}

#[test]
fn a_bad_page_width_or_kind_of_preview_is_a_validation_error() {
    let _serial = serial();
    let world = Fixture::new();
    let tiff = world.add("two.tif", &hostile::tiff(2).bytes);
    let text = world.add("note.txt", b"hello");
    let pdf = world.add("a.pdf", &document_fixture("three-pages.pdf"));
    let tiff_id = Fixture::id_of(&world.open_ok(tiff));

    assert_eq!(
        render(&world, tiff_id, 2, 100).unwrap_err(),
        "VALIDATION_ERROR",
        "page out of range"
    );
    assert_eq!(render(&world, tiff_id, 0, 0).unwrap_err(), "VALIDATION_ERROR", "widthPx under 1");
    assert!(render(&world, tiff_id, 1, 1).is_ok(), "the last page, one pixel wide");

    let text_id = Fixture::id_of(&world.open_ok(text));
    assert_eq!(render(&world, text_id, 0, 100).unwrap_err(), "VALIDATION_ERROR", "a text preview");
    let pdf_id = Fixture::id_of(&world.open_ok(pdf));
    assert_eq!(render(&world, pdf_id, 0, 100).unwrap_err(), "VALIDATION_ERROR", "a PDF preview");
}

// --- The helper ends with the preview (US1-8) -------------------------------------

#[test]
fn the_helper_is_gone_after_close_preview_and_after_opening_another_document() {
    let _serial = serial();
    let world = Fixture::new();
    let tiff = world.add("a.tif", &document_fixture("one-page.tif"));
    let other_tiff = world.add("b.tif", &document_fixture("four-pages-mixed.tif"));
    let text = world.add("c.txt", b"hello");

    let first = Fixture::id_of(&world.open_ok(tiff));
    let pid = world.helper_pid().unwrap();
    world.close(first);
    assert_gone(pid, "after close_preview");
    assert_eq!(world.helper_pid(), None);

    world.open_ok(tiff);
    let pid = world.helper_pid().unwrap();
    world.open_ok(text);
    assert_gone(pid, "after opening a text document");

    world.open_ok(tiff);
    let pid = world.helper_pid().unwrap();
    world.open_ok(other_tiff);
    assert_gone(pid, "after opening another TIFF");
    let replacement = world.helper_pid().unwrap();
    assert_ne!(replacement, pid);
    assert!(!preview_support::process_is_gone(replacement));
}

// --- Nothing reaches the disk (FR-003, SC-002) ------------------------------------

/// Points `TMPDIR` and the XDG folders at a scratch folder for the test and
/// puts them back after. Only this test changes the environment.
struct ScratchEnv {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl ScratchEnv {
    const VARS: [&'static str; 5] =
        ["TMPDIR", "XDG_CACHE_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME"];

    fn point_at(scratch: &std::path::Path) -> Self {
        let saved = Self::VARS.iter().map(|v| (*v, std::env::var_os(v))).collect();
        for var in Self::VARS {
            let folder = scratch.join(var);
            std::fs::create_dir_all(&folder).unwrap();
            // SAFETY: no other test in this binary reads or writes these
            // variables, and the helper is started with a cleared environment.
            unsafe { std::env::set_var(var, folder) };
        }
        Self { saved }
    }
}

impl Drop for ScratchEnv {
    fn drop(&mut self) {
        for (var, value) in &self.saved {
            // SAFETY: as in `point_at`.
            unsafe {
                match value {
                    Some(value) => std::env::set_var(var, value),
                    None => std::env::remove_var(var),
                }
            }
        }
    }
}

#[test]
fn a_preview_leaves_no_file_holding_the_document() {
    let _serial = serial();
    let scratch = TempDir::new().unwrap();
    let _env = ScratchEnv::point_at(scratch.path());
    let marker: [u8; 64] =
        hoplodex_lib::db::random_hex(32).unwrap().into_bytes().try_into().unwrap();
    let world = Fixture::new();
    let id = world.add("marker.tif", &hostile::marker_tiff(&marker).bytes);

    let info = world.open_ok(id);
    let preview_id = Fixture::id_of(&info);
    render(&world, preview_id, 0, 16).unwrap();
    assert_eq!(any_file_holds(scratch.path(), &marker), None, "during the preview");

    world.close(preview_id);
    assert_eq!(any_file_holds(scratch.path(), &marker), None, "after it");
}
