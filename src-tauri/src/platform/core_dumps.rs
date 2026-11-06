//! Keeps a crashed process of the app from leaving its memory on disk: a
//! core of the app or of a web process holds the open database's decrypted
//! records and the passphrase (#166).
//!
//! - [`turn_off_core_dumps`]: core limit 0, soft and hard, for the app and
//!   every process it starts (Linux and macOS), and on Linux a core dump filter
//!   of 0, which keeps memory out of a core that apport takes anyway.
//! - [`exit_on_fatal_signals`]: a crash ends the app at once, so no core
//!   handler gets a core of it, whatever the limit (Linux). apport, Ubuntu's
//!   handler on 24.04 and 26.04, reads the core whatever the limit says.
//! - [`turn_off_web_process_core_dumps`]: inside the Flatpak, the core limit
//!   of the web process the Flatpak portal starts for WebKit, which doesn't
//!   inherit the app's (Linux).

/// Turns core dumps off for this process and every process it starts, which
/// inherit the limit: on Linux WebKit's web and network processes and the
/// helpers (`services/helper/confine.rs` lowers theirs too). The hard limit
/// too: with only the soft one lowered, a compromised helper could raise the
/// app's again with `prlimit`. On macOS the soft limit is already 0 for an app
/// started from the Finder or the Dock, but a shell that raised `ulimit -c`
/// passes its limit on, and a core is then written once an administrator makes
/// `/cores` writable; WebKit's processes there are launchd's XPC services,
/// which this doesn't reach. Inside the Flatpak, WebKit's sandboxed web process
/// is started by the Flatpak portal, not by the app, so it doesn't inherit
/// this: [`turn_off_web_process_core_dumps`] lowers it.
///
/// On Linux it also sets the core dump filter to 0, which the kernel applies
/// whichever handler takes the core, and which a process inherits across
/// `fork` and `exec`. apport reads a core whatever the limit says, and WebKit's
/// binaries can't go on its ignore list, since every WebKitGTK app shares them.
/// So on Ubuntu 24.04, where the web process runs outside WebKit's sandbox, its
/// crash report holds registers and the names of mapped files, not its memory
/// (#166).
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn turn_off_core_dumps() {
    let core = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: a plain system call with a pointer to a local.
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &core) } != 0 {
        log::error!("core dumps could not be turned off: {}", std::io::Error::last_os_error());
    }
    #[cfg(target_os = "linux")]
    if let Err(err) = std::fs::write("/proc/self/coredump_filter", "0") {
        log::error!("the core dump filter could not be set to 0: {err}");
    }
}

/// The signals whose default action dumps core and that a crash raises: a
/// bad memory access, an illegal instruction, an arithmetic fault, an abort
/// (glibc's heap checks, a Rust panic in a release build, `panic = "abort"`)
/// and a trap (GLib's `g_error`).
#[cfg(target_os = "linux")]
pub const FATAL_SIGNALS: [libc::c_int; 6] =
    [libc::SIGSEGV, libc::SIGBUS, libc::SIGILL, libc::SIGFPE, libc::SIGABRT, libc::SIGTRAP];

/// Makes each of [`FATAL_SIGNALS`] end this process at once, with exit status
/// 128 plus the signal and a line on standard error, instead of the default
/// action, which dumps core. A process a handler ends dumps no core, so
/// neither the kernel nor a core handler (apport, systemd-coredump) gets this
/// process's memory, whatever the core limit. Called first in `main()`, so the
/// helpers and the sandbox probe, which are this executable started again,
/// are covered too. It replaces Rust's own handler for a stack overflow, so
/// an overflow ends the app with the SIGSEGV line rather than Rust's message.
/// Two crashes still get the default action, as the kernel gives a fault no
/// handler can run: one in a thread that blocks the signal (GLib starts its
/// worker threads with every signal blocked), and a stack overflow in a thread
/// without an alternate signal stack (one a C library started). The core
/// limit of 0, and on Ubuntu the `.deb`'s apport ignore entry, still cover
/// those.
#[cfg(target_os = "linux")]
pub fn exit_on_fatal_signals() {
    for signal in FATAL_SIGNALS {
        // SAFETY: `action` is a local the kernel only reads; the handler is
        // an `extern "C"` function that calls only async-signal-safe
        // functions (`write`, `_exit`).
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = end_at_once as extern "C" fn(libc::c_int) as libc::sighandler_t;
            // On Rust's alternate stack where the thread has one, so a stack
            // overflow can still run it; every other signal waits meanwhile.
            action.sa_flags = libc::SA_ONSTACK;
            libc::sigfillset(&mut action.sa_mask);
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                let err = std::io::Error::last_os_error();
                // Before the log exists; standard error is all there is.
                eprintln!("could not handle signal {signal}, so a crash can dump core: {err}");
            }
        }
    }
}

/// The handler [`exit_on_fatal_signals`] installs: one line, then `_exit`,
/// which runs no exit handlers and flushes nothing, so nothing else runs in
/// a process that is already broken.
#[cfg(target_os = "linux")]
extern "C" fn end_at_once(signal: libc::c_int) {
    let line: &[u8] = match signal {
        libc::SIGSEGV => b"hoplodex: SIGSEGV, ending without a core dump (#166)\n",
        libc::SIGBUS => b"hoplodex: SIGBUS, ending without a core dump (#166)\n",
        libc::SIGILL => b"hoplodex: SIGILL, ending without a core dump (#166)\n",
        libc::SIGFPE => b"hoplodex: SIGFPE, ending without a core dump (#166)\n",
        libc::SIGABRT => b"hoplodex: SIGABRT, ending without a core dump (#166)\n",
        libc::SIGTRAP => b"hoplodex: SIGTRAP, ending without a core dump (#166)\n",
        _ => b"hoplodex: a fatal signal, ending without a core dump (#166)\n",
    };
    // SAFETY: `write` and `_exit` are async-signal-safe; `line` is static.
    unsafe {
        libc::write(libc::STDERR_FILENO, line.as_ptr().cast(), line.len());
        libc::_exit(128 + signal);
    }
}

/// The name the kernel gives WebKit's web process (`comm`, 15 bytes).
#[cfg(target_os = "linux")]
pub const WEB_PROCESS_NAME: &str = "WebKitWebProces";

/// The processes under `proc_dir` (`/proc`) whose name is WebKit's web
/// process's. Inside the Flatpak, `/proc` shows only the sandbox's own
/// processes and those the portal started for it, which WebKit asks it to
/// make visible (`flatpak-spawn --expose-pids`).
#[cfg(target_os = "linux")]
pub fn web_process_pids(proc_dir: &std::path::Path) -> Vec<libc::pid_t> {
    let Ok(entries) = std::fs::read_dir(proc_dir) else { return Vec::new() };
    let mut pids: Vec<libc::pid_t> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .filter(|pid: &libc::pid_t| {
            std::fs::read_to_string(proc_dir.join(pid.to_string()).join("comm"))
                .is_ok_and(|name| name.trim_end_matches('\n') == WEB_PROCESS_NAME)
        })
        .collect();
    pids.sort_unstable();
    pids
}

/// Sets the core limit of process `pid` to 0, soft and hard. Lowering
/// another process's limit needs only the same user.
#[cfg(target_os = "linux")]
pub fn turn_off_core_dumps_of(pid: libc::pid_t) -> std::io::Result<()> {
    let core = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: a plain system call with a pointer to a local; the old limit
    // isn't asked for.
    if unsafe { libc::prlimit(pid, libc::RLIMIT_CORE, &core, std::ptr::null_mut()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Inside the Flatpak only: turns core dumps off for each web process the
/// Flatpak portal started for WebKit (`flatpak-spawn --sandbox`), which runs
/// with the portal's core limit, not the app's, and on a systemd desktop that
/// is unlimited, so its crash would store a core in systemd-coredump's store.
/// `main.rs` calls it at each page load of either web view, so a web process
/// WebKit starts again after one ended (`web-process-terminated`) is lowered
/// at its first load. Outside the Flatpak WebKit's processes are the app's
/// children and already inherit [`turn_off_core_dumps`], and `/proc` shows
/// every other program's processes too, so it does nothing there.
#[cfg(target_os = "linux")]
pub fn turn_off_web_process_core_dumps(in_flatpak: bool) {
    if !in_flatpak {
        return;
    }
    for pid in web_process_pids(std::path::Path::new("/proc")) {
        if let Err(err) = turn_off_core_dumps_of(pid) {
            log::warn!("could not turn core dumps off for web process {pid}: {err}");
        }
    }
}
