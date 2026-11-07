//! The TIFF render helper itself, run as `CARGO_BIN_EXE_hoplodex
//! --render-helper` (007 T033; 007 research.md §11's confinement table):
//! once confined it can open no file and no TCP socket, on Linux a seccomp
//! filter refuses its listed system calls and it won't serve without the
//! filter, it holds no inherited descriptor, it exits when its parent dies, its
//! environment is empty, core dumps are off, the parent kills it past the
//! `Load` and `Render` limits, and the parent encodes what it answers only if
//! the pixels fit the request.
//!
//! Assumed API (007 T050-T053):
//! - `services::preview::HelperHandle::command(exe: &Path) -> std::process::Command`:
//!   the command the parent runs, `--render-helper` already its argument,
//!   with a cleared environment, piped stdin and stdout and stderr discarded.
//!   Tests add arguments to it and spawn it themselves.
//! - `HelperHandle::spawn(exe, HelperLimits) -> Result<HelperHandle, _>`,
//!   `.load(&[u8]) -> Result<Vec<PageSize>, _>`,
//!   `.render(page: u32, width_px: u32) -> Result<Vec<u8>, _>`, `.pid() -> u32`,
//!   and `Drop` kills and waits. `HelperLimits { load, render }` (`Duration`s)
//!   has a `Default` of 20 s and 10 s. A load or render that passes its limit
//!   returns an error, and the helper has been killed by then.
//! - A self-check mode, `hoplodex --render-helper --self-check <port>
//!   <file> [<process id>]`: it tries to open `<file>`, to connect to 127.0.0.1:<port> and to
//!   start a program, confines itself exactly as it does before serving
//!   frames, tries all three again, and prints one JSON line then exits 0:
//!   `{ "file_before": bool, "tcp_before": bool,   // could, before confining
//!      "exec_before": bool,                       // could start a program
//!      "file_after": bool, "tcp_after": bool,     // could, once confined
//!      "exec_after": bool,                        // could start it, once confined
//!      "process_after": bool | null,              // Windows, given <process id>: could open it
//!      "windows_after": number | null,            // Windows: top-level windows it sees
//!      "env_vars": number,                        // variables in its environment
//!      "rlimit_core": [soft, max] | null,         // unix
//!      "dumpable": bool | null,                   // Linux
//!      "landlock_abi": number | null,             // Linux: the ABI enforced
//!      "landlock_enforced": bool,                 // Linux: any rule in force
//!      "descriptors_before": [number],            // open from 3 up, before and
//!      "descriptors_after": [number],             // once confined
//!      "filter": object | null }`                 // Linux: `filter_report`'s
//!   It is the only way to look inside a confined process from outside, and it
//!   is compiled into debug and test builds only (`cfg(debug_assertions)`): a
//!   release binary has no such mode, so the tests that use it are skipped
//!   under `--release`.
//!
//! On Windows (007 research.md §11, amended 2026-11-07) HoploDex owns the helper's
//! job object and starts the helper into it, in an AppContainer, with `CreateProcessW`
//! (`services::helper::job::spawn`). That needs a desktop session, so these tests
//! go through `gui.ps1` over SSH (docs/development/windows.md). The helper refuses to serve
//! outside such a job, so the self-check is run through `helper::job::spawn`,
//! `HelperHandle::command` is what a helper outside a job is started with,
//! and a parent that is terminated takes the helper with it.

use crate::{preview_support, support};

#[cfg(debug_assertions)]
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;

#[cfg(windows)]
use hoplodex_lib::services::helper::job;
#[cfg(windows)]
use hoplodex_lib::services::preview::helper::{ARGUMENT, MEMORY_LIMIT};
use hoplodex_lib::services::preview::{HelperHandle, HelperLimits};
#[cfg(debug_assertions)]
use serde_json::Value;
use support::document_fixture;
use support::hostile_documents as hostile;
#[cfg(debug_assertions)]
use tempfile::TempDir;

use preview_support::{assert_gone, png_size};
#[cfg(unix)]
use preview_support::{process_is_gone, wait_until_gone};

fn exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_hoplodex"))
}

/// What the self-check saw, and how many connections the test's own listener
/// took (the self-check's one connection before confining, and no other).
#[cfg(debug_assertions)]
#[allow(dead_code)]
struct Report {
    json: Value,
    connections: usize,
}

/// Runs the helper's self-check the way the app starts the helper: on
/// Windows into a job object made for it (`helper::job::spawn`), which the
/// helper insists on; elsewhere through `HelperHandle::command`. Whether it
/// exited 0, and what it printed.
#[cfg(debug_assertions)]
fn run_self_check(args: &[&str]) -> (bool, String) {
    #[cfg(windows)]
    {
        use std::io::Read;

        let args: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsStr::new).collect();
        let (mut child, stdin, mut stdout) =
            job::spawn(&exe(), ARGUMENT, MEMORY_LIMIT, &args).unwrap();
        let mut text = String::new();
        stdout.read_to_string(&mut text).unwrap();
        let code = child.wait().unwrap();
        drop(stdin);
        (code == 0, text)
    }
    #[cfg(not(windows))]
    {
        let output = HelperHandle::command(&exe()).args(args).output().unwrap();
        (output.status.success(), String::from_utf8(output.stdout).unwrap())
    }
}

#[cfg(debug_assertions)]
fn self_check() -> Report {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("secret.txt");
    std::fs::write(&file, b"not for the helper").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();

    let args = ["--self-check", &port.to_string(), file.to_str().unwrap()];
    let (success, stdout) = run_self_check(&args);

    assert!(success, "the self-check failed");
    let line = stdout;
    let json: Value = serde_json::from_str(line.trim()).unwrap_or_else(|e| panic!("{e}: {line}"));
    let mut connections = 0;
    while listener.accept().is_ok() {
        connections += 1;
    }
    Report { json, connections }
}

// --- Confinement ------------------------------------------------------------

#[cfg(all(debug_assertions, any(target_os = "linux", target_os = "macos")))]
#[test]
fn once_confined_it_can_open_no_file_and_no_tcp_socket() {
    let Report { json, connections } = self_check();

    // The probes mean something only if they worked before confining.
    assert_eq!(json["file_before"], true, "{json}");
    assert_eq!(json["tcp_before"], true, "{json}");
    assert_eq!(json["exec_before"], true, "{json}");

    // Whatever Landlock this kernel has: the filter holds without it.
    #[cfg(target_os = "linux")]
    eprintln!(
        "Landlock ABI of the helper: {:?}, enforced: {}",
        json["landlock_abi"], json["landlock_enforced"]
    );
    assert_eq!(json["file_after"], false, "a confined helper opened a file: {json}");
    assert_eq!(json["tcp_after"], false, "a confined helper connected: {json}");
    assert_eq!(json["exec_after"], false, "a confined helper started a program: {json}");
    assert_eq!(connections, 1, "only the connection made before confining arrived");
}

/// Every listed call gets `EPERM`, the filter's answer and not Landlock's
/// `EACCES`, so this holds on a kernel with no Landlock too. What depends on a
/// call's arguments passes only as the filter's rules say, and a new thread
/// still starts.
#[cfg(all(debug_assertions, target_os = "linux"))]
#[test]
fn the_filter_refuses_every_listed_call_and_nothing_a_helper_needs() {
    let json = self_check().json;
    let filter = &json["filter"];

    assert_eq!(filter["refused_wrongly"], serde_json::json!([]), "[call, errno]: {json}");
    assert_eq!(filter["wrong"], serde_json::json!([]), "{json}");
    let eperm = serde_json::json!(libc::EPERM);
    assert_eq!(filter["open"], eperm, "opening a file: {json}");
    assert_eq!(filter["udp"], eperm, "a UDP socket: {json}");
    assert_eq!(filter["unix"], eperm, "a pair of Unix sockets: {json}");
    assert_eq!(filter["start"], eperm, "starting a program: {json}");
}

/// The numbers in the filter mean other calls on another architecture, so a
/// call from one is refused rather than judged by them.
#[cfg(all(debug_assertions, target_os = "linux", target_arch = "x86_64"))]
#[test]
fn a_call_from_a_foreign_architecture_is_refused() {
    let output = HelperHandle::command(&exe())
        .args(["--self-check", "9", "unused", "--foreign-arch"])
        .output()
        .unwrap();

    // 128 + SIGSEGV from `exit_on_fatal_signals`: this kernel has no 32-bit
    // entry at all, so no such call can reach the filter.
    if output.status.code() == Some(128 + libc::SIGSEGV) {
        eprintln!("this kernel has no 32-bit entry: nothing to check");
        return;
    }
    assert!(output.status.success(), "{:?}", output.status);
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["answer"], -libc::EPERM, "{json}");
}

/// Before Linux 5.16 a filter turns on CPU speculation mitigations unless asked
/// not to, and a helper then reads a file several times slower. It only shows on
/// such a kernel: elsewhere this passes with or without the flag that asks.
#[cfg(all(debug_assertions, target_os = "linux"))]
#[test]
fn its_filter_leaves_its_speculation_settings_as_they_were() {
    // `PR_GET_SPECULATION_CTRL` of store bypass and of indirect branches, as the
    // helper's self-check reads them.
    // SAFETY: plain system calls that only read a setting.
    let own = unsafe { [0, 1].map(|what| libc::prctl(52, what, 0, 0, 0)) };
    assert_eq!(self_check().json["filter"]["speculation"], serde_json::json!(own));
}

/// The filter fails closed. A filter in the helper's parent (stacked, so the
/// helper can't remove it) refuses to install any other, and all else in
/// `confine` still works: a helper that can't filter itself reads no document.
#[cfg(target_os = "linux")]
#[test]
fn without_its_filter_it_refuses_to_serve() {
    use std::os::unix::process::CommandExt;

    fn statement(code: u32, k: u32) -> libc::sock_filter {
        libc::sock_filter { code: code as u16, jt: 0, jf: 0, k }
    }
    fn when_equal(k: u32, jf: u8) -> libc::sock_filter {
        let code = libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K;
        libc::sock_filter { code: code as u16, jt: 0, jf, k }
    }
    let load = libc::BPF_LD | libc::BPF_W | libc::BPF_ABS;
    let ret = libc::BPF_RET | libc::BPF_K;
    let filter = [
        statement(load, 0),
        when_equal(libc::SYS_seccomp as u32, 1),
        statement(ret, libc::SECCOMP_RET_ERRNO | libc::EPERM as u32),
        statement(ret, libc::SECCOMP_RET_ALLOW),
    ];

    let mut command = HelperHandle::command(&exe());
    // SAFETY: between fork and exec, only two system calls and a program on
    // the stack, which the kernel copies.
    unsafe {
        command.pre_exec(move || {
            let program =
                libc::sock_fprog { len: filter.len() as u16, filter: filter.as_ptr().cast_mut() };
            let program = &raw const program;
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1 as libc::c_ulong, 0, 0, 0) != 0
                || libc::prctl(
                    libc::PR_SET_SECCOMP,
                    libc::SECCOMP_MODE_FILTER as libc::c_ulong,
                    program,
                ) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let output = command.output().unwrap();

    assert_eq!(output.status.code(), Some(3), "a helper without its filter served");
    assert!(output.stdout.is_empty());
}

/// Whatever a parent leaves inheritable is closed before the helper reads a
/// document, on macOS too, which has no way to start a process with only its
/// three pipes. What's left is the macOS parent watch's own queue.
#[cfg(all(debug_assertions, unix))]
#[test]
fn it_holds_no_descriptor_it_inherited() {
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;

    const INHERITED: i32 = 57;
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("secret.txt");
    std::fs::write(&file, b"not for the helper").unwrap();
    let open = std::fs::File::open(&file).unwrap();
    let source = open.as_raw_fd();

    let mut command = HelperHandle::command(&exe());
    command.args(["--self-check", "9", file.to_str().unwrap()]);
    // SAFETY: between fork and exec, one system call on numbers.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(source, INHERITED) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "{:?}", output.status);
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();

    let held = |key: &str| {
        json[key].as_array().unwrap().iter().map(|fd| fd.as_i64().unwrap()).collect::<Vec<_>>()
    };
    assert!(
        held("descriptors_before").contains(&i64::from(INHERITED)),
        "the probe means nothing unless it was inherited: {json}"
    );
    let allowed = usize::from(cfg!(target_os = "macos"));
    assert!(held("descriptors_after").len() <= allowed, "{json}");
    assert!(!held("descriptors_after").contains(&i64::from(INHERITED)), "{json}");
}

/// Windows' measures (007 research.md §11, amended 2026-11-07): the
/// AppContainer has no capability, so the helper has no user file, no network
/// and no reach into HoploDex from its first instruction, and the job object's
/// one-process limit and child-process policy stop it starting a program. The
/// probes mean something because this test, outside the container, can do each.
#[cfg(all(debug_assertions, windows))]
#[test]
fn once_confined_it_can_reach_no_file_network_program_or_parent() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("secret.txt");
    std::fs::write(&file, b"not for the helper").unwrap();
    std::fs::File::open(&file).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let ours = listener.accept().unwrap();
    listener.set_nonblocking(true).unwrap();
    assert!(
        std::process::Command::new(r"C:\Windows\System32\whoami.exe")
            .stdout(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success()),
        "the probe program does not run here, so the check means nothing"
    );

    let parent = std::process::id().to_string();
    let (success, stdout) =
        run_self_check(&["--self-check", &port.to_string(), file.to_str().unwrap(), &parent]);
    assert!(success, "the self-check failed");
    let json: Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    drop(ours);

    // Before confining too: the container is there from the first instruction.
    assert_eq!(json["file_before"], false, "a helper opened a user's file: {json}");
    assert_eq!(json["tcp_before"], false, "a helper connected: {json}");
    assert_eq!(json["file_after"], false, "a confined helper opened a user's file: {json}");
    assert_eq!(json["tcp_after"], false, "a confined helper connected: {json}");
    assert_eq!(json["exec_after"], false, "a confined helper started a program: {json}");
    assert_eq!(json["process_after"], false, "a confined helper opened HoploDex: {json}");
    assert!(listener.accept().is_err(), "a connection reached the listener: {json}");
}

/// Each helper has a container of its own (007 research.md §11, amended
/// 2026-11-07), so one can't open another, which may hold another document.
#[cfg(all(debug_assertions, windows))]
#[test]
fn once_confined_it_cannot_open_another_helper() {
    let (sibling, _stdin, _stdout) = job::spawn(&exe(), ARGUMENT, MEMORY_LIMIT, &[]).unwrap();

    let id = sibling.id().to_string();
    let (success, stdout) = run_self_check(&["--self-check", "9", r"C:\nothing", &id]);

    assert!(success, "the self-check failed");
    let json: Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    assert_eq!(json["process_after"], false, "a confined helper opened another: {json}");
}

/// Its container's profile, the one place it can write, goes with it (007
/// research.md §11, amended 2026-11-07), so nothing stays for a later helper.
#[cfg(windows)]
#[test]
fn its_containers_profile_is_deleted_when_it_ends() {
    let (mut helper, stdin, stdout) = job::spawn(&exe(), ARGUMENT, MEMORY_LIMIT, &[]).unwrap();
    let profile = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap())
        .join("Packages")
        .join(helper.container().to_lowercase());
    assert!(profile.is_dir(), "no profile at {}", profile.display());

    helper.kill().unwrap();
    helper.wait().unwrap();
    drop((helper, stdin, stdout));

    // Its drop waits a moment for the delete, which on a slow machine goes on behind it.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while profile.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!profile.exists(), "{} is still there", profile.display());
}

/// The job's UI limits (007 research.md §11, amended 2026-11-07): a helper sees
/// no other window, though this test, in the same desktop, sees several.
#[cfg(all(debug_assertions, windows))]
#[test]
fn once_confined_it_cannot_see_other_windows() {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, TRUE};
    use windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows;

    unsafe extern "system" fn count(_: HWND, seen: LPARAM) -> i32 {
        // SAFETY: `seen` is the pointer to the local below.
        unsafe { *(seen as *mut u32) += 1 };
        TRUE
    }
    let mut ours = 0u32;
    // SAFETY: the callback only writes the local, which outlives the call.
    unsafe { EnumWindows(Some(count), std::ptr::from_mut(&mut ours) as LPARAM) };
    assert!(ours > 1, "this test sees {ours} windows, so the check means nothing");

    let json = self_check().json;
    assert!(json["windows_after"].as_u64().unwrap() <= 1, "a helper saw other windows: {json}");
}

/// On Windows the job object is HoploDex's, so a helper that was not put in
/// one the app's way (here: started by `HelperHandle::command`, which the app
/// does not run on Windows) refuses to serve, and so does its self-check:
/// both exit with 3 before reading a document.
#[cfg(windows)]
#[test]
fn started_outside_hoplodexs_job_it_refuses_to_serve() {
    let serve = HelperHandle::command(&exe()).output().unwrap();
    assert_eq!(serve.status.code(), Some(3), "a helper outside the job served");
    assert!(serve.stdout.is_empty());

    #[cfg(debug_assertions)]
    {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("secret.txt");
        std::fs::write(&file, b"not for the helper").unwrap();
        let check = HelperHandle::command(&exe())
            .args(["--self-check", "9", file.to_str().unwrap()])
            .output()
            .unwrap();
        assert_eq!(check.status.code(), Some(3), "a self-check outside the job confined itself");
    }
}

#[cfg(debug_assertions)]
#[test]
fn its_environment_holds_only_what_the_os_needs() {
    let json = self_check().json;

    // macOS's libSystem adds `__CF_USER_TEXT_ENCODING` to a process started
    // with none, whatever the parent passes. Windows has the `LOCALAPPDATA` an
    // AppContainer needs, and `TEMP` and `TMP`, which starting one adds.
    let allowed = if cfg!(target_os = "macos") {
        1
    } else if cfg!(windows) {
        3
    } else {
        0
    };
    assert!(json["env_vars"].as_u64().unwrap() <= allowed, "{json}");
}

#[cfg(all(debug_assertions, unix))]
#[test]
fn core_dumps_are_off() {
    let json = self_check().json;

    assert_eq!(json["rlimit_core"], serde_json::json!([0, 0]), "{json}");
    #[cfg(target_os = "linux")]
    assert_eq!(json["dumpable"], false, "{json}");
}

// --- Dying with its parent ----------------------------------------------------

#[cfg(unix)]
#[test]
fn it_exits_when_its_parent_dies() {
    it_exits_within(Duration::from_secs(5));
}

/// If HoploDex exits, everything it spawned goes with it at once, with its
/// stdin still open (007 research.md §11, amended 2026-10-07): on macOS, which has
/// no `PR_SET_PDEATHSIG`, a `kqueue` watch on the parent does it.
#[cfg(target_os = "macos")]
#[test]
fn on_macos_it_is_gone_within_a_second_of_its_parent_being_killed() {
    it_exits_within(Duration::from_secs(1));
}

/// An intermediate process stands in for HoploDex: it starts the helper on its
/// own stdin, prints the helper's PID and waits. The test kills it with
/// SIGKILL and times the helper's end. Its stdin stays open (`shell.stdin` is
/// held), so only the death of the parent can end the helper, not the end of
/// its input.
#[cfg(unix)]
fn it_exits_within(limit: Duration) {
    use std::io::BufRead;
    use std::process::{Command, Stdio};

    let mut shell = Command::new("sh")
        .arg("-c")
        .arg(r#"exec 3<&0; "$0" --render-helper <&3 & echo $!; wait"#)
        .arg(exe())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    std::io::BufReader::new(shell.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let helper: u32 = line.trim().parse().unwrap_or_else(|_| panic!("no PID in {line:?}"));
    assert!(wait_for_alive(helper), "the helper did not start");
    std::thread::sleep(Duration::from_millis(200));
    assert!(!process_is_gone(helper), "the helper exited on its own");

    shell.kill().unwrap();
    shell.wait().unwrap();

    assert!(wait_until_gone(helper, limit), "the helper outlived its parent by {limit:?}");
    drop(shell.stdin.take());
}

/// The environment variable that makes `the_parent_stub` act as HoploDex.
#[cfg(windows)]
const STUB: &str = "HOPLODEX_TEST_PARENT_STUB";

/// Not a test of its own: the process `the_helper_dies_with_hoplodex_even_if_it_is_killed`
/// starts and kills, standing in for HoploDex. It spawns the helper the app's
/// way (`HelperHandle::spawn`), has it load and render a page (so it is
/// serving, which means it is confined and in its job), prints its PID and
/// waits to be killed. Run on its own, without the variable, it does nothing.
#[cfg(windows)]
#[test]
fn the_parent_stub() {
    use std::io::Write;

    if std::env::var_os(STUB).is_none() {
        return;
    }
    let mut helper = HelperHandle::spawn(&exe(), HelperLimits::default()).unwrap();
    helper.load(&document_fixture("one-page.tif")).unwrap();
    helper.render(0, 100).unwrap();
    println!("HELPER_PID={}", helper.pid());
    std::io::stdout().flush().unwrap();
    std::thread::sleep(Duration::from_secs(120));
}

/// "If HoploDex exits, everything it spawned goes with it at once": HoploDex
/// owns the helper's job object, so a HoploDex that is terminated (no chance
/// to run any code of its own) closes the handle and the kernel kills the
/// helper (007 research.md §11, amended 2026-10-07).
#[cfg(windows)]
#[test]
fn the_helper_dies_with_hoplodex_even_if_it_is_killed() {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    let mut parent = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "tiff_helper_test::the_parent_stub", "--nocapture", "--test-threads=1"])
        .env(STUB, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut lines = std::io::BufReader::new(parent.stdout.take().unwrap()).lines();
    let helper = loop {
        let line = lines.next().expect("the parent stub ended before it started a helper");
        // libtest starts the line with the test's name.
        let line = line.unwrap();
        if let Some((_, pid)) = line.split_once("HELPER_PID=") {
            break pid.trim().parse::<u32>().unwrap();
        }
    };
    // SAFETY: a plain system call; the handle is closed below.
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, helper) };
    assert!(!handle.is_null(), "the helper {helper} is not there to watch");
    assert!(
        // SAFETY: the handle is open.
        unsafe { WaitForSingleObject(handle, 0) } != WAIT_OBJECT_0,
        "the helper exited on its own"
    );

    // `Child::kill` is `TerminateProcess`: nothing of the parent runs again.
    parent.kill().unwrap();
    let killed = Instant::now();
    // SAFETY: the handle is open.
    let waited = unsafe { WaitForSingleObject(handle, 1000) };
    let took = killed.elapsed();
    // SAFETY: the handle is open and not used again.
    unsafe { CloseHandle(handle) };
    // A killed parent can't delete its helper's profile, so this does.
    let profile: Vec<u16> = format!("HoploDex.Helper.{}.0\0", parent.id()).encode_utf16().collect();
    // SAFETY: a NUL-terminated name that outlives the call.
    unsafe { windows_sys::Win32::Security::Isolation::DeleteAppContainerProfile(profile.as_ptr()) };
    parent.wait().unwrap();

    assert_eq!(waited, WAIT_OBJECT_0, "the helper outlived its parent by more than a second");
    crate::support::assert_budget!(took < Duration::from_secs(1), "the helper took {took:?} to go");
}

#[cfg(unix)]
fn wait_for_alive(pid: u32) -> bool {
    preview_support::wait_for(Duration::from_secs(5), || !process_is_gone(pid))
}

// --- Time limits ----------------------------------------------------------------

#[test]
fn the_limits_are_20_seconds_to_load_and_10_to_render() {
    let limits = HelperLimits::default();

    assert_eq!(limits.load, Duration::from_secs(20));
    assert_eq!(limits.render, Duration::from_secs(10));
}

#[test]
fn a_healthy_helper_loads_and_renders_and_is_killed_when_dropped() {
    let mut helper = HelperHandle::spawn(&exe(), HelperLimits::default()).unwrap();
    let pid = helper.pid();

    let pages = helper.load(&document_fixture("one-page.tif")).unwrap();
    let png = helper.render(0, 100).unwrap();

    assert_eq!(pages.len(), 1);
    assert_eq!(png_size(&png).0, 100);
    drop(helper);
    assert_gone(pid, "after its handle was dropped");
}

// --- A helper that lies about its page -----------------------------------------

/// A stand-in for a hostile helper: a program, where `HelperHandle::spawn` is
/// told the helper is, that answers with `frames` whatever it is asked.
#[cfg(unix)]
struct LyingHelper {
    _dir: tempfile::TempDir,
    exe: PathBuf,
}

#[cfg(unix)]
fn lying_helper(page: &[u8]) -> LyingHelper {
    use std::os::unix::fs::PermissionsExt;

    use hoplodex_lib::services::preview::PageSize;
    use hoplodex_lib::services::preview::helper_protocol::{Response, write_response};

    let dir = tempfile::TempDir::new().unwrap();
    let mut frames = Vec::new();
    let pages = vec![PageSize { width: 100.0, height: 100.0 }];
    write_response(&mut frames, &Response::Loaded { pages }).unwrap();
    frames.extend_from_slice(page);
    std::fs::write(dir.path().join("frames"), frames).unwrap();
    let exe = dir.path().join("helper");
    std::fs::write(&exe, "#!/bin/sh\n/bin/cat \"${0%/*}/frames\"\nexec /bin/sleep 60\n").unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    LyingHelper { _dir: dir, exe }
}

#[cfg(unix)]
fn page_frame(width: u32, height: u32, channels: u8, samples: Vec<u8>) -> Vec<u8> {
    use hoplodex_lib::services::preview::helper_protocol::{Pixels, Response, write_response};

    let mut frame = Vec::new();
    let page = Response::Page(Pixels { width, height, channels, samples });
    write_response(&mut frame, &page).unwrap();
    frame
}

/// What reaches the web view is a PNG the app made of the pixels the helper
/// sent, not anything the helper wrote.
#[cfg(unix)]
#[test]
fn the_app_encodes_the_pixels_it_is_sent() {
    let samples = vec![10, 20, 30, 40, 50, 60];
    let helper = lying_helper(&page_frame(2, 1, 3, samples.clone()));
    let mut handle = HelperHandle::spawn(&helper.exe, HelperLimits::default()).unwrap();
    handle.load(b"a document").unwrap();

    let png = handle.render(0, 100).unwrap();

    let mut reader = png::Decoder::new(std::io::Cursor::new(&png)).read_info().unwrap();
    let mut decoded = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut decoded).unwrap();
    assert_eq!((info.width, info.height, info.color_type), (2, 1, png::ColorType::Rgb));
    assert_eq!(&decoded[..info.buffer_size()], samples);
    assert_eq!(handle.restarts(), 0);
}

/// Pixels that don't fit the request, or a frame that is not pixels at all
/// (here a whole PNG, which is what a helper once sent), are never encoded: the
/// helper is treated as any that breaks the protocol, killed and replaced, and
/// the page fails alone.
#[cfg(unix)]
#[test]
fn a_page_that_does_not_fit_the_request_kills_the_helper_and_fails_alone() {
    use hoplodex_lib::services::helper::frames::{MAX_FRAME, write_frame};
    use hoplodex_lib::services::preview::HelperError;

    const PAGE: u8 = 13;
    let mut a_png = Vec::new();
    write_frame(&mut a_png, PAGE, b"\x89PNG\r\n\x1a\n and the rest of a file", MAX_FRAME).unwrap();
    let lies = [
        ("wider than asked", page_frame(101, 1, 1, vec![0; 101])),
        ("samples short of its size", page_frame(10, 10, 1, vec![0; 99])),
        ("samples over its size", page_frame(10, 10, 1, vec![0; 101])),
        ("a channel count no page has", page_frame(10, 10, 4, vec![0; 400])),
        ("no pixels at all", page_frame(0, 0, 1, Vec::new())),
        ("a file and not pixels", a_png),
    ];
    for (lie, frame) in lies {
        let helper = lying_helper(&frame);
        let mut handle = HelperHandle::spawn(&helper.exe, HelperLimits::default()).unwrap();
        handle.load(b"a document").unwrap();
        let liar = handle.pid();

        let result = handle.render(0, 100);

        assert!(matches!(result, Err(HelperError::PageFailed)), "{lie}: {result:?}");
        assert_gone(liar, lie);
        assert_eq!(handle.restarts(), 1, "{lie}");
        assert_ne!(handle.pid(), liar, "{lie}");
    }
}

#[test]
fn a_load_over_its_limit_is_killed_by_the_parent() {
    let limits = HelperLimits { load: Duration::from_millis(1), render: Duration::from_secs(10) };
    let mut helper = HelperHandle::spawn(&exe(), limits).unwrap();
    let pid = helper.pid();
    // Writing 32 MB down a pipe alone takes longer than a millisecond.
    let large = hostile::large_tiff(32_000_000);

    let result = helper.load(&large.bytes);

    assert!(result.is_err(), "a load over its limit succeeded");
    assert_gone(pid, "after its load passed the limit");
}

#[test]
fn a_render_over_its_limit_is_killed_by_the_parent() {
    let limits = HelperLimits { load: Duration::from_secs(60), render: Duration::from_millis(1) };
    let mut helper = HelperHandle::spawn(&exe(), limits).unwrap();
    let pid = helper.pid();
    // 8192 x 2000 pixels: a page that takes far longer than a millisecond.
    let large = hostile::large_tiff(8192 * 2000);
    assert_eq!(helper.load(&large.bytes).unwrap().len(), 1);

    let result = helper.render(0, 4096);

    assert!(result.is_err(), "a render over its limit succeeded");
    assert_gone(pid, "after its render passed the limit");
}
