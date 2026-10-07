//! The render helper's confinement: Landlock on Linux, a job object (made by
//! HoploDex, checked here) and mitigations on Windows, the sandbox profile on
//! macOS (research.md §11).
//!
//! [`confine`] is called by the helper before it reads a byte of any document
//! (`helper::run`). Each branch also makes the helper die with HoploDex at
//! once, by whatever its OS offers: `PR_SET_PDEATHSIG` on Linux, a job object
//! that HoploDex itself owns on Windows (the kernel ends the helper when
//! HoploDex's handle to it closes, however HoploDex ends), and a `kqueue`
//! `NOTE_EXIT` watch on the parent on macOS (research.md §11, amended 2026-10-07).

/// What [`confine`] managed, for the helper's self-check and the log.
#[derive(Debug, Clone, Copy, Default)]
pub struct Confinement {
    /// Linux: the Landlock ABI in force (the kernel's, even when newer than
    /// this build knows), `None` when Landlock isn't available.
    pub landlock_abi: Option<u32>,
    /// Linux: some Landlock rule is in force.
    pub landlock_enforced: bool,
}

/// Confines this process, which is the render helper. A measure that can
/// fail without making the helper unsafe to run is best effort (Landlock on
/// an older kernel); one that can't (no new privileges, the core dump limit)
/// is an error, and the helper exits without reading a document.
pub fn confine() -> Result<Confinement, String> {
    imp::confine()
}

/// Logs once per run, from the app, what this computer's kernel lets the
/// helper be confined with, since the helper's own stderr is discarded
/// (research.md §11: "best effort on older kernels, logged once").
pub fn log_support_once() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(imp::log_support);
}

#[cfg(target_os = "linux")]
mod imp {
    use landlock::{
        ABI, Access, AccessFs, AccessNet, CompatLevel, Compatible, LandlockStatus, Ruleset,
        RulesetAttr, RulesetStatus, Scope,
    };

    use super::Confinement;

    /// RLIMIT_DATA: 2 GiB of heap and writable mappings.
    const DATA_LIMIT: libc::rlim_t = 2 * 1024 * 1024 * 1024;

    pub fn log_support() {
        // SAFETY: the version query takes no pointer and no flags but its own.
        let abi = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<libc::c_void>(),
                0 as libc::size_t,
                1 as libc::c_uint, // LANDLOCK_CREATE_RULESET_VERSION
            )
        };
        match abi {
            n if n >= 4 => log::info!("the TIFF helper is confined with Landlock ABI {n}"),
            n if n >= 1 => log::warn!(
                "the TIFF helper's file access is confined with Landlock ABI {n}, but its \
                 network access is not: this kernel is older than 6.7"
            ),
            _ => log::warn!(
                "this kernel has no Landlock, so the TIFF helper is kept from files and the \
                 network by its pipes and limits only"
            ),
        }
    }

    fn last_error(what: &str) -> String {
        format!("{what}: {}", std::io::Error::last_os_error())
    }

    pub fn confine() -> Result<Confinement, String> {
        // SAFETY: plain system calls with integer arguments.
        unsafe {
            // Dies with HoploDex. The parent may already be gone (it was
            // reparented to init before the signal was requested).
            let parent = libc::getppid();
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL as libc::c_ulong, 0, 0, 0) != 0 {
                return Err(last_error("PR_SET_PDEATHSIG"));
            }
            if libc::getppid() != parent || parent == 1 {
                return Err("the parent is gone".into());
            }
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1 as libc::c_ulong, 0, 0, 0) != 0 {
                return Err(last_error("PR_SET_NO_NEW_PRIVS"));
            }
            if libc::prctl(libc::PR_SET_DUMPABLE, 0 as libc::c_ulong, 0, 0, 0) != 0 {
                return Err(last_error("PR_SET_DUMPABLE"));
            }
            let core = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
            if libc::setrlimit(libc::RLIMIT_CORE, &core) != 0 {
                return Err(last_error("RLIMIT_CORE"));
            }
            let data = libc::rlimit { rlim_cur: DATA_LIMIT, rlim_max: DATA_LIMIT };
            if libc::setrlimit(libc::RLIMIT_DATA, &data) != 0 {
                return Err(last_error("RLIMIT_DATA"));
            }
            // Only the three pipes: close whatever else was inherited.
            libc::syscall(
                libc::SYS_close_range,
                3 as libc::c_uint,
                libc::c_uint::MAX,
                0 as libc::c_uint,
            );
        }
        Ok(landlock())
    }

    /// Denies every filesystem right (which blocks `execve` too), and on ABI
    /// 4 and later TCP bind and connect and, from ABI 6, connecting to an
    /// abstract Unix socket. A kernel without Landlock, or with an older ABI,
    /// gets what it can, which is logged once (the helper's stderr is
    /// discarded, so the line is the parent's: see `HelperHandle::spawn`).
    fn landlock() -> Confinement {
        let attempt = || -> Result<_, landlock::RulesetError> {
            Ruleset::default()
                .set_compatibility(CompatLevel::BestEffort)
                .handle_access(AccessFs::from_all(ABI::V9))?
                .handle_access(AccessNet::from_all(ABI::V4))?
                .scope(Scope::from_all(ABI::V6))?
                .create()?
                .restrict_self()
        };
        match attempt() {
            Ok(status) => {
                let (abi, available) = match status.landlock {
                    LandlockStatus::Available { effective_abi, kernel_abi } => (
                        Some(kernel_abi.map_or(effective_abi as u32, |abi| abi.max(0) as u32)),
                        true,
                    ),
                    _ => (None, false),
                };
                Confinement {
                    landlock_abi: abi,
                    landlock_enforced: available && status.ruleset != RulesetStatus::NotEnforced,
                }
            }
            Err(_) => Confinement::default(),
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{CStr, c_char, c_int};

    use super::Confinement;

    /// The helper's exit code when its parent is gone.
    const PARENT_GONE: c_int = 5;

    /// `kSBXProfilePureComputation`: the profile that denies everything but
    /// pure computation: no file, no network, no Mach service, no process
    /// creation (`fork` and `exec`), no IPC. Files and pipes already open
    /// stay usable, which is how the helper reads and writes its frames.
    const PURE_COMPUTATION: &CStr = c"pure-computation";
    /// `SANDBOX_NAMED`: the profile argument is the name of a built-in one.
    const SANDBOX_NAMED: u64 = 0x1;

    // `sandbox_init` is deprecated, and still what a plain command-line
    // process has: it is in libSystem, which every Rust program links.
    unsafe extern "C" {
        fn sandbox_init(profile: *const c_char, flags: u64, errorbuf: *mut *mut c_char) -> c_int;
        fn sandbox_free_error(errorbuf: *mut c_char);
    }

    pub fn log_support() {
        log::info!(
            "the TIFF helper is confined with sandbox_init's pure-computation profile and ends \
             with HoploDex"
        );
    }

    fn last_error(what: &str) -> String {
        format!("{what}: {}", std::io::Error::last_os_error())
    }

    /// Order: the parent watch first, since it needs `kqueue` and `kevent`
    /// (which the profile would deny a new queue) and so that the helper
    /// never reads a document with no one to end it, then the core limit,
    /// then the profile, which nothing may follow that needs a file, a
    /// process or a service. Every step fails closed: the helper exits
    /// without reading a document.
    pub fn confine() -> Result<Confinement, String> {
        die_with_parent()?;
        let core = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        // SAFETY: a plain system call with a pointer to a local.
        if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &core) } != 0 {
            return Err(last_error("RLIMIT_CORE"));
        }
        sandbox()?;
        Ok(Confinement::default())
    }

    /// Dies with HoploDex at once, as `PR_SET_PDEATHSIG` does on Linux and
    /// the job object HoploDex owns does on Windows: macOS has neither, and the
    /// end of stdin is too late when a page is being decoded. A thread
    /// blocks in `kevent` on the parent's exit and calls `_exit`.
    ///
    /// The queue is made and the watch registered here, before the profile,
    /// so the wait after it needs nothing new: a `kevent` on a descriptor the
    /// process holds is not a file, network or service access
    /// (tests/tiff_helper_test.rs kills a parent and times the end). The
    /// parent is checked again after the registration: if it died before
    /// it, the event would never come (the registration fails with `ESRCH`,
    /// or the parent has changed already).
    fn die_with_parent() -> Result<(), String> {
        // SAFETY: plain system calls; the event structures are zeroed and
        // live across the calls that use them, and the queue descriptor is
        // kept open for the life of the process.
        unsafe {
            let parent = libc::getppid();
            if parent <= 1 {
                return Err("the parent is gone".into());
            }
            let queue = libc::kqueue();
            if queue < 0 {
                return Err(last_error("kqueue"));
            }
            let mut watch: libc::kevent = std::mem::zeroed();
            watch.ident = parent as libc::uintptr_t;
            watch.filter = libc::EVFILT_PROC;
            watch.flags = libc::EV_ADD | libc::EV_ONESHOT;
            watch.fflags = libc::NOTE_EXIT;
            if libc::kevent(queue, &watch, 1, std::ptr::null_mut(), 0, std::ptr::null()) != 0 {
                return Err(last_error("kevent (the parent watch)"));
            }
            if libc::getppid() != parent {
                return Err("the parent is gone".into());
            }
            std::thread::Builder::new()
                .name("parent-watch".into())
                .spawn(move || {
                    let mut event: libc::kevent = std::mem::zeroed();
                    loop {
                        let n = libc::kevent(
                            queue,
                            std::ptr::null(),
                            0,
                            &mut event,
                            1,
                            std::ptr::null(),
                        );
                        // An interrupted wait goes on waiting; anything else
                        // is the parent's exit, or a queue that can no longer
                        // tell, which is as unsafe to carry on with.
                        if n < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
                        {
                            continue;
                        }
                        libc::_exit(PARENT_GONE);
                    }
                })
                .map(|_| ())
                .map_err(|e| format!("the parent watch: {e}"))
        }
    }

    /// Enters the pure-computation sandbox: it can't be left.
    fn sandbox() -> Result<(), String> {
        let mut error: *mut c_char = std::ptr::null_mut();
        // SAFETY: a NUL-terminated name and a pointer to a local; a message
        // `sandbox_init` returns is read once and handed back to
        // `sandbox_free_error`.
        unsafe {
            if sandbox_init(PURE_COMPUTATION.as_ptr(), SANDBOX_NAMED, &mut error) == 0 {
                return Ok(());
            }
            let message = if error.is_null() {
                "no message".to_owned()
            } else {
                let text = CStr::from_ptr(error).to_string_lossy().into_owned();
                sandbox_free_error(error);
                text
            };
            Err(format!("sandbox_init: {message}"))
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::mem::size_of;

    use windows_sys::Win32::Foundation::{FALSE, GetLastError};
    use windows_sys::Win32::System::Diagnostics::Debug::{
        SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX, SetErrorMode,
    };
    use windows_sys::Win32::System::ErrorReporting::{
        WER_FAULT_REPORTING_DISABLE_SNAPSHOT_CRASH, WER_FAULT_REPORTING_FLAG_NO_HEAP_ON_QUEUE,
        WER_FAULT_REPORTING_FLAG_NOHEAP, WER_FAULT_REPORTING_NO_UI, WerSetFlags,
    };
    use windows_sys::Win32::System::JobObjects::{
        IsProcessInJob, JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_JOB_MEMORY,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, QueryInformationJobObject,
    };
    use windows_sys::Win32::System::SystemServices::{
        PROCESS_MITIGATION_CHILD_PROCESS_POLICY, PROCESS_MITIGATION_DYNAMIC_CODE_POLICY,
        PROCESS_MITIGATION_IMAGE_LOAD_POLICY,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, ProcessChildProcessPolicy, ProcessDynamicCodePolicy,
        ProcessImageLoadPolicy, SetProcessMitigationPolicy,
    };

    use super::super::helper_job::MEMORY_LIMIT;
    use super::Confinement;

    /// `NoRemoteImages` and `NoLowMandatoryLabelImages` of
    /// `PROCESS_MITIGATION_IMAGE_LOAD_POLICY`'s `Flags`.
    const NO_REMOTE_OR_LOW_LABEL_IMAGES: u32 = 0b11;
    /// `ProhibitDynamicCode` of `PROCESS_MITIGATION_DYNAMIC_CODE_POLICY`'s `Flags`.
    const PROHIBIT_DYNAMIC_CODE: u32 = 0b1;
    /// `NoChildProcessCreation` of `PROCESS_MITIGATION_CHILD_PROCESS_POLICY`'s `Flags`.
    const NO_CHILD_PROCESS_CREATION: u32 = 0b1;

    pub fn log_support() {
        log::info!(
            "the TIFF helper is confined with a job object HoploDex owns (2 GiB, no other \
             process, ended when HoploDex is) and process mitigation policies"
        );
    }

    fn last_error(what: &str) -> String {
        // SAFETY: reads the calling thread's last error.
        format!("{what}: error {}", unsafe { GetLastError() })
    }

    /// Order: the job check first (its limits bound everything later), then
    /// the error mode and WER, then the mitigations, the child-process one
    /// last so nothing an earlier step needed is blocked.
    ///
    /// Everything but the image-load policy fails closed: the helper exits
    /// without reading a document. The image-load policy is the one measure
    /// that can be missing from a Windows that is otherwise fine, so it is
    /// best effort, as Landlock is on an older kernel.
    pub fn confine() -> Result<Confinement, String> {
        in_the_parents_job()?;
        errors();
        let _ = mitigate(ProcessImageLoadPolicy, NO_REMOTE_OR_LOW_LABEL_IMAGES);
        mitigate(ProcessDynamicCodePolicy, PROHIBIT_DYNAMIC_CODE)?;
        mitigate(ProcessChildProcessPolicy, NO_CHILD_PROCESS_CREATION)?;
        Ok(Confinement::default())
    }

    /// Dies with HoploDex, and is bounded, because HoploDex put this process
    /// in a job object of its own making (`helper_job::spawn`) before it ran
    /// any code. The helper can't make that job itself: one it made and
    /// owned would outlive a HoploDex that was killed. So it checks that it
    /// is in a job, and that the job has the limits of HoploDex's (kill on
    /// close, one process, 2 GiB), and refuses to serve when it isn't: a
    /// helper started any other way, by hand or by a bug, is not confined.
    /// Jobs nest, so a job the whole of HoploDex was started in (a test
    /// runner's) doesn't matter: the one asked about is this process's own.
    fn in_the_parents_job() -> Result<(), String> {
        // SAFETY: plain system calls with pointers to locals; the structure
        // is zeroed and `size_of` is its own size.
        unsafe {
            let mut in_job = FALSE;
            if IsProcessInJob(GetCurrentProcess(), std::ptr::null_mut(), &mut in_job) == FALSE {
                return Err(last_error("IsProcessInJob"));
            }
            if in_job == FALSE {
                return Err("not in a job object".into());
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            // A null job is the one the calling process is in.
            if QueryInformationJobObject(
                std::ptr::null_mut(),
                JobObjectExtendedLimitInformation,
                std::ptr::from_mut(&mut limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                std::ptr::null_mut(),
            ) == FALSE
            {
                return Err(last_error("QueryInformationJobObject"));
            }
            let wanted = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
                | JOB_OBJECT_LIMIT_JOB_MEMORY;
            let basic = &limits.BasicLimitInformation;
            if basic.LimitFlags & wanted != wanted
                || basic.ActiveProcessLimit != 1
                || limits.JobMemoryLimit == 0
                || limits.JobMemoryLimit > MEMORY_LIMIT
            {
                return Err("the job object is not HoploDex's".into());
            }
        }
        Ok(())
    }

    /// No error box and no Windows Error Reporting heap or snapshot for this
    /// process, so a crash writes no content (FR-003). Excluding the program
    /// by name would write the user's registry, so `WerSetFlags` (per
    /// process) is used instead; its failure isn't fatal, since
    /// `SetErrorMode` has already stopped the dialog.
    fn errors() {
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

    /// One mitigation policy, whose `Flags` word is `flags`, on this process.
    fn mitigate(policy: i32, flags: u32) -> Result<(), String> {
        // The three policies set here are each one `Flags` word.
        const _: () = {
            assert!(size_of::<PROCESS_MITIGATION_CHILD_PROCESS_POLICY>() == 4);
            assert!(size_of::<PROCESS_MITIGATION_DYNAMIC_CODE_POLICY>() == 4);
            assert!(size_of::<PROCESS_MITIGATION_IMAGE_LOAD_POLICY>() == 4);
        };
        // SAFETY: a pointer to a local `u32`, which is the policies' layout.
        let set = unsafe {
            SetProcessMitigationPolicy(policy, std::ptr::from_ref(&flags).cast(), size_of::<u32>())
        };
        if set == FALSE {
            return Err(last_error(&format!("SetProcessMitigationPolicy({policy})")));
        }
        Ok(())
    }
}
