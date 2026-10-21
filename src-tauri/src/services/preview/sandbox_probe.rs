//! Whether WebKitGTK's sandbox works on this Linux, probed at startup
//! (007 research.md §9).
//!
//! WebKit's web-process sandbox is turned on for the whole app with
//! `WEBKIT_FORCE_SANDBOX=1`, which must be in the environment before the
//! first web context exists. Forcing it where bubblewrap can't run ends the
//! app at its first web view, so before Tauri starts the app runs itself
//! once with [`ARGUMENT`] and the variable set: the child makes a real web
//! view, loads `about:blank` and exits 0 when it has loaded. Only an exit of
//! 0 turns the sandbox on; a crash, another exit status or a time-out means
//! it stays off. A container is not probed at all: WebKit won't sandbox in
//! it. A Flatpak is probed like any other install: WebKit sandboxes there
//! through `flatpak-spawn --sandbox`, so the probe's child shows whether it
//! works (011 research.md §9).

use std::{
    ffi::{CStr, c_int, c_void},
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

/// The application id WebKit names its sandboxed web processes after (Tauri's
/// bundle identifier, which is the Flatpak's app id).
const APPLICATION_ID: &CStr = c"io.github.exodious.HoploDex";

/// Gives the process a default `GApplication` with [`APPLICATION_ID`], never
/// registered or run. WebKit names a sandboxed web process's accessibility bus
/// name after the default application's id, or, with none, after a hash, and
/// the Flatpak portal behind `flatpak-spawn --sandbox` refuses a name that
/// doesn't start with the Flatpak's app id ("Invalid sandbox a11y own name"),
/// which ended the app at its first web view once the probe passed. Tauri's
/// own GTK application (tao's) has no id unless the config sets
/// `enableGTKAppId`, which would also make a second HoploDex on the same
/// session bus hand over to the first and exit (tao), so this stands in for
/// it. Call it before tao creates its application, which then isn't the
/// default (011 research.md §9, 011 FR-011).
///
/// # Safety
/// Call it from `main()` before it starts a thread, or in the probe child.
unsafe fn name_default_application() {
    // SAFETY: GIO's C API; the application lives for the process.
    unsafe {
        let application = webkit2gtk::gio::ffi::g_application_new(APPLICATION_ID.as_ptr(), 0);
        webkit2gtk::gio::ffi::g_application_set_default(application);
    }
}

/// The file a Flatpak puts at the root of every app's sandbox, and nowhere
/// else.
const FLATPAK_INFO: &str = "/.flatpak-info";

/// Whether to name the default application: only inside a Flatpak, which is
/// where the portal checks the a11y bus name. On a `.deb` or `.rpm` install
/// nothing is named, so they behave as before 011 (011 FR-011, 011
/// research.md §9). A pure function of the file's path, so a test can pass
/// one that exists and one that doesn't.
fn names_default_application(flatpak_info: &Path) -> bool {
    flatpak_info.exists()
}

/// [`name_default_application`] when [`names_default_application`] says so
/// for this computer's [`FLATPAK_INFO`]; the one call both the app and the
/// probe child make, so they cannot differ.
///
/// # Safety
/// As [`name_default_application`].
unsafe fn name_default_application_in_flatpak() {
    if names_default_application(Path::new(FLATPAK_INFO)) {
        // SAFETY: the caller upholds `name_default_application`'s contract.
        unsafe { name_default_application() };
    }
}

/// This process's own executable, as the kernel has it.
const SELF_EXE: &str = "/proc/self/exe";

/// How long the probe may take before it counts as having failed.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// The files that say this is a container, where the probe doesn't run.
/// `/.flatpak-info` is not one: a Flatpak runs the probe (011 FR-011).
pub const SKIP_MARKERS: [&str; 2] = ["/run/.containerenv", "/.dockerenv"];

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
                "this is a container ({} exists), where WebKit doesn't sandbox",
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
/// for every web view it makes) and, inside a Flatpak only, names its default
/// application ([`name_default_application_in_flatpak`]). Call it first in `main()`, before any
/// thread and before Tauri.
pub fn decide_and_apply() -> WebKitSandbox {
    let markers: Vec<&Path> = SKIP_MARKERS.iter().map(Path::new).collect();
    let decision = decide(&markers, run_child);
    if matches!(decision, WebKitSandbox::On) {
        // SAFETY: called from `main()` before it starts a thread, so nothing
        // else reads or writes the environment.
        unsafe {
            std::env::set_var("WEBKIT_FORCE_SANDBOX", "1");
            name_default_application_in_flatpak();
        }
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
        // The app names its default application the same way when the probe
        // passes ([`decide_and_apply`]), so the probe must too.
        name_default_application_in_flatpak();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// 011 FR-011, 011 research.md §9: the Flatpak runs the probe, so its
    /// marker is not one that skips it; a container's two still are.
    #[test]
    fn flatpak_marker_no_longer_skips_the_probe() {
        assert!(!SKIP_MARKERS.contains(&"/.flatpak-info"));
        assert!(SKIP_MARKERS.contains(&"/run/.containerenv"));
        assert!(SKIP_MARKERS.contains(&"/.dockerenv"));
    }

    /// 011 FR-011, 011 research.md §9 (2026-10-19): the default application is
    /// named only where `/.flatpak-info` exists.
    #[test]
    fn the_default_application_is_named_only_inside_a_flatpak() {
        let dir = tempfile::tempdir().unwrap();
        let info = dir.path().join(".flatpak-info");
        assert!(!names_default_application(&info));
        std::fs::write(&info, "[Application]\nname=io.github.exodious.HoploDex\n").unwrap();
        assert!(names_default_application(&info));
        assert_eq!(FLATPAK_INFO, "/.flatpak-info");
    }

    #[test]
    fn a_container_marker_skips_the_probe() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join(".containerenv");
        std::fs::write(&marker, "").unwrap();
        let ran = Cell::new(false);
        let decision = decide(&[marker.as_path()], || {
            ran.set(true);
            ProbeOutcome::Exited(0)
        });
        assert!(!ran.get());
        assert!(matches!(decision, WebKitSandbox::Off { .. }));
    }
}
