// The drop check (code scanning alert `paths/cwe-73` on
// `add_document_from_path`): the app's own drop recorder (`drops::register`)
// on a main window opened the way the E2E suite isolates one, with a session
// open on a scratch database, driven with real input. It is built without the
// `e2e` feature, so the drops it sees are the OS's, delivered as a shipped
// build gets them.
//
//   cargo run --profile e2e --example drop_check -- \
//       --input CMD [--input ARG]... --source CMD [--source ARG]... \
//       [--scratch DIR] [--out DIR] [--timeout SECONDS] [--no-refusal]
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
//      URI list naming another file, and drops it on the main web view: the
//      session must never hold that file (a drag that starts in the page is
//      not a drop of the user's files);
//   3. does the same from a second web view in the main window, as the PDF
//      surface is one, onto the main web view.
// It fails (exit 4, one "CHECK FAIL" line each) if the native drop isn't
// attachable, if a drag from the app is, or if a drag from the app never
// started (so that part proved nothing). `--no-refusal` leaves out
// `drops::refuse_app_drags`, to show what it refuses: parts 2 and 3 may then
// fail.
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
    AppHandle, DragDropEvent, LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewEvent,
    WebviewUrl, WebviewWindowBuilder, WindowEvent, Wry, http, webview::PageLoadEvent,
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
  planted.addEventListener('dragstart', (e) => {
    e.dataTransfer.setData('text/uri-list', 'PLANTED');
    e.dataTransfer.setData('text/plain', 'PLANTED');
    e.dataTransfer.effectAllowed = 'copy';
    fetch('/dragstart?from=' + location.pathname);
  });
</script>"#;

/// Where a page's link is (its middle), in logical pixels of its web view.
const LINK: (f64, f64) = (150.0, 70.0);
/// Where the second web view is asked to go in the window. Linux packs it
/// beside the main one instead (`services::preview::surface::linux`), so the
/// check asks each web view where it is.
const OTHER_AT: (f64, f64) = (20.0, 460.0);
const OTHER_SIZE: (f64, f64) = (320.0, 200.0);

/// A web view's place in the window's content area, logical pixels.
#[derive(Debug, Clone, Copy)]
struct Bounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Bounds {
    /// The point `fraction` of the way across and down.
    fn at(&self, fraction: (f64, f64)) -> (f64, f64) {
        (self.x + self.width * fraction.0, self.y + self.height * fraction.1)
    }

    fn link(&self) -> (f64, f64) {
        (self.x + LINK.0, self.y + LINK.1)
    }
}

/// Where `label`'s web view is in its window.
fn bounds(app: &AppHandle<Wry>, label: &str, scale: f64) -> Bounds {
    let webview = app.get_webview(label).unwrap();
    #[cfg(target_os = "linux")]
    {
        use gtk::prelude::*;
        let _ = scale;
        let (send, receive) = std::sync::mpsc::channel();
        webview
            .with_webview(move |view| {
                let view = view.inner();
                let top = view.toplevel().unwrap();
                let (x, y) = view.translate_coordinates(&top, 0, 0).unwrap_or((0, 0));
                let size = view.allocation();
                let _ = send.send(Bounds {
                    x: f64::from(x),
                    y: f64::from(y),
                    width: f64::from(size.width()),
                    height: f64::from(size.height()),
                });
            })
            .unwrap();
        receive.recv_timeout(Duration::from_secs(5)).unwrap()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let at = webview.position().unwrap();
        let size = webview.size().unwrap();
        Bounds {
            x: f64::from(at.x) / scale,
            y: f64::from(at.y) / scale,
            width: f64::from(size.width) / scale,
            height: f64::from(size.height) / scale,
        }
    }
}
/// Where the source window goes, in the input command's units.
const SOURCE_AT: (i64, i64) = (40, 160);
/// The main window's top-left corner, logical pixels.
const MAIN_AT: (f64, f64) = (300.0, 60.0);

/// How long a drop may take to be recorded after the input that made it.
const DROP_LIMIT: Duration = Duration::from_secs(6);

/// The pages that finished loading: both web views'.
static FINISHED: AtomicUsize = AtomicUsize::new(0);

#[derive(Default)]
struct Shared {
    drag_starts: AtomicUsize,
    planted_uri: Mutex<String>,
}

fn serve(shared: &Shared, req: &http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let empty = || http::Response::builder().body(Vec::new()).unwrap();
    match req.uri().path() {
        "/dragstart" => {
            shared.drag_starts.fetch_add(1, Ordering::SeqCst);
            let from = req.uri().query().unwrap_or("");
            eprintln!("CHECK a page's drag started ({from})");
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
    refusal: bool,
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

    /// A drag from `from` to `to`, by way of `via` if there is one.
    fn drag(&self, from: (i64, i64), to: (i64, i64), via: Option<(i64, i64)>) -> bool {
        let mut all = vec!["drag".to_string()];
        all.extend([from.0, from.1, to.0, to.1].map(|v| v.to_string()));
        if let Some(via) = via {
            all.extend([via.0, via.1].map(|v| v.to_string()));
        }
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
         [--scratch DIR] [--out DIR] [--timeout SECONDS] [--no-refusal]\n       drop_check --drag-source FILE X Y"
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

/// Prints the app's own drop lines (`drops`), so the log shows each refusal.
struct Capture;

impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if record.module_path().unwrap_or("").contains("drops") {
            eprintln!("CHECK app: {}", record.args());
        }
    }

    fn flush(&self) {}
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

    fn native_drop(&mut self, options: &Options, file: &Path, target: (f64, f64)) {
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
        let to = self.input.at(target);
        eprintln!("CHECK native drag from {from:?} to {to:?}");
        let sent = self.input.drag(from, to, None);
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

    /// Drags from `from` (screen units of the input command) onto the main
    /// web view at `target` (logical pixels of the window's content), by way
    /// of `via` (screen units): a drag that starts in the app.
    fn app_drag(
        &mut self,
        what: &str,
        from: (i64, i64),
        target: (f64, f64),
        via: Option<(i64, i64)>,
        planted: &Path,
    ) {
        // Only a page reports its drag's start.
        let page = what.contains("page") || what.contains("web view");
        let starts = self.shared.drag_starts.load(Ordering::SeqCst);
        let to = self.input.at(target);
        eprintln!("CHECK drag from {what}: from {from:?} to {to:?} by way of {via:?}");
        if !self.input.drag(from, to, via) {
            self.fail(format!("the input of the drag from {what} couldn't be sent"));
            return;
        }
        let started = !page
            || wait_until(Duration::from_secs(3), || {
                self.shared.drag_starts.load(Ordering::SeqCst) > starts
            });
        if !started {
            self.fail(format!("the drag from {what} never started, so it proved nothing"));
        }
        if self.attachable(planted, DROP_LIMIT) {
            self.fail(format!(
                "a drag from {what} was recorded as a drop: {} is attachable",
                planted.display()
            ));
        } else if started {
            eprintln!("CHECK the drag from {what} was not recorded");
        }
    }
}

fn run(app: AppHandle<Wry>, shared: Arc<Shared>, session: Session, options: Options) {
    if !wait_until(options.timeout, || FINISHED.load(Ordering::SeqCst) >= 2) {
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
    let main = bounds(&app, drops::MAIN_WINDOW, scale);
    #[allow(unused_mut)]
    let mut other = bounds(&app, "other", scale);
    // macOS places a child web view from the window's frame, title bar
    // included, not its content area.
    #[cfg(target_os = "macos")]
    {
        other.y -= title_bar;
    }
    eprintln!("CHECK main web view at {main:?}, second web view at {other:?}");
    // The main web view's area away from where the second one is laid over it
    // (macOS and Windows) or beside it (Linux).
    let (open, page_target) = if cfg!(target_os = "linux") {
        (main.at((0.5, 0.6)), main.at((0.8, 0.8)))
    } else {
        (main.at((0.6, 0.35)), main.at((0.8, 0.5)))
    };
    check.native_drop(&options, &native, open);
    let (main_link, other_link) = (check.input.at(main.link()), check.input.at(other.link()));
    check.app_drag("the main page", main_link, page_target, None, &planted);
    check.app_drag("a second web view", other_link, open, None, &planted);
    // Out of the window (above it, on the screen's edge) and back.
    let outside = (main_link.0, 5);
    check.app_drag("the main page, out and back", main_link, open, Some(outside), &planted);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        // A window of the app's own process holding the planted file, as a
        // file manager offers one: a page's drag that WebKit kept to itself
        // above reached nothing, so this shows the refusal at work on a drag
        // that does reach wry, with its source in the process.
        let (send, receive) = std::sync::mpsc::channel();
        let file = planted.display().to_string();
        app.run_on_main_thread(move || {
            in_app_source(&file, IN_APP_SOURCE_AT);
            let _ = send.send(());
        })
        .unwrap();
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
        thread::sleep(Duration::from_secs(2));
        let from = (IN_APP_SOURCE_AT.0 + 80, IN_APP_SOURCE_AT.1 + 80);
        check.app_drag("a window of the app's own process", from, open, None, &planted);
    }
    screenshot(&options.out.join(format!("drop-{}.png", std::env::consts::OS)));

    if check.failures.is_empty() {
        eprintln!("CHECK drops ok");
        app.exit(0);
    } else {
        eprintln!("CHECK {} failure(s)", check.failures.len());
        std::process::exit(4);
    }
}

/// Where the in-process source window goes, in the input command's units.
#[cfg(any(target_os = "linux", target_os = "macos"))]
const IN_APP_SOURCE_AT: (i64, i64) = (40, 780);

/// Shows a window of this process at `at` holding `file` to drag, kept for
/// the process's life. On the main thread.
#[cfg(target_os = "linux")]
fn in_app_source(file: &str, at: (i64, i64)) {
    std::mem::forget(source_window(file, at.0 as i32, at.1 as i32));
}

/// Shows a window of this process at `at` (points from the screen's top
/// left) holding `file` to drag, kept for the process's life: a borderless
/// window whose view starts a drag of the file's URL. On the main thread.
#[cfg(target_os = "macos")]
fn in_app_source(file: &str, at: (i64, i64)) {
    use objc2::{
        msg_send,
        rc::{Allocated, Retained},
        runtime::{AnyClass, AnyObject, ClassBuilder, Sel},
        sel,
    };
    use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString, NSURL};

    static FILE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let _ = FILE.set(file.to_owned());

    unsafe extern "C-unwind" fn mouse_dragged(this: &AnyObject, _: Sel, event: &AnyObject) {
        let url = NSURL::fileURLWithPath(&NSString::from_str(FILE.get().unwrap()));
        // SAFETY: AppKit's public classes and methods, with their arguments.
        unsafe {
            let item: Allocated<AnyObject> =
                msg_send![AnyClass::get(c"NSDraggingItem").unwrap(), alloc];
            let item: Retained<AnyObject> = msg_send![item, initWithPasteboardWriter: &*url];
            let frame = NSRect::new(NSPoint::new(40.0, 40.0), NSSize::new(80.0, 80.0));
            let () =
                msg_send![&*item, setDraggingFrame: frame, contents: std::ptr::null::<AnyObject>()];
            let items = NSArray::from_retained_slice(&[item]);
            let _: *mut AnyObject =
                msg_send![this, beginDraggingSessionWithItems: &*items, event: event, source: this];
        }
    }
    /// `draggingSession:sourceOperationMaskForDraggingContext:`: copy.
    unsafe extern "C-unwind" fn operations(
        _: &AnyObject,
        _: Sel,
        _: &AnyObject,
        _: isize,
    ) -> usize {
        1
    }

    // SAFETY: a new NSView subclass with two methods of the signatures
    // AppKit calls them with, then AppKit's public window methods.
    unsafe {
        let mut builder =
            ClassBuilder::new(c"DropCheckSource", AnyClass::get(c"NSView").unwrap()).unwrap();
        builder
            .add_method(sel!(mouseDragged:), mouse_dragged as unsafe extern "C-unwind" fn(_, _, _));
        builder.add_method(
            sel!(draggingSession:sourceOperationMaskForDraggingContext:),
            operations as unsafe extern "C-unwind" fn(_, _, _, _) -> _,
        );
        let class = builder.register();
        let screen: *mut AnyObject = msg_send![AnyClass::get(c"NSScreen").unwrap(), mainScreen];
        let screen: NSRect = msg_send![screen, frame];
        let rect = NSRect::new(
            NSPoint::new(at.0 as f64, screen.size.height - at.1 as f64 - 160.0),
            NSSize::new(160.0, 160.0),
        );
        let window: Allocated<AnyObject> = msg_send![AnyClass::get(c"NSWindow").unwrap(), alloc];
        // Borderless (0), buffered (2).
        let window: Retained<AnyObject> = msg_send![window, initWithContentRect: rect, styleMask: 0usize, backing: 2usize, defer: false];
        let view: Allocated<AnyObject> = msg_send![class, alloc];
        let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(160.0, 160.0));
        let view: Retained<AnyObject> = msg_send![view, initWithFrame: bounds];
        let () = msg_send![&*window, setReleasedWhenClosed: false];
        let () = msg_send![&*window, setContentView: &*view];
        // Floating (3), over the main window.
        let () = msg_send![&*window, setLevel: 3isize];
        let () = msg_send![&*window, makeKeyAndOrderFront: std::ptr::null::<AnyObject>()];
        std::mem::forget(window);
    }
}

/// `--drag-source FILE X Y` on Linux: a window of its own process at X Y
/// holding FILE to drag, as a file manager offers one.
#[cfg(target_os = "linux")]
fn drag_source(file: &str, x: i32, y: i32) -> ! {
    gtk::init().expect("GTK starts");
    let _window = source_window(file, x, y);
    gtk::main();
    std::process::exit(0)
}

/// A window at X Y holding FILE to drag.
#[cfg(target_os = "linux")]
fn source_window(file: &str, x: i32, y: i32) -> gtk::Window {
    use gtk::{gdk, prelude::*};
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
    window
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut scratch = None::<PathBuf>;
    let mut out = None::<PathBuf>;
    let mut timeout = Duration::from_secs(40);
    let (mut input, mut source) = (Vec::new(), Vec::new());
    let mut refusal = true;
    while let Some(flag) = args.next() {
        if flag == "--no-refusal" {
            refusal = false;
            continue;
        }
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
    log::set_logger(&Capture).expect("no logger yet");
    log::set_max_level(log::LevelFilter::Info);
    let temp = scratch.is_none().then(|| tempfile::tempdir().unwrap());
    let scratch = scratch.unwrap_or_else(|| temp.as_ref().unwrap().path().to_path_buf());
    std::fs::create_dir_all(&scratch).unwrap();
    // The paths a drop names are the real ones (macOS's /var is /private/var),
    // without Windows' verbatim prefix, which a drop's paths don't have.
    let scratch = scratch.canonicalize().unwrap();
    let scratch = match scratch.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(plain) => PathBuf::from(plain),
        None => scratch,
    };
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
    let options = Mutex::new(Some(Options {
        refusal,
        input,
        source,
        scratch: scratch.clone(),
        out,
        timeout,
    }));
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
        .on_page_load(|webview, payload| {
            if let PageLoadEvent::Finished = payload.event() {
                eprintln!("CHECK {} loaded {}", webview.label(), payload.url());
                FINISHED.fetch_add(1, Ordering::SeqCst);
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
            let main = main.build()?;
            let options = options.lock().unwrap().take().expect("setup runs once");
            // The app's own refusal, as `setup()` installs it.
            if options.refusal {
                drops::refuse_app_drags(&main)?;
            } else {
                drops::record_without_refusal();
                eprintln!("CHECK note: without drops::refuse_app_drags");
            }
            // A second web view in the window, as the PDF surface is one.
            let other = WebviewBuilder::new(
                "other",
                WebviewUrl::External(format!("{base}other").parse().unwrap()),
            );
            // A web context of its own, so the check's protocol is registered
            // for it too (WebKitGTK shows "The URL can't be shown" otherwise).
            #[cfg(windows)]
            let other = other.data_directory(scratch.join("main-webview"));
            #[cfg(not(windows))]
            let other = other.incognito(true).data_directory(scratch.join("other-webview"));
            main.as_ref().window().add_child(
                other,
                LogicalPosition::new(OTHER_AT.0, OTHER_AT.1),
                LogicalSize::new(OTHER_SIZE.0, OTHER_SIZE.1),
            )?;
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
