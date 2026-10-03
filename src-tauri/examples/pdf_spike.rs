// Spike for 007: can the web view's own PDF viewer show a document served
// from memory, in a window with no permissions, without writing it to disk,
// reaching HoploDex's commands or the network? Results and how to run it on
// each OS: specs/007-document-preview/spike-webview-pdf.md. Linux runs use
// examples/pdf_spike.sh in the dev container.
//
// It opens one window, "preview", which no capability names, on a PDF that
// a custom protocol serves from memory. The PDF holds a marker string (to find
// any copy on disk), a link, a JavaScript open action and padding to ~3.6 MB.
// A probe in the top frame and in every child frame (the viewer's) reports
// what each can reach, by navigating to `spikereport:`, which the navigation
// handler logs and refuses. Everything is logged to stderr with "SPIKE".
//
// Environment:
// - SPIKE_PROXY=1: send the web view's network through a closed port.
// - SPIKE_NOSCRIPT=1: turn off PDF.js scripting and XFA (Linux's viewer).
// - SPIKE_ALLOW_BLOB=1: let the viewer's Save navigate, so it downloads.
// - SPIKE_INCOGNITO=0: the default, persistent data store instead.
// - SPIKE_DOWNLOAD_DIR: where an allowed Save is written.
// - SPIKE_EXIT_SECONDS: when to quit (default 24).

use std::{path::PathBuf, thread, time::Duration};
use tauri::{
    Manager, WebviewUrl, WebviewWindowBuilder, http,
    webview::{DownloadEvent, NewWindowResponse},
};

const MARKER: &str = "HDSPIKEMARKER";
const SCHEME: &str = "hdpreview";

/// Stands in for any HoploDex command. The preview window must not reach it.
#[tauri::command]
fn spike_secret() -> String {
    eprintln!("SPIKE !!! spike_secret WAS CALLED");
    "the-secret".into()
}

/// The PDF's address: custom protocols are `<scheme>://localhost/` on Linux
/// and macOS, `http://<scheme>.localhost/` on Windows.
fn base_url() -> String {
    if cfg!(windows) {
        format!("http://{SCHEME}.localhost/")
    } else {
        format!("{SCHEME}://localhost/")
    }
}

fn build_pdf() -> Vec<u8> {
    let page1 = format!(
        "BT /F1 24 Tf 72 700 Td ({MARKER} page one) Tj ET \
         BT /F1 14 Tf 72 650 Td (A link to example.com is below) Tj ET \
         0 0 1 rg 72 600 200 20 re f"
    );
    let page2 = format!("BT /F1 24 Tf 72 700 Td (Second page {MARKER}) Tj ET");
    let pad = format!("{MARKER}-PAD ").repeat(200_000);
    let objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R /OpenAction 7 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R 8 0 R] /Count 2 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R >> >> /Annots [6 0 R] >>"
            .into(),
        format!("<< /Length {} >>\nstream\n{page1}\nendstream", page1.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        "<< /Type /Annot /Subtype /Link /Rect [72 600 272 620] /Border [0 0 0] \
         /A << /S /URI /URI (http://example.com/link-target) >> >>"
            .into(),
        "<< /S /JavaScript /JS (app.alert\\(\"HD SPIKE PDF JAVASCRIPT RAN\"\\);) >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 9 0 R \
         /Resources << /Font << /F1 5 0 R >> >> >>"
            .into(),
        format!("<< /Length {} >>\nstream\n{page2}\nendstream", page2.len()),
        format!("<< /Length {} >>\nstream\n{pad}\nendstream", pad.len()),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![];
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// Run in the top frame by `eval`, 7 s in: what the document the window
/// loaded can reach.
const TOP_PROBE: &str = r#"
(async () => {
  const r = { step: 'top' };
  r.href = location.href; r.origin = location.origin; r.frames = window.frames.length;
  r.internals = typeof window.__TAURI_INTERNALS__;
  const inv = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke;
  if (inv) {
    try { r.appCmd = 'OK ' + await inv('spike_secret'); } catch (e) { r.appCmd = 'ERR ' + String(e).slice(0, 120); }
    try { r.pluginCmd = 'OK ' + await inv('plugin:window|title', { label: 'preview' }); } catch (e) { r.pluginCmd = 'ERR ' + String(e).slice(0, 120); }
  }
  try { const x = await fetch('BASE' + 'doc.pdf'); r.refetch = x.status; } catch (e) { r.refetch = 'ERR ' + String(e); }
  try { const x = await fetch('http://example.com/top-fetch', { mode: 'no-cors' }); r.net = 'reached (' + x.type + ')'; } catch (e) { r.net = 'ERR ' + String(e); }
  location.href = 'spikereport://r/' + encodeURIComponent(JSON.stringify(r));
})();
"#;

/// Injected into every frame at document start. The top frame relays child
/// frames' reports; a child frame (the viewer) probes what code running in
/// it, such as a PDF that exploited the viewer, could reach.
const FRAME_SCRIPT: &str = r#"
(() => {
const report = r => { try { window.top.postMessage({ spike: r }, '*'); } catch (e) {} };
if (window === window.top) {
  window.addEventListener('message', e => { if (e.data && e.data.spike) location.href = 'spikereport://r/' + encodeURIComponent(JSON.stringify(Object.assign({ from: e.origin }, e.data.spike))); });
  return;
}
report({ step: 'frame-start', href: location.href });
if (NOSCRIPT) document.addEventListener('webviewerloaded', () => {
  try { PDFViewerApplicationOptions.set('enableScripting', false); PDFViewerApplicationOptions.set('enableXfa', false); report({ step: 'noscript-hook', scripting: String(PDFViewerApplicationOptions.get('enableScripting')) }); }
  catch (e) { report({ step: 'noscript-hook', error: String(e) }); }
});
window.alert = m => report({ step: 'ALERT-CALLED', message: String(m) });
setTimeout(async () => {
  const r = { step: 'frame' };
  try {
    r.href = location.href; r.origin = location.origin;
    r.internals = typeof window.__TAURI_INTERNALS__;
    r.webkitIpc = typeof (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc);
    r.chromeIpc = typeof (window.chrome && window.chrome.webview);
    try { r.parentHref = String(parent.location.href); } catch (e) { r.parentHref = 'ERR ' + String(e).slice(0, 80); }
    try { r.parentInternals = typeof parent.__TAURI_INTERNALS__; } catch (e) { r.parentInternals = 'ERR ' + String(e).slice(0, 80); }
    const msg = JSON.stringify({ cmd: 'spike_secret', callback: 1, error: 2, payload: {}, options: {} });
    try { if (r.webkitIpc === 'object') window.webkit.messageHandlers.ipc.postMessage(msg); if (r.chromeIpc === 'object') window.chrome.webview.postMessage(msg); r.postMessage = 'sent'; } catch (e) { r.postMessage = 'ERR ' + String(e); }
    try { const x = await fetch('ipc://localhost/spike_secret', { method: 'POST', body: '{}', headers: { 'Content-Type': 'application/json', 'Tauri-Callback': '1', 'Tauri-Error': '2', 'Tauri-Invoke-Key': 'guess' } }); r.rawIpc = x.status + ' ' + (await x.text()).slice(0, 60); } catch (e) { r.rawIpc = 'ERR ' + String(e).slice(0, 80); }
    try { const x = await fetch('http://example.com/frame-fetch', { mode: 'no-cors' }); r.net = 'reached (' + x.type + ')'; } catch (e) { r.net = 'ERR ' + String(e).slice(0, 80); }
    try { r.scripting = String(PDFViewerApplicationOptions.get('enableScripting')); } catch (e) { r.scripting = 'n/a'; }
    r.pages = document.querySelectorAll('.page').length;
    r.textLayer = Array.from(document.querySelectorAll('.textLayer')).map(t => t.textContent).join(' | ').slice(0, 120);
  } catch (e) { r.fatal = String(e); }
  report(r);
}, 5000); })();
"#;

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "1")
}

fn main() {
    let proxy = env_flag("SPIKE_PROXY");
    let noscript = env_flag("SPIKE_NOSCRIPT");
    let allow_blob = env_flag("SPIKE_ALLOW_BLOB");
    let incognito = std::env::var("SPIKE_INCOGNITO").map_or(true, |v| v != "0");
    let download_dir = std::env::var("SPIKE_DOWNLOAD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("spike-dl"));
    let exit_after: u64 = std::env::var("SPIKE_EXIT_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);
    let base = base_url();
    eprintln!(
        "SPIKE base={base} proxy={proxy} noscript={noscript} allow_blob={allow_blob} incognito={incognito}"
    );

    let pdf = build_pdf();
    eprintln!("SPIKE pdf bytes={}", pdf.len());

    let mut ctx = tauri::generate_context!();
    ctx.config_mut().app.windows.clear();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![spike_secret])
        .register_uri_scheme_protocol(SCHEME, move |_ctx, req| {
            eprintln!("SPIKE protocol request {} {}", req.method(), req.uri());
            http::Response::builder()
                .header("Content-Type", "application/pdf")
                .header("Cache-Control", "no-store")
                .body(pdf.clone())
                .unwrap()
        })
        .setup(move |app| {
            let nav_base = base.clone();
            let mut builder = WebviewWindowBuilder::new(
                app,
                "preview",
                WebviewUrl::External(format!("{base}doc.pdf").parse().unwrap()),
            )
            .title("PDF spike")
            .inner_size(1000.0, 800.0)
            .incognito(incognito)
            .initialization_script_for_all_frames(
                FRAME_SCRIPT.replace("NOSCRIPT", if noscript { "true" } else { "false" }),
            )
            .on_navigation(move |u| {
                if u.scheme() == "spikereport" {
                    let raw = u.path().trim_start_matches('/');
                    eprintln!("SPIKE REPORT {}", percent_decode(raw));
                    return false;
                }
                let ok = u.as_str().starts_with(&nav_base)
                    || u.scheme() == "webkit-pdfjs-viewer"
                    || u.as_str() == "about:blank"
                    || (allow_blob && u.scheme() == "blob");
                eprintln!("SPIKE navigation {u} -> {}", if ok { "allow" } else { "DENY" });
                ok
            })
            .on_new_window(|u, _| {
                eprintln!("SPIKE new-window request {u} -> DENY");
                NewWindowResponse::Deny
            })
            .on_download(move |_, ev| {
                match ev {
                    DownloadEvent::Requested { url, destination } => {
                        eprintln!(
                            "SPIKE download requested {url} default-dest={}",
                            destination.display()
                        );
                        *destination = download_dir.join("saved.pdf");
                        eprintln!("SPIKE download -> {}", destination.display());
                    }
                    DownloadEvent::Finished { url, path, success } => {
                        eprintln!("SPIKE download finished {url} path={path:?} success={success}");
                    }
                    _ => {}
                }
                true
            })
            .on_page_load(|_, p| eprintln!("SPIKE page-load {:?} {}", p.event(), p.url()));
            if proxy {
                eprintln!("SPIKE proxy -> http://127.0.0.1:9");
                builder = builder.proxy_url("http://127.0.0.1:9".parse().unwrap());
            }
            builder.build()?;

            let handle = app.handle().clone();
            let probe = TOP_PROBE.replace("BASE", &base);
            thread::spawn(move || {
                let window = handle.get_webview_window("preview").unwrap();
                thread::sleep(Duration::from_secs(7));
                window.eval(probe).ok();
                thread::sleep(Duration::from_secs(exit_after.saturating_sub(7)));
                eprintln!("SPIKE exiting");
                handle.exit(0);
            });
            Ok(())
        })
        .run(ctx)
        .expect("spike failed");
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
