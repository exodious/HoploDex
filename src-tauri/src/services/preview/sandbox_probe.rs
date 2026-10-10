//! Whether WebKitGTK's sandbox works on this Linux, probed at startup
//! (research.md §9).
//!
//! WebKit's web-process sandbox is turned on for the whole app with
//! `WEBKIT_FORCE_SANDBOX=1`, which must be in the environment before the
//! first web context exists. Forcing it where bubblewrap can't run ends the
//! app at its first web view, so before Tauri starts the app runs itself
//! once with [`ARGUMENT`] and the variable set: the child makes a real web
//! view, loads `about:blank` and exits 0 when it has loaded. Only an exit of
//! 0 turns the sandbox on; a crash, another exit status or a time-out means
//! it stays off. A container or Flatpak is not probed at all: WebKit won't
//! sandbox in the first, and the second has a sandbox of its own.

use std::{
    ffi::{c_int, c_void},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use webkit2gtk::glib::{
    ffi::{g_main_loop_new, g_main_loop_quit, g_main_loop_run},
    gobject_ffi::g_signal_connect_data,
};

/// The argument that makes `main()` run [`run_child`] instead of the app,
/// checked before anything else, as the render helper's is.
pub const ARGUMENT: &str = "--webkit-sandbox-probe";

/// This process's own executable, as the kernel has it.
const SELF_EXE: &str = "/proc/self/exe";

/// How long the probe may take before it counts as having failed.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// The files that say this is a container or Flatpak.
pub const SKIP_MARKERS: [&str; 3] = ["/run/.containerenv", "/.dockerenv", "/.flatpak-info"];

/// What the probe child did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProbeOutcome {
    /// It exited with this status.
    Exited(i32),
    /// A signal ended it (WebKit's `g_error` when bubblewrap can't run).
    Crashed,
    /// It was still running after [`PROBE_TIMEOUT`] and was killed.
    TimedOut,
}

/// Whether WebKit's sandbox is on in this run, and why not when it isn't.
/// Tauri state on Linux.
#[derive(Debug)]
pub enum WebKitSandbox {
    On,
    Off { reason: String },
}

impl WebKitSandbox {
    /// What the one line in the log says.
    pub fn describe(&self) -> String {
        match self {
            Self::On => "WebKit's sandbox is on".to_owned(),
            Self::Off { reason } => format!("WebKit's sandbox is off: {reason}"),
        }
    }
}

/// Decides from the markers and, if none is present, one run of `probe`.
pub fn decide(markers: &[&Path], probe: impl FnOnce() -> ProbeOutcome) -> WebKitSandbox {
    if let Some(found) = markers.iter().find(|marker| marker.exists()) {
        return WebKitSandbox::Off {
            reason: format!(
                "this is a container or Flatpak ({} exists), where WebKit doesn't sandbox",
                found.display()
            ),
        };
    }
    match probe() {
        ProbeOutcome::Exited(0) => WebKitSandbox::On,
        ProbeOutcome::Exited(code) => {
            WebKitSandbox::Off { reason: format!("the probe web view exited with status {code}") }
        }
        ProbeOutcome::Crashed => {
            WebKitSandbox::Off { reason: "the probe web view crashed".to_owned() }
        }
        ProbeOutcome::TimedOut => WebKitSandbox::Off {
            reason: format!("the probe web view timed out after {} s", PROBE_TIMEOUT.as_secs()),
        },
    }
}

/// Runs the probe where the app starts: decides for this computer, and when
/// the sandbox works sets `WEBKIT_FORCE_SANDBOX=1` for this process (and so
/// for every web view it makes). Call it first in `main()`, before any
/// thread and before Tauri.
pub fn decide_and_apply() -> WebKitSandbox {
    let markers: Vec<&Path> = SKIP_MARKERS.iter().map(Path::new).collect();
    let decision = decide(&markers, run_child);
    if matches!(decision, WebKitSandbox::On) {
        // SAFETY: called from `main()` before it starts a thread, so nothing
        // else reads or writes the environment.
        unsafe { std::env::set_var("WEBKIT_FORCE_SANDBOX", "1") };
    }
    decision
}

/// Starts this executable as the probe and waits for it, for at most
/// [`PROBE_TIMEOUT`]. It runs `/proc/self/exe`, the image this process is
/// running, rather than the path it was started from, which may since have
/// been replaced or removed.
fn run_child() -> ProbeOutcome {
    let spawned = Command::new(SELF_EXE)
        .arg(ARGUMENT)
        .env("WEBKIT_FORCE_SANDBOX", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = spawned else {
        return ProbeOutcome::Exited(-1);
    };
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return status.code().map_or(ProbeOutcome::Crashed, ProbeOutcome::Exited);
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return ProbeOutcome::TimedOut;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return ProbeOutcome::Exited(-1),
        }
    }
}

// libgtk-3 is already linked: wry's `gtk` crate links it. GTK is called
// directly because `gtk` is not a dependency here, and its Rust wrapper
// would refuse a web view without its own `init`.
unsafe extern "C" {
    fn gtk_init_check(argc: *mut c_int, argv: *mut *mut *mut i8) -> c_int;
}

/// What the probe child's signal handlers need.
struct Probe {
    main_loop: *mut webkit2gtk::glib::ffi::GMainLoop,
}

/// `load-changed`: finished is the answer, exit 0.
unsafe extern "C" fn on_load_changed(_view: *mut c_void, event: c_int, probe: *mut Probe) {
    // `WEBKIT_LOAD_FINISHED`.
    if event == 3 {
        // SAFETY: the probe lives until the loop returns.
        unsafe { g_main_loop_quit((*probe).main_loop) };
    }
}

/// `load-failed`: the probe failed.
unsafe extern "C" fn on_load_failed(
    _view: *mut c_void,
    _event: c_int,
    _uri: *const i8,
    _error: *mut c_void,
    _probe: *mut Probe,
) -> c_int {
    std::process::exit(1)
}

/// The probe itself, run by `main()` for [`ARGUMENT`]: initializes GTK,
/// creates a web view (and with it a web process, which is where the sandbox
/// is set up or fails), loads `about:blank`, and exits 0 once it has loaded.
/// Never returns. Whatever goes wrong is a non-zero exit, a crash or a
/// time-out, which the parent reads as "no sandbox".
pub fn run_child_probe() -> ! {
    // SAFETY: GTK and WebKit's C API, used on this, the only, thread, in
    // the order they document.
    unsafe {
        if gtk_init_check(std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
            std::process::exit(2);
        }
        let view = webkit2gtk::ffi::webkit_web_view_new();
        let main_loop = g_main_loop_new(std::ptr::null_mut(), 0);
        let probe = Box::into_raw(Box::new(Probe { main_loop }));
        g_signal_connect_data(
            view.cast(),
            c"load-changed".as_ptr(),
            Some(std::mem::transmute::<
                unsafe extern "C" fn(*mut c_void, c_int, *mut Probe),
                unsafe extern "C" fn(),
            >(on_load_changed)),
            probe.cast(),
            None,
            0,
        );
        g_signal_connect_data(
            view.cast(),
            c"load-failed".as_ptr(),
            Some(std::mem::transmute::<
                unsafe extern "C" fn(
                    *mut c_void,
                    c_int,
                    *const i8,
                    *mut c_void,
                    *mut Probe,
                ) -> c_int,
                unsafe extern "C" fn(),
            >(on_load_failed)),
            probe.cast(),
            None,
            0,
        );
        webkit2gtk::ffi::webkit_web_view_load_uri(view.cast(), c"about:blank".as_ptr());
        g_main_loop_run(main_loop);
    }
    std::process::exit(0)
}
