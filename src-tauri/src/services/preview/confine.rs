//! The render helper's confinement: Landlock on Linux, a job object and
//! mitigations on Windows, the sandbox profile on macOS (research.md §11).
//!
//! [`confine`] is called by the helper before it reads a byte of any document
//! (`helper::run`). Only the Linux branch is written (T052); the macOS and
//! Windows branches (T055, T056) do what they can without their OS's
//! sandbox until then.

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
    use super::Confinement;

    pub fn log_support() {}

    /// The job object and the mitigation policies are T056.
    pub fn confine() -> Result<Confinement, String> {
        Ok(Confinement::default())
    }
}
