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
//!   It is the only way to look inside a confined process from outside.

#[path = "support/preview_support.rs"]
mod preview_support;
mod support;

use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;

use hoplodex_lib::services::preview::{HelperHandle, HelperLimits};
use serde_json::Value;
use support::document_fixture;
use support::hostile_documents as hostile;
use tempfile::TempDir;

use preview_support::{assert_gone, png_size, process_is_gone, wait_until_gone};

fn exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_hoplodex"))
}

/// What the self-check saw, and how many connections the test's own listener
/// took (the self-check's one connection before confining, and no other).
#[allow(dead_code)]
struct Report {
    json: Value,
    connections: usize,
}

fn self_check() -> Report {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("secret.txt");
    std::fs::write(&file, b"not for the helper").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();

    let mut command = HelperHandle::command(&exe());
    command.args(["--self-check", &port.to_string(), file.to_str().unwrap()]);
    let output = command.output().unwrap();

    assert!(output.status.success(), "the self-check failed: {:?}", output.status);
    let line = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(line.trim()).unwrap_or_else(|e| panic!("{e}: {line}"));
    let mut connections = 0;
    while listener.accept().is_ok() {
        connections += 1;
    }
    Report { json, connections }
}

// --- Confinement ------------------------------------------------------------

#[cfg(any(target_os = "linux", target_os = "macos"))]
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

#[test]
fn its_environment_is_empty() {
    let json = self_check().json;

    assert_eq!(json["env_vars"], 0, "{json}");
}

#[cfg(unix)]
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
    use std::io::BufRead;
    use std::process::{Command, Stdio};

    // A shell stands in for HoploDex: it starts the helper on its own stdin,
    // prints the helper's PID and waits.
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
    // On Linux the pipe stays open (`stdin` is still held), so only the death
    // of its parent can end the helper (`PR_SET_PDEATHSIG`); elsewhere the end
    // of its stdin is the signal (research.md §11).
    #[cfg(not(target_os = "linux"))]
    drop(shell.stdin.take());

    assert!(wait_until_gone(helper, Duration::from_secs(5)), "the helper outlived its parent");
    drop(shell.stdin.take());
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
