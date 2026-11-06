//! #166: a crash of the app, or of a process it starts, mustn't leave the
//! app's memory on disk, which holds the open database's decrypted records
//! and its passphrase (`platform::core_dumps`).

use std::path::PathBuf;
#[cfg(unix)]
use std::process::Command;

/// Set in the child process the tests below start from this test program.
#[cfg(unix)]
const CHILD_ENV: &str = "HOPLODEX_TEST_CORE_DUMPS_CHILD";

/// Runs `test` (a `core_dumps_test::child_*` test) alone in a new process of
/// this test program, with [`CHILD_ENV`] set to `mode`.
#[cfg(unix)]
fn run_child(test: &str, mode: &str) -> std::process::Output {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &format!("core_dumps_test::{test}"), "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, mode)
        .output()
        .expect("could not start the child process")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn core_limit() -> (libc::rlim_t, libc::rlim_t) {
    let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: a plain system call with a pointer to a local.
    assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut limit) }, 0);
    (limit.rlim_cur, limit.rlim_max)
}

/// The child half of `core_dumps_are_off_soft_and_hard`: raises its soft
/// limit to its hard one, as a shell's `ulimit -c unlimited` would, then
/// turns core dumps off and prints both limits.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn child_turns_core_dumps_off() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    let (_, hard) = core_limit();
    let raised = libc::rlimit { rlim_cur: hard, rlim_max: hard };
    // SAFETY: a plain system call with a pointer to a local.
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_CORE, &raised) }, 0);
    let before = core_limit();
    hoplodex_lib::platform::core_dumps::turn_off_core_dumps();
    let after = core_limit();
    println!("BEFORE={}/{} AFTER={}/{}", before.0, before.1, after.0, after.1);
}

/// On Linux and macOS (a macOS shell that raised `ulimit -c` passes it on),
/// the app's core limit is 0, soft and hard, so no process it starts can
/// raise it again.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn core_dumps_are_off_soft_and_hard() {
    let output = run_child("child_turns_core_dumps_off", "limit");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find_map(|line| line.find("BEFORE=").map(|at| &line[at..]))
        .unwrap_or_else(|| panic!("the child reported nothing: {stdout}"));
    let before = line.split_whitespace().next().unwrap();
    assert_ne!(
        before, "BEFORE=0/0",
        "this test needs a hard core limit above 0 (`ulimit -H -c`) to show anything"
    );
    assert!(line.ends_with("AFTER=0/0"), "core dumps are still on: {line}");
}

/// The child half of `the_core_dump_filter_is_off_and_children_inherit_it`:
/// starts from the kernel's default filter, turns core dumps off, and prints
/// its own filter and that of a program it starts.
#[cfg(target_os = "linux")]
#[test]
fn child_turns_the_core_dump_filter_off() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    const FILTER: &str = "/proc/self/coredump_filter";
    std::fs::write(FILTER, "0x33").unwrap();
    let before = std::fs::read_to_string(FILTER).unwrap();
    hoplodex_lib::platform::core_dumps::turn_off_core_dumps();
    let after = std::fs::read_to_string(FILTER).unwrap();
    let started = Command::new("cat").arg(FILTER).output().unwrap();
    println!(
        "BEFORE={} AFTER={} STARTED={}",
        before.trim(),
        after.trim(),
        String::from_utf8_lossy(&started.stdout).trim()
    );
}

/// apport reads a core whatever the limit, and on Ubuntu 24.04 WebKit's web
/// process is outside the sandbox it skips, so the filter is what keeps its
/// decrypted records out of `/var/crash`: 0 in the app and in what it starts.
#[cfg(target_os = "linux")]
#[test]
fn the_core_dump_filter_is_off_and_children_inherit_it() {
    let output = run_child("child_turns_the_core_dump_filter_off", "filter");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find_map(|line| line.find("BEFORE=").map(|at| &line[at..]))
        .unwrap_or_else(|| panic!("the child reported nothing: {stdout}"));
    assert_eq!(line, "BEFORE=00000033 AFTER=00000000 STARTED=00000000");
}

/// The child half of `a_crash_ends_the_process_without_a_core`: handles the
/// fatal signals, then crashes for real.
#[cfg(target_os = "linux")]
#[test]
fn child_crashes() {
    let Some(mode) = std::env::var_os(CHILD_ENV) else { return };
    hoplodex_lib::platform::core_dumps::exit_on_fatal_signals();
    match mode.to_str() {
        // A bad memory access, as a use-after-free in native code would make.
        // SAFETY: none; reading address 16 is the crash this test wants.
        Some("segv") => unsafe {
            std::ptr::read_volatile(std::ptr::null::<u8>().wrapping_add(16));
        },
        // What `panic = "abort"` and glibc's heap checks end in.
        Some("abort") => std::process::abort(),
        other => panic!("unknown mode {other:?}"),
    }
}

/// A real bad access or abort, once the fatal signals are handled, ends the
/// process with an exit status (128 plus the signal) rather than the signal
/// itself, so neither the kernel nor a core handler (apport, systemd-coredump)
/// gets a core of it, whatever the core limit (#166).
#[cfg(target_os = "linux")]
#[test]
fn a_crash_ends_the_process_without_a_core() {
    use std::os::unix::process::ExitStatusExt;

    for (mode, signal) in [("segv", libc::SIGSEGV), ("abort", libc::SIGABRT)] {
        let output = run_child("child_crashes", mode);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.signal(),
            None,
            "{mode}: the signal ended the process, so it could dump core ({stderr})"
        );
        assert_eq!(output.status.code(), Some(128 + signal), "{mode}: {stderr}");
        assert!(stderr.contains("ending without a core dump (#166)"), "{mode}: {stderr}");
    }
}

/// Whether `/proc/<pid>/status` shows a handler for every one of `signals`
/// (`SigCgt`, a hexadecimal mask with bit `signal - 1` set for each).
#[cfg(target_os = "linux")]
fn catches(pid: u32, signals: &[libc::c_int]) -> bool {
    let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) else { return false };
    let Some(mask) = status.lines().find_map(|line| line.strip_prefix("SigCgt:")) else {
        return false;
    };
    let mask = u64::from_str_radix(mask.trim(), 16).unwrap();
    signals.iter().all(|signal| mask & (1 << (signal - 1)) != 0)
}

/// The app's own executable handles every fatal signal before it does
/// anything else, so its helpers (which hold decrypted document contents)
/// and the app itself end without a core: each signal sent to a running import
/// helper ends it with an exit status, not the signal.
#[cfg(target_os = "linux")]
#[test]
fn the_app_handles_fatal_signals_from_its_start() {
    use hoplodex_lib::platform::core_dumps::FATAL_SIGNALS;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    for signal in FATAL_SIGNALS {
        // It reads its requests from standard input, so it waits there.
        let mut helper = Command::new(env!("CARGO_BIN_EXE_hoplodex"))
            .arg(hoplodex_lib::services::import_reader::reader::ARGUMENT)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("could not start the import helper");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !catches(helper.id(), &FATAL_SIGNALS) {
            if let Some(status) = helper.try_wait().unwrap() {
                panic!("the import helper exited before it handled the signals: {status}");
            }
            assert!(
                Instant::now() < deadline,
                "the import helper handles not all of {FATAL_SIGNALS:?} after 10 s"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        // SAFETY: a plain system call; the pid is our own child, still running.
        assert_eq!(unsafe { libc::kill(helper.id() as libc::pid_t, signal) }, 0);
        let output = helper.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), None, "signal {signal} ended the helper: {stderr}");
        assert_eq!(output.status.code(), Some(128 + signal), "signal {signal}: {stderr}");
    }
}

/// Inside the Flatpak, the web processes are found by their name in the
/// sandbox's `/proc`, and nothing else is.
#[cfg(target_os = "linux")]
#[test]
fn web_processes_are_found_by_name() {
    let proc_dir = tempfile::tempdir().unwrap();
    for (pid, name) in [
        ("12", "WebKitWebProces\n"),
        ("310", "WebKitWebProces\n"),
        ("13", "WebKitNetworkPr\n"),
        ("2", "hoplodex\n"),
        ("14", "bwrap\n"),
    ] {
        std::fs::create_dir(proc_dir.path().join(pid)).unwrap();
        std::fs::write(proc_dir.path().join(pid).join("comm"), name).unwrap();
    }
    // Not a process.
    std::fs::create_dir(proc_dir.path().join("sys")).unwrap();
    std::fs::create_dir(proc_dir.path().join("15")).unwrap();

    assert_eq!(
        hoplodex_lib::platform::core_dumps::web_process_pids(proc_dir.path()),
        vec![12, 310]
    );
}

/// The soft and hard core limit `/proc/<pid>/limits` shows (`unlimited` as
/// `u64::MAX`).
#[cfg(target_os = "linux")]
fn core_limit_of(pid: u32) -> (u64, u64) {
    let limits = std::fs::read_to_string(format!("/proc/{pid}/limits")).unwrap();
    let row = limits.lines().find(|line| line.starts_with("Max core file size")).unwrap();
    let mut values = row["Max core file size".len()..]
        .split_whitespace()
        .map(|value| if value == "unlimited" { u64::MAX } else { value.parse().unwrap() });
    (values.next().unwrap(), values.next().unwrap())
}

/// Another process of the same user, as the portal's web process is to the
/// app, gets a core limit of 0, soft and hard.
#[cfg(target_os = "linux")]
#[test]
fn another_processes_core_dumps_are_turned_off() {
    let mut other = Command::new("sleep").arg("30").spawn().unwrap();
    let pid = other.id();
    let before = core_limit_of(pid);
    let turned_off = hoplodex_lib::platform::core_dumps::turn_off_core_dumps_of(pid as libc::pid_t);
    let after = core_limit_of(pid);
    other.kill().unwrap();
    other.wait().unwrap();

    turned_off.unwrap();
    assert_ne!(before, (0, 0), "this test needs a hard core limit above 0 to show anything");
    assert_eq!(after, (0, 0));
}

fn manifest_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// The .deb puts `/usr/bin/hoplodex`, and only it, on apport's ignore list,
/// which apport 2.28 (Ubuntu 24.04) and 2.34 (26.04) read before they take
/// the core: Ubuntu's apport is the kernel's core handler and reads a
/// crashed process's core whatever its core limit. WebKit's own processes
/// aren't listed: they belong to every WebKitGTK app on the computer.
#[test]
fn the_deb_puts_the_app_on_apports_ignore_list() {
    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(manifest_path("tauri.conf.json")).unwrap())
            .unwrap();
    let files = &config["bundle"]["linux"]["deb"]["files"];
    let source = files["/etc/apport/blacklist.d/hoplodex"]
        .as_str()
        .unwrap_or_else(|| panic!("the .deb has no apport ignore entry: {files}"));
    let text = std::fs::read_to_string(manifest_path(source)).unwrap();
    let listed: Vec<&str> =
        text.lines().filter(|line| !line.is_empty() && !line.starts_with('#')).collect();

    assert_eq!(listed, ["/usr/bin/hoplodex"]);
}
