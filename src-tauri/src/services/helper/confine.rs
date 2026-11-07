//! A confined helper's confinement (the TIFF render helper's and the import
//! helper's, 008 research.md §8): a seccomp filter and Landlock on Linux, an
//! AppContainer and a job object (both made by HoploDex, the job checked here)
//! and mitigations on Windows, the sandbox profile on macOS (007 research.md §11).
//!
//! [`confine`] is called by the helper before it reads a byte of any document
//! (`preview::helper::run`). Each branch also makes the helper die with HoploDex at
//! once, by whatever its OS offers: `PR_SET_PDEATHSIG` on Linux, a job object
//! that HoploDex itself owns on Windows (the kernel ends the helper when
//! HoploDex's handle to it closes, however HoploDex ends), and a `kqueue`
//! `NOTE_EXIT` watch on the parent on macOS (007 research.md §11, amended 2026-10-07).

/// What [`confine`] managed, for the helper's self-check and the log.
#[derive(Debug, Clone, Copy, Default)]
pub struct Confinement {
    /// Linux: the Landlock ABI in force (the kernel's, even when newer than
    /// this build knows), `None` when Landlock isn't available.
    pub landlock_abi: Option<u32>,
    /// Linux: some Landlock rule is in force.
    pub landlock_enforced: bool,
}

/// Confines this process, which is a helper. `memory_limit` is the most it may
/// allocate, in bytes: `RLIMIT_DATA` on Linux and the most the job object
/// HoploDex made for it may have on Windows (`job::spawn` is given the same
/// number); macOS has no enforced limit. A measure that can
/// fail without making the helper unsafe to run is best effort (Landlock on
/// an older kernel); one that can't (no new privileges, the core dump limit,
/// the seccomp filter) is an error, and the helper exits without reading a
/// document.
pub fn confine(memory_limit: usize) -> Result<Confinement, String> {
    imp::confine(memory_limit)
}

/// Logs once per run, from the app, what this computer's kernel lets the
/// helper be confined with, since the helper's own stderr is discarded
/// (007 research.md §11: "best effort on older kernels, logged once").
pub fn log_support_once() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(imp::log_support);
}

/// The system calls the filter refuses outright: the self-check tries each.
#[cfg(all(target_os = "linux", debug_assertions))]
pub fn refused_calls() -> impl Iterator<Item = libc::c_long> {
    imp::REFUSED.iter().chain(imp::REFUSED_ON_THIS_ARCH).copied()
}

#[cfg(target_os = "linux")]
mod imp {
    use landlock::{
        ABI, Access, AccessFs, AccessNet, CompatLevel, Compatible, LandlockStatus, Ruleset,
        RulesetAttr, RulesetStatus, Scope,
    };

    use super::Confinement;

    /// The architecture a system call must come from for the filter's numbers
    /// to mean what they say.
    #[cfg(target_arch = "x86_64")]
    const AUDIT_ARCH: u32 = 0xC000_003E; // AUDIT_ARCH_X86_64
    #[cfg(target_arch = "aarch64")]
    const AUDIT_ARCH: u32 = 0xC000_00B7; // AUDIT_ARCH_AARCH64
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    compile_error!("the helpers' seccomp filter needs this architecture's guard and call numbers");

    /// x32's system calls are x86_64's numbers with this bit set.
    const X32_SYSCALL_BIT: u32 = 0x4000_0000;

    // Newer than the `libc` crate's lists; every architecture since 5.1 shares
    // these numbers.
    const SYS_FCHMODAT2: libc::c_long = 452;
    const SYS_STATMOUNT: libc::c_long = 457;
    const SYS_LISTMOUNT: libc::c_long = 458;
    const SYS_SETXATTRAT: libc::c_long = 463;
    const SYS_GETXATTRAT: libc::c_long = 464;
    const SYS_LISTXATTRAT: libc::c_long = 465;
    const SYS_REMOVEXATTRAT: libc::c_long = 466;
    const SYS_OPEN_TREE_ATTR: libc::c_long = 467;
    const SYS_FILE_GETATTR: libc::c_long = 468;
    const SYS_FILE_SETATTR: libc::c_long = 469;
    const SYS_LISTNS: libc::c_long = 470;
    /// `fcntl`'s other way to set a file's owner, on both architectures.
    const F_SETOWN_EX: u32 = 15;

    /// What a helper never needs, on every architecture: it reads its pipe,
    /// computes and writes its pipe. A call that isn't listed is allowed, so a
    /// C library's choice of calls can't end a helper (007 research.md §11).
    pub const REFUSED: &[libc::c_long] = &[
        // Sockets.
        libc::SYS_socket,
        libc::SYS_socketpair,
        // Other processes and their memory, and starting a program.
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_process_madvise,
        libc::SYS_process_mrelease,
        libc::SYS_pidfd_open,
        libc::SYS_pidfd_getfd,
        libc::SYS_pidfd_send_signal,
        libc::SYS_kcmp,
        libc::SYS_tkill,
        libc::SYS_rt_sigqueueinfo,
        libc::SYS_rt_tgsigqueueinfo,
        libc::SYS_migrate_pages,
        libc::SYS_move_pages,
        libc::SYS_sched_setaffinity,
        libc::SYS_sched_setparam,
        libc::SYS_sched_setscheduler,
        libc::SYS_sched_setattr,
        libc::SYS_setpriority,
        libc::SYS_ioprio_set,
        libc::SYS_execve,
        libc::SYS_execveat,
        // Namespaces and mounts.
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_pivot_root,
        libc::SYS_chroot,
        libc::SYS_open_tree,
        SYS_OPEN_TREE_ATTR,
        libc::SYS_move_mount,
        libc::SYS_fsopen,
        libc::SYS_fspick,
        libc::SYS_fsconfig,
        libc::SYS_fsmount,
        libc::SYS_mount_setattr,
        SYS_STATMOUNT,
        SYS_LISTMOUNT,
        SYS_LISTNS,
        // Kernel interfaces, the kernel keyring and shared memory.
        libc::SYS_io_uring_setup,
        libc::SYS_io_uring_enter,
        libc::SYS_io_uring_register,
        libc::SYS_bpf,
        libc::SYS_perf_event_open,
        libc::SYS_userfaultfd,
        libc::SYS_syslog,
        libc::SYS_fanotify_init,
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
        libc::SYS_shmget,
        libc::SYS_shmat,
        libc::SYS_shmctl,
        libc::SYS_msgget,
        libc::SYS_msgsnd,
        libc::SYS_msgrcv,
        libc::SYS_msgctl,
        libc::SYS_semget,
        libc::SYS_semop,
        libc::SYS_semtimedop,
        libc::SYS_semctl,
        libc::SYS_mq_open,
        libc::SYS_mq_unlink,
        libc::SYS_memfd_create,
        libc::SYS_memfd_secret,
        // Changing a file's metadata.
        libc::SYS_fchmod,
        libc::SYS_fchmodat,
        SYS_FCHMODAT2,
        libc::SYS_fchown,
        libc::SYS_fchownat,
        libc::SYS_utimensat,
        libc::SYS_setxattr,
        libc::SYS_lsetxattr,
        libc::SYS_fsetxattr,
        SYS_SETXATTRAT,
        libc::SYS_removexattr,
        libc::SYS_lremovexattr,
        libc::SYS_fremovexattr,
        SYS_REMOVEXATTRAT,
        SYS_FILE_SETATTR,
        libc::SYS_truncate,
        libc::SYS_ftruncate,
        libc::SYS_mknodat,
        // Opening a file, and every lookup of a path.
        libc::SYS_openat,
        libc::SYS_openat2,
        libc::SYS_newfstatat,
        libc::SYS_statx,
        libc::SYS_faccessat,
        libc::SYS_faccessat2,
        libc::SYS_readlinkat,
        libc::SYS_getdents64,
        libc::SYS_name_to_handle_at,
        libc::SYS_open_by_handle_at,
        libc::SYS_statfs,
        libc::SYS_getxattr,
        libc::SYS_lgetxattr,
        SYS_GETXATTRAT,
        libc::SYS_listxattr,
        libc::SYS_llistxattr,
        SYS_LISTXATTRAT,
        SYS_FILE_GETATTR,
        libc::SYS_linkat,
        libc::SYS_symlinkat,
        libc::SYS_unlinkat,
        libc::SYS_renameat2,
        libc::SYS_mkdirat,
        libc::SYS_chdir,
        libc::SYS_fchdir,
        libc::SYS_inotify_add_watch,
        libc::SYS_fanotify_mark,
        libc::SYS_quotactl,
        libc::SYS_acct,
    ];

    /// The same kinds of call under the names only this architecture has.
    #[cfg(target_arch = "x86_64")]
    pub const REFUSED_ON_THIS_ARCH: &[libc::c_long] = &[
        libc::SYS_fork,
        libc::SYS_vfork,
        libc::SYS_chmod,
        libc::SYS_chown,
        libc::SYS_lchown,
        libc::SYS_utime,
        libc::SYS_utimes,
        libc::SYS_futimesat,
        libc::SYS_mknod,
        libc::SYS_open,
        libc::SYS_creat,
        libc::SYS_stat,
        libc::SYS_lstat,
        libc::SYS_access,
        libc::SYS_readlink,
        libc::SYS_getdents,
        libc::SYS_link,
        libc::SYS_symlink,
        libc::SYS_unlink,
        libc::SYS_rename,
        libc::SYS_renameat,
        libc::SYS_mkdir,
        libc::SYS_rmdir,
        libc::SYS_uselib,
        libc::SYS_sysfs,
        libc::SYS_ustat,
    ];
    /// `renameat`, which aarch64 has and the `libc` crate doesn't name there.
    #[cfg(target_arch = "aarch64")]
    pub const REFUSED_ON_THIS_ARCH: &[libc::c_long] = &[38];

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
            n if n >= 1 => {
                log::info!("the helpers are confined with Landlock ABI {n} and a seccomp filter")
            }
            _ => log::info!(
                "this kernel has no Landlock, so the helpers are confined with a seccomp \
                 filter alone"
            ),
        }
    }

    fn last_error(what: &str) -> String {
        format!("{what}: {}", std::io::Error::last_os_error())
    }

    pub fn confine(memory_limit: usize) -> Result<Confinement, String> {
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
            // RLIMIT_DATA: heap and writable mappings.
            let limit = memory_limit as libc::rlim_t;
            let data = libc::rlimit { rlim_cur: limit, rlim_max: limit };
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
        let confinement = landlock();
        refuse_system_calls().map_err(|e| format!("the seccomp filter: {e}"))?;
        Ok(confinement)
    }

    fn statement(code: u32, k: u32) -> libc::sock_filter {
        libc::sock_filter { code: code as u16, jt: 0, jf: 0, k }
    }

    fn jump(condition: u32, k: u32, jt: u8, jf: u8) -> libc::sock_filter {
        libc::sock_filter { code: (libc::BPF_JMP | condition | libc::BPF_K) as u16, jt, jf, k }
    }

    /// Loads the 32-bit word at `offset` of the kernel's `seccomp_data`.
    fn load(offset: u32) -> libc::sock_filter {
        statement(libc::BPF_LD | libc::BPF_W | libc::BPF_ABS, offset)
    }

    /// The filter, on top of Landlock and whatever the kernel's version of
    /// Landlock leaves open: it allows by default and answers a listed call
    /// with `EPERM`, a normal error, and never kills. It holds for this thread
    /// and the threads it starts, and a helper has no other at this point.
    fn refuse_system_calls() -> Result<(), std::io::Error> {
        // Offsets in `seccomp_data`; an argument's low half on a little-endian
        // machine, and each argument checked here is a 32-bit `int` or flags.
        const NR: u32 = 0;
        const ARCH: u32 = 4;
        const fn argument(n: u32) -> u32 {
            16 + 8 * n
        }

        let ret = |action: u32| statement(libc::BPF_RET | libc::BPF_K, action);
        let allow = ret(libc::SECCOMP_RET_ALLOW);
        let refuse = ret(libc::SECCOMP_RET_ERRNO | libc::EPERM as u32);
        // `nr` passes only with argument `n` equal to `value`.
        let only_with = |nr: libc::c_long, n: u32, value: u32| {
            [
                jump(libc::BPF_JEQ, nr as u32, 0, 4),
                load(argument(n)),
                jump(libc::BPF_JEQ, value, 0, 1),
                allow,
                refuse,
            ]
        };
        // SAFETY: a plain system call.
        let me = unsafe { libc::getpid() } as u32;

        let mut filter = vec![
            // A foreign architecture's numbers mean other calls (the 32-bit
            // `int 0x80` on x86_64), and so do x32's.
            load(ARCH),
            jump(libc::BPF_JEQ, AUDIT_ARCH, 1, 0),
            refuse,
            load(NR),
            jump(libc::BPF_JGE, X32_SYSCALL_BIT, 0, 1),
            refuse,
            // A new thread passes, and a new process doesn't.
            jump(libc::BPF_JEQ, libc::SYS_clone as u32, 0, 4),
            load(argument(0)),
            jump(libc::BPF_JSET, libc::CLONE_THREAD as u32, 0, 1),
            allow,
            refuse,
            // No memory shared with anything: `MAP_SHARED_VALIDATE` has this
            // bit too.
            jump(libc::BPF_JEQ, libc::SYS_mmap as u32, 0, 4),
            load(argument(3)),
            jump(libc::BPF_JSET, libc::MAP_SHARED as u32, 0, 1),
            refuse,
            allow,
            // `clone3`'s flags are behind a pointer the filter can't read.
            // "Not implemented" is what makes the C library fall back to
            // `clone`, and `EPERM` would fail every new thread.
            jump(libc::BPF_JEQ, libc::SYS_clone3 as u32, 0, 1),
            ret(libc::SECCOMP_RET_ERRNO | libc::ENOSYS as u32),
            // A file's owner is who the kernel signals when the file has news,
            // whichever process that is, so no file gets one.
            jump(libc::BPF_JEQ, libc::SYS_fcntl as u32, 0, 5),
            load(argument(1)),
            jump(libc::BPF_JEQ, libc::F_SETOWN as u32, 1, 0),
            jump(libc::BPF_JEQ, F_SETOWN_EX, 0, 1),
            refuse,
            allow,
        ];
        // A signal to this process only (an abort is one), and its own limits
        // only (`getrlimit` is `prlimit64` of pid 0).
        filter.extend(only_with(libc::SYS_kill, 0, me));
        filter.extend(only_with(libc::SYS_tgkill, 0, me));
        filter.extend(only_with(libc::SYS_prlimit64, 0, 0));
        for &call in REFUSED.iter().chain(REFUSED_ON_THIS_ARCH) {
            filter.extend([jump(libc::BPF_JEQ, call as u32, 0, 1), refuse]);
        }
        filter.push(allow);

        let program = libc::sock_fprog { len: filter.len() as u16, filter: filter.as_mut_ptr() };
        // Before Linux 5.16 a filter also turns on CPU speculation mitigations
        // unless this flag says not to, which made reading a file over three
        // times slower. The helper stays as it would be with no filter.
        // SAFETY: the kernel copies the program, which is alive for the call;
        // `PR_SET_NO_NEW_PRIVS` is already set.
        let installed = unsafe {
            libc::syscall(
                libc::SYS_seccomp,
                libc::SECCOMP_SET_MODE_FILTER,
                libc::SECCOMP_FILTER_FLAG_SPEC_ALLOW,
                &raw const program,
            )
        };
        if installed != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
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

    /// The profile, as source (`flags` 0): denies everything, so no file, no
    /// network, no Mach service, no process creation (`fork` and `exec`), no
    /// IPC. Files and pipes already open stay usable, which is how the helper
    /// reads and writes its frames. Not the built-in `pure-computation` it
    /// replaced: macOS 27 kills a process that enters that one.
    const DENY_EVERYTHING: &CStr = c"(version 1)(deny default)";

    // `sandbox_init` is deprecated, and still what a plain command-line
    // process has: it is in libSystem, which every Rust program links.
    unsafe extern "C" {
        fn sandbox_init(profile: *const c_char, flags: u64, errorbuf: *mut *mut c_char) -> c_int;
        fn sandbox_free_error(errorbuf: *mut c_char);
    }

    pub fn log_support() {
        log::info!(
            "the helpers are confined with a sandbox_init profile that denies everything and \
             end with HoploDex"
        );
    }

    fn last_error(what: &str) -> String {
        format!("{what}: {}", std::io::Error::last_os_error())
    }

    /// Order: whatever was inherited closed first, since macOS has no way to
    /// start a process with only its three pipes, then the parent watch, since
    /// it needs `kqueue` and `kevent`
    /// (which the profile would deny a new queue) and so that the helper
    /// never reads a document with no one to end it, then the core limit,
    /// then the profile, which nothing may follow that needs a file, a
    /// process or a service. Every step fails closed: the helper exits
    /// without reading a document.
    pub fn confine(_memory_limit: usize) -> Result<Confinement, String> {
        // SAFETY: plain system calls; nothing in this process holds a
        // descriptor from 3 up yet, so only an inherited one is closed.
        unsafe {
            for descriptor in 3..libc::getdtablesize() {
                libc::close(descriptor);
            }
        }
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
                        if n < 0
                            && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
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

    /// Enters the sandbox: it can't be left.
    fn sandbox() -> Result<(), String> {
        let mut error: *mut c_char = std::ptr::null_mut();
        // SAFETY: a NUL-terminated profile and a pointer to a local; a message
        // `sandbox_init` returns is read once and handed back to
        // `sandbox_free_error`.
        unsafe {
            if sandbox_init(DENY_EVERYTHING.as_ptr(), 0, &mut error) == 0 {
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
            "the helpers are confined with an AppContainer with no capabilities, a job object \
             HoploDex owns (memory-limited, no other process, UI limits, ended when HoploDex \
             is) and process mitigation policies"
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
    pub fn confine(memory_limit: usize) -> Result<Confinement, String> {
        in_the_parents_job(memory_limit)?;
        // No error box, and no heap or snapshot in a report (007 FR-003).
        crate::services::crash_reports::quiet_error_reporting();
        let _ = mitigate(ProcessImageLoadPolicy, NO_REMOTE_OR_LOW_LABEL_IMAGES);
        mitigate(ProcessDynamicCodePolicy, PROHIBIT_DYNAMIC_CODE)?;
        mitigate(ProcessChildProcessPolicy, NO_CHILD_PROCESS_CREATION)?;
        Ok(Confinement::default())
    }

    /// Dies with HoploDex, and is bounded, because HoploDex put this process
    /// in a job object of its own making (`job::spawn`) before it ran
    /// any code. The helper can't make that job itself: one it made and
    /// owned would outlive a HoploDex that was killed. So it checks that it
    /// is in a job, and that the job has the limits of HoploDex's (kill on
    /// close, one process, no more than `memory_limit`), and refuses to serve when it isn't: a
    /// helper started any other way, by hand or by a bug, is not confined.
    /// Jobs nest, so a job the whole of HoploDex was started in (a test
    /// runner's) doesn't matter: the one asked about is this process's own.
    fn in_the_parents_job(memory_limit: usize) -> Result<(), String> {
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
                || limits.JobMemoryLimit > memory_limit
            {
                return Err("the job object is not HoploDex's".into());
            }
        }
        Ok(())
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
