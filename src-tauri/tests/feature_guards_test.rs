//! The test-only Cargo features that must never reach a shipped build each
//! refuse to compile there with a `compile_error!`: `test-kdf` (`db/cipher.rs`),
//! `upgrade-test-steps` (`db/upgrade.rs`) and `mock-keyring` without `e2e`
//! (`services/keyring.rs`). A shipped build is one with Tauri's
//! `custom-protocol`, which `tauri build` turns on without the app's own
//! feature, so each guard tests tauri-build's `not(dev)`. This checks the
//! library with all three and Tauri's `custom-protocol`, and fails unless
//! every guard stops it.

// shortcut: Linux only, since the guards' cfgs don't depend on the OS and a
// separate `cargo check` per OS would add minutes; run it everywhere if one does.
#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn every_test_only_feature_refuses_to_compile_in_a_shipped_build() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO"))
        .args(["check", "--lib", "--locked", "--message-format", "short", "--features"])
        .arg("tauri/custom-protocol,test-kdf,upgrade-test-steps,mock-keyring")
        .arg("--manifest-path")
        .arg(manifest_dir.join("Cargo.toml"))
        // Its own target folder, so it never waits on the test build's lock.
        .env("CARGO_TARGET_DIR", manifest_dir.join("target").join("feature-guards"))
        .output()
        .expect("cargo runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "a shipped build compiled with the test-only features");
    for feature in ["test-kdf", "upgrade-test-steps", "mock-keyring"] {
        assert!(
            stderr.contains(&format!("the `{feature}` feature is for `cargo test`")),
            "nothing stopped `{feature}` in a shipped build:\n{stderr}"
        );
    }
}
