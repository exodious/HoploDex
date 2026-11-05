//! What keeps a crash's memory, and reports about it, from leaving the app on
//! Windows (#166). A crashed process's memory can hold the open database's
//! decrypted records and the passphrase, from which the key follows.
//!
//! - **WebView2's own crash reports.** Each of WebView2's processes writes a
//!   Crashpad minidump, with some of its memory, into the web view's data
//!   folder, and by default sends it to Microsoft. Each web view's
//!   environment is built here, with custom crash reporting on, which stops
//!   the sending ([`web_view_environment`]), and the dumps left behind are
//!   deleted with the decrypted document copies ([`sweep_web_view_reports`]).
//! - **Windows Error Reporting for the app's own process.** No error box and
//!   no heap or snapshot in a report ([`quiet_error_reporting`], as the
//!   helpers have had since 007 FR-003), and a panic ends the process at once
//!   rather than abort's fail-fast, which goes to Windows Error Reporting
//!   whatever the error mode is ([`end_process_on_panic`]). A failed
//!   allocation doesn't panic: the global allocator (`allocator.rs`, in the
//!   app and in both helpers, whose job memory caps a hostile file can reach)
//!   ends the process with `TerminateProcess` on a null allocation, so Rust's
//!   default alloc-error handler never gets to `std::process::abort`'s
//!   fail-fast either (#166).

use std::io;
use std::path::{Path, PathBuf};

use crate::services::secure_delete::{secure_delete_dir, secure_delete_file};

/// What Crashpad keeps in a web view data folder, as `EBWebView\Crashpad\`'s
/// entries: the reports' minidumps, the list of reports with their upload
/// attempts, and Microsoft's replies (#166).
const CRASHPAD_REPORTS: &str = "reports";
const CRASHPAD_FILES: [&str; 2] = ["metadata", "watson_metadata"];

/// Securely deletes the crash reports WebView2 left in each of the web view
/// data `folders`, returning what could not be deleted (#166). Nothing makes
/// them on Linux or macOS, where there is nothing to find.
pub fn sweep_web_view_reports(folders: &[PathBuf]) -> Vec<PathBuf> {
    let mut leftovers = Vec::new();
    for folder in folders {
        let crashpad = crashpad_dir(folder);
        leftovers.extend(secure_delete_dir(&crashpad.join(CRASHPAD_REPORTS)));
        for name in CRASHPAD_FILES {
            let path = crashpad.join(name);
            if let Err(err) = secure_delete_file(&path)
                && err.kind() != io::ErrorKind::NotFound
            {
                leftovers.push(path);
            }
        }
    }
    leftovers
}

fn crashpad_dir(folder: &Path) -> PathBuf {
    folder.join("EBWebView").join("Crashpad")
}

/// wry's browser arguments, which it adds only when an app gives none (wry
/// 0.55's `webview2/mod.rs`), and which a web view whose environment is built
/// here doesn't get from wry at all, so they are written out here.
#[cfg(windows)]
pub const WRY_DEFAULT_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

/// The main window's browser arguments: what wry gave it when it built the
/// environment itself (its defaults and, since Tauri leaves autoplay on, the
/// autoplay policy).
#[cfg(windows)]
pub fn main_browser_args() -> String {
    format!("{WRY_DEFAULT_ARGS} --autoplay-policy=no-user-gesture-required")
}

/// A WebView2 environment for the web view data folder `data_directory`, as
/// wry builds one (the browser arguments `browser_args` and the user's UI
/// language), with custom crash reporting on: WebView2 then sends no crash
/// data to Microsoft, and only writes its dumps locally, where
/// [`sweep_web_view_reports`] deletes them (#166). wry has no setting for it,
/// so the environment is handed to the web view with Tauri's
/// `with_environment`. Call it on the main thread, which the web view is
/// built on and which this pumps messages on while it waits.
#[cfg(windows)]
pub fn web_view_environment(
    data_directory: &Path,
    browser_args: &str,
) -> Result<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment, String> {
    use std::sync::mpsc;

    use ::windows::Win32::Foundation::E_POINTER;
    use ::windows::core::{HSTRING, PCWSTR};
    use webview2_com::{
        CoreWebView2EnvironmentOptions, CreateCoreWebView2EnvironmentCompletedHandler,
        Microsoft::Web::WebView2::Win32::{
            CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2EnvironmentOptions,
        },
    };

    // As wry does before it builds one: setup() runs before any window has
    // initialized COM on the main thread. Already initialized is fine.
    // SAFETY: a plain system call with no pointer.
    let _ = unsafe {
        windows_sys::Win32::System::Com::CoInitializeEx(
            std::ptr::null(),
            windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED as u32,
        )
    };
    let options = CoreWebView2EnvironmentOptions::default();
    // SAFETY: setters of an options object only this function holds.
    unsafe {
        options.set_additional_browser_arguments(browser_args.to_owned());
        options.set_language(user_ui_language());
        options.set_is_custom_crash_reporting_enabled(true);
    }
    let (tx, rx) = mpsc::channel();
    let completed = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
        move |result: ::windows::core::Result<()>, environment| {
            let _ = tx.send(result.and_then(|()| environment.ok_or_else(|| E_POINTER.into())));
            Ok(())
        },
    ));
    // SAFETY: every argument outlives the call; the handler owns what it
    // captures.
    unsafe {
        CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::null(),
            &HSTRING::from(data_directory),
            &ICoreWebView2EnvironmentOptions::from(options),
            &completed,
        )
    }
    .map_err(|err| format!("could not create the web view's environment: {err}"))?;
    webview2_com::wait_with_pump(rx)
        .map_err(|err| format!("could not create the web view's environment: {err:?}"))?
        .map_err(|err| format!("could not create the web view's environment: {err}"))
}

/// The user's UI language as a locale name ("en-US"), which wry gives
/// WebView2 too.
#[cfg(windows)]
fn user_ui_language() -> String {
    use windows_sys::Win32::Globalization::{
        GetUserDefaultUILanguage, LCIDToLocaleName, LOCALE_ALLOW_NEUTRAL_NAMES,
    };
    use windows_sys::Win32::System::SystemServices::LOCALE_NAME_MAX_LENGTH;

    let mut name = [0u16; LOCALE_NAME_MAX_LENGTH as usize];
    // SAFETY: `name` is a local buffer of the length given.
    let written = unsafe {
        LCIDToLocaleName(
            u32::from(GetUserDefaultUILanguage()),
            name.as_mut_ptr(),
            name.len() as i32,
            LOCALE_ALLOW_NEUTRAL_NAMES,
        )
    };
    // `written` counts the terminating NUL; 0 is a failure, and leaves
    // WebView2 to choose.
    String::from_utf16_lossy(&name[..(written.max(1) as usize - 1)])
}

/// No error box, and no heap or snapshot in a Windows Error Reporting report,
/// for this process (007 FR-003 for the helpers; #166 for the app).
/// Excluding the program by name would write the user's registry, so
/// `WerSetFlags` (per process) is used instead; its failure isn't fatal,
/// since `SetErrorMode` has already stopped the report of a fault.
#[cfg(windows)]
pub fn quiet_error_reporting() {
    use windows_sys::Win32::System::Diagnostics::Debug::{
        SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX, SetErrorMode,
    };
    use windows_sys::Win32::System::ErrorReporting::{
        WER_FAULT_REPORTING_DISABLE_SNAPSHOT_CRASH, WER_FAULT_REPORTING_FLAG_NO_HEAP_ON_QUEUE,
        WER_FAULT_REPORTING_FLAG_NOHEAP, WER_FAULT_REPORTING_NO_UI, WerSetFlags,
    };

    // SAFETY: plain system calls with integer arguments.
    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
        let _ = WerSetFlags(
            WER_FAULT_REPORTING_FLAG_NOHEAP
                | WER_FAULT_REPORTING_FLAG_NO_HEAP_ON_QUEUE
                | WER_FAULT_REPORTING_NO_UI
                | WER_FAULT_REPORTING_DISABLE_SNAPSHOT_CRASH,
        );
    }
}

/// The exit code of a process [`end_process_on_panic`] ended: Rust's own for
/// a panic that unwound out of `main`.
pub const PANIC_EXIT_CODE: u32 = 101;

/// After a panic's message (`report`) or none, ends the process with
/// `TerminateProcess`, so that `panic = "abort"` never gets to its fail-fast,
/// which Windows Error Reporting reports to Microsoft whatever the error mode
/// is, and from which Microsoft can ask for a dump of the process (#166).
/// Replaces the panic hook. A failed allocation doesn't panic; the global
/// allocator (`allocator.rs`) ends the process with `TerminateProcess` for it
/// (#166).
#[cfg(windows)]
pub fn end_process_on_panic(report: bool) {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if report {
            previous(info);
        }
        // SAFETY: ends this process; the pseudo-handle needs no closing.
        unsafe { TerminateProcess(GetCurrentProcess(), PANIC_EXIT_CODE) };
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sweep_deletes_crashpad_reports_in_every_web_view_folder() {
        let cache = tempfile::TempDir::new().unwrap();
        let folders =
            vec![cache.path().join("main-webview"), cache.path().join("preview-webview2")];
        for folder in &folders {
            let crashpad = crashpad_dir(folder);
            std::fs::create_dir_all(crashpad.join("reports")).unwrap();
            std::fs::write(crashpad.join("reports").join("a.dmp"), b"memory").unwrap();
            for name in CRASHPAD_FILES {
                std::fs::write(crashpad.join(name), b"report list").unwrap();
            }
            std::fs::write(crashpad.join("settings.dat"), b"settings").unwrap();
        }

        assert!(sweep_web_view_reports(&folders).is_empty());

        for folder in &folders {
            let crashpad = crashpad_dir(folder);
            assert!(!crashpad.join("reports").exists());
            for name in CRASHPAD_FILES {
                assert!(!crashpad.join(name).exists(), "{name} is still there");
            }
            // Crashpad's own settings hold no report, and stay.
            assert!(crashpad.join("settings.dat").exists());
        }
    }

    #[test]
    fn the_sweep_has_nothing_to_do_where_webview2_left_nothing() {
        let cache = tempfile::TempDir::new().unwrap();
        assert!(sweep_web_view_reports(&[cache.path().join("main-webview")]).is_empty());
    }

    /// Set by the parent of [`a_panic_ends_the_process_without_aborting`].
    #[cfg(windows)]
    const PANIC_CHILD: &str = "HOPLODEX_PANIC_HOOK_CHILD";

    /// A panic ends the process at once (#166). The child panics on a thread
    /// of its own, which without the hook would end only that thread and let
    /// the child exit 0.
    #[cfg(windows)]
    #[test]
    fn a_panic_ends_the_process_without_aborting() {
        if std::env::var_os(PANIC_CHILD).is_some() {
            end_process_on_panic(false);
            // Left alone, a panic on a thread of its own just ends that
            // thread, and the test goes on to exit 0 below.
            let _ = std::thread::spawn(|| panic!("the child's panic")).join();
            std::process::exit(0);
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "services::crash_reports::tests::a_panic_ends_the_process_without_aborting",
            ])
            .env(PANIC_CHILD, "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(PANIC_EXIT_CODE as i32));
    }

    /// The main window's environment is built in `setup()`, on a thread
    /// that nothing has initialized COM on yet (#166).
    #[cfg(windows)]
    #[test]
    fn an_environment_is_built_on_a_thread_without_com() {
        let folder = tempfile::TempDir::new().unwrap();
        let path = folder.path().to_owned();
        let built = std::thread::spawn(move || {
            web_view_environment(&path, &main_browser_args()).map(|_| ())
        })
        .join()
        .unwrap();
        assert_eq!(built, Ok(()));
    }

    #[cfg(windows)]
    #[test]
    fn error_reporting_gets_no_box_and_no_heap() {
        use windows_sys::Win32::System::Diagnostics::Debug::{
            GetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX,
        };
        use windows_sys::Win32::System::ErrorReporting::{
            WER_FAULT_REPORTING_FLAG_NOHEAP, WerGetFlags,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;

        quiet_error_reporting();

        let wanted = SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX;
        // SAFETY: plain system calls; `flags` is a local.
        let (mode, flags) = unsafe {
            let mut flags = 0;
            assert_eq!(WerGetFlags(GetCurrentProcess(), &mut flags), 0);
            (GetErrorMode(), flags)
        };
        assert_eq!(mode & wanted, wanted);
        assert_ne!(flags & WER_FAULT_REPORTING_FLAG_NOHEAP, 0);
    }
}
