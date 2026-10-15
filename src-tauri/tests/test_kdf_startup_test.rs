//! A `test-kdf` build's app refuses to start (main.rs, #77 step 11), so no
//! database is ever made with the tests' KDF outside the tests. The tests
//! start the same binary for its helpers, which still run.
//!
//! Only a `test-kdf` build has the refusal, so only it runs this. It starts
//! the app in a sandbox: a `test-kdf` build, like an E2E build, takes its
//! directories only from the `HOPLODEX_E2E_*` variables (app_dirs.rs), so if
//! the refusal were ever lost the app would start there, and fail this test,
//! never on the developer's data.
#![cfg(feature = "test-kdf")]

use std::fs;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

/// Far longer than a refusal takes; an app that started is killed then.
const STARTED: Duration = Duration::from_secs(30);

#[test]
fn the_app_refuses_to_start_in_a_test_kdf_build() {
    let sandbox = TempDir::new().unwrap();
    let dir = |name: &str| {
        let path = sandbox.path().join(name);
        fs::create_dir(&path).unwrap();
        path
    };
    let (config, cache, documents, home) =
        (dir("config"), dir("cache"), dir("documents"), dir("home"));

    let mut app = Command::new(env!("CARGO_BIN_EXE_hoplodex"))
        .env("HOPLODEX_E2E_CONFIG_HOME", &config)
        .env("HOPLODEX_E2E_CACHE_HOME", &cache)
        .env("HOPLODEX_E2E_DOCUMENTS", &documents)
        .env("HOPLODEX_E2E_HOME", &home)
        // What the OS's own folders come from, where they are read from the
        // environment, in the sandbox too.
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("XDG_CONFIG_HOME", &config)
        .env("XDG_CACHE_HOME", &cache)
        .env("XDG_DATA_HOME", &home)
        .env("APPDATA", &config)
        .env("LOCALAPPDATA", &cache)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(status) = app.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > STARTED {
            let _ = app.kill();
            let _ = app.wait();
            panic!("the app started in a test-kdf build");
        }
        thread::sleep(Duration::from_millis(20));
    };
    let mut stderr = String::new();
    app.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();

    assert_eq!(status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("test-kdf feature"), "{stderr}");
    for folder in [&config, &cache, &documents, &home] {
        assert_eq!(fs::read_dir(folder).unwrap().count(), 0, "{} was written", folder.display());
    }
}
