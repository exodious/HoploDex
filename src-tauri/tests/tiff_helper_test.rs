//! The TIFF render helper itself, run as `CARGO_BIN_EXE_hoplodex
//! --render-helper` (feature 007, T033; research.md §11's confinement table):
//! once confined it can open no file and no TCP socket, it exits when its
//! parent dies, its environment is empty, core dumps are off, and the parent
//! kills it past the `Load` and `Render` limits.
//!
//! Assumed API (T050-T053):
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
//!   <file>`: it tries to open `<file>` and to connect to 127.0.0.1:<port>,
//!   confines itself exactly as it does before serving frames, tries both
//!   again, and prints one JSON line then exits 0:
//!   `{ "file_before": bool, "tcp_before": bool,   // could, before confining
//!      "file_after": bool, "tcp_after": bool,     // could, once confined
//!      "exec_after": bool,                        // could start /bin/true
//!      "env_vars": number,                        // variables in its environment
//!      "rlimit_core": [soft, max] | null,         // unix
//!      "dumpable": bool | null,                   // Linux
//!      "landlock_abi": number | null,             // Linux: the ABI enforced
//!      "landlock_enforced": bool }`               // Linux: any rule in force
//!   It is the only way to look inside a confined process from outside, and it
//!   is compiled into debug and test builds only (`cfg(debug_assertions)`): a
//!   release binary has no such mode, so the tests that use it are skipped
//!   under `--release`.
//!
//! On Windows (research.md §11, amended 2026-10-07) HoploDex owns the helper's
//! job object and starts the helper into it with `CreateProcessW`
//! (`services::preview::helper_job::spawn`). The helper refuses to serve
//! outside such a job, so the self-check is run through `helper_job::spawn`,
//! `HelperHandle::command` is what a helper outside a job is started with,
//! and a parent that is terminated takes the helper with it.

#[path = "support/preview_support.rs"]
mod preview_support;
mod support;

#[cfg(debug_assertions)]
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;

#[cfg(windows)]
use hoplodex_lib::services::preview::helper_job;
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
/// Windows into a job object made for it (`helper_job::spawn`), which the
/// helper insists on; elsewhere through `HelperHandle::command`. Whether it
/// exited 0, and what it printed.
#[cfg(debug_assertions)]
fn run_self_check(args: &[&str]) -> (bool, String) {
    #[cfg(windows)]
    {
        use std::io::Read;

        let args: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsStr::new).collect();
        let (mut child, stdin, mut stdout) = helper_job::spawn(&exe(), &args).unwrap();
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

    #[cfg(target_os = "linux")]
    {
        let abi = json["landlock_abi"].as_u64();
        println!("Landlock ABI enforced by the helper: {abi:?}");
        eprintln!("Landlock ABI enforced by the helper: {abi:?}");
        let Some(abi) = abi.filter(|_| json["landlock_enforced"] == true) else {
            eprintln!(
                "Landlock is not available here (best effort, research.md §11): \
                 the file and network confinement can't be checked"
            );
            return;
        };
        assert!(abi >= 1);
        assert_eq!(json["file_after"], false, "a confined helper opened a file: {json}");
        assert_eq!(json["exec_after"], false, "a confined helper started a program: {json}");
        if abi >= 4 {
            assert_eq!(json["tcp_after"], false, "a confined helper connected: {json}");
            assert_eq!(connections, 1, "only the connection made before confining arrived");
        } else {
            eprintln!("ABI {abi} < 4: TCP is not restricted, so it can't be checked");
        }
    }
    #[cfg(target_os = "macos")]
    {
        assert_eq!(json["file_after"], false, "{json}");
        assert_eq!(json["tcp_after"], false, "{json}");
        assert_eq!(json["exec_after"], false, "{json}");
        assert_eq!(connections, 1);
    }
}

/// Windows' measures (research.md §11): the job object's one-process limit and
/// the child-process policy stop the helper from starting a program. The
/// mitigations don't confine files or the network, which the table doesn't ask.
#[cfg(all(debug_assertions, windows))]
#[test]
fn once_confined_it_cannot_start_a_program() {
    let Report { json, .. } = self_check();

    assert!(
        std::process::Command::new(r"C:\Windows\System32\whoami.exe")
            .stdout(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success()),
        "the probe program does not run here, so the check means nothing"
    );
    assert_eq!(json["exec_after"], false, "a confined helper started a program: {json}");
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
fn its_environment_is_empty() {
    let json = self_check().json;

    // macOS's libSystem adds `__CF_USER_TEXT_ENCODING` to a process started
    // with none, whatever the parent passes: it holds no more than that.
    let allowed = if cfg!(target_os = "macos") { 1 } else { 0 };
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
/// stdin still open (research.md §11, amended 2026-10-07): on macOS, which has
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
/// helper (research.md §11, amended 2026-10-07).
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
        .args(["--exact", "the_parent_stub", "--nocapture", "--test-threads=1"])
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
    parent.wait().unwrap();

    assert_eq!(waited, WAIT_OBJECT_0, "the helper outlived its parent by more than a second");
    assert!(took < Duration::from_secs(1), "the helper took {took:?} to go");
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
