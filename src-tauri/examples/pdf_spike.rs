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
// - SPIKE_PROXY=closed: send the web view's network to 127.0.0.1:9, a port
//   nothing is expected to listen on. SPIKE_PROXY=own: to a port the spike
//   binds and holds itself (the tripwire), which logs and drops every
//   connection.
// - SPIKE_HARDEN=1: turn off WebRTC (and DNS prefetching, which WebKitGTK no
//   longer honours) in the preview's settings (Linux).
// - SPIKE_BLOCKER=1: a WebKit content filter that blocks every URL but the
//   preview's own (Linux).
// - SPIKE_NOSCRIPT=1: turn off PDF.js scripting and XFA (Linux's viewer).
// - SPIKE_ALLOW_BLOB=1: let the viewer's Save navigate, so it downloads.
// - SPIKE_INCOGNITO=0: the default, persistent data store instead.
// - SPIKE_DOWNLOAD_DIR: where an allowed Save is written.
// - SPIKE_EXIT_SECONDS: when to quit (default 30).
//
// Whatever the settings, the spike also starts a fake local service (a TCP
// and a UDP port on 127.0.0.1) that logs anything reaching it, and the
// viewer-frame probe tries to reach it and the outside by fetch, image,
// beacon, WebSocket, WebRTC and DNS prefetch.

use std::{
    io::{BufRead, BufReader},
    net::{TcpListener, UdpSocket},
    path::PathBuf,
    thread,
    time::Duration,
};
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
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1)
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
    const limit = (p, ms) => Promise.race([p, new Promise(res => setTimeout(() => res('timeout'), ms))]);
    const f = async u => { try { const x = await limit(fetch(u, { mode: 'no-cors' }), 4000); return x === 'timeout' ? x : 'reached (' + x.type + ')'; } catch (e) { return 'ERR ' + String(e).slice(0, 50); } };
    const ws = u => new Promise(res => { try { const s = new WebSocket(u); s.onopen = () => { res('OPEN'); s.close(); }; s.onerror = () => res('error'); setTimeout(() => res('timeout'), 4000); } catch (e) { res('ERR ' + String(e).slice(0, 50)); } });
    const img = u => new Promise(res => { const i = new Image(); i.onload = () => res('loaded'); i.onerror = () => res('error'); setTimeout(() => res('timeout'), 4000); i.src = u; });
    const rtc = async () => {
      if (typeof RTCPeerConnection === 'undefined') return 'absent';
      try {
        const pc = new RTCPeerConnection({ iceServers: [{ urls: 'stun:127.0.0.1:VUDP' }, { urls: 'stun:dns-leak-stun.invalid:3478' }] });
        const cands = []; pc.onicecandidate = e => { if (e.candidate) cands.push(e.candidate.candidate.slice(0, 70)); };
        pc.createDataChannel('x'); await pc.setLocalDescription(await pc.createOffer());
        await new Promise(res => setTimeout(res, 3000)); pc.close();
        return 'present; candidates=' + JSON.stringify(cands);
      } catch (e) { return 'ERR ' + String(e).slice(0, 60); }
    };
    const l = document.createElement('link'); l.rel = 'dns-prefetch'; l.href = '//dns-leak-prefetch.invalid'; document.head.appendChild(l);
    const a = document.createElement('a'); a.href = 'http://dns-leak-anchor.invalid/'; a.textContent = '.'; document.body.appendChild(a);
    try { r.beacon = String(navigator.sendBeacon('http://127.0.0.1:VTCP/beacon', 'x')); } catch (e) { r.beacon = 'ERR ' + String(e).slice(0, 50); }
    [r.netExt, r.netDns, r.netLoop, r.netLocalhost, r.wsLoop, r.wsExt, r.imgLoop, r.imgExt, r.rtc] = await Promise.all([
      f('http://example.com/frame-fetch'), f('http://dns-leak-fetch.invalid/'), f('http://127.0.0.1:VTCP/fetch-loop'), f('http://localhost:VTCP/fetch-localhost'),
      ws('ws://127.0.0.1:VTCP/ws-loop'), ws('ws://dns-leak-ws.invalid/'), img('http://127.0.0.1:VTCP/img-loop'), img('http://dns-leak-img.invalid/x.png'), rtc()]);
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
    let proxy = std::env::var("SPIKE_PROXY").unwrap_or_default();
    let harden = env_flag("SPIKE_HARDEN");
    let blocker = env_flag("SPIKE_BLOCKER");
    let noscript = env_flag("SPIKE_NOSCRIPT");
    let allow_blob = env_flag("SPIKE_ALLOW_BLOB");
    let incognito = std::env::var("SPIKE_INCOGNITO").map_or(true, |v| v != "0");
    let download_dir = std::env::var("SPIKE_DOWNLOAD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("spike-dl"));
    let exit_after: u64 =
        std::env::var("SPIKE_EXIT_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
    let base = base_url();
    eprintln!(
        "SPIKE base={base} proxy={proxy:?} harden={harden} blocker={blocker} noscript={noscript} allow_blob={allow_blob} incognito={incognito}"
    );

    let victim_tcp = watch_tcp("VICTIM", TcpListener::bind("127.0.0.1:0").unwrap());
    let victim_udp = watch_udp(UdpSocket::bind("127.0.0.1:0").unwrap());
    let proxy_url = match proxy.as_str() {
        "closed" | "1" => Some("http://127.0.0.1:9".to_string()),
        "own" => {
            let port = watch_tcp("TRIPWIRE", TcpListener::bind("127.0.0.1:0").unwrap());
            #[cfg(target_os = "linux")]
            try_to_take_port(port);
            Some(format!("http://127.0.0.1:{port}"))
        }
        _ => None,
    };
    eprintln!("SPIKE victim tcp={victim_tcp} udp={victim_udp} proxy_url={proxy_url:?}");

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
                WebviewUrl::External("about:blank".parse().unwrap()),
            )
            .title("PDF spike")
            .inner_size(1000.0, 800.0)
            .incognito(incognito)
            .initialization_script_for_all_frames(
                FRAME_SCRIPT
                    .replace("NOSCRIPT", if noscript { "true" } else { "false" })
                    .replace("VTCP", &victim_tcp.to_string())
                    .replace("VUDP", &victim_udp.to_string()),
            )
            .on_navigation(move |u| {
                if u.scheme() == "spikereport" {
                    let raw = u.path().trim_start_matches('/');
                    eprintln!("SPIKE REPORT {}", percent_decode(raw));
                    return false;
                }
                let ok = u.as_str().starts_with(&nav_base)
                    || u.scheme() == "about"
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
            if let Some(p) = &proxy_url {
                builder = builder.proxy_url(p.parse().unwrap());
            }
            let window = builder.build()?;
            let doc_url = format!("{base}doc.pdf");
            #[cfg(target_os = "linux")]
            {
                let store = std::env::temp_dir().join("spike-filter-store");
                window.with_webview(move |wv| {
                    linux::configure(&wv.inner(), harden, blocker, &store, doc_url);
                })?;
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (harden, blocker);
                window.navigate(doc_url.parse().unwrap())?;
            }

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

/// Logs every connection to `listener` with its first line, then drops it.
fn watch_tcp(name: &'static str, listener: TcpListener) -> u16 {
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            stream.set_read_timeout(Some(Duration::from_secs(1))).ok();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).ok();
            eprintln!("SPIKE {name} connection on {port}: {:?}", line.trim_end());
        }
    });
    port
}

fn watch_udp(socket: UdpSocket) -> u16 {
    let port = socket.local_addr().unwrap().port();
    thread::spawn(move || {
        let mut buf = [0u8; 1500];
        while let Ok((n, from)) = socket.recv_from(&mut buf) {
            eprintln!("SPIKE VICTIM udp packet on {port}: {n} bytes from {from}");
        }
    });
    port
}

/// Tries to take the tripwire's port away, as another program might.
#[cfg(target_os = "linux")]
fn try_to_take_port(port: u16) {
    for addr in [format!("127.0.0.1:{port}"), format!("0.0.0.0:{port}")] {
        let result = TcpListener::bind(&addr).map(|_| "BOUND").map_err(|e| e.to_string());
        eprintln!("SPIKE take-over std bind {addr}: {result:?}");
    }
    // SO_REUSEADDR and SO_REUSEPORT, as a program trying to share it would.
    let result = unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0);
        let one: libc::c_int = 1;
        for opt in [libc::SO_REUSEADDR, libc::SO_REUSEPORT] {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                opt,
                (&raw const one).cast(),
                size_of::<libc::c_int>() as libc::socklen_t,
            );
        }
        let addr = libc::sockaddr_in {
            sin_family: libc::AF_INET as libc::sa_family_t,
            sin_port: port.to_be(),
            sin_addr: libc::in_addr { s_addr: u32::from_be_bytes([127, 0, 0, 1]).to_be() },
            sin_zero: [0; 8],
        };
        let rc = libc::bind(
            fd,
            (&raw const addr).cast(),
            size_of::<libc::sockaddr_in>() as libc::socklen_t,
        );
        let result = if rc == 0 {
            format!("BOUND (listen rc {})", libc::listen(fd, 1))
        } else {
            format!("refused: {}", std::io::Error::last_os_error())
        };
        libc::close(fd);
        result
    };
    eprintln!("SPIKE take-over SO_REUSEADDR+SO_REUSEPORT bind 127.0.0.1:{port}: {result}");
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{ffi::CString, path::Path, ptr};
    use webkit2gtk::{
        SettingsExt, WebViewExt, ffi,
        gio::ffi as gio_ffi,
        glib::{self, ffi as glib_ffi, gobject_ffi, translate::ToGlibPtr},
    };

    /// Settings, then (optionally) the content filter, then the document.
    pub fn configure(
        wv: &webkit2gtk::WebView,
        harden: bool,
        blocker: bool,
        store: &Path,
        url: String,
    ) {
        if let Some(settings) = WebViewExt::settings(wv) {
            if harden {
                settings.set_enable_webrtc(false);
                #[allow(deprecated)]
                settings.set_enable_dns_prefetching(false);
            }
            eprintln!(
                "SPIKE settings webrtc={} media_stream={} media={}",
                settings.enables_webrtc(),
                settings.enables_media_stream(),
                settings.enables_media()
            );
        }
        let raw: *mut ffi::WebKitWebView = wv.to_glib_none().0;
        unsafe {
            let ctx = ffi::webkit_web_view_get_context(raw);
            eprintln!(
                "SPIKE web process sandbox enabled={}",
                ffi::webkit_web_context_get_sandbox_enabled(ctx)
            );
        }
        if !blocker {
            wv.load_uri(&url);
            return;
        }
        // Block every load but the preview's own (and the spike's reports).
        let rules = r#"[
          {"trigger":{"url-filter":".*"},"action":{"type":"block"}},
          {"trigger":{"url-filter":"^hdpreview:"},"action":{"type":"ignore-previous-rules"}},
          {"trigger":{"url-filter":"^webkit-pdfjs-viewer:"},"action":{"type":"ignore-previous-rules"}},
          {"trigger":{"url-filter":"^blob:"},"action":{"type":"ignore-previous-rules"}},
          {"trigger":{"url-filter":"^data:"},"action":{"type":"ignore-previous-rules"}},
          {"trigger":{"url-filter":"^about:"},"action":{"type":"ignore-previous-rules"}},
          {"trigger":{"url-filter":"^spikereport:"},"action":{"type":"ignore-previous-rules"}}
        ]"#;
        let bytes = glib::Bytes::from_owned(rules.as_bytes().to_vec());
        let path = CString::new(store.to_string_lossy().as_bytes()).unwrap();
        let id = CString::new("preview-block-all").unwrap();
        let data = Box::into_raw(Box::new((raw, CString::new(url).unwrap())));
        unsafe {
            gobject_ffi::g_object_ref(raw.cast());
            let store = ffi::webkit_user_content_filter_store_new(path.as_ptr());
            ffi::webkit_user_content_filter_store_save(
                store,
                id.as_ptr(),
                bytes.to_glib_none().0,
                ptr::null_mut(),
                Some(saved),
                data.cast(),
            );
        }
    }

    unsafe extern "C" fn saved(
        source: *mut gobject_ffi::GObject,
        result: *mut gio_ffi::GAsyncResult,
        data: glib_ffi::gpointer,
    ) {
        unsafe {
            let data = Box::from_raw(data.cast::<(*mut ffi::WebKitWebView, CString)>());
            let (raw, url) = *data;
            let mut error: *mut glib_ffi::GError = ptr::null_mut();
            let filter = ffi::webkit_user_content_filter_store_save_finish(
                source.cast(),
                result,
                &mut error,
            );
            if filter.is_null() {
                let message = std::ffi::CStr::from_ptr((*error).message).to_string_lossy();
                eprintln!("SPIKE content filter FAILED: {message}");
            } else {
                let manager = ffi::webkit_web_view_get_user_content_manager(raw);
                ffi::webkit_user_content_manager_add_filter(manager, filter);
                ffi::webkit_user_content_filter_unref(filter);
                eprintln!("SPIKE content filter installed");
            }
            ffi::webkit_web_view_load_uri(raw, url.as_ptr());
            gobject_ffi::g_object_unref(raw.cast());
            gobject_ffi::g_object_unref(source);
        }
    }
}
