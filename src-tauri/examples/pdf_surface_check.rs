// The PDF surface check (research.md §23, tasks T005-T008 and T069-T074 of
// specs/007-document-preview/tasks.md): the app's own PDF surface
// (`services::preview::surface`, not a copy) in a window under the same
// isolation as the E2E suite, shown hostile documents, probed from inside and
// driven with real input.
//
//   cargo run --example pdf_surface_check -- check [--scratch DIR]
//       [--screenshot PNG] [--timeout SECONDS] [--input ARG]... [--dns-log FILE]
//       [--scan DIR]... [--skip DIR]... [--hud-on]
//   cargo run --example pdf_surface_check -- shows [--scratch DIR]
//       [--screenshot PNG] [--timeout SECONDS] [--no-probe]
//
// `shows` (T005) opens the window (1000x800 logical), adds the surface over
// the rectangle the page draws (x=40, y=100, 920x660), loads a 3-page PDF,
// waits 4 s for the viewer to draw, saves a screenshot of the whole screen and
// exits.
//
// `check` (T069) serves, one after the other through one surface:
//   1. a hostile PDF: a unique marker, a link, a JavaScript open action, a
//      remote image, a remote font, a form that submits to a remote address,
//      an embedded file and a launch action;
//   2. T004's truncated and bit-flipped PDFs (the viewer may fail on them; the
//      process must stay up);
//   3. a PDF of about 10 MB (pages carrying a large incompressible image),
//      whose time to first paint it prints (SC-001).
// While the hostile PDF is shown it runs the reach probe in every frame a
// script reaches (fetch, image, beacon, WebSocket and WebRTC to a local test
// service and to unresolvable names, a raw IPC request and a command call; on
// Windows Edge's own frames through the DevTools protocol), then, with real
// input, clicks every toolbar button and every link, form button and menu item
// it finds, presses Ctrl/Cmd+S, +P and +O, Escape and F6, and scrolls. It fails
// (exit 4, one "CHECK FAIL" line each) if:
//   - the local test service, the tripwire or (with `--dns-log`, a shim's list of
//     every name any process looked up) the outside saw anything;
//   - a command answered the surface (the main web view's own call is the
//     control: it must be answered);
//   - Escape or F6 didn't reach the main web view, scrolling didn't call
//     `note_activity`, a download was asked for, the surface ended, or another
//     window opened;
//   - any file written during the run, under --scan (default: the scratch
//     folder, the temp folder and the home folder), holds the document's
//     marker (on macOS also a new `WebKitPDFs-*` folder in the temp folder);
//   - the launch action ran.
// `--hud-on` (macOS only) leaves WebKit's PDF HUD on and passes only if the
// watch (T065) deletes Open in Preview's copy, closes the surface and sets the
// hold (FR-003a).
//
// Real input: `--input ARG` (repeated) is the command that posts it, given
// `click X Y`, `rclick X Y`, `scroll X Y N` (negative: up) and `key ...`
// after it with screen coordinates: examples/pdf_surface_input.py (Linux, with
// python3), pdf_surface_input.swift (macOS, compiled) or pdf_surface_input.ps1
// (Windows). Without it the input steps are skipped and said to be.
//
// Exit codes:
//   0  passed (shows: the 3-page PDF was drawn inside the red rectangle, look
//      at the screenshot)
//   1  the window or the surface couldn't be built
//   2  a document didn't load within --timeout (default 40 s)
//   3  usage
//   4  a check failed
// Everything is logged to stderr with a "CHECK" prefix. Nothing outside the
// scratch folder is touched but what the check scans and the surface's own
// folders under the scratch folder. On Linux run it with
// `scripts/dev-container.sh scripts/pdf-surface-check.sh`.

use std::{
    collections::HashSet,
    io::Read,
    net::{TcpListener, UdpSocket},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

use hoplodex_lib::services::preview::{
    PdfEndReason,
    surface::{Hooks, Rect, Surface, SurfaceConfig},
    tripwire,
};
#[cfg(target_os = "macos")]
use hoplodex_lib::services::{
    machine_settings::MachineSettings, preview::availability::PdfAvailability,
};
use serde_json::Value;
use tauri::{
    AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder, Wry, http, webview::PageLoadEvent,
};

const SCHEME: &str = "hdpreview";
/// The main window's own page comes from a protocol too (`data:` URLs need a
/// Tauri feature the app doesn't use).
const MAIN_SCHEME: &str = "hdmain";
const THREE_PAGES: &[u8] = include_bytes!("../tests/fixtures/documents/three-pages.pdf");
const TRUNCATED: &[u8] = include_bytes!("../tests/fixtures/documents/three-pages-truncated.pdf");
const BIT_FLIPPED: &[u8] = include_bytes!("../tests/fixtures/documents/three-pages-bitflipped.pdf");
/// Where the surface goes, in logical pixels, and the page's placeholder for
/// it (a red outline), so a wrong place is visible in the screenshot.
const BOUNDS: Rect = Rect { x: 40.0, y: 100.0, width: 920.0, height: 660.0 };
/// SC-001's budget for a first page.
const FIRST_PAINT_BUDGET: Duration = Duration::from_secs(1);
/// Names the reach probe uses for "the outside", which nothing may look up.
const PROBE_NAME_SUFFIXES: &[&str] = &[".invalid", "example.com"];

// ---------------------------------------------------------------------------
// The documents
// ---------------------------------------------------------------------------

/// A PDF written object by object, with its cross-reference table.
struct PdfWriter {
    out: Vec<u8>,
    offsets: Vec<usize>,
}

impl PdfWriter {
    fn new() -> Self {
        Self { out: b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec(), offsets: Vec::new() }
    }

    /// Adds object `number` (they go in order: a wrong number is a bug here).
    fn object(&mut self, number: usize, body: &str) {
        self.raw_object(number, body.as_bytes());
    }

    fn raw_object(&mut self, number: usize, body: &[u8]) {
        assert_eq!(number, self.offsets.len() + 1, "PDF objects are added in order");
        self.offsets.push(self.out.len());
        self.out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        self.out.extend_from_slice(body);
        self.out.extend_from_slice(b"\nendobj\n");
    }

    fn stream(&mut self, number: usize, dict: &str, data: &[u8]) {
        let mut body = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(b"\nendstream");
        self.raw_object(number, &body);
    }

    fn finish(mut self) -> Vec<u8> {
        let xref = self.out.len();
        let count = self.offsets.len() + 1;
        self.out.extend_from_slice(format!("xref\n0 {count}\n0000000000 65535 f \n").as_bytes());
        for offset in &self.offsets {
            self.out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        self.out.extend_from_slice(
            format!("trailer\n<< /Size {count} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n")
                .as_bytes(),
        );
        self.out
    }
}

/// A PDF string literal's text, escaped.
fn pdf_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
}

/// The hostile document: everything a PDF can ask a viewer to reach or run.
/// `marker` is in its text and its embedded file (so a copy on disk is found),
/// `launch` is a file the launch action names, which the check made.
fn hostile_pdf(marker: &str, launch: &Path) -> Vec<u8> {
    let page_one = format!(
        "BT /F1 24 Tf 72 700 Td ({marker} page one) Tj ET \
         BT /F1 14 Tf 72 650 Td (The link, the launch action and the form are below) Tj ET \
         0 0 1 rg 72 600 200 20 re f 1 0 0 rg 72 450 200 20 re f \
         q 100 0 0 100 300 400 cm /Im1 Do Q \
         BT /F2 14 Tf 72 380 Td (text in a remote font) Tj ET"
    );
    let page_two = format!("BT /F1 24 Tf 72 700 Td (Second page {marker}) Tj ET");
    let mut pdf = PdfWriter::new();
    pdf.object(
        1,
        "<< /Type /Catalog /Pages 2 0 R /OpenAction 7 0 R /AcroForm 12 0 R \
         /Names << /EmbeddedFiles << /Names [(attachment.txt) 14 0 R] >> >> >>",
    );
    pdf.object(2, "<< /Type /Pages /Kids [3 0 R 8 0 R] /Count 2 >>");
    pdf.object(
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R /F2 10 0 R >> /XObject << /Im1 11 0 R >> >> \
         /Annots [6 0 R 13 0 R 18 0 R] >>",
    );
    pdf.stream(4, "", page_one.as_bytes());
    pdf.object(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
    pdf.object(
        6,
        "<< /Type /Annot /Subtype /Link /Rect [72 600 272 620] /Border [0 0 0] \
         /A << /S /URI /URI (http://example.com/link-target) >> >>",
    );
    pdf.object(
        7,
        "<< /S /JavaScript /JS (app.alert\\(\"PDF JAVASCRIPT RAN\"\\); \
         app.launchURL\\(\"http://dns-leak-js.invalid/\"\\);) >>",
    );
    pdf.object(
        8,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 9 0 R \
         /Resources << /Font << /F1 5 0 R >> >> >>",
    );
    pdf.stream(9, "", page_two.as_bytes());
    pdf.object(
        10,
        "<< /Type /Font /Subtype /Type1 /BaseFont /RemoteFont /FontDescriptor 16 0 R >>",
    );
    pdf.stream(
        11,
        "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray \
         /BitsPerComponent 8 /F << /FS /URL /F (http://dns-leak-img.invalid/image.png) >>",
        b"",
    );
    pdf.object(12, "<< /Fields [18 0 R] >>");
    pdf.object(
        13,
        &format!(
            "<< /Type /Annot /Subtype /Link /Rect [72 450 272 470] /Border [0 0 0] \
             /A << /S /Launch /F ({}) /Win << /F ({}) >> >> >>",
            pdf_text(&launch.to_string_lossy()),
            pdf_text(&launch.to_string_lossy())
        ),
    );
    pdf.object(14, "<< /Type /Filespec /F (attachment.txt) /EF << /F 15 0 R >> >>");
    pdf.stream(15, "/Type /EmbeddedFile", format!("{marker} embedded file").as_bytes());
    pdf.object(
        16,
        "<< /Type /FontDescriptor /FontName /RemoteFont /Flags 32 /FontBBox [0 0 1000 1000] \
         /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontFile 17 0 R >>",
    );
    pdf.stream(17, "/F << /FS /URL /F (http://dns-leak-font.invalid/font.pfb) >>", b"");
    pdf.object(
        18,
        "<< /Type /Annot /Subtype /Widget /FT /Btn /Ff 65536 /T (submit) /Rect [72 500 172 520] \
         /MK << /CA (Submit) >> /A << /S /SubmitForm \
         /F << /FS /URL /F (http://dns-leak-submit.invalid/submit) >> >> >>",
    );
    pdf.finish()
}

/// A PDF of about 10 MB: two pages, each a full-page image of random bytes
/// (which no compression shrinks), as the E2E suite's `largePdf.ts` builds.
fn large_pdf() -> Vec<u8> {
    const SIDE: usize = 1290;
    let mut pdf = PdfWriter::new();
    pdf.object(1, "<< /Type /Catalog /Pages 2 0 R >>");
    pdf.object(2, "<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>");
    for (page, (contents, image)) in [(3, (4, 5)), (6, (7, 8))] {
        pdf.object(
            page,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {contents} 0 R \
                 /Resources << /XObject << /Im {image} 0 R >> >> >>"
            ),
        );
        pdf.stream(contents, "", b"q 612 0 0 792 0 0 cm /Im Do Q");
        let mut pixels = vec![0u8; SIDE * SIDE * 3];
        getrandom::fill(&mut pixels).unwrap();
        pdf.stream(
            image,
            &format!(
                "/Type /XObject /Subtype /Image /Width {SIDE} /Height {SIDE} \
                 /ColorSpace /DeviceRGB /BitsPerComponent 8"
            ),
            &pixels,
        );
    }
    pdf.finish()
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).unwrap();
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// The probe: what a script in a frame of the surface can reach
// ---------------------------------------------------------------------------

/// An async function that fills in and returns the report it is given: what
/// code running in a document (a PDF that exploited the viewer, say) could
/// reach: Tauri's IPC, the parent frame, the local test service and the
/// outside.
const REACH_PROBE: &str = r#"async r => {
  const limit = (p, ms) => Promise.race([p, new Promise(res => setTimeout(() => res('timeout'), ms))]);
  r.href = location.href; r.origin = location.origin;
  r.internals = typeof window.__TAURI_INTERNALS__;
  r.webkitIpc = typeof (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc);
  r.chromeIpc = typeof (window.chrome && window.chrome.webview);
  try { r.parentInternals = typeof parent.__TAURI_INTERNALS__; } catch (e) { r.parentInternals = 'ERR ' + String(e).slice(0, 60); }
  const inv = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke;
  if (inv) {
    try { r.appCmd = 'OK ' + await limit(inv('get_chooser_state'), 3000); } catch (e) { r.appCmd = 'ERR ' + String(e).slice(0, 80); }
    try { r.pluginCmd = 'OK ' + await limit(inv('plugin:window|title', { label: 'preview' }), 3000); } catch (e) { r.pluginCmd = 'ERR ' + String(e).slice(0, 80); }
  }
  const msg = JSON.stringify({ cmd: 'get_chooser_state', callback: 1, error: 2, payload: {}, options: {} });
  try {
    if (r.webkitIpc === 'object') window.webkit.messageHandlers.ipc.postMessage(msg);
    if (r.chromeIpc === 'object') window.chrome.webview.postMessage(msg);
    r.postMessage = 'sent';
  } catch (e) { r.postMessage = 'ERR ' + String(e).slice(0, 60); }
  try {
    const x = await limit(fetch('__IPC__get_chooser_state', { method: 'POST', body: '{}',
      headers: { 'Content-Type': 'application/json', 'Tauri-Callback': '1', 'Tauri-Error': '2', 'Tauri-Invoke-Key': 'guess' } }), 4000);
    r.rawIpc = x === 'timeout' ? x : x.status + ' ' + (await x.text()).slice(0, 60);
  } catch (e) { r.rawIpc = 'ERR ' + String(e).slice(0, 60); }
  const f = async u => { try { const x = await limit(fetch(u, { mode: 'no-cors' }), 4000); return x === 'timeout' ? x : 'reached (' + x.type + ')'; } catch (e) { return 'ERR ' + String(e).slice(0, 50); } };
  const ws = u => new Promise(res => { try { const s = new WebSocket(u); s.onopen = () => { res('OPEN'); s.close(); }; s.onerror = () => res('error'); setTimeout(() => res('timeout'), 4000); } catch (e) { res('ERR ' + String(e).slice(0, 50)); } });
  const img = u => new Promise(res => { const i = new Image(); i.onload = () => res('loaded'); i.onerror = () => res('error'); setTimeout(() => res('timeout'), 4000); i.src = u; });
  const rtc = async () => {
    if (typeof RTCPeerConnection === 'undefined') return 'absent';
    try {
      const pc = new RTCPeerConnection({ iceServers: [{ urls: 'stun:127.0.0.1:__UDP__' }, { urls: 'stun:dns-leak-stun.invalid:3478' }] });
      const found = []; pc.onicecandidate = e => { if (e.candidate) found.push(e.candidate.candidate.slice(0, 70)); };
      pc.createDataChannel('x'); await pc.setLocalDescription(await pc.createOffer());
      await new Promise(res => setTimeout(res, 3000)); pc.close();
      return 'present; candidates=' + JSON.stringify(found);
    } catch (e) { return 'ERR ' + String(e).slice(0, 60); }
  };
  const link = document.createElement('link'); link.rel = 'dns-prefetch'; link.href = '//dns-leak-prefetch.invalid';
  (document.head || document.documentElement).appendChild(link);
  try { r.beacon = String(navigator.sendBeacon('http://127.0.0.1:__TCP__/beacon', 'x')); } catch (e) { r.beacon = 'ERR ' + String(e).slice(0, 50); }
  [r.netExt, r.netDns, r.netLoop, r.netLocalhost, r.wsLoop, r.wsExt, r.imgLoop, r.imgExt, r.rtc] = await Promise.all([
    f('http://example.com/frame-fetch'), f('http://dns-leak-fetch.invalid/'),
    f('http://127.0.0.1:__TCP__/fetch-loop'), f('http://localhost:__TCP__/fetch-localhost'),
    ws('ws://127.0.0.1:__TCP__/ws-loop'), ws('ws://dns-leak-ws.invalid/'),
    img('http://127.0.0.1:__TCP__/img-loop'), img('http://dns-leak-img.invalid/x.png'), rtc()]);
  return r;
}"#;

/// Run in every frame (a user script on Linux, `eval` in the top frame
/// elsewhere). It reports to the check through `hdpreview` (a request the
/// surface's filter lets through and the check's handler answers), runs the
/// reach probe 3 s after the frame starts, and in PDF.js's frame (Linux)
/// reports when page 1 is drawn and where its buttons and links are.
const FRAME_PROBE: &str = r#"
(() => {
  if (window.__hdProbeRan) return;
  window.__hdProbeRan = true;
  const isTop = window === window.top;
  // PDF.js's frame can't load an `hdpreview:` image itself (the E2E run found
  // it), so a child frame tells its top frame, which makes the request.
  const send = (k, r) => { try { new Image().src = '__BASE____report?k=' + k + '&d=' + encodeURIComponent(JSON.stringify(r)); } catch (e) {} };
  const report = (k, r) => { if (isTop) send(k, r); else { try { parent.postMessage({ hdProbe: [k, r] }, '*'); } catch (e) {} } };
  if (isTop) addEventListener('message', e => { if (e.data && e.data.hdProbe) send(e.data.hdProbe[0], e.data.hdProbe[1]); });
  const REACH = __REACH__;
  setTimeout(async () => {
    const r = { step: 'reach', frame: isTop ? 'top' : 'child' };
    try { await REACH(r); } catch (e) { r.fatal = String(e); }
    report('reach', r);
  }, 3000);
  if (location.protocol !== 'webkit-pdfjs-viewer:') return;
  const rect = el => {
    const b = el.getBoundingClientRect();
    return b.width > 0 && b.height > 0 ? { x: b.x + b.width / 2, y: b.y + b.height / 2, w: b.width, h: b.height } : null;
  };
  const key = el => el.id || el.getAttribute('title') || el.getAttribute('data-l10n-id') || (el.textContent || '').trim().slice(0, 30) || el.tagName;
  let painted = false;
  const tick = () => {
    if (!painted && document.querySelector('.page[data-loaded]')) { painted = true; report('painted', { at: performance.now() }); }
    const pick = (selector, withHref) => Array.from(document.querySelectorAll(selector))
      .filter(el => !el.disabled).map(el => ({ key: key(el), href: withHref ? el.getAttribute('href') : null, ...rect(el) }))
      .filter(e => e.x !== undefined);
    const page = document.querySelector('.page');
    report('layout', {
      buttons: pick('#toolbarContainer button, #toolbarContainer input[type=button], #secondaryToolbar button, #sidebarContainer button'),
      links: pick('.annotationLayer a, .annotationLayer section.buttonWidgetAnnotation, .annotationLayer input, .annotationLayer button', true),
      page: page ? rect(page) : null,
      view: { w: innerWidth, h: innerHeight },
    });
  };
  setInterval(tick, 300);
})();
"#;

/// What the script's placeholders become for this run.
fn fill_probe(script: &str, base: &str, tcp: u16, udp: u16) -> String {
    let ipc = if cfg!(windows) { "http://ipc.localhost/" } else { "ipc://localhost/" };
    script
        .replace("__REACH__", REACH_PROBE)
        .replace("__IPC__", ipc)
        .replace("__BASE__", base)
        .replace("__TCP__", &tcp.to_string())
        .replace("__UDP__", &udp.to_string())
}

/// The failures a reach report holds: each field says what the frame could
/// reach, and nothing may be reached.
fn judge_reach(report: &Value) -> Vec<String> {
    let frame = report["frame"].as_str().unwrap_or("?");
    let text = |name: &str| report[name].as_str().unwrap_or("");
    let mut failures = Vec::new();
    // Windows' top frame is refused by answering 403, which a no-cors fetch
    // reports as a response: what reached the proxy is `judge_proxied`'s.
    let refused_by_answer = cfg!(windows) && frame == "top";
    for name in ["netExt", "netDns", "netLoop", "netLocalhost"] {
        if !refused_by_answer && text(name).starts_with("reached") {
            failures.push(format!("the {frame} frame's fetch ({name}) reached its target"));
        }
    }
    for name in ["wsLoop", "wsExt"] {
        if text(name) == "OPEN" {
            failures.push(format!("the {frame} frame opened a WebSocket ({name})"));
        }
    }
    for name in ["imgLoop", "imgExt"] {
        if text(name) == "loaded" {
            failures.push(format!("the {frame} frame loaded an image ({name})"));
        }
    }
    for name in ["appCmd", "pluginCmd"] {
        if text(name).starts_with("OK") {
            failures.push(format!("the {frame} frame's command call ({name}) was answered"));
        }
    }
    if text("rawIpc").starts_with('2') {
        failures
            .push(format!("the {frame} frame's raw IPC request was answered: {}", text("rawIpc")));
    }
    if let Some(fatal) = report["fatal"].as_str() {
        failures.push(format!("the {frame} frame's probe failed to run: {fatal}"));
    }
    failures
}

// ---------------------------------------------------------------------------
// What the check watches
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Shared {
    /// What the surface's hooks were called with.
    escape: AtomicUsize,
    focus_chrome: AtomicUsize,
    activity: AtomicUsize,
    downloads: Mutex<Vec<String>>,
    ended: Mutex<Vec<PdfEndReason>>,
    /// What reached the main web view (it ran script the hooks sent it).
    main_saw_escape: AtomicUsize,
    main_saw_focus_chrome: AtomicUsize,
    main_ticks: AtomicUsize,
    /// `get_chooser_state` answered, by the label of the web view that asked.
    answered: Mutex<Vec<String>>,
    /// The main web view's own call: whether the control was answered.
    control: Mutex<Option<String>>,
    /// The local test service's connections and packets.
    victim_tcp: AtomicUsize,
    victim_udp: AtomicUsize,
    /// Reports from frames, as `(kind, json)`; the latest layout apart.
    reports: Mutex<Vec<(String, Value, Instant)>>,
    layout: Mutex<Option<Value>>,
    /// Documents by path, and the paths served.
    documents: Mutex<Vec<(String, Arc<Vec<u8>>)>>,
    served: Mutex<Vec<(String, Instant)>>,
    /// Page-load events as `(finished, url, when)`.
    loads: Mutex<Vec<(bool, Url, Instant)>>,
    /// Windows: what reached the surface's proxy, as `(stage, request line)`.
    proxied: Mutex<Vec<(&'static str, String)>>,
}

impl Shared {
    fn report_count(&self, kind: &str) -> usize {
        self.reports.lock().unwrap().iter().filter(|(k, ..)| k == kind).count()
    }

    fn reports_of(&self, kind: &str) -> Vec<Value> {
        self.reports
            .lock()
            .unwrap()
            .iter()
            .filter(|(k, ..)| k == kind)
            .map(|(_, v, _)| v.clone())
            .collect()
    }

    fn painted_after(&self, since: Instant) -> Option<Instant> {
        self.reports
            .lock()
            .unwrap()
            .iter()
            .find(|(k, _, at)| k == "painted" && *at >= since)
            .map(|(_, _, at)| *at)
    }

    fn finished_at(&self, url: &Url) -> Option<Instant> {
        self.loads.lock().unwrap().iter().find(|(f, u, _)| *f && u == url).map(|(_, _, at)| *at)
    }
}

/// A TCP and a UDP port on loopback that count what reaches them: the "local
/// test service" the probe tries to reach.
fn start_victim(shared: &Arc<Shared>) -> (u16, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let tcp = listener.local_addr().unwrap().port();
    let counters = shared.clone();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let n = counters.victim_tcp.fetch_add(1, Ordering::SeqCst) + 1;
            eprintln!("CHECK test service: TCP connection {n}");
            drop(stream);
        }
    });
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let udp = socket.local_addr().unwrap().port();
    let counters = shared.clone();
    thread::spawn(move || {
        let mut buf = [0u8; 1500];
        while let Ok((n, from)) = socket.recv_from(&mut buf) {
            counters.victim_udp.fetch_add(1, Ordering::SeqCst);
            eprintln!("CHECK test service: UDP packet of {n} bytes from {from}");
        }
    });
    (tcp, udp)
}

/// The main window's page: a placeholder where the surface goes, a tick every
/// second (the page must still run with the surface over it), and the control
/// command call.
fn main_page() -> String {
    let Rect { x, y, width, height } = BOUNDS;
    format!(
        "<body style=\"margin:0;background:#dde\">\
         <p style=\"font:16px sans-serif;margin:16px 40px\">HoploDex main window. \
         The PDF surface must fill the red outline.</p>\
         <div style=\"position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;\
         box-sizing:border-box;border:2px solid red\"></div>\
         <script>setInterval(() => fetch('/tick?' + Date.now()), 1000);\
         (async () => {{ try {{\
           const r = await window.__TAURI_INTERNALS__.invoke('get_chooser_state');\
           fetch('/control?ok=1&r=' + encodeURIComponent(r));\
         }} catch (e) {{ fetch('/control?ok=0&r=' + encodeURIComponent(String(e).slice(0, 100))); }} }})();\
         </script></body>",
        x - 2.0,
        y - 2.0,
        width + 4.0,
        height + 4.0
    )
}

/// Stands in for any HoploDex command the main window is allowed (it is in the
/// build's app manifest and the window's capability) and the surface is not:
/// the surface's call, were it answered, is a failure.
#[tauri::command]
fn get_chooser_state(webview: tauri::Webview, shared: tauri::State<'_, Arc<Shared>>) -> String {
    eprintln!(
        "CHECK command answered for web view {} (in window {})",
        webview.label(),
        webview.window().label()
    );
    shared.answered.lock().unwrap().push(webview.label().to_string());
    "answered".into()
}

/// The document's address: custom protocols are `<scheme>://localhost/` on
/// Linux and macOS, `http://<scheme>.localhost/` on Windows (research.md §4).
fn protocol_base(scheme: &str) -> String {
    if cfg!(windows) {
        format!("http://{scheme}.localhost/")
    } else {
        format!("{scheme}://localhost/")
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| percent_decode(value))
    })
}

/// The `hdpreview` protocol: the documents under `/<token>/`, `__report` for
/// the frame probe, and an empty answer for the frame script's `hooked` call.
fn serve_preview(shared: &Shared, req: &http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let path = req.uri().path().to_string();
    if path == "/__report" {
        let query = req.uri().query().unwrap_or("");
        if let (Some(kind), Some(data)) = (query_param(query, "k"), query_param(query, "d")) {
            let value: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
            if kind == "layout" {
                *shared.layout.lock().unwrap() = Some(value);
            } else {
                eprintln!("CHECK report {kind} {value}");
                shared.reports.lock().unwrap().push((kind, value, Instant::now()));
            }
        }
        return http::Response::builder().status(204).body(Vec::new()).unwrap();
    }
    if path.starts_with("/hooked/") {
        return http::Response::builder().status(204).body(Vec::new()).unwrap();
    }
    eprintln!("CHECK protocol request {} {}", req.method(), req.uri());
    let document = shared
        .documents
        .lock()
        .unwrap()
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, bytes)| bytes.clone());
    match document {
        Some(bytes) => {
            shared.served.lock().unwrap().push((path, Instant::now()));
            http::Response::builder()
                .header("Content-Type", "application/pdf")
                .header("Cache-Control", "no-store")
                .body(bytes.to_vec())
                .unwrap()
        }
        None => http::Response::builder().status(404).body(Vec::new()).unwrap(),
    }
}

/// The main window's own protocol: its page, the tick, what the hooks sent it
/// and the control command's answer.
fn serve_main(shared: &Shared, req: &http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let empty = || http::Response::builder().body(Vec::new()).unwrap();
    match req.uri().path() {
        "/tick" => {
            shared.main_ticks.fetch_add(1, Ordering::SeqCst);
            empty()
        }
        "/seen/escape" => {
            shared.main_saw_escape.fetch_add(1, Ordering::SeqCst);
            empty()
        }
        "/seen/focus-chrome" => {
            shared.main_saw_focus_chrome.fetch_add(1, Ordering::SeqCst);
            empty()
        }
        "/control" => {
            let query = req.uri().query().unwrap_or("");
            let ok = query_param(query, "ok").as_deref() == Some("1");
            let result = query_param(query, "r").unwrap_or_default();
            eprintln!(
                "CHECK control: the main web view's command call {}: {result}",
                if ok { "was answered" } else { "FAILED" }
            );
            *shared.control.lock().unwrap() = Some(if ok { "answered".into() } else { result });
            empty()
        }
        _ => http::Response::builder()
            .header("Content-Type", "text/html")
            .body(main_page().into_bytes())
            .unwrap(),
    }
}

/// The surface's hooks as the app would run them, counted, and the main web
/// view told (so that "reached the main web view" is a script running there).
fn counting_hooks(shared: &Arc<Shared>, app: &AppHandle<Wry>) -> Arc<Hooks> {
    let tell_main = |app: AppHandle<Wry>, what: &'static str| {
        move || {
            // `get_webview` (not `get_webview_window`, which is `None` once the
            // window has the surface's web view too).
            if let Some(main) = app.get_webview("main") {
                let _ = main.eval(format!("fetch('/seen/{what}')"));
            }
        }
    };
    let (s1, s2, s3, s4, s5) =
        (shared.clone(), shared.clone(), shared.clone(), shared.clone(), shared.clone());
    let (escape, focus) =
        (tell_main(app.clone(), "escape"), tell_main(app.clone(), "focus-chrome"));
    Hooks::new(
        Box::new(move || {
            s1.escape.fetch_add(1, Ordering::SeqCst);
            escape();
        }),
        Box::new(move || {
            s2.focus_chrome.fetch_add(1, Ordering::SeqCst);
            focus();
        }),
        Box::new(move || {
            s3.activity.fetch_add(1, Ordering::SeqCst);
        }),
        Box::new(move |reason| {
            eprintln!("CHECK the surface ended: {reason:?}");
            s4.ended.lock().unwrap().push(reason);
        }),
        Box::new(move |url| {
            eprintln!("CHECK a download was asked for: {url}");
            s5.downloads.lock().unwrap().push(url.to_string());
        }),
    )
}

// ---------------------------------------------------------------------------
// Real input
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Key {
    Escape,
    F6,
    Save,
    Print,
    Open,
}

/// Posts real mouse and keyboard input through the command `--input` names.
struct Input {
    command: Vec<String>,
    /// Where the main window's content area starts on the screen, in the
    /// units the input command takes, and how many of those a logical pixel is.
    origin: (f64, f64),
    unit: f64,
}

impl Input {
    fn run(&self, args: &[String]) {
        let (program, rest) = self.command.split_first().expect("a command");
        match Command::new(program).args(rest).args(args).status() {
            Ok(status) if status.success() => {}
            other => eprintln!("CHECK input {args:?} failed: {other:?}"),
        }
    }

    /// A point of the window (logical pixels) as screen coordinates.
    fn at(&self, x: f64, y: f64) -> [String; 2] {
        [
            format!("{}", (self.origin.0 + x * self.unit).round() as i64),
            format!("{}", (self.origin.1 + y * self.unit).round() as i64),
        ]
    }

    #[cfg(target_os = "macos")]
    fn mouse_move(&self, x: f64, y: f64) {
        let [sx, sy] = self.at(x, y);
        self.run(&["move".into(), sx, sy]);
    }

    fn click(&self, x: f64, y: f64) {
        let [sx, sy] = self.at(x, y);
        self.run(&["click".into(), sx, sy]);
    }

    fn right_click(&self, x: f64, y: f64) {
        let [sx, sy] = self.at(x, y);
        self.run(&["rclick".into(), sx, sy]);
    }

    fn scroll(&self, x: f64, y: f64, notches: i32) {
        let [sx, sy] = self.at(x, y);
        self.run(&["scroll".into(), sx, sy, notches.to_string()]);
    }

    fn key(&self, key: Key) {
        let args: Vec<&str> = if cfg!(target_os = "linux") {
            vec![
                "key",
                match key {
                    Key::Escape => "Escape",
                    Key::F6 => "F6",
                    Key::Save => "Control_L+s",
                    Key::Print => "Control_L+p",
                    Key::Open => "Control_L+o",
                },
            ]
        } else if cfg!(target_os = "macos") {
            // Virtual key codes, with Command for the three shortcuts.
            match key {
                Key::Escape => vec!["key", "53"],
                Key::F6 => vec!["key", "97"],
                Key::Save => vec!["key", "1", "cmd"],
                Key::Print => vec!["key", "35", "cmd"],
                Key::Open => vec!["key", "31", "cmd"],
            }
        } else {
            // SendKeys.
            vec![
                "key",
                match key {
                    Key::Escape => "{ESC}",
                    Key::F6 => "{F6}",
                    Key::Save => "^s",
                    Key::Print => "^p",
                    Key::Open => "^o",
                },
            ]
        };
        self.run(&args.into_iter().map(String::from).collect::<Vec<_>>());
    }
}

// ---------------------------------------------------------------------------
// The check
// ---------------------------------------------------------------------------

struct Options {
    scratch: PathBuf,
    shot: PathBuf,
    timeout: Duration,
    input: Vec<String>,
    dns_log: Option<PathBuf>,
    scan: Vec<PathBuf>,
    skip: Vec<PathBuf>,
    hud_on: bool,
    probe: bool,
}

fn usage() -> ! {
    eprintln!(
        "usage: pdf_surface_check (check | shows) [--scratch DIR] [--screenshot PNG] \
         [--timeout SECONDS]\n       check: [--input ARG]... [--dns-log FILE] [--scan DIR]... \
         [--skip DIR]... [--hud-on]\n       shows: [--no-probe]"
    );
    std::process::exit(3)
}

/// Saves the whole screen with the OS's own tool. Best effort.
fn screenshot(path: &Path) -> bool {
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).ok();
    let status = if cfg!(target_os = "linux") {
        Command::new("import").args(["-window", "root"]).arg(path).status()
    } else if cfg!(target_os = "macos") {
        Command::new("screencapture").arg("-x").arg(path).status()
    } else {
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; \
             $b=[System.Windows.Forms.SystemInformation]::VirtualScreen; \
             $bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height; \
             [System.Drawing.Graphics]::FromImage($bmp).CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size); \
             $bmp.Save('{}')",
            path.display()
        );
        Command::new("powershell").args(["-NoProfile", "-Command", &script]).status()
    };
    status.map(|s| s.success()).unwrap_or(false) && path.exists()
}

/// Windows: the browser processes (those with no `--type=`) with their user
/// data folders and arguments. The surface must have one of its own.
fn log_browser_processes() {
    if !cfg!(windows) {
        return;
    }
    let script = "Get-CimInstance Win32_Process | \
                  Where-Object { $_.Name -eq 'msedgewebview2.exe' -and $_.CommandLine -notmatch '--type=' } | \
                  ForEach-Object { '{0} {1}' -f $_.ProcessId, $_.CommandLine }";
    match Command::new("powershell").args(["-NoProfile", "-Command", script]).output() {
        Ok(out) => {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                eprintln!("CHECK browser process {line}");
            }
        }
        Err(e) => eprintln!("CHECK browser processes not listed: {e}"),
    }
}

/// The `WebKitPDFs-*` folders in the temp folder, where WebKit's Open in
/// Preview writes its copies (macOS).
#[cfg(target_os = "macos")]
fn webkit_pdf_dirs() -> HashSet<PathBuf> {
    std::fs::read_dir(std::env::temp_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("WebKitPDFs-"))
        .map(|e| e.path())
        .collect()
}

fn wait_until(limit: Duration, condition: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    condition()
}

/// Every file under `roots` written since `since` that holds `needle` (ASCII
/// or UTF-16), and how many files were written. `skip` folders are left out.
fn scan_for_marker(
    roots: &[PathBuf],
    skip: &[PathBuf],
    since: SystemTime,
    marker: &str,
) -> (usize, Vec<PathBuf>) {
    let wide: Vec<u8> = marker.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let needles = [marker.as_bytes().to_vec(), wide];
    let mut written = 0;
    let mut holding = Vec::new();
    let mut stack: Vec<PathBuf> = roots.to_vec();
    let mut seen = HashSet::new();
    while let Some(path) = stack.pop() {
        if !seen.insert(path.clone()) || skip.iter().any(|s| path.starts_with(s)) {
            continue;
        }
        if cfg!(unix) && ["/proc", "/sys", "/dev"].iter().any(|p| path == Path::new(p)) {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                stack.extend(entries.flatten().map(|e| e.path()));
            }
        } else if meta.is_file() && meta.modified().is_ok_and(|m| m >= since) {
            written += 1;
            if file_holds(&path, &needles) {
                holding.push(path);
            }
        }
    }
    (written, holding)
}

/// Whether the file holds any of `needles`, read in chunks that overlap by
/// the longest needle so one across a boundary is found.
fn file_holds(path: &Path, needles: &[Vec<u8>]) -> bool {
    const CHUNK: usize = 8 << 20;
    let keep = needles.iter().map(Vec::len).max().unwrap_or(0).saturating_sub(1);
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let mut buf = vec![0u8; CHUNK + keep];
    let mut carried = 0;
    loop {
        let Ok(read) = file.read(&mut buf[carried..]) else { return false };
        if read == 0 {
            return false;
        }
        let end = carried + read;
        if needles.iter().any(|n| buf[..end].windows(n.len()).any(|w| w == n.as_slice())) {
            return true;
        }
        carried = keep.min(end);
        buf.copy_within(end - carried..end, 0);
    }
}

/// A document the check shows next: its path under the token, and what it is.
fn add_document(shared: &Shared, token: &str, name: &str, bytes: Vec<u8>) -> Url {
    let path = format!("/{token}/{name}");
    shared.documents.lock().unwrap().push((path.clone(), Arc::new(bytes)));
    format!("{}{token}/{name}", protocol_base(SCHEME)).parse().unwrap()
}

/// Everything the worker thread needs.
struct Run {
    app: AppHandle<Wry>,
    shared: Arc<Shared>,
    options: Options,
    token: String,
    tcp: u16,
    udp: u16,
    /// Where the surface's network goes: the app's tripwire, and on Windows
    /// the reading proxy.
    proxy: std::net::SocketAddr,
    started: SystemTime,
}

impl Run {
    fn fail(&self, code: i32, what: &str) -> ! {
        eprintln!("CHECK FAILED: {what}");
        screenshot(&self.options.shot.with_extension("failed.png"));
        std::process::exit(code)
    }

    /// Shows `url` in the surface and waits for its page to finish loading;
    /// `None` if it didn't within `limit`.
    fn show(&self, surface: &Surface<Wry>, url: &Url, limit: Duration) -> Option<Duration> {
        let started = Instant::now();
        surface.navigate(url.clone());
        wait_until(limit, || self.shared.finished_at(url).is_some())
            .then(|| self.shared.finished_at(url).unwrap().duration_since(started))
    }

    fn open_surface(&self, url: &Url, hooks: Arc<Hooks>) -> Surface<Wry> {
        let shared = self.shared.clone();
        let config = SurfaceConfig {
            url: url.clone(),
            proxy_url: format!("http://{}", self.proxy).parse().unwrap(),
            data_directory: self.options.scratch.join("cache").join("preview-webview2"),
            bounds: BOUNDS,
            secret: "0".repeat(32),
            hooks,
            on_page_load: Some(Box::new(move |event, url| {
                eprintln!("CHECK page-load {event:?} {url}");
                shared.loads.lock().unwrap().push((
                    matches!(event, PageLoadEvent::Finished),
                    url.clone(),
                    Instant::now(),
                ));
            })),
            // The one place the HUD is left on: this variant's config field
            // (FR-003a). The app's surface never sets it.
            hud_on: self.options.hud_on,
        };
        let window = self.app.get_window("main").unwrap();
        match Surface::open(&window, config) {
            Ok(surface) => {
                eprintln!("CHECK surface built");
                if let Err(e) = surface.set_bounds(BOUNDS, true) {
                    self.fail(1, &format!("set_bounds failed: {e}"));
                }
                surface
            }
            Err(e) => self.fail(1, &format!("the surface could not be built: {e}")),
        }
    }

    fn probe_script(&self) -> String {
        fill_probe(FRAME_PROBE, &protocol_base(SCHEME), self.tcp, self.udp)
    }
}

/// Linux: the probe as a user script in every frame, so it runs in PDF.js's
/// frame as well as the page that holds it. Done on the GTK thread; returns
/// once the script is in.
#[cfg(target_os = "linux")]
fn install_frame_probe(surface: &Surface<Wry>, script: String) -> bool {
    use webkit2gtk::{
        UserContentInjectedFrames, UserContentManagerExt, UserScript, UserScriptInjectionTime,
        WebViewExt,
    };
    let (done, installed) = std::sync::mpsc::channel();
    let sent = surface.webview().with_webview(move |platform| {
        let view = platform.inner();
        let ok = match view.user_content_manager() {
            Some(manager) => {
                manager.add_script(&UserScript::new(
                    &script,
                    UserContentInjectedFrames::AllFrames,
                    UserScriptInjectionTime::Start,
                    &[],
                    &[],
                ));
                true
            }
            None => false,
        };
        let _ = done.send(ok);
    });
    sent.is_ok() && installed.recv_timeout(Duration::from_secs(10)).unwrap_or(false)
}

/// Windows: Edge's viewer draws into a plugin no script reaches, so the first
/// paint of the 10 MB PDF (two pages of random pixels) is seen on the screen:
/// a run of pixels in the surface's page area that are all different.
#[cfg(windows)]
mod pixels {
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
    };

    /// The width of the run of pixels read.
    const RUN: i32 = 24;

    /// Whether the 24 pixels from `(x, y)` rightwards hold at least 12 colours
    /// (a drawn page of noise; the viewer's grey and the toolbar hold 1 or 2).
    /// The run is copied from the screen once, with one `BitBlt` and one
    /// `GetDIBits`, and read from memory: `GetPixel` takes 17-55 ms a call on
    /// the Windows test machine, so 24 of them made the first poll alone take
    /// 1.3 s and the time it ended at was reported as the first paint (T074,
    /// T152); a copy takes well under a millisecond.
    pub fn noisy(x: i32, y: i32) -> bool {
        // SAFETY: the screen's device context and a memory device context and
        // bitmap made for this call, all released before returning.
        unsafe {
            let screen = GetDC(std::ptr::null_mut());
            let memory = CreateCompatibleDC(screen);
            let bitmap = CreateCompatibleBitmap(screen, RUN, 1);
            let previous = SelectObject(memory, bitmap);
            let mut colours = std::collections::HashSet::new();
            if BitBlt(memory, 0, 0, RUN, 1, screen, x, y, SRCCOPY) != 0 {
                let mut info: BITMAPINFO = std::mem::zeroed();
                info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = RUN;
                info.bmiHeader.biHeight = -1;
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB;
                let mut buffer = [0u32; RUN as usize];
                // The bitmap is deselected first, as `GetDIBits` requires.
                SelectObject(memory, previous);
                if GetDIBits(
                    memory,
                    bitmap,
                    0,
                    1,
                    buffer.as_mut_ptr().cast(),
                    &mut info,
                    DIB_RGB_COLORS,
                ) == 1
                {
                    // The colour without the unused fourth byte.
                    colours.extend(buffer.iter().map(|pixel| pixel & 0x00FF_FFFF));
                }
            } else {
                SelectObject(memory, previous);
            }
            DeleteObject(bitmap);
            DeleteDC(memory);
            ReleaseDC(std::ptr::null_mut(), screen);
            colours.len() >= 12
        }
    }

    /// Starts watching the screen at `(x, y)`; the instant it first looks
    /// drawn is put in the returned cell (nothing in 30 s: `None`). `None` is
    /// returned at once if it looks drawn already, since nothing could then
    /// be told apart.
    pub fn watch(
        x: i32,
        y: i32,
    ) -> Option<std::sync::Arc<std::sync::Mutex<Option<std::time::Instant>>>> {
        if noisy(x, y) {
            return None;
        }
        let cell = std::sync::Arc::new(std::sync::Mutex::new(None));
        let found = cell.clone();
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while std::time::Instant::now() < until {
                if noisy(x, y) {
                    *found.lock().unwrap() = Some(std::time::Instant::now());
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        });
        Some(cell)
    }
}

/// macOS: WebKit's PDF viewer draws into a plugin no script reaches, so, as on
/// Windows, the first paint of the 10 MB PDF (two pages of random pixels) is
/// seen on the screen: a run of pixels in the surface's page area that are
/// all different, read from the display with CoreGraphics (Screen Recording
/// permission, which `scripts/tart-vm.sh setup` grants).
#[cfg(target_os = "macos")]
mod pixels {
    use std::ffi::c_void;

    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    struct CGSize {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGMainDisplayID() -> u32;
        fn CGDisplayCreateImageForRect(display: u32, rect: CGRect) -> *mut c_void;
        fn CGImageGetWidth(image: *mut c_void) -> usize;
        fn CGImageGetBitsPerPixel(image: *mut c_void) -> usize;
        fn CGImageGetDataProvider(image: *mut c_void) -> *mut c_void;
        fn CGDataProviderCopyData(provider: *mut c_void) -> *mut c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDataGetBytePtr(data: *mut c_void) -> *const u8;
        fn CFDataGetLength(data: *mut c_void) -> isize;
        fn CFRelease(object: *mut c_void);
    }

    /// Whether the pixels along the top of the 24-point-wide strip at `(x, y)`
    /// (points from the main display's top left) hold at least 12 colours (a
    /// drawn page of noise; the viewer's grey and the toolbar hold 1 or 2).
    /// `None` when the display can't be read.
    pub fn noisy(x: f64, y: f64) -> Option<bool> {
        let rect = CGRect { origin: CGPoint { x, y }, size: CGSize { width: 24.0, height: 1.0 } };
        // SAFETY: CoreGraphics and CoreFoundation calls on the objects they
        // return, each released before returning.
        unsafe {
            let image = CGDisplayCreateImageForRect(CGMainDisplayID(), rect);
            if image.is_null() {
                return None;
            }
            let bytes_per_pixel = CGImageGetBitsPerPixel(image) / 8;
            let width = CGImageGetWidth(image).min(24);
            let data = CGDataProviderCopyData(CGImageGetDataProvider(image));
            let mut colours = std::collections::HashSet::new();
            let mut readable = false;
            if !data.is_null() {
                let length = CFDataGetLength(data) as usize;
                let first_row = std::slice::from_raw_parts(CFDataGetBytePtr(data), length);
                if bytes_per_pixel >= 3 && first_row.len() >= width * bytes_per_pixel {
                    readable = true;
                    for pixel in 0..width {
                        let at = pixel * bytes_per_pixel;
                        colours.insert(first_row[at..at + bytes_per_pixel].to_vec());
                    }
                }
                CFRelease(data);
            }
            CFRelease(image);
            readable.then_some(colours.len() >= 12)
        }
    }

    /// Starts watching the screen at `(x, y)`; the instant it first looks
    /// drawn is put in the returned cell (nothing in 30 s: `None`). `None` is
    /// returned at once if it looks drawn already or can't be read, since
    /// nothing could then be told apart.
    pub fn watch(
        x: f64,
        y: f64,
    ) -> Option<std::sync::Arc<std::sync::Mutex<Option<std::time::Instant>>>> {
        if noisy(x, y) != Some(false) {
            return None;
        }
        let cell = std::sync::Arc::new(std::sync::Mutex::new(None));
        let found = cell.clone();
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while std::time::Instant::now() < until {
                if noisy(x, y) == Some(true) {
                    *found.lock().unwrap() = Some(std::time::Instant::now());
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        });
        Some(cell)
    }
}

/// Windows: Edge's viewer lives in a frame no page script reaches, so the
/// reach probe runs there through the DevTools protocol (attach to every
/// target but the surface's own page, evaluate the probe).
#[cfg(windows)]
mod cdp {
    use super::*;
    use tauri::webview::PlatformWebview;
    use webview2_com::{
        CallDevToolsProtocolMethodCompletedHandler, Microsoft::Web::WebView2::Win32::*,
    };
    use windows::core::{HSTRING, Interface, Result};

    fn call(
        webview: &ICoreWebView2,
        session: Option<&str>,
        method: &str,
        params: &str,
        done: impl FnOnce(String) + 'static,
    ) {
        let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
            move |result: Result<()>, json: String| {
                done(match result {
                    Ok(()) => json,
                    Err(e) => format!("ERR {e}"),
                });
                Ok(())
            },
        ));
        let (method, params) = (HSTRING::from(method), HSTRING::from(params));
        let sent = unsafe {
            match session {
                None => webview.CallDevToolsProtocolMethod(&method, &params, &handler),
                Some(session) => webview.cast::<ICoreWebView2_11>().and_then(|w| {
                    w.CallDevToolsProtocolMethodForSession(
                        &HSTRING::from(session),
                        &method,
                        &params,
                        &handler,
                    )
                }),
            }
        };
        if let Err(e) = sent {
            eprintln!("CHECK cdp {method} not sent: {e}");
        }
    }

    /// Runs `reach` in every target that isn't the surface's own page and
    /// files its report as a `reach` report from the `cdp` frame.
    pub fn probe(wv: &PlatformWebview, reach: String, shared: Arc<Shared>) {
        let Ok(webview) = (unsafe { wv.controller().CoreWebView2() }) else { return };
        let inner = webview.clone();
        call(&webview, None, "Target.getTargets", "{}", move |json| {
            let targets: Value = serde_json::from_str(&json).unwrap_or_default();
            for target in targets["targetInfos"].as_array().into_iter().flatten() {
                let url = target["url"].as_str().unwrap_or_default().to_string();
                if target["type"] == "page" && url.starts_with("http://hdpreview.") {
                    continue;
                }
                eprintln!("CHECK cdp target {} {url}", target["type"]);
                let attach = serde_json::json!({ "targetId": target["targetId"], "flatten": true });
                let (webview, reach, shared) = (inner.clone(), reach.clone(), shared.clone());
                call(
                    &inner.clone(),
                    None,
                    "Target.attachToTarget",
                    &attach.to_string(),
                    move |json| {
                        let attached: Value = serde_json::from_str(&json).unwrap_or_default();
                        let Some(session) = attached["sessionId"].as_str() else {
                            eprintln!("CHECK cdp attach {url}: {json}");
                            return;
                        };
                        let expression = format!(
                            "({reach})({{ step: 'reach', frame: 'cdp' }}).then(r => JSON.stringify(r))"
                        );
                        let params = serde_json::json!({
                            "expression": expression, "awaitPromise": true, "returnByValue": true
                        });
                        call(
                            &webview,
                            Some(session),
                            "Runtime.evaluate",
                            &params.to_string(),
                            move |json| {
                                let result: Value = serde_json::from_str(&json).unwrap_or_default();
                                match result["result"]["value"]
                                    .as_str()
                                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                                {
                                    Some(report) => {
                                        eprintln!("CHECK report reach {report}");
                                        shared.reports.lock().unwrap().push((
                                            "reach".into(),
                                            report,
                                            Instant::now(),
                                        ));
                                    }
                                    None => eprintln!("CHECK cdp viewer {url}: {json}"),
                                }
                            },
                        );
                    },
                );
            }
        });
    }
}

/// How many connections reached the surface's proxy: the app's tripwire's
/// count, or on Windows the reading proxy's.
fn proxied_count(shared: &Shared, wire: &tripwire::Tripwire) -> usize {
    if cfg!(windows) { shared.proxied.lock().unwrap().len() } else { wire.connections() }
}

/// Windows: what the run is doing, for the tripwire sampler's lines.
static STAGE: Mutex<&'static str> = Mutex::new("start");

fn stage(name: &'static str) {
    *STAGE.lock().unwrap_or_else(|e| e.into_inner()) = name;
}

/// Windows: the surface's proxy, in place of the app's tripwire (which only
/// counts): it accepts, reads the request line (a `CONNECT host:port` or a
/// `GET http://host/...`) and closes the connection unanswered, as the
/// tripwire does, and notes what it read with the run's stage and the seconds
/// since the run began, so what reached it can be told apart: WebView2's own
/// hosts (research.md §6), the probe's attempts from the frames the content
/// filter can't see (Edge's viewer, WebSockets), or anything else.
#[cfg(windows)]
fn start_reading_proxy(shared: &Arc<Shared>) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (shared, began) = (shared.clone(), Instant::now());
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let shared = shared.clone();
            thread::spawn(move || {
                let mut stream = stream;
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                let mut buf = [0u8; 512];
                let read = stream.read(&mut buf).unwrap_or(0);
                let line = String::from_utf8_lossy(&buf[..read])
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .chars()
                    .take(120)
                    .collect::<String>();
                let at = began.elapsed().as_secs_f32();
                let stage = *STAGE.lock().unwrap_or_else(|e| e.into_inner());
                eprintln!("CHECK proxy saw at {at:.1}s during {stage}: {line:?}");
                shared.proxied.lock().unwrap().push((stage, line));
            });
        }
    });
    addr
}

/// Windows: judges what reached the surface's proxy (the app's tripwire does
/// the same job elsewhere, where any hit is a failure). On Windows the proxy is
/// the layer that covers Edge's viewer frames and WebSockets, which the
/// content filter can't see (research.md §6), so the probe's attempts from
/// those frames *must* arrive here, and are expected:
/// - a request line that names none of the probe's targets and is not a
///   WebView2 host of its own (research.md §6: `config.edge.skype.com`,
///   `edge.microsoft.com`) fails: it is the document's doing (its link, remote
///   image, font, form, JavaScript) or something unexplained;
/// - in the stage where only the top frame's probe runs, a request (not a
///   WebSocket's CONNECT) got past the content filter and fails;
/// - the DevTools probe's attempts must have arrived, or the proxy layer (or
///   the probe) didn't work;
/// - a connection that sent nothing is Chromium's idle preconnect to its
///   proxy, which names no destination; they are counted.
#[cfg(windows)]
fn judge_proxied(seen: &[(&'static str, String)], victim_port: u16) -> Vec<String> {
    const EDGE_OWN: &[&str] = &[".skype.com", ".microsoft.com", ".msedge.net", ".windows.net"];
    let targets = [
        "example.com/frame-fetch".to_string(),
        "dns-leak-fetch.invalid".to_string(),
        "dns-leak-ws.invalid".to_string(),
        "dns-leak-img.invalid/x.png".to_string(),
        "ipc.localhost/get_chooser_state".to_string(),
        format!("127.0.0.1:{victim_port}"),
        format!("localhost:{victim_port}"),
    ];
    let mut failures = Vec::new();
    let (mut empty, mut own, mut probe, mut probe_by_devtools) = (0, 0, 0, 0);
    for (stage, line) in seen {
        let mut words = line.split_whitespace();
        let (method, target) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
        let host = target
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .split(['/', ':'])
            .next()
            .unwrap_or("");
        if line.is_empty() {
            empty += 1;
        } else if EDGE_OWN.iter().any(|suffix| host.ends_with(suffix)) {
            own += 1;
        } else if targets.iter().any(|t| target.contains(t.as_str())) {
            if *stage == "the top frame's probe" && method != "CONNECT" {
                failures.push(format!("the content filter let the top frame's {line:?} through"));
            } else {
                probe += 1;
                probe_by_devtools += usize::from(*stage == "the probes");
            }
        } else {
            failures.push(format!("{line:?} reached the proxy during {stage}"));
        }
    }
    eprintln!(
        "CHECK proxy: {} connection(s): {empty} with no request, {own} WebView2's own, \
         {probe} the probe's ({probe_by_devtools} of them while the DevTools probe ran)",
        seen.len()
    );
    if probe_by_devtools == 0 {
        failures.push(
            "the DevTools probe's attempts never reached the proxy: Edge's viewer frames \
             were not covered by it, or the probe did not run"
                .into(),
        );
    }
    failures
}

/// The whole check, on a thread of its own (the main thread runs the window).
fn run_check(run: Run) {
    let Run { app, shared, options, .. } = &run;
    let started = Instant::now();
    let mut failures: Vec<String> = Vec::new();
    let wire = tripwire::shared().expect("the tripwire binds");
    let marker = format!("HDSURFACEMARKER{}", random_hex(8));
    let launch = options.scratch.join(if cfg!(windows) { "launch.bat" } else { "launch.sh" });
    let launched = options.scratch.join("launched");
    {
        // What the launch action names: a program that leaves a file, if run.
        let script = if cfg!(windows) {
            format!("@echo off\r\necho x > \"{}\"\r\n", launched.display())
        } else {
            format!("#!/bin/sh\necho x > '{}'\n", launched.display())
        };
        std::fs::write(&launch, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&launch, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    eprintln!("CHECK marker={}... scratch={}", &marker[..10], options.scratch.display());

    let hostile = add_document(shared, &run.token, "hostile.pdf", hostile_pdf(&marker, &launch));
    let truncated = add_document(shared, &run.token, "truncated.pdf", TRUNCATED.to_vec());
    let flipped = add_document(shared, &run.token, "bit-flipped.pdf", BIT_FLIPPED.to_vec());
    let large_bytes = large_pdf();
    eprintln!("CHECK the large PDF is {} bytes", large_bytes.len());
    let large = add_document(shared, &run.token, "large.pdf", large_bytes);
    let warmup: Url = format!("{}{}/warmup.pdf", protocol_base(SCHEME), run.token).parse().unwrap();

    #[cfg(target_os = "macos")]
    let pdf_dirs_before = webkit_pdf_dirs();
    // The tripwire may see WebView2's own traffic on Windows (research.md
    // §6): what it sees while nothing is shown is the baseline.

    stage("the first seconds");
    thread::sleep(Duration::from_secs(2));
    let hooks = counting_hooks(shared, app);
    // Opened on a document the handler doesn't have, so the probe can be put
    // in (Linux) before the first real document is asked for.
    let surface = run.open_surface(&warmup, hooks);
    #[cfg(target_os = "linux")]
    if !install_frame_probe(&surface, run.probe_script()) {
        run.fail(1, "the frame probe could not be installed in the surface");
    }
    thread::sleep(Duration::from_secs(3));
    let tripwire_baseline = proxied_count(shared, wire);
    eprintln!("CHECK tripwire baseline {tripwire_baseline}");

    // The window's place on the screen, for real input.
    let window = app.get_window("main").unwrap();
    let scale = window.scale_factor().unwrap_or(1.0);
    let inner = window.inner_position().unwrap();
    // macOS: the content view's top is under the title bar; the page, which
    // BOUNDS are in, starts below it.
    #[cfg(target_os = "macos")]
    let title_bar = hoplodex_lib::services::preview::surface::title_bar_height(surface.webview());
    #[cfg(not(target_os = "macos"))]
    let title_bar = 0.0;
    let input = (!options.input.is_empty()).then(|| Input {
        command: options.input.clone(),
        origin: if cfg!(target_os = "macos") {
            (f64::from(inner.x) / scale, f64::from(inner.y) / scale + title_bar)
        } else {
            (f64::from(inner.x), f64::from(inner.y))
        },
        unit: if cfg!(target_os = "macos") { 1.0 } else { scale },
    });
    eprintln!("CHECK window content at {inner:?} scale {scale}");

    #[cfg(target_os = "macos")]
    if options.hud_on {
        let ok = hud_on_check(&run, surface, &hostile, input.as_ref(), &pdf_dirs_before);
        if !ok {
            run.fail(4, "the HUD-on variant did not pass");
        }
        eprintln!("CHECK hud-on ok");
        exit_with(app, 0);
        return;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = &launched;

    stage("the hostile PDF");
    // 1. The hostile PDF.
    let shown = Instant::now();
    let Some(load) = run.show(&surface, &hostile, options.timeout) else {
        run.fail(2, "the hostile PDF didn't load");
    };
    eprintln!("CHECK hostile PDF loaded after {load:?}");
    #[cfg(not(target_os = "linux"))]
    {
        // No user script here: the probe runs in the top frame by `eval`
        // (a PDF document shown by WebKit or Edge's viewer may run none: said
        // below), and on Windows in Edge's frames through the DevTools protocol.
        thread::sleep(Duration::from_secs(2));
        stage("the top frame's probe");
        let _ = surface.webview().eval(run.probe_script());
        #[cfg(windows)]
        {
            // Alone first: whatever of the top frame's reaches the proxy as a
            // request (not a WebSocket's CONNECT) got past the content
            // filter (`judge_proxied`). The probe waits 3 s, then tries each
            // for up to 4 s, then gathers WebRTC candidates for 3 s.
            thread::sleep(Duration::from_secs(14));
            stage("the probes");
            let (reach, shared_) = (
                REACH_PROBE
                    .replace("__IPC__", "http://ipc.localhost/")
                    .replace("__TCP__", &run.tcp.to_string())
                    .replace("__UDP__", &run.udp.to_string()),
                shared.clone(),
            );
            let _ = surface
                .webview()
                .with_webview(move |platform| cdp::probe(&platform, reach, shared_));
        }
    }
    // The first page drawn: Linux's viewer says so; elsewhere the load event
    // stands for it (the large PDF is seen on the screen below, on Windows
    // and macOS).
    let first_paint = |since: Instant, loaded: Option<Duration>| -> (Duration, &'static str) {
        if cfg!(target_os = "linux") {
            let painted =
                wait_until(Duration::from_secs(30), || shared.painted_after(since).is_some());
            if let (true, Some(at)) = (painted, shared.painted_after(since)) {
                return (at.duration_since(since), "page 1 drawn");
            }
        }
        (loaded.unwrap_or_default(), "load event")
    };
    let (hostile_paint, how) = first_paint(shown, Some(load));
    eprintln!("CHECK first paint of the hostile PDF: {hostile_paint:?} ({how})");

    // The reach probe's reports: the top frame and, on Linux, PDF.js's frame.
    let want = if cfg!(target_os = "linux") { 2 } else { 1 };
    let got = wait_until(Duration::from_secs(40), || shared.report_count("reach") >= want);
    if cfg!(target_os = "linux") && !got {
        failures.push("the reach probe did not report from both frames of the viewer".into());
    } else if !got {
        eprintln!(
            "CHECK note: no reach report: the PDF document runs no page script here, \
             so the counters and the DNS log are the evidence"
        );
    }
    if cfg!(windows) {
        // The DevTools probe's report.
        wait_until(Duration::from_secs(20), || {
            shared.reports_of("reach").iter().any(|r| r["frame"] == "cdp")
        });
        if !shared.reports_of("reach").iter().any(|r| r["frame"] == "cdp") {
            failures.push("the reach probe did not run in Edge's viewer frame (DevTools)".into());
        }
    }
    let frames: HashSet<String> = shared
        .reports_of("reach")
        .iter()
        .filter_map(|r| r["frame"].as_str().map(String::from))
        .collect();
    eprintln!("CHECK reach probe reports from frames: {frames:?}");
    if cfg!(target_os = "linux") && !(frames.contains("top") && frames.contains("child")) {
        failures.push(format!("the reach probe reported from {frames:?}, not top and child"));
    }
    let mut judged = 0;
    let mut judge_new = |failures: &mut Vec<String>| {
        let reports = shared.reports_of("reach");
        for report in &reports[judged..] {
            failures.extend(judge_reach(report));
        }
        judged = reports.len();
    };
    judge_new(&mut failures);
    // The control: the main web view's own command call must have been
    // answered, or "the surface's call wasn't" proves nothing.
    wait_until(Duration::from_secs(10), || shared.control.lock().unwrap().is_some());
    match shared.control.lock().unwrap().as_deref() {
        Some("answered") => {}
        other => failures.push(format!(
            "the control failed: the main web view's own command call was not answered ({other:?}), \
             so \"no command answered the surface\" proves nothing"
        )),
    }

    stage("real input");
    // Real input.
    match &input {
        None => eprintln!("CHECK note: no --input command: the real input steps were skipped"),
        Some(input) => failures.extend(drive_input(&run, input)),
    }
    thread::sleep(Duration::from_secs(2));

    stage("the damaged PDFs");
    // 2. The truncated and bit-flipped PDFs: the viewer may fail on them.
    for (name, url) in [("truncated", &truncated), ("bit-flipped", &flipped)] {
        let ticks = shared.main_ticks.load(Ordering::SeqCst);
        let load = run.show(&surface, url, Duration::from_secs(20));
        eprintln!("CHECK {name} PDF: load finished = {load:?}");
        thread::sleep(Duration::from_secs(8));
        if shared.main_ticks.load(Ordering::SeqCst) <= ticks + 2 {
            failures.push(format!("the main window stopped running after the {name} PDF"));
        }
    }
    judge_new(&mut failures);

    stage("the 10 MB PDF");
    // 3. The 10 MB PDF.
    #[cfg(any(windows, target_os = "macos"))]
    let drawn = {
        // Watched from before the document is asked for.
        let window = app.get_window("main").unwrap();
        let scale = window.scale_factor().unwrap_or(1.0);
        let inner = window.inner_position().unwrap();
        #[cfg(windows)]
        let drawn = {
            let x = f64::from(inner.x) + (BOUNDS.x + 300.0) * scale;
            let y = f64::from(inner.y) + (BOUNDS.y + 300.0) * scale;
            pixels::watch(x as i32, y as i32)
        };
        // macOS reads the display in points.
        #[cfg(target_os = "macos")]
        let drawn = {
            let x = f64::from(inner.x) / scale + BOUNDS.x + 300.0;
            let title_bar =
                hoplodex_lib::services::preview::surface::title_bar_height(surface.webview());
            let y = f64::from(inner.y) / scale + title_bar + BOUNDS.y + 300.0;
            pixels::watch(x, y)
        };
        if drawn.is_none() {
            eprintln!(
                "CHECK the surface looks drawn before the 10 MB PDF is asked for: first paint falls back to the load event"
            );
        }
        drawn
    };
    let shown = Instant::now();
    let load = run.show(&surface, &large, options.timeout);
    let served = shared
        .served
        .lock()
        .unwrap()
        .iter()
        .find(|(p, _)| p.ends_with("/large.pdf"))
        .map(|(_, at)| at.duration_since(shown));
    let (paint, how) = first_paint(shown, load);
    // Windows and macOS: the page seen on the screen, not the load event.
    #[cfg(any(windows, target_os = "macos"))]
    let (paint, how) = match drawn {
        Some(cell) if wait_until(Duration::from_secs(30), || cell.lock().unwrap().is_some()) => {
            let at = cell.lock().unwrap().unwrap();
            (at.duration_since(shown), "page 1 seen on the screen")
        }
        _ => (paint, how),
    };
    let verdict = if paint <= FIRST_PAINT_BUDGET { "within" } else { "OVER" };
    eprintln!(
        "CHECK first paint of the 10 MB PDF: {paint:?} ({how}); asked for -> served {served:?}, \
         -> load finished {load:?}; {verdict} SC-001's {FIRST_PAINT_BUDGET:?}"
    );
    thread::sleep(Duration::from_secs(2));
    if screenshot(&options.shot) {
        eprintln!("CHECK screenshot {}", options.shot.display());
    } else {
        eprintln!("CHECK screenshot FAILED ({})", options.shot.display());
    }
    log_browser_processes();
    // The window and the surface's own tripwire.
    if let Ok(b) = surface.bounds() {
        eprintln!("CHECK surface bounds {b:?}");
    }
    eprintln!("CHECK main page ticks={}", shared.main_ticks.load(Ordering::SeqCst));
    thread::sleep(Duration::from_secs(3));

    stage("settling");
    // What nothing may have seen.
    let tcp = shared.victim_tcp.load(Ordering::SeqCst);
    let udp = shared.victim_udp.load(Ordering::SeqCst);
    if tcp + udp > 0 {
        failures
            .push(format!("the local test service saw {tcp} connection(s) and {udp} packet(s)"));
    }
    let hits = proxied_count(shared, wire) - tripwire_baseline.min(proxied_count(shared, wire));
    eprintln!("CHECK tripwire hits={} (baseline {tripwire_baseline})", proxied_count(shared, wire));
    #[cfg(not(windows))]
    if hits > 0 {
        failures.push(format!("the tripwire saw {hits} connection(s) beyond its baseline"));
    }
    #[cfg(windows)]
    failures.extend(judge_proxied(&shared.proxied.lock().unwrap(), run.tcp));
    #[cfg(windows)]
    let _ = hits;
    if let Some(log) = &options.dns_log {
        let names = std::fs::read_to_string(log).unwrap_or_default();
        let mut names: Vec<&str> = names.lines().collect();
        names.sort_unstable();
        names.dedup();
        eprintln!("CHECK names looked up by any process: {names:?}");
        for name in names {
            if PROBE_NAME_SUFFIXES.iter().any(|s| name.ends_with(s)) {
                failures.push(format!("a process looked up {name}"));
            }
        }
    } else {
        eprintln!("CHECK note: no --dns-log: the outside was judged by the counters alone");
    }
    let answered = shared.answered.lock().unwrap().clone();
    if answered.iter().any(|label| label != "main") {
        failures.push(format!("a command answered web views {answered:?}"));
    }
    if launched.exists() {
        failures.push("the PDF's launch action ran".into());
    }
    if !shared.downloads.lock().unwrap().is_empty() {
        failures.push(format!("a download was asked for: {:?}", shared.downloads.lock().unwrap()));
    }
    if !shared.ended.lock().unwrap().is_empty() {
        failures.push(format!("the surface ended: {:?}", shared.ended.lock().unwrap()));
    }
    // `windows()`, not `webview_windows()`: that lists only a window with a
    // single web view, and the main window has the surface's besides.
    let windows: Vec<String> = app.windows().into_keys().filter(|l| l != "main").collect();
    if !windows.is_empty() {
        failures.push(format!("another window opened: {windows:?}"));
    }
    #[cfg(target_os = "macos")]
    {
        let new: Vec<_> = webkit_pdf_dirs().difference(&pdf_dirs_before).cloned().collect();
        if !new.is_empty() {
            failures.push(format!("WebKit left {new:?} in the temp folder"));
        }
    }
    // The disk: nothing written during the run holds the marker.
    let mut roots = options.scan.clone();
    if roots.is_empty() {
        roots = vec![options.scratch.clone(), std::env::temp_dir()];
        roots.extend(std::env::home_dir());
    }
    let (written, holding) = scan_for_marker(&roots, &options.skip, run.started, &marker);
    eprintln!("CHECK disk scan of {roots:?}: {written} file(s) written during the run");
    for path in holding {
        failures.push(format!("{} holds the document's marker", path.display()));
    }

    eprintln!("CHECK finished after {:?}", started.elapsed());
    if failures.is_empty() {
        eprintln!("CHECK check ok");
        exit_with(app, 0);
    } else {
        for failure in &failures {
            eprintln!("CHECK FAIL {failure}");
        }
        screenshot(&options.shot.with_extension("failed.png"));
        exit_with(app, 4);
    }
}

/// Exits with `code`. macOS's and Windows' `App::exit` end the process with 0
/// whatever the code, so a failure leaves it explicitly.
fn exit_with(app: &AppHandle<Wry>, code: i32) {
    if code != 0 && cfg!(any(target_os = "macos", windows)) {
        std::process::exit(code);
    }
    app.exit(code);
}

/// With the hostile PDF shown: clicks every toolbar button and every link,
/// form button and menu item it finds, scrolls, and presses the shortcuts.
/// Returns the failures.
fn drive_input(run: &Run, input: &Input) -> Vec<String> {
    let shared = &run.shared;
    let mut failures = Vec::new();
    let (cx, cy) = (BOUNDS.x + BOUNDS.width / 2.0, BOUNDS.y + BOUNDS.height / 2.0);
    let layout = || shared.layout.lock().unwrap().clone();
    let in_surface = |x: f64, y: f64| (BOUNDS.x + x, BOUNDS.y + y);
    // A point of the viewer's page that is neither a link nor a control: right
    // of the annotations, which sit in the page's left part, and half way down.
    let neutral = layout()
        .and_then(|l| {
            let (x, w) = (l["page"]["x"].as_f64()?, l["page"]["w"].as_f64()?);
            Some(in_surface(x + w * 0.3, l["view"]["h"].as_f64()? / 2.0))
        })
        .unwrap_or((cx + 150.0, cy));

    // Focus the viewer, then scroll: activity must reach `note_activity`.
    input.click(neutral.0, neutral.1);
    thread::sleep(Duration::from_millis(1200));
    let before = shared.activity.load(Ordering::SeqCst);
    input.scroll(cx, cy, 5);
    input.scroll(cx, cy, -5);
    thread::sleep(Duration::from_millis(500));
    if shared.activity.load(Ordering::SeqCst) <= before {
        failures.push("scrolling did not call note_activity".into());
    }

    // The shortcuts the viewer must not act on.
    input.click(neutral.0, neutral.1);
    for (name, key) in [("Save", Key::Save), ("Print", Key::Print), ("Open", Key::Open)] {
        input.key(key);
        thread::sleep(Duration::from_millis(800));
        eprintln!("CHECK pressed the {name} shortcut");
    }
    // Escape and F6 must reach the main web view.
    for (name, key) in [("Escape", Key::Escape), ("F6", Key::F6)] {
        let (hook, main) = if matches!(key, Key::Escape) {
            (&shared.escape, &shared.main_saw_escape)
        } else {
            (&shared.focus_chrome, &shared.main_saw_focus_chrome)
        };
        let (hook_before, main_before) = (hook.load(Ordering::SeqCst), main.load(Ordering::SeqCst));
        input.click(neutral.0, neutral.1);
        input.key(key);
        if !wait_until(Duration::from_secs(3), || hook.load(Ordering::SeqCst) > hook_before) {
            failures.push(format!("{name} did not reach the surface's hook"));
        } else if !wait_until(Duration::from_secs(3), || main.load(Ordering::SeqCst) > main_before)
        {
            failures.push(format!("{name} did not reach the main web view"));
        }
    }

    // The toolbar's buttons (and what they open), the links and the form.
    match layout() {
        None => {
            // No layout from the frame (macOS, Windows): sweep the toolbar's
            // band and the page with clicks, which finds the same things.
            eprintln!("CHECK note: no layout reported: sweeping the viewer with clicks");
            for step in 0..20 {
                input.click(BOUNDS.x + 14.0 + f64::from(step) * 46.0, BOUNDS.y + 24.0);
            }
            // The page's left part, where the link, the launch action and the
            // form's button are, whatever the zoom: a column of clicks every
            // few pixels (the link is a band 20 points tall).
            for row in 0..17 {
                for col in 0..3 {
                    input.click(
                        BOUNDS.x + 100.0 + f64::from(col) * 100.0,
                        BOUNDS.y + 60.0 + f64::from(row) * 24.0,
                    );
                }
            }
            for step in 0..6 {
                input.right_click(cx + f64::from(step) * 40.0, cy);
                input.click(cx + f64::from(step) * 40.0 + 12.0, cy + 12.0);
            }
        }
        Some(_) => {
            let mut clicked: HashSet<String> = HashSet::new();
            let mut seen: HashSet<String> = HashSet::new();
            let mut toggles = 0;
            for _ in 0..80 {
                let Some(now) = layout() else { break };
                let buttons: Vec<Value> = now["buttons"].as_array().cloned().unwrap_or_default();
                // Presentation mode is left for the end: it may take the whole
                // window full screen.
                let buttons: Vec<Value> = buttons
                    .into_iter()
                    .filter(|b| {
                        !b["key"].as_str().unwrap_or("").to_lowercase().contains("presentation")
                    })
                    .collect();
                for b in &buttons {
                    seen.insert(b["key"].as_str().unwrap_or("?").to_string());
                }
                let next =
                    buttons.iter().find(|b| !clicked.contains(b["key"].as_str().unwrap_or("?")));
                let target = match next {
                    Some(b) => Some(b.clone()),
                    // The tools menu closes after an item: open it again for
                    // the items not yet clicked.
                    None if toggles < 12 && seen.difference(&clicked).next().is_some() => {
                        toggles += 1;
                        buttons
                            .iter()
                            .find(|b| {
                                b["key"]
                                    .as_str()
                                    .is_some_and(|k| k.starts_with("secondaryToolbarToggle"))
                            })
                            .cloned()
                    }
                    None => None,
                };
                let Some(b) = target else { break };
                let key = b["key"].as_str().unwrap_or("?").to_string();
                let (x, y) =
                    in_surface(b["x"].as_f64().unwrap_or(0.0), b["y"].as_f64().unwrap_or(0.0));
                eprintln!("CHECK clicking toolbar button {key}");
                input.click(x, y);
                clicked.insert(key);
                thread::sleep(Duration::from_millis(500));
            }
            // The controls the frame script hides must never have been shown.
            for key in &seen {
                let key = key.to_lowercase();
                if ["download", "print", "openfile", "bookmark", "editor"]
                    .iter()
                    .any(|forbidden| key.contains(forbidden))
                {
                    failures.push(format!("the viewer showed a {key} control"));
                }
            }
            failures.extend(click_presentation_mode(run, input));
            let unclicked: Vec<_> = seen.difference(&clicked).collect();
            eprintln!(
                "CHECK clicked {} toolbar buttons; not reachable: {unclicked:?}",
                clicked.len()
            );
            // Links, the launch action and the form's button.
            if let Some(links) = layout().and_then(|l| l["links"].as_array().cloned()) {
                if links.is_empty() {
                    failures.push("the viewer showed no link or form button to click".into());
                }
                for link in links {
                    let (x, y) = in_surface(
                        link["x"].as_f64().unwrap_or(0.0),
                        link["y"].as_f64().unwrap_or(0.0),
                    );
                    eprintln!("CHECK clicking {} {}", link["key"], link["href"]);
                    input.click(x, y);
                    thread::sleep(Duration::from_millis(1200));
                }
            }
            // The context menu: whatever it offers, an item at each of the
            // first rows below the pointer.
            input.right_click(neutral.0, neutral.1);
            thread::sleep(Duration::from_millis(600));
            for row in 0..6 {
                input.click(neutral.0 + 12.0, neutral.1 + 8.0 + f64::from(row) * 24.0);
                thread::sleep(Duration::from_millis(300));
                input.right_click(neutral.0, neutral.1);
                thread::sleep(Duration::from_millis(300));
            }
        }
    }
    failures
}

/// Opens the viewer's tools menu and clicks Presentation Mode, which asks the
/// window to go full screen: it must not, and if it did the window is put back.
fn click_presentation_mode(run: &Run, input: &Input) -> Vec<String> {
    let layout = || run.shared.layout.lock().unwrap().clone();
    let button = |name: &str| {
        layout().and_then(|l| {
            l["buttons"]
                .as_array()?
                .iter()
                .find(|b| b["key"].as_str().is_some_and(|k| k.to_lowercase().contains(name)))
                .cloned()
        })
    };
    let at = |b: &Value| {
        (BOUNDS.x + b["x"].as_f64().unwrap_or(0.0), BOUNDS.y + b["y"].as_f64().unwrap_or(0.0))
    };
    if let Some(toggle) =
        button("secondarytoolbartoggle").filter(|_| button("presentation").is_none())
    {
        let (x, y) = at(&toggle);
        input.click(x, y);
        thread::sleep(Duration::from_millis(600));
    }
    let Some(presentation) = button("presentation") else {
        eprintln!("CHECK note: no Presentation Mode button was reachable");
        return Vec::new();
    };
    let (x, y) = at(&presentation);
    eprintln!("CHECK clicking Presentation Mode");
    input.click(x, y);
    thread::sleep(Duration::from_millis(1500));
    let window = run.app.get_window("main").unwrap();
    if window.is_fullscreen().unwrap_or(false) {
        screenshot(&run.options.shot.with_extension("fullscreen.png"));
        let _ = window.set_fullscreen(false);
        thread::sleep(Duration::from_secs(1));
        return vec!["the viewer's Presentation Mode took the window full screen".into()];
    }
    Vec::new()
}

/// macOS, `--hud-on`: with the HUD left on, clicks its Open in Preview and
/// waits for the watch to delete the copy, then closes the surface as the app
/// does on `CopyCaught` and checks the hold is set.
#[cfg(target_os = "macos")]
fn hud_on_check(
    run: &Run,
    surface: Surface<Wry>,
    hostile: &Url,
    input: Option<&Input>,
    before: &HashSet<PathBuf>,
) -> bool {
    let Some(input) = input else {
        eprintln!("CHECK FAIL the HUD-on variant needs --input");
        return false;
    };
    if run.show(&surface, hostile, run.options.timeout).is_none() {
        eprintln!("CHECK FAIL the PDF did not load");
        return false;
    }
    thread::sleep(Duration::from_secs(4));
    // Focus the window, on its margin (the spike's way).
    input.click(BOUNDS.x + BOUNDS.width + 20.0, BOUNDS.y + 300.0);
    thread::sleep(Duration::from_millis(500));
    // The HUD appears over the viewer's bottom centre when the pointer is
    // there; Open in Preview is its second button (the spike's offsets, from
    // the viewer's centre and bottom).
    let (cx, bottom) = (BOUNDS.x + BOUNDS.width / 2.0, BOUNDS.y + BOUNDS.height);
    input.mouse_move(cx - 10.0, bottom - 200.0);
    thread::sleep(Duration::from_millis(300));
    input.mouse_move(cx, bottom - 80.0);
    thread::sleep(Duration::from_millis(800));
    input.click(cx + 30.0, bottom - 72.0);
    let ended =
        wait_until(Duration::from_secs(15), || !run.shared.ended.lock().unwrap().is_empty());
    let reasons = run.shared.ended.lock().unwrap().clone();
    eprintln!("CHECK the surface ended: {reasons:?}");
    let copy_caught = ended && reasons == [PdfEndReason::CopyCaught];
    // What the app does on `CopyCaught`: closes the surface, whose last sweep
    // and monitor removal `Surface::close` triggers.
    if let Err(e) = surface.close() {
        eprintln!("CHECK FAIL the surface could not be closed: {e}");
        return false;
    }
    thread::sleep(Duration::from_secs(2));
    let leftover: Vec<_> = webkit_pdf_dirs().difference(before).cloned().collect();
    eprintln!("CHECK new WebKitPDFs folders left: {leftover:?}");
    let closed = wait_until(Duration::from_secs(5), || run.app.get_webview("preview").is_none());
    eprintln!("CHECK surface closed: {closed}");
    let config = run.options.scratch.join("config");
    let hold = run
        .app
        .try_state::<MachineSettings>()
        .map(|settings| settings.pdf_preview_hold())
        .or_else(|| MachineSettings::load(&config).ok().map(|s| s.pdf_preview_hold()));
    eprintln!("CHECK hold: {hold:?}");
    // A restart reads the hold from `machine.json` again (FR-003a): the same
    // version finds PDF preview held off, another version clears the hold and
    // finds it available (the OS check is taken as passed: it needs the main
    // thread).
    let restart = |version: &str| {
        MachineSettings::load(&config)
            .ok()
            .map(|machine| PdfAvailability::at_startup(&machine, version, true))
    };
    let same = restart(env!("CARGO_PKG_VERSION"));
    eprintln!("CHECK a restart on the same version: {same:?}");
    let other = restart("0.0.0-another-version");
    eprintln!("CHECK a restart on another version: {other:?}");
    let held = same.is_some_and(|a| !a.is_available());
    let cleared = other.is_some_and(|a| a.is_available());
    // Preview, if the copy got that far, was not given a document: the watch
    // deletes the copy first (spike: Preview opens to nothing or not at all).
    let preview_running =
        Command::new("pgrep").args(["-x", "Preview"]).status().is_ok_and(|s| s.success());
    eprintln!("CHECK Preview is running: {preview_running}");
    copy_caught
        && leftover.is_empty()
        && closed
        && hold.is_some_and(|h| h.is_some())
        && held
        && cleared
}

/// `shows` (T005): the 3-page PDF in the surface, a screenshot, and out.
fn run_shows(run: Run) {
    let Run { app, shared, options, .. } = &run;
    let wire = tripwire::shared().expect("the tripwire binds");
    let url = add_document(shared, &run.token, "document.pdf", THREE_PAGES.to_vec());
    thread::sleep(Duration::from_secs(2));
    let started = Instant::now();
    let surface = run.open_surface(&url, counting_hooks(shared, app));
    let wait = wait_until(options.timeout, || {
        !shared.served.lock().unwrap().is_empty() && shared.finished_at(&url).is_some()
    });
    if !wait {
        run.fail(
            2,
            &format!(
                "the PDF didn't load (served={}, loaded={})",
                !shared.served.lock().unwrap().is_empty(),
                shared.finished_at(&url).is_some()
            ),
        );
    }
    eprintln!("CHECK served and loaded after {:?}", started.elapsed());
    // Anything this surface fetches must go to the proxy. The script runs in
    // the page that holds the viewer (Linux, Windows's top frame); a PDF
    // document itself, as on macOS, runs none.
    if options.probe {
        surface
            .webview()
            .eval("new Image().src = 'http://hoplodex-proxy-probe.invalid/probe'")
            .ok();
    }
    thread::sleep(Duration::from_secs(4));
    match surface.bounds() {
        Ok(b) => eprintln!("CHECK surface bounds {b:?}"),
        Err(e) => eprintln!("CHECK surface bounds unavailable: {e}"),
    }
    eprintln!("CHECK tripwire hits={}", proxied_count(shared, wire));
    // The main page pings once a second: it must still be running (Linux
    // moves its web view to attach the surface).
    eprintln!("CHECK main page ticks={}", shared.main_ticks.load(Ordering::SeqCst));
    log_browser_processes();
    if screenshot(&options.shot) {
        eprintln!("CHECK screenshot {}", options.shot.display());
    } else {
        eprintln!("CHECK screenshot FAILED ({})", options.shot.display());
    }
    eprintln!("CHECK shows ok");
    exit_with(app, 0);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_default();
    if mode != "shows" && mode != "check" {
        usage();
    }
    let mut scratch = None::<PathBuf>;
    let mut shot = None::<PathBuf>;
    let mut options = Options {
        scratch: PathBuf::new(),
        shot: PathBuf::new(),
        timeout: Duration::from_secs(40),
        input: Vec::new(),
        dns_log: None,
        scan: Vec::new(),
        skip: Vec::new(),
        hud_on: false,
        probe: true,
    };
    while let Some(flag) = args.next() {
        match flag.as_str() {
            // A control run for `shows`: without the probe's request, any
            // tripwire hit is the surface's own traffic.
            "--no-probe" => options.probe = false,
            "--hud-on" => options.hud_on = true,
            _ => {
                let value = args.next().unwrap_or_else(|| usage());
                match flag.as_str() {
                    "--scratch" => scratch = Some(value.into()),
                    "--screenshot" => shot = Some(value.into()),
                    "--timeout" => {
                        options.timeout =
                            Duration::from_secs(value.parse().unwrap_or_else(|_| usage()));
                    }
                    "--input" => options.input.push(value),
                    "--dns-log" => options.dns_log = Some(value.into()),
                    "--scan" => options.scan.push(value.into()),
                    "--skip" => options.skip.push(value.into()),
                    _ => usage(),
                }
            }
        }
    }
    if options.hud_on && !cfg!(target_os = "macos") {
        eprintln!("--hud-on is for macOS only");
        usage();
    }
    let temp = scratch.is_none().then(|| tempfile::tempdir().unwrap());
    options.scratch = scratch.unwrap_or_else(|| temp.as_ref().unwrap().path().to_path_buf());
    let cache = options.scratch.join("cache");
    // The names `app_dirs` gives the two web views' folders.
    let (main_dir, preview_dir) = (cache.join("main-webview"), cache.join("preview-webview2"));
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::create_dir_all(options.scratch.join("config")).unwrap();
    options.shot = shot.unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../e2e/screenshots-out/pdf-surface")
            .join(format!("{mode}-{}.png", std::env::consts::OS))
    });
    let _ = &preview_dir;

    let shared = Arc::new(Shared::default());
    let (tcp, udp) = start_victim(&shared);
    let token = random_hex(16);
    let started = SystemTime::now();
    // Windows: a proxy that reads what reaches it (`start_reading_proxy`), in
    // place of the app's tripwire, which only counts.
    #[cfg(windows)]
    let proxy = start_reading_proxy(&shared);
    #[cfg(not(windows))]
    let proxy = tripwire::shared().expect("the tripwire binds").addr();
    eprintln!("CHECK mode={mode} proxy={proxy} test service tcp={tcp} udp={udp}");

    let mut ctx = tauri::generate_context!();
    ctx.config_mut().app.windows.clear();

    let (preview_shared, main_shared) = (shared.clone(), shared.clone());
    let setup_shared = shared.clone();
    let mut options = Some(options);
    tauri::Builder::default()
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![get_chooser_state])
        .register_uri_scheme_protocol(SCHEME, move |_ctx, req| serve_preview(&preview_shared, &req))
        .register_uri_scheme_protocol(MAIN_SCHEME, move |_ctx, req| serve_main(&main_shared, &req))
        .setup(move |app| {
            let main = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(protocol_base(MAIN_SCHEME).parse().unwrap()),
            )
            .title("HoploDex PDF surface check")
            .inner_size(1000.0, 800.0)
            .incognito(true);
            // As the app's own: a folder of its own, away from the surface's.
            #[cfg(windows)]
            let main = main.data_directory(main_dir.clone());
            #[cfg(not(windows))]
            let _ = &main_dir;
            main.build()?;
            #[cfg(target_os = "macos")]
            if let Some(options) = &options
                && options.hud_on
                && let Ok(settings) = MachineSettings::load(&options.scratch.join("config"))
            {
                // Where the watch (T065) records the hold, as the app's own
                // state has it.
                app.manage(settings);
            }
            let run = Run {
                app: app.handle().clone(),
                shared: setup_shared.clone(),
                options: options.take().expect("setup runs once"),
                token: token.clone(),
                tcp,
                udp,
                proxy,
                started,
            };
            thread::spawn(move || {
                if mode == "check" {
                    run_check(run);
                } else {
                    run_shows(run);
                }
            });
            Ok(())
        })
        .build(ctx)
        .expect("the check's app failed to build")
        .run(|_, _| {});
    drop(temp);
}
