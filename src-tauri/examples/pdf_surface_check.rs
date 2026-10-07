// Proof, on each OS, that the PDF surface (research.md §4,
// src/services/preview/surface/) works as a child web view inside the main
// window: a window as the E2E suite isolates one (scratch folders, an
// incognito main web view, its own user data folder on Windows), a minimal
// `hdpreview` handler serving a fixture PDF from memory, and the surface
// placed over a rectangle of the window. Tasks T005 to T008 of
// specs/007-document-preview/tasks.md; the result goes in research.md §4.
//
//   cargo run --example pdf_surface_check -- shows [--scratch DIR]
//       [--screenshot PNG] [--timeout SECONDS] [--no-probe]
//
// Modes (only one for now; later tasks add more):
//   shows   Opens the window (1000x800 logical, at the OS's default place),
//           adds the surface over the rectangle drawn in the page (x=40,
//           y=100, 920x660 logical), loads the PDF, and once it has loaded
//           waits 4 s for the viewer to draw, saves a screenshot of the
//           whole screen, and exits.
//
// Exit codes:
//   0  the PDF was served and the page finished loading (look at the
//      screenshot: the 3-page PDF must be drawn by the OS's viewer inside the
//      red rectangle, with the window's own page around it)
//   1  the window or the surface couldn't be built
//   2  the PDF didn't load within --timeout (default 40 s)
//   3  usage
// A screenshot that couldn't be taken is logged ("CHECK screenshot FAILED")
// and doesn't change the exit code. Everything the check learns is logged to
// stderr with a "CHECK" prefix: the surface's bounds as the OS reports them,
// the page-load events, the proxy's tripwire hits, and on Windows the browser
// processes with their user data folders and arguments.
//
// Nothing outside the scratch folder is touched: --scratch defaults to a new
// temporary folder, removed at the end. On Linux run it under Xvfb in the dev
// container: `scripts/dev-container.sh bash src-tauri/examples/pdf_surface_check.sh`.

use std::{
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use hoplodex_lib::services::preview::surface::{Rect, Surface, SurfaceConfig};
use tauri::{Manager, Url, WebviewUrl, WebviewWindowBuilder, http, webview::PageLoadEvent};

const SCHEME: &str = "hdpreview";
/// The main window's own page comes from a protocol too (`data:` URLs need a
/// Tauri feature the app doesn't use).
const MAIN_SCHEME: &str = "hdmain";
const PDF: &[u8] = include_bytes!("../tests/fixtures/documents/three-pages.pdf");
/// Where the surface goes, in logical pixels, and the page's placeholder for
/// it (a red outline), so a wrong place is visible in the screenshot.
const BOUNDS: Rect = Rect { x: 40.0, y: 100.0, width: 920.0, height: 660.0 };

/// The main window's page: a placeholder where the surface goes.
fn main_page() -> String {
    let Rect { x, y, width, height } = BOUNDS;
    format!(
        "<body style=\"margin:0;background:#dde\">\
         <p style=\"font:16px sans-serif;margin:16px 40px\">HoploDex main window. \
         The PDF surface must fill the red outline.</p>\
         <div style=\"position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;\
         box-sizing:border-box;border:2px solid red\"></div>\
         <script>setInterval(() => fetch('/tick?' + Date.now()), 1000)</script></body>",
        x - 2.0,
        y - 2.0,
        width + 4.0,
        height + 4.0
    )
}

fn token() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).unwrap();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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

fn document_url(token: &str) -> Url {
    format!("{}{token}/document.pdf", protocol_base(SCHEME)).parse().unwrap()
}

/// Stands in for the tripwire: accepts, reads nothing, closes, and counts.
fn tripwire(hits: Arc<AtomicUsize>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let n = hits.fetch_add(1, Ordering::SeqCst) + 1;
            eprintln!("CHECK tripwire hit {n}");
            drop(stream);
        }
    });
    port
}

fn usage() -> ! {
    eprintln!(
        "usage: pdf_surface_check shows [--scratch DIR] [--screenshot PNG] [--timeout SECONDS] \
         [--no-probe]"
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

fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("shows") {
        usage();
    }
    let (mut scratch, mut shot, mut timeout) =
        (None::<PathBuf>, None::<PathBuf>, Duration::from_secs(40));
    let mut probe = true;
    while let Some(flag) = args.next() {
        // A control run: without the probe's request, any tripwire hit is the
        // surface's own traffic.
        if flag == "--no-probe" {
            probe = false;
            continue;
        }
        let value = args.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--scratch" => scratch = Some(value.into()),
            "--screenshot" => shot = Some(value.into()),
            "--timeout" => timeout = Duration::from_secs(value.parse().unwrap_or_else(|_| usage())),
            _ => usage(),
        }
    }
    let temp = scratch.is_none().then(|| tempfile::tempdir().unwrap());
    let scratch = scratch.unwrap_or_else(|| temp.as_ref().unwrap().path().to_path_buf());
    let cache = scratch.join("cache");
    // The names `app_dirs` gives the two web views' folders.
    let (main_dir, preview_dir) = (cache.join("main-webview"), cache.join("preview-webview2"));
    std::fs::create_dir_all(&cache).unwrap();
    let shot = shot.unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../e2e/screenshots-out/pdf-surface")
            .join(format!("shows-{}.png", std::env::consts::OS))
    });

    let hits = Arc::new(AtomicUsize::new(0));
    let port = tripwire(hits.clone());
    let token = token();
    let url = document_url(&token);
    eprintln!("CHECK scratch={} url={url} proxy=127.0.0.1:{port}", scratch.display());

    let served = Arc::new(AtomicBool::new(false));
    let loaded = Arc::new(AtomicBool::new(false));

    let mut ctx = tauri::generate_context!();
    ctx.config_mut().app.windows.clear();

    let ticks = Arc::new(AtomicUsize::new(0));
    let ticks_proto = ticks.clone();
    let (served_proto, token_) = (served.clone(), token.clone());
    let url_ = url.clone();
    tauri::Builder::default()
        .register_uri_scheme_protocol(SCHEME, move |_ctx, req| {
            eprintln!("CHECK protocol request {} {}", req.method(), req.uri());
            if req.uri().path() != format!("/{token_}/document.pdf") {
                return http::Response::builder().status(404).body(Vec::new()).unwrap();
            }
            served_proto.store(true, Ordering::SeqCst);
            http::Response::builder()
                .header("Content-Type", "application/pdf")
                .header("Cache-Control", "no-store")
                .body(PDF.to_vec())
                .unwrap()
        })
        .register_uri_scheme_protocol(MAIN_SCHEME, move |_ctx, req| {
            if req.uri().path() == "/tick" {
                ticks_proto.fetch_add(1, Ordering::SeqCst);
                return http::Response::builder().body(Vec::new()).unwrap();
            }
            http::Response::builder()
                .header("Content-Type", "text/html")
                .body(main_page().into_bytes())
                .unwrap()
        })
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

            let handle = app.handle().clone();
            let (served, loaded, hits, ticks) =
                (served.clone(), loaded.clone(), hits.clone(), ticks.clone());
            let loaded_hook = loaded.clone();
            let url_hook = url_.clone();
            thread::spawn(move || {
                let started = Instant::now();
                let fail = |code: i32, what: &str| -> ! {
                    eprintln!("CHECK FAILED: {what}");
                    screenshot(&shot.with_extension("failed.png"));
                    std::process::exit(code)
                };
                thread::sleep(Duration::from_secs(2));
                let window = handle.get_window("main").unwrap();
                let config = SurfaceConfig {
                    url: url_.clone(),
                    proxy_url: format!("http://127.0.0.1:{port}").parse().unwrap(),
                    data_directory: preview_dir,
                    bounds: BOUNDS,
                    on_page_load: Some(Box::new(move |event, url| {
                        eprintln!("CHECK page-load {event:?} {url}");
                        if matches!(event, PageLoadEvent::Finished) && *url == url_hook {
                            loaded_hook.store(true, Ordering::SeqCst);
                        }
                    })),
                };
                let surface = match Surface::open(&window, config) {
                    Ok(surface) => surface,
                    Err(e) => fail(1, &format!("the surface could not be built: {e}")),
                };
                eprintln!("CHECK surface built");
                if let Err(e) = surface.set_bounds(BOUNDS, true) {
                    fail(1, &format!("set_bounds failed: {e}"));
                }
                while !(served.load(Ordering::SeqCst) && loaded.load(Ordering::SeqCst)) {
                    if started.elapsed() > timeout {
                        fail(
                            2,
                            &format!(
                                "the PDF didn't load (served={}, loaded={})",
                                served.load(Ordering::SeqCst),
                                loaded.load(Ordering::SeqCst)
                            ),
                        );
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                eprintln!("CHECK served and loaded after {:?}", started.elapsed());
                // Anything this surface fetches must go to the proxy. The script
                // runs in the page that holds the viewer (Linux, Windows's top
                // frame); a PDF document itself, as on macOS, runs none.
                if probe {
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
                eprintln!("CHECK tripwire hits={}", hits.load(Ordering::SeqCst));
                // The main page pings once a second: it must still be running
                // (Linux moves its web view to attach the surface).
                eprintln!("CHECK main page ticks={}", ticks.load(Ordering::SeqCst));
                log_browser_processes();
                if screenshot(&shot) {
                    eprintln!("CHECK screenshot {}", shot.display());
                } else {
                    eprintln!("CHECK screenshot FAILED ({})", shot.display());
                }
                eprintln!("CHECK shows ok");
                handle.exit(0);
            });
            Ok(())
        })
        .build(ctx)
        .expect("the check's app failed to build")
        .run(|_, _| {});
    drop(temp);
}
