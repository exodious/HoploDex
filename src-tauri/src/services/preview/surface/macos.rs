//! The surface on macOS: WKWebView's content rule list, switches, input
//! monitor and the copy watch (research.md §6, §7, §10).
//!
//! `attach` runs once after `add_child`, from a thread other than the main
//! one, and does what research.md asks of WKWebView before the document is
//! asked for:
//!
//! - the PDF HUD and WebRTC off through WebKit's SPI
//!   ([`webkit_switches::switch_off`], the same code the startup check
//!   tries), read back;
//! - the content rule list, compiled into a store under the cache folder and
//!   added to the web view's controller (every URL blocked but the
//!   surface's own);
//! - a local `NSEvent` monitor for Escape, F6 and activity, which only
//!   acts while the surface's view is, or holds, the first responder (keys)
//!   or under the pointer (mouse and scroll);
//! - the watch (research.md §7, FR-003a): a `kqueue` on `$TMPDIR`.
//!
//! A surface that can't have all of that (an SPI that is gone, a rule list
//! that won't compile, a watch that can't start) is refused: `attach`
//! returns an error and the caller closes it, so a PDF is never shown
//! without every measure.
//!
//! The monitor and the watch end with the surface, at an exact point:
//! `Surface::close` (and a surface refused by `attach`) calls [`on_close`],
//! which has the watch's thread make its last sweep and end, and removes the
//! monitor. (A `WKWebView` is not freed when Tauri closes it, checked in the
//! macOS VM: it outlives `Webview::close` by minutes, so neither an
//! associated object's `dealloc` nor the view's absence from the window is
//! a signal worth polling for.) Only the newest surface is kept: attaching
//! another ends the one before it the same way, and a close of a surface that
//! has been replaced ends nothing.

use std::{
    cell::RefCell,
    collections::HashMap,
    collections::HashSet,
    ffi::OsString,
    fs,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    path::{Path, PathBuf},
    ptr::{self, NonNull},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use block2::RcBlock;
use objc2::{
    msg_send,
    rc::{Retained, Weak},
    runtime::AnyObject,
};
use objc2_app_kit::{NSEvent, NSEventMask, NSEventType};
use objc2_foundation::{NSError, NSPoint, NSString, NSURL};
use objc2_web_kit::{WKContentRuleList, WKContentRuleListStore, WKWebView};
use tauri::{Manager, Runtime, Url, Webview, Window, webview::WebviewBuilder};

use super::{Hooks, Rect};
use crate::services::{
    machine_settings::MachineSettings,
    preview::{
        PdfEndReason,
        availability::{PdfAvailabilityState, webkit_switches},
    },
};

/// The proxy: `proxy_url` (Tauri's `macos-proxy` feature). Loopback bypasses
/// it on macOS, so the content filter is the layer that matters there
/// (research.md §6).
pub(super) fn customize<R: Runtime>(
    builder: WebviewBuilder<R>,
    proxy_url: &Url,
    _data_directory: &Path,
) -> WebviewBuilder<R> {
    builder.proxy_url(proxy_url.clone())
}

/// The folder under the cache folder where the compiled rule list is kept.
/// It holds rules only, no document content.
const FILTER_STORE: &str = "preview-filter-store";

/// The identifier the rule list is stored under.
const FILTER_ID: &str = "preview-block-all";

/// How long the rule list may take to compile and install before the surface
/// is refused. WebKit compiles it on its own thread, in a few milliseconds.
const FILTER_TIMEOUT: Duration = Duration::from_secs(10);

/// The content filter (research.md §6): the same rules as WebKitGTK's. Every
/// URL is blocked but the surface's own scheme; `blob:`, `data:` and
/// `about:` name nothing outside the page.
const BLOCK_RULES: &str = r#"[
  {"trigger":{"url-filter":".*"},"action":{"type":"block"}},
  {"trigger":{"url-filter":"^hdpreview:"},"action":{"type":"ignore-previous-rules"}},
  {"trigger":{"url-filter":"^blob:"},"action":{"type":"ignore-previous-rules"}},
  {"trigger":{"url-filter":"^data:"},"action":{"type":"ignore-previous-rules"}},
  {"trigger":{"url-filter":"^about:"},"action":{"type":"ignore-previous-rules"}}
]"#;

/// `kVK_Escape` and `kVK_F6`.
const KEY_ESCAPE: u16 = 53;
const KEY_F6: u16 = 97;

/// The name WebKit gives the folder Open in Preview's copy is written into,
/// in `$TMPDIR` (research.md §7).
const COPY_FOLDER_PREFIX: &str = "WebKitPDFs-";

/// Configures the surface (switches, rule list, monitor, watch), and waits
/// for the rule list to be installed. See the module's header. `hud_on` is
/// the surface check's `--hud-on` variant: the app always passes `false`.
pub(super) fn attach<R: Runtime>(
    _window: &Window<R>,
    webview: &Webview<R>,
    hooks: &Arc<Hooks>,
    hud_on: bool,
) -> tauri::Result<()> {
    let store = crate::app_dirs::cache_dir(webview.app_handle())?.join(FILTER_STORE);
    fs::create_dir_all(&store)?;
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = webview.app_handle().clone();
    // Before the web view can load anything, so a copy made from the first
    // document is a new one.
    let watch =
        Watch::start(std::env::temp_dir(), caught_copies_handler(app.clone(), Arc::clone(hooks)))?;
    let replaced = CURRENT.lock().unwrap_or_else(|e| e.into_inner()).replace(Current {
        generation,
        view: 0,
        watch,
    });
    if let Some(old) = replaced {
        end_surface(&app, old);
    }
    let attached = attach_view(webview, Arc::clone(hooks), store, generation, hud_on);
    if attached.is_err() {
        // The caller closes the surface, which would end it too; this makes
        // it independent of that.
        end_current(&app, |current| current.generation == generation);
    }
    attached
}

fn attach_view<R: Runtime>(
    webview: &Webview<R>,
    hooks: Arc<Hooks>,
    store: PathBuf,
    generation: u64,
    hud_on: bool,
) -> tauri::Result<()> {
    let (done, outcome) = mpsc::channel::<Result<(), String>>();
    webview.with_webview(move |platform| {
        // SAFETY: on macOS, Tauri's `PlatformWebview::inner` is the
        // `WKWebView`, and this runs on the main thread.
        let view: &WKWebView = unsafe { &*platform.inner().cast() };
        if let Some(current) = CURRENT.lock().unwrap_or_else(|e| e.into_inner()).as_mut()
            && current.generation == generation
        {
            current.view = ptr::from_ref(view) as usize;
        }
        match harden(view, &store, done.clone(), hud_on) {
            Ok(()) => {
                if let Some(monitor) = input_monitor(view, hooks) {
                    MONITORS.with(|m| m.borrow_mut().insert(generation, monitor));
                }
            }
            // The caller closes the surface, which ends the watch.
            Err(why) => {
                let _ = done.send(Err(why));
            }
        }
    })?;
    match outcome.recv_timeout(FILTER_TIMEOUT) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(why)) => Err(std::io::Error::other(why).into()),
        Err(_) => Err(std::io::Error::other("the content filter was not installed in time").into()),
    }
}

/// The surface's web view is being closed (called from `Surface::close`, before
/// `Webview::close`): the surface's watch makes its last sweep and ends, and
/// its input monitor is removed. Nothing if `webview` is not the current
/// surface's (one a newer surface has replaced has ended already).
pub(super) fn on_close<R: Runtime>(webview: &Webview<R>) {
    let app = webview.app_handle().clone();
    let on_main = app.clone();
    let asked = webview.with_webview(move |platform| {
        let view = platform.inner() as usize;
        // The last click may not land after the last sweep: hidden first.
        // SAFETY: `inner` is the `WKWebView`; a plain setter on the main thread.
        let _: () = unsafe { msg_send![platform.inner().cast::<AnyObject>(), setHidden: true] };
        end_current(&on_main, |current| current.view == view || current.view == 0);
    });
    if asked.is_err() {
        // The web view is already gone, so it is the surface that was current.
        end_current(&app, |_| true);
    }
}

/// The SPI switches, then the compile and install of the rule list, which
/// reports through `done` from the main thread when it finishes.
fn harden(
    view: &WKWebView,
    store: &Path,
    done: mpsc::Sender<Result<(), String>>,
    hud_on: bool,
) -> Result<(), String> {
    // `configuration` is a copy, but its preferences and user content
    // controller are the web view's own.
    // SAFETY: plain getters, on the main thread.
    let (prefs, controller) = unsafe {
        let config = view.configuration();
        (config.preferences(), config.userContentController())
    };
    webkit_switches::switch_off(&prefs, hud_on)?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(&store.to_string_lossy()));
    let mtm = objc2::MainThreadMarker::new().ok_or("not on the main thread")?;
    // SAFETY: `url` is a file URL.
    let Some(rule_store) = (unsafe { WKContentRuleListStore::storeWithURL(Some(&url), mtm) })
    else {
        return Err("the content filter's store could not be made".into());
    };
    let installed = RcBlock::new(move |list: *mut WKContentRuleList, error: *mut NSError| {
        // SAFETY: WebKit passes the list or the error, the other null.
        let outcome = match unsafe { list.as_ref() } {
            Some(list) => {
                unsafe { controller.addContentRuleList(list) };
                Ok(())
            }
            None => {
                let message = unsafe { error.as_ref() }.map_or_else(
                    || "no reason given".to_owned(),
                    |e| e.localizedDescription().to_string(),
                );
                Err(format!("the content filter could not be compiled: {message}"))
            }
        };
        let _ = done.send(outcome);
    });
    // SAFETY: the identifier and the rules are strings; the handler is a
    // block of the signature WebKit declares.
    unsafe {
        rule_store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
            Some(&NSString::from_str(FILTER_ID)),
            Some(&NSString::from_str(BLOCK_RULES)),
            Some(&installed),
        );
    }
    Ok(())
}

/// What a key press in the surface means to the app.
#[derive(Debug, PartialEq, Eq)]
enum Key {
    Escape,
    F6,
    Other,
}

fn classify_key(key_code: u16) -> Key {
    match key_code {
        KEY_ESCAPE => Key::Escape,
        KEY_F6 => Key::F6,
        _ => Key::Other,
    }
}

/// A local `NSEvent` monitor (research.md §10): Escape and F6 are consumed
/// and told to the app while the surface's view is, or holds, the first
/// responder; any key then, and a mouse or scroll event over the view, is
/// activity. Events pass on to the page. Returns the monitor, to remove
/// with `NSEvent::removeMonitor`.
fn input_monitor(view: &WKWebView, hooks: Arc<Hooks>) -> Option<Retained<AnyObject>> {
    // Weak: the monitor must not keep the view alive (see the header).
    let weak = Weak::from_retained(&unsafe { Retained::retain(ptr::from_ref(view).cast_mut()) }?);
    let handler = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit passes a live event, which is returned unchanged
        // unless it is consumed.
        let event_ref = unsafe { event.as_ref() };
        let Some(view) = weak.load() else { return event.as_ptr() };
        if event_ref.r#type() == NSEventType::KeyDown {
            if !in_first_responder(&view, event_ref) {
                return event.as_ptr();
            }
            return match classify_key(event_ref.keyCode()) {
                Key::Escape => {
                    hooks.escape();
                    ptr::null_mut()
                }
                Key::F6 => {
                    hooks.focus_chrome();
                    ptr::null_mut()
                }
                Key::Other => {
                    hooks.activity();
                    event.as_ptr()
                }
            };
        }
        if under_the_pointer(&view, event_ref) {
            hooks.activity();
        }
        event.as_ptr()
    });
    let mask = NSEventMask::KeyDown
        | NSEventMask::LeftMouseDown
        | NSEventMask::RightMouseDown
        | NSEventMask::OtherMouseDown
        | NSEventMask::LeftMouseDragged
        | NSEventMask::MouseMoved
        | NSEventMask::ScrollWheel;
    // SAFETY: the block returns the event it was given or null.
    let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &handler) };
    if monitor.is_none() {
        log::warn!("the preview surface's input monitor could not be added");
    }
    monitor
}

/// The key event is for the surface: its window's first responder is the
/// view or inside it, and the view is shown.
fn in_first_responder(view: &WKWebView, event: &NSEvent) -> bool {
    // SAFETY: plain AppKit getters on the main thread.
    unsafe {
        let hidden: bool = msg_send![view, isHiddenOrHasHiddenAncestor];
        let window: Option<Retained<AnyObject>> = msg_send![view, window];
        let event_window: Option<Retained<AnyObject>> = msg_send![event, window];
        let (Some(window), Some(event_window)) = (window, event_window) else { return false };
        if hidden || !ptr::eq(&*window, &*event_window) {
            return false;
        }
        let responder: Option<Retained<AnyObject>> = msg_send![&*window, firstResponder];
        let Some(responder) = responder else { return false };
        let is_view: bool =
            msg_send![&*responder, respondsToSelector: objc2::sel!(isDescendantOf:)];
        is_view && msg_send![&*responder, isDescendantOf: view]
    }
}

/// The mouse or scroll event is over the shown surface.
fn under_the_pointer(view: &WKWebView, event: &NSEvent) -> bool {
    // SAFETY: plain AppKit getters on the main thread.
    unsafe {
        let window: Option<Retained<AnyObject>> = msg_send![view, window];
        let event_window: Option<Retained<AnyObject>> = msg_send![event, window];
        let (Some(window), Some(event_window)) = (window, event_window) else { return false };
        if !ptr::eq(&*window, &*event_window) {
            return false;
        }
        let superview: Option<Retained<AnyObject>> = msg_send![view, superview];
        let Some(superview) = superview else { return false };
        let in_window: NSPoint = event.locationInWindow();
        let nobody: *const AnyObject = ptr::null();
        let point: NSPoint = msg_send![&*superview, convertPoint: in_window, fromView: nobody];
        // A hidden view is never hit.
        let hit: Option<Retained<AnyObject>> = msg_send![view, hitTest: point];
        hit.is_some()
    }
}

/// Which surface is the current one: each `attach` takes the next number.
static GENERATION: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// The input monitors, by surface, on the main thread (a monitor token
    /// belongs there).
    static MONITORS: RefCell<HashMap<u64, Retained<AnyObject>>> = RefCell::new(HashMap::new());
}

/// The surface that is current: what ends with it.
struct Current {
    generation: u64,
    /// The `WKWebView`'s address, set on the main thread by `attach` (0 until
    /// then): how `on_close` knows its web view is this one.
    view: usize,
    watch: Watch,
}

static CURRENT: Mutex<Option<Current>> = Mutex::new(None);

/// Ends the current surface if `which` says so: its watch makes the last
/// sweep, and its input monitor is removed on the main thread.
fn end_current<R: Runtime>(app: &tauri::AppHandle<R>, which: impl FnOnce(&Current) -> bool) {
    let mut current = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    if current.as_ref().is_some_and(which) {
        let ended = current.take().expect("just checked");
        drop(current);
        end_surface(app, ended);
    }
}

fn end_surface<R: Runtime>(app: &tauri::AppHandle<R>, ended: Current) {
    ended.watch.stop();
    let generation = ended.generation;
    let _ = app.run_on_main_thread(move || remove_monitor(generation));
}

/// Removes surface `generation`'s input monitor. Main thread.
fn remove_monitor(generation: u64) {
    if let Some(monitor) = MONITORS.with(|m| m.borrow_mut().remove(&generation)) {
        // SAFETY: made by `addLocalMonitor…`, removed once.
        unsafe { NSEvent::removeMonitor(&monitor) };
    }
}

/// Called with the paths of the copies the watch deleted, and whether the
/// surface should be closed for it (not at the last sweep, when it is
/// closing already). The call with no paths and `false` is the last.
type CaughtHandler = Box<dyn Fn(&[PathBuf], bool) + Send + 'static>;

/// What the app does when a copy is caught (research.md §7): records each
/// path in the hold's `leftovers` until a later sweep finds it gone, holds
/// PDF preview off for this version, and closes the surface and tells the
/// viewer (`preview:pdf-ended { copyCaught }`). The last sweep (`ending`)
/// deletes what the hold still names.
fn caught_copies_handler<R: Runtime>(app: tauri::AppHandle<R>, hooks: Arc<Hooks>) -> CaughtHandler {
    Box::new(move |paths, announce| {
        let machine = app.try_state::<MachineSettings>();
        if !paths.is_empty() {
            log::error!(
                "the PDF viewer wrote {} copy(ies) of a document to disk; deleted, and PDF \
                 preview is held off",
                paths.len()
            );
            if let Some(machine) = &machine {
                for path in paths {
                    machine.add_hold_leftover(path);
                }
                match app.try_state::<PdfAvailabilityState>() {
                    Some(availability) => availability.hold(machine),
                    None => machine.hold_pdf_preview(env!("CARGO_PKG_VERSION")),
                }
            }
        }
        if !paths.is_empty() && announce {
            hooks.end(PdfEndReason::CopyCaught);
        }
        if !announce && let Some(machine) = &machine {
            machine.sweep_hold_leftovers();
        }
    })
}

/// The new `WebKitPDFs-*` entries of `folder`: those not in `known`, which
/// are added to it. Entries are listed by name, so a name never repeats.
fn new_copies(folder: &Path, known: &mut HashSet<OsString>) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(folder) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(COPY_FOLDER_PREFIX))
        .filter(|e| known.insert(e.file_name()))
        .map(|e| e.path())
        .collect()
}

/// Deletes a copy at once, contents first (a folder, or a file if WebKit
/// ever writes one there). Tries again while WebKit is still writing into it.
fn delete_copy(path: &Path) {
    for _ in 0..50 {
        let result = match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
            Ok(_) => fs::remove_file(path),
            Err(_) => return,
        };
        if result.is_ok() || !path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(2));
    }
    log::warn!("a copy the PDF viewer wrote could not be deleted: {}", path.display());
}

/// Lists `folder` for copies that are new, deletes each, and returns their
/// paths.
fn sweep(folder: &Path, known: &mut HashSet<OsString>) -> Vec<PathBuf> {
    let found = new_copies(folder, known);
    for path in &found {
        delete_copy(path);
    }
    found
}

/// The `WebKitPDFs-*` entries in `folder` now.
fn existing_copies(folder: &Path) -> HashSet<OsString> {
    let mut known = HashSet::new();
    new_copies(folder, &mut known);
    known
}

fn kevent_zero() -> libc::kevent {
    // SAFETY: `kevent` is plain integers and a pointer; all zero is valid.
    unsafe { std::mem::zeroed() }
}

/// The `kevent` identifier of the user event that stops the watch.
const STOP: libc::uintptr_t = 1;

/// The watch (research.md §7): a thread holding a `kqueue` on `$TMPDIR`
/// (`EVFILT_VNODE`, `NOTE_WRITE`), and a user event to end it.
struct Watch {
    queue: Arc<OwnedFd>,
}

impl Watch {
    /// Registers the folder and the stop event with a new `kqueue`, takes the
    /// entries already there, and starts the thread, which runs until
    /// [`Watch::stop`], makes a last sweep and ends. The registration comes
    /// first, so no entry made after it can be missed.
    fn start(folder: PathBuf, caught: CaughtHandler) -> std::io::Result<Self> {
        use std::os::unix::ffi::OsStrExt;
        let path = std::ffi::CString::new(folder.as_os_str().as_bytes())
            .map_err(|_| std::io::Error::other("$TMPDIR has a NUL in it"))?;
        // SAFETY: plain syscalls; each descriptor is checked and owned once.
        let (queue, dir) = unsafe {
            let queue = libc::kqueue();
            if queue < 0 {
                return Err(std::io::Error::last_os_error());
            }
            let queue = OwnedFd::from_raw_fd(queue);
            let dir = libc::open(path.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC);
            if dir < 0 {
                return Err(std::io::Error::last_os_error());
            }
            (Arc::new(queue), OwnedFd::from_raw_fd(dir))
        };
        let mut watch_dir = kevent_zero();
        watch_dir.ident = dir.as_raw_fd() as libc::uintptr_t;
        watch_dir.filter = libc::EVFILT_VNODE;
        watch_dir.flags = libc::EV_ADD | libc::EV_CLEAR;
        watch_dir.fflags = libc::NOTE_WRITE;
        let mut stop = kevent_zero();
        stop.ident = STOP;
        stop.filter = libc::EVFILT_USER;
        stop.flags = libc::EV_ADD | libc::EV_CLEAR;
        let changes = [watch_dir, stop];
        // SAFETY: two valid events; no events are asked for.
        let registered = unsafe {
            libc::kevent(queue.as_raw_fd(), changes.as_ptr(), 2, ptr::null_mut(), 0, ptr::null())
        };
        if registered < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let known = existing_copies(&folder);
        let thread_queue = Arc::clone(&queue);
        thread::Builder::new()
            .name("preview-pdf-watch".into())
            .spawn(move || run(&thread_queue, dir, &folder, known, &caught))?;
        Ok(Self { queue })
    }

    /// Has the thread make its last sweep and end. Returns at once.
    fn stop(&self) {
        let mut trigger = kevent_zero();
        trigger.ident = STOP;
        trigger.filter = libc::EVFILT_USER;
        trigger.fflags = libc::NOTE_TRIGGER;
        // SAFETY: one valid event; no events are asked for.
        let n = unsafe {
            libc::kevent(self.queue.as_raw_fd(), &trigger, 1, ptr::null_mut(), 0, ptr::null())
        };
        if n < 0 {
            log::warn!(
                "the PDF watch could not be told to stop: {}",
                std::io::Error::last_os_error()
            );
        }
    }
}

/// The thread: on each write to the folder, deletes the new copies and says
/// so. On the stop event, the same once more and the hold's leftovers swept
/// (a copy found then is not announced: the surface is closed).
fn run(
    queue: &OwnedFd,
    dir: OwnedFd,
    folder: &Path,
    mut known: HashSet<OsString>,
    caught: &CaughtHandler,
) {
    let _dir = dir;
    let mut events = [kevent_zero(), kevent_zero()];
    loop {
        // SAFETY: room for the two events it is told of; no timeout.
        let n = unsafe {
            libc::kevent(queue.as_raw_fd(), ptr::null(), 0, events.as_mut_ptr(), 2, ptr::null())
        };
        if n < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }
        let stopping = events[..n as usize].iter().any(|e| e.filter == libc::EVFILT_USER);
        let found = sweep(folder, &mut known);
        if !found.is_empty() {
            caught(&found, !stopping);
        }
        if stopping {
            caught(&[], false);
            break;
        }
    }
}

/// How far the page's top is below the top of the window's content view: the
/// title bar's height. The main window has a full-size content view (the
/// style mask has `NSWindowStyleMaskFullSizeContentView`), so its content
/// view runs under the title bar while the page starts below it
/// (`contentLayoutRect`), and wry places a child web view by the content
/// view's height. 0 for a window whose content view starts at the page, or
/// when it can't be read.
pub(super) fn title_bar_height<R: Runtime>(webview: &Webview<R>) -> f64 {
    use objc2_foundation::NSRect;
    let (sender, receiver) = mpsc::channel();
    let asked = webview.with_webview(move |platform| {
        // SAFETY: plain getters on the main thread: the surface's web view,
        // its window and that window's content view.
        let height = unsafe {
            let view: *mut AnyObject = platform.inner().cast();
            let window: *mut AnyObject = msg_send![view, window];
            if window.is_null() {
                0.0
            } else {
                let content: *mut AnyObject = msg_send![window, contentView];
                let frame: NSRect = msg_send![content, frame];
                let layout: NSRect = msg_send![window, contentLayoutRect];
                (frame.size.height - layout.origin.y - layout.size.height).max(0.0)
            }
        };
        let _ = sender.send(height);
    });
    if asked.is_err() {
        return 0.0;
    }
    receiver.recv_timeout(Duration::from_secs(2)).unwrap_or(0.0)
}

/// Places the surface over `bounds`, which are in the page's own coordinates,
/// as the layout reports them.
pub(super) fn place<R: Runtime>(
    webview: &Webview<R>,
    bounds: Rect,
    visible: bool,
) -> tauri::Result<()> {
    let below_the_title_bar = Rect { y: bounds.y + title_bar_height(webview), ..bounds };
    super::place_with_tauri(webview, below_the_title_bar, visible)
}

pub(super) fn bounds<R: Runtime>(webview: &Webview<R>) -> tauri::Result<Rect> {
    let placed = super::bounds_from_tauri(webview)?;
    Ok(Rect { y: placed.y - title_bar_height(webview), ..placed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Well under the 250 ms the watch once polled at.
    const POLL_BOUND: Duration = Duration::from_millis(200);

    fn names(paths: &[PathBuf]) -> Vec<String> {
        let mut names: Vec<_> =
            paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn a_sweep_deletes_only_the_webkit_copies_that_are_new() {
        let tmp = tempfile::TempDir::new().unwrap();
        let old = tmp.path().join("WebKitPDFs-old");
        fs::create_dir(&old).unwrap();
        fs::write(old.join("a.pdf"), b"old").unwrap();
        fs::create_dir(tmp.path().join("other-app")).unwrap();
        let mut known = existing_copies(tmp.path());
        assert_eq!(known.len(), 1);

        let new = tmp.path().join("WebKitPDFs-new");
        fs::create_dir(&new).unwrap();
        fs::write(new.join("document.pdf"), b"secret").unwrap();
        let found = sweep(tmp.path(), &mut known);

        assert_eq!(names(&found), ["WebKitPDFs-new"]);
        assert!(!new.exists());
        assert!(old.join("a.pdf").exists(), "a copy that was there before is not ours");
        assert!(tmp.path().join("other-app").exists());
        // Seen once: a second sweep finds nothing, even if the name comes back.
        assert!(sweep(tmp.path(), &mut known).is_empty());
    }

    #[test]
    fn keys_are_told_apart() {
        assert_eq!(classify_key(53), Key::Escape);
        assert_eq!(classify_key(97), Key::F6);
        assert_eq!(classify_key(0), Key::Other);
    }

    type Caught = (Vec<PathBuf>, bool);

    /// A watch on `folder` that reports to a channel. Its last call (no paths,
    /// not announced) is the thread's end.
    fn watch(folder: &Path) -> (Watch, mpsc::Receiver<Caught>) {
        let (tx, rx) = mpsc::channel::<Caught>();
        let tx = Mutex::new(tx);
        let watch = Watch::start(
            folder.to_owned(),
            Box::new(move |paths, announce| {
                let _ = tx.lock().unwrap().send((paths.to_vec(), announce));
            }),
        )
        .unwrap();
        (watch, rx)
    }

    #[test]
    fn the_watch_deletes_a_copy_as_it_is_made_and_sweeps_once_more_when_it_is_stopped() {
        let tmp = tempfile::TempDir::new().unwrap();
        let before = tmp.path().join("WebKitPDFs-before");
        fs::create_dir(&before).unwrap();
        let (watch, rx) = watch(tmp.path());

        let copy = tmp.path().join("WebKitPDFs-copy");
        fs::create_dir(&copy).unwrap();
        let (paths, announce) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(names(&paths), ["WebKitPDFs-copy"]);
        assert!(announce);
        assert!(!copy.exists());
        assert!(before.exists());

        // Stopped (the surface closes): one more call, nothing caught, no
        // announcement, at once (no poll to wait for).
        let stopped = Instant::now();
        watch.stop();
        let (paths, announce) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(paths.is_empty());
        assert!(!announce);
        assert!(stopped.elapsed() < POLL_BOUND, "the last sweep waits for nothing");
    }

    #[test]
    fn a_copy_that_arrives_as_the_watch_is_stopped_is_deleted_and_not_announced() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (watch, rx) = watch(tmp.path());
        let copy = tmp.path().join("WebKitPDFs-late");
        fs::create_dir(&copy).unwrap();
        watch.stop();
        let mut caught = Vec::new();
        loop {
            let call = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let last = call.0.is_empty() && !call.1;
            caught.push(call);
            if last {
                break;
            }
        }
        assert!(!copy.exists());
        // The thread may have seen the copy before or with the stop.
        assert!(caught.iter().any(|(paths, _)| names(paths) == ["WebKitPDFs-late"]));
    }

    #[test]
    fn a_watch_that_was_stopped_does_not_touch_later_copies() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (watch, rx) = watch(tmp.path());
        watch.stop();
        let (paths, announce) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(paths.is_empty() && !announce);
        let copy = tmp.path().join("WebKitPDFs-after");
        fs::create_dir(&copy).unwrap();
        thread::sleep(Duration::from_millis(300));
        assert!(copy.exists());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn the_rules_block_everything_but_the_surfaces_own_urls() {
        let rules: serde_json::Value = serde_json::from_str(BLOCK_RULES).unwrap();
        let rules = rules.as_array().unwrap();
        assert_eq!(rules[0]["action"]["type"], "block");
        assert!(rules[1..].iter().all(|r| r["action"]["type"] == "ignore-previous-rules"));
    }
}
