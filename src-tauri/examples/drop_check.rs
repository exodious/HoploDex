// The drop check (code scanning alert `paths/cwe-73` on
// `add_document_from_path`): the app's own drop recorder (`drops::register`)
// on a main window opened the way the E2E suite isolates one, with a session
// open on a scratch database, driven with real input. It is built without the
// `e2e` feature, so the drops it sees are the OS's, delivered as a shipped
// build gets them.
//
//   cargo run --profile e2e --example drop_check -- \
//       --input CMD [--input ARG]... --source CMD [--source ARG]... \
//       [--scratch DIR] [--out DIR] [--timeout SECONDS]
//
// `--input` is the command that posts real input, given `drag X1 Y1 X2 Y2`
// (press at the first point, move to the second, release) and `click X Y`
// after it, with screen coordinates: examples/real_input.py (Linux),
// real_input.swift (macOS, compiled) or real_input.ps1 (Windows). `--source`
// is the command that shows a window another process owns holding a file to
// drag, given `FILE X Y` after it: the window's top-left corner at X Y, in the
// input command's units, 160 by 160 of them. On Linux that is this example
// itself, `drop_check --drag-source`; examples/drag_source.swift on macOS and
// drag_source.ps1 on Windows.
//
// The check:
//   1. drags a file from the other process's window onto the main window and
//      waits for the session to hold it: `Session::take_drop`, which
//      `add_photo_from_path` and `add_document_from_path` call first, must
//      give it back (the drop is attachable);
//   2. drags a link out of the page itself, whose drag carries a `file://`
//      URI list naming another file, and drops it on the main window: the
//      session must never hold that file (a drag that starts in the page is
//      not a drop of the user's files).
// It fails (exit 4, one "CHECK FAIL" line each) if the native drop isn't
// attachable, if the page's drag is, or if the page's drag never started (so
// the second part proved nothing).
//
// Exit codes:
//   0  passed
//   1  the window couldn't be built, its page didn't load or the database
//      couldn't be made
//   3  usage
//   4  a check failed
// Everything is logged to stderr with a "CHECK" prefix. Nothing outside the
// scratch folder is touched. On Linux run it with
// `scripts/dev-container.sh scripts/drop-check.sh`.

use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use hoplodex_lib::db;
use hoplodex_lib::drops;
use hoplodex_lib::services::machine_settings::MachineIdentity;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::session::{OpenDatabase, Session};
use tauri::{
    AppHandle, DragDropEvent, Manager, WebviewEvent, WebviewUrl, WebviewWindowBuilder,
    WindowEvent, Wry, http,
};

/// The main window's page comes from a protocol, as in the window controls
/// check.
const MAIN_SCHEME: &str = "hddrop";

/// The page: one link that, dragged, carries a `file://` URI list naming the
/// planted file (`PLANTED`, filled in when served), and reports the drag.
const PAGE: &str = r#"<!doctype html><meta charset="utf-8"><title>drop check</title>
<style>
  body { margin: 0; font: 16px sans-serif; background: #eef; height: 100vh; }
  #planted { position: absolute; left: 40px; top: 40px; width: 220px; height: 60px;
    line-height: 60px; text-align: center; background: #fcc; border: 1px solid #a66;
    color: #000; text-decoration: none; }
</style>
<a id="planted" href="PLANTED" draggable="true">a page link to drag</a>
<script>
  fetch('/loaded');
  planted.addEventListener('dragstart', (e) => {
    e.dataTransfer.setData('text/uri-list', 'PLANTED');
    e.dataTransfer.setData('text/plain', 'PLANTED');
    e.dataTransfer.effectAllowed = 'copy';
    fetch('/dragstart');
  });
</script>"#;

/// Where the page's link is (its middle) and where drops land, in logical
/// pixels of the window's content area.
const LINK: (f64, f64) = (150.0, 70.0);
const TARGET: (f64, f64) = (500.0, 400.0);
const PAGE_TARGET: (f64, f64) = (600.0, 500.0);
/// Where the source window goes, in the input command's units.
const SOURCE_AT: (i64, i64) = (40, 160);
/// The main window's top-left corner, logical pixels.
const MAIN_AT: (f64, f64) = (300.0, 60.0);

/// How long a drop may take to be recorded after the input that made it.
const DROP_LIMIT: Duration = Duration::from_secs(6);

#[derive(Default)]
struct Shared {
    loads: AtomicUsize,
    drag_starts: AtomicUsize,
    planted_uri: Mutex<String>,
}

fn serve(shared: &Shared, req: &http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let empty = || http::Response::builder().body(Vec::new()).unwrap();
    match req.uri().path() {
        "/loaded" => {
            shared.loads.fetch_add(1, Ordering::SeqCst);
            eprintln!("CHECK page loaded");
            empty()
        }
        "/dragstart" => {
            shared.drag_starts.fetch_add(1, Ordering::SeqCst);
            eprintln!("CHECK the page's drag started");
            empty()
        }
        _ => {
            let page = PAGE.replace("PLANTED", &shared.planted_uri.lock().unwrap());
            http::Response::builder()
                .header("Content-Type", "text/html")
                .body(page.into_bytes())
                .unwrap()
        }
    }
}

struct Options {
    input: Vec<String>,
    source: Vec<String>,
    scratch: PathBuf,
    out: PathBuf,
    timeout: Duration,
}

/// Posts real input through the command `--input` names.
struct Input {
    command: Vec<String>,
    origin: (f64, f64),
    unit: f64,
}

impl Input {
    fn run(&self, args: &[String]) -> bool {
        let (program, rest) = self.command.split_first().expect("a command");
        match Command::new(program).args(rest).args(args).output() {
            Ok(out) if out.status.success() => true,
            other => {
                eprintln!("CHECK input {args:?} failed: {other:?}");
                false
            }
        }
    }

    fn at(&self, (x, y): (f64, f64)) -> (i64, i64) {
        (
            (self.origin.0 + x * self.unit).round() as i64,
            (self.origin.1 + y * self.unit).round() as i64,
        )
    }

    fn drag(&self, from: (i64, i64), to: (i64, i64)) -> bool {
        let args = [from.0, from.1, to.0, to.1].map(|v| v.to_string());
        let mut all = vec!["drag".to_string()];
        all.extend(args);
        self.run(&all)
    }
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

fn usage() -> ! {
    eprintln!(
        "usage: drop_check --input CMD [--input ARG]... --source CMD [--source ARG]... \
         [--scratch DIR] [--out DIR] [--timeout SECONDS]\n       drop_check --drag-source FILE X Y"
    );
    std::process::exit(3)
}

/// Saves the whole screen with the OS's own tool. Best effort.
fn screenshot(path: &Path) {
    let status = if cfg!(target_os = "linux") {
        Command::new("import").args(["-window", "root"]).arg(path).status()
    } else if cfg!(target_os = "macos") {
        Command::new("screencapture").arg("-x").arg(path).status()
    } else {
        return;
    };
    if !matches!(status, Ok(s) if s.success()) {
        eprintln!("CHECK note: no screenshot ({status:?})");
    }
}

/// Logs the drag events Tauri delivers, so the log shows what reached it.
fn log_drag(source: &str, label: &str, event: &DragDropEvent) {
    match event {
        DragDropEvent::Enter { paths, .. } => {
            eprintln!("CHECK tauri {source} {label}: enter {paths:?}")
        }
        DragDropEvent::Drop { paths, .. } => {
            eprintln!("CHECK tauri {source} {label}: drop {paths:?}")
        }
        DragDropEvent::Leave => eprintln!("CHECK tauri {source} {label}: leave"),
        _ => {}
    }
}

struct Check {
    session: Session,
    shared: Arc<Shared>,
    input: Input,
    failures: Vec<String>,
}

impl Check {
    fn fail(&mut self, what: String) {
        eprintln!("CHECK FAIL: {what}");
        self.failures.push(what);
    }

    /// Whether the session holds a drop of `path`, waiting up to `limit`.
    /// Taking it uses it up, as attaching it would.
    fn attachable(&self, path: &Path, limit: Duration) -> bool {
        let id = self.session.current_id();
        wait_until(limit, || self.session.take_drop(id, path).is_ok())
    }

    fn native_drop(&mut self, options: &Options, file: &Path) {
        let (program, rest) = options.source.split_first().expect("a source command");
        let child: Option<Child> = Command::new(program)
            .args(rest)
            .arg(file)
            .args([SOURCE_AT.0.to_string(), SOURCE_AT.1.to_string()])
            .spawn()
            .map_err(|err| eprintln!("CHECK the source window couldn't start: {err}"))
            .ok();
        thread::sleep(Duration::from_secs(3));
        let from = (SOURCE_AT.0 + 80, SOURCE_AT.1 + 80);
        let to = self.input.at(TARGET);
        eprintln!("CHECK native drag from {from:?} to {to:?}");
        let sent = self.input.drag(from, to);
        if !sent {
            self.fail("the native drag's input couldn't be sent".into());
        } else if self.attachable(file, DROP_LIMIT) {
            eprintln!("CHECK the native drop is attachable");
        } else {
            self.fail(format!(
                "a file dropped from another app was refused: the session never held {}",
                file.display()
            ));
        }
        if let Some(mut child) = child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn page_drag(&mut self, planted: &Path) {
        let starts = self.shared.drag_starts.load(Ordering::SeqCst);
        let from = self.input.at(LINK);
        let to = self.input.at(PAGE_TARGET);
        eprintln!("CHECK page drag from {from:?} to {to:?}");
        if !self.input.drag(from, to) {
            self.fail("the page drag's input couldn't be sent".into());
            return;
        }
        let started = wait_until(Duration::from_secs(3), || {
            self.shared.drag_starts.load(Ordering::SeqCst) > starts
        });
        if !started {
            self.fail("the page's drag never started, so it proved nothing".into());
        }
        if self.attachable(planted, DROP_LIMIT) {
            self.fail(format!(
                "a drag that started in the page was recorded as a drop: {} is attachable",
                planted.display()
            ));
        } else if started {
            eprintln!("CHECK the page's drag was not recorded");
        }
    }
}

fn run(app: AppHandle<Wry>, shared: Arc<Shared>, session: Session, options: Options) {
    if !wait_until(options.timeout, || shared.loads.load(Ordering::SeqCst) > 0) {
        eprintln!("CHECK FAILED: the page didn't load");
        screenshot(&options.out.join(format!("drop-failed-{}.png", std::env::consts::OS)));
        std::process::exit(1);
    }
    thread::sleep(Duration::from_secs(2));
    let window = app.get_window("main").unwrap();
    let scale = window.scale_factor().unwrap_or(1.0);
    let inner = window.inner_position().unwrap();
    #[cfg(target_os = "macos")]
    let title_bar = hoplodex_lib::services::preview::surface::title_bar_height(
        &app.get_webview("main").unwrap(),
    );
    #[cfg(not(target_os = "macos"))]
    let title_bar = 0.0;
    let input = Input {
        command: options.input.clone(),
        origin: if cfg!(target_os = "macos") {
            (f64::from(inner.x) / scale, f64::from(inner.y) / scale + title_bar)
        } else {
            (f64::from(inner.x), f64::from(inner.y))
        },
        unit: if cfg!(target_os = "macos") { 1.0 } else { scale },
    };
    eprintln!("CHECK window content at {inner:?} scale {scale}");

    let native = options.scratch.join("native.pdf");
    let planted = options.scratch.join("planted.pdf");
    let mut check = Check { session, shared, input, failures: Vec::new() };
    check.native_drop(&options, &native);
    check.page_drag(&planted);
    screenshot(&options.out.join(format!("drop-{}.png", std::env::consts::OS)));

    if check.failures.is_empty() {
        eprintln!("CHECK drops ok");
        app.exit(0);
    } else {
        eprintln!("CHECK {} failure(s)", check.failures.len());
        std::process::exit(4);
    }
}

/// `--drag-source FILE X Y` on Linux: a window of its own process at X Y
/// holding FILE to drag, as a file manager offers one.
#[cfg(target_os = "linux")]
fn drag_source(file: &str, x: i32, y: i32) -> ! {
    use gtk::{gdk, prelude::*};
    gtk::init().expect("GTK starts");
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("drop check source");
    window.set_default_size(160, 160);
    window.move_(x, y);
    let area = gtk::EventBox::new();
    area.add(&gtk::Label::new(Some("drag me")));
    let targets = [gtk::TargetEntry::new("text/uri-list", gtk::TargetFlags::empty(), 0)];
    area.drag_source_set(gdk::ModifierType::BUTTON1_MASK, &targets, gdk::DragAction::COPY);
    let uri = format!("file://{file}");
    area.connect_drag_data_get(move |_, _, data, _, _| {
        data.set_uris(&[uri.as_str()]);
    });
    window.add(&area);
    window.show_all();
    gtk::main();
    std::process::exit(0)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut scratch = None::<PathBuf>;
    let mut out = None::<PathBuf>;
    let mut timeout = Duration::from_secs(40);
    let (mut input, mut source) = (Vec::new(), Vec::new());
    while let Some(flag) = args.next() {
        if flag == "--drag-source" {
            #[cfg(target_os = "linux")]
            {
                let rest: Vec<String> = args.by_ref().collect();
                let [file, x, y] = rest.as_slice() else { usage() };
                drag_source(file, x.parse().unwrap_or(0), y.parse().unwrap_or(0));
            }
            #[cfg(not(target_os = "linux"))]
            usage();
        }
        let value = args.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--scratch" => scratch = Some(value.into()),
            "--out" => out = Some(value.into()),
            "--timeout" => timeout = Duration::from_secs(value.parse().unwrap_or_else(|_| usage())),
            "--input" => input.push(value),
            "--source" => source.push(value),
            _ => usage(),
        }
    }
    if input.is_empty() || source.is_empty() {
        eprintln!("--input and --source are required: the check is driven with real input");
        usage();
    }
    let temp = scratch.is_none().then(|| tempfile::tempdir().unwrap());
    let scratch = scratch.unwrap_or_else(|| temp.as_ref().unwrap().path().to_path_buf());
    std::fs::create_dir_all(&scratch).unwrap();
    // The paths a drop names are the real ones (macOS's /var is /private/var).
    let scratch = scratch.canonicalize().unwrap();
    let out = out.unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/screenshots-out/drop-check")
    });
    std::fs::create_dir_all(&out).unwrap();
    for name in ["native.pdf", "planted.pdf"] {
        std::fs::write(scratch.join(name), b"%PDF-1.4 a drop check file").unwrap();
    }

    // A session open on a scratch database, as after the user unlocks one.
    let session = Session::default();
    let machine = MachineIdentity { id: "d".repeat(32), display_name: "Drop check".into() };
    let passphrase = Passphrase::from_input("drop check passphrase, not a real one".into());
    let db_path = scratch.join("drop-check.hoplodex");
    let opened = db::create_database(&db_path, &passphrase, &machine)
        .map_err(|err| format!("{err:?}"))
        .and_then(|conn| OpenDatabase::new(conn, &db_path).map_err(|err| format!("{err:?}")));
    match opened {
        Ok(open) => session.install(open),
        Err(err) => {
            eprintln!("CHECK FAILED: the scratch database couldn't be made: {err}");
            std::process::exit(1);
        }
    }

    let shared = Arc::new(Shared::default());
    let planted = scratch.join("planted.pdf");
    *shared.planted_uri.lock().unwrap() = if cfg!(windows) {
        format!("file:///{}", planted.display().to_string().replace('\\', "/"))
    } else {
        format!("file://{}", planted.display())
    };
    let options = Mutex::new(Some(Options { input, source, scratch: scratch.clone(), out, timeout }));
    let mut ctx = tauri::generate_context!();
    ctx.config_mut().app.windows.clear();

    let (serve_shared, setup_shared) = (shared.clone(), shared.clone());
    let setup_session = session.clone();
    let builder = tauri::Builder::default()
        .manage(session)
        .register_uri_scheme_protocol(MAIN_SCHEME, move |_ctx, req| serve(&serve_shared, &req))
        .on_window_event(|window, event| {
            if let WindowEvent::DragDrop(drop) = event {
                log_drag("window", window.label(), drop);
            }
        })
        .on_webview_event(|webview, event| {
            if let WebviewEvent::DragDrop(drop) = event {
                log_drag("webview", webview.label(), drop);
            }
        });
    // The app's own recorder, as main.rs registers it.
    drops::register(builder)
        .setup(move |app| {
            let base = if cfg!(windows) {
                format!("http://{MAIN_SCHEME}.localhost/")
            } else {
                format!("{MAIN_SCHEME}://localhost/")
            };
            let main = WebviewWindowBuilder::new(
                app,
                drops::MAIN_WINDOW,
                WebviewUrl::External(base.parse().unwrap()),
            )
            .title("HoploDex drop check")
            .position(MAIN_AT.0, MAIN_AT.1)
            .inner_size(900.0, 700.0)
            .incognito(true);
            #[cfg(windows)]
            let main = main.data_directory(scratch.join("main-webview"));
            let _main = main.build()?;
            let options = options.lock().unwrap().take().expect("setup runs once");
            let (app, shared, session) =
                (app.handle().clone(), setup_shared.clone(), setup_session.clone());
            thread::spawn(move || run(app, shared, session, options));
            Ok(())
        })
        .build(ctx)
        .expect("the check's app failed to build")
        .run(|_, _| {});
    drop(temp);
}
