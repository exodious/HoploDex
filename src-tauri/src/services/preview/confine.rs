//! The render helper's confinement: Landlock on Linux, a job object and
//! mitigations on Windows, the sandbox profile on macOS (research.md §11).
//!
//! [`confine`] is called by the helper before it reads a byte of any document
//! (`helper::run`). The Linux and Windows branches are written (T052, T056);
//! the macOS branch (T055) does what it can without its OS's sandbox until
//! then.

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

#[cfg(all(unix, not(target_os = "linux")))]
mod imp {
    use super::Confinement;

    pub fn log_support() {}

    /// macOS's `sandbox_init` profile is T055. Until then: no core dump.
    pub fn confine() -> Result<Confinement, String> {
        let core = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        // SAFETY: a plain system call with a pointer to a local.
        if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &core) } != 0 {
            return Err(format!("RLIMIT_CORE: {}", std::io::Error::last_os_error()));
        }
        Ok(Confinement::default())
    }
}

#[cfg(windows)]
mod imp {
    use std::mem::size_of;

    use windows_sys::Win32::Foundation::{
        CloseHandle, FALSE, FILETIME, GetLastError, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::{
        SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX, SetErrorMode,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::ErrorReporting::{
        WER_FAULT_REPORTING_DISABLE_SNAPSHOT_CRASH, WER_FAULT_REPORTING_FLAG_NO_HEAP_ON_QUEUE,
        WER_FAULT_REPORTING_FLAG_NOHEAP, WER_FAULT_REPORTING_NO_UI, WerSetFlags,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows_sys::Win32::System::SystemServices::{
        PROCESS_MITIGATION_CHILD_PROCESS_POLICY, PROCESS_MITIGATION_DYNAMIC_CODE_POLICY,
        PROCESS_MITIGATION_IMAGE_LOAD_POLICY,
    };
    use windows_sys::Win32::System::Threading::{
        ExitProcess, GetCurrentProcess, GetCurrentProcessId, GetProcessTimes, INFINITE,
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        ProcessChildProcessPolicy, ProcessDynamicCodePolicy, ProcessImageLoadPolicy,
        SetProcessMitigationPolicy, WaitForSingleObject,
    };

    use super::Confinement;

    /// The job's memory limit: 2 GiB, as Linux's `RLIMIT_DATA` (research.md §11).
    const MEMORY_LIMIT: usize = 2 * 1024 * 1024 * 1024;

    /// The helper's exit code when its parent is gone.
    const PARENT_GONE: u32 = 5;

    /// `NoRemoteImages` and `NoLowMandatoryLabelImages` of
    /// `PROCESS_MITIGATION_IMAGE_LOAD_POLICY`'s `Flags`.
    const NO_REMOTE_OR_LOW_LABEL_IMAGES: u32 = 0b11;
    /// `ProhibitDynamicCode` of `PROCESS_MITIGATION_DYNAMIC_CODE_POLICY`'s `Flags`.
    const PROHIBIT_DYNAMIC_CODE: u32 = 0b1;
    /// `NoChildProcessCreation` of `PROCESS_MITIGATION_CHILD_PROCESS_POLICY`'s `Flags`.
    const NO_CHILD_PROCESS_CREATION: u32 = 0b1;

    pub fn log_support() {
        log::info!(
            "the TIFF helper is confined with a job object (2 GiB, no other process) and \
             process mitigation policies"
        );
    }

    fn last_error(what: &str) -> String {
        // SAFETY: reads the calling thread's last error.
        format!("{what}: error {}", unsafe { GetLastError() })
    }

    /// Order: the job first (its limits bound everything later), then the
    /// error mode and WER, then the parent watch, which needs `OpenProcess`,
    /// then the mitigations, the child-process one last so nothing an earlier
    /// step needed is blocked.
    ///
    /// Everything but the image-load policy fails closed: the helper exits
    /// without reading a document. The image-load policy is the one measure
    /// that can be missing from a Windows that is otherwise fine, so it is
    /// best effort, as Landlock is on an older kernel.
    pub fn confine() -> Result<Confinement, String> {
        job()?;
        errors();
        die_with_parent()?;
        let _ = mitigate(ProcessImageLoadPolicy, NO_REMOTE_OR_LOW_LABEL_IMAGES);
        mitigate(ProcessDynamicCodePolicy, PROHIBIT_DYNAMIC_CODE)?;
        mitigate(ProcessChildProcessPolicy, NO_CHILD_PROCESS_CREATION)?;
        Ok(Confinement::default())
    }

    /// A job object holding only this process: `KILL_ON_JOB_CLOSE`, no other
    /// process (`ACTIVE_PROCESS = 1`, kept beside the child-process policy as
    /// a second line) and 2 GiB of memory. The helper makes and joins it
    /// itself, so the limits are in force before it reads a document, and it
    /// never closes the handle. A job it was started in doesn't matter: jobs
    /// nest.
    fn job() -> Result<(), String> {
        // SAFETY: plain system calls; the structure is zeroed and `size_of`
        // is its own size.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(last_error("CreateJobObjectW"));
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
                | JOB_OBJECT_LIMIT_JOB_MEMORY;
            limits.BasicLimitInformation.ActiveProcessLimit = 1;
            limits.JobMemoryLimit = MEMORY_LIMIT;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == FALSE
            {
                return Err(last_error("SetInformationJobObject"));
            }
            if AssignProcessToJobObject(job, GetCurrentProcess()) == FALSE {
                return Err(last_error("AssignProcessToJobObject"));
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

    /// A process's creation time, in 100 ns units since 1601.
    fn created(process: HANDLE) -> Option<u64> {
        let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        // SAFETY: four pointers to locals.
        let read =
            unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) };
        (read != FALSE)
            .then(|| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }

    /// The process that started this one, if it is still the one that did:
    /// found in a snapshot of the processes, and rejected when it was created
    /// after this one (its identifier has been reused).
    fn parent() -> Option<HANDLE> {
        // SAFETY: plain system calls; the snapshot is closed, and the
        // structure is zeroed and sized.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return None;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            let me = GetCurrentProcessId();
            let mut parent_id = None;
            let mut more = Process32FirstW(snapshot, &mut entry);
            while more != FALSE {
                if entry.th32ProcessID == me {
                    parent_id = Some(entry.th32ParentProcessID);
                    break;
                }
                more = Process32NextW(snapshot, &mut entry);
            }
            CloseHandle(snapshot);
            let parent = OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                FALSE,
                parent_id.filter(|id| *id != 0)?,
            );
            if parent.is_null() {
                return None;
            }
            match (created(parent), created(GetCurrentProcess())) {
                (Some(theirs), Some(ours)) if theirs <= ours => Some(parent),
                _ => {
                    CloseHandle(parent);
                    None
                }
            }
        }
    }

    /// Dies with HoploDex. The job object can't do that here, since it is
    /// the helper's own: it closes only when the helper is gone already. So a
    /// thread waits on the parent and ends the process when it dies, even in
    /// the middle of a page, which the closed pipe alone would let finish.
    fn die_with_parent() -> Result<(), String> {
        let parent = parent().ok_or("the parent is gone")? as usize;
        std::thread::Builder::new()
            .name("parent-watch".into())
            .spawn(move || {
                // SAFETY: the handle stays open for the life of the process.
                unsafe {
                    if WaitForSingleObject(parent as HANDLE, INFINITE) == WAIT_OBJECT_0 {
                        ExitProcess(PARENT_GONE);
                    }
                }
            })
            .map(|_| ())
            .map_err(|e| format!("the parent watch: {e}"))
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
