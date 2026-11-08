//! The render helper's process: its start and its main loop (007 research.md
//! §11).
//!
//! The app starts itself again as `hoplodex --render-helper`. `main()` hands
//! the arguments to [`run`] before it sets up anything else, so the helper
//! has no Tauri, no keyring, no session and no logging. `run` confines the
//! process ([`confine`](crate::services::helper::confine)) and only then reads a frame, serves them
//! over stdin and stdout with [`super::tiff`], and exits when stdin ends.
//! Nothing a document says is written anywhere else: stderr is discarded by
//! the parent and the helper writes nothing to it.

use std::io::{self, BufReader, BufWriter, Write};

use super::helper_protocol::{Request, Response, read_request, write_response};
use super::tiff;
use crate::services::helper::confine;

/// What the app starts its helper with.
pub const ARGUMENT: &str = "--render-helper";

/// The most the TIFF helper may allocate: 2 GiB, as `RLIMIT_DATA` on Linux
/// and the job object's limit on Windows (007 research.md §11).
pub const MEMORY_LIMIT: usize = 2 * 1024 * 1024 * 1024;

/// Runs the helper to its end and exits the process: `args` are the
/// process's arguments after the program name, the first being
/// [`ARGUMENT`]. It never returns to the app's own start-up path.
pub fn run(args: &[String]) -> ! {
    // Nothing a panic says is wanted anywhere, and stderr is discarded. On
    // Windows the panic also ends the process at once, before abort's
    // fail-fast reaches Windows Error Reporting (#166).
    #[cfg(windows)]
    crate::services::crash_reports::end_process_on_panic(false);
    #[cfg(not(windows))]
    std::panic::set_hook(Box::new(|_| {}));
    #[cfg(debug_assertions)]
    if args.get(1).map(String::as_str) == Some("--self-check") {
        std::process::exit(self_check::run(&args[2..]));
    }
    let _ = args;
    if confine::confine(MEMORY_LIMIT).is_err() {
        // Not confined: read no document.
        std::process::exit(3);
    }
    std::process::exit(match serve() {
        Ok(()) => 0,
        Err(_) => 4,
    });
}

/// The main loop: one `Load`, then renders, until the parent closes the pipe.
fn serve() -> io::Result<()> {
    let mut input = BufReader::new(io::stdin().lock());
    let mut output = BufWriter::new(io::stdout().lock());
    let mut document: Option<Vec<u8>> = None;
    while let Some(request) = read_request(&mut input)? {
        let response = match request {
            Request::Load(bytes) => {
                document = None;
                match tiff::load(&bytes) {
                    Ok(pages) => {
                        document = Some(bytes);
                        Response::Loaded { pages }
                    }
                    Err(reason) => Response::Failed { reason },
                }
            }
            Request::Render { page, width_px } => match &document {
                Some(bytes) => match tiff::render_page(bytes, page, width_px) {
                    Ok(pixels) => Response::Page(pixels),
                    Err(_) => Response::PageFailed,
                },
                None => Response::PageFailed,
            },
        };
        write_response(&mut output, &response)?;
        output.flush()?;
    }
    Ok(())
}

/// `hoplodex --render-helper --self-check <port> <file> [<process id>]`: what the helper
/// could do before and after confining itself, as one JSON line. With
/// `--foreign-arch` after them it only tries a call from another architecture,
/// which a kernel without that compatibility mode ends the process for. It is the
/// only way to look inside a confined process from outside
/// (tests/tiff_helper_test.rs), so it is compiled into debug and test builds
/// only: a release binary has no such mode.
#[cfg(debug_assertions)]
mod self_check {
    use std::fs::File;
    use std::net::TcpStream;
    use std::process::Command;

    use serde_json::json;

    use super::{MEMORY_LIMIT, confine};

    /// A program that exists on the OS and exits at once: whether the
    /// confined helper can start it is the probe.
    #[cfg(target_os = "linux")]
    const PROGRAM: &str = "/bin/true";
    #[cfg(target_os = "macos")]
    const PROGRAM: &str = "/usr/bin/true";
    #[cfg(windows)]
    const PROGRAM: &str = r"C:\Windows\System32\whoami.exe";

    pub fn run(args: &[String]) -> i32 {
        let (Some(port), Some(file)) =
            (args.first().and_then(|p| p.parse::<u16>().ok()), args.get(1))
        else {
            return 2;
        };
        let can_open = || File::open(file).is_ok();
        let can_connect = || TcpStream::connect(("127.0.0.1", port)).is_ok();
        let can_start = || Command::new(PROGRAM).status().is_ok();
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        if args.get(2).map(String::as_str) == Some("--foreign-arch") {
            if confine::confine(MEMORY_LIMIT).is_err() {
                return 3;
            }
            println!("{}", json!({ "answer": i386_getpid() }));
            return 0;
        }
        let descriptors_before = open_descriptors();
        let file_before = can_open();
        let tcp_before = can_connect();
        let exec_before = can_start();
        let Ok(confinement) = confine::confine(MEMORY_LIMIT) else { return 3 };
        let descriptors_after = open_descriptors();
        let exec_after = can_start();
        #[cfg(windows)]
        let (process_after, windows_after) =
            (args.get(2).and_then(|id| id.parse().ok()).map(can_touch), Some(windows()));
        #[cfg(not(windows))]
        let (process_after, windows_after): (Option<bool>, Option<u32>) = (None, None);
        #[cfg(unix)]
        let rlimit_core = {
            let mut limit = libc::rlimit { rlim_cur: 1, rlim_max: 1 };
            // SAFETY: a plain system call with a pointer to a local.
            let read = unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut limit) } == 0;
            read.then(|| json!([limit.rlim_cur, limit.rlim_max]))
        };
        #[cfg(not(unix))]
        let rlimit_core: Option<serde_json::Value> = None;
        #[cfg(target_os = "linux")]
        // SAFETY: a plain system call.
        let dumpable = Some(unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) } != 0);
        #[cfg(not(target_os = "linux"))]
        let dumpable: Option<bool> = None;
        #[cfg(target_os = "linux")]
        let filter = filter_report(file);
        #[cfg(not(target_os = "linux"))]
        let filter: Option<serde_json::Value> = None;
        println!(
            "{}",
            json!({
                "file_before": file_before,
                "tcp_before": tcp_before,
                "exec_before": exec_before,
                "file_after": can_open(),
                "tcp_after": can_connect(),
                "exec_after": exec_after,
                "process_after": process_after,
                "windows_after": windows_after,
                "env_vars": std::env::vars_os().count(),
                "rlimit_core": rlimit_core,
                "dumpable": dumpable,
                "landlock_abi": confinement.landlock_abi,
                "landlock_enforced": confinement.landlock_enforced,
                "descriptors_before": descriptors_before,
                "descriptors_after": descriptors_after,
                "filter": filter,
            })
        );
        0
    }

    /// The descriptors from 3 up that are open, up to any a test would use.
    fn open_descriptors() -> Vec<i32> {
        #[cfg(unix)]
        // SAFETY: a plain system call that only asks about the number.
        return (3..1024).filter(|&fd| unsafe { libc::fcntl(fd, libc::F_GETFD) } != -1).collect();
        #[cfg(not(unix))]
        Vec::new()
    }

    /// The 32-bit `getpid`, through the entry a 32-bit program uses: `-1`
    /// (`EPERM`) when the filter refuses a foreign architecture's call.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn i386_getpid() -> i32 {
        let mut answer: i32 = 20;
        // SAFETY: a system call that takes nothing; `int 0x80` clobbers only
        // its result register.
        unsafe { std::arch::asm!("int 0x80", inout("eax") answer, options(nostack)) };
        answer
    }

    /// What the seccomp filter answers, tried from inside it. `refused_wrongly`
    /// holds every listed call that got anything but `EPERM`, with what it got
    /// (the arguments are all zero, so one the filter let through is harmless).
    /// `wrong` names each call whose answer depends on its arguments and came
    /// out otherwise than the filter's rules say. The errors of the real
    /// library calls show that it is the filter refusing and not Landlock.
    #[cfg(target_os = "linux")]
    fn filter_report(file: &str) -> serde_json::Value {
        use std::net::UdpSocket;
        use std::os::unix::net::UnixStream;

        use libc::{c_long, syscall};

        const EPERM: i32 = libc::EPERM;
        let errno = || std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        // SAFETY: plain system calls.
        let (me, parent) = unsafe { (libc::getpid() as c_long, libc::getppid() as c_long) };
        // The child of a `fork` or `clone` the filter let through goes no further.
        let answer = |result: c_long| {
            // SAFETY: plain system calls.
            if result == 0 && unsafe { libc::getpid() } as c_long != me {
                unsafe { libc::_exit(0) };
            }
            if result == -1 { errno() } else { 0 }
        };

        let mut refused_wrongly = Vec::new();
        for call in confine::refused_calls() {
            // A zero request to `ptrace` would have its parent trace it, and the
            // test would wait forever on a helper stopped by its next signal.
            let first: c_long = if call == libc::SYS_ptrace { -1 } else { 0 };
            // SAFETY: these arguments are refused or do nothing a test minds,
            // whichever answers.
            let got = answer(unsafe { syscall(call, first, 0, 0, 0, 0, 0) });
            if got != EPERM {
                refused_wrongly.push(json!([call, got]));
            }
        }

        let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        let limit = &raw mut limit;
        let core = libc::RLIMIT_CORE as c_long;
        // SAFETY: plain system calls: a signal 0 tests only that it may be
        // sent, and a mapping is at no fixed address; each call is on a pointer
        // to a local or none.
        let checks: [(&str, i32, i32); 14] = unsafe {
            let anonymous = (libc::PROT_READ, libc::MAP_ANONYMOUS);
            let mapping = |flags: i32| {
                syscall(libc::SYS_mmap, 0, 4096, anonymous.0, anonymous.1 | flags, -1, 0)
            };
            [
                ("kill of itself", answer(syscall(libc::SYS_kill, me, 0)), 0),
                ("kill of its parent", answer(syscall(libc::SYS_kill, parent, 0)), EPERM),
                ("kill of its group", answer(syscall(libc::SYS_kill, 0, 0)), EPERM),
                ("kill of everything", answer(syscall(libc::SYS_kill, -1, 0)), EPERM),
                ("tgkill of itself", answer(syscall(libc::SYS_tgkill, me, me, 0)), 0),
                (
                    "tgkill of its parent",
                    answer(syscall(libc::SYS_tgkill, parent, parent, 0)),
                    EPERM,
                ),
                ("its own limit", answer(syscall(libc::SYS_prlimit64, 0, core, 0, limit)), 0),
                (
                    "its parent's limit",
                    answer(syscall(libc::SYS_prlimit64, parent, core, 0, limit)),
                    EPERM,
                ),
                ("private memory", answer(mapping(libc::MAP_PRIVATE)), 0),
                ("shared memory", answer(mapping(libc::MAP_SHARED)), EPERM),
                (
                    "a new process",
                    answer(syscall(libc::SYS_clone, libc::SIGCHLD, 0, 0, 0, 0)),
                    EPERM,
                ),
                ("a file's flags", answer(syscall(libc::SYS_fcntl, 0, libc::F_GETFL)), 0),
                ("a file's owner", answer(syscall(libc::SYS_fcntl, 0, libc::F_SETOWN, me)), EPERM),
                ("a file's owner, by thread", answer(syscall(libc::SYS_fcntl, 0, 15, 0)), EPERM),
            ]
        };
        let mut wrong: Vec<String> = checks
            .into_iter()
            .filter(|&(_, got, wanted)| got != wanted)
            .map(|(name, ..)| name.to_owned())
            .collect();
        // SAFETY: an x32 number with zero arguments; on a kernel without x32
        // it would answer `ENOSYS`, and with it, x32's `socket`.
        if answer(unsafe { syscall(libc::SYS_socket | 0x4000_0000, 0, 0, 0) }) != EPERM {
            wrong.push("x32's socket".to_owned());
        }
        // SAFETY: as for any call the C library makes: `clone3` is refused in a
        // way it takes as "not implemented", and a thread is what is left.
        if answer(unsafe { syscall(libc::SYS_clone3, 0, 0) }) != libc::ENOSYS {
            wrong.push("clone3".to_owned());
        }
        if std::thread::spawn(|| ()).join().is_err() {
            wrong.push("a new thread".to_owned());
        }

        let code = |error: std::io::Error| error.raw_os_error();
        json!({
            "refused_wrongly": refused_wrongly,
            "wrong": wrong,
            "open": File::open(file).err().map(code),
            "udp": UdpSocket::bind("127.0.0.1:0").err().map(code),
            "unix": UnixStream::pair().err().map(code),
            "start": Command::new(PROGRAM).status().err().map(code),
            // `PR_GET_SPECULATION_CTRL` of store bypass and of indirect branches,
            // which `libc` names only on x86_64.
            // SAFETY: plain system calls that only read a setting.
            "speculation": unsafe { [0, 1].map(|what| libc::prctl(52, what, 0, 0, 0)) },
        })
    }

    /// Whether a handle that can read, write or run code in process `id` opens.
    #[cfg(windows)]
    fn can_touch(id: u32) -> bool {
        use windows_sys::Win32::Foundation::{CloseHandle, FALSE};
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_CREATE_THREAD, PROCESS_VM_READ, PROCESS_VM_WRITE,
        };
        let access = PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_CREATE_THREAD;
        // SAFETY: a plain system call; a handle it returns is closed once.
        unsafe {
            let handle = OpenProcess(access, FALSE, id);
            if handle.is_null() {
                return false;
            }
            CloseHandle(handle);
            true
        }
    }

    /// How many top-level windows this process can see.
    #[cfg(windows)]
    fn windows() -> u32 {
        use windows_sys::Win32::Foundation::{HWND, LPARAM, TRUE};
        use windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows;
        unsafe extern "system" fn count(_: HWND, seen: LPARAM) -> i32 {
            // SAFETY: `seen` is the pointer to the local below.
            unsafe { *(seen as *mut u32) += 1 };
            TRUE
        }
        let mut seen = 0u32;
        // SAFETY: the callback only writes the local, which outlives the call.
        unsafe { EnumWindows(Some(count), std::ptr::from_mut(&mut seen) as LPARAM) };
        seen
    }
}
